use std::path::PathBuf;

use codewhale_state::{SessionSource, StateStore, ThreadListFilters, ThreadMetadata, ThreadStatus};
use rusqlite::Connection;

fn temp_state_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "deepseek_state_test_{}_{}_{}.db",
        label,
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    ))
}

fn assert_workflow_trace_schema(conn: &Connection) {
    let user_version: u32 = conn
        .query_row("PRAGMA user_version;", [], |row| row.get(0))
        .expect("read user_version");
    // v5 (goal stall-history migration) adds `thread_goals.last_gap_fingerprint`,
    // `repeated_gap_count`, `last_gap_pass` and `pause_reason` on top of the v4
    // continuation-count column and the v3 workflow-trace + thread_goals tables.
    // v6 adds `thread_runtime_links` (client thread -> runtime thread).
    // v7 adds bound canonical alias/operation receipts on this same connection.
    assert_eq!(user_version, 7);

    for table in [
        "workflow_runs",
        "branch_runs",
        "leaf_runs",
        "control_node_runs",
        "teacher_candidates",
        "thread_goals",
        "thread_runtime_links",
        "state_store_identity",
        "thread_runtime_receipts",
        "thread_runtime_operations",
    ] {
        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
                [table],
                |row| row.get(0),
            )
            .unwrap_or_else(|err| panic!("read sqlite_master for {table}: {err}"));
        assert!(exists, "missing workflow trace table {table}");
    }
}

#[test]
fn upsert_and_resume_thread_metadata() {
    let path = temp_state_path("upsert_resume");
    let store = StateStore::open(Some(path.clone())).expect("open state store");
    let now = chrono::Utc::now().timestamp();
    let thread = ThreadMetadata {
        id: "thread-test-1".to_string(),
        rollout_path: Some(PathBuf::from("/tmp/rollout.jsonl")),
        preview: "hello".to_string(),
        ephemeral: false,
        model_provider: "deepseek".to_string(),
        created_at: now,
        updated_at: now,
        status: ThreadStatus::Running,
        path: Some(PathBuf::from("/tmp/project")),
        cwd: PathBuf::from("/tmp/project"),
        cli_version: "0.0.0-test".to_string(),
        source: SessionSource::Interactive,
        name: Some("Test Thread".to_string()),
        sandbox_policy: Some("workspace-write".to_string()),
        approval_mode: Some("on-request".to_string()),
        archived: false,
        archived_at: None,
        git_sha: None,
        git_branch: None,
        git_origin_url: None,
        memory_mode: Some("extended".to_string()),
        current_leaf_id: None,
    };
    store.upsert_thread(&thread).expect("upsert thread");

    let loaded = store
        .get_thread("thread-test-1")
        .expect("read thread")
        .expect("thread must exist");
    assert_eq!(loaded.id, "thread-test-1");
    assert_eq!(loaded.name.as_deref(), Some("Test Thread"));
    assert_eq!(loaded.memory_mode.as_deref(), Some("extended"));
    assert_eq!(
        loaded.rollout_path,
        Some(PathBuf::from("/tmp/rollout.jsonl"))
    );

    store
        .mark_archived("thread-test-1")
        .expect("archive thread");
    let archived = store
        .get_thread("thread-test-1")
        .expect("read archived thread")
        .expect("thread exists after archive");
    assert!(archived.archived);

    let listed = store
        .list_threads(ThreadListFilters {
            include_archived: true,
            limit: Some(10),
        })
        .expect("list threads");
    assert!(!listed.is_empty());
}

