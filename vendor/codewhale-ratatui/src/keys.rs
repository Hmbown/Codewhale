//! Key labels, spelled one way everywhere.
//!
//! Spelling follows the Codewhale engine's help catalog
//! (`crates/tui/src/tui/keybindings.rs`) and platform rules from
//! `key_shortcuts.rs` and `shell_key_routing.rs::display_chord`
//! (`Hmbown/CodeWhale` `58b1dd3dd`; `display_chord` last changed in
//! `da1937a048`): `Ctrl+O`, `Alt+V` (`⌥V` on macOS), `Shift+Tab`, `PgUp`.
//! Arrows are glyphs (`↑↓`), and ASCII-safe terminals get words
//! (`Up/Down`).

use std::borrow::Cow;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Where the labels will be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Platform {
    pub macos: bool,
    pub ascii: bool,
}

impl Platform {
    /// The platform this binary was built for, with ASCII-safety supplied by
    /// the caller (usually [`crate::theme::Theme::ascii`]).
    #[must_use]
    pub const fn current(ascii: bool) -> Self {
        Self {
            macos: cfg!(target_os = "macos"),
            ascii,
        }
    }
}

/// Render a catalog chord such as `Alt+V` for the platform: macOS shows
/// `⌥V` and `fn+F1`; ASCII-safe terminals keep the portable spelling.
#[must_use]
pub fn display_chord(chord: &str, platform: Platform) -> Cow<'_, str> {
    if platform.ascii || !platform.macos {
        return Cow::Borrowed(chord);
    }
    let rendered = chord.replace("Alt+", "⌥").replace("F1", "fn+F1");
    if rendered == chord {
        Cow::Borrowed(chord)
    } else {
        Cow::Owned(rendered)
    }
}

/// The label for one key without modifiers.
#[must_use]
pub fn key_name(code: KeyCode, platform: Platform) -> Cow<'static, str> {
    let arrow = |glyph: &'static str, word: &'static str| {
        Cow::Borrowed(if platform.ascii { word } else { glyph })
    };
    match code {
        KeyCode::Up => arrow("↑", "Up"),
        KeyCode::Down => arrow("↓", "Down"),
        KeyCode::Left => arrow("←", "Left"),
        KeyCode::Right => arrow("→", "Right"),
        KeyCode::Enter => Cow::Borrowed("Enter"),
        KeyCode::Esc => Cow::Borrowed("Esc"),
        KeyCode::Tab => Cow::Borrowed("Tab"),
        KeyCode::BackTab => Cow::Borrowed("Shift+Tab"),
        KeyCode::Backspace => Cow::Borrowed("Backspace"),
        KeyCode::Delete => Cow::Borrowed("Delete"),
        KeyCode::Insert => Cow::Borrowed("Insert"),
        KeyCode::Home => Cow::Borrowed("Home"),
        KeyCode::End => Cow::Borrowed("End"),
        KeyCode::PageUp => Cow::Borrowed("PgUp"),
        KeyCode::PageDown => Cow::Borrowed("PgDn"),
        KeyCode::F(n) => Cow::Owned(format!("F{n}")),
        KeyCode::Char(' ') => Cow::Borrowed("Space"),
        KeyCode::Char(c) => Cow::Owned(c.to_string()),
        _ => Cow::Borrowed("?"),
    }
}

/// The label for a key event: `Ctrl+O`, `⌥V`, `Ctrl+Shift+E`, `Enter`, `a`.
/// A letter under a modifier is shown in capitals, as the catalog does.
#[must_use]
pub fn chord_label(key: &KeyEvent, platform: Platform) -> String {
    let mods = key.modifiers;
    let mut out = String::new();
    if mods.contains(KeyModifiers::CONTROL) {
        out.push_str("Ctrl+");
    }
    if mods.contains(KeyModifiers::ALT) {
        out.push_str("Alt+");
    }
    if mods.contains(KeyModifiers::SUPER) {
        out.push_str(if platform.macos { "Cmd+" } else { "Super+" });
    }
    let shifted_letter = matches!(key.code, KeyCode::Char(c) if c.is_ascii_uppercase());
    if mods.contains(KeyModifiers::SHIFT) && !matches!(key.code, KeyCode::BackTab) {
        out.push_str("Shift+");
    }
    let name = match key.code {
        KeyCode::Char(c) if !out.is_empty() || shifted_letter => {
            Cow::Owned(c.to_ascii_uppercase().to_string())
        }
        code => key_name(code, platform),
    };
    out.push_str(&name);
    display_chord(&out, platform).into_owned()
}

