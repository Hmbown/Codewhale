//! Gallery: toasts.

use std::time::{Duration, Instant};

use ratatui::{buffer::Buffer, layout::Rect};

use super::Entry;
use crate::{MotionMode, Paint, State, Theme, Toast, Toasts, Ttl};

fn toasts(area: Rect, buf: &mut Buffer, theme: &Theme) {
    Toasts::new(vec![
        Toast::new(State::Done, "Theme set to Shoreline"),
        Toast::new(State::NeedsYou, "A command is waiting for your approval").opens(),
        Toast::new(
            State::Failed,
            "Could not save: the settings file is read-only",
        )
        .opens(),
    ])
    .paint(area, buf, theme);
}

/// Six toasts, three visible: the older three fold into `+3 more`.
fn stacked(area: Rect, buf: &mut Buffer, theme: &Theme) {
    Toasts::new(vec![
        Toast::new(State::Done, "Saved summary.md"),
        Toast::new(State::Done, "Saved CHANGELOG.md"),
        Toast::new(State::Working, "Indexing 1,204 files"),
        Toast::new(State::Done, "Theme set to Shoreline"),
        Toast::new(State::NeedsYou, "A command is waiting for your approval").opens(),
        Toast::new(
            State::Failed,
            "Could not save: the settings file is read-only",
        ),
    ])
    .paint(area, buf, theme);
}

/// A toast mid-fade: the host's clock says 70 ms after it was born, so the
/// sentence is in the second step of ink. The same stack under reduced
/// motion is settled.
fn fading(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let base = Instant::now();
    let at = base + Duration::from_millis(70);
    let items = || {
        vec![
            Toast::new(State::Done, "Theme set to Shoreline").born(base),
            Toast::new(State::Done, "Saved summary.md")
                .born(base.checked_sub(Duration::from_millis(900)).unwrap_or(base))
                .ttl(Ttl::Seconds(1)),
        ]
    };
    let half = area.height / 2;
    let top = Rect::new(area.x, area.y, area.width, half);
    let bottom = Rect::new(area.x, area.y + half, area.width, area.height - half);
    Toasts::new(items()).at(at).paint(top, buf, theme);
    Toasts::new(items())
        .at(at)
        .motion(MotionMode::Reduced)
        .paint(bottom, buf, theme);
}

pub(crate) fn entries() -> Vec<Entry> {
    vec![
        Entry {
            name: "toasts",
            width: 60,
            height: 3,
            draw: toasts,
        },
        Entry {
            name: "toasts-stacked",
            width: 60,
            height: 5,
            draw: stacked,
        },
        Entry {
            name: "toasts-fading",
            width: 60,
            height: 4,
            draw: fading,
        },
    ]
}
