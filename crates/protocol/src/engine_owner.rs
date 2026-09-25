//! Safe, ordered Engine owner projection shared by desktop and web hosts.
//!
//! The Engine owns operation and turn facts: it emits
//! `operation_activity_started` / `operation_activity_completed` with an
//! [`OwnerActivityKind`] and [`OwnerOperationOutcome`], never a tool name,
//! argument, command or result. Apps may bind this session-scoped projection
//! to an authenticated account and serve it to another client; the account
//! identity is intentionally not supplied by the Engine.
//!
//! Known limits:
//! - Operation events only carry `Reading`, `Editing`, `Searching`,
//!   `Testing`, `Executing`, `Browsing`, `Computer`, `Memory` or `Tool`.
//!   `Thinking`, `Responding` and `Delegating` are derived by the reducer
//!   from message, reasoning and agent lifecycle events.
//! - [`EngineOwnerProjection`] itself is computed today by the pet reducer
//!   (`pet/src/core/pet-engine.ts`, bundled into the TUI pet worker), and
//!   validated here on the way back. No Rust producer exists yet; a host
//!   that needs it outside the pet must decide whether the runtime produces
//!   it in Rust rather than adding a second reducer.
//! - Code-mode (`execute_tools`) nested calls do not report activity yet.

use serde::{Deserialize, Serialize};

use crate::event_msg::TurnOutcomeStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnerActivityKind {
    Reading,
    Editing,
    Searching,
    Testing,
    Executing,
    Browsing,
    Computer,
    Memory,
    Tool,
    Thinking,
    Responding,
    Delegating,
}

