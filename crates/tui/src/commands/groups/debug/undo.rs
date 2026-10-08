//! Portable edit/diff/undo/retry policy. Hosts own I/O and mutation; typed
//! outcomes preserve refusal versus conversation-fallback without parsing text.

use super::{CommandResult, DebugAction};
use codewhale_command_contract::facets::*;
use codewhale_command_contract::handler::{
    CommandCapabilities as Caps, CommandContexts, CommandHandler,
};
use codewhale_command_contract::metadata::{CommandInfo, RegisterCommand};

macro_rules! registration {
    ($ty:ident, $name:literal, $aliases:expr, $key:literal, $caps:expr, $handler:ident) => {
        pub(in crate::commands) struct $ty;
        impl RegisterCommand<CommandResult> for $ty {
            fn info() -> &'static CommandInfo {
                &CommandInfo {
                    name: $name,
                    aliases: $aliases,
                    usage: concat!("/", $name),
                    description_key: $key,
                }
            }
            fn handler() -> CommandHandler<CommandResult> {
                CommandHandler::Contextual {
                    capabilities: $caps,
                    handler: $handler,
                }
            }
        }
    };
}
registration!(
    EditCmd,
    "edit",
    &[],
    "cmd_edit_description",
    Caps::DEBUG_HISTORY,
    edit
);
registration!(
    DiffCmd,
    "diff",
    &[],
    "cmd_diff_description",
    Caps::DEBUG_DIFF,
    diff
);
registration!(
    UndoCmd,
    "undo",
    &[],
    "cmd_undo_description",
    Caps::DEBUG_UNDO.union(Caps::DEBUG_HISTORY),
    undo
);
registration!(
    RetryCmd,
    "retry",
    &["chongshi"],
    "cmd_retry_description",
    Caps::DEBUG_HISTORY,
    retry
);

pub(in crate::commands) const SNAPSHOT_REPO_UNAVAILABLE_PREFIX: &str = "Snapshot repo unavailable";
pub(in crate::commands) const FILES_NOT_REVERTED_NOTE: &str =
    "Workspace files were NOT reverted — only the conversation was rolled back.";

pub(super) fn edit(contexts: CommandContexts<'_>, _: Option<&str>) -> CommandResult {
    let mut parts = contexts.into_parts();
    let Some(history) = parts.debug_history.as_deref_mut() else {
        return CommandResult::error("Command capability unavailable: debug_history");
    };
    match history.last_user_input() {
        Some(input) => {
            history.load_composer(input);
            CommandResult::message(
                "Last message loaded into composer — edit and press Enter to resubmit",
            )
        }
        None => CommandResult::message("No previous message to edit"),
    }
}

pub(super) fn retry(contexts: CommandContexts<'_>, _: Option<&str>) -> CommandResult {
    let mut parts = contexts.into_parts();
    let Some(history) = parts.debug_history.as_deref_mut() else {
        return CommandResult::error("Command capability unavailable: debug_history");
    };
    match history.last_user_input() {
        Some(input) => {
            let undone = history.undo_conversation();
            let display_input = if input.len() > 50 {
                let truncate_at = input
                    .char_indices()
                    .take_while(|(i, _)| *i <= 50)
                    .last()
                    .map_or(0, |(i, _)| i);
                format!("{}...", &input[..truncate_at])
            } else {
                input.clone()
            };
            CommandResult::with_message_and_action(
                format!("Retrying: {display_input}"),
                DebugAction::ConversationUndo {
                    sync: undone.sync,
                    retry_input: Some(input),
                },
            )
        }
        None => CommandResult::error("No previous request to retry"),
    }
}

pub(super) fn diff(contexts: CommandContexts<'_>, _: Option<&str>) -> CommandResult {
    let mut parts = contexts.into_parts();
    let Some(diff) = parts.debug_diff.as_deref_mut() else {
        return CommandResult::error("Command capability unavailable: debug_diff");
    };
    match diff.diff() {
        DebugDiffObservation::GitUnavailable => CommandResult::error("git not found on PATH"),
        DebugDiffObservation::Failed(error) => CommandResult::message(format!(
            "Git diff failed — is this a git repository?\n{error}"
        )),
        DebugDiffObservation::Output { names, stat } => {
            if names.trim().is_empty() {
                return CommandResult::message("No changes since session start");
            }

            let files: Vec<&str> = names.lines().filter(|l| !l.is_empty()).collect();
            let file_count = files.len();
            let file_list = files.join("\n");

            // Detect rename entries (e.g. "foo -> bar") and exclude them
            // from the file-count header so the user sees only actual
            // modifications.
            let renamed_count = files.iter().filter(|f| f.contains(" -> ")).count();
            let summary = if renamed_count > 0 {
                format!("Changed files ({file_count}, {renamed_count} renamed):\n{file_list}")
            } else {
                format!("Changed files ({file_count}):\n{file_list}")
            };

            let stat_str = stat.trim();
            let mut message = summary;
            if !stat_str.is_empty() {
                message.push_str("\n\n── Stat ──\n");
                message.push_str(stat_str);
            }
            CommandResult::message(message)
        }
    }
}

