use std::fmt::Display;

use super::{style, Attribute, Color, ContentStyle, StyledContent};

macro_rules! stylize_method {
    ($method_name:ident Attribute::$attribute:ident) => {
        calculated_docs! {
            #[doc = concat!(
                "Applies the [`",
                stringify!($attribute),
                "`](Attribute::",
                stringify!($attribute),
                ") attribute to the text.",
            )]
            fn $method_name(self) -> Self::Styled {
                self.attribute(Attribute::$attribute)
            }
        }
    };
    ($method_name_fg:ident, $method_name_bg:ident, $method_name_ul:ident Color::$color:ident) => {
        calculated_docs! {
            #[doc = concat!(
                "Sets the foreground color to [`",
                stringify!($color),
                "`](Color::",
                stringify!($color),
                ")."
            )]
            fn $method_name_fg(self) -> Self::Styled {
                self.with(Color::$color)
            }

            #[doc = concat!(
                "Sets the background color to [`",
                stringify!($color),
                "`](Color::",
                stringify!($color),
                ")."
            )]
            fn $method_name_bg(self) -> Self::Styled {
                self.on(Color::$color)
            }

            #[doc = concat!(
                "Sets the underline color to [`",
                stringify!($color),
                "`](Color::",
                stringify!($color),
                ")."
            )]
            fn $method_name_ul(self) -> Self::Styled {
                self.underline(Color::$color)
            }
        }
    };
}

/// Provides a set of methods to set attributes and colors.
///
/// # Examples
///
/// ```no_run
/// use crossterm::style::Stylize;
///
/// println!("{}", "Bold text".bold());
/// println!("{}", "Underlined text".underlined());
/// println!("{}", "Negative text".negative());
/// println!("{}", "Red on blue".red().on_blue());
/// ```
pub trait Stylize: Sized {
    /// This type with styles applied.
    type Styled: AsRef<ContentStyle> + AsMut<ContentStyle>;

    /// Styles this type.
    fn stylize(self) -> Self::Styled;

    /// Sets the foreground color.
    fn with(self, color: Color) -> Self::Styled {
        let mut styled = self.stylize();
        styled.as_mut().foreground_color = Some(color);
        styled
    }

    /// Sets the background color.
    fn on(self, color: Color) -> Self::Styled {
        let mut styled = self.stylize();
        styled.as_mut().background_color = Some(color);
        styled
    }

    /// Sets the underline color.
    fn underline(self, color: Color) -> Self::Styled {
        let mut styled = self.stylize();
        styled.as_mut().underline_color = Some(color);
        styled
    }

    /// Styles the content with the attribute.
    fn attribute(self, attr: Attribute) -> Self::Styled {
        let mut styled = self.stylize();
        styled.as_mut().attributes.set(attr);
        styled
    }

    stylize_method!(reset Attribute::Reset);
    stylize_method!(bold Attribute::Bold);
    stylize_method!(underlined Attribute::Underlined);
    stylize_method!(reverse Attribute::Reverse);
    stylize_method!(dim Attribute::Dim);
    stylize_method!(italic Attribute::Italic);
    stylize_method!(negative Attribute::Reverse);
    stylize_method!(slow_blink Attribute::SlowBlink);
    stylize_method!(rapid_blink Attribute::RapidBlink);
    stylize_method!(hidden Attribute::Hidden);
    stylize_method!(crossed_out Attribute::CrossedOut);

    stylize_method!(black, on_black, underline_black Color::Black);
    stylize_method!(dark_grey, on_dark_grey, underline_dark_grey Color::DarkGrey);
    stylize_method!(red, on_red, underline_red Color::Red);
    stylize_method!(dark_red, on_dark_red, underline_dark_red Color::DarkRed);
    stylize_method!(green, on_green, underline_green Color::Green);
    stylize_method!(dark_green, on_dark_green, underline_dark_green Color::DarkGreen);
    stylize_method!(yellow, on_yellow, underline_yellow Color::Yellow);
    stylize_method!(dark_yellow, on_dark_yellow, underline_dark_yellow Color::DarkYellow);
    stylize_method!(blue, on_blue, underline_blue Color::Blue);
    stylize_method!(dark_blue, on_dark_blue, underline_dark_blue Color::DarkBlue);
    stylize_method!(magenta, on_magenta, underline_magenta Color::Magenta);
    stylize_method!(dark_magenta, on_dark_magenta, underline_dark_magenta Color::DarkMagenta);
    stylize_method!(cyan, on_cyan, underline_cyan Color::Cyan);
    stylize_method!(dark_cyan, on_dark_cyan, underline_dark_cyan Color::DarkCyan);
    stylize_method!(white, on_white, underline_white Color::White);
    stylize_method!(grey, on_grey, underline_grey Color::Grey);
}

macro_rules! impl_stylize_for_display {
    ($($t:ty),*) => { $(
        impl Stylize for $t {
            type Styled = StyledContent<Self>;
            #[inline]
            fn stylize(self) -> Self::Styled {
                style(self)
            }
        }
    )* }
}
impl_stylize_for_display!(String, char, &str);

impl Stylize for ContentStyle {
    type Styled = Self;
    #[inline]
    fn stylize(self) -> Self::Styled {
        self
    }
}
impl<D: Display> Stylize for StyledContent<D> {
    type Styled = StyledContent<D>;
    fn stylize(self) -> Self::Styled {
        self
    }
}

// Workaround for https://github.com/rust-lang/rust/issues/78835
macro_rules! calculated_docs {
    ($(#[doc = $doc:expr] $item:item)*) => { $(#[doc = $doc] $item)* };
}
// Remove once https://github.com/rust-lang/rust-clippy/issues/7106 stabilizes.
#[allow(clippy::single_component_path_imports)]
#[allow(clippy::useless_attribute)]
use calculated_docs;

#[cfg(test)]
mod tests {
    use super::super::{Attribute, Color, ContentStyle, Stylize};

    #[test]
    fn set_fg_bg_add_attr() {
        let style = ContentStyle::new()
            .with(Color::Blue)
            .on(Color::Red)
            .attribute(Attribute::Bold);

        assert_eq!(style.foreground_color, Some(Color::Blue));
        assert_eq!(style.background_color, Some(Color::Red));
        assert!(style.attributes.has(Attribute::Bold));

        let mut styled_content = style.apply("test");

        styled_content = styled_content
            .with(Color::Green)
            .on(Color::Magenta)
            .attribute(Attribute::NoItalic);

        let style = styled_content.style();

        assert_eq!(style.foreground_color, Some(Color::Green));
        assert_eq!(style.background_color, Some(Color::Magenta));
        assert!(style.attributes.has(Attribute::Bold));
        assert!(style.attributes.has(Attribute::NoItalic));
    }
}
