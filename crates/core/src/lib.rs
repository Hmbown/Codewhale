pub mod context_reference;
pub mod fragments;
pub mod ids;
pub mod journal;
pub mod prefix_cache;
pub mod request;
pub mod role;
pub mod secret_eq;
pub mod session;
pub mod tool_parser;

pub use context_reference::{
    ContextReference, ContextReferenceKind, ContextReferenceSource, MediaAttachmentReference,
    media_attachment_references,
};

use std::collections::HashMap;

use anyhow::Result;
use codewhale_config::{ConfigToml, ProviderKind};
use codewhale_hooks::HookDispatcher;
use codewhale_protocol::{AppResponse, EventFrame, ResponseChannel, Status};
use codewhale_state::{JobStateRecord, JobStateStatus, StateStore};
use serde_json::{Value, json};
use uuid::Uuid;

/// Status of a background job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStatus {
    /// Waiting to be picked up.
    Queued,
    /// Currently executing.
    Running,
    /// Temporarily paused.
    Paused,
    /// Finished successfully.
    Completed,
    /// Finished with an error.
    Failed,
    /// Cancelled by the user.
    Cancelled,
}

impl Status for JobStatus {
    fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
    fn is_active(&self) -> bool {
        matches!(self, Self::Queued | Self::Running)
    }
    fn is_paused(&self) -> bool {
        matches!(self, Self::Paused)
    }
}

const JOB_DETAIL_SCHEMA_VERSION: u8 = 1;
const DEFAULT_JOB_MAX_ATTEMPTS: u32 = 3;
const DEFAULT_JOB_BACKOFF_BASE_MS: u64 = 500;
const MAX_JOB_HISTORY_ENTRIES: usize = 64;

/// Retry state for a job that failed and may be retried.
#[derive(Debug, Clone)]
pub struct JobRetryMetadata {
    /// Current attempt number (0 = not yet retried).
    pub attempt: u32,
    /// Maximum number of retry attempts before giving up.
    pub max_attempts: u32,
    /// Base delay in milliseconds for exponential backoff.
    pub backoff_base_ms: u64,
    /// Computed delay in milliseconds until the next retry.
    pub next_backoff_ms: u64,
    /// Timestamp when the next retry should be attempted.
    pub next_retry_at: Option<i64>,
}

impl Default for JobRetryMetadata {
    fn default() -> Self {
        Self {
            attempt: 0,
            max_attempts: DEFAULT_JOB_MAX_ATTEMPTS,
            backoff_base_ms: DEFAULT_JOB_BACKOFF_BASE_MS,
            next_backoff_ms: 0,
            next_retry_at: None,
        }
    }
}

/// A single entry in a job's history log.
#[derive(Debug, Clone)]
pub struct JobHistoryEntry {
    /// Timestamp when this entry was recorded.
    pub at: i64,
    /// Phase name (e.g., "created", "running", "failed").
    pub phase: String,
    /// Job status at this point in time.
    pub status: JobStatus,
    /// Progress percentage at this point, if available.
    pub progress: Option<u8>,
    /// Human-readable detail message.
    pub detail: Option<String>,
    /// Retry state snapshot at this point.
    pub retry: JobRetryMetadata,
}

#[derive(Debug, Clone)]
struct PersistedJobDetail {
    pub status: JobStatus,
    pub detail: Option<String>,
    pub retry: JobRetryMetadata,
    pub history: Vec<JobHistoryEntry>,
}

/// A complete job record with all metadata and history.
#[derive(Debug, Clone)]
pub struct JobRecord {
    /// Unique job identifier.
    pub id: String,
    /// Human-readable job name.
    pub name: String,
    /// Current job status.
    pub status: JobStatus,
    /// Current progress percentage (0-100).
    pub progress: Option<u8>,
    /// Human-readable detail about the current state.
    pub detail: Option<String>,
    /// Retry state for failed jobs.
    pub retry: JobRetryMetadata,
    /// Chronological history of state transitions.
    pub history: Vec<JobHistoryEntry>,
    /// Timestamp when the job was created.
    pub created_at: i64,
    /// Timestamp of the last state change.
    pub updated_at: i64,
}

/// Map a durable [`JobRecord`] to the dependency-neutral run read model.
///
/// Pure projection of the record as persisted: unknown budgets stay unset and
/// nothing is fabricated. `updated_at` (epoch seconds) provides the terminal
/// timestamp because the job manager records no separate end time. The
/// free-form job detail is intentionally omitted because this owner does not
/// classify it as safe for a cross-surface read model.
#[must_use]
pub fn job_record_to_agent_run(
    record: &JobRecord,
) -> codewhale_protocol::agent_run::AgentRunSnapshot {
    use codewhale_protocol::agent_run::{
        AgentRunSnapshot, BudgetSummary, RunSource, RunState, TerminalOutcome, TerminalSummary,
    };

    let (state, terminal) = match record.status {
        JobStatus::Queued => (RunState::Queued, None),
        JobStatus::Running => (RunState::Running, None),
        JobStatus::Paused => (RunState::Paused, None),
        JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled => {
            let outcome = match record.status {
                JobStatus::Completed => TerminalOutcome::Completed,
                JobStatus::Failed => TerminalOutcome::Failed,
                _ => TerminalOutcome::Cancelled,
            };
            (
                RunState::Terminal,
                Some(TerminalSummary {
                    outcome,
                    ended_at_ms: record.updated_at.checked_mul(1000),
                    detail: None,
                }),
            )
        }
    };

    AgentRunSnapshot {
        run_id: record.id.clone(),
        parent: None,
        source: RunSource::CoreJob,
        state,
        budget: BudgetSummary::default(),
        terminal,
        refs: Vec::new(),
    }
}

