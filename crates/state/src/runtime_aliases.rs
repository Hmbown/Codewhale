//! Compatibility receipts on the existing StateStore connection. These records
//! describe committed canonical work; they never execute or own a conversation.
use super::*;
use codewhale_protocol::{
    CanonicalThreadReceipt, LegacyThreadHistory, MAX_CANONICAL_HISTORY_BYTES,
    MAX_CANONICAL_HISTORY_ENTRIES, RuntimeOwnerReceipt,
};

impl StateStore {
    /// Restore an immutable historical archive into an absent SQLite thread.
    /// Validation and every row write share one transaction. Existing targets,
    /// canonical aliases and partial/cyclic/oversized archives are refused.
    pub fn restore_legacy_thread_archive(&self, archive: &LegacyThreadArchive) -> Result<()> {
        validate_legacy_archive(archive)?;
        let mut conn = self.conn()?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let occupied: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM threads WHERE id = ?1) OR EXISTS(SELECT 1 FROM thread_runtime_links WHERE thread_id = ?1) OR EXISTS(SELECT 1 FROM thread_runtime_receipts WHERE thread_id = ?1)",
            params![archive.thread.id], |row| row.get(0),
        )?;
        anyhow::ensure!(
            !occupied,
            "legacy archive target is already present; no replacement or append is permitted"
        );
        write_thread_metadata_on(&tx, &archive.thread)?;
        for message in &archive.messages {
            tx.execute(
                "INSERT INTO messages(id, thread_id, role, content, item_json, created_at, parent_entry_id) VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![message.id, message.thread_id, message.role, message.content,
                    message.item.as_ref().map(serde_json::to_string).transpose()?, message.created_at, message.parent_entry_id],
            )?;
        }
        tx.execute(
            "UPDATE threads SET current_leaf_id = ?1 WHERE id = ?2",
            params![archive.thread.current_leaf_id, archive.thread.id],
        )?;
        if let Some(goal) = &archive.goal {
            write_thread_goal_on(&tx, goal)?;
        }
        for checkpoint in &archive.checkpoints {
            tx.execute("INSERT INTO checkpoints(thread_id, checkpoint_id, state_json, created_at) VALUES(?1,?2,?3,?4)", params![checkpoint.thread_id, checkpoint.checkpoint_id, serde_json::to_string(&checkpoint.state)?, checkpoint.created_at])?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Read all branches and the active leaf in one SQLite read transaction.
    /// Bounds are checked against SQLite lengths before retaining payloads.
    pub fn snapshot_legacy_thread_history(&self, thread_id: &str) -> Result<LegacyThreadHistory> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let snapshot = snapshot_history_on(&tx, thread_id)?;
        tx.commit()?;
        Ok(snapshot)
    }

    pub fn get_canonical_runtime_link(
        &self,
        thread_id: &str,
        owner: &RuntimeOwnerReceipt,
    ) -> Result<Option<CanonicalThreadReceipt>> {
        let conn = self.conn()?;
        let record: Option<(String, Option<String>)> = conn.query_row(
            "SELECT r.receipt_json, l.runtime_thread_id FROM thread_runtime_receipts r LEFT JOIN thread_runtime_links l ON l.thread_id = r.thread_id WHERE r.thread_id = ?1",
            params![thread_id], |row| Ok((row.get(0)?, row.get(1)?)),
        ).optional()?;
        let Some((encoded, linked)) = record else {
            return Ok(None);
        };
        let receipt: CanonicalThreadReceipt = serde_json::from_str(&encoded)?;
        validate_canonical_receipt(&receipt)?;
        anyhow::ensure!(
            Some(receipt.runtime_thread_id.as_str()) == linked.as_deref(),
            "legacy link changed behind its bound canonical receipt; recovery required"
        );
        anyhow::ensure!(
            receipt.data_dir == owner.data_dir && receipt.execution_scope == owner.execution_scope,
            "thread belongs to a different canonical Runtime store; owner attachment required"
        );
        Ok(Some(receipt))
    }

    /// Publish only an already committed owner receipt, comparing the previous
    /// link under the same write transaction. Cancellation of the outer waiter
    /// cannot roll back a committed canonical operation into a remint.
    pub fn publish_canonical_runtime_link(
        &self,
        thread_id: &str,
        expected_previous: Option<&str>,
        expected_history: &LegacyThreadHistory,
        owner: &RuntimeOwnerReceipt,
        receipt: &CanonicalThreadReceipt,
    ) -> Result<()> {
        validate_canonical_receipt(receipt)?;
        anyhow::ensure!(
            receipt.data_dir == owner.data_dir && receipt.execution_scope == owner.execution_scope,
            "canonical receipt does not match the authenticated owner"
        );
        let encoded = serde_json::to_string(receipt)?;
        let mut conn = self.conn()?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        anyhow::ensure!(
            expected_history.thread_id == thread_id
                && serde_json::to_value(snapshot_history_on(&tx, thread_id)?)?
                    == serde_json::to_value(expected_history)?,
            "legacy history changed before canonical publication; source and canonical result retained for recovery"
        );
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM threads WHERE id = ?1)",
            params![thread_id],
            |row| row.get(0),
        )?;
        anyhow::ensure!(
            exists,
            "cannot publish canonical link for an absent compatibility thread"
        );
        let previous: Option<String> = tx
            .query_row(
                "SELECT runtime_thread_id FROM thread_runtime_links WHERE thread_id = ?1",
                params![thread_id],
                |row| row.get(0),
            )
            .optional()?;
        let saved: Option<String> = tx
            .query_row(
                "SELECT receipt_json FROM thread_runtime_receipts WHERE thread_id = ?1",
                params![thread_id],
                |row| row.get(0),
            )
            .optional()?;
        if saved.as_deref() == Some(&encoded)
            && previous.as_deref() == Some(&receipt.runtime_thread_id)
        {
            tx.commit()?;
            return Ok(());
        }
        anyhow::ensure!(
            saved.is_none(),
            "canonical alias is already committed to another operation; recovery required"
        );
        anyhow::ensure!(
            previous.as_deref() == expected_previous,
            "compatibility link changed before canonical publication"
        );
        let operation: Option<(String, String)> = tx.query_row("SELECT request_digest, receipt_json FROM thread_runtime_operations WHERE operation_key = ?1", params![receipt.operation_key], |row| Ok((row.get(0)?, row.get(1)?))).optional()?;
        if let Some((digest, saved)) = operation {
            anyhow::ensure!(
                digest == receipt.request_digest && saved == encoded,
                "canonical operation key was used for different data"
            );
        }
        tx.execute("INSERT OR IGNORE INTO thread_runtime_operations(operation_key, request_digest, receipt_json) VALUES(?1, ?2, ?3)", params![receipt.operation_key, receipt.request_digest, encoded])?;
        tx.execute("INSERT INTO thread_runtime_links(thread_id, runtime_thread_id, created_at) VALUES(?1, ?2, ?3) ON CONFLICT(thread_id) DO UPDATE SET runtime_thread_id = excluded.runtime_thread_id, created_at = excluded.created_at", params![thread_id, receipt.runtime_thread_id, Utc::now().timestamp()])?;
        tx.execute(
            "INSERT INTO thread_runtime_receipts(thread_id, receipt_json) VALUES(?1, ?2)",
            params![thread_id, encoded],
        )?;
        tx.commit()?;
        Ok(())
    }
}

