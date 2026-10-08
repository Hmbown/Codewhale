use super::CommandResult;
use super::DebugAction as AppAction;
use crate::diagnostics_reports::{render_tool_snapshot_json, render_tool_snapshot_text};
use codewhale_command_contract::handler::{CommandCapabilities, CommandContexts, CommandHandler};
use codewhale_command_contract::metadata::{
    CommandInfo as ContractInfo, RegisterCommand as ContractRegisterCommand,
};

pub(in crate::commands) struct ToolsCmd;

const CONTRACT_INFO: ContractInfo = ContractInfo {
    name: "tools",
    aliases: &["tool-studio"],
    usage: "/tools [text|json]",
    description_key: "cmd_tools_description",
};

impl ContractRegisterCommand<CommandResult> for ToolsCmd {
    fn info() -> &'static ContractInfo {
        &CONTRACT_INFO
    }

    fn handler() -> CommandHandler<CommandResult> {
        CommandHandler::Contextual {
            capabilities: CommandCapabilities::DEBUG_DIAGNOSTICS,
            handler: tools,
        }
    }
}

pub(super) fn tools(contexts: CommandContexts<'_>, arg: Option<&str>) -> CommandResult {
    let mut parts = contexts.into_parts();
    let Some(diagnostics) = parts.debug_diagnostics.as_deref_mut() else {
        return CommandResult::error("Command capability unavailable: debug_diagnostics");
    };
    // The original availability check precedes format validation. No prior
    // request differs from an observed empty prepared catalog.
    let Some(snapshot) = diagnostics.tool_snapshot() else {
        return CommandResult::message(
            "Tool request snapshot unavailable — no model request has been captured for the latest turn.",
        );
    };
    match arg.unwrap_or("text").trim() {
        "" | "text" => CommandResult::action(AppAction::OpenTextPager {
            title: "Prepared Tool Request".to_string(),
            content: render_tool_snapshot_text(&snapshot),
        }),
        "json" => match render_tool_snapshot_json(&snapshot) {
            Ok(output) => CommandResult::action(AppAction::OpenTextPager {
                title: "Prepared Tool Request (JSON)".to_string(),
                content: output,
            }),
            Err(error) => CommandResult::error(format!(
                "tool request snapshot could not be serialized: {error}"
            )),
        },
        _ => CommandResult::error("usage: /tools [text|json]"),
    }
}
