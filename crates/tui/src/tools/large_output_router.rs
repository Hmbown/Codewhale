//! Adaptive evidence routing for tool results (#4619) — explicit opt-in.
//!
//! Classic bounded spillover is the default: `tools/truncate.rs` keeps results
//! at or under its byte threshold fully inline and gives larger ones a
//! head/tail preview plus a session artifact. Set
//! `CODEWHALE_ADAPTIVE_OUTPUT_ROUTING` to enable the adaptive lane, which
//! classifies results as inline, hybrid, or handle-only by estimated tokens
//! and publishes non-inline results exactly once under their origin session
//! with immutable evidence metadata for bounded retrieval.

use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

use crate::tools::spec::ToolResult;

// ── Constants ──────────────────────────────────────────────────────────────────

/// Default token threshold separating hybrid from handle-only evidence.
///
/// 32K tokens (≈96 KiB of text at the 3 chars/token estimate) keeps ordinary
/// tool results — file reads, test runs, build logs up to a few thousand
/// lines — fully inline. Only genuinely large outputs spill to evidence
/// artifacts, where the model-facing preview names the artifact path and how
/// to recover the omitted range.
pub const DEFAULT_LARGE_OUTPUT_THRESHOLD_TOKENS: usize = 32_768;

/// Approximate characters-per-token ratio used for the heuristic estimate.
/// We intentionally choose a conservative value (3 chars/token) so we err
/// on the side of routing rather than dumping raw data into the parent.
const CHARS_PER_TOKEN_ESTIMATE: usize = 3;

static ACTIVE_WORKSHOP: OnceLock<Mutex<WorkshopConfig>> = OnceLock::new();

#[cfg(test)]
static ACTIVE_WORKSHOP_TEST_SERIAL: OnceLock<Mutex<()>> = OnceLock::new();

#[cfg(test)]
std::thread_local! {
    static ACTIVE_WORKSHOP_TEST_SERIAL_HELD: std::cell::Cell<bool> = const {
        std::cell::Cell::new(false)
    };
}

/// Holds every test-side workshop activation behind one process-wide gate.
/// The thread-local marker lets the owning current-thread test call
/// `install_active` without trying to acquire its own non-reentrant lock.
#[cfg(test)]
pub(crate) struct ActiveWorkshopTestGuard {
    _serial: std::sync::MutexGuard<'static, ()>,
}

#[cfg(test)]
impl Drop for ActiveWorkshopTestGuard {
    fn drop(&mut self) {
        ACTIVE_WORKSHOP_TEST_SERIAL_HELD.with(|held| held.set(false));
    }
}

#[cfg(test)]
pub(crate) fn active_workshop_test_guard() -> ActiveWorkshopTestGuard {
    assert!(
        !ACTIVE_WORKSHOP_TEST_SERIAL_HELD.with(std::cell::Cell::get),
        "active workshop test guard is not reentrant"
    );
    let serial = ACTIVE_WORKSHOP_TEST_SERIAL
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    ACTIVE_WORKSHOP_TEST_SERIAL_HELD.with(|held| held.set(true));
    ActiveWorkshopTestGuard { _serial: serial }
}

fn active_workshop_slot() -> &'static Mutex<WorkshopConfig> {
    ACTIVE_WORKSHOP.get_or_init(|| Mutex::new(WorkshopConfig::default()))
}

// ── Configuration ─────────────────────────────────────────────────────────────

/// Existing `[workshop]` threshold configuration, retained for compatibility.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct WorkshopConfig {
    /// Token threshold above which results become handle-only evidence.
    #[serde(default)]
    pub large_output_threshold_tokens: Option<usize>,

    /// Per-tool threshold overrides (tool name → token limit). A tool whose
    /// name appears here uses this limit instead of
    /// `large_output_threshold_tokens`.
    #[serde(default)]
    pub per_tool_thresholds: Option<HashMap<String, usize>>,

    /// Optional model-visible byte budget for a single `read` / `read_file`
    /// result. Absent keeps the compile-time default (#5367).
    #[serde(default)]
    pub read_result_max_bytes: Option<usize>,

    /// Optional model-visible byte budget for a generic tool result after
    /// spillover. Absent keeps the compile-time default (#5367).
    #[serde(default)]
    pub tool_result_max_bytes: Option<usize>,
}

impl WorkshopConfig {
    /// Install the process-wide workshop budgets used by read/tool compactors.
    ///
    /// The returned immutable receipt is the snapshot written while the
    /// singleton lock was held. Callers that need evidence of their own
    /// activation can inspect it without racing a later process-wide update.
    pub fn install_active(config: Option<&Self>) -> Self {
        #[cfg(test)]
        let _test_serial = if ACTIVE_WORKSHOP_TEST_SERIAL_HELD.with(std::cell::Cell::get) {
            None
        } else {
            Some(
                ACTIVE_WORKSHOP_TEST_SERIAL
                    .get_or_init(|| Mutex::new(()))
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
            )
        };
        let snapshot = config.cloned().unwrap_or_default();
        let mut slot = active_workshop_slot()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *slot = snapshot;
        slot.clone()
    }

