//! Gallery: icons.

use std::time::Duration;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::{Line, Span},
    widgets::Widget,
};

use super::Entry;
use crate::{Icon, MotionMode, Role, Theme};

fn icons(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let rows: [(Icon, &str); 4] = [
        (
            Icon::Sonar {
                elapsed: Duration::from_millis(600),
                motion: MotionMode::Full,
            },
            "Searching the codebase",
        ),
        (Icon::tide(3, 5).expect("known total"), "3 of 5 phases done"),
        (Icon::Shell, "Receipt: edited summary.md"),
        (Icon::Kelp, "Divider"),
    ];
    for (row, (icon, words)) in rows.iter().enumerate() {
        Line::from(vec![
            icon.span(theme),
            Span::raw(" "),
            Span::styled(*words, theme.fg(Role::Foreground)),
            Span::styled(format!("  ({})", icon.label()), theme.fg(Role::Muted)),
        ])
        .render(
            Rect {
                y: area.y.saturating_add(row as u16),
                height: 1,
                ..area
            }
            .intersection(area),
            buf,
        );
    }
}

pub(crate) fn entries() -> Vec<Entry> {
    vec![Entry {
        name: "icons",
        width: 50,
        height: 4,
        draw: icons,
    }]
}
