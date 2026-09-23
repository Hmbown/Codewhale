//! Additive migration: source files are never rewritten, renamed, or deleted.
//! Imported notes are candidates. Export imports receive new local memory IDs.
use crate::{
    Access, Draft, Evidence, Memory, MemoryBackend, Result, Scope, SourceKind, Store, policy,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportIssue {
    pub line: usize,
    pub code: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportReport {
    pub created: usize,
    pub reused: usize,
    pub rejected: Vec<ImportIssue>,
    pub source_sha256: String,
}

/// Native MEMORY.md stores bullet notes. Paragraph-form legacy notes also work.
/// Headings and fenced code are skipped; no conversation/thinking dump is ingested.
pub fn parse_markdown(text: &str) -> Vec<(usize, String)> {
    let mut notes = Vec::new();
    let mut current = String::new();
    let mut start = 0;
    let mut fenced = false;
    let flush = |current: &mut String, start: usize, notes: &mut Vec<(usize, String)>| {
        if !current.trim().is_empty() {
            notes.push((start, current.trim().to_owned()));
        }
        current.clear();
    };
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            flush(&mut current, start, &mut notes);
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed == "---" {
            flush(&mut current, start, &mut notes);
            continue;
        }
        if let Some(body) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
        {
            flush(&mut current, start, &mut notes);
            start = i + 1;
            current.push_str(body);
        } else {
            if current.is_empty() {
                start = i + 1;
            } else {
                current.push('\n');
            }
            current.push_str(trimmed);
        }
    }
    flush(&mut current, start, &mut notes);
    notes
}
pub fn markdown(
    store: &mut Store,
    access: &Access,
    scope: &Scope,
    source_uri: &str,
    text: &str,
) -> Result<ImportReport> {
    policy::bounded(text, "Markdown import", 1024 * 1024, false)?;
    policy::bounded(source_uri, "import URI", 1024, true)?;
    let mut report = ImportReport {
        created: 0,
        reused: 0,
        rejected: Vec::new(),
        source_sha256: policy::sha256(text.as_bytes()),
    };
    for (line, note) in parse_markdown(text) {
        let digest = policy::sha256(note.as_bytes());
        let evidence = Evidence {
            kind: SourceKind::Import,
            uri: source_uri.into(),
            locator: format!("line:{line}"),
            sha256: Some(digest.clone()),
            observed_at: 0,
        };
        let mut draft = Draft::note(scope.clone(), policy::excerpt(&note, 80), note, evidence);
        draft.confidence = 0.25;
        let request = format!("markdown:{source_uri}:{line}:{digest}");
        match store.capture(access, &request, draft) {
            Ok(r) if r.created => report.created += 1,
            Ok(_) => report.reused += 1,
            Err(e) => report.rejected.push(ImportIssue {
                line,
                code: e.code().into(),
            }),
        }
    }
    Ok(report)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportRow {
    schema: String,
    memory: Memory,
}
/// Portable export import, NOT a restore of trusted status or a DB backup.
/// Review, graph edges, grants and original local IDs are intentionally not restored.
pub fn jsonl(
    store: &mut Store,
    access: &Access,
    scope: &Scope,
    text: &str,
) -> Result<ImportReport> {
    policy::bounded(text, "JSONL import", 16 * 1024 * 1024, false)?;
    let mut report = ImportReport {
        created: 0,
        reused: 0,
        rejected: vec![],
        source_sha256: policy::sha256(text.as_bytes()),
    };
    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let result = (|| -> Result<bool> {
            let row: ExportRow = serde_json::from_str(line)?;
            if row.schema != "codewhale.memory.export.v1" {
                return Err(crate::Error::Invalid("unsupported export schema".into()));
            }
            let source_id = row.memory.id;
            let source_hash = row.memory.content_hash;
            let mut draft = row.memory.draft;
            draft.scope = scope.clone();
            draft.parent_ids.clear();
            draft.evidence = vec![Evidence {
                kind: SourceKind::Import,
                uri: format!("codewhale-export:{source_id}"),
                locator: "imported candidate; prior authority not retained".into(),
                sha256: Some(source_hash.clone()),
                observed_at: 0,
            }];
            Ok(store
                .capture(access, &format!("export:{source_id}:{source_hash}"), draft)?
                .created)
        })();
        match result {
            Ok(true) => report.created += 1,
            Ok(false) => report.reused += 1,
            Err(e) => report.rejected.push(ImportIssue {
                line: index + 1,
                code: e.code().into(),
            }),
        }
    }
    Ok(report)
}