impl OwnerActivityKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Reading => "reading",
            Self::Editing => "editing",
            Self::Searching => "searching",
            Self::Testing => "testing",
            Self::Executing => "executing",
            Self::Browsing => "browsing",
            Self::Computer => "computer",
            Self::Memory => "memory",
            Self::Tool => "tool",
            Self::Thinking => "thinking",
            Self::Responding => "responding",
            Self::Delegating => "delegating",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnerOperationOutcome {
    Succeeded,
    Failed,
    Cancelled,
    Denied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnerPresence {
    Unknown,
    Working,
    NeedsYou,
    Done,
    Idle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnerFreshness {
    Missing,
    Fresh,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OwnerActiveSpan {
    pub activity_kind: OwnerActivityKind,
    pub started_at_ms: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OwnerFailedToolAge {
    pub activity_kind: OwnerActivityKind,
    pub age_ms: f64,
}

/// Ordered, account-neutral read model. `cursor` is monotonic within the
/// existing session owner. The Apps control plane supplies account isolation
/// by authenticating the caller and resolving the requested session there.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EngineOwnerProjection {
    pub schema_version: u8,
    pub session_id: Option<String>,
    pub cursor: u64,
    pub observed: bool,
    pub freshness: OwnerFreshness,
    pub authoritative_presence: OwnerPresence,
    pub activity_kind: Option<OwnerActivityKind>,
    pub observed_at_ms: Option<f64>,
    pub parallel_agent_count: u32,
    pub active_spans: Vec<OwnerActiveSpan>,
    pub turn_id: Option<String>,
    pub turn_outcome: Option<TurnOutcomeStatus>,
    pub done_effect_id: Option<String>,
    pub failed_tool_age: Option<OwnerFailedToolAge>,
}

impl EngineOwnerProjection {
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.schema_version == 1
            && self.session_id.as_ref().is_none_or(|id| {
                !id.is_empty() && id.len() <= 256 && id.bytes().all(|byte| !byte.is_ascii_control())
            })
            && self.turn_id.as_ref().is_none_or(|id| {
                !id.is_empty() && id.len() <= 256 && id.bytes().all(|byte| !byte.is_ascii_control())
            })
            && self.active_spans.len() <= 4
            && self.parallel_agent_count <= 100_000
            && self
                .observed_at_ms
                .is_none_or(|time| time.is_finite() && time >= 0.0)
            && self.active_spans.iter().all(|span| {
                span.started_at_ms.is_finite()
                    && span.started_at_ms >= 0.0
                    && self
                        .observed_at_ms
                        .is_none_or(|observed_at| span.started_at_ms <= observed_at)
            })
            && self
                .failed_tool_age
                .as_ref()
                .is_none_or(|failure| failure.age_ms.is_finite() && failure.age_ms >= 0.0)
            && (self.turn_outcome.is_none() || self.turn_id.is_some())
            && match self.freshness {
                OwnerFreshness::Missing => {
                    !self.observed
                        && self.observed_at_ms.is_none()
                        && self.activity_kind.is_none()
                        && self.active_spans.is_empty()
                        && self.parallel_agent_count == 0
                        && self.authoritative_presence == OwnerPresence::Unknown
                        && self.turn_id.is_none()
                        && self.turn_outcome.is_none()
                        && self.failed_tool_age.is_none()
                        && self.done_effect_id.is_none()
                }
                OwnerFreshness::Stale => {
                    self.observed
                        && self.session_id.is_some()
                        && self.observed_at_ms.is_some()
                        && self.activity_kind.is_none()
                        && self.active_spans.is_empty()
                        && self.parallel_agent_count == 0
                        && self.authoritative_presence == OwnerPresence::Unknown
                        && self.failed_tool_age.is_none()
                        && self.done_effect_id.is_none()
                }
                OwnerFreshness::Fresh => {
                    self.observed
                        && self.session_id.is_some()
                        && self.observed_at_ms.is_some()
                        && (self.authoritative_presence != OwnerPresence::Done
                            || (self.turn_outcome == Some(TurnOutcomeStatus::Completed)
                                && self.turn_id.is_some()
                                && self.done_effect_id == self.turn_id))
                        && (self.done_effect_id.is_none()
                            || self.authoritative_presence == OwnerPresence::Done)
                }
            }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn projection() -> EngineOwnerProjection {
        EngineOwnerProjection {
            schema_version: 1,
            session_id: Some("session:opaque".into()),
            cursor: 4,
            observed: true,
            freshness: OwnerFreshness::Fresh,
            authoritative_presence: OwnerPresence::Working,
            activity_kind: Some(OwnerActivityKind::Reading),
            observed_at_ms: Some(100.0),
            parallel_agent_count: 0,
            active_spans: vec![OwnerActiveSpan {
                activity_kind: OwnerActivityKind::Reading,
                started_at_ms: 50.0,
            }],
            turn_id: Some("turn-1".into()),
            turn_outcome: None,
            done_effect_id: None,
            failed_tool_age: None,
        }
    }

    #[test]
    fn projection_accepts_trusted_scoped_activity_and_rejects_bad_clocks() {
        assert!(projection().is_valid());

        let mut future_span = projection();
        future_span.active_spans[0].started_at_ms = 101.0;
        assert!(!future_span.is_valid());

        let mut too_many = projection();
        too_many.active_spans = vec![too_many.active_spans[0].clone(); 5];
        assert!(!too_many.is_valid());
    }

    #[test]
    fn projection_needs_authoritative_presence_for_wait_and_completed_turn_for_done() {
        let mut waiting = projection();
        waiting.authoritative_presence = OwnerPresence::NeedsYou;
        assert!(waiting.is_valid());

        let mut done = projection();
        done.authoritative_presence = OwnerPresence::Done;
        done.activity_kind = None;
        done.active_spans.clear();
        done.turn_outcome = Some(TurnOutcomeStatus::Completed);
        done.done_effect_id = done.turn_id.clone();
        assert!(done.is_valid());

        done.turn_outcome = Some(TurnOutcomeStatus::Interrupted);
        assert!(!done.is_valid());
        done.turn_outcome = Some(TurnOutcomeStatus::Failed);
        assert!(!done.is_valid());
    }

    #[test]
    fn missing_and_stale_projection_cannot_claim_specific_activity() {
        let missing = EngineOwnerProjection {
            schema_version: 1,
            session_id: None,
            cursor: 0,
            observed: false,
            freshness: OwnerFreshness::Missing,
            authoritative_presence: OwnerPresence::Unknown,
            activity_kind: None,
            observed_at_ms: None,
            parallel_agent_count: 0,
            active_spans: Vec::new(),
            turn_id: None,
            turn_outcome: None,
            done_effect_id: None,
            failed_tool_age: None,
        };
        assert!(missing.is_valid());

        let mut stale = projection();
        stale.freshness = OwnerFreshness::Stale;
        stale.authoritative_presence = OwnerPresence::Unknown;
        stale.activity_kind = None;
        stale.parallel_agent_count = 0;
        stale.active_spans.clear();
        stale.failed_tool_age = None;
        stale.done_effect_id = None;
        assert!(stale.is_valid());

        stale.activity_kind = Some(OwnerActivityKind::Reading);
        assert!(!stale.is_valid());
    }
}
