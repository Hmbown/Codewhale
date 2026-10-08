//! # Style
//!
//! The `style` module provides a functionality to apply attributes and colors on your text.
//!
//! This documentation does not contain a lot of examples. The reason is that it's fairly
//! obvious how to use this crate. Although, we do provide
//! [examples](https://github.com/crossterm-rs/crossterm/tree/master/examples) repository
//! to demonstrate the capabilities.
//!
//! ## Platform-specific Notes
//!
//! Not all features are supported on all terminals/platforms. You should always consult
//! platform-specific notes of the following types:
//!
//! * [Color](enum.Color.html#platform-specific-notes)
//! * [Attribute](enum.Attribute.html#platform-specific-notes)
//!
//! ## Examples
//!
//! A few examples of how to use the style module.
//!
//! ### Colors
//!
//! How to change the terminal text color.
//!
//! Command API:
//!
//! Using the Command API to color text.
//!
//! ```no_run
//! use std::io::{self, Write};
//! use crossterm::execute;
//! use crossterm::style::{Print, SetForegroundColor, SetBackgroundColor, ResetColor, Color, Attribute};
//!
//! fn main() -> io::Result<()> {
//!     execute!(
//!         io::stdout(),
//!         // Blue foreground
//!         SetForegroundColor(Color::Blue),
//!         // Red background
//!         SetBackgroundColor(Color::Red),
//!         // Print text
//!         Print("Blue text on Red.".to_string()),
//!         // Reset to default colors
//!         ResetColor
//!     )
//! }
//! ```
//!
//! Functions:
//!
//! Using functions from [`Stylize`](crate::style::Stylize) on a `String` or `&'static str` to color
//! it.
//!
//! ```no_run
//! use crossterm::style::Stylize;
//!
//! println!("{}", "Red foreground color & blue background.".red().on_blue());
//! ```
//!
//! ### Attributes
//!
//! How to apply terminal attributes to text.
//!
//! Command API:
//!
//! Using the Command API to set attributes.
//!
//! ```no_run
//! use std::io::{self, Write};
//!
//! use crossterm::execute;
//! use crossterm::style::{Attribute, Print, SetAttribute};
//!
//! fn main() -> io::Result<()> {
//!     execute!(
//!         io::stdout(),
//!         // Set to bold
//!         SetAttribute(Attribute::Bold),
//!         Print("Bold text here.".to_string()),
//!         // Reset all attributes
//!         SetAttribute(Attribute::Reset)
//!     )
//! }
//! ```
//!
//! Functions:
//!
//! Using [`Stylize`](crate::style::Stylize) functions on a `String` or `&'static str` to set
//! attributes to it.
//!
//! ```no_run
//! use crossterm::style::Stylize;
//!
//! println!("{}", "Bold".bold());
//! println!("{}", "Underlined".underlined());
//! println!("{}", "Negative".negative());
//! ```
//!
//! Displayable:
//!
//! [`Attribute`](enum.Attribute.html) implements [Display](https://doc.rust-lang.org/beta/std/fmt/trait.Display.html) and therefore it can be formatted like:
//!
//! ```no_run
//! use crossterm::style::Attribute;
//!
//! println!(
//!     "{} Underlined {} No Underline",
//!     Attribute::Underlined,
//!     Attribute::NoUnderline
//! );
//! ```

use std::{
    env,
    fmt::{self, Display},
};

use crate::command::execute_fmt;
use crate::{csi, impl_display, Command};

pub use self::{
    attributes::Attributes,
    content_style::ContentStyle,
    styled_content::StyledContent,
    stylize::Stylize,
    types::{Attribute, Color, Colored, Colors},
};

mod attributes;
mod content_style;
mod styled_content;
mod stylize;
mod sys;
mod types;

