//! This module contains the logic to style some content.

use std::fmt::{self, Display, Formatter};

use super::{ContentStyle, PrintStyledContent};

/// The style with the content to be styled.
///
/// # Examples
///
/// ```rust
/// use crossterm::style::{style, Color, Attribute, Stylize};
///
/// let styled = "Hello there"
///     .with(Color::Yellow)
///     .on(Color::Blue)
///     .attribute(Attribute::Bold);
///
/// println!("{}", styled);
/// ```
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct StyledContent<D: Display> {
    /// The style (colors, content attributes).
    style: ContentStyle,
    /// A content to apply the style on.
    content: D,
}

impl<D: Display> StyledContent<D> {
    /// Creates a new `StyledContent`.
    #[inline]
    pub fn new(style: ContentStyle, content: D) -> StyledContent<D> {
        StyledContent { style, content }
    }

    /// Returns the content.
    #[inline]
    pub fn content(&self) -> &D {
        &self.content
    }

    /// Returns the style.
    #[inline]
    pub fn style(&self) -> &ContentStyle {
        &self.style
    }

    /// Returns a mutable reference to the style, so that it can be further
    /// manipulated
    #[inline]
    pub fn style_mut(&mut self) -> &mut ContentStyle {
        &mut self.style
    }
}

impl<D: Display> AsRef<ContentStyle> for StyledContent<D> {
    fn as_ref(&self) -> &ContentStyle {
        &self.style
    }
}
impl<D: Display> AsMut<ContentStyle> for StyledContent<D> {
    fn as_mut(&mut self) -> &mut ContentStyle {
        &mut self.style
    }
}

impl<D: Display> Display for StyledContent<D> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        crate::command::execute_fmt(
            f,
            PrintStyledContent(StyledContent {
                style: self.style,
                content: &self.content,
            }),
        )
    }
}
