use super::tests::{make_snapshot, make_worker_spec, run_incomplete_response_worker, stub_runtime};
use super::*;
use std::sync::atomic::Ordering;
use tempfile::tempdir;

fn record(
    manager: &mut SubAgentManager,
    id: &str,
    parent: Option<&str>,
    cap: Option<u64>,
    spent: u64,
) {
    let mut spec = make_worker_spec(id, manager.workspace.clone());
    spec.parent_run_id = parent.map(str::to_string);
    spec.runtime_profile.token_budget = cap;
    manager.register_worker(spec);
    manager
        .worker_records
        .get_mut(id)
        .unwrap()
        .usage
        .total_tokens = Some(spent);
}

fn continuation(manager: &mut SubAgentManager, id: &str, source: &str) {
    let record = manager.worker_records.get_mut(id).unwrap();
    record.spec.launch_manifest = Some(
        serde_json::from_value(json!({
            "owner_session": "root", "child_id": id,
            "profile": record.spec.runtime_profile,
            "prompt": "continue", "cwd": null, "worktree": false,
            "writable_roots": [], "writable_files": [], "coordination_contracts": [],
            "token_budget": record.spec.runtime_profile.token_budget,
            "resume_identity": id, "generation": 1, "resume_from_agent_id": source
        }))
        .expect("continuation manifest"),
    );
}

#[test]
fn depth_one_child_cannot_spawn_a_grandchild_even_with_a_wider_profile() {
    let root = stub_runtime().with_max_spawn_depth(1);
    let mut child = root.child_runtime();
    child.worker_profile = worker_profile_for_spawn(
        &child,
        &FleetRole::Worker,
        &AgentWorkerToolProfile::Inherited,
        "deepseek-v4-flash",
        None,
        false,
    );
    assert_eq!(child.spawn_depth, 1);
    assert_eq!(child.max_spawn_depth, 1);
    assert_eq!(child.worker_profile.max_spawn_depth, 1);
    assert_eq!(child.worker_profile.spawn_depth, 1);
    assert!(child.would_exceed_depth());
    assert!(!child.worker_profile.can_spawn_child());
    child.worker_profile.max_spawn_depth = u32::MAX;
    assert!(
        child.would_exceed_depth(),
        "a widened projection cannot bypass runtime ceiling"
    );
    assert!(child.background_runtime().would_exceed_depth());
}

#[test]
fn depth_overflow_fails_closed() {
    let mut runtime = stub_runtime();
    runtime.spawn_depth = u32::MAX;
    runtime.max_spawn_depth = u32::MAX;
    assert!(runtime.would_exceed_depth());
    assert_eq!(runtime.child_runtime().spawn_depth, u32::MAX);
}

#[test]
fn old_relative_profile_depth_does_not_gain_authority_on_recovery() {
    let tmp = tempdir().unwrap();
    let mut spec = make_worker_spec("legacy", tmp.path().to_path_buf());
    spec.spawn_depth = 2;
    spec.max_spawn_depth = 3;
    spec.runtime_profile.max_spawn_depth = 1; // old remaining allowance
    let record = AgentWorkerRecord::new(spec, epoch_millis_now());
    let mut encoded = serde_json::to_value(&record).unwrap();
    encoded["spec"]["runtime_profile"]
        .as_object_mut()
        .unwrap()
        .remove("spawn_depth");
    let decoded: AgentWorkerRecord = serde_json::from_value(encoded).unwrap();
    let recovered = normalize_worker_record(decoded);
    assert_eq!(recovered.spec.runtime_profile.spawn_depth, 2);
    assert_eq!(recovered.spec.max_spawn_depth, 1);
    assert_eq!(recovered.spec.runtime_profile.max_spawn_depth, 1);
    assert!(!recovered.spec.runtime_profile.can_spawn_child());
}

#[test]
fn operator_and_inherited_budgets_only_narrow_including_zero_sentinels() {
    assert_eq!(resolve_max_steps(FleetRole::Worker, Some(99), Some(7)), 7);
    assert_eq!(resolve_max_steps(FleetRole::Worker, Some(0), Some(7)), 7);
    let parent = WorkerRuntimeProfile {
        max_steps: 8,
        token_budget: Some(100),
        wall_time_secs: Some(40),
        wall_deadline_ms: Some(123_000),
        ..WorkerRuntimeProfile::default()
    };
    let requested = WorkerRuntimeProfile {
        token_budget: Some(1_000),
        wall_time_secs: Some(4_000),
        wall_deadline_ms: Some(456_000),
        ..WorkerRuntimeProfile::default()
    };
    let child = parent.derive_child(&requested);
    assert_eq!(child.max_steps, 8);
    assert_eq!(child.token_budget, Some(100));
    assert_eq!(child.wall_time_secs, Some(40));
    assert_eq!(child.wall_deadline_ms, Some(123_000));
}

