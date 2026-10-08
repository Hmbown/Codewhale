//! `/subagents` compatibility command.

use crate::commands::traits::{CommandInfo, RegisterCommand};
use crate::tui::app::App;
use codewhale_localization::MessageId;

use super::CommandResult;

pub(in crate::commands) const COMMAND_INFO: CommandInfo = CommandInfo {
    name: "subagents",
    aliases: &["agents", "zhinengti"],
    usage: "/subagents [list]",
    description_id: MessageId::CmdSubagentsDescription,
};

pub(in crate::commands) struct SubagentsCmd;

impl RegisterCommand for SubagentsCmd {
    fn info() -> &'static CommandInfo {
        &COMMAND_INFO
    }

    fn execute(app: &mut App, arg: Option<&str>) -> CommandResult {
        // `list` prints the roster into the transcript instead of opening the
        // modal, so a session transcript (and `exec`, which has no modal at
        // all) carries the same agent history the TUI shows (#5479 spec 5).
        match arg.map(str::trim).unwrap_or_default() {
            "" => super::core::subagents(app),
            "list" | "roster" | "ls" => super::core::subagents_roster(app),
            other => CommandResult::error(format!(
                "Unknown /agents argument {other:?}. Usage: /agents [list]"
            )),
        }
    }
}
