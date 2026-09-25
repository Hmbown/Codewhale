//! `/router` command (#6525): set up the per-turn router for Auto model
//! routing. `/model router …` is the same command. Both open the one Router
//! setup view (`tui::views::router_setup`) or run a preset through it.

use crate::commands::traits::{CommandInfo, RegisterCommand};
use crate::tui::app::{App, AppAction};
use crate::tui::views::router_setup::parse_router_args;
use codewhale_localization::MessageId;

use super::CommandResult;

pub(in crate::commands) const COMMAND_INFO: CommandInfo = CommandInfo {
    name: "router",
    aliases: &[],
    usage: "/router [jev|fast|off|custom|save]",
    description_id: MessageId::CmdRouterDescription,
};

pub(in crate::commands) struct RouterCmd;

impl RegisterCommand for RouterCmd {
    fn info() -> &'static CommandInfo {
        &COMMAND_INFO
    }

    fn execute(_app: &mut App, arg: Option<&str>) -> CommandResult {
        router_command(arg)
    }
}

/// Shared by `/router` and `/model router`.
pub(in crate::commands) fn router_command(arg: Option<&str>) -> CommandResult {
    match parse_router_args(arg) {
        Ok(request) => CommandResult::action(AppAction::RouterSetup { request }),
        Err(usage) => CommandResult::error(usage),
    }
}