fn validate_canonical_receipt(receipt: &CanonicalThreadReceipt) -> Result<()> {
    anyhow::ensure!(
        receipt.version == 1
            && receipt.data_dir.is_absolute()
            && !receipt.execution_scope.is_empty()
            && receipt.execution_scope.len() <= 128,
        "invalid canonical store receipt"
    );
    anyhow::ensure!(
        !receipt.operation_key.is_empty()
            && receipt.operation_key.len() <= 128
            && !receipt.operation_key.chars().any(char::is_control),
        "invalid canonical operation key"
    );
    for digest in [&receipt.request_digest, &receipt.history_digest] {
        anyhow::ensure!(
            digest.len() == 64
                && digest
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()),
            "invalid canonical operation digest"
        );
    }
    for id in [&receipt.runtime_thread_id, &receipt.session_id] {
        anyhow::ensure!(
            !id.is_empty()
                && id.len() <= 128
                && id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')),
            "invalid canonical record identity"
        );
    }
    Ok(())
}

fn snapshot_history_on(conn: &Connection, thread_id: &str) -> Result<LegacyThreadHistory> {
    let leaf = conn.query_row(
        "SELECT current_leaf_id FROM threads WHERE id = ?1",
        params![thread_id],
        |row| row.get::<_, Option<i64>>(0),
    )?;
    let identity: String = conn.query_row(
        "SELECT identity FROM state_store_identity WHERE singleton = 1",
        [],
        |row| row.get(0),
    )?;
    let (count, bytes): (i64, i64) = conn.query_row(
            "SELECT count(*), coalesce(sum(length(cast(role AS BLOB)) + length(cast(content AS BLOB)) + coalesce(length(cast(item_json AS BLOB)), 0) + 128), 0) FROM messages WHERE thread_id = ?1",
            params![thread_id], |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
    anyhow::ensure!(
        count >= 0
            && count as u64 <= MAX_CANONICAL_HISTORY_ENTRIES as u64
            && bytes >= 0
            && bytes as u64 <= MAX_CANONICAL_HISTORY_BYTES as u64,
        "legacy history exceeds canonical import bounds; source retained for recovery"
    );
    let mut messages = Vec::with_capacity(count as usize);
    {
        let mut statement = conn.prepare("SELECT id, role, content, item_json, created_at, parent_entry_id FROM messages WHERE thread_id = ?1 ORDER BY id")?;
        let mut rows = statement.query(params![thread_id])?;
        while let Some(row) = rows.next()? {
            let raw: Option<String> = row.get(3)?;
            messages.push(MessageRecord {
                id: row.get(0)?,
                thread_id: thread_id.into(),
                role: row.get(1)?,
                content: row.get(2)?,
                item: raw
                    .as_deref()
                    .map(serde_json::from_str)
                    .transpose()
                    .context("unsupported legacy JSON; source retained for recovery")?,
                created_at: row.get(4)?,
                parent_entry_id: row.get(5)?,
            });
        }
    }
    let snapshot = LegacyThreadHistory {
        version: 1,
        state_store_id: identity,
        thread_id: thread_id.into(),
        current_leaf_id: leaf,
        messages,
        goal: StateStore::read_thread_goal_snapshot(conn, thread_id)?
            .map(|goal| serde_json::from_value(serde_json::to_value(goal)?))
            .transpose()?,
    };
    anyhow::ensure!(
        serde_json::to_vec(&snapshot)?.len() <= MAX_CANONICAL_HISTORY_BYTES,
        "encoded legacy history exceeds canonical import bounds; source retained for recovery"
    );
    Ok(snapshot)
}

fn validate_legacy_archive(archive: &LegacyThreadArchive) -> Result<()> {
    anyhow::ensure!(
        !archive.thread.id.is_empty()
            && archive.thread.id.len() <= 128
            && !archive.thread.id.chars().any(char::is_control),
        "invalid legacy archive thread identity"
    );
    anyhow::ensure!(
        archive
            .messages
            .len()
            .saturating_add(archive.checkpoints.len())
            <= MAX_CANONICAL_HISTORY_ENTRIES,
        "legacy archive exceeds entry bounds; source retained"
    );
    struct Measure(usize);
    impl std::io::Write for Measure {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if bytes.len() > self.0 {
                return Err(std::io::Error::other("legacy archive exceeds byte bounds"));
            }
            self.0 -= bytes.len();
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    serde_json::to_writer(&mut Measure(MAX_CANONICAL_HISTORY_BYTES), archive)?;
    let mut parents = HashMap::new();
    for message in &archive.messages {
        anyhow::ensure!(
            message.thread_id == archive.thread.id && message.id > 0 && !message.role.is_empty(),
            "invalid or foreign legacy archive entry"
        );
        anyhow::ensure!(
            parents
                .insert(message.id, message.parent_entry_id)
                .is_none(),
            "duplicate legacy archive entry"
        );
    }
    anyhow::ensure!(
        match archive.thread.current_leaf_id {
            Some(leaf) => parents.contains_key(&leaf),
            None => parents.is_empty(),
        },
        "legacy archive has no valid selected leaf"
    );
    let mut completed = std::collections::HashSet::new();
    for &id in parents.keys() {
        let mut path = std::collections::HashSet::new();
        let mut cursor = Some(id);
        while let Some(id) = cursor {
            if completed.contains(&id) {
                break;
            }
            anyhow::ensure!(path.insert(id), "cyclic legacy archive graph");
            cursor = *parents
                .get(&id)
                .ok_or_else(|| anyhow::anyhow!("missing legacy archive parent"))?;
        }
        completed.extend(path);
    }
    if let Some(goal) = &archive.goal {
        anyhow::ensure!(
            goal.thread_id == archive.thread.id
                && !goal.goal_id.is_empty()
                && !goal.objective.trim().is_empty()
                && goal.tokens_used >= 0
                && goal.time_used_seconds >= 0
                && goal.continuation_count >= 0
                && goal.token_budget.is_none_or(|budget| budget >= 0),
            "invalid or foreign legacy archive goal"
        );
        codewhale_protocol::validate_goal_stall_state(
            goal.last_gap_fingerprint.as_deref(),
            goal.repeated_gap_count,
            goal.last_gap_pass,
            u32::try_from(goal.continuation_count).map_err(anyhow::Error::from)?,
        )
        .map_err(anyhow::Error::msg)?;
    }
    let mut checkpoints = std::collections::HashSet::new();
    for checkpoint in &archive.checkpoints {
        anyhow::ensure!(
            checkpoint.thread_id == archive.thread.id
                && !checkpoint.checkpoint_id.is_empty()
                && checkpoints.insert(&checkpoint.checkpoint_id),
            "invalid, foreign or duplicate legacy archive checkpoint"
        );
    }
    Ok(())
}
