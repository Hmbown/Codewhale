//! Preserved snapshot/history safety tests outside the portable debug group.
use super::CommandResult;
use super::contract::debug_operations::prune_undone_tool_context;
use super::groups::debug::undo;
fn patch_undo(app: &mut App) -> CommandResult {
    super::debug_group::host_result(undo::patch_result(
        super::contract::debug_operations::undo_files(app),
    ))
}
fn undo_conversation(app: &mut App) -> CommandResult {
    super::debug_group::host_result(undo::conversation_result(
        super::contract::debug_operations::undo_conversation_for_engine(app),
    ))
}
fn retry(app: &mut App) -> CommandResult {
    super::execute("/retry", app)
}
use crate::config::Config;
use crate::tui::app::{App, AppAction, TuiOptions};
use crate::tui::history::{GenericToolCell, HistoryCell, ToolCell, ToolStatus};
use codewhale_models::Role;
use codewhale_models::{ContentBlock, Message, Tool};
use std::path::PathBuf;

pub(in crate::commands) fn create_test_app() -> App {
    let options = TuiOptions {
        skills_dir: PathBuf::from("/tmp/test-skills"),
        ..crate::test_support::test_tui_options(PathBuf::from("/tmp/test-workspace"))
    };
    let mut app = App::new(options, &Config::default());
    app.ui_locale = codewhale_localization::Locale::En;
    app.cost_currency = crate::pricing::CostCurrency::Usd;
    app.api_provider = crate::config::ApiProvider::Deepseek;
    app
}

#[test]
fn edit_dispatch_loads_unicode_composer_without_truncating_history() {
    let mut app = create_test_app();
    app.push_history_cell(HistoryCell::User {
        content: "edit 漢字🙂".into(),
    });
    let count = app.history.len();
    let result = super::execute("/edit", &mut app);
    assert_eq!(
        result.message.as_deref(),
        Some("Last message loaded into composer — edit and press Enter to resubmit")
    );
    assert_eq!(app.input, "edit 漢字🙂");
    assert_eq!(app.cursor_position, "edit 漢字🙂".chars().count());
    assert!(app.edit_in_progress);
    assert_eq!(app.history.len(), count);
    assert!(result.action.is_none());
    assert!(!result.is_error);
}

#[test]
fn diff_dispatch_reads_only_the_apps_workspace_without_changing_files() {
    let workspace = tempfile::tempdir().unwrap();
    let git = |args: &[&str]| {
        let result = std::process::Command::new("git")
            .args(args)
            .current_dir(workspace.path())
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        String::from_utf8(result.stdout).unwrap()
    };
    git(&["init", "--quiet"]);
    std::fs::write(workspace.path().join("tracked.txt"), "before\n").unwrap();
    git(&["add", "tracked.txt"]);
    git(&[
        "-c",
        "user.name=Fixture",
        "-c",
        "user.email=fixture@example.invalid",
        "-c",
        "commit.gpgsign=false",
        "commit",
        "--quiet",
        "-m",
        "fixture",
    ]);
    let mut app = create_test_app();
    app.workspace = workspace.path().to_path_buf();
    assert_eq!(
        super::execute("/diff", &mut app).message.as_deref(),
        Some("No changes since session start")
    );
    std::fs::write(workspace.path().join("tracked.txt"), "after\n").unwrap();
    let stat = git(&["diff", "--stat"]);
    let result = super::execute("/diff", &mut app);
    assert_eq!(
        result.message,
        Some(format!(
            "Changed files (1):\ntracked.txt\n\n── Stat ──\n{}",
            stat.trim()
        ))
    );
    assert!(!result.is_error);
    assert!(result.action.is_none());
    assert_eq!(
        std::fs::read_to_string(workspace.path().join("tracked.txt")).unwrap(),
        "after\n"
    );
}

pub(in crate::commands) fn test_tool(name: &str) -> Tool {
    Tool {
        tool_type: Some("function".to_string()),
        name: name.to_string(),
        description: format!("{name} test tool"),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "path": {"type": "string"}
            }
        }),
        allowed_callers: None,
        defer_loading: Some(false),
        input_examples: None,
        strict: Some(true),
        cache_control: None,
    }
}

#[test]
fn test_undo_conversation_stages_last_exchange_without_mutating_live_history() {
    let mut app = create_test_app();
    app.history.push(HistoryCell::User {
        content: "Hello".to_string(),
    });
    app.history.push(HistoryCell::Assistant {
        content: "Hi".to_string(),
        streaming: false,
    });
    app.api_messages_mut().push(Message {
        role: Role::User,
        content: vec![],
    });
    app.api_messages_mut().push(Message {
        role: Role::Assistant,
        content: vec![],
    });

    let initial_history_len = app.history.len();
    let initial_api_len = app.api_messages.len();
    let result = undo_conversation(&mut app);

    assert!(result.message.is_some());
    let msg = result.message.unwrap();
    assert!(msg.contains("Removed"));
    assert_eq!(
        app.history.len(),
        initial_history_len,
        "planning leaves live UI untouched"
    );
    assert_eq!(app.api_messages.len(), initial_api_len);
    assert!(
        matches!(result.action, Some(AppAction::ConversationUndo { sync, retry_input: None }) if sync.messages.is_empty())
    );
}

#[test]
fn conversation_undo_includes_following_tool_results_and_runtime_notes() {
    let mut app = create_test_app();
    app.history.push(HistoryCell::User {
        content: "undo this".into(),
    });
    let messages: Vec<Message> = serde_json::from_value(serde_json::json!([
        {"role":"user","content":[{"type":"text","text":"keep this"}]},
        {"role":"assistant","content":[{"type":"text","text":"kept answer"}]},
        {"role":"user","content":[{"type":"text","text":"undo this"}]},
        {"role":"assistant","content":[{"type":"tool_use","id":"call-1","name":"read_file","input":{}}]},
        {"role":"user","content":[{"type":"tool_result","tool_use_id":"call-1","content":"undone tool result"}]},
        {"role":"assistant","content":[{"type":"text","text":"undone answer"}]}
    ])).unwrap();
    app.set_api_messages(std::sync::Arc::new(messages.clone()));
    let result = undo_conversation(&mut app);
    let Some(AppAction::ConversationUndo { sync, .. }) = result.action else {
        panic!("missing rollback")
    };
    assert_eq!(sync.messages, messages[..2]);
    assert_eq!(app.api_messages.as_ref(), &messages);
}

