use super::tests::{make_worker_spec, stub_runtime};
use super::*;
use axum::{Json, Router, http::StatusCode, response::IntoResponse, routing::post};
use tempfile::{TempDir, tempdir};
use tokio::sync::Notify;

struct Fixture {
    workspace: TempDir,
    manager: SharedSubAgentManager,
    task: Option<JoinHandle<()>>,
    server: JoinHandle<()>,
    requests: Arc<std::sync::Mutex<Vec<Value>>>,
    report_started: Arc<Notify>,
    release_report: Arc<Notify>,
    cancel: CancellationToken,
    resume_runtime: SubAgentRuntime,
    completions: mpsc::UnboundedReceiver<SubAgentCompletion>,
    mailbox: MailboxReceiver,
}

impl Fixture {
    async fn finish(&mut self) -> SubAgentResult {
        tokio::time::timeout(Duration::from_secs(5), self.task.take().unwrap())
            .await
            .expect("bounded worker")
            .expect("worker task");
        self.manager
            .read()
            .await
            .get_result("report-worker")
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
        }
        self.server.abort();
    }
}

async fn fixture(
    mode: &'static str,
    first_tokens: u64,
    cap: Option<u64>,
    max_steps: u32,
) -> Fixture {
    let workspace = tempdir().unwrap();
    fs::write(
        workspace.path().join("README.md"),
        "TOOL_EVIDENCE: checksum validation is still missing.\n",
    )
    .unwrap();
    let requests = Arc::new(std::sync::Mutex::new(Vec::new()));
    let report_started = Arc::new(Notify::new());
    let release_report = Arc::new(Notify::new());
    let app = Router::new().route("/{*path}", post({
        let requests = Arc::clone(&requests);
        let report_started = Arc::clone(&report_started);
        let release_report = Arc::clone(&release_report);
        move |Json(body): Json<Value>| {
            let requests = Arc::clone(&requests);
            let report_started = Arc::clone(&report_started);
            let release_report = Arc::clone(&release_report);
            async move {
                let call = {
                    let mut requests = requests.lock().unwrap();
                    requests.push(body);
                    requests.len()
                };
                if mode == "resume-unknown" && call == 2 {
                    report_started.notify_one();
                    release_report.notified().await;
                }
                let choice = if call == 1 || (mode == "resume-unknown" && call == 3) {
                    json!({"index": 0, "message": {"role": "assistant",
                        "content": if mode == "tool-only" { Value::Null } else { json!("RECORDED_FINDING: checksum validation is missing.") },
                        "tool_calls": [{"id": "read-one", "type": "function", "function": {
                            "name": "read", "arguments": "{\"path\":\"README.md\"}"
                        }}]}, "finish_reason": "tool_calls"})
                } else {
                    report_started.notify_one();
                    if matches!(mode, "hold" | "timeout" | "work-timeout") { release_report.notified().await; }
                    if mode == "failure" {
                        return (StatusCode::BAD_REQUEST, Json(json!({"error": {"message": "fixture rejection"}}))).into_response();
                    }
                    if mode == "tool" {
                        json!({"index": 0, "message": {"role": "assistant", "content": "REJECTED_REPORT: I wrote report.md.",
                            "tool_calls": [{"id": "must-not-write", "type": "function", "function": {
                                "name": "write_file", "arguments": "{\"path\":\"report.md\",\"content\":\"must not execute\"}"
                            }}]}, "finish_reason": "tool_calls"})
                    } else {
                        json!({"index": 0, "message": {"role": "assistant", "content":
                            "PARTIAL_REPORT: README evidence identifies missing checksum validation. No report file was produced. Next: implement and verify the checksum check."}, "finish_reason": "stop"})
                    }
                };
                let usage = if (call == 1 && matches!(mode, "unknown" | "resume-unknown")) || (call > 1 && mode == "report-unknown") { Value::Null }
                    else if call == 1 { json!({"prompt_tokens": first_tokens.saturating_sub(5), "completion_tokens": 5, "total_tokens": first_tokens}) }
                    else if mode == "resume-unknown" { json!({"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}) }
                    else { json!({"prompt_tokens": 20, "completion_tokens": 10, "total_tokens": 30}) };
                Json(json!({"id": format!("handback-{call}"), "model": "deepseek-v4-flash", "choices": [choice], "usage": usage})).into_response()
            }
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
        retry: Some(crate::config::RetryConfig {
            enabled: Some(false),
            max_retries: Some(0),
            initial_delay: Some(0.0),
            max_delay: Some(0.0),
            exponential_base: Some(1.0),
        }),
        ..Default::default()
    };
    let manager = Arc::new(RwLock::new(
        SubAgentManager::new(workspace.path().to_path_buf(), 4)
            .with_state_path(workspace.path().join(".codewhale/subagents/state.json")),
    ));
    let mut spec = make_worker_spec("report-worker", workspace.path().to_path_buf());
    spec.max_steps = max_steps;
    spec.runtime_profile.token_budget = cap;
    spec.runtime_profile.max_steps = max_steps;
    spec.runtime_profile.wall_time_secs = Some(5);
    spec.runtime_profile.wall_deadline_ms = Some(epoch_millis_now() + 5_000);
    if mode == "work-timeout" {
        // A partly consumed original deadline leaves time to persist the
        // missing-coverage receipt after the in-flight call is abandoned.
        spec.runtime_profile.wall_deadline_ms = Some(epoch_millis_now() + 2_000);
    }
    spec.launch_manifest = Some(serde_json::from_value(json!({
        "owner_session": "root", "child_id": "report-worker", "profile": spec.runtime_profile,
        "prompt": "Read README.md and produce report.md", "cwd": workspace.path(), "worktree": false,
        "writable_roots": [], "writable_files": [], "coordination_contracts": [], "deliverables": ["report.md"],
        "resume_identity": null, "generation": 1, "resume_from_agent_id": null
    })).unwrap());
    if mode == "resume-unknown" {
        spec.launch_manifest.as_mut().unwrap().deliverables.clear();
    }
    let mut runtime = stub_runtime();
    runtime.client = DeepSeekClient::new(&config).unwrap();
    runtime.api_config = Some(Arc::new(config));
    runtime.context = ToolContext::new(workspace.path().to_path_buf());
    runtime.accounting_origin = SubAgentAccountingOrigin::capture(&runtime.context);
    runtime.manager = Arc::clone(&manager);
    runtime.worker_profile = spec.runtime_profile.clone();
    runtime.spawn_depth = spec.spawn_depth;
    runtime.allow_shell = false;
    runtime.accept_edits = false;
    runtime.step_api_timeout = if mode == "timeout" {
        Duration::from_millis(100)
    } else {
        Duration::from_secs(2)
    };
    let cancel = runtime.cancel_token.clone();
    let (parent_tx, completions) = mpsc::unbounded_channel();
    runtime.parent_completion_tx = Some(parent_tx);
    let (mailbox, mailbox_rx) = Mailbox::new(CancellationToken::new());
    runtime.mailbox = Some(mailbox);
    let assignment = SubAgentAssignment::new(
        "Read README.md, report findings and identify what remains.".to_string(),
        None,
    );
    let (input_tx, input_rx) = mpsc::unbounded_channel();
    let mut agent = SubAgent::new(
        "report-worker".to_string(),
        FleetRole::Scout,
        assignment.objective.clone(),
        assignment.clone(),
        runtime.model.clone(),
        None,
        Some(vec!["read_file".to_string()]),
        input_tx,
        workspace.path().to_path_buf(),
        manager.read().await.current_session_boot_id.clone(),
    );
    agent.status = SubAgentStatus::Running;
    {
        let mut guard = manager.write().await;
        guard.register_worker_for_session(spec, &runtime.context.state_namespace);
        if let Some(cap) = cap {
            guard.attach_shared_budget_scope("report-worker", "report-pool", cap);
        }
        guard.agents.insert("report-worker".to_string(), agent);
        if mode == "shared-unknown" {
            let mut sibling = make_worker_spec("settled-sibling", workspace.path().to_path_buf());
            sibling.runtime_profile.token_budget = cap;
            guard.register_worker_for_session(sibling, &runtime.context.state_namespace);
            guard.attach_shared_budget_scope("settled-sibling", "report-pool", cap.unwrap());
            guard.record_worker_usage(
                "settled-sibling",
                "missing-sibling",
                &Usage::default(),
                None,
            );
            guard.record_worker_usage(
                "settled-sibling",
                "known-sibling",
                &Usage {
                    input_tokens: 7,
                    output_tokens: 4,
                    ..Usage::default()
                },
                None,
            );
            guard
                .worker_records
                .get_mut("settled-sibling")
                .unwrap()
                .status = AgentWorkerStatus::Completed;
        }
    }
    let resume_runtime = runtime.clone();
    let task = tokio::spawn(run_subagent_task(SubAgentTask {
        manager_handle: Arc::clone(&manager),
        runtime,
        agent_id: "report-worker".to_string(),
        agent_type: FleetRole::Scout,
        prompt: assignment.objective.clone(),
        assignment,
        allowed_tools: Some(vec!["read_file".to_string()]),
        fork_context: false,
        started_at: Instant::now(),
        max_steps,
        token_budget: cap,
        wall_time: Duration::from_secs(5),
        input_rx,
        launch_gate: None,
        _foreground_child_registration: None,
    }));
    Fixture {
        workspace,
        manager,
        task: Some(task),
        server,
        requests,
        report_started,
        release_report,
        cancel,
        resume_runtime,
        completions,
        mailbox: mailbox_rx,
    }
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn budget_handback_turn_consolidates_tool_only_work_and_checks_declared_deliverables() {
    let _retry = crate::retry_status::test_guard();
    crate::retry_status::clear_rate_limit();
    let mut fixture = fixture("tool-only", 15, Some(100_000), 2).await;
    let result = fixture.finish().await;
    assert_eq!(result.status, SubAgentStatus::BudgetExhausted);
    assert_eq!(result.steps_taken, 2);
    assert_eq!(result.usage.as_ref().unwrap().total_tokens, Some(45));
    assert!(result.result.as_deref().unwrap().contains("PARTIAL_REPORT"));
    assert!(
        result
            .checkpoint
            .as_ref()
            .unwrap()
            .messages
            .iter()
            .flat_map(|message| &message.content)
            .any(
                |block| matches!(block, ContentBlock::ToolResult { tool_use_id, content, .. }
                if tool_use_id == "read-one" && content.contains("TOOL_EVIDENCE"))
            ),
        "the read must execute successfully before reporting: {:?}",
        result.checkpoint,
    );
    let requests = fixture.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0]["model"], "deepseek-v4-flash");
    assert!(
        requests[0]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool["function"]["name"] == "read")
    );
    assert_eq!(requests[1]["model"], requests[0]["model"]);
    assert!(requests[1].get("tools").is_none_or(Value::is_null));
    assert!(requests[1].get("tool_choice").is_none_or(Value::is_null));
    assert!(
        requests[1].to_string().contains("TOOL_EVIDENCE"),
        "report must read actual completed tool output"
    );
    assert!(
        requests[1]["max_tokens"]
            .as_u64()
            .or_else(|| requests[1]["max_completion_tokens"].as_u64())
            .unwrap()
            <= 1_024
    );
    drop(requests);
    let guard = fixture.manager.read().await;
    let worker = &guard.worker_records["report-worker"];
    assert_eq!(worker.verification.status, "deliverable_missing");
    assert_eq!(worker.verification.deliverables[0].path, "report.md");
    assert!(
        !worker.spec.runtime_profile.permissions.write,
        "report did not widen the Scout's authority"
    );
    drop(guard);
    let completion = fixture.completions.try_recv().unwrap();
    assert!(completion.payload.contains("budget_exhausted"));
    assert!(completion.payload.contains("deliverable_missing"));
    assert!(fixture.completions.try_recv().is_err());
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn budget_handback_turn_uses_remaining_token_reserve_before_hard_exhaustion() {
    let _retry = crate::retry_status::test_guard();
    crate::retry_status::clear_rate_limit();
    let mut fixture = fixture("success", 92_000, Some(100_000), 8).await;
    let result = fixture.finish().await;
    assert_eq!(result.status, SubAgentStatus::BudgetExhausted);
    assert_eq!(result.usage.as_ref().unwrap().total_tokens, Some(92_030));
    assert_eq!(fixture.requests.lock().unwrap().len(), 2);
    assert!(result.result.as_deref().unwrap().contains("PARTIAL_REPORT"));
    assert!(
        fixture.requests.lock().unwrap()[1]
            .to_string()
            .contains("RECORDED_FINDING")
    );
    assert!(result.checkpoint.as_ref().unwrap().messages.iter().flat_map(|message| &message.content)
        .any(|block| matches!(block, ContentBlock::ToolResult { tool_use_id, content, is_error, .. }
            if tool_use_id == "read-one" && *is_error == Some(true)
                && content.contains("not executed") && content.contains("budget_exhausted")
                && !content.contains("crashed_and_repaired"))));
    assert!(
        !fixture
            .mailbox
            .drain()
            .iter()
            .any(|entry| matches!(entry.message, MailboxMessage::ToolCallStarted { .. })),
        "normal work stops at the reporting reserve"
    );
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn budget_handback_turn_rejects_provider_tools_and_preserves_fallback_verdicts() {
    let _retry = crate::retry_status::test_guard();
    crate::retry_status::clear_rate_limit();
    let mut fixture = fixture("tool", 15, Some(100_000), 2).await;
    let result = fixture.finish().await;
    assert_eq!(fixture.requests.lock().unwrap().len(), 2);
    assert_eq!(result.status, SubAgentStatus::BudgetExhausted);
    assert!(!fixture.workspace.path().join("report.md").exists());
    assert!(
        result
            .result
            .as_deref()
            .unwrap()
            .contains("provider returned a tool call")
    );
    assert!(
        !result
            .result
            .as_deref()
            .unwrap()
            .contains("REJECTED_REPORT")
    );
    assert_eq!(result.usage.as_ref().unwrap().total_tokens, Some(45));
    let history = &result.checkpoint.as_ref().unwrap().messages;
    let completed = history
        .iter()
        .flat_map(|message| &message.content)
        .filter_map(|block| match block {
            ContentBlock::ToolResult { tool_use_id, .. } => Some(tool_use_id.as_str()),
            _ => None,
        })
        .collect::<HashSet<_>>();
    for block in history.iter().flat_map(|message| &message.content) {
        match block {
            ContentBlock::ToolUse { id, .. } => assert!(
                completed.contains(id.as_str()),
                "no orphan tool call may survive for replay"
            ),
            ContentBlock::ServerToolUse { .. } => {
                panic!("a rejected server tool call entered history")
            }
            ContentBlock::Text { text, .. } => assert!(!text.contains("REJECTED_REPORT")),
            _ => {}
        }
    }
    assert!(
        history
            .iter()
            .flat_map(|message| &message.content)
            .any(|block| matches!(block,
        ContentBlock::Text { text, .. } if text.contains("Host budget hand-back receipt")))
    );
    assert_eq!(
        fixture.manager.read().await.worker_records["report-worker"]
            .verification
            .status,
        "deliverable_missing"
    );
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn budget_handback_turn_failure_and_timeout_do_not_add_worker_retries() {
    let _retry = crate::retry_status::test_guard();
    crate::retry_status::clear_rate_limit();
    for (mode, reason) in [
        ("failure", "provider call failed"),
        ("timeout", "report deadline expired"),
    ] {
        let mut fixture = fixture(mode, 15, Some(100_000), 2).await;
        let result = fixture.finish().await;
        assert_eq!(fixture.requests.lock().unwrap().len(), 2);
        assert_eq!(result.status, SubAgentStatus::BudgetExhausted);
        assert!(
            result.result.as_deref().unwrap().contains(reason),
            "{result:?}"
        );
        assert_eq!(result.usage.as_ref().unwrap().total_tokens, Some(15));
        assert_eq!(
            fixture.manager.read().await.worker_records["report-worker"].has_unreported_usage,
            mode == "timeout",
            "only the timed-out dispatched call establishes missing coverage here",
        );
        assert_eq!(
            fixture.manager.read().await.worker_records["report-worker"]
                .verification
                .status,
            "deliverable_missing"
        );
    }
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn budget_handback_turn_tiny_exhausted_and_unknown_allowances_refuse_paid_summary() {
    let _retry = crate::retry_status::test_guard();
    crate::retry_status::clear_rate_limit();
    for (mode, tokens, cap, reason) in [
        ("success", 15, 15, "too small"),
        ("success", 100_000, 100_000, "remaining token allowance"),
        ("unknown", 0, 100_000, "usage is unknown"),
    ] {
        let mut fixture = fixture(mode, tokens, Some(cap), 2).await;
        let result = fixture.finish().await;
        assert_eq!(fixture.requests.lock().unwrap().len(), 1);
        assert_eq!(result.status, SubAgentStatus::BudgetExhausted);
        assert!(
            result.result.as_deref().unwrap().contains(reason),
            "{result:?}"
        );
        if mode == "unknown" {
            assert_eq!(result.usage.as_ref().unwrap().total_tokens, None);
        }
    }
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn budget_handback_turn_missing_report_usage_is_not_claimed_as_zero_cost() {
    let _retry = crate::retry_status::test_guard();
    crate::retry_status::clear_rate_limit();
    let mut fixture = fixture("report-unknown", 15, Some(100_000), 2).await;
    let result = fixture.finish().await;
    assert_eq!(fixture.requests.lock().unwrap().len(), 2);
    assert_eq!(result.status, SubAgentStatus::BudgetExhausted);
    assert_eq!(result.usage.as_ref().unwrap().total_tokens, Some(15));
    let report = result.result.as_deref().unwrap();
    assert!(report.contains("PARTIAL_REPORT"));
    assert!(report.contains("only a subtotal, not a zero-cost report"));
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn budget_handback_resumed_known_response_keeps_interrupted_source_usage_unknown() {
    let _retry = crate::retry_status::test_guard();
    crate::retry_status::clear_rate_limit();
    let mut fixture = fixture("resume-unknown", 0, Some(100_000), 4).await;
    // The first response omits usage; the second ordinary request is held so
    // the real interrupt/followup path resumes that checkpoint, not a forged
    // continuation edge. A known response in the successor cannot repair the
    // source's positively known missing coverage.
    tokio::time::timeout(Duration::from_secs(2), fixture.report_started.notified())
        .await
        .unwrap();
    let successor = {
        let mut manager = fixture.manager.write().await;
        let (_, interrupted) = manager
            .interrupt_child("report-worker", None, "fixture interruption".to_string())
            .unwrap();
        assert!(matches!(interrupted.status, SubAgentStatus::Interrupted(_)));
        assert!(interrupted.checkpoint.as_ref().unwrap().continuable);
        assert!(manager.worker_records["report-worker"].has_unreported_usage);
        assert_eq!(
            manager.worker_records["report-worker"].usage.total_tokens,
            None
        );
        // This fixture owns the original task handle instead of the manager.
        // Abort it just as production interrupt does before launching followup.
        fixture.task.take().unwrap().abort();
        let saved = serde_json::to_vec(&manager.worker_records).unwrap();
        manager.worker_records = serde_json::from_slice(&saved).unwrap();
        assert!(manager.worker_records["report-worker"].has_unreported_usage);
        manager
            .resume_from_checkpoint(
                Arc::clone(&fixture.manager),
                fixture.resume_runtime.clone(),
                "report-worker",
                "Continue the inspection.",
            )
            .unwrap()
            .agent_id
    };
    let result = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let result = fixture.manager.read().await.get_result(&successor).unwrap();
            if result.status != SubAgentStatus::Running {
                return result;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("resumed worker must settle");
    assert_eq!(result.status, SubAgentStatus::BudgetExhausted);
    assert_eq!(result.usage.as_ref().unwrap().total_tokens, Some(15));
    assert!(
        result
            .result
            .as_deref()
            .unwrap()
            .contains("usage is unknown in an applicable budget scope")
    );
    assert_eq!(
        fixture.requests.lock().unwrap().len(),
        3,
        "no successor reporting request"
    );
    let manager = fixture.manager.read().await;
    assert!(manager.worker_records["report-worker"].has_unreported_usage);
    assert!(!manager.worker_records[&successor].has_unreported_usage);
    assert_eq!(
        manager.worker_records["report-worker"].usage.total_tokens,
        None
    );
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn budget_handback_shared_sibling_unknown_usage_blocks_only_paid_reporting() {
    let _retry = crate::retry_status::test_guard();
    crate::retry_status::clear_rate_limit();
    let mut fixture = fixture("shared-unknown", 15, Some(100_000), 2).await;
    let result = fixture.finish().await;
    assert_eq!(result.status, SubAgentStatus::BudgetExhausted);
    assert_eq!(result.usage.as_ref().unwrap().total_tokens, Some(15));
    assert!(
        result
            .result
            .as_deref()
            .unwrap()
            .contains("usage is unknown in an applicable budget scope")
    );
    assert_eq!(fixture.requests.lock().unwrap().len(), 1);
    let manager = fixture.manager.read().await;
    assert!(manager.worker_records["settled-sibling"].has_unreported_usage);
    assert!(!manager.worker_records["report-worker"].has_unreported_usage);
    assert_eq!(
        manager.worker_records["settled-sibling"].usage.total_tokens,
        Some(11)
    );
    assert_eq!(
        manager.aggregate_budget_spent("report-pool"),
        26,
        "known subtotals are never erased"
    );
    assert_eq!(
        manager.worker_records["report-worker"].verification.status,
        "deliverable_missing"
    );
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn budget_handback_inflight_wall_timeout_persists_unknown_scope_coverage() {
    let _retry = crate::retry_status::test_guard();
    crate::retry_status::clear_rate_limit();
    let mut fixture = fixture("work-timeout", 15, Some(100_000), 4).await;
    let result = fixture.finish().await;
    assert_eq!(result.status, SubAgentStatus::BudgetExhausted);
    assert!(
        result
            .result
            .as_deref()
            .unwrap()
            .contains("wall-time budget exhausted during a model request")
    );
    assert_eq!(
        fixture.requests.lock().unwrap().len(),
        2,
        "no report after an unmeasured in-flight request"
    );
    let mut manager = fixture.manager.write().await;
    assert!(manager.worker_records["report-worker"].has_unreported_usage);
    assert_eq!(
        manager.worker_records["report-worker"].usage.total_tokens,
        Some(15)
    );
    let saved = serde_json::to_vec(&manager.worker_records).unwrap();
    manager.worker_records = serde_json::from_slice(&saved).unwrap();
    let mut sibling = make_worker_spec("later-sibling", fixture.workspace.path().to_path_buf());
    sibling.runtime_profile.token_budget = Some(100_000);
    manager.register_worker(sibling);
    manager.attach_shared_budget_scope("later-sibling", "report-pool", 100_000);
    assert!(matches!(
        manager.reserve_handback("later-sibling", Some(100_000), 8_192, 500, 1_024),
        Err(reason) if reason.contains("usage is unknown")
    ));
    assert_eq!(manager.aggregate_budget_spent("report-pool"), 15);
}

#[test]
fn budget_handback_coverage_marker_is_sticky_without_reclassifying_legacy_or_measured_zero() {
    let tmp = tempdir().unwrap();
    let mut manager = SubAgentManager::new(tmp.path().to_path_buf(), 4);
    for id in ["worker", "unrelated"] {
        let mut spec = make_worker_spec(id, tmp.path().to_path_buf());
        spec.runtime_profile.token_budget = Some(20_000);
        manager.register_worker(spec);
    }
    let measured_zero = Usage {
        prompt_cache_hit_tokens: Some(0),
        ..Usage::default()
    };
    manager.record_worker_usage("worker", "zero", &measured_zero, None);
    manager.record_worker_usage("unrelated", "unrelated-missing", &Usage::default(), None);
    assert_eq!(manager.worker_records["worker"].usage.total_tokens, Some(0));
    assert!(!manager.worker_records["worker"].has_unreported_usage);
    let mut legacy = serde_json::to_value(&manager.worker_records["worker"]).unwrap();
    legacy
        .as_object_mut()
        .unwrap()
        .remove("has_unreported_usage");
    let legacy: AgentWorkerRecord = serde_json::from_value(legacy).unwrap();
    assert!(!legacy.has_unreported_usage);
    manager.worker_records.insert("worker".to_string(), legacy);
    let lease = manager
        .reserve_handback("worker", Some(20_000), 2_000, 500, 1_024)
        .unwrap();
    drop(lease);
    manager.record_worker_usage("worker", "missing", &Usage::default(), None);
    manager.record_worker_usage(
        "worker",
        "known-later",
        &Usage {
            input_tokens: 10,
            output_tokens: 5,
            ..Usage::default()
        },
        None,
    );
    let saved = serde_json::to_vec(&manager.worker_records).unwrap();
    manager.worker_records = serde_json::from_slice(&saved).unwrap();
    assert_eq!(
        manager.worker_records["worker"].usage.total_tokens,
        Some(15)
    );
    assert!(manager.worker_records["worker"].has_unreported_usage);
    assert!(matches!(
        manager.reserve_handback("worker", Some(20_000), 2_000, 500, 1_024),
        Err(reason) if reason.contains("usage is unknown")
    ));
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn budget_handback_turn_cancellation_wins_once_and_releases_shared_reservation() {
    let _retry = crate::retry_status::test_guard();
    crate::retry_status::clear_rate_limit();
    let mut fixture = fixture("hold", 15, Some(100_000), 2).await;
    tokio::time::timeout(Duration::from_secs(2), fixture.report_started.notified())
        .await
        .unwrap();
    fixture.cancel.cancel();
    let result = fixture.finish().await;
    assert_eq!(result.status, SubAgentStatus::Cancelled);
    assert_eq!(result.usage.as_ref().unwrap().total_tokens, Some(15));
    assert!(fixture.manager.read().await.worker_records["report-worker"].has_unreported_usage);
    assert!(
        fixture
            .completions
            .try_recv()
            .unwrap()
            .payload
            .contains("cancelled")
    );
    assert!(fixture.completions.try_recv().is_err());
    assert!(
        fixture
            .manager
            .read()
            .await
            .handback_reservations
            .values()
            .all(|value| value.upgrade().is_none())
    );
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn budget_handback_turn_cancellation_after_response_preserves_actual_usage() {
    let _retry = crate::retry_status::test_guard();
    crate::retry_status::clear_rate_limit();
    let mut fixture = fixture("hold", 15, Some(100_000), 2).await;
    tokio::time::timeout(Duration::from_secs(2), fixture.report_started.notified())
        .await
        .unwrap();
    let manager = Arc::clone(&fixture.manager);
    let guard = manager.write().await;
    fixture.release_report.notify_one();
    // Runtime billing publishes the decoded response before it waits for the
    // worker ledger lock. This makes the cancellation seam deterministic.
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let entry = fixture.mailbox.recv().await.unwrap();
            if matches!(entry.message, MailboxMessage::TokenUsage { ref source_id, .. } if source_id.contains(":handback:")) { break; }
        }
    }).await.unwrap();
    fixture.cancel.cancel();
    drop(guard);
    let result = fixture.finish().await;
    assert_eq!(result.status, SubAgentStatus::Cancelled);
    assert_eq!(result.usage.as_ref().unwrap().total_tokens, Some(45));
    assert!(!fixture.manager.read().await.worker_records["report-worker"].has_unreported_usage);
    assert!(
        fixture
            .completions
            .try_recv()
            .unwrap()
            .payload
            .contains("cancelled")
    );
    assert!(fixture.completions.try_recv().is_err());
}

#[test]
fn budget_handback_reservations_intersect_shared_ancestor_and_source_caps_without_refunds() {
    let tmp = tempdir().unwrap();
    let mut manager = SubAgentManager::new(tmp.path().to_path_buf(), 8);
    for (id, parent, cap) in [
        ("parent", None, 20_000),
        ("source", None, 15_000),
        ("one", Some("parent"), 15_000),
        ("two", Some("parent"), 15_000),
    ] {
        let mut spec = make_worker_spec(id, tmp.path().to_path_buf());
        spec.parent_run_id = parent.map(str::to_string);
        spec.runtime_profile.token_budget = Some(cap);
        manager.register_worker(spec);
        manager.attach_shared_budget_scope(id, "shared", 20_000);
    }
    manager
        .worker_records
        .get_mut("parent")
        .unwrap()
        .usage
        .total_tokens = Some(18_000);
    let one = manager
        .reserve_handback("one", Some(2_000), 2_000, 900, 1_024)
        .unwrap();
    assert_eq!(manager.available_worker_tokens("two", false), Some(76));
    assert!(
        manager
            .reserve_handback("two", Some(2_000), 2_000, 100, 1_024)
            .is_err()
    );
    assert!(
        manager
            .resolve_spawn_budget_scope("new", Some("parent"), None)
            .is_err()
    );
    assert_eq!(
        manager.aggregate_budget_spent("shared"),
        18_000,
        "a reservation is not actual usage"
    );
    drop(one);
    assert_eq!(manager.available_worker_tokens("two", false), Some(2_000));
    let source = manager.worker_records.get_mut("source").unwrap();
    source.usage.total_tokens = Some(1_000);
    source.spec.runtime_profile.token_budget = Some(1_050);
    let profile = manager.worker_records["two"].spec.runtime_profile.clone();
    manager
        .worker_records
        .get_mut("two")
        .unwrap()
        .spec
        .launch_manifest = Some(
        serde_json::from_value(json!({
            "owner_session": "parent", "child_id": "two", "profile": profile,
            "prompt": "continue", "cwd": null, "worktree": false, "writable_roots": [],
            "writable_files": [], "coordination_contracts": [], "generation": 1,
            "resume_identity": null, "resume_from_agent_id": "source"
        }))
        .unwrap(),
    );
    assert_eq!(manager.available_worker_tokens("two", false), Some(50));
    assert!(
        manager
            .reserve_handback("two", None, 2_000, 100, 1_024)
            .is_err()
    );
    let saved = serde_json::to_vec(&manager.worker_records).unwrap();
    manager.worker_records = serde_json::from_slice(&saved).unwrap();
    assert_eq!(manager.available_worker_tokens("two", false), Some(50));
    assert_eq!(manager.aggregate_budget_spent("shared"), 19_000);
}

#[tokio::test]
async fn budget_handback_expired_original_deadline_refuses_the_model_call() {
    let tmp = tempdir().unwrap();
    let mut runtime = stub_runtime();
    runtime.context = ToolContext::new(tmp.path().to_path_buf());
    runtime.manager = Arc::new(RwLock::new(SubAgentManager::new(
        tmp.path().to_path_buf(),
        1,
    )));
    runtime
        .manager
        .write()
        .await
        .register_worker(make_worker_spec("expired", tmp.path().to_path_buf()));
    let mut steps = 1;
    let outcome = budget_handback::request_report(
        &runtime,
        "expired",
        &SubAgentAssignment::new("report".to_string(), None),
        &mut vec![],
        &mut steps,
        2,
        Some(100_000),
        15,
        8_192,
        true,
        Some(Instant::now() - Duration::from_millis(1)),
        "wall-time budget exhausted",
    )
    .await;
    assert!(
        matches!(outcome, budget_handback::Outcome::Fallback(ref why) if why.contains("deadline has expired"))
    );
    assert_eq!(steps, 1, "no model turn was admitted");
    assert!(!runtime.manager.read().await.worker_records["expired"].has_unreported_usage);
    assert!(
        runtime
            .manager
            .read()
            .await
            .handback_reservations
            .is_empty()
    );
}
