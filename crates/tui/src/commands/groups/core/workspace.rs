//! `/workspace` command.

use crate::commands::traits::{CommandInfo, RegisterCommand};
use crate::tui::app::{App, AppAction};
use codewhale_localization::MessageId;

use super::CommandResult;

pub(in crate::commands) const COMMAND_INFO: CommandInfo = CommandInfo {
    name: "workspace",
    aliases: &["cwd"],
    usage: "/workspace [path|worktrees]",
    description_id: MessageId::CmdWorkspaceDescription,
};

pub(in crate::commands) struct WorkspaceCmd;

impl RegisterCommand for WorkspaceCmd {
    fn info() -> &'static CommandInfo {
        &COMMAND_INFO
    }

    fn execute(app: &mut App, arg: Option<&str>) -> CommandResult {
        match arg.map(str::trim).filter(|a| !a.is_empty()) {
            Some("worktrees" | "worktree" | "wt") => {
                CommandResult::action(AppAction::OpenWorktreeManager)
            }
            other => super::core::workspace_switch(app, other),
        }
    }
}
