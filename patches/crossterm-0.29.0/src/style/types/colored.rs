use parking_lot::Once;
use std::fmt::{self, Formatter};
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::style::{parse_next_u8, Color};

/// Represents a foreground or background color.
///
/// This can be converted to a [Colors](struct.Colors.html) by calling `into()` and applied
/// using the [SetColors](struct.SetColors.html) command.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub enum Colored {
    /// A foreground color.
    ForegroundColor(Color),
    /// A background color.
    BackgroundColor(Color),
    /// An underline color.
    /// Important: doesn't work on windows 10 or lower.
    UnderlineColor(Color),
}

static ANSI_COLOR_DISABLED: AtomicBool = AtomicBool::new(false);
static INITIALIZER: Once = Once::new();

impl Colored {
    /// Parse an ANSI foreground or background color.
    /// This is the string that would appear within an `ESC [ <str> m` escape sequence, as found in
    /// various configuration files.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossterm::style::{Colored::{self, ForegroundColor, BackgroundColor}, Color};
    ///
    /// assert_eq!(Colored::parse_ansi("38;5;0"), Some(ForegroundColor(Color::Black)));
    /// assert_eq!(Colored::parse_ansi("38;5;26"), Some(ForegroundColor(Color::AnsiValue(26))));
    /// assert_eq!(Colored::parse_ansi("48;2;50;60;70"), Some(BackgroundColor(Color::Rgb { r: 50, g: 60, b: 70 })));
    /// assert_eq!(Colored::parse_ansi("49"), Some(BackgroundColor(Color::Reset)));
    /// assert_eq!(Colored::parse_ansi("invalid color"), None);
    /// ```
    ///
    /// Currently, 3/4 bit color values aren't supported so return `None`.
    ///
    /// See also: [`Color::parse_ansi`].
    pub fn parse_ansi(ansi: &str) -> Option<Self> {
        use Colored::{BackgroundColor, ForegroundColor, UnderlineColor};

        let values = &mut ansi.split(';');

        let output = match parse_next_u8(values)? {
            38 => return Color::parse_ansi_iter(values).map(ForegroundColor),
            48 => return Color::parse_ansi_iter(values).map(BackgroundColor),
            58 => return Color::parse_ansi_iter(values).map(UnderlineColor),

            39 => ForegroundColor(Color::Reset),
            49 => BackgroundColor(Color::Reset),
            59 => UnderlineColor(Color::Reset),

            _ => return None,
        };

        if values.next().is_some() {
            return None;
        }

        Some(output)
    }

    /// Checks whether ansi color sequences are disabled by setting of NO_COLOR
    /// in environment as per <https://no-color.org/>
    pub fn ansi_color_disabled() -> bool {
        !std::env::var("NO_COLOR")
            .unwrap_or("".to_string())
            .is_empty()
    }

    pub fn ansi_color_disabled_memoized() -> bool {
        INITIALIZER.call_once(|| {
            ANSI_COLOR_DISABLED.store(Self::ansi_color_disabled(), Ordering::SeqCst);
        });

        ANSI_COLOR_DISABLED.load(Ordering::SeqCst)
    }

    pub fn set_ansi_color_disabled(val: bool) {
        // Force the one-time initializer to run.
        _ = Self::ansi_color_disabled_memoized();
        ANSI_COLOR_DISABLED.store(val, Ordering::SeqCst);
    }
}

impl fmt::Display for Colored {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let color;

        if Self::ansi_color_disabled_memoized() {
            return Ok(());
        }

        match *self {
            Colored::ForegroundColor(new_color) => {
                if new_color == Color::Reset {
                    return f.write_str("39");
                } else {
                    f.write_str("38;")?;
                    color = new_color;
                }
            }
            Colored::BackgroundColor(new_color) => {
                if new_color == Color::Reset {
                    return f.write_str("49");
                } else {
                    f.write_str("48;")?;
                    color = new_color;
                }
            }
            Colored::UnderlineColor(new_color) => {
                if new_color == Color::Reset {
                    return f.write_str("59");
                } else {
                    f.write_str("58;")?;
                    color = new_color;
                }
            }
        }

