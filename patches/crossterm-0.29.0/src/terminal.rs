//! # Terminal
//!
//! The `terminal` module provides functionality to work with the terminal.
//!
//! This documentation does not contain a lot of examples. The reason is that it's fairly
//! obvious how to use this crate. Although, we do provide
//! [examples](https://github.com/crossterm-rs/crossterm/tree/master/examples) repository
//! to demonstrate the capabilities.
//!
//! Most terminal actions can be performed with commands.
//! Please have a look at [command documentation](../index.html#command-api) for a more detailed documentation.
//!
//! ## Screen Buffer
//!
//! A screen buffer is a two-dimensional array of character
//! and color data which is displayed in a terminal screen.
//!
//! The terminal has several of those buffers and is able to switch between them.
//! The default screen in which you work is called the 'main screen'.
//! The other screens are called the 'alternative screen'.
//!
//! It is important to understand that crossterm does not yet support creating screens,
//! or switch between more than two buffers, and only offers the ability to change
//! between the 'alternate' and 'main screen'.
//!
//! ### Alternate Screen
//!
//! By default, you will be working on the main screen.
//! There is also another screen called the 'alternative' screen.
//! This screen is slightly different from the main screen.
//! For example, it has the exact dimensions of the terminal window,
//! without any scroll-back area.
//!
//! Crossterm offers the possibility to switch to the 'alternative' screen,
//! make some modifications, and move back to the 'main' screen again.
//! The main screen will stay intact and will have the original data as we performed all
//! operations on the alternative screen.
//!
//! An good example of this is Vim.
//! When it is launched from bash, a whole new buffer is used to modify a file.
//! Then, when the modification is finished, it closes again and continues on the main screen.
//!
//! ### Raw Mode
//!
//! By default, the terminal functions in a certain way.
//! For example, it will move the cursor to the beginning of the next line when the input hits the end of a line.
//! Or that the backspace is interpreted for character removal.
//!
//! Sometimes these default modes are irrelevant,
//! and in this case, we can turn them off.
//! This is what happens when you enable raw modes.
//!
//! Those modes will be set when enabling raw modes:
//!
//! - Input will not be forwarded to screen
//! - Input will not be processed on enter press
//! - Input will not be line buffered (input sent byte-by-byte to input buffer)
//! - Special keys like backspace and CTRL+C will not be processed by terminal driver
//! - New line character will not be processed therefore `println!` can't be used, use `write!` instead
//!
//! Raw mode can be enabled/disabled with the [enable_raw_mode](terminal::enable_raw_mode) and [disable_raw_mode](terminal::disable_raw_mode) functions.
//!
//! ## Examples
//!
//! ```no_run
//! use std::io::{self, Write};
//! use crossterm::{execute, terminal::{ScrollUp, SetSize, size}};
//!
//! fn main() -> io::Result<()> {
//!     let (cols, rows) = size()?;
//!     // Resize terminal and scroll up.
//!     execute!(
//!         io::stdout(),
//!         SetSize(10, 10),
//!         ScrollUp(5)
//!     )?;
//!
//!     // Be a good citizen, cleanup
//!     execute!(io::stdout(), SetSize(cols, rows))?;
//!     Ok(())
//! }
//! ```
//!
//! For manual execution control check out [crossterm::queue](../macro.queue.html).

use std::{fmt, io};

#[cfg(windows)]
use crossterm_winapi::{ConsoleMode, Handle, ScreenBuffer};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
#[cfg(windows)]
use winapi::um::wincon::ENABLE_WRAP_AT_EOL_OUTPUT;

#[doc(no_inline)]
use crate::Command;
use crate::{csi, impl_display};

pub(crate) mod sys;

#[cfg(feature = "events")]
pub use sys::supports_keyboard_enhancement;

