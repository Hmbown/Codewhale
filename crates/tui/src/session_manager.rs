//! Session management for resuming conversations.
//!
//! This module provides functionality for:
//! - Saving sessions to disk
//! - Listing previous sessions
//! - Resuming sessions by ID
//! - Managing session lifecycle

use crate::approval_log::{ApprovalReceipt, ApprovalReceiptStore, ApprovalReplay};
use crate::artifacts::ArtifactRecord;
use crate::config::ApiProvider;
use crate::model_routing::AutoRouteReceipt;
use crate::project_context::find_git_root;
use crate::session_tree::{SessionEntry, SessionImportContainer, SessionJournal};
use crate::tools::goal::{GoalPauseReason, GoalSnapshot};
use crate::tools::plan::PlanSnapshot;
use crate::tools::todo::TodoListSnapshot;
use crate::utils::write_atomic;
use crate::work_graph::ReasoningEffortTier;
use chrono::{DateTime, Utc};
use codewhale_core::ContextReference;
#[cfg(test)]
use codewhale_core::{ContextReferenceKind, ContextReferenceSource};
use codewhale_models::{ContentBlock, Message, SystemPrompt};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Component, Path, PathBuf};
use uuid::Uuid;

/// Maximum number of sessions to retain
const MAX_SESSIONS: usize = 50;
/// Maximum session title length, in `char`s. Matches the bound the session
/// picker's rename prompt has always enforced.
pub const MAX_SESSION_TITLE_CHARS: usize = 100;
const WORK_GRAPH_IMPORT_ARCHIVE_DIR: &str = ".work-graph-import-archive";
const SESSION_GOALS_DIR: &str = ".goals";
const CURRENT_SESSION_GOAL_SCHEMA_VERSION: u32 = 2;
const MAX_SESSION_GOAL_OBJECTIVE_CHARS: usize = 8_192;
const MAX_SESSION_GOAL_FILE_BYTES: u64 = 64 * 1_024;
const CURRENT_SESSION_SCHEMA_VERSION: u32 = 1;
const CURRENT_QUEUE_SCHEMA_VERSION: u32 = 1;
const LATE_USAGE_DIR: &str = ".late-usage";
const CURRENT_LATE_USAGE_SCHEMA_VERSION: u32 = 1;
const MAX_LATE_USAGE_RECORDS_PER_SESSION: usize = 64;
const MAX_LATE_USAGE_LEDGER_BYTES: u64 = 1024 * 1024;
const LATE_USAGE_DELETED: &[u8] = b"codewhale-session-deleted-v1\n";
const LATE_USAGE_UNAVAILABLE_REASON: &str = "late_usage_ledger_unavailable";

#[derive(Clone, Copy)]
enum SessionRemoval {
    Explicit,
    Retention,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LateUsageRecord {
    source_fingerprint: String,
    turn_fingerprint: String,
    route: crate::cost_status::EffectiveRouteEnvelope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    usage: Option<codewhale_models::Usage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LateUsageLedger {
    schema_version: u32,
    #[serde(default)]
    records: Vec<LateUsageRecord>,
    #[serde(default)]
    overflowed: bool,
}

impl Default for LateUsageLedger {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_LATE_USAGE_SCHEMA_VERSION,
            records: Vec::new(),
            overflowed: false,
        }
    }
}

fn is_sha256_fingerprint(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

const fn default_session_schema_version() -> u32 {
    CURRENT_SESSION_SCHEMA_VERSION
}

const fn default_queue_schema_version() -> u32 {
    CURRENT_QUEUE_SCHEMA_VERSION
}

fn normalize_managed_dir(path: PathBuf) -> std::io::Result<PathBuf> {
    if path.as_os_str().is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "managed directory path cannot be empty",
        ));
    }
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::Prefix(_) | Component::RootDir
        )
    }) && path.is_relative()
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "managed directory path cannot contain traversal components",
        ));
    }
    if path.is_absolute() {
        return Ok(path);
    }
    std::env::current_dir().map(|cwd| cwd.join(path))
}

fn open_private_lock_file(path: &Path) -> io::Result<fs::File> {
    let mut options = OpenOptions::new();
    options.create(true).read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
        let file = options.open(path)?;
        validate_private_regular_file(&file, path)?;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
        Ok(file)
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt as _;
        use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
        let file = options.open(path)?;
        validate_private_regular_file(&file, path)?;
        Ok(file)
    }
    #[cfg(all(not(unix), not(windows)))]
    {
        let file = options.open(path)?;
        validate_private_regular_file(&file, path)?;
        Ok(file)
    }
}

fn open_private_read_file(path: &Path) -> io::Result<fs::File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt as _;
        use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options.open(path)?;
    validate_private_regular_file(&file, path)?;
    Ok(file)
}

#[cfg(unix)]
fn validate_private_regular_file(file: &fs::File, path: &Path) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt as _;

    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.nlink() != 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "private sidecar file {} must be one regular filesystem link",
                path.display()
            ),
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn validate_private_regular_file(file: &fs::File, path: &Path) -> io::Result<()> {
    use std::os::windows::fs::MetadataExt as _;
    use std::os::windows::io::AsRawHandle as _;
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_REPARSE_POINT, GetFileInformationByHandle,
    };

    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "private sidecar file {} must be a non-reparse regular file",
                path.display()
            ),
        ));
    }
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: `file` keeps the handle valid and `info` is writable for the call.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if info.nNumberOfLinks != 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "private sidecar file {} must have exactly one filesystem link",
                path.display()
            ),
        ));
    }
    Ok(())
}

#[cfg(all(not(unix), not(windows)))]
fn validate_private_regular_file(file: &fs::File, path: &Path) -> io::Result<()> {
    if !file.metadata()?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("private sidecar file {} must be regular", path.display()),
        ));
    }
    Ok(())
}

/// Persisted queued message for offline/degraded mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedSessionMessage {
    pub display: String,
    #[serde(default)]
    pub skill_instruction: Option<String>,
    #[serde(default)]
    pub skill_provenance: Option<crate::plugins::types::PluginAuthority>,
}

/// Persisted queue state for recovery after restart/crash.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfflineQueueState {
    #[serde(default = "default_queue_schema_version")]
    pub schema_version: u32,
    /// Session ID this queue belongs to. Redundant with the per-session file
    /// name it is stored under; the UI's restore path still compares it
    /// against the live session before adopting the messages.
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub messages: Vec<QueuedSessionMessage>,
    #[serde(default)]
    pub draft: Option<QueuedSessionMessage>,
}

/// Result of explicitly repairing a persisted session for process resume.
///
/// Normal snapshot reads must not infer that an unmatched tool call crashed:
/// an embedding host can persist and inspect a session while that tool is
/// still running. Hosts should use [`SessionManager::load_session_snapshot`]
/// during normal operation and reserve this recovery path for a known process
/// or engine restart.
#[derive(Debug, Clone)]
pub struct SessionRecovery {
    pub session: SavedSession,
    pub changed: bool,
    pub repaired_call_count: usize,
    pub duplicate_result_count: usize,
    pub orphan_result_count: usize,
}

impl Default for OfflineQueueState {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_QUEUE_SCHEMA_VERSION,
            session_id: None,
            messages: Vec::new(),
            draft: None,
        }
    }
}

/// Durable context-reference metadata attached to a user message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionContextReference {
    pub message_index: usize,
    pub reference: ContextReference,
}

/// Session metadata stored with each saved session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    /// Unique session identifier
    pub id: String,
    /// Actual host Runtime authority; independent of the conversation id.
    /// Legacy/imported conversations have no binding until saved by a host.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_store: Option<crate::runtime_threads::RuntimeStoreBinding>,
    /// Human-readable title (derived from first message)
    pub title: String,
    /// When the session was created
    pub created_at: DateTime<Utc>,
    /// When the session was last updated
    pub updated_at: DateTime<Utc>,
    /// Number of messages in the session
    pub message_count: usize,
    /// Total tokens used
    pub total_tokens: u64,
    /// Model used for the session
    pub model: String,
    /// Provider used for the session model. Defaults for legacy saved sessions.
    #[serde(default = "default_model_provider")]
    pub model_provider: String,
    /// Exact configured provider key. This is separate from `model_provider`
    /// so old consumers can keep treating that field as the built-in provider
    /// kind (`custom` for every named custom route).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_provider_id: Option<String>,
    /// Workspace directory
    pub workspace: PathBuf,
    /// Optional mode label (agent/plan/etc.)
    #[serde(default)]
    pub mode: Option<String>,
    /// Accumulated cost data for persisted billing and high-water mark.
    #[serde(default)]
    pub cost: SessionCostSnapshot,
    /// Source session id when this session was created with `deepseek fork`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_session_id: Option<String>,
    /// Source message count at fork time. This is intentionally coarse:
    /// current saved sessions are linear JSON files, not per-entry trees.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forked_from_message_count: Option<usize>,
    /// Cumulative turn duration in seconds (sum of completed turn elapsed
    /// times). Persisted so the footer "worked" chip survives restarts
    /// (#2038).
    #[serde(default)]
    pub cumulative_turn_secs: u64,
    /// Durable archive flag (#2934 / #4397). Archived sessions stay on disk
    /// and stay loadable; they are hidden from the default browse surfaces
    /// and are never chosen by auto-resume.
    ///
    /// This mirrors `ThreadRecord::archived` in [`crate::runtime_threads`] so
    /// the TUI session surfaces and the Runtime API/web dashboard project the
    /// same lifecycle field instead of two divergent notions of "put away".
    /// Additive and `skip_serializing_if`-guarded: sessions written before
    /// v0.9.2 load as `archived = false` and round-trip byte-identically
    /// until the flag is actually set.
    #[serde(default, skip_serializing_if = "is_not_archived")]
    pub archived: bool,
    #[serde(default)]
    pub spawn_depth: u32,
}

fn is_not_archived(archived: &bool) -> bool {
    !*archived
}

/// Sessions currently owned by an in-process interactive surface (the TUI).
///
/// A saved session is a file, and a running TUI holds the authoritative copy
/// in memory: it autosaves the whole document from `App` state. That makes an
/// out-of-band write to the *same* session unsafe — the next autosave would
/// silently revert it. Rather than let that happen quietly, the owner claims
/// the id here and any external writer is refused.
///
/// A static registry rather than a field on `RuntimeApiState` because the
/// embedded Runtime API runs inside the TUI process; a standalone
/// `codewhale web` has an empty registry and is therefore never blocked, which
/// is exactly right — there is no TUI holding anything.
static LIVE_SESSIONS: std::sync::OnceLock<std::sync::RwLock<std::collections::HashSet<String>>> =
    std::sync::OnceLock::new();

fn live_sessions() -> &'static std::sync::RwLock<std::collections::HashSet<String>> {
    LIVE_SESSIONS.get_or_init(Default::default)
}

/// Who is asking to mutate a saved session.
///
/// This is an authority distinction, not a convenience one: the owner may
/// write because it will update its in-memory copy in the same step; anyone
/// else may not, because it cannot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionMutator {
    /// The in-process surface that currently owns the session (the TUI). It
    /// is responsible for updating its cached metadata atomically with the
    /// write — see `App::apply_session_mutation`.
    Owner,
    /// Any other writer: the Runtime API, the web dashboard, a second
    /// process. Refused while the session is claimed.
    External,
}

/// Set the claimed session to exactly `session_id` (or nothing).
///
/// The TUI owns at most one session at a time, so switching sessions must
/// release the previous claim in the same step — otherwise a `/new` would
/// leave the old id permanently locked against the dashboard.
pub fn set_live_session(session_id: Option<&str>) {
    if let Ok(mut live) = live_sessions().write() {
        live.clear();
        if let Some(id) = session_id.map(str::trim).filter(|id| !id.is_empty()) {
            live.insert(id.to_string());
        }
    }
}

/// Is this session currently owned by **this process's** interactive surface?
///
/// The registry is process-local. Reclamation must not treat a missing entry
/// here as proof that no other Codewhale process still owns the directory.
#[must_use]
pub fn is_live_session(session_id: &str) -> bool {
    live_sessions()
        .read()
        .is_ok_and(|live| live.contains(session_id))
}

/// The error an external writer gets when the session is live.
///
/// `ResourceBusy` so callers can map it to a typed conflict rather than
/// pattern-matching on a message.
fn live_session_conflict(session_id: &str) -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::ResourceBusy,
        format!(
            "session '{session_id}' is open in an interactive Codewhale session; \
             change it there instead — an external write would be reverted by its next autosave"
        ),
    )
}

/// File-name stem of the sidecar mapping session ids to the session
/// instance (process boot) that created their persisted record. Lives in
/// the sessions directory next to the `<id>.json` records it describes.
const SESSION_BOOT_OWNERS_STEM: &str = "session_boot_owners";

static SESSION_BOOT_ID: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Identity of this running session instance (one per process boot).
///
/// Mirrors the `SubAgentManager` boot id from #405: persisted records are
/// stamped with the instance that created them, so a later Codewhale
/// instance in the same workspace can tell restored rows from its own live
/// work (#4416).
#[must_use]
pub fn current_session_boot_id() -> &'static str {
    SESSION_BOOT_ID.get_or_init(|| format!("boot_{}", &Uuid::new_v4().to_string()[..12]))
}

/// Which archive states a session listing includes.
///
/// Deliberately the same three-way shape as
/// [`crate::runtime_threads::ThreadListFilter`] so `/v1/sessions` and
/// `/v1/threads` answer the same `include_archived` / `archived_only` query
/// pair with the same semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SessionListFilter {
    /// Only `archived = false` sessions. The browse default.
    #[default]
    ActiveOnly,
    /// Active and archived sessions, newest first.
    IncludeArchived,
    /// Only `archived = true` sessions.
    ArchivedOnly,
}

impl SessionListFilter {
    /// Resolve the `include_archived` / `archived_only` query pair the same
    /// way the threads routes do.
    #[must_use]
    pub fn from_query(include_archived: Option<bool>, archived_only: Option<bool>) -> Self {
        if archived_only.unwrap_or(false) {
            Self::ArchivedOnly
        } else if include_archived.unwrap_or(false) {
            Self::IncludeArchived
        } else {
            Self::ActiveOnly
        }
    }

    #[must_use]
    pub fn admits(self, archived: bool) -> bool {
        match self {
            Self::ActiveOnly => !archived,
            Self::IncludeArchived => true,
            Self::ArchivedOnly => archived,
        }
    }
}

fn default_model_provider() -> String {
    "deepseek".to_string()
}

impl SessionMetadata {
    pub(crate) fn set_model_provider_route(&mut self, kind: &str, identity: Option<&str>) {
        self.model_provider = kind.to_string();
        self.model_provider_id = identity.map(str::to_string);
    }
}

/// Cost and high-water-mark fields persisted with each session.
///
/// The coverage fields below are persisted **alongside** the money so a restored
/// session can still say what its total covers. Without them a reload produced a
/// dollar figure with no completeness information, which then rendered as "0 of 0
/// turns priced" — a fabricated claim of a complete total. Sessions written
/// before these fields existed deserialize them from `Default`, which is
/// indistinguishable from that same false reading, so the load path detects the
/// legacy shape explicitly (see [`Self::coverage_is_legacy_unknown`]) rather than
/// trusting the defaults (#4318).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionCostSnapshot {
    /// Accumulated parent-turn session cost in USD.
    #[serde(default)]
    pub session_cost_usd: f64,
    /// Accumulated parent-turn session cost in CNY.
    #[serde(default)]
    pub session_cost_cny: f64,
    /// Accumulated sub-agent/background LLM cost in USD.
    #[serde(default)]
    pub subagent_cost_usd: f64,
    /// Accumulated sub-agent/background LLM cost in CNY.
    #[serde(default)]
    pub subagent_cost_cny: f64,
    /// Max-ever displayed session+subagent cost in USD (preserves #244
    /// monotonic guarantee across session restarts).
    #[serde(default)]
    pub displayed_cost_high_water_usd: f64,
    /// Max-ever displayed session+subagent cost in CNY.
    #[serde(default)]
    pub displayed_cost_high_water_cny: f64,
    /// Turns whose route was money-metered and produced an authoritative price.
    /// These are exactly the turns the persisted totals contain.
    #[serde(default)]
    pub priced_turns: u32,
    /// Money-metered (or unknown-basis) turns that produced no authoritative
    /// price, so their spend is missing from the persisted totals.
    #[serde(default)]
    pub unpriced_turns: u32,
    /// CNY-specific coverage. USD-only routes are unpriced in CNY rather than
    /// silently contributing a fabricated zero.
    #[serde(default)]
    pub cny_priced_turns: u32,
    #[serde(default)]
    pub cny_unpriced_turns: u32,
    /// Stable reason labels for the unpriced turns.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub unpriced_reasons: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub cny_unpriced_reasons: BTreeSet<String>,
    /// Token classes used on some route that carry no published price.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub unpriced_classes: BTreeSet<String>,
    /// Provenance labels of the pricing rows the totals were built from.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub pricing_provenances: BTreeSet<String>,
    /// Live-pricing downgrade receipts recorded while building the totals.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub live_pricing_defects: BTreeSet<String>,
    /// Live rows that failed validation and had no usable bundled fallback.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub live_pricing_unusable_defects: BTreeSet<String>,
    /// Redacted per-route receipts: provider, configured identity, wire model,
    /// billing surface, endpoint fingerprint, billing mode, currency. Never a URL, a
    /// credential, or a filesystem path.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub route_receipts: BTreeSet<String>,
    /// Redacted provider-response identities already included in the live and
    /// durable sub-agent totals. Worker records persist the same fingerprints.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub usage_source_fingerprints: BTreeSet<String>,
    /// Written by builds that track coverage, so a reader can tell "this session
    /// genuinely had zero money-metered turns" apart from "this session predates
    /// coverage tracking". Absent on legacy rows.
    #[serde(default)]
    pub coverage_recorded: bool,
}

impl SessionCostSnapshot {
    fn absorb_late_background_cost(&mut self, pool: &crate::cost_status::PendingBackgroundCost) {
        let estimate = crate::pricing::CostEstimate {
            usd: self.subagent_cost_usd,
            cny: self.subagent_cost_cny,
        }
        .saturating_add(pool.estimate);
        self.subagent_cost_usd = estimate.usd;
        self.subagent_cost_cny = estimate.cny;
        self.priced_turns = self.priced_turns.saturating_add(pool.priced_turns);
        self.unpriced_turns = self.unpriced_turns.saturating_add(pool.unpriced_turns);
        self.cny_priced_turns = self.cny_priced_turns.saturating_add(pool.cny_priced_turns);
        self.cny_unpriced_turns = self
            .cny_unpriced_turns
            .saturating_add(pool.cny_unpriced_turns);
        self.unpriced_reasons
            .extend(pool.unpriced_reasons.iter().map(ToString::to_string));
        self.cny_unpriced_reasons
            .extend(pool.cny_unpriced_reasons.iter().map(ToString::to_string));
        self.unpriced_classes
            .extend(pool.unpriced_classes.iter().map(ToString::to_string));
        self.pricing_provenances
            .extend(pool.pricing_provenances.iter().map(ToString::to_string));
        self.live_pricing_defects
            .extend(pool.live_pricing_defects.iter().map(ToString::to_string));
        self.live_pricing_unusable_defects.extend(
            pool.live_pricing_unusable_defects
                .iter()
                .map(ToString::to_string),
        );
        self.route_receipts
            .extend(pool.route_receipts.iter().cloned());
        self.usage_source_fingerprints
            .extend(pool.usage_source_fingerprints.iter().cloned());
        self.coverage_recorded = true;
        let total = self.total_estimate();
        self.displayed_cost_high_water_usd = self.displayed_cost_high_water_usd.max(total.usd);
        self.displayed_cost_high_water_cny = self.displayed_cost_high_water_cny.max(total.cny);
    }

    /// Session + subagent spend as **one** dual-currency accumulator.
    ///
    /// The persisted USD and CNY columns are projections of per-turn
    /// [`crate::pricing::CostEstimate`]s that were accumulated jointly; every
    /// display total is derived from this single fold so the two currencies
    /// cannot be re-summed by separate code paths that then drift (#4939).
    /// CNY is *not* an FX multiple of USD: a turn carries CNY only when its
    /// route published an authoritative CNY row (provider-published
    /// dual-currency pricing, e.g. DeepSeek's CNY table), and a USD-only turn
    /// contributes exactly zero CNY while `cny_unpriced_turns` records the gap.
    #[must_use]
    pub fn total_estimate(&self) -> crate::pricing::CostEstimate {
        crate::pricing::CostEstimate {
            usd: self.session_cost_usd,
            cny: self.session_cost_cny,
        }
        .saturating_add(crate::pricing::CostEstimate {
            usd: self.subagent_cost_usd,
            cny: self.subagent_cost_cny,
        })
    }

    /// Session + subagent cost in USD.
    pub fn total_usd(&self) -> f64 {
        self.total_estimate()
            .amount(crate::pricing::CostCurrency::Usd)
    }

    /// Session + subagent cost in CNY.
    pub fn total_cny(&self) -> f64 {
        self.total_estimate()
            .amount(crate::pricing::CostCurrency::Cny)
    }

    /// Whether this snapshot's coverage state must be shown as unknown.
    ///
    /// True when the snapshot has no coverage evidence — the signature of a
    /// session written before coverage was persisted. Reporting any such
    /// session as "0 of 0 priced" would claim completeness without evidence,
    /// including when the saved amount is zero.
    #[must_use]
    pub fn coverage_is_legacy_unknown(&self) -> bool {
        !self.coverage_recorded
    }
}

impl SessionMetadata {
    /// Copy cost fields from another metadata (used when forking a session).
    #[allow(dead_code)]
    pub fn copy_cost_from(&mut self, other: &SessionMetadata) {
        self.cost = other.cost.clone();
    }

    /// Record additive lineage metadata for a forked saved session.
    pub fn mark_forked_from(&mut self, parent: &SessionMetadata) {
        self.parent_session_id = Some(parent.id.clone());
        self.forked_from_message_count = Some(parent.message_count);
    }
}

/// Durable Work-panel state. Optional on [`SavedSession`] so every session
/// written before v0.8.68 remains loadable without migration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SessionWorkState {
    /// Authoritative Work Graph. Optional so pre-Work-Graph sessions and old
    /// binaries continue to exchange fully populated Plan/To-do views.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph: Option<crate::work_graph::WorkGraphSnapshot>,
    #[serde(default, skip_serializing_if = "TodoListSnapshot::is_empty")]
    pub todos: TodoListSnapshot,
    #[serde(default, skip_serializing_if = "PlanSnapshot::is_empty")]
    pub plan: PlanSnapshot,
}

/// Bounded goal projection persisted beside the owning saved session.
///
/// This intentionally excludes completion prose, verifier output, transcripts,
/// and filesystem evidence. The saved session already owns conversation
/// history; restart only needs the typed control state that makes the next turn
/// continue the same objective without trusting text reconstructed from it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SessionGoalState {
    #[serde(default = "current_session_goal_schema_version")]
    pub schema_version: u32,
    pub objective: String,
    pub status: SessionGoalStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_budget: Option<u32>,
    #[serde(default)]
    pub tokens_used: u64,
    #[serde(default)]
    pub time_used_seconds: u64,
    #[serde(default)]
    pub continuation_count: u32,
    #[serde(default)]
    pub elapsed_seconds: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pause_reason: Option<GoalPauseReason>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_gap_fingerprint: Option<String>,
    #[serde(default)]
    pub repeated_gap_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_gap_pass: Option<u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionGoalStatus {
    Active,
    Paused,
    Complete,
    Blocked,
}

const fn current_session_goal_schema_version() -> u32 {
    CURRENT_SESSION_GOAL_SCHEMA_VERSION
}

impl SessionGoalState {
    /// Convert a runtime update into the durable, bounded session contract.
    /// The canonical empty runtime snapshot removes the sidecar.
    pub fn from_runtime(snapshot: &GoalSnapshot) -> io::Result<Option<Self>> {
        if snapshot.objective.is_none() && snapshot.status.trim() == "none" {
            return Ok(None);
        }
        let objective = snapshot
            .objective
            .as_deref()
            .map(str::trim)
            .filter(|objective| !objective.is_empty())
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "goal snapshot has no objective")
            })?;
        let status = match snapshot.status.trim() {
            "active" => SessionGoalStatus::Active,
            "paused" => SessionGoalStatus::Paused,
            "complete" => SessionGoalStatus::Complete,
            "blocked" => SessionGoalStatus::Blocked,
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("goal snapshot has unsupported status '{other}'"),
                ));
            }
        };
        let state = Self {
            schema_version: CURRENT_SESSION_GOAL_SCHEMA_VERSION,
            objective: objective.to_string(),
            status,
            token_budget: snapshot.token_budget,
            tokens_used: snapshot.tokens_used,
            time_used_seconds: snapshot.time_used_seconds,
            continuation_count: snapshot.continuation_count,
            elapsed_seconds: snapshot.elapsed_seconds.unwrap_or_default(),
            pause_reason: snapshot.pause_reason,
            goal_id: snapshot.goal_id.clone(),
            last_gap_fingerprint: snapshot.last_gap_fingerprint.clone(),
            repeated_gap_count: snapshot.repeated_gap_count,
            last_gap_pass: snapshot.last_gap_pass,
        };
        state.validate()?;
        Ok(Some(state))
    }

    pub fn validate(&self) -> io::Result<()> {
        if self.schema_version > CURRENT_SESSION_GOAL_SCHEMA_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Session goal schema v{} is newer than supported v{}",
                    self.schema_version, CURRENT_SESSION_GOAL_SCHEMA_VERSION
                ),
            ));
        }
        let objective = self.objective.trim();
        if objective.is_empty() || objective.chars().count() > MAX_SESSION_GOAL_OBJECTIVE_CHARS {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Session goal objective must contain 1..={MAX_SESSION_GOAL_OBJECTIVE_CHARS} characters"
                ),
            ));
        }
        if self
            .goal_id
            .as_ref()
            .is_some_and(|id| id.is_empty() || id.len() > 128)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid session goal revision",
            ));
        }
        codewhale_protocol::validate_goal_stall_state(
            self.last_gap_fingerprint.as_deref(),
            self.repeated_gap_count,
            self.last_gap_pass,
            self.continuation_count,
        )
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }

    #[must_use]
    pub fn to_runtime_snapshot(&self) -> GoalSnapshot {
        GoalSnapshot {
            goal_id: self.goal_id.clone(),
            objective: Some(self.objective.clone()),
            status: match self.status {
                SessionGoalStatus::Active => "active",
                SessionGoalStatus::Paused => "paused",
                SessionGoalStatus::Complete => "complete",
                SessionGoalStatus::Blocked => "blocked",
            }
            .to_string(),
            token_budget: self.token_budget,
            tokens_used: self.tokens_used,
            time_used_seconds: self.time_used_seconds,
            continuation_count: self.continuation_count,
            elapsed_seconds: Some(self.elapsed_seconds),
            evidence: None,
            blocker: None,
            pause_reason: self.pause_reason,
            completion_verification: None,
            advisories: Vec::new(),
            last_gap_fingerprint: self.last_gap_fingerprint.clone(),
            repeated_gap_count: self.repeated_gap_count,
            last_gap_pass: self.last_gap_pass,
            progress: None,
        }
    }
}

impl SessionWorkState {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.graph
            .as_ref()
            .is_none_or(crate::work_graph::WorkGraphSnapshot::is_empty)
            && self.todos.is_empty()
            && self.plan.is_empty()
    }
}

/// Latest concrete Auto route and the decision receipt that produced it.
///
/// This is additive, optional session metadata: sessions written before
/// v0.9.1 deserialize with no receipt and keep their legacy restore behavior.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SavedAutoRouteReceipt {
    pub(crate) provider: ApiProvider,
    pub(crate) provider_identity: String,
    pub(crate) model: String,
    pub(crate) receipt: AutoRouteReceipt,
    /// Canonical effective reasoning receipt for the selected route, including
    /// routes where a concrete tier cannot be proven. Optional so older
    /// sessions remain loadable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) effective_reasoning_effort: Option<ReasoningEffortTier>,
}

/// A saved session containing full conversation history
/// Starting with v0.9.5 (#5262) the canonical history is the append-only entry journal (`journal` / `leaf_id`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedSession {
    /// Schema version for migration compatibility
    #[serde(default = "default_session_schema_version")]
    pub schema_version: u32,
    /// Session metadata
    pub metadata: SessionMetadata,
    /// Conversation messages — derived from the journal's active branch (kept for compat).
    pub messages: Vec<Message>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub journal: Option<SessionJournal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leaf_id: Option<String>,
    /// System prompt if any
    pub system_prompt: Option<String>,
    /// Compact linked context references for user-visible `@path` and
    /// `/attach` mentions. Optional for backward-compatible session loads.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub context_references: Vec<SessionContextReference>,
    /// Metadata registry of large outputs produced during this session.
    /// Artifact contents are stored in the session-owned artifact directory.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<ArtifactRecord>,
    /// Session-owned approval evidence. The append-only sidecar is canonical
    /// during a live turn; this projection makes saved snapshots self-
    /// describing without putting receipts in the model transcript.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) approval_receipts: Vec<ApprovalReceipt>,
    /// To-do and plan state shown in the Work sidebar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_state: Option<SessionWorkState>,
    /// User-configured tab/window title for this session (`/title`), shown as
    /// `[title] …` in front of the terminal window title. Optional for
    /// backward-compatible session loads; absent sessions use the `title`
    /// config default instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_title: Option<String>,
    /// Most recent accepted/completed Auto decision, when the saved model mode
    /// is `auto`. Optional for backward-compatible session loads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) last_auto_route: Option<SavedAutoRouteReceipt>,
}
impl SavedSession {
    /// Drop the journal-derived compatibility projection before an async
    /// persistence request takes ownership. Disk serialization restores it.
    pub(crate) fn compact_for_persistence_queue(&mut self) {
        if self.journal.is_some() {
            self.messages = Vec::new();
        }
    }

