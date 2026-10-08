//! `/sessions` command — picker UI or housekeeping sub-actions.

use super::CommandResult;

use codewhale_command_contract::facets::CommandSessionLifecycleContext;
use codewhale_command_contract::handler::{CommandContexts, CommandHandler};
use codewhale_command_contract::metadata::{
    CommandInfo as ContractInfo, RegisterCommand as ContractRegisterCommand,
};

pub(in crate::commands) struct SessionsCmd;

// ---------------------------------------------------------------------------
// FEAT-023 Phase 4 (D3/D5/D6): portable contextual registration and handler.
// ---------------------------------------------------------------------------

pub(in crate::commands) const CONTRACT_INFO: ContractInfo = ContractInfo {
    name: "sessions",
    aliases: &[],
    usage: "/sessions [show|open <id>|archive <id>|unarchive <id>|prune <days>]",
    description_key: "cmd_sessions_description",
};

impl ContractRegisterCommand<CommandResult> for SessionsCmd {
    fn info() -> &'static ContractInfo {
        &CONTRACT_INFO
    }
    fn handler() -> CommandHandler<CommandResult> {
        CommandHandler::Contextual {
            capabilities:
                codewhale_command_contract::handler::CommandCapabilities::SESSION_LIFECYCLE,
            handler: sessions_contextual,
        }
    }
}

pub(in crate::commands) fn sessions_contextual(
    contexts: CommandContexts<'_>,
    arg: Option<&str>,
) -> CommandResult {
    let mut parts = contexts.into_parts();
    let Some(lifecycle) = parts.lifecycle.as_deref_mut() else {
        return CommandResult::error(
            "Command capability unavailable: session_lifecycle".to_string(),
        );
    };
    sessions_portable(lifecycle, arg)
}

pub(in crate::commands) fn sessions_portable(
    lifecycle: &mut dyn CommandSessionLifecycleContext,
    arg: Option<&str>,
) -> CommandResult {
    let trimmed = arg.unwrap_or("").trim();
    if trimmed.is_empty() {
        lifecycle.open_picker(None);
        return CommandResult::ok();
    }

    let mut parts = trimmed.split_whitespace();
    let action = parts.next().unwrap_or("").to_ascii_lowercase();
    match action.as_str() {
        "prune" => {
            let days_str = match parts.next() {
                Some(s) => s,
                None => {
                    return CommandResult::error(
                        "usage: /sessions prune <days>   (e.g. `/sessions prune 30` to drop sessions older than 30 days)"
                            .to_string(),
                    );
                }
            };
            let days: u64 = match days_str.parse() {
                Ok(n) if n > 0 => n,
                _ => {
                    return CommandResult::error(format!(
                        "expected a positive integer number of days, got `{days_str}`"
                    ));
                }
            };
            match lifecycle.prune_sessions(days) {
                Ok(0) => CommandResult::message(format!("no sessions older than {days}d to prune")),
                Ok(n) => CommandResult::message(format!(
                    "pruned {n} session{} older than {days}d",
                    if n == 1 { "" } else { "s" }
                )),
                Err(error) => CommandResult::error(error),
            }
        }
        "show" | "list" | "picker" => {
            lifecycle.open_picker(None);
            CommandResult::ok()
        }
        "open" => {
            let Some(session_id) = parts.next().map(str::trim).filter(|id| !id.is_empty()) else {
                return CommandResult::error("usage: /sessions open <session-id>".to_string());
            };
            lifecycle.open_picker(Some(session_id.to_string()));
            CommandResult::ok()
        }
        "archive" | "unarchive" | "restore" => {
            let archived = action == "archive";
            let verb = if archived { "archive" } else { "unarchive" };
            let Some(session_id) = parts.next().map(str::trim).filter(|id| !id.is_empty()) else {
                return CommandResult::error(format!("usage: /sessions {verb} <session-id>"));
            };
            match lifecycle.set_archived(session_id, archived) {
                Ok(receipt) => CommandResult::message(format!(
                    "{} session {} ({})",
                    if archived { "Archived" } else { "Restored" },
                    receipt.truncated_id,
                    receipt.title
                )),
                Err(error) => CommandResult::error(error),
            }
        }
        _ => CommandResult::error(format!(
            "unknown subcommand `{action}`. usage: /sessions [show|open <id>|archive <id>|unarchive <id>|prune <days>]"
        )),
    }
}