/// Two keys that do one thing in opposite directions: `↑↓`, or `Up/Down`
/// when ASCII-safe.
#[must_use]
pub fn pair_label(a: KeyCode, b: KeyCode, platform: Platform) -> String {
    let (a, b) = (key_name(a, platform), key_name(b, platform));
    if platform.ascii {
        format!("{a}/{b}")
    } else {
        format!("{a}{b}")
    }
}

/// `Ctrl` on Linux and Windows, or `Cmd` (SUPER) on macOS.
#[must_use]
pub fn has_control_like_modifier(modifiers: KeyModifiers, platform: Platform) -> bool {
    modifiers.contains(KeyModifiers::CONTROL)
        || (platform.macos && modifiers.contains(KeyModifiers::SUPER))
}

/// `Alt+<key>` navigation: requires Alt, rejects Ctrl and Super so it never
/// collides with clipboard or window-management chords. Shift is allowed.
#[must_use]
pub fn alt_nav_modifiers(modifiers: KeyModifiers) -> bool {
    modifiers.contains(KeyModifiers::ALT)
        && !modifiers.contains(KeyModifiers::CONTROL)
        && !modifiers.contains(KeyModifiers::SUPER)
}

/// `Ctrl+H` is the ASCII backspace many terminals still send for Backspace.
#[must_use]
pub fn is_ctrl_h_backspace(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('h'))
        && key.modifiers.contains(KeyModifiers::CONTROL)
        && !key.modifiers.contains(KeyModifiers::ALT)
        && !key.modifiers.contains(KeyModifiers::SUPER)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINUX: Platform = Platform {
        macos: false,
        ascii: false,
    };
    const MAC: Platform = Platform {
        macos: true,
        ascii: false,
    };
    const ASCII: Platform = Platform {
        macos: true,
        ascii: true,
    };

    #[test]
    fn chords_match_the_engine_catalog_spelling() {
        let ctrl_o = KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL);
        assert_eq!(chord_label(&ctrl_o, LINUX), "Ctrl+O");
        let alt_v = KeyEvent::new(KeyCode::Char('v'), KeyModifiers::ALT);
        assert_eq!(chord_label(&alt_v, LINUX), "Alt+V");
        assert_eq!(chord_label(&alt_v, MAC), "⌥V");
        assert_eq!(chord_label(&alt_v, ASCII), "Alt+V");
        let ctrl_shift_e = KeyEvent::new(
            KeyCode::Char('E'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        );
        assert_eq!(chord_label(&ctrl_shift_e, LINUX), "Ctrl+Shift+E");
        let back_tab = KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT);
        assert_eq!(chord_label(&back_tab, LINUX), "Shift+Tab");
        let plain = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        assert_eq!(chord_label(&plain, LINUX), "a");
        let page = KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE);
        assert_eq!(chord_label(&page, LINUX), "PgDn");
    }

    #[test]
    fn arrows_are_one_spelling() {
        assert_eq!(pair_label(KeyCode::Up, KeyCode::Down, LINUX), "↑↓");
        assert_eq!(pair_label(KeyCode::Up, KeyCode::Down, ASCII), "Up/Down");
        assert_eq!(display_chord("F1", MAC), "fn+F1");
        assert_eq!(display_chord("Ctrl+O", MAC), "Ctrl+O");
    }

    #[test]
    fn platform_predicates() {
        assert!(has_control_like_modifier(KeyModifiers::SUPER, MAC));
        assert!(!has_control_like_modifier(KeyModifiers::SUPER, LINUX));
        assert!(alt_nav_modifiers(KeyModifiers::ALT | KeyModifiers::SHIFT));
        assert!(!alt_nav_modifiers(
            KeyModifiers::ALT | KeyModifiers::CONTROL
        ));
        assert!(is_ctrl_h_backspace(&KeyEvent::new(
            KeyCode::Char('h'),
            KeyModifiers::CONTROL
        )));
    }
}
