//! Anchor command: keep critical facts across compaction.
//!
//! Unlike `/note` (active lookup), anchors are passive. They are automatically
//! re-injected into context after compaction in trusted workspaces. Use anchors to
//! preserve invariants like "This API's status field is unreliable" or
//! ".ssh/ must never be touched".

use std::io::Write;

use crate::commands::traits::{CommandInfo, RegisterCommand};
use crate::tui::app::App;
use codewhale_localization::MessageId;

use super::CommandResult;

const USAGE: &str = "/anchor <text> | /anchor list | /anchor remove <n>";

pub(in crate::commands) const COMMAND_INFO: CommandInfo = CommandInfo {
    name: "anchor",
    aliases: &["maodian"],
    usage: USAGE,
    description_id: MessageId::CmdAnchorDescription,
};

pub(in crate::commands) struct AnchorCmd;

impl RegisterCommand for AnchorCmd {
    fn info() -> &'static CommandInfo {
        &COMMAND_INFO
    }

    fn execute(app: &mut App, arg: Option<&str>) -> CommandResult {
        anchor(app, arg)
    }
}

/// Handle the `/anchor` command with subcommands:
/// - `/anchor <text>` — add a new anchor
/// - `/anchor list` — list all anchors
/// - `/anchor remove <n>` — remove anchor by 1-based index
pub fn anchor(app: &mut App, content: Option<&str>) -> CommandResult {
    let input = match content {
        Some(c) => c.trim(),
        None => {
            return CommandResult::error(format!("Usage: {USAGE}"));
        }
    };

    if input.is_empty() {
        return CommandResult::error(format!("Usage: {USAGE}"));
    }

    // Parse subcommands.
    if input.eq_ignore_ascii_case("list") {
        return list_anchors(app);
    }

    if let Some(rest) = input
        .strip_prefix("remove ")
        .or_else(|| input.strip_prefix("rm "))
        .or_else(|| input.strip_prefix("delete "))
    {
        return remove_anchor(app, rest.trim());
    }

    // Default: add a new anchor.
    add_anchor(app, input)
}

fn anchors_path(app: &App) -> std::path::PathBuf {
    let primary = app.workspace.join(".codewhale").join("anchors.md");
    if primary.symlink_metadata().is_ok() || app.workspace.join(".codewhale").is_symlink() {
        return primary;
    }
    app.workspace.join(".deepseek").join("anchors.md")
}

/// Read and split anchors from the file. Each anchor is separated by "\n---\n".
fn read_anchors(app: &App) -> Result<Vec<String>, String> {
    let path = anchors_path(app);
    let content = match crate::fs_confined::read_to_string(&app.workspace, &path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("Failed to read anchors file: {e}")),
    };

    Ok(content
        .split("\n---\n")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect())
}

/// Write anchors back to the file, joined by "\n---\n".
fn write_anchors(app: &App, anchors: &[String]) -> Result<(), String> {
    let path = anchors_path(app);

    let content = anchors.join("\n---\n");
    crate::fs_confined::write(&app.workspace, &path, content.as_bytes())
        .map_err(|e| format!("Failed to write anchors file: {e}"))
}

fn add_anchor(app: &mut App, text: &str) -> CommandResult {
    let path = anchors_path(app);

    let mut file = match crate::fs_confined::open_append(&app.workspace, &path) {
        Ok(f) => f,
        Err(e) => {
            return CommandResult::error(format!("Failed to open anchors file: {e}"));
        }
    };

    // Write separator and anchor content.
    if let Err(e) = writeln!(file, "\n---\n{text}") {
        return CommandResult::error(format!("Failed to write anchor: {e}"));
    }

    CommandResult::message(format!("Anchor pinned.\nStored in: {}", path.display()))
}

fn list_anchors(app: &App) -> CommandResult {
    let anchors = match read_anchors(app) {
        Ok(anchors) => anchors,
        Err(e) => return CommandResult::error(e),
    };

    if anchors.is_empty() {
        return CommandResult::message(
            "No anchors set. Use /anchor <text> to pin a fact that survives compaction.",
        );
    }

    let mut output = format!("Pinned anchors ({} total):\n", anchors.len());
    for (i, anchor) in anchors.iter().enumerate() {
        output.push_str(&format!("\n  {}. {}", i + 1, anchor));
    }
    output.push_str("\n\nUse /anchor remove <n> to remove an anchor.");

    CommandResult::message(output)
}

