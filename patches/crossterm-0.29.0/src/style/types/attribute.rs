use std::fmt::Display;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use super::super::SetAttribute;

// This macro generates the Attribute enum, its iterator
// function, and the static array containing the sgr code
// of each attribute
macro_rules! Attribute {
    (
        $(
            $(#[$inner:ident $($args:tt)*])*
            $name:ident = $sgr:expr,
        )*
    ) => {
        /// Represents an attribute.
        ///
        /// # Platform-specific Notes
        ///
        /// * Only UNIX and Windows 10 terminals do support text attributes.
        /// * Keep in mind that not all terminals support all attributes.
        /// * Crossterm implements almost all attributes listed in the
        ///   [SGR parameters](https://en.wikipedia.org/wiki/ANSI_escape_code#Select_Graphic_Rendition_parameters).
        ///
        /// | Attribute | Windows | UNIX | Notes |
        /// | :-- | :--: | :--: | :-- |
        /// | `Reset` | ✓ | ✓ | |
        /// | `Bold` | ✓ | ✓ | |
        /// | `Dim` | ✓ | ✓ | |
        /// | `Italic` | ? | ? | Not widely supported, sometimes treated as inverse. |
        /// | `Underlined` | ✓ | ✓ | |
        /// | `SlowBlink` | ? | ? | Not widely supported, sometimes treated as inverse. |
        /// | `RapidBlink` | ? | ? | Not widely supported. MS-DOS ANSI.SYS; 150+ per minute. |
        /// | `Reverse` | ✓ | ✓ | |
        /// | `Hidden` | ✓ | ✓ | Also known as Conceal. |
        /// | `Fraktur` | ✗ | ✓ | Legible characters, but marked for deletion. |
        /// | `DefaultForegroundColor` | ? | ? | Implementation specific (according to standard). |
        /// | `DefaultBackgroundColor` | ? | ? | Implementation specific (according to standard). |
        /// | `Framed` | ? | ? | Not widely supported. |
        /// | `Encircled` | ? | ? | This should turn on the encircled attribute. |
        /// | `OverLined` | ? | ? | This should draw a line at the top of the text. |
        ///
        /// # Examples
        ///
        /// Basic usage:
        ///
        /// ```no_run
        /// use crossterm::style::Attribute;
        ///
        /// println!(
        ///     "{} Underlined {} No Underline",
        ///     Attribute::Underlined,
        ///     Attribute::NoUnderline
        /// );
        /// ```
        ///
        /// Style existing text:
        ///
        /// ```no_run
        /// use crossterm::style::Stylize;
        ///
        /// println!("{}", "Bold text".bold());
        /// println!("{}", "Underlined text".underlined());
        /// println!("{}", "Negative text".negative());
        /// ```
        #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
        #[derive(Copy, Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Hash)]
        #[non_exhaustive]
        pub enum Attribute {
            $(
                $(#[$inner $($args)*])*
                $name,
            )*
        }

        pub static SGR: &'static[i16] = &[
            $($sgr,)*
        ];

        impl Attribute {
            /// Iterates over all the variants of the Attribute enum.
            pub fn iterator() -> impl Iterator<Item = Attribute> {
                use self::Attribute::*;
                [ $($name,)* ].iter().copied()
            }
        }
    }
}

Attribute! {
    /// Resets all the attributes.
    Reset = 0,
    /// Increases the text intensity.
    Bold = 1,
    /// Decreases the text intensity.
    Dim = 2,
    /// Emphasises the text.
    Italic = 3,
    /// Underlines the text.
    Underlined = 4,

    // Other types of underlining
    /// Double underlines the text.
    DoubleUnderlined = 2,
    /// Undercurls the text.
    Undercurled = 3,
    /// Underdots the text.
    Underdotted = 4,
    /// Underdashes the text.
    Underdashed = 5,

    /// Makes the text blinking (< 150 per minute).
    SlowBlink = 5,
    /// Makes the text blinking (>= 150 per minute).
    RapidBlink = 6,
    /// Swaps foreground and background colors.
    Reverse = 7,
    /// Hides the text (also known as Conceal).
    Hidden = 8,
    /// Crosses the text.
    CrossedOut = 9,
    /// Sets the [Fraktur](https://en.wikipedia.org/wiki/Fraktur) typeface.
    ///
    /// Mostly used for [mathematical alphanumeric symbols](https://en.wikipedia.org/wiki/Mathematical_Alphanumeric_Symbols).
    Fraktur = 20,
    /// Turns off the `Bold` attribute. - Inconsistent - Prefer to use NormalIntensity
    NoBold = 21,
    /// Switches the text back to normal intensity (no bold, italic).
    NormalIntensity = 22,
    /// Turns off the `Italic` attribute.
    NoItalic = 23,
    /// Turns off the `Underlined` attribute.
    NoUnderline = 24,
    /// Turns off the text blinking (`SlowBlink` or `RapidBlink`).
    NoBlink = 25,
    /// Turns off the `Reverse` attribute.
    NoReverse = 27,
    /// Turns off the `Hidden` attribute.
    NoHidden = 28,
    /// Turns off the `CrossedOut` attribute.
    NotCrossedOut = 29,
    /// Makes the text framed.
    Framed = 51,
    /// Makes the text encircled.
    Encircled = 52,
    /// Draws a line at the top of the text.
    OverLined = 53,
    /// Turns off the `Frame` and `Encircled` attributes.
    NotFramedOrEncircled = 54,
    /// Turns off the `OverLined` attribute.
    NotOverLined = 55,
}

impl Display for Attribute {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", SetAttribute(*self))?;
        Ok(())
    }
}

impl Attribute {
    /// Returns a u32 with one bit set, which is the
    /// signature of this attribute in the Attributes
    /// bitset.
    ///
    /// The +1 enables storing Reset (whose index is 0)
    ///  in the bitset Attributes.
    #[inline(always)]
    pub const fn bytes(self) -> u32 {
        1 << ((self as u32) + 1)
    }
    /// Returns the SGR attribute value.
    ///
    /// See <https://en.wikipedia.org/wiki/ANSI_escape_code#SGR_parameters>
    pub fn sgr(self) -> String {
        if (self as usize) > 4 && (self as usize) < 9 {
            return "4:".to_string() + SGR[self as usize].to_string().as_str();
        }
        SGR[self as usize].to_string()
    }
}
