use std::path::{Path as FsPath, PathBuf};

use axum::Json;
use axum::extract::{Query, State};
use serde::{Deserialize, Serialize};

use crate::dependencies::{ExternalTool as _, Git};

use super::{ApiError, RuntimeApiState};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WorkspaceFileSearchQuery {
    #[serde(default)]
    query: String,
    limit: Option<usize>,
}

#[derive(Debug, Serialize)]
pub(super) struct WorkspaceFileSearchResponse {
    paths: Vec<String>,
}

/// Read-only suggestions: never use the process cwd or a caller-selected root.
pub(super) async fn workspace_file_search(
    State(state): State<RuntimeApiState>,
    Query(query): Query<WorkspaceFileSearchQuery>,
) -> Result<Json<WorkspaceFileSearchResponse>, ApiError> {
    if query.query.len() > 256 {
        return Err(ApiError::bad_request(
            "query must be at most 256 UTF-8 bytes",
        ));
    }
    let limit = query.limit.unwrap_or(20);
    if !(1..=100).contains(&limit) {
        return Err(ApiError::bad_request("limit must be between 1 and 100"));
    }
    if query.query.trim().is_empty() {
        return Ok(Json(WorkspaceFileSearchResponse { paths: Vec::new() }));
    }
    let paths = tokio::task::spawn_blocking(move || {
        collect_workspace_file_suggestions(&state.workspace, &query.query, limit)
    })
    .await
    .map_err(|_| ApiError::internal("workspace file search failed"))??;
    Ok(Json(WorkspaceFileSearchResponse { paths }))
}

fn collect_workspace_file_suggestions(
    root: &FsPath,
    query: &str,
    limit: usize,
) -> Result<Vec<String>, ApiError> {
    use crate::working_set::{Workspace, rank_completion_candidates};

    let root = root
        .canonicalize()
        .map_err(|_| ApiError::internal("workspace is unavailable"))?;
    let workspace = Workspace::with_cwd(root.clone(), None);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    // Reuse the composer's bounded discovery and ignore policy, without its
    // divergent-cwd pass or symlink-directory traversal. No persistent index.
    let candidates = workspace
        .completion_discovery_candidates(20_000, &|| std::time::Instant::now() >= deadline);
    Ok(
        rank_completion_candidates(&candidates, query, candidates.len())
            .into_iter()
            .filter(|candidate| {
                let path = FsPath::new(candidate);
                path.components()
                    .all(|component| matches!(component, std::path::Component::Normal(_)))
                    && root
                        .join(path)
                        .canonicalize()
                        .is_ok_and(|resolved| resolved.starts_with(&root) && resolved.is_file())
            })
            .take(limit)
            .collect(),
    )
}

#[derive(Debug, Serialize)]
pub(super) struct WorkspaceStatusResponse {
    pub(super) workspace: PathBuf,
    pub(super) git_repo: bool,
    pub(super) branch: Option<String>,
    pub(super) head: Option<String>,
    pub(super) dirty: bool,
    pub(super) staged: usize,
    pub(super) unstaged: usize,
    pub(super) untracked: usize,
    pub(super) ahead: Option<u32>,
    pub(super) behind: Option<u32>,
}

#[derive(Debug, Default)]
pub(super) struct WorkspaceGitMetadata {
    pub(super) branch: Option<String>,
    pub(super) head: Option<String>,
    pub(super) dirty: bool,
}

pub(super) async fn workspace_status(
    State(state): State<RuntimeApiState>,
) -> Result<Json<WorkspaceStatusResponse>, ApiError> {
    Ok(Json(collect_workspace_status(&state.workspace)))
}

pub(super) fn collect_workspace_status(workspace: &FsPath) -> WorkspaceStatusResponse {
    let mut status = WorkspaceStatusResponse {
        workspace: workspace.to_path_buf(),
        git_repo: false,
        branch: None,
        head: None,
        dirty: false,
        staged: 0,
        unstaged: 0,
        untracked: 0,
        ahead: None,
        behind: None,
    };

    let Some(repo_check) = run_git(workspace, &["rev-parse", "--is-inside-work-tree"]) else {
        return status;
    };
    if repo_check.trim() != "true" {
        return status;
    }

    status.git_repo = true;
    let metadata = collect_workspace_git_metadata(workspace);
    status.branch = metadata.branch;
    status.head = metadata.head;
    status.dirty = metadata.dirty;

    if let Some(porcelain) = run_git(workspace, &["status", "--porcelain=v1"]) {
        for line in porcelain.lines() {
            if line.starts_with("??") {
                status.untracked += 1;
                continue;
            }
            let chars: Vec<char> = line.chars().collect();
            if chars.len() >= 2 {
                if chars[0] != ' ' {
                    status.staged += 1;
                }
                if chars[1] != ' ' {
                    status.unstaged += 1;
                }
            }
        }
    }

    if let Some(counts) = run_git(
        workspace,
        &["rev-list", "--left-right", "--count", "@{upstream}...HEAD"],
    ) {
        let mut parts = counts.split_whitespace();
        if let (Some(behind), Some(ahead)) = (parts.next(), parts.next()) {
            status.behind = behind.parse::<u32>().ok();
            status.ahead = ahead.parse::<u32>().ok();
        }
    }

    status
}

