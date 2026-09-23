//! Git history tools: `git_log`, `git_show`, and `git_blame`.
//!
//! These tools provide read-only access to commit history and attribution
//! without exposing arbitrary shell execution.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

use async_trait::async_trait;
use serde_json::{Value, json};

use super::spec::{
    ApprovalRequirement, ToolCapability, ToolContext, ToolError, ToolResult, ToolSpec,
    optional_bool, optional_str, optional_u64, required_str,
};
use crate::dependencies::ExternalTool;

const MAX_OUTPUT_CHARS: usize = 40_000;
const DEFAULT_LOG_MAX_COUNT: u64 = 20;
const MAX_LOG_MAX_COUNT: u64 = 200;
const DEFAULT_UNIFIED: u64 = 3;
const MAX_UNIFIED: u64 = 50;
const DEFAULT_BLAME_START_LINE: u64 = 1;
const DEFAULT_BLAME_MAX_LINES: u64 = 200;
const MAX_BLAME_MAX_LINES: u64 = 2_000;

/// Tool for reading recent commit history.
pub struct GitLogTool;

#[async_trait]
impl ToolSpec for GitLogTool {
    fn name(&self) -> &'static str {
        "git_log"
    }

    fn model_visible(&self) -> bool {
        false
    }

    fn description(&self) -> &'static str {
        "Run `git log` in the workspace with optional path and author/date filters."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Optional subdirectory or file path to scope history to."
                },
                "max_count": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": MAX_LOG_MAX_COUNT,
                    "default": DEFAULT_LOG_MAX_COUNT,
                    "description": "Maximum number of commits to return."
                },
                "author": {
                    "type": "string",
                    "description": "Optional git author filter (same semantics as `git log --author`)."
                },
                "since": {
                    "type": "string",
                    "description": "Optional lower date bound, e.g. '2 weeks ago' or ISO date."
                },
                "until": {
                    "type": "string",
                    "description": "Optional upper date bound, e.g. 'yesterday' or ISO date."
                }
            },
            "additionalProperties": false
        })
    }

    fn capabilities(&self) -> Vec<ToolCapability> {
        vec![ToolCapability::ReadOnly, ToolCapability::Sandboxable]
    }

    fn approval_requirement(&self) -> ApprovalRequirement {
        ApprovalRequirement::Auto
    }

    fn supports_parallel(&self) -> bool {
        true
    }

    async fn execute(&self, input: Value, context: &ToolContext) -> Result<ToolResult, ToolError> {
        let git_ctx = resolve_git_context(context, optional_str(&input, "path")?)?;
        let max_count =
            optional_u64(&input, "max_count", DEFAULT_LOG_MAX_COUNT)?.clamp(1, MAX_LOG_MAX_COUNT);
        let author = optional_str(&input, "author")?.map(ToOwned::to_owned);
        let since = optional_str(&input, "since")?.map(ToOwned::to_owned);
        let until = optional_str(&input, "until")?.map(ToOwned::to_owned);

        let mut args = vec![
            "log".to_string(),
            "--no-color".to_string(),
            format!("--max-count={max_count}"),
            "--date=iso-strict".to_string(),
            "--pretty=format:%H%nAuthor: %an <%ae>%nDate: %ad%nSubject: %s%n".to_string(),
        ];
        if let Some(author) = &author {
            args.push(format!("--author={author}"));
        }
        if let Some(since) = &since {
            args.push(format!("--since={since}"));
        }
        if let Some(until) = &until {
            args.push(format!("--until={until}"));
        }
        if let Some(pathspec) = &git_ctx.pathspec {
            args.push("--".to_string());
            args.push(pathspec.display().to_string());
        }

        let command_str = format_command(&git_ctx.working_dir, &args);
        let output = run_git_command_async(git_ctx.working_dir.clone(), args).await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Ok(
                ToolResult::error(format!("git log failed: {}", stderr.trim())).with_metadata(
                    json!({
                        "command": command_str,
                        "exit_code": output.status.code(),
                        "stderr": stderr.trim(),
                    }),
                ),
            );
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let (content, truncated, omitted_chars) = truncate_with_note(&stdout, MAX_OUTPUT_CHARS);
        Ok(ToolResult::success(content).with_metadata(json!({
            "command": command_str,
            "working_dir": git_ctx.working_dir,
            "pathspec": git_ctx.pathspec,
            "max_count": max_count,
            "author": author,
            "since": since,
            "until": until,
            "truncated": truncated,
            "omitted_chars": omitted_chars,
        })))
    }
}

/// Tool for showing a specific commit with optional patch/stat output.
pub struct GitShowTool;