/// Manages background jobs with retry logic and persistence.
#[derive(Debug, Default)]
pub struct JobManager {
    jobs: HashMap<String, JobRecord>,
}

impl JobManager {
    fn now_ts() -> i64 {
        chrono::Utc::now().timestamp()
    }

    fn deterministic_backoff_ms(retry: &JobRetryMetadata) -> u64 {
        if retry.attempt == 0 {
            return 0;
        }
        let exponent = retry.attempt.saturating_sub(1).min(20);
        let multiplier = 1u64.checked_shl(exponent).unwrap_or(u64::MAX);
        retry.backoff_base_ms.saturating_mul(multiplier)
    }

    fn clear_retry_schedule(retry: &mut JobRetryMetadata) {
        retry.next_backoff_ms = 0;
        retry.next_retry_at = None;
    }

    fn push_history(job: &mut JobRecord, phase: &str) {
        job.history.push(JobHistoryEntry {
            at: job.updated_at,
            phase: phase.to_string(),
            status: job.status,
            progress: job.progress,
            detail: job.detail.clone(),
            retry: job.retry.clone(),
        });
        if job.history.len() > MAX_JOB_HISTORY_ENTRIES {
            let to_drain = job.history.len() - MAX_JOB_HISTORY_ENTRIES;
            job.history.drain(0..to_drain);
        }
    }

    fn parse_persisted_detail(raw: Option<&str>) -> Option<PersistedJobDetail> {
        let raw = raw?;
        let parsed: Value = serde_json::from_str(raw).ok()?;
        let status = parsed
            .get("status")
            .and_then(Value::as_str)
            .and_then(job_status_from_str)?;
        let detail = parsed.get("detail").and_then(json_optional_string);
        let retry = parse_retry_metadata(parsed.get("retry"));
        let history = parsed
            .get("history")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(parse_history_entry)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        Some(PersistedJobDetail {
            status,
            detail,
            retry,
            history,
        })
    }

    fn encode_persisted_detail(job: &JobRecord) -> Result<Option<String>> {
        let encoded = json!({
            "schema_version": JOB_DETAIL_SCHEMA_VERSION,
            "status": job_status_to_str(job.status),
            "detail": job.detail.clone(),
            "retry": job_retry_to_value(&job.retry),
            "history": job.history.iter().map(job_history_to_value).collect::<Vec<_>>()
        })
        .to_string();
        Ok(Some(encoded))
    }

    /// Enqueues a new job and returns its record.
    pub fn enqueue(&mut self, name: impl Into<String>) -> JobRecord {
        let now = Self::now_ts();
        let id = format!("job-{}", Uuid::new_v4());
        let mut job = JobRecord {
            id: id.clone(),
            name: name.into(),
            status: JobStatus::Queued,
            progress: Some(0),
            detail: None,
            retry: JobRetryMetadata::default(),
            history: Vec::new(),
            created_at: now,
            updated_at: now,
        };
        Self::push_history(&mut job, "created");
        self.jobs.insert(id, job.clone());
        job
    }

    /// Transitions a job to running and clears its retry schedule.
    pub fn set_running(&mut self, id: &str) {
        if let Some(job) = self.jobs.get_mut(id) {
            job.status = JobStatus::Running;
            Self::clear_retry_schedule(&mut job.retry);
            job.updated_at = Self::now_ts();
            Self::push_history(job, "running");
        }
    }

    /// Updates a job's progress (clamped to 100) and optional detail message.
    pub fn update_progress(&mut self, id: &str, progress: u8, detail: Option<String>) {
        if let Some(job) = self.jobs.get_mut(id) {
            job.progress = Some(progress.min(100));
            job.detail = detail;
            job.updated_at = Self::now_ts();
            Self::push_history(job, "progress_updated");
        }
    }

    /// Marks a job as completed with 100% progress and clears its retry schedule.
    pub fn complete(&mut self, id: &str) {
        if let Some(job) = self.jobs.get_mut(id) {
            job.status = JobStatus::Completed;
            job.progress = Some(100);
            Self::clear_retry_schedule(&mut job.retry);
            job.updated_at = Self::now_ts();
            Self::push_history(job, "completed");
        }
    }

    /// Marks a job as failed and schedules a retry if attempts remain.
    pub fn fail(&mut self, id: &str, detail: impl Into<String>) {
        if let Some(job) = self.jobs.get_mut(id) {
            let now = Self::now_ts();
            job.status = JobStatus::Failed;
            job.detail = Some(detail.into());
            if job.retry.attempt < job.retry.max_attempts {
                job.retry.attempt += 1;
                job.retry.next_backoff_ms = Self::deterministic_backoff_ms(&job.retry);
                let delay_secs = ((job.retry.next_backoff_ms.saturating_add(999)) / 1000)
                    .min(i64::MAX as u64) as i64;
                job.retry.next_retry_at = Some(now.saturating_add(delay_secs));
            } else {
                Self::clear_retry_schedule(&mut job.retry);
            }
            job.updated_at = now;
            Self::push_history(job, "failed");
        }
    }

