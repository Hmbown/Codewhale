//! Shared command result data. Hosts consume actions after command dispatch;
//! constructing an outcome never executes an action.

use crate::facets::SessionSyncPayload;

#[derive(Debug, Clone, PartialEq)]
pub struct CommandResult<A> {
    pub message: Option<String>,
    pub action: Option<A>,
    pub is_error: bool,
}

impl<A> CommandResult<A> {
    pub fn ok() -> Self {
        Self {
            message: None,
            action: None,
            is_error: false,
        }
    }
    pub fn message(msg: impl Into<String>) -> Self {
        Self {
            message: Some(msg.into()),
            action: None,
            is_error: false,
        }
    }
    pub fn action(action: A) -> Self {
        Self {
            message: None,
            action: Some(action),
            is_error: false,
        }
    }
    pub fn with_message_and_action(msg: impl Into<String>, action: A) -> Self {
        Self {
            message: Some(msg.into()),
            action: Some(action),
            is_error: false,
        }
    }
    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            message: Some(format!("Error: {}", msg.into())),
            action: None,
            is_error: true,
        }
    }
}

/// Complete debug-group action vocabulary. This replaces its TUI AppAction
/// references; other command groups' action ownership remains staged.
#[derive(Debug, Clone, PartialEq)]
pub enum DebugAction {
    FetchBalance,
    CacheWarmup,
    PreviewOutboundRequest {
        json: bool,
        base_prompt_only: bool,
        hypothetical_prompt: Option<String>,
    },
    OpenTextPager {
        title: String,
        content: String,
    },
    OpenContextInspector,
    SendMessage(String),
    SyncSession(SessionSyncPayload),
    /// Install and persist a conversation rollback; `/retry` resends its
    /// input only after both succeed, replacing the removed exchange (#6788).
    ConversationUndo {
        sync: SessionSyncPayload,
        retry_input: Option<String>,
    },
}

pub type DebugCommandResult = CommandResult<DebugAction>;

/// Structural copy cannot request a host action.
pub type StructcopyCommandResult = CommandResult<std::convert::Infallible>;

/// Complete session-group vocabulary. Hosts execute these requests after dispatch.
#[derive(Debug, Clone, PartialEq)]
pub enum SessionAction {
    CompactContext { focus: Option<String> },
    PurgeContext,
    LoadSession(std::path::PathBuf),
    SyncSession(SessionSyncPayload),
    SendMessage(String),
    RemoteControl(SessionRemoteControlAction),
    OpenExternalUrl { url: String, label: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionRemoteControlAction {
    Start,
    Stop,
}

pub type SessionCommandResult = CommandResult<SessionAction>;

/// Only permission removal can request a host action in the config policy slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigPolicyAction {
    PermissionRulesChanged,
}
pub type ConfigPolicyCommandResult = CommandResult<ConfigPolicyAction>;
pub type ConfigStatusCommandResult = CommandResult<std::convert::Infallible>;
