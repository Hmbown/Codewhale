use super::*;
use codewhale_config::user_constitution::{ProfileConstitution, ProfileConstitutionSnapshot};

#[tokio::test]
async fn profile_constitution_reaches_real_engine_requests_and_survives_history_repair() {
    use crate::llm_client::mock::{MockLlmClient, canned};
    let _home = crate::test_support::SealedHome::new();
    let workspace = tempfile::tempdir().unwrap();
    let config = Config::default();
    let mock = Arc::new(MockLlmClient::new(vec![
        canned::simple_text_turn("Done"),
        canned::simple_text_turn("Done again"),
        canned::simple_text_turn("Follow-up done"),
    ]));
    let (mut engine, handle) = Engine::new_with_model_client(
        deterministic_engine_config(workspace.path()),
        &config,
        mock.clone(),
    );
    let drainer =
        tokio::spawn(async move { while handle.rx_event.write().await.recv().await.is_some() {} });
    for (revision, note) in [
        (1, "Profile alpha preference"),
        (2, "Profile beta preference"),
    ] {
        let Op::SendMessage(mut spec) =
            external_user_message_op("Say hello", AppMode::Agent, &config)
        else {
            unreachable!()
        };
        spec.profile_constitution = Some(ProfileConstitutionSnapshot {
            account_id: "acct_fixture".into(),
            revision,
            constitution: ProfileConstitution {
                notes: note.into(),
                ..ProfileConstitution::default()
            },
        });
        let outcome = engine.handle_send_message(spec).await;
        assert!(!matches!(outcome, SendMessageOutcome::NotStarted { .. }));
        let requests = mock.captured_requests();
        let request = serde_json::to_string(requests.last().unwrap()).unwrap();
        assert!(request.contains(note));
        assert!(request.contains("replaces all earlier personal constitution snapshots"));
        assert!(request.contains("do not change permissions"));
    }
    let latest = engine
        .session
        .messages
        .iter()
        .rev()
        .find(|message| crate::runtime_handoff::constitution_display(message).is_some())
        .unwrap()
        .clone();
    assert!(
        crate::runtime_handoff::constitution_display(&latest)
            .unwrap()
            .contains("revision 2")
    );
    let Op::SendMessage(mut continuation) =
        external_user_message_op("A background task finished", AppMode::Agent, &config)
    else {
        unreachable!()
    };
    continuation.provenance = UserInputProvenance::Runtime;
    // A host can admit a runtime follow-up itself, so this must not depend on
    // the Engine's autonomous scheduling flag.
    let outcome = engine.handle_admitted_message(continuation, false).await;
    assert!(!matches!(outcome, SendMessageOutcome::NotStarted { .. }));
    assert!(
        serde_json::to_string(mock.captured_requests().last().unwrap())
            .unwrap()
            .contains("Profile beta preference")
    );
    assert_eq!(
        engine
            .session
            .messages
            .iter()
            .rev()
            .find(|message| crate::runtime_handoff::constitution_display(message).is_some()),
        Some(&latest)
    );
    engine.session.messages.clear();
    engine.record_current_constitution().await;
    assert_eq!(engine.session.messages.last(), Some(&latest));
    let count = engine.session.messages.len();
    engine.record_current_constitution().await;
    assert_eq!(engine.session.messages.len(), count);
    engine.constitution_block = None;
    engine.record_current_constitution().await;
    assert!(
        crate::runtime_handoff::constitution_display(engine.session.messages.last().unwrap())
            .unwrap()
            .contains("withdrawn")
    );
    drainer.abort();
}
