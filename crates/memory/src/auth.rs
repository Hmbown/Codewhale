use crate::{Error, Result, Scope};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Capability {
    Read,
    Propose,
    Review,
    Correct,
    Forget,
    Index,
    Link,
    Checkpoint,
    Export,
    Maintenance,
    ContextDispatch,
}

/// Constructed by trusted host code. Deliberately NOT Deserialize: request bodies
/// cannot create or enlarge grants. These checks do not sandbox a hostile local OS user.
#[derive(Debug, Clone)]
pub struct Access {
    actor: String,
    reads: BTreeSet<Scope>,
    writes: BTreeSet<Scope>,
    capabilities: BTreeSet<Capability>,
}
impl Access {
    pub fn new(
        actor: impl Into<String>,
        reads: Vec<Scope>,
        writes: Vec<Scope>,
        caps: Vec<Capability>,
    ) -> Result<Self> {
        let actor = actor.into();
        if actor.trim().is_empty() || actor.len() > 256 {
            return Err(Error::Invalid("actor identity is required".into()));
        }
        if reads.len() > 64 || writes.len() > 64 {
            return Err(Error::Invalid(
                "at most 64 exact scopes may be granted".into(),
            ));
        }
        for scope in reads.iter().chain(writes.iter()) {
            scope.validate()?;
        }
        if let Some(first) = reads.first()
            && reads
                .iter()
                .chain(writes.iter())
                .any(|s| s.tenant != first.tenant || s.user != first.user)
        {
            return Err(Error::Invalid(
                "one access context must belong to exactly one tenant and user".into(),
            ));
        }
        // One trusted freshness snapshot represents one live workspace/branch/
        // session/agent. Do not apply its file hashes to another repository.
        for field in 0..4 {
            let values: BTreeSet<&str> = reads
                .iter()
                .filter_map(|s| match field {
                    0 => s.workspace.as_deref(),
                    1 => s.branch.as_deref(),
                    2 => s.session.as_deref(),
                    _ => s.agent.as_deref(),
                })
                .collect();
            if values.len() > 1 {
                return Err(Error::Invalid("one access context cannot span different workspace, branch, session, or agent identities".into()));
            }
        }
        let reads: BTreeSet<_> = reads.into_iter().collect();
        let writes: BTreeSet<_> = writes.into_iter().collect();
        if !writes.is_subset(&reads) {
            return Err(Error::Denied);
        }
        Ok(Self {
            actor,
            reads,
            writes,
            capabilities: caps.into_iter().collect(),
        })
    }
    pub fn operator(scopes: Vec<Scope>) -> Result<Self> {
        Self::new(
            "local-operator",
            scopes.clone(),
            scopes,
            vec![
                Capability::Read,
                Capability::Propose,
                Capability::Review,
                Capability::Correct,
                Capability::Forget,
                Capability::Index,
                Capability::Link,
                Capability::Checkpoint,
                Capability::Export,
                Capability::Maintenance,
                Capability::ContextDispatch,
            ],
        )
    }
    pub fn agent(scopes: Vec<Scope>) -> Result<Self> {
        Self::new(
            "agent",
            scopes.clone(),
            scopes,
            vec![
                Capability::Read,
                Capability::Propose,
                Capability::Link,
                Capability::Checkpoint,
            ],
        )
    }
    pub fn readonly(scopes: Vec<Scope>) -> Result<Self> {
        Self::new("reader", scopes, vec![], vec![Capability::Read])
    }
    pub fn delegate(
        &self,
        actor: impl Into<String>,
        reads: Vec<Scope>,
        writes: Vec<Scope>,
        caps: Vec<Capability>,
    ) -> Result<Self> {
        let child = Self::new(actor, reads, writes, caps)?;
        if !child.reads.is_subset(&self.reads)
            || !child.writes.is_subset(&self.writes)
            || !child.capabilities.is_subset(&self.capabilities)
        {
            return Err(Error::Denied);
        }
        Ok(child)
    }
    pub fn has(&self, cap: Capability) -> bool {
        self.capabilities.contains(&cap)
    }
    pub fn require(&self, cap: Capability) -> Result<()> {
        if self.has(cap) {
            Ok(())
        } else {
            Err(Error::Denied)
        }
    }
    pub fn read(&self, scope: &Scope) -> Result<()> {
        self.require(Capability::Read)?;
        if self.reads.contains(scope) {
            Ok(())
        } else {
            Err(Error::NotFound)
        }
    }
    pub fn write(&self, scope: &Scope, cap: Capability) -> Result<()> {
        self.require(cap)?;
        if self.writes.contains(scope) {
            Ok(())
        } else {
            Err(Error::Denied)
        }
    }
    pub fn scopes(&self) -> impl Iterator<Item = &Scope> {
        self.reads.iter()
    }
    pub fn writable_scopes(&self) -> impl Iterator<Item = &Scope> {
        self.writes.iter()
    }
    pub(crate) fn scope_json(&self) -> Result<String> {
        Ok(serde_json::to_string(
            &self
                .reads
                .iter()
                .map(Scope::key)
                .collect::<Result<Vec<_>>>()?,
        )?)
    }
    pub(crate) fn actor_hash(&self) -> String {
        crate::policy::sha256(self.actor.as_bytes())
    }
}
