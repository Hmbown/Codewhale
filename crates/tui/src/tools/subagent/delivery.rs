//! Delivery evidence replacing the old prose-verb/git-status heuristic.
//! The worker ledger retains the spawn baseline; this module only reads files.

use super::{AgentRunVerificationSummary, AgentWorkerSpec, normalize_claim_path};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(super) const MAX_DELIVERABLES: usize = 16;
const MAX_BASELINE_PATHS: usize = 4096;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeliverableVerdict {
    pub path: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeliveryEvidence {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    baseline: Option<GitDeliveryBaseline>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub(super) observed_writes: BTreeSet<String>,
    #[serde(default)]
    pub(super) checked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct GitDeliveryBaseline {
    root: PathBuf,
    head: Option<String>,
    dirty: BTreeMap<String, String>,
}

fn git(root: &Path, args: &[&str]) -> Option<Vec<u8>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.untrackedCache=false",
        ])
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_NO_LAZY_FETCH", "1")
        .output()
        .ok()?;
    output.status.success().then_some(output.stdout)
}

fn status_paths(root: &Path) -> Option<BTreeSet<String>> {
    let output = git(
        root,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )?;
    let mut entries = output
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty());
    let mut paths = BTreeSet::new();
    while let Some(entry) = entries.next() {
        let path = std::str::from_utf8(entry.get(3..)?).ok()?;
        paths.insert(path.to_string());
        if entry[..2].iter().any(|byte| matches!(byte, b'R' | b'C')) {
            // Porcelain -z emits the destination first, then the source.
            if let Some(source) = entries.next() {
                paths.insert(std::str::from_utf8(source).ok()?.to_string());
            }
        }
    }
    (paths.len() <= MAX_BASELINE_PATHS).then_some(paths)
}

fn fingerprint(root: &Path, relative: &str) -> Option<String> {
    let mut parent = root.to_path_buf();
    let components = Path::new(relative).components().collect::<Vec<_>>();
    for component in components.iter().take(components.len().saturating_sub(1)) {
        parent.push(component);
        if fs::symlink_metadata(&parent).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
            return Some("symlink_ancestor".into());
        }
    }
    let path = root.join(relative);

    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Some("missing".into());
        }
        Err(_) => return None,
    };
    if metadata.file_type().is_symlink() {
        return Some(format!("symlink:{}", fs::read_link(&path).ok()?.display()));
    }
    if !metadata.is_file() {
        return Some("non_file".into());
    }
    let mut file = fs::File::open(&path).ok()?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 65536];
    loop {
        let count = file.read(&mut buffer).ok()?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Some(format!(
        "sha256:{}",
        crate::hashing::hex_bytes(hash.finalize())
    ))
}

impl DeliveryEvidence {
    pub(super) fn capture(spec: &AgentWorkerSpec) -> Self {
        let baseline = spec
            .runtime_profile
            .permissions
            .write
            .then(|| {
                let root =
                    String::from_utf8(git(&spec.workspace, &["rev-parse", "--show-toplevel"])?)
                        .ok()?;
                let root = PathBuf::from(root.trim());
                let head = git(&root, &["rev-parse", "--verify", "HEAD"])
                    .and_then(|bytes| String::from_utf8(bytes).ok())
                    .map(|head| head.trim().to_string());
                let dirty = status_paths(&root)?
                    .into_iter()
                    .map(|path| {
                        let path = normalize_claim_path(&path).ok()?;
                        fingerprint(&root, &path).map(|hash| (path, hash))
                    })
                    .collect::<Option<BTreeMap<_, _>>>()?;
                Some(GitDeliveryBaseline { root, head, dirty })
            })
            .flatten();
        Self {
            baseline,
            ..Self::default()
        }
    }

