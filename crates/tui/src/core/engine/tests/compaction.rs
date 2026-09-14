use super::*;

#[tokio::test]
async fn rejected_manual_compaction_route_closes_typed_lifecycle() {
    let _env_lock = lock_test_env();
    let _api_key = EnvVarGuard::remove("DEEPSEEK_API_KEY");
    let route_config = Config {
        provider: Some("deepseek".to_string()),
        api_key: Some(String::new()),
        default_text_model: Some(crate::config::DEFAULT_TEXT_MODEL.to_string()),
        ..Config::default()
    };
    let route = resolve_runtime_route(
        &route_config,
        ApiProvider::Deepseek,
        Some(crate::config::DEFAULT_TEXT_MODEL),
    )
    .expect("structurally resolve route without credential");
    assert!(
        route.clone().validate().is_err(),
        "fixture must fail at engine route installation"
    );
    let (mut engine, handle) = Engine::new(EngineConfig::default(), &route_config);

    engine
        .handle_manual_compaction_op(
            "compact-route-invalid".to_string(),
            route,
            CompactionConfig::default(),
        )
        .await;

    let mut started_id = None;
    let mut failed_id = None;
    let mut order = Vec::new();
    let mut events = handle.rx_event.write().await;
    while let Ok(event) = events.try_recv() {
        match event {
            Event::CompactionStarted { id, auto, .. } => {
                assert!(!auto);
                started_id = Some(id);
                order.push("started");
            }
            Event::CompactionFailed { id, auto, message } => {
                assert!(!auto);
                assert!(message.contains("provider route is not ready"));
                failed_id = Some(id);
                order.push("failed");
            }
            Event::Error { .. } => order.push("error"),
            _ => {}
        }
    }
    assert_eq!(order, ["started", "failed", "error"]);
    assert_eq!(started_id, failed_id);
}

#[tokio::test]
async fn queued_manual_compaction_cancellation_is_idempotent_and_skips_route_activation() {
    let _env_lock = lock_test_env();
    let _api_key = EnvVarGuard::remove("DEEPSEEK_API_KEY");
    let route_config = Config {
        provider: Some("deepseek".to_string()),
        api_key: Some(String::new()),
        default_text_model: Some(crate::config::DEFAULT_TEXT_MODEL.to_string()),
        ..Config::default()
    };
    let route = resolve_runtime_route(
        &route_config,
        ApiProvider::Deepseek,
        Some(crate::config::DEFAULT_TEXT_MODEL),
    )
    .expect("structurally resolve route without credential");
    let (mut engine, handle) = Engine::new(EngineConfig::default(), &route_config);
    let id = "compact-cancel-before-start";

    handle.cancel_compaction(id).expect("first cancel accepted");
    handle
        .cancel_compaction(id)
        .expect("replayed cancel remains idempotent");
    engine
        .handle_manual_compaction_op(id.to_string(), route, CompactionConfig::default())
        .await;

    let mut events = handle.rx_event.write().await;
    let drained = std::iter::from_fn(|| events.try_recv().ok()).collect::<Vec<_>>();
    assert!(matches!(
        drained.as_slice(),
        [
            Event::CompactionStarted { id: started, auto: false, .. },
            Event::CompactionCancelled { id: cancelled, auto: false, .. },
            Event::TurnComplete { status: TurnOutcomeStatus::Interrupted, .. }
        ] if started == id && cancelled == id
    ));
    assert!(
        !drained
            .iter()
            .any(|event| matches!(event, Event::Error { .. })),
        "pre-start cancellation must not activate or validate the provider route"
    );

    let retry = engine
        .claim_compaction(id)
        .expect("the same stable id can be retried after terminal settlement");
    assert!(!retry.is_cancelled());
    handle
        .cancel_compaction(id)
        .expect("running cancel accepted");
    assert!(
        retry.is_cancelled(),
        "running cancellation reaches its token"
    );
    engine.finish_compaction(id);
}

