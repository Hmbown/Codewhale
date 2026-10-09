//! Motion policy and the working spinner.
//!
//! Frames, cadence and the earned-marker delay come from the engine's
//! `crates/tui/src/tui/spinner.rs` and `motion/mode.rs` (`Hmbown/CodeWhale`
//! `58b1dd3dd`; last changed in `9b34ab54b` and `ad20493c0e`). Motion shows
//! something real: the spinner runs only while work runs, and reduced motion
//! holds one readable frame.

use std::time::Duration;

use ratatui::{buffer::Buffer, layout::Rect, text::Span, widgets::Widget};

use crate::{Paint, Role, Theme, text};

/// How much the interface moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum MotionMode {
    /// Spinners and the whale's frames run.
    #[default]
    Full,
    /// Static markers only (`low_motion`).
    Reduced,
    /// Nothing animates; redraw on state change only.
    Still,
}

impl MotionMode {
    /// From the engine's two settings: `low_motion` wins over
    /// `fancy_animations = false`.
    #[must_use]
    pub const fn from_settings(low_motion: bool, fancy_animations: bool) -> Self {
        if low_motion {
            Self::Reduced
        } else if !fancy_animations {
            Self::Still
        } else {
            Self::Full
        }
    }

    #[must_use]
    pub const fn animates(self) -> bool {
        matches!(self, Self::Full)
    }
}

/// A small swell: rises and recedes through adjacent dot counts, and never
/// flashes from full to empty.
pub const FRAMES: [&str; 8] = ["⣀", "⣄", "⣤", "⣦", "⣶", "⣦", "⣤", "⣄"];
/// Held under reduced motion: the charter's mark for current work, so a
/// still spinner says "working" instead of showing a stopped animation.
pub const STILL_FRAME: &str = crate::glyphs::CURRENT;
/// The swell in ASCII-safe terminals. `*` would read as "needs you" there.
pub const ASCII_FRAMES: [&str; 4] = ["-", "\\", "|", "/"];
/// Shown before the spinner is earned: fast work lands as a receipt.
pub const PENDING_FRAME: &str = "›";
/// Work must survive this long before anything moves.
pub const EARN_DELAY: Duration = Duration::from_millis(400);
/// Five steps a second.
pub const FRAME_INTERVAL: Duration = Duration::from_millis(200);

/// The frame for work that has run for `elapsed`.
#[must_use]
pub fn frame(elapsed: Duration, motion: MotionMode, ascii: bool) -> &'static str {
    if !motion.animates() {
        return crate::glyphs::pick(STILL_FRAME, ascii);
    }
    if elapsed < EARN_DELAY {
        return crate::glyphs::pick(PENDING_FRAME, ascii);
    }
    let steps = (elapsed - EARN_DELAY).as_millis() / FRAME_INTERVAL.as_millis();
    let frames: &[&str] = if ascii { &ASCII_FRAMES } else { &FRAMES };
    frames[(steps % frames.len() as u128) as usize]
}

/// When the next frame is due, or `None` when nothing will change: idle and
/// reduced motion schedule no redraws.
#[must_use]
pub fn next_frame_in(elapsed: Duration, motion: MotionMode) -> Option<Duration> {
    if !motion.animates() {
        return None;
    }
    if elapsed < EARN_DELAY {
        return Some(EARN_DELAY - elapsed);
    }
    let into = (elapsed - EARN_DELAY).as_millis() % FRAME_INTERVAL.as_millis();
    Some(FRAME_INTERVAL - Duration::from_millis(into as u64))
}

/// `⣦ Working · 38 s`: the spinner, a verb, and the measured time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spinner {
    pub verb: String,
    pub elapsed: Duration,
    pub motion: MotionMode,
}

impl Spinner {
    #[must_use]
    pub fn new(verb: impl Into<String>, elapsed: Duration, motion: MotionMode) -> Self {
        Self {
            verb: verb.into(),
            elapsed,
            motion,
        }
    }

    #[must_use]
    pub fn spans(&self, theme: &Theme) -> Vec<Span<'static>> {
        let mut spans = vec![
            Span::styled(
                frame(self.elapsed, self.motion, theme.ascii()),
                theme.fg(Role::Live),
            ),
            Span::raw(" "),
            Span::styled(
                text::display_safe(&self.verb).into_owned(),
                theme.fg(Role::Foreground),
            ),
        ];
        // Time is shown once it is worth reading, because it is measured.
        if self.elapsed >= Duration::from_secs(1) {
            let sep = if theme.ascii() { " - " } else { " · " };
            spans.push(Span::styled(sep, theme.fg(Role::Border)));
            spans.push(Span::styled(duration(self.elapsed), theme.fg(Role::Muted)));
        }
        spans
    }
}

impl Paint for Spinner {
    fn paint(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        ratatui::text::Line::from(self.spans(theme)).render(area, buf);
    }
}

/// `850 ms`, `12 s`, `4m 06s`, `1h 02m`.
#[must_use]
pub fn duration(d: Duration) -> String {
    let secs = d.as_secs();
    if secs == 0 {
        format!("{} ms", d.as_millis())
    } else if secs < 60 {
        format!("{secs} s")
    } else if secs < 3600 {
        format!("{}m {:02}s", secs / 60, secs % 60)
    } else {
        format!("{}h {:02}m", secs / 3600, (secs % 3600) / 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spinner_is_earned_then_swells() {
        let ms = Duration::from_millis;
        assert_eq!(frame(ms(100), MotionMode::Full, false), PENDING_FRAME);
        assert_eq!(frame(ms(400), MotionMode::Full, false), FRAMES[0]);
        assert_eq!(frame(ms(600), MotionMode::Full, false), FRAMES[1]);
        assert_eq!(frame(ms(5000), MotionMode::Reduced, false), STILL_FRAME);
        assert_eq!(frame(ms(5000), MotionMode::Still, false), STILL_FRAME);
    }

    #[test]
    fn ascii_frames_never_borrow_a_state_mark() {
        let ms = Duration::from_millis;
        assert_eq!(frame(ms(100), MotionMode::Full, true), ">");
        assert_eq!(frame(ms(400), MotionMode::Full, true), "-");
        assert_eq!(frame(ms(600), MotionMode::Full, true), "\\");
        // Held still, the spinner is the ASCII mark for current work.
        assert_eq!(frame(ms(5000), MotionMode::Reduced, true), ".");
        let needs_you = crate::glyphs::pick(crate::glyphs::ATTENTION, true);
        for t in (0..4000).step_by(100) {
            assert_ne!(frame(ms(t), MotionMode::Full, true), needs_you);
        }
    }

    #[test]
    fn reduced_motion_schedules_nothing() {
        assert_eq!(
            next_frame_in(Duration::from_secs(3), MotionMode::Reduced),
            None
        );
        assert_eq!(
            next_frame_in(Duration::from_millis(450), MotionMode::Full),
            Some(Duration::from_millis(150))
        );
    }

    #[test]
    fn durations_read_as_words() {
        assert_eq!(duration(Duration::from_millis(850)), "850 ms");
        assert_eq!(duration(Duration::from_secs(12)), "12 s");
        assert_eq!(duration(Duration::from_secs(246)), "4m 06s");
        assert_eq!(duration(Duration::from_secs(3720)), "1h 02m");
    }

    #[test]
    fn motion_settings_match_the_engine() {
        assert_eq!(MotionMode::from_settings(true, true), MotionMode::Reduced);
        assert_eq!(MotionMode::from_settings(false, false), MotionMode::Still);
        assert_eq!(MotionMode::from_settings(false, true), MotionMode::Full);
    }
}
