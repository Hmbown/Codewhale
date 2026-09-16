//! Engine-owned Context Lens and Whalesong event projection.
//!
//! A prepared packet is NOT proof of context delivery. Only the trusted engine
//! can acknowledge append/dispatch. No tool can upgrade its own authority.
use crate::store::Tx;
use crate::{
    Access, Capability, ContextBudget, ContextPacket, Error, Freshness, Memory, MemoryBackend,
    MemoryRef, Recall, RecallReport, Result, Scope, Snapshot, Store, TokenCounter, compile_context,
    policy,
};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Preferences {
    pub revision: i64,
    pub pinned: bool,
    pub suppressed: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LensEntry {
    pub memory: Memory,
    pub freshness: Freshness,
    pub preferences: Preferences,
    pub in_contexts: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextReceipt {
    pub id: String,
    pub trace_id: String,
    pub stage: String,
    pub packet_hash: String,
    pub created_at: i64,
    pub appended_at: Option<i64>,
    pub dispatched_at: Option<i64>,
    pub used_units: usize,
    pub unit: String,
    pub omitted: usize,
    pub selected: Vec<MemoryRef>,
    /// Current knowledge cannot rewrite what a previous request contained.
    pub invalidated_ids: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LensSnapshot {
    pub schema: String,
    pub entries: Vec<LensEntry>,
    pub next_after: Option<String>,
    pub contexts: Vec<ContextReceipt>,
    pub as_of: i64,
    pub can_review: bool,
    pub can_forget: bool,
    pub can_dispatch: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedContext {
    pub receipt: ContextReceipt,
    pub packet: ContextPacket,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventPage {
    pub schema: String,
    pub producer: String,
    pub epoch: String,
    pub events: Vec<Value>,
    pub next_after: i64,
    pub has_more: bool,
    /// A filtered feed's seq gaps are not evidence of dropped records.
    pub sequence_domain: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalMemory {
    pub memory: Memory,
    pub known_at: i64,
    pub valid_at: i64,
    pub history_starts_at: i64,
    pub valid: bool,
    /// Historical file contents are not reconstructed by this API.
    pub repository_freshness: String,
}

fn identity(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-.:".contains(&b))
    {
        return Err(Error::Invalid("expected a bounded opaque identity".into()));
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)]
fn metadata_event(
    conn: &rusqlite::Connection,
    scope: &Scope,
    entity: &str,
    revision: i64,
    action: &str,
    now: i64,
    trace: Option<&str>,
    context: Option<&str>,
    units: Option<usize>,
    unit: Option<&str>,
) -> Result<()> {
    conn.execute("INSERT INTO memory_outbox(scope,entity_id,revision,action,recorded_at,trace_id,context_id,units,unit) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![scope.key()?,entity,revision,action,now,trace,context,units.map(|n|n as i64),unit])?;
    Ok(())
}

impl Store {
    pub fn preferences(&self, access: &Access, id: &str) -> Result<Preferences> {
        self.get(access, id)?;
        Ok(self
            .conn
            .query_row(
                "SELECT revision,pinned,suppressed FROM memory_preferences WHERE memory_id=?1",
                [id],
                |r| {
                    Ok(Preferences {
                        revision: r.get(0)?,
                        pinned: r.get(1)?,
                        suppressed: r.get(2)?,
                    })
                },
            )
            .optional()?
            .unwrap_or_default())
    }
    /// A pin affects context packing; it never bypasses validity, scope or review.
    /// Suppression affects future recall only, not context already sent.
    pub fn set_preferences(
        &self,
        access: &Access,
        id: &str,
        expected: i64,
        pinned: bool,
        suppressed: bool,
    ) -> Result<Preferences> {
        if !(0..i64::MAX).contains(&expected) {
            return Err(Error::Invalid("invalid preference revision".into()));
        }
        let tx = Tx::begin(&self.conn)?;
        let m = self.get(access, id)?;
        access.write(&m.draft.scope, Capability::Review)?;
        let old = self.preferences(access, id)?;
        if old.revision == expected + 1 && old.pinned == pinned && old.suppressed == suppressed {
            tx.commit()?;
            return Ok(old);
        }
        if old.revision != expected {
            return Err(Error::RevisionConflict);
        }
        self.conn.execute("INSERT INTO memory_preferences(memory_id,revision,pinned,suppressed) VALUES(?1,?2,?3,?4) ON CONFLICT(memory_id) DO UPDATE SET revision=excluded.revision,pinned=excluded.pinned,suppressed=excluded.suppressed",
            params![id,expected+1,pinned,suppressed])?;
        metadata_event(
            &self.conn,
            &m.draft.scope,
            id,
            m.revision,
            if suppressed {
                "suppressed"
            } else if pinned {
                "pinned"
            } else {
                "unrestricted"
            },
            self.timestamp(),
            None,
            None,
            None,
            None,
        )?;
        let flags = match (pinned, suppressed) {
            (true, true) => "preferences:11",
            (true, false) => "preferences:10",
            (false, true) => "preferences:01",
            (false, false) => "preferences:00",
        };
        self.conn.execute(
            "UPDATE memory_outbox SET reason_code=?1 WHERE seq=last_insert_rowid()",
            [flags],
        )?;
        tx.commit()?;
        Ok(Preferences {
            revision: expected + 1,
            pinned,
            suppressed,
        })
    }
    pub fn lens_snapshot(
        &self,
        access: &Access,
        trace: Option<&str>,
        after: Option<&str>,
        limit: usize,
        snapshot: &Snapshot,
    ) -> Result<LensSnapshot> {
        let tx = Tx::begin(&self.conn)?;
        let limit = limit.clamp(1, 100);
        let mut records = self.list(access, after, limit + 1)?;
        let more = records.len() > limit;
        records.truncate(limit);
        let contexts = self.context_receipts(access, trace, snapshot)?;
        let mut entries = Vec::new();
        for m in records {
            let preferences = self.preferences(access, &m.id)?;
            let freshness = self.freshness(access, &m, snapshot)?;
            let in_contexts = contexts
                .iter()
                .filter(|c| c.stage != "prepared" && c.selected.iter().any(|r| r.id == m.id))
                .map(|c| c.id.clone())
                .collect();
            entries.push(LensEntry {
                memory: m,
                freshness,
                preferences,
                in_contexts,
            });
        }
        let next_after = if more {
            entries.last().map(|e| e.memory.id.clone())
        } else {
            None
        };
        let out = LensSnapshot {
            schema: "codewhale.memory.lens/v1".into(),
            entries,
            next_after,
            contexts,
            as_of: self.timestamp(),
            can_review: access.has(Capability::Review),
            can_forget: access.has(Capability::Forget),
            can_dispatch: access.has(Capability::ContextDispatch),
        };
        tx.commit()?;
        Ok(out)
    }
    /// Two clocks: status known by `known_at`, fact valid at `valid_at`.
    /// Later supersession is not leaked into historical status. Forgotten content
    /// is unavailable at every cutoff. Migrated stores expose only known history.
    pub fn memory_as_of(
        &self,
        access: &Access,
        id: &str,
        known_at: i64,
        valid_at: i64,
    ) -> Result<HistoricalMemory> {
        if known_at < 0 || valid_at < 0 || known_at > self.timestamp() {
            return Err(Error::Invalid("invalid historical cutoff".into()));
        }
        let tx = Tx::begin(&self.conn)?;
        let mut memory = self.get(access, id)?;
        let (revision,status,recorded): (i64,String,i64)=self.conn.query_row(
            "SELECT revision,status,recorded_at FROM memory_history WHERE memory_id=?1 AND recorded_at<=?2 ORDER BY seq DESC LIMIT 1",
            params![id,known_at],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?.ok_or(Error::NotFound)?;
        let starts = self.conn.query_row(
            "SELECT min(recorded_at) FROM memory_history WHERE memory_id=?1",
            [id],
            |r| r.get(0),
        )?;
        memory.revision = revision;
        memory.status = crate::Status::parse(&status)?;
        memory.updated_at = recorded;
        let d = &memory.draft;
        let valid = d.valid_from.is_none_or(|t| t <= valid_at)
            && d.valid_until.is_none_or(|t| valid_at < t)
            && d.expires_at.is_none_or(|t| valid_at < t);
        let result = HistoricalMemory {
            memory,
            known_at,
            valid_at,
            history_starts_at: starts,
            valid,
            repository_freshness: "not_reconstructed".into(),
        };
        tx.commit()?;
        Ok(result)
    }
    /// The read-only half of `prepare_context`: filtered recall plus explicit
    /// pins, compiled into a packet — with no receipt recorded. Host surfaces
    /// that assemble the same bytes but have no session scope to attest
    /// (previews, reports, frozen prompts) must use this so the traced and
    /// untraced paths can never diverge on pin membership or packet shape.
    pub fn context_packet(
        &self,
        access: &Access,
        query: &Recall,
        counter: &dyn TokenCounter,
        budget: &ContextBudget,
    ) -> Result<(RecallReport, ContextPacket)> {
        let mut q = query.clone();
        q.include_stale = false;
        let mut report = self.recall(access, &q)?;
        // Explicit pins are included even when outside the lexical shortlist.
        let pinned: Vec<String> = {
            let mut stmt=self.conn.prepare("SELECT p.memory_id FROM memory_preferences p JOIN memories m ON m.id=p.memory_id WHERE p.pinned=1 AND p.suppressed=0 AND m.scope IN(SELECT value FROM json_each(?1)) ORDER BY p.memory_id LIMIT 64")?;
            stmt.query_map([access.scope_json()?], |r| r.get(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        for id in &pinned {
            if report.hits.iter().any(|h| &h.memory.id == id) {
                continue;
            }
            let memory = self.get(access, id)?;
            let freshness = self.freshness(access, &memory, &query.snapshot)?;
            if freshness == Freshness::Current {
                report.hits.push(crate::Hit {
                    memory,
                    freshness,
                    score: 0.0,
                    reasons: vec!["explicit_pin".into()],
                });
            }
        }
        report.hits.sort_by_key(|h| !pinned.contains(&h.memory.id));
        let packet = compile_context(&report.hits, counter, budget)?;
        Ok((report, packet))
    }
    /// Called by the host at a meaningful retrieval boundary, not every token.
    /// Idempotent within a trace + host request key. Text is returned, not logged.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_context(
        &self,
        access: &Access,
        context_scope: &Scope,
        trace: &str,
        request_key: &str,
        query: &Recall,
        counter: &dyn TokenCounter,
        budget: &ContextBudget,
    ) -> Result<PreparedContext> {
        access.write(context_scope, Capability::ContextDispatch)?;
        identity(trace)?;
        identity(request_key)?;
        if context_scope.session.is_none() {
            return Err(Error::Invalid(
                "context receipts require a session scope".into(),
            ));
        }
        let tx = Tx::begin(&self.conn)?;
        let input_hash = policy::sha256(&serde_json::to_vec(
            &json!({"q":query.query,"snapshot":query.snapshot,
            "limit":query.limit,"embedding":query.embedding,"graph":query.expand_graph,"vector_cap":query.vector_scan_limit,
            "scopes":access.scope_json()?,"budget":[budget.max_units,budget.max_bytes,budget.max_entries],"unit":counter.unit()}),
        )?);
        let (report, packet) = self.context_packet(access, query, counter, budget)?;
        let packet_hash = policy::sha256(packet.text.as_bytes());
        let old:Option<(String,String,String)>=self.conn.query_row("SELECT id,input_hash,packet_hash FROM memory_contexts WHERE scope=?1 AND trace_id=?2 AND request_key=?3",
            params![context_scope.key()?,trace,request_key],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
        if let Some((id, old_input, old_packet)) = old {
            if old_input != input_hash {
                return Err(Error::IdempotencyConflict);
            }
            if old_packet != packet_hash {
                return Err(Error::RevisionConflict);
            }
            let receipt = self
                .context_receipts(access, Some(trace), &query.snapshot)?
                .into_iter()
                .find(|c| c.id == id)
                .ok_or(Error::NotFound)?;
            tx.commit()?;
            return Ok(PreparedContext { receipt, packet });
        }
        let id = Uuid::new_v4().to_string();
        let now = self.timestamp();
        self.conn.execute("INSERT INTO memory_contexts(id,scope,trace_id,request_key,input_hash,packet_hash,stage,created_at,used_units,unit,omitted) VALUES(?1,?2,?3,?4,?5,?6,'prepared',?7,?8,?9,?10)",
            params![id,context_scope.key()?,trace,request_key,input_hash,packet_hash,now,packet.used_units as i64,packet.unit,packet.omitted as i64])?;
        for (position, r) in packet.selected.iter().enumerate() {
            self.conn.execute("INSERT INTO memory_context_refs(context_id,memory_id,revision,content_hash,position) VALUES(?1,?2,?3,?4,?5)",params![id,r.id,r.revision,r.content_hash,position as i64])?;
        }
        for hit in &report.hits {
            metadata_event(
                &self.conn,
                &hit.memory.draft.scope,
                &hit.memory.id,
                hit.memory.revision,
                "retrieved",
                now,
                Some(trace),
                Some(&id),
                None,
                None,
            )?;
        }
        for r in &packet.selected {
            let memory = self.get(access, &r.id)?;
            metadata_event(
                &self.conn,
                &memory.draft.scope,
                &r.id,
                r.revision,
                "context_prepared",
                now,
                Some(trace),
                Some(&id),
                Some(packet.used_units),
                Some(&packet.unit),
            )?;
        }
        let receipt = self
            .context_receipts(access, Some(trace), &query.snapshot)?
            .into_iter()
            .find(|c| c.id == id)
            .ok_or(Error::NotFound)?;
        tx.commit()?;
        Ok(PreparedContext { receipt, packet })
    }
    /// Before-action gate. The host must serialize this check and append/dispatch
    /// against mutations within its single owner. The acknowledgement is separate.
    pub fn preflight_context(
        &self,
        access: &Access,
        id: &str,
        packet_hash: &str,
        snapshot: &Snapshot,
    ) -> Result<()> {
        access.require(Capability::ContextDispatch)?;
        let tx = Tx::begin(&self.conn)?;
        let stored:Option<String>=self.conn.query_row("SELECT packet_hash FROM memory_contexts WHERE id=?1 AND scope IN(SELECT value FROM json_each(?2))",params![id,access.scope_json()?],|r|r.get(0)).optional()?;
        if stored.as_deref() != Some(packet_hash) {
            return Err(Error::RevisionConflict);
        }
        for r in self.context_refs(id)? {
            let m = self.get(access, &r.id)?;
            if m.revision != r.revision
                || m.content_hash != r.content_hash
                || self.freshness(access, &m, snapshot)? != Freshness::Current
                || self.preferences(access, &r.id)?.suppressed
            {
                return Err(Error::RevisionConflict);
            }
        }
        tx.commit()?;
        Ok(())
    }
    /// Invoke ONLY after the engine has durably appended these exact packet bytes.
    /// An error means missing acknowledgement, not permission to append twice.
    pub fn acknowledge_append(
        &self,
        access: &Access,
        id: &str,
        packet_hash: &str,
        history_sequence: i64,
        snapshot: &Snapshot,
    ) -> Result<()> {
        access.require(Capability::ContextDispatch)?;
        if history_sequence < 0 {
            return Err(Error::Invalid("invalid history sequence".into()));
        }
        self.advance_context(
            access,
            id,
            packet_hash,
            Some(history_sequence),
            None,
            snapshot,
        )
    }
    /// Invoke after a transport dispatch. Does not claim provider acceptance,
    /// successful execution, useful recall, or causal attribution of an outcome.
    pub fn acknowledge_dispatch(
        &self,
        access: &Access,
        id: &str,
        packet_hash: &str,
        transport_key: &str,
        snapshot: &Snapshot,
    ) -> Result<()> {
        access.require(Capability::ContextDispatch)?;
        identity(transport_key)?;
        self.advance_context(access, id, packet_hash, None, Some(transport_key), snapshot)
    }
    fn advance_context(
        &self,
        access: &Access,
        id: &str,
        hash: &str,
        history: Option<i64>,
        transport: Option<&str>,
        snapshot: &Snapshot,
    ) -> Result<()> {
        let tx = Tx::begin(&self.conn)?;
        let (scope,trace,stage,stored_hash,old_history,old_transport):(String,String,String,String,Option<i64>,Option<String>)=
            self.conn.query_row("SELECT scope,trace_id,stage,packet_hash,history_sequence,transport_key FROM memory_contexts WHERE id=?1 AND scope IN(SELECT value FROM json_each(?2))",params![id,access.scope_json()?],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).optional()?.ok_or(Error::NotFound)?;
        let scope: Scope = serde_json::from_str(&scope)?;
        access.write(&scope, Capability::ContextDispatch)?;
        if hash != stored_hash {
            return Err(Error::RevisionConflict);
        }
        if let Some(n) = history
            && let Some(old) = old_history
        {
            return if old == n {
                Ok(())
            } else {
                Err(Error::IdempotencyConflict)
            };
        }
        if let Some(key) = transport
            && let Some(old) = old_transport
        {
            return if old == key {
                Ok(())
            } else {
                Err(Error::IdempotencyConflict)
            };
        }
        let expected = if history.is_some() {
            "prepared"
        } else {
            "appended"
        };
        if stage != expected {
            return Err(Error::InvalidState);
        }
        let refs = self.context_refs(id)?;
        // This records an already-observed engine action, even when knowledge
        // became stale afterwards. `preflight_context` is the BEFORE-action gate.
        // Lens inspection marks invalidated references without rewriting history.
        let _ = snapshot;
        let now = self.timestamp();
        let (next, action) = if history.is_some() {
            ("appended", "context_appended")
        } else {
            ("dispatched", "context_dispatched")
        };
        if let Some(sequence) = history {
            self.conn.execute("UPDATE memory_contexts SET stage='appended',appended_at=?2,history_sequence=?3 WHERE id=?1",params![id,now,sequence])?;
        } else {
            self.conn.execute("UPDATE memory_contexts SET stage='dispatched',dispatched_at=?2,transport_key=?3 WHERE id=?1",params![id,now,transport])?;
        }
        for r in refs {
            let m = self.get(access, &r.id)?;
            metadata_event(
                &self.conn,
                &m.draft.scope,
                &r.id,
                r.revision,
                action,
                now,
                Some(&trace),
                Some(id),
                None,
                None,
            )?;
        }
        let _ = next;
        tx.commit()?;
        Ok(())
    }
    fn context_refs(&self, id: &str) -> Result<Vec<MemoryRef>> {
        let mut q=self.conn.prepare("SELECT memory_id,revision,content_hash FROM memory_context_refs WHERE context_id=?1 ORDER BY position")?;
        Ok(q.query_map([id], |r| {
            Ok(MemoryRef {
                id: r.get(0)?,
                revision: r.get(1)?,
                content_hash: r.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?)
    }
    pub fn context_receipts(
        &self,
        access: &Access,
        trace: Option<&str>,
        snapshot: &Snapshot,
    ) -> Result<Vec<ContextReceipt>> {
        access.require(Capability::Read)?;
        if let Some(t) = trace {
            identity(t)?;
        }
        let mut stmt=self.conn.prepare("SELECT id,trace_id,stage,packet_hash,created_at,appended_at,dispatched_at,used_units,unit,omitted FROM memory_contexts WHERE scope IN(SELECT value FROM json_each(?1)) AND (?2 IS NULL OR trace_id=?2) ORDER BY created_at DESC,rowid DESC LIMIT 100")?;
        let mut receipts = stmt
            .query_map(params![access.scope_json()?, trace], |r| {
                Ok(ContextReceipt {
                    id: r.get(0)?,
                    trace_id: r.get(1)?,
                    stage: r.get(2)?,
                    packet_hash: r.get(3)?,
                    created_at: r.get(4)?,
                    appended_at: r.get(5)?,
                    dispatched_at: r.get(6)?,
                    used_units: r.get::<_, i64>(7)? as usize,
                    unit: r.get(8)?,
                    omitted: r.get::<_, i64>(9)? as usize,
                    selected: vec![],
                    invalidated_ids: vec![],
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for c in &mut receipts {
            let refs = self.context_refs(&c.id)?;
            // Fail closed on a narrower capability grant; never reveal a referenced
            // global/private identity merely because the caller can read a session.
            for r in refs {
                let m = match self.get(access, &r.id) {
                    Ok(m) => m,
                    Err(Error::NotFound) => continue,
                    Err(e) => return Err(e),
                };
                if m.revision != r.revision
                    || m.content_hash != r.content_hash
                    || self.freshness(access, &m, snapshot)? != Freshness::Current
                    || self.preferences(access, &r.id)?.suppressed
                {
                    c.invalidated_ids.push(r.id.clone());
                }
                c.selected.push(r);
            }
        }
        Ok(receipts)
    }
    /// Metadata-only Whalesong event-v1 export. UUIDs/hashes/timing are linkable,
    /// not anonymous. No body/title/evidence URI/query/embedding enters the feed.
    pub fn event_page(&self, access: &Access, after: i64, limit: usize) -> Result<EventPage> {
        access.require(Capability::Read)?;
        if after < 0 {
            return Err(Error::Invalid("negative cursor".into()));
        }
        let (producer, epoch): (String, String) = self.conn.query_row(
            "SELECT producer,epoch FROM memory_observer WHERE singleton=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let mut q=self.conn.prepare("SELECT seq,scope,entity_id,revision,action,recorded_at,trace_id,context_id,units,unit,reason_code FROM memory_outbox WHERE seq>?1 AND scope IN(SELECT value FROM json_each(?2)) ORDER BY seq LIMIT ?3")?;
        let limit = limit.clamp(1, 500);
        let records = q
            .query_map(
                params![after, access.scope_json()?, (limit + 1) as i64],
                |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, i64>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, i64>(5)?,
                        r.get::<_, Option<String>>(6)?,
                        r.get::<_, Option<String>>(7)?,
                        r.get::<_, Option<i64>>(8)?,
                        r.get::<_, Option<String>>(9)?,
                        r.get::<_, Option<String>>(10)?,
                    ))
                },
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let more = records.len() > limit;
        let mut cursor = after;
        let mut events = Vec::new();
        for (seq, scope, id, revision, action, time, trace, context, units, unit, reason_code) in
            records.into_iter().take(limit)
        {
            cursor = seq;
            let mut attributes = BTreeMap::<String, Value>::new();
            for (k, v) in [
                ("memory.schema", json!(1)),
                ("memory.id", json!(id)),
                ("memory.scope", json!(policy::sha256(scope.as_bytes()))),
                ("memory.revision", json!(revision)),
                ("memory.action", json!(action)),
                ("memory.producer", json!(producer)),
                ("memory.epoch", json!(epoch)),
                ("memory.sequence", json!(seq)),
                ("memory.receipt_delay_ms", json!(0)),
                ("memory.origin", json!("engine")),
                ("memory.sequence_domain", json!("filtered_store")),
            ] {
                attributes.insert(k.into(), v);
            }
            if let Some(flags) = reason_code
                .as_deref()
                .and_then(|s| s.strip_prefix("preferences:"))
                && matches!(flags, "00" | "01" | "10" | "11")
            {
                attributes.insert("memory.pinned".into(), json!(flags.starts_with('1')));
                attributes.insert("memory.suppressed".into(), json!(flags.ends_with('1')));
            }
            if let Some(c) = context {
                attributes.insert("memory.context_id".into(), json!(c));
            }
            if let Some(n) = units {
                attributes.insert("memory.budget_units".into(), json!(n));
            }
            if let Some(u) = unit {
                attributes.insert("memory.budget_unit".into(), json!(u));
            }
            events.push(json!({"schemaVersion":1,"id":format!("memory-{producer}-{seq}"),
                "traceId":trace.unwrap_or_else(||format!("memory-{producer}")),"startTime":time.saturating_mul(1000),"endTime":time.saturating_mul(1000),
                "agentId":"memory-engine","category":"memory","subtype":format!("memory.{action}"),"name":format!("memory.{action}"),"status":"success","attributes":attributes}));
        }
        Ok(EventPage {
            schema: "codewhale.memory.events/v1".into(),
            producer,
            epoch,
            events,
            next_after: cursor,
            has_more: more,
            sequence_domain: "filtered_store".into(),
        })
    }
}

impl Store {
    pub fn numeric_id(&self, access: &Access, id: &str) -> Result<i64> {
        self.get(access, id)?;
        Ok(self.conn.query_row(
            "SELECT id FROM memory_numeric_aliases WHERE memory_id=?1",
            [id],
            |r| r.get(0),
        )?)
    }
    pub fn get_numeric(&self, access: &Access, id: i64) -> Result<Memory> {
        access.require(Capability::Read)?;
        let memory: Option<String> = self
            .conn
            .query_row(
                "SELECT memory_id FROM memory_numeric_aliases WHERE id=?1",
                [id],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        self.get(access, &memory.ok_or(Error::NotFound)?)
    }
    pub fn replacement_of(&self, access: &Access, id: &str) -> Result<Option<Memory>> {
        self.get(access, id)?;
        let replacement: Option<String> = self
            .conn
            .query_row(
                "SELECT new_id FROM replacements WHERE old_id=?1",
                [id],
                |r| r.get(0),
            )
            .optional()?;
        replacement.map(|id| self.get(access, &id)).transpose()
    }
    /// Explicit local-maintenance inventory, not a model-facing cross-repo read.
    /// Requires the user scope plus Maintenance and returns no source text.
    pub fn local_scope_inventory(&self, access: &Access) -> Result<Vec<Scope>> {
        access.require(Capability::Maintenance)?;
        let owner = access
            .scopes()
            .find(|s| s.workspace.is_none() && s.session.is_none())
            .ok_or(Error::Denied)?;
        let mut stmt=self.conn.prepare("SELECT DISTINCT scope FROM memories WHERE json_extract(scope,'$.tenant')=?1 AND json_extract(scope,'$.user')=?2 ORDER BY scope LIMIT 10000")?;
        let rows = stmt
            .query_map(params![owner.tenant, owner.user], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows.into_iter()
            .map(|s| serde_json::from_str(&s).map_err(Into::into))
            .collect()
    }
}
