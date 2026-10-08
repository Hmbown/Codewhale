//! Portable regression evidence for the expanded debug group. Fake facets
//! record authority and ordering; all handlers and renderers are production code.

use super::{DebugAction, portable_handlers, receipts, undo};
use codewhale_command_contract::facets::*;
use codewhale_command_contract::handler::{
    CommandCapabilities as Caps, CommandContexts, CommandHandler,
};
use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn diff_formats_observations_without_host_state_or_actions() {
    struct Diff(DebugDiffObservation);
    impl CommandDebugDiffContext for Diff {
        fn diff(&self) -> DebugDiffObservation {
            self.0.clone()
        }
    }
    for (observation, message, error) in [
        (
            DebugDiffObservation::GitUnavailable,
            "Error: git not found on PATH",
            true,
        ),
        (
            DebugDiffObservation::Failed("fixture I/O".into()),
            "Git diff failed — is this a git repository?\nfixture I/O",
            false,
        ),
        (
            DebugDiffObservation::Output {
                names: " \n".into(),
                stat: "ignored".into(),
            },
            "No changes since session start",
            false,
        ),
        (
            DebugDiffObservation::Output {
                names: "a.rs\n\nb.rs\n".into(),
                stat: "".into(),
            },
            "Changed files (2):\na.rs\nb.rs",
            false,
        ),
        (
            DebugDiffObservation::Output {
                names: "a -> b\nc.rs\n".into(),
                stat: " 2 files changed\n".into(),
            },
            "Changed files (2, 1 renamed):\na -> b\nc.rs\n\n── Stat ──\n2 files changed",
            false,
        ),
    ] {
        let result = undo::diff(
            CommandContexts::empty().with_debug_diff(&mut Diff(observation)),
            None,
        );
        assert_eq!(result.message.as_deref(), Some(message));
        assert_eq!(result.is_error, error);
        assert!(result.action.is_none());
    }
}

#[test]
fn change_checks_presentation_before_reading_the_host_projection() {
    struct NoRead;
    impl CommandDebugChangeContext for NoRead {
        fn change_projection(&self) -> DebugChangeProjection {
            panic!("missing capability must prevent host reads")
        }
    }
    let result = super::change::change(
        CommandContexts::empty().with_debug_change(&mut NoRead),
        None,
    );
    assert_eq!(
        result.message.as_deref(),
        Some("Error: Command capability unavailable: presentation")
    );
    assert!(result.is_error);
    assert!(result.action.is_none());
}

#[test]
fn whole_group_has_exact_authority_and_safe_missing_facets() {
    let expected = [
        ("tokens", Caps::DEBUG_DIAGNOSTICS | Caps::PRESENTATION),
        ("cost", Caps::DEBUG_DIAGNOSTICS | Caps::PRESENTATION),
        ("receipts", Caps::DEBUG_RECEIPTS),
        ("balance", Caps::DEBUG_DIAGNOSTICS),
        ("cache", Caps::DEBUG_DIAGNOSTICS | Caps::PRESENTATION),
        ("preview-request", Caps::NONE),
        ("tools", Caps::DEBUG_DIAGNOSTICS),
        ("change", Caps::DEBUG_CHANGE | Caps::PRESENTATION),
        ("system", Caps::DEBUG_DIAGNOSTICS),
        ("context", Caps::DEBUG_DIAGNOSTICS),
        ("edit", Caps::DEBUG_HISTORY),
        ("diff", Caps::DEBUG_DIFF),
        ("undo", Caps::DEBUG_UNDO | Caps::DEBUG_HISTORY),
        ("retry", Caps::DEBUG_HISTORY),
    ];
    for ((info, handler), (name, expected)) in portable_handlers().into_iter().zip(expected) {
        assert_eq!(info.name, name);
        match handler {
            CommandHandler::Pure(run) => {
                assert_eq!(name, "preview-request");
                assert!(expected.is_empty());
                assert!(matches!(
                    run(Some("json")).action,
                    Some(DebugAction::PreviewOutboundRequest { json: true, .. })
                ));
            }
            CommandHandler::Contextual {
                capabilities,
                handler,
            } => {
                assert_eq!(capabilities, expected, "/{name}");
                let result = handler(CommandContexts::empty(), None);
                assert!(result.is_error, "/{name}");
                assert!(
                    result
                        .message
                        .unwrap()
                        .starts_with("Error: Command capability unavailable:")
                );
                assert!(result.action.is_none());
            }
        }
    }
}

fn empty_receipt() -> Receipt {
    Receipt {
        schema_id: RECEIPT_SCHEMA_ID,
        source: ReceiptSource {
            kind: SourceKind::Session,
            id: "session-1".into(),
            title: None,
            workspace: None,
            model: None,
            started_at: None,
            updated_at: None,
        },
        turn: None,
        postures: vec![],
        totals: ReceiptTotals::default(),
        actions: vec![],
        omitted_actions: 0,
        not_recorded: vec![],
        claim_ceiling: [
            "local_record_only",
            "not_safety_certification",
            "not_provider_compatibility_certification",
        ],
    }
}

