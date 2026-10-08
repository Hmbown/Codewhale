//! Live caller-owned motion: `cargo run --example motion`.
//!
//! Space finishes/restarts the demonstration, v switches working/verification,
//! r replays, p selects a terminal profile, m changes motion, q or Escape exits.
//! `--frames DIR [--profile NAME]` exports 128 deterministic actual-buffer SVGs.
//! Only the host reads the clock and waits. Completion comes from a host event;
//! the exported specimen has an explicitly authored completion at 4.2 seconds.

use std::{
    io,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use codewhale_ratatui::{
    Caps, FrameBudget, KeyHint, KeyHints, MotionDemo, MotionDemoWords, MotionMode, MotionPolicy,
    MotionSet, Paint, PaneHeader, Receipt, ReceiptValue, Role, Spinner, State, Theme,
    VerificationSpinner, WorkspaceFrame, spin,
    testing::{self, Profile},
    text,
};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    buffer::Buffer,
    layout::Rect,
    text::{Line, Span},
};

const VERIFY_AT: Duration = Duration::from_millis(2_200);
const FINISH_AT: Duration = Duration::from_millis(4_200);
const FRAME_MS: u64 = 50;
const FRAME_COUNT: u64 = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Working,
    Verifying,
    Done,
}

impl Stage {
    fn word(self) -> &'static str {
        match self {
            Self::Working => "Working",
            Self::Verifying => "Verifying",
            Self::Done => "Done",
        }
    }
}

/// Display state of this specimen, never a provider/session inference.
struct Demo {
    stage: Stage,
    run_started: Instant,
    phase_started: Instant,
    finished_elapsed: Duration,
    motions: MotionSet,
}

impl Demo {
    fn new(now: Instant) -> Self {
        Self {
            stage: Stage::Working,
            run_started: now,
            phase_started: now,
            finished_elapsed: Duration::ZERO,
            motions: MotionSet::new(),
        }
    }

    fn phase_elapsed(&self, now: Instant) -> Duration {
        now.saturating_duration_since(self.phase_started)
    }

    fn finish(&mut self, now: Instant, policy: MotionPolicy) {
        if self.stage == Stage::Done {
            return;
        }
        self.finished_elapsed = now.saturating_duration_since(self.run_started);
        self.stage = Stage::Done;
        MotionDemo::start(&mut self.motions, true, now, policy);
    }

    fn restart(&mut self, now: Instant, policy: MotionPolicy) {
        self.stage = Stage::Working;
        self.run_started = now;
        self.phase_started = now;
        self.finished_elapsed = Duration::ZERO;
        // Retarget the existing steps: a mid-flight replay preserves spatial
        // continuity rather than replacing the set with an unrelated frame.
        MotionDemo::start(&mut self.motions, false, now, policy);
    }

    fn toggle_verification(&mut self, now: Instant) {
        self.stage = match self.stage {
            Stage::Working => Stage::Verifying,
            Stage::Verifying => Stage::Working,
            Stage::Done => return,
        };
        self.phase_started = now;
    }

    fn next_frame_in(&self, now: Instant, theme: &Theme, mode: MotionMode) -> Option<Duration> {
        let mut budget = FrameBudget::new();
        if self.stage != Stage::Done && budget.claim_spinner() {
            let elapsed = self.phase_elapsed(now);
            budget.request(match self.stage {
                Stage::Working => spin::next_frame_in(elapsed, mode),
                Stage::Verifying => VerificationSpinner::next_frame_in(elapsed, mode),
                Stage::Done => None,
            });
        }
        budget.request_at(
            now,
            self.motions
                .next_frame_at(now, MotionPolicy::new(mode, theme)),
        );
        budget.next_frame_in()
    }
}

fn mode_word(mode: MotionMode) -> &'static str {
    match mode {
        MotionMode::Full => "Full motion",
        MotionMode::Reduced => "Reduced motion",
        MotionMode::Still => "Still",
    }
}

