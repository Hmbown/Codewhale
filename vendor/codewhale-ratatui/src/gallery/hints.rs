//! Gallery: key hints, and hints folded from a keymap.

use crossterm::event::KeyCode;
use ratatui::{buffer::Buffer, layout::Rect};

use super::{Entry, arrows};
use crate::{
    Binding, KeyChord, KeyHint, KeyHints, KeyHintsWords, Keymap, Paint, Theme, keys::Platform,
};

fn key_hints(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let hints = KeyHints::new(vec![
        KeyHint::new(arrows(theme), "move"),
        KeyHint::new("Enter", "change"),
        KeyHint::new("r", "reset"),
        KeyHint::new("/", "search"),
        KeyHint::new("Ctrl+S", "save").disabled(),
        KeyHint::new("Esc", "close"),
    ]);
    hints.paint(area, buf, theme);
}

/// The settings screen's keys, most important first by priority: when the
/// row is narrow the lowest fold away and `? more` says so.
fn settings_keymap() -> Keymap<&'static str> {
    Keymap::new()
        .with(Binding::pair(KeyCode::Up, KeyCode::Down, "move", "move").priority(200))
        .with(Binding::new(KeyCode::Enter, "change", "change").priority(250))
        .with(Binding::new(KeyChord::char('r'), "reset", "reset").priority(100))
        .with(Binding::new(KeyChord::char('/'), "search", "search").priority(120))
        .with(
            Binding::new(KeyChord::ctrl('s'), "save", "save")
                .priority(80)
                .disabled(),
        )
        .with(Binding::new(KeyCode::Esc, "close", "close").priority(240))
        .with(Binding::help(KeyChord::char('?'), "help"))
}

fn keymap_hints(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let platform = Platform::current(theme.ascii());
    let words = KeyHintsWords::default();
    let map = settings_keymap();
    // The row folds to the width it is given: the whole area, then a
    // narrower one a host with a sidebar would have.
    for (row, width) in [area.width, 34.min(area.width)].into_iter().enumerate() {
        if row >= usize::from(area.height) {
            break;
        }
        map.hints(platform, width, &words).paint(
            Rect {
                y: area.y + row as u16,
                width,
                height: 1,
                ..area
            },
            buf,
            theme,
        );
    }
}

pub(crate) fn entries() -> Vec<Entry> {
    vec![
        Entry {
            name: "key-hints",
            width: 60,
            height: 2,
            draw: key_hints,
        },
        Entry {
            name: "keymap-hints",
            width: 60,
            height: 2,
            draw: keymap_hints,
        },
    ]
}