fn remove_anchor(app: &mut App, index_str: &str) -> CommandResult {
    let index: usize = match index_str.parse() {
        Ok(n) if n >= 1 => n,
        _ => {
            return CommandResult::error(
                "Invalid index. Use /anchor list to see anchor numbers, then /anchor remove <n>.",
            );
        }
    };

    let mut anchors = match read_anchors(app) {
        Ok(anchors) => anchors,
        Err(e) => return CommandResult::error(e),
    };

    if index > anchors.len() {
        return CommandResult::error(format!(
            "Anchor #{index} does not exist. You have {} anchor(s). Use /anchor list to see them.",
            anchors.len()
        ));
    }

    let removed = anchors.remove(index - 1);
    if let Err(e) = write_anchors(app, &anchors) {
        return CommandResult::error(e);
    }

    CommandResult::message(format!("Removed anchor #{index}: {removed}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::tui::app::{App, TuiOptions};
    use tempfile::TempDir;

    fn create_test_app_with_tmpdir(tmpdir: &TempDir) -> App {
        let options = TuiOptions {
            skills_dir: tmpdir.path().join("skills"),
            memory_path: tmpdir.path().join("memory.md"),
            notes_path: tmpdir.path().join("notes.txt"),
            mcp_config_path: tmpdir.path().join("mcp.json"),
            ..crate::test_support::test_tui_options(tmpdir.path())
        };
        App::new(options, &Config::default())
    }

    #[cfg(unix)]
    #[test]
    fn confined_anchors_refuse_linked_files_and_directories() {
        use std::os::unix::fs::symlink;

        for directory in [".codewhale", ".deepseek"] {
            for linked_directory in [false, true] {
                let workspace = TempDir::new().unwrap();
                let outside = TempDir::new().unwrap();
                let mut app = create_test_app_with_tmpdir(&workspace);
                let original = "first anchor\n---\nsecond anchor";
                let target = outside.path().join("anchors.md");
                std::fs::write(&target, original).unwrap();
                let parent = workspace.path().join(directory);
                if linked_directory {
                    symlink(outside.path(), &parent).unwrap();
                } else {
                    std::fs::create_dir_all(&parent).unwrap();
                    symlink(&target, parent.join("anchors.md")).unwrap();
                }
                for command in ["list", "next anchor", "remove 1"] {
                    assert!(anchor(&mut app, Some(command)).is_error);
                    assert_eq!(std::fs::read_to_string(&target).unwrap(), original);
                }
                // Exercise the truncate path independently of the read refusal.
                assert!(write_anchors(&app, &[]).is_err());
                assert_eq!(std::fs::read_to_string(&target).unwrap(), original);
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn confined_anchors_refuse_dangling_links() {
        for directory in [".codewhale", ".deepseek"] {
            let workspace = TempDir::new().unwrap();
            let outside = TempDir::new().unwrap();
            let mut app = create_test_app_with_tmpdir(&workspace);
            let target = outside.path().join("missing.md");
            let parent = workspace.path().join(directory);
            std::fs::create_dir_all(&parent).unwrap();
            std::os::unix::fs::symlink(&target, parent.join("anchors.md")).unwrap();
            assert!(anchor(&mut app, Some("list")).is_error);
            assert!(anchor(&mut app, Some("next anchor")).is_error);
            assert!(write_anchors(&app, &[]).is_err());
            assert!(!target.exists());
        }
    }

    #[test]
    fn test_anchor_without_content_returns_error() {
        let tmpdir = TempDir::new().unwrap();
        let mut app = create_test_app_with_tmpdir(&tmpdir);
        let result = anchor(&mut app, None);
        assert!(result.is_error);
        assert!(result.message.unwrap().contains("Usage:"));
    }

    #[test]
    fn test_anchor_with_empty_content_returns_error() {
        let tmpdir = TempDir::new().unwrap();
        let mut app = create_test_app_with_tmpdir(&tmpdir);
        let result = anchor(&mut app, Some("   "));
        assert!(result.is_error);
        assert!(result.message.unwrap().contains("Usage:"));
    }

    #[test]
    fn test_anchor_add() {
        let tmpdir = TempDir::new().unwrap();
        let mut app = create_test_app_with_tmpdir(&tmpdir);
        let result = anchor(&mut app, Some("API status field is unreliable"));
        assert!(!result.is_error);
        assert!(result.message.unwrap().contains("Anchor pinned"));

        let path = tmpdir.path().join(".deepseek").join("anchors.md");
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("API status field is unreliable"));
    }

    #[test]
    fn test_anchor_list_empty() {
        let tmpdir = TempDir::new().unwrap();
        let mut app = create_test_app_with_tmpdir(&tmpdir);
        let result = anchor(&mut app, Some("list"));
        assert!(!result.is_error);
        assert!(result.message.unwrap().contains("No anchors set"));
    }

    #[test]
    fn test_anchor_list_with_items() {
        let tmpdir = TempDir::new().unwrap();
        let mut app = create_test_app_with_tmpdir(&tmpdir);
        anchor(&mut app, Some("First anchor"));
        anchor(&mut app, Some("Second anchor"));

        let result = anchor(&mut app, Some("list"));
        let msg = result.message.unwrap();
        assert!(msg.contains("2 total"));
        assert!(msg.contains("1. First anchor"));
        assert!(msg.contains("2. Second anchor"));
    }

    #[test]
    fn test_anchor_remove() {
        let tmpdir = TempDir::new().unwrap();
        let mut app = create_test_app_with_tmpdir(&tmpdir);
        anchor(&mut app, Some("First anchor"));
        anchor(&mut app, Some("Second anchor"));

        let result = anchor(&mut app, Some("remove 1"));
        assert!(!result.is_error);
        assert!(result.message.unwrap().contains("Removed anchor #1"));

        let result = anchor(&mut app, Some("list"));
        let msg = result.message.unwrap();
        assert!(msg.contains("1 total"));
        assert!(msg.contains("Second anchor"));
        assert!(!msg.contains("First anchor"));
    }

    #[test]
    fn test_anchor_remove_invalid_index() {
        let tmpdir = TempDir::new().unwrap();
        let mut app = create_test_app_with_tmpdir(&tmpdir);
        anchor(&mut app, Some("Only anchor"));

        let result = anchor(&mut app, Some("remove 5"));
        assert!(result.is_error);
        assert!(result.message.unwrap().contains("does not exist"));
    }

    #[test]
    fn test_anchor_remove_non_numeric() {
        let tmpdir = TempDir::new().unwrap();
        let mut app = create_test_app_with_tmpdir(&tmpdir);
        let result = anchor(&mut app, Some("remove abc"));
        assert!(result.is_error);
        assert!(result.message.unwrap().contains("Invalid index"));
    }
}