#[test]
fn per_call_budget_fields_reject_empty_zero_null_negative_and_oversized_values() {
    for field in ["token_budget", "max_steps", "wall_time_secs"] {
        for invalid in [
            json!(0),
            json!(-1),
            json!(null),
            json!(""),
            json!("7"),
            json!(false),
            json!(1.5),
        ] {
            let mut input = json!({"prompt": "inspect"});
            input[field] = invalid;
            assert!(parse_spawn_request(&input).is_err(), "{input}");
        }
    }
    for (field, value) in [
        ("max_steps", u64::from(MAX_SUBAGENT_STEPS) + 1),
        ("wall_time_secs", MAX_CHILD_WALL_TIME.as_secs() + 1),
    ] {
        let mut input = json!({"prompt": "inspect"});
        input[field] = json!(value);
        assert!(parse_spawn_request(&input).is_err(), "{input}");
    }
    let request =
        parse_spawn_request(&json!({"prompt": "inspect", "token_budget": 100, "max_tokens": 9}))
            .unwrap();
    assert_eq!(
        request.token_budget,
        Some(9),
        "aliases cannot erase a tighter supplied cap"
    );
}

#[test]
fn explicit_child_budget_keeps_parent_pool_and_default_is_a_ceiling() {
    let tmp = tempdir().unwrap();
    let mut manager =
        SubAgentManager::new(tmp.path().to_path_buf(), 4).with_default_token_budget(Some(100));
    record(&mut manager, "parent", None, Some(100), 25);
    manager.attach_shared_budget_scope("parent", "pool", 100);
    let scope = manager
        .resolve_spawn_budget_scope("child", Some("parent"), Some(999))
        .unwrap()
        .unwrap();
    assert_eq!(scope.scope_id, "pool");
    assert_eq!((scope.limit, scope.spent, scope.remaining), (100, 25, 75));
    let root = manager
        .resolve_spawn_budget_scope("new_root", None, Some(999))
        .unwrap()
        .unwrap();
    assert_eq!(root.limit, 100);
}

#[test]
fn descendant_and_resume_usage_consume_each_ancestor_once_at_equality() {
    let tmp = tempdir().unwrap();
    let mut manager = SubAgentManager::new(tmp.path().to_path_buf(), 8);
    record(&mut manager, "root", None, Some(100), 10);
    manager.attach_shared_budget_scope("root", "pool", 100);
    record(&mut manager, "child", Some("root"), Some(30), 10);
    manager.attach_shared_budget_scope("child", "pool", 100);
    record(&mut manager, "grandchild", Some("child"), None, 10);
    manager.attach_shared_budget_scope("grandchild", "pool", 100);
    record(&mut manager, "resume", Some("root"), Some(20), 10);
    continuation(&mut manager, "resume", "child");
    manager.attach_shared_budget_scope("resume", "pool", 100);
    assert_eq!(manager.subtree_budget_spent("child"), 30);
    assert_eq!(manager.aggregate_budget_spent("pool"), 40);
    assert_eq!(manager.remaining_worker_tokens("resume"), Some(0));
    assert!(
        manager
            .token_budget_exhausted_detail("resume")
            .unwrap()
            .contains("30/30")
    );
    let encoded = serde_json::to_vec(&manager.worker_records).unwrap();
    let restored: HashMap<String, AgentWorkerRecord> = serde_json::from_slice(&encoded).unwrap();
    manager.worker_records = restored
        .into_iter()
        .map(|(id, record)| (id, normalize_worker_record(record)))
        .collect();
    assert_eq!(
        manager.remaining_worker_tokens("resume"),
        Some(0),
        "reload never refunds earlier usage"
    );
}

