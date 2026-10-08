//! Host-side freshness collection. A model never supplies a trusted Snapshot.
use crate::{Error, Result, Snapshot, policy};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Component, Path},
    process::Command,
};

pub fn validate_relative_path(path: &str) -> Result<()> {
    policy::bounded(path, "dependency path", 1024, true)?;
    // Also reject Windows separators and drive forms on Unix: exports are portable.
    if path.contains('\\')
        || path.contains(':')
        || Path::new(path)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(Error::Invalid(
            "dependency paths must be relative and traversal-free".into(),
        ));
    }
    Ok(())
}
/// Missing, unreadable, symlink-escaped, or oversized files remain unknown.
/// Hashes the working tree, not only HEAD, so uncommitted edits invalidate memory.
pub fn snapshot(root: &Path, paths: impl IntoIterator<Item = String>) -> Result<Snapshot> {
    let root = fs::canonicalize(root)?;
    if !root.is_dir() {
        return Err(Error::Invalid("workspace root is not a directory".into()));
    }
    let revision = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["rev-parse", "--verify", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_owned());
    let mut files = BTreeMap::new();
    let mut remaining = 64usize * 1024 * 1024;
    for (i, path) in paths.into_iter().enumerate() {
        if i >= 4096 {
            break;
        }
        validate_relative_path(&path)?;
        let Ok(resolved) = fs::canonicalize(root.join(&path)) else {
            continue;
        };
        if !resolved.starts_with(&root) {
            continue;
        }
        let Ok(meta) = fs::metadata(&resolved) else {
            continue;
        };
        if !meta.is_file() || meta.len() > 16 * 1024 * 1024 {
            continue;
        }
        if meta.len() as usize > remaining {
            continue;
        }
        let Ok(file) = fs::File::open(resolved) else {
            continue;
        };
        let cap = remaining.min(16 * 1024 * 1024);
        let mut bytes = Vec::new();
        if file.take(cap as u64 + 1).read_to_end(&mut bytes).is_err() || bytes.len() > cap {
            continue;
        }
        remaining = remaining.saturating_sub(bytes.len());
        files.insert(path, policy::sha256(&bytes));
        if remaining == 0 {
            break;
        }
    }
    Ok(Snapshot { revision, files })
}
