//! Gallery: the spinner, before and after it is earned, and under reduced
//! motion.

use std::time::Duration;

use ratatui::{buffer::Buffer, layout::Rect};

use super::Entry;
use crate::{MotionMode, Paint, Spinner, Theme};

fn spinners(area: Rect, buf: &mut Buffer, theme: &Theme) {
    for (row, (elapsed, motion)) in [
        (Duration::from_millis(200), MotionMode::Full),
        (Duration::from_millis(1400), MotionMode::Full),
        (Duration::from_secs(246), MotionMode::Reduced),
    ]
    .into_iter()
    .enumerate()
    {
        Spinner::new("Running cargo test", elapsed, motion).paint(
            Rect {
                y: area.y.saturating_add(row as u16),
                height: 1,
                ..area
            }
            .intersection(area),
            buf,
            theme,
        );
    }
}

pub(crate) fn entries() -> Vec<Entry> {
    vec![Entry {
        name: "spinner",
        width: 40,
        height: 3,
        draw: spinners,
    }]
}