pub(super) fn collect_workspace_git_metadata(workspace: &FsPath) -> WorkspaceGitMetadata {
    let Some(repo_check) = run_git(workspace, &["rev-parse", "--is-inside-work-tree"]) else {
        return WorkspaceGitMetadata::default();
    };
    if repo_check.trim() != "true" {
        return WorkspaceGitMetadata::default();
    }

    WorkspaceGitMetadata {
        branch: current_git_branch(workspace),
        head: current_git_head(workspace),
        dirty: run_git(workspace, &["status", "--porcelain=v1"])
            .is_some_and(|porcelain| !porcelain.trim().is_empty()),
    }
}

fn run_git(workspace: &FsPath, args: &[&str]) -> Option<String> {
    let output = Git::output(args, workspace).ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

fn current_git_branch(workspace: &FsPath) -> Option<String> {
    let repo_check = run_git(workspace, &["rev-parse", "--is-inside-work-tree"])?;
    if repo_check.trim() != "true" {
        return None;
    }
    let branch = run_git(workspace, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    let branch = branch.trim();
    if branch.is_empty() {
        return None;
    }
    if branch != "HEAD" {
        return Some(branch.to_string());
    }
    let short_hash = run_git(workspace, &["rev-parse", "--short", "HEAD"])?;
    let short_hash = short_hash.trim();
    (!short_hash.is_empty()).then(|| format!("detached@{short_hash}"))
}

fn current_git_head(workspace: &FsPath) -> Option<String> {
    let head = run_git(workspace, &["rev-parse", "--short", "HEAD"])?;
    let head = head.trim();
    (!head.is_empty()).then(|| head.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_file_search_reuses_discovery_ignores() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".git"))?;
        std::fs::create_dir_all(root.join(".agents"))?;
        std::fs::create_dir_all(root.join("target"))?;
        std::fs::write(root.join(".gitignore"), ".agents/\ntarget/\nlocal.rs\n")?;
        std::fs::write(root.join(".ignore"), "blocked.rs\n")?;
        std::fs::write(root.join(".deepseekignore"), "private.rs\n")?;
        for name in [
            "local.rs",
            "blocked.rs",
            "private.rs",
            ".agents/guide.rs",
            "target/build.rs",
        ] {
            std::fs::write(root.join(name), "not returned")?;
        }
        let paths = collect_workspace_file_suggestions(root, ".rs", 100)
            .map_err(|error| anyhow::anyhow!(error.message))?;
        assert_eq!(paths, [".agents/guide.rs", "local.rs"]);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn workspace_file_search_contains_symlinks_and_excludes_other_workspaces() -> anyhow::Result<()>
    {
        use std::os::unix::fs::symlink;

        let tmp = tempfile::tempdir()?;
        let root = tmp.path().join("workspace");
        let other = tmp.path().join("workspace-other");
        std::fs::create_dir_all(&root)?;
        std::fs::create_dir_all(&other)?;
        std::fs::write(root.join("inside.rs"), "inside")?;
        std::fs::write(other.join("outside.rs"), "outside")?;
        symlink(root.join("inside.rs"), root.join("alias.rs"))?;
        symlink(other.join("outside.rs"), root.join("escape.rs"))?;
        symlink(&other, root.join("external-directory"))?;
        symlink(&other, root.join(".agents"))?;
        symlink(other.join("missing.rs"), root.join("broken.rs"))?;
        symlink(&root, root.join("loop"))?;
        let paths = collect_workspace_file_suggestions(&root, ".rs", 100)
            .map_err(|error| anyhow::anyhow!(error.message))?;
        assert_eq!(paths, ["alias.rs", "inside.rs"]);
        Ok(())
    }
}
