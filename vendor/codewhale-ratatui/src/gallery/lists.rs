//! Gallery entries for package Lists: the list in its states and the empty
//! state. The fuzzy picker's entries live in `picker.rs`, beside the other
//! pickers.

use ratatui::{buffer::Buffer, layout::Rect};

use super::Entry;
use crate::{
    EmptyMark, EmptyState, List, ListRow, ListRowState, ListState, Paint, Role, Theme, text,
};

/// A settings-style row: a name, its value, and optionally a second line.
/// It shows how a caller paints its own row: `ListRowState::ink` for the
/// name, `Muted` for the rest, a reason in place of the value when disabled.
struct Setting {
    name: &'static str,
    value: &'static str,
    note: Option<&'static str>,
    disabled: Option<&'static str>,
}

impl Setting {
    const fn new(name: &'static str, value: &'static str) -> Self {
        Self {
            name,
            value,
            note: None,
            disabled: None,
        }
    }

    const fn note(mut self, note: &'static str) -> Self {
        self.note = Some(note);
        self
    }

    const fn disabled(mut self, reason: &'static str) -> Self {
        self.disabled = Some(reason);
        self
    }
}

impl ListRow for Setting {
    fn height(&self, _width: u16) -> u16 {
        1 + u16::from(self.note.is_some())
    }

    fn is_disabled(&self) -> bool {
        self.disabled.is_some()
    }

    fn paint_row(&self, area: Rect, buf: &mut Buffer, theme: &Theme, state: ListRowState) {
        let ascii = theme.ascii();
        let width = usize::from(area.width);
        let name_w = (width / 2).min(20);
        let name = text::display_safe(self.name);
        let name = text::truncate(&name, name_w, ascii);
        buf.set_stringn(area.x, area.y, &name, name_w, state.ink(theme));
        let value = self.disabled.unwrap_or(self.value);
        let value_w = width.saturating_sub(name_w + 2);
        if value_w >= 4 {
            let value = text::display_safe(value);
            let value = text::truncate(&value, value_w, ascii);
            let x = area.x + u16::try_from(name_w + 2).unwrap_or(0);
            buf.set_stringn(x, area.y, &value, value_w, theme.fg(Role::Muted));
        }
        if let Some(note) = self.note
            && area.height > 1
        {
            let note = text::display_safe(note);
            let note = text::truncate_words(&note, width, ascii);
            buf.set_stringn(area.x, area.y + 1, &note, width, theme.fg(Role::Muted));
        }
    }
}

fn settings() -> Vec<Setting> {
    vec![
        Setting::new("Theme", "Shoreline, follows terminal"),
        Setting::new("Motion", "Full"),
        Setting::new("Density", "Comfortable"),
        Setting::new("Provider", "Not connected").disabled("Sign in first"),
        Setting::new("Language", "English"),
    ]
}

fn list(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let rows = settings();
    List::new(&rows, ListState::new(1)).paint(area, buf, theme);
}

/// Twelve rows in five: the scrollbar, and a selection held in view.
fn list_scrolling(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let rows: Vec<String> = (1..=12).map(|n| format!("Session {n:02}")).collect();
    let mut state = ListState::new(8);
    state.scroll_into_view(rows.len(), area.height, |_| 1);
    List::new(&rows, state).paint(area, buf, theme);
}

/// Rows two lines tall; the selection takes both.
fn list_tall(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let rows = [
        Setting::new("Approvals", "Ask").note("When the agent may run a command without you"),
        Setting::new("Sandbox", "Workspace").note("Where files can be written"),
        Setting::new("Network", "Off").note("Whether tools can reach the internet"),
        Setting::new("Telemetry", "Off").note("What leaves this machine"),
    ];
    List::new(&rows, ListState::new(1)).paint(area, buf, theme);
}

/// Long names are cut with an ellipsis, between graphemes. (CJK text is
/// covered in `tests/lists.rs`: the gallery's own rule check holds the ASCII
/// profile to ASCII, which caller text in another script cannot meet.)
fn list_long(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let rows = [
        "Refactor the session store so it keeps every receipt",
        "src/components/receipt_table/cell_alignment.rs",
        "Untitled",
        "A name with trailing words that run past the edge",
    ];
    List::new(&rows, ListState::new(1)).paint(area, buf, theme);
}

fn empty_list(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let rows: [&str; 0] = [];
    List::new(&rows, ListState::default())
        .empty(
            EmptyState::new("No saved sessions")
                .action("Start one with a message")
                .mark(EmptyMark::Glyph),
        )
        .paint(area, buf, theme);
}

fn empty_state(area: Rect, buf: &mut Buffer, theme: &Theme) {
    EmptyState::new("No workflow runs yet")
        .body("Runs appear here once a workflow starts, with what each one changed.")
        .action("Start one with /workflow, or press n")
        .paint(area, buf, theme);
}

fn empty_state_small(area: Rect, buf: &mut Buffer, theme: &Theme) {
    EmptyState::new("Nothing queued")
        .body("Messages you send while the agent works wait here.")
        .action("Type a message")
        .paint(area, buf, theme);
}

pub(crate) fn entries() -> Vec<Entry> {
    vec![
        Entry {
            name: "list",
            width: 64,
            height: 5,
            draw: list,
        },
        Entry {
            name: "list-scrolling",
            width: 64,
            height: 5,
            draw: list_scrolling,
        },
        Entry {
            name: "list-tall-rows",
            width: 64,
            height: 6,
            draw: list_tall,
        },
        Entry {
            name: "list-long",
            width: 40,
            height: 4,
            draw: list_long,
        },
        Entry {
            name: "list-empty",
            width: 48,
            height: 5,
            draw: empty_list,
        },
        Entry {
            name: "empty-state",
            width: 64,
            height: 14,
            draw: empty_state,
        },
        Entry {
            name: "empty-state-small",
            width: 40,
            height: 6,
            draw: empty_state_small,
        },
    ]
}