#[async_trait]
impl ToolSpec for GitShowTool {
    fn name(&self) -> &'static str {
        "git_show"
    }

    fn model_visible(&self) -> bool {
        false
    }

    fn description(&self) -> &'static str {
        "Run `git show` for a specific revision with optional patch and stats."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "rev": {
                    "type": "string",
                    "description": "Revision to show (commit SHA, tag, branch, or ref expression)."
                },
                "path": {
                    "type": "string",
                    "description": "Optional subdirectory or file path to scope output."
                },
                "patch": {
                    "type": "boolean",
                    "default": true,
                    "description": "Include patch hunks (default true)."
                },
                "stat": {
                    "type": "boolean",
                    "default": true,
                    "description": "Include --stat summary (default true)."
                },
                "unified": {
                    "type": "integer",
                    "minimum": 0,
                    "maximum": MAX_UNIFIED,
                    "default": DEFAULT_UNIFIED,
                    "description": "Context lines for patch output when patch=true."
                }
            },
            "required": ["rev"],
            "additionalProperties": false
        })
    }

    fn capabilities(&self) -> Vec<ToolCapability> {
        vec![ToolCapability::ReadOnly, ToolCapability::Sandboxable]
    }

    fn approval_requirement(&self) -> ApprovalRequirement {
        ApprovalRequirement::Auto
    }

    fn supports_parallel(&self) -> bool {
        true
    }

    async fn execute(&self, input: Value, context: &ToolContext) -> Result<ToolResult, ToolError> {
        let rev = required_str(&input, "rev")?;
        validate_git_rev(rev)?;
        let git_ctx = resolve_git_context(context, optional_str(&input, "path")?)?;
        let patch = optional_bool(&input, "patch", true)?;
        let stat = optional_bool(&input, "stat", true)?;
        let unified = optional_u64(&input, "unified", DEFAULT_UNIFIED)?.min(MAX_UNIFIED);

        let mut args = vec![
            "show".to_string(),
            "--no-color".to_string(),
            "--no-ext-diff".to_string(),
        ];
        if patch {
            args.push(format!("--unified={unified}"));
        } else {
            args.push("--no-patch".to_string());
        }
        if stat {
            args.push("--stat".to_string());
        }
        args.push(rev.to_string());
        if let Some(pathspec) = &git_ctx.pathspec {
            args.push("--".to_string());
            args.push(pathspec.display().to_string());
        }

        let command_str = format_command(&git_ctx.working_dir, &args);
        let output = run_git_command_async(git_ctx.working_dir.clone(), args).await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Ok(ToolResult::error(format!(
                "git show failed for '{rev}': {}",
                stderr.trim()
            ))
            .with_metadata(json!({
                "command": command_str,
                "exit_code": output.status.code(),
                "stderr": stderr.trim(),
            })));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let (content, truncated, omitted_chars) = truncate_with_note(&stdout, MAX_OUTPUT_CHARS);
        Ok(ToolResult::success(content).with_metadata(json!({
            "command": command_str,
            "working_dir": git_ctx.working_dir,
            "pathspec": git_ctx.pathspec,
            "rev": rev,
            "patch": patch,
            "stat": stat,
            "unified": if patch { Some(unified) } else { None },
            "truncated": truncated,
            "omitted_chars": omitted_chars,
        })))
    }
}

/// Tool for attributing lines in a file to commits and authors.
pub struct GitBlameTool;

