//! Host operations moved out of `groups/debug/undo.rs` and `receipts.rs`.
//! Snapshot safety and authoritative state mutation remain here. The portable
//! commands receive typed observations/outcomes, never completed command text.
//! Existing synchronous dispatch timing is preserved; this does not introduce
//! an asynchronous I/O execution model.

use super::SharedCommandHost;
use crate::dependencies::{ExternalTool, Git};
use crate::tui::app::App;
use crate::tui::history::HistoryCell;
use codewhale_command_contract::facets::*;
use codewhale_models::ContentBlock;
use std::path::PathBuf;

pub(super) struct DebugOperationsAdapter<'a> {
    pub(super) host: SharedCommandHost<'a>,
}

impl CommandDebugReceiptsContext for DebugOperationsAdapter<'_> {
    fn receipt(&self, turn: Option<&str>) -> Result<Receipt, DebugReceiptError> {
        let app = self.host.app.borrow();
        let approvals = match app.current_session_id.as_deref() {
            Some(id) => crate::approval_log::ApprovalReceiptStore::default_location()
                .and_then(|store| store.load(id))
                .map_err(|error| DebugReceiptError::ApprovalLog(error.to_string()))?,
            None => Vec::new(),
        };
        let source = ReceiptSource {
            kind: SourceKind::Session,
            id: app
                .current_session_id
                .clone()
                .unwrap_or_else(|| "unsaved".to_string()),
            title: app.session_title.clone(),
            workspace: Some(app.workspace.display().to_string()),
            model: Some(app.model.clone()),
            started_at: Some(app.session_started_at),
            updated_at: None,
        };
        crate::receipts::session_receipt(source, &app.api_messages, &approvals, turn)
            .map_err(|error| DebugReceiptError::Build(error.to_string()))
    }
}

impl CommandDebugChangeContext for DebugOperationsAdapter<'_> {
    fn change_projection(&self) -> DebugChangeProjection {
        let app = self.host.app.borrow();
        DebugChangeProjection {
            changelog: include_str!("../../../CHANGELOG.md"),
            is_english: app.ui_locale == codewhale_localization::Locale::En,
            translation_available: !app.offline_mode && !app.onboarding_needs_api_key,
            translation_target: app.ui_locale.translation_target_name(),
        }
    }
}

impl CommandDebugHistoryContext for DebugOperationsAdapter<'_> {
    fn last_user_input(&self) -> Option<String> {
        self.host
            .app
            .borrow()
            .history
            .iter()
            .rev()
            .find_map(|cell| match cell {
                HistoryCell::User { content } => Some(content.clone()),
                _ => None,
            })
    }
    fn load_composer(&mut self, input: String) {
        let mut app = self.host.app.borrow_mut();
        // A queued follow-up still open for editing would otherwise stay bound
        // to the composer and be overwritten, or sent in place of this edit.
        // Return it to the queue first — the same hand-back as Esc.
        if app.cancel_queued_draft_edit() {
            app.status_message = Some("Queued edit canceled; follow-up restored".to_string());
        }
        app.input = input;
        app.cursor_position = app.input.chars().count();
        app.edit_in_progress = true;
    }
    fn undo_conversation(&mut self) -> DebugConversationUndo {
        undo_conversation_for_engine(&mut self.host.app.borrow_mut())
    }
}

impl CommandDebugUndoContext for DebugOperationsAdapter<'_> {
    fn undo_files(&mut self) -> DebugUndoOutcome {
        undo_files(&mut self.host.app.borrow_mut())
    }
}

impl CommandDebugDiffContext for DebugOperationsAdapter<'_> {
    fn diff(&self) -> DebugDiffObservation {
        let workspace = self.host.app.borrow().workspace.clone();
        let Some(mut name_only_cmd) = Git::command() else {
            return DebugDiffObservation::GitUnavailable;
        };
        let Some(mut stat_cmd) = Git::command() else {
            return DebugDiffObservation::GitUnavailable;
        };
        let names = name_only_cmd
            .args(["diff", "--name-only"])
            .current_dir(&workspace)
            .output();
        let stat = stat_cmd
            .args(["diff", "--stat"])
            .current_dir(&workspace)
            .output();
        match (names, stat) {
            (Ok(names), Ok(stat)) => DebugDiffObservation::Output {
                names: String::from_utf8_lossy(&names.stdout).into_owned(),
                stat: String::from_utf8_lossy(&stat.stdout).into_owned(),
            },
            (Err(error), _) | (_, Err(error)) => DebugDiffObservation::Failed(error.to_string()),
        }
    }
}

