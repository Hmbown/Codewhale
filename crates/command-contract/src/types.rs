//! Prototype boundary values used by the command capability shapes.
//!
//! These types deliberately do not replace the current TUI-owned production
//! types in FEAT-014. During the in-place adoption stage, thin TUI adapters
//! convert between existing application values and these boundary values.

/// Stable provider identity at the command boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandProviderId(pub String);

// Slice 4 removed `CommandReasoningEffort`: it was a verbatim 9-variant copy of
// `codewhale_tui::reasoning_preference::ReasoningEffort` with no independent
// behavior and no consumer, so it had no place on the command boundary.

/// Application mode visible to commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandMode {
    Agent,
    Plan,
    Operate,
}

/// Tool-approval posture visible to commands.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum CommandApprovalMode {
    Auto,
    Bypass,
    #[default]
    Suggest,
    Never,
}

/// Cost currency used by command-facing accounting operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandCurrency {
    Usd,
    Cny,
}
