//! Foreground Engine observation in the existing work surface. The one
//! Whalesong bucketer/world runs in an isolated, bounded QuickJS worker.
//! Rendering and audio policy belong to this host; telemetry semantics do not.
use std::time::{Duration, Instant};

use codewhale_localization::{MessageId, tr};
use codewhale_palette::{ChromeInk, chrome_style};
use ratatui::{Frame, layout::Rect, style::Modifier};
use serde_json::{Value, json};
use unicode_width::UnicodeWidthStr;

use crate::core::events::Event;
use crate::core::protocol_parity::{ProtocolIds, event_to_protocol};
use crate::tui::ambient_life::pet_sim::ChannelId;
use crate::tui::app::{App, StatusToastLevel};
use crate::tui::underwater::ShellPhase;
use crate::tui::work_surface::RailPanel;

mod audio;
mod persistence;
mod worker;
use worker::{Command, Notice, Raster, Worker};

#[derive(Default)]
pub struct PetWatch {
    worker: Option<Worker>,
    session: Option<String>,
    origin: Option<Instant>,
    last_tick: Option<Instant>,
    failed: bool,
    exporting: bool,
    sound_requested: bool,
    audio: Option<audio::Output>,
    pub(crate) area: Option<Rect>,
    raster: Option<Raster>,
}

impl PetWatch {
    pub fn set_sound(&mut self, enabled: bool) {
        self.sound_requested = enabled;
        if !enabled {
            self.audio = None;
        }
    }

    pub fn sound_label(&self) -> MessageId {
        if !self.sound_requested {
            MessageId::PetWatchSoundOff
        } else if self.audio.is_some() {
            MessageId::PetWatchSoundOn
        } else {
            MessageId::PetWatchSoundPaused
        }
    }

    fn reset(&mut self, session: Option<String>) {
        // The app-local user preference survives session selection; the old
        // stream and all of its queued sound are cancelled with this reset.
        *self = Self {
            session,
            sound_requested: self.sound_requested,
            ..Self::default()
        };
    }

    pub fn export(&mut self) -> bool {
        if self.worker.is_none() || self.session.is_none() || self.failed || self.exporting {
            return false;
        }
        self.send(Command::Export);
        self.exporting = !self.failed;
        self.exporting
    }

    pub fn retry(&mut self) {
        if self.failed {
            self.reset(self.session.clone());
        }
    }

    pub fn observe(&mut self, event: &Event, session: Option<&str>, now: Instant) {
        if self.session.as_deref() != session {
            self.reset(session.map(str::to_owned));
            return;
        }
        if self.worker.is_none() {
            return;
        }
        if let Some(json) = metadata(event) {
            let time_ms = self.origin.map_or(0.0, |origin| {
                now.duration_since(origin).as_secs_f64() * 1000.0
            });
            self.send(Command::Observe { json, time_ms });
        }
    }