/// Prepare the rollback; the UI action owns Engine acknowledgement and save.
/// The last real user boundary includes following tool results/runtime notes.
pub(in crate::commands) fn undo_conversation_for_engine(app: &mut App) -> DebugConversationUndo {
    let removed = app
        .history
        .iter()
        .rposition(|cell| matches!(cell, HistoryCell::User { .. }))
        .map_or(0, |index| app.history.len() - index);
    let mut sync = session_sync_payload(app);
    if let Some(index) = sync.messages.iter().rposition(|message| {
        !matches!(
            crate::runtime_handoff::classify_user_turn_prompt(message),
            crate::runtime_handoff::UserTurnPromptKind::NotPrompt
        )
    }) {
        sync.messages.truncate(index);
    }
    DebugConversationUndo { removed, sync }
}

fn session_sync_payload(app: &App) -> SessionSyncPayload {
    SessionSyncPayload {
        session_id: app.current_session_id.clone(),
        messages: app.api_messages.as_ref().clone(),
        system_prompt: app.system_prompt.clone(),
        model: app.model.clone(),
        workspace: app.workspace.clone(),
        mode: super::to_command_mode(app.mode),
    }
}

pub(crate) fn prune_undone_tool_context(app: &mut App, tool_id: &str) {
    // A display/control id alone must not choose between duplicated or mixed
    // legacy/local history. Refuse to prune when the source is ambiguous.
    let mut matches = app
        .api_messages
        .iter()
        .enumerate()
        .flat_map(|(msg_idx, msg)| {
            msg.content
                .iter()
                .enumerate()
                .filter_map(move |(block_idx, block)| {
                    (matches!(block, ContentBlock::ToolUse { .. })
                        && block.tool_call_key().is_some_and(|key| {
                            !key.as_str().trim().is_empty() && key.as_str() == tool_id
                        }))
                    .then_some((msg_idx, block_idx))
                })
        });
    let Some((msg_idx, block_idx)) = matches.next() else {
        return;
    };
    if matches.next().is_some() {
        return;
    }
    drop(matches);
    if let Some(history_idx) = app.tool_cells.get(tool_id).copied() {
        app.truncate_history_to(history_idx);
    }
    let kept_blocks = app.api_messages[msg_idx].content[..block_idx].to_vec();
    let kept_tool_ids: std::collections::HashSet<_> = kept_blocks
        .iter()
        .filter_map(|block| match block {
            ContentBlock::ToolUse { id, .. } => block.tool_call_key().map(|key| (key, id.as_str())),
            _ => None,
        })
        .collect();
    if kept_blocks.is_empty() {
        app.truncate_api_messages(msg_idx);
        return;
    }
    // Preserve surviving result blocks even when a message also contains the
    // undone result; retain the stamp of the original message.
    let preserved_tool_results: Vec<_> = app
        .api_messages_stamped()
        .skip(msg_idx + 1)
        .take_while(|(msg, _)| {
            msg.role == "user"
                && !msg.content.is_empty()
                && msg
                    .content
                    .iter()
                    .all(|block| tool_result_id(block).is_some())
        })
        .filter_map(|(msg, stamp)| {
            let mut retained = msg.clone();
            retained.content.retain(|block| {
                tool_result_id(block).is_some_and(|key| kept_tool_ids.contains(&key))
            });
            (!retained.content.is_empty()).then_some((retained, stamp))
        })
        .collect();
    app.truncate_api_messages(msg_idx + 1);
    app.api_messages_mut()[msg_idx].content = kept_blocks;
    for (message, stamp) in preserved_tool_results {
        app.push_api_message_stamped(message, stamp);
    }
}

