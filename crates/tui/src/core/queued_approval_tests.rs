//! E2 regression (DOGFOOD-DESKTOP-20260922, app #97): approving one call
//! must never cancel the calls queued behind it.
//!
//! The dogfood report showed an approval that failed the call it approved
//! and then cancelled the queued calls after it (`request was cancelled
//! before this tool ran`). `59c2a1668` fixed the posture re-check for a
//! single call; this pins the queue: one model step emits three gated shell
//! calls, the second and third wait behind the first approval card, and the
//! client republishes its unchanged posture as the desktop app does on every
//! approval. Every call must run, in order, and none may come back
//! cancelled.

use std::time::Duration;

use codewhale_config::AppMode;
use codewhale_execpolicy::ApprovalMode;
use tempfile::tempdir;

use crate::compaction::CompactionConfig;
use crate::config::Config;
use crate::core::engine::{Engine, EngineConfig};
use crate::core::events::Event;
use crate::core::ops::{Op, TurnSpec, UserInputProvenance};
use crate::test_support::lock_test_env;

const CALLS: [&str; 3] = ["call_e2_first", "call_e2_second", "call_e2_third"];

fn event_timeout() -> Duration {
    // The Windows runner shares CPU with the whole TUI test binary; an
    // approval-gated turn must not be mistaken for a lifecycle failure.
    if cfg!(windows) {
        Duration::from_secs(60)
    } else {
        Duration::from_secs(10)
    }
}

/// One SSE response whose single step carries three gated `Bash` calls, each
/// writing its own marker file.
fn three_gated_calls_sse() -> String {
    let mut sse = String::new();
    for (index, id) in CALLS.iter().enumerate() {
        let arguments = format!(
            "{{\\\"action\\\":\\\"run\\\",\\\"command\\\":\\\"echo {index} > {id}.txt\\\"}}"
        );
        sse.push_str(&format!(
            "data: {{\"id\":\"chatcmpl-e2q\",\"choices\":[{{\"index\":0,\"delta\":{{\"tool_calls\":[\
             {{\"index\":{index},\"id\":\"{id}\",\"type\":\"function\",\"function\":{{\"name\":\"Bash\",\
             \"arguments\":\"{arguments}\"}}}}]}},\"finish_reason\":null}}]}}\n\n"
        ));
    }
    sse.push_str(concat!(
        "data: {\"id\":\"chatcmpl-e2q\",\"choices\":[{\"index\":0,\"delta\":{},",
        "\"finish_reason\":\"tool_calls\"}]}\n\n",
        "data: [DONE]\n\n",
    ));
    sse
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn approving_the_first_of_three_queued_calls_cancels_none_of_them() {
    use wiremock::matchers::{body_string_contains, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let _lock = lock_test_env();
    let workspace = tempdir().expect("tempdir");
    let server = MockServer::start().await;
    let done_sse = concat!(
        "data: {\"id\":\"chatcmpl-e2q-done\",\"choices\":[{\"index\":0,",
        "\"delta\":{\"content\":\"done\"},\"finish_reason\":null}]}\n\n",
        "data: {\"id\":\"chatcmpl-e2q-done\",\"choices\":[{\"index\":0,\"delta\":{},",
        "\"finish_reason\":\"stop\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    // The follow-up request carries the third call's result.
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(body_string_contains(CALLS[2]))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(done_sse),
        )
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(three_gated_calls_sse()),
        )
        .expect(1)
        .with_priority(2)
        .mount(&server)
        .await;

    let api_config = Config {
        ..Config::default()
    }
    .with_legacy_root(Some("test-key".to_string()), Some(server.uri()));
    let route = crate::route_runtime::resolve_runtime_route(
        &api_config,
        api_config.api_provider(),
        Some(crate::config::DEFAULT_TEXT_MODEL),
    )
    .expect("resolve test route");
    let (engine, handle) = Engine::new(
        EngineConfig {
            model: crate::config::DEFAULT_TEXT_MODEL.to_string(),
            workspace: workspace.path().to_path_buf(),
            snapshots_enabled: false,
            subagents_enabled: false,
            terminal_chrome_enabled: false,
            ..EngineConfig::default()
        },
        &api_config,
    );
    let run_task = tokio::spawn(engine.run());
    handle
        .send(Op::SendMessage(TurnSpec {
            max_output_tokens: None,
            content: "Record three approval fixtures in the workspace".to_string(),
            images: Vec::new(),
            mode: AppMode::Agent,
            route: Box::new(route),
            compaction: Box::new(CompactionConfig::default()),
            initial_routed_usage: Box::default(),
            goal_objective: None,
            goal_token_budget: None,
            goal_status: crate::tools::goal::GoalStatus::Active,
            reasoning_effort: None,
            reasoning_effort_auto: false,
            auto_model: false,
            allow_shell: true,
            trust_mode: false,
            auto_approve: false,
            approval_mode: ApprovalMode::Suggest,
            translation_enabled: false,
            allowed_tools: None,
            dynamic_tools: Vec::new(),
            hook_executor: None,
            verbosity: None,
            provenance: UserInputProvenance::ExternalUser,
        }))
        .await
        .expect("send model turn");

    let mut approvals = 0usize;
    let mut results = Vec::new();
    let mut rx = handle.rx_event.write().await;
    while let Some(event) = tokio::time::timeout(event_timeout(), rx.recv())
        .await
        .expect("timed out waiting for turn event")
    {
        match event {
            Event::ApprovalRequired { id, .. } => {
                approvals += 1;
                if approvals == 1 {
                    // The desktop client republishes its unchanged posture
                    // alongside the first approval; with two calls queued
                    // behind this card, that must invalidate nothing.
                    handle
                        .try_send(Op::ChangeMode {
                            mode: AppMode::Agent,
                            allow_shell: true,
                            trust_mode: false,
                            auto_approve: false,
                            approval_mode: ApprovalMode::Suggest,
                            configured_sandbox_mode: None,
                        })
                        .expect("republish posture");
                }
                handle.approve_tool_call(id).await.expect("approve call");
            }
            Event::ToolCallComplete { id, name, result } if name == "Bash" => {
                results.push((id, result));
            }
            Event::TurnComplete { .. } => break,
            _ => {}
        }
    }
    drop(rx);
    handle.send(Op::Shutdown).await.expect("shutdown engine");
    run_task.await.expect("engine task");

    assert!(
        approvals >= 1,
        "the first call waited on an approval card, with the others queued behind it"
    );
    assert_eq!(
        results.len(),
        CALLS.len(),
        "every queued call reported a result: {results:?}"
    );
    for (id, result) in &results {
        let result = result
            .as_ref()
            .unwrap_or_else(|err| panic!("{id} failed after an approval: {err}"));
        assert!(result.success, "{id}: {result:?}");
        let content = result.content.to_ascii_lowercase();
        assert!(
            !content.contains("cancelled") && !content.contains("canceled"),
            "{id} was cancelled by an approval: {}",
            result.content
        );
    }
    let order: Vec<&str> = results.iter().map(|(id, _)| id.as_str()).collect();
    assert_eq!(order, CALLS, "queued calls run in the order the model gave");
    for id in CALLS {
        assert!(
            workspace.path().join(format!("{id}.txt")).exists(),
            "{id} ran: its marker file exists"
        );
    }
}
