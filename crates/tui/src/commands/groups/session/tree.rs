//! `/tree` command — render the session entry journal or linear transcript.

use super::CommandResult;

use codewhale_command_contract::facets::{CommandSessionLifecycleContext, TreeBodyProjection};
use codewhale_command_contract::handler::{CommandContexts, CommandHandler};
use codewhale_command_contract::metadata::{
    CommandInfo as ContractInfo, RegisterCommand as ContractRegisterCommand,
};

pub(in crate::commands) struct TreeCmd;

// ---------------------------------------------------------------------------
// FEAT-023 Phase 4 (D3/D5/D6): portable contextual registration and handler.
// ---------------------------------------------------------------------------

pub(in crate::commands) const CONTRACT_INFO: ContractInfo = ContractInfo {
    name: "tree",
    aliases: &[],
    usage: "/tree [interactive]",
    description_key: "cmd_tree_description",
};

impl ContractRegisterCommand<CommandResult> for TreeCmd {
    fn info() -> &'static ContractInfo {
        &CONTRACT_INFO
    }
    fn handler() -> CommandHandler<CommandResult> {
        CommandHandler::Contextual {
            capabilities:
                codewhale_command_contract::handler::CommandCapabilities::SESSION_LIFECYCLE,
            handler: tree_contextual,
        }
    }
}

pub(in crate::commands) fn tree_contextual(
    contexts: CommandContexts<'_>,
    arg: Option<&str>,
) -> CommandResult {
    let mut parts = contexts.into_parts();
    let Some(lifecycle) = parts.lifecycle.as_deref_mut() else {
        return CommandResult::error(
            "Command capability unavailable: session_lifecycle".to_string(),
        );
    };
    tree_portable(lifecycle, arg)
}

pub(in crate::commands) fn tree_portable(
    lifecycle: &mut dyn CommandSessionLifecycleContext,
    _arg: Option<&str>,
) -> CommandResult {
    match lifecycle.tree_body() {
        Ok(TreeBodyProjection::Journal { rendered }) => {
            let mut out = rendered;
            out.push_str("\nUse `/branch <entry_id>` to branch (moves leaf only, never rewrites history).\n");
            out.push_str("Use `/fork [session_id]` to fork this session at any node.\n");
            CommandResult::message(out)
        }
        Ok(TreeBodyProjection::Linear { rendered }) => {
            let mut out = rendered;
            out.push_str("\nUse `/branch <n>` with entry id after journal is saved.\n");
            CommandResult::message(out)
        }
        Ok(TreeBodyProjection::EmptySession) => CommandResult::message(
            "(empty session — no entries yet)\nSend a message first, then `/tree` will show the entry journal."
                .to_string(),
        ),
        Ok(TreeBodyProjection::NoSession) => CommandResult::message(
            "No active session. Use `/resume` to pick a session, then `/tree` to see its journal."
                .to_string(),
        ),
        Err(error) => CommandResult::error(error),
    }
}
