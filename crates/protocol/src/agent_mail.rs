//! Canonical protocol contract for durable communication between Codewhale tasks.
//!
//! Agent Mail is distinct from same-session subagent control messages. An envelope
//! is persisted by the runtime, scoped to an owner and workspace, and projected
//! into a destination turn only at an explicit safe boundary. The summary is the
//! complete model-visible payload: runtimes must sanitize it before constructing
//! an envelope, and this module enforces the wire-size and control-character
//! boundary.

use std::error::Error;
use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use uuid::Uuid;

pub const AGENT_MAIL_SCHEMA_VERSION: u32 = 1;

pub const AGENT_MAIL_EVENT_QUEUED: &str = "agent_mail.queued";
pub const AGENT_MAIL_EVENT_DELIVERING: &str = "agent_mail.delivering";
pub const AGENT_MAIL_EVENT_DELIVERED: &str = "agent_mail.delivered";
pub const AGENT_MAIL_EVENT_READ: &str = "agent_mail.read";
pub const AGENT_MAIL_EVENT_DELIVERY_FAILED: &str = "agent_mail.delivery_failed";
pub const AGENT_MAIL_EVENT_CANCELED: &str = "agent_mail.canceled";

pub const MAX_AGENT_MAIL_MESSAGE_ID_BYTES: usize = 80;
pub const MAX_AGENT_MAIL_OPAQUE_ID_BYTES: usize = 128;
pub const MAX_AGENT_MAIL_DISPLAY_LABEL_BYTES: usize = 64;
pub const MAX_AGENT_MAIL_SUMMARY_BYTES: usize = 2_048;
pub const MAX_AGENT_MAIL_EVIDENCE_REFS: usize = 8;
pub const MAX_AGENT_MAIL_EVIDENCE_LABEL_BYTES: usize = 96;
pub const MAX_AGENT_MAIL_HOPS: u8 = 4;
pub const MAX_AGENT_MAIL_DELIVERY_ATTEMPTS: u8 = 8;
pub const MAX_AGENT_MAIL_FAILURE_MESSAGE_BYTES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentMailValidationError {
    pub field: &'static str,
    pub message: String,
}

impl AgentMailValidationError {
    fn new(field: &'static str, message: impl Into<String>) -> Self {
        Self {
            field,
            message: message.into(),
        }
    }
}

impl fmt::Display for AgentMailValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid Agent Mail {}: {}", self.field, self.message)
    }
}

impl Error for AgentMailValidationError {}

/// Stable, caller-supplied idempotency key for an Agent Mail envelope.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct AgentMailMessageId(String);

