//! Views of one durable local pet. Engine events are projected here once;
//! the companion owns simulation, persistence and the sole audio output.
use crate::core::events::{Event, TurnOutcomeStatus};
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
use serde_json::json;
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
    active_turn_id: Option<String>,
    last_tick: Option<Instant>,
    failed: bool,
    /// The companion said it cannot be reached. Cleared by the next frame.
    unavailable: bool,
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
        self.active_turn_id = None;
        self.raster = None;
        self.failed = false;
        self.unavailable = false;
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
            // A live view follows the session switch: restart it for the new
            // session and forward the event that switched, so the reducer
            // sees the turn that opened it. Without a live view, the next
            // render starts one; turn tracking below still records the turn.
            let had_worker = self.worker.is_some();
            self.reset(session.map(str::to_owned));
            if had_worker {
                self.ensure(self.session.clone());
            }
        }
        if let Event::TurnStarted { turn_id, .. } = event {
            self.active_turn_id = Some(turn_id.clone());
        }
        if let Some(text) = metadata(event, self.active_turn_id.as_deref()) {
            self.send(Command::Observe(text));
        }
        if matches!(event, Event::TurnComplete { .. }) {
            self.active_turn_id = None;
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
fn metadata(event: &Event, turn_id: Option<&str>) -> Option<String> {
    let value = match event {
        Event::TurnStarted { turn_id, .. } => json!({"event":"turn_started","turn_id":turn_id}),
        Event::TurnComplete { status, .. } => {
            let mut value = json!({
                "event":"turn_complete",
                "turn_outcome":match status {
                    TurnOutcomeStatus::Completed => "completed",
                    TurnOutcomeStatus::Interrupted => "interrupted",
                    TurnOutcomeStatus::Failed => "failed",
                }
            });
            // An untracked turn omits the id; the reducer rejects `null`, and
            // without an id a completed turn cannot claim Done.
            if let Some(turn_id) = turn_id {
                value["turn_id"] = json!(turn_id);
            }
            value
        }
        Event::MessageStarted { index } => json!({"event":"message_started","index":index}),
        Event::MessageDelta { index, .. } => {
            json!({"event":"response_delta","index":index,"channel":"text"})
        }
        Event::MessageComplete { index } => json!({"event":"message_complete","index":index}),
        Event::ThinkingStarted { index } => json!({"event":"thinking_started","index":index}),
        Event::ThinkingDelta { index, .. } => {
            json!({"event":"response_delta","index":index,"channel":"reasoning"})
        }
        Event::ThinkingComplete { index } => json!({"event":"thinking_complete","index":index}),
        Event::OperationActivityStarted {
            span_id,
            activity_kind,
        } => json!({
            "event":"operation_activity_started",
            "span_id":span_id,
            "activity_kind":activity_kind
        }),
        Event::OperationActivityCompleted {
            span_id,
            activity_kind,
            outcome,
        } => json!({
            "event":"operation_activity_completed",
            "span_id":span_id,
            "activity_kind":activity_kind,
            "outcome":outcome
        }),
        Event::ToolCallHeartbeat => json!({"event":"tool_call_heartbeat"}),
        // A denied (by a human or by policy) or cancelled call ends any wait
        // on it. Only the Engine call id and typed outcome cross; the tool
        // name and error text stay in the transcript. Approvals that run are
        // cleared by their operation start and by the shell's typed
        // `waiting` flag on every tick.
        Event::ToolCallComplete {
            id,
            result: Err(crate::tools::spec::ToolError::PermissionDenied { .. }),
            ..
        } => json!({
            "event":"approval_resolved","id":id,"outcome":"denied"
        }),
        Event::ToolCallComplete {
            id,
            result: Err(crate::tools::spec::ToolError::Cancelled { .. }),
            ..
        } => json!({
            "event":"approval_resolved","id":id,"outcome":"cancelled"
        }),
        Event::AgentSpawned { id, .. } => json!({"event":"agent_spawned","id":id}),
        Event::AgentProgress { id, activity, .. } => json!({
            "event":"agent_progress",
            "id":id,
            "worker_status":crate::core::protocol_parity::worker_status_str(activity.worker_status)
        }),
        Event::AgentComplete { id, .. } => json!({"event":"agent_complete","id":id}),
        Event::ApprovalRequired { id, .. } | Event::UserInputRequired { id, .. } => json!({
            "event":if matches!(event, Event::ApprovalRequired { .. }) {"approval_required"} else {"user_input_required"},
            "id":id
        }),
        _ => return None,
    };
    let json = serde_json::to_string(&value).ok()?;
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
    } else if let Event::TurnComplete { status, .. } = event {
        app.pet_watch.work_enter_pending = false;
        app.pet_watch.work_complete = *status == TurnOutcomeStatus::Completed
            && app.view_stack.top_kind() == Some(ModalKind::PetHabitat);
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
        // Pet mode never hides a running turn behind a companion that is
        // not there; the transcript stays in view until it answers again.
        if !app.pet_watch.unavailable {
            open_habitat(app);
        }
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
        state.unavailable = false;
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
            // A refused action (select, export, open) leaves a reachable
            // companion and the habitat as they are.
            Notice::Message(message) => (
                format!(
                    "{} · {message}",
                    tr(app.ui_locale, MessageId::PetWatchUnavailable)
                ),
                StatusToastLevel::Warning,
            ),
            Notice::Unreachable(message) => {
                app.pet_watch.unavailable = true;
                if app.is_loading && is_open(app) {
                    app.view_stack.pop();
                }
                (
                    format!(
                        "{} · {message}",
                        tr(app.ui_locale, MessageId::PetWatchUnavailable)
                    ),
                    StatusToastLevel::Warning,
                )
            }
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
    let scene = raster
        .map(|r| {
            let mut text = format!(
                "{} · {} · {}",
                r.scene.style.channel, r.scene.style.arch, r.scene.behaviour
            );
            if let Some(activity) = &r.scene.activity
                && activity.observed
                && activity.freshness == codewhale_protocol::engine_owner::OwnerFreshness::Fresh
                && r.frame_changed.elapsed().as_millis() < 800
            {
                if let Some(kind) = activity.activity_kind {
                    text = format!("{} · {}", kind.as_str(), text);
                }
                if activity.parallel_agent_count > 0 {
                    text.push_str(&format!(" · ×{}", activity.parallel_agent_count));
                }
            }
            text
        })
        .unwrap_or_default();
    // With no companion frame the tank still paints the resting whale and
    // says why, instead of a blank tank under an orphan separator.
    let label = if raster.is_none() && app.pet_watch.unavailable {
        tr(app.ui_locale, MessageId::PetOffline).into_owned()
    } else {
        let presence = hollow.then(|| tr(app.ui_locale, MessageId::PetUnobserved));
        let sound = tr(app.ui_locale, app.pet_watch.sound_label());
        [
            Some(scene.as_str()),
            presence.as_deref(),
            Some(sound.as_ref()),
        ]
        .into_iter()
        .flatten()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
    };
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
        if raster.is_none() {
            paint_resting_whale(frame, area, Style::default().fg(ink));
        }
    }
}