#[test]
fn init_schema_migration() {
    let path = temp_state_path("init_schema_migration");
    let conn = Connection::open(&path).expect("open state db");
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS threads (
            id TEXT PRIMARY KEY,
            rollout_path TEXT,
            preview TEXT NOT NULL,
            ephemeral INTEGER NOT NULL,
            model_provider TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            status TEXT NOT NULL,
            path TEXT,
            cwd TEXT NOT NULL,
            cli_version TEXT NOT NULL,
            source TEXT NOT NULL,
            title TEXT,
            sandbox_policy TEXT,
            approval_mode TEXT,
            archived INTEGER NOT NULL DEFAULT 0,
            archived_at INTEGER,
            git_sha TEXT,
            git_branch TEXT,
            git_origin_url TEXT,
            memory_mode TEXT
        );
        CREATE TABLE IF NOT EXISTS messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            thread_id TEXT NOT NULL,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            item_json TEXT,
            created_at INTEGER NOT NULL,
            FOREIGN KEY(thread_id) REFERENCES threads(id) ON DELETE CASCADE
        );
        INSERT INTO threads (
            id, preview, ephemeral, model_provider, created_at, updated_at, status, cwd, cli_version, source, archived
        )
        VALUES (
            'thread-test-1', 'hello', false, 'deepseek', 0, 0, 'running', '/tmp/project', '0.0.0-test', 'interactive', false
        );
        INSERT INTO messages (thread_id, role, content, created_at) VALUES
        ('thread-test-1', 'foo0', 'bar0', 0),
        ('thread-test-1', 'foo1', 'bar1', 1),
        ('thread-test-1', 'foo2', 'bar2', 2);
        "#,
    )
    .expect("init schema migration");

    let store = StateStore::open(Some(path.clone())).expect("open state store");
    let thread = store
        .get_thread("thread-test-1")
        .expect("read thread")
        .unwrap();
    assert_eq!(thread.id, "thread-test-1");
    assert_eq!(thread.preview, "hello");
    assert!(!thread.ephemeral);
    assert_eq!(thread.model_provider, "deepseek");
    assert_eq!(thread.created_at, 0);
    assert_eq!(thread.updated_at, 0);
    assert_eq!(thread.status, ThreadStatus::Running);
    assert_eq!(thread.cwd, PathBuf::from("/tmp/project"));
    assert_eq!(thread.cli_version, "0.0.0-test");
    assert_eq!(thread.source, SessionSource::Interactive);
    assert!(thread.current_leaf_id.is_some());

    let messages = store
        .list_messages("thread-test-1", None)
        .expect("list messages");
    assert_eq!(messages.len(), 3);
    for (i, message) in messages.iter().enumerate() {
        assert_eq!(message.thread_id, "thread-test-1");
        assert_eq!(message.role, format!("foo{i}"));
        assert_eq!(message.content, format!("bar{i}"));
        assert_eq!(message.created_at, i as i64);
    }

    // Test idempotent
    StateStore::open(Some(path.clone())).expect("open state store");
}

#[test]
fn fresh_schema_includes_workflow_trace_tables() {
    let path = temp_state_path("fresh_schema_includes_workflow_trace_tables");

    StateStore::open(Some(path.clone())).expect("open state store");

    let conn = Connection::open(&path).expect("open state db");
    assert_workflow_trace_schema(&conn);
}

