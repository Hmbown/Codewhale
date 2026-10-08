//! Heading: the title of a page, a section or a group.
//!
//! Hierarchy comes from weight and ink, never from color or capitals: page
//! and section titles are bold `Foreground`, a sub-heading is `Muted` and
//! not bold. A section is followed by a blank row (its [`Paint::height`] is
//! two). Trailing meta text (a count, "changed 2 min ago") sits right
//! aligned in `Muted`, and gives way before the title does. Text that does
//! not fit ends in `…`, never `...` (the ASCII form is the honest `...`).

use std::borrow::Cow;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
    widgets::Widget,
};

use crate::{Paint, Role, Theme, text};

/// How much a heading heads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum HeadingLevel {
    /// The name of the whole view: bold `Foreground`.
    #[default]
    Page,
    /// A section of a view: bold `Foreground`, then a blank row.
    Section,
    /// A group inside a section: `Muted`, not bold.
    Sub,
}

impl HeadingLevel {
    /// Rows a heading of this level takes: its own and the blank row that
    /// follows a section.
    #[must_use]
    pub const fn rows(self) -> u16 {
        match self {
            HeadingLevel::Section => 2,
            HeadingLevel::Page | HeadingLevel::Sub => 1,
        }
    }
}

/// A heading with optional right-aligned meta text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Heading<'a> {
    pub text: Cow<'a, str>,
    pub level: HeadingLevel,
    /// Muted text at the right edge: a count, "changed 2 min ago".
    pub meta: Option<Cow<'a, str>>,
}

impl<'a> Heading<'a> {
    /// The narrowest row that still shows meta beside a title.
    const META_MIN_ROW: u16 = 12;
    /// Words a truncated meta must keep to be worth showing.
    const META_MIN: usize = 4;

    #[must_use]
    pub fn new(text: impl Into<Cow<'a, str>>) -> Self {
        Self {
            text: text.into(),
            level: HeadingLevel::Page,
            meta: None,
        }
    }

    #[must_use]
    pub fn level(mut self, level: HeadingLevel) -> Self {
        self.level = level;
        self
    }

    #[must_use]
    pub fn page(self) -> Self {
        self.level(HeadingLevel::Page)
    }

    #[must_use]
    pub fn section(self) -> Self {
        self.level(HeadingLevel::Section)
    }

    #[must_use]
    pub fn sub(self) -> Self {
        self.level(HeadingLevel::Sub)
    }

    #[must_use]
    pub fn meta(mut self, meta: impl Into<Cow<'a, str>>) -> Self {
        self.meta = Some(meta.into());
        self
    }

    /// The title and meta as they will be painted in a row `width` cells
    /// wide: the title keeps its room, and meta takes what is left, at most
    /// a third of the row.
    fn fit(&self, width: u16, ascii: bool) -> (String, Option<String>) {
        let width = usize::from(width);
        let title = text::display_safe(&self.text);
        let meta = self
            .meta
            .as_deref()
            .map(text::display_safe)
            .filter(|m| !m.is_empty());
        let Some(meta) = meta else {
            return (text::truncate(&title, width, ascii).into_owned(), None);
        };
        let gap = 2;
        if text::width(&title) + gap + text::width(&meta) <= width {
            return (title.into_owned(), Some(meta.into_owned()));
        }
        let budget = if width >= usize::from(Self::META_MIN_ROW) {
            text::width(&meta).min(width / 3)
        } else {
            0
        };
        let meta = text::truncate(&meta, budget, ascii);
        if budget < Self::META_MIN || meta.is_empty() {
            return (text::truncate(&title, width, ascii).into_owned(), None);
        }
        let room = width.saturating_sub(text::width(&meta) + gap);
        (
            text::truncate(&title, room, ascii).into_owned(),
            Some(meta.into_owned()),
        )
    }
}

impl Paint for Heading<'_> {
    fn paint(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let area = area.intersection(buf.area);
        if area.is_empty() {
            return;
        }
        let row = Rect { height: 1, ..area };
        let style = match self.level {
            HeadingLevel::Page | HeadingLevel::Section => {
                theme.fg(Role::Foreground).add_modifier(Modifier::BOLD)
            }
            HeadingLevel::Sub => theme.fg(Role::Muted),
        };
        let (title, meta) = self.fit(row.width, theme.ascii());
        Line::from(Span::styled(title, style)).render(row, buf);
        if let Some(meta) = meta {
            Line::from(Span::styled(meta, theme.fg(Role::Muted)))
                .right_aligned()
                .render(row, buf);
        }
    }

    fn height(&self, _width: u16, _theme: &Theme) -> u16 {
        self.level.rows()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{Profile, render, text};

    fn row(heading: &Heading<'_>, width: u16) -> String {
        let theme = Profile::DarkTrue.theme();
        let buf = render(width, 1, |area, buf| heading.paint(area, buf, &theme));
        text(&buf)
    }

    #[test]
    fn meta_sits_at_the_right_edge() {
        let h = Heading::new("Agents").section().meta("3 running");
        let line = row(&h, 40);
        assert!(line.starts_with("Agents"), "{line:?}");
        assert!(line.ends_with("3 running"), "{line:?}");
        assert_eq!(text::width(&line), 40);
    }

    #[test]
    fn the_title_keeps_its_room_and_meta_gives_way() {
        let h = Heading::new("Background workflows").meta("changed 2 minutes ago");
        let line = row(&h, 34);
        assert!(line.starts_with("Background workflows"), "{line:?}");
        assert!(line.contains('…'), "meta is cut with an ellipsis: {line:?}");
        assert!(text::width(&line) <= 34);
        // Too narrow for both: the title wins and meta is gone.
        let line = row(&h, 10);
        assert!(!line.contains("changed"), "{line:?}");
        assert!(line.starts_with("Backgroun"), "{line:?}");
    }

    #[test]
    fn a_long_title_ends_in_an_ellipsis_not_three_dots() {
        let line = row(&Heading::new("An extremely long page title indeed"), 12);
        assert_eq!(text::width(&line), 12);
        assert!(line.ends_with('…') && !line.contains("..."), "{line:?}");
    }

    #[test]
    fn rows_follow_the_level_and_tiny_areas_do_not_panic() {
        let theme = Profile::DarkTrue.theme();
        assert_eq!(Heading::new("A").height(40, &theme), 1);
        assert_eq!(Heading::new("A").section().height(40, &theme), 2);
        assert_eq!(Heading::new("A").sub().height(40, &theme), 1);
        for (w, h) in [(0, 0), (1, 1), (2, 1), (1, 0)] {
            let buf = render(w, h, |area, buf| {
                Heading::new("Title").meta("meta").paint(area, buf, &theme)
            });
            assert_eq!(buf.area.width, w);
        }
    }

    #[test]
    fn bidi_overrides_cannot_reorder_a_title() {
        let line = row(&Heading::new("rm -rf ~/\u{202E}txt.exe"), 40);
        assert!(!line.contains('\u{202E}'));
    }
}
