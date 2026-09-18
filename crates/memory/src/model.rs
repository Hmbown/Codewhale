use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

macro_rules! string_enum {
    ($name:ident { $($variant:ident => $value:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
        pub enum $name { $(#[serde(rename = $value)] $variant),+ }
        impl $name {
            pub fn as_str(self) -> &'static str { match self { $(Self::$variant => $value),+ } }
            pub fn parse(s: &str) -> Result<Self> {
                match s { $($value => Ok(Self::$variant)),+, _ => Err(Error::Invalid("unknown enum value".into())) }
            }
        }
    }
}
string_enum!(Kind {
    Preference => "preference", Fact => "fact", Decision => "decision",
    Constraint => "constraint", Procedure => "procedure", Lesson => "lesson",
    Commitment => "commitment", Episode => "episode", Handoff => "handoff"
});
string_enum!(Status { Candidate => "candidate", Active => "active", Stale => "stale", Superseded => "superseded", Rejected => "rejected" });
string_enum!(SourceKind { User => "user", Repository => "repository", Tool => "tool", Import => "import", Agent => "agent" });
string_enum!(Relation { Related => "related", Supports => "supports", Contradicts => "contradicts", DecisionOutcome => "decision_outcome" });
string_enum!(Freshness { Current => "current", Unknown => "unknown", Changed => "changed", Expired => "expired", Inactive => "inactive" });

/// Scope IDs are opaque, host-assigned identities, NOT filesystem paths or ACLs.
/// Every grant is an exact scope match; no prefix or wildcard matching occurs.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    pub tenant: String,
    pub user: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
}
impl Scope {
    pub fn user(tenant: impl Into<String>, user: impl Into<String>) -> Self {
        Self {
            tenant: tenant.into(),
            user: user.into(),
            workspace: None,
            branch: None,
            session: None,
            agent: None,
        }
    }
    pub fn workspace(&self, id: impl Into<String>) -> Self {
        Self {
            workspace: Some(id.into()),
            branch: None,
            session: None,
            agent: None,
            ..self.clone()
        }
    }
    pub fn branch(&self, id: impl Into<String>) -> Self {
        Self {
            branch: Some(id.into()),
            session: None,
            agent: None,
            ..self.clone()
        }
    }
    pub fn session(&self, id: impl Into<String>) -> Self {
        Self {
            session: Some(id.into()),
            agent: None,
            ..self.clone()
        }
    }
    pub fn agent(&self, id: impl Into<String>) -> Self {
        Self {
            agent: Some(id.into()),
            ..self.clone()
        }
    }
    pub fn key(&self) -> Result<String> {
        Ok(serde_json::to_string(self)?)
    }
    pub fn validate(&self) -> Result<()> {
        for value in std::iter::once(&self.tenant)
            .chain(std::iter::once(&self.user))
            .chain(self.workspace.iter())
            .chain(self.branch.iter())
            .chain(self.session.iter())
            .chain(self.agent.iter())
        {
            if value.trim().is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
                return Err(Error::Invalid(
                    "scope identities must be nonempty, bounded, and printable".into(),
                ));
            }
        }
        if (self.branch.is_some() && self.workspace.is_none())
            || (self.agent.is_some() && self.session.is_none())
        {
            return Err(Error::Invalid(
                "branch requires workspace; agent requires session".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub kind: SourceKind,
    pub uri: String,
    #[serde(default)]
    pub locator: String,
    #[serde(default)]
    pub sha256: Option<String>,
    /// Unix seconds; observation time is evidence metadata, not an authority grant.
    pub observed_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Draft {
    pub scope: Scope,
    pub kind: Kind,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "default_confidence")]
    pub confidence: f64,
    #[serde(default = "default_importance")]
    pub importance: f64,
    pub evidence: Vec<Evidence>,
    #[serde(default)]
    pub repository_revision: Option<String>,
    #[serde(default)]
    pub dependencies: BTreeMap<String, String>,
    #[serde(default)]
    pub expires_at: Option<i64>,
    /// Valid time (Unix seconds), separate from recorded/knowledge time.
    #[serde(default)]
    pub valid_from: Option<i64>,
    #[serde(default)]
    pub valid_until: Option<i64>,
    #[serde(default)]
    pub parent_ids: Vec<String>,
}
fn default_confidence() -> f64 {
    0.5
}
fn default_importance() -> f64 {
    0.5
}
impl Draft {
    pub fn note(
        scope: Scope,
        title: impl Into<String>,
        body: impl Into<String>,
        evidence: Evidence,
    ) -> Self {
        Self {
            scope,
            kind: Kind::Fact,
            title: title.into(),
            body: body.into(),
            key: None,
            tags: vec![],
            confidence: 0.5,
            importance: 0.5,
            evidence: vec![evidence],
            repository_revision: None,
            dependencies: BTreeMap::new(),
            expires_at: None,
            valid_from: None,
            valid_until: None,
            parent_ids: vec![],
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: String,
    pub revision: i64,
    pub status: Status,
    pub draft: Draft,
    pub created_at: i64,
    pub updated_at: i64,
    pub content_hash: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureReceipt {
    pub memory: Memory,
    pub created: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationReceipt {
    pub content_hash: String,
    pub validator: String,
    pub evidence_uri: String,
    pub passed: bool,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub revision: Option<String>,
    #[serde(default)]
    pub files: BTreeMap<String, String>,
}
impl Memory {
    pub fn freshness(&self, snapshot: &Snapshot, now: i64) -> Freshness {
        if self.draft.valid_from.is_some_and(|t| t > now) {
            return Freshness::Unknown;
        }
        if self.draft.valid_until.is_some_and(|t| t <= now) {
            return Freshness::Expired;
        }
        if self.draft.expires_at.is_some_and(|t| t <= now) {
            return Freshness::Expired;
        }
        if !matches!(self.status, Status::Active | Status::Stale) {
            return Freshness::Inactive;
        }
        if self.status == Status::Stale {
            return Freshness::Changed;
        }
        if !self.draft.dependencies.is_empty() {
            let mut unknown = false;
            for (path, digest) in &self.draft.dependencies {
                match snapshot.files.get(path) {
                    Some(current) if current == digest => (),
                    Some(_) => return Freshness::Changed,
                    None => unknown = true,
                }
            }
            return if unknown {
                Freshness::Unknown
            } else {
                Freshness::Current
            };
        }
        match (&self.draft.repository_revision, &snapshot.revision) {
            (None, _) => Freshness::Current,
            (Some(_), None) => Freshness::Unknown,
            (Some(old), Some(current)) if old == current => Freshness::Current,
            _ => Freshness::Changed,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Embedding {
    pub model: String,
    pub vector: Vec<f32>,
}
#[derive(Debug, Clone)]
pub struct Recall {
    pub query: String,
    pub limit: usize,
    pub snapshot: Snapshot,
    pub include_stale: bool,
    pub embedding: Option<Embedding>,
    pub vector_scan_limit: usize,
    pub expand_graph: bool,
}
impl Default for Recall {
    fn default() -> Self {
        Self {
            query: String::new(),
            limit: 12,
            snapshot: Snapshot::default(),
            include_stale: false,
            embedding: None,
            vector_scan_limit: 10_000,
            expand_graph: true,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hit {
    pub memory: Memory,
    pub freshness: Freshness,
    pub score: f64,
    pub reasons: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecallReport {
    pub hits: Vec<Hit>,
    pub vector_candidates: usize,
    pub vector_scan_truncated: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgetReport {
    pub memories_deleted: usize,
    pub checkpoints_deleted: usize,
    pub wal_truncated: bool,
    pub physical_erasure_guaranteed: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingOperation {
    pub operation_id: String,
    pub state: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkingState {
    pub summary: String,
    #[serde(default)]
    pub next_steps: Vec<String>,
    #[serde(default)]
    pub artifact_refs: Vec<String>,
    #[serde(default)]
    pub pending_operations: Vec<PendingOperation>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointDraft {
    pub scope: Scope,
    pub key: String,
    pub state: WorkingState,
    #[serde(default)]
    pub memory_ids: Vec<String>,
    #[serde(default)]
    pub expires_at: Option<i64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRef {
    pub id: String,
    pub revision: i64,
    pub content_hash: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: String,
    pub revision: i64,
    pub draft: CheckpointDraft,
    pub memories: Vec<MemoryRef>,
    pub updated_at: i64,
    pub expires_at: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resume {
    pub checkpoint: Checkpoint,
    pub invalidated_memory_ids: Vec<String>,
    /// A checkpoint can NEVER restore approvals, permissions or completed side effects.
    pub reconcile_pending_operations: bool,
}