#[test]
fn a_fork_cannot_move_usage_out_of_either_source_or_current_parent_pool() {
    let tmp = tempdir().unwrap();
    let mut manager = SubAgentManager::new(tmp.path().to_path_buf(), 8);
    record(&mut manager, "source", None, None, 10);
    manager.attach_shared_budget_scope("source", "source_pool", 40);
    record(&mut manager, "parent", None, None, 5);
    manager.attach_shared_budget_scope("parent", "parent_pool", 100);
    record(&mut manager, "fork", Some("parent"), None, 30);
    continuation(&mut manager, "fork", "source");
    manager.attach_shared_budget_scope("fork", "source_pool", 40);
    assert_eq!(manager.aggregate_budget_spent("source_pool"), 40);
    assert_eq!(manager.aggregate_budget_spent("parent_pool"), 35);
    assert_eq!(manager.remaining_worker_tokens("fork"), Some(0));
}

#[test]
fn malformed_budget_lineage_cycles_are_finite_and_do_not_double_count() {
    let tmp = tempdir().unwrap();
    let mut manager = SubAgentManager::new(tmp.path().to_path_buf(), 4);
    record(&mut manager, "a", Some("b"), Some(3), 1);
    record(&mut manager, "b", Some("a"), Some(3), 2);
    assert_eq!(manager.subtree_budget_spent("a"), 3);
    assert_eq!(manager.remaining_worker_tokens("b"), Some(0));
}

#[test]
fn cleanup_retains_completed_budget_evidence_while_a_pool_member_runs() {
    let tmp = tempdir().unwrap();
    let mut manager = SubAgentManager::new(tmp.path().to_path_buf(), 4);
    record(&mut manager, "old", None, Some(100), 90);
    manager.attach_shared_budget_scope("old", "pool", 100);
    record(&mut manager, "live", None, None, 10);
    manager.attach_shared_budget_scope("live", "pool", 100);
    let old = manager.worker_records.get_mut("old").unwrap();
    old.status = AgentWorkerStatus::Completed;
    old.completed_at_ms = Some(0);
    old.updated_at_ms = 0;
    manager.cleanup(Duration::ZERO);
    assert!(manager.worker_records.contains_key("old"));
    assert_eq!(manager.aggregate_budget_spent("pool"), 100);
    assert_eq!(manager.remaining_worker_tokens("live"), Some(0));
}

#[test]
fn budget_partial_handback_is_bounded_and_keeps_unknown_usage_honest() {
    let mut snapshot = make_snapshot(SubAgentStatus::Running);
    snapshot.result = Some("partial 🐳 ".repeat(2_000));
    let result = budget_partial_result(snapshot, "child wall-time budget exhausted");
    assert_eq!(result.status, SubAgentStatus::BudgetExhausted);
    let summary = result.result.unwrap();
    assert!(summary.chars().count() < 4_500);
    assert!(summary.contains("usage has not been reported"));
    assert!(summary.contains("makes no further model request"));
    let checkpoint = result.checkpoint.unwrap();
    assert!(!checkpoint.continuable);
    assert_eq!(
        subagent_failure_class(&result.status, &checkpoint.reason),
        "wall_time_budget"
    );
}

#[tokio::test]
async fn token_equality_stops_before_tools_and_returns_measured_partial_work() {
    let tmp = tempdir().unwrap();
    let (result, calls, mailbox, total_tokens) =
        run_incomplete_response_worker(tmp.path(), "max_tokens", 8, Some(15)).await;
    assert_eq!(result.status, SubAgentStatus::BudgetExhausted);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "no post-budget summary request"
    );
    assert_eq!(total_tokens, Some(15));
    assert!(
        !mailbox
            .iter()
            .any(|message| matches!(message, MailboxMessage::ToolCallStarted { .. }))
    );
    let partial = result.result.unwrap();
    assert!(partial.contains("15/15"));
    assert!(partial.contains("partial response diagnostics"));
    assert!(partial.contains("output was truncated"));
}

