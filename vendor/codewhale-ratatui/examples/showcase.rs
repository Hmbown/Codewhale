//! A live component gallery of Codewhale's actual terminal kit.
//!
//! F1-F6 choose Work, Decisions, Controls, Color, Life and Components.
//! F7 profile, F8 motion, F9 palette, F10 next work phase.
//! The composer starts focused. Enter queues while working; Escape interrupts
//! the illustrative turn and preserves the draft. Ctrl+C closes the gallery.
//! Shift+Tab changes permission; Ctrl+X focuses the native workbar.
//! Life: Left/Right studies an action; Space plays all seventeen native acts.
//! --frames DIR [--profile NAME] [--section NAME] exports actual-buffer SVGs.
//! The fixture never executes its displayed command or contacts a provider.

use std::{
    io,
    path::PathBuf,
    time::{Duration, Instant},
};

use codewhale_ratatui::{
    ApprovalOutcome, FormOutcome, FrameBudget, MotionMode, OmbreDirection, Paint, PickerOutcome,
    SegmentedOutcome, SegmentedState, TextInputOutcome, Theme, ToggleOutcome, TuiPalette,
    VerificationSpinner, WaterPalette, WhaleState, WorkbarOutcome, WorkbarPanel,
    detect::Appearance,
    gallery::showcase::{
        ShowcaseFrame, ShowcasePhase, ShowcaseSection, ShowcaseState, showcase_inputs,
    },
    spin,
    testing::{self, Profile},
    whale_motion::{ColoredGrid, Stage, Tier, colored_braille},
};
use crossterm::{
    event::{
        self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEvent, KeyEventKind,
        KeyModifiers,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend, layout::Rect};

const FRAME_INTERVAL: Duration = Duration::from_millis(200);
const FRAME_COUNT: u64 = 128;
const ACTION_FRAMES: u64 = 10;
const NATIVE_FINISH: Duration = Duration::from_millis(1_600);

/// One owner for all presentation state and the native whale's Stage. The
/// only Instant is supplied by this host; views receive an elapsed Duration.
struct Studio {
    view: ShowcaseState,
    started: Instant,
    stage: Stage,
    life_started: Duration,
    generation: u64,
    viewport: Rect,
}
impl Studio {
    fn new(now: Instant) -> Self {
        Self {
            view: ShowcaseState::new(),
            started: now,
            stage: Stage::new(),
            life_started: Duration::ZERO,
            generation: 0,
            viewport: Rect::new(0, 0, 104, 30),
        }
    }
    fn elapsed(&self, now: Instant) -> Duration {
        now.saturating_duration_since(self.started)
    }
    fn advance_phase(&mut self, elapsed: Duration) {
        let next = match self.view.phase {
            ShowcasePhase::Working => ShowcasePhase::NeedsYou,
            ShowcasePhase::NeedsYou => {
                self.view.approval_open = true;
                self.view.section = ShowcaseSection::Decisions;
                return;
            }
            ShowcasePhase::Verifying => ShowcasePhase::Done,
            ShowcasePhase::Done => ShowcasePhase::Working,
        };
        self.view.set_phase(next, elapsed);
        if next == ShowcasePhase::NeedsYou {
            self.view.section = ShowcaseSection::Decisions;
        } else if next == ShowcasePhase::Done && !self.view.queued.is_empty() {
            self.view.followups.push(self.view.queued.remove(0));
            self.view.set_phase(ShowcasePhase::Working, elapsed);
            self.view.section = ShowcaseSection::Work;
            self.view.editing = true;
        }
    }
    fn restart(&mut self, elapsed: Duration) {
        self.generation = self.generation.saturating_add(1);
        self.view.set_phase(ShowcasePhase::Working, elapsed);
        self.view.section = ShowcaseSection::Work;
        self.view.editing = true;
        self.view.dock.focused = false;
        self.view.note = "Restarted the example; your draft is retained.".into();
    }
    fn actor(&mut self, now: Instant, area: Rect, theme: &Theme) -> Option<ColoredGrid> {
        self.viewport = area;
        let elapsed = self.elapsed(now);
        if self.view.section == ShowcaseSection::Life && self.view.action_playing {
            self.view.action = ((elapsed.saturating_sub(self.life_started).as_millis() / 2_000)
                as usize)
                % WhaleState::ALL.len();
        }
        let size = ShowcaseFrame::new(&self.view, elapsed).whale_size(area);
        let finite_done = self.view.phase == ShowcasePhase::Done
            && self.view.phase_elapsed(elapsed) < NATIVE_FINISH;
        let moving = self.view.active_motion()
            || (finite_done
                && self.view.section == ShowcaseSection::Work
                && self.view.motion == MotionMode::Full);
        let action = if self.view.section == ShowcaseSection::Life {
            WhaleState::ALL[self.view.action.min(WhaleState::ALL.len() - 1)]
        } else {
            self.view.phase.whale()
        };
        let session = if self.view.section == ShowcaseSection::Life {
            format!("action-study-{}", action.key())
        } else {
            format!("component-example-{}", self.generation)
        };
        let inputs = showcase_inputs(action, &session);
        self.stage.observe(Some(&session), inputs, !moving);
        self.stage
            .set_visible(size.is_some() && !self.view.approval_open);
        if moving {
            self.stage.advance(now);
        }
        size.map(|(cols, rows)| {
            colored_braille(
                self.stage.director(),
                cols,
                rows,
                theme.caps().appearance != Appearance::Light,
            )
        })
    }
    fn next_frame_in(&self, now: Instant, area: Rect, theme: &Theme) -> Option<Duration> {
        let elapsed = self.elapsed(now);
        if self.view.motion != MotionMode::Full || self.view.approval_open {
            return None;
        }
        let mut budget = FrameBudget::new();
        if self.view.active_motion() {
            // One active spinner, one native actor, and one shared water
            // column ask the same host for a frame. The terminal ceiling
            // applies to the whole composition rather than each actor.
            if matches!(
                self.view.section,
                ShowcaseSection::Work | ShowcaseSection::Decisions
            ) && budget.claim_spinner()
            {
                let phase = self.view.phase_elapsed(elapsed);
                budget.request(match self.view.phase {
                    ShowcasePhase::Working => spin::next_frame_in(phase, self.view.motion),
                    ShowcasePhase::Verifying => {
                        VerificationSpinner::next_frame_in(phase, self.view.motion)
                    }
                    _ => None,
                });
            }
            if ShowcaseFrame::new(&self.view, elapsed)
                .whale_size(area)
                .is_some()
            {
                budget.request(self.stage.cadence(Tier::Terminal));
            }
            budget.request(Some(FRAME_INTERVAL));
        } else if self.view.section == ShowcaseSection::Work
            && self.view.phase == ShowcasePhase::Done
            && self.view.phase_elapsed(elapsed) < NATIVE_FINISH
        {
            budget.request(Some(FRAME_INTERVAL));
        }
        // No frame deadline after a finite completion, in a hidden scene,
        // while waiting for an answer, or under Reduced/Still motion.
        let _ = theme;
        budget
            .next_frame_in()
            .map(|d| d.max(Duration::from_secs_f64(1.0 / 6.0)))
    }
    fn select_section(&mut self, index: usize) {
        self.view.section = ShowcaseSection::ALL[index];
        self.view.editing = self.view.section == ShowcaseSection::Work;
        self.view.dock.focused = false;
        self.view.focus = if self.view.section == ShowcaseSection::Controls {
            2
        } else {
            0
        };
    }
    fn key(&mut self, key: KeyEvent, now: Instant) -> bool {
        if key.kind == KeyEventKind::Release {
            return false;
        }
        if key.kind != KeyEventKind::Press && !self.view.editing {
            return false;
        }
        let elapsed = self.elapsed(now);
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return true;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('r') {
            self.restart(elapsed);
            return false;
        }
        if let KeyCode::F(n @ 1..=6) = key.code {
            self.select_section(usize::from(n - 1));
            return false;
        }
        match key.code {
            KeyCode::F(7) => {
                cycle_profile(&mut self.view, key.modifiers.contains(KeyModifiers::SHIFT));
                return false;
            }
            KeyCode::F(8) => {
                self.view.motion = next_motion(self.view.motion);
                return false;
            }
            KeyCode::F(9) => {
                cycle_palette(&mut self.view, key.modifiers.contains(KeyModifiers::SHIFT));
                return false;
            }
            KeyCode::F(10) => {
                self.view.editing = false;
                self.advance_phase(elapsed);
                return false;
            }
            _ => {}
        }
        if self.view.approval_open
            && matches!(
                self.view.section,
                ShowcaseSection::Work | ShowcaseSection::Decisions
            )
        {
            match self.view.approval.handle_key(key) {
                ApprovalOutcome::Chose(id) if self.view.approval.grants(id) => {
                    self.view.set_phase(ShowcasePhase::Verifying, elapsed);
                    self.view.note = "Command approved in the example.".into();
                }
                ApprovalOutcome::Chose(_) | ApprovalOutcome::Cancelled => {
                    self.view.approval_open = false;
                    self.view.note = "Waiting for your answer; draft retained.".into();
                }
                ApprovalOutcome::Reveal => {
                    self.view.note = "The complete illustrative command is visible.".into();
                }
                _ => {}
            }
            return false;
        }
        if self.view.section == ShowcaseSection::Work {
            if !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
                && (key.code == KeyCode::BackTab
                    || (key.code == KeyCode::Tab && key.modifiers.contains(KeyModifiers::SHIFT)))
            {
                self.view.permission = self.view.permission.next();
                return false;
            }
            if key.modifiers == KeyModifiers::ALT && key.code == KeyCode::Char('w') {
                self.view.readouts.on = true;
                if self.view.dock.focused {
                    self.view.dock.focused = false;
                } else {
                    let rows = self.view.workbar().rows;
                    self.view.dock.handle_key(key, &rows, 2);
                }
                self.view.editing = !self.view.dock.focused;
                return false;
            }
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('x') {
                self.view.readouts.on = true;
                self.view.dock.panel = WorkbarPanel::Fleet;
                self.view.dock.focused = false;
                let rows = self.view.workbar().rows;
                self.view.dock.handle_key(
                    KeyEvent::new(KeyCode::Char('w'), KeyModifiers::ALT),
                    &rows,
                    2,
                );
                self.view.editing = false;
                return false;
            }
        }
        let panel_chord = key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(
                key.code,
                KeyCode::Tab | KeyCode::BackTab | KeyCode::Char(']')
            );
        if self.view.section == ShowcaseSection::Work && (!self.view.editing || panel_chord) {
            // The row facts are the same fixture supplied to the rendered dock.
            let dock = self.view.workbar();
            let theme = self.view.theme_for(&self.view.profile.theme());
            let region = ShowcaseFrame::new(&self.view, elapsed)
                .work_areas(self.viewport, &theme)
                .workbar;
            let visible_rows = dock.layout(region).visible_rows;
            match self.view.dock.handle_key(key, &dock.rows, visible_rows) {
                WorkbarOutcome::Close => {
                    self.view.readouts.on = false;
                    self.view.editing = true;
                    return false;
                }
                WorkbarOutcome::Panel(_) | WorkbarOutcome::Changed => {
                    self.view.readouts.on = true;
                    return false;
                }
                WorkbarOutcome::Activate(id) => {
                    self.view.note = format!("Selected {id}");
                    return false;
                }
                WorkbarOutcome::ReleaseFocus => {
                    self.view.editing = true;
                }
                WorkbarOutcome::Ignored => {}
            }
        }
        if self.view.editing {
            match self.view.section {
                ShowcaseSection::Work => match self.view.draft.handle_key(key) {
                    TextInputOutcome::Submitted => {
                        if !self.view.draft.text().trim().is_empty() {
                            if matches!(
                                self.view.phase,
                                ShowcasePhase::Working | ShowcasePhase::Verifying
                            ) {
                                self.view.queued.push(self.view.draft.text().to_owned());
                                self.view.draft.set_text("");
                            } else {
                                self.view.editing = false;
                                self.view.set_phase(ShowcasePhase::NeedsYou, elapsed);
                                self.view.section = ShowcaseSection::Decisions;
                            }
                        }
                    }
                    TextInputOutcome::Cancelled => {
                        self.view.editing = false;
                        if matches!(
                            self.view.phase,
                            ShowcasePhase::Working | ShowcasePhase::Verifying
                        ) {
                            self.view.set_phase(ShowcasePhase::NeedsYou, elapsed);
                            self.view.approval_open = false;
                        }
                    }
                    _ => {}
                },
                ShowcaseSection::Controls => match self.view.form.handle_key(key) {
                    FormOutcome::Submitted => {
                        self.view.editing = false;
                        self.view.note = "Settings applied to this example".into();
                    }
                    FormOutcome::Cancelled => self.view.editing = false,
                    _ => {}
                },
                ShowcaseSection::Components => {
                    let before = self.view.query.text().to_owned();
                    match self.view.query.handle_key(key) {
                        TextInputOutcome::Cancelled | TextInputOutcome::Submitted => {
                            self.view.editing = false
                        }
                        _ => {}
                    }
                    if self.view.query.text() != before {
                        self.view.picker.reset();
                        self.view.preview_scroll = 0;
                    }
                    self.catalogue_key(key);
                }
                _ => self.view.editing = false,
            }
            return false;
        }
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => return true,
            KeyCode::Tab | KeyCode::BackTab => {
                let count = match self.view.section {
                    ShowcaseSection::Controls => 3,
                    ShowcaseSection::Color => 4,
                    _ => 1,
                };
                self.view.focus = if key.code == KeyCode::BackTab {
                    (self.view.focus + count - 1) % count
                } else {
                    (self.view.focus + 1) % count
                };
            }
            KeyCode::Enter => match self.view.section {
                ShowcaseSection::Work | ShowcaseSection::Components => self.view.editing = true,
                ShowcaseSection::Controls if self.view.focus == 2 => self.view.editing = true,
                ShowcaseSection::Controls => self.control_key(key),
                _ => {}
            },
            _ => match self.view.section {
                ShowcaseSection::Controls => self.control_key(key),
                ShowcaseSection::Color => self.color_key(key),
                ShowcaseSection::Life => match key.code {
                    KeyCode::Left | KeyCode::Right => {
                        self.view.action = if key.code == KeyCode::Left {
                            (self.view.action + 16) % 17
                        } else {
                            (self.view.action + 1) % 17
                        };
                        self.view.action_playing = false;
                    }
                    KeyCode::Char(' ') => {
                        self.view.action_playing = !self.view.action_playing;
                        self.life_started = elapsed
                            .saturating_sub(Duration::from_secs(self.view.action as u64 * 2));
                    }
                    _ => {}
                },
                ShowcaseSection::Components => self.catalogue_key(key),
                _ => {}
            },
        }
        false
    }
    fn control_key(&mut self, key: KeyEvent) {
        let toggle = if self.view.focus == 0 {
            &mut self.view.details
        } else if self.view.focus == 1 {
            &mut self.view.readouts
        } else {
            return;
        };
        if let ToggleOutcome::Toggled(_) = toggle.handle_key(key, true) {
            self.view.note = "Choice applied.".into();
        }
    }
    fn color_key(&mut self, key: KeyEvent) {
        match self.view.focus {
            0 => {
                let current = WaterPalette::ALL
                    .iter()
                    .position(|v| *v == self.view.palette)
                    .unwrap_or(0);
                let mut state = SegmentedState::new(current);
                if let SegmentedOutcome::Selected(index) =
                    state.handle_key(key, WaterPalette::ALL.len(), true)
                {
                    self.view.palette = WaterPalette::ALL[index];
                }
            }
            1 => {
                let mut state = SegmentedState::new(match self.view.motion {
                    MotionMode::Full => 0,
                    MotionMode::Reduced => 1,
                    MotionMode::Still => 2,
                });
                if let SegmentedOutcome::Selected(index) = state.handle_key(key, 3, true) {
                    self.view.motion =
                        [MotionMode::Full, MotionMode::Reduced, MotionMode::Still][index];
                }
            }
            2 if matches!(key.code, KeyCode::Left | KeyCode::Right) => {
                cycle_profile(&mut self.view, key.code == KeyCode::Left)
            }
            3 if matches!(
                key.code,
                KeyCode::Left | KeyCode::Right | KeyCode::Char(' ')
            ) =>
            {
                self.view.direction = match self.view.direction {
                    OmbreDirection::Vertical => OmbreDirection::Diagonal,
                    OmbreDirection::Diagonal => OmbreDirection::Vertical,
                }
            }
            _ => {}
        }
    }
    fn catalogue_key(&mut self, key: KeyEvent) {
        if matches!(key.code, KeyCode::PageUp | KeyCode::PageDown) {
            self.view.preview_scroll = if key.code == KeyCode::PageUp {
                self.view.preview_scroll.saturating_sub(5)
            } else {
                self.view.preview_scroll.saturating_add(5)
            };
        } else if matches!(
            key.code,
            KeyCode::Up | KeyCode::Down | KeyCode::Home | KeyCode::End
        ) {
            let (_, _, matches) = self.view.catalogue();
            if self.view.picker.handle_key(key, matches.len(), 10) != PickerOutcome::Ignored {
                self.view.preview_scroll = 0;
            }
        } else if key.code == KeyCode::Char('/') {
            self.view.editing = true;
        }
    }
    fn paste(&mut self, value: &str) {
        if !self.view.editing {
            return;
        }
        match self.view.section {
            ShowcaseSection::Work => {
                self.view.draft.paste(value);
            }
            ShowcaseSection::Controls => {
                self.view.form.paste(value);
            }
            ShowcaseSection::Components => {
                self.view.query.paste(value);
                self.view.picker.reset();
            }
            _ => {}
        }
    }
}

