//! LSP-over-HTTP for native clients (APPS-93).
//!
//! A native file view needs live diagnostics and semantic references, not just
//! receipt-projected ones. These routes serve the *server workspace* through
//! one lazily-built [`LspManager`]; engine threads keep their own per-thread
//! managers for the post-edit hook. Language servers spawn on first use only —
//! a server that never serves an LSP route never pays for one.
//!
//! Fail-closed as data: a file with no language server, a disabled `[lsp]`
//! config, or an LSP timeout all answer `200` with `ok: false` + a machine-
//! readable `reason`. Only malformed input (bad path, missing `line`) is an
//! HTTP error — a file without a server is a normal state, not a failure.
//!
//! Routes:
//!   GET /v1/lsp          — capability: enabled, languages, operations
//!   GET /v1/diagnostics  — ?path= (workspace-relative)
//!   GET /v1/definition   — ?path=&line=&character= (1-based)
//!   GET /v1/references   — ?path=&line=&character= (1-based)
//!   GET /v1/symbols      — ?path=&query= (query empty → document symbols)

use std::path::PathBuf;
use std::sync::Arc;

use axum::Json;
use axum::extract::{Query, State};
use serde::Deserialize;
use serde_json::{Value, json};

use super::workspace::{canonical_workspace, precheck_file_target, relative_request_path};
use super::{ApiError, RuntimeApiState};
use crate::lsp::LspManager;
use crate::lsp::registry::{self, Language};

const LSP_OPERATIONS: &[&str] = &["diagnostics", "symbols", "definition", "references"];
const LANGUAGES: &[Language] = &[
    Language::Rust,
    Language::Go,
    Language::Python,
    Language::TypeScript,
    Language::JavaScript,
    Language::Java,
    Language::Php,
    Language::Vue,
    Language::C,
    Language::Cpp,
];

/// The shared workspace manager: built once, from the live config's `[lsp]`
/// table and the canonical workspace root.
fn lsp_manager(state: &RuntimeApiState) -> Result<Arc<LspManager>, ApiError> {
    if let Some(manager) = state.lsp_manager.get() {
        return Ok(manager.clone());
    }
    let workspace = state
        .workspace
        .canonicalize()
        .map_err(|_| ApiError::internal("workspace is unavailable"))?;
    let config = state
        .config
        .read()
        .lsp
        .clone()
        .map(|toml| toml.into_runtime())
        .unwrap_or_default();
    Ok(state
        .lsp_manager
        .get_or_init(|| Arc::new(LspManager::new(config, workspace)))
        .clone())
}

/// Resolve a workspace-relative `path` to an absolute file inside the
/// workspace, refusing traversal, `.git`, links, and missing files — the same
/// confinement the file routes apply.
///
/// Async because the confinement it applies is filesystem work:
/// `canonical_workspace` canonicalizes and `precheck_file_target` stats every
/// component. Both are blocking syscalls, so they ride `spawn_blocking` rather
/// than the caller's Tokio worker (#6149). The blocking-calls budget cannot
/// see this — the `std::fs` calls live in `workspace.rs`, so a handler calling
/// straight through to them is invisible to a per-file scanner.
async fn resolve_workspace_file(state: &RuntimeApiState, raw: &str) -> Result<PathBuf, ApiError> {
    let relative = relative_request_path(raw, false)?;
    let workspace = state.workspace.clone();
    tokio::task::spawn_blocking(move || {
        let root = canonical_workspace(&workspace)?;
        precheck_file_target(&root, &relative)?
            .ok_or_else(|| ApiError::not_found("file not found"))?;
        Ok(root.join(&relative))
    })
    .await
    .map_err(|_| ApiError::internal("workspace file resolution failed"))?
}

/// `intelligence` reports ordinary states (disabled, no server) as error
/// strings; split them back into the honest machine-readable reasons.
fn lsp_failure(error: String) -> Value {
    let reason = if error.contains("no LSP server") {
        "no_server"
    } else if error.contains("disabled") {
        "lsp_disabled"
    } else {
        "lsp_error"
    };
    json!({ "ok": false, "reason": reason, "detail": error })
}