    fn storage_compatible_copy(&self) -> Option<Self> {
        let journal = self.journal.as_ref()?;
        let active_messages = journal.to_messages();
        if !self.messages.is_empty() && self.messages == active_messages {
            return None;
        }
        let mut copy = self.clone();
        if copy.messages.is_empty() {
            copy.messages = active_messages;
        } else if let Some(journal) = copy.journal.as_mut() {
            journal.rebranch_active_messages(&copy.messages);
            copy.leaf_id = journal.leaf_id.clone();
        }
        copy.metadata.message_count = copy.messages.len();
        Some(copy)
    }

    pub fn ensure_journal(&mut self) {
        if self.journal.is_some() {
            if self.leaf_id.is_none() {
                self.leaf_id = self.journal.as_ref().and_then(|j| j.leaf_id.clone());
            }
            let active = self
                .journal
                .as_ref()
                .map(|j| j.to_messages())
                .unwrap_or_default();
            if !active.is_empty() {
                self.messages = active;
                self.metadata.message_count = self.messages.len();
            }
            return;
        }
        let journal =
            SessionJournal::from_messages(self.messages.clone(), self.metadata.spawn_depth);
        self.leaf_id = journal.leaf_id.clone();
        self.journal = Some(journal);
    }
    pub fn journal_append_message(&mut self, message: Message) -> String {
        self.ensure_journal();
        let journal = self.journal.as_mut().expect("journal ensured");
        let id = journal.append_message(message.clone());
        self.leaf_id = journal.leaf_id.clone();
        self.messages = journal.to_messages();
        self.metadata.message_count = self.messages.len();
        self.metadata.updated_at = Utc::now();
        id
    }
    pub fn journal_branch_to(&mut self, entry_id: &str) -> Result<(), String> {
        self.ensure_journal();
        let journal = self.journal.as_mut().expect("journal ensured");
        journal.branch_to(entry_id)?;
        self.leaf_id = journal.leaf_id.clone();
        self.messages = journal.to_messages();
        self.metadata.updated_at = Utc::now();
        Ok(())
    }
    pub fn active_entries(&self) -> Vec<SessionEntry> {
        self.journal
            .as_ref()
            .map(|j| j.root_to_leaf().into_iter().cloned().collect())
            .unwrap_or_default()
    }
    /// `created_at` of the active branch's message entries, in order — the
    /// stamps a resumed session hands back to the live message log so the
    /// next save preserves append times instead of rewriting them to resume
    /// time.
    pub fn journal_message_stamps(&self) -> Vec<DateTime<Utc>> {
        self.journal
            .as_ref()
            .map(|journal| {
                journal
                    .root_to_leaf()
                    .iter()
                    .filter(|entry| {
                        matches!(
                            entry.kind,
                            crate::session_tree::SessionEntryKind::Message { .. }
                        )
                    })
                    .map(|entry| entry.created_at)
                    .collect()
            })
            .unwrap_or_default()
    }
    pub fn export_container(&self, source: &str) -> SessionImportContainer {
        let journal = self.journal.clone().unwrap_or_else(|| {
            SessionJournal::from_messages(self.messages.clone(), self.metadata.spawn_depth)
        });
        SessionImportContainer::new(
            source.to_string(),
            &journal,
            serde_json::to_value(&self.metadata).ok(),
        )
    }
    pub fn import_foreign(
        container: SessionImportContainer,
        workspace: PathBuf,
        model: String,
    ) -> Result<Self, String> {
        let journal = container.into_journal()?;
        let leaf_id = journal.leaf_id.clone();
        let messages = journal.to_messages();
        let now = Utc::now();
        let spawn_depth = journal.spawn_depth.saturating_add(1);
        // Reuse the conversation-derived title so an imported session that
        // opens with runtime-owned control traffic (Operate contract, restore
        // checkpoint) is named after the real prompt, not the envelope.
        let title = conversation_derived_title(&messages)
            .unwrap_or_else(|| crate::session_manager::DEFAULT_SESSION_TITLE.to_string());
        let metadata = SessionMetadata {
            id: Uuid::new_v4().to_string(),
            title,
            created_at: now,
            updated_at: now,
            message_count: messages.len(),
            total_tokens: 0,
            model,
            model_provider: default_model_provider(),
            model_provider_id: None,
            workspace,
            mode: None,
            cost: SessionCostSnapshot::default(),
            parent_session_id: None,
            forked_from_message_count: None,
            runtime_store: None,
            cumulative_turn_secs: 0,
            archived: false,
            spawn_depth,
        };
        let mut journal = journal;
        journal.spawn_depth = spawn_depth;
        Ok(Self {
            schema_version: CURRENT_SESSION_SCHEMA_VERSION,
            metadata,
            messages,
            journal: Some(journal),
            leaf_id,
            system_prompt: None,
            context_references: Vec::new(),
            artifacts: Vec::new(),
            approval_receipts: Vec::new(),
            work_state: None,
            window_title: None,
            last_auto_route: None,
        })
    }
}

fn serialize_saved_session(session: &SavedSession) -> io::Result<String> {
    let compatible = session.storage_compatible_copy();
    serde_json::to_string_pretty(compatible.as_ref().unwrap_or(session))
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

/// Manager for session persistence operations
#[derive(Debug)]
pub struct SessionManager {
    /// Directory where sessions are stored
    sessions_dir: PathBuf,
}

/// One interactive editor owns a session's unsent text until its last queued
/// write finishes. The stable lock file is never unlinked: replacing it would
/// let two processes lock different files for the same session.
#[derive(Debug)]
pub struct OfflineQueueLease {
    session_id: String,
    _file: fs::File,
}

impl OfflineQueueLease {
    pub fn session_id(&self) -> &str {
        &self.session_id
    }
}

impl Drop for OfflineQueueLease {
    fn drop(&mut self) {
        // A forked child can briefly retain the same open-file description.
        // Release the editor's lock now, rather than waiting for every inherited
        // descriptor to close, as RuntimeProcessOwnerLock does on shutdown.
        #[cfg(all(unix, not(target_os = "solaris")))]
        {
            use std::os::fd::AsRawFd as _;
            // SAFETY: the lease still owns this descriptor throughout Drop.
            unsafe {
                libc::flock(self._file.as_raw_fd(), libc::LOCK_UN);
            }
        }
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle as _;
            use windows_sys::Win32::Storage::FileSystem::UnlockFile;
            // SAFETY: the lease owns the handle; fd-lock locks byte 0 only.
            unsafe {
                UnlockFile(self._file.as_raw_handle() as _, 0, 0, 1, 0);
            }
        }
        // fd-lock uses process-associated fcntl locks on Solaris. They are not
        // inherited by fork and closing this descriptor releases the lock.
    }
}

/// Origin of a crash-recovery checkpoint file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckpointSource {
    /// Per-session checkpoint file `checkpoints/<session_id>.json`.
    Session(String),
    /// Legacy single-slot checkpoint file `checkpoints/latest.json`.
    Legacy,
}

/// A crash-recovery checkpoint file discovered on disk (metadata only —
/// callers load the session content separately).
#[derive(Debug, Clone)]
pub struct CheckpointRef {
    pub source: CheckpointSource,
    pub path: PathBuf,
    pub modified: std::time::SystemTime,
}

/// File names in `checkpoints/` that are never per-session checkpoints.
const LEGACY_CHECKPOINT_FILE: &str = "latest.json";
/// Pre-per-session global offline queue, still read once for migration.
const OFFLINE_QUEUE_FILE: &str = "offline_queue.json";
/// Per-session offline queue file: `checkpoints/<session_id>.offline_queue.json`.
const OFFLINE_QUEUE_SUFFIX: &str = ".offline_queue.json";

pub(crate) fn is_offline_queue_file(name: &str) -> bool {
    name == OFFLINE_QUEUE_FILE || name.ends_with(OFFLINE_QUEUE_SUFFIX)
}

impl SessionManager {
    fn approval_receipt_store(&self) -> ApprovalReceiptStore {
        ApprovalReceiptStore::new(self.sessions_dir.clone())
    }

    fn hydrate_approval_receipts(&self, session: &mut SavedSession) -> io::Result<()> {
        let durable = self.approval_receipt_store().load(&session.metadata.id)?;
        if !durable.is_empty() {
            session.approval_receipts = durable;
        }
        ApprovalReplay::from_receipts(&session.approval_receipts)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        Ok(())
    }

    /// Reconstruct completed approvals and interrupted unmatched asks for one
    /// session without consulting the model transcript.
    pub(crate) fn replay_approvals(&self, session_id: &str) -> io::Result<ApprovalReplay> {
        self.approval_receipt_store().replay(session_id)
    }