#[test]
fn test_undo_conversation_nothing_to_undo() {
    let mut app = create_test_app();
    // Clear any default history
    app.history.clear();
    app.api_messages_mut().clear();
    let result = undo_conversation(&mut app);
    assert!(result.message.is_some());
    let msg = result.message.unwrap();
    assert!(msg.contains("Nothing to undo") || msg.contains("Removed"));
}

#[test]
fn test_retry_with_previous_message() {
    let mut app = create_test_app();
    app.history.push(HistoryCell::User {
        content: "Test message".to_string(),
    });
    app.history.push(HistoryCell::Assistant {
        content: "Response".to_string(),
        streaming: false,
    });

    let result = retry(&mut app);
    assert!(result.message.is_some());
    let msg = result.message.unwrap();
    assert!(msg.contains("Retrying"));
    assert!(msg.contains("Test message"));
    assert!(matches!(
        result.action.as_ref(),
        Some(AppAction::ConversationUndo { retry_input: Some(input), .. }) if input == "Test message"
    ));
}

#[test]
fn test_retry_no_previous_message() {
    let mut app = create_test_app();
    let result = retry(&mut app);
    assert!(result.message.is_some());
    let msg = result.message.unwrap();
    assert!(msg.contains("No previous request to retry"));
    assert!(result.action.is_none());
}

#[test]
fn test_retry_truncates_long_input() {
    let mut app = create_test_app();
    let long_input = "x".repeat(100);
    app.history.push(HistoryCell::User {
        content: long_input.clone(),
    });
    app.history.push(HistoryCell::Assistant {
        content: "Response".to_string(),
        streaming: false,
    });

    let result = retry(&mut app);
    assert!(result.message.is_some());
    let msg = result.message.unwrap();
    assert!(msg.contains("Retrying"));
    assert!(msg.contains("..."));
}

#[test]
fn test_patch_undo_requests_session_resync_after_restore() {
    use crate::snapshot::SnapshotRepo;
    use crate::test_support::lock_test_env;
    use tempfile::tempdir;

    struct HomeGuard {
        prev: Option<std::ffi::OsString>,
        _lock: crate::test_support::TestEnvLock,
    }

    impl Drop for HomeGuard {
        fn drop(&mut self) {
            // SAFETY: process-wide lock still held.
            unsafe {
                match self.prev.take() {
                    Some(v) => std::env::set_var("HOME", v),
                    None => std::env::remove_var("HOME"),
                }
            }
        }
    }

    fn scoped_home(home: &std::path::Path) -> HomeGuard {
        let lock = lock_test_env();
        let prev = std::env::var_os("HOME");
        // SAFETY: serialized by the global env lock.
        unsafe {
            std::env::set_var("HOME", home);
        }
        HomeGuard { prev, _lock: lock }
    }

    let tmp = tempdir().unwrap();
    let workspace = tmp.path().join("ws");
    std::fs::create_dir_all(&workspace).unwrap();
    let _guard = scoped_home(tmp.path());

    let repo = SnapshotRepo::open_or_init(&workspace).unwrap();
    std::fs::write(workspace.join("a.txt"), b"original").unwrap();
    repo.snapshot_with_session("pre-turn:1", Some("test-session"))
        .unwrap();
    std::fs::write(workspace.join("a.txt"), b"modified").unwrap();
    repo.snapshot_with_session("post-turn:1", Some("test-session"))
        .unwrap();

    let mut app = create_test_app();
    app.workspace = workspace.clone();
    app.yolo = true;
    app.current_session_id = Some("test-session".to_string());
    app.api_messages_mut().push(Message {
        role: Role::User,
        content: vec![ContentBlock::Text {
            text: "please edit a.txt".to_string(),
            cache_control: None,
        }],
    });

    // A running turn owns the workspace: nothing is restored under it.
    app.is_loading = true;
    let refused = patch_undo(&mut app);
    assert!(
        refused
            .message
            .as_deref()
            .is_some_and(|message| message.contains("still running")),
        "{:?}",
        refused.message
    );
    assert!(refused.action.is_none());
    assert_eq!(
        std::fs::read(workspace.join("a.txt")).unwrap(),
        b"modified",
        "a refused undo changes no file"
    );
    app.is_loading = false;

    let result = patch_undo(&mut app);
    assert_eq!(std::fs::read(workspace.join("a.txt")).unwrap(), b"original");

    assert!(!result.is_error);
    assert!(matches!(
        result.action,
        Some(AppAction::SyncSession {
            ref messages,
            ref workspace,
            ..
        }) if messages.as_slice() == app.api_messages.as_slice()
            && workspace == &app.workspace
    ));
}

#[test]
fn test_undo_legacy_chain_falls_back_to_conversation_only() {
    use crate::snapshot::SnapshotRepo;
    use crate::test_support::lock_test_env;
    use tempfile::tempdir;

    struct HomeGuard {
        prev: Option<std::ffi::OsString>,
        _lock: crate::test_support::TestEnvLock,
    }

    impl Drop for HomeGuard {
        fn drop(&mut self) {
            // SAFETY: process-wide lock still held.
            unsafe {
                match self.prev.take() {
                    Some(v) => std::env::set_var("HOME", v),
                    None => std::env::remove_var("HOME"),
                }
            }
        }
    }

    fn scoped_home(home: &std::path::Path) -> HomeGuard {
        let lock = lock_test_env();
        let prev = std::env::var_os("HOME");
        // SAFETY: serialized by the global env lock.
        unsafe {
            std::env::set_var("HOME", home);
        }
        HomeGuard { prev, _lock: lock }
    }

    let tmp = tempdir().unwrap();
    let workspace = tmp.path().join("ws");
    std::fs::create_dir_all(&workspace).unwrap();
    let _guard = scoped_home(tmp.path());

    let repo = SnapshotRepo::open_or_init(&workspace).unwrap();
    let file = workspace.join("a.txt");
    std::fs::write(&file, b"zero").unwrap();
    repo.snapshot("tool:first").unwrap();
    std::fs::write(&file, b"one").unwrap();
    repo.snapshot("tool:second").unwrap();
    std::fs::write(&file, b"two").unwrap();

    let mut app = create_test_app();
    app.workspace = workspace.clone();
    app.current_session_id = Some("current-session".to_string());
    app.history.push(HistoryCell::User {
        content: "chat only".to_string(),
    });
    app.history.push(HistoryCell::Assistant {
        content: "reply".to_string(),
        streaming: false,
    });

    let result = super::execute("/undo", &mut app);
    assert!(!result.is_error);
    assert!(
        result
            .message
            .as_deref()
            .is_some_and(|m| m.contains("Removed")),
        "expected conversation fallback, got: {:?}",
        result.message
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "two");
}

