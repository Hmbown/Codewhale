//! # Clipboard
//!
//! The `clipboard` module provides functionality to work with a host clipboard.
//!
//! ## Implemented operations:
//!
//! - Copy: [`CopyToClipboard`](struct.CopyToClipboard.html)
use base64::prelude::{Engine, BASE64_STANDARD};

use std::fmt;
use std::str::FromStr;

use crate::{osc, Command};

/// Different clipboard types
///
/// Some operating systems and desktop environments support multiple buffers
/// for copy/cut/paste. Their details differ between operating systems.
/// See <https://specifications.freedesktop.org/clipboard-spec/latest/>
/// for a detailed survey of supported types based on the X window system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardType {
    /// Default clipboard when using Ctrl+C or Ctrl+V
    Clipboard,

    /// Clipboard on Linux/X/Wayland when using selection and middle mouse button
    Primary,

    /// Other clipboard type not explicitly supported by crossterm
    ///
    /// See
    /// [XTerm Control Sequences](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html#h3-Operating-System-Commands)
    /// for potential values.
    ///
    /// Note that support for these in terminal emulators is very limited.
    Other(char),
}

impl From<&ClipboardType> for char {
    fn from(val: &ClipboardType) -> Self {
        match val {
            ClipboardType::Clipboard => 'c',
            ClipboardType::Primary => 'p',
            ClipboardType::Other(other) => *other,
        }
    }
}

impl From<char> for ClipboardType {
    fn from(value: char) -> Self {
        match value {
            'c' => ClipboardType::Clipboard,
            'p' => ClipboardType::Primary,
            other => ClipboardType::Other(other),
        }
    }
}

/// A sequence of clipboard types
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardSelection(
    /// An ordered list of clipboards which will be the destination for the copied selection.
    ///
    /// Order matters due to implementations deviating from the
    /// [XTerm Control Sequences](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html#h3-Operating-System-Commands)
    /// reference. Some terminal emulators may only interpret the first character of this
    /// parameter. For differences, see
    /// [`CopyToClipboard` (Terminal Support)](struct.CopyToClipboard.html#terminal-support).
    pub Vec<ClipboardType>,
);

impl ClipboardSelection {
    /// Returns a String corresponsing to the "Pc" parameter of the OSC52
    /// sequence.
    fn to_osc52_pc(&self) -> String {
        self.0.iter().map(Into::<char>::into).collect()
    }
}

impl fmt::Display for ClipboardSelection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_osc52_pc())
    }
}

impl FromStr for ClipboardSelection {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(ClipboardSelection(
            s.chars().map(From::<char>::from).collect(),
        ))
    }
}

/// A command that copies to clipboard
///
/// This command uses OSC control sequence `Pr = 5 2` (See
/// [XTerm Control Sequences](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html#h3-Operating-System-Commands) )
/// to copy data to the terminal host clipboard.
///
/// This only works if it is enabled on the user's terminal emulator. If a terminal multiplexer
/// is used, the multiplexer must support it, too.
///
/// Commands must be executed/queued for execution otherwise they do nothing.
///
/// # Examples
///
/// ```no_run
/// use crossterm::execute;
/// use crossterm::clipboard::CopyToClipboard;
/// // Copy foo to clipboard
/// execute!(std::io::stdout(), CopyToClipboard::to_clipboard_from("foo"));
/// // Copy bar to primary
/// execute!(std::io::stdout(), CopyToClipboard::to_primary_from("bar"));
/// ```
///
/// See also examples/copy-to-clipboard.rs.
///
/// # Terminal Support
///
/// The following table shows what destinations are filled by different terminal emulators when
/// asked to copy to different destination sequences.
///
/// | Terminal (Version)    | dest ''   | dest 'c'  | dest 'p' | dest 'cp'     | dest'pc'      |
/// | --------------------- | --------- | --------- | -------- | ------------- | ------------- |
/// | xterm (397) *3        | primary   | clipboard | primary  | clipb., prim. | clipb., prim. |
/// | Alacritty (0.15.1) *3 | clipboard | clipboard | primary  | clipb.        | prim.         |
/// | Wezterm (*1) *3       | clipboard | clipboard | primary  | clipb.        | clipb.        |
/// | Konsole (24.12.3) *3  | clipboard | clipboard | primary  | clipb., prim. | clipb., prim. |
/// | Kitty (0.40.0) *3     | clipboard | clipboard | primary  | clipb.        | clipb.        |
/// | foot (1.20.2) *3      | clipboard | clipboard | primary  | clipb., prim. | clipb., prim. |
/// | tmux (3.5a) *2 *3     | primary   | clipboard | primary  | clipb., prim. | clipb., prim. |
///
/// Asterisks:
/// 1. 20240203-110809-5046fc22
/// 2. set-clipboard set to [external](https://github.com/tmux/tmux/wiki/Clipboard#how-it-works),
///    i.e. this is OSC52 pass-through.
/// 3. This was tested on wayland with the
///    [primary selection protocol](https://wayland.app/protocols/primary-selection-unstable-v1)
///    enabled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyToClipboard<T> {
    /// Content to be copied
    pub content: T,
    /// Sequence of copy destinations
    ///
    /// Not all sequences are equally supported by terminal emulators. See
    /// [`CopyToClipboard` (Terminal Support)](struct.CopyToClipboard.html#terminal-support).
    pub destination: ClipboardSelection,
}