    /// Optional model-visible read budget, when the user opted in (#5367).
    #[must_use]
    pub fn active_read_result_max_bytes() -> Option<usize> {
        active_workshop_slot()
            .lock()
            .ok()
            .and_then(|cfg| cfg.read_result_max_bytes.filter(|n| *n > 0))
    }

    /// Optional model-visible tool-result budget, when the user opted in (#5367).
    #[must_use]
    pub fn active_tool_result_max_bytes() -> Option<usize> {
        active_workshop_slot()
            .lock()
            .ok()
            .and_then(|cfg| cfg.tool_result_max_bytes.filter(|n| *n > 0))
    }

    /// Resolve the effective threshold for the given tool name.
    #[must_use]
    pub fn threshold_for(&self, tool_name: &str) -> usize {
        if let Some(per_tool) = self.per_tool_thresholds.as_ref()
            && let Some(&limit) = per_tool.get(tool_name)
        {
            return limit;
        }
        self.large_output_threshold_tokens
            .unwrap_or(DEFAULT_LARGE_OUTPUT_THRESHOLD_TOKENS)
    }
}

// ── Token estimation ──────────────────────────────────────────────────────────

/// Estimate the number of tokens in `text` using a character-count heuristic.
///
/// This avoids a real tokeniser dependency; the estimate is deliberately
/// conservative (under-counts tokens) so we route aggressively rather than
/// letting a 5K-token blob slip through.
#[must_use]
pub fn estimate_tokens(text: &str) -> usize {
    let chars = text.chars().count();
    // Round up: partial last token still costs a token.
    chars.div_ceil(CHARS_PER_TOKEN_ESTIMATE)
}

// ── Router ────────────────────────────────────────────────────────────────────

/// Classifies tool results for adaptive evidence routing.
///
/// This type is intentionally `Clone` and `Default` so it can be embedded
/// cheaply in [`ToolContext`](crate::tools::spec::ToolContext) without
/// requiring `Arc` wrappers.
#[derive(Debug, Clone, Default)]
pub struct LargeOutputRouter {
    config: WorkshopConfig,
}

impl LargeOutputRouter {
    /// Construct a router from the resolved workshop config.
    #[must_use]
    pub fn new(config: WorkshopConfig) -> Self {
        Self { config }
    }

    #[must_use]
    pub fn evidence_routing(
        &self,
        tool_name: &str,
        result: &ToolResult,
        _raw_bypass: bool,
    ) -> (EvidenceRouting, usize, usize) {
        let threshold = self.config.threshold_for(tool_name);
        let estimated_tokens = estimate_tokens(&result.content);
        // `raw=true` no longer bypasses the context bound. Exact bytes remain
        // available through the artifact handle, so bypass is unnecessary.
        let routing = EvidenceRouting::from_token_estimate(estimated_tokens, threshold);
        (routing, estimated_tokens, threshold)
    }
}

// ── Adaptive evidence routing (#4619) ─────────────────────────────────────────

/// Routing policy for tool results: how much of the output stays inline in the
/// conversation vs. being stored as an external artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceRouting {
    /// Full result stays inline in the conversation context.
    Inline,
    /// A bounded observation (head/tail/summary) stays inline; the exact bytes
    /// are stored as an artifact recoverable via handle.
    Hybrid,
    /// Only a handle/reference stays inline; the full result is artifact-only.
    HandleOnly,
}

impl EvidenceRouting {
    /// Determine routing from estimated token count and threshold.
    #[must_use]
    pub fn from_token_estimate(estimated_tokens: usize, threshold: usize) -> Self {
        if estimated_tokens <= threshold / 4 {
            Self::Inline
        } else if estimated_tokens <= threshold {
            Self::Hybrid
        } else {
            Self::HandleOnly
        }
    }
}

/// Immutable metadata for a stored evidence artifact (#4619).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceArtifact {
    pub handle: String,
    pub digest: String,
    pub size_bytes: u64,
    pub content_type: String,
    pub tool_name: String,
    pub call_id: String,
    pub origin_session: String,
    pub generation: u32,
    pub redacted: bool,
    pub encoding: String,
    pub retention_state: EvidenceRetentionState,
    pub created_at_unix_ms: u64,
    pub retain_until_unix_ms: u64,
    pub storage_path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceRetentionState {
    Live,
    Expired,
}

pub const EVIDENCE_RETENTION_SECS: u64 = 7 * 24 * 60 * 60;

