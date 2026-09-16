//! Workspace git surface for native clients (APPS-106).
//!
//! One authority: these routes run `git` against the server's configured
//! workspace through the same hardened primitives the agent tools use —
//! reads go through [`Git::review_command`] (filters, fsmonitor, hooks,
//! lazy fetches and replace-objects neutralized), writes run through
//! [`Git::tokio_command`] with interactive prompts disabled so a credential
//! or host-key prompt can never hang an HTTP request. There is no second
//! index, cache, or diff store here; every response is computed live from
//! the repository.
//!
//! Routes (workspace-scoped, matching the client contract):
//!   GET  /v1/git           — branch/head/ahead-behind, per-file porcelain
//!                            entries, local branches and remotes
//!   GET  /v1/changes       — the file-change inventory only (status rows)
//!   GET  /v1/diff?path=    — unified diff for one workspace-relative file
//!   GET  /v1/workspace/diff — bounded whole-tree diff + per-file numstat
//!   GET  /v1/git/graph     — recent commit graph rows (bounded)
//!   POST /v1/git/stage     — `{ "paths": [...] }` or `{ "all": true }`
//!   POST /v1/git/unstage   — `{ "paths": [...] }` or `{ "all": true }`
//!   POST /v1/git/discard   — `{ "paths": [...] }` (tracked only; no `all`)
//!   POST /v1/git/commit    — `{ "message": "…", "all": false }`
//!   POST /v1/git/push      — `{ "remote"?, "set_upstream"?: bool }`
//!   POST /v1/git/branch    — `{ "name": "…", "create"?: bool }`
//!
//! Mutations answer with the command output tail plus a refreshed workspace
//! status so the client re-renders in one round trip. The caller holds the
//! operator token; the repository's own hooks run for `commit` exactly as
//! they would for the user's terminal.

