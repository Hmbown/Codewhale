//! Find and scrub credentials that older builds stored in saved sessions.
//!
//! Tool output is redacted as it enters the transcript now (B1), so new
//! session files never hold a live token. Sessions written before that fix
//! can: the redaction used to run only when a request was built, while the
//! transcript on disk kept the raw tool output (a `cat ~/.codex/auth.json`
//! result, a printed bearer token). Nothing rewrites those files on its own —
//! `codewhale doctor` reports them and `codewhale sessions scrub-secrets`
//! rewrites them on request.
//!
//! Only `tool_result` text is touched (in `messages`, the journal, and
//! checkpoints), using the same credential-shaped masking the model boundary
//! applies, so file bytes the model quoted for exact edits stay intact.

use std::io;
use std::path::{Path, PathBuf};

use serde_json::Value;

/// The documented command that rewrites affected sessions.
pub(crate) const SCRUB_COMMAND: &str = "codewhale sessions scrub-secrets";

/// What a scan found (and, when applied, rewrote).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct ScrubReport {
    /// Session and checkpoint files examined.
    pub files_scanned: usize,
    /// Files holding at least one unredacted credential in tool output.
    pub flagged_files: Vec<PathBuf>,
    /// Tool-result text fields that contained a credential.
    pub flagged_tool_results: usize,
    /// Files the scan could not read or parse, left untouched.
    pub unreadable: Vec<PathBuf>,
}

/// Session JSON files under `sessions_dir`: `<id>.json` and
/// `checkpoints/<id>.json`, newest first.
pub(crate) fn session_files(sessions_dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    for dir in [sessions_dir.to_path_buf(), sessions_dir.join("checkpoints")] {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if !meta.is_file() {
                continue;
            }
            let modified = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
            files.push((modified, path));
        }
    }
    files.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    files.into_iter().map(|(_, path)| path).collect()
}

/// Scan `files`; with `apply`, rewrite each affected file atomically with its
/// tool-result credentials masked. Each rewrite re-reads its file under the
/// same per-session lock every session save takes, so a live session saving
/// at the same moment is never overwritten with older content.
pub(crate) fn scrub_files(
    files: &[PathBuf],
    apply: Option<&crate::session_manager::SessionManager>,
) -> io::Result<ScrubReport> {
    let mut report = ScrubReport::default();
    for path in files {
        report.files_scanned += 1;
        // The scan is read-only; its counts are what the report prints.
        let scan = scrub_file(path, false)?;
        let scan = match (apply, scan) {
            (Some(manager), FileScan::Dirty(redacted)) => {
                // `<id>.json` and `checkpoints/<id>.json` share the id's lock.
                // The rewrite re-reads the file under that lock, so it masks
                // whatever the file holds at that moment.
                let session_id = path.file_stem().and_then(|stem| stem.to_str());
                match session_id.map(|id| {
                    manager.with_session_file_lock(id, || scrub_file(path, true).map(|_| ()))
                }) {
                    Some(Ok(Some(()))) => FileScan::Dirty(redacted),
                    // A deleted session is not resurrected by a rewrite.
                    Some(Ok(None)) => FileScan::Clean,
                    // No lockable session id: leave the file untouched.
                    None => FileScan::Unreadable,
                    Some(Err(error)) if error.kind() == io::ErrorKind::InvalidInput => {
                        FileScan::Unreadable
                    }
                    Some(Err(error)) => return Err(error),
                }
            }
            (_, scan) => scan,
        };
        match scan {
            FileScan::Unreadable => report.unreadable.push(path.clone()),
            FileScan::Clean => {}
            FileScan::Dirty(redacted) => {
                report.flagged_tool_results += redacted;
                report.flagged_files.push(path.clone());
            }
        }
    }
    Ok(report)
}

enum FileScan {
    Unreadable,
    Clean,
    Dirty(usize),
}

fn scrub_file(path: &Path, apply: bool) -> io::Result<FileScan> {
    let Ok(raw) = std::fs::read(path) else {
        return Ok(FileScan::Unreadable);
    };
    let Ok(mut value) = serde_json::from_slice::<Value>(&raw) else {
        return Ok(FileScan::Unreadable);
    };
    let redacted = scrub_value(&mut value);
    if redacted == 0 {
        return Ok(FileScan::Clean);
    }
    if apply {
        let bytes = serde_json::to_vec_pretty(&value).map_err(io::Error::other)?;
        codewhale_config::persistence::atomic_write(path, &bytes).map_err(io::Error::other)?;
    }
    Ok(FileScan::Dirty(redacted))
}

