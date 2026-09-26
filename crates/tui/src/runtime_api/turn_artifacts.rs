//! Turn artifact routes (#6653): what a runtime turn produced.
//!
//! The authority is the runtime store's turn record and its items. Nothing
//! is scanned: every reference was recorded where its bytes were written,
//! and the workspace delta comes from the engine's own snapshot pair.

use axum::Json;
use axum::extract::{Path, State};

use super::{ApiError, RuntimeApiState, map_thread_err};
use crate::runtime_threads::TurnArtifactsView;

pub(super) async fn list_turn_artifacts(
    State(state): State<RuntimeApiState>,
    Path((thread_id, turn_id)): Path<(String, String)>,
) -> Result<Json<TurnArtifactsView>, ApiError> {
    state
        .runtime_threads
        .turn_artifacts(&thread_id, &turn_id)
        .await
        .map_err(map_thread_err)?
        .map(Json)
        .ok_or_else(|| ApiError::not_found(format!("turn '{turn_id}' not found in this thread")))
}