#[tokio::test]
async fn launch_narrows_all_limits_and_continuation_cannot_restart_deadline() {
    let tmp = tempdir().unwrap();
    let manager = Arc::new(RwLock::new(
        SubAgentManager::new(tmp.path().to_path_buf(), 4)
            .with_default_token_budget(Some(100))
            .with_default_max_steps(Some(4))
            .with_default_wall_time(Some(Duration::from_secs(10))),
    ));
    let mut runtime = stub_runtime().child_runtime();
    runtime.context = ToolContext::new(tmp.path().to_path_buf());
    runtime.manager = Arc::clone(&manager);
    runtime.cancel_token.cancel(); // inspect admission; no provider request may run
    let options = SubAgentSpawnOptions {
        token_budget: Some(999),
        max_steps: Some(999),
        wall_time: Some(Duration::from_secs(999)),
        ..Default::default()
    };
    let mut guard = manager.write().await;
    let child = guard
        .spawn_background_with_assignment_options(
            Arc::clone(&manager),
            runtime.clone(),
            FleetRole::Scout,
            "inspect".to_string(),
            SubAgentAssignment::new("inspect".to_string(), None),
            Some(vec![]),
            options,
        )
        .unwrap();
    let profile = &guard.worker_records[&child.agent_id].spec.runtime_profile;
    assert_eq!(profile.token_budget, Some(100));
    assert_eq!(profile.max_steps, 4);
    assert!(profile.wall_time_secs.unwrap() <= 10);
    guard
        .worker_records
        .get_mut(&child.agent_id)
        .unwrap()
        .spec
        .runtime_profile
        .wall_deadline_ms = Some(1);
    let refused = guard.spawn_background_with_assignment_options(
        Arc::clone(&manager),
        runtime,
        FleetRole::Scout,
        "continue".to_string(),
        SubAgentAssignment::new("continue".to_string(), None),
        Some(vec![]),
        SubAgentSpawnOptions {
            resume_from_agent_id: Some(child.agent_id),
            ..Default::default()
        },
    );
    assert!(
        refused
            .unwrap_err()
            .to_string()
            .contains("cannot reset its deadline")
    );
}

#[tokio::test]
async fn resume_intersects_saved_write_shell_and_tool_permissions_with_current_caller() {
    let tmp = tempdir().unwrap();
    let manager = Arc::new(RwLock::new(SubAgentManager::new(
        tmp.path().to_path_buf(),
        4,
    )));
    let mut runtime = stub_runtime().child_runtime();
    runtime.context = ToolContext::new(tmp.path().to_path_buf());
    runtime.manager = Arc::clone(&manager);
    runtime.cancel_token.cancel();
    runtime.worker_profile.permissions.write = false;
    runtime.worker_profile.permissions.network = false;
    runtime.worker_profile.shell = ShellPolicy::ReadOnly;
    runtime.worker_profile.tools = ToolScope::Explicit(vec!["read_file".to_string()]);
    runtime.worker_profile.denied_tools = vec!["exec_shell".to_string()];
    let saved = WorkerRuntimeProfile::for_role(FleetRole::Worker);
    let mut guard = manager.write().await;
    let child = guard
        .spawn_background_with_assignment_options(
            Arc::clone(&manager),
            runtime,
            FleetRole::Worker,
            "resume".to_string(),
            SubAgentAssignment::new("resume".to_string(), None),
            None,
            SubAgentSpawnOptions {
                preserve_runtime_profile: Some(saved),
                ..Default::default()
            },
        )
        .unwrap();
    let profile = &guard.worker_records[&child.agent_id].spec.runtime_profile;
    assert!(!profile.permissions.write);
    assert!(!profile.permissions.network);
    assert_eq!(profile.shell, ShellPolicy::ReadOnly);
    assert_eq!(
        profile.tools,
        ToolScope::Explicit(vec!["read_file".to_string()])
    );
    assert!(profile.denied_tools.contains(&"exec_shell".to_string()));
}