#[test]
fn test_patch_undo_prunes_tool_turn_context() {
    use crate::snapshot::SnapshotRepo;
    use crate::test_support::lock_test_env;
    use tempfile::tempdir;

    struct HomeGuard {
        prev: Option<std::ffi::OsString>,
        _lock: crate::test_support::TestEnvLock,
    }

    impl Drop for HomeGuard {
        fn drop(&mut self) {
            // SAFETY: process-wide lock still held.
            unsafe {
                match self.prev.take() {
                    Some(v) => std::env::set_var("HOME", v),
                    None => std::env::remove_var("HOME"),
                }
            }
        }
    }

    fn scoped_home(home: &std::path::Path) -> HomeGuard {
        let lock = lock_test_env();
        let prev = std::env::var_os("HOME");
        // SAFETY: serialized by the global env lock.
        unsafe {
            std::env::set_var("HOME", home);
        }
        HomeGuard { prev, _lock: lock }
    }

    let tmp = tempdir().unwrap();
    let workspace = tmp.path().join("ws");
    std::fs::create_dir_all(&workspace).unwrap();
    let _guard = scoped_home(tmp.path());

    let repo = SnapshotRepo::open_or_init(&workspace).unwrap();
    let file = workspace.join("a.txt");
    std::fs::write(&file, b"alpha").unwrap();
    repo.snapshot_with_session("tool:call-1", Some("test-session"))
        .unwrap();
    std::fs::write(&file, b"alpha-fixed").unwrap();

    let mut app = create_test_app();
    app.workspace = workspace.clone();
    app.yolo = true;
    app.current_session_id = Some("test-session".to_string());
    app.history.push(HistoryCell::User {
        content: "please edit a.txt".to_string(),
    });
    app.history.push(HistoryCell::Assistant {
        content: "I will update the file.".to_string(),
        streaming: false,
    });
    app.history
        .push(HistoryCell::Tool(ToolCell::Generic(GenericToolCell {
            name: "write_file".to_string(),
            status: ToolStatus::Success,
            input_summary: Some("a.txt".to_string()),
            output: Some("updated".to_string()),
            prompts: None,
            spillover_path: None,
            output_summary: None,
            is_diff: false,
        })));
    app.history.push(HistoryCell::Assistant {
        content: "Done, file is fixed now.".to_string(),
        streaming: false,
    });
    app.tool_cells.insert("call-1".to_string(), 2);

    app.api_messages_mut().push(Message {
        role: Role::User,
        content: vec![ContentBlock::Text {
            text: "please edit a.txt".to_string(),
            cache_control: None,
        }],
    });
    app.api_messages_mut().push(Message {
        role: Role::Assistant,
        content: vec![
            ContentBlock::Text {
                text: "I will update the file.".to_string(),
                cache_control: None,
            },
            ContentBlock::ToolUse {
                execution_id: None,
                id: "call-1".to_string(),
                name: "write_file".to_string(),
                input: serde_json::json!({"path": "a.txt"}),
                caller: None,
                thought_signature: None,
            },
        ],
    });
    app.api_messages_mut().push(Message {
        role: Role::User,
        content: vec![ContentBlock::ToolResult {
            execution_id: None,
            tool_use_id: "call-1".to_string(),
            content: "updated".to_string(),
            is_error: None,
            content_blocks: None,
        }],
    });
    app.api_messages_mut().push(Message {
        role: Role::Assistant,
        content: vec![ContentBlock::Text {
            text: "Done, file is fixed now.".to_string(),
            cache_control: None,
        }],
    });

    let result = patch_undo(&mut app);

    assert!(!result.is_error);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "alpha");
    assert_eq!(app.history.len(), 3);
    assert!(matches!(
        app.history.last(),
        Some(HistoryCell::System { content }) if content.contains("/undo reverted workspace")
    ));
    assert_eq!(app.api_messages.len(), 2);
    assert!(matches!(
        &app.api_messages[0].content[0],
        ContentBlock::Text { text, .. } if text == "please edit a.txt"
    ));
    assert_eq!(app.api_messages[1].content.len(), 1);
    assert!(matches!(
        &app.api_messages[1].content[0],
        ContentBlock::Text { text, .. } if text == "I will update the file."
    ));
}

#[test]
fn test_patch_undo_prunes_pre_turn_context() {
    use crate::snapshot::SnapshotRepo;
    use crate::test_support::lock_test_env;
    use tempfile::tempdir;

    struct HomeGuard {
        prev: Option<std::ffi::OsString>,
        _lock: crate::test_support::TestEnvLock,
    }

    impl Drop for HomeGuard {
        fn drop(&mut self) {
            // SAFETY: process-wide lock still held.
            unsafe {
                match self.prev.take() {
                    Some(v) => std::env::set_var("HOME", v),
                    None => std::env::remove_var("HOME"),
                }
            }
        }
    }

    fn scoped_home(home: &std::path::Path) -> HomeGuard {
        let lock = lock_test_env();
        let prev = std::env::var_os("HOME");
        // SAFETY: serialized by the global env lock.
        unsafe {
            std::env::set_var("HOME", home);
        }
        HomeGuard { prev, _lock: lock }
    }

    let tmp = tempdir().unwrap();
    let workspace = tmp.path().join("ws");
    std::fs::create_dir_all(&workspace).unwrap();
    let _guard = scoped_home(tmp.path());

    let repo = SnapshotRepo::open_or_init(&workspace).unwrap();
    let file = workspace.join("a.txt");
    std::fs::write(&file, b"alpha").unwrap();
    repo.snapshot_with_session("pre-turn:1", Some("test-session"))
        .unwrap();
    std::fs::write(&file, b"alpha-fixed").unwrap();

    let mut app = create_test_app();
    app.workspace = workspace.clone();
    app.yolo = true;
    app.current_session_id = Some("test-session".to_string());
    app.history.push(HistoryCell::User {
        content: "please edit a.txt".to_string(),
    });
    app.history.push(HistoryCell::Assistant {
        content: "Done, file is fixed now.".to_string(),
        streaming: false,
    });
    app.api_messages_mut().push(Message {
        role: Role::User,
        content: vec![ContentBlock::Text {
            text: "please edit a.txt".to_string(),
            cache_control: None,
        }],
    });
    app.api_messages_mut().push(Message {
        role: Role::Assistant,
        content: vec![ContentBlock::Text {
            text: "Done, file is fixed now.".to_string(),
            cache_control: None,
        }],
    });

    let result = patch_undo(&mut app);

    assert!(!result.is_error);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "alpha");
    assert_eq!(app.history.len(), 1);
    assert!(matches!(
        app.history.last(),
        Some(HistoryCell::System { content }) if content.contains("/undo reverted workspace")
    ));
    assert!(app.api_messages.is_empty());
}

