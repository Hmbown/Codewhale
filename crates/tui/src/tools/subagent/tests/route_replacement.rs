//! Operator-approved route replacement at the first-request seam.
//!
//! The observed failure: a saved reviewer pin answered its first request with
//! `Authorization failed: You have run out of credits or need a Grok
//! subscription.` and no review was produced.
use super::*;

const REFUSAL: &str = "You have run out of credits or need a Grok subscription.";

/// A provider fixture that refuses every request with HTTP 403.
async fn refusing_chat_server() -> (String, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let app = Router::new().route(
        "/{*path}",
        post({
            let calls = Arc::clone(&calls);
            move |Json(_body): Json<Value>| {
                let calls = Arc::clone(&calls);
                async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    (
                        StatusCode::FORBIDDEN,
                        Json(json!({"error": {"message": REFUSAL}})),
                    )
                        .into_response()
                }
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind refusing server");
    let addr = listener.local_addr().expect("refusing server addr");
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (format!("http://{addr}/v1"), calls)
}

async fn write_replacement_config(
    path: &std::path::Path,
    pin_url: &str,
    backup_url: &str,
    pin: &str,
) {
    tokio::fs::write(
        path,
        format!(
            r#"
provider = "deepseek"
model = "deepseek-v4-flash"
api_key = "fixture-key"
base_url = "{backup_url}"

[retry]
enabled = false
max_retries = 0

[providers.PinRoute]
kind = "openai-compatible"
api_key = "fixture-pin-key"
base_url = "{pin_url}"
model = "fixture-pin-model"

[providers.BackupRoute]
kind = "openai-compatible"
api_key = "fixture-backup-key"
base_url = "{backup_url}"
model = "fixture-backup-model"

{pin}
"#
        ),
    )
    .await
    .unwrap();
}

async fn reviewer_tool(
    root: &std::path::Path,
    pin: &str,
) -> (
    AgentTool,
    ToolContext,
    SharedSubAgentManager,
    Arc<AtomicUsize>,
    Arc<AtomicUsize>,
) {
    let (backup, backup_calls, _) = delayed_chat_client(Duration::ZERO, "review done").await;
    let (pin_url, pin_calls) = refusing_chat_server().await;
    let config_path = root.join("config.toml");
    write_replacement_config(&config_path, &pin_url, backup.base_url(), pin).await;
    let config = crate::config::Config::load(Some(config_path), None).unwrap();
    let client = CodewhaleClient::new(&config).unwrap();
    let manager = new_shared_subagent_manager(root.to_path_buf(), 2);
    let context = ToolContext::new(root).with_state_namespace("route-replacement");
    let runtime = SubAgentRuntime::new(
        client,
        "deepseek-v4-flash".into(),
        context.clone(),
        false,
        None,
        manager.clone(),
    )
    .with_api_config(config);
    (
        AgentTool::new(manager.clone(), runtime),
        context,
        manager,
        pin_calls,
        backup_calls,
    )
}

async fn run_reviewer(pin: &str) -> (Value, SubAgentResult, usize, usize) {
    let root = tempdir().unwrap();
    let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", root.path().join("state"));
    let _provider = crate::test_support::EnvVarGuard::set("CODEWHALE_PROVIDER", "deepseek");
    let _model = crate::test_support::EnvVarGuard::set("CODEWHALE_MODEL", "deepseek-v4-flash");
    let (tool, context, manager, pin_calls, backup_calls) = reviewer_tool(root.path(), pin).await;
    let started = tool
        .execute(
            json!({"type": "reviewer", "prompt": "Review the change."}),
            &context,
        )
        .await
        .unwrap();
    let meta = started.metadata.clone().unwrap();
    let id = meta["agent_id"].as_str().unwrap().to_string();
    let result = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let result = manager.read().await.get_result(&id).expect("registered");
            if result.status != SubAgentStatus::Running {
                return result;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("child settles");
    (
        meta,
        result,
        pin_calls.load(Ordering::SeqCst),
        backup_calls.load(Ordering::SeqCst),
    )
}

#[tokio::test]
async fn approved_replacement_takes_a_refused_first_request_and_is_receipted() {
    let _env = crate::test_support::lock_test_env();
    let (meta, result, pin_calls, backup_calls) = run_reviewer(
        r#"
[subagents.roles.reviewer]
model = "PinRoute/fixture-pin-model"
replacements = ["BackupRoute/fixture-backup-model"]
"#,
    )
    .await;
    assert_eq!(meta["child_route"]["provider_id"], "PinRoute");
    assert_eq!(pin_calls, 1, "the refused route is asked exactly once");
    assert!(backup_calls >= 1, "the approved route took the request");
    assert_eq!(
        result.status,
        SubAgentStatus::Completed,
        "{:?}",
        result.status
    );
    assert_eq!(result.result.as_deref(), Some("review done"));
    let route = result.child_route.expect("route receipt");
    assert_eq!(route.provider_id, "BackupRoute");
    assert_eq!(route.model_id, "fixture-backup-model");
    assert_eq!(route.route_source, "role.replacement");
    let note = route.fallback_note.expect("replacement note");
    for fact in [
        "fixture-pin-model",
        "provider refused authorization",
        "run out of credits",
        "BackupRoute/fixture-backup-model",
        "attempt 1 of 1",
    ] {
        assert!(note.contains(fact), "{fact} missing from {note}");
    }
    assert!(!note.contains("fixture-backup-key") && !note.contains("fixture-pin-key"));
}

#[tokio::test]
async fn a_pin_without_approved_replacements_stays_exact() {
    let _env = crate::test_support::lock_test_env();
    let (_meta, result, pin_calls, backup_calls) = run_reviewer(
        r#"
[subagents.roles.reviewer]
model = "PinRoute/fixture-pin-model"
"#,
    )
    .await;
    assert_eq!(pin_calls, 1);
    assert_eq!(backup_calls, 0, "no provider was asked without approval");
    let SubAgentStatus::Failed(error) = &result.status else {
        panic!("an exact refused pin fails: {:?}", result.status);
    };
    assert!(error.contains("run out of credits"), "{error}");
    assert_eq!(result.child_route.unwrap().provider_id, "PinRoute");
}

#[test]
fn replacement_reasons_are_typed_never_message_matched() {
    let refusal = |status| anyhow::Error::new(LlmError::from_http_response(status, REFUSAL));
    assert_eq!(
        route_replacement_reason(&refusal(403)),
        Some("provider refused authorization")
    );
    assert_eq!(
        route_replacement_reason(&refusal(401)),
        Some("credentials rejected")
    );
    // A credits-themed message without typed evidence is not a route refusal.
    assert_eq!(route_replacement_reason(&anyhow!(REFUSAL)), None);
    assert_eq!(
        route_replacement_reason(&anyhow::Error::new(LlmError::ContentPolicyError(
            "blocked".into()
        ))),
        None,
        "content refusals are never shopped to another provider"
    );
    assert_eq!(
        route_replacement_reason(&anyhow::Error::new(LlmError::ContextLengthError(
            "too long".into()
        ))),
        None
    );
}

#[test]
fn replacements_must_name_their_provider() {
    let config: crate::config::Config = toml::from_str(
        r#"
[subagents.roles.reviewer]
model = "deepseek-v4-pro"
replacements = ["deepseek/deepseek-v4-flash", "bare-model"]
"#,
    )
    .unwrap();
    let routes = config.subagent_route_replacements("reviewer");
    assert_eq!(routes.len(), 2);
    assert_eq!(routes[0].provider.as_deref(), Some("deepseek"));
    assert_eq!(routes[1].provider, None);
    assert!(config.subagent_route_replacements("builder").is_empty());
}

#[tokio::test]
async fn a_replacement_without_an_explicit_provider_fails_at_spawn() {
    let _env = crate::test_support::lock_test_env();
    let root = tempdir().unwrap();
    let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", root.path().join("state"));
    let (tool, context, manager, pin_calls, backup_calls) = reviewer_tool(
        root.path(),
        r#"
[subagents.roles.reviewer]
model = "PinRoute/fixture-pin-model"
replacements = ["fixture-backup-model"]
"#,
    )
    .await;
    let error = tool
        .execute(json!({"type": "reviewer", "prompt": "Review."}), &context)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("provider/model"), "{error}");
    assert!(manager.read().await.agents.is_empty());
    assert_eq!(
        pin_calls.load(Ordering::SeqCst) + backup_calls.load(Ordering::SeqCst),
        0
    );
}