impl<T: AsRef<[u8]>> Command for CopyToClipboard<T> {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        write!(
            f,
            osc!("52;{destination};{encoded_text}"),
            destination = self.destination.to_osc52_pc(),
            encoded_text = BASE64_STANDARD.encode(&self.content)
        )
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        use std::io;

        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Copying is not implemented for the Windows API.",
        ))
    }
}

impl<T: AsRef<[u8]>> CopyToClipboard<T> {
    /// Construct a [`CopyToClipboard`] that writes content into the
    /// "clipboard" (or 'c') clipboard selection.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use crossterm::{execute, Command};
    /// use crossterm::clipboard::CopyToClipboard;
    /// execute!(std::io::stdout(), CopyToClipboard::to_clipboard_from("foo"));
    /// ```
    pub fn to_clipboard_from(content: T) -> CopyToClipboard<T> {
        CopyToClipboard {
            content,
            destination: ClipboardSelection(vec![ClipboardType::Clipboard]),
        }
    }

    /// Construct a [`CopyToClipboard`] that writes content into the "primary"
    /// (or 'p') clipboard selection.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use crossterm::execute;
    /// use crossterm::clipboard::CopyToClipboard;
    /// execute!(std::io::stdout(), CopyToClipboard::to_primary_from("foo"));
    /// ```
    pub fn to_primary_from(content: T) -> CopyToClipboard<T> {
        CopyToClipboard {
            content,
            destination: ClipboardSelection(vec![ClipboardType::Primary]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clipboard_string_to_selection() {
        assert_eq!(
            ClipboardSelection::from_str("p").unwrap(),
            ClipboardSelection(vec![ClipboardType::Primary])
        );
        assert_eq!(
            ClipboardSelection::from_str("").unwrap(),
            ClipboardSelection(vec![])
        );
        assert_eq!(
            ClipboardSelection::from_str("cp").unwrap(),
            ClipboardSelection(vec![ClipboardType::Clipboard, ClipboardType::Primary])
        );
    }
    #[test]
    fn test_clipboard_selection_to_osc52_pc() {
        assert_eq!(ClipboardSelection(vec![]).to_osc52_pc(), "");
        assert_eq!(
            ClipboardSelection(vec![ClipboardType::Clipboard]).to_osc52_pc(),
            "c"
        );
        assert_eq!(
            ClipboardSelection(vec![ClipboardType::Primary]).to_osc52_pc(),
            "p"
        );
        assert_eq!(
            ClipboardSelection(vec![ClipboardType::Primary, ClipboardType::Clipboard])
                .to_osc52_pc(),
            "pc"
        );
        assert_eq!(
            ClipboardSelection(vec![ClipboardType::Clipboard, ClipboardType::Primary])
                .to_osc52_pc(),
            "cp"
        );
        assert_eq!(
            ClipboardSelection(vec![ClipboardType::Other('s')]).to_osc52_pc(),
            "s"
        );
    }

    #[test]
    fn test_clipboard_copy_string_osc52() {
        let mut buffer = String::new();
        super::CopyToClipboard {
            content: "foo",
            destination: ClipboardSelection(vec![ClipboardType::Clipboard]),
        }
        .write_ansi(&mut buffer)
        .unwrap();
        assert_eq!(buffer, "\x1b]52;c;Zm9v\x1b\\");

        buffer.clear();
        super::CopyToClipboard {
            content: "foo",
            destination: ClipboardSelection(vec![ClipboardType::Primary]),
        }
        .write_ansi(&mut buffer)
        .unwrap();
        assert_eq!(buffer, "\x1b]52;p;Zm9v\x1b\\");

        buffer.clear();
        super::CopyToClipboard {
            content: "foo",
            destination: ClipboardSelection(vec![ClipboardType::Primary, ClipboardType::Clipboard]),
        }
        .write_ansi(&mut buffer)
        .unwrap();
        assert_eq!(buffer, "\x1b]52;pc;Zm9v\x1b\\");

        buffer.clear();
        super::CopyToClipboard {
            content: "foo",
            destination: ClipboardSelection(vec![]),
        }
        .write_ansi(&mut buffer)
        .unwrap();
        assert_eq!(buffer, "\x1b]52;;Zm9v\x1b\\");
    }

    #[test]
    fn test_clipboard_copy_string_osc52_constructor() {
        let mut buffer = String::new();
        super::CopyToClipboard::to_clipboard_from("foo")
            .write_ansi(&mut buffer)
            .unwrap();
        assert_eq!(buffer, "\x1b]52;c;Zm9v\x1b\\");

        let mut buffer = String::new();
        super::CopyToClipboard::to_primary_from("foo")
            .write_ansi(&mut buffer)
            .unwrap();
        assert_eq!(buffer, "\x1b]52;p;Zm9v\x1b\\");
    }
}