#[test]
fn test_prune_undone_tool_context_preserves_prior_tool_pairs() {
    let mut app = create_test_app();
    app.history.push(HistoryCell::User {
        content: "edit two files".to_string(),
    });
    app.history.push(HistoryCell::Assistant {
        content: "I will update both files.".to_string(),
        streaming: false,
    });
    app.history
        .push(HistoryCell::Tool(ToolCell::Generic(GenericToolCell {
            name: "write_file".to_string(),
            status: ToolStatus::Success,
            input_summary: Some("a.txt".to_string()),
            output: Some("updated a".to_string()),
            prompts: None,
            spillover_path: None,
            output_summary: None,
            is_diff: false,
        })));
    app.history
        .push(HistoryCell::Tool(ToolCell::Generic(GenericToolCell {
            name: "write_file".to_string(),
            status: ToolStatus::Success,
            input_summary: Some("b.txt".to_string()),
            output: Some("updated b".to_string()),
            prompts: None,
            spillover_path: None,
            output_summary: None,
            is_diff: false,
        })));
    app.history.push(HistoryCell::Assistant {
        content: "Done.".to_string(),
        streaming: false,
    });
    app.tool_cells.insert("call-a".to_string(), 2);
    app.tool_cells.insert("call-b".to_string(), 3);

    app.api_messages_mut().push(Message {
        role: Role::User,
        content: vec![ContentBlock::Text {
            text: "edit two files".to_string(),
            cache_control: None,
        }],
    });
    app.api_messages_mut().push(Message {
        role: Role::Assistant,
        content: vec![
            ContentBlock::Text {
                text: "I will update both files.".to_string(),
                cache_control: None,
            },
            ContentBlock::ToolUse {
                execution_id: None,
                id: "call-a".to_string(),
                name: "write_file".to_string(),
                input: serde_json::json!({"path": "a.txt"}),
                caller: None,
                thought_signature: None,
            },
            ContentBlock::ToolUse {
                execution_id: None,
                id: "call-b".to_string(),
                name: "write_file".to_string(),
                input: serde_json::json!({"path": "b.txt"}),
                caller: None,
                thought_signature: None,
            },
        ],
    });
    app.api_messages_mut().push(Message {
        role: Role::User,
        content: vec![ContentBlock::ToolResult {
            execution_id: None,
            tool_use_id: "call-a".to_string(),
            content: "updated a".to_string(),
            is_error: None,
            content_blocks: None,
        }],
    });
    app.api_messages_mut().push(Message {
        role: Role::User,
        content: vec![ContentBlock::ToolResult {
            execution_id: None,
            tool_use_id: "call-b".to_string(),
            content: "updated b".to_string(),
            is_error: None,
            content_blocks: None,
        }],
    });
    app.api_messages_mut().push(Message {
        role: Role::Assistant,
        content: vec![ContentBlock::Text {
            text: "Done.".to_string(),
            cache_control: None,
        }],
    });

    prune_undone_tool_context(&mut app, "call-b");

    assert_eq!(app.history.len(), 3);
    assert_eq!(app.api_messages.len(), 3);
    assert!(matches!(
        &app.api_messages[1].content[..],
        [
            ContentBlock::Text { .. },
            ContentBlock::ToolUse { id, ..}
        ] if id == "call-a"
    ));
    assert!(matches!(
        &app.api_messages[2].content[0],
        ContentBlock::ToolResult { tool_use_id, .. } if tool_use_id == "call-a"
    ));
}

#[test]
fn undo_uses_execution_identity_and_preserves_coalesced_result_stamp() {
    let mut app = create_test_app();
    let stamp = chrono::DateTime::parse_from_rfc3339("2026-01-02T03:04:05Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    let messages: Vec<Message> = serde_json::from_value(serde_json::json!([
        {"role":"assistant","content":[
            {"type":"tool_use","id":"wire","execution_id":"first","name":"write_file","input":{"path":"a.txt"}},
            {"type":"tool_use","id":"wire","execution_id":"second","name":"write_file","input":{"path":"b.txt"}}
        ]},
        {"role":"user","content":[
            {"type":"tool_result","tool_use_id":"wire","execution_id":"first","content":"kept"},
            {"type":"tool_result","tool_use_id":"wire","execution_id":"second","content":"undone"}
        ]}
    ])).unwrap();
    for message in messages {
        app.push_api_message_stamped(message, stamp);
    }
    prune_undone_tool_context(&mut app, "second");
    assert_eq!(app.api_messages.len(), 2);
    assert_eq!(app.api_messages[0].content.len(), 1);
    assert!(
        matches!(&app.api_messages[1].content[..], [ContentBlock::ToolResult {
        execution_id: Some(id), tool_use_id, content, ..
    }] if id == "first" && tool_use_id == "wire" && content == "kept")
    );
    assert_eq!(app.api_messages_stamped().nth(1).unwrap().1, stamp);

    // A legacy provider ID that happens to spell a local ID is not another
    // spelling for that execution. With both present, the raw undo request is
    // ambiguous and must leave every message intact.
    app.push_api_message_stamped(
        serde_json::from_value(serde_json::json!({
            "role":"assistant","content":[
                {"type":"tool_use","id":"first","name":"write_file","input":{}}
            ]
        }))
        .unwrap(),
        stamp,
    );
    let before = serde_json::to_value(&*app.api_messages).unwrap();
    prune_undone_tool_context(&mut app, "first");
    assert_eq!(serde_json::to_value(&*app.api_messages).unwrap(), before);
}