impl AgentMailMessageId {
    #[must_use]
    pub fn new() -> Self {
        Self(format!("mail_{}", Uuid::new_v4().simple()))
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, AgentMailValidationError> {
        let value = value.into();
        validate_message_id(&value)?;
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl Default for AgentMailMessageId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for AgentMailMessageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for AgentMailMessageId {
    type Error = AgentMailValidationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl TryFrom<&str> for AgentMailMessageId {
    type Error = AgentMailValidationError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl<'de> Deserialize<'de> for AgentMailMessageId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(value).map_err(serde::de::Error::custom)
    }
}

/// Durable ownership and routing scope resolved by the receiving runtime.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentMailAddress {
    pub owner_id: String,
    pub workspace_id: String,
    pub thread_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

impl AgentMailAddress {
    pub fn validate(&self) -> Result<(), AgentMailValidationError> {
        validate_opaque_id("address.owner_id", &self.owner_id)?;
        validate_opaque_id("address.workspace_id", &self.workspace_id)?;
        validate_opaque_id("address.thread_id", &self.thread_id)?;
        if let Some(task_id) = &self.task_id {
            validate_opaque_id("address.task_id", task_id)?;
        }
        if let Some(session_id) = &self.session_id {
            validate_opaque_id("address.session_id", session_id)?;
        }
        if self.task_id.is_none() && self.session_id.is_none() {
            return Err(AgentMailValidationError::new(
                "address",
                "task_id or session_id is required",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentMailSender {
    /// Runtime-authorized stable identity; never a free-form transcript name.
    pub identity: String,
    pub display_label: String,
}

impl AgentMailSender {
    pub fn validate(&self) -> Result<(), AgentMailValidationError> {
        validate_opaque_id("sender.identity", &self.identity)?;
        validate_bounded_text(
            "sender.display_label",
            &self.display_label,
            MAX_AGENT_MAIL_DISPLAY_LABEL_BYTES,
            false,
        )
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentMailDeliveryMode {
    QueueOnly,
    WakeAtSafeBoundary,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentMailEvidenceKind {
    RuntimeEvent,
    TurnItem,
    ArtifactReceipt,
}

/// Bounded pointer to evidence already authorized by the destination runtime.
///
/// `reference_id` is deliberately opaque: paths and URLs are not valid evidence
/// references and must not be smuggled through this contract.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentMailEvidenceRef {
    pub kind: AgentMailEvidenceKind,
    pub reference_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

impl AgentMailEvidenceRef {
    pub fn validate(&self) -> Result<(), AgentMailValidationError> {
        validate_opaque_id("evidence.reference_id", &self.reference_id)?;
        if let Some(label) = &self.label {
            validate_bounded_text(
                "evidence.label",
                label,
                MAX_AGENT_MAIL_EVIDENCE_LABEL_BYTES,
                false,
            )?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentMailStatus {
    Queued,
    Delivering,
    Delivered,
    Read,
    Failed,
    /// Explicitly withdrawn while queued (#6176). Terminal: delivery and the
    /// wake pump never claim it, and re-cancel is an idempotent no-op.
    Canceled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentMailFailureCode {
    AuthorizationDenied,
    DestinationUnavailable,
    DeliveryRejected,
    Persistence,
    AttemptLimit,
    InvalidEnvelope,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentMailFailureReceipt {
    pub code: AgentMailFailureCode,
    pub message: String,
    pub retryable: bool,
    pub failed_at: DateTime<Utc>,
}

impl AgentMailFailureReceipt {
    pub fn validate(&self) -> Result<(), AgentMailValidationError> {
        validate_bounded_text(
            "failure.message",
            &self.message,
            MAX_AGENT_MAIL_FAILURE_MESSAGE_BYTES,
            false,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentMailEnvelope {
    #[serde(default = "default_agent_mail_schema_version")]
    pub schema_version: u32,
    pub message_id: AgentMailMessageId,
    pub source: AgentMailAddress,
    pub destination: AgentMailAddress,
    pub sender: AgentMailSender,
    /// Sanitized, bounded content presented to the destination task and UI.
    pub summary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<AgentMailEvidenceRef>,
    pub delivery_mode: AgentMailDeliveryMode,
    /// Explicit loop-breaking decision. It must agree with `delivery_mode`.
    pub trigger_turn: bool,
    pub hop_count: u8,
    pub status: AgentMailStatus,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivered_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub attempt_count: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<AgentMailFailureReceipt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivery_turn_id: Option<String>,
}

impl AgentMailEnvelope {
    pub fn validate(&self) -> Result<(), AgentMailValidationError> {
        if self.schema_version != AGENT_MAIL_SCHEMA_VERSION {
            return Err(AgentMailValidationError::new(
                "schema_version",
                format!("expected {AGENT_MAIL_SCHEMA_VERSION}"),
            ));
        }
        self.source.validate()?;
        self.destination.validate()?;
        self.sender.validate()?;
        validate_summary_and_delivery(
            &self.summary,
            &self.evidence,
            self.delivery_mode,
            self.trigger_turn,
            self.hop_count,
        )?;
        if self.attempt_count > MAX_AGENT_MAIL_DELIVERY_ATTEMPTS {
            return Err(AgentMailValidationError::new(
                "attempt_count",
                format!("must be at most {MAX_AGENT_MAIL_DELIVERY_ATTEMPTS}"),
            ));
        }
        if let Some(turn_id) = &self.delivery_turn_id {
            validate_opaque_id("delivery_turn_id", turn_id)?;
        }

        match self.status {
            AgentMailStatus::Queued => {
                require_absent(self.delivered_at.is_some(), "delivered_at", "queued")?;
                require_absent(self.read_at.is_some(), "read_at", "queued")?;
                require_absent(self.failure.is_some(), "failure", "queued")?;
                require_absent(
                    self.delivery_turn_id.is_some(),
                    "delivery_turn_id",
                    "queued",
                )?;
            }
            AgentMailStatus::Delivering => {
                if self.attempt_count == 0 {
                    return Err(AgentMailValidationError::new(
                        "attempt_count",
                        "delivering mail requires at least one attempt",
                    ));
                }
                require_absent(self.delivered_at.is_some(), "delivered_at", "delivering")?;
                require_absent(self.read_at.is_some(), "read_at", "delivering")?;
                require_absent(self.failure.is_some(), "failure", "delivering")?;
            }
            AgentMailStatus::Delivered => self.validate_delivered(false)?,
            AgentMailStatus::Read => self.validate_delivered(true)?,
            AgentMailStatus::Canceled => {
                // Canceled mail never started delivery: same absences as
                // queued. Attempts stay 0 — cancel is only accepted while
                // queued, before any delivery claim.
                require_absent(self.delivered_at.is_some(), "delivered_at", "canceled")?;
                require_absent(self.read_at.is_some(), "read_at", "canceled")?;
                require_absent(self.failure.is_some(), "failure", "canceled")?;
                require_absent(
                    self.delivery_turn_id.is_some(),
                    "delivery_turn_id",
                    "canceled",
                )?;
            }
            AgentMailStatus::Failed => {
                if self.attempt_count == 0 {
                    return Err(AgentMailValidationError::new(
                        "attempt_count",
                        "failed mail requires at least one attempt",
                    ));
                }
                let failure = self.failure.as_ref().ok_or_else(|| {
                    AgentMailValidationError::new("failure", "failed mail requires a receipt")
                })?;
                failure.validate()?;
                if failure.failed_at < self.created_at {
                    return Err(AgentMailValidationError::new(
                        "failure.failed_at",
                        "cannot precede created_at",
                    ));
                }
                require_absent(self.delivered_at.is_some(), "delivered_at", "failed")?;
                require_absent(self.read_at.is_some(), "read_at", "failed")?;
            }
        }
        Ok(())
    }

    fn validate_delivered(&self, read: bool) -> Result<(), AgentMailValidationError> {
        let delivered_at = self.delivered_at.as_ref().ok_or_else(|| {
            AgentMailValidationError::new("delivered_at", "delivered mail requires a timestamp")
        })?;
        if delivered_at < &self.created_at {
            return Err(AgentMailValidationError::new(
                "delivered_at",
                "cannot precede created_at",
            ));
        }
        if self.delivery_turn_id.is_none() {
            return Err(AgentMailValidationError::new(
                "delivery_turn_id",
                "delivered mail requires a destination turn",
            ));
        }
        if self.attempt_count == 0 {
            return Err(AgentMailValidationError::new(
                "attempt_count",
                "delivered mail requires at least one attempt",
            ));
        }
        require_absent(self.failure.is_some(), "failure", "delivered")?;
        match (read, self.read_at.as_ref()) {
            (true, Some(read_at)) if read_at >= delivered_at => Ok(()),
            (true, Some(_)) => Err(AgentMailValidationError::new(
                "read_at",
                "cannot precede delivered_at",
            )),
            (true, None) => Err(AgentMailValidationError::new(
                "read_at",
                "read mail requires a timestamp",
            )),
            (false, Some(_)) => Err(AgentMailValidationError::new(
                "read_at",
                "delivered mail cannot have read_at before entering read status",
            )),
            (false, None) => Ok(()),
        }
    }

    /// Compares only immutable delivery intent, ignoring lifecycle state.
    #[must_use]
    pub fn is_idempotent_replay_of(&self, other: &Self) -> bool {
        self.schema_version == other.schema_version
            && self.message_id == other.message_id
            && self.source == other.source
            && self.destination == other.destination
            && self.sender == other.sender
            && self.summary == other.summary
            && self.evidence == other.evidence
            && self.delivery_mode == other.delivery_mode
            && self.trigger_turn == other.trigger_turn
            && self.hop_count == other.hop_count
    }

    /// Checks whether a replayed API request describes this persisted message.
    #[must_use]
    pub fn matches_send_request(&self, request: &AgentMailSendRequest) -> bool {
        self.message_id == request.message_id
            && self.source.thread_id == request.source_thread_id
            && self.destination.thread_id == request.destination_thread_id
            && self.sender == request.sender
            && self.summary == request.summary
            && self.evidence == request.evidence
            && self.delivery_mode == request.delivery_mode
            && self.trigger_turn == request.trigger_turn
            && self.hop_count == request.hop_count
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentMailSendRequest {
    pub message_id: AgentMailMessageId,
    pub source_thread_id: String,
    pub destination_thread_id: String,
    pub sender: AgentMailSender,
    /// Runtime-sanitized before the request becomes a persisted envelope.
    pub summary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<AgentMailEvidenceRef>,
    pub delivery_mode: AgentMailDeliveryMode,
    pub trigger_turn: bool,
    #[serde(default)]
    pub hop_count: u8,
}

impl AgentMailSendRequest {
    pub fn validate(&self) -> Result<(), AgentMailValidationError> {
        validate_opaque_id("source_thread_id", &self.source_thread_id)?;
        validate_opaque_id("destination_thread_id", &self.destination_thread_id)?;
        self.sender.validate()?;
        validate_summary_and_delivery(
            &self.summary,
            &self.evidence,
            self.delivery_mode,
            self.trigger_turn,
            self.hop_count,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentMailSendResponse {
    pub envelope: AgentMailEnvelope,
    /// True when the message id and immutable intent already existed.
    pub idempotent_replay: bool,
}

/// Canonical payload placed in every Agent Mail runtime event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentMailEventPayload {
    pub mail: AgentMailEnvelope,
}

fn default_agent_mail_schema_version() -> u32 {
    AGENT_MAIL_SCHEMA_VERSION
}

fn validate_message_id(value: &str) -> Result<(), AgentMailValidationError> {
    if !value.starts_with("mail_") {
        return Err(AgentMailValidationError::new(
            "message_id",
            "must start with mail_",
        ));
    }
    if value.len() > MAX_AGENT_MAIL_MESSAGE_ID_BYTES {
        return Err(AgentMailValidationError::new(
            "message_id",
            format!("must be at most {MAX_AGENT_MAIL_MESSAGE_ID_BYTES} bytes"),
        ));
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(AgentMailValidationError::new(
            "message_id",
            "contains unsupported characters",
        ));
    }
    if value.len() == "mail_".len() {
        return Err(AgentMailValidationError::new(
            "message_id",
            "requires an id after mail_",
        ));
    }
    Ok(())
}

fn validate_opaque_id(field: &'static str, value: &str) -> Result<(), AgentMailValidationError> {
    if value.is_empty() {
        return Err(AgentMailValidationError::new(field, "must not be empty"));
    }
    if value.len() > MAX_AGENT_MAIL_OPAQUE_ID_BYTES {
        return Err(AgentMailValidationError::new(
            field,
            format!("must be at most {MAX_AGENT_MAIL_OPAQUE_ID_BYTES} bytes"),
        ));
    }
    if value == "."
        || value.contains("..")
        || !value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':' | b'@')
        })
    {
        return Err(AgentMailValidationError::new(
            field,
            "must be an opaque id, not a path or URL",
        ));
    }
    Ok(())
}

fn validate_bounded_text(
    field: &'static str,
    value: &str,
    max_bytes: usize,
    allow_line_breaks: bool,
) -> Result<(), AgentMailValidationError> {
    if value.is_empty() {
        return Err(AgentMailValidationError::new(field, "must not be empty"));
    }
    if value.len() > max_bytes {
        return Err(AgentMailValidationError::new(
            field,
            format!("must be at most {max_bytes} bytes"),
        ));
    }
    if value.trim() != value {
        return Err(AgentMailValidationError::new(
            field,
            "must not have leading or trailing whitespace",
        ));
    }
    let has_forbidden_control = value
        .chars()
        .any(|ch| ch.is_control() && !(allow_line_breaks && matches!(ch, '\n' | '\t')));
    if has_forbidden_control {
        return Err(AgentMailValidationError::new(
            field,
            "contains unsupported control characters",
        ));
    }
    Ok(())
}

fn validate_summary_and_delivery(
    summary: &str,
    evidence: &[AgentMailEvidenceRef],
    delivery_mode: AgentMailDeliveryMode,
    trigger_turn: bool,
    hop_count: u8,
) -> Result<(), AgentMailValidationError> {
    validate_bounded_text("summary", summary, MAX_AGENT_MAIL_SUMMARY_BYTES, true)?;
    if evidence.len() > MAX_AGENT_MAIL_EVIDENCE_REFS {
        return Err(AgentMailValidationError::new(
            "evidence",
            format!("must contain at most {MAX_AGENT_MAIL_EVIDENCE_REFS} references"),
        ));
    }
    for reference in evidence {
        reference.validate()?;
    }
    if hop_count > MAX_AGENT_MAIL_HOPS {
        return Err(AgentMailValidationError::new(
            "hop_count",
            format!("must be at most {MAX_AGENT_MAIL_HOPS}"),
        ));
    }
    let expected_trigger = matches!(delivery_mode, AgentMailDeliveryMode::WakeAtSafeBoundary);
    if trigger_turn != expected_trigger {
        return Err(AgentMailValidationError::new(
            "trigger_turn",
            "must be false for queue_only and true for wake_at_safe_boundary",
        ));
    }
    Ok(())
}

fn require_absent(
    present: bool,
    field: &'static str,
    status: &'static str,
) -> Result<(), AgentMailValidationError> {
    if present {
        return Err(AgentMailValidationError::new(
            field,
            format!("must be absent while status is {status}"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn address(thread_id: &str) -> AgentMailAddress {
        AgentMailAddress {
            owner_id: "acct_local".into(),
            workspace_id: "ws_123".into(),
            thread_id: thread_id.into(),
            task_id: Some(format!("task_{thread_id}")),
            session_id: None,
        }
    }

    fn queued_envelope() -> AgentMailEnvelope {
        AgentMailEnvelope {
            schema_version: AGENT_MAIL_SCHEMA_VERSION,
            message_id: AgentMailMessageId::parse("mail_123").unwrap(),
            source: address("thr_a"),
            destination: address("thr_b"),
            sender: AgentMailSender {
                identity: "agent_a".into(),
                display_label: "Agent A".into(),
            },
            summary: "A bounded handoff".into(),
            evidence: vec![AgentMailEvidenceRef {
                kind: AgentMailEvidenceKind::RuntimeEvent,
                reference_id: "evt_42".into(),
                label: Some("Build receipt".into()),
            }],
            delivery_mode: AgentMailDeliveryMode::QueueOnly,
            trigger_turn: false,
            hop_count: 0,
            status: AgentMailStatus::Queued,
            created_at: Utc::now(),
            delivered_at: None,
            read_at: None,
            attempt_count: 0,
            failure: None,
            delivery_turn_id: None,
        }
    }

    #[test]
    fn queued_envelope_roundtrips_with_canonical_event_names() {
        let envelope = queued_envelope();
        envelope.validate().unwrap();
        let value = serde_json::to_value(AgentMailEventPayload {
            mail: envelope.clone(),
        })
        .unwrap();
        let decoded: AgentMailEventPayload = serde_json::from_value(value).unwrap();
        assert_eq!(decoded.mail, envelope);
        assert_eq!(AGENT_MAIL_EVENT_QUEUED, "agent_mail.queued");
        assert_eq!(
            AGENT_MAIL_EVENT_DELIVERY_FAILED,
            "agent_mail.delivery_failed"
        );
    }

    #[test]
    fn rejects_bounds_controls_paths_and_excess_hops() {
        assert!(AgentMailMessageId::parse("../../secret").is_err());
        let mut envelope = queued_envelope();
        envelope.summary = format!("ok\0{}", "x".repeat(MAX_AGENT_MAIL_SUMMARY_BYTES));
        assert!(envelope.validate().is_err());

        let mut envelope = queued_envelope();
        envelope.evidence[0].reference_id = "/tmp/transcript".into();
        assert!(envelope.validate().is_err());

        let mut envelope = queued_envelope();
        envelope.hop_count = MAX_AGENT_MAIL_HOPS + 1;
        assert!(envelope.validate().is_err());
    }

    #[test]
    fn requires_task_or_session_and_consistent_wake_control() {
        let mut envelope = queued_envelope();
        envelope.destination.task_id = None;
        assert!(envelope.validate().is_err());

        let mut envelope = queued_envelope();
        envelope.trigger_turn = true;
        assert!(envelope.validate().is_err());
        envelope.delivery_mode = AgentMailDeliveryMode::WakeAtSafeBoundary;
        assert!(envelope.validate().is_ok());
    }

    #[test]
    fn lifecycle_fields_are_validated() {
        let mut envelope = queued_envelope();
        envelope.status = AgentMailStatus::Delivered;
        envelope.attempt_count = 1;
        envelope.delivered_at = Some(envelope.created_at);
        envelope.delivery_turn_id = Some("turn_1".into());
        assert!(envelope.validate().is_ok());

        envelope.status = AgentMailStatus::Read;
        assert!(envelope.validate().is_err());
        envelope.read_at = envelope.delivered_at;
        assert!(envelope.validate().is_ok());
    }

    #[test]
    fn canceled_envelope_validates_without_delivery_fields() {
        let mut envelope = queued_envelope();
        envelope.status = AgentMailStatus::Canceled;
        assert!(envelope.validate().is_ok());
        envelope.delivery_turn_id = Some("turn_1".into());
        assert!(envelope.validate().is_err());
        assert_eq!(AGENT_MAIL_EVENT_CANCELED, "agent_mail.canceled");
    }

    #[test]
    fn replay_equivalence_ignores_delivery_state_but_not_intent() {
        let queued = queued_envelope();
        let mut delivered = queued.clone();
        delivered.status = AgentMailStatus::Delivered;
        delivered.attempt_count = 1;
        delivered.created_at += chrono::Duration::seconds(1);
        delivered.delivered_at = Some(delivered.created_at);
        delivered.delivery_turn_id = Some("turn_1".into());
        assert!(queued.is_idempotent_replay_of(&delivered));

        delivered.summary = "Different intent under the same id".into();
        assert!(!queued.is_idempotent_replay_of(&delivered));
    }
}
