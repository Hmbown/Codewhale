//! Gallery: status marks.

use ratatui::{buffer::Buffer, layout::Rect};

use super::Entry;
use crate::{Paint, State, StatusMark, Theme};

fn status_marks(area: Rect, buf: &mut Buffer, theme: &Theme) {
    for (row, state) in State::ALL.iter().enumerate() {
        let rect = Rect {
            y: area.y.saturating_add(row as u16),
            height: 1,
            ..area
        }
        .intersection(area);
        StatusMark::new(*state).paint(rect, buf, theme);
    }
}

pub(crate) fn entries() -> Vec<Entry> {
    vec![Entry {
        name: "status-marks",
        width: 20,
        height: 7,
        draw: status_marks,
    }]
}