fn row(area: Rect, buf: &mut Buffer, line: &Line<'_>) {
    let area = area.intersection(buf.area);
    if !area.is_empty() {
        buf.set_line(area.x, area.y, line, area.width);
    }
}

fn at(area: Rect, offset: u16, height: u16) -> Rect {
    Rect::new(
        area.x,
        area.y.saturating_add(offset.min(area.height)),
        area.width,
        height.min(area.height.saturating_sub(offset)),
    )
}

fn copy_paint(component: &impl Paint, area: Rect, buf: &mut Buffer, theme: &Theme) {
    let area = area.intersection(buf.area);
    if area.is_empty() {
        return;
    }
    // The receipt and key-hint components can use ordinary terminal widgets
    // inside local coordinates, even when the destination reaches u16::MAX.
    let mut local = Buffer::empty(Rect::new(0, 0, area.width, area.height));
    for y in 0..area.height {
        for x in 0..area.width {
            local[(x, y)] = buf[(area.x + x, area.y + y)].clone();
        }
    }
    component.paint(local.area, &mut local, theme);
    for y in 0..area.height {
        for x in 0..area.width {
            buf[(area.x + x, area.y + y)] = local[(x, y)].clone();
        }
    }
}

fn caption(value: &str, area: Rect, buf: &mut Buffer, theme: &Theme) {
    let safe = text::display_safe(value);
    row(
        area,
        buf,
        &Line::styled(
            text::truncate_words(&safe, usize::from(area.width), theme.ascii()).into_owned(),
            theme.fg(Role::Muted),
        ),
    );
}

fn transitions(
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
    demo: &Demo,
    now: Instant,
    mode: MotionMode,
) -> u16 {
    let compact = area.width < 64;
    let labels = if compact {
        ["Ink / 180ms", "Focus / 180ms", "Detail / 340ms"]
    } else {
        [
            "State ink / 180ms",
            "Selection / 180ms",
            "Detail reveal / 340ms",
        ]
    };
    let words = MotionDemoWords {
        working: demo.stage.word().into(),
        done: "Done".into(),
        slide: "Selected".into(),
        detail: "Details are ready".into(),
    };
    let component = MotionDemo::new(
        &demo.motions,
        now,
        MotionPolicy::new(mode, theme),
        demo.stage == Stage::Done,
    )
    .with_words(words);
    if area.width >= 30 {
        let label_width = if compact { 15 } else { 24 };
        for (index, label) in labels.into_iter().enumerate() {
            caption(label, at(area, index as u16, 1), buf, theme);
        }
        component.paint(
            Rect::new(
                area.x.saturating_add(label_width),
                area.y,
                area.width.saturating_sub(label_width),
                area.height.min(3),
            ),
            buf,
            theme,
        );
        3
    } else {
        // Keep labels legible on unusually narrow terminals. The same
        // three actual MotionDemo rows are stacked below their own labels.
        let mut local = Buffer::empty(Rect::new(0, 0, area.width, 3));
        component.paint(local.area, &mut local, theme);
        for (index, label) in labels.into_iter().enumerate() {
            let offset = index as u16 * 2;
            caption(label, at(area, offset, 1), buf, theme);
            let target = at(area, offset + 1, 1);
            if !target.is_empty() {
                for x in 0..target.width {
                    buf[(target.x + x, target.y)] = local[(x, index as u16)].clone();
                }
            }
        }
        6
    }
}