#[async_trait]
impl ToolSpec for GitBlameTool {
    fn name(&self) -> &'static str {
        "git_blame"
    }

    fn model_visible(&self) -> bool {
        false
    }

    fn description(&self) -> &'static str {
        "Run `git blame` on a file with optional revision and line-range controls."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to a tracked file within the workspace."
                },
                "rev": {
                    "type": "string",
                    "description": "Optional revision to blame against (default: HEAD)."
                },
                "start_line": {
                    "type": "integer",
                    "minimum": 1,
                    "default": DEFAULT_BLAME_START_LINE,
                    "description": "First line to include in blame output."
                },
                "max_lines": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": MAX_BLAME_MAX_LINES,
                    "default": DEFAULT_BLAME_MAX_LINES,
                    "description": "Maximum number of lines to include."
                },
                "porcelain": {
                    "type": "boolean",
                    "default": false,
                    "description": "When true, emit `--line-porcelain` output."
                }
            },
            "required": ["path"],
            "additionalProperties": false
        })
    }

    fn capabilities(&self) -> Vec<ToolCapability> {
        vec![ToolCapability::ReadOnly, ToolCapability::Sandboxable]
    }

    fn approval_requirement(&self) -> ApprovalRequirement {
        ApprovalRequirement::Auto
    }

    fn supports_parallel(&self) -> bool {
        true
    }

    async fn execute(&self, input: Value, context: &ToolContext) -> Result<ToolResult, ToolError> {
        let path_str = required_str(&input, "path")?;
        let resolved_path = context.resolve_path(path_str)?;
        let metadata = tokio::fs::metadata(&resolved_path).await.map_err(|e| {
            ToolError::invalid_input(format!(
                "Path does not exist or is not accessible: {path_str} ({e})"
            ))
        })?;
        if !metadata.is_file() {
            return Err(ToolError::invalid_input(format!(
                "Path must point to a file: {path_str}"
            )));
        }

        let working_dir = resolved_path.parent().ok_or_else(|| {
            ToolError::invalid_input(format!("Path has no parent directory: {path_str}"))
        })?;
        let pathspec = pathspec_from(working_dir, &resolved_path);
        let rev = optional_str(&input, "rev")?.unwrap_or("HEAD");
        validate_git_rev(rev)?;
        let start_line = optional_u64(&input, "start_line", DEFAULT_BLAME_START_LINE)?.max(1);
        let max_lines = optional_u64(&input, "max_lines", DEFAULT_BLAME_MAX_LINES)?
            .clamp(1, MAX_BLAME_MAX_LINES);
        let end_line = start_line.saturating_add(max_lines.saturating_sub(1));
        let porcelain = optional_bool(&input, "porcelain", false)?;

        let mut args = vec![
            "blame".to_string(),
            "--date=iso".to_string(),
            format!("-L{start_line},{end_line}"),
        ];
        if porcelain {
            args.push("--line-porcelain".to_string());
        }
        args.push(rev.to_string());
        args.push("--".to_string());
        args.push(pathspec.display().to_string());

        let command_str = format_command(working_dir, &args);
        let output = run_git_command_async(working_dir.to_path_buf(), args).await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Ok(ToolResult::error(format!(
                "git blame failed for '{path_str}' at '{rev}': {}",
                stderr.trim()
            ))
            .with_metadata(json!({
                "command": command_str,
                "exit_code": output.status.code(),
                "stderr": stderr.trim(),
            })));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let (content, truncated, omitted_chars) = truncate_with_note(&stdout, MAX_OUTPUT_CHARS);
        Ok(ToolResult::success(content).with_metadata(json!({
            "command": command_str,
            "working_dir": working_dir,
            "pathspec": pathspec,
            "rev": rev,
            "start_line": start_line,
            "max_lines": max_lines,
            "porcelain": porcelain,
            "truncated": truncated,
            "omitted_chars": omitted_chars,
        })))
    }
}

/// Tool for fetching remote refs: `git fetch <remote> [<refspec>...]`.
///
/// The bounded verify-mode git surface (#6298): a verifier child cannot reach
/// raw shell, so `git fetch` arrives as a structured call instead of a shell
/// command. The bound is structural — fixed argv, argv-direct spawning (no
/// shell), a remote that must be a *configured* remote name (never a URL, so
/// an operator-supplied address cannot exfiltrate or redirect), and refspecs
/// that pass the option/whitespace/control gates. Only remote-tracking refs
/// (plus `FETCH_HEAD` and the fetched objects) move: never a checkout, merge,
/// or push. The execution envelope classes this as bounded fetch — shell plus
/// network authority, not write authority.
///
/// Known limitation: shares the existing git tools' no-timeout behavior; a
/// hung remote is bounded by the caller's wall clock, not the tool.
pub struct GitFetchTool;

#[async_trait]
impl ToolSpec for GitFetchTool {
    fn name(&self) -> &'static str {
        "git_fetch"
    }

    fn model_visible(&self) -> bool {
        false
    }

    fn description(&self) -> &'static str {
        "Run `git fetch` against a configured remote. Updates remote-tracking refs only; never checks out, merges, or pushes."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "remote": {
                    "type": "string",
                    "default": "origin",
                    "description": "Configured remote name to fetch from (default origin). Must be a name from `git remote`, never a URL."
                },
                "refspecs": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional refspecs to fetch (e.g. pull/123/head). Empty fetches the remote's defaults."
                },
                "path": {
                    "type": "string",
                    "description": "Optional subdirectory to run from."
                }
            },
            "additionalProperties": false
        })
    }

    fn capabilities(&self) -> Vec<ToolCapability> {
        vec![ToolCapability::Network, ToolCapability::Sandboxable]
    }

    fn approval_requirement(&self) -> ApprovalRequirement {
        ApprovalRequirement::Required
    }

    fn supports_parallel(&self) -> bool {
        false
    }

    async fn execute(&self, input: Value, context: &ToolContext) -> Result<ToolResult, ToolError> {
        let remote = optional_str(&input, "remote")?.unwrap_or("origin");
        validate_git_remote_name(remote)?;
        let refspecs = parse_git_refspecs(&input)?;
        let git_ctx = resolve_git_context(context, optional_str(&input, "path")?)?;
        require_configured_remote(&git_ctx.working_dir, remote).await?;

        let mut args = vec!["fetch".to_string(), remote.to_string()];
        args.extend(refspecs.clone());

        let command_str = format_command(&git_ctx.working_dir, &args);
        let output = run_git_command_async(git_ctx.working_dir.clone(), args).await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Ok(ToolResult::error(format!(
                "git fetch failed for remote '{remote}': {}",
                stderr.trim()
            ))
            .with_metadata(json!({
                "command": command_str,
                "exit_code": output.status.code(),
                "stderr": stderr.trim(),
            })));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = if stderr.trim().is_empty() {
            stdout.to_string()
        } else {
            format!("{stdout}\n{stderr}")
        };
        let (content, truncated, omitted_chars) = truncate_with_note(&combined, MAX_OUTPUT_CHARS);
        Ok(ToolResult::success(content).with_metadata(json!({
            "command": command_str,
            "working_dir": git_ctx.working_dir,
            "remote": remote,
            "refspecs": refspecs,
            "truncated": truncated,
            "omitted_chars": omitted_chars,
        })))
    }
}