async fn run_intelligence(
    state: &RuntimeApiState,
    operation: &str,
    file: PathBuf,
    line: Option<u32>,
    character: Option<u32>,
    query: Option<String>,
) -> Result<Json<Value>, ApiError> {
    let manager = lsp_manager(state)?;
    if !manager.config().enabled {
        return Ok(Json(
            json!({ "ok": false, "reason": "lsp_disabled", "enabled": false }),
        ));
    }
    let result = manager
        .intelligence(operation, &file, line, character, query.as_deref())
        .await;
    match result {
        Ok(mut value) => {
            if let Some(object) = value.as_object_mut() {
                object.insert("ok".to_string(), Value::Bool(true));
            }
            Ok(Json(value))
        }
        Err(error) => Ok(Json(lsp_failure(error))),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LspFileQuery {
    path: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LspPositionQuery {
    path: String,
    /// 1-based line; required by definition/references.
    line: Option<u32>,
    /// 1-based column; defaults to 1.
    character: Option<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LspSymbolsQuery {
    path: String,
    query: Option<String>,
}

/// `GET /v1/lsp` — capability report: what this runtime can serve without
/// probing or spawning anything.
pub(super) async fn lsp_status(
    State(state): State<RuntimeApiState>,
) -> Result<Json<Value>, ApiError> {
    let manager = lsp_manager(&state)?;
    let config = manager.config();
    let languages: Vec<Value> = LANGUAGES
        .iter()
        .filter_map(|language| {
            registry::server_for(*language)
                .map(|(command, _)| json!({ "language": language.as_key(), "server": command }))
        })
        .collect();
    let custom: Vec<Value> = config
        .custom
        .iter()
        .map(|(extension, def)| {
            json!({
                "extension": extension,
                "language_id": def.language_id,
                "server": def.command,
            })
        })
        .collect();
    Ok(Json(json!({
        "enabled": config.enabled,
        "workspace": state.workspace.display().to_string(),
        "operations": LSP_OPERATIONS,
        "languages": languages,
        "custom_languages": custom,
        "poll_after_edit_ms": config.poll_after_edit_ms,
        "max_diagnostics_per_file": config.max_diagnostics_per_file,
        "include_warnings": config.include_warnings,
    })))
}

pub(super) async fn lsp_diagnostics(
    State(state): State<RuntimeApiState>,
    Query(query): Query<LspFileQuery>,
) -> Result<Json<Value>, ApiError> {
    let file = resolve_workspace_file(&state, &query.path).await?;
    run_intelligence(&state, "diagnostics", file, None, None, None).await
}

pub(super) async fn lsp_definition(
    State(state): State<RuntimeApiState>,
    Query(query): Query<LspPositionQuery>,
) -> Result<Json<Value>, ApiError> {
    let line = query
        .line
        .ok_or_else(|| ApiError::bad_request("definition requires line (1-based)"))?;
    let file = resolve_workspace_file(&state, &query.path).await?;
    run_intelligence(
        &state,
        "definition",
        file,
        Some(line),
        query.character,
        None,
    )
    .await
}

pub(super) async fn lsp_references(
    State(state): State<RuntimeApiState>,
    Query(query): Query<LspPositionQuery>,
) -> Result<Json<Value>, ApiError> {
    let line = query
        .line
        .ok_or_else(|| ApiError::bad_request("references requires line (1-based)"))?;
    let file = resolve_workspace_file(&state, &query.path).await?;
    run_intelligence(
        &state,
        "references",
        file,
        Some(line),
        query.character,
        None,
    )
    .await
}

pub(super) async fn lsp_symbols(
    State(state): State<RuntimeApiState>,
    Query(query): Query<LspSymbolsQuery>,
) -> Result<Json<Value>, ApiError> {
    let file = resolve_workspace_file(&state, &query.path).await?;
    run_intelligence(&state, "symbols", file, None, None, query.query).await
}
