//! The actual `/pet on` surface over the foreground Engine session. This
//! replaces the old full-screen tank layout; the companion still owns its
//! durable world and audio. No prompt, worker or execution authority lives here.
use super::{PetWatch, habitat};
use crate::{
    agent_roster::RosterState,
    tools::subagent::AgentWorkerStatus,
    tui::{
        app::App,
        history::HistoryCell,
        shell_key_routing::{self, ShellBindingId as Id},
        underwater::ShellPhase,
    },
};
use codewhale_localization::{MessageId, tr};
use codewhale_protocol::engine_owner::OwnerFreshness;
use codewhale_ratatui::{
    AgentCard, Caps, MotionMode, PetMode, PetModeAreas, State, StatusMark, Subagent,
    SubagentControls, SubagentUsage, SubagentViewWords, Theme,
    whale_motion::{Activity, Context, Inputs, Presence, Span},
};
use ratatui::{Frame, text::Line};
use std::time::{Duration, Instant};

pub(super) fn canonical(state: &PetWatch) -> bool {
    matches!(state.avatars.key.as_str(), "" | "whale")
        && state.avatars.action.is_none()
        && state.avatars.view.is_none()
}
fn theme(app: &App) -> Theme {
    let mut caps = Caps::detect();
    if let Some(appearance) =
        codewhale_ratatui::detect::appearance_for_background(app.ui_theme.surface_bg)
    {
        caps.appearance = appearance;
    }
    Theme::new(caps).tui_palette(codewhale_ratatui::TuiPalette::Underwater)
}