    /// Cancels a job and clears any pending retry schedule.
    pub fn cancel(&mut self, id: &str) {
        if let Some(job) = self.jobs.get_mut(id) {
            job.status = JobStatus::Cancelled;
            Self::clear_retry_schedule(&mut job.retry);
            job.updated_at = Self::now_ts();
            Self::push_history(job, "cancelled");
        }
    }

    /// Pauses a job, optionally updating its detail message.
    pub fn pause(&mut self, id: &str, detail: Option<String>) {
        if let Some(job) = self.jobs.get_mut(id) {
            job.status = JobStatus::Paused;
            if detail.is_some() {
                job.detail = detail;
            }
            job.updated_at = Self::now_ts();
            Self::push_history(job, "paused");
        }
    }

    /// Resumes a paused or failed job back to running status.
    pub fn resume(&mut self, id: &str, detail: Option<String>) {
        if let Some(job) = self.jobs.get_mut(id) {
            job.status = JobStatus::Running;
            if detail.is_some() {
                job.detail = detail;
            }
            Self::clear_retry_schedule(&mut job.retry);
            job.updated_at = Self::now_ts();
            Self::push_history(job, "resumed");
        }
    }

    /// Returns all jobs sorted by most recently updated first.
    pub fn list(&self) -> Vec<JobRecord> {
        let mut out = self.jobs.values().cloned().collect::<Vec<_>>();
        out.sort_by_key(|job| std::cmp::Reverse(job.updated_at));
        out
    }

    /// Returns the history entries for a job, or an empty vec if not found.
    pub fn history(&self, id: &str) -> Vec<JobHistoryEntry> {
        self.jobs
            .get(id)
            .map(|job| job.history.clone())
            .unwrap_or_default()
    }

    /// Resets queued or running jobs back to queued on application resume.
    pub fn resume_pending(&mut self) -> Vec<JobRecord> {
        let mut resumed = Vec::new();
        for job in self.jobs.values_mut() {
            if matches!(job.status, JobStatus::Queued | JobStatus::Running) {
                job.status = JobStatus::Queued;
                job.updated_at = Self::now_ts();
                Self::push_history(job, "queued_after_resume");
                resumed.push(job.clone());
            }
        }
        resumed
    }

    /// Loads jobs from the state store, deserializing extended detail when available.
    pub fn load_from_store(&mut self, store: &StateStore) -> Result<()> {
        let persisted = store.list_jobs(Some(500))?;
        for job in persisted {
            let fallback_status = job_state_status_to_runtime(job.status);
            let parsed = Self::parse_persisted_detail(job.detail.as_deref());
            let (status, detail, retry, history) = if let Some(detail_state) = parsed {
                (
                    detail_state.status,
                    detail_state.detail,
                    detail_state.retry,
                    detail_state.history,
                )
            } else {
                (
                    fallback_status,
                    job.detail,
                    JobRetryMetadata::default(),
                    Vec::new(),
                )
            };
            self.jobs.insert(
                job.id.clone(),
                JobRecord {
                    id: job.id,
                    name: job.name,
                    status,
                    progress: job.progress,
                    detail,
                    retry,
                    history,
                    created_at: job.created_at,
                    updated_at: job.updated_at,
                },
            );
        }
        Ok(())
    }

    /// Persists a single job's current state to the state store.
    pub fn persist_job(&self, store: &StateStore, id: &str) -> Result<()> {
        let Some(job) = self.jobs.get(id) else {
            return Ok(());
        };
        let encoded_detail = Self::encode_persisted_detail(job)?;
        store.upsert_job(&JobStateRecord {
            id: job.id.clone(),
            name: job.name.clone(),
            status: runtime_status_to_job_state(job.status),
            progress: job.progress,
            detail: encoded_detail,
            created_at: job.created_at,
            updated_at: job.updated_at,
        })
    }

    /// Persists all in-memory jobs to the state store.
    pub fn persist_all(&self, store: &StateStore) -> Result<()> {
        for id in self.jobs.keys() {
            self.persist_job(store, id)?;
        }
        Ok(())
    }
}

/// Compatibility configuration, hooks and job bookkeeping.
///
/// Conversation execution and durable history belong to the held canonical
/// RuntimeThreadManager/Engine. Legacy State data is retained for read-only
/// recovery and bound alias publication, never as a second transcript writer.
pub struct Runtime {
    pub config: ConfigToml,
    state: StateStore,
    pub hooks: HookDispatcher,
    pub jobs: JobManager,
}

impl Runtime {
    pub fn new(config: ConfigToml, state: StateStore, hooks: HookDispatcher) -> Self {
        let mut jobs = JobManager::default();
        if let Err(e) = jobs.load_from_store(&state) {
            tracing::warn!("Failed to load job store, starting with empty job list: {e}");
        }
        Self {
            config,
            state,
            hooks,
            jobs,
        }
    }

    /// Existing compatibility data and receipts; no transcript controller.
    pub fn state_store(&self) -> &StateStore {
        &self.state
    }