/// Tool for computing a merge result without touching the working tree.
///
/// `git merge-tree` is a pure read: it performs the merge in memory and
/// prints the resulting tree plus conflicted-file info, writing nothing, so
/// the envelope classes it Bounded like the other inspection actions. Uses
/// the modern two-revision form (git 2.38+, 2022); an explicit base arrives
/// via `--merge-base`, and otherwise git finds the bases itself — including
/// the multi-base virtual-base case a hand-rolled `merge-base` call cannot
/// express. Older gits fail with their own usage error, surfaced below.
pub struct GitMergeTreeTool;

#[async_trait]
impl ToolSpec for GitMergeTreeTool {
    fn name(&self) -> &'static str {
        "git_merge_tree"
    }

    fn model_visible(&self) -> bool {
        false
    }

    fn description(&self) -> &'static str {
        "Compute the merge result of two revisions without touching the working tree (`git merge-tree`). Pure read."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "ours": {
                    "type": "string",
                    "description": "First revision (e.g. main)."
                },
                "theirs": {
                    "type": "string",
                    "description": "Second revision (e.g. the PR head)."
                },
                "base": {
                    "type": "string",
                    "description": "Optional merge base (--merge-base). Omit it and git finds the bases itself."
                },
                "path": {
                    "type": "string",
                    "description": "Optional subdirectory to run from."
                }
            },
            "required": ["ours", "theirs"],
            "additionalProperties": false
        })
    }

    fn capabilities(&self) -> Vec<ToolCapability> {
        vec![ToolCapability::ReadOnly, ToolCapability::Sandboxable]
    }

    fn approval_requirement(&self) -> ApprovalRequirement {
        ApprovalRequirement::Auto
    }

    fn supports_parallel(&self) -> bool {
        true
    }

    async fn execute(&self, input: Value, context: &ToolContext) -> Result<ToolResult, ToolError> {
        let ours = required_str(&input, "ours")?;
        let theirs = required_str(&input, "theirs")?;
        validate_git_rev(ours)?;
        validate_git_rev(theirs)?;
        let git_ctx = resolve_git_context(context, optional_str(&input, "path")?)?;

        let mut args = vec!["merge-tree".to_string()];
        let base = match optional_str(&input, "base")? {
            Some(base) => {
                validate_git_rev(base)?;
                // `--opt=value` form: a validated rev can never split into a
                // second argv element, so no option injection through `base`.
                args.push(format!("--merge-base={base}"));
                Some(base.to_string())
            }
            None => None,
        };
        args.push(ours.to_string());
        args.push(theirs.to_string());
        let command_str = format_command(&git_ctx.working_dir, &args);
        let output = run_git_command_async(git_ctx.working_dir.clone(), args).await?;
        // merge-tree exits 1 both for conflicts (a successful report on
        // stdout) and for real failures (empty stdout, reason on stderr).
        // The report is the answer; only the empty case is an error.
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !output.status.success() && stdout.trim().is_empty() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Ok(ToolResult::error(format!(
                "git merge-tree failed for '{ours}' + '{theirs}': {}",
                stderr.trim()
            ))
            .with_metadata(json!({
                "command": command_str,
                "exit_code": output.status.code(),
                "stderr": stderr.trim(),
            })));
        }

        let conflicts = !output.status.success();
        let (content, truncated, omitted_chars) = truncate_with_note(&stdout, MAX_OUTPUT_CHARS);
        Ok(ToolResult::success(content).with_metadata(json!({
            "command": command_str,
            "working_dir": git_ctx.working_dir,
            "ours": ours,
            "theirs": theirs,
            "base": base,
            "conflicts": conflicts,
            "truncated": truncated,
            "omitted_chars": omitted_chars,
        })))
    }
}

struct GitContext {
    working_dir: PathBuf,
    pathspec: Option<PathBuf>,
}

