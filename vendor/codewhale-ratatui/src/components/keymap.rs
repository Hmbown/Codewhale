//! Keymap: the bindings a view declares, which are also its hint list.
//!
//! A [`Binding`] is a chord (spelled by [`crate::keys`], so hints and
//! bindings share one spelling), a verb, a priority and an optional group.
//! A [`Keymap`] looks keys up and turns itself into [`KeyHints`] with
//! priority folding ([`KeyHints::from_keymap`]), so the footer cannot drift
//! from what the keys do (gitui's `CommandInfo`, and the engine's
//! `ActionHint` fed from one list).
//!
//! Priority runs the other way from the design draft: a **higher** number is
//! **more important** and is kept longest when the row is narrow.

use std::borrow::Cow;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::{
    KeyHint, KeyHints, KeyHintsWords,
    keys::{Platform, chord_label, pair_label},
};

/// One key and the modifiers held with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyChord {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl From<KeyCode> for KeyChord {
    fn from(code: KeyCode) -> Self {
        Self::plain(code)
    }
}

/// Fold what terminals disagree about: `Shift` on a lone character is already
/// in its case (and some terminals add it to `?`), `BackTab` always carries
/// `Shift`, and a letter held with a command modifier may arrive in either
/// case.
///
/// `Shift` is kept once Control, Alt or Super is held: `Ctrl+Shift+E` and
/// `Ctrl+E` are two chords a keymap may bind to two actions, and their hints
/// already print differently.
fn normalize(code: KeyCode, modifiers: KeyModifiers) -> (KeyCode, KeyModifiers) {
    match code {
        KeyCode::Char(c) => {
            if modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
            {
                (KeyCode::Char(c.to_ascii_lowercase()), modifiers)
            } else {
                (code, modifiers & !KeyModifiers::SHIFT)
            }
        }
        KeyCode::BackTab => (code, modifiers & !KeyModifiers::SHIFT),
        _ => (code, modifiers),
    }
}

impl KeyChord {
    #[must_use]
    pub const fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        Self { code, modifiers }
    }

    /// A key with no modifiers.
    #[must_use]
    pub const fn plain(code: KeyCode) -> Self {
        Self::new(code, KeyModifiers::NONE)
    }

    /// A character key with no modifiers: `?`, `r`, `/`.
    #[must_use]
    pub const fn char(c: char) -> Self {
        Self::plain(KeyCode::Char(c))
    }

    /// `Ctrl+<c>`.
    #[must_use]
    pub const fn ctrl(c: char) -> Self {
        Self::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    /// `Alt+<c>` (`⌥<c>` on macOS).
    #[must_use]
    pub const fn alt(c: char) -> Self {
        Self::new(KeyCode::Char(c), KeyModifiers::ALT)
    }

    /// Whether `key` is this chord. Releases never match.
    #[must_use]
    pub fn matches(&self, key: &KeyEvent) -> bool {
        key.kind != KeyEventKind::Release
            && normalize(self.code, self.modifiers) == normalize(key.code, key.modifiers)
    }

    /// `Ctrl+O`, `⌥V`, `Enter`, `↑`: one spelling, from [`crate::keys`].
    #[must_use]
    pub fn label(&self, platform: Platform) -> String {
        chord_label(&KeyEvent::new(self.code, self.modifiers), platform)
    }
}

/// The key or keys that fire a binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BindingKeys {
    One(KeyChord),
    /// Two keys that do one thing in opposite directions, spelled `↑↓`
    /// (`Up/Down` when ASCII-safe).
    Pair(KeyChord, KeyChord),
}

impl BindingKeys {
    #[must_use]
    pub fn matches(&self, key: &KeyEvent) -> bool {
        match self {
            Self::One(a) => a.matches(key),
            Self::Pair(a, b) => a.matches(key) || b.matches(key),
        }
    }

    #[must_use]
    pub fn label(&self, platform: Platform) -> String {
        match self {
            Self::One(a) => a.label(platform),
            Self::Pair(a, b) if a.modifiers.is_empty() && b.modifiers.is_empty() => {
                pair_label(a.code, b.code, platform)
            }
            Self::Pair(a, b) => format!("{}/{}", a.label(platform), b.label(platform)),
        }
    }
}