/// Creates a `StyledContent`.
///
/// This could be used to style any type that implements `Display` with colors and text attributes.
///
/// See [`StyledContent`](struct.StyledContent.html) for more info.
///
/// # Examples
///
/// ```no_run
/// use crossterm::style::{style, Stylize, Color};
///
/// let styled_content = style("Blue colored text on yellow background")
///     .with(Color::Blue)
///     .on(Color::Yellow);
///
/// println!("{}", styled_content);
/// ```
pub fn style<D: Display>(val: D) -> StyledContent<D> {
    ContentStyle::new().apply(val)
}

/// Returns available color count.
///
/// # Notes
///
/// This does not always provide a good result.
pub fn available_color_count() -> u16 {
    #[cfg(windows)]
    {
        // Check if we're running in a pseudo TTY, which supports true color.
        // Fall back to env vars otherwise for other terminals on Windows.
        if crate::ansi_support::supports_ansi() {
            return u16::MAX;
        }
    }

    const DEFAULT: u16 = 8;
    env::var("COLORTERM")
        .or_else(|_| env::var("TERM"))
        .map_or(DEFAULT, |x| match x {
            _ if x.contains("24bit") || x.contains("truecolor") => u16::MAX,
            _ if x.contains("256") => 256,
            _ => DEFAULT,
        })
}

/// Forces colored output on or off globally, overriding NO_COLOR.
///
/// # Notes
///
/// crossterm supports NO_COLOR (<https://no-color.org/>) to disabled colored output.
///
/// This API allows applications to override that behavior and force colorized output
/// even if NO_COLOR is set.
pub fn force_color_output(enabled: bool) {
    Colored::set_ansi_color_disabled(!enabled)
}

/// A command that sets the the foreground color.
///
/// See [`Color`](enum.Color.html) for more info.
///
/// [`SetColors`](struct.SetColors.html) can also be used to set both the foreground and background
/// color in one command.
///
/// # Notes
///
/// Commands must be executed/queued for execution otherwise they do nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetForegroundColor(pub Color);

impl Command for SetForegroundColor {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        write!(f, csi!("{}m"), Colored::ForegroundColor(self.0))
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        sys::windows::set_foreground_color(self.0)
    }
}

/// A command that sets the the background color.
///
/// See [`Color`](enum.Color.html) for more info.
///
/// [`SetColors`](struct.SetColors.html) can also be used to set both the foreground and background
/// color with one command.
///
/// # Notes
///
/// Commands must be executed/queued for execution otherwise they do nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetBackgroundColor(pub Color);

impl Command for SetBackgroundColor {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        write!(f, csi!("{}m"), Colored::BackgroundColor(self.0))
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        sys::windows::set_background_color(self.0)
    }
}

/// A command that sets the the underline color.
///
/// See [`Color`](enum.Color.html) for more info.
///
/// [`SetColors`](struct.SetColors.html) can also be used to set both the foreground and background
/// color with one command.
///
/// # Notes
///
/// Commands must be executed/queued for execution otherwise they do nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetUnderlineColor(pub Color);

impl Command for SetUnderlineColor {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        write!(f, csi!("{}m"), Colored::UnderlineColor(self.0))
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "SetUnderlineColor not supported by winapi.",
        ))
    }
}

/// A command that optionally sets the foreground and/or background color.
///
/// For example:
/// ```no_run
/// use std::io::{stdout, Write};
///
/// use crossterm::execute;
/// use crossterm::style::{Color::{Green, Black}, Colors, Print, SetColors};
///
/// execute!(
///     stdout(),
///     SetColors(Colors::new(Green, Black)),
///     Print("Hello, world!".to_string()),
/// ).unwrap();
/// ```
///
/// See [`Colors`](struct.Colors.html) for more info.
///
/// # Notes
///
/// Commands must be executed/queued for execution otherwise they do nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetColors(pub Colors);