#[test]
fn v1_schema_migrates_workflow_trace_tables() {
    let path = temp_state_path("v1_schema_migrates_workflow_trace_tables");
    let conn = Connection::open(&path).expect("open state db");
    conn.execute_batch(
        r#"
        CREATE TABLE threads (
            id TEXT PRIMARY KEY,
            rollout_path TEXT,
            preview TEXT NOT NULL,
            ephemeral INTEGER NOT NULL,
            model_provider TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            status TEXT NOT NULL,
            path TEXT,
            cwd TEXT NOT NULL,
            cli_version TEXT NOT NULL,
            source TEXT NOT NULL,
            title TEXT,
            sandbox_policy TEXT,
            approval_mode TEXT,
            archived INTEGER NOT NULL DEFAULT 0,
            archived_at INTEGER,
            git_sha TEXT,
            git_branch TEXT,
            git_origin_url TEXT,
            memory_mode TEXT,
            current_leaf_id INTEGER
        );
        CREATE TABLE messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            thread_id TEXT NOT NULL,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            item_json TEXT,
            created_at INTEGER NOT NULL,
            parent_entry_id INTEGER
        );
        CREATE TABLE checkpoints (
            thread_id TEXT NOT NULL,
            checkpoint_id TEXT NOT NULL,
            state_json TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            PRIMARY KEY(thread_id, checkpoint_id)
        );
        CREATE TABLE jobs (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            status TEXT NOT NULL,
            progress INTEGER,
            detail TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );
        CREATE TABLE thread_dynamic_tools (
            thread_id TEXT NOT NULL,
            position INTEGER NOT NULL,
            name TEXT NOT NULL,
            description TEXT,
            input_schema TEXT NOT NULL,
            PRIMARY KEY (thread_id, position)
        );
        INSERT INTO threads (
            id, preview, ephemeral, model_provider, created_at, updated_at, status, cwd, cli_version, source, archived
        )
        VALUES (
            'thread-test-1', 'hello', false, 'deepseek', 0, 0, 'running', '/tmp/project', '0.0.0-test', 'interactive', false
        );
        PRAGMA user_version = 1;
        "#,
    )
    .expect("create v1 schema");
    drop(conn);

    let store = StateStore::open(Some(path.clone())).expect("open state store");
    let thread = store
        .get_thread("thread-test-1")
        .expect("read thread")
        .expect("thread survives migration");
    assert_eq!(thread.preview, "hello");

    let conn = Connection::open(&path).expect("open state db");
    assert_workflow_trace_schema(&conn);
}

#[test]
fn init_schema_migration_same_second_messages() {
    let path = temp_state_path("init_schema_migration_same_second_messages");
    let conn = Connection::open(&path).expect("open state db");
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS threads (
            id TEXT PRIMARY KEY,
            rollout_path TEXT,
            preview TEXT NOT NULL,
            ephemeral INTEGER NOT NULL,
            model_provider TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            status TEXT NOT NULL,
            path TEXT,
            cwd TEXT NOT NULL,
            cli_version TEXT NOT NULL,
            source TEXT NOT NULL,
            title TEXT,
            sandbox_policy TEXT,
            approval_mode TEXT,
            archived INTEGER NOT NULL DEFAULT 0,
            archived_at INTEGER,
            git_sha TEXT,
            git_branch TEXT,
            git_origin_url TEXT,
            memory_mode TEXT
        );
        CREATE TABLE IF NOT EXISTS messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            thread_id TEXT NOT NULL,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            item_json TEXT,
            created_at INTEGER NOT NULL,
            FOREIGN KEY(thread_id) REFERENCES threads(id) ON DELETE CASCADE
        );
        INSERT INTO threads (
            id, preview, ephemeral, model_provider, created_at, updated_at, status, cwd, cli_version, source, archived
        )
        VALUES (
            'thread-test-2', 'hello', false, 'deepseek', 0, 0, 'running', '/tmp/project', '0.0.0-test', 'interactive', false
        );
        INSERT INTO messages (thread_id, role, content, created_at) VALUES
            ('thread-test-2', 'foo0', 'bar0', 123),
            ('thread-test-2', 'foo1', 'bar1', 123),
            ('thread-test-2', 'foo2', 'bar2', 123),
            ('thread-test-2', 'foo3', 'bar3', 123);
        "#,
    )
    .expect("init schema migration");

    let store = StateStore::open(Some(path.clone())).expect("open state store");
    let messages = store
        .list_messages("thread-test-2", None)
        .expect("list messages");
    assert_eq!(messages.len(), 4);
    for (i, message) in messages.iter().enumerate() {
        assert_eq!(message.thread_id, "thread-test-2");
        assert_eq!(message.role, format!("foo{i}"));
        assert_eq!(message.content, format!("bar{i}"));
        assert_eq!(message.created_at, 123);
    }
    assert_eq!(messages[0].parent_entry_id, None);
    assert_eq!(messages[1].parent_entry_id, Some(messages[0].id));
    assert_eq!(messages[2].parent_entry_id, Some(messages[1].id));
    assert_eq!(messages[3].parent_entry_id, Some(messages[2].id));

    // Test idempotent reopen after same-second parent links are migrated.
    StateStore::open(Some(path.clone())).expect("open state store - idempotent");
}