/// Mask credentials in every `tool_result` text field below `value`.
/// Returns how many fields changed.
fn scrub_value(value: &mut Value) -> usize {
    match value {
        Value::Object(map) => {
            let mut changed = 0;
            if map.get("type").and_then(Value::as_str) == Some("tool_result") {
                if let Some(Value::String(content)) = map.get_mut("content") {
                    changed += usize::from(scrub_text(content));
                }
                if let Some(Value::Array(blocks)) = map.get_mut("content_blocks") {
                    for block in blocks {
                        if block.get("type").and_then(Value::as_str) == Some("text")
                            && let Some(Value::String(text)) = block.get_mut("text")
                        {
                            changed += usize::from(scrub_text(text));
                        }
                    }
                }
            }
            // Strings below are only rewritten through a tool_result above,
            // so walking the rest (including this object's own fields) never
            // double-counts.
            changed + map.values_mut().map(scrub_value).sum::<usize>()
        }
        Value::Array(items) => items.iter_mut().map(scrub_value).sum(),
        _ => 0,
    }
}

fn scrub_text(text: &mut String) -> bool {
    let redacted = codewhale_config::persistence::redact_model_bound_secrets(text);
    if redacted == *text {
        return false;
    }
    *text = redacted;
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const TOKEN: &str = "sk-ant-oat01-AbCdEfGhIjKlMnOpQrStUvWxYz0123456789abcdefghij";

    fn session_with_tool_output(output: &str) -> Value {
        json!({
            "schema_version": 1,
            "metadata": {"id": "s1"},
            "messages": [
                {"role": "user", "content": [{"type": "text", "text": "show auth"}]},
                {"role": "user", "content": [{
                    "type": "tool_result",
                    "tool_use_id": "call-1",
                    "content": output,
                    "content_blocks": [{"type": "text", "text": output}]
                }]}
            ],
            "journal": {"entries": [{"message": {"role": "user", "content": [{
                "type": "tool_result", "tool_use_id": "call-1", "content": output
            }]}}]}
        })
    }

    #[test]
    fn scan_reports_and_scrub_masks_stored_tool_output_tokens() {
        let dir = tempfile::tempdir().expect("tempdir");
        let output = format!("{{\"access_token\": \"{TOKEN}\"}}");
        let dirty = dir.path().join("dirty.json");
        std::fs::write(&dirty, session_with_tool_output(&output).to_string()).unwrap();
        let clean = dir.path().join("clean.json");
        std::fs::write(
            &clean,
            session_with_tool_output("nothing secret").to_string(),
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("checkpoints")).unwrap();
        let checkpoint = dir.path().join("checkpoints").join("dirty.json");
        std::fs::write(&checkpoint, session_with_tool_output(&output).to_string()).unwrap();
        std::fs::write(dir.path().join("broken.json"), "{not json").unwrap();

        let files = session_files(dir.path());
        assert_eq!(files.len(), 4, "{files:?}");

        let report = scrub_files(&files, None).expect("scan");
        assert_eq!(report.files_scanned, 4);
        assert_eq!(report.flagged_files.len(), 2, "{report:?}");
        assert_eq!(report.flagged_tool_results, 6);
        assert_eq!(report.unreadable, vec![dir.path().join("broken.json")]);
        assert!(
            std::fs::read_to_string(&dirty).unwrap().contains(TOKEN),
            "a scan must not rewrite anything"
        );

        let manager =
            crate::session_manager::SessionManager::new(dir.path().to_path_buf()).expect("manager");
        let applied = scrub_files(&files, Some(&manager)).expect("scrub");
        assert_eq!(applied.flagged_files.len(), 2);
        assert_eq!(applied.unreadable, vec![dir.path().join("broken.json")]);
        for path in [&dirty, &checkpoint] {
            let text = std::fs::read_to_string(path).unwrap();
            assert!(!text.contains(TOKEN), "{text}");
            let value: Value = serde_json::from_str(&text).expect("still valid JSON");
            assert_eq!(value["metadata"]["id"], "s1");
            assert_eq!(value["messages"][0]["content"][0]["text"], "show auth");
        }
        assert_eq!(
            scrub_files(&files, None).expect("rescan").flagged_files,
            Vec::<PathBuf>::new(),
            "a scrubbed store scans clean"
        );
    }

    #[test]
    fn non_tool_result_text_is_left_alone() {
        let mut value = json!({
            "messages": [{"role": "user", "content": [{"type": "text", "text": TOKEN}]}]
        });
        assert_eq!(scrub_value(&mut value), 0);
        assert_eq!(value["messages"][0]["content"][0]["text"], TOKEN);
    }
}
