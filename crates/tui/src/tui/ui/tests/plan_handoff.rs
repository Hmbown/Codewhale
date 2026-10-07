//! Plan hand-off: the answer to the "plan ready" question is applied by the
//! host — permission first, then the mode — and never touches an engine
//! `request_user_input` call.

use super::*;
use crate::tui::plan_handoff::{self, PlanHandoffChoice};
use crate::tui::ui::handlers::{apply_plan_handoff, enter_work_for_plan, is_plan_handoff_request};

/// A session sitting in Plan whose Work permission is Ask.
fn app_in_plan() -> App {
    let mut app = create_test_app();
    app.set_agent_approval_posture(ApprovalMode::Suggest);
    let _ = app.set_mode(AppMode::Plan);
    assert_eq!(app.mode, AppMode::Plan);
    app
}

#[tokio::test]
async fn work_choice_leaves_plan_with_the_chosen_permission() {
    let mut app = app_in_plan();
    let mut engine = mock_engine_handle();

    assert!(
        enter_work_for_plan(
            &mut app,
            &Config::default(),
            &engine.handle,
            ApprovalMode::Suggest
        )
        .await
    );

    assert_eq!(app.mode, AppMode::Agent);
    assert_eq!(app.approval_mode, ApprovalMode::Suggest);
    match engine.rx_op.recv().await.expect("change mode op") {
        crate::core::ops::Op::ChangeMode {
            mode,
            approval_mode,
            ..
        } => {
            assert_eq!(mode, AppMode::Agent);
            assert_eq!(approval_mode, ApprovalMode::Suggest);
        }
        other => panic!("expected ChangeMode, got {other:?}"),
    }
}

#[tokio::test]
async fn a_locked_permission_keeps_the_session_in_plan() {
    let mut app = app_in_plan();
    app.mark_approval_policy_locked();
    let mut engine = mock_engine_handle();

    // Ask is in force and locked; the person asked for Auto-Review. Starting
    // the work under a permission they did not pick is the wrong outcome.
    assert!(
        !enter_work_for_plan(
            &mut app,
            &Config::default(),
            &engine.handle,
            ApprovalMode::Auto
        )
        .await
    );

    assert_eq!(app.mode, AppMode::Plan);
    assert_eq!(app.agent_approval_baseline(), ApprovalMode::Suggest);
    assert!(
        engine.rx_op.try_recv().is_err(),
        "no mode change may reach the engine"
    );
}

#[tokio::test]
async fn keep_planning_changes_nothing_and_sends_nothing() {
    let mut app = app_in_plan();
    let history_len = app.history.len();
    let mut engine = mock_engine_handle();

    apply_plan_handoff(
        &mut app,
        &Config::default(),
        &engine.handle,
        PlanHandoffChoice::KeepPlanning,
    )
    .await
    .expect("keep planning");

    assert_eq!(app.mode, AppMode::Plan);
    assert_eq!(app.history.len(), history_len);
    assert!(!app.is_loading);
    assert!(engine.rx_op.try_recv().is_err());
}

#[test]
fn an_engine_question_with_the_handoff_id_still_goes_to_the_engine() {
    let mut app = app_in_plan();
    assert!(is_plan_handoff_request(&app, plan_handoff::REQUEST_ID));
    assert!(!is_plan_handoff_request(&app, "call_1"));

    // Tool-call ids come from the provider. One that collides with the
    // hand-off id is still the engine's question while it is pending.
    app.pending_user_input_prompt = Some((
        plan_handoff::REQUEST_ID.to_string(),
        plan_handoff::request(app.ui_locale),
    ));
    assert!(!is_plan_handoff_request(&app, plan_handoff::REQUEST_ID));
}
