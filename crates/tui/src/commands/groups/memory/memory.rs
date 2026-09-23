//! One `/memory` surface for the authoritative native store. Never edits the
//! old Markdown anchor or reports an index-cache deletion as memory erasure.
use crate::commands::CommandResult;
use codewhale_command_contract::facets::{
    CommandMemoryContext, MemoryDeleteScope, MemoryGetOutcome, MemoryImportOutcome,
    MemoryRememberTarget,
};
use codewhale_command_contract::handler::{CommandCapabilities, CommandContexts, CommandHandler};
use codewhale_command_contract::metadata::{CommandInfo, RegisterCommand};
use std::path::Path;
const HELP: &str = "Memory is reviewed context, not another instruction layer.\n\n/memory [status|path|search <query>|get <numeric-id>|remember [global|workspace] <note>|import|export|reindex|clear <scope> --confirm|help]\n\nThe `native` prefix is a compatibility alias. `show` shows store status.\nCaptures, including legacy # entry points, become candidates; review in Context Lens.\nCorrect, approve, pin and exclude through Context Lens or the operator CLI.\n`clear` scopes: global, workspace, all. Confirmation removes tracked revision families and dependent checkpoints. Backups, exports and previously sent model context are not erased.\nThe database is authoritative. Never open it in a text editor or delete it as a cache.";
fn split(input: &str) -> (&str, &str) {
    input
        .trim()
        .split_once(char::is_whitespace)
        .map(|(a, b)| (a, b.trim()))
        .unwrap_or((input.trim(), ""))
}
fn normalize(input: Option<&str>) -> (&str, &str) {
    let (command, arg) = split(input.unwrap_or("status"));
    let (command, arg) = if command == "native" {
        split(arg)
    } else {
        (command, arg)
    };
    (
        if matches!(command, "" | "show") {
            "status"
        } else {
            command
        },
        arg,
    )
}
fn memory(workspace: &Path, memory: &dyn CommandMemoryContext, arg: Option<&str>) -> CommandResult {
    if !memory.memory_enabled() {
        return CommandResult::error(
            "Memory is disabled. Enable [memory] enabled = true in user configuration, then restart. No store was opened.",
        );
    }
    let (command, arg) = normalize(arg);
    match command {
        "help" => CommandResult::message(HELP),
        "status" => match memory.status() {
            Ok(s) => CommandResult::message(format!(
                "Native memory root: {}\nAuthoritative structured database: {}\nUse search/get to inspect entries and Context Lens to review candidates. The legacy Markdown source is not authoritative.",
                s.root.display(),
                s.index.display()
            )),
            Err(e) => CommandResult::error(format!("Memory status unavailable: {e}")),
        },
        "path" => match memory.path() {
            Ok(root) => CommandResult::message(root.display().to_string()),
            Err(e) => CommandResult::error(format!("Memory path unavailable: {e}")),
        },
        "edit" => CommandResult::message(
            "Use Context Lens to correct a memory. The authoritative database must not be edited as Markdown.",
        ),
        "search" => {
            if arg.is_empty() {
                return CommandResult::error("Usage: /memory search <query>");
            }
            match memory.search(workspace, arg, 10) {
                Ok(rows) if rows.is_empty() => {
                    CommandResult::message("No reviewed, current memory matches in this scope.")
                }
                Ok(rows) => CommandResult::message(
                    rows.into_iter()
                        .map(|r| {
                            format!(
                                "[untrusted memory; store={}] {}",
                                r.source.display(),
                                r.text
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                ),
                Err(e) => CommandResult::error(format!("Memory search failed: {e}")),
            }
        }
        "get" => {
            let Ok(id) = arg.parse::<i64>() else {
                return CommandResult::error("Usage: /memory get <numeric-id>");
            };
            match memory.get(workspace, id) {
                Ok(MemoryGetOutcome::Found(r)) => CommandResult::message(format!(
                    "[untrusted memory #{id}; store={}]\n{}",
                    r.source.display(),
                    r.text
                )),
                Ok(MemoryGetOutcome::NotFound) => {
                    CommandResult::error("Memory entry was not found in this scope.")
                }
                Err(e) => CommandResult::error(format!("Memory read failed: {e}")),
            }
        }
        "remember" => {
            let (first, rest) = split(arg);
            let (target, note) = match first {
                "workspace" => match memory.workspace_id(workspace) {
                    Ok(id) => (MemoryRememberTarget::Workspace { workspace_id: id }, rest),
                    Err(e) => return CommandResult::error(e),
                },
                "global" => (MemoryRememberTarget::Global, rest),
                _ => (MemoryRememberTarget::Global, arg),
            };
            if note.is_empty() {
                return CommandResult::error(
                    "Usage: /memory remember [global|workspace] <complete note>",
                );
            }
            match memory.remember(target, note) {
                Ok(_) => CommandResult::message(
                    "Memory candidate saved. Review it in Context Lens before it is used as durable context.",
                ),
                Err(e) => CommandResult::error(format!("Memory capture failed: {e}")),
            }
        }
        "import" => match memory.import() {
            Ok(MemoryImportOutcome::Imported { destination }) => CommandResult::message(format!(
                "Legacy notes imported as candidates into {}. The original file was not deleted.",
                destination.display()
            )),
            Ok(MemoryImportOutcome::Skipped) => {
                CommandResult::message("No new legacy candidates were imported.")
            }
            Err(e) => CommandResult::error(format!("Memory import failed: {e}")),
        },
        "export" => match memory.export() {
            Ok(e) => CommandResult::message(e.content),
            Err(e) => CommandResult::error(format!("Memory export failed: {e}")),
        },
        "reindex" => match memory.reindex() {
            Ok(r) => CommandResult::message(format!(
                "Rebuilt the derived search indexes for {} entries. The authoritative database was retained.",
                r.entry_count
            )),
            Err(e) => CommandResult::error(format!("Reindex failed: {e}")),
        },
        "clear" | "delete" => {
            let (scope, confirm) = split(arg);
            if confirm != "--confirm" || !matches!(scope, "global" | "workspace" | "all") {
                return CommandResult::error(
                    "Usage: /memory clear <global|workspace|all> --confirm. This removes tracked revision families and dependent checkpoints, not backups, exports or already-sent provider context.",
                );
            }
            let result = match scope {
                "global" => memory.delete(MemoryDeleteScope::Global),
                "workspace" => memory.delete_workspace(workspace),
                _ => memory.delete(MemoryDeleteScope::All),
            };
            match result {
                Ok(_) => CommandResult::message(format!(
                    "Completed explicit {scope} memory deletion. External copies and already-sent context are outside this operation."
                )),
                Err(e) => CommandResult::error(format!(
                    "Memory deletion stopped: {e}. Inspect current state before retrying."
                )),
            }
        }
        _ => CommandResult::error(HELP),
    }
}
pub(in crate::commands) const COMMAND_INFO: CommandInfo = CommandInfo {
    name: "memory",
    aliases: &[],
    usage: "/memory [status|path|search|get|remember|import|export|reindex|clear|help]",
    description_key: "cmd_memory_description",
};
pub(in crate::commands) struct MemoryCmd;
impl RegisterCommand<CommandResult> for MemoryCmd {
    fn info() -> &'static CommandInfo {
        &COMMAND_INFO
    }
    fn handler() -> CommandHandler<CommandResult> {
        CommandHandler::Contextual {
            capabilities: CommandCapabilities::WORKSPACE.union(CommandCapabilities::MEMORY),
            handler: memory_contextual,
        }
    }
}
fn memory_contextual(contexts: CommandContexts<'_>, arg: Option<&str>) -> CommandResult {
    let parts = contexts.into_parts();
    let Some(workspace) = parts.workspace.as_deref() else {
        return CommandResult::error("Command capability unavailable: workspace");
    };
    let Some(memory_ctx) = parts.memory.as_deref() else {
        return CommandResult::error("Command capability unavailable: memory");
    };
    memory(&workspace.workspace(), memory_ctx, arg)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_to_structured_status() {
        assert_eq!(normalize(None), ("status", ""));
    }
    #[test]
    fn native_is_an_exact_alias() {
        assert_eq!(
            normalize(Some("native search multiple words")),
            ("search", "multiple words")
        );
        assert_eq!(normalize(Some("natively")), ("natively", ""));
    }
    #[test]
    fn complete_note_is_preserved() {
        assert_eq!(
            split("workspace all the words remain"),
            ("workspace", "all the words remain")
        );
    }
    #[test]
    fn show_does_not_read_a_markdown_file() {
        assert_eq!(normalize(Some("show")), ("status", ""));
    }
    #[test]
    fn clear_requires_scope_and_confirmation() {
        assert_eq!(normalize(Some("clear")), ("clear", ""));
        assert_eq!(
            normalize(Some("clear workspace --confirm")),
            ("clear", "workspace --confirm")
        );
    }
}