struct ReceiptFacet {
    calls: RefCell<Vec<Option<String>>>,
    result: Result<Receipt, DebugReceiptError>,
}
impl CommandDebugReceiptsContext for ReceiptFacet {
    fn receipt(&self, turn: Option<&str>) -> Result<Receipt, DebugReceiptError> {
        self.calls.borrow_mut().push(turn.map(str::to_owned));
        self.result.clone()
    }
}

#[test]
fn receipts_validate_before_host_reads_and_preserve_last_turn_selector() {
    let mut facet = ReceiptFacet {
        calls: RefCell::new(vec![]),
        result: Ok(empty_receipt()),
    };
    for argument in [
        "bad",
        "json -1",
        "999999999999999999999999999999999999999999",
    ] {
        let result = receipts::receipts(
            CommandContexts::empty().with_debug_receipts(&mut facet),
            Some(argument),
        );
        assert!(result.is_error);
        assert!(
            result
                .message
                .unwrap()
                .contains("Use /receipts [json] [<turn>].")
        );
    }
    assert!(facet.calls.borrow().is_empty());
    let result = receipts::receipts(
        CommandContexts::empty().with_debug_receipts(&mut facet),
        Some("json 2 03"),
    );
    assert!(!result.is_error);
    assert_eq!(&*facet.calls.borrow(), &[Some("03".into())]);
    let result = receipts::receipts(
        CommandContexts::empty().with_debug_receipts(&mut facet),
        None,
    );
    assert_eq!(
        result.message.as_deref(),
        Some("# Receipt: session session-1\n\nsession session-1\n\nNo actions recorded.")
    );
    assert!(result.action.is_none());
}

#[test]
fn receipt_failures_keep_the_approval_and_builder_error_contracts() {
    for (error, message) in [
        (
            DebugReceiptError::ApprovalLog("broken record".into()),
            "Error: Could not read this session's approval log: broken record",
        ),
        (
            DebugReceiptError::Build("turn 9 not found".into()),
            "Error: turn 9 not found",
        ),
    ] {
        let mut facet = ReceiptFacet {
            calls: RefCell::new(vec![]),
            result: Err(error),
        };
        let result = receipts::receipts(
            CommandContexts::empty().with_debug_receipts(&mut facet),
            None,
        );
        assert!(result.is_error);
        assert_eq!(result.message.as_deref(), Some(message));
        assert!(result.action.is_none());
        assert_eq!(facet.calls.borrow().len(), 1);
    }
}

#[test]
fn receipt_rendering_preserves_json_fences_and_prevents_forged_terminal_lines() {
    let mut receipt = empty_receipt();
    receipt.source.title = Some("````\nforged\u{1b}[31m".into());
    let mut facet = ReceiptFacet {
        calls: RefCell::new(vec![]),
        result: Ok(receipt),
    };
    let result = receipts::receipts(
        CommandContexts::empty().with_debug_receipts(&mut facet),
        Some("json"),
    );
    let output = result.message.unwrap();
    let json = output
        .strip_prefix("`````json\n")
        .unwrap()
        .strip_suffix("\n`````")
        .unwrap();
    let value: serde_json::Value = serde_json::from_str(json).unwrap();
    assert_eq!(value["schema_id"], RECEIPT_SCHEMA_ID);
    assert_eq!(value["source"]["title"], "````\nforged\u{1b}[31m");
    let result = receipts::receipts(
        CommandContexts::empty().with_debug_receipts(&mut facet),
        None,
    );
    let output = result.message.unwrap();
    assert!(!output.contains("\nforged"));
    assert!(!output.contains('\u{1b}'));
    assert!(output.contains("\\nforged\\u{1b}"));
}

#[derive(Default)]
struct HistoryFacet {
    events: Rc<RefCell<Vec<&'static str>>>,
    input: Option<String>,
    composer: Option<String>,
    removed: usize,
}
impl CommandDebugHistoryContext for HistoryFacet {
    fn last_user_input(&self) -> Option<String> {
        self.events.borrow_mut().push("last_user");
        self.input.clone()
    }
    fn load_composer(&mut self, input: String) {
        self.events.borrow_mut().push("composer");
        self.composer = Some(input);
    }
    fn undo_conversation(&mut self) -> DebugConversationUndo {
        self.events.borrow_mut().push("undo_chat");
        DebugConversationUndo {
            removed: self.removed,
            sync: synced_conversation(),
        }
    }
}
fn synced_conversation() -> SessionSyncPayload {
    SessionSyncPayload {
        session_id: Some("undo-session".to_string()),
        messages: Vec::new(),
        system_prompt: None,
        model: "model".to_string(),
        workspace: std::path::PathBuf::from("/tmp/undo-workspace"),
        mode: codewhale_command_contract::CommandMode::Agent,
    }
}
struct UndoFacet {
    events: Rc<RefCell<Vec<&'static str>>>,
    outcome: DebugUndoOutcome,
}
impl CommandDebugUndoContext for UndoFacet {
    fn undo_files(&mut self) -> DebugUndoOutcome {
        self.events.borrow_mut().push("undo_files");
        self.outcome.clone()
    }
}