fn prune_undone_turn_context(app: &mut App) {
    if let Some(history_idx) = app
        .history
        .iter()
        .rposition(|cell| matches!(cell, HistoryCell::User { .. }))
    {
        app.truncate_history_to(history_idx);
    }

    if let Some(api_idx) = app.api_messages.iter().rposition(|msg| msg.role == "user") {
        app.truncate_api_messages(api_idx);
    }
}

fn tool_result_id(block: &ContentBlock) -> Option<(codewhale_models::ToolCallKey<'_>, &str)> {
    match block {
        ContentBlock::ToolResult { tool_use_id, .. }
        | ContentBlock::ToolSearchToolResult { tool_use_id, .. }
        | ContentBlock::CodeExecutionToolResult { tool_use_id, .. } => {
            block.tool_call_key().map(|key| (key, tool_use_id.as_str()))
        }
        _ => None,
    }
}

/// Deepest fork chain [`snapshot_owners`] follows. A chain this long is
/// already unusual; the bound only stops a corrupt lineage from looping.
const MAX_FORK_ANCESTORS: usize = 32;

/// A session whose restore points this conversation owns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::commands) struct SnapshotOwner {
    /// Session tag the snapshots carry.
    pub(in crate::commands) session_id: String,
    /// Newest snapshot time (Unix seconds) owned from this session: `None`
    /// for the current session, the fork time for a session it was forked
    /// from. The source keeps working after the fork, and its later
    /// snapshots are not the fork's.
    pub(in crate::commands) until: Option<i64>,
}

impl SnapshotOwner {
    fn owns(&self, snapshot: &crate::snapshot::Snapshot) -> bool {
        snapshot.session_id.as_deref() == Some(self.session_id.as_str())
            && self.until.is_none_or(|until| snapshot.timestamp <= until)
    }
}

/// The sessions whose restore points `/undo` may use: the current session,
/// and for a fork each session it was forked from, up to the fork. A fork
/// copies its source's turns, so the snapshots those turns took (tagged
/// with the source's id) are the fork's too, as the Runtime's thread-owned
/// restore points are (#6621).
///
/// Lineage the saved sessions cannot prove ends the chain: fewer owners
/// means fewer restorable steps, never someone else's.
pub(in crate::commands) fn snapshot_owners(app: &App) -> Vec<SnapshotOwner> {
    let Some(current) = app.current_session_id.clone() else {
        return Vec::new();
    };
    let manager = crate::session_manager::SessionManager::default_location().ok();
    let load = |id: &str| {
        manager
            .as_ref()
            .and_then(|manager| manager.load_session_metadata_by_id(id).ok())
    };
    let mut metadata = app
        .current_session_metadata
        .clone()
        .filter(|metadata| metadata.id == current)
        .or_else(|| load(&current));
    let mut owners = vec![SnapshotOwner {
        session_id: current,
        until: None,
    }];
    while let Some(child) = metadata.take() {
        let Some(parent) = child.parent_session_id.clone() else {
            break;
        };
        if owners.len() > MAX_FORK_ANCESTORS
            || owners.iter().any(|owner| owner.session_id == parent)
        {
            break;
        }
        let forked_at = child.created_at.timestamp();
        let until = owners
            .last()
            .and_then(|owner| owner.until)
            .map_or(forked_at, |child_until| child_until.min(forked_at));
        metadata = load(&parent);
        owners.push(SnapshotOwner {
            session_id: parent,
            until: Some(until),
        });
    }
    owners
}

/// Labels a `/undo` step starts at: before one tool call, or before a turn.
fn is_undo_step_label(label: &str) -> bool {
    label.starts_with("tool:") || label.starts_with("pre-turn:")
}

/// Labels of the restore points an engine takes for a turn. A step runs from
/// one of them to the next one the conversation owns.
fn is_restore_point_label(label: &str) -> bool {
    is_undo_step_label(label) || label.starts_with("post-tool:") || label.starts_with("post-turn:")
}