fn held_samples(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let ascii = Theme::new(Caps {
        ascii: true,
        ..theme.caps()
    })
    .ground(theme.ground_kind());
    let specimens = [
        (
            "ASCII / captured",
            "800ms specimen",
            // This is one frozen frame, not a second live working marker.
            Spinner::new("Working", Duration::from_millis(800), MotionMode::Full).spans(&ascii),
        ),
        (
            "Reduced motion",
            "Static marker",
            Spinner::new("Working", Duration::ZERO, MotionMode::Reduced).spans(theme),
        ),
        (
            "Still",
            "Static marker",
            Spinner::new("Working", Duration::ZERO, MotionMode::Still).spans(theme),
        ),
    ];
    if area.width >= 72 {
        let width = area.width.saturating_sub(4) / 3;
        for (index, (title, note, spans)) in specimens.into_iter().enumerate() {
            let column = Rect::new(
                area.x.saturating_add(index as u16 * (width + 2)),
                area.y,
                width,
                area.height.min(3),
            );
            PaneHeader::new(title)
                .divider(false)
                .paint(at(column, 0, 1), buf, theme);
            row(at(column, 1, 1), buf, &Line::from(spans));
            caption(note, at(column, 2, 1), buf, theme);
        }
    } else {
        for (index, (title, _, mut spans)) in specimens.into_iter().enumerate() {
            let title = if index == 0 { "ASCII / 800ms" } else { title };
            let mut line = vec![
                Span::styled(title, theme.fg(Role::Muted)),
                Span::raw(" ".repeat(16usize.saturating_sub(text::width(title)))),
            ];
            line.append(&mut spans);
            row(at(area, index as u16, 1), buf, &Line::from(line));
        }
    }
}

fn paint(area: Rect, buf: &mut Buffer, theme: &Theme, demo: &Demo, now: Instant, mode: MotionMode) {
    let area = area.intersection(buf.area);
    if area.is_empty() {
        return;
    }
    let frame = WorkspaceFrame::new("Motion")
        .mode("Demonstration")
        .side_width(0)
        .footer("Demonstration / caller-owned time and state / q or Esc closes");
    frame.paint(area, buf, theme);
    let main = frame.areas(area).main;
    PaneHeader::new("Motion in Codewhale")
        .meta(mode_word(mode))
        .divider(false)
        .paint(at(main, 0, 1), buf, theme);
    caption(
        &format!("{} / caller-owned time and state", mode_word(mode)),
        at(main, 1, 1),
        buf,
        theme,
    );
    PaneHeader::new("Work / verification / receipt")
        .meta(demo.stage.word())
        .divider(false)
        .paint(at(main, 3, 1), buf, theme);
    let elapsed = demo.phase_elapsed(now);
    match demo.stage {
        Stage::Working => row(
            at(main, 4, 1),
            buf,
            &Line::from(Spinner::new("Working", elapsed, mode).spans(theme)),
        ),
        Stage::Verifying => row(
            at(main, 4, 1),
            buf,
            &Line::from(VerificationSpinner::new("Verifying", elapsed, mode).spans(theme)),
        ),
        Stage::Done => copy_paint(
            &Receipt::new(State::Done, "Motion sample")
                .value(ReceiptValue::Duration(Some(demo.finished_elapsed))),
            at(main, 4, 1),
            buf,
            theme,
        ),
    }
    caption(
        if demo.stage == Stage::Done {
            "A receipt replaces the live marker."
        } else {
            "Earned after 400ms / 200ms cadence"
        },
        at(main, 5, 1),
        buf,
        theme,
    );
    PaneHeader::new("Three state transitions")
        .meta("Space finishes")
        .divider(false)
        .paint(at(main, 7, 1), buf, theme);
    let transition_rows = transitions(at(main, 8, 6), buf, theme, demo, now, mode);
    let samples_start = 8 + transition_rows + 1;
    PaneHeader::new("Held mode specimens").divider(false).paint(
        at(main, samples_start, 1),
        buf,
        theme,
    );
    held_samples(at(main, samples_start + 1, 3), buf, theme);
    let hints = KeyHints::new(vec![
        KeyHint::new("Space", "toggle"),
        KeyHint::new("v", "verify"),
        KeyHint::new("r", "replay"),
        KeyHint::new("p", "profile"),
        KeyHint::new("m", "motion"),
        KeyHint::new("q", "close"),
    ]);
    let hints_start = samples_start + 5;
    copy_paint(&hints, at(main, hints_start, 3), buf, theme);
    caption(
        if demo.next_frame_in(now, theme, mode).is_some() {
            "Redraws only while a marker or step moves."
        } else {
            "Quiet: waiting for input."
        },
        at(main, hints_start + 4, 1),
        buf,
        theme,
    );
}