#[test]
fn test_fork() {
    // Historical branching remains readable in every selected projection; live
    // fork/mutation now belongs to the canonical Runtime owner.
    for (leaf, expected) in [
        (5, vec![0, 1, 2, 3, 4]),
        (6, vec![0, 1, 2, 5]),
        (7, vec![0, 1, 2, 3, 4, 6]),
    ] {
        let (_path, store, _, _) = canonical_alias_fixture(
            &format!("archived-fork-{leaf}"),
            (0..7)
                .map(|index| {
                    let parent = match index {
                        0 => None,
                        5 => Some(3),
                        6 => Some(5),
                        _ => Some(index),
                    };
                    legacy_entry(
                        index + 1,
                        parent,
                        &format!("foo{index}"),
                        &format!("bar{index}"),
                    )
                })
                .collect(),
            Some(leaf),
            None,
        );
        let messages = store.list_messages("legacy-import", None).unwrap();
        assert_eq!(messages.len(), expected.len());
        for (message, index) in messages.iter().zip(expected) {
            assert_eq!(message.role, format!("foo{index}"));
            assert_eq!(message.content, format!("bar{index}"));
            assert_eq!(message.thread_id, "legacy-import");
        }
        assert_eq!(store.list_leaf_messages("legacy-import").unwrap().len(), 2);
        let archive = store
            .snapshot_legacy_thread_history("legacy-import")
            .unwrap();
        assert_eq!(
            archive.messages.len(),
            7,
            "all inactive branches are retained"
        );
        assert_eq!(archive.current_leaf_id, Some(leaf));
    }
}

fn legacy_entry(
    id: i64,
    parent_entry_id: Option<i64>,
    role: &str,
    content: &str,
) -> codewhale_state::MessageRecord {
    codewhale_state::MessageRecord {
        id,
        thread_id: "legacy-import".into(),
        role: role.into(),
        content: content.into(),
        item: None,
        created_at: 1_700_000_000 + id,
        parent_entry_id,
    }
}

fn canonical_alias_fixture(
    label: &str,
    messages: Vec<codewhale_state::MessageRecord>,
    current_leaf_id: Option<i64>,
    goal: Option<codewhale_state::ThreadGoalRecord>,
) -> (
    PathBuf,
    StateStore,
    codewhale_protocol::RuntimeOwnerReceipt,
    codewhale_protocol::CanonicalThreadReceipt,
) {
    let path = temp_state_path(label);
    let store = StateStore::open(Some(path.clone())).expect("store");
    let now = chrono::Utc::now().timestamp();
    let thread = ThreadMetadata {
        id: "legacy-import".into(),
        rollout_path: None,
        preview: "import".into(),
        ephemeral: false,
        model_provider: "deepseek".into(),
        created_at: now,
        updated_at: now,
        status: ThreadStatus::Idle,
        path: None,
        cwd: std::env::temp_dir(),
        cli_version: "test".into(),
        source: SessionSource::Resume,
        name: None,
        sandbox_policy: None,
        approval_mode: None,
        archived: false,
        archived_at: None,
        git_sha: None,
        git_branch: None,
        git_origin_url: None,
        memory_mode: None,
        current_leaf_id,
    };
    store
        .restore_legacy_thread_archive(&codewhale_state::LegacyThreadArchive {
            thread,
            messages,
            goal,
            checkpoints: Vec::new(),
        })
        .expect("immutable legacy archive");
    // This is a database-only test of captured receipt comparison, not proof of
    // peer authentication; the actual owner transport is tested in Runtime.
    let owner = codewhale_protocol::RuntimeOwnerReceipt {
        version: 1,
        data_dir: std::env::temp_dir().join("canonical-alias-fixture"),
        execution_scope: "held-scope".into(),
        lease_generation: "held-generation".into(),
        pid: std::process::id(),
        process_start: "fixture-start".into(),
        principal: "fixture-user".into(),
        socket_path: std::env::temp_dir().join("fixture.sock"),
        config_path: None,
    };
    let receipt = codewhale_protocol::CanonicalThreadReceipt {
        version: 1,
        data_dir: owner.data_dir.clone(),
        execution_scope: owner.execution_scope.clone(),
        operation_key: format!("import-{label}"),
        request_digest: "a".repeat(64),
        history_digest: "b".repeat(64),
        runtime_thread_id: "thr_reserved".into(),
        session_id: "session_reserved".into(),
    };
    (path, store, owner, receipt)
}

