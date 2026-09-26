//! Turn artifact routes (#6653): what a runtime turn produced.
//!
//! The authority is the runtime store's turn record and its items. Nothing
//! is scanned: every reference was recorded where its bytes were written,
//! and the workspace delta comes from the engine's own snapshot pair. Reads
//! go through the same confined openers as the workspace file and session
//! artifact routes; there is no second store.

use std::path::Path as FsPath;

use axum::Json;
use axum::extract::{Path, Query, State};
use serde::{Deserialize, Serialize};

use super::sessions::{ArtifactAuthority, resolve_session_artifact};
use super::workspace::{
    ConfinedFileBytes, FILE_SERVE_MAX_BYTES, canonical_workspace, encode_window,
    open_confined_file, parse_read_window, precheck_file_target, read_confined_bytes, read_window,
    relative_request_path,
};
use super::{ApiError, RuntimeApiState, map_thread_err};
use crate::runtime_threads::{
    FileChangeKind, TurnArtifactKind, TurnArtifactRef, TurnArtifactsView, TurnWorkspaceArtifacts,
};

pub(super) async fn list_turn_artifacts(
    State(state): State<RuntimeApiState>,
    Path((thread_id, turn_id)): Path<(String, String)>,
) -> Result<Json<TurnArtifactsView>, ApiError> {
    load_view(&state, &thread_id, &turn_id).await.map(Json)
}