/// One `/undo` step, planned but not applied.
pub(in crate::commands) struct UndoStep {
    /// Restore point the step started at.
    pub(in crate::commands) target: crate::snapshot::Snapshot,
    /// Tree the step ended at: the next restore point this conversation
    /// owns, or, for the newest step, a snapshot of the workspace as it is
    /// now. Trees, not commit ids, because a prune rewrites commit ids.
    pub(in crate::commands) end: crate::snapshot::SnapshotId,
    /// The paths the step changed that are still as it left them.
    pub(in crate::commands) restore: Vec<PathBuf>,
    /// Changed paths `/undo` leaves in place because they are not regular
    /// files (a symlink, a directory, a submodule), in this step or in a
    /// newer one it walked past.
    pub(in crate::commands) skipped: Vec<PathBuf>,
    /// The `pre-restore:` snapshot planning took of the workspace, when the
    /// step ends now; the restore reuses it as its safety backup.
    pub(in crate::commands) backup: Option<crate::snapshot::SnapshotId>,
}

/// Find the newest step of `snapshots` (newest first) that `owners` own and
/// that is not undone yet, and the paths undoing it restores.
///
/// A step is scoped to the paths that changed between its restore point and
/// the next one: edits to any other file (the user's, another session's)
/// are never touched. A path the step changed that changed again since is
/// refused rather than overwritten. A step whose paths are all back at its
/// restore point is already undone, so `/undo` walks back one tool call (or
/// turn) at a time (#384). A changed path that is not a regular file is
/// left in place and reported (file-scoped restore never writes symlinks or
/// directories); it does not block the step's other paths or older steps.
///
/// Planning writes nothing, except when the newest step ends now: the
/// workspace is then snapshotted, and only when `trusted`, since `/undo`
/// outside trusted mode refuses to touch files anyway.
///
/// Known limits: the TUI records no per-tool receipts (the Runtime's
/// `post-tool:` spans and declared write paths), so a step owns everything
/// that changed between its restore point and the next one this
/// conversation owns, including a write another session made in that window.
/// The newest step, when no later restore point exists yet (the turn is still
/// running, or its post-turn snapshot failed), ends at the workspace as it
/// is now, so an edit made since the step's restore point counts as the
/// step's. [`undo_files`] first waits for a post-turn snapshot this process
/// is still taking, so this is not the case right after a turn.
// The planner is host-internal. Box only its uncommon early outcome to keep
// Result small; the public facet still returns an owned data-only value.
fn plan_undo_step(
    repo: &crate::snapshot::SnapshotRepo,
    snapshots: Vec<crate::snapshot::Snapshot>,
    owners: &[SnapshotOwner],
    trusted: bool,
) -> Result<UndoStep, Box<DebugUndoOutcome>> {
    let owned: Vec<crate::snapshot::Snapshot> = snapshots
        .into_iter()
        .filter(|snapshot| is_restore_point_label(&snapshot.label))
        .filter(|snapshot| owners.iter().any(|owner| owner.owns(snapshot)))
        .collect();
    if !owned
        .iter()
        .any(|snapshot| is_undo_step_label(&snapshot.label))
    {
        return Err(Box::new(DebugUndoOutcome::NoOwnedSteps));
    }

    let compare_failed = |error: std::io::Error| DebugUndoOutcome::CompareFailed(error.to_string());
    // `InvalidInput` from a path comparison: the path is not a regular file
    // (or not a safe workspace path) on one side, so it is left alone.
    let unrestorable = |error: &std::io::Error| error.kind() == std::io::ErrorKind::InvalidInput;

    let mut skipped: Vec<PathBuf> = Vec::new();
    for (index, target) in owned.iter().enumerate() {
        if !is_undo_step_label(&target.label) {
            continue;
        }
        let mut backup = None;
        let end = match index.checked_sub(1) {
            Some(newer) => owned[newer].tree.clone(),
            // The newest step has no later restore point (the turn is still
            // running, stopped early, or its post-turn snapshot has not
            // landed): the workspace now is the only record of its end.
            None => {
                if repo
                    .work_tree_matches_snapshot(&target.tree)
                    .map_err(compare_failed)?
                {
                    continue;
                }
                if !trusted {
                    return Err(Box::new(DebugUndoOutcome::Untrusted));
                }
                let short = &target.id.as_str()[..target.id.as_str().len().min(12)];
                let taken = repo
                    .take_snapshot(&format!("pre-restore:{short}"), None)
                    .map_err(|error| DebugUndoOutcome::SnapshotFailed(error.to_string()))?;
                backup = Some(taken.id);
                taken.tree
            }
        };
        let changed = repo
            .changed_paths_between(&target.tree, &end)
            .map_err(compare_failed)?;
        let mut restore = Vec::new();
        let mut changed_since = Vec::new();
        'paths: for path in changed {
            match repo.path_matches_snapshot(&end, &path) {
                // Still as the step left it. Comparing the step's start too
                // proves it holds a regular file (or nothing) to restore.
                Ok(true) => match repo.path_same_in_snapshots(&target.tree, &end, &path) {
                    Ok(_) => {
                        restore.push(path);
                        continue;
                    }
                    Err(error) if unrestorable(&error) => {
                        skipped.push(path);
                        continue;
                    }
                    Err(error) => return Err(Box::new(compare_failed(error))),
                },
                Ok(false) => {}
                Err(error) if unrestorable(&error) => {
                    skipped.push(path);
                    continue;
                }
                Err(error) => return Err(Box::new(compare_failed(error))),
            }
            // Back at the step's start, or at an older restore point that an
            // earlier `/undo` walked it back to: already undone.
            for older in &owned[index..] {
                match repo.path_matches_snapshot(&older.tree, &path) {
                    Ok(true) => continue 'paths,
                    Ok(false) => {}
                    Err(error) if unrestorable(&error) => {
                        skipped.push(path);
                        continue 'paths;
                    }
                    Err(error) => return Err(Box::new(compare_failed(error))),
                }
            }
            changed_since.push(path.display().to_string());
        }
        if !changed_since.is_empty() {
            return Err(Box::new(DebugUndoOutcome::ChangedSince {
                label: target.label.clone(),
                paths: changed_since,
            }));
        }
        if restore.is_empty() {
            // Already undone, changed nothing, or changed only paths `/undo`
            // cannot restore: keep walking back.
            continue;
        }
        skipped.sort();
        skipped.dedup();
        return Ok(UndoStep {
            target: target.clone(),
            end,
            restore,
            skipped,
            backup,
        });
    }
    Err(Box::new(DebugUndoOutcome::NoDifference))
}

