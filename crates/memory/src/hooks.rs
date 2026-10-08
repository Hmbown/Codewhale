//! Typed, pure hook planning. Hooks return intentions, not shell commands.
//! Permission checks remain with the host. Completion receipts are recorded only
//! AFTER the host finishes an operation; retries use that operation's own key.
use crate::store::Tx;
use crate::{Access, Capability, Error, Result, Scope, Store, policy};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Boundary {
    SessionStart,
    TaskStart,
    BeforePlanning,
    BeforeDispatch,
    BeforeCompaction,
    AfterCompaction,
    RepositoryChanged,
    SubagentSpawn,
    SubagentJoin,
    UserRemember,
    UserCorrection,
    TestFinished,
    TaskFinished,
    SessionClose,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HookEvent {
    pub id: String,
    pub boundary: Boundary,
    pub trace_id: String,
    pub sequence: u64,
    pub observed_at: i64,
    #[serde(default)]
    pub explicit_user_request: bool,
    #[serde(default)]
    pub success: Option<bool>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Intent {
    Recall,
    ValidatePreparedContext,
    SaveCheckpoint,
    ReconcilePendingOperations,
    RefreshRepositoryFingerprints,
    AttenuateSubagentScope,
    ProposeCandidate,
    RequestCorrectionReview,
    AttachOutcomeEvidence,
    FlushOutbox,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookPlan {
    pub enabled: bool,
    pub intents: Vec<Intent>,
    pub approval_required: bool,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookPolicy {
    pub enabled: bool,
    pub auto_candidates: bool,
    pub recall_on_task: bool,
}
impl Default for HookPolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            auto_candidates: false,
            recall_on_task: true,
        }
    }
}

pub fn plan(policy: &HookPolicy, event: &HookEvent) -> HookPlan {
    use Boundary::*;
    use Intent::*;
    if !policy.enabled {
        return HookPlan {
            enabled: false,
            intents: vec![],
            approval_required: false,
            reason: "disabled_no_io".into(),
        };
    }
    let mut approval = false;
    let intents = match event.boundary {
        SessionStart => vec![ReconcilePendingOperations],
        TaskStart | BeforePlanning if policy.recall_on_task => vec![Recall],
        BeforeDispatch => vec![ValidatePreparedContext],
        BeforeCompaction => vec![SaveCheckpoint],
        AfterCompaction => vec![ReconcilePendingOperations, Recall],
        RepositoryChanged => vec![RefreshRepositoryFingerprints],
        SubagentSpawn => vec![AttenuateSubagentScope, Recall],
        SubagentJoin => vec![AttachOutcomeEvidence],
        UserRemember => {
            approval = true;
            if event.explicit_user_request {
                vec![ProposeCandidate]
            } else {
                vec![]
            }
        }
        UserCorrection => {
            approval = true;
            vec![RequestCorrectionReview]
        }
        TestFinished | TaskFinished => {
            let mut jobs = vec![AttachOutcomeEvidence];
            if policy.auto_candidates && event.success == Some(true) {
                jobs.push(ProposeCandidate);
                approval = true;
            }
            jobs
        }
        SessionClose => vec![SaveCheckpoint, FlushOutbox],
        _ => vec![],
    };
    HookPlan {
        enabled: true,
        intents,
        approval_required: approval,
        reason: "typed_host_boundary".into(),
    }
}
impl Store {
    /// Durable deduplication of COMPLETED host hooks. This does not execute the
    /// plan, promise exactly-once external effects, or install an OS observer.
    pub fn record_completed_hook(
        &self,
        access: &Access,
        scope: &Scope,
        event: &HookEvent,
    ) -> Result<bool> {
        access.write(scope, Capability::ContextDispatch)?;
        policy::bounded(&event.id, "hook id", 128, true)?;
        policy::bounded(&event.trace_id, "trace id", 128, true)?;
        if event.observed_at < 0 || event.observed_at > self.timestamp().saturating_add(300) {
            return Err(Error::Invalid("invalid hook observation time".into()));
        }
        let digest = policy::sha256(&serde_json::to_vec(event)?);
        let tx = Tx::begin(&self.conn)?;
        let old: Option<String> = self
            .conn
            .query_row(
                "SELECT input_hash FROM memory_hook_receipts WHERE scope=?1 AND event_id=?2",
                params![scope.key()?, event.id],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(old) = old {
            return if old == digest {
                Ok(false)
            } else {
                Err(Error::IdempotencyConflict)
            };
        }
        self.conn.execute("INSERT INTO memory_hook_receipts(scope,event_id,input_hash,recorded_at) VALUES(?1,?2,?3,?4)",params![scope.key()?,event.id,digest,self.timestamp()])?;
        tx.commit()?;
        Ok(true)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn event(boundary: Boundary) -> HookEvent {
        HookEvent {
            id: "e".into(),
            boundary,
            trace_id: "r".into(),
            sequence: 1,
            observed_at: 1,
            explicit_user_request: false,
            success: None,
        }
    }
    #[test]
    fn off_is_empty() {
        assert!(
            plan(&HookPolicy::default(), &event(Boundary::SessionStart))
                .intents
                .is_empty()
        );
    }
    #[test]
    fn dispatch_never_auto_approves() {
        let p = HookPolicy {
            enabled: true,
            ..Default::default()
        };
        assert_eq!(
            plan(&p, &event(Boundary::BeforeDispatch)).intents,
            vec![Intent::ValidatePreparedContext]
        );
    }
    #[test]
    fn a_success_is_not_a_lesson() {
        let p = HookPolicy {
            enabled: true,
            auto_candidates: true,
            ..Default::default()
        };
        let mut e = event(Boundary::TestFinished);
        e.success = Some(true);
        let result = plan(&p, &e);
        assert!(result.approval_required);
        assert!(result.intents.contains(&Intent::ProposeCandidate));
    }
    #[test]
    fn restore_reconciles_before_recall() {
        let p = HookPolicy {
            enabled: true,
            ..Default::default()
        };
        assert_eq!(
            plan(&p, &event(Boundary::AfterCompaction)).intents,
            vec![Intent::ReconcilePendingOperations, Intent::Recall]
        );
    }
}