    pub(super) fn changed_paths(&self, workspace: &Path) -> Option<BTreeSet<String>> {
        let baseline = self.baseline.as_ref()?;
        // Persisted evidence is data, never authority to inspect another tree
        // or pass caller-controlled options to git after a restart.
        let current_root =
            String::from_utf8(git(workspace, &["rev-parse", "--show-toplevel"])?).ok()?;
        let current_root = PathBuf::from(current_root.trim());
        if !same_path_identity(&baseline.root, &current_root)
            || baseline.dirty.len() > MAX_BASELINE_PATHS
            || baseline
                .dirty
                .keys()
                .any(|path| normalize_claim_path(path).as_ref() != Ok(path))
            || baseline.head.as_ref().is_some_and(|head| {
                !matches!(head.len(), 40 | 64) || !head.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
        {
            return None;
        }
        let mut candidates = status_paths(&baseline.root)?;
        candidates.extend(baseline.dirty.keys().cloned());
        if let Some(head) = baseline.head.as_deref() {
            let output = git(
                &baseline.root,
                &[
                    "diff",
                    "--no-ext-diff",
                    "--no-textconv",
                    "--name-only",
                    "-z",
                    head,
                    "HEAD",
                    "--",
                ],
            )?;
            for path in output
                .split(|byte| *byte == 0)
                .filter(|path| !path.is_empty())
            {
                candidates.insert(std::str::from_utf8(path).ok()?.to_string());
            }
        }
        // Git status paths are already repo-relative. Prefer them when the
        // worker workspace is the git toplevel by identity; otherwise project
        // through a shared canonicalize spelling. Always normalize separators
        // so Windows `src\lib.rs` matches claim paths like `src/lib.rs`.
        let workspace_root = workspace.canonicalize().ok()?;
        let baseline_root = baseline.root.canonicalize().ok()?;
        let workspace_is_repo_root = same_path_identity(&workspace_root, &baseline_root);
        let mut changed = BTreeSet::new();
        for path in candidates {
            let Ok(path) = normalize_claim_path(&path) else {
                continue;
            };
            if let Some(before) = baseline.dirty.get(&path)
                && fingerprint(&baseline.root, &path).as_ref() == Some(before)
            {
                continue;
            }
            let relative = if workspace_is_repo_root {
                path
            } else {
                let absolute = baseline_root.join(Path::new(&path));
                let Ok(relative) = absolute.strip_prefix(&workspace_root) else {
                    continue;
                };
                let Ok(normalized) = normalize_claim_path(&relative.to_string_lossy()) else {
                    continue;
                };
                normalized
            };
            changed.insert(relative);
        }
        Some(changed)
    }
}

fn same_path_identity(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

pub(super) fn declared_paths(
    paths: &[String],
    legacy: Option<&str>,
) -> Result<Vec<String>, String> {
    if paths.len() > MAX_DELIVERABLES {
        return Err(format!(
            "deliverables accepts at most {MAX_DELIVERABLES} paths"
        ));
    }
    let mut paths = paths.to_vec();
    if paths.is_empty()
        && let Some(legacy) = legacy
        && !legacy.chars().any(char::is_whitespace)
        && (legacy.contains('/') || legacy.contains('.'))
    {
        paths.push(legacy.to_string());
    }
    let mut normalized = Vec::new();
    for path in paths {
        let path = normalize_claim_path(&path)?;
        if path == "."
            || path
                .split('/')
                .any(|part| part.eq_ignore_ascii_case(".git"))
        {
            return Err("deliverables must name files outside git metadata".into());
        }
        if !normalized.contains(&path) {
            normalized.push(path);
        }
    }
    Ok(normalized)
}

pub(super) fn safe_deliverable_path(workspace: &Path, path: &str) -> Result<PathBuf, String> {
    let path = declared_paths(&[path.to_string()], None)?
        .pop()
        .ok_or_else(|| "deliverable must name a file".to_string())?;
    let root = workspace
        .canonicalize()
        .map_err(|_| "deliverable workspace is unavailable".to_string())?;
    let mut resolved = root.clone();
    for component in Path::new(&path).components() {
        resolved.push(component);
        match fs::symlink_metadata(&resolved) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err("deliverable path traverses a symlink".into());
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err("deliverable path cannot be inspected".into()),
        }
    }
    if resolved == root || !resolved.starts_with(root) {
        return Err("deliverable must be a file inside the worker workspace".into());
    }
    Ok(resolved)
}

pub(super) fn check_deliverable(workspace: &Path, path: &str, allowed: bool) -> DeliverableVerdict {
    let mut verdict = DeliverableVerdict {
        path: path.into(),
        status: "out_of_scope".into(),
        bytes: None,
    };
    if !allowed {
        return verdict;
    }
    let resolved = match safe_deliverable_path(workspace, path) {
        Ok(path) => path,
        Err(_) => {
            verdict.status = "invalid_path".into();
            return verdict;
        }
    };
    match fs::symlink_metadata(resolved) {
        Ok(metadata) if metadata.is_file() => {
            verdict.bytes = Some(metadata.len());
            verdict.status = if metadata.len() == 0 {
                "empty"
            } else {
                "present"
            }
            .into();
        }
        Ok(_) => verdict.status = "not_file".into(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            verdict.status = "missing".into()
        }
        Err(_) => verdict.status = "unreadable".into(),
    }
    verdict
}

fn citation(token: &str) -> bool {
    // A citation may be sentence-final or either side of a Markdown link.
    // Normalize punctuation only for citation detection; never rewrite a path.
    token.split("](").any(|part| {
        let part = part.trim_end_matches(|ch: char| {
            matches!(
                ch,
                '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']' | '}' | '\'' | '"' | '`'
            )
        });
        let Some((_, line)) = part.rsplit_once(':') else {
            return false;
        };
        let mut numbers = line.split('-');
        let numeric =
            |part: &str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
        numbers.next().is_some_and(numeric)
            && numbers.next().is_none_or(numeric)
            && numbers.next().is_none()
    })
}