pub(super) fn agents(app: &App) -> Vec<Subagent<'static>> {
    app.current_agent_roster()
        .iter()
        .map(|row| {
            let (state, label) = match row.state {
                RosterState::Running if row.status == AgentWorkerStatus::Queued => {
                    (State::Ready, MessageId::AgentRailQueuedCount)
                }
                RosterState::Running => (State::Working, MessageId::SubagentsStatusRunning),
                RosterState::Waiting => (State::NeedsYou, MessageId::PhaseWaitingOnYou),
                RosterState::Parked => (State::Stopped, MessageId::AgentStatusParked),
                RosterState::Done => (State::Done, MessageId::SubagentsStatusCompleted),
                RosterState::Failed => (State::Failed, MessageId::SubagentsStatusFailed),
                RosterState::Cancelled => (State::Stopped, MessageId::SubagentsStatusCancelled),
            };
            let mut card = AgentCard::new(row.display_name.clone(), state)
                .status_word(tr(app.ui_locale, label).replace("{count}", "1"))
                .task(row.activity.clone().unwrap_or_default());
            if !row.model.trim().is_empty() {
                card = card.route(row.model.clone());
            }
            let mut agent = Subagent::new(row.worker_id.clone(), card);
            agent.elapsed = row.millis.map(Duration::from_millis);
            agent.usage = Some(SubagentUsage {
                input_tokens: row.input_tokens,
                output_tokens: row.output_tokens,
                cost_microusd: row.cost_microusd,
            });
            agent.steps = Some(row.steps_taken);
            // This pane is the retained roster. Enter opens the existing full
            // agent transcript, so long outcomes are not cloned on animation frames.
            agent.controls = SubagentControls {
                open: true,
                ..Default::default()
            };
            agent
        })
        .collect()
}
fn inputs(app: &App, now: Instant) -> Inputs {
    let presence = match ShellPhase::from_app(app) {
        ShellPhase::Working | ShellPhase::Verifying => Presence::Working,
        ShellPhase::Waiting | ShellPhase::Approval => Presence::NeedsYou,
        ShellPhase::Typing => Presence::Listening,
        ShellPhase::Done => Presence::Done,
        ShellPhase::Idle | ShellPhase::Failed => Presence::Idle,
    };
    let raster = app.pet_watch.raster.as_ref().filter(|r| {
        r.scene.producer_connected
            && now.saturating_duration_since(r.frame_changed) < Duration::from_millis(800)
            && Some(r.scene.source.as_str()) == app.current_session_id.as_deref()
    });
    let owner = raster.and_then(|r| r.scene.activity.as_ref()).filter(|a| {
        a.observed && a.freshness == OwnerFreshness::Fresh && a.session_id == app.current_session_id
    });
    Inputs {
        presence,
        activity: owner.map(|a| Activity {
            kind: a.activity_kind.map(|k| k.as_str().into()),
            observed: true,
            parallel: Some(f64::from(a.parallel_agent_count)),
            active: a
                .active_spans
                .iter()
                .map(|s| Span {
                    kind: s.activity_kind.as_str().into(),
                    since_ms: s.started_at_ms,
                })
                .collect(),
        }),
        context: Context {
            live: owner.is_some(),
            turn_id: app.runtime_turn_id.clone(),
            status: app.runtime_turn_status.clone(),
            // Only owner time, never a local timer pretending to be an error receipt.
            now_ms: raster.map(|r| r.scene.time_ms),
            ..Default::default()
        },
    }
}
pub(super) fn motion(app: &App) -> MotionMode {
    // The habitat owns the viewport through the modal stack. The ordinary
    // shell's ambient helper intentionally stops under every modal, so using
    // it here would freeze the pet simply because its own view is open.
    match app.motion_policy().mode() {
        crate::tui::motion::MotionMode::Still => MotionMode::Still,
        crate::tui::motion::MotionMode::Reduced => MotionMode::Reduced,
        crate::tui::motion::MotionMode::Full
            if matches!(
                ShellPhase::from_app(app),
                ShellPhase::Approval | ShellPhase::Waiting
            ) =>
        {
            MotionMode::Reduced
        }
        crate::tui::motion::MotionMode::Full => MotionMode::Full,
    }
}
pub(super) fn tick(app: &mut App, visible: bool, now: Instant) {
    app.pet_watch.full.set_visible(visible);
    if !visible {
        app.pet_watch.full_next = None;
        return;
    }
    let agents = agents(app);
    // Revalidate the painted identity against the current session before the
    // modal can return an open intent. A removed/ambiguous worker never turns
    // into the new fallback selection before it has actually been painted.
    if let Ok(mut selected) = app.pet_watch.selection.lock()
        && selected.as_deref().is_some_and(|id| {
            agents
                .iter()
                .filter(|agent| agent.id == id && agent.controls.open)
                .count()
                != 1
        })
    {
        *selected = None;
    }
    let inputs = inputs(app, now);
    let changed = app.pet_watch.full_inputs.as_ref().is_none_or(|old| {
        old.presence != inputs.presence
            || old.activity != inputs.activity
            || old.context.live != inputs.context.live
            || old.context.turn_id != inputs.context.turn_id
            || old.context.status != inputs.context.status
    });
    let motion = motion(app);
    let theme = theme(app);
    let state = &mut app.pet_watch;
    state.full.update(
        app.current_session_id.as_deref(),
        inputs.clone(),
        &agents,
        now,
        motion,
    );
    state.full_inputs = Some(inputs);
    if changed {
        state.full_next = None;
        app.needs_redraw = true;
    }
    if let Some(interval) = state.full.next_frame_in(&agents, &theme) {
        if state.full_next.is_none_or(|deadline| now >= deadline) {
            state.full_next = now.checked_add(interval);
            app.needs_redraw = true;
        }
    } else {
        state.full_next = None;
    }
}