fn resolve_git_context(context: &ToolContext, path: Option<&str>) -> Result<GitContext, ToolError> {
    let workspace = canonical_or_workspace(&context.workspace);
    let mut working_dir = workspace.clone();
    let mut pathspec = None;

    if let Some(raw) = path {
        let resolved = context.resolve_path(raw)?;
        let metadata = fs::metadata(&resolved).map_err(|e| {
            ToolError::invalid_input(format!(
                "Path does not exist or is not accessible: {raw} ({e})"
            ))
        })?;

        if metadata.is_dir() {
            working_dir = resolved;
            pathspec = Some(PathBuf::from("."));
        } else {
            let parent = resolved.parent().ok_or_else(|| {
                ToolError::invalid_input(format!("Path has no parent directory: {raw}"))
            })?;
            working_dir = parent.to_path_buf();
            pathspec = Some(pathspec_from(&working_dir, &resolved));
        }
    }

    if !working_dir.exists() {
        return Err(ToolError::invalid_input(format!(
            "Working directory does not exist: {}",
            working_dir.display()
        )));
    }

    Ok(GitContext {
        working_dir,
        pathspec,
    })
}

fn validate_git_rev(rev: &str) -> Result<(), ToolError> {
    let trimmed = rev.trim();
    if trimmed.is_empty() {
        return Err(ToolError::invalid_input(
            "git revision must not be empty".to_string(),
        ));
    }
    if trimmed.starts_with('-') {
        return Err(ToolError::invalid_input(
            "git revision must not start with '-'".to_string(),
        ));
    }
    if trimmed.chars().any(char::is_whitespace) {
        return Err(ToolError::invalid_input(
            "git revision must not contain whitespace".to_string(),
        ));
    }
    if trimmed
        .chars()
        .any(|ch| ch == '\0' || ch.is_ascii_control())
    {
        return Err(ToolError::invalid_input(
            "git revision must not contain control characters".to_string(),
        ));
    }
    Ok(())
}

/// A fetch remote is a configured remote *name*, never a URL: URLs and paths
/// fail the membership check below, but rejecting their shapes here keeps the
/// refusal precise (`:` kills `https://`, `user@host:path`, and `file://`;
/// `/` kills paths) instead of "unknown remote".
fn validate_git_remote_name(remote: &str) -> Result<(), ToolError> {
    let trimmed = remote.trim();
    if trimmed.is_empty() {
        return Err(ToolError::invalid_input(
            "git remote must not be empty".to_string(),
        ));
    }
    if trimmed.starts_with('-') {
        return Err(ToolError::invalid_input(
            "git remote must not start with '-'".to_string(),
        ));
    }
    if trimmed.chars().any(char::is_whitespace) {
        return Err(ToolError::invalid_input(
            "git remote must not contain whitespace".to_string(),
        ));
    }
    if trimmed
        .chars()
        .any(|ch| ch == '\0' || ch.is_ascii_control() || ch == ':' || ch == '/')
    {
        return Err(ToolError::invalid_input(
            "git remote must be a configured remote name (from `git remote`), never a URL or path"
                .to_string(),
        ));
    }
    Ok(())
}

/// Parse the optional `refspecs` array: `[+]<src>[:<dst>]`, each side passing
/// the revision gates. A wrong type is an error, never a silent default.
fn parse_git_refspecs(input: &Value) -> Result<Vec<String>, ToolError> {
    let items = match input.get("refspecs") {
        None | Some(Value::Null) => return Ok(Vec::new()),
        Some(Value::Array(items)) => items,
        Some(other) => {
            return Err(super::spec::type_mismatch(
                "refspecs",
                other,
                "an array of strings",
            ));
        }
    };
    let mut refspecs = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let Some(refspec) = item.as_str() else {
            return Err(super::spec::type_mismatch(
                &format!("refspecs[{index}]"),
                item,
                "a string",
            ));
        };
        validate_git_refspec(refspec)?;
        refspecs.push(refspec.to_string());
    }
    Ok(refspecs)
}

fn validate_git_refspec(refspec: &str) -> Result<(), ToolError> {
    let body = refspec.strip_prefix('+').unwrap_or(refspec);
    let parts: Vec<&str> = body.split(':').collect();
    if parts.len() > 2 {
        return Err(ToolError::invalid_input(format!(
            "git refspec '{refspec}' must have at most one ':'"
        )));
    }
    for side in parts {
        validate_git_refspec_side(refspec, side)?;
    }
    Ok(())
}

fn validate_git_refspec_side(refspec: &str, side: &str) -> Result<(), ToolError> {
    if side.is_empty() {
        return Err(ToolError::invalid_input(format!(
            "git refspec '{refspec}' has an empty side"
        )));
    }
    validate_git_rev(side).map_err(|_| {
        ToolError::invalid_input(format!(
            "git refspec '{refspec}' is not a plain refspec (no options, whitespace, or control characters)"
        ))
    })
}