// ── /cache stats tests ──────────────────────────────────────────────

#[test]
fn test_patch_undo_refuses_outside_trusted_mode() {
    use crate::snapshot::SnapshotRepo;
    use crate::test_support::lock_test_env;
    use tempfile::tempdir;

    struct HomeGuard {
        prev: Option<std::ffi::OsString>,
        _lock: crate::test_support::TestEnvLock,
    }
    impl Drop for HomeGuard {
        fn drop(&mut self) {
            // SAFETY: process-wide lock still held.
            unsafe {
                match self.prev.take() {
                    Some(v) => std::env::set_var("HOME", v),
                    None => std::env::remove_var("HOME"),
                }
            }
        }
    }
    fn scoped_home(home: &std::path::Path) -> HomeGuard {
        let lock = lock_test_env();
        let prev = std::env::var_os("HOME");
        // SAFETY: serialised by the global env lock.
        unsafe {
            std::env::set_var("HOME", home);
        }
        HomeGuard { prev, _lock: lock }
    }

    let tmp = tempdir().unwrap();
    let workspace = tmp.path().join("ws");
    std::fs::create_dir_all(&workspace).unwrap();
    let _guard = scoped_home(tmp.path());

    let repo = SnapshotRepo::open_or_init(&workspace).unwrap();
    std::fs::write(workspace.join("a.txt"), b"original").unwrap();
    repo.snapshot_with_session("pre-turn:1", Some("test-session"))
        .unwrap();
    std::fs::write(workspace.join("a.txt"), b"modified").unwrap();

    // yolo/trust_mode stay false (create_test_app defaults).
    let mut app = create_test_app();
    app.workspace = workspace.clone();
    app.current_session_id = Some("test-session".to_string());

    let result = patch_undo(&mut app);
    assert!(!result.is_error);
    assert!(
        result
            .message
            .as_deref()
            .is_some_and(|m| m.contains("Refusing to undo workspace files")),
        "expected refusal message, got: {:?}",
        result.message
    );
    // Workspace must be untouched by the gate.
    assert_eq!(
        std::fs::read_to_string(workspace.join("a.txt")).unwrap(),
        "modified"
    );
}

#[test]
fn test_patch_undo_never_crosses_session_boundary() {
    use crate::snapshot::SnapshotRepo;
    use crate::test_support::lock_test_env;
    use tempfile::tempdir;

    struct HomeGuard {
        prev: Option<std::ffi::OsString>,
        _lock: crate::test_support::TestEnvLock,
    }
    impl Drop for HomeGuard {
        fn drop(&mut self) {
            // SAFETY: process-wide lock still held.
            unsafe {
                match self.prev.take() {
                    Some(v) => std::env::set_var("HOME", v),
                    None => std::env::remove_var("HOME"),
                }
            }
        }
    }
    fn scoped_home(home: &std::path::Path) -> HomeGuard {
        let lock = lock_test_env();
        let prev = std::env::var_os("HOME");
        // SAFETY: serialised by the global env lock.
        unsafe {
            std::env::set_var("HOME", home);
        }
        HomeGuard { prev, _lock: lock }
    }

    let tmp = tempdir().unwrap();
    let workspace = tmp.path().join("ws");
    std::fs::create_dir_all(&workspace).unwrap();
    let _guard = scoped_home(tmp.path());

    let repo = SnapshotRepo::open_or_init(&workspace).unwrap();
    let file = workspace.join("a.txt");

    // Session A: an earlier conversation that modified the workspace.
    std::fs::write(&file, b"a-before").unwrap();
    repo.snapshot_with_session("pre-turn:1", Some("session-a"))
        .unwrap();
    std::fs::write(&file, b"a-after").unwrap();

    // Session B (current): a later conversation that also modified it.
    std::fs::write(&file, b"b-before").unwrap();
    repo.snapshot_with_session("pre-turn:1", Some("session-b"))
        .unwrap();
    std::fs::write(&file, b"b-after").unwrap();

    let mut app = create_test_app();
    app.workspace = workspace.clone();
    app.yolo = true;
    app.current_session_id = Some("session-b".to_string());

    let result = patch_undo(&mut app);
    assert!(!result.is_error);
    // Must restore session B's pre-turn state — never session A's.
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "b-before");

    let repeated = patch_undo(&mut app);
    assert!(!repeated.is_error);
    assert!(
        repeated
            .message
            .as_deref()
            .is_some_and(|m| m.contains("No undoable snapshot")),
        "repeated undo must stop at the session boundary: {:?}",
        repeated.message
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "b-before");
}

/// `/undo` used to report only `Removed N message(s)` when the snapshot repo
/// could not be opened, implying a file rollback that never happened. The
/// conversation-only fallback must say so, and say why.
#[test]
fn test_undo_reports_that_files_were_not_reverted_when_the_repo_is_unavailable() {
    use crate::test_support::{EnvVarGuard, lock_test_env};
    use tempfile::tempdir;

    let _lock = lock_test_env();
    let tmp = tempdir().unwrap();
    let _home = EnvVarGuard::set("HOME", tmp.path());
    let _profile = EnvVarGuard::set("USERPROFILE", tmp.path());

    // The home directory itself is refused by the snapshot safety gate, so
    // `patch_undo` cannot open a repo at all.
    let mut app = create_test_app();
    app.workspace = tmp.path().to_path_buf();
    app.current_session_id = Some("test-session".to_string());
    app.yolo = true;
    app.history.push(HistoryCell::User {
        content: "change something".to_string(),
    });
    app.api_messages_mut().push(Message {
        role: Role::User,
        content: vec![ContentBlock::Text {
            text: "change something".to_string(),
            cache_control: None,
        }],
    });

    let result = super::execute("/undo", &mut app);
    let message = result.message.as_deref().unwrap_or_default();
    assert!(
        message.contains("Removed 1 message(s)"),
        "conversation undo still runs: {message}"
    );
    assert!(
        message.contains(undo::FILES_NOT_REVERTED_NOTE),
        "the user must be told files were not reverted: {message}"
    );
    assert!(
        message.contains(undo::SNAPSHOT_REPO_UNAVAILABLE_PREFIX),
        "the reason must travel with the fallback: {message}"
    );
}