/// Tells whether the raw mode is enabled.
///
/// Please have a look at the [raw mode](./index.html#raw-mode) section.
pub fn is_raw_mode_enabled() -> io::Result<bool> {
    #[cfg(unix)]
    {
        Ok(sys::is_raw_mode_enabled())
    }

    #[cfg(windows)]
    {
        sys::is_raw_mode_enabled()
    }
}

/// Enables raw mode.
///
/// Please have a look at the [raw mode](./index.html#raw-mode) section.
pub fn enable_raw_mode() -> io::Result<()> {
    sys::enable_raw_mode()
}

/// Disables raw mode.
///
/// Please have a look at the [raw mode](./index.html#raw-mode) section.
pub fn disable_raw_mode() -> io::Result<()> {
    sys::disable_raw_mode()
}

/// Returns the terminal size `(columns, rows)`.
///
/// The top left cell is represented `(1, 1)`.
pub fn size() -> io::Result<(u16, u16)> {
    sys::size()
}

#[derive(Debug)]
pub struct WindowSize {
    pub rows: u16,
    pub columns: u16,
    pub width: u16,
    pub height: u16,
}

/// Returns the terminal size `[WindowSize]`.
///
/// The width and height in pixels may not be reliably implemented or default to 0.
/// For unix, <https://man7.org/linux/man-pages/man4/tty_ioctl.4.html> documents them as "unused".
/// For windows it is not implemented.
pub fn window_size() -> io::Result<WindowSize> {
    sys::window_size()
}

/// Disables line wrapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisableLineWrap;

impl Command for DisableLineWrap {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        f.write_str(csi!("?7l"))
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        let screen_buffer = ScreenBuffer::current()?;
        let console_mode = ConsoleMode::from(screen_buffer.handle().clone());
        let new_mode = console_mode.mode()? & !ENABLE_WRAP_AT_EOL_OUTPUT;
        console_mode.set_mode(new_mode)?;
        Ok(())
    }
}

/// Enable line wrapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnableLineWrap;

impl Command for EnableLineWrap {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        f.write_str(csi!("?7h"))
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        let screen_buffer = ScreenBuffer::current()?;
        let console_mode = ConsoleMode::from(screen_buffer.handle().clone());
        let new_mode = console_mode.mode()? | ENABLE_WRAP_AT_EOL_OUTPUT;
        console_mode.set_mode(new_mode)?;
        Ok(())
    }
}

/// A command that switches to alternate screen.
///
/// # Notes
///
/// * Commands must be executed/queued for execution otherwise they do nothing.
/// * Use [LeaveAlternateScreen](./struct.LeaveAlternateScreen.html) command to leave the entered alternate screen.
///
/// # Examples
///
/// ```no_run
/// use std::io::{self, Write};
/// use crossterm::{execute, terminal::{EnterAlternateScreen, LeaveAlternateScreen}};
///
/// fn main() -> io::Result<()> {
///     execute!(io::stdout(), EnterAlternateScreen)?;
///
///     // Do anything on the alternate screen
///
///     execute!(io::stdout(), LeaveAlternateScreen)
/// }
/// ```
///
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnterAlternateScreen;

impl Command for EnterAlternateScreen {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        f.write_str(csi!("?1049h"))
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        let alternate_screen = ScreenBuffer::create()?;
        alternate_screen.show()?;
        Ok(())
    }
}

/// A command that switches back to the main screen.
///
/// # Notes
///
/// * Commands must be executed/queued for execution otherwise they do nothing.
/// * Use [EnterAlternateScreen](./struct.EnterAlternateScreen.html) to enter the alternate screen.
///
/// # Examples
///
/// ```no_run
/// use std::io::{self, Write};
/// use crossterm::{execute, terminal::{EnterAlternateScreen, LeaveAlternateScreen}};
///
/// fn main() -> io::Result<()> {
///     execute!(io::stdout(), EnterAlternateScreen)?;
///
///     // Do anything on the alternate screen
///
///     execute!(io::stdout(), LeaveAlternateScreen)
/// }
/// ```
///
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LeaveAlternateScreen;

