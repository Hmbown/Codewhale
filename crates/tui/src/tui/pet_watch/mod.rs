//! Views of one durable local pet. Engine events are projected here once;
//! the companion owns simulation, persistence and the sole audio output.
use crate::core::{
    events::Event,
    protocol_parity::{ProtocolIds, event_to_protocol},
};
use crate::tui::{
    app::{App, StatusToastLevel},
    underwater::ShellPhase,
    views::ModalKind,
};
use codewhale_localization::{MessageId, tr};
use codewhale_palette::{ChromeInk, chrome_style};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Paragraph},
};
use serde_json::{Value, json};
use std::{
    io::{self, Write},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
mod appearance;
mod audio;
mod audio_cursor;
mod graphics;
mod habitat;
mod live;
pub(crate) mod owner;
mod persistence;
#[cfg(test)]
mod worker;
use live::{Command, Notice, Presentation, Worker};
#[derive(Clone, Copy)]
pub enum Control {
    Sound,
    Browser,
    Window,
    Select,
    Scroll(i16),
}
#[derive(Default)]
pub struct PetWatch {
    worker: Option<Worker>,
    session: Option<String>,
    last_tick: Option<Instant>,
    failed: bool,
    exporting: bool,
    sound_requested: bool,
    pub(crate) area: Option<Rect>,
    raster: Option<Presentation>,
    controls: Arc<Mutex<Vec<Control>>>,
    desired: Option<Rect>,
    painted: Option<Rect>,
    sent: Option<Instant>,
    started: Option<Instant>,
    frames: u64,
    bytes: u64,
    render_ms: f64,
    output_ms: f64,
    /// `/pet on`: accepted turns enter the full habitat automatically.
    pub(crate) enabled: bool,
    work_enter_pending: bool,
    work_complete: bool,
    work_history_start: usize,
    result_scroll: u16,
}
impl PetWatch {
    pub fn set_sound(&mut self, enabled: bool) {
        self.sound_requested = enabled;
    }
    pub fn sound_label(&self) -> MessageId {
        if !self.sound_requested {
            MessageId::PetWatchSoundOff
        } else if self.raster.as_ref().is_some_and(|r| {
            r.scene.audio_owner.as_deref() == Some(r.client.as_str()) && !r.scene.audio_unavailable
        }) {
            MessageId::PetWatchSoundOn
        } else {
            MessageId::PetWatchSoundPaused
        }
    }
    fn reset(&mut self, session: Option<String>) {
        self.worker = None;
        self.session = session;
        self.raster = None;
        self.failed = false;
        self.last_tick = None;
        self.work_enter_pending = false;
        self.work_complete = false;
        self.result_scroll = 0;
    }
    fn ensure(&mut self, session: Option<String>) {
        if self.session != session {
            self.reset(session)
        }
        if self.worker.is_none() && !self.failed {
            match Worker::start(self.session.clone()) {
                Ok(worker) => self.worker = Some(worker),
                Err(_) => self.failed = true,
            }
        }
    }
    pub fn export(&mut self) -> bool {
        if self.worker.is_none() || self.exporting {
            return false;
        }
        self.send(Command::Export);
        self.exporting = !self.failed;
        self.exporting
    }
    /// Tests drive the shell without a companion: never start a view worker.
    #[cfg(test)]
    pub(crate) fn detach_for_test(&mut self) {
        self.worker = None;
        self.failed = true;
    }
    pub fn observe(&mut self, event: &Event, session: Option<&str>, _now: Instant) {
        if self.session.as_deref() != session {
            self.reset(session.map(str::to_owned));
            return;
        }
        if let Some(text) = metadata(event) {
            self.send(Command::Observe(text));
        }
    }
    fn send(&mut self, command: Command) {
        if self
            .worker
            .as_ref()
            .is_some_and(|w| w.tx.try_send(command).is_err())
        {
            // A gap drops this producer lease. Restart from a fresh unobserved
            // handshake; never infer continuity from events we could not queue.
            self.worker = None;
            self.raster = None;
            self.failed = true;
        }
    }
    pub fn prepare_frame(&mut self) {
        self.desired = None;
    }
    pub fn present(&mut self, output: &mut impl Write) -> io::Result<()> {
        let raster = self
            .raster
            .as_ref()
            .filter(|r| r.frame_changed.elapsed() < Duration::from_millis(800));
        let desired = self.desired.filter(|a| {
            raster.is_some_and(|r| r.image.is_some() && r.width == a.width && r.height == a.height)
        });
        if desired.is_none() {
            if self.painted.take().is_some() {
                graphics::clear(output)?;
            }
            self.sent = None;
            return Ok(());
        }
        let area = desired.unwrap();
        let raster = raster.unwrap();
        if self.painted == Some(area) && self.sent == Some(raster.created) {
            return Ok(());
        }
        let began = Instant::now();
        // Within the existing synchronized frame: delete this process's one
        // image, replace it, restore the cursor. No terminal-side frame queue.
        graphics::clear(output)?;
        write!(output, "\x1b7\x1b[{};{}H", area.y + 1, area.x + 1)?;
        output.write_all(raster.image.as_ref().unwrap())?;
        output.write_all(b"\x1b8")?;
        self.painted = Some(area);
        self.sent = Some(raster.created);
        self.started.get_or_insert(began);
        self.frames += 1;
        self.bytes += raster.bytes as u64;
        self.render_ms += raster.render_ms;
        self.output_ms += began.elapsed().as_secs_f64() * 1000.0;
        Ok(())
    }
    pub fn status(&self) -> String {
        let seconds = self.started.map_or(0.0, |s| s.elapsed().as_secs_f64());
        let identity = self
            .raster
            .as_ref()
            .map(|r| {
                format!(
                    "{} · tick {} · {} · {}",
                    r.scene.identity, r.scene.tick, r.scene.digest, r.scene.source
                )
            })
            .unwrap_or_default();
        format!(
            "{identity} · {} pixel frames / {:.1}s · {:.1} fps · {:.2} MiB/s · render {:.2}ms · write {:.2}ms",
            self.frames,
            seconds,
            self.frames as f64 / seconds.max(0.001),
            self.bytes as f64 / 1048576.0 / seconds.max(0.001),
            self.render_ms / self.frames.max(1) as f64,
            self.output_ms / self.frames.max(1) as f64
        )
    }
}
/// Existing protocol projection defines the variant names. This allowlist
/// removes payloads before the bounded worker queue; no model text escapes.
fn metadata(event: &Event) -> Option<String> {
    if !matches!(
        event,
        Event::TurnStarted { .. }
            | Event::TurnComplete { .. }
            | Event::MessageStarted { .. }
            | Event::MessageDelta { .. }
            | Event::MessageComplete { .. }
            | Event::ThinkingStarted { .. }
            | Event::ThinkingDelta { .. }
            | Event::ThinkingComplete { .. }
            | Event::ToolCallStarted { .. }
            | Event::ToolCallHeartbeat
            | Event::ToolCallComplete { .. }
            | Event::AgentSpawned { .. }
            | Event::AgentProgress { .. }
            | Event::AgentComplete { .. }
            | Event::ApprovalRequired { .. }
            | Event::UserInputRequired { .. }
            | Event::Error { .. }
    ) {
        return None;
    }
    let ids = ProtocolIds {
        thread_id: "foreground".to_owned().into(),
        session_id: "foreground".to_owned().into(),
    };
    let projected = serde_json::to_value(event_to_protocol(event, &ids)).ok()?;
    let mut out = serde_json::Map::new();
    for key in [
        "event",
        "index",
        "channel",
        "tool_call_id",
        "tool_name",
        "id",
        "worker_status",
    ] {
        if let Some(value) = projected.get(key) {
            out.insert(key.to_owned(), value.clone());
        }
    }
    if let Some(status) = projected.pointer("/activity/worker_status") {
        out.insert("worker_status".into(), status.clone());
    }
    let failed = projected.get("status") == Some(&json!("failed"))
        || projected.get("worker_status") == Some(&json!("failed"))
        || projected.pointer("/activity/worker_status") == Some(&json!("failed"))
        || projected.pointer("/result/outcome") == Some(&json!("err"))
        || projected.pointer("/result/success") == Some(&Value::Bool(false));
    if failed {
        out.insert("failed".into(), Value::Bool(true));
    }
    let json = serde_json::to_string(&out).ok()?;
    // Producer data is bounded before crossing the queue, including tool names.
    (json.len() <= 16_384).then_some(json)
}

pub fn command(app: &mut App, control: Control) {
    app.pet_watch.ensure(app.current_session_id.clone());
    apply(&mut app.pet_watch, control);
    app.needs_redraw = true;
}
fn apply(state: &mut PetWatch, control: Control) {
    match control {
        Control::Browser => state.send(Command::Browser),
        Control::Window => state.send(Command::Window),
        Control::Select => state.send(Command::Select),
        Control::Sound => state.sound_requested = !state.sound_requested,
        Control::Scroll(delta) => {
            state.result_scroll = state.result_scroll.saturating_add_signed(delta)
        }
    }
}
pub fn open_habitat(app: &mut App) {
    app.pet_watch.ensure(app.current_session_id.clone());
    if app.view_stack.top_kind() != Some(ModalKind::PetHabitat) {
        app.view_stack
            .push(habitat::Habitat::new(app.pet_watch.controls.clone()));
    }
    app.needs_redraw = true;
}
pub fn is_open(app: &App) -> bool {
    app.view_stack.top_kind() == Some(ModalKind::PetHabitat)
}
/// `/pet on|off`. Enabling enters the habitat now and lets every accepted
/// turn re-enter it; disabling closes the view and stops automatic entry.
/// The durable pet keeps living in its companion either way, and the
/// composer draft, transcript and active Engine turn are never touched.
pub fn set_enabled(app: &mut App, enabled: bool) {
    app.pet_watch.enabled = enabled;
    if enabled {
        open_habitat(app);
        return;
    }
    app.pet_watch.work_enter_pending = false;
    app.pet_watch.work_complete = false;
    if is_open(app) {
        app.view_stack.pop();
    }
    let session = app.pet_watch.session.clone();
    app.pet_watch.reset(session);
    app.needs_redraw = true;
}
/// The existing Engine determines work boundaries. Only the shell reads the
/// answer; no conversation text enters the pet owner or recording.
pub fn observe(app: &mut App, event: &Event, now: Instant) {
    app.pet_watch
        .observe(event, app.current_session_id.as_deref(), now);
    if matches!(event, Event::TurnStarted { .. }) {
        app.pet_watch.work_history_start = app.history.len();
        app.pet_watch.work_complete = false;
        app.pet_watch.result_scroll = 0;
        app.pet_watch.work_enter_pending = app.pet_watch.enabled;
    } else if matches!(event, Event::TurnComplete { .. }) {
        app.pet_watch.work_enter_pending = false;
        app.pet_watch.work_complete = app.view_stack.top_kind() == Some(ModalKind::PetHabitat);
        app.needs_redraw = true;
    }
}
pub fn tick(app: &mut App, now: Instant) {
    if app.pet_watch.work_enter_pending
        && app.view_stack.is_empty()
        && !app.redaction_gate
        && app.onboarding == crate::tui::app::OnboardingState::None
    {
        app.pet_watch.work_enter_pending = false;
        open_habitat(app);
    }
    // The habitat is the pet's only terminal view: it owns the whole content
    // viewport or nothing. Reduced motion follows the shell's motion setting.
    let visible = !app.redaction_gate
        && app.onboarding == crate::tui::app::OnboardingState::None
        && is_open(app);
    let motion = visible && crate::tui::underwater::decorative_shell_motion_enabled(app);
    let waiting = matches!(
        ShellPhase::from_app(app),
        ShellPhase::Waiting | ShellPhase::Approval
    );
    let sound_allowed = visible
        && app.onboarding == crate::tui::app::OnboardingState::None
        && !app.notification_settings.quiet
        && !app.notification_settings.event_sound.quiet;
    let controls = app
        .pet_watch
        .controls
        .lock()
        .map(|mut c| std::mem::take(&mut *c))
        .unwrap_or_default();
    let state = &mut app.pet_watch;
    if state.session != app.current_session_id {
        state.reset(app.current_session_id.clone());
    }
    if visible {
        state.ensure(app.current_session_id.clone());
    }
    for control in controls {
        apply(state, control)
    }
    if let Some(update) = state
        .worker
        .as_ref()
        .and_then(|w| w.latest.lock().ok().and_then(|mut s| s.take()))
    {
        if update.scene.audio_unavailable {
            state.sound_requested = false;
        }
        state.raster = Some(update);
        if visible {
            app.needs_redraw = true;
        }
    }
    if state.raster.as_ref().is_some_and(|r| {
        now.saturating_duration_since(r.frame_changed) > Duration::from_millis(800)
    }) {
        state.raster = None;
        if visible {
            app.needs_redraw = true;
        }
    }
    if state
        .last_tick
        .is_none_or(|t| now.saturating_duration_since(t) >= Duration::from_millis(30))
    {
        let a = state.area.unwrap_or(Rect::new(0, 0, 40, 8));
        let cell = crossterm::terminal::window_size()
            .ok()
            .filter(|s| s.columns > 0 && s.rows > 0 && s.width > 0 && s.height > 0)
            .map(|s| {
                (
                    f64::from(s.width) / f64::from(s.columns),
                    f64::from(s.height) / f64::from(s.rows),
                )
            })
            .unwrap_or((8.0, 16.0));
        let next = live::View {
            width: a.width.clamp(1, 512),
            height: a.height.saturating_sub(1).clamp(1, 256),
            cell_width: cell.0,
            cell_height: cell.1,
            motion,
            pixels: crate::tui::mark::kitty_graphics_supported()
                && app.synchronized_output_enabled
                && std::env::var("CODEWHALE_PET_GRAPHICS").as_deref() != Ok("braille"),
            visible,
            waiting,
            sound: state.sound_requested && sound_allowed,
        };
        if let Some(worker) = &state.worker
            && let Ok(mut view) = worker.view.lock()
        {
            *view = next;
        }
        state.last_tick = Some(now);
    }
    let notices: Vec<_> = state
        .worker
        .as_ref()
        .map(|w| w.notices.try_iter().take(16).collect())
        .unwrap_or_default();
    for notice in notices {
        app.pet_watch.exporting = false;
        let (text, level) = match notice {
            Notice::Exported(path) => (
                tr(app.ui_locale, MessageId::PetWatchExported)
                    .replace("{path}", &path.display().to_string()),
                StatusToastLevel::Info,
            ),
            Notice::Message(message) => (
                format!(
                    "{} · {message}",
                    tr(app.ui_locale, MessageId::PetWatchUnavailable)
                ),
                StatusToastLevel::Warning,
            ),
        };
        app.add_message(crate::tui::history::HistoryCell::System {
            content: text.clone(),
        });
        app.push_status_toast(text, level, Some(12000));
        app.needs_redraw = true;
    }
}
fn render_tank(frame: &mut Frame, area: Rect, app: &mut App) {
    app.pet_watch.area = Some(area);
    let raster = app.pet_watch.raster.as_ref();
    let hollow = raster.is_none_or(|r| !r.scene.producer_connected || r.scene.style.hollow);
    let mut label = raster
        .map(|r| {
            let mut text = format!(
                "{} · {} · {}",
                r.scene.style.channel, r.scene.style.arch, r.scene.behaviour
            );
            if let Some(activity) = &r.scene.activity
                && activity.observed
                && r.frame_changed.elapsed().as_millis() < 800
            {
                text = format!(
                    "{} · {}",
                    activity.tool.as_deref().unwrap_or(&activity.label),
                    text
                );
                if activity.parallel > 0 {
                    text.push_str(&format!(" · ×{}", activity.parallel));
                }
            }
            text
        })
        .unwrap_or_default();
    if hollow {
        label.push_str(&format!(
            " · {}",
            tr(app.ui_locale, MessageId::PetUnobserved)
        ));
    }
    label.push_str(&format!(
        " · {}",
        tr(app.ui_locale, app.pet_watch.sound_label())
    ));
    let image = (app.view_stack.is_empty()
        || app.view_stack.top_kind() == Some(ModalKind::PetHabitat))
        && raster.is_some_and(|r| {
            r.image.is_some() && r.width == area.width && r.height == area.height.saturating_sub(1)
        })
        && area.height >= 4;
    let bg = raster
        .map(|r| r.scene.appearance.background)
        .unwrap_or([8, 15, 21]);
    let ink = raster
        .map(|r| {
            Color::Rgb(
                r.scene.style.r.clamp(0.0, 255.0) as u8,
                r.scene.style.g.clamp(0.0, 255.0) as u8,
                r.scene.style.b.clamp(0.0, 255.0) as u8,
            )
        })
        .unwrap_or(Color::Rgb(180, 210, 216));
    frame.render_widget(
        Block::default().style(Style::default().bg(Color::Rgb(bg[0], bg[1], bg[2]))),
        area,
    );
    if image {
        let tank = Rect {
            height: area.height.saturating_sub(1),
            ..area
        };
        app.pet_watch.desired = Some(tank);
        frame.render_widget(
            Paragraph::new(label).style(chrome_style(&app.ui_theme, ChromeInk::Metadata)),
            Rect {
                y: area.bottom() - 1,
                height: 1,
                ..area
            },
        );
    } else {
        crate::tui::ambient_life::pet_widget::render_grid(
            area,
            frame.buffer_mut(),
            raster
                .filter(|r| r.width == area.width && r.height == area.height.saturating_sub(1))
                .map_or(&[], |r| r.cells.as_slice()),
            &label,
            Style::default().fg(ink),
        );
    }
}
pub fn render_full(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(app.ui_theme.surface_bg)),
        area,
    );
    frame.render_widget(
        Paragraph::new(tr(app.ui_locale, MessageId::PetHabitatTitle))
            .style(chrome_style(&app.ui_theme, ChromeInk::Active)),
        Rect { height: 1, ..area },
    );
    let tank = Rect {
        x: area.x,
        y: area.y + 2,
        width: area.width,
        height: if app.pet_watch.work_complete {
            area.height.saturating_sub(5) / 3
        } else {
            area.height.saturating_sub(5)
        },
    };
    render_tank(frame, tank, app);
    if app.pet_watch.work_complete {
        let result_area = Rect {
            x: area.x.saturating_add(3),
            y: tank.bottom().saturating_add(1),
            width: area.width.saturating_sub(6),
            height: area
                .bottom()
                .saturating_sub(tank.bottom())
                .saturating_sub(4),
        };
        let result = app
            .history
            .iter()
            .skip(app.pet_watch.work_history_start)
            .rfind(|cell| {
                matches!(
                    cell,
                    crate::tui::history::HistoryCell::Assistant { .. }
                        | crate::tui::history::HistoryCell::Error { .. }
                )
            });
        let lines = result
            .map(|cell| cell.transcript_lines(result_area.width))
            .unwrap_or_else(|| {
                vec![ratatui::text::Line::from(
                    tr(app.ui_locale, MessageId::NotificationTurnComplete).into_owned(),
                )]
            });
        app.pet_watch.result_scroll = app.pet_watch.result_scroll.min(
            lines
                .len()
                .saturating_sub(usize::from(result_area.height))
                .min(usize::from(u16::MAX)) as u16,
        );
        frame.render_widget(
            Paragraph::new(lines).scroll((app.pet_watch.result_scroll, 0)),
            result_area,
        );
    }
    let hints = if app.pet_watch.work_complete {
        format!(
            "↑↓ / PgUp/PgDn {} · {}",
            tr(app.ui_locale, MessageId::SetupActionScrollBody),
            habitat::hints(app.ui_locale)
        )
    } else {
        habitat::hints(app.ui_locale)
    };
    frame.render_widget(
        Paragraph::new(hints).style(chrome_style(&app.ui_theme, ChromeInk::Metadata)),
        Rect {
            x: area.x,
            y: area.bottom().saturating_sub(2),
            width: area.width,
            height: 2,
        },
    );
}
pub(crate) fn clear_images(output: &mut impl Write) -> io::Result<()> {
    graphics::clear(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_completion_reveals_existing_answer_without_sending_text_to_owner() {
        use crate::core::events::TurnOutcomeStatus;
        let mut app =
            crate::test_support::test_app_with_options(crate::test_support::test_tui_options("."));
        app.onboarding = crate::tui::app::OnboardingState::None;
        app.redaction_gate = false;
        assert!(app.view_stack.is_empty());
        app.input = "retained draft".into();
        app.pet_watch.session = app.current_session_id.clone();
        app.pet_watch.failed = true; // No connection or provider for this shell test.
        app.pet_watch.enabled = true;
        observe(
            &mut app,
            &Event::TurnStarted {
                turn_id: "preview-turn".into(),
                created_at: chrono::Utc::now(),
                route: None,
            },
            Instant::now(),
        );
        tick(&mut app, Instant::now());
        assert_eq!(app.view_stack.top_kind(), Some(ModalKind::PetHabitat));
        app.add_message(crate::tui::history::HistoryCell::Assistant {
            content: "Prepared result stays in the transcript".into(),
            streaming: false,
        });
        observe(
            &mut app,
            &Event::TurnComplete {
                usage: Default::default(),
                parent_route_usage: Default::default(),
                routed_usage_dropped_records: 0,
                status: TurnOutcomeStatus::Completed,
                error: None,
                tool_catalog: None,
                base_url: None,
            },
            Instant::now(),
        );
        assert!(app.pet_watch.work_complete);
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 40)).unwrap();
        terminal.draw(|frame| render_full(frame, &mut app)).unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(text.contains("Prepared result stays in the transcript"));
        assert_eq!(app.input, "retained draft");
        assert!(app.pet_watch.worker.is_none());
    }

    #[test]
    fn pet_off_stops_automatic_entry_and_keeps_the_draft() {
        let mut app =
            crate::test_support::test_app_with_options(crate::test_support::test_tui_options("."));
        app.onboarding = crate::tui::app::OnboardingState::None;
        app.redaction_gate = false;
        app.input = "kept draft".into();
        app.pet_watch.session = app.current_session_id.clone();
        app.pet_watch.detach_for_test();
        app.pet_watch.enabled = true;
        observe(
            &mut app,
            &Event::TurnStarted {
                turn_id: "turn".into(),
                created_at: chrono::Utc::now(),
                route: None,
            },
            Instant::now(),
        );
        assert!(app.pet_watch.work_enter_pending);
        set_enabled(&mut app, false);
        tick(&mut app, Instant::now());
        assert!(!app.pet_watch.enabled);
        assert!(!app.pet_watch.work_enter_pending);
        assert!(app.view_stack.is_empty());
        assert_eq!(app.input, "kept draft");
        assert!(app.pet_watch.worker.is_none());
    }

    #[test]
    fn foreground_projection_keeps_lifecycle_and_excludes_private_payloads() {
        let call = metadata(&Event::ToolCallStarted {
            id: "call-a".into(),
            name: "exec_command".into(),
            input: json!({"command":"PRIVATE TOOL INPUT"}),
        })
        .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&call).unwrap(),
            json!({
                "event":"tool_call_started", "tool_call_id":"call-a", "tool_name":"exec_command",
            })
        );
        let thought = metadata(&Event::ThinkingDelta {
            index: 2,
            content: "PRIVATE REASONING".into(),
        })
        .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&thought).unwrap(),
            json!({"event":"response_delta","index":2,"channel":"reasoning"})
        );
        let message = metadata(&Event::MessageDelta {
            index: 3,
            content: "PRIVATE MESSAGE".into(),
        })
        .unwrap();
        assert!(!message.contains("PRIVATE"));
        assert!(!call.contains("PRIVATE"));
        assert!(!thought.contains("PRIVATE"));
    }
}