    fn validated_session_id<'a>(&self, id: &'a str) -> std::io::Result<&'a str> {
        let trimmed = id.trim();
        if trimmed.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Session id cannot be empty",
            ));
        }
        if !trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Invalid session id '{id}'"),
            ));
        }
        if trimmed == SESSION_BOOT_OWNERS_STEM {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Session id '{trimmed}' collides with a reserved sessions file"),
            ));
        }
        Ok(trimmed)
    }

    fn validated_session_path(&self, id: &str) -> std::io::Result<PathBuf> {
        let trimmed = self.validated_session_id(id)?;
        Ok(self.sessions_dir.join(format!("{trimmed}.json")))
    }

    fn checkpoints_dir(&self) -> PathBuf {
        self.sessions_dir.join("checkpoints")
    }

    fn session_goals_dir(&self) -> PathBuf {
        self.sessions_dir.join(SESSION_GOALS_DIR)
    }

    fn checked_existing_session_goals_dir(&self) -> std::io::Result<Option<PathBuf>> {
        let dir = self.session_goals_dir();
        let metadata = match fs::symlink_metadata(&dir) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Session goal store {} must be a real directory",
                    dir.display()
                ),
            ));
        }
        Ok(Some(dir))
    }

    fn ensure_session_goals_dir(&self) -> std::io::Result<PathBuf> {
        if let Some(dir) = self.checked_existing_session_goals_dir()? {
            return Ok(dir);
        }
        let dir = self.session_goals_dir();
        match fs::create_dir(&dir) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        self.checked_existing_session_goals_dir()?.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("Session goal store {} was not created", dir.display()),
            )
        })
    }

    fn validated_session_goal_path(&self, session_id: &str) -> std::io::Result<PathBuf> {
        let id = self.validated_session_id(session_id)?;
        Ok(self.session_goals_dir().join(format!("{id}.json")))
    }

    fn checked_existing_session_goal_file(path: &Path) -> std::io::Result<bool> {
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error),
        };
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Session goal {} must be a regular file", path.display()),
            ));
        }
        Ok(true)
    }

    fn validated_checkpoint_path(&self, session_id: &str) -> std::io::Result<PathBuf> {
        let trimmed = self.validated_session_id(session_id)?;
        // Reserved file names inside `checkpoints/` must never collide with a
        // per-session checkpoint file.
        if format!("{trimmed}.json") == LEGACY_CHECKPOINT_FILE
            || format!("{trimmed}.json") == OFFLINE_QUEUE_FILE
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Session id '{trimmed}' collides with a reserved checkpoint file"),
            ));
        }
        Ok(self.checkpoints_dir().join(format!("{trimmed}.json")))
    }

    /// Create a new `SessionManager` with the specified sessions directory
    pub fn new(sessions_dir: PathBuf) -> std::io::Result<Self> {
        let sessions_dir = normalize_managed_dir(sessions_dir)?;
        // Ensure the sessions directory exists
        fs::create_dir_all(&sessions_dir)?;
        Ok(Self { sessions_dir })
    }

    /// Create a `SessionManager` using the default location.
    pub fn default_location() -> std::io::Result<Self> {
        Self::new(default_sessions_dir()?)
    }

    /// Return the resolved sessions directory path.
    pub fn sessions_dir(&self) -> &Path {
        &self.sessions_dir
    }

    fn late_usage_paths(&self, session_id: &str) -> io::Result<(PathBuf, PathBuf)> {
        let session_id = self.validated_session_id(session_id)?;
        let dir = self.sessions_dir.join(LATE_USAGE_DIR);
        match fs::symlink_metadata(&dir) {
            Ok(metadata) => {
                #[cfg(windows)]
                let linked = {
                    use std::os::windows::fs::MetadataExt as _;
                    metadata.file_attributes()
                        & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT
                        != 0
                };
                #[cfg(not(windows))]
                let linked = metadata.file_type().is_symlink();
                if linked || !metadata.is_dir() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "late usage store must be a real directory",
                    ));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        Ok((
            dir.join(format!("{session_id}.json")),
            dir.join(format!("{session_id}.lock")),
        ))
    }

    /// Only mutations create accounting storage. Snapshot/list reads must work
    /// for a healthy transcript even when no sidecar has ever been written.
    fn ensure_late_usage_paths(&self, session_id: &str) -> io::Result<(PathBuf, PathBuf)> {
        self.late_usage_paths(session_id)?;
        let dir = self.sessions_dir.join(LATE_USAGE_DIR);
        match fs::create_dir(&dir) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        let paths = self.late_usage_paths(session_id)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
            OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(&dir)?
                .set_permissions(fs::Permissions::from_mode(0o700))?;
        }
        Ok(paths)
    }

    /// A deletion marker and its stable lock survive deletion, without any
    /// route or usage data. A captured callback must never recreate the ledger.
    fn late_usage_is_deleted(path: &Path) -> io::Result<bool> {
        use std::io::Read as _;
        let tombstone = match open_private_read_file(&path.with_extension("deleted")) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error),
        };
        let mut marker = Vec::with_capacity(LATE_USAGE_DELETED.len());
        tombstone
            .take(u64::try_from(LATE_USAGE_DELETED.len()).unwrap_or(u64::MAX) + 1)
            .read_to_end(&mut marker)?;
        if marker != LATE_USAGE_DELETED {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid late usage deletion marker",
            ));
        }
        Ok(true)
    }

    fn write_late_usage_ledger(path: &Path, ledger: &LateUsageLedger) -> io::Result<()> {
        let bytes = serde_json::to_vec(ledger)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_LATE_USAGE_LEDGER_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "late usage ledger exceeds its size bound",
            ));
        }
        write_atomic(path, &bytes)
    }

    fn load_late_usage_unlocked(path: &Path) -> io::Result<LateUsageLedger> {
        let file = match open_private_read_file(path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(LateUsageLedger::default());
            }
            Err(error) => return Err(error),
        };
        let metadata = file.metadata()?;
        if metadata.len() > MAX_LATE_USAGE_LEDGER_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "late usage ledger {} exceeds its size bound",
                    path.display()
                ),
            ));
        }
        use std::io::Read as _;
        let mut raw = Vec::with_capacity(
            usize::try_from(metadata.len().min(MAX_LATE_USAGE_LEDGER_BYTES)).unwrap_or(0),
        );
        file.take(MAX_LATE_USAGE_LEDGER_BYTES.saturating_add(1))
            .read_to_end(&mut raw)?;
        if u64::try_from(raw.len()).unwrap_or(u64::MAX) > MAX_LATE_USAGE_LEDGER_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "late usage ledger {} exceeds its size bound",
                    path.display()
                ),
            ));
        }
        let ledger: LateUsageLedger = serde_json::from_slice(&raw)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        if ledger.schema_version != CURRENT_LATE_USAGE_SCHEMA_VERSION
            || ledger.records.len() > MAX_LATE_USAGE_RECORDS_PER_SESSION
            || ledger.records.iter().any(|record| {
                !is_sha256_fingerprint(&record.source_fingerprint)
                    || !is_sha256_fingerprint(&record.turn_fingerprint)
            })
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "late usage ledger has an unsupported or unbounded shape",
            ));
        }
        Ok(ledger)
    }

    fn with_session_write_admission<T>(
        &self,
        session_id: &str,
        write: impl FnOnce() -> io::Result<T>,
    ) -> io::Result<Option<T>> {
        let (path, lock_path) = self.ensure_late_usage_paths(session_id)?;
        let lock_file = open_private_lock_file(&lock_path)?;
        let mut lock = fd_lock::RwLock::new(lock_file);
        let _guard = lock.write()?;
        if Self::late_usage_is_deleted(&path)? {
            return Ok(None);
        }
        write().map(Some)
    }

    /// Serialize active accounting admission with deletion of its origin.
    /// A retired origin is handled without running the callback. Callers must
    /// release this boundary before attempting a late-usage append, which
    /// independently checks retirement under the same stable lock.
    pub(crate) fn with_live_session_origin(
        &self,
        session_id: &str,
        accept: impl FnOnce() -> bool,
    ) -> io::Result<Option<bool>> {
        self.with_session_write_admission(session_id, || Ok(accept()))
    }

    fn retired_session_write_error() -> io::Error {
        io::Error::new(io::ErrorKind::NotFound, "session was deleted")
    }

    fn persist_late_usage_record(
        &self,
        session_id: &str,
        turn_id: &str,
        source_id: &str,
        route: &crate::cost_status::EffectiveRouteEnvelope,
        usage: Option<&codewhale_models::Usage>,
    ) -> io::Result<bool> {
        let (path, lock_path) = self.ensure_late_usage_paths(session_id)?;
        let lock_file = open_private_lock_file(&lock_path)?;
        let mut lock = fd_lock::RwLock::new(lock_file);
        let _guard = lock.write()?;
        if Self::late_usage_is_deleted(&path)? {
            // Handled, rather than a failed sink that should queue a retry.
            return Ok(true);
        }
        let mut ledger = Self::load_late_usage_unlocked(&path)?;
        let source_fingerprint = crate::cost_status::usage_source_fingerprint(source_id);
        if ledger
            .records
            .iter()
            .any(|record| record.source_fingerprint == source_fingerprint)
        {
            return Ok(true);
        }
        if ledger.records.len() == MAX_LATE_USAGE_RECORDS_PER_SESSION {
            if !ledger.overflowed {
                ledger.overflowed = true;
                Self::write_late_usage_ledger(&path, &ledger)?;
            }
            return Ok(true);
        }
        ledger.records.push(LateUsageRecord {
            source_fingerprint,
            turn_fingerprint: crate::cost_status::usage_source_fingerprint(turn_id),
            route: route.sanitized_for_persistence(),
            usage: usage.cloned(),
        });
        Self::write_late_usage_ledger(&path, &ledger)?;
        Ok(true)
    }

    pub(crate) fn persist_late_runtime_usage(
        &self,
        session_id: &str,
        turn_id: &str,
        record: &crate::cost_status::RuntimeUsageRecord,
    ) -> io::Result<bool> {
        self.persist_late_usage_record(
            session_id,
            turn_id,
            &record.source_id,
            &record.usage.route,
            Some(&record.usage.usage),
        )
    }

    pub(crate) fn persist_late_runtime_drop(
        &self,
        session_id: &str,
        turn_id: &str,
        record: &crate::cost_status::RuntimeUsageDropRecord,
    ) -> io::Result<bool> {
        self.persist_late_usage_record(session_id, turn_id, &record.source_id, &record.route, None)
    }

    fn with_session_read_lock<T>(
        &self,
        session_id: &str,
        read: impl FnOnce(&Path) -> io::Result<T>,
    ) -> io::Result<T> {
        let (path, lock_path) = self.late_usage_paths(session_id)?;
        let lock_file = match open_private_read_file(&lock_path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                // Atomic replacement makes a copied ledger readable without
                // creating a lock. No writer can have published a tombstone
                // without first creating the stable lock.
                return read(&path);
            }
            Err(error) => return Err(error),
        };
        let lock = fd_lock::RwLock::new(lock_file);
        let _guard = lock.read()?;
        read(&path)
    }

    fn load_late_usage(&self, session_id: &str) -> io::Result<LateUsageLedger> {
        self.with_session_read_lock(session_id, |path| {
            if Self::late_usage_is_deleted(path)? {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "session accounting was deleted",
                ));
            }
            Self::load_late_usage_unlocked(path)
        })
    }

    fn apply_late_usage_to_metadata(&self, metadata: &mut SessionMetadata) {
        let ledger = match self.load_late_usage(&metadata.id) {
            Ok(ledger) => ledger,
            Err(_) => {
                // The transcript is independent of optional accounting data.
                // Keep a stable gap receipt even when this projection is later
                // saved; loading it again must not invent another missing call.
                let fingerprint = crate::cost_status::usage_source_fingerprint(&format!(
                    "late-usage-unavailable:{}",
                    crate::cost_status::usage_source_fingerprint(&metadata.id)
                ));
                if metadata.cost.usage_source_fingerprints.insert(fingerprint) {
                    metadata.cost.unpriced_turns = metadata.cost.unpriced_turns.saturating_add(1);
                    metadata.cost.cny_unpriced_turns =
                        metadata.cost.cny_unpriced_turns.saturating_add(1);
                }
                metadata
                    .cost
                    .unpriced_reasons
                    .insert(LATE_USAGE_UNAVAILABLE_REASON.to_string());
                metadata
                    .cost
                    .cny_unpriced_reasons
                    .insert(LATE_USAGE_UNAVAILABLE_REASON.to_string());
                metadata.cost.coverage_recorded = true;
                return;
            }
        };
        for record in ledger.records {
            let source_fingerprint = record.source_fingerprint.clone();
            let source_id = format!("late:{}", record.source_fingerprint);
            let mut pending = if let Some(usage) = record.usage.as_ref() {
                crate::cost_status::background_cost_for_runtime_usage(
                    &crate::cost_status::RuntimeUsageRecord {
                        source_id,
                        usage: crate::cost_status::EffectiveRouteUsage {
                            route: record.route,
                            usage: usage.clone(),
                        },
                    },
                )
            } else {
                crate::cost_status::background_cost_for_runtime_drop(
                    &crate::cost_status::RuntimeUsageDropRecord {
                        source_id,
                        route: record.route,
                    },
                )
            };
            // The sidecar already stores the canonical SHA-256 identity. Do
            // not hash it again while projecting the receipt into the saved
            // session, or a concurrent main-snapshot writer that already
            // contains the response would not dedupe against this overlay.
            pending.usage_source_fingerprints.clear();
            pending
                .usage_source_fingerprints
                .insert(source_fingerprint.clone());
            if metadata
                .cost
                .usage_source_fingerprints
                .contains(&source_fingerprint)
            {
                continue;
            }
            if let Some(usage) = record.usage {
                metadata.total_tokens = metadata
                    .total_tokens
                    .saturating_add(u64::from(usage.input_tokens))
                    .saturating_add(u64::from(usage.output_tokens));
            }
            metadata.cost.absorb_late_background_cost(&pending);
        }
        if ledger.overflowed {
            let fingerprint = crate::cost_status::usage_source_fingerprint(&format!(
                "late-usage-overflow:{}",
                crate::cost_status::usage_source_fingerprint(&metadata.id)
            ));
            if metadata.cost.usage_source_fingerprints.insert(fingerprint) {
                metadata.cost.unpriced_turns = metadata.cost.unpriced_turns.saturating_add(1);
                metadata.cost.cny_unpriced_turns =
                    metadata.cost.cny_unpriced_turns.saturating_add(1);
                metadata
                    .cost
                    .unpriced_reasons
                    .insert("late_usage_ledger_overflow".to_string());
                metadata
                    .cost
                    .cny_unpriced_reasons
                    .insert("late_usage_ledger_overflow".to_string());
                metadata.cost.coverage_recorded = true;
            }
        }
    }

    /// Persist the bounded goal control state for one saved session.
    /// `None` is the canonical clear operation and is idempotent.
    pub fn save_session_goal(
        &self,
        session_id: &str,
        goal: Option<&SessionGoalState>,
    ) -> std::io::Result<()> {
        let path = self.validated_session_goal_path(session_id)?;
        let Some(goal) = goal else {
            if self.checked_existing_session_goals_dir()?.is_some() && path.exists() {
                fs::remove_file(path)?;
            }
            return Ok(());
        };
        goal.validate()?;
        self.ensure_session_goals_dir()?;
        Self::checked_existing_session_goal_file(&path)?;
        let content = serde_json::to_string_pretty(goal)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        write_atomic(&path, content.as_bytes())
    }

    /// Load a saved session's durable goal, rejecting malformed or future
    /// records instead of silently starting a different objective.
    pub fn load_session_goal(&self, session_id: &str) -> std::io::Result<Option<SessionGoalState>> {
        let path = self.validated_session_goal_path(session_id)?;
        if self.checked_existing_session_goals_dir()?.is_none()
            || !Self::checked_existing_session_goal_file(&path)?
        {
            return Ok(None);
        }
        let file_len = fs::metadata(&path)?.len();
        if file_len > MAX_SESSION_GOAL_FILE_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Session goal {} is {file_len} bytes; maximum is {MAX_SESSION_GOAL_FILE_BYTES}",
                    path.display()
                ),
            ));
        }
        let raw = fs::read_to_string(path)?;
        let goal: SessionGoalState = serde_json::from_str(&raw)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        goal.validate()?;
        Ok(Some(goal))
    }

    fn hydrate_recovered_runtime_binding(&self, session: &mut SavedSession) -> std::io::Result<()> {
        // Compare under the session write lock. A stale process may neither
        // resurrect a missing binding nor replace a different recovered owner.
        if let Some(incoming) = session.metadata.runtime_store.as_ref()
            && let Ok(persisted) =
                Self::load_session_metadata(&self.validated_session_path(&session.metadata.id)?)
            && let Some(binding) = persisted.runtime_store
            && incoming != &binding
        {
            if incoming.is_missing_session_store().unwrap_or(false)
                && binding.validate_existing_store().is_ok()
            {
                session.metadata.runtime_store = Some(binding);
            } else if !binding.is_missing_session_store().unwrap_or(false) {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "Session Runtime ownership changed; reopen the session before saving",
                ));
            }
        }
        Ok(())
    }

    /// Save a session to disk using atomic write (temp file + fsync + rename).
    pub fn save_session(&self, session: &SavedSession) -> std::io::Result<PathBuf> {
        let path = self.validated_session_path(&session.metadata.id)?;
        self.with_session_write_admission(&session.metadata.id, || {
            let already_persisted = path.exists()
                || self
                    .validated_checkpoint_path(&session.metadata.id)
                    .is_ok_and(|checkpoint| checkpoint.exists());

            self.archive_before_first_graph_write(session, &path)?;

            let mut durable_session = session.clone();
            self.hydrate_recovered_runtime_binding(&mut durable_session)?;
            self.hydrate_approval_receipts(&mut durable_session)?;
            let content = serialize_saved_session(&durable_session)?;

            // Atomic write via write_atomic (NamedTempFile + fsync + persist)
            write_atomic(&path, content.as_bytes())?;
            self.stamp_session_boot_owner_for_new_record(&session.metadata.id, already_persisted);
            Ok(())
        })?
        .ok_or_else(Self::retired_session_write_error)?;

        // Cleanup may delete sessions, so release this session's lifecycle
        // lock first instead of recursively acquiring it during cleanup.
        self.cleanup_old_sessions()?;

        Ok(path)
    }

    /// Save a crash-recovery checkpoint for in-flight turns.
    ///
    /// Checkpoints are keyed per session (`checkpoints/<session_id>.json`) so
    /// concurrent sessions never overwrite each other's crash-recovery state.
    pub fn save_checkpoint(&self, session: &SavedSession) -> std::io::Result<PathBuf> {
        let path = self.validated_checkpoint_path(&session.metadata.id)?;
        self.with_session_write_admission(&session.metadata.id, || {
            let session_path = self.validated_session_path(&session.metadata.id)?;
            self.archive_before_first_graph_write(session, &session_path)?;
            fs::create_dir_all(self.checkpoints_dir())?;
            let already_persisted = path.exists() || session_path.exists();
            let mut durable_session = session.clone();
            self.hydrate_recovered_runtime_binding(&mut durable_session)?;
            self.hydrate_approval_receipts(&mut durable_session)?;
            let content = serialize_saved_session(&durable_session)?;
            write_atomic(&path, content.as_bytes())?;
            self.stamp_session_boot_owner_for_new_record(&session.metadata.id, already_persisted);
            Ok(())
        })?
        .ok_or_else(Self::retired_session_write_error)?;
        Ok(path)
    }

    fn session_boot_owners_path(&self) -> PathBuf {
        self.sessions_dir
            .join(format!("{SESSION_BOOT_OWNERS_STEM}.json"))
    }

    fn load_session_boot_owners(&self) -> BTreeMap<String, String> {
        fs::read_to_string(self.session_boot_owners_path())
            .ok()
            .and_then(|content| serde_json::from_str(&content).ok())
            .unwrap_or_default()
    }

    /// Does any durable record (session file or crash checkpoint) exist for
    /// this session id?
    fn session_record_exists(&self, session_id: &str) -> bool {
        self.validated_session_path(session_id)
            .is_ok_and(|path| path.exists())
            || self
                .validated_checkpoint_path(session_id)
                .is_ok_and(|path| path.exists())
    }

    /// Record which session instance owns `session_id`'s persisted record.
    ///
    /// Entries whose durable record no longer exists are pruned on the same
    /// write, so the sidecar cannot grow without bound.
    pub(crate) fn record_session_boot_owner(
        &self,
        session_id: &str,
        boot_id: &str,
    ) -> std::io::Result<()> {
        let id = self.validated_session_id(session_id)?.to_string();
        let mut owners = self.load_session_boot_owners();
        owners.retain(|owned, _| owned == &id || self.session_record_exists(owned));
        owners.insert(id, boot_id.to_string());
        let content = serde_json::to_string_pretty(&owners)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        write_atomic(&self.session_boot_owners_path(), content.as_bytes())
    }

    /// The session-instance boot id stamped on this session's persisted
    /// record, when one was recorded.
    #[must_use]
    pub fn session_boot_owner(&self, session_id: &str) -> Option<String> {
        let id = self.validated_session_id(session_id).ok()?;
        self.load_session_boot_owners().get(id).cloned()
    }

    /// Was this session's persisted record created by a different session
    /// instance (an earlier or sibling Codewhale process)?
    ///
    /// Mirrors `SubAgentManager::is_from_prior_session` (#405): a durable
    /// record with no stamped owner predates the marker and is classified as
    /// prior-instance work, while an id with no durable record at all is
    /// this instance's own not-yet-persisted session.
    #[must_use]
    pub fn session_from_prior_instance(&self, session_id: &str) -> bool {
        match self.session_boot_owner(session_id) {
            Some(owner) => owner != current_session_boot_id(),
            None => self.session_record_exists(session_id),
        }
    }

    /// Stamp this instance as creator when a save writes the first durable
    /// record for `session_id`. A record that already existed keeps its
    /// original owner: re-serializing another instance's work (crash
    /// recovery, external mutation) must not re-badge it as ours.
    fn stamp_session_boot_owner_for_new_record(&self, session_id: &str, already_persisted: bool) {
        if already_persisted || self.session_boot_owner(session_id).is_some() {
            return;
        }
        if let Err(error) = self.record_session_boot_owner(session_id, current_session_boot_id()) {
            tracing::warn!(session_id, %error, "could not stamp session boot owner");
        }
    }

    fn clear_session_boot_owner(&self, session_id: &str) {
        let Ok(id) = self.validated_session_id(session_id) else {
            return;
        };
        let mut owners = self.load_session_boot_owners();
        if owners.remove(id).is_none() {
            return;
        }
        if let Ok(content) = serde_json::to_string_pretty(&owners) {
            let _ = write_atomic(&self.session_boot_owners_path(), content.as_bytes());
        }
    }

    /// Preserve the exact pre-import session once, before the first graph-
    /// bearing session or checkpoint write can replace it.
    fn archive_before_first_graph_write(
        &self,
        session: &SavedSession,
        source: &Path,
    ) -> std::io::Result<()> {
        let writes_graph = session
            .work_state
            .as_ref()
            .and_then(|state| state.graph.as_ref())
            .is_some_and(|graph| !graph.is_empty());
        if !writes_graph || !source.exists() {
            return Ok(());
        }
        let bytes = fs::read(source)?;
        let already_graph_backed = serde_json::from_slice::<SavedSession>(&bytes)
            .ok()
            .and_then(|saved| saved.work_state)
            .and_then(|state| state.graph)
            .is_some_and(|graph| !graph.is_empty());
        if already_graph_backed {
            return Ok(());
        }
        let archive_dir = self.sessions_dir.join(WORK_GRAPH_IMPORT_ARCHIVE_DIR);
        fs::create_dir_all(&archive_dir)?;
        let archive =
            archive_dir.join(source.file_name().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "invalid session path")
            })?);
        if !archive.exists() {
            write_atomic(&archive, &bytes)?;
        }
        Ok(())
    }

    fn read_checkpoint_file(&self, path: &Path) -> std::io::Result<Option<SavedSession>> {
        if !path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(path)?;
        let mut session: SavedSession = serde_json::from_str(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        if session.schema_version > CURRENT_SESSION_SCHEMA_VERSION {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "Checkpoint schema v{} is newer than supported v{}",
                    session.schema_version, CURRENT_SESSION_SCHEMA_VERSION
                ),
            ));
        }
        // A crash after retirement but before checkpoint removal must not
        // offer the deleted origin for recovery. Optional accounting damage
        // still permits recovery and is projected as incomplete below.
        if self
            .with_session_read_lock(&session.metadata.id, Self::late_usage_is_deleted)
            .unwrap_or(false)
        {
            return Ok(None);
        }
        session.system_prompt = strip_legacy_truncation_note(session.system_prompt);
        self.hydrate_approval_receipts(&mut session)?;
        self.apply_late_usage_to_metadata(&mut session.metadata);
        Ok(Some(session))
    }

    /// Load a specific session's crash-recovery checkpoint if present.
    pub fn load_session_checkpoint(
        &self,
        session_id: &str,
    ) -> std::io::Result<Option<SavedSession>> {
        let path = self.validated_checkpoint_path(session_id)?;
        self.read_checkpoint_file(&path)
    }

    /// Load the legacy single-slot checkpoint (`checkpoints/latest.json`) if
    /// present. Compatibility read only — this release no longer writes it.
    pub fn load_legacy_checkpoint(&self) -> std::io::Result<Option<SavedSession>> {
        let path = self.checkpoints_dir().join(LEGACY_CHECKPOINT_FILE);
        self.read_checkpoint_file(&path)
    }

    fn legacy_checkpoint_origin(&self) -> io::Result<Option<String>> {
        use std::io::Read as _;

        let path = self.checkpoints_dir().join(LEGACY_CHECKPOINT_FILE);
        let file = match open_private_read_file(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        // Lifecycle cleanup only needs the leading metadata. Never follow
        // links or read an unbounded legacy transcript to identify its owner.
        let mut prefix = Vec::new();
        file.take(1024 * 1024).read_to_end(&mut prefix)?;
        extract_top_level_metadata(&prefix)
            .map(|metadata| Some(metadata.id))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "unknown legacy checkpoint origin",
                )
            })
    }

    /// Clear one session's crash-recovery checkpoint. Scoped: this can never
    /// remove another session's checkpoint file or the legacy slot.
    pub fn clear_session_checkpoint(&self, session_id: &str) -> std::io::Result<()> {
        let path = self.validated_checkpoint_path(session_id)?;
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    /// Remove the legacy single-slot checkpoint file.
    pub fn clear_legacy_checkpoint(&self) -> std::io::Result<()> {
        let path = self.checkpoints_dir().join(LEGACY_CHECKPOINT_FILE);
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    /// Enumerate all crash-recovery checkpoint files (per-session files plus
    /// the legacy single slot), sorted most recently modified first. Only
    /// file metadata is read here; callers load content per candidate.
    pub fn list_checkpoints(&self) -> std::io::Result<Vec<CheckpointRef>> {
        let dir = self.checkpoints_dir();
        let mut refs = Vec::new();
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(refs),
            Err(err) => return Err(err),
        };
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() || path.extension().is_none_or(|ext| ext != "json") {
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let source = if name == LEGACY_CHECKPOINT_FILE {
                CheckpointSource::Legacy
            } else if is_offline_queue_file(name) {
                // Parked offline queues live in this directory but are not
                // crash-recovery checkpoints.
                continue;
            } else {
                let session_id = name.trim_end_matches(".json").to_string();
                if self.validated_checkpoint_path(&session_id).is_err() {
                    continue;
                }
                CheckpointSource::Session(session_id)
            };
            let Ok(modified) = entry.metadata().and_then(|m| m.modified()) else {
                continue;
            };
            refs.push(CheckpointRef {
                source,
                path,
                modified,
            });
        }
        refs.sort_by_key(|r| std::cmp::Reverse(r.modified));
        Ok(refs)
    }

    /// Migrate a session recovered from the legacy single-slot checkpoint to
    /// a per-session checkpoint file. Never overwrites an existing
    /// per-session file and leaves the legacy file in place (older binaries
    /// still read it; the legacy writer is already gone). Returns whether a
    /// file was written.
    pub fn write_session_checkpoint_if_absent(
        &self,
        session: &SavedSession,
    ) -> std::io::Result<bool> {
        let path = self.validated_checkpoint_path(&session.metadata.id)?;
        if path.exists() {
            return Ok(false);
        }
        self.save_checkpoint(session)?;
        Ok(true)
    }

    /// Acquire before loading or editing a queue, including on in-process
    /// resume. A per-write lock is insufficient: the second editor's stale
    /// snapshot would overwrite the first as soon as its write completed.
    pub fn acquire_offline_queue_lease(
        &self,
        session_id: &str,
    ) -> io::Result<std::sync::Arc<OfflineQueueLease>> {
        let session_id = self.validated_session_id(session_id)?.to_string();
        let directory = self.checkpoints_dir();
        fs::create_dir_all(&directory)?;
        let path = directory.join(format!("{session_id}.offline_queue.lock"));
        let file = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)?;
        let mut lock = fd_lock::RwLock::new(file);
        let guard = lock.try_write().map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("Cannot open session {session_id}: its queued input is already open in another window, or its previous writes are still finishing ({error})"),
            )
        })?;
        // fd-lock's guard borrows its owner. Retain the underlying descriptor
        // instead so this lease can travel with asynchronous writes. Forgetting
        // this non-owning guard keeps the OS lock held; the final Arc explicitly
        // unlocks in Drop. The OS also releases it when the process crashes.
        std::mem::forget(guard);
        Ok(std::sync::Arc::new(OfflineQueueLease {
            session_id,
            _file: lock.into_inner(),
        }))
    }

    /// Park this session's offline queue (queued + draft messages).
    ///
    /// Queues are keyed per session (`checkpoints/<session_id>.offline_queue.json`)
    /// for exactly the reason checkpoints are: concurrent Codewhale instances
    /// must never overwrite — or delete — each other's unsent user text.
    ///
    /// A queue with no session id has no owner to restore it to, so parking is
    /// refused rather than written to a shared file where the next boot would
    /// destroy it.
    pub fn save_offline_queue_state(
        &self,
        state: &OfflineQueueState,
        session_id: Option<&str>,
    ) -> std::io::Result<PathBuf> {
        let session_id = session_id.ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Offline queue cannot be parked without a session id",
            )
        })?;
        let path = self.validated_offline_queue_path(session_id)?;
        fs::create_dir_all(self.checkpoints_dir())?;
        let mut owned = state.clone();
        // The stamp is redundant with the file name; it stays because the UI's
        // restore path still compares it against the live session id.
        owned.session_id = Some(self.validated_session_id(session_id)?.to_string());
        let content = serde_json::to_string_pretty(&owned)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        write_atomic(&path, content.as_bytes())?;
        Ok(path)
    }

    /// Load one session's parked offline queue if present.
    pub fn load_offline_queue_state(
        &self,
        session_id: &str,
    ) -> std::io::Result<Option<OfflineQueueState>> {
        let path = self.validated_offline_queue_path(session_id)?;
        Ok(match Self::read_offline_queue_file(&path)? {
            Some(state) => Some(state),
            None => self.adopt_legacy_offline_queue(session_id, &path)?,
        })
    }

    /// Remove one named session's parked offline queue.
    pub fn clear_offline_queue_state_for(&self, session_id: &str) -> std::io::Result<()> {
        let path = self.validated_offline_queue_path(session_id)?;
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        Ok(())
    }

    fn validated_offline_queue_path(&self, session_id: &str) -> std::io::Result<PathBuf> {
        let trimmed = self.validated_session_id(session_id)?;
        Ok(self
            .checkpoints_dir()
            .join(format!("{trimmed}{OFFLINE_QUEUE_SUFFIX}")))
    }

    fn read_offline_queue_file(path: &Path) -> std::io::Result<Option<OfflineQueueState>> {
        let content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let state: OfflineQueueState = serde_json::from_str(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        if state.schema_version > CURRENT_QUEUE_SCHEMA_VERSION {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "Offline queue schema v{} is newer than supported v{}",
                    state.schema_version, CURRENT_QUEUE_SCHEMA_VERSION
                ),
            ));
        }
        Ok(Some(state))
    }

    /// Migrate the pre-per-session global queue (`checkpoints/offline_queue.json`).
    ///
    /// It holds user-authored text, so it is adopted only by the session it was
    /// stamped for, and it is removed only once this session's copy is durably
    /// written. A queue stamped for someone else — or for nobody — is left
    /// exactly where it is, still readable, for its owner to claim.
    fn adopt_legacy_offline_queue(
        &self,
        session_id: &str,
        path: &Path,
    ) -> std::io::Result<Option<OfflineQueueState>> {
        let legacy = self.checkpoints_dir().join(OFFLINE_QUEUE_FILE);
        // A corrupt or future-schema legacy file must not fail this session's
        // boot: leave it on disk untouched and start with an empty queue.
        let Ok(Some(state)) = Self::read_offline_queue_file(&legacy) else {
            return Ok(None);
        };
        if state.session_id.as_deref() != Some(self.validated_session_id(session_id)?) {
            return Ok(None);
        }
        let content = serde_json::to_string_pretty(&state)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        fs::create_dir_all(self.checkpoints_dir())?;
        write_atomic(path, content.as_bytes())?;
        match fs::remove_file(&legacy) {
            Ok(()) => {}
            // A second instance of the same session can win the adoption
            // race: both read the legacy file, both write this session's
            // per-session copy, and the twin's remove already retired the
            // legacy one. The queue is durably adopted either way, so a
            // vanished legacy file is success here, not a boot error.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        Ok(Some(state))
    }

    /// Read a session snapshot without repairing tool call/result pairs.
    ///
    /// This is the correct API for embedding hosts that inspect or update a
    /// durable session while an engine may still be executing a tool call.
    /// A dangling `tool_use` is not proof of a crashed process in that state.
    pub fn load_session_snapshot(&self, id: &str) -> std::io::Result<SavedSession> {
        let path = self.validated_session_path(id)?;

        let content = fs::read_to_string(&path)?;
        let mut session: SavedSession = serde_json::from_str(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        if session.schema_version > CURRENT_SESSION_SCHEMA_VERSION {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "Session schema v{} is newer than supported v{}",
                    session.schema_version, CURRENT_SESSION_SCHEMA_VERSION
                ),
            ));
        }

        session.system_prompt = strip_legacy_truncation_note(session.system_prompt);
        session.ensure_journal();
        self.hydrate_approval_receipts(&mut session)?;
        self.apply_late_usage_to_metadata(&mut session.metadata);

        Ok(session)
    }

    /// Load and repair a session after a known process or engine restart.
    ///
    /// The returned repair remains in memory until the caller persists
    /// `recovery.session`. Keeping persistence explicit lets embedding hosts
    /// serialize recovery with their own transcript mutation lock.
    pub fn recover_session_for_resume(&self, id: &str) -> std::io::Result<SessionRecovery> {
        let mut session = self.load_session_snapshot(id)?;

        let repair = crate::tool_history_repair::repair_tool_call_pairs(&mut session.messages);
        let changed = !repair.is_empty();
        if changed {
            if let Some(journal) = session.journal.as_mut() {
                journal.rebranch_active_messages(&session.messages);
                session.leaf_id = journal.leaf_id.clone();
            }
            session.metadata.message_count = session.messages.len();
            tracing::warn!(
                session_id = %session.metadata.id,
                repaired_call_ids = ?repair.repaired_call_ids,
                duplicate_result_ids = ?repair.duplicate_result_ids,
                orphan_result_ids = ?repair.orphan_result_ids,
                "repaired persisted tool call/result history"
            );
        }

        Ok(SessionRecovery {
            session,
            changed,
            repaired_call_count: repair.repaired_call_ids.len(),
            duplicate_result_count: repair.duplicate_result_ids.len(),
            orphan_result_count: repair.orphan_result_ids.len(),
        })
    }

    /// Load a session by ID for the standalone CodeWhale resume flow.
    ///
    /// This preserves the historical recovery behavior for existing callers.
    /// Embedding hosts performing ordinary runtime reads should use
    /// [`Self::load_session_snapshot`] instead.
    pub fn load_session(&self, id: &str) -> std::io::Result<SavedSession> {
        self.recover_session_for_resume(id)
            .map(|recovery| recovery.session)
    }

    /// Load a session by partial ID prefix
    pub fn load_session_by_prefix(&self, prefix: &str) -> std::io::Result<SavedSession> {
        self.load_session(&self.resolve_session_id_prefix(prefix)?)
    }

    /// Resolve a unique ID without applying resume-time repair to its record.
    pub(crate) fn resolve_session_id_prefix(&self, prefix: &str) -> std::io::Result<String> {
        let sessions = self.list_sessions()?;

        let matches: Vec<_> = sessions
            .into_iter()
            .filter(|s| s.id.starts_with(prefix))
            .collect();

        match matches.len() {
            0 => Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("No session found with prefix: {prefix}"),
            )),
            1 => Ok(matches[0].id.clone()),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "Ambiguous prefix '{}' matches {} sessions",
                    prefix,
                    matches.len()
                ),
            )),
        }
    }

    /// List all saved sessions, sorted by most recently updated
    pub fn list_sessions(&self) -> std::io::Result<Vec<SessionMetadata>> {
        let mut sessions = Vec::new();

        for entry in fs::read_dir(&self.sessions_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().is_some_and(|ext| ext == "json")
                && let Ok(mut session) = Self::load_session_metadata(&path)
            {
                self.apply_late_usage_to_metadata(&mut session);
                sessions.push(session);
            }
        }

        // Sort by updated_at descending (most recent first)
        sessions.sort_by_key(|s| std::cmp::Reverse(s.updated_at));

        Ok(sessions)
    }

    /// Set the durable archive flag on a saved session and return the
    /// resulting metadata.
    ///
    /// This is the single writer for the flag: the picker, the `/sessions`
    /// command, and `PATCH /v1/sessions/{id}` all route through it so the TUI
    /// and the web dashboard cannot drift into two archive notions. A no-op
    /// call (already in the requested state) still returns the metadata and
    /// does not rewrite the file.
    pub fn set_session_archived(
        &self,
        id: &str,
        archived: bool,
        mutator: SessionMutator,
    ) -> std::io::Result<SessionMetadata> {
        if mutator == SessionMutator::External && is_live_session(id) {
            return Err(live_session_conflict(id));
        }
        let mut session = self.load_session(id)?;
        if session.metadata.archived == archived {
            return Ok(session.metadata);
        }
        session.metadata.archived = archived;
        self.save_session(&session)?;
        Ok(session.metadata)
    }

    /// Re-read the durable lifecycle fields for `metadata` from disk.
    ///
    /// This is the autosave-survival guard. A TUI autosave rebuilds the whole
    /// session document from in-memory `App` state; any lifecycle field it
    /// carries from a stale cache would silently revert a rename or archive
    /// that landed in between — including one applied by the picker earlier in
    /// the same event loop, or by `/rename` while a snapshot was already
    /// queued.
    ///
    /// So rather than trusting any cache, the writer re-reads the persisted
    /// values immediately before writing. `title`, `archived`, `created_at`,
    /// and fork lineage are *lifecycle* state owned by the file, not
    /// conversation state owned by the running turn. Reading them back costs
    /// one bounded metadata-prefix read.
    ///
    /// Returns `true` when an existing record was found and merged. A missing
    /// record is not an error: the first save of a new session has nothing to
    /// merge from.
    pub fn merge_persisted_lifecycle(&self, metadata: &mut SessionMetadata) -> bool {
        let Ok(path) = self.validated_session_path(&metadata.id) else {
            return false;
        };
        let Ok(persisted) = Self::load_session_metadata(&path) else {
            return false;
        };
        metadata.title = persisted.title;
        metadata.archived = persisted.archived;
        metadata.created_at = persisted.created_at;
        metadata.parent_session_id = persisted.parent_session_id;
        metadata.forked_from_message_count = persisted.forked_from_message_count;
        metadata.runtime_store = persisted.runtime_store;
        true
    }

    /// Rename a saved session and return the resulting metadata.
    ///
    /// Titles are trimmed and bounded to [`MAX_SESSION_TITLE_CHARS`]
    /// characters (counted in `char`s, not bytes, so a CJK or emoji title is
    /// not truncated mid-scalar). Created-at and fork lineage are untouched.
    pub fn rename_session(
        &self,
        id: &str,
        title: &str,
        mutator: SessionMutator,
    ) -> std::io::Result<SessionMetadata> {
        let title = normalize_session_title(title)?;
        if mutator == SessionMutator::External && is_live_session(id) {
            return Err(live_session_conflict(id));
        }
        let mut session = self.load_session(id)?;
        if session.metadata.title == title {
            return Ok(session.metadata);
        }
        session.metadata.title = title;
        self.save_session(&session)?;
        Ok(session.metadata)
    }

    /// Load only the metadata from a session file.
    ///
    /// Optimization for #337: previously this called
    /// `serde_json::from_reader` which forces serde to scan every token in
    /// the file just to validate JSON structure — including the
    /// (potentially many MB of) `messages` and `tool_log` arrays we're
    /// going to discard. For a user with hundreds of long sessions, a
    /// single `list_sessions()` call could chew through tens of MB of
    /// JSON per startup.
    ///
    /// We now read at most 64 KB up front and string-extract the
    /// top-level `metadata` object, which is invariably tiny (~500 B)
    /// and appears before any large `messages`/`tool_log` payload. We
    /// fall back to a full-file read only if the prefix doesn't yield a
    /// parseable metadata block (e.g. an oddly-formatted legacy file).
    fn load_session_metadata(path: &Path) -> std::io::Result<SessionMetadata> {
        use std::io::Read;

        const PREFIX_BYTES: usize = 64 * 1024;
        let mut file = fs::File::open(path)?;
        let mut buf = Vec::with_capacity(PREFIX_BYTES);
        file.by_ref()
            .take(PREFIX_BYTES as u64)
            .read_to_end(&mut buf)?;

        if let Some(mut metadata) = extract_top_level_metadata(&buf) {
            apply_legacy_title_recovery(&mut metadata, &buf);
            return Ok(metadata);
        }

        // Metadata wasn't extractable from the prefix (truncated mid-block,
        // unusual key ordering, etc.). Read the rest and try again with the
        // full buffer before giving up.
        let mut rest = Vec::new();
        file.read_to_end(&mut rest)?;
        buf.extend_from_slice(&rest);
        let mut metadata = extract_top_level_metadata(&buf).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "session file missing parseable `metadata` block",
            )
        })?;
        apply_legacy_title_recovery(&mut metadata, &buf);
        Ok(metadata)
    }

    /// Delete a session and its recovery checkpoints, retiring its origin.
    pub fn delete_session(&self, id: &str) -> std::io::Result<()> {
        self.remove_session(id, SessionRemoval::Explicit)
    }

    fn remove_session(&self, id: &str, removal: SessionRemoval) -> std::io::Result<()> {
        let path = self.validated_session_path(id)?;
        // Older ordinary snapshots may use a name reserved by the checkpoint
        // directory. Such a name must never address its shared legacy files.
        let checkpoint = self.validated_checkpoint_path(id).ok();
        let legacy_checkpoint = self.checkpoints_dir().join(LEGACY_CHECKPOINT_FILE);
        let (late_path, lock_path) = self.ensure_late_usage_paths(id)?;
        let lock_file = open_private_lock_file(&lock_path)?;
        let mut lock = fd_lock::RwLock::new(lock_file);
        let _guard = lock.write()?;
        let already_deleted = Self::late_usage_is_deleted(&late_path)?;
        let legacy_origin = self.legacy_checkpoint_origin();
        let owns_legacy_checkpoint =
            matches!(&legacy_origin, Ok(Some(origin)) if origin == id.trim());
        let has_recovery = match checkpoint.as_ref() {
            Some(path) => path.try_exists()?,
            None => false,
        } || owns_legacy_checkpoint;
        if !already_deleted {
            // An unknown id must not acquire a deletion marker. A prior
            // tombstone, however, lets a retry finish interrupted cleanup.
            match fs::symlink_metadata(&path) {
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound && has_recovery => {}
                Err(error) => return Err(error),
            }
        }
        if matches!(removal, SessionRemoval::Retention)
            && (has_recovery || legacy_origin.is_err())
            && !already_deleted
        {
            // Retention owns the ordinary snapshot, not crash recovery. Keep
            // the origin and its accounting/evidence writable for resume.
            // An unreadable legacy origin cannot justify retiring any id.
            return match fs::remove_file(&path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error),
            };
        }
        self.save_session_goal(id, None)?;
        // Publish the tombstone before removing data. A crash or a delayed
        // callback can no longer re-create this session's accounting. The
        // stable lock inode must never be removed or atomically replaced.
        if !already_deleted {
            write_atomic(&late_path.with_extension("deleted"), LATE_USAGE_DELETED)?;
        }
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        match fs::remove_file(&late_path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        if let Some(checkpoint) = checkpoint {
            match fs::remove_file(checkpoint) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        if owns_legacy_checkpoint {
            match fs::remove_file(&legacy_checkpoint) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        self.clear_session_boot_owner(id);
        let session_dir = self.sessions_dir.join(id.trim());
        if session_dir.exists() {
            if crate::plugins::metadata_is_link_or_reparse(&fs::symlink_metadata(&session_dir)?) {
                // Preserve remove_dir_all's existing no-follow behavior.
                fs::remove_dir_all(session_dir)?;
                return Ok(());
            }
            // Other conversations and automations can share this host's Runtime
            // authority. Deleting a transcript must never delete that store.
            for entry in fs::read_dir(&session_dir)? {
                let entry = entry?;
                if entry.file_name() == "runtime" {
                    continue;
                }
                if entry.file_type()?.is_dir() {
                    fs::remove_dir_all(entry.path())?;
                } else {
                    fs::remove_file(entry.path())?;
                }
            }
            if fs::read_dir(&session_dir)?.next().is_none() {
                fs::remove_dir(session_dir)?;
            }
        }
        Ok(())
    }

    /// Clean up old sessions to stay within `MAX_SESSIONS` limit.
    pub fn cleanup_old_sessions(&self) -> std::io::Result<()> {
        self.cleanup_old_sessions_keeping(None)
    }

    /// As [`Self::cleanup_old_sessions`], but never deletes `keep` — the
    /// session being resumed at boot. Without this, a background cleanup that
    /// races session restore can prune the just-resumed session when 50+
    /// newer records exist (its `updated_at` is not bumped until first save).
    pub fn cleanup_old_sessions_keeping(&self, keep: Option<&str>) -> std::io::Result<()> {
        let sessions = self.list_sessions()?;

        if sessions.len() > MAX_SESSIONS {
            for session in sessions.iter().skip(MAX_SESSIONS) {
                if keep.is_some_and(|id| id == session.id) {
                    continue;
                }
                let _ = self.remove_session(&session.id, SessionRemoval::Retention);
            }
        }
        // A directory without a top-level session snapshot is not proof of an
        // orphan: runtime threads and automations own independent durable stores,
        // including in other processes and before their first snapshot. Retention
        // only removes records it listed above; never infer authority to delete
        // other directories from an absent transcript or process-local claim.

        Ok(())
    }

    /// Remove session files whose `updated_at` is older than `max_age`
    /// from the persisted-sessions directory. Returns the number of
    /// records pruned. Building block for #406's phase-2 auto-archive
    /// on boot; today the user-facing entry point is the
    /// `/sessions prune <days>` slash command.
    ///
    /// Crash-recovery safety: skips the per-session checkpoint files
    /// (`checkpoints/<session_id>.json`), the legacy single-slot
    /// checkpoint (`checkpoints/latest.json`), and any file under `checkpoints/`
    /// — those are owned by the checkpoint subsystem and live with
    /// stricter durability rules. Only top-level `<session_id>.json`
    /// files are candidates.
    ///
    /// `max_age` is checked against the metadata's `updated_at`
    /// timestamp embedded in the JSON, not the filesystem mtime — the
    /// user may have rsynced their `~/.deepseek` between machines and
    /// fs mtimes can lie.
    pub fn prune_sessions_older_than(
        &self,
        max_age: std::time::Duration,
    ) -> std::io::Result<usize> {
        self.prune_sessions_older_than_keeping(max_age, None)
    }

    /// As [`Self::prune_sessions_older_than`], but never deletes `keep` — the
    /// active session. A just-resumed session's `updated_at` is stale until
    /// its first post-resume save, so an age prune could otherwise delete the
    /// live session out from under the TUI.
    pub fn prune_sessions_older_than_keeping(
        &self,
        max_age: std::time::Duration,
        keep: Option<&str>,
    ) -> std::io::Result<usize> {
        let cutoff = Utc::now()
            - chrono::Duration::from_std(max_age).unwrap_or(chrono::Duration::days(365 * 10));
        let sessions = self.list_sessions()?;
        let mut pruned = 0usize;
        for session in sessions {
            if keep.is_some_and(|id| id == session.id) {
                continue;
            }
            if session.updated_at < cutoff {
                if let Err(err) = self.remove_session(&session.id, SessionRemoval::Retention) {
                    tracing::warn!(
                        target: "session",
                        session = session.id,
                        ?err,
                        "session prune skipped a record",
                    );
                    continue;
                }
                pruned += 1;
            }
        }
        Ok(pruned)
    }

    /// Get the most recent session scoped to the current workspace.
    ///
    /// Archived sessions are skipped: archiving is the user saying "not this
    /// one", and `--continue` / auto-resume must honour that rather than
    /// dragging a put-away session back.
    pub fn get_latest_session_for_workspace(
        &self,
        workspace: &Path,
    ) -> std::io::Result<Option<SessionMetadata>> {
        let sessions = self.list_sessions()?;
        Ok(sessions.into_iter().find(|session| {
            !session.archived
                && workspace_scope_matches(&session.workspace, workspace)
                && !is_empty_auto_created_session(session)
        }))
    }

    /// Search sessions by title
    pub fn search_sessions(&self, query: &str) -> std::io::Result<Vec<SessionMetadata>> {
        let query_lower = query.to_lowercase();
        let sessions = self.list_sessions()?;

        Ok(sessions
            .into_iter()
            .filter(|s| s.title.to_lowercase().contains(&query_lower))
            .collect())
    }
}

/// Unicode format characters that never belong in a session title: bidi
/// embeddings/overrides/isolates and marks, zero-width joiners/spaces, the
/// soft hyphen, BOM, and line/paragraph separators. Together with
/// `char::is_control` (C0, DEL, C1 — so ESC, BEL, ST, and OSC introducers)
/// this is the one character policy for the persisted title, the terminal
/// tab title, and every plain-text listing that echoes a title.
pub(crate) fn is_title_format_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{00ad}'
            | '\u{061c}'
            | '\u{200b}'..='\u{200f}'
            | '\u{2028}'..='\u{202e}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{2069}'
            | '\u{feff}'
    )
}