    pub fn update_config(&mut self, config: ConfigToml) {
        self.config = config;
    }

    /// Returns the current application status including all jobs and their history.
    pub fn app_status(&self) -> AppResponse {
        let jobs = self.jobs.list();
        let events = jobs
            .iter()
            .flat_map(|job| {
                job.history.iter().map(|entry| EventFrame::ResponseDelta {
                    response_id: job.id.clone(),
                    delta: json!({
                        "kind": "job_transition",
                        "job_id": job.id.clone(),
                        "phase": entry.phase.clone(),
                        "status": job_status_to_str(entry.status),
                        "progress": entry.progress,
                        "detail": entry.detail.clone(),
                        "retry": job_retry_to_value(&entry.retry),
                        "at": entry.at
                    })
                    .to_string(),
                    channel: ResponseChannel::Text,
                })
            })
            .collect::<Vec<_>>();
        AppResponse {
            ok: true,
            data: json!({
                "jobs": jobs.into_iter().map(|job| {
                    json!({
                        "id": job.id,
                        "name": job.name,
                        "status": job_status_to_str(job.status),
                        "progress": job.progress,
                        "detail": job.detail,
                        "retry": job_retry_to_value(&job.retry),
                        "history": job.history.iter().map(job_history_to_value).collect::<Vec<_>>()
                    })
                }).collect::<Vec<_>>()
            }),
            events,
        }
    }

    /// Returns the default model provider from the resolved configuration.
    pub fn provider_default(&self) -> ProviderKind {
        self.config.provider
    }
}

/// How many leading entries of `offered` repeat the end of `persisted`: the
/// longest suffix of `persisted` that is also a prefix of `offered`.
/// Longest persisted suffix repeated by an offered history prefix. Linear
/// work preserves genuine repeated messages without an unbounded nested scan.
pub fn persisted_overlap<T: PartialEq>(persisted: &[T], offered: &[T]) -> usize {
    if offered.is_empty() {
        return 0;
    }
    let mut failure = vec![0; offered.len()];
    for index in 1..offered.len() {
        let mut matched = failure[index - 1];
        while matched > 0 && offered[index] != offered[matched] {
            matched = failure[matched - 1];
        }
        if offered[index] == offered[matched] {
            matched += 1;
        }
        failure[index] = matched;
    }
    let mut matched = 0;
    for item in persisted {
        if matched == offered.len() {
            matched = failure[matched - 1];
        }
        while matched > 0 && *item != offered[matched] {
            matched = failure[matched - 1];
        }
        if *item == offered[matched] {
            matched += 1;
        }
    }
    matched
}

fn json_optional_string(value: &Value) -> Option<String> {
    if value.is_null() {
        None
    } else {
        value.as_str().map(ToString::to_string)
    }
}

fn parse_retry_metadata(value: Option<&Value>) -> JobRetryMetadata {
    let Some(value) = value else {
        return JobRetryMetadata::default();
    };
    JobRetryMetadata {
        attempt: value
            .get("attempt")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            .min(u32::MAX as u64) as u32,
        max_attempts: value
            .get("max_attempts")
            .and_then(Value::as_u64)
            .unwrap_or(DEFAULT_JOB_MAX_ATTEMPTS as u64)
            .min(u32::MAX as u64) as u32,
        backoff_base_ms: value
            .get("backoff_base_ms")
            .and_then(Value::as_u64)
            .unwrap_or(DEFAULT_JOB_BACKOFF_BASE_MS),
        next_backoff_ms: value
            .get("next_backoff_ms")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        next_retry_at: value.get("next_retry_at").and_then(Value::as_i64),
    }
}

fn parse_history_entry(value: &Value) -> Option<JobHistoryEntry> {
    let status = value
        .get("status")
        .and_then(Value::as_str)
        .and_then(job_status_from_str)?;
    Some(JobHistoryEntry {
        at: value.get("at").and_then(Value::as_i64).unwrap_or(0),
        phase: value
            .get("phase")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        status,
        progress: value
            .get("progress")
            .and_then(Value::as_u64)
            .map(|v| v.min(u8::MAX as u64) as u8),
        detail: value.get("detail").and_then(json_optional_string),
        retry: parse_retry_metadata(value.get("retry")),
    })
}

fn job_status_to_str(status: JobStatus) -> &'static str {
    match status {
        JobStatus::Queued => "queued",
        JobStatus::Running => "running",
        JobStatus::Paused => "paused",
        JobStatus::Completed => "completed",
        JobStatus::Failed => "failed",
        JobStatus::Cancelled => "cancelled",
    }
}

fn job_status_from_str(value: &str) -> Option<JobStatus> {
    match value {
        "queued" => Some(JobStatus::Queued),
        "running" => Some(JobStatus::Running),
        "paused" => Some(JobStatus::Paused),
        "completed" => Some(JobStatus::Completed),
        "failed" => Some(JobStatus::Failed),
        "cancelled" => Some(JobStatus::Cancelled),
        _ => None,
    }
}

fn job_retry_to_value(retry: &JobRetryMetadata) -> Value {
    json!({
        "attempt": retry.attempt,
        "max_attempts": retry.max_attempts,
        "backoff_base_ms": retry.backoff_base_ms,
        "next_backoff_ms": retry.next_backoff_ms,
        "next_retry_at": retry.next_retry_at
    })
}