/// The remote must already be configured on this repository. A name git never
/// heard of fails here — before any network — so a typo cannot become a fetch
/// from somewhere else.
async fn require_configured_remote(working_dir: &Path, remote: &str) -> Result<(), ToolError> {
    let output =
        run_git_command_async(working_dir.to_path_buf(), vec!["remote".to_string()]).await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ToolError::execution_failed(format!(
            "git remote failed: {}",
            stderr.trim()
        )));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let configured: Vec<&str> = stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    if configured.contains(&remote) {
        Ok(())
    } else {
        Err(ToolError::invalid_input(format!(
            "unknown git remote '{remote}'; configured remotes: {}",
            if configured.is_empty() {
                "(none)".to_string()
            } else {
                configured.join(", ")
            }
        )))
    }
}

fn canonical_or_workspace(workspace: &Path) -> PathBuf {
    workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_path_buf())
}

fn pathspec_from(working_dir: &Path, resolved: &Path) -> PathBuf {
    match resolved.strip_prefix(working_dir) {
        Ok(rel) if rel.as_os_str().is_empty() => PathBuf::from("."),
        Ok(rel) => rel.to_path_buf(),
        Err(_) => PathBuf::from("."),
    }
}

fn run_git_command(working_dir: &Path, args: &[String]) -> Result<Output, ToolError> {
    let Some(mut cmd) = crate::dependencies::Git::command() else {
        return Err(ToolError::not_available(
            "git is not installed or not in PATH",
        ));
    };
    cmd.args(args).current_dir(working_dir);
    cmd.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ToolError::not_available("git is not installed or not in PATH")
        } else {
            ToolError::execution_failed(format!("Failed to run git: {e}"))
        }
    })
}

/// Async wrapper that offloads the blocking `git` invocation onto a
/// blocking-capable thread so the tokio worker is not stalled.
async fn run_git_command_async(
    working_dir: PathBuf,
    args: Vec<String>,
) -> Result<Output, ToolError> {
    tokio::task::spawn_blocking(move || run_git_command(&working_dir, &args))
        .await
        .map_err(|e| ToolError::execution_failed(format!("git task panicked: {e}")))?
}

fn format_command(working_dir: &Path, args: &[String]) -> String {
    format!(
        "git -C {} {}",
        working_dir.display(),
        args.iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(" ")
    )
}

fn truncate_with_note(text: &str, max_chars: usize) -> (String, bool, usize) {
    if text.chars().count() <= max_chars {
        return (text.to_string(), false, 0);
    }
    let end = char_boundary_index(text, max_chars);
    let truncated = &text[..end];
    let omitted_chars = text
        .chars()
        .count()
        .saturating_sub(truncated.chars().count());
    let note = format!(
        "\n\n[output truncated to {max_chars} characters; {omitted_chars} characters omitted]"
    );
    (format!("{truncated}{note}"), true, omitted_chars)
}

