use crate::{Draft, Embedding, Error, Result};
use regex::RegexSet;
use sha2::{Digest, Sha256};
use std::sync::OnceLock;

pub const MAX_BODY_BYTES: usize = 8192;
pub const MAX_FRAME_BYTES: usize = 64 * 1024;

pub fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
pub fn content_hash(draft: &Draft) -> Result<String> {
    // Exclude timestamps and provenance so replaying the same normalized content
    // with a new observation time does not bypass exact-content tombstones.
    Ok(sha256(&serde_json::to_vec(&(
        draft.kind,
        &draft.title,
        &draft.body,
        &draft.key,
        &draft.tags,
    ))?))
}
pub fn ensure_no_secret(text: &str) -> Result<()> {
    static SECRETS: OnceLock<RegexSet> = OnceLock::new();
    let rules = SECRETS.get_or_init(|| RegexSet::new([
        r"-----BEGIN (?:RSA |EC |OPENSSH |DSA )?PRIVATE KEY-----",
        r"\b(?:sk-(?:proj-|ant-)?[A-Za-z0-9_-]{16,}|gh[pousr]_[A-Za-z0-9_]{20,}|github_pat_[A-Za-z0-9_]{20,})",
        r"\b(?:AKIA|ASIA)[A-Z0-9]{16}\b",
        r"\bxox[baprs]-[A-Za-z0-9-]{16,}",
        r"(?i)\bbearer\s+[A-Za-z0-9._~+/-]{16,}",
        r#"(?i)\b(?:api[_-]?key|password|passwd|client[_-]?secret|access[_-]?token)\s*[=:]\s*["']?[A-Za-z0-9+/_=.-]{8,}"#,
        r"\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}",
    ]).expect("static secret patterns are valid"));
    if rules.is_match(text) {
        Err(Error::SecretDetected)
    } else {
        Ok(())
    }
}
pub fn bounded(value: &str, name: &str, max: usize, nonempty: bool) -> Result<()> {
    if value.len() > max || (nonempty && value.trim().is_empty()) || value.contains('\0') {
        return Err(Error::Invalid(format!(
            "{name} is empty, contains NUL, or exceeds its size limit"
        )));
    }
    Ok(())
}
pub fn validate_draft(d: &Draft, now: i64) -> Result<()> {
    d.scope.validate()?;
    if d.valid_from.is_some_and(|t| t < 0)
        || d.valid_until.is_some_and(|t| t < 0)
        || matches!((d.valid_from,d.valid_until),(Some(a),Some(b)) if a>=b)
    {
        return Err(Error::Invalid("invalid valid-time interval".into()));
    }
    bounded(&d.title, "title", 256, true)?;
    bounded(&d.body, "body", MAX_BODY_BYTES, true)?;
    if let Some(k) = &d.key {
        bounded(k, "semantic key", 256, true)?;
    }
    for v in [d.confidence, d.importance] {
        if !v.is_finite() || !(0.0..=1.0).contains(&v) {
            return Err(Error::Invalid(
                "confidence and importance must be finite values in [0,1]".into(),
            ));
        }
    }
    if d.evidence.is_empty()
        || d.evidence.len() > 16
        || d.tags.len() > 32
        || d.dependencies.len() > 64
        || d.parent_ids.len() > 32
    {
        return Err(Error::Invalid(
            "evidence required; collection limits exceeded".into(),
        ));
    }
    for tag in &d.tags {
        bounded(tag, "tag", 96, true)?;
    }
    if d.expires_at.is_some_and(|t| t <= now) {
        return Err(Error::Invalid("expiry must be in the future".into()));
    }
    if let Some(rev) = &d.repository_revision {
        bounded(rev, "repository revision", 256, true)?;
    }
    for e in &d.evidence {
        bounded(&e.uri, "evidence URI", 1024, true)?;
        bounded(&e.locator, "evidence locator", 256, false)?;
        if e.observed_at < 0 || e.observed_at > now.saturating_add(300) {
            return Err(Error::Invalid("invalid observation time".into()));
        }
        if let Some(h) = &e.sha256 {
            validate_digest(h)?;
        }
    }
    for (path, digest) in &d.dependencies {
        crate::workspace::validate_relative_path(path)?;
        validate_digest(digest)?;
    }
    if (!d.dependencies.is_empty() || d.repository_revision.is_some())
        && d.scope.workspace.is_none()
    {
        return Err(Error::Invalid(
            "repository-bound memories require a workspace scope".into(),
        ));
    }
    for id in &d.parent_ids {
        bounded(id, "parent id", 128, true)?;
    }
    ensure_no_secret(&serde_json::to_string(d)?)?;
    Ok(())
}
pub fn validate_digest(h: &str) -> Result<()> {
    if h.len() != 64
        || !h
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    {
        return Err(Error::Invalid(
            "SHA-256 digests must be 64 lowercase hexadecimal characters".into(),
        ));
    }
    Ok(())
}
pub fn normalize_embedding(e: &Embedding) -> Result<Vec<f32>> {
    bounded(&e.model, "embedding model identity", 256, true)?;
    if e.vector.is_empty() || e.vector.len() > 8192 || e.vector.iter().any(|v| !v.is_finite()) {
        return Err(Error::Invalid(
            "invalid embedding dimensions or values".into(),
        ));
    }
    let norm = e
        .vector
        .iter()
        .map(|v| (*v as f64).powi(2))
        .sum::<f64>()
        .sqrt();
    if norm <= f64::EPSILON || !norm.is_finite() {
        return Err(Error::Invalid(
            "embedding norm must be finite and nonzero".into(),
        ));
    }
    Ok(e.vector.iter().map(|v| (*v as f64 / norm) as f32).collect())
}
/// All MATCH operators become literal text. Never accept raw FTS syntax from a model.
pub fn fts_query(text: &str) -> Result<Option<String>> {
    bounded(text, "query", 1024, false)?;
    let tokens: Vec<_> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .take(32)
        .collect();
    if tokens.is_empty() {
        return Ok(None);
    }
    Ok(Some(
        tokens
            .iter()
            .map(|s| format!("\"{}\"", s.replace('"', "\"\"")))
            .collect::<Vec<_>>()
            .join(" OR "),
    ))
}
pub fn excerpt(text: &str, chars: usize) -> String {
    text.chars().take(chars).collect()
}
