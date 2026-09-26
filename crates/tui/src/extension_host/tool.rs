//! `HostToolSpec`: an extension tool as an ordinary registry `ToolSpec`.
//!
//! Because it is a registry tool, every existing gate applies unchanged:
//! plan mode, the authority envelope, deferral, hooks, approval, and code
//! mode (which, on main, refuses it as mutating/needs-approval before any
//! host call). Two rules are specific to extension tools:
//!
//! * **Always `ApprovalRequirement::Required`.** A plugin's own read-only
//!   hint (`presentCall` `kind: 'read'`, MCP-style annotations) is display
//!   data at most. Honouring it would let a plugin switch approval off for a
//!   tool whose body runs arbitrary Node — self-approval.
//! * **Liveness is re-checked at call time**: the plugin's reviewed receipt,
//!   the Native adapter in this build's policy, and the exact owner
//!   generation. A revocation mid-turn fails the call closed.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;

use super::ManagerShared;
use super::protocol::{ContentBlockWire, CoreRequest, ToolCallParams, ToolResultWire};
use super::registry::ToolRegistration;
use super::supervisor::{HostCallError, HostProcess};
use crate::tools::spec::{
    ApprovalRequirement, PreparedToolCall, ToolCapability, ToolContext, ToolError, ToolResult,
    ToolSpec,
};

/// Default per-call deadline, matching script tools.
pub const TOOL_CALL_DEADLINE: Duration = Duration::from_secs(120);

pub(crate) struct HostToolSpec {
    registration: ToolRegistration,
    manager: Arc<ManagerShared>,
}

impl HostToolSpec {
    pub(crate) fn new(registration: ToolRegistration, manager: Arc<ManagerShared>) -> Self {
        Self {
            registration,
            manager,
        }
    }

    /// `extension:<plugin>`: the origin shown in approval cards and diagnostics.
    #[must_use]
    pub fn origin(&self) -> String {
        format!("extension:{}", self.registration.plugin_name)
    }

    /// The approval-card text. Rust composes it; the extension supplies none.
    /// Plugins in one host share a process and can interfere with each other,
    /// so the card says when this one is not alone (design §4.4, threat 3).
    #[must_use]
    pub fn approval_text(&self) -> String {
        let others = self
            .manager
            .registry
            .lock()
            .expect("registry lock")
            .other_active_owners(&self.registration.owner.plugin_id);
        let sharing = match others {
            0 => String::new(),
            1 => "; it shares one host process with 1 other plugin, which can alter its behaviour"
                .to_string(),
            n => format!(
                "; it shares one host process with {n} other plugins, which can alter its behaviour"
            ),
        };
        format!(
            "Extension tool `{}` from plugin `{}` ({}) runs JavaScript on this computer with the extension host's permissions{sharing}",
            self.registration.name,
            self.registration.plugin_name,
            self.origin()
        )
    }
}

impl std::fmt::Debug for HostToolSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostToolSpec")
            .field("name", &self.registration.name)
            .field("plugin", &self.registration.plugin_name)
            .field("handle", &self.registration.handle)
            .finish()
    }
}

/// Sends `$/cancel` if the call future is dropped before it resolves (turn
/// interrupt, deadline, or the engine abandoning the call).
struct CancelOnDrop {
    host: Arc<HostProcess>,
    id: u64,
    armed: bool,
}

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        if self.armed {
            self.host.cancel(self.id);
            self.host.forget(self.id);
        }
    }
}

fn map_call_error(tool: &str, error: HostCallError) -> ToolError {
    match error {
        HostCallError::Cancelled(reason) => ToolError::Cancelled {
            message: format!("extension tool `{tool}`: {reason}"),
        },
        HostCallError::Timeout(after) => ToolError::Timeout {
            seconds: after.as_secs(),
        },
        HostCallError::Exited(reason) => {
            ToolError::not_available(format!("extension host exited: {reason}"))
        }
        HostCallError::Busy => {
            ToolError::not_available("extension host is busy; try again".to_string())
        }
        HostCallError::Rpc { code, message } => {
            if code == super::protocol::error_code::NOT_AVAILABLE {
                ToolError::not_available(format!("extension tool `{tool}`: {message}"))
            } else {
                ToolError::execution_failed(format!("extension tool `{tool}` failed: {message}"))
            }
        }
    }
}

