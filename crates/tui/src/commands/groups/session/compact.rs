//! `/compact` command.

use super::CommandResult;

pub(in crate::commands) struct CompactCmd;

// ---------------------------------------------------------------------------
// FEAT-023 Phase 4 (D3/D6): portable pure registration. `/compact` parses and
// normalizes its focus argument and emits the existing receipt + action with
// no host context bundle (the baseline `App` parameter is unused).
// ---------------------------------------------------------------------------

use codewhale_command_contract::handler::CommandHandler;
use codewhale_command_contract::metadata::{
    CommandInfo as ContractInfo, RegisterCommand as ContractRegisterCommand,
};

use crate::tui::app::AppAction;

pub(in crate::commands) const CONTRACT_INFO: ContractInfo = ContractInfo {
    name: "compact",
    aliases: &["yasuo"],
    usage: "/compact [focus]",
    description_key: "cmd_compact_description",
};

impl ContractRegisterCommand<CommandResult> for CompactCmd {
    fn info() -> &'static ContractInfo {
        &CONTRACT_INFO
    }

    fn handler() -> CommandHandler<CommandResult> {
        CommandHandler::Pure(compact_pure)
    }
}

/// Pure `/compact` — byte-identical to the baseline `session::compact`.
pub(in crate::commands) fn compact_pure(arg: Option<&str>) -> CommandResult {
    let focus = arg
        .map(str::trim)
        .filter(|focus| !focus.is_empty())
        .map(str::to_string);
    let receipt = match focus.as_deref() {
        Some(focus) => format!("Making room (focus: {focus})…"),
        None => "Making room…".to_string(),
    };
    CommandResult::with_message_and_action(receipt, AppAction::CompactContext { focus })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::app::AppAction;

    #[test]
    fn pure_compact_matches_baseline_receipts() {
        let none = compact_pure(None);
        assert_eq!(none.message.as_deref(), Some("Making room…"));
        assert!(matches!(
            none.action,
            Some(AppAction::CompactContext { focus: None })
        ));
        assert!(!none.is_error);

        let blank = compact_pure(Some("   "));
        assert!(matches!(
            blank.action,
            Some(AppAction::CompactContext { focus: None })
        ));

        let focus = compact_pure(Some("  the auth refactor  "));
        assert_eq!(
            focus.message.as_deref(),
            Some("Making room (focus: the auth refactor)…")
        );
        assert!(matches!(
            focus.action,
            Some(AppAction::CompactContext { focus: Some(ref f) }) if f == "the auth refactor"
        ));
    }
}
