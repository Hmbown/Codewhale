//! Status marks: a glyph and a word, never color alone.
//!
//! Replaces the engine's `menu_style::StatusMark` (`crates/tui/src/tui/
//! menu_style.rs`, `Hmbown/CodeWhale` `58b1dd3dd`). The constructor takes a
//! [`State`], and the word comes with it, so there is no way to build a
//! colored dot that means nothing without its color.

use std::borrow::Cow;

use ratatui::{buffer::Buffer, layout::Rect, text::Span, widgets::Widget};

use crate::{Paint, Role, Theme, glyphs, text};

/// Product state. One hue, one mark and one word each, on every surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum State {
    Working,
    Done,
    NeedsYou,
    Failed,
    Stopped,
    Ready,
    /// Nobody reported a state. Shown as unknown, never as a failure.
    Unknown,
}

impl State {
    pub const ALL: [State; 7] = [
        State::Working,
        State::Done,
        State::NeedsYou,
        State::Failed,
        State::Stopped,
        State::Ready,
        State::Unknown,
    ];

    #[must_use]
    pub const fn role(self) -> Role {
        match self {
            State::Working | State::Done => Role::Live,
            State::NeedsYou => Role::Attention,
            State::Failed => Role::Danger,
            State::Stopped | State::Ready | State::Unknown => Role::Muted,
        }
    }

    #[must_use]
    pub const fn glyph(self) -> &'static str {
        match self {
            State::Working => glyphs::CURRENT,
            State::Done => glyphs::DONE,
            State::NeedsYou => glyphs::ATTENTION,
            State::Failed => glyphs::FAILED,
            State::Stopped => glyphs::STOPPED,
            State::Ready => glyphs::READY,
            State::Unknown => glyphs::UNKNOWN,
        }
    }

    /// The English default word, from [`StateWords::english`]. Hosts pass
    /// their own words with [`StatusMark::with_words`] or
    /// [`StatusMark::word`].
    #[must_use]
    pub const fn word(self) -> &'static str {
        StateWords::english(self)
    }
}

/// The word shown beside each [`State`]'s mark. The kit owns no copy beyond
/// this default: a host fills one per locale and passes it in. `Default` is
/// English.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StateWords {
    pub working: Cow<'static, str>,
    pub done: Cow<'static, str>,
    pub needs_you: Cow<'static, str>,
    pub failed: Cow<'static, str>,
    pub stopped: Cow<'static, str>,
    pub ready: Cow<'static, str>,
    pub unknown: Cow<'static, str>,
}

impl StateWords {
    /// The English word for `state`: the only copy in the kit's status
    /// marks, and what [`StateWords::default`] holds.
    #[must_use]
    pub const fn english(state: State) -> &'static str {
        match state {
            State::Working => "Working",
            State::Done => "Done",
            State::NeedsYou => "Needs you",
            State::Failed => "Failed",
            State::Stopped => "Stopped",
            State::Ready => "Ready",
            State::Unknown => "Unknown",
        }
    }

    /// The word for `state`.
    #[must_use]
    pub fn get(&self, state: State) -> &str {
        match state {
            State::Working => &self.working,
            State::Done => &self.done,
            State::NeedsYou => &self.needs_you,
            State::Failed => &self.failed,
            State::Stopped => &self.stopped,
            State::Ready => &self.ready,
            State::Unknown => &self.unknown,
        }
    }
}

impl Default for StateWords {
    fn default() -> Self {
        let english = |state| Cow::Borrowed(Self::english(state));
        Self {
            working: english(State::Working),
            done: english(State::Done),
            needs_you: english(State::NeedsYou),
            failed: english(State::Failed),
            stopped: english(State::Stopped),
            ready: english(State::Ready),
            unknown: english(State::Unknown),
        }
    }
}

/// `● Working`, `✓ Done`, `◆ Needs you`: the mark takes the state's hue; the
/// word stays `Foreground` so it reads at full contrast.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusMark {
    pub state: State,
    pub word: Cow<'static, str>,
    /// Paint the word in the state's hue too, for a mark that stands alone.
    pub tinted: bool,
}

impl StatusMark {
    #[must_use]
    pub fn new(state: State) -> Self {
        Self {
            state,
            word: Cow::Borrowed(state.word()),
            tinted: false,
        }
    }

    /// A mark whose word comes from the host's [`StateWords`].
    #[must_use]
    pub fn with_words(state: State, words: &StateWords) -> Self {
        Self {
            state,
            word: Cow::Owned(words.get(state).to_owned()),
            tinted: false,
        }
    }

    #[must_use]
    pub fn word(mut self, word: impl Into<Cow<'static, str>>) -> Self {
        self.word = word.into();
        self
    }

    #[must_use]
    pub fn tinted(mut self) -> Self {
        self.tinted = true;
        self
    }

    /// The mark's glyph for this terminal.
    #[must_use]
    pub fn glyph(&self, theme: &Theme) -> &'static str {
        glyphs::pick(self.state.glyph(), theme.ascii())
    }

    #[must_use]
    pub fn spans(&self, theme: &Theme) -> Vec<Span<'static>> {
        let hue = theme.fg(self.state.role());
        let word_style = if self.tinted {
            hue
        } else {
            theme.fg(Role::Foreground)
        };
        vec![
            Span::styled(self.glyph(theme), hue),
            Span::raw(" "),
            Span::styled(text::display_safe(&self.word).into_owned(), word_style),
        ]
    }
}

impl Paint for StatusMark {
    fn paint(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        ratatui::text::Line::from(self.spans(theme)).render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::Profile;

    #[test]
    fn every_state_has_a_distinct_mark_and_a_word() {
        for profile in [Profile::DarkTrue, Profile::Ascii] {
            let theme = profile.theme();
            let marks: Vec<_> = State::ALL
                .iter()
                .map(|s| StatusMark::new(*s).glyph(&theme))
                .collect();
            for (i, a) in marks.iter().enumerate() {
                assert!(!a.trim().is_empty());
                for b in &marks[i + 1..] {
                    assert_ne!(a, b, "{profile:?}: two states share {a}");
                }
            }
        }
        for state in State::ALL {
            assert!(!state.word().is_empty());
        }
    }

    /// The default words are the English ones, and a host's words reach the
    /// painted mark.
    #[test]
    fn words_are_parameters_with_english_defaults() {
        let english = StateWords::default();
        for state in State::ALL {
            assert_eq!(english.get(state), state.word());
            assert_eq!(
                StatusMark::new(state),
                StatusMark::with_words(state, &english)
            );
        }
        let host = StateWords {
            needs_you: "Braucht dich".into(),
            ..StateWords::default()
        };
        let theme = Profile::DarkTrue.theme();
        let buf = crate::testing::render(20, 1, |area, buf| {
            StatusMark::with_words(State::NeedsYou, &host).paint(area, buf, &theme);
        });
        assert_eq!(crate::testing::text(&buf), "\u{25c6} Braucht dich");
        assert_eq!(host.get(State::Done), "Done");
    }
}