/// The priority a binding has until it is given one.
pub const DEFAULT_PRIORITY: u8 = 100;

/// One declared binding: keys, the verb shown for them, and the action a
/// host runs when they are pressed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding<A = ()> {
    pub keys: BindingKeys,
    /// Verb first, lower case: "move", "change", "reset". An empty verb
    /// makes a binding that works but is not shown as a hint.
    pub verb: Cow<'static, str>,
    pub action: A,
    /// Higher is more important and folds last. See [`DEFAULT_PRIORITY`].
    pub priority: u8,
    /// A heading for the full help list ("Navigation", "Editing").
    pub group: Option<Cow<'static, str>>,
    /// `false` dims the hint and the key does nothing right now.
    pub enabled: bool,
    /// The key that opens the full list: shown as `? more` when other hints
    /// fold away, never as an ordinary hint.
    pub is_help: bool,
}

impl<A> Binding<A> {
    #[must_use]
    pub fn new(keys: impl Into<KeyChord>, verb: impl Into<Cow<'static, str>>, action: A) -> Self {
        Self {
            keys: BindingKeys::One(keys.into()),
            verb: verb.into(),
            action,
            priority: DEFAULT_PRIORITY,
            group: None,
            enabled: true,
            is_help: false,
        }
    }

    /// Two keys for one verb, spelled `↑↓`.
    #[must_use]
    pub fn pair(
        a: impl Into<KeyChord>,
        b: impl Into<KeyChord>,
        verb: impl Into<Cow<'static, str>>,
        action: A,
    ) -> Self {
        Self {
            keys: BindingKeys::Pair(a.into(), b.into()),
            ..Self::new(KeyCode::Null, verb, action)
        }
    }

    /// The key that opens the full list of bindings.
    #[must_use]
    pub fn help(keys: impl Into<KeyChord>, action: A) -> Self {
        Self {
            is_help: true,
            ..Self::new(keys, "", action)
        }
    }

    #[must_use]
    pub fn priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }

    #[must_use]
    pub fn group(mut self, group: impl Into<Cow<'static, str>>) -> Self {
        self.group = Some(group.into());
        self
    }

    #[must_use]
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// The hint this binding shows, or `None` for a binding with no verb
    /// and for the help binding.
    #[must_use]
    pub fn hint(&self, platform: Platform) -> Option<KeyHint> {
        if self.is_help || self.verb.is_empty() {
            return None;
        }
        let hint = KeyHint::new(self.keys.label(platform), self.verb.clone());
        Some(if self.enabled { hint } else { hint.disabled() })
    }
}

/// A declared set of bindings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Keymap<A = ()> {
    bindings: Vec<Binding<A>>,
}

impl<A> Default for Keymap<A> {
    fn default() -> Self {
        Self {
            bindings: Vec::new(),
        }
    }
}

