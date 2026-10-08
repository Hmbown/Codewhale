//! Gallery entries for package Chrome: the heading, tabs, toggle and
//! segmented control. (Dialog and sheet are in `surface`, keymap hints in
//! `hints`, next to the components they extend.)

use ratatui::{buffer::Buffer, layout::Rect};

use super::Entry;
use crate::{Heading, Paint, Segmented, SegmentedState, Tab, Tabs, TabsState, Theme, Toggle};

fn row(area: Rect, y: u16, height: u16) -> Rect {
    Rect {
        y: area.y.saturating_add(y),
        height,
        ..area
    }
    .intersection(area)
}

fn heading(area: Rect, buf: &mut Buffer, theme: &Theme) {
    Heading::new("Settings")
        .meta("saved to this project")
        .paint(row(area, 0, 1), buf, theme);
    Heading::new("Appearance")
        .section()
        .meta("3 changed")
        .paint(row(area, 2, 2), buf, theme);
    Heading::new("Defaults")
        .sub()
        .paint(row(area, 4, 1), buf, theme);
    Heading::new("A page title far too long for a narrow terminal to hold")
        .meta("12 items")
        .paint(row(area, 6, 1), buf, theme);
}

fn tab_items() -> Vec<Tab> {
    vec![
        Tab::new("General"),
        Tab::new("Appearance"),
        Tab::new("Agents").badge(3),
        Tab::new("Connections"),
        Tab::new("Privacy"),
        Tab::new("Advanced"),
    ]
}

fn tabs(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let items = tab_items();
    Tabs::new(&items, TabsState::new(2))
        .focused(true)
        .paint(row(area, 0, 2), buf, theme);
    // A narrow strip scrolls to keep its selection and marks what it hides.
    let narrow = Rect {
        width: area.width.min(34),
        ..area
    };
    Tabs::new(&items, TabsState::new(4)).paint(row(narrow, 3, 2), buf, theme);
    // Given one row there is no room for an underline: the selection is
    // bracketed instead.
    Tabs::new(&items[..3], TabsState::new(1)).paint(row(narrow, 6, 1), buf, theme);
}

fn toggle(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let rows = [
        Toggle::new("Show tool details", true),
        Toggle::new("Reduced motion", false).focused(true),
        Toggle::new("Share usage data", false).disabled("set by your admin"),
        Toggle::new("Show reasoning as it streams in the transcript", true),
    ];
    for (i, toggle) in rows.iter().enumerate() {
        toggle.paint(row(area, i as u16, 1), buf, theme);
    }
}

fn segmented(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let motion = ["Full", "Reduced", "Still"];
    Segmented::new(motion, SegmentedState::new(1)).paint(row(area, 0, 1), buf, theme);
    Segmented::new(["Light", "Dark", "System"], SegmentedState::new(0))
        .focused(true)
        .paint(row(area, 1, 1), buf, theme);
    Segmented::new(motion, SegmentedState::new(2))
        .disabled("set by your admin")
        .paint(row(area, 2, 1), buf, theme);
    // Too narrow for every option: the current one and its place.
    let narrow = Rect {
        width: area.width.min(18),
        ..area
    };
    Segmented::new(motion, SegmentedState::new(1)).paint(row(narrow, 4, 1), buf, theme);
}

pub(crate) fn entries() -> Vec<Entry> {
    vec![
        Entry {
            name: "heading",
            width: 64,
            height: 7,
            draw: heading,
        },
        Entry {
            name: "tabs",
            width: 56,
            height: 7,
            draw: tabs,
        },
        Entry {
            name: "toggle",
            width: 48,
            height: 4,
            draw: toggle,
        },
        Entry {
            name: "segmented",
            width: 40,
            height: 5,
            draw: segmented,
        },
    ]
}
