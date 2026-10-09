//! Key hints: `↑↓ move · Enter select · Esc cancel`.
//!
//! Lifted from the engine's `ActionHint` and `action_footer_lines`
//! (`crates/tui/src/tui/views/mod.rs`, `Hmbown/CodeWhale` `58b1dd3dd`): hints
//! wrap onto another row rather than run off the edge, and no action is ever
//! dropped (#3732). The key is bold `Foreground` and the verb `Muted`, joined
//! by ` · `. Verbs come first and lower case: "move", not "Navigation".
//!
//! A row built from a [`Keymap`] ([`KeyHints::from_keymap`]) folds by
//! priority when it is too narrow: the lowest-priority hints drop first, the
//! highest stays, and a `? more` hint says that something was dropped.

use std::borrow::Cow;
use unicode_segmentation::UnicodeSegmentation;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

use crate::{Keymap, Paint, Role, Theme, keys::Platform, text};

/// One key and what it does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyHint {
    pub keys: Cow<'static, str>,
    pub verb: Cow<'static, str>,
    /// `false` dims the hint: the key exists but does nothing right now.
    pub enabled: bool,
}

impl KeyHint {
    #[must_use]
    pub fn new(keys: impl Into<Cow<'static, str>>, verb: impl Into<Cow<'static, str>>) -> Self {
        Self {
            keys: keys.into(),
            verb: verb.into(),
            enabled: true,
        }
    }

    /// A hint whose key is spelled from a key event by [`crate::keys`], so
    /// hints and bindings share one spelling: `Ctrl+O`, `⌥V`, `Shift+Tab`.
    #[must_use]
    pub fn chord(
        key: &crossterm::event::KeyEvent,
        platform: crate::keys::Platform,
        verb: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self::new(crate::keys::chord_label(key, platform), verb)
    }

    #[must_use]
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    fn width(&self) -> usize {
        text::width(&text::display_safe(&self.keys))
            + 1
            + text::width(&text::display_safe(&self.verb))
    }

    fn spans(&self, theme: &Theme) -> [Span<'static>; 3] {
        let (key_style, verb_style) = if self.enabled {
            (
                theme.fg(Role::Foreground).add_modifier(Modifier::BOLD),
                theme.fg(Role::Muted),
            )
        } else {
            (
                theme.fg(Role::Muted),
                theme.fg(Role::Muted).add_modifier(Modifier::DIM),
            )
        };
        [
            Span::styled(text::display_safe(&self.keys).into_owned(), key_style),
            Span::raw(" "),
            Span::styled(text::display_safe(&self.verb).into_owned(), verb_style),
        ]
    }
}

/// The words [`KeyHints::from_keymap`] shows itself. The kit owns no copy
/// beyond this English default: a host passes its own per locale.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyHintsWords {
    /// The verb beside the help key when hints were folded away: `? more`.
    pub more: Cow<'static, str>,
}

impl Default for KeyHintsWords {
    fn default() -> Self {
        Self {
            more: Cow::Borrowed("more"),
        }
    }
}

/// A row of key hints that wraps instead of clipping.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KeyHints {
    pub items: Vec<KeyHint>,
}

impl KeyHints {
    #[must_use]
    pub fn new(items: Vec<KeyHint>) -> Self {
        Self { items }
    }

    /// Hints for the bindings of `keymap` that fit one row `width` cells
    /// wide, in declaration order.
    ///
    /// When the row is too narrow the lowest-priority hints drop first (a
    /// tie drops the later declaration), the highest-priority hint is always
    /// kept, and a hint is never cut in half. If something was dropped and
    /// the keymap has a help binding, the row ends with `? more` (the help
    /// key, then `words.more`). Disabled bindings show dimmed, and bindings
    /// with no verb and the help binding itself are not ordinary hints.
    #[must_use]
    pub fn from_keymap<A>(
        keymap: &Keymap<A>,
        platform: Platform,
        width: u16,
        words: &KeyHintsWords,
    ) -> Self {
        let shown: Vec<(u8, KeyHint)> = keymap
            .bindings()
            .iter()
            .filter_map(|b| b.hint(platform).map(|h| (b.priority, h)))
            .collect();
        let more = keymap
            .help_binding()
            .map(|b| KeyHint::new(b.keys.label(platform), words.more.clone()));
        let (width, sep) = (usize::from(width), Self::separator_width(platform.ascii));

        // Most important first; a tie keeps the earlier declaration.
        let mut by_importance: Vec<usize> = (0..shown.len()).collect();
        by_importance.sort_by_key(|&i| (std::cmp::Reverse(shown[i].0), i));
        let fits = |keep: usize| -> bool {
            let hints: usize = by_importance[..keep]
                .iter()
                .map(|&i| shown[i].1.width())
                .sum();
            let more = match (&more, keep < shown.len()) {
                (Some(more), true) => sep + more.width(),
                _ => 0,
            };
            hints + sep * keep.saturating_sub(1) + more <= width
        };
        let mut keep = shown.len();
        while keep > 1 && !fits(keep) {
            keep -= 1;
        }

        let mut kept = by_importance[..keep].to_vec();
        kept.sort_unstable();
        let mut items: Vec<KeyHint> = kept.into_iter().map(|i| shown[i].1.clone()).collect();
        if keep < shown.len()
            && let Some(more) = more
        {
            // Even the one hint that must stay may leave no room for it.
            let room = fits_after(&items, sep, width, more.width());
            if room {
                items.push(more);
            }
        }
        Self::new(items)
    }