        match color {
            Color::Black => f.write_str("5;0"),
            Color::DarkGrey => f.write_str("5;8"),
            Color::Red => f.write_str("5;9"),
            Color::DarkRed => f.write_str("5;1"),
            Color::Green => f.write_str("5;10"),
            Color::DarkGreen => f.write_str("5;2"),
            Color::Yellow => f.write_str("5;11"),
            Color::DarkYellow => f.write_str("5;3"),
            Color::Blue => f.write_str("5;12"),
            Color::DarkBlue => f.write_str("5;4"),
            Color::Magenta => f.write_str("5;13"),
            Color::DarkMagenta => f.write_str("5;5"),
            Color::Cyan => f.write_str("5;14"),
            Color::DarkCyan => f.write_str("5;6"),
            Color::White => f.write_str("5;15"),
            Color::Grey => f.write_str("5;7"),
            Color::Rgb { r, g, b } => write!(f, "2;{r};{g};{b}"),
            Color::AnsiValue(val) => write!(f, "5;{val}"),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::style::{Color, Colored};

    fn check_format_color(colored: Colored, expected: &str) {
        Colored::set_ansi_color_disabled(true);
        assert_eq!(colored.to_string(), "");
        Colored::set_ansi_color_disabled(false);
        assert_eq!(colored.to_string(), expected);
    }

    #[test]
    fn test_format_fg_color() {
        let colored = Colored::ForegroundColor(Color::Red);
        check_format_color(colored, "38;5;9");
    }

    #[test]
    fn test_format_bg_color() {
        let colored = Colored::BackgroundColor(Color::Red);
        check_format_color(colored, "48;5;9");
    }

    #[test]
    fn test_format_reset_fg_color() {
        let colored = Colored::ForegroundColor(Color::Reset);
        check_format_color(colored, "39");
    }

    #[test]
    fn test_format_reset_bg_color() {
        let colored = Colored::BackgroundColor(Color::Reset);
        check_format_color(colored, "49");
    }

    #[test]
    fn test_format_fg_rgb_color() {
        let colored = Colored::BackgroundColor(Color::Rgb { r: 1, g: 2, b: 3 });
        check_format_color(colored, "48;2;1;2;3");
    }

    #[test]
    fn test_format_fg_ansi_color() {
        let colored = Colored::ForegroundColor(Color::AnsiValue(255));
        check_format_color(colored, "38;5;255");
    }

    #[test]
    fn test_parse_ansi_fg() {
        test_parse_ansi(Colored::ForegroundColor)
    }

    #[test]
    fn test_parse_ansi_bg() {
        test_parse_ansi(Colored::ForegroundColor)
    }

    /// Used for test_parse_ansi_fg and test_parse_ansi_bg
    fn test_parse_ansi(bg_or_fg: impl Fn(Color) -> Colored) {
        /// Formats a re-parses `color` to check the result.
        macro_rules! test {
            ($color:expr) => {
                let colored = bg_or_fg($color);
                assert_eq!(Colored::parse_ansi(&format!("{}", colored)), Some(colored));
            };
        }

        use Color::*;

        test!(Reset);
        test!(Black);
        test!(DarkGrey);
        test!(Red);
        test!(DarkRed);
        test!(Green);
        test!(DarkGreen);
        test!(Yellow);
        test!(DarkYellow);
        test!(Blue);
        test!(DarkBlue);
        test!(Magenta);
        test!(DarkMagenta);
        test!(Cyan);
        test!(DarkCyan);
        test!(White);
        test!(Grey);

        // n in 0..=15 will give us the color values above back.
        for n in 16..=255 {
            test!(AnsiValue(n));
        }

        for r in 0..=255 {
            for g in [0, 2, 18, 19, 60, 100, 200, 250, 254, 255].iter().copied() {
                for b in [0, 12, 16, 99, 100, 161, 200, 255].iter().copied() {
                    test!(Rgb { r, g, b });
                }
            }
        }
    }

    #[test]
    fn test_parse_invalid_ansi_color() {
        /// Checks that trying to parse `s` yields None.
        fn test(s: &str) {
            assert_eq!(Colored::parse_ansi(s), None);
        }
        test("");
        test(";");
        test(";;");
        test(";;");
        test("0");
        test("1");
        test("12");
        test("100");
        test("100048949345");
        test("39;");
        test("49;");
        test("39;2");
        test("49;2");
        test("38");
        test("38;");
        test("38;0");
        test("38;5");
        test("38;5;0;");
        test("38;5;0;2");
        test("38;5;80;");
        test("38;5;80;2");
        test("38;5;257");
        test("38;2");
        test("38;2;");
        test("38;2;0");
        test("38;2;0;2");
        test("38;2;0;2;257");
        test("38;2;0;2;25;");
        test("38;2;0;2;25;3");
        test("48");
        test("48;");
        test("48;0");
        test("48;5");
        test("48;5;0;");
        test("48;5;0;2");
        test("48;5;80;");
        test("48;5;80;2");
        test("48;5;257");
        test("48;2");
        test("48;2;");
        test("48;2;0");
        test("48;2;0;2");
        test("48;2;0;2;257");
        test("48;2;0;2;25;");
        test("48;2;0;2;25;3");
    }

    #[test]
    fn test_no_color() {
        std::env::set_var("NO_COLOR", "1");
        assert!(Colored::ansi_color_disabled());
        std::env::set_var("NO_COLOR", "XXX");
        assert!(Colored::ansi_color_disabled());
        std::env::set_var("NO_COLOR", "");
        assert!(!Colored::ansi_color_disabled());
        std::env::remove_var("NO_COLOR");
        assert!(!Colored::ansi_color_disabled());
    }
}