fn exported_demo(base: Instant, elapsed: Duration, theme: &Theme) -> Demo {
    let mut demo = Demo::new(base);
    if elapsed >= VERIFY_AT {
        demo.toggle_verification(base + VERIFY_AT);
    }
    if elapsed >= FINISH_AT {
        demo.finish(base + FINISH_AT, MotionPolicy::new(MotionMode::Full, theme));
    }
    demo
}

fn export_frames(dir: &Path, profile: Profile) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let theme = profile.theme();
    let base = Instant::now();
    let mut manifest = Vec::new();
    for index in 0..FRAME_COUNT {
        let elapsed_ms = index * FRAME_MS;
        let elapsed = Duration::from_millis(elapsed_ms);
        let demo = exported_demo(base, elapsed, &theme);
        let buf = testing::render(96, 30, |area, buf| {
            paint(area, buf, &theme, &demo, base + elapsed, MotionMode::Full);
        });
        let file = format!("frame-{index:03}.svg");
        std::fs::write(dir.join(&file), testing::svg(&buf, &theme))?;
        manifest.push(serde_json::json!({
            "file":file, "elapsed_ms":elapsed_ms, "profile":profile.name(), "width":96, "height":30,
        }));
    }
    std::fs::write(
        dir.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    Ok(())
}

fn export_options(args: &[String]) -> io::Result<Option<(PathBuf, Profile)>> {
    if args.is_empty() {
        return Ok(None);
    }
    let usage = || {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: motion [--frames DIR [--profile NAME]]",
        )
    };
    if args.first().map(String::as_str) != Some("--frames") || !matches!(args.len(), 2 | 4) {
        return Err(usage());
    }
    let profile = if args.len() == 4 {
        if args[2] != "--profile" {
            return Err(usage());
        }
        Profile::from_name(&args[3]).ok_or_else(usage)?
    } else {
        Profile::DarkTrue
    };
    Ok(Some((PathBuf::from(&args[1]), profile)))
}

struct Restore;
impl Drop for Restore {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    }
}