pub(crate) fn wire_to_result(wire: ToolResultWire, origin: &str) -> ToolResult {
    let content = wire
        .content
        .iter()
        .map(|block| match block {
            ContentBlockWire::Text { text } => text.as_str(),
        })
        .collect::<Vec<_>>()
        .join("\n");
    let mut metadata = serde_json::Map::new();
    metadata.insert("origin".to_string(), Value::String(origin.to_string()));
    ToolResult {
        content,
        success: !wire.is_error,
        metadata: Some(Value::Object(metadata)),
    }
}

#[async_trait]
impl ToolSpec for HostToolSpec {
    fn name(&self) -> &str {
        &self.registration.name
    }

    fn registration_origin(&self) -> std::borrow::Cow<'_, str> {
        self.origin().into()
    }

    fn description(&self) -> &str {
        &self.registration.description
    }

    fn input_schema(&self) -> Value {
        self.registration.input_schema.clone()
    }

    fn capabilities(&self) -> Vec<ToolCapability> {
        vec![
            ToolCapability::ExecutesCode,
            ToolCapability::RequiresApproval,
        ]
    }

    fn approval_requirement(&self) -> ApprovalRequirement {
        ApprovalRequirement::Required
    }

    fn approval_requirement_for(&self, _input: &Value) -> ApprovalRequirement {
        ApprovalRequirement::Required
    }

    fn is_read_only(&self) -> bool {
        false
    }

    fn is_read_only_for(&self, _input: &Value) -> bool {
        false
    }

    fn defer_loading(&self) -> bool {
        true
    }

    fn prepare(&self, input: Value, _context: &ToolContext) -> Result<PreparedToolCall, ToolError> {
        Ok(PreparedToolCall {
            name: self.registration.name.clone(),
            // Rust composes the card; the extension cannot supply approval text.
            description: self.approval_text(),
            read_only: false,
            supports_parallel: false,
            starts_detached: false,
            approval: ApprovalRequirement::Required,
            resources: vec![crate::tools::spec::ResourceClaim::GlobalExclusive],
            input,
        })
    }

    async fn execute(&self, input: Value, context: &ToolContext) -> Result<ToolResult, ToolError> {
        let registration = &self.registration;
        let host = self
            .manager
            .live_host_for(registration)
            .await
            .map_err(ToolError::not_available)?;
        let call_id = context
            .execution
            .owner_agent_id
            .clone()
            .map(|agent| format!("{agent}:{}", uuid::Uuid::new_v4().simple()))
            .unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string());
        let request = CoreRequest::ToolCall(ToolCallParams {
            handle: registration.handle,
            call_id,
            input,
            deadline_ms: TOOL_CALL_DEADLINE.as_millis() as u64,
        });
        let (id, rx) = host
            .start_request(request, Some(registration.owner.plugin_id.clone()))
            .map_err(|error| map_call_error(&registration.name, error))?;
        let mut guard = CancelOnDrop {
            host: Arc::clone(&host),
            id,
            armed: true,
        };
        let outcome = match tokio::time::timeout(TOOL_CALL_DEADLINE, rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(HostCallError::Exited("channel closed".to_string())),
            Err(_) => Err(HostCallError::Timeout(TOOL_CALL_DEADLINE)),
        };
        // A timeout leaves the guard armed so the host is told to stop.
        if !matches!(outcome, Err(HostCallError::Timeout(_))) {
            guard.armed = false;
        }
        drop(guard);
        let value = outcome.map_err(|error| map_call_error(&registration.name, error))?;
        let wire: ToolResultWire = serde_json::from_value(value).map_err(|error| {
            ToolError::execution_failed(format!(
                "extension tool `{}` returned a malformed result: {error}",
                registration.name
            ))
        })?;
        Ok(wire_to_result(wire, &self.origin()))
    }
}