#[test]
fn undo_refusals_never_fall_back_to_history_or_parse_backend_text() {
    let events = Rc::new(RefCell::new(vec![]));
    let mut history = HistoryFacet {
        events: events.clone(),
        removed: 2,
        ..Default::default()
    };
    for outcome in [
        DebugUndoOutcome::Untrusted,
        DebugUndoOutcome::SnapshotPending,
        DebugUndoOutcome::ChangedSince {
            label: "tool:1".into(),
            paths: vec!["a.txt".into()],
        },
        // Deliberately resembles the old English fallback prefixes. Its typed
        // failure must stay a refusal and cannot silently delete conversation.
        DebugUndoOutcome::RestoreBlocked("No snapshots found, but operation was refused".into()),
        DebugUndoOutcome::CompareFailed("read error".into()),
        DebugUndoOutcome::RestoreFailed("write error".into()),
    ] {
        events.borrow_mut().clear();
        let mut facet = UndoFacet {
            events: events.clone(),
            outcome,
        };
        let result = undo::undo(
            CommandContexts::empty()
                .with_debug_undo(&mut facet)
                .with_debug_history(&mut history),
            None,
        );
        assert!(result.action.is_none());
        assert_eq!(&*events.borrow(), &["undo_files"]);
    }
}

#[test]
fn undo_fallback_requires_both_facets_and_keeps_file_warning() {
    let events = Rc::new(RefCell::new(vec![]));
    let mut facet = UndoFacet {
        events: events.clone(),
        outcome: DebugUndoOutcome::RepoUnavailable {
            workspace: "/workspace".into(),
            error: "read denied".into(),
        },
    };
    let result = undo::undo(CommandContexts::empty().with_debug_undo(&mut facet), None);
    assert_eq!(
        result.message.as_deref(),
        Some("Error: Command capability unavailable: debug_history")
    );
    assert!(events.borrow().is_empty());
    let mut history = HistoryFacet {
        events: events.clone(),
        removed: 2,
        ..Default::default()
    };
    let result = undo::undo(
        CommandContexts::empty()
            .with_debug_undo(&mut facet)
            .with_debug_history(&mut history),
        None,
    );
    assert_eq!(
        result.message.as_deref(),
        Some(
            "Removed 2 message(s)\nWorkspace files were NOT reverted — only the conversation was rolled back.\nSnapshot repo unavailable for /workspace: read denied"
        )
    );
    assert_eq!(&*events.borrow(), &["undo_files", "undo_chat"]);
    for outcome in [
        DebugUndoOutcome::NoSnapshots,
        DebugUndoOutcome::NoSession,
        DebugUndoOutcome::NoOwnedSteps,
        DebugUndoOutcome::NoDifference,
    ] {
        facet.outcome = outcome;
        events.borrow_mut().clear();
        let result = undo::undo(
            CommandContexts::empty()
                .with_debug_undo(&mut facet)
                .with_debug_history(&mut history),
            None,
        );
        assert_eq!(result.message.as_deref(), Some("Removed 2 message(s)"));
        assert_eq!(&*events.borrow(), &["undo_files", "undo_chat"]);
    }
}

#[test]
fn retry_truncates_history_before_emitting_the_exact_original_input() {
    let input = "漢字".repeat(20);
    let mut history = HistoryFacet {
        input: Some(input.clone()),
        removed: 2,
        ..Default::default()
    };
    let result = undo::retry(
        CommandContexts::empty().with_debug_history(&mut history),
        None,
    );
    assert_eq!(
        result.action,
        Some(DebugAction::ConversationUndo {
            sync: synced_conversation(),
            retry_input: Some(input.clone()),
        })
    );
    assert_eq!(
        result.message,
        Some(format!("Retrying: {}...", "漢字".repeat(8)))
    );
    assert_eq!(&*history.events.borrow(), &["last_user", "undo_chat"]);
    history.events.borrow_mut().clear();
    let result = undo::edit(
        CommandContexts::empty().with_debug_history(&mut history),
        None,
    );
    assert!(!result.is_error);
    assert_eq!(history.composer, Some(input));
    assert_eq!(&*history.events.borrow(), &["last_user", "composer"]);
}