/// How long `/undo` waits for a post-turn snapshot still being written.
const POST_TURN_SNAPSHOT_WAIT: std::time::Duration = std::time::Duration::from_secs(10);

/// Revert the most recent write tool (apply_patch/edit_file/write_file) or turn.
///
/// Opens the side-git snapshot repo and finds the newest `tool:*` or
/// `pre-turn:*` restore point this conversation owns (see
/// [`snapshot_owners`]) whose step is not undone yet, then restores only the
/// files that step changed (see [`plan_undo_step`]). Falls back to
/// conversation undo when no snapshots exist.
///
/// Posts a `HistoryCell::System` entry so the user can see what was
/// reverted in the transcript.
/// Why workspace files may not be rolled back right now, if they may not.
///
/// A running turn is reading and writing this workspace: restoring files
/// under it discards the turn's in-flight work and leaves the model's view of
/// the files wrong. `/undo` and `/restore` refuse while one is active, like
/// the Runtime's restore routes.
pub(in crate::commands) fn active_turn_restore_refusal(app: &App) -> Option<String> {
    let turn_active = app.is_loading
        || app.is_compacting
        || matches!(app.runtime_turn_status.as_deref(), Some("in_progress"));
    turn_active.then(|| {
        "A turn is still running in this workspace, so files were not restored and nothing was changed. Wait for it to finish, or press Esc to stop it, then run the command again."
            .to_string()
    })
}

