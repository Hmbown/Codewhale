//! Portable `/receipts` parsing and rendering. The host reads the authoritative
//! session/approval records and projects their shared receipt shape.

use super::CommandResult;
use crate::diagnostics_reports::receipts::{render_json_block, render_markdown};
use codewhale_command_contract::facets::DebugReceiptError;
use codewhale_command_contract::handler::{CommandCapabilities, CommandContexts, CommandHandler};
use codewhale_command_contract::metadata::{CommandInfo, RegisterCommand};

pub(in crate::commands) struct ReceiptsCmd;
impl RegisterCommand<CommandResult> for ReceiptsCmd {
    fn info() -> &'static CommandInfo {
        &CommandInfo {
            name: "receipts",
            aliases: &["receipt"],
            usage: "/receipts [json] [<turn>]",
            description_key: "cmd_receipts_description",
        }
    }
    fn handler() -> CommandHandler<CommandResult> {
        CommandHandler::Contextual {
            capabilities: CommandCapabilities::DEBUG_RECEIPTS,
            handler: receipts,
        }
    }
}

pub(super) fn receipts(contexts: CommandContexts<'_>, arg: Option<&str>) -> CommandResult {
    let mut parts = contexts.into_parts();
    let Some(receipts) = parts.debug_receipts.as_deref_mut() else {
        return CommandResult::error("Command capability unavailable: debug_receipts");
    };
    let mut json = false;
    let mut turn: Option<&str> = None;
    for word in arg.unwrap_or_default().split_whitespace() {
        match word {
            "json" => json = true,
            word if word.parse::<usize>().is_ok() => turn = Some(word),
            other => {
                return CommandResult::error(format!(
                    "Unknown argument '{other}'. Use /receipts [json] [<turn>]."
                ));
            }
        }
    }
    match receipts.receipt(turn) {
        Ok(receipt) if json => CommandResult::message(render_json_block(&receipt)),
        Ok(receipt) => CommandResult::message(render_markdown(&receipt).trim_end().to_string()),
        Err(DebugReceiptError::ApprovalLog(error)) => CommandResult::error(format!(
            "Could not read this session's approval log: {error}"
        )),
        Err(DebugReceiptError::Build(error)) => CommandResult::error(error),
    }
}