struct BlockingEmergencyCompactionModelClient {
    entered: std::sync::Arc<tokio::sync::Notify>,
    request_dropped: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

#[tokio::test]
async fn failed_emergency_compaction_preserves_history_instead_of_trimming() {
    use crate::llm_client::mock::MockLlmClient;
    let _env_lock = lock_test_env();
    let workspace = tempdir().unwrap();
    // Emergency compaction persists a checkpoint before calling the model.
    // Keep that prerequisite away from other tests' shared state fixtures so
    // this exercises a summary failure, rather than an unrelated write failure.
    let _home = EnvVarGuard::set("CODEWHALE_HOME", workspace.path());
    let (mut engine, handle) = Engine::new(
        deterministic_engine_config(workspace.path()),
        &Config::default(),
    );
    engine.session.messages = (0..12)
        .map(|i| Message {
            role: if i % 2 == 0 {
                Role::User
            } else {
                Role::Assistant
            },
            content: vec![ContentBlock::Text {
                text: format!(
                    "must preserve instruction and evidence {i}: {}",
                    "x".repeat(20_000)
                ),
                cache_control: None,
            }],
        })
        .collect::<Vec<_>>()
        .into();
    let before = engine.session.messages.clone();
    let summary = engine.session.compaction_summary_prompt.clone();
    let client = MockLlmClient::new(Vec::new());
    let tools = vec![catalog_tool("read")];
    let mut turn = TurnContext::new(1);
    assert!(
        !engine
            .recover_context_overflow(
                &client,
                Some(&tools),
                "provider rejection fixture",
                &mut turn
            )
            .await
    );
    assert_eq!(engine.session.messages.as_slice(), before.as_slice());
    assert_eq!(engine.session.compaction_summary_prompt, summary);
    let mut events = handle.rx_event.write().await;
    let drained = std::iter::from_fn(|| events.try_recv().ok()).collect::<Vec<_>>();
    assert_eq!(
        client.call_count(),
        1,
        "a deterministic summary failure is not retried unchanged: {drained:?}"
    );
    let requests = client.captured_requests();
    assert_eq!(requests[0].tools.as_deref(), Some(tools.as_slice()));
    assert_eq!(requests[0].tool_choice, Some(json!("none")));
    assert_eq!(requests[0].system, engine.session.system_prompt);
}

#[tokio::test]
async fn manual_compaction_accounts_accepted_and_rejected_responses_once() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let _env_lock = lock_test_env();
    let _cost_scope = crate::cost_status::test_scope();
    let workspace = tempdir().expect("isolated compaction workspace");
    let _home = EnvVarGuard::set("CODEWHALE_HOME", workspace.path());
    for (finish_reason, expected_status) in [
        ("stop", TurnOutcomeStatus::Completed),
        ("length", TurnOutcomeStatus::Failed),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": format!("compaction-{finish_reason}"),
                "object": "chat.completion",
                "model": crate::config::DEFAULT_TEXT_MODEL,
                "choices": [{
                    "index": 0,
                    "message": { "role": "assistant", "content": "Primary request: preserve the session migration. Completed: inspected the existing store. Constraints: keep every user message and failing test. Next: finish the transactional migration and rerun session_store::roundtrip." },
                    "finish_reason": finish_reason,
                }],
                "usage": { "prompt_tokens": 41, "completion_tokens": 7, "total_tokens": 48 },
            })))
            .expect(1)
            .mount(&server)
            .await;
        let route_config = Config {
            provider: Some("deepseek".to_string()),
            api_key: Some("fixture-key".to_string()),
            base_url: Some(format!("{}/v1", server.uri())),
            ..Config::default()
        };
        let (mut engine, handle) = Engine::new(
            EngineConfig {
                workspace: workspace.path().to_path_buf(),
                snapshots_enabled: false,
                subagents_enabled: false,
                ..EngineConfig::default()
            },
            &route_config,
        );
        engine.session.messages.push(Message {
            role: Role::User,
            content: vec![ContentBlock::Text {
                text: "Preserve the transactional session migration.".to_string(),
                cache_control: None,
            }],
        });
        engine.config.goal_state.lock().unwrap().replace(
            "Finish the session migration",
            Some(1000),
            None,
        );
        engine
            .handle_manual_compaction("compact-accounting".to_string(), CancellationToken::new())
            .await;

        assert_eq!(engine.session.total_usage.input_tokens, 41);
        assert_eq!(engine.session.total_usage.output_tokens, 7);
        assert_eq!(
            engine
                .config
                .goal_state
                .lock()
                .unwrap()
                .snapshot()
                .tokens_used,
            48
        );
        let mut events = handle.rx_event.write().await;
        let mut telemetry_count = 0;
        let mut terminal_count = 0;
        while let Ok(event) = events.try_recv() {
            match event {
                Event::RoutedTurnUsage { usage, .. } => {
                    telemetry_count += 1;
                    assert_eq!((usage.input_tokens, usage.output_tokens), (41, 7));
                }
                Event::TurnComplete {
                    usage,
                    parent_route_usage,
                    status,
                    ..
                } => {
                    terminal_count += 1;
                    assert_eq!((usage.input_tokens, usage.output_tokens), (41, 7));
                    assert_eq!(parent_route_usage, Usage::default());
                    assert_eq!(status, expected_status);
                }
                _ => {}
            }
        }
        assert_eq!((telemetry_count, terminal_count), (1, 1));
    }
}