#[test]
fn canonical_alias_publication_rejects_changed_full_graph_and_leaf() {
    let (path, store, owner, receipt) = canonical_alias_fixture(
        "source_cas",
        vec![
            legacy_entry(1, None, "system", "system context"),
            legacy_entry(2, Some(1), "user", "current branch"),
        ],
        Some(2),
        None,
    );
    let captured = store
        .snapshot_legacy_thread_history("legacy-import")
        .unwrap();
    // A historical external process can still tamper with SQLite. The canonical
    // alias CAS must refuse it; no product append/leaf writer is exposed.
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch("INSERT INTO messages(id,thread_id,role,content,created_at,parent_entry_id) VALUES(3,'legacy-import','user','concurrent alternate branch',1700000003,1); UPDATE threads SET current_leaf_id=3 WHERE id='legacy-import';").unwrap();
    assert!(
        store
            .publish_canonical_runtime_link("legacy-import", None, &captured, &owner, &receipt)
            .is_err()
    );
    assert!(
        store
            .get_canonical_runtime_link("legacy-import", &owner)
            .unwrap()
            .is_none()
    );
    let current = store
        .snapshot_legacy_thread_history("legacy-import")
        .unwrap();
    assert_eq!(
        current.messages.len(),
        3,
        "both source branches survive refusal"
    );
    store
        .publish_canonical_runtime_link("legacy-import", None, &current, &owner, &receipt)
        .unwrap();
    store
        .publish_canonical_runtime_link("legacy-import", None, &current, &owner, &receipt)
        .unwrap();
    assert_eq!(
        store
            .get_canonical_runtime_link("legacy-import", &owner)
            .unwrap(),
        Some(receipt)
    );
    assert!(
        store
            .restore_legacy_thread_archive(&codewhale_state::LegacyThreadArchive {
                thread: store.get_thread("legacy-import").unwrap().unwrap(),
                messages: current.messages,
                goal: None,
                checkpoints: Vec::new(),
            })
            .is_err(),
        "a bound alias cannot be replaced with an archive"
    );
}

#[test]
fn canonical_alias_transaction_rolls_back_and_restart_keeps_exact_owner_binding() {
    let (path, store, owner, receipt) = canonical_alias_fixture(
        "atomic_restart",
        vec![legacy_entry(1, None, "user", "retained original")],
        Some(1),
        None,
    );
    let captured = store
        .snapshot_legacy_thread_history("legacy-import")
        .unwrap();
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch("CREATE TRIGGER fail_canonical_receipt BEFORE INSERT ON thread_runtime_receipts BEGIN SELECT RAISE(ABORT, 'injected publication failure'); END;").unwrap();
    assert!(
        store
            .publish_canonical_runtime_link("legacy-import", None, &captured, &owner, &receipt)
            .is_err()
    );
    for table in [
        "thread_runtime_links",
        "thread_runtime_operations",
        "thread_runtime_receipts",
    ] {
        let count: i64 = conn
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0, "{table} rolls back with the failed transaction");
    }
    conn.execute_batch("DROP TRIGGER fail_canonical_receipt")
        .unwrap();
    store
        .publish_canonical_runtime_link("legacy-import", None, &captured, &owner, &receipt)
        .unwrap();
    drop(store);
    let reopened = StateStore::open(Some(path)).unwrap();
    assert_eq!(
        reopened
            .get_canonical_runtime_link("legacy-import", &owner)
            .unwrap(),
        Some(receipt)
    );
    let mut foreign = owner.clone();
    foreign.execution_scope = "another-store-scope".into();
    assert!(
        reopened
            .get_canonical_runtime_link("legacy-import", &foreign)
            .is_err()
    );
}