use std::path::{Path as FsPath, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use axum::Json;
use axum::extract::{Query, State};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::dependencies::{ExternalTool as _, Git};

use super::workspace::{canonical_workspace, collect_workspace_status, relative_request_path};
use super::{ApiError, RuntimeApiState};

/// Generous bound for local git work on very large repositories.
const GIT_READ_TIMEOUT: Duration = Duration::from_secs(30);
/// Pushes cross the network; still bounded so a dead remote cannot pin a
/// handler forever.
const GIT_WRITE_TIMEOUT: Duration = Duration::from_secs(120);
/// Output tail carried back to the client for mutations.
const MAX_OUTPUT_TAIL: usize = 8 * 1024;
/// Commit-graph row bounds.
const GRAPH_LIMIT_DEFAULT: usize = 100;
const GRAPH_LIMIT_MAX: usize = 500;
/// Unified-diff response caps: a single file gets a generous window; the
/// whole-tree surface defaults smaller and always reports `truncated`.
const FILE_DIFF_MAX_BYTES: usize = 512 * 1024;
const WORKSPACE_DIFF_DEFAULT_BYTES: usize = 256 * 1024;
const WORKSPACE_DIFF_MAX_BYTES: usize = 4 * 1024 * 1024;
/// The empty tree — diff base for repositories whose HEAD is unborn.
const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";
/// Branch/commit message caps — generous for real messages, hostile to
/// accidental binary paste.
const MAX_COMMIT_MESSAGE_BYTES: usize = 64 * 1024;
const MAX_BRANCH_NAME_BYTES: usize = 256;
const MAX_PATH_ARGS: usize = 512;

// ---------------------------------------------------------------------------
// git invocation
// ---------------------------------------------------------------------------

struct GitRun {
    status_success: bool,
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
}

/// Hardened read path: the same primitive review/tooling uses, so fsmonitor,
/// content filters, hooks, lazy fetches and replace-objects cannot run inside
/// an HTTP read either.
async fn git_read(workspace: &FsPath, args: &[&str]) -> Result<GitRun, ApiError> {
    let workspace = workspace.to_path_buf();
    let command = tokio::task::spawn_blocking(move || Git::review_command(&workspace))
        .await
        .map_err(|_| ApiError::internal("git read setup failed"))?
        .map_err(|error| ApiError::internal(format!("git is unavailable: {error}")))?;
    let mut command = tokio::process::Command::from(command);
    command.args(args).kill_on_drop(true);
    finish_git(command.output(), GIT_READ_TIMEOUT).await
}

/// Write path for operator-driven mutations. Non-interactive by contract:
/// no terminal prompt, no pager, and BatchMode ssh (unless the user already
/// pins their own `GIT_SSH_COMMAND`) so a key prompt can never hang the
/// request. Hooks and filters run exactly as they do for the user's own
/// `git` — a Review-sheet commit is the user's commit.
async fn git_write(workspace: &FsPath, args: Vec<String>) -> Result<GitRun, ApiError> {
    let mut command = Git::tokio_command()
        .ok_or_else(|| ApiError::internal("git is not installed or not in PATH"))?;
    command
        .args(&args)
        .current_dir(workspace)
        .stdin(Stdio::null())
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_PAGER", "")
        .kill_on_drop(true);
    if std::env::var_os("GIT_SSH_COMMAND").is_none() {
        command.env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes");
    }
    finish_git(command.output(), GIT_WRITE_TIMEOUT).await
}

async fn finish_git(
    output: impl std::future::Future<Output = std::io::Result<std::process::Output>>,
    timeout: Duration,
) -> Result<GitRun, ApiError> {
    let output = tokio::time::timeout(timeout, output)
        .await
        .map_err(|_| ApiError::internal("git operation timed out"))?
        .map_err(|error| ApiError::internal(format!("failed to run git: {error}")))?;
    Ok(GitRun {
        status_success: output.status.success(),
        exit_code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

fn output_tail(run: &GitRun) -> String {
    let mut text = String::new();
    for part in [run.stdout.trim(), run.stderr.trim()] {
        if !part.is_empty() {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(part);
        }
    }
    if text.len() > MAX_OUTPUT_TAIL {
        let mut boundary = text.len() - MAX_OUTPUT_TAIL;
        while !text.is_char_boundary(boundary) {
            boundary -= 1;
        }
        text = text[boundary..].to_string();
    }
    text
}

/// A non-repo workspace is a 404, not a 500: `rev-parse --is-inside-work-tree`
/// exits 128 there, so probe directly rather than through the erroring helper.
fn require_repo(workspace: &FsPath) -> Result<(), ApiError> {
    match Git::output(&["rev-parse", "--is-inside-work-tree"], workspace) {
        Ok(output)
            if output.status.success()
                && String::from_utf8_lossy(&output.stdout).trim() == "true" =>
        {
            Ok(())
        }
        Ok(_) => Err(ApiError::not_found("workspace is not a git repository")),
        Err(error) => Err(ApiError::internal(format!("failed to run git: {error}"))),
    }
}

/// Cheap one-shot read used only for repo probes where the hardened review
/// command's filter dance would be wasted work.
fn run_git_sync(workspace: &FsPath, args: &[&str]) -> Result<String, ApiError> {
    let output = Git::output(args, workspace)
        .map_err(|error| ApiError::internal(format!("failed to run git: {error}")))?;
    if !output.status.success() {
        return Err(ApiError::internal(format!(
            "git {} failed: {}",
            args.first().copied().unwrap_or(""),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

// ---------------------------------------------------------------------------
// GET /v1/git — status detail
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct GitFileEntry {
    path: String,
    /// Raw porcelain v1 index (X) and worktree (Y) columns.
    index: String,
    worktree: String,
    /// True when the index column records a change.
    staged: bool,
    /// Leading human state: modified / added / deleted / renamed /
    /// typechange / untracked / conflicted / ignored.
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    old_path: Option<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct GitStatusDetailResponse {
    git_repo: bool,
    workspace: PathBuf,
    branch: Option<String>,
    detached: bool,
    head: Option<String>,
    ahead: Option<u32>,
    behind: Option<u32>,
    staged: usize,
    unstaged: usize,
    untracked: usize,
    files: Vec<GitFileEntry>,
    branches: Vec<String>,
    remotes: Vec<String>,
}

pub(super) async fn git_status_detail(
    State(state): State<RuntimeApiState>,
) -> Result<Json<GitStatusDetailResponse>, ApiError> {
    let workspace = state.workspace.clone();
    tokio::task::spawn_blocking(move || collect_git_status_detail(&workspace))
        .await
        .map_err(|_| ApiError::internal("git status failed"))?
        .map(Json)
}

fn collect_git_status_detail(workspace: &FsPath) -> Result<GitStatusDetailResponse, ApiError> {
    let status = collect_workspace_status(workspace);
    let mut detail = GitStatusDetailResponse {
        git_repo: status.git_repo,
        workspace: workspace.to_path_buf(),
        branch: status.branch.clone(),
        detached: false,
        head: status.head,
        ahead: status.ahead,
        behind: status.behind,
        staged: status.staged,
        unstaged: status.unstaged,
        untracked: status.untracked,
        files: Vec::new(),
        branches: Vec::new(),
        remotes: Vec::new(),
    };
    if !status.git_repo {
        return Ok(detail);
    }
    detail.detached = status.branch.is_none()
        || status
            .branch
            .as_deref()
            .is_some_and(|branch| branch.starts_with("detached@"));

    // `-z` keeps paths verbatim: one NUL-terminated `XY <path>` record each,
    // with renames/copies carrying the source path in the following record.
    if let Ok(porcelain) = run_git_sync(workspace, &["status", "--porcelain=v1", "-z"]) {
        detail.files = parse_porcelain(&porcelain);
    }
    if let Ok(branches) = run_git_sync(workspace, &["branch", "--format=%(refname:short)"]) {
        detail.branches = branches
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect();
    }
    if let Ok(remotes) = run_git_sync(workspace, &["remote"]) {
        detail.remotes = remotes
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect();
    }
    Ok(detail)
}

fn parse_porcelain(porcelain: &str) -> Vec<GitFileEntry> {
    let mut entries = Vec::new();
    let mut records = porcelain.split('\0').peekable();
    while let Some(record) = records.next() {
        if record.is_empty() || record.starts_with("## ") {
            continue;
        }
        if record.len() < 4 {
            continue;
        }
        let index = record.as_bytes()[0] as char;
        let worktree = record.as_bytes()[1] as char;
        let path = record[3..].to_string();
        let mut old_path = None;
        if matches!(index, 'R' | 'C') {
            old_path = records.next().map(str::to_string);
        }
        entries.push(GitFileEntry {
            path,
            index: index.to_string(),
            worktree: worktree.to_string(),
            staged: !matches!(index, ' ' | '?' | '!'),
            status: porcelain_status(index, worktree),
            old_path,
        });
    }
    entries
}

fn porcelain_status(index: char, worktree: char) -> &'static str {
    match (index, worktree) {
        ('?', '?') => "untracked",
        ('!', '!') => "ignored",
        ('U', _) | (_, 'U') | ('D', 'D') | ('A', 'A') => "conflicted",
        ('R', _) | (_, 'R') => "renamed",
        ('C', _) | (_, 'C') => "copied",
        ('A', _) | (_, 'A') => "added",
        ('D', _) | (_, 'D') => "deleted",
        ('T', _) | (_, 'T') => "typechange",
        ('M', _) | (_, 'M') => "modified",
        _ => "unchanged",
    }
}

// ---------------------------------------------------------------------------
// GET /v1/changes — the file-change inventory only
// ---------------------------------------------------------------------------

/// The Review sheet's change list: the same porcelain projection as
/// `GET /v1/git` minus repo chrome (branches/remotes). One authority — a
/// client that loaded both cannot see them disagree.
pub(super) async fn git_changes(
    State(state): State<RuntimeApiState>,
) -> Result<Json<Value>, ApiError> {
    let workspace = state.workspace.clone();
    let detail = tokio::task::spawn_blocking(move || collect_git_status_detail(&workspace))
        .await
        .map_err(|_| ApiError::internal("git status failed"))??;
    Ok(Json(json!({
        "git_repo": detail.git_repo,
        "branch": detail.branch,
        "staged": detail.staged,
        "unstaged": detail.unstaged,
        "untracked": detail.untracked,
        "files": detail.files,
    })))
}

// ---------------------------------------------------------------------------
// GET /v1/diff + /v1/workspace/diff — unified diffs against HEAD
// ---------------------------------------------------------------------------

/// `git diff <base>` compares the worktree to <base>, covering staged and
/// unstaged changes in one output. On an unborn branch the base is the
/// empty tree, which reads every staged/tracked file as new — the honest
/// "everything changed" picture for a repo with no commits.
async fn diff_base(workspace: &FsPath) -> Result<String, ApiError> {
    let head = git_read(workspace, &["rev-parse", "--verify", "HEAD"]).await?;
    Ok(if head.status_success {
        "HEAD".to_string()
    } else {
        EMPTY_TREE.to_string()
    })
}

/// Byte-bounded cut at a char boundary; reports whether bytes were dropped.
fn bounded_patch(text: &str, limit: usize) -> (String, bool) {
    if text.len() <= limit {
        return (text.to_string(), false);
    }
    let mut end = limit;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    (text[..end].to_string(), true)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GitDiffQuery {
    /// Workspace-relative file; may name a deleted file (the diff survives).
    path: String,
}

/// `GET /v1/diff?path=` — one file's unified diff against HEAD (or the empty
/// tree on an unborn branch). An untracked file has no diff by definition:
/// the response says `untracked: true` with an empty `diff` so the client
/// reads the file itself instead of mistaking it for unchanged.
pub(super) async fn git_diff(
    State(state): State<RuntimeApiState>,
    Query(query): Query<GitDiffQuery>,
) -> Result<Json<Value>, ApiError> {
    let path = relative_request_path(&query.path, false)?;
    let workspace = canonical_workspace(&state.workspace)?;
    require_repo(&workspace)?;
    let base = diff_base(&workspace).await?;
    let path_arg = path.to_string_lossy().into_owned();
    let run = git_read(
        &workspace,
        &[
            "diff",
            "--no-color",
            "--no-ext-diff",
            &base,
            "--",
            &path_arg,
        ],
    )
    .await?;
    if !run.status_success {
        return Err(ApiError::internal(format!(
            "git diff failed: {}",
            run.stderr.trim()
        )));
    }
    let (diff, truncated) = bounded_patch(&run.stdout, FILE_DIFF_MAX_BYTES);
    let untracked = if diff.is_empty() {
        let status = git_read(
            &workspace,
            &["status", "--porcelain=v1", "-z", "--", &path_arg],
        )
        .await?;
        status
            .stdout
            .split('\0')
            .any(|record| record.starts_with("??"))
    } else {
        false
    };
    Ok(Json(json!({
        "ok": true,
        "path": path_arg,
        "base": base,
        "untracked": untracked,
        "diff": diff,
        "truncated": truncated,
    })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WorkspaceDiffQuery {
    /// Byte cap on the returned patch (default 256 KiB, max 4 MiB).
    limit: Option<usize>,
}

/// `GET /v1/workspace/diff?limit=` — the whole tree's diff against HEAD plus
/// a complete `--numstat` inventory, so a client renders every changed file
/// row even when the patch body is truncated.
pub(super) async fn workspace_diff(
    State(state): State<RuntimeApiState>,
    Query(query): Query<WorkspaceDiffQuery>,
) -> Result<Json<Value>, ApiError> {
    let limit = query.limit.unwrap_or(WORKSPACE_DIFF_DEFAULT_BYTES);
    if !(1024..=WORKSPACE_DIFF_MAX_BYTES).contains(&limit) {
        return Err(ApiError::bad_request(format!(
            "limit must be between 1024 and {WORKSPACE_DIFF_MAX_BYTES} bytes"
        )));
    }
    let workspace = canonical_workspace(&state.workspace)?;
    require_repo(&workspace)?;
    let base = diff_base(&workspace).await?;

    let numstat = git_read(&workspace, &["diff", "--numstat", &base]).await?;
    if !numstat.status_success {
        return Err(ApiError::internal(format!(
            "git diff --numstat failed: {}",
            numstat.stderr.trim()
        )));
    }
    let files: Vec<Value> = numstat
        .stdout
        .lines()
        .filter_map(|line| {
            let mut fields = line.splitn(3, '\t');
            let added = fields.next()?;
            let deleted = fields.next()?;
            let path = fields.next()?;
            Some(json!({
                "path": path,
                // Binary files report "-" rather than a count.
                "added": added.parse::<u64>().ok(),
                "deleted": deleted.parse::<u64>().ok(),
            }))
        })
        .collect();

    let run = git_read(&workspace, &["diff", "--no-color", "--no-ext-diff", &base]).await?;
    if !run.status_success {
        return Err(ApiError::internal(format!(
            "git diff failed: {}",
            run.stderr.trim()
        )));
    }
    let (diff, truncated) = bounded_patch(&run.stdout, limit);
    Ok(Json(json!({
        "ok": true,
        "git_repo": true,
        "base": base,
        "files": files,
        "diff": diff,
        "truncated": truncated,
    })))
}

// ---------------------------------------------------------------------------
// GET /v1/git/graph — bounded commit graph rows
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GitGraphQuery {
    limit: Option<usize>,
}

const GRAPH_FIELD: char = '\u{1f}';
const GRAPH_RECORD: char = '\u{1e}';

pub(super) async fn git_graph(
    State(state): State<RuntimeApiState>,
    Query(query): Query<GitGraphQuery>,
) -> Result<Json<Value>, ApiError> {
    let limit = query.limit.unwrap_or(GRAPH_LIMIT_DEFAULT);
    if !(1..=GRAPH_LIMIT_MAX).contains(&limit) {
        return Err(ApiError::bad_request(format!(
            "limit must be between 1 and {GRAPH_LIMIT_MAX}"
        )));
    }
    let workspace = canonical_workspace(&state.workspace)?;
    require_repo(&workspace)?;
    let limit_arg = format!("-n{limit}");
    let format = format!(
        "%H{GRAPH_FIELD}%h{GRAPH_FIELD}%P{GRAPH_FIELD}%an{GRAPH_FIELD}%ae{GRAPH_FIELD}%aI{GRAPH_FIELD}%D{GRAPH_FIELD}%s{GRAPH_RECORD}"
    );
    let format_arg = format!("--format={format}");
    let run = git_read(&workspace, &["log", &limit_arg, &format_arg]).await?;
    if !run.status_success {
        // `git log` exits non-zero on an unborn branch; that is a valid empty
        // graph, not a failure. Distinguish with a HEAD probe instead of
        // trusting stderr text.
        let head = git_read(&workspace, &["rev-parse", "--verify", "HEAD"]).await?;
        if head.status_success {
            return Err(ApiError::internal(format!(
                "git log failed: {}",
                run.stderr.trim()
            )));
        }
        return Ok(Json(json!({ "commits": [], "truncated": false })));
    }
    let mut commits = Vec::new();
    for record in run.stdout.split(GRAPH_RECORD) {
        let record = record.trim_matches('\n');
        if record.is_empty() {
            continue;
        }
        let mut fields = record.split(GRAPH_FIELD);
        let (
            Some(id),
            Some(short),
            Some(parents),
            Some(name),
            Some(email),
            Some(timestamp),
            Some(refs),
            Some(subject),
        ) = (
            fields.next(),
            fields.next(),
            fields.next(),
            fields.next(),
            fields.next(),
            fields.next(),
            fields.next(),
            fields.next(),
        )
        else {
            continue;
        };
        commits.push(json!({
            "id": id,
            "short": short,
            "parents": parents.split_whitespace().collect::<Vec<_>>(),
            "author": { "name": name, "email": email },
            "timestamp": timestamp,
            "refs": refs
                .split(", ")
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .collect::<Vec<_>>(),
            "subject": subject,
        }));
    }
    let truncated = commits.len() >= limit;
    Ok(Json(json!({ "commits": commits, "truncated": truncated })))
}

// ---------------------------------------------------------------------------
// Mutations
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GitPathsRequest {
    #[serde(default)]
    paths: Vec<String>,
    /// Stage/unstage may take the whole tree; discard cannot.
    #[serde(default)]
    all: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GitCommitRequest {
    message: String,
    /// Also stage tracked modifications (`git commit -a`).
    #[serde(default)]
    all: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GitPushRequest {
    remote: Option<String>,
    #[serde(default)]
    set_upstream: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GitBranchRequest {
    name: String,
    /// Create and switch (`git switch -c`); default is switch to existing.
    #[serde(default)]
    create: bool,
}

fn mutation_response(workspace: &FsPath, run: GitRun) -> Result<Json<Value>, ApiError> {
    if !run.status_success {
        let tail = output_tail(&run);
        return Err(ApiError::bad_request(if tail.is_empty() {
            format!("git exited {}", run.exit_code.unwrap_or(-1))
        } else {
            tail
        }));
    }
    Ok(Json(json!({
        "ok": true,
        "output": output_tail(&run),
        "status": collect_workspace_status(workspace),
    })))
}

/// Validate and collect pathspecs. Every path is workspace-relative with no
/// `.`/`..`/`.git` components and is passed after `--`, so it can never be
/// read as an option.
fn validated_paths(request: &GitPathsRequest, allow_all: bool) -> Result<Vec<String>, ApiError> {
    if request.all && !allow_all {
        return Err(ApiError::bad_request(
            "discard requires explicit paths; refusing to discard the whole tree",
        ));
    }
    if request.all && !request.paths.is_empty() {
        return Err(ApiError::bad_request(
            "all and paths are mutually exclusive",
        ));
    }
    if !request.all && request.paths.is_empty() {
        return Err(ApiError::bad_request("paths is required (or all: true)"));
    }
    if request.paths.len() > MAX_PATH_ARGS {
        return Err(ApiError::bad_request(format!(
            "at most {MAX_PATH_ARGS} paths per request"
        )));
    }
    request
        .paths
        .iter()
        .map(|raw| {
            relative_request_path(raw, false).map(|path| path.to_string_lossy().into_owned())
        })
        .collect()
}

pub(super) async fn git_stage(
    State(state): State<RuntimeApiState>,
    Json(request): Json<GitPathsRequest>,
) -> Result<Json<Value>, ApiError> {
    let workspace = canonical_workspace(&state.workspace)?;
    require_repo(&workspace)?;
    let paths = validated_paths(&request, true)?;
    let mut args = vec!["add".to_string()];
    if request.all {
        args.push("--all".to_string());
    }
    if !paths.is_empty() {
        args.push("--".to_string());
        args.extend(paths);
    }
    mutation_response(&workspace, git_write(&workspace, args).await?)
}

pub(super) async fn git_unstage(
    State(state): State<RuntimeApiState>,
    Json(request): Json<GitPathsRequest>,
) -> Result<Json<Value>, ApiError> {
    let workspace = canonical_workspace(&state.workspace)?;
    require_repo(&workspace)?;
    let paths = validated_paths(&request, true)?;
    let mut args = vec!["restore".to_string(), "--staged".to_string()];
    if request.all {
        args.push(":/".to_string());
    }
    if !paths.is_empty() {
        args.push("--".to_string());
        args.extend(paths);
    }
    mutation_response(&workspace, git_write(&workspace, args).await?)
}

pub(super) async fn git_discard(
    State(state): State<RuntimeApiState>,
    Json(request): Json<GitPathsRequest>,
) -> Result<Json<Value>, ApiError> {
    let workspace = canonical_workspace(&state.workspace)?;
    require_repo(&workspace)?;
    let paths = validated_paths(&request, false)?;
    let mut args = vec!["checkout".to_string(), "--".to_string()];
    args.extend(paths);
    mutation_response(&workspace, git_write(&workspace, args).await?)
}

pub(super) async fn git_commit(
    State(state): State<RuntimeApiState>,
    Json(request): Json<GitCommitRequest>,
) -> Result<Json<Value>, ApiError> {
    let message = request.message.trim();
    if message.is_empty() {
        return Err(ApiError::bad_request("message is required"));
    }
    if message.len() > MAX_COMMIT_MESSAGE_BYTES {
        return Err(ApiError::bad_request(format!(
            "message must be at most {MAX_COMMIT_MESSAGE_BYTES} bytes"
        )));
    }
    let workspace = canonical_workspace(&state.workspace)?;
    require_repo(&workspace)?;
    let mut args = vec!["commit".to_string()];
    if request.all {
        args.push("--all".to_string());
    }
    args.push("--message".to_string());
    args.push(message.to_string());
    mutation_response(&workspace, git_write(&workspace, args).await?)
}

pub(super) async fn git_push(
    State(state): State<RuntimeApiState>,
    Json(request): Json<GitPushRequest>,
) -> Result<Json<Value>, ApiError> {
    let workspace = canonical_workspace(&state.workspace)?;
    require_repo(&workspace)?;
    let remote = request
        .remote
        .as_deref()
        .map(str::trim)
        .filter(|remote| !remote.is_empty())
        .map(str::to_string);
    if let Some(remote) = &remote
        && !remote
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'/'))
    {
        return Err(ApiError::bad_request("remote must be a remote name"));
    }
    let mut args = vec!["push".to_string()];
    if request.set_upstream {
        let branch = run_git_sync(&workspace, &["rev-parse", "--abbrev-ref", "HEAD"])?;
        let branch = branch.trim();
        if branch.is_empty() || branch == "HEAD" {
            return Err(ApiError::bad_request(
                "cannot set upstream from a detached HEAD",
            ));
        }
        args.push("--set-upstream".to_string());
        args.push(remote.unwrap_or_else(|| "origin".to_string()));
        args.push(branch.to_string());
    } else if let Some(remote) = remote {
        args.push(remote);
    }
    mutation_response(&workspace, git_write(&workspace, args).await?)
}

pub(super) async fn git_branch(
    State(state): State<RuntimeApiState>,
    Json(request): Json<GitBranchRequest>,
) -> Result<Json<Value>, ApiError> {
    let name = request.name.trim();
    if name.is_empty() || name.len() > MAX_BRANCH_NAME_BYTES {
        return Err(ApiError::bad_request("name is required"));
    }
    let workspace = canonical_workspace(&state.workspace)?;
    require_repo(&workspace)?;
    // `check-ref-format --branch` is the ref authority — it rejects option
    // lookalikes, `..`, `@{`, control bytes, and every other unsafe name.
    let check = git_write(
        &workspace,
        vec![
            "check-ref-format".to_string(),
            "--branch".to_string(),
            name.to_string(),
        ],
    )
    .await?;
    if !check.status_success {
        return Err(ApiError::bad_request("name is not a valid branch"));
    }
    let mut args = vec!["switch".to_string()];
    if request.create {
        args.push("--create".to_string());
    }
    args.push(name.to_string());
    mutation_response(&workspace, git_write(&workspace, args).await?)
}