impl Command for LeaveAlternateScreen {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        f.write_str(csi!("?1049l"))
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        let screen_buffer = ScreenBuffer::from(Handle::current_out_handle()?);
        screen_buffer.show()?;
        Ok(())
    }
}

/// Different ways to clear the terminal buffer.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub enum ClearType {
    /// All cells.
    All,
    /// All plus history
    Purge,
    /// All cells from the cursor position downwards.
    FromCursorDown,
    /// All cells from the cursor position upwards.
    FromCursorUp,
    /// All cells at the cursor row.
    CurrentLine,
    /// All cells from the cursor position until the new line.
    UntilNewLine,
}

/// A command that scrolls the terminal screen a given number of rows up.
///
/// # Notes
///
/// Commands must be executed/queued for execution otherwise they do nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollUp(pub u16);

impl Command for ScrollUp {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        if self.0 != 0 {
            write!(f, csi!("{}S"), self.0)?;
        }
        Ok(())
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        sys::scroll_up(self.0)
    }
}

/// A command that scrolls the terminal screen a given number of rows down.
///
/// # Notes
///
/// Commands must be executed/queued for execution otherwise they do nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollDown(pub u16);

impl Command for ScrollDown {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        if self.0 != 0 {
            write!(f, csi!("{}T"), self.0)?;
        }
        Ok(())
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        sys::scroll_down(self.0)
    }
}

/// A command that clears the terminal screen buffer.
///
/// See the [`ClearType`](enum.ClearType.html) enum.
///
/// # Notes
///
/// Commands must be executed/queued for execution otherwise they do nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clear(pub ClearType);

impl Command for Clear {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        f.write_str(match self.0 {
            ClearType::All => csi!("2J"),
            ClearType::Purge => csi!("3J"),
            ClearType::FromCursorDown => csi!("J"),
            ClearType::FromCursorUp => csi!("1J"),
            ClearType::CurrentLine => csi!("2K"),
            ClearType::UntilNewLine => csi!("K"),
        })
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        sys::clear(self.0)
    }
}

/// A command that sets the terminal buffer size `(columns, rows)`.
///
/// # Notes
///
/// Commands must be executed/queued for execution otherwise they do nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetSize(pub u16, pub u16);

impl Command for SetSize {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        write!(f, csi!("8;{};{}t"), self.1, self.0)
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        sys::set_size(self.0, self.1)
    }
}

/// A command that sets the terminal title
///
/// # Notes
///
/// Commands must be executed/queued for execution otherwise they do nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetTitle<T>(pub T);

impl<T: fmt::Display> Command for SetTitle<T> {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        write!(f, "\x1B]0;{}\x07", &self.0)
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        sys::set_window_title(&self.0)
    }
}

/// A command that instructs the terminal emulator to begin a synchronized frame.
///
/// # Notes
///
/// * Commands must be executed/queued for execution otherwise they do nothing.
/// * Use [EndSynchronizedUpdate](./struct.EndSynchronizedUpdate.html) command to leave the entered alternate screen.
///
/// When rendering the screen of the terminal, the Emulator usually iterates through each visible grid cell and
/// renders its current state. With applications updating the screen at a higher frequency this can cause tearing.
///
/// This mode attempts to mitigate that.
///
/// When the synchronization mode is enabled following render calls will keep rendering the last rendered state.
/// The terminal Emulator keeps processing incoming text and sequences. When the synchronized update mode is disabled
/// again the renderer may fetch the latest screen buffer state again, effectively avoiding the tearing effect
/// by unintentionally rendering in the middle a of an application screen update.
///
/// # Examples
///
/// ```no_run
/// use std::io::{self, Write};
/// use crossterm::{execute, terminal::{BeginSynchronizedUpdate, EndSynchronizedUpdate}};
///
/// fn main() -> io::Result<()> {
///     execute!(io::stdout(), BeginSynchronizedUpdate)?;
///
///     // Anything performed here will not be rendered until EndSynchronizedUpdate is called.
///
///     execute!(io::stdout(), EndSynchronizedUpdate)?;
///     Ok(())
/// }
/// ```
///
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BeginSynchronizedUpdate;

