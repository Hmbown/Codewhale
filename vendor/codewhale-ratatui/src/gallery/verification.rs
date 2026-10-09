//! Fixed verification instants for the gallery; no fixture owns a timer.

use std::time::Duration;

use ratatui::{buffer::Buffer, layout::Rect};

use super::Entry;
use crate::{MotionMode, Paint, Theme, VerificationSpinner};

fn pending(area: Rect, buf: &mut Buffer, theme: &Theme) {
    VerificationSpinner::new(
        "Checking the received result",
        Duration::from_millis(200),
        MotionMode::Full,
    )
    .paint(area, buf, theme);
}

fn earned(area: Rect, buf: &mut Buffer, theme: &Theme) {
    VerificationSpinner::new(
        "Verifying terminal layout",
        Duration::from_millis(1400),
        MotionMode::Full,
    )
    .paint(area, buf, theme);
}

fn modes(area: Rect, buf: &mut Buffer, theme: &Theme) {
    for (index, (motion, verb)) in [
        (MotionMode::Full, "Checking"),
        (MotionMode::Reduced, "Checking with reduced motion"),
        (MotionMode::Still, "Checking with still motion"),
    ]
    .into_iter()
    .enumerate()
    {
        let row = Rect::new(area.x, area.y.saturating_add(index as u16), area.width, 1)
            .intersection(area);
        VerificationSpinner::new(verb, Duration::from_secs(3), motion).paint(row, buf, theme);
    }
}

pub(crate) fn entries() -> Vec<Entry> {
    vec![
        Entry {
            name: "verification-pending",
            width: 40,
            height: 1,
            draw: pending,
        },
        Entry {
            name: "verification-earned",
            width: 40,
            height: 1,
            draw: earned,
        },
        Entry {
            name: "verification-modes",
            width: 40,
            height: 3,
            draw: modes,
        },
    ]
}