fn main() -> io::Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if let Some((dir, profile)) = export_options(&args)? {
        return export_frames(&dir, profile);
    }
    enable_raw_mode()?;
    let _restore = Restore;
    execute!(io::stdout(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut demo = Demo::new(Instant::now());
    let mut profile = 0;
    let mut mode = MotionMode::Full;
    loop {
        let now = Instant::now();
        let theme = Profile::ALL[profile].theme();
        terminal.draw(|frame| paint(frame.area(), frame.buffer_mut(), &theme, &demo, now, mode))?;
        if let Some(after) = demo.next_frame_in(now, &theme, mode)
            && !event::poll(after.saturating_sub(now.elapsed()))?
        {
            continue;
        }
        // No deadline: read blocks until input, including while a reduced
        // motion specimen is active and after the completed steps settle.
        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            let now = Instant::now();
            let policy = MotionPolicy::new(mode, &theme);
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Char(' ') => {
                    if demo.stage == Stage::Done {
                        demo.restart(now, policy);
                    } else {
                        demo.finish(now, policy);
                    }
                }
                KeyCode::Char('v') => demo.toggle_verification(now),
                KeyCode::Char('r') => demo.restart(now, policy),
                KeyCode::Char('p') => {
                    profile = (profile + 1) % Profile::ALL.len();
                    demo.motions.settle_all();
                }
                KeyCode::Char('m') => {
                    mode = match mode {
                        MotionMode::Full => MotionMode::Reduced,
                        MotionMode::Reduced => MotionMode::Still,
                        MotionMode::Still => MotionMode::Full,
                    };
                    demo.motions.settle_all();
                }
                _ => {}
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(
        base: Instant,
        elapsed: Duration,
        profile: Profile,
        width: u16,
        height: u16,
    ) -> Buffer {
        let theme = profile.theme();
        let demo = exported_demo(base, elapsed, &theme);
        testing::render(width, height, |area, buf| {
            paint(area, buf, &theme, &demo, base + elapsed, MotionMode::Full);
        })
    }

    #[test]
    fn export_has_fixed_phases_deterministic_frames_and_a_static_completed_hold() {
        let base = Instant::now();
        let later_base = base + Duration::from_secs(3_600);
        let theme = Profile::DarkTrue.theme();
        for index in 0..FRAME_COUNT {
            let elapsed = Duration::from_millis(index * FRAME_MS);
            let demo = exported_demo(base, elapsed, &theme);
            let stage = if elapsed < VERIFY_AT {
                Stage::Working
            } else if elapsed < FINISH_AT {
                Stage::Verifying
            } else {
                Stage::Done
            };
            assert_eq!(demo.stage, stage);
            assert_eq!(
                frame(base, elapsed, Profile::DarkTrue, 96, 30),
                frame(later_base, elapsed, Profile::DarkTrue, 96, 30),
                "frame {index} must depend on elapsed time, not the wall clock",
            );
        }
        let verifying = exported_demo(base, VERIFY_AT, &theme);
        assert_eq!(verifying.phase_elapsed(base + VERIFY_AT), Duration::ZERO);
        let finished = exported_demo(base, FINISH_AT, &theme);
        assert_eq!(finished.finished_elapsed, FINISH_AT);
        assert!(
            testing::text(&frame(
                base,
                Duration::from_millis(2_600),
                Profile::DarkTrue,
                96,
                30
            ))
            .contains("Verifying")
        );
        assert_eq!(
            frame(
                base,
                Duration::from_millis(4_550),
                Profile::DarkTrue,
                96,
                30
            ),
            frame(
                base,
                Duration::from_millis(6_350),
                Profile::DarkTrue,
                96,
                30
            ),
            "the final hold cannot keep a spinner, time readout or transition moving",
        );
    }

    #[test]
    fn scheduler_uses_earned_cadence_then_stops_for_quiet_modes_and_settled_completion() {
        let base = Instant::now();
        let theme = Profile::DarkTrue.theme();
        let policy = MotionPolicy::new(MotionMode::Full, &theme);
        let mut demo = Demo::new(base);
        assert_eq!(
            demo.next_frame_in(base, &theme, MotionMode::Full),
            Some(spin::EARN_DELAY)
        );
        assert_eq!(
            demo.next_frame_in(base + spin::EARN_DELAY, &theme, MotionMode::Full),
            Some(spin::FRAME_INTERVAL)
        );
        for mode in [MotionMode::Reduced, MotionMode::Still] {
            assert_eq!(demo.next_frame_in(base, &theme, mode), None);
        }
        demo.toggle_verification(base + VERIFY_AT);
        assert_eq!(
            demo.next_frame_in(base + VERIFY_AT, &theme, MotionMode::Full),
            VerificationSpinner::next_frame_in(Duration::ZERO, MotionMode::Full)
        );
        demo.finish(base + FINISH_AT, policy);
        assert!(
            demo.next_frame_in(base + FINISH_AT, &theme, MotionMode::Full)
                .is_some()
        );
        assert_eq!(
            demo.next_frame_in(
                base + FINISH_AT + Duration::from_millis(340),
                &theme,
                MotionMode::Full
            ),
            None
        );
        let mut reduced = Demo::new(base);
        reduced.finish(
            base + FINISH_AT,
            MotionPolicy::new(MotionMode::Reduced, &theme),
        );
        assert!(reduced.motions.is_settled(base + FINISH_AT));
        assert_eq!(
            reduced.next_frame_in(base + FINISH_AT, &theme, MotionMode::Reduced),
            None
        );
    }

    #[test]
    fn replay_preserves_step_continuity_and_resets_only_caller_owned_elapsed_time() {
        let base = Instant::now();
        let theme = Profile::DarkTrue.theme();
        let policy = MotionPolicy::new(MotionMode::Full, &theme);
        let mut demo = Demo::new(base);
        demo.finish(base + FINISH_AT, policy);
        let midway = base + FINISH_AT + Duration::from_millis(50);
        let before = demo.motions.step("slide").at(midway);
        assert!(before > 0.0 && before < 1.0);
        demo.restart(midway, policy);
        assert_eq!(demo.stage, Stage::Working);
        assert_eq!(demo.phase_elapsed(midway), Duration::ZERO);
        assert_eq!(demo.finished_elapsed, Duration::ZERO);
        assert_eq!(demo.motions.step("slide").at(midway), before);
        assert_eq!(demo.motions.step("slide").target(), 0.0);
        demo.toggle_verification(midway + Duration::from_millis(100));
        assert_eq!(demo.stage, Stage::Verifying);
        assert_eq!(
            demo.phase_elapsed(midway + Duration::from_millis(100)),
            Duration::ZERO
        );
        demo.finish(midway + Duration::from_millis(700), policy);
        assert_eq!(demo.finished_elapsed, Duration::from_millis(700));
        let last_step = demo.motions.step("reveal");
        demo.finish(midway + Duration::from_secs(20), policy);
        assert_eq!(demo.finished_elapsed, Duration::from_millis(700));
        assert_eq!(
            demo.motions.step("reveal"),
            last_step,
            "same completion never restarts a transition"
        );
    }

    #[test]
    fn narrow_profiles_keep_state_words_and_partial_tiny_or_maximum_buffers_are_bounded() {
        let base = Instant::now();
        for profile in Profile::ALL {
            let theme = profile.theme();
            for (elapsed, state) in [(1_000, "Working"), (2_800, "Verifying"), (4_550, "Done")] {
                let buf = frame(base, Duration::from_millis(elapsed), profile, 40, 28);
                let shown = testing::text(&buf);
                assert!(
                    shown.contains("Motion in Codewhale") && shown.contains(state),
                    "{shown}"
                );
                assert!(
                    shown.contains("Ink / 180ms") && shown.contains("Detail / 340ms"),
                    "{shown}"
                );
                assert!(
                    shown.contains("ASCII / 800ms")
                        && shown.contains("Reduced motion")
                        && shown.contains("Still"),
                    "{shown}"
                );
                assert!(
                    shown.contains("v verify") && shown.contains("r replay"),
                    "{shown}"
                );
                let checked = testing::Frame::new("narrow motion", profile, buf);
                assert!(checked.violations().is_empty(), "{}", checked.label());
            }
            for origin in [(5, 4), (u16::MAX - 82, u16::MAX - 31)] {
                for width in [0, 1, 4, 80] {
                    for height in [0, 1, 4, 30] {
                        let bounds = Rect::new(origin.0, origin.1, 82, 31);
                        let requested = Rect::new(origin.0 + 2, origin.1 + 1, width, height);
                        let clipped = requested.intersection(bounds);
                        let mut buf = Buffer::empty(bounds);
                        for cell in &mut buf.content {
                            cell.set_symbol("z");
                        }
                        let before = buf.clone();
                        let elapsed = Duration::from_millis(4_250);
                        let demo = exported_demo(base, elapsed, &theme);
                        paint(
                            requested,
                            &mut buf,
                            &theme,
                            &demo,
                            base + elapsed,
                            MotionMode::Full,
                        );
                        for y in bounds.y..bounds.bottom() {
                            for x in bounds.x..bounds.right() {
                                if !clipped.contains((x, y).into()) {
                                    assert_eq!(
                                        buf[(x, y)],
                                        before[(x, y)],
                                        "{} {width}x{height}",
                                        profile.name()
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