fn next_motion(mode: MotionMode) -> MotionMode {
    match mode {
        MotionMode::Full => MotionMode::Reduced,
        MotionMode::Reduced => MotionMode::Still,
        MotionMode::Still => MotionMode::Full,
    }
}
fn cycle_profile(state: &mut ShowcaseState, reverse: bool) {
    let current = Profile::ALL
        .iter()
        .position(|p| *p == state.profile)
        .unwrap_or(0);
    state.profile = Profile::ALL
        [(current + if reverse { Profile::ALL.len() - 1 } else { 1 }) % Profile::ALL.len()];
}
fn cycle_palette(state: &mut ShowcaseState, reverse: bool) {
    let current = TuiPalette::ALL
        .iter()
        .position(|p| *p == state.native_palette)
        .unwrap_or(0);
    state.native_palette = TuiPalette::ALL[(current
        + if reverse {
            TuiPalette::ALL.len() - 1
        } else {
            1
        })
        % TuiPalette::ALL.len()];
}

struct Export {
    dir: PathBuf,
    profile: Profile,
    section: Option<ShowcaseSection>,
}
fn options(args: &[String]) -> io::Result<Option<Export>> {
    if args.is_empty() {
        return Ok(None);
    }
    let usage = || {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: showcase [--frames DIR [--profile NAME] [--section work|decisions|controls|color|life|components]]",
        )
    };
    if args.first().map(String::as_str) != Some("--frames")
        || args.len() < 2
        || !args.len().is_multiple_of(2)
    {
        return Err(usage());
    }
    let mut export = Export {
        dir: PathBuf::from(&args[1]),
        profile: Profile::DarkTrue,
        section: None,
    };
    let mut seen_profile = false;
    let mut seen_section = false;
    for pair in args[2..].as_chunks::<2>().0 {
        match pair[0].as_str() {
            "--profile" if !seen_profile => {
                export.profile = Profile::from_name(&pair[1]).ok_or_else(usage)?;
                seen_profile = true;
            }
            "--section" if !seen_section => {
                export.section = Some(
                    ShowcaseSection::ALL
                        .into_iter()
                        .find(|s| s.name().eq_ignore_ascii_case(&pair[1]))
                        .ok_or_else(usage)?,
                );
                seen_section = true;
            }
            _ => return Err(usage()),
        }
    }
    Ok(Some(export))
}
fn exported_state(studio: &mut Studio, elapsed: Duration, section: Option<ShowcaseSection>) {
    let ms = elapsed.as_millis();
    let (phase, started) = if ms < 5_000 {
        (ShowcasePhase::Working, 0)
    } else if ms < 9_000 {
        (ShowcasePhase::NeedsYou, 5_000)
    } else if ms < 13_000 {
        (ShowcasePhase::Verifying, 9_000)
    } else {
        (ShowcasePhase::Done, 13_000)
    };
    if studio.view.phase != phase {
        studio.view.set_phase(phase, Duration::from_millis(started));
    }
    studio.view.section = section.unwrap_or(if phase == ShowcasePhase::NeedsYou {
        ShowcaseSection::Decisions
    } else {
        ShowcaseSection::Work
    });
    if section == Some(ShowcaseSection::Life) {
        studio.view.action_playing = true;
        studio.view.approval_open = false;
    }
    if section == Some(ShowcaseSection::Color) {
        studio.view.palette = WaterPalette::ALL[((ms / 2_000) as usize) % WaterPalette::ALL.len()];
    }
}
fn export_frames(export: Export) -> io::Result<()> {
    std::fs::create_dir_all(&export.dir)?;
    let theme = export.profile.theme().tui();
    let base = Instant::now();
    let mut studio = Studio::new(base);
    studio.view.profile = export.profile;
    let count = if export.section == Some(ShowcaseSection::Life) {
        WhaleState::ALL.len() as u64 * ACTION_FRAMES
    } else {
        FRAME_COUNT
    };
    let mut manifest = Vec::new();
    for index in 0..count {
        let elapsed = Duration::from_millis(index * 200);
        exported_state(&mut studio, elapsed, export.section);
        let area = if matches!(
            export.section,
            None | Some(ShowcaseSection::Work | ShowcaseSection::Decisions)
        ) {
            Rect::new(0, 0, 104, 30)
        } else {
            Rect::new(0, 0, 112, 38)
        };
        let actor = studio.actor(base + elapsed, area, &theme);
        let buf = testing::render(area.width, area.height, |area, buf| {
            let frame = ShowcaseFrame::new(&studio.view, elapsed);
            if let Some(actor) = &actor {
                frame.whale(actor).paint(area, buf, &theme);
            } else {
                frame.paint(area, buf, &theme);
            }
        });
        let file = format!("frame-{index:03}.svg");
        std::fs::write(export.dir.join(&file), testing::svg(&buf, &theme))?;
        manifest.push(serde_json::json!({"file":file,"elapsed_ms":index*200,"profile":export.profile.name(),"width":area.width,"height":area.height}));
    }
    std::fs::write(
        export.dir.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    Ok(())
}

struct Restore;
impl Drop for Restore {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), DisableBracketedPaste, LeaveAlternateScreen);
    }
}
fn main() -> io::Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if let Some(export) = options(&args)? {
        return export_frames(export);
    }
    enable_raw_mode()?;
    let _restore = Restore;
    execute!(io::stdout(), EnterAlternateScreen, EnableBracketedPaste)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut studio = Studio::new(Instant::now());
    loop {
        let now = Instant::now();
        let elapsed = studio.elapsed(now);
        let theme = studio.view.theme_for(&studio.view.profile.theme());
        let size = terminal.size()?;
        let area = Rect::new(0, 0, size.width, size.height);
        let actor = studio.actor(now, area, &theme);
        terminal.draw(|frame| {
            let view = ShowcaseFrame::new(&studio.view, elapsed);
            let cursor = if studio.view.editing && studio.view.section == ShowcaseSection::Work {
                view.composer_cursor(frame.area(), &theme)
            } else {
                None
            };
            if let Some(actor) = &actor {
                view.whale(actor)
                    .paint(frame.area(), frame.buffer_mut(), &theme);
            } else {
                view.paint(frame.area(), frame.buffer_mut(), &theme);
            }
            if let Some(cursor) = cursor {
                frame.set_cursor_position(cursor);
            }
        })?;
        if let Some(after) = studio.next_frame_in(now, area, &theme)
            && !event::poll(after.saturating_sub(now.elapsed()))?
        {
            continue;
        }
        match event::read()? {
            Event::Key(key) => {
                if studio.key(key, Instant::now()) {
                    break;
                }
            }
            Event::Paste(value) => studio.paste(&value),
            Event::Resize(_, _) => {}
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use codewhale_ratatui::whale_motion::acting_for;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn quiet_scenes_wait_for_input_and_terminal_animation_has_one_ceiling() {
        let base = Instant::now();
        let area = Rect::new(0, 0, 112, 38);
        let theme = Profile::DarkTrue.theme();
        let mut studio = Studio::new(base);
        let now = base + Duration::from_secs(1);
        studio.actor(now, area, &theme);
        assert!(
            studio
                .next_frame_in(now, area, &theme)
                .is_some_and(|d| d >= Duration::from_secs_f64(1.0 / 6.0))
        );
        for mode in [MotionMode::Reduced, MotionMode::Still] {
            studio.view.motion = mode;
            assert_eq!(studio.next_frame_in(now, area, &theme), None);
        }
        studio.view.motion = MotionMode::Full;
        for section in [
            ShowcaseSection::Controls,
            ShowcaseSection::Color,
            ShowcaseSection::Components,
        ] {
            studio.view.section = section;
            assert_eq!(studio.next_frame_in(now, area, &theme), None);
        }
        studio.view.section = ShowcaseSection::Work;
        studio
            .view
            .set_phase(ShowcasePhase::NeedsYou, Duration::ZERO);
        assert_eq!(studio.next_frame_in(now, area, &theme), None);
        studio.view.set_phase(ShowcasePhase::Done, Duration::ZERO);
        assert_eq!(
            studio.next_frame_in(base + Duration::from_secs(4), area, &theme),
            None
        );
    }

    #[test]
    fn editable_unicode_draft_keeps_letters_and_escape_preserves_it() {
        let base = Instant::now();
        let mut studio = Studio::new(base);
        studio.view.draft.set_text("鲸鱼 cafe\u{301}");
        assert!(studio.view.editing);
        assert!(!studio.key(key(KeyCode::Char('q')), base));
        assert_eq!(studio.view.draft.text(), "鲸鱼 cafe\u{301}q");
        assert!(!studio.key(key(KeyCode::Esc), base));
        assert!(!studio.view.editing);
        assert_eq!(studio.view.draft.text(), "鲸鱼 cafe\u{301}q");
        assert_eq!(studio.view.phase, ShowcasePhase::NeedsYou);
        studio.restart(Duration::from_secs(2));
        assert_eq!(studio.view.draft.text(), "鲸鱼 cafe\u{301}q");
    }

    #[test]
    fn composer_focus_queue_permission_and_dock_keys_match_the_visible_chrome() {
        let now = Instant::now();
        let mut studio = Studio::new(now);
        assert!(studio.view.editing && !studio.view.dock.focused);
        let draft = studio.view.draft.text().to_owned();
        studio.key(key(KeyCode::Enter), now);
        assert_eq!(studio.view.queued, [draft]);
        assert!(studio.view.draft.text().is_empty());
        assert_eq!(studio.view.phase, ShowcasePhase::Working);
        studio.key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT), now);
        assert_eq!(studio.view.permission.word(), "auto-review");
        studio.key(
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
            now,
        );
        assert!(studio.view.dock.focused && !studio.view.editing);
        studio.key(key(KeyCode::Esc), now);
        assert!(studio.view.editing && !studio.view.dock.focused);
        studio.view.dock.panel = WorkbarPanel::Tasks;
        assert_eq!(studio.view.workbar().rows.len(), 2);
        studio
            .view
            .set_phase(ShowcasePhase::Done, Duration::from_secs(6));
        assert!(
            studio
                .view
                .workbar()
                .rows
                .iter()
                .all(|row| row.tone == codewhale_ratatui::WorkbarTone::Success)
        );
        assert_eq!(
            studio.view.workbar().progress.as_deref(),
            Some("TODO · 2/2 · done")
        );
    }

    #[test]
    fn dock_preserves_first_typed_key_panel_chords_and_narrow_selection() {
        let now = Instant::now();
        let mut studio = Studio::new(now);
        studio.viewport = Rect::new(0, 0, 40, 28);
        studio.key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::ALT), now);
        assert!(studio.view.dock.focused && studio.view.dock.selected.is_some());
        studio.key(key(KeyCode::Down), now);
        assert_eq!(studio.view.dock.offset, 1);
        let before = studio.view.draft.text().to_owned();
        studio.key(key(KeyCode::Char('q')), now);
        assert!(studio.view.editing && !studio.view.dock.focused);
        assert_eq!(studio.view.draft.text(), format!("{before}q"));
        studio.key(
            KeyEvent::new(
                KeyCode::BackTab,
                KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            ),
            now,
        );
        assert_eq!(studio.view.dock.panel, WorkbarPanel::Cost);
        assert_eq!(studio.view.permission.word(), "ask");
        for word in ["auto-review", "full access", "ask"] {
            studio.key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT), now);
            assert_eq!(studio.view.permission.word(), word);
        }
        studio.key(key(KeyCode::BackTab), now);
        assert_eq!(studio.view.permission.word(), "auto-review");
        studio.key(KeyEvent::new(KeyCode::Tab, KeyModifiers::SHIFT), now);
        assert_eq!(studio.view.permission.word(), "full access");
    }

    #[test]
    fn queued_followup_advances_after_completion_and_restart_keeps_pending_input() {
        let now = Instant::now();
        let mut studio = Studio::new(now);
        let draft = studio.view.draft.text().to_owned();
        studio.key(key(KeyCode::Enter), now);
        studio.restart(Duration::from_secs(1));
        assert_eq!(studio.view.queued, [draft.clone()]);
        studio
            .view
            .set_phase(ShowcasePhase::Verifying, Duration::from_secs(2));
        studio.advance_phase(Duration::from_secs(6));
        assert!(studio.view.queued.is_empty());
        assert_eq!(studio.view.followups, [draft]);
        assert_eq!(studio.view.phase, ShowcasePhase::Working);
        assert_eq!(studio.view.phase_started, Duration::from_secs(6));
    }

    #[test]
    fn work_surface_shares_native_water_and_light_has_no_underwater_habitat() {
        use codewhale_ratatui::{OceanColumn, OceanPhase};
        let state = ShowcaseState::new();
        let elapsed = Duration::from_millis(1_600);
        let frame = ShowcaseFrame::new(&state, elapsed);
        let theme = state.theme_for(&Profile::DarkTrue.theme());
        let area = Rect::new(0, 0, 104, 30);
        let regions = frame.work_areas(area, &theme);
        assert_eq!(regions.composer.height, 3);
        assert_eq!(regions.workbar.height, 5);
        let buf = testing::render(area.width, area.height, |a, b| frame.paint(a, b, &theme));
        let column = OceanColumn::new(elapsed, MotionMode::Full)
            .phase(OceanPhase::Working)
            .context_percent(24)
            .viewport(area);
        for y in [
            regions.composer.y + 1,
            regions.posture.y,
            regions.metrics.y,
            regions.workbar.y + 4,
        ] {
            assert_eq!(buf[(0, y)].bg, column.color_at_y(y, area, &theme).unwrap());
        }
        let text = testing::text(&buf);
        assert!(text.contains("send after this turn") && text.contains("ctx 24%"));
        assert!(!text.contains("Shift+Enter newline"));
        let light = state.theme_for(&Profile::LightTrue.theme());
        let buf = testing::render(area.width, area.height, |a, b| frame.paint(a, b, &light));
        for y in 12..regions.conversation.bottom() {
            assert!((0..area.width).all(|x| buf[(x, y)].symbol() == " "));
        }
    }

    #[test]
    fn decision_waits_for_an_explicit_answer_and_release_does_not_advance() {
        let base = Instant::now();
        let mut studio = Studio::new(base);
        studio.key(key(KeyCode::F(10)), base);
        assert_eq!(studio.view.phase, ShowcasePhase::NeedsYou);
        assert!(studio.view.approval_open);
        let mut release = key(KeyCode::Char('y'));
        release.kind = KeyEventKind::Release;
        studio.key(release, base);
        assert_eq!(studio.view.phase, ShowcasePhase::NeedsYou);
        studio.key(key(KeyCode::Esc), base);
        assert_eq!(studio.view.phase, ShowcasePhase::NeedsYou);
        assert!(!studio.view.approval_open);
        studio.key(key(KeyCode::F(10)), base);
        studio.key(key(KeyCode::Char('y')), base + Duration::from_secs(3));
        assert_eq!(studio.view.phase, ShowcasePhase::Verifying);
        assert_eq!(studio.view.phase_started, Duration::from_secs(3));
    }

    #[test]
    fn action_study_reports_all_seventeen_actual_native_actions() {
        for action in WhaleState::ALL {
            let inputs = showcase_inputs(action, "action-study");
            assert_eq!(
                acting_for(inputs.presence, inputs.activity.as_ref(), &inputs.context).id(),
                action.key()
            );
        }
        let args = [
            "--frames",
            "frames",
            "--profile",
            "light-truecolor",
            "--section",
            "life",
        ]
        .map(str::to_owned);
        let export = options(&args).unwrap().unwrap();
        assert_eq!(export.profile, Profile::LightTrue);
        assert_eq!(export.section, Some(ShowcaseSection::Life));
        assert!(options(&["--frames".into()]).is_err());
    }

    #[test]
    fn shared_renderer_clips_offset_tiny_and_ascii_frames_in_every_section() {
        use ratatui::buffer::Buffer;
        let base = Instant::now();
        for profile in Profile::ALL {
            for section in ShowcaseSection::ALL {
                let mut studio = Studio::new(base);
                studio.view.section = section;
                for (width, height) in [(0, 0), (1, 1), (4, 3), (40, 28)] {
                    let mut buffer = Buffer::empty(Rect::new(7, 11, width + 4, height + 4));
                    for cell in &mut buffer.content {
                        cell.set_symbol(".");
                    }
                    let area = Rect::new(9, 13, width, height);
                    ShowcaseFrame::new(&studio.view, Duration::from_secs(2)).paint(
                        area,
                        &mut buffer,
                        &profile.theme(),
                    );
                    for y in buffer.area.top()..buffer.area.bottom() {
                        for x in buffer.area.left()..buffer.area.right() {
                            if !area.contains((x, y).into()) {
                                assert_eq!(buffer[(x, y)].symbol(), ".");
                            }
                        }
                    }
                    if profile == Profile::Ascii {
                        assert!(buffer.content.iter().all(|c| c.symbol().is_ascii()));
                    }
                }
                let mut edge = Buffer::empty(Rect::new(u16::MAX - 6, u16::MAX - 5, 6, 5));
                ShowcaseFrame::new(&studio.view, Duration::ZERO).paint(
                    edge.area,
                    &mut edge,
                    &profile.theme(),
                );
            }
        }
    }

    #[test]
    fn completed_hold_is_deterministic_and_native_stage_is_shared_with_renderer() {
        fn frame(base: Instant, elapsed: Duration) -> ratatui::buffer::Buffer {
            let mut studio = Studio::new(base);
            let theme = Profile::DarkTrue.theme();
            exported_state(&mut studio, elapsed, None);
            let area = Rect::new(0, 0, 112, 38);
            let actor = studio.actor(base + elapsed, area, &theme);
            testing::render(area.width, area.height, |area, buf| {
                let view = ShowcaseFrame::new(&studio.view, elapsed);
                if let Some(actor) = &actor {
                    view.whale(actor).paint(area, buf, &theme);
                } else {
                    view.paint(area, buf, &theme);
                }
            })
        }
        let base = Instant::now();
        assert_eq!(
            frame(base, Duration::from_secs(20)),
            frame(base + Duration::from_secs(3_600), Duration::from_secs(20))
        );
        assert_eq!(
            frame(base, Duration::from_secs(20)),
            frame(base, Duration::from_secs(24))
        );
        let text = testing::text(&frame(base, Duration::from_secs(20)));
        assert!(text.contains("Done"));
        assert!(text.contains("Tasks") && text.contains("The session picker is ready for review."));
        assert!(!text.contains("queued") && !text.contains("Send now / Edit / Drop"));
    }
    #[test]
    fn native_theme_appearance_and_workbar_keys_share_the_rendered_state() {
        let now = Instant::now();
        let mut studio = Studio::new(now);
        studio.view.native_palette = TuiPalette::ShorelineLight;
        let theme = studio.view.theme_for(&Profile::DarkTrue.theme());
        assert_eq!(theme.caps().appearance, Appearance::Light);
        assert_eq!(theme.native_palette(), Some(TuiPalette::ShorelineLight));
        let ctrl = KeyModifiers::CONTROL;
        assert!(!studio.key(KeyEvent::new(KeyCode::Char('x'), ctrl), now));
        assert_eq!(studio.view.dock.panel, WorkbarPanel::Fleet);
        assert!(studio.view.readouts.on && studio.view.dock.focused);
        studio.key(key(KeyCode::Right), now);
        assert_eq!(studio.view.dock.panel, WorkbarPanel::Jobs);
        assert!(!studio.key(key(KeyCode::Esc), now));
        assert!(!studio.view.readouts.on);
        studio.view.editing = true;
        let frame = ShowcaseFrame::new(&studio.view, Duration::ZERO);
        let area = Rect::new(0, 0, 80, 24);
        assert!(
            frame
                .composer_cursor(area, &theme)
                .is_some_and(|cursor| frame.work_areas(area, &theme).composer.contains(cursor))
        );
    }
}