impl<A> Keymap<A> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a binding. The first binding that matches a key wins.
    #[must_use]
    pub fn with(mut self, binding: Binding<A>) -> Self {
        self.bindings.push(binding);
        self
    }

    pub fn push(&mut self, binding: Binding<A>) {
        self.bindings.push(binding);
    }

    #[must_use]
    pub fn bindings(&self) -> &[Binding<A>] {
        &self.bindings
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// The enabled binding `key` fires. Releases and disabled bindings match
    /// nothing.
    #[must_use]
    pub fn find(&self, key: &KeyEvent) -> Option<&Binding<A>> {
        self.bindings
            .iter()
            .find(|b| b.enabled && b.keys.matches(key))
    }

    /// The action `key` fires.
    #[must_use]
    pub fn lookup(&self, key: &KeyEvent) -> Option<&A> {
        self.find(key).map(|b| &b.action)
    }

    /// The binding that opens the full list, if one is declared.
    #[must_use]
    pub fn help_binding(&self) -> Option<&Binding<A>> {
        self.bindings.iter().find(|b| b.is_help)
    }

    /// Bindings gathered by group, in the order each group first appears.
    /// Ungrouped bindings come first under `None`.
    #[must_use]
    pub fn grouped(&self) -> Vec<(Option<&str>, Vec<&Binding<A>>)> {
        let mut out: Vec<(Option<&str>, Vec<&Binding<A>>)> = Vec::new();
        for binding in &self.bindings {
            let group = binding.group.as_deref();
            match out.iter_mut().find(|(g, _)| *g == group) {
                Some((_, members)) => members.push(binding),
                None => out.push((group, vec![binding])),
            }
        }
        out.sort_by_key(|(g, _)| g.is_some());
        out
    }

    /// Every hint, in declaration order, with nothing folded away: the full
    /// list a help view shows.
    #[must_use]
    pub fn all_hints(&self, platform: Platform) -> KeyHints {
        KeyHints::new(
            self.bindings
                .iter()
                .filter_map(|b| b.hint(platform))
                .collect(),
        )
    }

    /// The hints that fit one row `width` cells wide: lowest priority fold
    /// first, `? more` says so. See [`KeyHints::from_keymap`].
    #[must_use]
    pub fn hints(&self, platform: Platform, width: u16, words: &KeyHintsWords) -> KeyHints {
        KeyHints::from_keymap(self, platform, width, words)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINUX: Platform = Platform {
        macos: false,
        ascii: false,
    };

    fn press(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn chords_match_across_terminal_disagreements() {
        let question = KeyChord::char('?');
        assert!(question.matches(&press(KeyCode::Char('?'), KeyModifiers::NONE)));
        assert!(question.matches(&press(KeyCode::Char('?'), KeyModifiers::SHIFT)));
        let ctrl_o = KeyChord::ctrl('o');
        assert!(ctrl_o.matches(&press(KeyCode::Char('O'), KeyModifiers::CONTROL)));
        assert!(!ctrl_o.matches(&press(KeyCode::Char('o'), KeyModifiers::NONE)));
        let back = KeyChord::plain(KeyCode::BackTab);
        assert!(back.matches(&press(KeyCode::BackTab, KeyModifiers::SHIFT)));
        let mut release = press(KeyCode::Char('?'), KeyModifiers::NONE);
        release.kind = KeyEventKind::Release;
        assert!(!question.matches(&release));
    }

    #[test]
    fn a_pair_is_spelled_once_and_fires_on_either_key() {
        let keys = BindingKeys::Pair(KeyChord::plain(KeyCode::Up), KeyChord::plain(KeyCode::Down));
        assert_eq!(keys.label(LINUX), "↑↓");
        assert!(keys.matches(&press(KeyCode::Down, KeyModifiers::NONE)));
        assert!(!keys.matches(&press(KeyCode::Left, KeyModifiers::NONE)));
    }

    #[test]
    fn disabled_bindings_fire_nothing_and_hint_dimmed() {
        let map = Keymap::new()
            .with(Binding::new(KeyChord::char('r'), "reset", 1).disabled())
            .with(Binding::new(KeyChord::char('r'), "refresh", 2));
        // The disabled binding does not shadow the enabled one behind it.
        assert_eq!(
            map.lookup(&press(KeyCode::Char('r'), KeyModifiers::NONE)),
            Some(&2)
        );
        let hints = map.all_hints(LINUX);
        assert!(!hints.items[0].enabled);
        assert!(hints.items[1].enabled);
    }

    #[test]
    fn groups_keep_first_seen_order_with_ungrouped_first() {
        let map = Keymap::new()
            .with(Binding::new(KeyCode::Enter, "select", ()).group("Choose"))
            .with(Binding::new(KeyCode::Esc, "cancel", ()))
            .with(Binding::new(KeyChord::char('r'), "reset", ()).group("Edit"))
            .with(Binding::new(KeyChord::char('d'), "delete", ()).group("Choose"));
        let groups = map.grouped();
        let names: Vec<_> = groups.iter().map(|(g, _)| *g).collect();
        assert_eq!(names, [None, Some("Choose"), Some("Edit")]);
        assert_eq!(groups[1].1.len(), 2);
    }

    #[test]
    fn the_help_binding_is_never_an_ordinary_hint() {
        let map = Keymap::new()
            .with(Binding::new(KeyCode::Enter, "select", ()))
            .with(Binding::help(KeyChord::char('?'), ()));
        assert_eq!(map.all_hints(LINUX).items.len(), 1);
        assert!(map.help_binding().is_some());
    }
}