pub(in crate::commands) fn undo_files(app: &mut App) -> DebugUndoOutcome {
    if let Some(refusal) = active_turn_restore_refusal(app) {
        return DebugUndoOutcome::RestoreBlocked(refusal);
    }
    let workspace = app.workspace.clone();

    let repo = match crate::snapshot::SnapshotRepo::open_or_init(&workspace) {
        Ok(r) => r,
        Err(e) => {
            return DebugUndoOutcome::RepoUnavailable {
                workspace,
                error: e.to_string(),
            };
        }
    };

    // A post-turn snapshot this process is still taking is the newest step's
    // end: without it, every edit since the step's restore point would count
    // as the step's.
    if !crate::snapshot::wait_for_pending_post_turn_snapshots(POST_TURN_SNAPSHOT_WAIT) {
        return DebugUndoOutcome::SnapshotPending;
    }

    // The whole store: an older restore point that is still stored must not
    // be mistaken for a pruned one.
    let snapshots = match repo.list(usize::MAX) {
        Ok(s) => s,
        Err(e) => {
            return DebugUndoOutcome::ListFailed(e.to_string());
        }
    };

    if snapshots.is_empty() {
        return DebugUndoOutcome::NoSnapshots;
    }

    // Automatic file rollback is allowed only when ownership is provable.
    // Untagged legacy snapshots and snapshots from another conversation may
    // describe unrelated user work in this same workspace, so fail closed
    // and let the command dispatcher fall back to conversation-only undo.
    let owners = snapshot_owners(app);
    if owners.is_empty() {
        return DebugUndoOutcome::NoSession;
    }

    // Restoring workspace files is a mutation. Apply the trust gate only
    // after finding a real, owned step so chat-only `/undo` can still fall
    // back to conversation history in ordinary mode; planning itself writes
    // nothing outside trusted mode.
    let trusted = app.yolo || app.trust_mode;
    let step = match plan_undo_step(&repo, snapshots, &owners, trusted) {
        Ok(step) => step,
        Err(outcome) => return *outcome,
    };
    let target = &step.target;
    if !trusted {
        return DebugUndoOutcome::Untrusted;
    }

    let plan: Vec<(PathBuf, crate::snapshot::SnapshotId)> = step
        .restore
        .iter()
        .map(|path| (path.clone(), target.tree.clone()))
        .collect();
    // Re-verify after the safety snapshot, immediately before the first
    // write: a change that landed meanwhile is refused.
    let preflight = || {
        for path in &step.restore {
            if !repo.path_matches_snapshot(&step.end, path)? {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::WouldBlock,
                    format!(
                        "'{}' changed while the undo was being prepared; nothing was changed.",
                        path.display()
                    ),
                ));
            }
        }
        Ok(())
    };
    let restored = match &step.backup {
        // Planning already snapshotted the workspace (and `preflight` proves
        // every planned path is still as that snapshot holds it).
        Some(backup) => repo.restore_path_plan_with_backup(&plan, backup, true, preflight),
        None => {
            let backup_short = &target.id.as_str()[..target.id.as_str().len().min(12)];
            repo.restore_path_plan(
                &plan,
                &format!("pre-restore:{backup_short}"),
                true,
                preflight,
            )
        }
    };
    let outcomes = match restored {
        Ok(outcomes) => outcomes,
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
            return DebugUndoOutcome::RestoreBlocked(e.to_string());
        }
        Err(e) => return DebugUndoOutcome::RestoreFailed(e.to_string()),
    };

    if let Some(tool_id) = target.label.strip_prefix("tool:") {
        prune_undone_tool_context(app, tool_id);
    } else if target.label.starts_with("pre-turn:") {
        prune_undone_turn_context(app);
    }

    let short = &target.id.as_str()[..target.id.as_str().len().min(8)];
    // Post a system cell so the reverted state is visible in the transcript.
    app.push_history_cell(HistoryCell::System {
        content: format!(
            "/undo reverted workspace files to snapshot '{}' ({})",
            target.label, short
        ),
    });

    DebugUndoOutcome::Restored(DebugUndoRestored {
        label: target.label.clone(),
        snapshot_id: target.id.as_str().to_string(),
        files: outcomes
            .into_iter()
            .map(|outcome| DebugRestoredFile {
                path: outcome.path,
                action: match outcome.action {
                    crate::snapshot::PathRestoreAction::Modified => DebugRestoreAction::Modified,
                    crate::snapshot::PathRestoreAction::Recreated => DebugRestoreAction::Recreated,
                    crate::snapshot::PathRestoreAction::Removed => DebugRestoreAction::Removed,
                },
            })
            .collect(),
        skipped: step.skipped,
        sync: session_sync_payload(app),
    })
}