/// Isolated HOME + workspace for the `/undo` restore tests (#6644).
// Fields drop in order: the env guards restore before the lock releases.
struct UndoFixture {
    workspace: PathBuf,
    repo: crate::snapshot::SnapshotRepo,
    _tmp: tempfile::TempDir,
    _codewhale_home: crate::test_support::EnvVarGuard,
    _profile: crate::test_support::EnvVarGuard,
    _home: crate::test_support::EnvVarGuard,
    _lock: crate::test_support::TestEnvLock,
}

impl UndoFixture {
    fn new() -> Self {
        use crate::test_support::{EnvVarGuard, lock_test_env};
        let lock = lock_test_env();
        let tmp = tempfile::tempdir().unwrap();
        let home = EnvVarGuard::set("HOME", tmp.path());
        let profile = EnvVarGuard::set("USERPROFILE", tmp.path());
        let codewhale_home = EnvVarGuard::remove("CODEWHALE_HOME");
        let workspace = tmp.path().join("ws");
        std::fs::create_dir_all(&workspace).unwrap();
        let repo = crate::snapshot::SnapshotRepo::open_or_init(&workspace).unwrap();
        Self {
            workspace,
            repo,
            _tmp: tmp,
            _codewhale_home: codewhale_home,
            _profile: profile,
            _home: home,
            _lock: lock,
        }
    }

    fn write(&self, name: &str, body: &str) {
        std::fs::write(self.workspace.join(name), body).unwrap();
    }

    fn read(&self, name: &str) -> String {
        std::fs::read_to_string(self.workspace.join(name)).unwrap()
    }

    fn snapshot(&self, label: &str, session: &str) {
        self.repo.take_snapshot(label, Some(session)).unwrap();
    }

    fn app(&self, session: &str) -> App {
        let mut app = create_test_app();
        app.workspace = self.workspace.clone();
        app.yolo = true;
        app.current_session_id = Some(session.to_string());
        app
    }
}

/// `/undo` restores only the paths the undone turn changed: an edit the user
/// made to another file after the turn survives. The whole-tree restore
/// reverted it too.
#[test]
fn patch_undo_restores_only_the_paths_the_undone_step_changed() {
    let fx = UndoFixture::new();
    fx.write("a.txt", "a0");
    fx.write("b.txt", "b0");
    fx.snapshot("pre-turn:1", "s1");
    fx.write("a.txt", "a1");
    fx.write("new.txt", "created by the turn");
    fx.snapshot("post-turn:1", "s1");
    // The user's own edit after the turn, and a file they created.
    fx.write("b.txt", "b-user");
    fx.write("mine.txt", "user file");

    let mut app = fx.app("s1");
    let result = patch_undo(&mut app);

    assert!(!result.is_error, "{:?}", result.message);
    assert_eq!(fx.read("a.txt"), "a0");
    assert!(!fx.workspace.join("new.txt").exists());
    assert_eq!(fx.read("b.txt"), "b-user");
    assert_eq!(fx.read("mine.txt"), "user file");
    let message = result.message.unwrap_or_default();
    assert!(
        message.contains("modified a.txt") && message.contains("removed new.txt"),
        "{message}"
    );
}

/// A path the undone step changed that changed again since is refused, not
/// overwritten, and nothing else is touched.
#[test]
fn patch_undo_refuses_when_a_changed_path_changed_since() {
    let fx = UndoFixture::new();
    fx.write("a.txt", "a0");
    fx.write("b.txt", "b0");
    fx.snapshot("pre-turn:1", "s1");
    fx.write("a.txt", "a1");
    fx.write("b.txt", "b1");
    fx.snapshot("post-turn:1", "s1");
    fx.write("a.txt", "a-user");

    let mut app = fx.app("s1");
    let result = super::execute("/undo", &mut app);

    let message = result.message.unwrap_or_default();
    assert!(
        message.contains("Refusing to undo snapshot") && message.contains("a.txt"),
        "{message}"
    );
    assert_eq!(fx.read("a.txt"), "a-user");
    assert_eq!(fx.read("b.txt"), "b1");
}

/// `/undo` keeps stepping back one tool call at a time (#384), each step
/// restoring only what that call changed.
#[test]
fn patch_undo_steps_back_one_tool_call_at_a_time() {
    let fx = UndoFixture::new();
    fx.write("a.txt", "a0");
    fx.snapshot("pre-turn:1", "s1");
    fx.snapshot("tool:call-1", "s1");
    fx.write("a.txt", "a1");
    fx.snapshot("tool:call-2", "s1");
    fx.write("a.txt", "a2");
    fx.write("b.txt", "b2");
    fx.snapshot("post-turn:1", "s1");

    let mut app = fx.app("s1");
    let first = patch_undo(&mut app);
    assert!(!first.is_error, "{:?}", first.message);
    assert_eq!(fx.read("a.txt"), "a1");
    assert!(!fx.workspace.join("b.txt").exists());

    let second = patch_undo(&mut app);
    assert!(!second.is_error, "{:?}", second.message);
    assert_eq!(fx.read("a.txt"), "a0");

    let third = patch_undo(&mut app);
    assert!(
        third
            .message
            .as_deref()
            .is_some_and(|m| m.starts_with("No undoable snapshot")),
        "{:?}",
        third.message
    );
}

/// Restore points older than the newest 100 snapshots are still found.
#[test]
fn patch_undo_finds_restore_points_beyond_the_newest_hundred_snapshots() {
    let fx = UndoFixture::new();
    fx.write("a.txt", "a0");
    fx.snapshot("pre-turn:1", "s1");
    fx.write("a.txt", "a1");
    fx.snapshot("post-turn:1", "s1");
    for i in 0..101 {
        fx.repo
            .take_snapshot(&format!("tool:other-{i}"), Some("other-session"))
            .unwrap();
    }

    let mut app = fx.app("s1");
    let result = patch_undo(&mut app);

    assert!(!result.is_error, "{:?}", result.message);
    assert_eq!(fx.read("a.txt"), "a0", "{:?}", result.message);
}