/// Drop control and bidi/zero-width format characters from a title.
///
/// A session title is user- or content-derived text that later reaches an
/// OSC 0 terminal title, `codewhale sessions` stdout, and the picker, so the
/// persisted value must not be able to carry a raw escape sequence. Ordinary
/// text, punctuation, CJK, and emoji pass through untouched.
pub fn sanitize_session_title(raw: &str) -> String {
    raw.chars()
        .filter(|ch| !ch.is_control() && !is_title_format_char(*ch))
        .collect()
}

/// Sanitize, trim, and bound a user-supplied session title.
///
/// Returns `InvalidInput` for an empty title or one longer than
/// [`MAX_SESSION_TITLE_CHARS`] so every rename surface (picker, `/rename`,
/// `PATCH /v1/sessions/{id}`) rejects the same inputs with the same reason.
pub fn normalize_session_title(title: &str) -> std::io::Result<String> {
    let sanitized = sanitize_session_title(title);
    let trimmed = sanitized.trim();
    if trimmed.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Session title cannot be empty",
        ));
    }
    if trimmed.chars().count() > MAX_SESSION_TITLE_CHARS {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("Session title cannot exceed {MAX_SESSION_TITLE_CHARS} characters"),
        ));
    }
    Ok(trimmed.to_string())
}

pub(crate) fn workspace_scope_matches(saved_workspace: &Path, current_workspace: &Path) -> bool {
    if paths_equivalent(saved_workspace, current_workspace) {
        return true;
    }

    // Repository identity comes from the containing checkout itself (Git
    // dir/worktree traversal shared with project-context scope resolution),
    // never from branch names or paths mentioned in conversation.
    let canonical = |path: &Path| fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    match (
        find_git_root(&canonical(saved_workspace)),
        find_git_root(&canonical(current_workspace)),
    ) {
        (Some(saved_root), Some(current_root)) => paths_equivalent(&saved_root, &current_root),
        _ => false,
    }
}

pub(crate) fn is_empty_auto_created_session(session: &SessionMetadata) -> bool {
    session.message_count == 0
        && session
            .title
            .trim()
            .eq_ignore_ascii_case(DEFAULT_SESSION_TITLE)
}

pub(crate) fn paths_equivalent(lhs: &Path, rhs: &Path) -> bool {
    let lhs_canonical = fs::canonicalize(lhs).ok();
    let rhs_canonical = fs::canonicalize(rhs).ok();
    match (lhs_canonical, rhs_canonical) {
        (Some(lhs), Some(rhs)) => lhs == rhs,
        _ => lhs == rhs,
    }
}

/// Resolve the default session directory path.
///
/// v0.8.44: prefers `~/.codewhale/sessions`, falls back to
/// `~/.deepseek/sessions` for existing installs. Uses the write-path resolver
/// so the first access relocates any legacy `~/.deepseek/sessions` into
/// `~/.codewhale/sessions` when the primary directory is missing (#3240).
/// If an older build already created an empty primary sessions directory, copy
/// missing legacy entries into it without overwriting newer CodeWhale data.
pub fn default_sessions_dir() -> std::io::Result<PathBuf> {
    let dir = codewhale_config::ensure_state_dir("sessions")
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::NotFound, e.to_string()))?;
    match merge_missing_legacy_session_entries(&dir) {
        Ok(0) => {}
        Ok(count) => {
            tracing::info!(
                target: "session::migration",
                "Copied {count} missing legacy session entries into {}",
                dir.display()
            );
        }
        Err(err) => {
            tracing::warn!(
                target: "session::migration",
                "Could not copy legacy sessions into {}: {err}",
                dir.display()
            );
        }
    }
    Ok(dir)
}

fn merge_missing_legacy_session_entries(primary: &Path) -> io::Result<usize> {
    if codewhale_paths::codewhale_home_is_explicit() {
        return Ok(0);
    }

    let legacy = codewhale_config::legacy_deepseek_home()
        .map_err(|e| io::Error::new(io::ErrorKind::NotFound, e.to_string()))?
        .join("sessions");
    if !legacy.is_dir() || paths_equivalent(primary, &legacy) {
        return Ok(0);
    }

    copy_missing_dir_entries(&legacy, primary)
}

fn copy_missing_dir_entries(src: &Path, dst: &Path) -> io::Result<usize> {
    fs::create_dir_all(dst)?;
    let mut copied = 0;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let source = entry.path();
        let target = dst.join(entry.file_name());

        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            if entry.file_name() == std::ffi::OsStr::new("checkpoints") || target.exists() {
                continue;
            }
            copied += copy_missing_dir_entries(&source, &target)?;
        } else if file_type.is_file() {
            copied += usize::from(copy_file_create_new(&source, &target)?);
        }
    }
    Ok(copied)
}

fn copy_file_create_new(src: &Path, dst: &Path) -> io::Result<bool> {
    let mut source = fs::File::open(src)?;
    let mut target = match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dst)
    {
        Ok(file) => file,
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => return Ok(false),
        Err(err) => return Err(err),
    };
    if let Err(err) = io::copy(&mut source, &mut target) {
        let _ = fs::remove_file(dst);
        return Err(err);
    }
    Ok(true)
}

/// Prune snapshots older than `max_age` for `workspace`.
///
/// Always non-fatal. Returns silently — callers don't need the count
/// (the underlying repo logs at WARN if anything blew up).
pub fn prune_workspace_snapshots(workspace: &Path, max_age: std::time::Duration) {
    match crate::snapshot::prune_older_than(workspace, max_age) {
        Ok(0) => {}
        Ok(n) => {
            tracing::debug!(target: "snapshot", "boot prune removed {n} snapshot(s)");
        }
        Err(e) => {
            tracing::warn!(target: "snapshot", "boot prune failed: {e}");
        }
    }
}

/// Create a new `SavedSession` from conversation state
pub fn create_saved_session(
    messages: &[Message],
    model: &str,
    workspace: &Path,
    total_tokens: u64,
    system_prompt: Option<&SystemPrompt>,
) -> SavedSession {
    create_saved_session_with_mode(
        messages,
        model,
        workspace,
        total_tokens,
        system_prompt,
        None,
    )
}

/// Placeholder title used for a session that has no first user message yet.
/// `build_session_snapshot` (tui/ui/frame.rs) treats a title equal to this
/// constant as an auto-generated placeholder and lets the conversation-derived
/// title win once a user message exists. Keep this string stable on purpose.
pub(crate) const DEFAULT_SESSION_TITLE: &str = "New Session";

/// Create a new `SavedSession` from conversation state with optional mode label
pub fn create_saved_session_with_mode(
    messages: &[Message],
    model: &str,
    workspace: &Path,
    total_tokens: u64,
    system_prompt: Option<&SystemPrompt>,
    mode: Option<&str>,
) -> SavedSession {
    create_saved_session_with_id_and_mode(
        Uuid::new_v4().to_string(),
        messages,
        model,
        workspace,
        total_tokens,
        system_prompt,
        mode,
    )
}

/// Create a new `SavedSession` using a caller-owned session id.
pub fn create_saved_session_with_id_and_mode(
    id: String,
    messages: &[Message],
    model: &str,
    workspace: &Path,
    total_tokens: u64,
    system_prompt: Option<&SystemPrompt>,
    mode: Option<&str>,
) -> SavedSession {
    create_saved_session_with_id_mode_and_stamps(
        id,
        messages,
        &[],
        model,
        workspace,
        total_tokens,
        system_prompt,
        mode,
    )
}

/// Create a new `SavedSession` whose journal entries keep the time each
/// message actually landed. `message_stamps[i]` is the append time of
/// `messages[i]`; a missing stamp falls back to now. Callers without a
/// live stamp log pass `&[]` and get the historical save-time behavior.
pub fn create_saved_session_with_id_mode_and_stamps(
    id: String,
    messages: &[Message],
    message_stamps: &[DateTime<Utc>],
    model: &str,
    workspace: &Path,
    total_tokens: u64,
    system_prompt: Option<&SystemPrompt>,
    mode: Option<&str>,
) -> SavedSession {
    let now = Utc::now();

    // Generate title from the first real user message (runtime-owned control
    // traffic is skipped by `conversation_derived_title`). Fall back to the
    // placeholder when no user-authored prompt exists yet.
    let title =
        conversation_derived_title(messages).unwrap_or_else(|| DEFAULT_SESSION_TITLE.to_string());

    let journal = SessionJournal::from_messages_stamped(messages.to_vec(), message_stamps, 0);
    let leaf_id = journal.leaf_id.clone();
    SavedSession {
        schema_version: CURRENT_SESSION_SCHEMA_VERSION,
        metadata: SessionMetadata {
            id,
            title,
            created_at: now,
            updated_at: now,
            message_count: messages.len(),
            total_tokens,
            model: model.to_string(),
            model_provider: default_model_provider(),
            model_provider_id: None,
            workspace: workspace.to_path_buf(),
            mode: mode.map(str::to_string),
            cost: SessionCostSnapshot::default(),
            parent_session_id: None,
            forked_from_message_count: None,
            runtime_store: None,
            cumulative_turn_secs: 0,
            archived: false,
            spawn_depth: 0,
        },
        messages: messages.to_vec(),
        journal: Some(journal),
        leaf_id,
        system_prompt: system_prompt_to_string(system_prompt),
        context_references: Vec::new(),
        artifacts: Vec::new(),
        approval_receipts: Vec::new(),
        work_state: None,
        window_title: None,
        last_auto_route: None,
    }
}

/// Update an existing session with new messages
pub fn update_session(
    mut session: SavedSession,
    messages: &[Message],
    total_tokens: u64,
    system_prompt: Option<&SystemPrompt>,
) -> SavedSession {
    session.schema_version = CURRENT_SESSION_SCHEMA_VERSION;
    session.ensure_journal();
    let old_len = session.messages.len();
    let new_len = messages.len();
    if new_len >= old_len && messages[..old_len] == session.messages[..] {
        if let Some(journal) = session.journal.as_mut() {
            for msg in &messages[old_len..] {
                journal.append_message(msg.clone());
            }
            session.leaf_id = journal.leaf_id.clone();
        }
    } else if (new_len != old_len || messages != session.messages.as_slice())
        && let Some(journal) = session.journal.as_mut()
    {
        let common = messages
            .iter()
            .zip(session.messages.iter())
            .take_while(|(a, b)| a == b)
            .count();
        if common > 0 && common <= journal.entries.len() {
            let target_id = journal
                .root_to_leaf()
                .get(common - 1)
                .map(|entry| entry.id.clone());
            if let Some(target_id) = target_id {
                let _ = journal.branch_to(&target_id);
            } else {
                journal.leaf_id = None;
            }
        } else if common == 0 {
            journal.leaf_id = journal.entries.first().and_then(|e| e.parent_id.clone());
            if journal.leaf_id.is_none() && !journal.entries.is_empty() {
                journal.leaf_id = None;
            }
        }
        for msg in messages.iter().skip(common) {
            journal.append_message(msg.clone());
        }
        session.leaf_id = journal.leaf_id.clone();
    }
    session.messages.clear();
    session.messages.extend_from_slice(messages);
    session.metadata.updated_at = Utc::now();
    session.metadata.message_count = messages.len();
    session.metadata.total_tokens = total_tokens;
    session.system_prompt = system_prompt_to_string(system_prompt);
    session
}

/// Strip a stale `[Session note]` block that was written by the old
/// 500-message cap. Only removes notes that contain the specific
/// "older messages were dropped" phrase — ordinary user-added
/// `[Session note]` prompts are left untouched.
fn strip_legacy_truncation_note(system_prompt: Option<String>) -> Option<String> {
    let sp = system_prompt?;
    let Some(trimmed) = sp.strip_prefix("[Session note]\n") else {
        return Some(sp);
    };
    // Only strip if this is the known cap_messages note.
    if !trimmed.contains("older messages were dropped") {
        return Some(sp);
    }
    // The note block ends with "\n\n---\n\n" (7 chars) followed by the real prompt.
    trimmed
        .find("\n\n---\n\n")
        .map(|pos| trimmed[pos + 7..].to_string())
}

/// Byte offset of `key` (a quoted JSON key such as `"metadata"`) outside any
/// string literal. Brace/string-aware so a key name quoted inside an earlier
/// message body is never matched.
fn find_json_key(bytes: &[u8], key: &[u8]) -> Option<usize> {
    let mut idx = 0usize;
    let mut in_string = false;
    let mut escape = false;
    while idx < bytes.len() {
        let c = bytes[idx];
        if escape {
            escape = false;
        } else if c == b'\\' {
            escape = true;
        } else if c == b'"' {
            if !in_string && bytes[idx..].starts_with(key) {
                return Some(idx);
            }
            in_string = !in_string;
        }
        idx += 1;
    }
    None
}

/// Offset of the value opening with `open` that follows the key at
/// `key_offset`.
fn json_value_start(bytes: &[u8], key_offset: usize, key_len: usize, open: u8) -> Option<usize> {
    let mut idx = key_offset + key_len;
    while idx < bytes.len() && (bytes[idx] as char).is_whitespace() {
        idx += 1;
    }
    if idx >= bytes.len() || bytes[idx] != b':' {
        return None;
    }
    idx += 1;
    while idx < bytes.len() && (bytes[idx] as char).is_whitespace() {
        idx += 1;
    }
    (idx < bytes.len() && bytes[idx] == open).then_some(idx)
}

/// Exclusive end of the balanced `{...}` starting at `start`, or `None` when
/// the buffer is truncated before it closes.
fn json_object_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape = false;
    for (offset, &c) in bytes[start..].iter().enumerate() {
        if escape {
            escape = false;
            continue;
        }
        match c {
            b'\\' => escape = true,
            b'"' => in_string = !in_string,
            b'{' if !in_string => depth += 1,
            b'}' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    return Some(start + offset + 1);
                }
            }
            _ => {}
        }
    }
    None
}

/// String-scan a JSON byte buffer for the top-level `"metadata":{...}`
/// block and return it parsed. Returns `None` if no balanced metadata
/// object is present in the buffer.
///
/// Supports the optimisation in `SessionManager::load_session_metadata`
/// (#337). The scanner is brace-balanced and string-aware so a `{` or
/// `}` appearing inside a string literal doesn't perturb the depth
/// count.
fn extract_top_level_metadata(buf: &[u8]) -> Option<SessionMetadata> {
    let s = std::str::from_utf8(buf).ok()?;
    let bytes = s.as_bytes();
    const KEY: &[u8] = b"\"metadata\"";
    let start = json_value_start(bytes, find_json_key(bytes, KEY)?, KEY.len(), b'{')?;
    let end = json_object_end(bytes, start)?;
    serde_json::from_str::<SessionMetadata>(&s[start..end]).ok()
}

/// Complete message objects from the front of the `messages` array, plus
/// whether the array was seen to end. A message the prefix cut in half is
/// simply absent; nothing is reconstructed.
fn extract_leading_messages(buf: &[u8], max: usize) -> (Vec<Message>, bool) {
    let Ok(s) = std::str::from_utf8(buf) else {
        return (Vec::new(), false);
    };
    let bytes = s.as_bytes();
    const KEY: &[u8] = b"\"messages\"";
    let Some(key_offset) = find_json_key(bytes, KEY) else {
        return (Vec::new(), false);
    };
    let Some(array_start) = json_value_start(bytes, key_offset, KEY.len(), b'[') else {
        return (Vec::new(), false);
    };
    let mut cursor = array_start + 1;
    let mut out = Vec::new();
    loop {
        while cursor < bytes.len() && matches!(bytes[cursor], b' ' | b'\t' | b'\r' | b'\n' | b',') {
            cursor += 1;
        }
        if cursor < bytes.len() && bytes[cursor] == b']' {
            return (out, true);
        }
        if out.len() >= max || cursor >= bytes.len() || bytes[cursor] != b'{' {
            return (out, false);
        }
        let Some(end) = json_object_end(bytes, cursor) else {
            return (out, false);
        };
        let Ok(message) = serde_json::from_str::<Message>(&s[cursor..end]) else {
            return (out, false);
        };
        out.push(message);
        cursor = end;
    }
}

/// How many leading messages the legacy-title recovery will parse. The
/// enclosing read is already bounded to a 64 KB prefix (#337); this bounds
/// the parse inside it.
const LEGACY_TITLE_SCAN_MESSAGES: usize = 24;

/// Recover a title that a superseded derivation took from runtime control
/// traffic, using the session's own first real user prompt.
///
/// Provenance is proven, never guessed: the stored title has to be exactly
/// what the old rule produced — [`truncate_title`] of a message the current
/// classifier rejects as not a user turn. A renamed session, and a person who
/// literally typed an envelope as their first message, never match, so their
/// text is kept. Returns `None` when there is nothing proven to recover.
fn recovered_legacy_title(
    stored: &str,
    messages: &[Message],
    array_complete: bool,
) -> Option<String> {
    let stale = messages.iter().any(|message| {
        crate::runtime_handoff::classify_user_turn_prompt(message)
            == crate::runtime_handoff::UserTurnPromptKind::NotPrompt
            && message.content.iter().any(|block| match block {
                ContentBlock::Text { text, .. } => {
                    truncate_title(text, 50) == stored
                        || truncate_title(extract_user_prompt(text), 50) == stored
                }
                _ => false,
            })
    });
    if !stale {
        return None;
    }
    match conversation_derived_title(messages) {
        Some(title) => Some(title),
        // No user turn in what we read. Only claim the conversation has none
        // when the array actually ended inside the prefix; a truncated read
        // keeps the stored title rather than inventing a neutral one.
        None if array_complete => Some(DEFAULT_SESSION_TITLE.to_string()),
        None => None,
    }
}

/// Apply [`recovered_legacy_title`] to freshly loaded metadata. In memory
/// only — the session file is never rewritten, so the stored title (and any
/// rename) survives on disk.
fn apply_legacy_title_recovery(metadata: &mut SessionMetadata, buf: &[u8]) {
    // Cost gate, never the rename decision: an envelope title always opens
    // with `<`, so this keeps #337's bounded-parse win for ordinary titles.
    // Whether to rewrite is `recovered_legacy_title`'s proven provenance.
    if !metadata.title.starts_with('<') {
        return;
    }
    let (messages, complete) = extract_leading_messages(buf, LEGACY_TITLE_SCAN_MESSAGES);
    if messages.is_empty() {
        return;
    }
    if let Some(title) = recovered_legacy_title(&metadata.title, &messages, complete) {
        metadata.title = title;
    }
}

fn system_prompt_to_string(system_prompt: Option<&SystemPrompt>) -> Option<String> {
    match system_prompt {
        Some(SystemPrompt::Text(text)) => Some(text.clone()),
        Some(SystemPrompt::Blocks(blocks)) => Some(
            blocks
                .iter()
                .map(|b| b.text.clone())
                .collect::<Vec<_>>()
                .join("\n\n---\n\n"),
        ),
        None => None,
    }
}

/// Truncate a session ID to 8 characters for compact display.
/// Returns a `&str` borrowing from the input — no allocation.
pub fn truncate_id(id: &str) -> &str {
    id.get(..8).unwrap_or(id)
}

/// Strip a leading `<turn_meta>...</turn_meta>` block from saved user text.
///
/// Older sessions can have turn metadata prefixed to the first user message.
/// The session picker and generated session titles should show the user's
/// prompt, not the cache/debug envelope.
pub(crate) fn extract_user_prompt(raw: &str) -> &str {
    let trimmed = raw.trim_start();
    let Some(after_open) = trimmed.strip_prefix("<turn_meta>") else {
        return trimmed;
    };
    if let Some(close_pos) = after_open.find("</turn_meta>") {
        return after_open[close_pos + "</turn_meta>".len()..].trim_start();
    }
    after_open.trim_start()
}

/// Clean a stored title for display, falling back to a neutral label.
pub(crate) fn extract_title(raw: &str) -> &str {
    let title = extract_user_prompt(raw);
    if title.is_empty() { "Session" } else { title }
}

/// Strip common inline thinking/reasoning XML sections from saved assistant
/// text before it is shown in session previews.
pub(crate) fn strip_thinking_tags(text: &str) -> String {
    if !text.contains("<think") && !text.contains("<thinking") && !text.contains("<reasoning") {
        return text.to_string();
    }

    let tags = ["think", "thinking", "reasoning"];
    let mut result = text.to_string();
    for tag in tags {
        let open = format!("<{tag}>");
        let close = format!("</{tag}>");
        while let Some(start) = result.find(&open) {
            let Some(end) = result[start..].find(&close) else {
                break;
            };
            let end_abs = start + end + close.len();
            result.replace_range(start..end_abs, "");
        }
    }
    result
}

/// Truncate a string to create a title (character-safe for UTF-8)
fn truncate_title(s: &str, max_len: usize) -> String {
    let s = s.trim();
    // Older sessions may carry a title saved before sanitization existed;
    // never echo raw controls into stdout or the picker. Take the first
    // line before sanitizing so a legacy multi-line title still shows only
    // its first line.
    let first_line = sanitize_session_title(s.lines().next().unwrap_or(s));
    let first_line = first_line.trim();

    let char_count = first_line.chars().count();
    if char_count <= max_len {
        first_line.to_string()
    } else {
        let truncated: String = first_line.chars().take(max_len - 3).collect();
        format!("{truncated}...")
    }
}

/// Derive the auto-title from the first real user message of a conversation.
///
/// Returns `None` when no user-authored message exists to name the session
/// after (an empty transcript, or one holding only runtime-owned control
/// traffic); callers fall back to [`DEFAULT_SESSION_TITLE`].
///
/// Chat-template compatibility forces runtime-owned control traffic
/// (sub-agent handoffs, the Operate contract, restore checkpoints) through
/// `role = "user"`, but an internal envelope is not what the person typed.
/// Prompt eligibility comes from the existing user-turn classifier; the live
/// title fallback shares the same selection through `conversation_title_prompt`.
fn conversation_derived_title(messages: &[Message]) -> Option<String> {
    conversation_title_prompt(messages).map(|prompt| truncate_title(prompt, 50))
}

/// Select the first real user turn's text for persisted and live titles.
/// Keep an image-only turn as the first user boundary, and strip historical
/// leading turn metadata without introducing another provenance classifier.
pub(crate) fn conversation_title_prompt(messages: &[Message]) -> Option<&str> {
    messages
        .iter()
        .find(|message| {
            crate::runtime_handoff::classify_user_turn_prompt(message)
                != crate::runtime_handoff::UserTurnPromptKind::NotPrompt
        })
        .and_then(|m| {
            m.content.iter().find_map(|block| match block {
                ContentBlock::Text { text, .. } => {
                    let prompt = extract_user_prompt(text);
                    if prompt.is_empty() {
                        None
                    } else {
                        Some(prompt)
                    }
                }
                _ => None,
            })
        })
}

/// Format a session for display in a picker
pub fn format_session_line(meta: &SessionMetadata) -> String {
    let age = format_age(&meta.updated_at);
    let updated = format_session_updated_at(&meta.updated_at, &age);
    let truncated_title = truncate_title(extract_title(&meta.title), 40);
    let fork_label = if meta.parent_session_id.is_some() {
        " | fork"
    } else {
        ""
    };

    format!(
        "{} | {} | {} msgs{} | {}",
        truncate_id(&meta.id),
        truncated_title,
        meta.message_count,
        fork_label,
        updated
    )
}

pub(crate) fn format_session_updated_at(dt: &DateTime<Utc>, age: &str) -> String {
    format!("{} ({age})", dt.format("%Y-%m-%d %H:%M UTC"))
}

/// Format a datetime as relative age
fn format_age(dt: &DateTime<Utc>) -> String {
    let now = Utc::now();
    let duration = now.signed_duration_since(*dt);

    if duration.num_minutes() < 1 {
        "just now".to_string()
    } else if duration.num_hours() < 1 {
        format!("{}m ago", duration.num_minutes())
    } else if duration.num_days() < 1 {
        format!("{}h ago", duration.num_hours())
    } else if duration.num_weeks() < 1 {
        format!("{}d ago", duration.num_days())
    } else {
        format!("{}w ago", duration.num_weeks())
    }
}

// === Unit Tests ===

#[cfg(test)]
mod tests {
    use super::*;
    use crate::approval_log::ApprovalOutcome;
    use crate::tools::plan::StepStatus;
    use crate::tui::history::{HistoryCell, ToolCell, history_cells_from_message};
    use codewhale_models::ContentBlock;
    use codewhale_models::Role;
    use std::fs;
    use tempfile::tempdir;

    fn make_test_message(role: &str, text: &str) -> Message {
        Message {
            role: Role::from(role),
            content: vec![ContentBlock::Text {
                text: text.to_string(),
                cache_control: None,
            }],
        }
    }

    /// The journal is the session's timeline: an entry's `created_at` is when
    /// the message landed, not when a save ran. A rebuilt journal must not
    /// collapse 90 minutes of appends into the save instant — an inspector
    /// reading the file needs "this loop is 12 seconds" to be true.
    #[test]
    fn journal_entries_keep_append_stamps_across_saves() {
        let tmp = tempdir().expect("tempdir");
        let messages = vec![
            make_test_message("user", "first"),
            make_test_message("assistant", "answer"),
        ];
        let t0 = Utc::now() - chrono::Duration::minutes(90);
        let t1 = t0 + chrono::Duration::seconds(12);
        let session = create_saved_session_with_id_mode_and_stamps(
            "stamped".to_string(),
            &messages,
            &[t0, t1],
            "deepseek-v4-flash",
            tmp.path(),
            0,
            None,
            None,
        );
        let journal = session.journal.as_ref().expect("journal");
        assert_eq!(journal.entries[0].created_at, t0);
        assert_eq!(journal.entries[1].created_at, t1);
        assert_ne!(
            journal.entries[0].created_at, session.metadata.updated_at,
            "an append 90 minutes before save must not read as save time"
        );
        // Resume hands the same stamps back to the live log.
        assert_eq!(session.journal_message_stamps(), vec![t0, t1]);
        // A save with no stamps keeps the old behavior: entries collapse to
        // save time rather than inventing times.
        let unstamped = create_saved_session_with_id_and_mode(
            "unstamped".to_string(),
            &messages,
            "deepseek-v4-flash",
            tmp.path(),
            0,
            None,
            None,
        );
        let journal = unstamped.journal.as_ref().expect("journal");
        assert!(
            journal
                .entries
                .iter()
                .all(|entry| entry.created_at >= unstamped.metadata.created_at),
            "without stamps, entries stamp at save as before"
        );
    }

    fn save_late_usage_test_session(manager: &SessionManager, id: &str) -> SavedSession {
        let session = create_saved_session_with_id_and_mode(
            id.to_string(),
            &[make_test_message("user", "recoverable transcript")],
            "deepseek-v4-flash",
            manager.sessions_dir(),
            0,
            None,
            Some("agent"),
        );
        manager.save_session(&session).expect("save session");
        session
    }

    fn late_usage_test_record(source_id: &str) -> crate::cost_status::RuntimeUsageRecord {
        crate::cost_status::RuntimeUsageRecord {
            source_id: source_id.to_string(),
            usage: crate::cost_status::EffectiveRouteUsage {
                route: crate::cost_status::EffectiveRouteEnvelope::capture(
                    None,
                    ApiProvider::Deepseek,
                    "deepseek",
                    "deepseek-v4-flash",
                    Some(crate::config::DEFAULT_DEEPSEEK_BASE_URL),
                    Utc::now(),
                ),
                usage: codewhale_models::Usage {
                    input_tokens: 1,
                    ..Default::default()
                },
            },
        }
    }

    #[test]
    fn late_usage_reads_do_not_create_accounting_storage() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("manager");
        let saved = save_late_usage_test_session(&manager, "no-late-usage");
        let directory = manager.sessions_dir().join(LATE_USAGE_DIR);
        let inventory = || {
            fs::read_dir(&directory)
                .expect("accounting directory")
                .map(|entry| entry.expect("entry").file_name())
                .collect::<BTreeSet<_>>()
        };
        let before = inventory();
        assert_eq!(before.len(), 1, "save creates the lifecycle lock only");
        assert_eq!(manager.list_sessions().expect("list").len(), 1);
        manager.load_session_by_prefix("no-late").expect("resume");
        manager
            .load_session_snapshot("no-late-usage")
            .expect("snapshot");
        assert_eq!(inventory(), before, "reads must not create sidecar files");

        // Imported snapshots predate lifecycle locks. Reading one must not
        // create either its missing accounting directory or a lock leaf.
        let imported = SessionManager::new(tmp.path().join("imported")).expect("imported store");
        write_atomic(
            &imported
                .validated_session_path(&saved.metadata.id)
                .expect("imported path"),
            serialize_saved_session(&saved)
                .expect("snapshot bytes")
                .as_bytes(),
        )
        .expect("import snapshot");
        let imported_directory = imported.sessions_dir().join(LATE_USAGE_DIR);
        imported.list_sessions().expect("imported list");
        imported
            .load_session_by_prefix("no-late")
            .expect("imported resume");
        assert!(
            !imported_directory.exists(),
            "reads must not create the sidecar directory"
        );