/// Whether adaptive evidence routing (#4619) is enabled for this process.
///
/// Off by default — classic bounded spillover owns large results. Set
/// `CODEWHALE_ADAPTIVE_OUTPUT_ROUTING` to opt in. The retired rollback
/// variable is still honored in the negative, so
/// `CODEWHALE_CLASSIC_OUTPUT_ROUTING=0` also selects the adaptive lane.
#[must_use]
pub fn adaptive_output_routing_enabled() -> bool {
    if let Ok(value) = std::env::var("CODEWHALE_ADAPTIVE_OUTPUT_ROUTING") {
        return matches!(value.trim(), "1" | "true" | "yes" | "on");
    }
    matches!(
        std::env::var("CODEWHALE_CLASSIC_OUTPUT_ROUTING")
            .ok()
            .as_deref()
            .map(str::trim),
        Some("0" | "false" | "no" | "off")
    )
}

#[must_use]
pub fn evidence_metadata_relative_path(handle: &str) -> PathBuf {
    PathBuf::from(crate::artifacts::ARTIFACTS_DIR_NAME).join(format!("{handle}.evidence.json"))
}

pub fn publish_evidence_metadata(
    session_id: &str,
    artifact: &EvidenceArtifact,
) -> io::Result<PathBuf> {
    let bytes = serde_json::to_vec_pretty(artifact)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    crate::artifacts::write_session_relative_immutable(
        session_id,
        &evidence_metadata_relative_path(&artifact.handle),
        &bytes,
    )
}

pub fn read_evidence_metadata(session_id: &str, handle: &str) -> io::Result<EvidenceArtifact> {
    let relative = evidence_metadata_relative_path(handle);
    let path = crate::artifacts::session_artifact_absolute_path(session_id, &relative)
        .ok_or_else(|| io::Error::new(io::ErrorKind::PermissionDenied, "invalid evidence owner"))?;
    let raw = std::fs::read(path)?;
    serde_json::from_slice(&raw).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
}

#[must_use]
pub fn unix_millis_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

#[must_use]
pub fn evidence_is_expired(artifact: &EvidenceArtifact, now_ms: u64) -> bool {
    artifact.retention_state == EvidenceRetentionState::Expired
        || now_ms > artifact.retain_until_unix_ms
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_result(content: &str) -> ToolResult {
        ToolResult::success(content.to_string())
    }

    #[test]
    fn default_threshold_is_32k_tokens() {
        assert_eq!(DEFAULT_LARGE_OUTPUT_THRESHOLD_TOKENS, 32_768);
    }

    #[test]
    fn adaptive_evidence_cannot_bypass_context_bound_with_raw_flag() {
        let router = LargeOutputRouter::default();
        let big = make_result(&"a".repeat(100_000));
        let (routing, _, _) = router.evidence_routing("exec_shell", &big, true);
        assert_eq!(routing, EvidenceRouting::HandleOnly);
    }

    #[test]
    fn per_tool_threshold_override() {
        let mut per_tool = HashMap::new();
        per_tool.insert("grep_files".to_string(), 100); // very low
        let config = WorkshopConfig {
            large_output_threshold_tokens: Some(4096),
            per_tool_thresholds: Some(per_tool),
            read_result_max_bytes: None,
            tool_result_max_bytes: None,
        };
        assert_eq!(config.threshold_for("grep_files"), 100);
        assert_eq!(config.threshold_for("read_file"), 4096);
        let default_config = WorkshopConfig::default();
        assert_eq!(
            default_config.threshold_for("read_file"),
            DEFAULT_LARGE_OUTPUT_THRESHOLD_TOKENS
        );
    }

    #[test]
    fn workshop_byte_budgets_raise_floor_only() {
        let _guard = active_workshop_test_guard();
        let installed = WorkshopConfig::install_active(Some(&WorkshopConfig {
            large_output_threshold_tokens: None,
            per_tool_thresholds: None,
            read_result_max_bytes: Some(102_400),
            tool_result_max_bytes: Some(80_000),
        }));
        assert_eq!(installed.read_result_max_bytes, Some(102_400));
        assert_eq!(installed.tool_result_max_bytes, Some(80_000));
        assert_eq!(
            WorkshopConfig::active_read_result_max_bytes(),
            Some(102_400)
        );
        assert_eq!(WorkshopConfig::active_tool_result_max_bytes(), Some(80_000));
        let cleared = WorkshopConfig::install_active(None);
        assert_eq!(cleared.read_result_max_bytes, None);
        assert_eq!(cleared.tool_result_max_bytes, None);
        assert_eq!(WorkshopConfig::active_read_result_max_bytes(), None);
        assert_eq!(WorkshopConfig::active_tool_result_max_bytes(), None);
    }

    #[test]
    fn estimate_tokens_conservative() {
        // 9 chars → ceil(9/3) = 3 tokens
        assert_eq!(estimate_tokens("123456789"), 3);
        // 10 chars → ceil(10/3) = 4 tokens
        assert_eq!(estimate_tokens("1234567890"), 4);
        // Empty string
        assert_eq!(estimate_tokens(""), 0);
    }
}