/// The launch screen's braille whale, centred in the tank above its caption,
/// at the largest rung that fits. Static: it rests until the companion's own
/// frames take over the tank.
fn paint_resting_whale(frame: &mut Frame, area: Rect, style: Style) {
    use crate::tui::mark::MarkSize;
    let tank_height = area.height.saturating_sub(1);
    let Some(size) = [MarkSize::Large, MarkSize::Small, MarkSize::Tiny]
        .into_iter()
        .find(|size| {
            let (cols, rows) = size.cells();
            cols <= area.width && rows <= tank_height
        })
    else {
        return;
    };
    let (cols, rows) = size.cells();
    let x0 = area.x + (area.width - cols) / 2;
    let y0 = area.y + (tank_height - rows) / 2;
    let buf = frame.buffer_mut();
    for (dy, row) in size.rows().iter().enumerate() {
        for (dx, ch) in row.chars().enumerate() {
            if ch == ' ' {
                continue;
            }
            if let Some(cell) = buf.cell_mut((x0 + dx as u16, y0 + dy as u16)) {
                cell.set_char(ch).set_style(style);
            }
        }
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
    use serde_json::Value;

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
    fn unavailable_companion_paints_the_resting_whale_and_keeps_the_turn_visible() {
        let mut app =
            crate::test_support::test_app_with_options(crate::test_support::test_tui_options("."));
        app.onboarding = crate::tui::app::OnboardingState::None;
        app.redaction_gate = false;
        app.pet_watch.session = app.current_session_id.clone();
        app.pet_watch.detach_for_test();
        app.pet_watch.unavailable = true;
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(60, 16)).unwrap();
        terminal
            .draw(|frame| render_tank(frame, frame.area(), &mut app))
            .unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(
            text.contains(crate::tui::mark::MarkSize::Large.rows()[3].trim()),
            "the tank paints the resting whale: {text}"
        );
        assert!(
            text.contains("offline — codewhale pet serve wakes it"),
            "{text}"
        );
        assert!(!text.contains(" · offline"), "no orphan separator: {text}");

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
        tick(&mut app, Instant::now());
        assert!(
            app.view_stack.is_empty(),
            "pet mode must not cover a turn while the companion is unavailable"
        );
    }

    #[test]
    fn a_refused_pet_action_keeps_the_habitat_and_only_unreachable_marks_offline() {
        let mut app =
            crate::test_support::test_app_with_options(crate::test_support::test_tui_options("."));
        app.onboarding = crate::tui::app::OnboardingState::None;
        app.redaction_gate = false;
        app.pet_watch.session = app.current_session_id.clone();
        app.pet_watch.detach_for_test();
        let (tx, _commands) = std::sync::mpsc::sync_channel(4);
        let (notices_tx, notices) = std::sync::mpsc::sync_channel(4);
        app.pet_watch.worker = Some(Worker {
            tx,
            latest: std::sync::Arc::new(std::sync::Mutex::new(None)),
            view: std::sync::Arc::new(std::sync::Mutex::new(live::View::default())),
            notices,
        });
        open_habitat(&mut app);
        app.is_loading = true;

        notices_tx
            .send(Notice::Message(
                "Save the terminal session before exporting".into(),
            ))
            .unwrap();
        tick(&mut app, Instant::now());
        assert!(is_open(&app), "a refused export must not close pet mode");
        assert!(
            !app.pet_watch.unavailable,
            "a refused export is not offline"
        );

        notices_tx
            .send(Notice::Unreachable("Shared pet reconnecting".into()))
            .unwrap();
        tick(&mut app, Instant::now());
        assert!(app.pet_watch.unavailable);
        assert!(
            !is_open(&app),
            "an unreachable companion hands the running turn back"
        );
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
    fn only_completed_turns_set_the_pet_done_state() {
        let mut app =
            crate::test_support::test_app_with_options(crate::test_support::test_tui_options("."));
        app.onboarding = crate::tui::app::OnboardingState::None;
        app.redaction_gate = false;
        app.pet_watch.detach_for_test();
        open_habitat(&mut app);

        let finish = |status| Event::TurnComplete {
            usage: Default::default(),
            parent_route_usage: Default::default(),
            routed_usage_dropped_records: 0,
            status,
            error: None,
            tool_catalog: None,
            base_url: None,
        };
        for status in [TurnOutcomeStatus::Interrupted, TurnOutcomeStatus::Failed] {
            observe(
                &mut app,
                &Event::TurnStarted {
                    turn_id: format!("turn-{:?}", status),
                    created_at: chrono::Utc::now(),
                    route: None,
                },
                Instant::now(),
            );
            observe(&mut app, &finish(status), Instant::now());
            assert!(!app.pet_watch.work_complete, "{status:?} cannot mark Done");
        }

        observe(
            &mut app,
            &Event::TurnStarted {
                turn_id: "turn-completed".into(),
                created_at: chrono::Utc::now(),
                route: None,
            },
            Instant::now(),
        );
        observe(
            &mut app,
            &finish(TurnOutcomeStatus::Completed),
            Instant::now(),
        );
        assert!(app.pet_watch.work_complete);
        assert!(app.pet_watch.worker.is_none());
    }

    #[test]
    fn session_switch_resets_the_view_and_tracks_the_switching_turn() {
        let mut watch = PetWatch::default();
        watch.detach_for_test();
        watch.session = Some("session-a".into());
        watch.active_turn_id = Some("turn-a".into());
        watch.work_complete = true;

        watch.observe(
            &Event::TurnStarted {
                turn_id: "turn-b".into(),
                created_at: chrono::Utc::now(),
                route: None,
            },
            Some("session-b"),
            Instant::now(),
        );
        assert_eq!(watch.session.as_deref(), Some("session-b"));
        // The event that switched sessions is not dropped: its turn is the
        // one a later `turn_complete` must carry to claim Done.
        assert_eq!(watch.active_turn_id.as_deref(), Some("turn-b"));
        assert!(!watch.work_complete, "Done never survives a session switch");
        assert!(!watch.failed, "a switch clears a prior failure");
        assert!(
            watch.worker.is_none(),
            "with no live view running, an Engine event never starts one"
        );

        let done = metadata(
            &Event::TurnComplete {
                usage: Default::default(),
                parent_route_usage: Default::default(),
                routed_usage_dropped_records: 0,
                status: TurnOutcomeStatus::Completed,
                error: None,
                tool_catalog: None,
                base_url: None,
            },
            watch.active_turn_id.as_deref(),
        )
        .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&done).unwrap(),
            json!({"event":"turn_complete","turn_id":"turn-b","turn_outcome":"completed"})
        );
    }

    #[test]
    fn foreground_projection_forwards_only_typed_owner_metadata() {
        let call = metadata(
            &Event::ToolCallStarted {
                id: "call-a".into(),
                name: "exec_command".into(),
                input: json!({"command":"PRIVATE TOOL INPUT"}),
            },
            Some("turn-a"),
        );
        assert!(
            call.is_none(),
            "tool names and inputs are not activity facts"
        );

        let thought = metadata(
            &Event::ThinkingDelta {
                index: 2,
                content: "PRIVATE REASONING".into(),
            },
            None,
        )
        .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&thought).unwrap(),
            json!({"event":"response_delta","index":2,"channel":"reasoning"})
        );
        let message = metadata(
            &Event::MessageDelta {
                index: 3,
                content: "PRIVATE MESSAGE".into(),
            },
            None,
        )
        .unwrap();

        let operation = metadata(
            &Event::OperationActivityStarted {
                span_id: "private-internal-span".into(),
                activity_kind: codewhale_protocol::engine_owner::OwnerActivityKind::Computer,
            },
            None,
        )
        .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&operation).unwrap(),
            json!({
                "event":"operation_activity_started",
                "span_id":"private-internal-span",
                "activity_kind":"computer"
            })
        );
        let completed = metadata(
            &Event::OperationActivityCompleted {
                span_id: "private-internal-span".into(),
                activity_kind: codewhale_protocol::engine_owner::OwnerActivityKind::Editing,
                outcome: codewhale_protocol::engine_owner::OwnerOperationOutcome::Denied,
            },
            None,
        )
        .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&completed).unwrap(),
            json!({
                "event":"operation_activity_completed",
                "span_id":"private-internal-span",
                "activity_kind":"editing",
                "outcome":"denied"
            })
        );

        let turn = metadata(
            &Event::TurnComplete {
                usage: Default::default(),
                parent_route_usage: Default::default(),
                routed_usage_dropped_records: 0,
                status: TurnOutcomeStatus::Completed,
                error: None,
                tool_catalog: None,
                base_url: None,
            },
            Some("stable-turn-a"),
        )
        .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&turn).unwrap(),
            json!({
                "event":"turn_complete",
                "turn_id":"stable-turn-a",
                "turn_outcome":"completed"
            })
        );

        assert!(!message.contains("PRIVATE"));
        assert!(call.is_none());
        assert!(!thought.contains("PRIVATE"));
        assert!(!operation.contains("tool_name"));
        assert!(!operation.contains("PRIVATE"));
    }

    /// The pet's multi-agent count is derived on the JS side from
    /// `agent:`-prefixed spans, keyed by the `id` this projection forwards
    /// (app-side issue #12). The counting itself is proven in
    /// `pet/tests/pet-engine.test.mjs`; the handoff is the half that fails
    /// silently — a trimmed allowlist or a dropped id zeroes `parallel` with
    /// no error and no log line, and nothing else here covers agent events.
    /// Pin the wire shape the JS dispatches on, and keep child text off it.
    #[test]
    fn agent_events_forward_span_identity_without_child_text() {
        use crate::core::events::AgentProgressEventMeta;
        use crate::tools::subagent::{AgentWorkerStatus, SubAgentStatus};

        let spawned = metadata(
            &Event::AgentSpawned {
                owner_session_id: "session-a".into(),
                id: "agent-1".into(),
                prompt: "PRIVATE CHILD PROMPT".into(),
                worker_status: Some(AgentWorkerStatus::Running),
                parent_run_id: Some("run-9".into()),
                spawn_depth: 2,
                model: "PRIVATE CHILD MODEL".into(),
                route_source: Some("task.model".into()),
            },
            None,
        )
        .expect("agent spawns are observed");
        assert_eq!(
            serde_json::from_str::<Value>(&spawned).unwrap(),
            // A spawn opens the span; the reducer reads no status from it.
            json!({"event":"agent_spawned","id":"agent-1"})
        );

        // The JS finishes a span when progress reports a terminal status.
        let progress = metadata(
            &Event::AgentProgress {
                owner_session_id: "session-a".into(),
                id: "agent-1".into(),
                status: "PRIVATE PROGRESS TEXT".into(),
                activity: AgentProgressEventMeta {
                    worker_status: AgentWorkerStatus::Completed,
                    step: Some(3),
                    tool_name: Some("exec_command".into()),
                    routine_wait: false,
                    approval_id: None,
                },
                parent_run_id: Some("run-9".into()),
                spawn_depth: 2,
            },
            None,
        )
        .expect("agent progress is observed");
        assert_eq!(
            serde_json::from_str::<Value>(&progress).unwrap(),
            json!({"event":"agent_progress","id":"agent-1","worker_status":"completed"})
        );

        let complete = metadata(
            &Event::AgentComplete {
                owner_session_id: "session-a".into(),
                id: "agent-1".into(),
                result: "PRIVATE CHILD RESULT".into(),
                outcome: Some(SubAgentStatus::Completed),
                parent_run_id: Some("run-9".into()),
                spawn_depth: Some(2),
                continuable: Some(false),
                usage: None,
            },
            None,
        )
        .expect("agent completions are observed");
        assert_eq!(
            serde_json::from_str::<Value>(&complete).unwrap(),
            json!({"event":"agent_complete","id":"agent-1"})
        );

        for (label, payload) in [
            ("spawned", &spawned),
            ("progress", &progress),
            ("complete", &complete),
        ] {
            assert!(
                !payload.contains("PRIVATE"),
                "{label} leaked child text: {payload}"
            );
        }
    }
}