impl Command for SetColors {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        // Writing both foreground and background colors in one command resulted in about 20% more
        // FPS (20 to 24 fps) on a fullscreen (171x51) app that writes every cell with a different
        // foreground and background color, compared to separately using the SetForegroundColor and
        // SetBackgroundColor commands (iTerm2, M2 Macbook Pro). `Esc[38;5;<fg>mEsc[48;5;<bg>m` (16
        // chars) vs `Esc[38;5;<fg>;48;5;<bg>m` (14 chars)
        match (self.0.foreground, self.0.background) {
            (Some(fg), Some(bg)) => {
                write!(
                    f,
                    csi!("{};{}m"),
                    Colored::ForegroundColor(fg),
                    Colored::BackgroundColor(bg)
                )
            }
            (Some(fg), None) => write!(f, csi!("{}m"), Colored::ForegroundColor(fg)),
            (None, Some(bg)) => write!(f, csi!("{}m"), Colored::BackgroundColor(bg)),
            (None, None) => Ok(()),
        }
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        if let Some(color) = self.0.foreground {
            sys::windows::set_foreground_color(color)?;
        }
        if let Some(color) = self.0.background {
            sys::windows::set_background_color(color)?;
        }
        Ok(())
    }
}

/// A command that sets an attribute.
///
/// See [`Attribute`](enum.Attribute.html) for more info.
///
/// # Notes
///
/// Commands must be executed/queued for execution otherwise they do nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetAttribute(pub Attribute);

impl Command for SetAttribute {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        write!(f, csi!("{}m"), self.0.sgr())
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        // attributes are not supported by WinAPI.
        Ok(())
    }
}

/// A command that sets several attributes.
///
/// See [`Attributes`](struct.Attributes.html) for more info.
///
/// # Notes
///
/// Commands must be executed/queued for execution otherwise they do nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetAttributes(pub Attributes);

impl Command for SetAttributes {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        for attr in Attribute::iterator() {
            if self.0.has(attr) {
                SetAttribute(attr).write_ansi(f)?;
            }
        }
        Ok(())
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        // attributes are not supported by WinAPI.
        Ok(())
    }
}

/// A command that sets a style (colors and attributes).
///
/// # Notes
///
/// Commands must be executed/queued for execution otherwise they do nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetStyle(pub ContentStyle);

impl Command for SetStyle {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        if let Some(bg) = self.0.background_color {
            execute_fmt(f, SetBackgroundColor(bg)).map_err(|_| fmt::Error)?;
        }
        if let Some(fg) = self.0.foreground_color {
            execute_fmt(f, SetForegroundColor(fg)).map_err(|_| fmt::Error)?;
        }
        if let Some(ul) = self.0.underline_color {
            execute_fmt(f, SetUnderlineColor(ul)).map_err(|_| fmt::Error)?;
        }
        if !self.0.attributes.is_empty() {
            execute_fmt(f, SetAttributes(self.0.attributes)).map_err(|_| fmt::Error)?;
        }

        Ok(())
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        panic!("tried to execute SetStyle command using WinAPI, use ANSI instead");
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }
}

/// A command that prints styled content.
///
/// See [`StyledContent`](struct.StyledContent.html) for more info.
///
/// # Notes
///
/// Commands must be executed/queued for execution otherwise they do nothing.
#[derive(Debug, Copy, Clone)]
pub struct PrintStyledContent<D: Display>(pub StyledContent<D>);

impl<D: Display> Command for PrintStyledContent<D> {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        let style = self.0.style();

        let mut reset_background = false;
        let mut reset_foreground = false;
        let mut reset = false;

        if let Some(bg) = style.background_color {
            execute_fmt(f, SetBackgroundColor(bg)).map_err(|_| fmt::Error)?;
            reset_background = true;
        }
        if let Some(fg) = style.foreground_color {
            execute_fmt(f, SetForegroundColor(fg)).map_err(|_| fmt::Error)?;
            reset_foreground = true;
        }
        if let Some(ul) = style.underline_color {
            execute_fmt(f, SetUnderlineColor(ul)).map_err(|_| fmt::Error)?;
            reset_foreground = true;
        }

        if !style.attributes.is_empty() {
            execute_fmt(f, SetAttributes(style.attributes)).map_err(|_| fmt::Error)?;
            reset = true;
        }

        write!(f, "{}", self.0.content())?;