fn job_history_to_value(entry: &JobHistoryEntry) -> Value {
    json!({
        "at": entry.at,
        "phase": entry.phase.clone(),
        "status": job_status_to_str(entry.status),
        "progress": entry.progress,
        "detail": entry.detail.clone(),
        "retry": job_retry_to_value(&entry.retry)
    })
}

fn runtime_status_to_job_state(status: JobStatus) -> JobStateStatus {
    match status {
        JobStatus::Queued => JobStateStatus::Queued,
        JobStatus::Running => JobStateStatus::Running,
        JobStatus::Paused => JobStateStatus::Paused,
        JobStatus::Completed => JobStateStatus::Completed,
        JobStatus::Failed => JobStateStatus::Failed,
        JobStatus::Cancelled => JobStateStatus::Cancelled,
    }
}

fn job_state_status_to_runtime(status: JobStateStatus) -> JobStatus {
    match status {
        JobStateStatus::Queued => JobStatus::Queued,
        JobStateStatus::Running => JobStatus::Running,
        JobStateStatus::Paused => JobStatus::Paused,
        JobStateStatus::Completed => JobStatus::Completed,
        JobStateStatus::Failed => JobStatus::Failed,
        JobStateStatus::Cancelled => JobStatus::Cancelled,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_core_state(name: &str) -> StateStore {
        let dir =
            std::env::temp_dir().join(format!("codewhale-core-{name}-{}", Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).expect("create temp state dir");
        StateStore::open(Some(dir.join("state.db"))).expect("open state store")
    }

    // ── JobManager: lifecycle ──────────────────────────────────────────

    #[test]
    fn enqueue_creates_queued_job_with_zero_progress() {
        let mut jm = JobManager::default();
        let job = jm.enqueue("build");
        assert_eq!(job.name, "build");
        assert_eq!(job.status, JobStatus::Queued);
        assert_eq!(job.progress, Some(0));
        assert!(job.detail.is_none());
        assert_eq!(job.history.len(), 1);
        assert_eq!(job.history[0].phase, "created");
    }

    #[test]
    fn set_running_transitions_from_queued() {
        let mut jm = JobManager::default();
        let job = jm.enqueue("deploy");
        let id = job.id.clone();
        jm.set_running(&id);
        let jobs = jm.list();
        let updated = jobs.iter().find(|j| j.id == id).unwrap();
        assert_eq!(updated.status, JobStatus::Running);
        assert_eq!(updated.history.last().unwrap().phase, "running");
    }

    #[test]
    fn update_progress_clamps_to_100() {
        let mut jm = JobManager::default();
        let job = jm.enqueue("task");
        let id = job.id.clone();
        jm.update_progress(&id, 150, Some("over".to_string()));
        let jobs = jm.list();
        let updated = jobs.iter().find(|j| j.id == id).unwrap();
        assert_eq!(updated.progress, Some(100));
    }

    #[test]
    fn complete_sets_progress_to_100() {
        let mut jm = JobManager::default();
        let job = jm.enqueue("task");
        let id = job.id.clone();
        jm.set_running(&id);
        jm.complete(&id);
        let jobs = jm.list();
        let updated = jobs.iter().find(|j| j.id == id).unwrap();
        assert_eq!(updated.status, JobStatus::Completed);
        assert_eq!(updated.progress, Some(100));
    }

    #[test]
    fn fail_increments_attempt_and_sets_backoff() {
        let mut jm = JobManager::default();
        let job = jm.enqueue("fragile");
        let id = job.id.clone();
        jm.set_running(&id);
        jm.fail(&id, "crashed");
        let jobs = jm.list();
        let updated = jobs.iter().find(|j| j.id == id).unwrap();
        assert_eq!(updated.status, JobStatus::Failed);
        assert_eq!(updated.retry.attempt, 1);
        assert!(updated.retry.next_backoff_ms > 0);
        assert!(updated.retry.next_retry_at.is_some());
        assert_eq!(updated.detail.as_deref(), Some("crashed"));
    }

    #[test]
    fn fail_clears_retry_after_max_attempts() {
        let mut jm = JobManager::default();
        let job = jm.enqueue("fragile");
        let id = job.id.clone();
        for _ in 0..=DEFAULT_JOB_MAX_ATTEMPTS {
            jm.set_running(&id);
            jm.fail(&id, "boom");
        }
        let jobs = jm.list();
        let updated = jobs.iter().find(|j| j.id == id).unwrap();
        assert_eq!(updated.retry.attempt, DEFAULT_JOB_MAX_ATTEMPTS);
        assert_eq!(updated.retry.next_backoff_ms, 0);
        assert!(updated.retry.next_retry_at.is_none());
    }

    #[test]
    fn cancel_sets_status_and_clears_retry() {
        let mut jm = JobManager::default();
        let job = jm.enqueue("task");
        let id = job.id.clone();
        jm.cancel(&id);
        let jobs = jm.list();
        let updated = jobs.iter().find(|j| j.id == id).unwrap();
        assert_eq!(updated.status, JobStatus::Cancelled);
        assert_eq!(updated.retry.next_backoff_ms, 0);
    }

    #[test]
    fn pause_and_resume_round_trip() {
        let mut jm = JobManager::default();
        let job = jm.enqueue("task");
        let id = job.id.clone();
        jm.set_running(&id);
        jm.pause(&id, Some("waiting".to_string()));
        let jobs = jm.list();
        let paused = jobs.iter().find(|j| j.id == id).unwrap();
        assert_eq!(paused.status, JobStatus::Paused);
        assert_eq!(paused.detail.as_deref(), Some("waiting"));

        jm.resume(&id, None);
        let jobs = jm.list();
        let resumed = jobs.iter().find(|j| j.id == id).unwrap();
        assert_eq!(resumed.status, JobStatus::Running);
        assert_eq!(resumed.history.last().unwrap().phase, "resumed");
    }

    #[test]
    fn list_returns_jobs_sorted_by_updated_at_desc() {
        let mut jm = JobManager::default();
        jm.enqueue("first");
        jm.enqueue("second");
        jm.enqueue("third");
        let jobs = jm.list();
        assert_eq!(jobs.len(), 3);
        for window in jobs.windows(2) {
            assert!(window[0].updated_at >= window[1].updated_at);
        }
    }

    #[test]
    fn history_returns_entries_for_existing_job() {
        let mut jm = JobManager::default();
        let job = jm.enqueue("task");
        let id = job.id.clone();
        jm.set_running(&id);
        jm.complete(&id);
        let history = jm.history(&id);
        assert_eq!(history.len(), 3); // created, running, completed
        assert_eq!(history[0].phase, "created");
        assert_eq!(history[1].phase, "running");
        assert_eq!(history[2].phase, "completed");
    }

    #[test]
    fn history_returns_empty_for_unknown_job() {
        let jm = JobManager::default();
        assert!(jm.history("nonexistent").is_empty());
    }

    #[test]
    fn resume_pending_requeues_running_and_queued() {
        let mut jm = JobManager::default();
        let _j1 = jm.enqueue("queued_task");
        let j2 = jm.enqueue("running_task");
        let j3 = jm.enqueue("completed_task");
        let id2 = j2.id.clone();
        let id3 = j3.id.clone();
        jm.set_running(&id2);
        jm.set_running(&id3);
        jm.complete(&id3);

        let resumed = jm.resume_pending();
        assert_eq!(resumed.len(), 2);
        for job in &resumed {
            assert_eq!(job.status, JobStatus::Queued);
        }
    }

    // ── JobManager: backoff ────────────────────────────────────────────

    #[test]
    fn deterministic_backoff_zero_on_first_attempt() {
        let retry = JobRetryMetadata {
            attempt: 0,
            ..Default::default()
        };
        assert_eq!(JobManager::deterministic_backoff_ms(&retry), 0);
    }

    #[test]
    fn deterministic_backoff_exponential_growth() {
        let base = DEFAULT_JOB_BACKOFF_BASE_MS;
        for attempt in 1..=5 {
            let retry = JobRetryMetadata {
                attempt,
                backoff_base_ms: base,
                ..Default::default()
            };
            let expected = base * 2u64.pow(attempt.saturating_sub(1).min(20));
            assert_eq!(
                JobManager::deterministic_backoff_ms(&retry),
                expected,
                "attempt {attempt}"
            );
        }
    }

    #[test]
    fn deterministic_backoff_saturates_at_high_exponent() {
        let retry = JobRetryMetadata {
            attempt: 63,
            backoff_base_ms: 1000,
            ..Default::default()
        };
        // Should not panic; result saturates
        let _ = JobManager::deterministic_backoff_ms(&retry);
    }

    // ── JobManager: history truncation ─────────────────────────────────

    #[test]
    fn push_history_truncates_beyond_max() {
        let mut jm = JobManager::default();
        let job = jm.enqueue("task");
        let id = job.id.clone();
        // Generate more history entries than the limit
        for i in 0..(MAX_JOB_HISTORY_ENTRIES + 20) {
            jm.update_progress(&id, (i % 100) as u8, Some(format!("step {i}")));
        }
        let history = jm.history(&id);
        assert_eq!(history.len(), MAX_JOB_HISTORY_ENTRIES);
    }

    // ── JobManager: persistence encoding/parsing ───────────────────────

    #[test]
    fn encode_and_parse_persisted_detail_round_trip() {
        let mut jm = JobManager::default();
        let job = jm.enqueue("task");
        let id = job.id.clone();
        jm.set_running(&id);
        jm.fail(&id, "oops");
        let job = jm.list().into_iter().find(|j| j.id == id).unwrap();

        let encoded = JobManager::encode_persisted_detail(&job).unwrap().unwrap();
        let parsed = JobManager::parse_persisted_detail(Some(&encoded)).unwrap();

        assert_eq!(parsed.status, job.status);
        assert_eq!(parsed.detail, job.detail);
        assert_eq!(parsed.retry.attempt, job.retry.attempt);
        assert_eq!(parsed.history.len(), job.history.len());
    }

    #[test]
    fn parse_persisted_detail_returns_none_for_none_input() {
        assert!(JobManager::parse_persisted_detail(None).is_none());
    }

    #[test]
    fn parse_persisted_detail_returns_none_for_invalid_json() {
        assert!(JobManager::parse_persisted_detail(Some("not json")).is_none());
    }

    // ── Helper functions ───────────────────────────────────────────────

    #[test]
    fn job_status_round_trip_str() {
        let statuses = [
            JobStatus::Queued,
            JobStatus::Running,
            JobStatus::Paused,
            JobStatus::Completed,
            JobStatus::Failed,
            JobStatus::Cancelled,
        ];
        for status in &statuses {
            let s = job_status_to_str(*status);
            let parsed = job_status_from_str(s);
            assert_eq!(parsed, Some(*status), "round-trip failed for {s:?}");
        }
    }

    #[test]
    fn job_status_from_str_returns_none_for_unknown() {
        assert_eq!(job_status_from_str("unknown"), None);
        assert_eq!(job_status_from_str(""), None);
    }

    #[test]
    fn runtime_status_to_job_state_maps_correctly() {
        assert_eq!(
            runtime_status_to_job_state(JobStatus::Queued),
            JobStateStatus::Queued
        );
        assert_eq!(
            runtime_status_to_job_state(JobStatus::Running),
            JobStateStatus::Running
        );
        assert_eq!(
            runtime_status_to_job_state(JobStatus::Paused),
            JobStateStatus::Paused
        );
        assert_eq!(
            runtime_status_to_job_state(JobStatus::Completed),
            JobStateStatus::Completed
        );
        assert_eq!(
            runtime_status_to_job_state(JobStatus::Failed),
            JobStateStatus::Failed
        );
        assert_eq!(
            runtime_status_to_job_state(JobStatus::Cancelled),
            JobStateStatus::Cancelled
        );
    }

    #[test]
    fn job_state_status_to_runtime_maps_correctly() {
        assert_eq!(
            job_state_status_to_runtime(JobStateStatus::Queued),
            JobStatus::Queued
        );
        assert_eq!(
            job_state_status_to_runtime(JobStateStatus::Running),
            JobStatus::Running
        );
        assert_eq!(
            job_state_status_to_runtime(JobStateStatus::Paused),
            JobStatus::Paused
        );
        assert_eq!(
            job_state_status_to_runtime(JobStateStatus::Completed),
            JobStatus::Completed
        );
        assert_eq!(
            job_state_status_to_runtime(JobStateStatus::Failed),
            JobStatus::Failed
        );
        assert_eq!(
            job_state_status_to_runtime(JobStateStatus::Cancelled),
            JobStatus::Cancelled
        );
    }

    #[test]
    fn json_optional_string_handles_null() {
        assert!(json_optional_string(&Value::Null).is_none());
    }

    #[test]
    fn json_optional_string_handles_string() {
        assert_eq!(
            json_optional_string(&Value::String("hello".to_string())),
            Some("hello".to_string())
        );
    }

    #[test]
    fn json_optional_string_handles_non_string() {
        assert!(json_optional_string(&json!(42)).is_none());
    }

    #[test]
    fn parse_retry_metadata_returns_default_for_none() {
        let retry = parse_retry_metadata(None);
        assert_eq!(retry.attempt, 0);
        assert_eq!(retry.max_attempts, DEFAULT_JOB_MAX_ATTEMPTS);
        assert_eq!(retry.backoff_base_ms, DEFAULT_JOB_BACKOFF_BASE_MS);
    }

    #[test]
    fn parse_retry_metadata_parses_fields() {
        let value = json!({
            "attempt": 2,
            "max_attempts": 5,
            "backoff_base_ms": 1000,
            "next_backoff_ms": 2000,
            "next_retry_at": 1234567890i64
        });
        let retry = parse_retry_metadata(Some(&value));
        assert_eq!(retry.attempt, 2);
        assert_eq!(retry.max_attempts, 5);
        assert_eq!(retry.backoff_base_ms, 1000);
        assert_eq!(retry.next_backoff_ms, 2000);
        assert_eq!(retry.next_retry_at, Some(1234567890));
    }

    #[test]
    fn parse_history_entry_returns_none_without_status() {
        let value = json!({"at": 1, "phase": "test"});
        assert!(parse_history_entry(&value).is_none());
    }

    #[test]
    fn parse_history_entry_parses_valid_entry() {
        let value = json!({
            "at": 100,
            "phase": "running",
            "status": "running",
            "progress": 50,
            "detail": "working",
            "retry": {"attempt": 0, "max_attempts": 3, "backoff_base_ms": 500}
        });
        let entry = parse_history_entry(&value).unwrap();
        assert_eq!(entry.at, 100);
        assert_eq!(entry.phase, "running");
        assert_eq!(entry.status, JobStatus::Running);
        assert_eq!(entry.progress, Some(50));
        assert_eq!(entry.detail.as_deref(), Some("working"));
    }

    #[test]
    fn paused_job_persists_as_paused_not_running() {
        let store = temp_core_state("paused-persist");
        let mut jm = JobManager::default();
        let job = jm.enqueue("task");
        let id = job.id.clone();
        jm.set_running(&id);
        jm.pause(&id, Some("waiting".to_string()));
        jm.persist_job(&store, &id).expect("persist paused job");

        let persisted = store.list_jobs(Some(10)).expect("list jobs");
        let record = persisted.iter().find(|job| job.id == id).unwrap();
        assert_eq!(record.status, JobStateStatus::Paused);

        let mut reloaded = JobManager::default();
        reloaded.load_from_store(&store).expect("reload jobs");
        let jobs = reloaded.list();
        let reloaded_job = jobs.iter().find(|job| job.id == id).unwrap();
        assert_eq!(reloaded_job.status, JobStatus::Paused);
    }

    // ── O1: JobRecord → AgentRunSnapshot adapter ────────────────────────

    fn sample_job_record(status: JobStatus, detail: Option<&str>) -> JobRecord {
        JobRecord {
            id: "job-o1-1".to_string(),
            name: "sample".to_string(),
            status,
            progress: None,
            detail: detail.map(str::to_string),
            retry: JobRetryMetadata {
                attempt: 0,
                max_attempts: DEFAULT_JOB_MAX_ATTEMPTS,
                backoff_base_ms: DEFAULT_JOB_BACKOFF_BASE_MS,
                next_backoff_ms: 0,
                next_retry_at: None,
            },
            history: Vec::new(),
            created_at: 1_700_000_000,
            updated_at: 1_700_000_042,
        }
    }

    #[test]
    fn job_record_to_agent_run_maps_non_terminal_states() {
        use codewhale_protocol::agent_run::RunState;

        for (status, expected) in [
            (JobStatus::Queued, RunState::Queued),
            (JobStatus::Running, RunState::Running),
            (JobStatus::Paused, RunState::Paused),
        ] {
            let snapshot = job_record_to_agent_run(&sample_job_record(status, None));
            assert!(snapshot.is_coherent());
            assert_eq!(snapshot.run_id, "job-o1-1");
            assert_eq!(snapshot.parent, None);
            assert_eq!(
                snapshot.source,
                codewhale_protocol::agent_run::RunSource::CoreJob
            );
            assert_eq!(snapshot.state, expected);
            assert!(snapshot.terminal.is_none());
            assert!(snapshot.refs.is_empty());
            assert_eq!(
                snapshot.budget,
                codewhale_protocol::agent_run::BudgetSummary::default()
            );
        }
    }

    #[test]
    fn job_record_to_agent_run_maps_terminal_states_without_fabricating_fields() {
        use codewhale_protocol::agent_run::{RunState, TerminalOutcome};

        let cases = [
            (
                JobStatus::Completed,
                TerminalOutcome::Completed,
                Some("done"),
            ),
            (JobStatus::Failed, TerminalOutcome::Failed, Some("boom")),
            (JobStatus::Cancelled, TerminalOutcome::Cancelled, None),
        ];

        for (status, outcome, detail) in cases {
            let snapshot = job_record_to_agent_run(&sample_job_record(status, detail));
            assert!(snapshot.is_coherent());
            assert_eq!(snapshot.state, RunState::Terminal);
            let terminal = snapshot.terminal.expect("terminal summary");
            assert_eq!(terminal.outcome, outcome);
            assert_eq!(terminal.ended_at_ms, Some(1_700_000_042_000));
            assert_eq!(terminal.detail, None);
            assert_eq!(
                snapshot.budget,
                codewhale_protocol::agent_run::BudgetSummary::default()
            );
            assert!(snapshot.refs.is_empty());
            assert_eq!(snapshot.parent, None);
        }
    }

    #[test]
    fn job_record_to_agent_run_does_not_export_unclassified_detail() {
        let record = sample_job_record(JobStatus::Failed, Some("owner-private diagnostic"));
        let snapshot = job_record_to_agent_run(&record);
        let terminal = snapshot.terminal.as_ref().expect("terminal summary");
        assert_eq!(terminal.detail, None);
        let serialized = serde_json::to_string(&snapshot).expect("serialize snapshot");
        assert!(!serialized.contains("owner-private diagnostic"));
    }

    #[test]
    fn job_record_to_agent_run_omits_ended_at_on_updated_at_overflow() {
        let mut record = sample_job_record(JobStatus::Completed, Some("ok"));
        record.updated_at = i64::MAX;
        let snapshot = job_record_to_agent_run(&record);
        assert!(snapshot.is_coherent());
        let terminal = snapshot.terminal.expect("terminal summary");
        assert_eq!(terminal.ended_at_ms, None);
    }

    #[test]
    fn runtime_bookkeeping_retains_jobs_and_read_only_legacy_store() {
        let store = temp_core_state("read-only-runtime");
        let before = store.list_threads(Default::default()).unwrap();
        let mut runtime = Runtime::new(ConfigToml::default(), store, HookDispatcher::default());
        let job = runtime.jobs.enqueue("retained job");
        runtime
            .jobs
            .persist_job(runtime.state_store(), &job.id)
            .unwrap();
        runtime.update_config(ConfigToml::default());
        assert_eq!(runtime.app_status().data["jobs"][0]["id"], job.id);
        assert_eq!(
            runtime
                .state_store()
                .list_threads(Default::default())
                .unwrap()
                .len(),
            before.len()
        );
        let reopened = Runtime::new(
            runtime.config.clone(),
            runtime.state_store().clone(),
            HookDispatcher::default(),
        );
        assert_eq!(reopened.jobs.list().len(), 1);
        assert!(
            reopened
                .state_store()
                .list_threads(Default::default())
                .unwrap()
                .is_empty()
        );
    }
}