pub(super) fn explicit_change_paths(summary: &str) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    let mut in_changes = false;
    for line in summary.lines() {
        let line = line.trim();
        let is_heading = line.starts_with('#');
        let line = line.trim_start_matches('#').trim();
        let lower = line.to_ascii_lowercase();
        // SUBAGENT_OUTPUT_FORMAT uses a bare Markdown heading, with ordinary
        // blank lines before its file bullets. That is an explicit declaration
        // boundary just like the compatibility CHANGES: label.
        if is_heading && lower == "changes" {
            in_changes = true;
            continue;
        }
        if line.is_empty() {
            continue;
        }
        let declaration = ["changes:", "changed files:", "files changed:"]
            .into_iter()
            .find(|prefix| lower.starts_with(prefix));
        let content = if let Some(prefix) = declaration {
            in_changes = true;
            &line[prefix.len()..]
        } else if in_changes && (line.starts_with('-') || line.starts_with('*')) {
            line
        } else {
            in_changes = false;
            continue;
        };
        if matches!(
            content
                .trim()
                .trim_end_matches('.')
                .to_ascii_lowercase()
                .as_str(),
            "none" | "no files changed" | "no changes"
        ) {
            in_changes = false;
            continue;
        }
        let content = content.trim();
        let mut remaining = content
            .strip_prefix("- ")
            .or_else(|| content.strip_prefix("* "))
            .unwrap_or(content);
        // File declarations lead with paths. Stop when their description
        // starts instead of interpreting sentence-final prose ("parsing.")
        // or a later reference ("see notes.md") as another claimed edit.
        while !remaining.is_empty() {
            remaining = remaining
                .trim_start_matches(|ch: char| ch.is_whitespace() || matches!(ch, ',' | ';'));
            let Some(first) = remaining.chars().next() else {
                break;
            };
            let quoted = matches!(first, '`' | '"' | '\'');
            let (token, rest) = if quoted {
                let Some((token, rest)) = remaining[1..].split_once(first) else {
                    break;
                };
                (token, rest)
            } else {
                let end = remaining
                    .find(|ch: char| ch.is_whitespace() || matches!(ch, ',' | ';'))
                    .unwrap_or(remaining.len());
                remaining.split_at(end)
            };
            remaining = rest;
            if citation(token) || token.contains("://") {
                break;
            }
            let token = if quoted {
                token
            } else {
                token
                    .trim_matches(|ch: char| matches!(ch, '(' | ')' | '[' | ']' | '*' | '-'))
                    .trim_end_matches(['.', ':', '!', '?'])
            };
            let token = token.split_once("](").map_or(token, |(label, _)| label);
            if !token.contains('/') && !token.contains('.') {
                break;
            }
            let Ok(path) = normalize_claim_path(token) else {
                break;
            };
            if path == "." {
                break;
            }
            paths.insert(path);
        }
    }
    paths
}

pub(super) fn verify_changes(
    summary: &str,
    write_capable: bool,
    evidence: &DeliveryEvidence,
    changed: Option<&BTreeSet<String>>,
    declared_outputs: &BTreeSet<String>,
) -> Option<AgentRunVerificationSummary> {
    if !write_capable {
        return None;
    }
    let claimed = explicit_change_paths(summary);
    let missing = changed
        .map(|changed| claimed.difference(changed).cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    // A path changing inside a writable scope proves neither the actor nor a
    // child write. External tools and people can edit the same checkout, so only
    // successful bounded write receipts can support an undeclared-write claim.
    let undeclared = evidence
        .observed_writes
        .iter()
        .filter(|path| {
            changed.is_none_or(|changed| changed.contains(*path))
                && !claimed.contains(*path)
                && !declared_outputs.contains(*path)
        })
        .cloned()
        .collect::<Vec<_>>();
    if missing.is_empty() && undeclared.is_empty() {
        return None;
    }
    Some(AgentRunVerificationSummary {
        status: "claim_mismatch".into(),
        summary: format!(
            "Compared with the workspace at spawn: declared but unchanged: {missing:?}; observed changes without a change declaration: {undeclared:?}. Inspect the worker receipt."
        ),
        deliverables: Vec::new(),
    })
}