#[async_trait::async_trait]
impl crate::core::model_client::ModelClient for BlockingEmergencyCompactionModelClient {
    fn provider_name(&self) -> &str {
        "deepseek"
    }

    fn model(&self) -> &str {
        crate::config::DEFAULT_TEXT_MODEL
    }

    async fn create_message(
        &self,
        _request: codewhale_models::MessageRequest,
    ) -> anyhow::Result<codewhale_models::MessageResponse> {
        let _drop_signal = DropSignal(std::sync::Arc::clone(&self.request_dropped));
        self.entered.notify_one();
        std::future::pending().await
    }

    async fn create_message_stream(
        &self,
        _request: codewhale_models::MessageRequest,
    ) -> anyhow::Result<crate::llm_client::StreamEventBox> {
        anyhow::bail!("emergency compaction uses the non-streaming model boundary")
    }

    async fn health_check(&self) -> anyhow::Result<bool> {
        Ok(true)
    }
}

#[tokio::test]
async fn emergency_compaction_cancellation_drops_provider_and_never_mutates_context() {
    let route_config = Config {
        provider: Some("deepseek".to_string()),
        default_text_model: Some(crate::config::DEFAULT_TEXT_MODEL.to_string()),
        ..Config::default()
    };
    let (mut engine, handle) = Engine::new(EngineConfig::default(), &route_config);
    engine.session.messages = (0..8)
        .map(|index| Message {
            role: if index % 2 == 0 {
                Role::User
            } else {
                Role::Assistant
            },
            content: vec![ContentBlock::Text {
                text: format!("preserve emergency context item {index}"),
                cache_control: None,
            }],
        })
        .collect::<Vec<_>>()
        .into();
    let messages_before = engine.session.messages.clone();
    let checkpoint_before = engine.session.compaction_summary_prompt.clone();
    let entered = std::sync::Arc::new(tokio::sync::Notify::new());
    let request_dropped = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let client = std::sync::Arc::new(BlockingEmergencyCompactionModelClient {
        entered: std::sync::Arc::clone(&entered),
        request_dropped: std::sync::Arc::clone(&request_dropped),
    });

    let recovery = tokio::spawn(async move {
        let mut turn = TurnContext::new(1);
        let recovered = engine
            .recover_context_overflow(client.as_ref(), None, "cancellation regression", &mut turn)
            .await;
        (engine, recovered)
    });

    let started_id = tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let event = handle
                .rx_event
                .write()
                .await
                .recv()
                .await
                .expect("emergency compaction start event");
            if let Event::CompactionStarted { id, auto: true, .. } = event {
                break id;
            }
        }
    })
    .await
    .expect("emergency compaction publishes its stable id");
    tokio::time::timeout(Duration::from_secs(1), entered.notified())
        .await
        .expect("emergency provider request starts");

    handle
        .cancel_compaction(started_id.clone())
        .expect("exact emergency cancellation accepted");
    let (engine, recovered) = tokio::time::timeout(Duration::from_secs(1), recovery)
        .await
        .expect("emergency cancellation settles promptly")
        .expect("recovery task");

    assert!(!recovered);
    assert_eq!(&*engine.session.messages, &*messages_before);
    assert_eq!(engine.session.compaction_summary_prompt, checkpoint_before);
    assert!(
        request_dropped.load(std::sync::atomic::Ordering::SeqCst),
        "cancellation must drop the in-flight provider future"
    );

    let mut events = handle.rx_event.write().await;
    let drained = std::iter::from_fn(|| events.try_recv().ok()).collect::<Vec<_>>();
    assert!(matches!(
        drained.as_slice(),
        [Event::CompactionCancelled { id, auto: true, .. }] if id == &started_id
    ));
    assert!(
        !drained.iter().any(|event| matches!(
            event,
            Event::CompactionCompleted { .. } | Event::CompactionFailed { .. }
        )),
        "a canceled emergency pass must have one canceled terminal event"
    );
}