        if reset {
            // NOTE: This will reset colors even though self has no colors, hence produce unexpected
            // resets.
            // TODO: reset the set attributes only.
            execute_fmt(f, ResetColor).map_err(|_| fmt::Error)?;
        } else {
            // NOTE: Since the above bug, we do not need to reset colors when we reset attributes.
            if reset_background {
                execute_fmt(f, SetBackgroundColor(Color::Reset)).map_err(|_| fmt::Error)?;
            }
            if reset_foreground {
                execute_fmt(f, SetForegroundColor(Color::Reset)).map_err(|_| fmt::Error)?;
            }
        }

        Ok(())
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        Ok(())
    }
}

/// A command that resets the colors back to default.
///
/// # Notes
///
/// Commands must be executed/queued for execution otherwise they do nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResetColor;

impl Command for ResetColor {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        f.write_str(csi!("0m"))
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        sys::windows::reset()
    }
}

/// A command that prints the given displayable type.
///
/// Commands must be executed/queued for execution otherwise they do nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Print<T: Display>(pub T);

impl<T: Display> Command for Print<T> {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        write!(f, "{}", self.0)
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        panic!("tried to execute Print command using WinAPI, use ANSI instead");
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }
}

impl<T: Display> Display for Print<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl_display!(for SetForegroundColor);
impl_display!(for SetBackgroundColor);
impl_display!(for SetColors);
impl_display!(for SetAttribute);
impl_display!(for PrintStyledContent<String>);
impl_display!(for PrintStyledContent<&'static str>);
impl_display!(for ResetColor);

/// Utility function for ANSI parsing in Color and Colored.
/// Gets the next element of `iter` and tries to parse it as a `u8`.
fn parse_next_u8<'a>(iter: &mut impl Iterator<Item = &'a str>) -> Option<u8> {
    iter.next().and_then(|s| s.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    // On Windows many env var tests will fail so we need to conditionally check for ANSI support.
    // This allows other terminals on Windows to still assert env var support.
    macro_rules! skip_windows_ansi_supported {
        () => {
            #[cfg(windows)]
            {
                if crate::ansi_support::supports_ansi() {
                    return;
                }
            }
        };
    }

    #[cfg_attr(windows, test)]
    #[cfg(windows)]
    fn windows_always_truecolor() {
        // This should always be true on supported Windows 10+,
        // but downlevel Windows clients and other terminals may fail `cargo test` otherwise.
        if crate::ansi_support::supports_ansi() {
            assert_eq!(u16::MAX, available_color_count());
        };
    }

    #[test]
    fn colorterm_overrides_term() {
        skip_windows_ansi_supported!();
        temp_env::with_vars(
            [
                ("COLORTERM", Some("truecolor")),
                ("TERM", Some("xterm-256color")),
            ],
            || {
                assert_eq!(u16::MAX, available_color_count());
            },
        );
    }

    #[test]
    fn term_24bits() {
        skip_windows_ansi_supported!();
        temp_env::with_vars(
            [("COLORTERM", None), ("TERM", Some("xterm-24bits"))],
            || {
                assert_eq!(u16::MAX, available_color_count());
            },
        );
    }

    #[test]
    fn term_256color() {
        skip_windows_ansi_supported!();
        temp_env::with_vars(
            [("COLORTERM", None), ("TERM", Some("xterm-256color"))],
            || {
                assert_eq!(256u16, available_color_count());
            },
        );
    }

    #[test]
    fn default_color_count() {
        skip_windows_ansi_supported!();
        temp_env::with_vars([("COLORTERM", None::<&str>), ("TERM", None)], || {
            assert_eq!(8, available_color_count());
        });
    }

    #[test]
    fn unsupported_term_colorterm_values() {
        skip_windows_ansi_supported!();
        temp_env::with_vars(
            [
                ("COLORTERM", Some("gibberish")),
                ("TERM", Some("gibberish")),
            ],
            || {
                assert_eq!(8u16, available_color_count());
            },
        );
    }
}
