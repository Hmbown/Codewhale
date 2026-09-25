//! Session receipts: what a session or turn actually did, read back from the
//! records Codewhale already persists.
//!
//! There is one builder. It reads two persisted shapes and nothing else:
//!
//! - a terminal session: the saved transcript (`sessions/<id>.json`, whose
//!   `tool_use`/`tool_result` blocks are the calls) plus the session's
//!   approval log (`sessions/<id>/approval_receipts.jsonl`);
//! - a Runtime thread (the app, `codewhale serve`): the thread's turn and item
//!   records plus the `approval.*` events in its append-only event log.
//!
//! Both are normalized into [`ToolStep`]s and [`ApprovalStep`]s and then
//! classified by the same code, so `/receipts`, `codewhale receipts`, and
//! `GET /v1/threads/{id}/receipt` cannot disagree about what happened.
//!
//! The builder only reads. It never calls a provider, runs a tool, or writes
//! a file. It exports no reasoning text and no raw tool output: commands,
//! queries, and error lines are bounded and passed through the shared secret
//! redactor. A fact the record does not hold is reported as not recorded,
//! never inferred from display text (see `docs/RECEIPTS.md`).

use std::collections::{BTreeSet, HashMap};

use chrono::{DateTime, Utc};
use codewhale_execpolicy::ApprovalMode;
use codewhale_models::{ContentBlock, Message};
use serde::Serialize;
use serde_json::Value;

use crate::approval_log::{ApprovalDecider, ApprovalOutcome, ApprovalReceipt, ApprovalReplay};
use crate::runtime_threads::{
    RuntimeEventRecord, RuntimeTurnStatus, ThreadRecord, TurnItemKind, TurnItemLifecycleStatus,
    TurnItemRecord, TurnRecord,
};

pub const RECEIPT_SCHEMA_ID: &str = "codewhale.receipt/v1";

/// Most actions one receipt lists. Totals always cover every action; only the
/// list is cut, and `omitted_actions` says by how many.
pub const MAX_RECEIPT_ACTIONS: usize = 2_000;
const MAX_COMMAND_CHARS: usize = 200;
const MAX_ERROR_CHARS: usize = 160;
const MAX_QUERY_CHARS: usize = 120;
const MAX_FILES_PER_ACTION: usize = 50;
const MAX_NESTED_CALLS: usize = 20;

/// Terminal sessions have kept an approval log since 0.9.10 (commit
/// 11717b48ff, released 2026-08-20). A session that started earlier has no
/// record of which calls asked first, so its receipt does not count calls that
/// "ran without asking".
const APPROVAL_LOG_SINCE: &str = "2026-08-20T00:00:00Z";

const CLAIM_CEILING: [&str; 3] = [
    "local_record_only",
    "not_safety_certification",
    "not_provider_compatibility_certification",
];

