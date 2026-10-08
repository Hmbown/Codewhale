//! `/purge` command.

use super::CommandResult;

pub(in crate::commands) struct PurgeCmd;

// ---------------------------------------------------------------------------
// FEAT-023 Phase 4 (D3/D6): portable pure registration. `/purge` emits the
// existing receipt + action with no host context bundle; any argument is
// ignored exactly like the baseline.
// ---------------------------------------------------------------------------

use codewhale_command_contract::handler::CommandHandler;
use codewhale_command_contract::metadata::{
    CommandInfo as ContractInfo, RegisterCommand as ContractRegisterCommand,
};

use codewhale_command_contract::outcome::SessionAction;

pub(in crate::commands) const CONTRACT_INFO: ContractInfo = ContractInfo {
    name: "purge",
    aliases: &["qingchu"],
    usage: "/purge",
    description_key: "cmd_purge_description",
};

impl ContractRegisterCommand<CommandResult> for PurgeCmd {
    fn info() -> &'static ContractInfo {
        &CONTRACT_INFO
    }

    fn handler() -> CommandHandler<CommandResult> {
        CommandHandler::Pure(purge_pure)
    }
}

/// Pure `/purge` — byte-identical to the baseline `session::purge`.
pub(in crate::commands) fn purge_pure(_arg: Option<&str>) -> CommandResult {
    CommandResult::with_message_and_action(
        "Agent context purge triggered...".to_string(),
        SessionAction::PurgeContext,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use codewhale_command_contract::outcome::SessionAction;

    #[test]
    fn pure_purge_matches_baseline_receipt() {
        let result = purge_pure(Some("ignored"));
        assert_eq!(
            result.message.as_deref(),
            Some("Agent context purge triggered...")
        );
        assert!(matches!(result.action, Some(SessionAction::PurgeContext)));
        assert!(!result.is_error);
    }
}