/// A fork owns the restore points of the turns it inherited, up to the fork,
/// and none its source took afterwards.
#[test]
fn patch_undo_restores_turns_a_fork_inherited() {
    let fx = UndoFixture::new();
    fx.write("a.txt", "a0");
    fx.snapshot("pre-turn:1", "source");
    fx.write("a.txt", "a1");
    fx.snapshot("post-turn:1", "source");

    let fork = |created_at: chrono::DateTime<chrono::Utc>| {
        let mut app = fx.app("fork");
        let mut metadata =
            crate::session_manager::create_saved_session(&[], "model", &fx.workspace, 0, None)
                .metadata;
        metadata.id = "fork".to_string();
        metadata.parent_session_id = Some("source".to_string());
        metadata.created_at = created_at;
        app.current_session_metadata = Some(metadata);
        app
    };

    let owners = super::contract::debug_operations::snapshot_owners(&fork(chrono::Utc::now()));
    assert_eq!(owners.len(), 2);
    assert_eq!(owners[1].session_id, "source");

    // Forked before the source took these snapshots: they are not the fork's.
    let mut early = fork(chrono::Utc::now() - chrono::Duration::hours(1));
    let refused = patch_undo(&mut early);
    assert!(
        refused
            .message
            .as_deref()
            .is_some_and(|m| m.starts_with("No undoable snapshot")),
        "{:?}",
        refused.message
    );
    assert_eq!(fx.read("a.txt"), "a1");

    let mut app = fork(chrono::Utc::now() + chrono::Duration::seconds(5));
    let result = patch_undo(&mut app);
    assert!(!result.is_error, "{:?}", result.message);
    assert_eq!(fx.read("a.txt"), "a0", "{:?}", result.message);
}

/// `/undo` typed while the post-turn snapshot is still being written waits
/// for it (#6644). Before, the two raced on the side repo: the post-turn
/// snapshot landed after the undo and recorded the reverted workspace as
/// the turn's end, and the undone step ended at "now".
#[test]
fn patch_undo_waits_for_a_pending_post_turn_snapshot() {
    let fx = UndoFixture::new();
    fx.write("a.txt", "a0");
    fx.snapshot("pre-turn:1", "s1");
    fx.snapshot("tool:call-1", "s1");
    fx.write("a.txt", "a1");

    // The turn has completed; its post-turn snapshot is still in flight.
    let pending = crate::snapshot::PendingPostTurnSnapshot::reserve();
    let writer = {
        let repo = crate::snapshot::SnapshotRepo::open_or_init(&fx.workspace).unwrap();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(500));
            let taken = repo.take_snapshot("post-turn:1", Some("s1")).unwrap();
            drop(pending);
            taken
        })
    };

    let mut app = fx.app("s1");
    let result = patch_undo(&mut app);
    let post_turn = writer.join().unwrap();

    assert!(!result.is_error, "{:?}", result.message);
    assert_eq!(fx.read("a.txt"), "a0", "{:?}", result.message);
    let tool = fx
        .repo
        .list(usize::MAX)
        .unwrap()
        .into_iter()
        .find(|snapshot| snapshot.label == "tool:call-1")
        .unwrap();
    assert_eq!(
        fx.repo
            .changed_paths_between(&tool.tree, &post_turn.tree)
            .unwrap(),
        vec![PathBuf::from("a.txt")],
        "the post-turn snapshot must record the turn's end, not the undone workspace"
    );
}

/// A step that changed a symlink restores its regular files and reports the
/// symlink, and it does not block older steps.
#[cfg(unix)]
#[test]
fn patch_undo_skips_non_regular_paths_without_blocking_older_steps() {
    let fx = UndoFixture::new();
    fx.write("a.txt", "a0");
    fx.snapshot("pre-turn:1", "s1");
    fx.write("a.txt", "a1");
    fx.snapshot("pre-turn:2", "s1");
    fx.write("a.txt", "a2");
    std::os::unix::fs::symlink("a.txt", fx.workspace.join("current")).unwrap();
    fx.snapshot("post-turn:2", "s1");

    let mut app = fx.app("s1");
    let first = patch_undo(&mut app);
    assert!(!first.is_error, "{:?}", first.message);
    assert_eq!(fx.read("a.txt"), "a1");
    let message = first.message.unwrap_or_default();
    assert!(
        message.contains("Left in place") && message.contains("current"),
        "{message}"
    );
    assert!(
        std::fs::symlink_metadata(fx.workspace.join("current"))
            .unwrap()
            .file_type()
            .is_symlink()
    );

    let second = patch_undo(&mut app);
    assert!(!second.is_error, "{:?}", second.message);
    assert_eq!(fx.read("a.txt"), "a0", "{:?}", second.message);
}

/// Outside trusted mode `/undo` refuses before writing anything: planning
/// the newest step does not add a snapshot to the side repo.
#[test]
fn patch_undo_outside_trusted_mode_writes_no_snapshot() {
    let fx = UndoFixture::new();
    fx.write("a.txt", "a0");
    fx.snapshot("pre-turn:1", "s1");
    fx.write("a.txt", "a1");
    let before = fx.repo.list(usize::MAX).unwrap().len();

    let mut app = fx.app("s1");
    app.yolo = false;
    app.trust_mode = false;
    let result = patch_undo(&mut app);

    assert!(
        result
            .message
            .as_deref()
            .is_some_and(|m| m.starts_with("Refusing to undo workspace files outside trusted mode")),
        "{:?}",
        result.message
    );
    assert_eq!(fx.repo.list(usize::MAX).unwrap().len(), before);
    assert_eq!(fx.read("a.txt"), "a1");
}