pub fn render_full(frame: &mut Frame, app: &mut App) {
    let agents = agents(app);
    let theme = theme(app);
    let phase = ShellPhase::from_app(app);
    let notice = app.active_status_toast(phase).map(|toast| {
        Line::styled(
            toast.text,
            codewhale_palette::chrome_style(&app.ui_theme, toast.level.ink()),
        )
    });
    let mark = StatusMark::new(match phase {
        ShellPhase::Working | ShellPhase::Verifying => State::Working,
        ShellPhase::Approval | ShellPhase::Waiting => State::NeedsYou,
        ShellPhase::Done => State::Done,
        ShellPhase::Failed => State::Failed,
        ShellPhase::Idle | ShellPhase::Typing => State::Ready,
    })
    .word(phase.label(app.ui_locale).into_owned());
    // ActiveCell is the same live transcript the normal shell reads. Include
    // streamed replies and terminal errors, without waiting for a successful turn.
    let response = app
        .history
        .iter()
        .skip(app.pet_watch.work_history_start)
        .chain(
            app.active_cell
                .as_ref()
                .into_iter()
                .flat_map(|cell| cell.entries()),
        )
        .rfind(|cell| {
            matches!(
                cell,
                HistoryCell::Assistant { .. } | HistoryCell::Error { .. }
            )
        });
    let plan = PetModeAreas::new(
        frame.area(),
        !agents.is_empty(),
        response.is_some(),
        app.pet_watch.full.focus_agents,
    );
    let owner = inputs(app, Instant::now());
    let motion = motion(app);
    let key = (
        app.history_version,
        app.active_cell.as_ref().map_or(0, |cell| cell.revision()),
        app.pet_watch.work_history_start,
    );
    if app.pet_watch.full_response_key != Some(key) {
        app.pet_watch.full_response = None;
        app.pet_watch.full_response_key = Some(key);
    }
    let mut options = app.transcript_render_options();
    options.low_motion = true;
    options.motion_mode = crate::tui::motion::MotionMode::Reduced;
    let cache = app
        .pet_watch
        .full_response
        .get_or_insert_with(crate::tui::transcript::TranscriptViewCache::new);
    cache.ensure_split(
        &[response.map_or(&[], std::slice::from_ref)],
        if response.is_some() { &[0] } else { &[] },
        plan.output.width.max(1),
        options,
        &std::collections::HashMap::new(),
        None,
        None,
    );
    let lines = cache.lines();
    let mut hints = if frame.area().width >= 100 {
        habitat::hints(app.ui_locale)
    } else {
        format!(
            "{} {}",
            shell_key_routing::binding(Id::PetBack).footer_chord,
            tr(app.ui_locale, MessageId::SetupActionBack)
        )
    };
    if !agents.is_empty() {
        hints.push_str(&format!(
            " · {} {}",
            shell_key_routing::binding(Id::PetFocusAgents).footer_chord,
            tr(app.ui_locale, MessageId::SubagentsHeaderRoster)
        ));
        if app.pet_watch.full.focus_agents {
            hints.push_str(&format!(
                " · {} {}",
                shell_key_routing::binding(Id::PetOpenAgent).footer_chord,
                tr(app.ui_locale, MessageId::CtxInspTranscript)
            ));
        }
    }
    if !lines.is_empty() {
        hints.push_str(&format!(
            " · {}{} {}",
            shell_key_routing::binding(Id::PetResultUp).footer_chord,
            shell_key_routing::binding(Id::PetResultDown).footer_chord,
            tr(app.ui_locale, MessageId::SetupActionScrollBody)
        ));
    }
    let mut view = PetMode::new(&theme, mark);
    view.notice = notice;
    view.title = "Codewhale".into();
    view.agents = &agents;
    view.agent_words = SubagentViewWords {
        title: tr(app.ui_locale, MessageId::SubagentsHeaderRoster),
        empty: tr(
            app.ui_locale,
            MessageId::SubagentsNoCurrentSessionFleetWorkers,
        ),
        hints: "".into(),
        open: "".into(),
        message: "".into(),
        stop: "".into(),
        summary: Some(agents.len().to_string().into()),
        steps_label: Some(tr(app.ui_locale, MessageId::SessionMetricsSteps)),
        ..Default::default()
    };
    view.output = lines;
    view.output_title = tr(app.ui_locale, MessageId::AgentFocusResult);
    view.hints = hints.into();
    app.pet_watch.full.set_visible(true);
    app.pet_watch.full.update(
        app.current_session_id.as_deref(),
        owner,
        &agents,
        Instant::now(),
        motion,
    );
    frame.render_stateful_widget(view, frame.area(), &mut app.pet_watch.full);
    if let Ok(mut selection) = app.pet_watch.selection.lock() {
        *selection = app.pet_watch.full.selected_agent_id().map(str::to_owned);
    }
    if canonical(&app.pet_watch) {
        app.pet_watch.area = Some(plan.pet);
    } else {
        // Existing reviewed avatar packs and explicit action/view previews keep
        // their renderer and permissions; only the full-screen layout is shared.
        super::render_tank(frame, plan.pet, app);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::{active_cell::ActiveCell, app::OnboardingState};
    fn app() -> App {
        let mut app =
            crate::test_support::test_app_with_options(crate::test_support::test_tui_options("."));
        app.onboarding = OnboardingState::None;
        app.redaction_gate = false;
        app.pet_watch.session = app.current_session_id.clone();
        app.pet_watch.detach_for_test();
        app
    }
    fn paint(app: &mut App, width: u16, height: u16) -> String {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| render_full(frame, app)).unwrap();
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }
    #[test]
    fn full_mode_animates_under_its_own_modal_and_respects_motion_preferences() {
        let mut app = app();
        app.is_loading = true;
        app.low_motion = false;
        app.fancy_animations = true;
        super::super::open_habitat(&mut app);
        assert!(super::super::is_open(&app));
        assert_eq!(motion(&app), MotionMode::Full);
        paint(&mut app, 100, 32);
        tick(&mut app, true, Instant::now());
        assert!(app.pet_watch.full_next.is_some());
        app.low_motion = true;
        tick(&mut app, true, Instant::now());
        assert!(app.pet_watch.full_next.is_none());
        app.low_motion = false;
        app.fancy_animations = false;
        assert_eq!(motion(&app), MotionMode::Still);
    }
    #[test]
    fn roster_scope_and_missing_vs_zero_receipts_come_from_the_existing_owner_guard() {
        let mut app = app();
        app.current_session_id = Some("foreground".into());
        app.agent_roster_session_id = Some("other-session".into());
        app.agent_roster = vec![crate::agent_roster::AgentRosterRow {
            worker_id: "retained-worker".into(),
            display_name: "Retained worker".into(),
            model: "reported-model".into(),
            state: RosterState::Waiting,
            status: AgentWorkerStatus::WaitingForUser,
            activity: Some("Owner activity".into()),
            outcome: None,
            millis: None,
            input_tokens: Some(0),
            output_tokens: None,
            cost_microusd: Some(1),
            steps_taken: 0,
            parent_run_id: None,
            run_id: "actual-run".into(),
        }];
        assert!(agents(&app).is_empty());
        *app.pet_watch.selection.lock().unwrap() = Some("retained-worker".into());
        tick(&mut app, true, Instant::now());
        assert!(app.pet_watch.selection.lock().unwrap().is_none());
        app.agent_roster_session_id = app.current_session_id.clone();
        let rows = agents(&app);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "retained-worker");
        assert_eq!(rows[0].card.status.state, State::NeedsYou);
        assert_eq!(rows[0].usage.unwrap().input_tokens, Some(0));
        assert_eq!(rows[0].usage.unwrap().output_tokens, None);
        assert_eq!(rows[0].usage.unwrap().cost_microusd, Some(1));
        assert_eq!(rows[0].elapsed, None);
        app.agent_roster[0].state = RosterState::Parked;
        assert_eq!(
            agents(&app)[0].card.status.word,
            tr(app.ui_locale, MessageId::AgentStatusParked)
        );
    }
    #[test]
    fn full_mode_reads_streaming_response_then_failure_and_preserves_the_session() {
        let mut app = app();
        app.is_loading = true;
        app.input = "retained draft".into();
        let session = app.current_session_id.clone();
        let mut active = ActiveCell::new();
        active.push_untracked(HistoryCell::Assistant {
            content: "Streaming response from this session".into(),
            streaming: true,
        });
        app.active_cell = Some(active);
        assert!(paint(&mut app, 80, 24).contains("Streaming response from this session"));
        app.active_cell = None;
        app.is_loading = false;
        app.runtime_turn_status = Some("failed".into());
        app.add_message(HistoryCell::Error {
            message: "Actual failure receipt".into(),
            severity: crate::error_taxonomy::ErrorSeverity::Error,
        });
        assert!(paint(&mut app, 40, 12).contains("Actual failure receipt"));
        assert_eq!(app.input, "retained draft");
        assert_eq!(app.current_session_id, session);
        assert!(app.pet_watch.worker.is_none());
    }
    #[test]
    fn reduced_completed_view_reuses_the_response_and_does_not_schedule_paints() {
        let mut app = app();
        app.runtime_turn_status = Some("completed".into());
        app.add_message(HistoryCell::Assistant {
            content: "Finished answer".into(),
            streaming: false,
        });
        let first = paint(&mut app, 100, 32);
        let lines = app
            .pet_watch
            .full_response
            .as_ref()
            .unwrap()
            .lines()
            .as_ptr();
        assert_eq!(paint(&mut app, 100, 32), first);
        assert_eq!(
            app.pet_watch
                .full_response
                .as_ref()
                .unwrap()
                .lines()
                .as_ptr(),
            lines
        );
        tick(&mut app, true, Instant::now() + Duration::from_secs(2));
        assert!(app.pet_watch.full_next.is_none());
        tick(&mut app, false, Instant::now() + Duration::from_secs(3));
        assert!(app.pet_watch.full_next.is_none());
    }
}