#[test]
fn full_legacy_snapshot_keeps_branches_beyond_active_listing_limit() {
    let mut messages = vec![legacy_entry(1, None, "user", "root")];
    messages
        .extend((0..550).map(|index| {
            legacy_entry(index + 2, Some(index + 1), "assistant", &index.to_string())
        }));
    let alternate = 552;
    messages.push(legacy_entry(alternate, Some(1), "user", "alternate"));
    let (_path, store, _, _) =
        canonical_alias_fixture("full_graph", messages, Some(alternate), None);
    let snapshot = store
        .snapshot_legacy_thread_history("legacy-import")
        .unwrap();
    assert_eq!(snapshot.messages.len(), 552);
    assert_eq!(snapshot.current_leaf_id, Some(alternate));
    assert_eq!(store.list_messages("legacy-import", None).unwrap().len(), 2);
}

#[test]
fn canonical_goal_snapshot_and_publication_cas_keep_original_source_status_and_timestamps() {
    let goal = codewhale_state::ThreadGoalRecord {
        thread_id: "legacy-import".into(),
        goal_id: "goal-source-revision".into(),
        objective: "original goal".into(),
        status: codewhale_state::ThreadGoalStatus::Active,
        token_budget: Some(1000),
        tokens_used: 41,
        time_used_seconds: 9,
        continuation_count: 2,
        created_at: 1_700_000_000,
        updated_at: 1_700_000_010,
        last_gap_fingerprint: Some("a".repeat(64)),
        repeated_gap_count: 3,
        last_gap_pass: Some(2),
        pause_reason: None,
    };
    let (path, store, owner, receipt) =
        canonical_alias_fixture("goal_source_cas", Vec::new(), None, Some(goal.clone()));
    let captured = store
        .snapshot_legacy_thread_history("legacy-import")
        .unwrap();
    let saved = captured.goal.as_ref().unwrap();
    assert_eq!(
        saved.status,
        codewhale_protocol::ThreadGoalStatus::Active,
        "source CAS keeps original persisted status rather than target normalization"
    );
    assert_eq!(saved.updated_at, goal.updated_at);
    assert_eq!(
        store
            .get_thread_goal("legacy-import")
            .unwrap()
            .unwrap()
            .status,
        codewhale_state::ThreadGoalStatus::Paused,
        "ordinary restored-reader behavior is unchanged"
    );
    let mut changed = goal;
    changed.objective = "goal changed during owner await".into();
    changed.updated_at += 1;
    let conn = Connection::open(&path).unwrap();
    conn.execute(
        "UPDATE thread_goals SET objective=?1, updated_at=?2 WHERE thread_id='legacy-import'",
        rusqlite::params![changed.objective, changed.updated_at],
    )
    .unwrap();
    assert!(
        store
            .publish_canonical_runtime_link("legacy-import", None, &captured, &owner, &receipt)
            .is_err()
    );
    assert!(
        store
            .get_canonical_runtime_link("legacy-import", &owner)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        store
            .get_thread_goal("legacy-import")
            .unwrap()
            .unwrap()
            .objective,
        changed.objective
    );
}
