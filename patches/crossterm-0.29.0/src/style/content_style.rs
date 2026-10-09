//! This module contains the `content style` that can be applied to an `styled content`.

use std::fmt::Display;

use crate::style::{Attributes, Color, StyledContent};

/// The style that can be put on content.
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
pub struct ContentStyle {
    /// The foreground color.
    pub foreground_color: Option<Color>,
    /// The background color.
    pub background_color: Option<Color>,
    /// The underline color.
    pub underline_color: Option<Color>,
    /// List of attributes.
    pub attributes: Attributes,
}

impl ContentStyle {
    /// Creates a `StyledContent` by applying the style to the given `val`.
    #[inline]
    pub fn apply<D: Display>(self, val: D) -> StyledContent<D> {
        StyledContent::new(self, val)
    }

    /// Creates a new `ContentStyle`.
    #[inline]
    pub fn new() -> ContentStyle {
        ContentStyle::default()
    }
}

impl AsRef<ContentStyle> for ContentStyle {
    fn as_ref(&self) -> &Self {
        self
    }
}
impl AsMut<ContentStyle> for ContentStyle {
    fn as_mut(&mut self) -> &mut Self {
        self
    }
}