fn char_boundary_index(text: &str, max_chars: usize) -> usize {
    if max_chars == 0 {
        return 0;
    }
    for (count, (idx, _)) in text.char_indices().enumerate() {
        if count == max_chars {
            return idx;
        }
    }
    text.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use tempfile::tempdir;

    fn git_available() -> bool {
        crate::dependencies::Git::available()
    }

    fn run_git(root: &Path, args: &[&str]) {
        let status = crate::dependencies::Git::status(args, root).expect("git should spawn");
        assert!(status.success(), "git {args:?} failed");
    }

    fn init_git_repo(root: &Path) {
        run_git(root, &["init", "-q"]);
        run_git(root, &["config", "core.autocrlf", "false"]);
        run_git(root, &["config", "user.email", "test@example.com"]);
        run_git(root, &["config", "user.name", "Test User"]);
    }

    fn commit_all(root: &Path, message: &str) {
        run_git(root, &["add", "."]);
        run_git(root, &["commit", "-q", "-m", message]);
    }

    #[tokio::test]
    async fn git_log_lists_recent_commits() {
        if !git_available() {
            return;
        }

        let tmp = tempdir().expect("tempdir");
        init_git_repo(tmp.path());
        fs::write(tmp.path().join("file.txt"), "one\n").expect("write");
        commit_all(tmp.path(), "first");
        fs::write(tmp.path().join("file.txt"), "two\n").expect("write");
        commit_all(tmp.path(), "second");

        let ctx = ToolContext::new(tmp.path());
        let result = GitLogTool
            .execute(json!({ "max_count": 1 }), &ctx)
            .await
            .expect("execute");
        assert!(result.success);
        assert!(result.content.contains("Subject: second"));
    }

    #[tokio::test]
    async fn git_show_returns_patch_for_revision() {
        if !git_available() {
            return;
        }

        let tmp = tempdir().expect("tempdir");
        init_git_repo(tmp.path());
        fs::write(tmp.path().join("file.txt"), "one\n").expect("write");
        commit_all(tmp.path(), "first");
        fs::write(tmp.path().join("file.txt"), "one\ntwo\n").expect("write");
        commit_all(tmp.path(), "second");

        let ctx = ToolContext::new(tmp.path());
        let result = GitShowTool
            .execute(json!({ "rev": "HEAD", "stat": false }), &ctx)
            .await
            .expect("execute");
        assert!(result.success);
        assert!(result.content.contains("diff --git"));
        assert!(result.content.contains("+two"));
    }

    #[tokio::test]
    async fn git_show_rejects_option_like_revision() {
        let tmp = tempdir().expect("tempdir");
        let ctx = ToolContext::new(tmp.path());
        let err = GitShowTool
            .execute(json!({ "rev": "--stat" }), &ctx)
            .await
            .expect_err("option-shaped rev should fail before git runs");
        assert!(matches!(err, ToolError::InvalidInput { .. }));
        assert!(err.to_string().contains("must not start with '-'"));
    }

    #[tokio::test]
    async fn git_show_rejects_whitespace_revision_payload() {
        let tmp = tempdir().expect("tempdir");
        let ctx = ToolContext::new(tmp.path());
        let err = GitShowTool
            .execute(
                json!({ "rev": "HEAD --output=/tmp/codewhale-git-show" }),
                &ctx,
            )
            .await
            .expect_err("whitespace rev payload should fail before git runs");
        assert!(matches!(err, ToolError::InvalidInput { .. }));
        assert!(err.to_string().contains("must not contain whitespace"));
    }

    #[tokio::test]
    async fn git_blame_reports_author_for_range() {
        if !git_available() {
            return;
        }

        let tmp = tempdir().expect("tempdir");
        init_git_repo(tmp.path());
        let src = tmp.path().join("src");
        fs::create_dir_all(&src).expect("mkdir");
        let file = src.join("lib.rs");
        fs::write(&file, "pub fn one() -> i32 { 1 }\n").expect("write");
        commit_all(tmp.path(), "first");
        fs::write(&file, "pub fn one() -> i32 { 2 }\n").expect("write");
        commit_all(tmp.path(), "second");

        let ctx = ToolContext::new(tmp.path());
        let result = GitBlameTool
            .execute(
                json!({
                    "path": "src/lib.rs",
                    "start_line": 1,
                    "max_lines": 1
                }),
                &ctx,
            )
            .await
            .expect("execute");
        assert!(result.success);
        assert!(result.content.contains("Test User"));
    }

    #[tokio::test]
    async fn git_blame_rejects_option_like_revision() {
        let tmp = tempdir().expect("tempdir");
        let file = tmp.path().join("file.txt");
        fs::write(&file, "one\n").expect("write");
        let ctx = ToolContext::new(tmp.path());
        let err = GitBlameTool
            .execute(
                json!({ "path": "file.txt", "rev": "--contents=/tmp/x" }),
                &ctx,
            )
            .await
            .expect_err("option-shaped rev should fail before git runs");
        assert!(matches!(err, ToolError::InvalidInput { .. }));
        assert!(err.to_string().contains("must not start with '-'"));
    }

    #[tokio::test]
    async fn git_blame_rejects_whitespace_revision_payload() {
        let tmp = tempdir().expect("tempdir");
        let file = tmp.path().join("file.txt");
        fs::write(&file, "one\n").expect("write");
        let ctx = ToolContext::new(tmp.path());
        let err = GitBlameTool
            .execute(
                json!({ "path": "file.txt", "rev": "HEAD --contents=/tmp/codewhale-git-blame" }),
                &ctx,
            )
            .await
            .expect_err("whitespace rev payload should fail before git runs");
        assert!(matches!(err, ToolError::InvalidInput { .. }));
        assert!(err.to_string().contains("must not contain whitespace"));
    }

    #[tokio::test]
    async fn git_blame_errors_for_non_file_path() {
        if !git_available() {
            return;
        }

        let tmp = tempdir().expect("tempdir");
        init_git_repo(tmp.path());

        let ctx = ToolContext::new(tmp.path());
        let result = GitBlameTool
            .execute(json!({ "path": "." }), &ctx)
            .await
            .expect_err("directory path should fail");
        assert!(matches!(result, ToolError::InvalidInput { .. }));
    }

    #[tokio::test]
    async fn git_fetch_rejects_url_and_option_shaped_remotes() {
        let tmp = tempdir().expect("tempdir");
        let ctx = ToolContext::new(tmp.path());
        for remote in [
            "https://example.com/repo.git",
            "git@example.com:org/repo.git",
            "/tmp/other-checkout",
            "--upload-pack=evil",
            "origin --prune",
        ] {
            let err = GitFetchTool
                .execute(json!({ "remote": remote }), &ctx)
                .await
                .expect_err("non-name remote should fail before git runs");
            assert!(
                matches!(err, ToolError::InvalidInput { .. }),
                "{remote}: {err}"
            );
        }
    }

    #[tokio::test]
    async fn git_fetch_rejects_unknown_remote_before_network() {
        if !git_available() {
            return;
        }

        let tmp = tempdir().expect("tempdir");
        init_git_repo(tmp.path());
        let ctx = ToolContext::new(tmp.path());
        let err = GitFetchTool
            .execute(json!({ "remote": "origin" }), &ctx)
            .await
            .expect_err("unconfigured remote must be refused");
        let message = err.to_string();
        assert!(message.contains("unknown git remote 'origin'"), "{message}");
        assert!(message.contains("(none)"), "{message}");
    }

    #[tokio::test]
    async fn git_fetch_rejects_malformed_refspecs() {
        if !git_available() {
            return;
        }

        let tmp = tempdir().expect("tempdir");
        init_git_repo(tmp.path());
        let ctx = ToolContext::new(tmp.path());
        for refspec in ["a:b:c", "src:", ":dst", "--prune", "a b"] {
            let err = GitFetchTool
                .execute(json!({ "refspecs": [refspec] }), &ctx)
                .await
                .expect_err("malformed refspec should fail before git runs");
            assert!(
                matches!(err, ToolError::InvalidInput { .. }),
                "{refspec}: {err}"
            );
        }
        let err = GitFetchTool
            .execute(json!({ "refspecs": "pull/1/head" }), &ctx)
            .await
            .expect_err("wrongly typed refspecs should fail");
        assert!(err.to_string().contains("an array of strings"), "{err}");
    }

    #[tokio::test]
    async fn git_fetch_brings_remote_refs_without_checkout() {
        if !git_available() {
            return;
        }

        let origin = tempdir().expect("tempdir");
        init_git_repo(origin.path());
        fs::write(origin.path().join("file.txt"), "one\n").expect("write");
        commit_all(origin.path(), "first");

        let work = tempdir().expect("tempdir");
        init_git_repo(work.path());
        run_git(
            work.path(),
            &[
                "remote",
                "add",
                "origin",
                &origin.path().display().to_string(),
            ],
        );

        let ctx = ToolContext::new(work.path());
        let result = GitFetchTool
            .execute(json!({ "remote": "origin" }), &ctx)
            .await
            .expect("execute");
        assert!(result.success, "{}", result.content);

        // The refs arrived, but nothing was checked out: the work tree has no
        // file.txt and no local branch moved.
        let refs = crate::dependencies::Git::output(&["branch", "-r"], work.path())
            .expect("git should spawn");
        assert!(refs.status.success());
        let refs = String::from_utf8_lossy(&refs.stdout);
        assert!(refs.contains("origin/"), "{refs}");
        assert!(!work.path().join("file.txt").exists());
    }

    #[tokio::test]
    async fn git_merge_tree_reports_conflicts_without_touching_tree() {
        if !git_available() {
            return;
        }

        let tmp = tempdir().expect("tempdir");
        init_git_repo(tmp.path());
        fs::write(tmp.path().join("file.txt"), "base\n").expect("write");
        commit_all(tmp.path(), "base");
        let main = current_branch(tmp.path());
        run_git(tmp.path(), &["checkout", "-qb", "side"]);
        fs::write(tmp.path().join("file.txt"), "side\n").expect("write");
        commit_all(tmp.path(), "side");
        run_git(tmp.path(), &["checkout", "-q", main.as_str()]);
        fs::write(tmp.path().join("file.txt"), "base\nmain\n").expect("write");
        commit_all(tmp.path(), "main");

        let ctx = ToolContext::new(tmp.path());
        let before = fs::read(tmp.path().join("file.txt")).expect("read");
        let result = GitMergeTreeTool
            .execute(json!({ "ours": main, "theirs": "side" }), &ctx)
            .await
            .expect("execute");
        assert!(result.success, "{}", result.content);
        // Modern merge-tree shape: result tree plus the conflicted path in
        // the stage table and the CONFLICT notice.
        assert!(result.content.contains("file.txt"), "{}", result.content);
        assert!(result.content.contains("CONFLICT"), "{}", result.content);
        assert_eq!(
            fs::read(tmp.path().join("file.txt")).expect("read"),
            before,
            "merge-tree must not touch the working tree"
        );
    }

    #[tokio::test]
    async fn git_merge_tree_rejects_option_shaped_revisions() {
        let tmp = tempdir().expect("tempdir");
        let ctx = ToolContext::new(tmp.path());
        let err = GitMergeTreeTool
            .execute(json!({ "ours": "--merge-base=x", "theirs": "HEAD" }), &ctx)
            .await
            .expect_err("option-shaped rev should fail before git runs");
        assert!(matches!(err, ToolError::InvalidInput { .. }));
        assert!(err.to_string().contains("must not start with '-'"));
    }

    fn current_branch(root: &Path) -> String {
        let output = crate::dependencies::Git::output(&["branch", "--show-current"], root)
            .expect("git should spawn");
        assert!(output.status.success());
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }
}