#[tokio::test]
async fn measured_budget_caps_actual_wire_output_and_accounts_overshoot_without_retry() {
    use axum::{Json, Router, routing::post};
    let tmp = tempdir().unwrap();
    let requests = Arc::new(std::sync::Mutex::new(Vec::<Value>::new()));
    let observed = Arc::clone(&requests);
    let app = Router::new().route("/v1/chat/completions", post(move |Json(request): Json<Value>| {
        let observed = Arc::clone(&observed);
        async move {
            observed.lock().unwrap().push(request);
            Json(json!({
                "id": "budget-probe", "model": "deepseek-v4-flash",
                "choices": [{"index": 0, "message": {"role": "assistant", "content": "partial evidence"}, "finish_reason": "stop"}],
                "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
            }))
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let config = crate::config::Config {
        api_key: Some("fixture-key".to_string()),
        base_url: Some(format!("http://{address}/v1")),
        ..Default::default()
    };
    let mut runtime = stub_runtime();
    runtime.client = DeepSeekClient::new(&config).unwrap();
    runtime.context = ToolContext::new(tmp.path().to_path_buf());
    runtime.manager = Arc::new(RwLock::new(SubAgentManager::new(
        tmp.path().to_path_buf(),
        4,
    )));
    record(
        &mut *runtime.manager.write().await,
        "probe",
        None,
        Some(7),
        0,
    );
    let (_tx, rx) = mpsc::unbounded_channel();
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        run_subagent(
            &runtime,
            "probe".to_string(),
            FleetRole::Scout,
            "report".to_string(),
            SubAgentAssignment::new("report".to_string(), None),
            Some(vec![]),
            false,
            Instant::now(),
            8,
            Some(7),
            None,
            rx,
        ),
    )
    .await
    .unwrap()
    .unwrap();
    server.abort();
    assert_eq!(result.status, SubAgentStatus::BudgetExhausted);
    assert_eq!(
        result.usage.as_ref().and_then(|usage| usage.total_tokens),
        Some(15)
    );
    let requests = requests.lock().unwrap();
    assert_eq!(
        requests.len(),
        1,
        "no summary retry after budget exhaustion"
    );
    assert_eq!(
        requests[0]
            .get("max_tokens")
            .or_else(|| requests[0].get("max_completion_tokens"))
            .and_then(Value::as_u64),
        Some(7)
    );
    assert!(result.result.as_deref().unwrap().contains("15/7"));
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn workflow_scope_precedes_child_admission_and_caps_the_first_request_after_a_yield() {
    use axum::{Json, Router, routing::post};
    let _retry = crate::retry_status::test_guard();
    crate::retry_status::clear();
    crate::retry_status::clear_rate_limit();
    let _env = crate::test_support::lock_test_env();
    let tmp = tempdir().unwrap();
    let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", tmp.path().join("state"));
    let requests = Arc::new(std::sync::Mutex::new(Vec::<Value>::new()));
    let observed = Arc::clone(&requests);
    let app = Router::new().route("/{*path}", post(move |Json(request): Json<Value>| {
        let observed = Arc::clone(&observed);
        async move {
            observed.lock().unwrap().push(request);
            Json(json!({
                "id": "workflow-budget-probe", "model": "deepseek-v4-flash",
                "choices": [{"index": 0, "message": {"role": "assistant", "content": "partial evidence"}, "finish_reason": "stop"}],
                "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
            }))
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let config = crate::config::Config {
        api_key: Some("fixture-key".to_string()),
        base_url: Some(format!("http://{address}/v1")),
        ..Default::default()
    };
    let manager = Arc::new(RwLock::new(
        SubAgentManager::new(tmp.path().to_path_buf(), 4).with_launch_concurrency(1),
    ));
    let mut runtime = stub_runtime().with_api_config(config.clone());
    runtime.client = DeepSeekClient::new(&config).unwrap();
    runtime.context = ToolContext::new(tmp.path().to_path_buf());
    runtime.manager = Arc::clone(&manager);
    let gate = manager.read().await.launch_gate.clone();
    let permit = gate.acquire_owned().await.unwrap();
    let request: codewhale_workflow_js::TaskRequest = serde_json::from_value(json!({
        "description": "report", "subagent_type": "scout", "allowed_tools": [],
        "worktree": false, "token_budget": 100,
    }))
    .unwrap();
    let identity = WorkflowTaskSpawnIdentity {
        workflow_run_id: "run-first-request-budget".to_string(),
        shared_token_budget: Some(12),
        workflow_phase_id: None,
        workflow_task_label: None,
        workflow_child_index: 0,
        fleet_authority_fingerprint: None,
        exact_fleet_binding: None,
    };
    let spawned = spawn_workflow_task(
        request.clone(),
        Arc::clone(&manager),
        runtime.clone(),
        identity.clone(),
    )
    .await
    .unwrap();
    // Force the already-scheduled child to poll while admission is held. No
    // caller-side attachment is performed, even after this scheduling boundary.
    tokio::task::yield_now().await;
    let handle = {
        let mut guard = manager.write().await;
        assert_eq!(
            guard.budget_scope_state(&spawned.result.agent_id),
            Some((0, 12))
        );
        assert_eq!(
            guard.remaining_worker_tokens(&spawned.result.agent_id),
            Some(12)
        );
        assert!(requests.lock().unwrap().is_empty());
        guard
            .agents
            .get_mut(&spawned.result.agent_id)
            .unwrap()
            .task_handle
            .take()
            .unwrap()
    };
    drop(permit);
    tokio::task::yield_now().await;
    tokio::time::timeout(Duration::from_secs(5), handle)
        .await
        .expect("budgeted child settles")
        .expect("child task succeeds");
    let count = manager.read().await.worker_records.len();
    let error = spawn_workflow_task(request, Arc::clone(&manager), runtime, identity)
        .await
        .expect_err("an exhausted host pool cannot admit another child");
    assert!(error.to_string().contains("15/12 tokens spent"), "{error}");
    let guard = manager.read().await;
    assert_eq!(
        guard.worker_records.len(),
        count,
        "refusal must precede registration"
    );
    assert_eq!(guard.budget_spent_for_scope("run-first-request-budget"), 15);
    assert_eq!(
        guard.get_result(&spawned.result.agent_id).unwrap().status,
        SubAgentStatus::BudgetExhausted
    );
    server.abort();
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 1, "no request after shared exhaustion");
    assert_eq!(
        requests[0]
            .get("max_tokens")
            .or_else(|| requests[0].get("max_completion_tokens"))
            .and_then(Value::as_u64),
        Some(12)
    );
}

#[tokio::test]
async fn workflow_scope_preserves_parent_source_and_per_call_lower_ceilings() {
    for (parent_cap, source_cap, call_cap, expected) in
        [(17, 80, 90, 10), (80, 16, 90, 11), (80, 80, 9, 9)]
    {
        let tmp = tempdir().unwrap();
        let manager = Arc::new(RwLock::new(SubAgentManager::new(
            tmp.path().to_path_buf(),
            4,
        )));
        let mut runtime = stub_runtime().child_runtime();
        runtime.context = ToolContext::new(tmp.path().to_path_buf());
        runtime.manager = Arc::clone(&manager);
        runtime.parent_agent_id = Some("parent".to_string());
        runtime.cancel_token.cancel(); // inspect admission without a provider request
        let mut guard = manager.write().await;
        record(&mut guard, "parent", None, Some(parent_cap), 7);
        guard.attach_shared_budget_scope("parent", "parent-pool", parent_cap);
        record(&mut guard, "source", None, Some(source_cap), 5);
        guard.attach_shared_budget_scope("source", "source-pool", source_cap);
        let child = guard
            .spawn_background_with_assignment_options(
                Arc::clone(&manager),
                runtime,
                FleetRole::Scout,
                "continue".to_string(),
                SubAgentAssignment::new("continue".to_string(), None),
                Some(vec![]),
                SubAgentSpawnOptions {
                    token_budget: Some(call_cap),
                    workflow_budget_scope: Some(("workflow-pool".to_string(), 100)),
                    resume_from_agent_id: Some("source".to_string()),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(guard.budget_scope_state(&child.agent_id), Some((0, 100)));
        assert_eq!(
            guard.remaining_worker_tokens(&child.agent_id),
            Some(expected)
        );
        guard
            .worker_records
            .get_mut(&child.agent_id)
            .unwrap()
            .usage
            .total_tokens = Some(expected);
        assert_eq!(guard.remaining_worker_tokens(&child.agent_id), Some(0));
        assert_eq!(guard.budget_spent_for_scope("parent-pool"), 7 + expected);
        assert_eq!(guard.budget_spent_for_scope("source-pool"), 5 + expected);
        assert_eq!(guard.budget_spent_for_scope("workflow-pool"), expected);
    }
}

#[tokio::test]
async fn root_fork_of_depth_two_leaf_cannot_regain_a_generation() {
    let tmp = tempdir().unwrap();
    let manager = Arc::new(RwLock::new(SubAgentManager::new(
        tmp.path().to_path_buf(),
        4,
    )));
    let mut runtime = stub_runtime().with_max_spawn_depth(3).child_runtime();
    runtime.context = ToolContext::new(tmp.path().to_path_buf());
    runtime.manager = Arc::clone(&manager);
    runtime.cancel_token.cancel();
    let mut guard = manager.write().await;
    let mut source = make_worker_spec("leaf", tmp.path().to_path_buf());
    source.spawn_depth = 2;
    source.max_spawn_depth = 2;
    source.runtime_profile.spawn_depth = 2;
    source.runtime_profile.max_spawn_depth = 2;
    guard.register_worker(source);
    let child = guard
        .spawn_background_with_assignment_options(
            Arc::clone(&manager),
            runtime,
            FleetRole::Scout,
            "fork leaf".to_string(),
            SubAgentAssignment::new("fork leaf".to_string(), None),
            Some(vec![]),
            SubAgentSpawnOptions {
                resume_from_agent_id: Some("leaf".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
    let spec = &guard.worker_records[&child.agent_id].spec;
    assert_eq!(spec.spawn_depth, 2);
    assert_eq!(spec.max_spawn_depth, 2);
    assert_eq!(spec.runtime_profile.spawn_depth, 2);
    assert!(!spec.runtime_profile.can_spawn_child());
}