    fn send(&mut self, command: Command) {
        if self
            .worker
            .as_ref()
            .is_some_and(|worker| worker.tx.try_send(command).is_err())
        {
            self.failed = true;
            self.worker = None;
            self.raster = None;
            self.audio = None;
        }
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

/// Called by the existing terminal event loop. No renderer reads the clock or
/// asks for a second draw loop. Still mode updates facts at bucket cadence.
pub fn tick(app: &mut App, now: Instant, obscured: bool) {
    let visible = app.work_surface.panel == RailPanel::Watch
        && app.work_surface.last_area.is_some()
        && !app.work_surface.dismissed
        && !obscured;
    let phase = ShellPhase::from_app(app);
    let motion = visible
        && matches!(phase, ShellPhase::Working | ShellPhase::Verifying)
        && crate::tui::underwater::decorative_shell_motion_enabled(app);
    let waiting = matches!(phase, ShellPhase::Waiting | ShellPhase::Approval);
    let state = &mut app.pet_watch;
    if state.session != app.current_session_id {
        state.reset(app.current_session_id.clone());
    }
    if visible && state.worker.is_none() && !state.failed {
        state.origin = Some(now);
        match Worker::start(state.session.clone()) {
            Ok(worker) => {
                state.worker = Some(worker);
                state.origin = Some(now);
                state.last_tick = None;
            }
            Err(_) => state.failed = true,
        }
    }
    let update = state
        .worker
        .as_ref()
        .and_then(|worker| worker.latest.lock().ok().and_then(|mut slot| slot.take()));
    match update {
        Some(Ok(raster)) => {
            if visible
                && state
                    .raster
                    .as_ref()
                    .is_none_or(|old| !old.same_picture(&raster))
            {
                app.needs_redraw = true;
            }
            state.raster = Some(raster);
        }
        Some(Err(())) => {
            state.worker = None;
            state.raster = None;
            state.failed = true;
        }
        None => {}
    }
    // A slow or stalled worker cannot keep asserting its old observed frame.
    // The UI's clock checks freshness; the renderer never reads wall time.
    if state.raster.as_ref().is_some_and(|raster| {
        state.origin.is_some_and(|origin| {
            now.duration_since(origin).as_secs_f64() * 1000.0 - raster.host_time_ms > 800.0
        })
    }) {
        state.raster = None;
        if visible {
            app.needs_redraw = true;
        }
    }
    let previous_sound = state.sound_label();
    let mut audio_failed = state.audio.as_ref().is_some_and(audio::Output::failed);
    let sound_allowed = visible
        && state.worker.is_some()
        && state.raster.is_some()
        && !state.failed
        && app.onboarding == crate::tui::app::OnboardingState::None
        && !app.launch.visible
        && app.view_stack.is_empty()
        && !app.notification_settings.quiet
        && !app.notification_settings.event_sound.quiet;
    if state.sound_requested && sound_allowed && !audio_failed {
        if state.audio.is_none() {
            match audio::Output::start() {
                Ok(output) => state.audio = Some(output),
                Err(_) => audio_failed = true,
            }
        }
    } else {
        state.audio = None;
    }
    if audio_failed {
        state.sound_requested = false;
        state.audio = None;
    }
    if visible && previous_sound != state.sound_label() {
        app.needs_redraw = true;
    }
    if state.worker.is_some()
        && state.last_tick.is_none_or(|last| {
            now.duration_since(last) >= Duration::from_millis(if motion { 33 } else { 400 })
        })
    {
        let elapsed = state.origin.map_or(0.0, |origin| {
            now.duration_since(origin).as_secs_f64() * 1000.0
        });
        let area = state.area.unwrap_or(Rect::new(0, 0, 40, 8));
        state.send(Command::Advance {
            time_ms: elapsed,
            motion,
            waiting,
            width: area.width.clamp(1, 512),
            height: area.height.saturating_sub(1).clamp(1, 512),
            audio: state.audio.as_ref().map(audio::Output::target),
        });
        state.last_tick = Some(now);
    }
    if state.failed && state.origin.take().is_some() {
        app.push_status_toast(
            tr(app.ui_locale, MessageId::PetWatchUnavailable).into_owned(),
            StatusToastLevel::Warning,
            Some(8_000),
        );
        app.needs_redraw = true;
    }
    if audio_failed {
        app.push_status_toast(
            tr(app.ui_locale, MessageId::PetWatchSoundUnavailable).into_owned(),
            StatusToastLevel::Warning,
            Some(12_000),
        );
        app.needs_redraw = true;
    }
    let notices: Vec<_> = app
        .pet_watch
        .worker
        .as_ref()
        .map(|worker| worker.notices.try_iter().collect())
        .unwrap_or_default();
    for notice in notices {
        let retain_receipt = matches!(&notice, Notice::Exported(_) | Notice::StorageUnavailable);
        let (text, level) = match notice {
            Notice::Restored => (
                tr(app.ui_locale, MessageId::PetWatchRestored).into_owned(),
                StatusToastLevel::Info,
            ),
            Notice::StorageUnavailable => (
                tr(app.ui_locale, MessageId::PetWatchStorageUnavailable)
                    .replace("{command}", "/workbar watch export"),
                StatusToastLevel::Warning,
            ),
            Notice::Exported(path) => {
                app.pet_watch.exporting = false;
                (
                    tr(app.ui_locale, MessageId::PetWatchExported)
                        .replace("{path}", &path.display().to_string()),
                    StatusToastLevel::Info,
                )
            }
            Notice::ExportFailed => {
                app.pet_watch.exporting = false;
                (
                    tr(app.ui_locale, MessageId::PetWatchExportFailed).into_owned(),
                    StatusToastLevel::Warning,
                )
            }
        };
        if retain_receipt {
            // Footer budgets can shorten notices. Explicit file receipts and
            // recovery instructions must also remain readable in the transcript.
            app.add_message(crate::tui::history::HistoryCell::System {
                content: text.clone(),
            });
        }
        app.push_status_toast(text, level, Some(12_000));
        app.needs_redraw = true;
    }
}

pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    app.pet_watch.area = Some(area);
    let raster = app.pet_watch.raster.as_ref();
    let hollow = raster.is_none_or(|r| r.hollow);
    let channel = raster
        .and_then(|r| ChannelId::from_key(&r.channel))
        .unwrap_or(ChannelId::Other);
    let arch = raster.map_or("drift", |r| r.arch.as_str());
    let mut label = format!(
        "{channel} · {arch}{}",
        if hollow {
            format!(" · {}", tr(app.ui_locale, MessageId::PetUnobserved))
        } else if raster.is_some_and(|r| r.dozing) {
            format!(" · {}", tr(app.ui_locale, MessageId::PetDozing))
        } else {
            String::new()
        }
    );
    let sound = tr(app.ui_locale, app.pet_watch.sound_label());
    // Keep the semantic/uncertainty cue before spending narrow cells on controls.
    if label.width() + sound.width() + 3 <= usize::from(area.width) {
        label.push_str(" · ");
        label.push_str(&sound);
    }
    let ink = if hollow {
        ChromeInk::Metadata
    } else {
        match channel {
            ChannelId::Error => ChromeInk::Failure,
            ChannelId::Human => ChromeInk::Waiting,
            _ => ChromeInk::Active,
        }
    };
    let mut style = chrome_style(&app.ui_theme, ink);
    if raster.is_some_and(|r| r.lit < 0.5) {
        style = style.add_modifier(Modifier::DIM);
    }
    let grid =
        raster.filter(|r| r.width == area.width && r.height == area.height.saturating_sub(1));
    crate::tui::ambient_life::pet_widget::render_grid(
        area,
        frame.buffer_mut(),
        grid.map_or(&[], |r| r.cells.as_slice()),
        &label,
        style,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

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