        fs::create_dir(&imported_directory).expect("empty accounting directory");
        imported
            .load_session_snapshot("no-late-usage")
            .expect("imported snapshot");
        assert_eq!(
            fs::read_dir(imported_directory).expect("directory").count(),
            0
        );
    }

    #[test]
    fn late_usage_projection_failure_preserves_recovery_and_is_idempotent() {
        for malformed in ["json", "oversized", "directory", "tombstone"] {
            let tmp = tempdir().expect("tempdir");
            let manager = SessionManager::new(tmp.path().join("sessions")).expect("manager");
            let affected = save_late_usage_test_session(&manager, "affected-session");
            save_late_usage_test_session(&manager, "healthy-session");
            let (ledger, _) = manager
                .ensure_late_usage_paths("affected-session")
                .expect("paths");
            match malformed {
                "json" => fs::write(&ledger, b"{invalid accounting").expect("malformed ledger"),
                "oversized" => fs::File::create(&ledger)
                    .expect("file")
                    .set_len(MAX_LATE_USAGE_LEDGER_BYTES + 1)
                    .expect("oversized ledger"),
                "directory" => fs::create_dir(&ledger).expect("special ledger"),
                "tombstone" => fs::write(ledger.with_extension("deleted"), b"invalid marker")
                    .expect("malformed tombstone"),
                _ => unreachable!(),
            }

            let listed = manager
                .list_sessions()
                .expect("list survives sidecar failure");
            assert_eq!(listed.len(), 2);
            let bad = listed
                .iter()
                .find(|session| session.id == affected.metadata.id)
                .expect("affected");
            assert!(
                bad.cost
                    .unpriced_reasons
                    .contains(LATE_USAGE_UNAVAILABLE_REASON)
            );
            assert_eq!(bad.cost.unpriced_turns, 1);
            let good = manager
                .load_session_by_prefix("healthy")
                .expect("unaffected resume");
            assert_eq!(good.metadata.cost.unpriced_turns, 0);

            let mut restored = manager
                .load_session_by_prefix("affected")
                .expect("affected recovery");
            assert_eq!(restored.messages, affected.messages);
            manager.apply_late_usage_to_metadata(&mut restored.metadata);
            assert_eq!(restored.metadata.cost.unpriced_turns, 1);
            assert_eq!(restored.metadata.cost.cny_unpriced_turns, 1);
            assert_eq!(restored.metadata.cost.usage_source_fingerprints.len(), 1);
            if malformed == "tombstone" {
                assert!(
                    manager.save_session(&restored).is_err(),
                    "an invalid deletion marker must fail closed for writes"
                );
                fs::remove_file(ledger.with_extension("deleted"))
                    .expect("repair malformed deletion marker");
            }
            manager
                .save_session(&restored)
                .expect("save recovered transcript");
            let again = manager
                .load_session_snapshot("affected-session")
                .expect("repeat recovery");
            assert_eq!(again.metadata.cost.unpriced_turns, 1);
            assert_eq!(again.metadata.cost.cny_unpriced_turns, 1);
            assert_eq!(
                again.metadata.total_tokens, 0,
                "unsafe accounting must not be used"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn linked_late_usage_directory_does_not_block_transcripts_or_touch_target() {
        use std::os::unix::fs::{PermissionsExt as _, symlink};
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("manager");
        save_late_usage_test_session(&manager, "linked-directory");
        let outside = tmp.path().join("outside");
        fs::create_dir(&outside).expect("outside directory");
        fs::set_permissions(&outside, fs::Permissions::from_mode(0o755))
            .expect("outside permissions");
        fs::rename(
            manager.sessions_dir().join(LATE_USAGE_DIR),
            tmp.path().join("original-accounting"),
        )
        .expect("park original accounting directory");
        symlink(&outside, manager.sessions_dir().join(LATE_USAGE_DIR)).expect("linked store");
        let recovered = manager
            .load_session_snapshot("linked-directory")
            .expect("transcript");
        assert!(
            recovered
                .metadata
                .cost
                .unpriced_reasons
                .contains(LATE_USAGE_UNAVAILABLE_REASON)
        );
        assert_eq!(manager.list_sessions().expect("listing").len(), 1);
        assert!(
            manager
                .persist_late_runtime_usage(
                    "linked-directory",
                    "turn",
                    &late_usage_test_record("source")
                )
                .is_err()
        );
        assert_eq!(fs::read_dir(&outside).expect("outside contents").count(), 0);
        assert_eq!(
            fs::metadata(outside)
                .expect("outside metadata")
                .permissions()
                .mode()
                & 0o777,
            0o755
        );
    }

    #[test]
    fn deleting_session_retires_late_usage_and_keeps_one_lock_inode() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("manager");
        save_late_usage_test_session(&manager, "deleted-session");
        let record = late_usage_test_record("before-deletion");
        manager
            .persist_late_runtime_usage("deleted-session", "turn", &record)
            .expect("append");
        let (ledger, lock_path) = manager.late_usage_paths("deleted-session").expect("paths");
        let mut old_lock =
            fd_lock::RwLock::new(open_private_lock_file(&lock_path).expect("captured lock"));

        manager.delete_session("deleted-session").expect("delete");
        assert!(!ledger.exists());
        assert!(
            !manager
                .validated_session_path("deleted-session")
                .expect("session path")
                .exists()
        );
        assert!(SessionManager::late_usage_is_deleted(&ledger).expect("tombstone"));
        assert!(manager.load_late_usage("deleted-session").is_err());
        assert!(
            manager
                .persist_late_runtime_usage("deleted-session", "turn", &record)
                .expect("retired replay")
        );
        assert!(
            !ledger.exists(),
            "late callback must not resurrect accounting"
        );
        manager
            .delete_session("deleted-session")
            .expect("idempotent cleanup retry");

        let mut new_lock =
            fd_lock::RwLock::new(open_private_lock_file(&lock_path).expect("current lock"));
        let _held = old_lock.write().expect("old handle still owns the lock");
        assert!(
            matches!(new_lock.try_write(), Err(error) if error.kind() == io::ErrorKind::WouldBlock),
            "deletion must not replace or unlink a held lock inode"
        );
    }

    #[test]
    fn lifecycle_admission_holds_delete_lock_and_rejects_retired_origin() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("manager");
        save_late_usage_test_session(&manager, "active-origin");
        let (_, lock_path) = manager.late_usage_paths("active-origin").expect("paths");
        let mut competing_lock =
            fd_lock::RwLock::new(open_private_lock_file(&lock_path).expect("competing lock"));
        assert_eq!(
            manager
                .with_live_session_origin("active-origin", || {
                    assert!(
                        matches!(competing_lock.try_write(), Err(error) if error.kind() == io::ErrorKind::WouldBlock),
                        "active acceptance must hold the deletion lock"
                    );
                    true
                })
                .expect("active acceptance"),
            Some(true)
        );
        assert_eq!(
            manager
                .with_live_session_origin("active-origin", || false)
                .expect("stale scope falls through"),
            Some(false)
        );
        drop(competing_lock.write().expect("admission releases the lock"));

        manager.delete_session("active-origin").expect("delete");
        let mut ran_after_delete = false;
        assert_eq!(
            manager
                .with_live_session_origin("active-origin", || {
                    ran_after_delete = true;
                    true
                })
                .expect("retired origin"),
            None
        );
        assert!(!ran_after_delete, "retired scopes cannot accept new usage");
    }

    #[test]
    fn deleted_session_rejects_stale_snapshot_and_checkpoint_saves() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("manager");
        let stale = save_late_usage_test_session(&manager, "stale-writer");
        manager.delete_session("stale-writer").expect("delete");
        for error in [
            manager.save_session(&stale).expect_err("reject stale save"),
            manager
                .save_checkpoint(&stale)
                .expect_err("reject stale checkpoint"),
        ] {
            assert_eq!(error.kind(), io::ErrorKind::NotFound);
        }
        assert!(manager.list_sessions().expect("list").is_empty());
        assert!(
            !manager
                .validated_checkpoint_path("stale-writer")
                .expect("checkpoint path")
                .exists(),
            "a retired writer must not recreate crash-recovery data"
        );
    }

    #[test]
    fn explicit_delete_removes_owned_recovery_checkpoints_only() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("manager");
        let retired = save_late_usage_test_session(&manager, "retired-recovery");
        let retained = save_late_usage_test_session(&manager, "retained-recovery");
        manager.save_checkpoint(&retired).expect("owned checkpoint");
        manager
            .save_checkpoint(&retained)
            .expect("other checkpoint");
        let legacy_path = manager.checkpoints_dir().join(LEGACY_CHECKPOINT_FILE);
        write_atomic(
            &legacy_path,
            serialize_saved_session(&retired)
                .expect("legacy bytes")
                .as_bytes(),
        )
        .expect("owned legacy checkpoint");

        manager.delete_session("retired-recovery").expect("delete");
        assert!(manager.load_legacy_checkpoint().expect("legacy").is_none());
        assert!(
            manager
                .load_session_checkpoint("retired-recovery")
                .expect("owned checkpoint")
                .is_none()
        );
        let checkpoints = manager.list_checkpoints().expect("checkpoint picker");
        assert_eq!(checkpoints.len(), 1);
        assert!(matches!(
            &checkpoints[0].source,
            CheckpointSource::Session(id) if id == "retained-recovery"
        ));
        assert!(
            manager
                .load_session_checkpoint("retained-recovery")
                .expect("other recovery")
                .is_some()
        );

        // An origin can exist only as crash recovery, with no ordinary
        // snapshot. Explicit deletion must still be able to retire it.
        fs::remove_file(
            manager
                .validated_session_path("retained-recovery")
                .expect("ordinary snapshot path"),
        )
        .expect("simulate checkpoint-only origin");
        manager
            .delete_session("retained-recovery")
            .expect("delete recovery-only origin");
        assert!(
            manager
                .list_checkpoints()
                .expect("checkpoint picker")
                .is_empty()
        );
        assert!(manager.save_checkpoint(&retained).is_err());
    }

    #[test]
    fn retention_preserves_checkpoint_origin_receipts_and_evidence() {
        for retention in ["age", "size"] {
            for checkpoint_kind in ["session", "legacy"] {
                let tmp = tempdir().expect("tempdir");
                let manager = SessionManager::new(tmp.path().join("sessions")).expect("manager");
                let id = "55555555-5555-4555-8555-555555555555";
                let mut old = save_late_usage_test_session(&manager, id);
                old.metadata.updated_at = Utc::now() - chrono::Duration::days(60);
                manager.save_session(&old).expect("old snapshot");
                let evidence = manager.sessions_dir().join(id).join("artifacts");
                fs::create_dir_all(&evidence).expect("recovery evidence");
                fs::write(evidence.join("receipt.txt"), b"recoverable evidence").expect("receipt");
                if checkpoint_kind == "session" {
                    manager.save_checkpoint(&old).expect("recovery checkpoint");
                } else {
                    fs::create_dir_all(manager.checkpoints_dir()).expect("checkpoints");
                    write_atomic(
                        &manager.checkpoints_dir().join(LEGACY_CHECKPOINT_FILE),
                        serialize_saved_session(&old)
                            .expect("legacy bytes")
                            .as_bytes(),
                    )
                    .expect("legacy recovery checkpoint");
                }
                manager
                    .persist_late_runtime_usage(id, "turn", &late_usage_test_record("before-prune"))
                    .expect("origin accounting");
                if retention == "age" {
                    assert_eq!(
                        manager
                            .prune_sessions_older_than(std::time::Duration::from_secs(24 * 3600))
                            .expect("age prune"),
                        1
                    );
                } else {
                    for index in 0..MAX_SESSIONS {
                        write_session_with_updated_at(
                            &manager,
                            &format!("fresh-{index}"),
                            Utc::now(),
                        );
                    }
                    manager.cleanup_old_sessions().expect("size cleanup");
                    assert_eq!(
                        manager.list_sessions().expect("sessions").len(),
                        MAX_SESSIONS
                    );
                }
                assert!(
                    !manager
                        .validated_session_path(id)
                        .expect("snapshot path")
                        .exists()
                );
                let (ledger, _) = manager.late_usage_paths(id).expect("ledger paths");
                assert!(!SessionManager::late_usage_is_deleted(&ledger).expect("origin retained"));
                assert!(ledger.exists(), "recovery must retain accounting");
                assert!(
                    evidence.join("receipt.txt").exists(),
                    "recovery must retain evidence"
                );
                assert_eq!(
                    manager
                        .with_live_session_origin(id, || true)
                        .expect("resume admission"),
                    Some(true)
                );
                let mut recovered = if checkpoint_kind == "session" {
                    manager
                        .load_session_checkpoint(id)
                        .expect("checkpoint read")
                } else {
                    manager.load_legacy_checkpoint().expect("legacy read")
                }
                .expect("retained recovery");
                assert_eq!(
                    recovered.metadata.total_tokens, 1,
                    "checkpoint overlays exact origin usage"
                );
                recovered.metadata.updated_at = Utc::now();
                manager
                    .save_session(&recovered)
                    .expect("save resumed origin");
                assert_eq!(
                    manager
                        .load_session_snapshot(id)
                        .expect("resumed snapshot")
                        .metadata
                        .total_tokens,
                    1,
                    "replayed recovery accounting remains idempotent"
                );
            }
        }
    }

    #[test]
    fn interrupted_session_deletion_keeps_recovery_incomplete_and_can_finish() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("manager");
        let saved = save_late_usage_test_session(&manager, "interrupted-delete");
        manager
            .save_checkpoint(&saved)
            .expect("checkpoint before deletion");
        write_atomic(
            &manager.checkpoints_dir().join(LEGACY_CHECKPOINT_FILE),
            serialize_saved_session(&saved)
                .expect("legacy bytes")
                .as_bytes(),
        )
        .expect("legacy checkpoint before deletion");
        let (ledger, _) = manager
            .ensure_late_usage_paths("interrupted-delete")
            .expect("paths");
        write_atomic(&ledger.with_extension("deleted"), LATE_USAGE_DELETED)
            .expect("crash after tombstone");
        let recovered = manager
            .load_session_snapshot("interrupted-delete")
            .expect("transcript remains recoverable");
        assert!(
            recovered
                .metadata
                .cost
                .unpriced_reasons
                .contains(LATE_USAGE_UNAVAILABLE_REASON)
        );
        assert!(
            manager
                .load_session_checkpoint("interrupted-delete")
                .expect("checkpoint read")
                .is_none(),
            "a checkpoint retired before a crash must not be offered for recovery"
        );
        assert!(
            manager
                .load_legacy_checkpoint()
                .expect("legacy read")
                .is_none()
        );
        manager
            .delete_session("interrupted-delete")
            .expect("finish deletion");
        assert!(manager.list_sessions().expect("list").is_empty());
        assert!(manager.list_checkpoints().expect("checkpoints").is_empty());
    }

    #[test]
    #[ignore = "subprocess helper for the late usage deletion regression"]
    fn late_usage_callback_subprocess() {
        let directory = PathBuf::from(
            std::env::var_os("CODEWHALE_LATE_USAGE_TEST_DIR").expect("fixture directory"),
        );
        let manager = SessionManager::new(directory.join("sessions")).expect("manager");
        manager
            .persist_late_runtime_usage(
                "process-delete-race",
                "turn",
                &late_usage_test_record("first-callback"),
            )
            .expect("first callback");
        fs::write(directory.join("ready"), b"ready").expect("signal ready");
        let started = std::time::Instant::now();
        while !directory.join("continue").exists() {
            assert!(
                started.elapsed() < std::time::Duration::from_secs(10),
                "callback gate timed out"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(
            manager
                .persist_late_runtime_usage(
                    "process-delete-race",
                    "turn",
                    &late_usage_test_record("late-callback")
                )
                .expect("retired callback")
        );
    }

    #[test]
    fn late_usage_callback_in_another_process_cannot_resurrect_deleted_session() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("manager");
        save_late_usage_test_session(&manager, "process-delete-race");
        let mut child =
            std::process::Command::new(std::env::current_exe().expect("test executable"))
                .args([
                    "--exact",
                    "session_manager::tests::late_usage_callback_subprocess",
                    "--ignored",
                ])
                .env("CODEWHALE_LATE_USAGE_TEST_DIR", tmp.path())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .expect("callback process");
        let started = std::time::Instant::now();
        while !tmp.path().join("ready").exists() {
            if started.elapsed() >= std::time::Duration::from_secs(10)
                || child.try_wait().expect("child status").is_some()
            {
                let _ = child.kill();
                let _ = child.wait();
                panic!("callback process did not reach the deletion gate");
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(
            manager
                .load_session_snapshot("process-delete-race")
                .expect("first callback persisted")
                .metadata
                .total_tokens,
            1
        );
        let deleted = manager.delete_session("process-delete-race");
        fs::write(tmp.path().join("continue"), b"continue").expect("release callback");
        let status = loop {
            if let Some(status) = child.try_wait().expect("child status") {
                break status;
            }
            if started.elapsed() >= std::time::Duration::from_secs(10) {
                let _ = child.kill();
                let _ = child.wait();
                panic!("late callback process did not finish");
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        deleted.expect("delete while callback process was pending");
        assert!(status.success(), "callback process failed");
        let (ledger, _) = manager
            .late_usage_paths("process-delete-race")
            .expect("paths");
        assert!(!ledger.exists());
        assert!(manager.list_sessions().expect("list").is_empty());
    }

    #[test]
    fn late_usage_sidecar_survives_stale_session_save_and_replays_once() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("manager");
        let old_id = "old-session";
        let new_id = "new-session";
        let old = create_saved_session_with_id_and_mode(
            old_id.to_string(),
            &[make_test_message("user", "old session")],
            "deepseek-v4-flash",
            tmp.path(),
            0,
            None,
            Some("agent"),
        );
        let new = create_saved_session_with_id_and_mode(
            new_id.to_string(),
            &[make_test_message("user", "new session")],
            "deepseek-v4-flash",
            tmp.path(),
            0,
            None,
            Some("agent"),
        );
        manager.save_session(&old).expect("save old");
        manager.save_session(&new).expect("save new");

        let priced_route = crate::cost_status::EffectiveRouteEnvelope::capture(
            None,
            ApiProvider::Deepseek,
            "deepseek",
            "deepseek-v4-flash",
            Some(crate::config::DEFAULT_DEEPSEEK_BASE_URL),
            Utc::now(),
        );
        let usage = codewhale_models::Usage {
            input_tokens: 17,
            output_tokens: 5,
            ..codewhale_models::Usage::default()
        };
        let usage_record = crate::cost_status::RuntimeUsageRecord {
            source_id: "translation:old-turn:assistant:1".to_string(),
            usage: crate::cost_status::EffectiveRouteUsage {
                route: priced_route.clone(),
                usage: usage.clone(),
            },
        };
        let missing_record = crate::cost_status::RuntimeUsageDropRecord {
            source_id: "advisor:old-turn:provider-response:0".to_string(),
            route: priced_route,
        };
        let mut subscription_route = missing_record.route.clone();
        subscription_route.billing_mode = crate::cost_status::RouteBillingMode::Subscription;
        let subscription_missing = crate::cost_status::RuntimeUsageDropRecord {
            source_id: "translation:old-turn:thinking:2".to_string(),
            route: subscription_route,
        };

        for _ in 0..2 {
            assert!(
                manager
                    .persist_late_runtime_usage(old_id, "old-turn", &usage_record)
                    .expect("persist late usage")
            );
            assert!(
                manager
                    .persist_late_runtime_drop(old_id, "old-turn", &missing_record)
                    .expect("persist missing usage")
            );
            assert!(
                manager
                    .persist_late_runtime_drop(old_id, "old-turn", &subscription_missing)
                    .expect("persist subscription missing usage")
            );
        }

        // A concurrent stale whole-session writer cannot erase the independent
        // origin ledger. Loading overlays it once by stable response identity.
        manager.save_session(&old).expect("stale old-session save");
        let first = manager.load_session_snapshot(old_id).expect("load old");
        let second = manager.load_session_snapshot(old_id).expect("replay old");
        for loaded in [&first, &second] {
            assert_eq!(loaded.metadata.total_tokens, 22);
            assert_eq!(loaded.metadata.cost.unpriced_turns, 1);
            assert_eq!(loaded.metadata.cost.cny_unpriced_turns, 1);
            assert_eq!(loaded.metadata.cost.usage_source_fingerprints.len(), 3);
            assert!(
                loaded
                    .metadata
                    .cost
                    .unpriced_reasons
                    .contains("provider_success_missing_usage")
            );
        }
        assert_eq!(first.metadata.cost.priced_turns, 1);

        let clean = manager.load_session_snapshot(new_id).expect("load new");
        assert_eq!(clean.metadata.total_tokens, 0);
        assert_eq!(clean.metadata.cost.priced_turns, 0);
        assert_eq!(clean.metadata.cost.unpriced_turns, 0);
        assert!(clean.metadata.cost.usage_source_fingerprints.is_empty());

        let ledger = fs::read_to_string(
            manager
                .sessions_dir()
                .join(LATE_USAGE_DIR)
                .join(format!("{old_id}.json")),
        )
        .expect("late ledger");
        assert!(!ledger.contains("translation:old-turn"));
        assert!(!ledger.contains(crate::config::DEFAULT_DEEPSEEK_BASE_URL));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let ledger_dir = manager.sessions_dir().join(LATE_USAGE_DIR);
            assert_eq!(
                fs::metadata(&ledger_dir)
                    .expect("private sidecar directory")
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
            for path in [
                ledger_dir.join(format!("{old_id}.json")),
                ledger_dir.join(format!("{old_id}.lock")),
            ] {
                assert_eq!(
                    fs::metadata(path)
                        .expect("private sidecar metadata")
                        .permissions()
                        .mode()
                        & 0o777,
                    0o600
                );
            }
        }
    }

    #[test]
    fn late_usage_sidecar_has_a_bounded_fail_closed_overflow() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("manager");
        let session_id = "bounded-session";
        let session = create_saved_session_with_id_and_mode(
            session_id.to_string(),
            &[make_test_message("user", "bounded session")],
            "local-model",
            tmp.path(),
            0,
            None,
            Some("agent"),
        );
        manager.save_session(&session).expect("save bounded");
        let mut route = crate::cost_status::EffectiveRouteEnvelope::capture(
            None,
            ApiProvider::Custom,
            "local-provider",
            "local-model",
            Some("http://127.0.0.1:11434/v1"),
            Utc::now(),
        );
        route.billing_mode = crate::cost_status::RouteBillingMode::Local;
        for index in 0..=MAX_LATE_USAGE_RECORDS_PER_SESSION {
            manager
                .persist_late_runtime_usage(
                    session_id,
                    "bounded-turn",
                    &crate::cost_status::RuntimeUsageRecord {
                        source_id: format!("late-bounded:{index}"),
                        usage: crate::cost_status::EffectiveRouteUsage {
                            route: route.clone(),
                            usage: codewhale_models::Usage {
                                input_tokens: 1,
                                ..codewhale_models::Usage::default()
                            },
                        },
                    },
                )
                .expect("bounded append");
        }

        let loaded = manager
            .load_session_snapshot(session_id)
            .expect("load bounded");
        assert_eq!(
            loaded.metadata.total_tokens,
            u64::try_from(MAX_LATE_USAGE_RECORDS_PER_SESSION).unwrap_or(u64::MAX)
        );
        assert_eq!(loaded.metadata.cost.unpriced_turns, 1);
        assert!(
            loaded
                .metadata
                .cost
                .unpriced_reasons
                .contains("late_usage_ledger_overflow")
        );
        let ledger = manager.load_late_usage(session_id).expect("bounded ledger");
        assert_eq!(ledger.records.len(), MAX_LATE_USAGE_RECORDS_PER_SESSION);
        assert!(ledger.overflowed);
    }

    #[cfg(unix)]
    #[test]
    fn late_usage_sidecar_rejects_linked_lock_and_ledger_leaves() {
        use std::os::unix::fs::symlink;

        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("manager");
        let session_id = "linked-sidecar-session";
        save_late_usage_test_session(&manager, session_id);
        save_late_usage_test_session(&manager, "unaffected-sidecar-session");
        let (ledger_path, lock_path) = manager.ensure_late_usage_paths(session_id).expect("paths");
        let route = crate::cost_status::EffectiveRouteEnvelope::capture(
            None,
            ApiProvider::Deepseek,
            "deepseek",
            "deepseek-v4-flash",
            Some(crate::config::DEFAULT_DEEPSEEK_BASE_URL),
            Utc::now(),
        );
        let record = crate::cost_status::RuntimeUsageRecord {
            source_id: "linked-sidecar-response".to_string(),
            usage: crate::cost_status::EffectiveRouteUsage {
                route,
                usage: codewhale_models::Usage {
                    input_tokens: 1,
                    ..codewhale_models::Usage::default()
                },
            },
        };

        let outside_lock = tmp.path().join("outside.lock");
        fs::write(&outside_lock, b"outside-lock").expect("outside lock");
        fs::remove_file(&lock_path).expect("replace fixture lifecycle lock");
        symlink(&outside_lock, &lock_path).expect("symlink lock");
        assert!(
            manager
                .persist_late_runtime_usage(session_id, "turn", &record)
                .is_err(),
            "a symlink lock leaf must fail closed"
        );
        assert_eq!(
            fs::read(&outside_lock).expect("outside lock unchanged"),
            b"outside-lock"
        );
        fs::remove_file(&lock_path).expect("remove lock symlink");

        fs::hard_link(&outside_lock, &lock_path).expect("hard-linked lock");
        assert!(
            manager
                .persist_late_runtime_usage(session_id, "turn", &record)
                .is_err(),
            "a multiply linked lock leaf must fail closed"
        );
        fs::remove_file(&lock_path).expect("remove hard-linked lock");

        let outside_ledger = tmp.path().join("outside.json");
        fs::write(
            &outside_ledger,
            br#"{"schema_version":1,"records":[],"overflowed":false}"#,
        )
        .expect("outside ledger");
        symlink(&outside_ledger, &ledger_path).expect("symlink ledger");
        assert!(
            manager.load_late_usage(session_id).is_err(),
            "a symlink ledger leaf must fail closed"
        );
        assert!(
            manager
                .load_session_snapshot(session_id)
                .expect("recover linked ledger transcript")
                .metadata
                .cost
                .unpriced_reasons
                .contains(LATE_USAGE_UNAVAILABLE_REASON)
        );
        fs::remove_file(&ledger_path).expect("remove ledger symlink");

        fs::hard_link(&outside_ledger, &ledger_path).expect("hard-linked ledger");
        assert!(
            manager.load_late_usage(session_id).is_err(),
            "a multiply linked ledger leaf must fail closed"
        );
        assert_eq!(
            manager
                .list_sessions()
                .expect("list linked ledger transcript")
                .len(),
            2
        );
        assert_eq!(
            manager
                .load_session_by_prefix("unaffected")
                .expect("unaffected resume")
                .metadata
                .cost
                .unpriced_turns,
            0
        );
        assert_eq!(
            fs::read(&outside_ledger).expect("outside ledger unchanged"),
            br#"{"schema_version":1,"records":[],"overflowed":false}"#
        );
    }

    fn container_with(messages: Vec<Message>, dir: &std::path::Path) -> SessionImportContainer {
        let session = create_saved_session(&messages, "test-model", dir, 100, None);
        session.export_container("test-session.json")
    }

    #[test]
    fn session_goal_sidecar_round_trips_control_state_without_model_output() {
        let tmp = tempdir().expect("tempdir");
        let sessions_dir = tmp.path().join("sessions");
        let manager = SessionManager::new(sessions_dir.clone()).expect("manager");
        let session_id = "11111111-2222-4333-8444-555555555555";
        let runtime = GoalSnapshot {
            goal_id: None,
            objective: Some("finish the provider migration".to_string()),
            status: "paused".to_string(),
            token_budget: Some(50_000),
            tokens_used: 12_345,
            time_used_seconds: 67,
            continuation_count: 4,
            elapsed_seconds: Some(91),
            evidence: Some("Bearer credential-shaped-model-output".to_string()),
            blocker: Some("/arbitrary/private/path".to_string()),
            pause_reason: Some(GoalPauseReason::User),
            completion_verification: None,
            advisories: Vec::new(),
            last_gap_fingerprint: None,
            repeated_gap_count: 0,
            last_gap_pass: None,
            progress: None,
        };
        let durable = SessionGoalState::from_runtime(&runtime)
            .expect("valid runtime goal")
            .expect("non-empty durable goal");

        manager
            .save_session_goal(session_id, Some(&durable))
            .expect("save goal");
        let raw = fs::read_to_string(
            sessions_dir
                .join(SESSION_GOALS_DIR)
                .join(format!("{session_id}.json")),
        )
        .expect("read goal sidecar");
        assert!(!raw.contains("credential-shaped-model-output"));
        assert!(!raw.contains("/arbitrary/private/path"));

        let reopened = SessionManager::new(sessions_dir).expect("reopen manager");
        let restored = reopened
            .load_session_goal(session_id)
            .expect("load goal")
            .expect("persisted goal");
        assert_eq!(restored, durable);
        assert_eq!(restored.to_runtime_snapshot().objective, runtime.objective);
        assert_eq!(restored.to_runtime_snapshot().status, "paused");

        reopened
            .save_session_goal(session_id, None)
            .expect("clear goal");
        assert_eq!(
            reopened.load_session_goal(session_id).expect("load clear"),
            None
        );
    }

    /// Coverage state round-trips with the money it qualifies, and a session
    /// written before coverage existed is detected as *unknown* rather than being
    /// read as a complete total covering zero turns (#4318).
    #[test]
    fn cost_snapshot_round_trips_coverage_and_detects_legacy_unknown() {
        // A pre-coverage row: real money, no coverage fields at all.
        let legacy: SessionCostSnapshot = serde_json::from_value(serde_json::json!({
            "session_cost_usd": 1.25,
            "session_cost_cny": 0.0,
            "subagent_cost_usd": 0.0,
            "subagent_cost_cny": 0.0,
            "displayed_cost_high_water_usd": 1.25,
            "displayed_cost_high_water_cny": 0.0
        }))
        .expect("legacy cost snapshot stays readable");
        assert_eq!(legacy.priced_turns, 0);
        assert_eq!(legacy.unpriced_turns, 0);
        assert!(!legacy.coverage_recorded);
        assert!(
            legacy.coverage_is_legacy_unknown(),
            "a non-zero total with no coverage evidence must not read as complete"
        );

        // An all-zero pre-coverage session is still unknown: zero may mean no
        // turns, all unpriced turns, or exact zero usage. Absence of evidence is
        // never rewritten into a complete 0/0 claim.
        let empty = SessionCostSnapshot::default();
        assert!(empty.coverage_is_legacy_unknown());

        // A coverage-aware writer that recorded zero money-metered turns is also
        // not unknown — it positively knows the answer is zero.
        let recorded_zero = SessionCostSnapshot {
            session_cost_usd: 1.25,
            coverage_recorded: true,
            ..SessionCostSnapshot::default()
        };
        assert!(!recorded_zero.coverage_is_legacy_unknown());

        // Full round-trip of every coverage field.
        let full = SessionCostSnapshot {
            session_cost_usd: 2.5,
            session_cost_cny: 3.0,
            subagent_cost_usd: 0.5,
            subagent_cost_cny: 0.25,
            displayed_cost_high_water_usd: 3.0,
            displayed_cost_high_water_cny: 3.25,
            priced_turns: 7,
            unpriced_turns: 2,
            cny_priced_turns: 1,
            cny_unpriced_turns: 8,
            unpriced_reasons: ["missing_class_price".to_string()].into(),
            cny_unpriced_reasons: ["currency_not_published".to_string()].into(),
            unpriced_classes: ["cache_write".to_string()].into(),
            pricing_provenances: ["models_dev_bundled".to_string()].into(),
            live_pricing_defects: ["live_pricing_stale".to_string()].into(),
            live_pricing_unusable_defects: ["live_pricing_scope_mismatch".to_string()].into(),
            route_receipts: ["provider=anthropic identity=- model=claude-haiku-4-5 \
                 surface=first-party-payg endpoint_fp=abc123 currency=usd"
                .to_string()]
            .into(),
            usage_source_fingerprints: ["response-fingerprint".to_string()].into(),
            coverage_recorded: true,
        };
        let json = serde_json::to_string(&full).expect("serialize");
        let back: SessionCostSnapshot = serde_json::from_str(&json).expect("round-trip");
        assert_eq!(back.priced_turns, 7);
        assert_eq!(back.unpriced_turns, 2);
        assert_eq!(back.cny_priced_turns, 1);
        assert_eq!(back.cny_unpriced_turns, 8);
        assert_eq!(back.unpriced_reasons, full.unpriced_reasons);
        assert_eq!(back.cny_unpriced_reasons, full.cny_unpriced_reasons);
        assert_eq!(back.unpriced_classes, full.unpriced_classes);
        assert_eq!(back.pricing_provenances, full.pricing_provenances);
        assert_eq!(back.live_pricing_defects, full.live_pricing_defects);
        assert_eq!(
            back.usage_source_fingerprints,
            full.usage_source_fingerprints
        );
        assert_eq!(
            back.live_pricing_unusable_defects,
            full.live_pricing_unusable_defects
        );
        assert_eq!(back.route_receipts, full.route_receipts);
        assert!(back.coverage_recorded);
        assert!(!back.coverage_is_legacy_unknown());

        // The persisted receipts carry no endpoint URL or credential.
        let lower = json.to_lowercase();
        for needle in ["http", "api_key", "authorization", "bearer", "sk-"] {
            assert!(!lower.contains(needle), "{needle} leaked into {json}");
        }
    }

    /// The USD and CNY totals a snapshot reports are projections of one
    /// dual-currency accumulation, never two independent sums that could
    /// disagree (#4939).
    ///
    /// For any turn sequence — dual-priced, USD-only, CNY-only, or garbage
    /// estimates — folding the turns jointly and projecting each currency must
    /// equal accumulating that currency on its own. This is the invariant that
    /// makes the persisted per-currency columns safe: they are written from the
    /// same joint fold, so a code path can no longer update one and forget the
    /// other. CNY is derived from provider-published CNY rows, not from an FX
    /// multiple of USD, so a USD-only turn must contribute exactly zero CNY.
    #[test]
    fn cost_snapshot_currency_totals_are_projections_of_one_accumulator() {
        use crate::pricing::CostEstimate;

        let turn_sequences: &[&[CostEstimate]] = &[
            // Dual-priced turns (DeepSeek-style routes with a published CNY row).
            &[
                CostEstimate {
                    usd: 0.01,
                    cny: 0.07,
                },
                CostEstimate {
                    usd: 0.02,
                    cny: 0.14,
                },
            ],
            // USD-only turns: CNY unpublished, so the CNY projection stays zero.
            &[
                CostEstimate {
                    usd: 0.25,
                    cny: 0.0,
                },
                CostEstimate { usd: 1.5, cny: 0.0 },
            ],
            // Mixed: one currency priced per turn, alternating.
            &[
                CostEstimate { usd: 0.5, cny: 0.0 },
                CostEstimate { usd: 0.0, cny: 3.5 },
                CostEstimate {
                    usd: 0.125,
                    cny: 0.875,
                },
            ],
            // Hostile values: sanitization must apply identically per currency.
            &[
                CostEstimate {
                    usd: f64::NAN,
                    cny: 0.25,
                },
                CostEstimate {
                    usd: 0.75,
                    cny: -1.0,
                },
                CostEstimate {
                    usd: f64::INFINITY,
                    cny: 0.25,
                },
            ],
        ];

        for turns in turn_sequences {
            // Joint fold: how the app accumulates (one accumulator, both
            // currencies advance together through the same saturating_add).
            let joint = turns.iter().fold(CostEstimate::default(), |acc, turn| {
                acc.saturating_add(*turn)
            });

            // Independent per-currency folds: what a drifted parallel
            // accumulator would compute if it only saw one currency.
            let usd_alone = turns.iter().fold(CostEstimate::default(), |acc, turn| {
                acc.saturating_add(CostEstimate {
                    usd: turn.usd,
                    cny: 0.0,
                })
            });
            let cny_alone = turns.iter().fold(CostEstimate::default(), |acc, turn| {
                acc.saturating_add(CostEstimate {
                    usd: 0.0,
                    cny: turn.cny,
                })
            });

            let snapshot = SessionCostSnapshot {
                session_cost_usd: joint.usd,
                session_cost_cny: joint.cny,
                ..SessionCostSnapshot::default()
            };
            assert_eq!(
                snapshot.total_usd(),
                usd_alone.usd,
                "USD projection drifted from independent accumulation for {turns:?}"
            );
            assert_eq!(
                snapshot.total_cny(),
                cny_alone.cny,
                "CNY projection drifted from independent accumulation for {turns:?}"
            );
            assert_eq!(snapshot.total_estimate().usd, snapshot.total_usd());
            assert_eq!(snapshot.total_estimate().cny, snapshot.total_cny());
        }

        // A USD-only session projects zero CNY — no fabricated FX conversion —
        // and the subagent column joins the same fold.
        let usd_only = SessionCostSnapshot {
            session_cost_usd: 2.5,
            subagent_cost_usd: 0.5,
            ..SessionCostSnapshot::default()
        };
        assert_eq!(usd_only.total_usd(), 3.0);
        assert_eq!(usd_only.total_cny(), 0.0);
    }

    fn write_session_record(
        manager: &SessionManager,
        id: &str,
        workspace: &Path,
        updated_at: DateTime<Utc>,
    ) {
        let session = SavedSession {
            schema_version: CURRENT_SESSION_SCHEMA_VERSION,
            messages: vec![make_test_message("user", "hi")],
            metadata: SessionMetadata {
                id: id.to_string(),
                title: format!("session-{id}"),
                created_at: updated_at,
                updated_at,
                message_count: 1,
                total_tokens: 0,
                model: "deepseek-v4-flash".to_string(),
                model_provider: "deepseek".to_string(),
                model_provider_id: None,
                workspace: workspace.to_path_buf(),
                mode: None,
                cost: SessionCostSnapshot::default(),
                parent_session_id: None,
                forked_from_message_count: None,
                runtime_store: None,
                cumulative_turn_secs: 0,
                archived: false,
                spawn_depth: 0,
            },
            journal: None,
            leaf_id: None,
            system_prompt: None,
            context_references: Vec::new(),
            artifacts: Vec::new(),
            approval_receipts: Vec::new(),
            work_state: None,
            window_title: None,
            last_auto_route: None,
        };
        manager.save_session(&session).expect("save");
    }

    fn write_empty_session_record(
        manager: &SessionManager,
        id: &str,
        workspace: &Path,
        updated_at: DateTime<Utc>,
    ) {
        let session = SavedSession {
            schema_version: CURRENT_SESSION_SCHEMA_VERSION,
            messages: Vec::new(),
            metadata: SessionMetadata {
                id: id.to_string(),
                title: DEFAULT_SESSION_TITLE.to_string(),
                created_at: updated_at,
                updated_at,
                message_count: 0,
                total_tokens: 0,
                model: "deepseek-v4-pro".to_string(),
                model_provider: "deepseek".to_string(),
                model_provider_id: None,
                workspace: workspace.to_path_buf(),
                mode: Some("yolo".to_string()),
                cost: SessionCostSnapshot::default(),
                parent_session_id: None,
                forked_from_message_count: None,
                runtime_store: None,
                cumulative_turn_secs: 0,
                archived: false,
                spawn_depth: 0,
            },
            journal: None,
            leaf_id: None,
            system_prompt: None,
            context_references: Vec::new(),
            artifacts: Vec::new(),
            approval_receipts: Vec::new(),
            work_state: None,
            window_title: None,
            last_auto_route: None,
        };
        manager.save_session(&session).expect("save empty");
    }

    // === session retention and independent runtime data ===

    #[test]
    fn cleanup_preserves_artifacts_without_a_session_snapshot() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().to_path_buf()).expect("manager");
        let workspace = tmp.path().join("ws");

        let orphan = "11111111-1111-4111-8111-111111111111";
        let live = "22222222-2222-4222-8222-222222222222";
        for id in [orphan, live] {
            let artifacts = tmp.path().join(id).join("artifacts");
            fs::create_dir_all(&artifacts).expect("artifact dir");
            fs::write(artifacts.join("art_evidence.txt"), b"stdout").expect("artifact");
        }
        // Only `live` still has a session document.
        write_session_record(&manager, live, &workspace, Utc::now());

        manager.cleanup_old_sessions().expect("cleanup");

        assert!(
            tmp.path()
                .join(orphan)
                .join("artifacts/art_evidence.txt")
                .exists(),
            "an absent snapshot does not authorize deleting independent evidence"
        );
        assert!(
            tmp.path().join(live).join("artifacts").exists(),
            "a directory whose session still exists must be left alone"
        );
    }

    #[tokio::test]
    async fn cleanup_in_another_process_preserves_runtime_without_a_snapshot() {
        const PROBE: &str = "CODEWHALE_RUNTIME_RETENTION_PROBE";
        if let Some(directory) = std::env::var_os(PROBE) {
            let manager = SessionManager::new(PathBuf::from(directory)).expect("child manager");
            manager.cleanup_old_sessions().expect("child retention");
            return;
        }
        let tmp = tempdir().expect("tempdir");
        let sessions = tmp.path().join("sessions");
        let manager = SessionManager::new(sessions.clone()).expect("manager");
        let runtime = sessions.join("44444444-4444-4444-8444-444444444444/runtime");
        let store = crate::runtime_threads::RuntimeThreadStore::open(runtime.clone())
            .expect("live automation store");
        let first = store
            .append_event(
                "thread_probe",
                None,
                None,
                "probe",
                serde_json::json!({"step": 1}),
            )
            .await
            .expect("first durable event");
        let run_cleanup = || {
            let output = std::process::Command::new(std::env::current_exe().expect("test executable"))
                .args([
                    "--exact",
                    "session_manager::tests::cleanup_in_another_process_preserves_runtime_without_a_snapshot",
                    "--nocapture",
                ])
                .env(PROBE, &sessions)
                .output()
                .expect("independent cleanup process");
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        };
        assert!(
            manager
                .list_sessions()
                .expect("no interactive snapshot")
                .is_empty()
        );
        run_cleanup();
        assert_eq!(
            store.current_seq().await.expect("live cursor survives"),
            first.seq
        );
        drop(store);
        // A closed store can still own resumable events. It is not garbage
        // simply because no process or interactive transcript claims it.
        run_cleanup();
        let reopened = crate::runtime_threads::RuntimeThreadStore::open(runtime)
            .expect("reopen preserved automation store");
        assert_eq!(
            reopened.current_seq().await.expect("recovered cursor"),
            first.seq
        );
        let next = reopened
            .append_event(
                "thread_probe",
                None,
                None,
                "probe",
                serde_json::json!({"step": 2}),
            )
            .await
            .expect("continue recovered event sequence");
        assert_eq!(next.seq, first.seq + 1);
    }

    #[test]
    fn a_crashed_sessions_evidence_survives_even_without_its_document() {
        // Recovery reads exactly this: a checkpoint with no session document.
        // Reclaiming its evidence would delete what recovery needs.
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().to_path_buf()).expect("manager");
        let crashed = "33333333-3333-4333-8333-333333333333";

        fs::create_dir_all(tmp.path().join(crashed).join("artifacts")).expect("artifacts");
        let checkpoints = tmp.path().join("checkpoints");
        fs::create_dir_all(&checkpoints).expect("checkpoints dir");
        fs::write(checkpoints.join(format!("{crashed}.json")), b"{}").expect("checkpoint");

        manager.cleanup_old_sessions().expect("cleanup");

        assert!(
            tmp.path().join(crashed).exists(),
            "a crashed session's evidence must outlive its missing document"
        );
    }

    #[test]
    fn reclamation_never_touches_bookkeeping_directories() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().to_path_buf()).expect("manager");
        // `checkpoints` is not a session id and must survive being empty.
        let checkpoints = tmp.path().join("checkpoints");
        fs::create_dir_all(&checkpoints).expect("checkpoints dir");
        let not_a_session = tmp.path().join("some-user-folder");
        fs::create_dir_all(&not_a_session).expect("user dir");

        manager.cleanup_old_sessions().expect("cleanup");

        assert!(checkpoints.exists(), "checkpoints/ is not a session dir");
        assert!(
            not_a_session.exists(),
            "a name that is not a valid session id is not ours to remove"
        );
    }

    #[test]
    fn save_and_resume_reconstructs_closed_and_interrupted_approvals() {
        let tmp = tempdir().expect("tempdir");
        let sessions_dir = tmp.path().join("sessions");
        let manager = SessionManager::new(sessions_dir.clone()).expect("manager");
        let session = create_saved_session(
            &[make_test_message("user", "approval recovery")],
            "test-model",
            tmp.path(),
            0,
            None,
        );
        let session_id = session.metadata.id.clone();
        let store = ApprovalReceiptStore::new(sessions_dir);
        store
            .append(
                &session_id,
                &ApprovalReceipt::asked("tool-complete", "exec_shell"),
            )
            .expect("persist completed ask");
        store
            .append(
                &session_id,
                &ApprovalReceipt::decided("tool-complete", ApprovalOutcome::Denied),
            )
            .expect("persist completed decision");
        store
            .append(
                &session_id,
                &ApprovalReceipt::asked("tool-interrupted", "write_file"),
            )
            .expect("persist interrupted ask");

        manager.save_session(&session).expect("save session");
        let resumed = manager
            .load_session_snapshot(&session_id)
            .expect("resume session");
        let replay = ApprovalReplay::from_receipts(&resumed.approval_receipts)
            .expect("replay resumed approval evidence");

        assert_eq!(resumed.messages, session.messages);
        assert_eq!(replay.completed.len(), 1);
        assert_eq!(replay.completed[0].outcome, ApprovalOutcome::Denied);
        assert_eq!(replay.unmatched_asks.len(), 1);
        assert!(matches!(
            &replay.unmatched_asks[0],
            ApprovalReceipt::Asked { tool_call_id, .. } if tool_call_id == "tool-interrupted"
        ));
        assert_eq!(
            manager
                .replay_approvals(&session_id)
                .expect("replay canonical sidecar"),
            replay
        );
    }

    #[test]
    fn session_boot_owner_stamps_only_the_creating_instance() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().to_path_buf()).expect("manager");
        let workspace = tmp.path().join("ws");

        // A record this instance creates is stamped with this boot id and is
        // therefore not prior-instance work.
        write_session_record(&manager, "mine", &workspace, Utc::now());
        assert_eq!(
            manager.session_boot_owner("mine").as_deref(),
            Some(current_session_boot_id())
        );
        assert!(!manager.session_from_prior_instance("mine"));

        // An id with no durable record at all is this instance's own
        // not-yet-persisted session.
        assert!(!manager.session_from_prior_instance("unsaved"));

        // A record stamped by another boot id stays owned by that instance,
        // even after this instance re-serializes it (crash recovery must not
        // re-badge restored work as ours).
        manager
            .record_session_boot_owner("theirs", "boot_other_instance")
            .expect("stamp");
        write_session_record(&manager, "theirs", &workspace, Utc::now());
        assert_eq!(
            manager.session_boot_owner("theirs").as_deref(),
            Some("boot_other_instance")
        );
        assert!(manager.session_from_prior_instance("theirs"));

        // A legacy record with no marker is classified as prior-instance
        // work, and a later re-save keeps it unclaimed.
        write_session_record(&manager, "legacy", &workspace, Utc::now());
        manager.clear_session_boot_owner("legacy");
        assert!(manager.session_from_prior_instance("legacy"));
        write_session_record(&manager, "legacy", &workspace, Utc::now());
        assert!(manager.session_from_prior_instance("legacy"));

        // Deleting the record drops its marker.
        manager.delete_session("theirs").expect("delete");
        assert_eq!(manager.session_boot_owner("theirs"), None);
    }

    #[test]
    fn session_boot_owner_sidecar_never_lists_as_a_session() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().to_path_buf()).expect("manager");
        write_session_record(&manager, "real", &tmp.path().join("ws"), Utc::now());
        assert!(manager.session_boot_owners_path().exists());
        let listed = manager.list_sessions().expect("list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, "real");
        // The reserved stem cannot be claimed as a session id either.
        assert!(manager.load_session("session_boot_owners").is_err());
    }

    #[test]
    fn test_session_manager_new() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("new");
        assert!(tmp.path().join("sessions").exists());
        let _ = manager;
    }

    #[test]
    fn test_save_and_load_session() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("new");

        let messages = vec![
            make_test_message("user", "Hello!"),
            make_test_message("assistant", "Hi there!"),
        ];

        let session = create_saved_session(&messages, "test-model", tmp.path(), 100, None);
        let session_id = session.metadata.id.clone();

        manager.save_session(&session).expect("save");

        let loaded = manager.load_session(&session_id).expect("load");
        assert_eq!(loaded.metadata.id, session_id);
        assert_eq!(loaded.messages.len(), 2);
    }

    /// #4681: reopening a session must not surface `<turn_meta>` machine
    /// blocks in the transcript. Covers the current trailing shape and the
    /// legacy leading shape (sessions saved before the turn-meta tail move),
    /// while the loaded API history keeps both envelopes intact for replay.
    #[test]
    fn rehydrated_turn_meta_blocks_never_render_in_history_cells() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("new");

        let turn_meta = "<turn_meta>\nCurrent local date: 2026-08-01\n</turn_meta>";
        let trailing_shape = Message {
            role: Role::User,
            content: vec![
                ContentBlock::Text {
                    text: "Fix the flaky test".to_string(),
                    cache_control: None,
                },
                ContentBlock::Text {
                    text: turn_meta.to_string(),
                    cache_control: None,
                },
            ],
        };
        let legacy_leading_shape = Message {
            role: Role::User,
            content: vec![
                ContentBlock::Text {
                    text: turn_meta.to_string(),
                    cache_control: None,
                },
                ContentBlock::Text {
                    text: "Now add the docs".to_string(),
                    cache_control: None,
                },
            ],
        };
        let messages = vec![
            trailing_shape,
            make_test_message("assistant", "Done."),
            legacy_leading_shape,
        ];
        let session = create_saved_session(&messages, "test-model", tmp.path(), 100, None);
        let session_id = session.metadata.id.clone();
        manager.save_session(&session).expect("save");

        let loaded = manager.load_session(&session_id).expect("load");

        // Display path: no rendered cell may carry turn_meta markup.
        let rendered: Vec<HistoryCell> = loaded
            .messages
            .iter()
            .flat_map(history_cells_from_message)
            .collect();
        let user_texts: Vec<&str> = rendered
            .iter()
            .filter_map(|cell| match cell {
                HistoryCell::User { content } => Some(content.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(user_texts, vec!["Fix the flaky test", "Now add the docs"]);
        assert!(
            !user_texts.iter().any(|text| text.contains("<turn_meta")),
            "rendered cells must not contain turn_meta markup: {user_texts:?}"
        );

        // Model-facing replay: the persisted envelopes survive the round trip.
        let replayed_envelopes = loaded
            .messages
            .iter()
            .flat_map(|message| &message.content)
            .filter(|block| {
                matches!(block, ContentBlock::Text { text, .. } if text.contains("<turn_meta>"))
            })
            .count();
        assert_eq!(replayed_envelopes, 2);
    }

    #[test]
    fn runtime_snapshot_load_preserves_in_flight_tool_call() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("new");
        let messages = vec![Message {
            role: Role::Assistant,
            content: vec![ContentBlock::ToolUse {
                id: "call-in-flight".to_string(),
                name: "read_file".to_string(),
                input: serde_json::json!({"path": "README.md"}),
                caller: None,
                thought_signature: None,
            }],
        }];
        let session = create_saved_session(&messages, "test-model", tmp.path(), 0, None);
        let session_id = session.metadata.id.clone();
        manager.save_session(&session).expect("save");

        let loaded = manager
            .load_session_snapshot(&session_id)
            .expect("snapshot load");

        assert_eq!(loaded.messages, messages);
        assert_eq!(loaded.metadata.message_count, 1);
        assert!(!loaded.messages.iter().any(|message| {
            message.content.iter().any(|block| {
                matches!(
                    block,
                    ContentBlock::ToolResult { content, .. }
                        if content.contains("crashed_and_repaired")
                )
            })
        }));
    }

    #[test]
    fn explicit_session_recovery_is_reported_and_idempotent_after_save() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("new");
        let messages = vec![Message {
            role: Role::Assistant,
            content: vec![ContentBlock::ToolUse {
                id: "call-crashed".to_string(),
                name: "read_file".to_string(),
                input: serde_json::json!({"path": "README.md"}),
                caller: None,
                thought_signature: None,
            }],
        }];
        let session = create_saved_session(&messages, "test-model", tmp.path(), 0, None);
        let session_id = session.metadata.id.clone();
        manager.save_session(&session).expect("save");

        let recovered = manager
            .recover_session_for_resume(&session_id)
            .expect("recover");
        assert!(recovered.changed);
        assert_eq!(recovered.repaired_call_count, 1);
        assert_eq!(recovered.duplicate_result_count, 0);
        assert_eq!(recovered.orphan_result_count, 0);
        manager
            .save_session(&recovered.session)
            .expect("persist recovery");

        let second = manager
            .recover_session_for_resume(&session_id)
            .expect("recover twice");
        assert!(!second.changed);
        assert_eq!(second.repaired_call_count, 0);
        assert_eq!(second.session.messages, recovered.session.messages);
    }

    #[test]
    fn load_session_repairs_dangling_tool_call_with_visible_receipt() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("new");
        let messages = vec![Message {
            role: Role::Assistant,
            content: vec![ContentBlock::ToolUse {
                id: "call-crashed".to_string(),
                name: "read_file".to_string(),
                input: serde_json::json!({"path": "README.md"}),
                caller: None,
                thought_signature: None,
            }],
        }];
        let session = create_saved_session(&messages, "test-model", tmp.path(), 0, None);
        let session_id = session.metadata.id.clone();
        manager.save_session(&session).expect("save");

        let loaded = manager.load_session(&session_id).expect("load");

        assert_eq!(loaded.metadata.message_count, loaded.messages.len());
        assert!(loaded.messages.iter().any(|message| {
            message.content.iter().any(|block| {
                matches!(
                    block,
                    ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        is_error: Some(true),
                        ..
                    } if tool_use_id == "call-crashed" && content.contains("crashed_and_repaired")
                )
            })
        }));
        assert_eq!(
            loaded.journal.as_ref().map(SessionJournal::to_messages),
            Some(loaded.messages.clone()),
            "the append-only journal must follow the repaired active branch"
        );
        assert!(loaded.messages.iter().any(|message| {
            (message.role == "assistant"
                || message.role == codewhale_models::INTERRUPTED_ASSISTANT_ROLE)
                && message.content.iter().any(|block| {
                    matches!(
                        block,
                        ContentBlock::Text { text, .. }
                            if text.contains("[tool_history_repair]")
                    )
                })
        }));
    }

    #[test]
    fn save_and_load_session_preserves_rich_update_plan_tool_payload() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("new");
        let messages = vec![
            make_test_message("user", "plan this carefully"),
            Message {
                role: Role::Assistant,
                content: vec![ContentBlock::ToolUse {
                    id: "plan-1".to_string(),
                    name: "update_plan".to_string(),
                    input: serde_json::json!({
                        "objective": "Make Plan mode reviewable",
                        "sources_used": ["gh issue view 2691"],
                        "critical_files": ["crates/tui/src/tools/plan.rs"],
                        "constraints": ["Preserve legacy update_plan payloads"],
                        "verification_plan": "Run focused plan tests",
                        "handoff_packet": "Next agent should inspect replay",
                        "plan": [
                            { "step": "render replay card", "status": "completed" }
                        ]
                    }),
                    caller: None,
                    thought_signature: None,
                }],
            },
            Message {
                role: Role::User,
                content: vec![ContentBlock::ToolResult {
                    tool_use_id: "plan-1".to_string(),
                    content: "Plan updated".to_string(),
                    is_error: None,
                    content_blocks: None,
                }],
            },
        ];
        let session = create_saved_session(&messages, "deepseek-v4-flash", tmp.path(), 42, None);
        let session_id = session.metadata.id.clone();

        manager.save_session(&session).expect("save");
        let loaded = manager.load_session(&session_id).expect("load");

        assert_eq!(loaded.messages.len(), 3);
        let cells = history_cells_from_message(&loaded.messages[1]);
        let Some(HistoryCell::Tool(ToolCell::PlanUpdate(cell))) = cells.first() else {
            panic!("expected loaded update_plan to replay as a PlanUpdate cell");
        };
        assert_eq!(
            cell.snapshot.objective.as_deref(),
            Some("Make Plan mode reviewable")
        );
        assert_eq!(
            cell.snapshot.critical_files,
            vec!["crates/tui/src/tools/plan.rs"]
        );
        assert_eq!(cell.snapshot.items[0].status, StepStatus::Completed);
    }

    #[test]
    fn save_session_preserves_large_tool_outputs_for_cache_fidelity() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("new");
        let raw = "RAW_SESSION_SENTINEL\n".repeat(2_000);
        let messages = vec![
            Message {
                role: Role::Assistant,
                content: vec![ContentBlock::ToolUse {
                    id: "call-big".to_string(),
                    name: "exec_shell".to_string(),
                    input: serde_json::json!({"command": "cargo test -p codewhale-tui"}),
                    caller: None,
                    thought_signature: None,
                }],
            },
            Message {
                role: Role::User,
                content: vec![ContentBlock::ToolResult {
                    tool_use_id: "call-big".to_string(),
                    content: raw.clone(),
                    is_error: None,
                    content_blocks: None,
                }],
            },
        ];
        let mut session = create_saved_session(&messages, "test-model", tmp.path(), 100, None);
        session.artifacts.push(crate::artifacts::ArtifactRecord {
            id: "art_call-big".to_string(),
            kind: crate::artifacts::ArtifactKind::ToolOutput,
            session_id: session.metadata.id.clone(),
            tool_call_id: "call-big".to_string(),
            tool_name: "exec_shell".to_string(),
            created_at: Utc::now(),
            byte_size: raw.len() as u64,
            preview: "checking crate ... error[E0425]".to_string(),
            storage_path: PathBuf::from("artifacts/art_call-big.txt"),
        });

        let path = manager.save_session(&session).expect("save");
        let persisted_json = fs::read_to_string(path).expect("read persisted session");
        // Raw output is preserved in-session so resume can hit the LLM cache.
        assert!(persisted_json.contains("RAW_SESSION_SENTINEL"));

        let loaded = manager.load_session(&session.metadata.id).expect("load");
        let ContentBlock::ToolResult { content, .. } = &loaded.messages[1].content[0] else {
            panic!("expected loaded tool result");
        };
        // Loaded session retains the original output for cache fidelity.
        assert!(content.contains("RAW_SESSION_SENTINEL"));
        assert!(!content.contains("[TOOL_OUTPUT_RECEIPT]"));
    }

    #[test]
    fn load_session_preserves_legacy_large_tool_outputs_for_cache_fidelity() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("new");
        let raw = "RAW_LEGACY_RESUME_SENTINEL\n".repeat(2_000);
        let messages = vec![
            Message {
                role: Role::Assistant,
                content: vec![ContentBlock::ToolUse {
                    id: "call-legacy".to_string(),
                    name: "exec_shell".to_string(),
                    input: serde_json::json!({"command": "cargo check"}),
                    caller: None,
                    thought_signature: None,
                }],
            },
            Message {
                role: Role::User,
                content: vec![ContentBlock::ToolResult {
                    tool_use_id: "call-legacy".to_string(),
                    content: raw.clone(),
                    is_error: None,
                    content_blocks: None,
                }],
            },
        ];
        let mut session = create_saved_session(&messages, "test-model", tmp.path(), 100, None);
        session.artifacts.push(crate::artifacts::ArtifactRecord {
            id: "art_call-legacy".to_string(),
            kind: crate::artifacts::ArtifactKind::ToolOutput,
            session_id: session.metadata.id.clone(),
            tool_call_id: "call-legacy".to_string(),
            tool_name: "exec_shell".to_string(),
            created_at: Utc::now(),
            byte_size: raw.len() as u64,
            preview: "cargo check output".to_string(),
            storage_path: PathBuf::from("artifacts/art_call-legacy.txt"),
        });
        let path = manager
            .validated_session_path(&session.metadata.id)
            .expect("path");
        fs::write(
            &path,
            serde_json::to_string_pretty(&session).expect("serialize legacy session"),
        )
        .expect("write legacy raw session");
        assert!(
            fs::read_to_string(&path)
                .expect("read legacy raw")
                .contains("RAW_LEGACY_RESUME_SENTINEL")
        );

        let loaded = manager.load_session(&session.metadata.id).expect("load");
        let ContentBlock::ToolResult { content, .. } = &loaded.messages[1].content[0] else {
            panic!("expected loaded tool result");
        };
        // Loaded session preserves original output so resume can hit the LLM cache.
        assert!(content.contains("RAW_LEGACY_RESUME_SENTINEL"));
        assert!(!content.contains("[TOOL_OUTPUT_RECEIPT]"));
    }

    #[test]
    fn test_list_sessions() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("new");

        // Create a few sessions
        for i in 0..3 {
            let messages = vec![make_test_message("user", &format!("Session {i}"))];
            let session = create_saved_session(&messages, "test-model", tmp.path(), 100, None);
            manager.save_session(&session).expect("save");
        }

        let sessions = manager.list_sessions().expect("list");
        assert_eq!(sessions.len(), 3);
    }

    #[test]
    fn default_manager_copies_legacy_sessions_when_primary_already_exists() {
        let _lock = crate::test_support::lock_test_env();
        let tmp = tempdir().expect("tempdir");
        let home = tmp.path().join("home");
        let _home = crate::test_support::EnvVarGuard::set("HOME", &home);
        let _codewhale_home = crate::test_support::EnvVarGuard::remove("CODEWHALE_HOME");

        let primary_sessions = home.join(".codewhale").join("sessions");
        let legacy_sessions = home.join(".deepseek").join("sessions");
        fs::create_dir_all(&primary_sessions).expect("primary sessions");
        fs::create_dir_all(&legacy_sessions).expect("legacy sessions");
        fs::create_dir_all(legacy_sessions.join("checkpoints")).expect("legacy checkpoints");
        fs::write(
            legacy_sessions.join("checkpoints").join("latest.json"),
            "{}",
        )
        .expect("legacy checkpoint");

        let mut legacy_session = create_saved_session(
            &[make_test_message("user", "find my old session")],
            "test-model",
            tmp.path(),
            100,
            None,
        );
        legacy_session.metadata.id = "legacy-visible".to_string();
        legacy_session.metadata.title = "session from legacy home".to_string();
        fs::write(
            legacy_sessions.join("legacy-visible.json"),
            serde_json::to_string_pretty(&legacy_session).expect("serialize legacy session"),
        )
        .expect("write legacy session");

        let manager = SessionManager::default_location().expect("default manager");
        assert_eq!(manager.sessions_dir(), primary_sessions.as_path());
        assert!(primary_sessions.join("legacy-visible.json").exists());
        assert!(!primary_sessions.join("checkpoints").exists());
        assert!(legacy_sessions.join("legacy-visible.json").exists());

        let sessions = manager.list_sessions().expect("list");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "legacy-visible");
    }

    #[test]
    fn legacy_session_copy_never_overwrites_primary_session() {
        let _lock = crate::test_support::lock_test_env();
        let tmp = tempdir().expect("tempdir");
        let home = tmp.path().join("home");
        let _home = crate::test_support::EnvVarGuard::set("HOME", &home);
        let _codewhale_home = crate::test_support::EnvVarGuard::remove("CODEWHALE_HOME");

        let primary_sessions = home.join(".codewhale").join("sessions");
        let legacy_sessions = home.join(".deepseek").join("sessions");
        fs::create_dir_all(&primary_sessions).expect("primary sessions");
        fs::create_dir_all(&legacy_sessions).expect("legacy sessions");

        let primary_path = primary_sessions.join("same-id.json");
        fs::write(&primary_path, "primary data wins").expect("write primary session");
        fs::write(
            legacy_sessions.join("same-id.json"),
            "legacy data must not overwrite",
        )
        .expect("write legacy session");

        let dir = default_sessions_dir().expect("default session dir");
        assert_eq!(dir, primary_sessions);
        assert_eq!(
            fs::read_to_string(primary_path).expect("read primary session"),
            "primary data wins"
        );
    }

    #[test]
    fn explicit_codewhale_home_disables_legacy_session_copy() {
        let _lock = crate::test_support::lock_test_env();
        let tmp = tempdir().expect("tempdir");
        let home = tmp.path().join("home");
        let explicit_home = tmp.path().join("explicit-codewhale");
        let _home = crate::test_support::EnvVarGuard::set("HOME", &home);
        let _codewhale_home =
            crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", &explicit_home);

        let legacy_sessions = home.join(".deepseek").join("sessions");
        fs::create_dir_all(&legacy_sessions).expect("legacy sessions");
        fs::write(legacy_sessions.join("legacy-visible.json"), "{}").expect("write legacy session");

        let dir = default_sessions_dir().expect("default session dir");
        assert_eq!(dir, explicit_home.join("sessions"));
        assert!(!dir.join("legacy-visible.json").exists());
    }

    #[cfg(unix)]
    #[test]
    fn non_unicode_codewhale_home_is_still_an_explicit_session_boundary() {
        use std::os::unix::ffi::OsStringExt;

        let _lock = crate::test_support::lock_test_env();
        let tmp = tempdir().expect("tempdir");
        let home = tmp.path().join("home");
        let explicit_home = tmp.path().join(std::ffi::OsString::from_vec(
            b"codewhale-\xff-home".to_vec(),
        ));
        let _home = crate::test_support::EnvVarGuard::set("HOME", &home);
        let _codewhale_home =
            crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", &explicit_home);

        let legacy_sessions = home.join(".deepseek").join("sessions");
        fs::create_dir_all(&legacy_sessions).expect("legacy sessions");
        fs::write(legacy_sessions.join("ambient.json"), "ambient").expect("ambient legacy session");
        let safe_primary = tmp.path().join("safe-primary");
        fs::create_dir_all(&safe_primary).expect("safe primary");

        assert_eq!(
            merge_missing_legacy_session_entries(&safe_primary).expect("merge decision"),
            0
        );
        assert!(!safe_primary.join("ambient.json").exists());
    }

    #[test]
    fn latest_session_for_workspace_ignores_newer_other_directory() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("new");
        let workspace_a = tmp.path().join("aa").join("aaa");
        let workspace_b = tmp.path().join("bb").join("bbb");
        fs::create_dir_all(&workspace_a).expect("mkdir workspace a");
        fs::create_dir_all(&workspace_b).expect("mkdir workspace b");
        fs::create_dir_all(tmp.path().join(".git")).expect("mkdir invalid git boundary");

        write_session_record(
            &manager,
            "current-workspace",
            &workspace_a,
            Utc::now() - chrono::Duration::minutes(10),
        );
        write_session_record(&manager, "other-workspace", &workspace_b, Utc::now());

        let global = manager
            .list_sessions()
            .expect("list")
            .into_iter()
            .next()
            .expect("global latest");
        assert_eq!(global.id, "other-workspace");

        let scoped = manager
            .get_latest_session_for_workspace(&workspace_a)
            .expect("latest for workspace")
            .expect("scoped latest");
        assert_eq!(scoped.id, "current-workspace");
    }

    #[test]
    fn latest_session_for_workspace_ignores_invalid_parent_git_marker() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("new");
        let workspace_a = tmp.path().join("aa").join("aaa");
        let workspace_b = tmp.path().join("bb").join("bbb");
        fs::create_dir_all(&workspace_a).expect("mkdir workspace a");
        fs::create_dir_all(&workspace_b).expect("mkdir workspace b");
        fs::create_dir_all(tmp.path().join(".git")).expect("mkdir invalid git marker");

        write_session_record(
            &manager,
            "current-workspace",
            &workspace_a,
            Utc::now() - chrono::Duration::minutes(10),
        );
        write_session_record(&manager, "other-workspace", &workspace_b, Utc::now());

        let scoped = manager
            .get_latest_session_for_workspace(&workspace_a)
            .expect("latest for workspace")
            .expect("scoped latest");
        assert_eq!(scoped.id, "current-workspace");
    }

    #[test]
    fn latest_session_for_workspace_matches_same_git_repository() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("new");
        let repo = tmp.path().join("repo");
        let repo_app = repo.join("apps").join("client");
        let repo_crate = repo.join("crates").join("server");
        let other_repo = tmp.path().join("other").join("project");
        fs::create_dir_all(repo.join(".git")).expect("mkdir .git");
        fs::write(repo.join(".git").join("HEAD"), "ref: refs/heads/main\n").expect("write HEAD");
        fs::create_dir_all(&repo_app).expect("mkdir repo app");
        fs::create_dir_all(&repo_crate).expect("mkdir repo crate");
        fs::create_dir_all(&other_repo).expect("mkdir other repo");

        write_session_record(
            &manager,
            "same-repo",
            &repo_app,
            Utc::now() - chrono::Duration::minutes(5),
        );
        write_session_record(&manager, "other-repo", &other_repo, Utc::now());

        let scoped = manager
            .get_latest_session_for_workspace(&repo_crate)
            .expect("latest for workspace")
            .expect("same repo latest");
        assert_eq!(scoped.id, "same-repo");
    }

    #[test]
    fn latest_session_for_workspace_skips_empty_auto_created_session() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("new");
        let workspace = tmp.path().join("repo");
        fs::create_dir_all(&workspace).expect("mkdir workspace");

        write_session_record(
            &manager,
            "interrupted-user-turn",
            &workspace,
            Utc::now() - chrono::Duration::minutes(5),
        );
        write_empty_session_record(&manager, "empty-auto-shell", &workspace, Utc::now());

        let global = manager
            .list_sessions()
            .expect("list")
            .into_iter()
            .next()
            .expect("global latest");
        assert_eq!(global.id, "empty-auto-shell");

        let scoped = manager
            .get_latest_session_for_workspace(&workspace)
            .expect("latest for workspace")
            .expect("scoped latest");
        assert_eq!(scoped.id, "interrupted-user-turn");
    }

    #[test]
    fn test_load_by_prefix() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("new");

        let messages = vec![make_test_message("user", "Test session")];
        let session = create_saved_session(&messages, "test-model", tmp.path(), 100, None);
        let prefix = truncate_id(&session.metadata.id).to_string();
        manager.save_session(&session).expect("save");

        let loaded = manager.load_session_by_prefix(&prefix).expect("load");
        assert_eq!(loaded.messages.len(), 1);
    }

    #[test]
    fn test_delete_session() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("new");

        let messages = vec![make_test_message("user", "To be deleted")];
        let session = create_saved_session(&messages, "test-model", tmp.path(), 100, None);
        let session_id = session.metadata.id.clone();

        manager.save_session(&session).expect("save");
        assert!(manager.load_session(&session_id).is_ok());

        manager.delete_session(&session_id).expect("delete");
        assert!(manager.load_session(&session_id).is_err());
    }

    #[test]
    fn delete_session_removes_artifact_directory() {
        let tmp = tempdir().expect("tempdir");
        let sessions_dir = tmp.path().join("sessions");
        let manager = SessionManager::new(sessions_dir.clone()).expect("new");

        let session = create_saved_session(
            &[make_test_message("user", "artifact session")],
            "test-model",
            tmp.path(),
            100,
            None,
        );
        let session_id = session.metadata.id.clone();
        let artifact_dir = sessions_dir.join(&session_id).join("artifacts");
        fs::create_dir_all(&artifact_dir).expect("artifact dir");
        fs::write(artifact_dir.join("art_call.txt"), "raw output").expect("artifact file");

        manager.save_session(&session).expect("save");
        manager.delete_session(&session_id).expect("delete");

        assert!(!sessions_dir.join(format!("{session_id}.json")).exists());
        assert!(!sessions_dir.join(&session_id).exists());
    }

    #[test]
    fn test_session_id_rejects_invalid_characters() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("new");

        let err = manager
            .load_session("../outside")
            .expect_err("invalid id should fail");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);

        let err = manager
            .delete_session("sess bad")
            .expect_err("invalid id should fail");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    }

    #[test]
    fn test_session_manager_rejects_relative_traversal_dir() {
        let err = SessionManager::new(PathBuf::from("../sessions"))
            .expect_err("relative traversal directory should fail");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    }

    #[test]
    fn test_truncate_title() {
        assert_eq!(truncate_title("Short", 50), "Short");
        assert_eq!(
            truncate_title("This is a very long title that should be truncated", 20),
            "This is a very lo..."
        );
        assert_eq!(truncate_title("Line 1\nLine 2", 50), "Line 1");
    }

    #[test]
    fn extract_user_prompt_strips_turn_meta_prefix() {
        assert_eq!(
            extract_user_prompt("<turn_meta>{\"cache\":\"x\"}</turn_meta>\nReal prompt"),
            "Real prompt"
        );
        assert_eq!(extract_user_prompt("  Real prompt"), "Real prompt");
        assert_eq!(
            extract_user_prompt("<turn_meta>{\"unterminated\":true}\nReal prompt"),
            "{\"unterminated\":true}\nReal prompt"
        );
    }

    #[test]
    fn create_saved_session_uses_prompt_after_turn_meta_for_title() {
        let tmp = tempdir().expect("tempdir");
        let messages = vec![make_test_message(
            "user",
            "<turn_meta>{\"cache\":\"x\"}</turn_meta>\nFix the session picker history pane",
        )];
        let session = create_saved_session(&messages, "test-model", tmp.path(), 100, None);
        assert_eq!(
            session.metadata.title,
            "Fix the session picker history pane"
        );
    }

    #[test]
    fn create_saved_session_skips_runtime_handoffs_when_deriving_title() {
        let tmp = tempdir().expect("tempdir");
        // Operate/automation sessions start with runtime-owned control traffic
        // as the first `user` message. The auto-title must come from the real
        // prompt that follows, never from the internal envelope.
        let messages = vec![
            crate::runtime_handoff::operate_contract_runtime_message(),
            make_test_message("user", "Ship the session-title fix"),
        ];
        let session = create_saved_session(&messages, "test-model", tmp.path(), 100, None);
        assert_eq!(session.metadata.title, "Ship the session-title fix");
        assert!(
            !session.metadata.title.contains("codewhale:runtime"),
            "internal envelope leaked into the session title: {}",
            session.metadata.title
        );
    }

    #[test]
    fn create_saved_session_with_only_runtime_traffic_keeps_placeholder_title() {
        let tmp = tempdir().expect("tempdir");
        let waiting = crate::runtime_handoff::waiting_for_subagents_runtime_message(2);
        let restored =
            crate::runtime_handoff::project_messages_for_restore(std::slice::from_ref(&waiting))
                .into_iter()
                .next()
                .expect("restore projection yields one message");
        // Runtime handoffs must stay out of the auto-title. Exercise the
        // Operate contract, a waiting/restored
        // sub-agent checkpoint, and a background shell completion.
        let messages = vec![
            crate::runtime_handoff::operate_contract_runtime_message(),
            waiting,
            restored,
            crate::runtime_handoff::shell_completion_runtime_message(&[]),
        ];
        let session = create_saved_session(&messages, "test-model", tmp.path(), 100, None);
        assert_eq!(session.metadata.title, DEFAULT_SESSION_TITLE);
        assert!(
            !session.metadata.title.contains("codewhale:runtime"),
            "internal envelope leaked into the session title: {}",
            session.metadata.title
        );
    }

    #[test]
    fn import_foreign_derives_title_from_the_first_real_user_message() {
        let tmp = tempdir().expect("tempdir");
        // Importing a session whose transcript opens with the Operate contract
        // (the shape this bug produced on export) must not re-derive the
        // envelope as the imported title.
        let container = container_with(
            vec![
                crate::runtime_handoff::operate_contract_runtime_message(),
                make_test_message("user", "Fix the session picker"),
            ],
            tmp.path(),
        );
        let imported = crate::session_manager::SavedSession::import_foreign(
            container,
            tmp.path().to_path_buf(),
            "test-model".to_string(),
        )
        .expect("import succeeds");
        assert_eq!(imported.metadata.title, "Fix the session picker");
        assert!(
            !imported.metadata.title.contains("codewhale:runtime"),
            "internal envelope leaked into the imported session title: {}",
            imported.metadata.title
        );
    }

    #[test]
    fn import_foreign_keeps_placeholder_when_only_runtime_traffic() {
        let tmp = tempdir().expect("tempdir");
        let container = container_with(
            vec![crate::runtime_handoff::operate_contract_runtime_message()],
            tmp.path(),
        );
        let imported = crate::session_manager::SavedSession::import_foreign(
            container,
            tmp.path().to_path_buf(),
            "test-model".to_string(),
        )
        .expect("import succeeds");
        assert_eq!(imported.metadata.title, DEFAULT_SESSION_TITLE);
    }

    #[test]
    fn title_derivation_skips_current_and_legacy_runtime_provenance() {
        let tmp = tempdir().expect("tempdir");
        for leading_metadata in [false, true] {
            let mut runtime = make_test_message("user", "Internal diagnostic update");
            let metadata = ContentBlock::Text {
                text: "<turn_meta>\nInput provenance: runtime (non-authoritative)\n</turn_meta>"
                    .to_string(),
                cache_control: None,
            };
            if leading_metadata {
                runtime.content.insert(0, metadata);
            } else {
                runtime.content.push(metadata);
            }
            let messages = vec![
                runtime,
                make_test_message("user", "Fix the diagnostic display"),
            ];
            let session = create_saved_session(&messages, "test-model", tmp.path(), 0, None);
            assert_eq!(session.metadata.title, "Fix the diagnostic display");
            let imported = SavedSession::import_foreign(
                container_with(messages, tmp.path()),
                tmp.path().to_path_buf(),
                "test-model".to_string(),
            )
            .expect("import succeeds");
            assert_eq!(imported.metadata.title, "Fix the diagnostic display");
        }
    }

    #[test]
    fn title_derivation_keeps_user_authored_runtime_example() {
        let tmp = tempdir().expect("tempdir");
        let messages = vec![make_test_message(
            "user",
            "<codewhale:runtime_event> example",
        )];
        let session = create_saved_session(&messages, "test-model", tmp.path(), 0, None);
        assert_eq!(session.metadata.title, "<codewhale:runtime_event> example");
    }

    /// The exact bytes `SessionManager::load_session_metadata` reads.
    fn session_bytes(session: &SavedSession, stored_title: &str) -> Vec<u8> {
        let mut stale = session.clone();
        stale.metadata.title = stored_title.to_string();
        serde_json::to_vec(&stale).expect("serialize session")
    }

    fn loaded_title(session: &SavedSession, stored_title: &str) -> String {
        let buf = session_bytes(session, stored_title);
        let mut metadata = extract_top_level_metadata(&buf).expect("metadata extractable");
        assert_eq!(metadata.title, stored_title);
        apply_legacy_title_recovery(&mut metadata, &buf);
        metadata.title
    }

    /// What the superseded derivation stored for an Operate-contract session:
    /// the first line of the engine envelope, cut at 50 characters.
    fn legacy_operate_title() -> String {
        let message = crate::runtime_handoff::operate_contract_runtime_message();
        let ContentBlock::Text { text, .. } = &message.content[0] else {
            panic!("operate contract opens with text");
        };
        truncate_title(text, 50)
    }

    #[test]
    fn legacy_runtime_titles_recover_the_real_user_prompt() {
        let tmp = tempdir().expect("tempdir");
        let messages = vec![
            crate::runtime_handoff::operate_contract_runtime_message(),
            make_test_message("user", "Fix the diagnostic display"),
        ];
        let session = create_saved_session(&messages, "test-model", tmp.path(), 0, None);
        let stored = legacy_operate_title();
        assert!(
            stored.starts_with("<codewhale:runtime_event kind="),
            "{stored:?}",
        );
        assert_eq!(
            loaded_title(&session, &stored),
            "Fix the diagnostic display"
        );
    }

    #[test]
    fn legacy_recovery_leaves_renames_and_user_authored_titles_alone() {
        let tmp = tempdir().expect("tempdir");
        let messages = vec![
            crate::runtime_handoff::operate_contract_runtime_message(),
            make_test_message("user", "Fix the diagnostic display"),
        ];
        let session = create_saved_session(&messages, "test-model", tmp.path(), 0, None);
        // Renames win, including ones that open with `<` and so pay for the
        // message scan: provenance is proven, never guessed from the shape.
        for rename in [
            "Operate contract",
            "<my own angle-bracket title>",
            "<codewhale:runtime_event kind=\"operate_contract\" but renamed by me",
        ] {
            assert_eq!(loaded_title(&session, rename), rename);
        }
    }

    #[test]
    fn a_user_who_types_an_attributed_envelope_keeps_their_title() {
        // The one case the earlier substring rule got wrong. The engine's
        // envelope carries a runtime provenance line; a person's message does
        // not, and the existing classifier is what tells them apart — so this
        // title is theirs and survives.
        let tmp = tempdir().expect("tempdir");
        let typed = "<codewhale:runtime_event kind=\"operate_contract\" visibility=\"internal\">";
        let messages = vec![make_test_message("user", typed)];
        let session = create_saved_session(&messages, "test-model", tmp.path(), 0, None);
        let stored = truncate_title(typed, 50);
        assert_eq!(session.metadata.title, stored);
        assert_eq!(loaded_title(&session, &stored), stored);
    }

    #[test]
    fn legacy_recovery_names_a_runtime_only_session_by_the_default() {
        // Nothing but runtime traffic: there is no user prompt to recover, and
        // the array ended inside the read, so the neutral default is provable.
        let tmp = tempdir().expect("tempdir");
        let messages = vec![crate::runtime_handoff::operate_contract_runtime_message()];
        let session = create_saved_session(&messages, "test-model", tmp.path(), 0, None);
        assert_eq!(
            loaded_title(&session, &legacy_operate_title()),
            DEFAULT_SESSION_TITLE
        );
    }

    #[test]
    fn legacy_recovery_keeps_the_stored_title_when_the_read_was_truncated() {
        // A prefix cut before the user's turn must not be read as "this
        // conversation has no prompt".
        let tmp = tempdir().expect("tempdir");
        let messages = vec![
            crate::runtime_handoff::operate_contract_runtime_message(),
            make_test_message("user", "Fix the diagnostic display"),
        ];
        let session = create_saved_session(&messages, "test-model", tmp.path(), 0, None);
        let stored = legacy_operate_title();
        let full = session_bytes(&session, &stored);
        let messages_at = full
            .windows(10)
            .position(|w| w == b"\"messages\"")
            .expect("messages key present");
        let cut = &full[..messages_at + 40];
        let mut metadata = extract_top_level_metadata(cut).expect("metadata precedes messages");
        apply_legacy_title_recovery(&mut metadata, cut);
        assert_eq!(metadata.title, stored, "a truncated read must not rename");
    }

    #[test]
    fn ordinary_titles_never_pay_for_the_message_scan() {
        // #337's bounded read is the reason `list_sessions` is cheap. The `<`
        // gate is a cost filter only; the rename decision is the provenance
        // check above.
        let tmp = tempdir().expect("tempdir");
        let messages = vec![make_test_message("user", "Fix the diagnostic display")];
        let session = create_saved_session(&messages, "test-model", tmp.path(), 0, None);
        let mut metadata = session.metadata.clone();
        assert!(!metadata.title.starts_with('<'));
        apply_legacy_title_recovery(&mut metadata, &[]);
        assert_eq!(metadata.title, "Fix the diagnostic display");
    }

    #[test]
    fn leading_messages_stop_at_the_edge_of_a_truncated_prefix() {
        let tmp = tempdir().expect("tempdir");
        let messages = vec![
            make_test_message("user", "first"),
            make_test_message("assistant", "second"),
            make_test_message("user", "third"),
        ];
        let session = create_saved_session(&messages, "test-model", tmp.path(), 0, None);
        let buf = serde_json::to_vec(&session).expect("serialize");
        let (all, complete) = extract_leading_messages(&buf, 24);
        assert!(complete, "a whole file ends its messages array");
        assert_eq!(all.len(), 3);

        let (capped, complete) = extract_leading_messages(&buf, 2);
        assert_eq!(capped.len(), 2);
        assert!(!complete, "a capped scan has not seen the array end");

        // The file midpoint depends on metadata path lengths and can already
        // follow the messages array. Cut inside the third message instead.
        let marker = b"\"third\"";
        let cut = buf
            .windows(marker.len())
            .position(|window| window == marker)
            .expect("third message is serialized")
            + marker.len() / 2;
        let (partial, complete) = extract_leading_messages(&buf[..cut], 24);
        assert!(!complete);
        assert_eq!(partial.len(), 2, "the cut message must remain absent");
    }

    #[test]
    fn title_derivation_keeps_the_first_image_only_user_boundary() {
        let tmp = tempdir().expect("tempdir");
        for with_metadata in [false, true] {
            let mut first = Message {
                role: Role::User,
                content: vec![ContentBlock::ImageUrl {
                    image_url: codewhale_models::ImageUrlContent {
                        url: "data:image/png;base64,AAAA".to_string(),
                    },
                }],
            };
            if with_metadata {
                first.content.push(ContentBlock::Text {
                    text: "<turn_meta>\nSession mode: Work\n</turn_meta>".to_string(),
                    cache_control: None,
                });
            }
            let messages = vec![first, make_test_message("user", "A later request")];
            assert_eq!(conversation_title_prompt(&messages), None);
            let session = create_saved_session(&messages, "test-model", tmp.path(), 0, None);
            assert_eq!(session.metadata.title, DEFAULT_SESSION_TITLE);
            let imported = SavedSession::import_foreign(
                container_with(messages, tmp.path()),
                tmp.path().to_path_buf(),
                "test-model".to_string(),
            )
            .expect("import succeeds");
            assert_eq!(imported.metadata.title, DEFAULT_SESSION_TITLE);
        }
    }

    #[test]
    fn strip_thinking_tags_removes_common_inline_blocks() {
        let text = "Before <think>private</think> middle <reasoning>hidden</reasoning> after";
        let cleaned = strip_thinking_tags(text);
        assert_eq!(cleaned, "Before  middle  after");
        assert_eq!(strip_thinking_tags("plain answer"), "plain answer");
    }

    #[test]
    fn test_format_age() {
        let now = Utc::now();
        assert_eq!(format_age(&now), "just now");

        let hour_ago = now - chrono::Duration::hours(2);
        assert_eq!(format_age(&hour_ago), "2h ago");

        let day_ago = now - chrono::Duration::days(3);
        assert_eq!(format_age(&day_ago), "3d ago");
    }

    #[test]
    fn session_titles_never_keep_terminal_controls_or_bidi_format_chars() {
        let raw = "Ev\u{1b}]0;PWNED\u{7}il\u{202e}R\u{200b}Z\u{9d}0;X\u{9c}After\u{2066}B\u{2069} 会議 🐳";
        assert_eq!(
            sanitize_session_title(raw),
            "Ev]0;PWNEDilRZ0;XAfterB 会議 🐳"
        );
        // Every rename surface goes through normalize_session_title.
        assert_eq!(
            normalize_session_title(raw).unwrap(),
            "Ev]0;PWNEDilRZ0;XAfterB 会議 🐳"
        );
        // A title that is nothing but controls is an empty title.
        assert!(normalize_session_title("\u{1b}\u{7}\u{200b}").is_err());
        // The listing line re-sanitizes titles saved before this policy.
        assert_eq!(truncate_title(raw, 40), "Ev]0;PWNEDilRZ0;XAfterB 会議 🐳");
    }

    #[test]
    fn format_session_line_includes_absolute_updated_timestamp() {
        let mut session = create_saved_session(
            &[make_test_message("user", "Find Friday work")],
            "test-model",
            Path::new("/tmp/project"),
            100,
            None,
        );
        session.metadata.updated_at = DateTime::parse_from_rfc3339("2026-06-01T12:34:00Z")
            .expect("timestamp")
            .with_timezone(&Utc);

        let line = format_session_line(&session.metadata);

        assert!(
            line.contains("2026-06-01 12:34 UTC"),
            "session list should include an absolute timestamp, got {line:?}"
        );
    }

    #[test]
    fn test_update_session() {
        let tmp = tempdir().expect("tempdir");

        let messages = vec![make_test_message("user", "Hello")];
        let session = create_saved_session(&messages, "test-model", tmp.path(), 50, None);

        let new_messages = vec![
            make_test_message("user", "Hello"),
            make_test_message("assistant", "Hi!"),
        ];

        let updated = update_session(session, &new_messages, 100, None);
        assert_eq!(updated.messages.len(), 2);
        assert_eq!(updated.metadata.total_tokens, 100);
    }

    #[test]
    fn save_load_round_trip_preserves_all_messages_for_cache_fidelity() {
        #[derive(serde::Deserialize)]
        struct LegacySession {
            messages: Vec<Message>,
        }

        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("new");
        // Covers the old 500-message cap boundary and well beyond.
        for count in [0, 1, 500, 501, 600, 1000] {
            let original: Vec<_> = (0..count)
                .map(|i| {
                    make_test_message(
                        if i % 2 == 0 { "user" } else { "assistant" },
                        &format!("round-trip message {i}"),
                    )
                })
                .collect();

            let mut session = create_saved_session(&original, "test-model", tmp.path(), 0, None);
            let expected_journal = session.journal.clone();
            session.compact_for_persistence_queue();
            let path = manager.save_session(&session).expect("save");
            let legacy: LegacySession =
                serde_json::from_slice(&fs::read(path).expect("read")).expect("legacy reader");
            let loaded = manager.load_session(&session.metadata.id).expect("load");

            assert_eq!(
                legacy.messages, original,
                "legacy messages for count={count}"
            );
            assert_eq!(
                loaded.journal, expected_journal,
                "journal for count={count}"
            );
            assert_eq!(
                loaded.messages.len(),
                count,
                "count preserved for count={count}"
            );
            assert_eq!(
                loaded.messages, original,
                "every message byte-identical after round-trip for count={count}"
            );
        }
    }

    #[test]
    fn test_checkpoint_round_trip_and_clear() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("new");
        let messages = vec![make_test_message("user", "checkpoint me")];
        let mut session = create_saved_session(&messages, "test-model", tmp.path(), 12, None);
        session.work_state = Some(SessionWorkState {
            todos: crate::tools::todo::TodoListSnapshot {
                items: vec![crate::tools::todo::TodoItem {
                    id: 1,
                    content: "verify checkpoint durability".to_string(),
                    status: crate::tools::todo::TodoStatus::InProgress,
                }],
                completion_pct: 0,
                in_progress_id: Some(1),
            },
            ..SessionWorkState::default()
        });
        let expected_messages = session.messages.clone();
        let expected_journal = session.journal.clone();
        session.compact_for_persistence_queue();

        let path = manager.save_checkpoint(&session).expect("save checkpoint");
        assert_eq!(
            path.file_name().and_then(|n| n.to_str()),
            Some(format!("{}.json", session.metadata.id).as_str()),
            "checkpoint file must be keyed by session id"
        );
        let loaded = manager
            .load_session_checkpoint(&session.metadata.id)
            .expect("load checkpoint")
            .expect("checkpoint exists");
        assert_eq!(loaded.metadata.id, session.metadata.id);
        assert_eq!(loaded.messages, expected_messages);
        assert_eq!(loaded.journal, expected_journal);
        assert_eq!(
            loaded.work_state, session.work_state,
            "work state must survive the checkpoint round trip"
        );

        manager
            .clear_session_checkpoint(&session.metadata.id)
            .expect("clear checkpoint");
        assert!(
            manager
                .load_session_checkpoint(&session.metadata.id)
                .expect("load checkpoint")
                .is_none()
        );
    }

    #[test]
    fn graph_backed_work_state_remains_readable_by_legacy_shape() {
        #[derive(serde::Deserialize)]
        struct LegacyWorkState {
            #[serde(default)]
            todos: crate::tools::todo::TodoListSnapshot,
            #[serde(default)]
            plan: crate::tools::plan::PlanSnapshot,
        }

        let fixture = include_bytes!("../tests/fixtures/work_graph_session_v1_reader.json");
        let current: SavedSession = serde_json::from_slice(fixture).expect("current reader");
        let state = current.work_state.expect("fixture Work state");
        let legacy: LegacyWorkState = serde_json::from_value(
            serde_json::from_slice::<serde_json::Value>(fixture)
                .expect("fixture JSON")["work_state"]
                .clone(),
        )
        .expect("v1 reader ignores graph");
        assert_eq!(legacy.todos, state.todos);
        assert_eq!(legacy.plan, state.plan);
        let graph = state.graph.expect("fixture graph");
        crate::work_graph::validate(&graph).expect("valid fixture graph");
        assert_eq!(crate::work_graph::project_todos(&graph), state.todos);
        assert_eq!(crate::work_graph::project_plan(&graph), state.plan);
    }

    #[test]
    fn first_graph_write_archives_exact_legacy_session_once() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("new");
        let mut session = create_saved_session(
            &[make_test_message("user", "archive before import")],
            "test-model",
            tmp.path(),
            0,
            None,
        );
        let plan = crate::tools::plan::PlanSnapshot {
            items: vec![crate::tools::plan::PlanItemArg {
                step: "Import".to_string(),
                status: crate::tools::plan::StepStatus::Pending,
            }],
            ..crate::tools::plan::PlanSnapshot::default()
        };
        let todos = crate::tools::todo::TodoListSnapshot::default();
        session.work_state = Some(SessionWorkState {
            graph: None,
            todos: todos.clone(),
            plan: plan.clone(),
        });
        let path = manager.save_session(&session).expect("save legacy session");
        let legacy_bytes = fs::read(&path).expect("read legacy bytes");

        let graph = crate::work_graph::import_legacy(&session.metadata.id, &plan, &todos)
            .expect("import graph");
        session.work_state = Some(SessionWorkState {
            graph: Some(graph),
            todos,
            plan,
        });
        manager.save_session(&session).expect("first graph write");
        let archive = manager
            .sessions_dir
            .join(WORK_GRAPH_IMPORT_ARCHIVE_DIR)
            .join(path.file_name().expect("session filename"));
        assert_eq!(fs::read(&archive).expect("archive exists"), legacy_bytes);

        session.metadata.title = "later graph write".to_string();
        manager.save_session(&session).expect("second graph write");
        assert_eq!(
            fs::read(&archive).expect("archive still exists"),
            legacy_bytes,
            "later graph writes must not replace the pre-import receipt"
        );
    }

    #[test]
    fn checkpoints_are_independent_per_session() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("new");
        let first = create_saved_session(
            &[make_test_message("user", "session one")],
            "test-model",
            tmp.path(),
            0,
            None,
        );
        let second = create_saved_session(
            &[make_test_message("user", "session two")],
            "test-model",
            tmp.path(),
            0,
            None,
        );

        manager.save_checkpoint(&first).expect("save first");
        manager.save_checkpoint(&second).expect("save second");
        manager
            .clear_session_checkpoint(&first.metadata.id)
            .expect("clear first");

        assert!(
            manager
                .load_session_checkpoint(&first.metadata.id)
                .expect("load first")
                .is_none(),
            "clearing one session must remove only that session's file"
        );
        let survivor = manager
            .load_session_checkpoint(&second.metadata.id)
            .expect("load second")
            .expect("second checkpoint survives");
        assert_eq!(survivor.metadata.id, second.metadata.id);
    }

    #[test]
    fn list_checkpoints_includes_legacy_slot_and_skips_offline_queue() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("new");
        let session = create_saved_session(
            &[make_test_message("user", "list me")],
            "test-model",
            tmp.path(),
            0,
            None,
        );
        manager.save_checkpoint(&session).expect("save checkpoint");
        let checkpoints = tmp.path().join("sessions").join("checkpoints");
        fs::write(checkpoints.join("latest.json"), "{}").expect("write legacy slot");
        fs::write(checkpoints.join("offline_queue.json"), "{}").expect("write legacy queue");
        fs::write(
            checkpoints.join(format!("{}.offline_queue.json", session.metadata.id)),
            "{}",
        )
        .expect("write per-session queue");

        let refs = manager.list_checkpoints().expect("list checkpoints");
        assert_eq!(refs.len(), 2, "offline queue must not be a candidate");
        assert!(
            refs.iter()
                .any(|r| r.source == CheckpointSource::Session(session.metadata.id.clone()))
        );
        assert!(refs.iter().any(|r| r.source == CheckpointSource::Legacy));
    }

    #[test]
    fn legacy_migration_never_overwrites_existing_per_session_checkpoint() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("new");
        let mut session = create_saved_session(
            &[make_test_message("user", "original")],
            "test-model",
            tmp.path(),
            0,
            None,
        );
        manager.save_checkpoint(&session).expect("save checkpoint");

        session.messages = vec![make_test_message("user", "stale legacy copy")];
        let written = manager
            .write_session_checkpoint_if_absent(&session)
            .expect("migration attempt");
        assert!(!written, "migration must not overwrite an existing file");
        let loaded = manager
            .load_session_checkpoint(&session.metadata.id)
            .expect("load")
            .expect("checkpoint exists");
        assert_eq!(
            loaded.messages,
            vec![make_test_message("user", "original")],
            "existing per-session checkpoint content must be preserved"
        );
    }

    #[test]
    fn workspace_scope_matches_subdirectories_in_same_git_checkout() {
        let tmp = tempdir().expect("tempdir");
        let repo = tmp.path().join("repo");
        let nested = repo.join("crates").join("tui");
        fs::create_dir_all(&nested).expect("mkdir nested");
        fs::write(repo.join(".git"), "gitdir: .git/worktrees/repo").expect("write git marker");

        assert!(workspace_scope_matches(&repo, &nested));
    }

    #[test]
    fn workspace_scope_rejects_sibling_git_checkouts() {
        let tmp = tempdir().expect("tempdir");
        let first = tmp.path().join("repo-a");
        let second = tmp.path().join("repo-b");
        fs::create_dir_all(&first).expect("mkdir first");
        fs::create_dir_all(&second).expect("mkdir second");
        fs::write(first.join(".git"), "gitdir: .git/worktrees/a").expect("write first marker");
        fs::write(second.join(".git"), "gitdir: .git/worktrees/b").expect("write second marker");

        assert!(!workspace_scope_matches(&first, &second));
    }

    #[test]
    fn test_offline_queue_round_trip_and_clear() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("new");

        let state = OfflineQueueState {
            messages: vec![QueuedSessionMessage {
                display: "queued message".to_string(),
                skill_instruction: Some("Use skill".to_string()),
                skill_provenance: None,
            }],
            draft: Some(QueuedSessionMessage {
                display: "draft message".to_string(),
                skill_instruction: None,
                skill_provenance: None,
            }),
            ..OfflineQueueState::default()
        };

        manager
            .save_offline_queue_state(&state, Some("test-session"))
            .expect("save queue state");
        let loaded = manager
            .load_offline_queue_state("test-session")
            .expect("load queue state")
            .expect("queue state exists");
        assert_eq!(loaded.messages.len(), 1);
        assert_eq!(loaded.messages[0].display, "queued message");
        assert!(loaded.draft.is_some());

        manager
            .clear_offline_queue_state_for("test-session")
            .expect("clear queue state");
        assert!(
            manager
                .load_offline_queue_state("test-session")
                .expect("load queue state")
                .is_none()
        );

        // A queue with no owning session has nowhere to be restored to, so it
        // is refused rather than written where another session would find it.
        let unowned = manager.save_offline_queue_state(&state, None);
        assert!(unowned.is_err(), "unowned queue must not be parked");
    }

    fn parked(text: &str) -> OfflineQueueState {
        OfflineQueueState {
            messages: vec![QueuedSessionMessage {
                display: text.to_string(),
                skill_instruction: None,
                skill_provenance: None,
            }],
            ..OfflineQueueState::default()
        }
    }

    #[test]
    fn offline_queues_are_keyed_per_session() {
        // Replaces the #487 single-slot test, which pinned the shared
        // `checkpoints/offline_queue.json`: two concurrent Codewhale
        // instances raced on it and the loser's unsent text was destroyed.
        // Queues are keyed per session for the same reason checkpoints are.
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("new");

        manager
            .save_offline_queue_state(&parked("A text"), Some("session-A"))
            .expect("park A");
        manager
            .save_offline_queue_state(&parked("B text"), Some("session-B"))
            .expect("park B");

        let a = manager
            .load_offline_queue_state("session-A")
            .expect("load A")
            .expect("A still parked");
        assert_eq!(a.messages[0].display, "A text");
        assert_eq!(a.session_id.as_deref(), Some("session-A"));
        let b = manager
            .load_offline_queue_state("session-B")
            .expect("load B")
            .expect("B still parked");
        assert_eq!(b.messages[0].display, "B text");

        // Clearing one session's queue leaves the other's alone.
        manager
            .clear_offline_queue_state_for("session-A")
            .expect("clear A");
        assert!(
            manager
                .load_offline_queue_state("session-A")
                .expect("load A")
                .is_none()
        );
        assert!(
            manager
                .load_offline_queue_state("session-B")
                .expect("load B")
                .is_some(),
            "clearing one session must never delete another's unsent text"
        );

        // A session with nothing parked reads back nothing — it can never
        // inherit, or destroy, a sibling's queue.
        assert!(
            manager
                .load_offline_queue_state("session-C")
                .expect("load C")
                .is_none()
        );
    }

    #[test]
    fn legacy_global_queue_is_adopted_only_by_its_own_session() {
        let tmp = tempdir().expect("tempdir");
        let sessions_dir = tmp.path().join("sessions");
        let manager = SessionManager::new(sessions_dir.clone()).expect("new");
        let checkpoints = sessions_dir.join("checkpoints");
        fs::create_dir_all(&checkpoints).expect("create checkpoints dir");
        let legacy = checkpoints.join("offline_queue.json");
        let mut state = parked("text from the old global queue");
        state.session_id = Some("session-A".to_string());
        fs::write(
            &legacy,
            serde_json::to_string_pretty(&state).expect("serialize"),
        )
        .expect("write legacy queue");

        // A different session must not inherit it, and must not delete it.
        assert!(
            manager
                .load_offline_queue_state("session-B")
                .expect("load B")
                .is_none()
        );
        assert!(legacy.exists(), "another session's text must survive");

        // Its own session adopts it, and the global file is retired only
        // after the per-session copy is durably written.
        let adopted = manager
            .load_offline_queue_state("session-A")
            .expect("load A")
            .expect("adopted");
        assert_eq!(
            adopted.messages[0].display,
            "text from the old global queue"
        );
        assert!(!legacy.exists(), "adopted legacy queue is retired");
        assert!(
            checkpoints.join("session-A.offline_queue.json").exists(),
            "adoption writes the per-session file"
        );
        let again = manager
            .load_offline_queue_state("session-A")
            .expect("reload A")
            .expect("still parked");
        assert_eq!(again.messages[0].display, "text from the old global queue");
    }

    #[test]
    fn test_session_context_references_round_trip() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("new");
        let mut session = create_saved_session(
            &[make_test_message("user", "read @src/main.rs")],
            "deepseek-v4-pro",
            tmp.path(),
            0,
            None,
        );
        session.context_references.push(SessionContextReference {
            message_index: 0,
            reference: ContextReference {
                kind: ContextReferenceKind::File,
                source: ContextReferenceSource::AtMention,
                badge: "file".to_string(),
                label: "src/main.rs".to_string(),
                target: tmp.path().join("src/main.rs").display().to_string(),
                included: true,
                expanded: true,
                detail: Some("included".to_string()),
            },
        });

        let path = manager.save_session(&session).expect("save session");
        let loaded = manager
            .load_session(&session.metadata.id)
            .expect("load session");
        assert!(path.exists());
        assert_eq!(loaded.context_references, session.context_references);
    }

    #[test]
    fn test_checkpoint_rejects_newer_schema() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("new");
        let checkpoints = tmp.path().join("sessions").join("checkpoints");
        fs::create_dir_all(&checkpoints).expect("create checkpoints dir");
        let path = checkpoints.join("latest.json");
        fs::write(
            &path,
            r#"{
                "schema_version": 999,
                "metadata": {
                    "id": "sid",
                    "title": "bad",
                    "created_at": "2026-01-01T00:00:00Z",
                    "updated_at": "2026-01-01T00:00:00Z",
                    "message_count": 0,
                    "total_tokens": 0,
                    "model": "m",
                    "workspace": "/tmp",
                    "mode": null
                },
                "messages": [],
                "system_prompt": null
            }"#,
        )
        .expect("write checkpoint");

        let err = manager
            .load_legacy_checkpoint()
            .expect_err("should reject schema");
        assert!(err.to_string().contains("newer than supported"));

        // The same guard applies to per-session checkpoint files.
        fs::rename(&path, checkpoints.join("sid.json")).expect("rename to per-session file");
        let err = manager
            .load_session_checkpoint("sid")
            .expect_err("should reject schema");
        assert!(err.to_string().contains("newer than supported"));
    }

    #[test]
    fn test_load_session_rejects_newer_schema() {
        let tmp = tempdir().expect("tempdir");
        let sessions_dir = tmp.path().join("sessions");
        let manager = SessionManager::new(sessions_dir.clone()).expect("new");

        let id = "future-session";
        let path = sessions_dir.join(format!("{id}.json"));
        fs::write(
            &path,
            r#"{
                "schema_version": 999,
                "metadata": {
                    "id": "future-session",
                    "title": "future",
                    "created_at": "2026-01-01T00:00:00Z",
                    "updated_at": "2026-01-01T00:00:00Z",
                    "message_count": 0,
                    "total_tokens": 0,
                    "model": "m",
                    "workspace": "/tmp",
                    "mode": null
                },
                "messages": [],
                "system_prompt": null
            }"#,
        )
        .expect("write session");

        let err = manager.load_session(id).expect_err("should reject schema");
        assert!(
            err.to_string().contains("newer than supported"),
            "unexpected error: {err}"
        );
    }

    /// Regression for #337: metadata extraction skips the (potentially
    /// huge) `messages` array — it must succeed even when the messages
    /// array is megabytes long, and it must NOT confuse a `"metadata"`
    /// substring inside a message body for the real top-level key.
    #[test]
    fn extract_top_level_metadata_skips_huge_messages_array() {
        // Build a session JSON with a large `messages` payload that
        // contains the literal string `"metadata"` in a user message —
        // a naive `find("\"metadata\"")` would mis-target this.
        let big_text = format!(
            r#"this message references "metadata" inside it, repeated:{}"#,
            "x".repeat(20_000)
        );
        let json = format!(
            r#"{{
                "schema_version": 1,
                "metadata": {{
                    "id": "abc-123",
                    "title": "Real Session",
                    "created_at": "2026-01-01T00:00:00Z",
                    "updated_at": "2026-01-02T00:00:00Z",
                    "message_count": 12,
                    "total_tokens": 4096,
                    "model": "deepseek-v4-flash",
                    "workspace": "/tmp"
                }},
                "messages": [
                    {{ "role": "user", "content": [ {{ "Text": {{ "text": {big_text:?} }} }} ] }}
                ]
            }}"#
        );

        let extracted =
            extract_top_level_metadata(json.as_bytes()).expect("metadata extractable from prefix");
        assert_eq!(extracted.id, "abc-123");
        assert_eq!(extracted.title, "Real Session");
        assert_eq!(extracted.message_count, 12);
        assert_eq!(extracted.total_tokens, 4096);
    }

    #[test]
    fn extract_top_level_metadata_handles_braces_inside_strings() {
        // A title containing `{` and `}` inside the metadata block must
        // not throw off the brace counter.
        let json = r#"{
            "metadata": {
                "id": "x",
                "title": "weird { title } with braces",
                "created_at": "2026-01-01T00:00:00Z",
                "updated_at": "2026-01-01T00:00:00Z",
                "message_count": 0,
                "total_tokens": 0,
                "model": "m",
                "workspace": "/tmp"
            },
            "messages": []
        }"#;
        let extracted = extract_top_level_metadata(json.as_bytes())
            .expect("brace-in-string survives the scanner");
        assert_eq!(extracted.title, "weird { title } with braces");
    }

    #[test]
    fn saved_session_deserializes_without_artifacts_as_empty_registry() {
        let json = r#"{
            "schema_version": 1,
            "metadata": {
                "id": "legacy-session",
                "title": "legacy",
                "created_at": "2026-05-08T00:00:00Z",
                "updated_at": "2026-05-08T00:00:00Z",
                "message_count": 0,
                "total_tokens": 0,
                "model": "deepseek-v4-pro",
                "workspace": "/tmp"
            },
            "messages": [],
            "system_prompt": null
        }"#;

        let session: SavedSession = serde_json::from_str(json).expect("legacy session loads");
        assert!(session.artifacts.is_empty());
        assert!(session.last_auto_route.is_none());
        assert!(session.metadata.parent_session_id.is_none());
        assert!(session.metadata.forked_from_message_count.is_none());
    }

    #[test]
    fn fork_lineage_metadata_round_trips_and_formats() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("new");
        let parent = create_saved_session(
            &[
                make_test_message("user", "try approach A"),
                make_test_message("assistant", "A looks viable"),
            ],
            "deepseek-v4-pro",
            Path::new("/tmp"),
            42,
            None,
        );
        let mut forked = create_saved_session(
            &parent.messages,
            &parent.metadata.model,
            &parent.metadata.workspace,
            parent.metadata.total_tokens,
            None,
        );
        forked.metadata.mark_forked_from(&parent.metadata);

        manager.save_session(&forked).expect("save fork");
        let loaded = manager
            .load_session(&forked.metadata.id)
            .expect("load fork");

        assert_eq!(
            loaded.metadata.parent_session_id.as_deref(),
            Some(parent.metadata.id.as_str())
        );
        assert_eq!(loaded.metadata.forked_from_message_count, Some(2));
        let line = format_session_line(&loaded.metadata);
        assert!(line.contains("fork"));
        assert!(!line.contains(parent.metadata.id.as_str()));
    }

    #[test]
    fn save_and_load_session_preserves_artifact_metadata() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("new");
        let mut session = create_saved_session(
            &[make_test_message("user", "run tests")],
            "deepseek-v4-pro",
            Path::new("/tmp"),
            0,
            None,
        );
        session.artifacts.push(crate::artifacts::ArtifactRecord {
            id: "art_call_big".to_string(),
            kind: crate::artifacts::ArtifactKind::ToolOutput,
            session_id: session.metadata.id.clone(),
            tool_call_id: "call-big".to_string(),
            tool_name: "exec_shell".to_string(),
            created_at: Utc::now(),
            byte_size: 512_000,
            preview: "cargo test output".to_string(),
            storage_path: PathBuf::from("/tmp/tool_outputs/call-big.txt"),
        });

        manager.save_session(&session).expect("save");
        let loaded = manager.load_session(&session.metadata.id).expect("load");

        assert_eq!(loaded.artifacts, session.artifacts);
    }

    // ---- #406 prune_sessions_older_than ----
    //
    // The helper is a building block for the auto-archive design: it
    // removes session files older than a threshold while leaving fresh
    // ones (and the checkpoint directory) alone. Tests cover the empty
    // case, the all-fresh case, the all-stale case, and the mixed case.

    fn write_session_with_updated_at(
        manager: &SessionManager,
        id: &str,
        updated_at: DateTime<Utc>,
    ) {
        // Build a minimal SavedSession by hand so the test isn't tied
        // to whatever the helper functions emit; we just need a
        // metadata block whose `updated_at` matches the requested
        // value.
        write_session_record(manager, id, Path::new("/tmp"), updated_at);
    }

    #[test]
    fn prune_sessions_older_than_returns_zero_for_empty_dir() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("new");
        let pruned = manager
            .prune_sessions_older_than(std::time::Duration::from_secs(3600))
            .expect("prune");
        assert_eq!(pruned, 0);
    }

    #[test]
    fn prune_sessions_older_than_keeps_fresh_records() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("new");
        // All updated within the last hour.
        write_session_with_updated_at(
            &manager,
            "fresh-1",
            Utc::now() - chrono::Duration::minutes(30),
        );
        write_session_with_updated_at(
            &manager,
            "fresh-2",
            Utc::now() - chrono::Duration::minutes(5),
        );
        let pruned = manager
            .prune_sessions_older_than(std::time::Duration::from_secs(3600))
            .expect("prune");
        assert_eq!(pruned, 0);
        // Both files still on disk.
        assert_eq!(manager.list_sessions().expect("list").len(), 2);
    }

    #[test]
    fn prune_sessions_older_than_removes_stale_records() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("new");
        // Two stale records ≥7 days old.
        write_session_with_updated_at(&manager, "stale-1", Utc::now() - chrono::Duration::days(8));
        write_session_with_updated_at(&manager, "stale-2", Utc::now() - chrono::Duration::days(30));
        let pruned = manager
            .prune_sessions_older_than(std::time::Duration::from_secs(7 * 24 * 3600))
            .expect("prune");
        assert_eq!(pruned, 2);
        assert_eq!(manager.list_sessions().expect("list").len(), 0);
    }

    #[test]
    fn prune_sessions_older_than_only_removes_stale_records_in_mixed_dir() {
        let tmp = tempdir().expect("tempdir");
        let manager = SessionManager::new(tmp.path().join("sessions")).expect("new");
        write_session_with_updated_at(&manager, "fresh", Utc::now() - chrono::Duration::hours(1));
        write_session_with_updated_at(&manager, "stale", Utc::now() - chrono::Duration::days(60));
        let pruned = manager
            .prune_sessions_older_than(std::time::Duration::from_secs(7 * 24 * 3600))
            .expect("prune");
        assert_eq!(pruned, 1);
        let remaining = manager.list_sessions().expect("list");
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, "fresh");
    }

    #[test]
    fn prune_sessions_older_than_skips_checkpoint_directory() {
        // The checkpoint subsystem owns `<sessions>/checkpoints/` —
        // prune must not walk into it. The list_sessions iterator
        // already filters to top-level `*.json` files (skipping
        // sub-directories), so this test pins that behaviour.
        let tmp = tempdir().expect("tempdir");
        let sessions_dir = tmp.path().join("sessions");
        let manager = SessionManager::new(sessions_dir.clone()).expect("new");
        let checkpoint_dir = sessions_dir.join("checkpoints");
        fs::create_dir_all(&checkpoint_dir).expect("mkdir checkpoints");
        // Drop a stale-looking JSON inside the checkpoint dir; prune
        // should leave it alone.
        let checkpoint_file = checkpoint_dir.join("latest.json");
        fs::write(&checkpoint_file, "{}").expect("write checkpoint");

        write_session_with_updated_at(&manager, "stale", Utc::now() - chrono::Duration::days(60));
        let pruned = manager
            .prune_sessions_older_than(std::time::Duration::from_secs(7 * 24 * 3600))
            .expect("prune");
        assert_eq!(pruned, 1, "the top-level stale session should be removed");
        assert!(
            checkpoint_file.exists(),
            "checkpoint file should be untouched"
        );
    }

    #[test]
    fn test_load_offline_queue_rejects_newer_schema() {
        let tmp = tempdir().expect("tempdir");
        let sessions_dir = tmp.path().join("sessions");
        let manager = SessionManager::new(sessions_dir.clone()).expect("new");
        let checkpoints = sessions_dir.join("checkpoints");
        fs::create_dir_all(&checkpoints).expect("create checkpoints dir");
        let path = checkpoints.join("session-A.offline_queue.json");
        fs::write(
            &path,
            r#"{
                "schema_version": 999,
                "messages": [],
                "draft": null
            }"#,
        )
        .expect("write queue");

        let err = manager
            .load_offline_queue_state("session-A")
            .expect_err("should reject schema");
        assert!(
            err.to_string().contains("newer than supported"),
            "unexpected error: {err}"
        );

        // An unreadable *legacy* global queue is somebody else's problem to
        // recover: it must not fail this session's boot, and must survive.
        let legacy = checkpoints.join("offline_queue.json");
        fs::write(&legacy, r#"{"schema_version": 999}"#).expect("write legacy queue");
        assert!(
            manager
                .load_offline_queue_state("session-B")
                .expect("legacy corruption must not fail the boot")
                .is_none()
        );
        assert!(legacy.exists(), "unreadable legacy queue is left in place");
    }
    #[cfg(all(unix, not(target_os = "solaris")))]
    #[test]
    fn offline_queue_lease_releases_while_an_inherited_descriptor_remains_open() {
        let directory = tempfile::tempdir().expect("queue fixture");
        let manager = SessionManager::new(directory.path().join("sessions")).expect("manager");
        let editor = manager
            .acquire_offline_queue_lease("shared-session")
            .expect("first editor");
        // dup and fork share the same open-file description. Keep it alive
        // without a timing race or forking the multithreaded test process.
        let inherited = editor._file.try_clone().expect("inherited descriptor");
        let pending_write = std::sync::Arc::clone(&editor);
        drop(editor);
        assert_eq!(
            manager
                .acquire_offline_queue_lease("shared-session")
                .unwrap_err()
                .kind(),
            io::ErrorKind::WouldBlock,
            "pending writes retain the exclusive editor lease"
        );
        drop(pending_write);
        let next_editor = manager
            .acquire_offline_queue_lease("shared-session")
            .expect("completed editor releases even while a child retains its descriptor");
        drop(inherited);
        assert_eq!(
            manager
                .acquire_offline_queue_lease("shared-session")
                .unwrap_err()
                .kind(),
            io::ErrorKind::WouldBlock,
            "closing the old descriptor must not release the next editor's lock"
        );
        drop(next_editor);
        assert!(
            manager
                .acquire_offline_queue_lease("shared-session")
                .is_ok()
        );
    }

    #[test]
    fn offline_queue_lease_excludes_another_process_and_releases() {
        const PROBE: &str = "CODEWHALE_QUEUE_LEASE_PROBE_DIR";
        const HELD: &str = "CODEWHALE_QUEUE_LEASE_PROBE_HELD";
        if let Some(directory) = std::env::var_os(PROBE) {
            let manager = SessionManager::new(PathBuf::from(directory)).expect("child store");
            let result = manager.acquire_offline_queue_lease("shared-session");
            if std::env::var(HELD).as_deref() == Ok("1") {
                assert_eq!(result.unwrap_err().kind(), io::ErrorKind::WouldBlock);
            } else {
                assert!(result.is_ok(), "closed owner must release its kernel lock");
            }
            return;
        }
        let directory = tempfile::tempdir().expect("queue fixture");
        let sessions = directory.path().join("sessions");
        let manager = SessionManager::new(sessions.clone()).expect("parent store");
        let lease = manager
            .acquire_offline_queue_lease("shared-session")
            .expect("first editor");
        let probe = |held: bool| {
            let output = std::process::Command::new(
                std::env::current_exe().expect("test executable"),
            )
            .args([
                "--exact",
                "session_manager::tests::offline_queue_lease_excludes_another_process_and_releases",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(PROBE, &sessions)
            .env(HELD, if held { "1" } else { "0" })
            .output()
            .expect("second editor process");
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        };
        probe(true);
        let _different_session = manager
            .acquire_offline_queue_lease("different-session")
            .expect("unrelated queue is available");
        drop(lease);
        probe(false);
        for invalid in ["", "../session", "nested/session"] {
            assert_eq!(
                manager
                    .acquire_offline_queue_lease(invalid)
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::InvalidInput
            );
        }
    }
}