// ---------------------------------------------------------------------------
// Output shape
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Receipt {
    pub schema_id: &'static str,
    pub source: ReceiptSource,
    /// Set when the receipt covers one turn instead of the whole session.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn: Option<String>,
    /// Permission postures the covered turns ran under, in first-seen order
    /// (`Ask`, `Auto-Review`, `Full Access`, `Never`). Read from each turn's
    /// own record; empty when no turn recorded one.
    pub postures: Vec<&'static str>,
    pub totals: ReceiptTotals,
    pub actions: Vec<ReceiptAction>,
    /// Actions left off the list because it hit [`MAX_RECEIPT_ACTIONS`].
    pub omitted_actions: usize,
    /// Facts this record does not hold, stated instead of guessed.
    pub not_recorded: Vec<String>,
    pub claim_ceiling: [&'static str; 3],
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    /// A terminal session (saved transcript + approval log).
    Session,
    /// A Runtime thread (turn/item records + event log).
    Thread,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ReceiptSource {
    pub kind: SourceKind,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct ReceiptTotals {
    /// Distinct paths changed by file tools.
    pub files_changed: usize,
    pub files_created: usize,
    pub files_deleted: usize,
    /// Sum over changes whose line counts are recorded.
    pub lines_added: u64,
    pub lines_removed: u64,
    /// False when at least one file change has no recorded line counts, so
    /// the sums above are a floor.
    pub line_counts_complete: bool,
    pub commands: usize,
    pub commands_failed: usize,
    pub code_runs: usize,
    pub network: usize,
    pub mcp_calls: usize,
    pub plugin_calls: usize,
    pub subagents: usize,
    pub approvals: ApprovalTotals,
    /// File changes, commands, code runs, web and MCP calls, and agents that
    /// ran with no approval on record: the posture, an allow rule, or a
    /// remembered grant let them run without a prompt. Zero, with a
    /// `not_recorded` note, for a session older than its approval log.
    pub ran_without_asking: usize,
    /// Actions that ran and failed, plus failed turns.
    pub failures: usize,
    /// Reads, searches, and other calls that are counted but not listed
    /// unless they failed.
    pub other_tool_calls: usize,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct ApprovalTotals {
    pub total: usize,
    pub approved: usize,
    pub denied: usize,
    pub timed_out: usize,
    /// Cancelled, or resolved by the host because nobody could be asked.
    pub not_answered: usize,
    pub pending: usize,
    pub by_you: usize,
    pub by_session_rule: usize,
    pub by_posture: usize,
    /// Approved or denied, but the record predates Codewhale keeping who
    /// decided (or a sub-agent's request, which does not carry it yet).
    pub decider_not_recorded: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ReceiptAction {
    /// 1-based position in the session's action order.
    pub seq: usize,
    /// Runtime turn id, or the 1-based turn number in a terminal session.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call_id: Option<String>,
    /// The tool name exactly as called.
    pub tool: String,
    #[serde(flatten)]
    pub what: ActionKind,
    pub status: ActionStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval: Option<ApprovalFact>,
    /// First line of the failure, bounded and redacted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ActionKind {
    FileChange {
        files: Vec<FileTouch>,
    },
    Command {
        command: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cwd: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        exit_code: Option<i64>,
    },
    Code {
        #[serde(skip_serializing_if = "Option::is_none")]
        exit_code: Option<i64>,
        /// Tool calls the program made (`execute_tools`), when recorded.
        #[serde(skip_serializing_if = "Vec::is_empty")]
        nested: Vec<NestedCall>,
    },
    Network {
        action: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        host: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        query: Option<String>,
    },
    Mcp {
        #[serde(skip_serializing_if = "Option::is_none")]
        server: Option<String>,
        plugin: bool,
    },
    Subagent {
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        agent_id: Option<String>,
        /// Last status the record holds for this agent.
        #[serde(skip_serializing_if = "Option::is_none")]
        outcome: Option<String>,
    },
    /// An approval with no matching call in the record (for example a
    /// sub-agent's request).
    Approval,
    /// Any other tool. Listed only when it failed.
    Tool,
    /// A Runtime turn that ended in failure.
    TurnFailed,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FileChangeKind {
    Edited,
    Created,
    Deleted,
    /// Written whole; the record does not say whether the file existed.
    Written,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FileTouch {
    pub path: String,
    pub change: FileChangeKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lines_added: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lines_removed: Option<u64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct NestedCall {
    pub tool: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elapsed_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionStatus {
    Ok,
    Failed,
    /// Held at approval: denied, timed out, or never answered.
    NotRun,
    Interrupted,
    Running,
    /// No result is in the record.
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecisionLabel {
    Approved,
    /// Approved with a wider sandbox after a sandbox denial.
    ApprovedWithPolicy,
    Denied,
    TimedOut,
    Cancelled,
    /// The host answered because nobody could be asked.
    Unavailable,
    Pending,
}

impl ApprovalDecisionLabel {
    fn ran(self) -> bool {
        matches!(self, Self::Approved | Self::ApprovedWithPolicy)
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct ApprovalFact {
    pub decision: ApprovalDecisionLabel,
    /// `None` when the decision names its own cause (timeout, pending) or the
    /// record predates deciders.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decided_by: Option<ApprovalDecider>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<DateTime<Utc>>,
}

// ---------------------------------------------------------------------------
// Normalized input
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StepOutcome {
    Ok,
    Failed,
    Interrupted,
    Running,
    Unknown,
}

/// One tool call as a persisted record holds it.
#[derive(Debug, Clone)]
struct ToolStep {
    turn: Option<String>,
    call_id: Option<String>,
    name: String,
    input: Value,
    outcome: StepOutcome,
    output: Option<String>,
    metadata: Option<Value>,
    started_at: Option<DateTime<Utc>>,
    ended_at: Option<DateTime<Utc>>,
}

/// A turn and the permission posture its own record names, if any.
type TurnPosture = (String, Option<&'static str>);

#[derive(Debug, Clone)]
struct ApprovalStep {
    turn: Option<String>,
    call_id: Option<String>,
    tool: String,
    fact: ApprovalFact,
}

// ---------------------------------------------------------------------------
// Entry points
// ---------------------------------------------------------------------------

/// Receipt for a terminal session: its transcript plus its approval log.
/// `turn` is a 1-based turn number; `None` covers the whole session.
pub(crate) fn session_receipt(
    source: ReceiptSource,
    messages: &[Message],
    approval_receipts: &[ApprovalReceipt],
    turn: Option<&str>,
) -> anyhow::Result<Receipt> {
    let mut notes = BTreeSet::new();
    let (steps, turn_postures) = steps_from_messages(messages);
    let approvals = match ApprovalReplay::from_receipts(approval_receipts) {
        Ok(replay) => approvals_from_replay(&replay),
        Err(error) => {
            notes.insert(format!(
                "Approvals: the session's approval log did not replay ({error}), so approvals are left out."
            ));
            Vec::new()
        }
    };
    if let Some(turn) = turn {
        let known = steps.iter().any(|step| step.turn.as_deref() == Some(turn));
        if !known && turn.parse::<usize>().is_err() {
            anyhow::bail!("turn '{turn}' is not a turn number in this session");
        }
    }
    notes.insert(
        "Timestamps: a terminal session saves calls in order, not when each one ran.".to_string(),
    );
    let log_since = DateTime::parse_from_rfc3339(APPROVAL_LOG_SINCE)
        .map(|at| at.with_timezone(&Utc))
        .ok();
    let approvals_recorded = match (source.started_at, log_since) {
        (Some(started), Some(since)) => started >= since,
        _ => true,
    };
    if !approvals_recorded {
        notes.insert(
            "Approvals: this session started before Codewhale kept an approval log (0.9.10, 2026-08-20), so it cannot show which calls asked first.".to_string(),
        );
    }
    Ok(assemble(
        source,
        steps,
        approvals,
        Vec::new(),
        Assembly {
            turn,
            kind: SourceKind::Session,
            turn_postures,
            approvals_recorded,
        },
        notes,
    ))
}

/// Receipt for a Runtime thread from its snapshot and event log. `turn` is a
/// turn id of this thread; `None` covers every turn.
pub(crate) fn thread_receipt(
    thread: &ThreadRecord,
    turns: &[TurnRecord],
    items: &[TurnItemRecord],
    events: &[RuntimeEventRecord],
    turn: Option<&str>,
) -> anyhow::Result<Receipt> {
    if let Some(turn) = turn
        && !turns.iter().any(|record| record.id == turn)
    {
        anyhow::bail!("turn '{turn}' does not belong to thread '{}'", thread.id);
    }
    let mut ordered: Vec<&TurnRecord> = turns.iter().collect();
    ordered.sort_by_key(|record| record.created_at);
    let items_by_id: HashMap<&str, &TurnItemRecord> =
        items.iter().map(|item| (item.id.as_str(), item)).collect();
    let mut steps = Vec::new();
    let mut failures = Vec::new();
    let mut turn_postures: Vec<TurnPosture> = Vec::new();
    for record in &ordered {
        turn_postures.push((
            record.id.clone(),
            record.permission_posture.as_deref().and_then(posture_label),
        ));
        let mut turn_items: Vec<&TurnItemRecord> = record
            .item_ids
            .iter()
            .filter_map(|id| items_by_id.get(id.as_str()).copied())
            .collect();
        // Items a turn record does not list yet (a live turn) still belong
        // to it by their own turn id.
        for item in items {
            if item.turn_id == record.id && !record.item_ids.contains(&item.id) {
                turn_items.push(item);
            }
        }
        for item in turn_items {
            if let Some(step) = step_from_item(item) {
                steps.push(step);
            }
        }
        if record.status == RuntimeTurnStatus::Failed {
            failures.push((record.id.clone(), record.ended_at, record.error.clone()));
        }
    }
    let approvals = approvals_from_events(events);
    let source = ReceiptSource {
        kind: SourceKind::Thread,
        id: thread.id.clone(),
        title: thread.title.clone().or_else(|| {
            ordered
                .first()
                .map(|record| record.input_summary.clone())
                .filter(|text| !text.trim().is_empty())
        }),
        workspace: Some(thread.workspace.display().to_string()),
        model: Some(thread.model.clone()),
        started_at: Some(thread.created_at),
        updated_at: Some(thread.updated_at),
    };
    Ok(assemble(
        source,
        steps,
        approvals,
        failures,
        Assembly {
            turn,
            kind: SourceKind::Thread,
            turn_postures,
            approvals_recorded: true,
        },
        BTreeSet::new(),
    ))
}

// ---------------------------------------------------------------------------
// Loading from disk (`codewhale receipts`)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ReceiptFormat {
    Md,
    Json,
}

/// Receipt for a Runtime thread read straight from its store.
pub(crate) fn thread_receipt_from_store(
    store: &crate::runtime_threads::RuntimeThreadStore,
    thread_id: &str,
    turn: Option<&str>,
) -> anyhow::Result<Receipt> {
    let thread = store.load_thread(thread_id)?;
    let turns = store.list_turns_for_thread(thread_id)?;
    let turn_ids: Vec<String> = turns.iter().map(|record| record.id.clone()).collect();
    let items: Vec<TurnItemRecord> = store
        .list_items_for_turns_map(&turn_ids)?
        .into_values()
        .flatten()
        .collect();
    let events: Vec<RuntimeEventRecord> = store
        .events_since(thread_id, None)?
        .into_iter()
        .filter(|event| event.event.starts_with("approval."))
        .collect();
    thread_receipt(&thread, &turns, &items, &events, turn)
}

pub(crate) fn session_source(metadata: &crate::session_manager::SessionMetadata) -> ReceiptSource {
    ReceiptSource {
        kind: SourceKind::Session,
        id: metadata.id.clone(),
        title: Some(metadata.title.clone()).filter(|title| !title.trim().is_empty()),
        workspace: Some(metadata.workspace.display().to_string()),
        model: Some(metadata.model.clone()).filter(|model| !model.is_empty()),
        started_at: Some(metadata.created_at),
        updated_at: Some(metadata.updated_at),
    }
}

/// `codewhale receipts [ID|--last] [--turn T] [--format md|json]`.
///
/// `ID` is a saved session id (or unique prefix) or a Runtime thread id
/// (`thr_…`). With neither an id nor `--last`, the most recently updated
/// session or thread is used. Reads only.
pub(crate) fn run_receipts_command(
    id: Option<&str>,
    turn: Option<&str>,
    format: ReceiptFormat,
) -> anyhow::Result<()> {
    use anyhow::Context as _;
    let sessions = crate::session_manager::SessionManager::default_location()
        .context("could not open the saved sessions directory")?;
    let runtime_root = crate::runtime_threads::RuntimeThreadManagerConfig::from_task_data_dir(
        crate::task_manager::default_tasks_dir(),
    )
    .data_dir;
    let store = crate::runtime_threads::RuntimeThreadStore::open_read_only(runtime_root)?;

    let receipt = match id.map(str::trim).filter(|id| !id.is_empty()) {
        Some(id) if id.starts_with("thr_") => {
            let store = store.context("no Runtime thread store exists on this machine")?;
            thread_receipt_from_store(&store, id, turn)?
        }
        Some(prefix) => {
            let id = sessions.resolve_session_id_prefix(prefix)?;
            let session = sessions.load_session_snapshot(&id)?;
            session_receipt(
                session_source(&session.metadata),
                &session.messages,
                &session.approval_receipts,
                turn,
            )?
        }
        None => {
            let latest_session = sessions.list_sessions()?.into_iter().next();
            let latest_thread = store
                .as_ref()
                .map(|store| store.list_threads())
                .transpose()?
                .and_then(|threads| threads.into_iter().find(|thread| !thread.archived));
            let thread_is_newer = match (&latest_session, &latest_thread) {
                (Some(session), Some(thread)) => thread.updated_at > session.updated_at,
                (None, Some(_)) => true,
                _ => false,
            };
            match (latest_session, latest_thread, store.as_ref()) {
                (_, Some(thread), Some(store)) if thread_is_newer => {
                    thread_receipt_from_store(store, &thread.id, turn)?
                }
                (Some(metadata), _, _) => {
                    let session = sessions.load_session_snapshot(&metadata.id)?;
                    session_receipt(
                        session_source(&session.metadata),
                        &session.messages,
                        &session.approval_receipts,
                        turn,
                    )?
                }
                _ => anyhow::bail!("no saved session or Runtime thread to read"),
            }
        }
    };
    let text = match format {
        ReceiptFormat::Md => render_markdown(&receipt),
        ReceiptFormat::Json => render_json(&receipt),
    };
    println!("{}", text.trim_end());
    Ok(())
}

// ---------------------------------------------------------------------------
// Readers: persisted shape -> normalized steps
// ---------------------------------------------------------------------------

/// Tool calls in transcript order, plus each turn's posture. A turn starts at
/// a real user prompt, by the same rule edit-last-turn and titles use
/// ([`crate::runtime_handoff::classify_user_turn_prompt`]); runtime-injected
/// messages and tool results do not start one.
fn steps_from_messages(messages: &[Message]) -> (Vec<ToolStep>, Vec<TurnPosture>) {
    let mut steps: Vec<ToolStep> = Vec::new();
    let mut by_id: HashMap<String, usize> = HashMap::new();
    let mut turn_postures: Vec<TurnPosture> = Vec::new();
    let mut turn = 0usize;
    for message in messages {
        if crate::runtime_handoff::classify_user_turn_prompt(message)
            != crate::runtime_handoff::UserTurnPromptKind::NotPrompt
        {
            turn += 1;
            turn_postures.push((turn.to_string(), turn_meta_posture(message)));
        }
        for block in &message.content {
            match block {
                ContentBlock::ToolUse {
                    id, name, input, ..
                }
                | ContentBlock::ServerToolUse { id, name, input } => {
                    by_id.insert(id.clone(), steps.len());
                    steps.push(ToolStep {
                        turn: (turn > 0).then(|| turn.to_string()),
                        call_id: Some(id.clone()),
                        name: name.clone(),
                        input: input.clone(),
                        outcome: StepOutcome::Unknown,
                        output: None,
                        metadata: None,
                        started_at: None,
                        ended_at: None,
                    });
                }
                ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                    ..
                } => {
                    if let Some(&index) = by_id.get(tool_use_id) {
                        let step = &mut steps[index];
                        step.outcome = if is_error.unwrap_or(false) {
                            StepOutcome::Failed
                        } else {
                            StepOutcome::Ok
                        };
                        step.output = Some(content.clone());
                    }
                }
                _ => {}
            }
        }
    }
    (steps, turn_postures)
}

/// The posture line the engine writes into a prompt's `<turn_meta>` block
/// ([`crate::core::engine::PERMISSION_POSTURE_LINE`]).
fn turn_meta_posture(message: &Message) -> Option<&'static str> {
    let (_, meta) = crate::runtime_handoff::turn_metadata_text(message)?;
    meta.lines().find_map(|line| {
        line.trim()
            .strip_prefix(crate::core::engine::PERMISSION_POSTURE_LINE)
            .and_then(posture_label)
    })
}

/// The chip label for a posture the host wrote: a `<turn_meta>` label
/// (`Full Access`) or a Runtime turn's wire value (`full_access`). Anything
/// else is not a posture and yields `None`.
fn posture_label(value: &str) -> Option<&'static str> {
    let value = value.trim();
    [
        ApprovalMode::Suggest,
        ApprovalMode::Auto,
        ApprovalMode::Bypass,
        ApprovalMode::Never,
    ]
    .into_iter()
    .map(ApprovalMode::permission_chip_label)
    .find(|label| label.eq_ignore_ascii_case(value))
    .or_else(|| ApprovalMode::from_config_value(value).map(ApprovalMode::permission_chip_label))
}

fn step_from_item(item: &TurnItemRecord) -> Option<ToolStep> {
    if !matches!(
        item.kind,
        TurnItemKind::ToolCall | TurnItemKind::FileChange | TurnItemKind::CommandExecution
    ) {
        return None;
    }
    let metadata = item.metadata.clone();
    let meta = metadata.as_ref();
    let name = meta
        .and_then(|meta| meta.get("tool_name"))
        .and_then(Value::as_str)?
        .to_string();
    let call_id = meta
        .and_then(|meta| meta.get("tool_use_id").or_else(|| meta.get("tool_call_id")))
        .and_then(Value::as_str)
        .map(str::to_string);
    let input = meta
        .and_then(|meta| meta.get("tool_input"))
        .map(|raw| match raw {
            Value::String(text) => serde_json::from_str(text).unwrap_or(Value::Null),
            other => other.clone(),
        })
        .unwrap_or(Value::Null);
    let outcome = match item.status {
        TurnItemLifecycleStatus::Completed => StepOutcome::Ok,
        TurnItemLifecycleStatus::Failed => StepOutcome::Failed,
        TurnItemLifecycleStatus::Interrupted | TurnItemLifecycleStatus::Canceled => {
            StepOutcome::Interrupted
        }
        TurnItemLifecycleStatus::Queued | TurnItemLifecycleStatus::InProgress => {
            StepOutcome::Running
        }
    };
    let finished = !matches!(outcome, StepOutcome::Running);
    Some(ToolStep {
        turn: Some(item.turn_id.clone()),
        call_id,
        name,
        input,
        outcome,
        output: finished.then(|| item.detail.clone()).flatten(),
        metadata,
        started_at: item.started_at,
        ended_at: item.ended_at,
    })
}

fn approvals_from_replay(replay: &ApprovalReplay) -> Vec<ApprovalStep> {
    let mut out = Vec::new();
    for completed in &replay.completed {
        let (decision, implied) = match &completed.outcome {
            ApprovalOutcome::ApprovedOnce => (ApprovalDecisionLabel::Approved, None),
            ApprovalOutcome::Denied => (ApprovalDecisionLabel::Denied, None),
            ApprovalOutcome::Timeout => (ApprovalDecisionLabel::TimedOut, None),
            ApprovalOutcome::Cancelled => (
                ApprovalDecisionLabel::Cancelled,
                Some(ApprovalDecider::Host),
            ),
            ApprovalOutcome::Unavailable => (
                ApprovalDecisionLabel::Unavailable,
                Some(ApprovalDecider::Host),
            ),
            ApprovalOutcome::RetryWithPolicy { .. } => {
                (ApprovalDecisionLabel::ApprovedWithPolicy, None)
            }
        };
        out.push(ApprovalStep {
            turn: None,
            call_id: Some(completed.ask.approval_id().to_string()),
            tool: completed.ask.tool_name().unwrap_or("unknown").to_string(),
            fact: ApprovalFact {
                decision,
                decided_by: completed.decided_by.or(implied),
                at: Some(completed.decided_at),
            },
        });
    }
    for ask in &replay.unmatched_asks {
        out.push(ApprovalStep {
            turn: None,
            call_id: Some(ask.approval_id().to_string()),
            tool: ask.tool_name().unwrap_or("unknown").to_string(),
            fact: ApprovalFact {
                decision: ApprovalDecisionLabel::Pending,
                decided_by: None,
                at: Some(ask.created_at()),
            },
        });
    }
    out
}

/// Pair `approval.required` with `approval.decided` by approval id. Who
/// decided is read from the flags the Runtime writes on the decision event:
/// `timeout`, `cancelled`, `posture`, `auto` (+ `grant_id` for a session
/// rule). A decision with none of them came from a client acting for a
/// person.
fn approvals_from_events(events: &[RuntimeEventRecord]) -> Vec<ApprovalStep> {
    let mut order: Vec<String> = Vec::new();
    let mut steps: HashMap<String, ApprovalStep> = HashMap::new();
    for event in events {
        let payload = &event.payload;
        let Some(approval_id) = payload
            .get("approval_id")
            .or_else(|| payload.get("id"))
            .and_then(Value::as_str)
        else {
            continue;
        };
        match event.event.as_str() {
            "approval.required" => {
                if !steps.contains_key(approval_id) {
                    order.push(approval_id.to_string());
                }
                steps.insert(
                    approval_id.to_string(),
                    ApprovalStep {
                        turn: event.turn_id.clone(),
                        call_id: payload
                            .get("tool_call_id")
                            .and_then(Value::as_str)
                            .map(str::to_string),
                        tool: payload
                            .get("tool_name")
                            .and_then(Value::as_str)
                            .unwrap_or("unknown")
                            .to_string(),
                        fact: ApprovalFact {
                            decision: ApprovalDecisionLabel::Pending,
                            decided_by: None,
                            at: Some(event.timestamp),
                        },
                    },
                );
            }
            "approval.decided" => {
                let flag = |key: &str| payload.get(key).and_then(Value::as_bool) == Some(true);
                let allowed = payload.get("decision").and_then(Value::as_str) == Some("allow");
                let decision = if allowed {
                    ApprovalDecisionLabel::Approved
                } else if flag("timeout") {
                    ApprovalDecisionLabel::TimedOut
                } else if flag("cancelled") {
                    ApprovalDecisionLabel::Cancelled
                } else {
                    ApprovalDecisionLabel::Denied
                };
                let decided_by = if flag("timeout") {
                    None
                } else if flag("cancelled") {
                    Some(ApprovalDecider::Host)
                } else if payload.get("posture").is_some() {
                    Some(ApprovalDecider::Posture)
                } else if flag("auto") {
                    if payload
                        .get("grant_id")
                        .is_some_and(|grant| !grant.is_null())
                    {
                        Some(ApprovalDecider::SessionRule)
                    } else {
                        Some(ApprovalDecider::Posture)
                    }
                } else {
                    Some(ApprovalDecider::User)
                };
                let fact = ApprovalFact {
                    decision,
                    decided_by,
                    at: Some(event.timestamp),
                };
                match steps.get_mut(approval_id) {
                    Some(step) => step.fact = fact,
                    None => {
                        order.push(approval_id.to_string());
                        steps.insert(
                            approval_id.to_string(),
                            ApprovalStep {
                                turn: event.turn_id.clone(),
                                call_id: payload
                                    .get("tool_call_id")
                                    .and_then(Value::as_str)
                                    .map(str::to_string),
                                tool: "unknown".to_string(),
                                fact,
                            },
                        );
                    }
                }
            }
            _ => {}
        }
    }
    order
        .into_iter()
        .filter_map(|id| steps.remove(&id))
        .collect()
}

// ---------------------------------------------------------------------------
// Classification: normalized step -> action
// ---------------------------------------------------------------------------

enum Classified {
    Listed(ActionKind),
    /// Counted as an other tool call; listed only if it failed.
    Other,
    /// A later call on an agent the receipt already lists (status, wait,
    /// cancel): it updates that agent's outcome instead of adding a line.
    AgentFollowUp {
        agent_id: String,
        status: Option<String>,
    },
}

fn classify(step: &ToolStep, notes: &mut BTreeSet<String>) -> Classified {
    let structured = structured_output(step);
    let facts = step.metadata.as_ref().or(structured.as_ref());
    if let Some(files) = mutation_files(facts) {
        return Classified::Listed(ActionKind::FileChange { files });
    }
    let semantic = crate::tools::canonical_action::canonical_action_alias(&step.name, &step.input);
    match semantic {
        "write_file" | "edit_file" | "apply_patch" | "fim_edit" => {
            let files = files_from_input(semantic, &step.input, step.outcome);
            if files.is_empty() {
                return Classified::Other;
            }
            if files.iter().any(|file| file.lines_added.is_none()) {
                notes.insert(
                    "Line counts: some file changes have no saved diff, so their +/- counts are not recorded.".to_string(),
                );
            }
            Classified::Listed(ActionKind::FileChange { files })
        }
        "exec_shell" | "task_shell_start" | "task_gate_run" | "run_tests" | "run_verifiers" => {
            let exit_code = number(facts, &["exit_code", "return_code"])
                .or_else(|| closing_exit_code(step.output.as_deref()));
            if exit_code.is_none() && matches!(step.outcome, StepOutcome::Ok) {
                notes.insert(
                    "Exit codes: calls that saved no structured result (terminal sessions) show pass or fail, not the exit code.".to_string(),
                );
            }
            Classified::Listed(ActionKind::Command {
                command: command_text(&step.name, semantic, &step.input),
                cwd: string_field(
                    Some(&step.input),
                    &["cwd", "working_dir", "workdir", "workspace"],
                )
                .or_else(|| string_field(facts, &["working_dir", "cwd"])),
                exit_code,
            })
        }
        "code_execution" | "js_execution" | "execute_tools" | "rlm_eval" => {
            Classified::Listed(ActionKind::Code {
                exit_code: number(facts, &["return_code", "exit_code"]),
                nested: nested_calls(structured.as_ref().or(facts)),
            })
        }
        "web_search" | "fetch_url" | "web.run" | "rlm_open" | "git_fetch" => {
            let url = string_field(Some(&step.input), &["url", "uri"]);
            let host = url
                .as_deref()
                .and_then(|url| reqwest::Url::parse(url).ok())
                .and_then(|url| url.host_str().map(str::to_string))
                .or_else(|| {
                    (semantic == "git_fetch")
                        .then(|| string_field(Some(&step.input), &["remote"]))
                        .flatten()
                });
            let action = match semantic {
                "web_search" => "search",
                "git_fetch" => "git_fetch",
                _ if url.is_some() => "fetch",
                _ => "request",
            };
            let query = string_field(Some(&step.input), &["query", "q", "search_query"])
                .map(|query| bounded(&redact(&query), MAX_QUERY_CHARS));
            Classified::Listed(ActionKind::Network {
                action: action.to_string(),
                host,
                query,
            })
        }
        name if name.starts_with("github_") => Classified::Listed(ActionKind::Network {
            action: name.trim_start_matches("github_").to_string(),
            host: Some("github.com".to_string()),
            query: None,
        }),
        name if name.starts_with("mcp_") => {
            let server = crate::tui::approval::connected_app_server(name).map(str::to_string);
            let plugin = server
                .as_deref()
                .is_some_and(|server| server.starts_with("plugin"));
            Classified::Listed(ActionKind::Mcp { server, plugin })
        }
        "agent" => {
            let action = step
                .input
                .get("action")
                .and_then(Value::as_str)
                .unwrap_or("start");
            let agent_id = string_field(facts, &["agent_id"]);
            let status = string_field(facts, &["status"]);
            if action == "start" {
                Classified::Listed(ActionKind::Subagent {
                    name: string_field(Some(&step.input), &["name"])
                        .or_else(|| string_field(facts, &["name"])),
                    agent_id,
                    outcome: status,
                })
            } else if let Some(agent_id) = agent_id {
                Classified::AgentFollowUp { agent_id, status }
            } else {
                Classified::Other
            }
        }
        _ => Classified::Other,
    }
}

/// The host-owned JSON a tool returned as its text result (agent, code,
/// execute_tools), when the persisted record has no structured metadata. A
/// leading approval note is skipped. Anything that is not a JSON object is
/// ignored rather than read as prose.
fn structured_output(step: &ToolStep) -> Option<Value> {
    let output = step.output.as_deref()?.trim_start();
    let body = if output.starts_with("[approval] ") {
        output.split_once("\n\n").map(|(_, rest)| rest)?
    } else {
        output
    };
    let value: Value = serde_json::from_str(body.trim()).ok()?;
    value.is_object().then_some(value)
}

fn mutation_files(facts: Option<&Value>) -> Option<Vec<FileTouch>> {
    let mutation = facts?.get("mutation")?;
    let files = mutation.get("files")?.as_array()?;
    let counts = mutation
        .get("diff")
        .and_then(Value::as_str)
        .map(diff_counts_by_path)
        .unwrap_or_default();
    let mut out: Vec<FileTouch> = files
        .iter()
        .filter_map(|file| {
            let path = file.get("path")?.as_str()?.to_string();
            let change = match file.get("outcome").and_then(Value::as_str) {
                Some("created") => FileChangeKind::Created,
                Some("deleted") => FileChangeKind::Deleted,
                _ => FileChangeKind::Edited,
            };
            let (added, removed) = counts.get(&path).copied().unwrap_or((0, 0));
            Some(FileTouch {
                path,
                change,
                lines_added: Some(added),
                lines_removed: Some(removed),
            })
        })
        .collect();
    if let Some(renames) = mutation.get("renames").and_then(Value::as_array) {
        for rename in renames {
            if let Some(to) = rename.get("to").and_then(Value::as_str) {
                out.push(FileTouch {
                    path: to.to_string(),
                    change: FileChangeKind::Created,
                    lines_added: Some(0),
                    lines_removed: Some(0),
                });
            }
            if let Some(from) = rename.get("from").and_then(Value::as_str) {
                out.push(FileTouch {
                    path: from.to_string(),
                    change: FileChangeKind::Deleted,
                    lines_added: Some(0),
                    lines_removed: Some(0),
                });
            }
        }
    }
    out.truncate(MAX_FILES_PER_ACTION);
    (!out.is_empty()).then_some(out)
}

/// Per-file `(+, -)` from a unified diff, keyed by the `+++ b/<path>` (or
/// `--- a/<path>` for deletions) header.
fn diff_counts_by_path(diff: &str) -> HashMap<String, (u64, u64)> {
    let mut counts: HashMap<String, (u64, u64)> = HashMap::new();
    let mut current: Option<String> = None;
    let mut pending_old: Option<String> = None;
    for line in diff.lines() {
        if let Some(rest) = line.strip_prefix("--- ") {
            pending_old = header_path(rest);
            continue;
        }
        if let Some(rest) = line.strip_prefix("+++ ") {
            current = header_path(rest).or_else(|| pending_old.take());
            if let Some(path) = &current {
                counts.entry(path.clone()).or_default();
            }
            continue;
        }
        if line.starts_with("diff --git ") {
            current = None;
            pending_old = None;
            continue;
        }
        let Some(path) = &current else { continue };
        let entry = counts.entry(path.clone()).or_default();
        if line.starts_with('+') {
            entry.0 += 1;
        } else if line.starts_with('-') {
            entry.1 += 1;
        }
    }
    counts
}

fn header_path(rest: &str) -> Option<String> {
    let path = rest.split('\t').next()?.trim();
    if path == "/dev/null" {
        return None;
    }
    Some(
        path.strip_prefix("a/")
            .or_else(|| path.strip_prefix("b/"))
            .unwrap_or(path)
            .to_string(),
    )
}

/// File changes from a call's own input, for records without a saved diff.
/// A failed call changed nothing, so it lists its paths with no counts.
fn files_from_input(semantic: &str, input: &Value, outcome: StepOutcome) -> Vec<FileTouch> {
    let succeeded = outcome == StepOutcome::Ok;
    let path = string_field(Some(input), &["path", "file_path", "filePath"]);
    match semantic {
        "write_file" => path
            .map(|path| {
                vec![FileTouch {
                    path,
                    change: FileChangeKind::Written,
                    lines_added: None,
                    lines_removed: None,
                }]
            })
            .unwrap_or_default(),
        "edit_file" | "fim_edit" => {
            let Some(path) = path else {
                return Vec::new();
            };
            let (added, removed) = if succeeded {
                edit_line_counts(input)
            } else {
                (None, None)
            };
            vec![FileTouch {
                path,
                change: FileChangeKind::Edited,
                lines_added: added,
                lines_removed: removed,
            }]
        }
        "apply_patch" => {
            let Some(patch) = string_field(Some(input), &["patch", "input"]) else {
                return path
                    .map(|path| {
                        vec![FileTouch {
                            path,
                            change: FileChangeKind::Edited,
                            lines_added: None,
                            lines_removed: None,
                        }]
                    })
                    .unwrap_or_default();
            };
            let mut files = patch_files(&patch, path.as_deref());
            if !succeeded {
                for file in &mut files {
                    file.lines_added = None;
                    file.lines_removed = None;
                }
            }
            files
        }
        _ => Vec::new(),
    }
}

/// `(+, -)` from an edit call's replacement pairs: the lines of every
/// replacement text against the lines of every searched text.
fn edit_line_counts(input: &Value) -> (Option<u64>, Option<u64>) {
    const SEARCH: &[&str] = &["search", "old_string", "old_str", "oldText", "old_text"];
    const REPLACE: &[&str] = &[
        "replace",
        "new_string",
        "new_str",
        "newText",
        "new_text",
        "replacement",
    ];
    let pairs: Vec<&Value> = match input.get("edits").and_then(Value::as_array) {
        Some(edits) => edits.iter().collect(),
        None => vec![input],
    };
    let mut added = 0u64;
    let mut removed = 0u64;
    let mut any = false;
    for pair in pairs {
        let old = string_field(Some(pair), SEARCH);
        let new = string_field(Some(pair), REPLACE);
        if old.is_none() && new.is_none() {
            continue;
        }
        any = true;
        removed += line_count(old.as_deref().unwrap_or(""));
        added += line_count(new.as_deref().unwrap_or(""));
    }
    if any {
        (Some(added), Some(removed))
    } else {
        (None, None)
    }
}

fn line_count(text: &str) -> u64 {
    if text.is_empty() {
        0
    } else {
        text.lines().count() as u64
    }
}

/// Files and counts from a patch the call supplied: `*** Add/Update/Delete
/// File:` envelopes or unified-diff headers.
fn patch_files(patch: &str, fallback_path: Option<&str>) -> Vec<FileTouch> {
    let mut files: Vec<FileTouch> = Vec::new();
    let mut pending_old: Option<String> = None;
    for line in patch.lines() {
        let envelope = [
            ("*** Add File: ", FileChangeKind::Created),
            ("*** Update File: ", FileChangeKind::Edited),
            ("*** Delete File: ", FileChangeKind::Deleted),
        ]
        .into_iter()
        .find_map(|(prefix, change)| line.strip_prefix(prefix).map(|path| (path, change)));
        if let Some((path, change)) = envelope {
            files.push(FileTouch {
                path: path.trim().to_string(),
                change,
                lines_added: Some(0),
                lines_removed: Some(0),
            });
            continue;
        }
        if let Some(rest) = line.strip_prefix("--- ") {
            pending_old = header_path(rest);
            continue;
        }
        if let Some(rest) = line.strip_prefix("+++ ") {
            let new_path = header_path(rest);
            let (path, change) = match (new_path, pending_old.take()) {
                (Some(path), Some(_)) => (path, FileChangeKind::Edited),
                (Some(path), None) => (path, FileChangeKind::Created),
                (None, Some(old)) => (old, FileChangeKind::Deleted),
                (None, None) => continue,
            };
            files.push(FileTouch {
                path,
                change,
                lines_added: Some(0),
                lines_removed: Some(0),
            });
            continue;
        }
        if line.starts_with("***") || line.starts_with("@@") {
            continue;
        }
        if files.is_empty()
            && let Some(path) = fallback_path
            && (line.starts_with('+') || line.starts_with('-'))
        {
            files.push(FileTouch {
                path: path.to_string(),
                change: FileChangeKind::Edited,
                lines_added: Some(0),
                lines_removed: Some(0),
            });
        }
        let Some(file) = files.last_mut() else {
            continue;
        };
        if line.starts_with('+') {
            *file.lines_added.get_or_insert(0) += 1;
        } else if line.starts_with('-') {
            *file.lines_removed.get_or_insert(0) += 1;
        }
    }
    files.truncate(MAX_FILES_PER_ACTION);
    files
}

fn command_text(tool: &str, semantic: &str, input: &Value) -> String {
    let raw = match input.get("command").or_else(|| input.get("cmd")) {
        Some(Value::String(command)) => command.clone(),
        Some(Value::Array(parts)) => parts
            .iter()
            .map(|part| {
                part.as_str()
                    .map_or_else(|| part.to_string(), str::to_string)
            })
            .collect::<Vec<_>>()
            .join(" "),
        _ => match semantic {
            "run_tests" | "run_verifiers" => {
                let what = if semantic == "run_tests" {
                    "tests"
                } else {
                    "verifiers"
                };
                let names = input
                    .get("commands")
                    .and_then(Value::as_array)
                    .map(|commands| {
                        commands
                            .iter()
                            .filter_map(|command| command.get("name").and_then(Value::as_str))
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .filter(|names| !names.is_empty());
                match names {
                    Some(names) => format!("{tool} {what}: {names}"),
                    None => format!("{tool} {what}"),
                }
            }
            _ => tool.to_string(),
        },
    };
    let flat = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    bounded(&redact(&flat), MAX_COMMAND_CHARS)
}

/// The engine's own closing status line for a failed shell call, e.g.
/// `Command exited with code 2`. This is a fixed format the shell tool writes,
/// not model prose; anything else yields `None`.
fn closing_exit_code(output: Option<&str>) -> Option<i64> {
    let last = output?.trim_end().lines().last()?.trim();
    last.strip_prefix("Command exited with code ")?.parse().ok()
}

fn nested_calls(facts: Option<&Value>) -> Vec<NestedCall> {
    let Some(calls) = facts
        .and_then(|facts| facts.get("calls"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    calls
        .iter()
        .take(MAX_NESTED_CALLS)
        .filter_map(|call| {
            Some(NestedCall {
                tool: call.get("tool")?.as_str()?.to_string(),
                ok: call.get("ok").and_then(Value::as_bool).unwrap_or(false),
                elapsed_ms: call.get("elapsed_ms").and_then(Value::as_u64),
            })
        })
        .collect()
}

fn string_field(value: Option<&Value>, keys: &[&str]) -> Option<String> {
    let value = value?;
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_string)
    })
}

fn number(value: Option<&Value>, keys: &[&str]) -> Option<i64> {
    let value = value?;
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_i64))
}

fn redact(text: &str) -> String {
    codewhale_secrets::redact::redact_secrets(text)
}

fn bounded(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn first_error_line(output: Option<&str>) -> Option<String> {
    let line = output?
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with("[approval]"))?;
    Some(bounded(&redact(line), MAX_ERROR_CHARS))
}

// ---------------------------------------------------------------------------
// Assembly
// ---------------------------------------------------------------------------

/// What [`assemble`] needs to know about the record besides its steps.
struct Assembly<'a> {
    turn: Option<&'a str>,
    kind: SourceKind,
    turn_postures: Vec<TurnPosture>,
    /// False when the record predates its approval log, so "no approval on
    /// record" does not mean "did not ask".
    approvals_recorded: bool,
}

fn assemble(
    source: ReceiptSource,
    steps: Vec<ToolStep>,
    approvals: Vec<ApprovalStep>,
    turn_failures: Vec<(String, Option<DateTime<Utc>>, Option<String>)>,
    scope: Assembly<'_>,
    mut notes: BTreeSet<String>,
) -> Receipt {
    let Assembly {
        turn,
        kind,
        turn_postures,
        approvals_recorded,
    } = scope;
    let mut approvals_by_call: HashMap<String, usize> = HashMap::new();
    for (index, approval) in approvals.iter().enumerate() {
        if let Some(call_id) = &approval.call_id {
            approvals_by_call.insert(call_id.clone(), index);
        }
    }
    let mut approval_used = vec![false; approvals.len()];
    let mut totals = ReceiptTotals {
        line_counts_complete: true,
        ..ReceiptTotals::default()
    };
    let mut actions: Vec<ReceiptAction> = Vec::new();
    let mut agents: HashMap<String, usize> = HashMap::new();
    let in_scope = |step_turn: Option<&str>| turn.is_none_or(|turn| step_turn == Some(turn));

    for step in &steps {
        let approval_index = step
            .call_id
            .as_ref()
            .and_then(|id| approvals_by_call.get(id).copied());
        if let Some(index) = approval_index {
            approval_used[index] = true;
        }
        let approval = approval_index.map(|index| approvals[index].fact);
        if !in_scope(step.turn.as_deref()) {
            continue;
        }
        let classified = classify(step, &mut notes);
        let status = match (approval, step.outcome) {
            (Some(fact), _)
                if !fact.decision.ran() && fact.decision != ApprovalDecisionLabel::Pending =>
            {
                ActionStatus::NotRun
            }
            (_, StepOutcome::Ok) => ActionStatus::Ok,
            (_, StepOutcome::Failed) => ActionStatus::Failed,
            (_, StepOutcome::Interrupted) => ActionStatus::Interrupted,
            (_, StepOutcome::Running) => ActionStatus::Running,
            (_, StepOutcome::Unknown) => ActionStatus::Unknown,
        };
        let what = match classified {
            Classified::Listed(what) => what,
            Classified::AgentFollowUp { agent_id, status } => {
                if let (Some(&index), Some(status)) = (agents.get(&agent_id), status)
                    && let ActionKind::Subagent { outcome, .. } = &mut actions[index].what
                {
                    *outcome = Some(status);
                }
                totals.other_tool_calls += 1;
                continue;
            }
            Classified::Other => {
                totals.other_tool_calls += 1;
                if status != ActionStatus::Failed && status != ActionStatus::NotRun {
                    continue;
                }
                ActionKind::Tool
            }
        };
        if let ActionKind::Subagent {
            agent_id: Some(agent_id),
            ..
        } = &what
        {
            agents.insert(agent_id.clone(), actions.len());
        }
        let duration_ms = number(step.metadata.as_ref(), &["duration_ms"])
            .and_then(|ms| u64::try_from(ms).ok())
            .or_else(|| match (step.started_at, step.ended_at) {
                (Some(start), Some(end)) if end >= start => {
                    u64::try_from((end - start).num_milliseconds()).ok()
                }
                _ => None,
            });
        actions.push(ReceiptAction {
            seq: 0,
            turn: step.turn.clone(),
            at: step.started_at,
            call_id: step.call_id.clone(),
            tool: step.name.clone(),
            what,
            status,
            duration_ms,
            approval,
            error: (status == ActionStatus::Failed)
                .then(|| first_error_line(step.output.as_deref()))
                .flatten(),
        });
    }

    for (index, approval) in approvals.iter().enumerate() {
        if approval_used[index] {
            continue;
        }
        // Session approvals carry no turn; they are in scope only for a
        // whole-session receipt.
        if turn.is_some() && approval.turn.as_deref() != turn {
            continue;
        }
        actions.push(ReceiptAction {
            seq: 0,
            turn: approval.turn.clone(),
            at: approval.fact.at,
            call_id: approval.call_id.clone(),
            tool: approval.tool.clone(),
            what: ActionKind::Approval,
            status: if approval.fact.decision.ran() {
                ActionStatus::Ok
            } else if approval.fact.decision == ApprovalDecisionLabel::Pending {
                ActionStatus::Running
            } else {
                ActionStatus::NotRun
            },
            duration_ms: None,
            approval: Some(approval.fact),
            error: None,
        });
    }

    for (turn_id, ended_at, error) in turn_failures {
        if !in_scope(Some(&turn_id)) {
            continue;
        }
        actions.push(ReceiptAction {
            seq: 0,
            turn: Some(turn_id),
            at: ended_at,
            call_id: None,
            tool: "turn".to_string(),
            what: ActionKind::TurnFailed,
            status: ActionStatus::Failed,
            duration_ms: None,
            approval: None,
            error: error.map(|error| bounded(&redact(&error), MAX_ERROR_CHARS)),
        });
    }

    if kind == SourceKind::Thread {
        // Runtime actions carry timestamps; keep each turn's order and slot
        // standalone approvals and turn failures where they happened.
        actions.sort_by_key(|action| action.at);
    }
    for (index, action) in actions.iter_mut().enumerate() {
        action.seq = index + 1;
    }

    tally(&actions, &mut totals);
    if approvals_recorded {
        totals.ran_without_asking = actions
            .iter()
            .filter(|action| action.ran_without_asking())
            .count();
    }
    let mut postures: Vec<&'static str> = Vec::new();
    for (turn_id, posture) in &turn_postures {
        if let Some(posture) = posture
            && in_scope(Some(turn_id.as_str()))
            && !postures.contains(posture)
        {
            postures.push(posture);
        }
    }
    if totals.approvals.decider_not_recorded > 0 {
        notes.insert(format!(
            "Who approved: {} approval(s) predate Codewhale recording the decider, or came from a sub-agent, so they show the decision without who made it.",
            totals.approvals.decider_not_recorded
        ));
    }
    notes.insert(
        "Shell file changes: files a command changes (for example `rm` or a build) are not itemized; only file tools are.".to_string(),
    );

    let omitted_actions = actions.len().saturating_sub(MAX_RECEIPT_ACTIONS);
    actions.truncate(MAX_RECEIPT_ACTIONS);
    Receipt {
        schema_id: RECEIPT_SCHEMA_ID,
        source,
        turn: turn.map(str::to_string),
        postures,
        totals,
        actions,
        omitted_actions,
        not_recorded: notes.into_iter().collect(),
        claim_ceiling: CLAIM_CEILING,
    }
}

impl ReceiptAction {
    /// A change, command, code run, web or MCP call, or agent that ran with
    /// no approval on record.
    fn ran_without_asking(&self) -> bool {
        self.approval.is_none()
            && matches!(
                self.status,
                ActionStatus::Ok
                    | ActionStatus::Failed
                    | ActionStatus::Interrupted
                    | ActionStatus::Running
            )
            && matches!(
                self.what,
                ActionKind::FileChange { .. }
                    | ActionKind::Command { .. }
                    | ActionKind::Code { .. }
                    | ActionKind::Network { .. }
                    | ActionKind::Mcp { .. }
                    | ActionKind::Subagent { .. }
            )
    }
}

fn tally(actions: &[ReceiptAction], totals: &mut ReceiptTotals) {
    let mut changed: BTreeSet<&str> = BTreeSet::new();
    let mut created: BTreeSet<&str> = BTreeSet::new();
    let mut deleted: BTreeSet<&str> = BTreeSet::new();
    for action in actions {
        let ran = action.status != ActionStatus::NotRun;
        if action.status == ActionStatus::Failed {
            totals.failures += 1;
        }
        match &action.what {
            ActionKind::FileChange { files } if action.status == ActionStatus::Ok => {
                for file in files {
                    changed.insert(&file.path);
                    match file.change {
                        FileChangeKind::Created => {
                            created.insert(&file.path);
                        }
                        FileChangeKind::Deleted => {
                            deleted.insert(&file.path);
                        }
                        FileChangeKind::Edited | FileChangeKind::Written => {}
                    }
                    match (file.lines_added, file.lines_removed) {
                        (Some(added), Some(removed)) => {
                            totals.lines_added += added;
                            totals.lines_removed += removed;
                        }
                        _ => totals.line_counts_complete = false,
                    }
                }
            }
            ActionKind::Command { .. } if ran => {
                totals.commands += 1;
                if action.status == ActionStatus::Failed {
                    totals.commands_failed += 1;
                }
            }
            ActionKind::Code { .. } if ran => totals.code_runs += 1,
            ActionKind::Network { .. } if ran => totals.network += 1,
            ActionKind::Mcp { plugin, .. } if ran => {
                totals.mcp_calls += 1;
                if *plugin {
                    totals.plugin_calls += 1;
                }
            }
            ActionKind::Subagent { .. } if ran => totals.subagents += 1,
            _ => {}
        }
        if let Some(fact) = action.approval {
            let approvals = &mut totals.approvals;
            approvals.total += 1;
            match fact.decision {
                ApprovalDecisionLabel::Approved | ApprovalDecisionLabel::ApprovedWithPolicy => {
                    approvals.approved += 1
                }
                ApprovalDecisionLabel::Denied => approvals.denied += 1,
                ApprovalDecisionLabel::TimedOut => approvals.timed_out += 1,
                ApprovalDecisionLabel::Cancelled | ApprovalDecisionLabel::Unavailable => {
                    approvals.not_answered += 1
                }
                ApprovalDecisionLabel::Pending => approvals.pending += 1,
            }
            let decided = matches!(
                fact.decision,
                ApprovalDecisionLabel::Approved
                    | ApprovalDecisionLabel::ApprovedWithPolicy
                    | ApprovalDecisionLabel::Denied
            );
            match fact.decided_by {
                Some(ApprovalDecider::User) => approvals.by_you += 1,
                Some(ApprovalDecider::SessionRule) => approvals.by_session_rule += 1,
                Some(ApprovalDecider::Posture) => approvals.by_posture += 1,
                Some(ApprovalDecider::Host) => {}
                None if decided => approvals.decider_not_recorded += 1,
                None => {}
            }
        }
    }
    totals.files_changed = changed.len();
    totals.files_created = created.len();
    totals.files_deleted = deleted.len();
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// One line of totals, verbs first: `Changed 4 files · ran 7 commands · 2
/// approvals by you · 9 ran without asking under Full Access`.
#[must_use]
pub fn totals_line(receipt: &Receipt) -> String {
    let totals = &receipt.totals;
    let mut parts: Vec<String> = Vec::new();
    if totals.files_changed > 0 {
        let mut part = format!("changed {}", plural(totals.files_changed, "file", "files"));
        if totals.line_counts_complete {
            part.push_str(&format!(
                " (+{} −{})",
                totals.lines_added, totals.lines_removed
            ));
        }
        parts.push(part);
    }
    if totals.commands > 0 {
        let mut part = format!("ran {}", plural(totals.commands, "command", "commands"));
        if totals.commands_failed > 0 {
            part.push_str(&format!(" ({} failed)", totals.commands_failed));
        }
        parts.push(part);
    }
    if totals.code_runs > 0 {
        parts.push(format!(
            "ran code {}",
            plural(totals.code_runs, "time", "times")
        ));
    }
    if totals.network > 0 {
        parts.push(format!(
            "made {}",
            plural(totals.network, "web request", "web requests")
        ));
    }
    if totals.mcp_calls > 0 {
        parts.push(format!(
            "made {}",
            plural(totals.mcp_calls, "MCP call", "MCP calls")
        ));
    }
    if totals.subagents > 0 {
        parts.push(format!(
            "started {}",
            plural(totals.subagents, "agent", "agents")
        ));
    }
    let approvals = &totals.approvals;
    for (count, label) in [
        (approvals.by_you, "by you"),
        (approvals.by_session_rule, "by session rule"),
        (approvals.by_posture, "by posture"),
        (approvals.decider_not_recorded, "decider not recorded"),
    ] {
        if count > 0 {
            parts.push(format!(
                "{} {label}",
                plural(count, "approval", "approvals")
            ));
        }
    }
    if totals.ran_without_asking > 0 {
        let mut part = format!("{} ran without asking", totals.ran_without_asking);
        if !receipt.postures.is_empty() {
            part.push_str(&format!(" under {}", receipt.postures.join(" and ")));
        }
        parts.push(part);
    }
    if approvals.denied > 0 {
        parts.push(format!("{} denied", approvals.denied));
    }
    if approvals.timed_out > 0 {
        parts.push(format!("{} timed out", approvals.timed_out));
    }
    if approvals.pending > 0 {
        parts.push(format!("{} waiting", approvals.pending));
    }
    let failures_beyond_commands = totals.failures.saturating_sub(totals.commands_failed);
    if failures_beyond_commands > 0 {
        parts.push(plural(
            failures_beyond_commands,
            "other failure",
            "other failures",
        ));
    }
    if parts.is_empty() {
        return if totals.other_tool_calls > 0 {
            format!(
                "Only read or looked things up ({}).",
                plural(totals.other_tool_calls, "call", "calls")
            )
        } else {
            "No actions recorded.".to_string()
        };
    }
    let mut line = parts.join(" · ");
    if let Some(first) = line.get(0..1) {
        line = first.to_uppercase() + &line[1..];
    }
    line
}

fn plural(count: usize, one: &str, many: &str) -> String {
    if count == 1 {
        format!("1 {one}")
    } else {
        format!("{count} {many}")
    }
}

fn decider_label(decider: ApprovalDecider) -> &'static str {
    match decider {
        ApprovalDecider::User => "you",
        ApprovalDecider::SessionRule => "session rule",
        ApprovalDecider::Posture => "posture",
        ApprovalDecider::Host => "Codewhale",
    }
}

fn approval_phrase(fact: &ApprovalFact) -> String {
    let by = fact
        .decided_by
        .map(|by| format!(" by {}", decider_label(by)))
        .unwrap_or_default();
    match fact.decision {
        ApprovalDecisionLabel::Approved => format!("approved{by}"),
        ApprovalDecisionLabel::ApprovedWithPolicy => format!("approved{by} with a wider sandbox"),
        ApprovalDecisionLabel::Denied => format!("denied{by}"),
        ApprovalDecisionLabel::TimedOut => "approval timed out".to_string(),
        ApprovalDecisionLabel::Cancelled => "turn stopped while waiting".to_string(),
        ApprovalDecisionLabel::Unavailable => "nobody could be asked".to_string(),
        ApprovalDecisionLabel::Pending => "waiting for approval".to_string(),
    }
}

fn counts(added: Option<u64>, removed: Option<u64>) -> String {
    match (added, removed) {
        (Some(added), Some(removed)) => format!(" (+{added} −{removed})"),
        _ => String::new(),
    }
}

fn file_phrase(file: &FileTouch) -> String {
    let verb = match file.change {
        FileChangeKind::Edited => "edited",
        FileChangeKind::Created => "created",
        FileChangeKind::Deleted => "deleted",
        FileChangeKind::Written => "wrote",
    };
    let counts = if file.change == FileChangeKind::Deleted {
        String::new()
    } else {
        counts(file.lines_added, file.lines_removed)
    };
    format!("{verb} {}{counts}", file.path)
}

fn action_phrase(action: &ReceiptAction) -> String {
    match &action.what {
        ActionKind::FileChange { files } => match files.as_slice() {
            [file] => file_phrase(file),
            files => format!(
                "changed {} files: {}",
                files.len(),
                files.iter().map(file_phrase).collect::<Vec<_>>().join(", ")
            ),
        },
        ActionKind::Command {
            command,
            cwd,
            exit_code,
        } => {
            let mut text = format!("ran `{command}`");
            if let Some(cwd) = cwd {
                text.push_str(&format!(" in {cwd}"));
            }
            if let Some(code) = exit_code {
                text.push_str(&format!(" — exit {code}"));
            }
            text
        }
        ActionKind::Code { exit_code, nested } => {
            let mut text = format!("ran code ({})", action.tool);
            if let Some(code) = exit_code {
                text.push_str(&format!(" — exit {code}"));
            }
            if !nested.is_empty() {
                let calls = nested
                    .iter()
                    .map(|call| format!("{}{}", call.tool, if call.ok { "" } else { " ✗" }))
                    .collect::<Vec<_>>()
                    .join(", ");
                text.push_str(&format!(" — called {calls}"));
            }
            text
        }
        ActionKind::Network {
            action: kind,
            host,
            query,
        } => match (kind.as_str(), host, query) {
            ("search", _, Some(query)) => format!("searched the web for “{query}”"),
            ("search", _, None) => "searched the web".to_string(),
            ("fetch", Some(host), _) => format!("fetched {host}"),
            ("git_fetch", Some(remote), _) => format!("fetched git remote {remote}"),
            (other, Some(host), _) => format!("{} on {host}", other.replace('_', " ")),
            (other, None, _) => format!("made a web request ({other})"),
        },
        ActionKind::Mcp { server, .. } => {
            let tool = server
                .as_deref()
                .and_then(|server| {
                    action
                        .tool
                        .strip_prefix("mcp_")
                        .and_then(|rest| rest.strip_prefix(server))
                        .map(|rest| rest.trim_start_matches('_'))
                })
                .filter(|tool| !tool.is_empty());
            match (server, tool) {
                (Some(server), Some(tool)) => format!("called {server} · {tool}"),
                _ => format!("called {}", action.tool),
            }
        }
        ActionKind::Subagent {
            name,
            agent_id,
            outcome,
        } => {
            let who = name
                .as_deref()
                .or(agent_id.as_deref())
                .unwrap_or("an agent");
            match outcome {
                Some(outcome) => format!("started agent {who} — {outcome}"),
                None => format!("started agent {who}"),
            }
        }
        ActionKind::Approval => format!("asked to use {}", action.tool),
        ActionKind::Tool => action.tool.clone(),
        ActionKind::TurnFailed => "turn failed".to_string(),
    }
}

fn duration_label(ms: u64) -> String {
    if ms < 1_000 {
        format!("{ms}ms")
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1_000.0)
    } else {
        format!("{}m{:02}s", ms / 60_000, (ms % 60_000) / 1_000)
    }
}

/// One line per action. Plain text that also reads as Markdown.
#[must_use]
pub fn action_line(action: &ReceiptAction) -> String {
    let mut line = action_phrase(action);
    match action.status {
        ActionStatus::NotRun => {
            line = format!("did not run: {line}");
        }
        ActionStatus::Failed => {
            line.push_str(" — failed");
            if let Some(error) = &action.error {
                line.push_str(&format!(": {error}"));
            }
        }
        ActionStatus::Interrupted => line.push_str(" — interrupted"),
        ActionStatus::Running if action.what != ActionKind::Approval => {
            line.push_str(" — still running")
        }
        ActionStatus::Unknown => line.push_str(" — no result recorded"),
        _ => {}
    }
    if let Some(ms) = action.duration_ms
        && action.status != ActionStatus::NotRun
    {
        line.push_str(&format!(" · {}", duration_label(ms)));
    }
    if let Some(fact) = &action.approval {
        line.push_str(&format!(" · {}", approval_phrase(fact)));
    }
    line
}

/// The readable receipt: header, totals, one line per action, then what the
/// record does not hold. Valid Markdown and plain enough for a terminal.
#[must_use]
pub fn render_markdown(receipt: &Receipt) -> String {
    let source = &receipt.source;
    let noun = match source.kind {
        SourceKind::Session => "session",
        SourceKind::Thread => "thread",
    };
    let mut out = String::new();
    let title = source
        .title
        .as_deref()
        .map(|title| bounded(title.trim(), 80))
        .filter(|title| !title.is_empty());
    match title {
        Some(title) => out.push_str(&format!("# Receipt: {title}\n\n")),
        None => out.push_str(&format!("# Receipt: {noun} {}\n\n", source.id)),
    }
    let mut facts = vec![format!("{noun} {}", source.id)];
    if let Some(turn) = &receipt.turn {
        facts.push(format!("turn {turn} only"));
    }
    if let Some(workspace) = &source.workspace {
        facts.push(workspace.clone());
    }
    if let Some(model) = &source.model {
        facts.push(model.clone());
    }
    if !receipt.postures.is_empty() {
        facts.push(receipt.postures.join(", "));
    }
    if let (Some(start), Some(end)) = (source.started_at, source.updated_at) {
        facts.push(format!(
            "{} → {}",
            start.format("%Y-%m-%d %H:%M UTC"),
            end.format("%Y-%m-%d %H:%M UTC")
        ));
    }
    out.push_str(&facts.join(" · "));
    out.push_str("\n\n");
    out.push_str(&totals_line(receipt));
    out.push_str("\n\n");
    let width = receipt.actions.len().to_string().len();
    for action in &receipt.actions {
        out.push_str(&format!(
            "{:>width$}. {}\n",
            action.seq,
            action_line(action),
            width = width
        ));
    }
    if receipt.omitted_actions > 0 {
        out.push_str(&format!(
            "\n{} more actions are counted above but not listed.\n",
            receipt.omitted_actions
        ));
    }
    if !receipt.not_recorded.is_empty() {
        out.push_str("\nNot recorded:\n");
        for note in &receipt.not_recorded {
            out.push_str(&format!("- {note}\n"));
        }
    }
    out
}

#[must_use]
pub fn render_json(receipt: &Receipt) -> String {
    serde_json::to_string_pretty(receipt).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
#[path = "receipts/tests.rs"]
mod tests;