async fn load_view(
    state: &RuntimeApiState,
    thread_id: &str,
    turn_id: &str,
) -> Result<TurnArtifactsView, ApiError> {
    state
        .runtime_threads
        .turn_artifacts(thread_id, turn_id)
        .await
        .map_err(map_thread_err)?
        .ok_or_else(|| ApiError::not_found(format!("turn '{turn_id}' not found in this thread")))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TurnArtifactReadQuery {
    offset: Option<usize>,
    limit: Option<usize>,
    /// Read an intermediate revision one of the turn's items recorded,
    /// instead of the reference's final one.
    revision: Option<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct TurnArtifactReadResponse {
    artifact: TurnArtifactRef,
    /// `workspace` (the live file), `snapshot` (the turn's post-turn
    /// snapshot), or `session_artifact` (the session artifact directory).
    source: &'static str,
    /// Whether the workspace still holds exactly these bytes. `null` for a
    /// session artifact, or a file reference with no recorded revision.
    current: Option<bool>,
    size: u64,
    /// SHA-256 of the whole content served.
    revision: String,
    offset: usize,
    bytes: usize,
    truncated: bool,
    encoding: &'static str,
    content: String,
}

pub(super) async fn read_turn_artifact(
    State(state): State<RuntimeApiState>,
    Path((thread_id, turn_id, artifact_id)): Path<(String, String, String)>,
    Query(query): Query<TurnArtifactReadQuery>,
) -> Result<Json<TurnArtifactReadResponse>, ApiError> {
    let (offset, limit) = parse_read_window(query.offset, query.limit)?;
    let view = load_view(&state, &thread_id, &turn_id).await?;
    let artifact = select_reference(&view, &artifact_id, query.revision.as_deref())?;
    let workspace = view.workspace.clone();
    let thread_workspace = view.thread_workspace.clone();
    tokio::task::spawn_blocking(move || {
        let (source, current, read) = match artifact.kind {
            TurnArtifactKind::File => {
                read_file_artifact(&thread_workspace, workspace.as_ref(), &artifact)?
            }
            TurnArtifactKind::ToolOutput | TurnArtifactKind::Media => {
                let session_id = artifact
                    .session_id
                    .as_deref()
                    .ok_or_else(|| ApiError::internal("artifact reference has no session"))?;
                let root = crate::artifacts::artifact_sessions_root()
                    .ok_or_else(|| ApiError::internal("session artifact root is unavailable"))?;
                let resolved = resolve_session_artifact(
                    &root,
                    session_id,
                    &artifact.id,
                    ArtifactAuthority::TurnRef {
                        path: &artifact.path,
                        revision: artifact.revision.as_deref(),
                    },
                )?;
                ("session_artifact", None, resolved.read)
            }
        };
        let (window, truncated) = read_window(&read.bytes, offset, limit);
        let (encoding, content) = encode_window(window);
        Ok(TurnArtifactReadResponse {
            artifact,
            source,
            current,
            size: read.size,
            revision: read.revision,
            offset: offset.min(read.bytes.len()),
            bytes: window.len(),
            truncated,
            encoding,
            content,
        })
    })
    .await
    .map_err(|_| ApiError::internal("turn artifact read failed"))?
    .map(Json)
}

/// The reference to serve: the turn aggregate's, or an item's when the turn
/// aggregate no longer lists it (a file written and then deleted still has
/// item-level history). `?revision=` selects the item ref that recorded
/// exactly that revision.
fn select_reference(
    view: &TurnArtifactsView,
    artifact_id: &str,
    revision: Option<&str>,
) -> Result<TurnArtifactRef, ApiError> {
    let aggregate = view.artifacts.iter().find(|r| r.id == artifact_id);
    let items = view
        .item_artifacts
        .iter()
        .rev()
        .filter(|r| r.id == artifact_id);
    let Some(wanted) = revision.map(|revision| revision.trim().to_ascii_lowercase()) else {
        return aggregate
            .or_else(|| {
                view.item_artifacts
                    .iter()
                    .rev()
                    .find(|r| r.id == artifact_id)
            })
            .cloned()
            .ok_or_else(|| ApiError::not_found("artifact not found in this turn"));
    };
    aggregate
        .into_iter()
        .chain(items)
        .find(|r| r.revision.as_deref() == Some(wanted.as_str()))
        .cloned()
        .ok_or_else(|| ApiError::not_found("this turn recorded no such revision of the artifact"))
}

/// Serve a file reference: from the workspace when it still holds the
/// recorded revision, otherwise from the turn's post-turn snapshot,
/// otherwise a conflict naming what the workspace holds now.
fn read_file_artifact(
    thread_workspace: &FsPath,
    workspace: Option<&TurnWorkspaceArtifacts>,
    artifact: &TurnArtifactRef,
) -> Result<(&'static str, Option<bool>, ConfinedFileBytes), ApiError> {
    if artifact.change == Some(FileChangeKind::Deleted) {
        return Err(ApiError::gone(
            "this turn deleted the file; restore its prior content with file-revert and the reference's restore_snapshot_id",
        ));
    }
    if artifact
        .size
        .is_some_and(|size| size > FILE_SERVE_MAX_BYTES)
    {
        return Err(ApiError::payload_too_large(format!(
            "file is larger than the {FILE_SERVE_MAX_BYTES}-byte serving limit"
        )));
    }
    let relative = relative_request_path(&artifact.path, false)?;
    let root = canonical_workspace(thread_workspace)?;
    let live = match precheck_file_target(&root, &relative)? {
        Some(_) => Some(read_confined_bytes(&open_confined_file(
            &root, &relative, false,
        )?)?),
        None => None,
    };
    let Some(wanted) = artifact.revision.as_deref() else {
        // No recorded revision (an older receipt): all that can be served
        // is the live file, and whether it is the turn's version is unknown.
        return live
            .map(|read| ("workspace", None, read))
            .ok_or_else(|| ApiError::gone("the file is no longer in the workspace"));
    };
    if let Some(read) = live.as_ref()
        && read.revision == wanted
    {
        return Ok(("workspace", Some(true), live.expect("checked above")));
    }
    if let Some(post) = workspace.and_then(|w| w.post_turn_snapshot_id.as_deref())
        && let Some(bytes) = read_snapshot_blob(&root, post, &artifact.path)?
    {
        let read = ConfinedFileBytes {
            size: bytes.len() as u64,
            revision: super::workspace::content_revision(&bytes),
            modified: None,
            bytes,
        };
        if read.revision == wanted {
            return Ok(("snapshot", Some(false), read));
        }
    }
    Err(ApiError::conflict(match live {
        Some(read) => format!(
            "this turn's revision is no longer in the workspace or the snapshot store; current revision is {}",
            read.revision
        ),
        None => "this turn's revision is no longer in the workspace or the snapshot store; the file is gone".to_string(),
    }))
}

fn read_snapshot_blob(
    workspace: &FsPath,
    snapshot_id: &str,
    path: &str,
) -> Result<Option<Vec<u8>>, ApiError> {
    let Ok(id) = crate::snapshot::SnapshotId::parse(snapshot_id) else {
        return Ok(None);
    };
    let Some(repo) = crate::snapshot::SnapshotRepo::open_existing(workspace)
        .map_err(|error| ApiError::internal(format!("snapshot repo unavailable: {error}")))?
    else {
        return Ok(None);
    };
    match repo.read_blob(&id, path, FILE_SERVE_MAX_BYTES) {
        Ok(bytes) => Ok(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::FileTooLarge => {
            Err(ApiError::payload_too_large(format!(
                "file is larger than the {FILE_SERVE_MAX_BYTES}-byte serving limit"
            )))
        }
        // A pruned snapshot is not an error: the revision is just gone.
        Err(error) => {
            tracing::debug!(%error, "snapshot blob read failed");
            Ok(None)
        }
    }
}