    /// The width of the separator [`KeyHints::lines`] draws: ` · `, or two
    /// spaces when ASCII-safe.
    fn separator_width(ascii: bool) -> usize {
        if ascii { 2 } else { 3 }
    }

    /// ` · ` between hints; two spaces when ASCII-safe, because the ASCII
    /// form of `·` is `.` and would read as punctuation.
    fn separator(theme: &Theme) -> Span<'static> {
        if theme.ascii() {
            Span::raw("  ")
        } else {
            Span::styled(" · ", theme.fg(Role::Border))
        }
    }

    /// Lay the hints out in rows no wider than `width`. Packs greedily and
    /// starts a new row rather than truncating; a hint wider than `width`
    /// wraps between styled graphemes, preserving all its actions.
    #[must_use]
    pub fn lines(&self, width: u16, theme: &Theme) -> Vec<Line<'static>> {
        let width = usize::from(width);
        if self.items.is_empty() || width == 0 {
            return Vec::new();
        }
        let sep = Self::separator(theme);
        let sep_width = text::width(&sep.content);
        let mut lines = Vec::new();
        let mut current: Vec<Span<'static>> = Vec::new();
        let mut used = 0usize;
        for hint in &self.items {
            let w = hint.width();
            if w > width {
                if !current.is_empty() {
                    lines.push(Line::from(std::mem::take(&mut current)));
                }
                lines.extend(wrap_spans(&hint.spans(theme), width as u16));
                used = 0;
                continue;
            }
            if !current.is_empty() && used + sep_width + w > width {
                lines.push(Line::from(std::mem::take(&mut current)));
                used = 0;
            }
            if !current.is_empty() {
                current.push(sep.clone());
                used += sep_width;
            }
            current.extend(hint.spans(theme));
            used += w;
        }
        if !current.is_empty() {
            lines.push(Line::from(current));
        }
        lines
    }
}

/// Hard-wrap already sanitized spans while retaining their styles. A wide
/// grapheme in a one-cell viewport becomes `?`: it cannot occupy half a cell.
pub(crate) fn wrap_spans(spans: &[Span<'static>], width: u16) -> Vec<Line<'static>> {
    let width = usize::from(width);
    if width == 0 {
        return Vec::new();
    }
    let mut lines = Vec::new();
    let mut current: Vec<Span<'static>> = Vec::new();
    let mut used = 0;
    for span in spans {
        for grapheme in span.content.graphemes(true) {
            let cells = text::width(grapheme);
            let (grapheme, cells) = if cells > width {
                ("?", 1)
            } else {
                (grapheme, cells)
            };
            if used + cells > width && !current.is_empty() {
                lines.push(Line::from(std::mem::take(&mut current)));
                used = 0;
            }
            if let Some(last) = current.last_mut()
                && last.style == span.style
            {
                last.content.to_mut().push_str(grapheme);
            } else {
                current.push(Span::styled(grapheme.to_string(), span.style));
            }
            used += cells;
        }
    }
    if !current.is_empty() {
        lines.push(Line::from(current));
    }
    lines
}

/// Whether one more hint of `extra` cells fits after `items` in `width`.
fn fits_after(items: &[KeyHint], sep: usize, width: usize, extra: usize) -> bool {
    let used: usize =
        items.iter().map(KeyHint::width).sum::<usize>() + sep * items.len().saturating_sub(1);
    used + sep + extra <= width
}

impl Paint for KeyHints {
    fn paint(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let area = area.intersection(buf.area);
        Paragraph::new(self.lines(area.width, theme)).render(area, buf);
    }

    fn height(&self, width: u16, theme: &Theme) -> u16 {
        u16::try_from(self.lines(width, theme).len()).unwrap_or(u16::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::Profile;

    fn hints() -> KeyHints {
        KeyHints::new(vec![
            KeyHint::new("↑↓", "move"),
            KeyHint::new("Enter", "select"),
            KeyHint::new("Esc", "cancel"),
        ])
    }

    fn plain(lines: &[Line<'_>]) -> Vec<String> {
        lines.iter().map(|l| l.to_string()).collect()
    }

    #[test]
    fn packs_on_one_row_when_it_fits() {
        let theme = Profile::DarkTrue.theme();
        assert_eq!(
            plain(&hints().lines(80, &theme)),
            ["↑↓ move · Enter select · Esc cancel"]
        );
    }

    #[test]
    fn chord_hints_use_the_key_label_authority() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let mac = crate::keys::Platform {
            macos: true,
            ascii: false,
        };
        let alt_v = KeyEvent::new(KeyCode::Char('v'), KeyModifiers::ALT);
        assert_eq!(KeyHint::chord(&alt_v, mac, "details").keys, "⌥V");
        let ctrl_o = KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL);
        assert_eq!(KeyHint::chord(&ctrl_o, mac, "reasoning").keys, "Ctrl+O");
    }

    #[test]
    fn wraps_and_never_drops_an_action() {
        let theme = Profile::DarkTrue.theme();
        let rows = plain(&hints().lines(22, &theme));
        assert_eq!(rows, ["↑↓ move · Enter select", "Esc cancel"]);
        let rows = plain(&hints().lines(4, &theme));
        assert_eq!(
            rows.concat(),
            "↑↓ moveEnter selectEsc cancel",
            "every action survives even when an individual hint must wrap"
        );
        assert!(rows.iter().all(|row| text::width(row) <= 4));
    }
}
