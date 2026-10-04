//! Gallery: the Braille whale, its states, its pod and its compact sizes.

use ratatui::{buffer::Buffer, layout::Rect};

use super::Entry;
use crate::{Paint, Theme, Whale, WhaleState};

fn whale(state: WhaleState) -> impl Fn(Rect, &mut Buffer, &Theme) {
    move |area, buf, theme| Whale::new(state).paint(area, buf, theme)
}

/// All 17 actions of the v2 whale at the compact size, three to a row.
fn whale_actions(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let area = area.intersection(buf.area);
    if area.is_empty() {
        return;
    }
    const CELL_W: u16 = 28;
    const CELL_H: u16 = 11;
    for (i, state) in WhaleState::ALL.into_iter().enumerate() {
        let i = u16::try_from(i).unwrap_or(u16::MAX);
        let cell = Rect {
            x: area.x.saturating_add((i % 3) * CELL_W),
            y: area.y.saturating_add((i / 3) * CELL_H),
            width: CELL_W,
            height: CELL_H,
        }
        .intersection(area);
        Whale::new(state).paint(cell, buf, theme);
    }
}

pub(crate) fn entries() -> Vec<Entry> {
    vec![
        Entry {
            name: "whale-rest",
            width: 36,
            height: 17,
            draw: |a, b, t| whale(WhaleState::Rest)(a, b, t),
        },
        Entry {
            name: "whale-busy",
            width: 36,
            height: 17,
            draw: |a, b, t| whale(WhaleState::Busy)(a, b, t),
        },
        Entry {
            name: "whale-needs",
            width: 36,
            height: 17,
            draw: |a, b, t| whale(WhaleState::NeedsYou)(a, b, t),
        },
        Entry {
            name: "whale-done",
            width: 36,
            height: 17,
            draw: |a, b, t| whale(WhaleState::Done)(a, b, t),
        },
        Entry {
            name: "whale-pod-1",
            width: 36,
            height: 17,
            draw: |a, b, t| whale(WhaleState::Pod { calves: 1 })(a, b, t),
        },
        Entry {
            name: "whale-pod-3",
            width: 36,
            height: 17,
            draw: |a, b, t| whale(WhaleState::Pod { calves: 3 })(a, b, t),
        },
        Entry {
            name: "whale-actions",
            width: 84,
            height: 66,
            draw: whale_actions,
        },
        Entry {
            name: "whale-compact",
            width: 24,
            height: 11,
            draw: |a, b, t| whale(WhaleState::Busy)(a, b, t),
        },
        Entry {
            name: "whale-words-only",
            width: 14,
            height: 3,
            draw: |a, b, t| whale(WhaleState::NeedsYou)(a, b, t),
        },
    ]
}