impl Command for BeginSynchronizedUpdate {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        f.write_str(csi!("?2026h"))
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        Ok(())
    }

    #[cfg(windows)]
    #[inline]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }
}

/// A command that instructs the terminal to end a synchronized frame.
///
/// # Notes
///
/// * Commands must be executed/queued for execution otherwise they do nothing.
/// * Use [BeginSynchronizedUpdate](./struct.BeginSynchronizedUpdate.html) to enter the alternate screen.
///
/// When rendering the screen of the terminal, the Emulator usually iterates through each visible grid cell and
/// renders its current state. With applications updating the screen a at higher frequency this can cause tearing.
///
/// This mode attempts to mitigate that.
///
/// When the synchronization mode is enabled following render calls will keep rendering the last rendered state.
/// The terminal Emulator keeps processing incoming text and sequences. When the synchronized update mode is disabled
/// again the renderer may fetch the latest screen buffer state again, effectively avoiding the tearing effect
/// by unintentionally rendering in the middle a of an application screen update.
///
/// # Examples
///
/// ```no_run
/// use std::io::{self, Write};
/// use crossterm::{execute, terminal::{BeginSynchronizedUpdate, EndSynchronizedUpdate}};
///
/// fn main() -> io::Result<()> {
///     execute!(io::stdout(), BeginSynchronizedUpdate)?;
///
///     // Anything performed here will not be rendered until EndSynchronizedUpdate is called.
///
///     execute!(io::stdout(), EndSynchronizedUpdate)?;
///     Ok(())
/// }
/// ```
///
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndSynchronizedUpdate;

impl Command for EndSynchronizedUpdate {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        f.write_str(csi!("?2026l"))
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        Ok(())
    }

    #[cfg(windows)]
    #[inline]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }
}

impl_display!(for ScrollUp);
impl_display!(for ScrollDown);
impl_display!(for SetSize);
impl_display!(for Clear);

#[cfg(test)]
mod tests {
    use std::{io::stdout, thread, time};

    use crate::execute;

    use super::*;

    // Test is disabled, because it's failing on Travis CI
    #[test]
    #[ignore]
    fn test_resize_ansi() {
        let (width, height) = size().unwrap();

        execute!(stdout(), SetSize(35, 35)).unwrap();

        // see issue: https://github.com/eminence/terminal-size/issues/11
        thread::sleep(time::Duration::from_millis(30));

        assert_eq!((35, 35), size().unwrap());

        // reset to previous size
        execute!(stdout(), SetSize(width, height)).unwrap();

        // see issue: https://github.com/eminence/terminal-size/issues/11
        thread::sleep(time::Duration::from_millis(30));

        assert_eq!((width, height), size().unwrap());
    }

    #[test]
    fn test_raw_mode() {
        // check we start from normal mode (may fail on some test harnesses)
        assert!(!is_raw_mode_enabled().unwrap());

        // enable the raw mode
        if enable_raw_mode().is_err() {
            // Enabling raw mode doesn't work on the ci
            // So we just ignore it
            return;
        }

        // check it worked (on unix it doesn't really check the underlying
        // tty but rather check that the code is consistent)
        assert!(is_raw_mode_enabled().unwrap());

        // enable it again, this should not change anything
        enable_raw_mode().unwrap();

        // check we're still in raw mode
        assert!(is_raw_mode_enabled().unwrap());

        // now let's disable it
        disable_raw_mode().unwrap();

        // check we're back to normal mode
        assert!(!is_raw_mode_enabled().unwrap());
    }
}