#[test]
fn receipts_command_is_registered_and_reads_the_transcript() {
    assert_eq!(
        crate::commands::get_command_info("receipts").map(|info| info.name),
        Some("receipts")
    );
    assert_eq!(
        crate::commands::get_command_info("receipt").map(|info| info.name),
        Some("receipts")
    );
    let mut app = create_test_app();
    app.current_session_id = None;
    let empty = crate::commands::execute("/receipts", &mut app);
    assert!(!empty.is_error, "{:?}", empty.message);
    assert!(
        empty
            .message
            .as_deref()
            .is_some_and(|text| text.contains("No actions recorded.")),
        "{:?}",
        empty.message
    );

    app.api_messages_mut().push(Message {
        role: Role::User,
        content: vec![ContentBlock::Text {
            text: "run the tests".to_string(),
            cache_control: None,
        }],
    });
    app.api_messages_mut().push(Message {
        role: Role::Assistant,
        content: vec![ContentBlock::ToolUse {
            execution_id: None,
            id: "call-1".to_string(),
            name: "bash".to_string(),
            input: serde_json::json!({"command": "cargo test"}),
            caller: None,
            thought_signature: None,
        }],
    });
    app.api_messages_mut().push(Message {
        role: Role::User,
        content: vec![ContentBlock::ToolResult {
            execution_id: None,
            tool_use_id: "call-1".to_string(),
            content: "ok".to_string(),
            is_error: None,
            content_blocks: None,
        }],
    });
    let listed = crate::commands::execute("/receipts", &mut app);
    let text = listed.message.expect("receipt text");
    assert!(text.contains("Ran 1 command"), "{text}");
    assert!(text.contains("1. ran `cargo test`"), "{text}");

    // `$`, `*`, and `_` in a command must reach the note cell as JSON, not
    // as math or emphasis.
    let shell = r#"echo "$HOME" && echo $PATH *_x_*"#;
    app.api_messages_mut().push(Message {
        role: Role::Assistant,
        content: vec![ContentBlock::ToolUse {
            execution_id: None,
            id: "call-2".to_string(),
            name: "bash".to_string(),
            input: serde_json::json!({ "command": shell }),
            caller: None,
            thought_signature: None,
        }],
    });
    app.api_messages_mut().push(Message {
        role: Role::User,
        content: vec![ContentBlock::ToolResult {
            execution_id: None,
            tool_use_id: "call-2".to_string(),
            content: "ok".to_string(),
            is_error: None,
            content_blocks: None,
        }],
    });
    let json = crate::commands::execute("/receipts json", &mut app);
    let fenced = json.message.expect("json");
    let body = fenced
        .strip_prefix("```json\n")
        .and_then(|rest| rest.strip_suffix("\n```"))
        .expect("the JSON is fenced");
    let value: serde_json::Value = serde_json::from_str(body).expect("valid json");
    assert_eq!(value["totals"]["commands"], 2);
    let rendered: String = HistoryCell::System { content: fenced }
        .lines(400)
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains(r#""command": "echo \"$HOME\" && echo $PATH *_x_*""#),
        "{rendered}"
    );
    let bad = crate::commands::execute("/receipts nope", &mut app);
    assert!(bad.is_error);
}

#[test]
fn whole_debug_registry_matches_portable_inventory_and_exact_host_authority() {
    use codewhale_command_contract::handler::{
        CommandCapabilities as Caps, CommandHandler, ContextParts,
    };
    let mut app = create_test_app();
    let portable = super::groups::debug::portable_handlers();
    assert_eq!(portable.len(), 14);
    for (info, portable_handler) in portable {
        for spelling in std::iter::once(info.name).chain(info.aliases.iter().copied()) {
            let registered = super::registry()
                .get(spelling)
                .expect("registered debug command");
            assert_eq!(registered.info().name, info.name);
            assert_eq!(registered.info().aliases, info.aliases);
            assert_eq!(registered.info().usage, info.usage);
            let handler = registered
                .contextual_handler()
                .expect("portable host registration");
            match (portable_handler.clone(), handler) {
                (CommandHandler::Pure(_), CommandHandler::Pure(_)) => {
                    assert_eq!(info.name, "preview-request")
                }
                (
                    CommandHandler::Contextual {
                        capabilities: expected,
                        ..
                    },
                    CommandHandler::Contextual { capabilities, .. },
                ) => {
                    assert_eq!(capabilities, expected, "/{spelling}");
                    let mut bundle = app.command_contexts();
                    let ContextParts {
                        session,
                        model,
                        cost,
                        mode_policy,
                        system_prompt,
                        skills,
                        workspace,
                        presentation,
                        media,
                        memory,
                        project,
                        skill_group,
                        plugin,
                        lifecycle,
                        control,
                        export,
                        structcopy,
                        debug_diagnostics,
                        debug_receipts,
                        debug_change,
                        debug_history,
                        debug_diff,
                        debug_undo,
                        permissions,
                        config_status,
                    } = bundle.contexts(capabilities).into_parts();
                    assert!(permissions.is_none() && config_status.is_none());
                    for (name, present, capability) in [
                        ("session", session.is_some(), Caps::SESSION),
                        ("model", model.is_some(), Caps::MODEL),
                        ("cost", cost.is_some(), Caps::COST),
                        ("mode_policy", mode_policy.is_some(), Caps::MODE_POLICY),
                        (
                            "system_prompt",
                            system_prompt.is_some(),
                            Caps::SYSTEM_PROMPT,
                        ),
                        ("skills", skills.is_some(), Caps::SKILLS),
                        ("workspace", workspace.is_some(), Caps::WORKSPACE),
                        ("presentation", presentation.is_some(), Caps::PRESENTATION),
                        ("media", media.is_some(), Caps::MEDIA),
                        ("memory", memory.is_some(), Caps::MEMORY),
                        ("project", project.is_some(), Caps::PROJECT),
                        ("skill_group", skill_group.is_some(), Caps::SKILL_GROUP),
                        ("plugin", plugin.is_some(), Caps::PLUGIN),
                        ("lifecycle", lifecycle.is_some(), Caps::SESSION_LIFECYCLE),
                        ("control", control.is_some(), Caps::SESSION_CONTROL),
                        ("export", export.is_some(), Caps::SESSION_EXPORT),
                        ("structcopy", structcopy.is_some(), Caps::SESSION_STRUCTCOPY),
                        (
                            "debug_diagnostics",
                            debug_diagnostics.is_some(),
                            Caps::DEBUG_DIAGNOSTICS,
                        ),
                        (
                            "debug_receipts",
                            debug_receipts.is_some(),
                            Caps::DEBUG_RECEIPTS,
                        ),
                        ("debug_change", debug_change.is_some(), Caps::DEBUG_CHANGE),
                        (
                            "debug_history",
                            debug_history.is_some(),
                            Caps::DEBUG_HISTORY,
                        ),
                        ("debug_diff", debug_diff.is_some(), Caps::DEBUG_DIFF),
                        ("debug_undo", debug_undo.is_some(), Caps::DEBUG_UNDO),
                    ] {
                        assert_eq!(
                            present,
                            capabilities.contains(capability),
                            "/{spelling}: {name}"
                        );
                    }
                }
                _ => panic!("host changed /{spelling} handler shape"),
            }
        }
    }
}