/// A conversation undo that removed anything hands the truncated conversation
/// to the engine, which owns the model context (#6788).
pub(in crate::commands) fn conversation_result(undone: DebugConversationUndo) -> CommandResult {
    if undone.removed > 0 {
        CommandResult::with_message_and_action(
            format!("Removed {} message(s)", undone.removed),
            DebugAction::ConversationUndo {
                sync: undone.sync,
                retry_input: None,
            },
        )
    } else {
        CommandResult::message("Nothing to undo")
    }
}

pub(super) fn undo(contexts: CommandContexts<'_>, _: Option<&str>) -> CommandResult {
    let mut parts = contexts.into_parts();
    let Some(undo) = parts.debug_undo.as_deref_mut() else {
        return CommandResult::error("Command capability unavailable: debug_undo");
    };
    let Some(history) = parts.debug_history.as_deref_mut() else {
        return CommandResult::error("Command capability unavailable: debug_history");
    };
    match undo.undo_files() {
        DebugUndoOutcome::NoSnapshots
        | DebugUndoOutcome::NoSession
        | DebugUndoOutcome::NoOwnedSteps
        | DebugUndoOutcome::NoDifference => conversation_result(history.undo_conversation()),
        DebugUndoOutcome::RepoUnavailable { workspace, error } => {
            let mut result = conversation_result(history.undo_conversation());
            let note = format!(
                "{FILES_NOT_REVERTED_NOTE}\n{SNAPSHOT_REPO_UNAVAILABLE_PREFIX} for {}: {error}",
                workspace.display()
            );
            result.message = Some(match result.message.take() {
                Some(message) => format!("{message}\n{note}"),
                None => note,
            });
            result
        }
        outcome => patch_result(outcome),
    }
}

/// Format snapshot outcomes without invoking host operations. Host tests also
/// use this for the file-only operation before the command's chat fallback.
pub(in crate::commands) fn patch_result(outcome: DebugUndoOutcome) -> CommandResult {
    match outcome {
        DebugUndoOutcome::RepoUnavailable { workspace, error } => CommandResult::error(format!(
            "{SNAPSHOT_REPO_UNAVAILABLE_PREFIX} for {}: {error}",
            workspace.display()
        )),
        DebugUndoOutcome::SnapshotPending => CommandResult::message(
            "The last turn's workspace snapshot is still being written; nothing was changed. Run /undo again in a moment.",
        ),
        DebugUndoOutcome::NoSnapshots => {
            CommandResult::message("No snapshots found to undo — nothing to revert.")
        }
        DebugUndoOutcome::NoSession => CommandResult::message(
            "No undoable snapshot is tagged for the current session — nothing to revert.",
        ),
        DebugUndoOutcome::NoOwnedSteps => CommandResult::message(
            "No undoable snapshots for the current session — nothing to revert.",
        ),
        DebugUndoOutcome::NoDifference => CommandResult::message(
            "No undoable snapshot differs from the current workspace — nothing to revert.",
        ),
        DebugUndoOutcome::Untrusted => CommandResult::message(
            "Refusing to undo workspace files outside trusted mode.\nRun `/trust on` or select Full Access with Shift+Tab, then re-run `/undo`.",
        ),
        DebugUndoOutcome::CompareFailed(error) => {
            CommandResult::error(format!("Failed to compare snapshot: {error}"))
        }
        DebugUndoOutcome::SnapshotFailed(error) => CommandResult::error(format!(
            "Failed to snapshot the workspace before undo: {error}"
        )),
        DebugUndoOutcome::ListFailed(error) => {
            CommandResult::error(format!("Failed to list snapshots: {error}"))
        }
        DebugUndoOutcome::RestoreBlocked(error) => CommandResult::message(error),
        DebugUndoOutcome::RestoreFailed(error) => {
            CommandResult::error(format!("Restore failed: {error}"))
        }
        DebugUndoOutcome::ChangedSince { label, paths } => CommandResult::message(format!(
            "Refusing to undo snapshot '{}': {} changed after it, and undoing would overwrite that change. Nothing was changed; revert those files yourself, or use /restore for a whole-workspace rollback.",
            label,
            paths.join(", ")
        )),
        DebugUndoOutcome::Restored(restored) => {
            let short = &restored.snapshot_id[..restored.snapshot_id.len().min(8)];
            let lines: Vec<String> = restored
                .files
                .iter()
                .map(|file| {
                    let action = match file.action {
                        DebugRestoreAction::Modified => "modified",
                        DebugRestoreAction::Recreated => "recreated",
                        DebugRestoreAction::Removed => "removed",
                    };
                    format!("{action} {}", file.path.display())
                })
                .collect();
            let mut summary = format!(
                "Restored {} file(s) to snapshot '{}' ({}):\n{}",
                restored.files.len(),
                restored.label,
                short,
                lines.join("\n")
            );
            if !restored.skipped.is_empty() {
                let skipped: Vec<String> = restored
                    .skipped
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect();
                summary.push_str(&format!("\nLeft in place (not a regular file, which /undo does not restore; use /restore for a whole-workspace rollback): {}", skipped.join(", ")));
            }
            CommandResult::with_message_and_action(summary, DebugAction::SyncSession(restored.sync))
        }
    }
}
