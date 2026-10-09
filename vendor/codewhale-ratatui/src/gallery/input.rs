//! Gallery entries for package Input: the text input in each of its states,
//! and a form that has just refused a submit.
//!
//! Each entry builds its state the way a host would, from keys, so the
//! gallery doubles as a usage example.

use std::borrow::Cow;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{buffer::Buffer, layout::Rect};

use super::Entry;
use crate::{Form, FormField, FormState, Paint, TextInput, TextInputState, Theme};

pub(crate) fn entries() -> Vec<Entry> {
    vec![
        Entry {
            name: "text-input-empty",
            width: 40,
            height: 2,
            draw: empty,
        },
        Entry {
            name: "text-input-typed",
            width: 40,
            height: 2,
            draw: typed,
        },
        Entry {
            name: "text-input-unfocused",
            width: 40,
            height: 2,
            draw: unfocused,
        },
        Entry {
            name: "text-input-invalid",
            width: 40,
            height: 3,
            draw: invalid,
        },
        Entry {
            name: "text-input-disabled",
            width: 40,
            height: 3,
            draw: disabled,
        },
        Entry {
            name: "text-input-secret",
            width: 40,
            height: 2,
            draw: secret,
        },
        Entry {
            name: "text-input-long",
            width: 40,
            height: 2,
            draw: long,
        },
        Entry {
            name: "text-input-wide-text",
            width: 40,
            height: 2,
            draw: wide_text,
        },
        Entry {
            name: "form",
            width: 60,
            height: 14,
            draw: form,
        },
    ]
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn type_text(state: &mut TextInputState, text: &str) {
    for c in text.chars() {
        state.handle_key(key(KeyCode::Char(c)));
    }
}

fn empty(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let state = TextInputState::new();
    TextInput::new(&state)
        .label("Port")
        .placeholder("e.g. 8080")
        .focused(true)
        .paint(area, buf, theme);
}

fn typed(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let mut state = TextInputState::new();
    type_text(&mut state, "nightly-report");
    TextInput::new(&state)
        .label("Name")
        .focused(true)
        .paint(area, buf, theme);
}

fn unfocused(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let state = TextInputState::with_text("nightly-report");
    TextInput::new(&state).label("Name").paint(area, buf, theme);
}

fn invalid(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let state = TextInputState::with_text("70000");
    TextInput::new(&state)
        .label("Port")
        .focused(true)
        .error("Enter a port from 1 to 65535.")
        .paint(area, buf, theme);
}

fn disabled(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let state = TextInputState::with_text("shannon-labs");
    TextInput::new(&state)
        .label("Organization")
        .disabled("Set by your administrator")
        .paint(area, buf, theme);
}

fn secret(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let mut state = TextInputState::secret();
    type_text(&mut state, "hunter2 hunter2");
    TextInput::new(&state)
        .label("API key")
        .focused(true)
        .paint(area, buf, theme);
}

/// Longer than the field: clipped on both sides, the cursor kept in view.
fn long(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let mut state = TextInputState::with_text(
        "/Volumes/work/shannon-labs/codewhale/crates/tui/src/tui/views/automations/editor.rs",
    );
    for _ in 0.."editor.rs".len() + 22 {
        state.handle_key(key(KeyCode::Left));
    }
    TextInput::new(&state)
        .label("Path")
        .focused(true)
        .paint(area, buf, theme);
}

/// Double-width characters, a combining mark and an emoji ZWJ sequence: each
/// is one step for the cursor.
fn wide_text(area: Rect, buf: &mut Buffer, theme: &Theme) {
    // ASCII-safe terminals get ASCII sample text: the frame rules want
    // ASCII-only output there. The unit tests cover wide text in every mode.
    let text = if theme.ascii() {
        "wide, combining and emoji text goes here"
    } else {
        "鲸鱼日报 cafe\u{301} 👩\u{200d}💻 夜间"
    };
    let mut state = TextInputState::with_text(text);
    state.handle_key(key(KeyCode::Left));
    TextInput::new(&state)
        .label("Title")
        .focused(true)
        .paint(area, buf, theme);
}

fn project(text: &str) -> Result<(), Cow<'static, str>> {
    if text.trim().is_empty() {
        Err("Choose a project.".into())
    } else {
        Ok(())
    }
}

fn name(text: &str) -> Result<(), Cow<'static, str>> {
    if text.trim().is_empty() {
        Err("Give it a name.".into())
    } else if text.contains(char::is_whitespace) {
        Err("Use dashes instead of spaces.".into())
    } else {
        Ok(())
    }
}

/// A form after Enter on a form with an empty project: the error shows under
/// its field, focus is there, and nothing typed was lost.
fn form(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let mut state = FormState::new(vec![
        FormField::text("Name").validate(name),
        FormField::text("Project")
            .placeholder("Choose a project")
            .validate(project),
        FormField::read_only("Schedule", "every day at 7"),
        FormField::check("Notify me", true).help("Sends one message when it finishes."),
    ]);
    type_text_into(&mut state, "nightly-report");
    state.handle_key(key(KeyCode::Enter));
    Form::new(&state).paint(area, buf, theme);
}

fn type_text_into(state: &mut FormState, text: &str) {
    for c in text.chars() {
        state.handle_key(key(KeyCode::Char(c)));
    }
}
