//! `/rlm` command.

use crate::commands::traits::{CommandInfo, RegisterCommand};
use crate::tui::app::{App, AppAction};
use codewhale_localization::MessageId;

use super::CommandResult;

pub(in crate::commands) const COMMAND_INFO: CommandInfo = CommandInfo {
    name: "rlm",
    aliases: &["recursive", "digui"],
    usage: "/rlm [N] <file_or_text>",
    description_id: MessageId::CmdRlmDescription,
};

pub(in crate::commands) struct RlmCmd;

impl RegisterCommand for RlmCmd {
    fn info() -> &'static CommandInfo {
        &COMMAND_INFO
    }

    fn execute(app: &mut App, arg: Option<&str>) -> CommandResult {
        rlm(app, arg)
    }
}

pub fn rlm(app: &mut App, arg: Option<&str>) -> CommandResult {
    // The `[N]` depth prefix stays accepted so saved guidance and muscle memory
    // keep working, but it was part of the retired open/configure/eval control
    // surface. The session-persistent working context now owns one route, so
    // the depth is parsed and dropped rather than rejected.
    let (_legacy_depth, target) = match super::util::parse_depth_prefixed_arg(arg, 1) {
        Ok(parsed) => parsed,
        Err(message) => return CommandResult::error(message),
    };
    let target = match target {
        Some(p) if !p.trim().is_empty() => p.trim().to_string(),
        _ => {
            return CommandResult::error(
                "Usage: /rlm [N] <file_or_text>\n\n\
                 Works through a large file or block of text in a context that \
                 stays loaded for the rest of the session."
                    .to_string(),
            );
        }
    };

    let source = if resolves_to_existing_file(app, &target) {
        format!("the workspace file `{target}`")
    } else {
        format!("this text: {target:?}")
    };
    let message = format!(
        "Use the session-persistent working context for this request. It stays alive across turns. Work on {source}. In a `repl` block, load a file into a normal Python variable when useful, retain useful variables and imports, inspect the durable transcript through `context_meta`, `search`, `peek`, or `chunk`, and use `sub_query` or `sub_rlm` only when extra reasoning genuinely helps. Do not use legacy `rlm` tool actions. Call `finalize(...)` only when ready to answer."
    );

    CommandResult::with_message_and_action(
        "Loading that into a persistent working context...".to_string(),
        AppAction::SendMessage(message),
    )
}

fn resolves_to_existing_file(app: &App, input: &str) -> bool {
    let path = std::path::Path::new(input);
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        app.workspace.join(path)
    };
    candidate.is_file()
}
