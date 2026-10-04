//! The Engine's verification tick, distinct from its working swell.
//!
//! The frame table and cadence come from `Hmbown/CodeWhale`
//! `a79ce5c4d5ed1a5f7032185710c27343a900351c`,
//! `crates/tui/src/tui/spinner.rs::verification_tick_frame`. Earned delay,
//! ASCII rotation, elapsed words and quiet-mode marks share this kit's
//! [`Spinner`]. The caller supplies elapsed time and owns all scheduling.

use std::time::Duration;

use crate::{MotionMode, Paint, Spinner, Theme, spin};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::{Line, Span},
};

/// A checking phase uses a round tick while working uses a swell.
///
/// Construct it only for verification the host has actually observed. The
/// verb is caller-owned, and the elapsed time is measured rather than a
/// progress estimate. Reduced and still motion show a static mark and word.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerificationSpinner {
    pub verb: String,
    pub elapsed: Duration,
    pub motion: MotionMode,
}

impl VerificationSpinner {
    /// The Engine's eight adjacent verification frames.
    pub const FRAMES: [&'static str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];

    #[must_use]
    pub fn new(verb: impl Into<String>, elapsed: Duration, motion: MotionMode) -> Self {
        Self {
            verb: verb.into(),
            elapsed,
            motion,
        }
    }

    /// Sample a verification frame without reading a clock.
    #[must_use]
    pub fn frame(elapsed: Duration, motion: MotionMode, ascii: bool) -> &'static str {
        if !motion.animates() || ascii || elapsed < spin::EARN_DELAY {
            return spin::frame(elapsed, motion, ascii);
        }
        let step = (elapsed - spin::EARN_DELAY).as_millis() / spin::FRAME_INTERVAL.as_millis();
        Self::FRAMES[(step % Self::FRAMES.len() as u128) as usize]
    }

    /// The shared five-step cadence; quiet motion asks for no redraws.
    #[must_use]
    pub fn next_frame_in(elapsed: Duration, motion: MotionMode) -> Option<Duration> {
        spin::next_frame_in(elapsed, motion)
    }

    /// A tick, the caller's sanitized verb and the shared measured-time words.
    #[must_use]
    pub fn spans(&self, theme: &Theme) -> Vec<Span<'static>> {
        let mut spans = Spinner::new(&self.verb, self.elapsed, self.motion).spans(theme);
        spans[0].content = Self::frame(self.elapsed, self.motion, theme.ascii()).into();
        spans
    }
}

impl Paint for VerificationSpinner {
    fn paint(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let area = area.intersection(buf.area);
        if !area.is_empty() {
            // Buffer::set_line also handles a right edge of u16::MAX, where
            // the widget Line renderer can overflow while clipping a span.
            super::workbench::row(area, buf, &Line::from(self.spans(theme)));
        }
    }
}
