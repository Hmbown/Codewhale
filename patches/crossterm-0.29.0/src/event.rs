//! # Event
//!
//! The `event` module provides the functionality to read keyboard, mouse and terminal resize events.
//!
//! * The [`read`](fn.read.html) function returns an [`Event`](enum.Event.html) immediately
//! (if available) or blocks until an [`Event`](enum.Event.html) is available.
//!
//! * The [`poll`](fn.poll.html) function allows you to check if there is or isn't an [`Event`](enum.Event.html) available
//! within the given period of time. In other words - if subsequent call to the [`read`](fn.read.html)
//! function will block or not.
//!
//! It's **not allowed** to call these functions from different threads or combine them with the
//! [`EventStream`](struct.EventStream.html). You're allowed to either:
//!
//! * use the [`read`](fn.read.html) & [`poll`](fn.poll.html) functions on any, but same, thread
//! * or the [`EventStream`](struct.EventStream.html).
//!
//! **Make sure to enable [raw mode](../terminal/index.html#raw-mode) in order for keyboard events to work properly**
//!
//! ## Mouse and Focus Events
//!
//! Mouse and focus events are not enabled by default. You have to enable them with the
//! [`EnableMouseCapture`](struct.EnableMouseCapture.html) / [`EnableFocusChange`](struct.EnableFocusChange.html) command.
//! See [Command API](../index.html#command-api) for more information.
//!
//! ## Examples
//!
//! Blocking read:
//!
//! ```no_run
//! #![cfg(feature = "bracketed-paste")]
//! use crossterm::{
//!     event::{
//!         read, DisableBracketedPaste, DisableFocusChange, DisableMouseCapture, EnableBracketedPaste,
//!         EnableFocusChange, EnableMouseCapture, Event,
//!     },
//!     execute,
//! };
//!
//! fn print_events() -> std::io::Result<()> {
//!     execute!(
//!          std::io::stdout(),
//!          EnableBracketedPaste,
//!          EnableFocusChange,
//!          EnableMouseCapture
//!     )?;
//!     loop {
//!         // `read()` blocks until an `Event` is available
//!         match read()? {
//!             Event::FocusGained => println!("FocusGained"),
//!             Event::FocusLost => println!("FocusLost"),
//!             Event::Key(event) => println!("{:?}", event),
//!             Event::Mouse(event) => println!("{:?}", event),
//!             #[cfg(feature = "bracketed-paste")]
//!             Event::Paste(data) => println!("{:?}", data),
//!             Event::Resize(width, height) => println!("New size {}x{}", width, height),
//!         }
//!     }
//!     execute!(
//!         std::io::stdout(),
//!         DisableBracketedPaste,
//!         DisableFocusChange,
//!         DisableMouseCapture
//!     )?;
//!     Ok(())
//! }
//! ```
//!
//! Non-blocking read:
//!
//! ```no_run
//! #![cfg(feature = "bracketed-paste")]
//! use std::{time::Duration, io};
//!
//! use crossterm::{
//!     event::{
//!         poll, read, DisableBracketedPaste, DisableFocusChange, DisableMouseCapture,
//!         EnableBracketedPaste, EnableFocusChange, EnableMouseCapture, Event,
//!     },
//!     execute,
//! };
//!
//! fn print_events() -> io::Result<()> {
//!     execute!(
//!          std::io::stdout(),
//!          EnableBracketedPaste,
//!          EnableFocusChange,
//!          EnableMouseCapture
//!     )?;
//!     loop {
//!         // `poll()` waits for an `Event` for a given time period
//!         if poll(Duration::from_millis(500))? {
//!             // It's guaranteed that the `read()` won't block when the `poll()`
//!             // function returns `true`
//!             match read()? {
//!                 Event::FocusGained => println!("FocusGained"),
//!                 Event::FocusLost => println!("FocusLost"),
//!                 Event::Key(event) => println!("{:?}", event),
//!                 Event::Mouse(event) => println!("{:?}", event),
//!                 #[cfg(feature = "bracketed-paste")]
//!                 Event::Paste(data) => println!("Pasted {:?}", data),
//!                 Event::Resize(width, height) => println!("New size {}x{}", width, height),
//!             }
//!         } else {
//!             // Timeout expired and no `Event` is available
//!         }
//!     }
//!     execute!(
//!         std::io::stdout(),
//!         DisableBracketedPaste,
//!         DisableFocusChange,
//!         DisableMouseCapture
//!     )?;
//!     Ok(())
//! }
//! ```
//!
//! Check the [examples](https://github.com/crossterm-rs/crossterm/tree/master/examples) folder for more of
//! them (`event-*`).

pub(crate) mod filter;
pub(crate) mod read;
pub(crate) mod source;
#[cfg(feature = "event-stream")]
pub(crate) mod stream;
pub(crate) mod sys;
pub(crate) mod timeout;

#[cfg(feature = "derive-more")]
use derive_more::derive::IsVariant;
#[cfg(feature = "event-stream")]
pub use stream::EventStream;

use crate::event::{
    filter::{EventFilter, Filter},
    read::InternalEventReader,
    timeout::PollTimeout,
};
use crate::{csi, Command};
use parking_lot::{MappedMutexGuard, Mutex, MutexGuard};
use std::fmt::{self, Display};
use std::time::Duration;

use bitflags::bitflags;
use std::hash::{Hash, Hasher};

/// Static instance of `InternalEventReader`.
/// This needs to be static because there can be one event reader.
static INTERNAL_EVENT_READER: Mutex<Option<InternalEventReader>> = parking_lot::const_mutex(None);

pub(crate) fn lock_internal_event_reader() -> MappedMutexGuard<'static, InternalEventReader> {
    MutexGuard::map(INTERNAL_EVENT_READER.lock(), |reader| {
        reader.get_or_insert_with(InternalEventReader::default)
    })
}
fn try_lock_internal_event_reader_for(
    duration: Duration,
) -> Option<MappedMutexGuard<'static, InternalEventReader>> {
    Some(MutexGuard::map(
        INTERNAL_EVENT_READER.try_lock_for(duration)?,
        |reader| reader.get_or_insert_with(InternalEventReader::default),
    ))
}

/// Checks if there is an [`Event`](enum.Event.html) available.
///
/// Returns `Ok(true)` if an [`Event`](enum.Event.html) is available otherwise it returns `Ok(false)`.
///
/// `Ok(true)` guarantees that subsequent call to the [`read`](fn.read.html) function
/// won't block.
///
/// # Arguments
///
/// * `timeout` - maximum waiting time for event availability
///
/// # Examples
///
/// Return immediately:
///
/// ```no_run
/// use std::{time::Duration, io};
/// use crossterm::{event::poll};
///
/// fn is_event_available() -> io::Result<bool> {
///     // Zero duration says that the `poll` function must return immediately
///     // with an `Event` availability information
///     poll(Duration::from_secs(0))
/// }
/// ```
///
/// Wait up to 100ms:
///
/// ```no_run
/// use std::{time::Duration, io};
///
/// use crossterm::event::poll;
///
/// fn is_event_available() -> io::Result<bool> {
///     // Wait for an `Event` availability for 100ms. It returns immediately
///     // if an `Event` is/becomes available.
///     poll(Duration::from_millis(100))
/// }
/// ```
pub fn poll(timeout: Duration) -> std::io::Result<bool> {
    poll_internal(Some(timeout), &EventFilter)
}

/// Reads a single [`Event`](enum.Event.html).
///
/// This function blocks until an [`Event`](enum.Event.html) is available. Combine it with the
/// [`poll`](fn.poll.html) function to get non-blocking reads.
///
/// # Examples
///
/// Blocking read:
///
/// ```no_run
/// use crossterm::event::read;
/// use std::io;
///
/// fn print_events() -> io::Result<bool> {
///     loop {
///         // Blocks until an `Event` is available
///         println!("{:?}", read()?);
///     }
/// }
/// ```
///
/// Non-blocking read:
///
/// ```no_run
/// use std::time::Duration;
/// use std::io;
///
/// use crossterm::event::{read, poll};
///
/// fn print_events() -> io::Result<bool> {
///     loop {
///         if poll(Duration::from_millis(100))? {
///             // It's guaranteed that `read` won't block, because `poll` returned
///             // `Ok(true)`.
///             println!("{:?}", read()?);
///         } else {
///             // Timeout expired, no `Event` is available
///         }
///     }
/// }
/// ```
pub fn read() -> std::io::Result<Event> {
    match read_internal(&EventFilter)? {
        InternalEvent::Event(event) => Ok(event),
        #[cfg(unix)]
        _ => unreachable!(),
    }
}

/// Polls to check if there are any `InternalEvent`s that can be read within the given duration.
pub(crate) fn poll_internal<F>(timeout: Option<Duration>, filter: &F) -> std::io::Result<bool>
where
    F: Filter,
{
    let (mut reader, timeout) = if let Some(timeout) = timeout {
        let poll_timeout = PollTimeout::new(Some(timeout));
        if let Some(reader) = try_lock_internal_event_reader_for(timeout) {
            (reader, poll_timeout.leftover())
        } else {
            return Ok(false);
        }
    } else {
        (lock_internal_event_reader(), None)
    };
    reader.poll(timeout, filter)
}

/// Reads a single `InternalEvent`.
pub(crate) fn read_internal<F>(filter: &F) -> std::io::Result<InternalEvent>
where
    F: Filter,
{
    let mut reader = lock_internal_event_reader();
    reader.read(filter)
}

bitflags! {
    /// Represents special flags that tell compatible terminals to add extra information to keyboard events.
    ///
    /// See <https://sw.kovidgoyal.net/kitty/keyboard-protocol/#progressive-enhancement> for more information.
    ///
    /// Alternate keys and Unicode codepoints are not yet supported by crossterm.
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(transparent))]
    #[derive(Debug, PartialOrd, PartialEq, Eq, Clone, Copy, Hash)]
    pub struct KeyboardEnhancementFlags: u8 {
        /// Represent Escape and modified keys using CSI-u sequences, so they can be unambiguously
        /// read.
        const DISAMBIGUATE_ESCAPE_CODES = 0b0000_0001;
        /// Add extra events with [`KeyEvent.kind`] set to [`KeyEventKind::Repeat`] or
        /// [`KeyEventKind::Release`] when keys are autorepeated or released.
        const REPORT_EVENT_TYPES = 0b0000_0010;
        /// Send [alternate keycodes](https://sw.kovidgoyal.net/kitty/keyboard-protocol/#key-codes)
        /// in addition to the base keycode. The alternate keycode overrides the base keycode in
        /// resulting `KeyEvent`s.
        const REPORT_ALTERNATE_KEYS = 0b0000_0100;
        /// Represent all keyboard events as CSI-u sequences. This is required to get repeat/release
        /// events for plain-text keys.
        const REPORT_ALL_KEYS_AS_ESCAPE_CODES = 0b0000_1000;
        // Send the Unicode codepoint as well as the keycode.
        //
        // *Note*: this is not yet supported by crossterm.
        // const REPORT_ASSOCIATED_TEXT = 0b0001_0000;
    }
}

/// A command that enables mouse event capturing.
///
/// Mouse events can be captured with [read](./fn.read.html)/[poll](./fn.poll.html).
#[cfg(feature = "events")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnableMouseCapture;

#[cfg(feature = "events")]
impl Command for EnableMouseCapture {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        f.write_str(concat!(
            // Normal tracking: Send mouse X & Y on button press and release
            csi!("?1000h"),
            // Button-event tracking: Report button motion events (dragging)
            csi!("?1002h"),
            // Any-event tracking: Report all motion events
            csi!("?1003h"),
            // RXVT mouse mode: Allows mouse coordinates of >223
            csi!("?1015h"),
            // SGR mouse mode: Allows mouse coordinates of >223, preferred over RXVT mode
            csi!("?1006h"),
        ))
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        sys::windows::enable_mouse_capture()
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        false
    }
}

/// A command that disables mouse event capturing.
///
/// Mouse events can be captured with [read](./fn.read.html)/[poll](./fn.poll.html).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisableMouseCapture;

impl Command for DisableMouseCapture {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        f.write_str(concat!(
            // The inverse commands of EnableMouseCapture, in reverse order.
            csi!("?1006l"),
            csi!("?1015l"),
            csi!("?1003l"),
            csi!("?1002l"),
            csi!("?1000l"),
        ))
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        sys::windows::disable_mouse_capture()
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        false
    }
}

/// A command that enables focus event emission.
///
/// It should be paired with [`DisableFocusChange`] at the end of execution.
///
/// Focus events can be captured with [read](./fn.read.html)/[poll](./fn.poll.html).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnableFocusChange;

impl Command for EnableFocusChange {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        f.write_str(csi!("?1004h"))
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        // Focus events are always enabled on Windows
        Ok(())
    }
}

/// A command that disables focus event emission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisableFocusChange;

impl Command for DisableFocusChange {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        f.write_str(csi!("?1004l"))
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        // Focus events can't be disabled on Windows
        Ok(())
    }
}

/// A command that enables [bracketed paste mode](https://en.wikipedia.org/wiki/Bracketed-paste).
///
/// It should be paired with [`DisableBracketedPaste`] at the end of execution.
///
/// This is not supported in older Windows terminals without
/// [virtual terminal sequences](https://docs.microsoft.com/en-us/windows/console/console-virtual-terminal-sequences).
#[cfg(feature = "bracketed-paste")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnableBracketedPaste;

#[cfg(feature = "bracketed-paste")]
impl Command for EnableBracketedPaste {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        f.write_str(csi!("?2004h"))
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Bracketed paste not implemented in the legacy Windows API.",
        ))
    }
}

/// A command that disables bracketed paste mode.
#[cfg(feature = "bracketed-paste")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisableBracketedPaste;

#[cfg(feature = "bracketed-paste")]
impl Command for DisableBracketedPaste {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        f.write_str(csi!("?2004l"))
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        Ok(())
    }
}

/// A command that enables the [kitty keyboard protocol](https://sw.kovidgoyal.net/kitty/keyboard-protocol/), which adds extra information to keyboard events and removes ambiguity for modifier keys.
///
/// It should be paired with [`PopKeyboardEnhancementFlags`] at the end of execution.
///
/// Example usage:
/// ```no_run
/// use std::io::{Write, stdout};
/// use crossterm::execute;
/// use crossterm::event::{
///     KeyboardEnhancementFlags,
///     PushKeyboardEnhancementFlags,
///     PopKeyboardEnhancementFlags
/// };
///
/// let mut stdout = stdout();
///
/// execute!(
///     stdout,
///     PushKeyboardEnhancementFlags(
///         KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
///     )
/// );
///
/// // ...
///
/// execute!(stdout, PopKeyboardEnhancementFlags);
/// ```
///
/// Note that, currently, only the following support this protocol:
/// * [kitty terminal](https://sw.kovidgoyal.net/kitty/)
/// * [foot terminal](https://codeberg.org/dnkl/foot/issues/319)
/// * [WezTerm terminal](https://wezfurlong.org/wezterm/config/lua/config/enable_kitty_keyboard.html)
/// * [alacritty terminal](https://github.com/alacritty/alacritty/issues/6378)
/// * [notcurses library](https://github.com/dankamongmen/notcurses/issues/2131)
/// * [neovim text editor](https://github.com/neovim/neovim/pull/18181)
/// * [kakoune text editor](https://github.com/mawww/kakoune/issues/4103)
/// * [dte text editor](https://gitlab.com/craigbarnes/dte/-/issues/138)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PushKeyboardEnhancementFlags(pub KeyboardEnhancementFlags);

impl Command for PushKeyboardEnhancementFlags {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        write!(f, "{}{}u", csi!(">"), self.0.bits())
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        use std::io;

        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Keyboard progressive enhancement not implemented for the legacy Windows API.",
        ))
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        false
    }
}

/// A command that disables extra kinds of keyboard events.
///
/// Specifically, it pops one level of keyboard enhancement flags.
///
/// See [`PushKeyboardEnhancementFlags`] and <https://sw.kovidgoyal.net/kitty/keyboard-protocol/> for more information.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PopKeyboardEnhancementFlags;

impl Command for PopKeyboardEnhancementFlags {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        f.write_str(csi!("<1u"))
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        use std::io;

        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Keyboard progressive enhancement not implemented for the legacy Windows API.",
        ))
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        false
    }
}

/// Represents an event.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "derive-more", derive(IsVariant))]
#[cfg_attr(not(feature = "bracketed-paste"), derive(Copy))]
#[derive(Debug, PartialOrd, PartialEq, Eq, Clone, Hash)]
pub enum Event {
    /// The terminal gained focus
    FocusGained,
    /// The terminal lost focus
    FocusLost,
    /// A single key event with additional pressed modifiers.
    Key(KeyEvent),
    /// A single mouse event with additional pressed modifiers.
    Mouse(MouseEvent),
    /// A string that was pasted into the terminal. Only emitted if bracketed paste has been
    /// enabled.
    #[cfg(feature = "bracketed-paste")]
    Paste(String),
    /// An resize event with new dimensions after resize (columns, rows).
    /// **Note** that resize events can occur in batches.
    Resize(u16, u16),
}

impl Event {
    /// Returns `true` if the event is a key press event.
    ///
    /// This is useful for waiting for any key press event, regardless of the key that was pressed.
    ///
    /// Returns `false` for key release and repeat events (as well as for non-key events).
    ///
    /// # Examples
    ///
    /// The following code runs a loop that processes events until a key press event is encountered:
    ///
    /// ```no_run
    /// use crossterm::event;
    ///
    /// while !event::read()?.is_key_press() {
    ///     // ...
    /// }
    /// # Ok::<(), std::io::Error>(())
    /// ```
    #[inline]
    pub fn is_key_press(&self) -> bool {
        matches!(
            self,
            Event::Key(KeyEvent {
                kind: KeyEventKind::Press,
                ..
            })
        )
    }

    /// Returns `true` if the event is a key release event.
    #[inline]
    pub fn is_key_release(&self) -> bool {
        matches!(
            self,
            Event::Key(KeyEvent {
                kind: KeyEventKind::Release,
                ..
            })
        )
    }

    /// Returns `true` if the event is a key repeat event.
    #[inline]
    pub fn is_key_repeat(&self) -> bool {
        matches!(
            self,
            Event::Key(KeyEvent {
                kind: KeyEventKind::Repeat,
                ..
            })
        )
    }

    /// Returns the key event if the event is a key event, otherwise `None`.
    ///
    /// This is a convenience method that makes apps that only care about key events easier to write.
    ///
    /// # Examples
    ///
    /// The following code runs a loop that only processes key events:
    ///
    /// ```no_run
    /// use crossterm::event;
    ///
    /// while let Some(key_event) = event::read()?.as_key_event() {
    ///     // ...
    /// }
    /// # std::io::Result::Ok(())
    /// ```
    #[inline]
    pub fn as_key_event(&self) -> Option<KeyEvent> {
        match self {
            Event::Key(event) => Some(*event),
            _ => None,
        }
    }

    /// Returns an Option containing the KeyEvent if the event is a key press event.
    ///
    /// This is a convenience method that makes apps that only care about key press events, and not
    /// key release or repeat events (or non-key events), easier to write.
    ///
    /// Returns `None` for key release and repeat events (as well as for non-key events).
    ///
    /// # Examples
    ///
    /// The following code runs a loop that only processes key press events:
    ///
    /// ```no_run
    /// use crossterm::event;
    ///
    /// while let Ok(event) = event::read() {
    ///     if let Some(key) = event.as_key_press_event() {
    ///         // ...
    ///     }
    /// }
    #[inline]
    pub fn as_key_press_event(&self) -> Option<KeyEvent> {
        match self {
            Event::Key(event) if self.is_key_press() => Some(*event),
            _ => None,
        }
    }

    /// Returns an Option containing the `KeyEvent` if the event is a key release event.
    #[inline]
    pub fn as_key_release_event(&self) -> Option<KeyEvent> {
        match self {
            Event::Key(event) if self.is_key_release() => Some(*event),
            _ => None,
        }
    }

    /// Returns an Option containing the `KeyEvent` if the event is a key repeat event.
    #[inline]
    pub fn as_key_repeat_event(&self) -> Option<KeyEvent> {
        match self {
            Event::Key(event) if self.is_key_repeat() => Some(*event),
            _ => None,
        }
    }

    /// Returns the mouse event if the event is a mouse event, otherwise `None`.
    ///
    /// This is a convenience method that makes code which only cares about mouse events easier to
    /// write.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use crossterm::event;
    ///
    /// while let Some(mouse_event) = event::read()?.as_mouse_event() {
    ///     // ...
    /// }
    /// # std::io::Result::Ok(())
    /// ```
    #[inline]
    pub fn as_mouse_event(&self) -> Option<MouseEvent> {
        match self {
            Event::Mouse(event) => Some(*event),
            _ => None,
        }
    }

    /// Returns the pasted string if the event is a paste event, otherwise `None`.
    ///
    /// This is a convenience method that makes code which only cares about paste events easier to write.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use crossterm::event;
    ///
    /// while let Some(paste) = event::read()?.as_paste_event() {
    ///     // ...
    /// }
    /// # std::io::Result::Ok(())
    /// ```
    #[cfg(feature = "bracketed-paste")]
    #[inline]
    pub fn as_paste_event(&self) -> Option<&str> {
        match self {
            Event::Paste(paste) => Some(paste),
            _ => None,
        }
    }

    /// Returns the size as a tuple if the event is a resize event, otherwise `None`.
    ///
    /// This is a convenience method that makes code which only cares about resize events easier to write.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use crossterm::event;
    ///
    /// while let Some((columns, rows)) = event::read()?.as_resize_event() {
    ///     // ...
    /// }
    /// # std::io::Result::Ok(())
    /// ```
    #[inline]
    pub fn as_resize_event(&self) -> Option<(u16, u16)> {
        match self {
            Event::Resize(columns, rows) => Some((*columns, *rows)),
            _ => None,
        }
    }
}

/// Represents a mouse event.
///
/// # Platform-specific Notes
///
/// ## Mouse Buttons
///
/// Some platforms/terminals do not report mouse button for the
/// `MouseEventKind::Up` and `MouseEventKind::Drag` events. `MouseButton::Left`
/// is returned if we don't know which button was used.
///
/// ## Key Modifiers
///
/// Some platforms/terminals does not report all key modifiers
/// combinations for all mouse event types. For example - macOS reports
/// `Ctrl` + left mouse button click as a right mouse button click.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, PartialOrd, PartialEq, Eq, Clone, Copy, Hash)]
pub struct MouseEvent {
    /// The kind of mouse event that was caused.
    pub kind: MouseEventKind,
    /// The column that the event occurred on.
    pub column: u16,
    /// The row that the event occurred on.
    pub row: u16,
    /// The key modifiers active when the event occurred.
    pub modifiers: KeyModifiers,
}

/// A mouse event kind.
///
/// # Platform-specific Notes
///
/// ## Mouse Buttons
///
/// Some platforms/terminals do not report mouse button for the
/// `MouseEventKind::Up` and `MouseEventKind::Drag` events. `MouseButton::Left`
/// is returned if we don't know which button was used.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "derive-more", derive(IsVariant))]
#[derive(Debug, PartialOrd, PartialEq, Eq, Clone, Copy, Hash)]
pub enum MouseEventKind {
    /// Pressed mouse button. Contains the button that was pressed.
    Down(MouseButton),
    /// Released mouse button. Contains the button that was released.
    Up(MouseButton),
    /// Moved the mouse cursor while pressing the contained mouse button.
    Drag(MouseButton),
    /// Moved the mouse cursor while not pressing a mouse button.
    Moved,
    /// Scrolled mouse wheel downwards (towards the user).
    ScrollDown,
    /// Scrolled mouse wheel upwards (away from the user).
    ScrollUp,
    /// Scrolled mouse wheel left (mostly on a laptop touchpad).
    ScrollLeft,
    /// Scrolled mouse wheel right (mostly on a laptop touchpad).
    ScrollRight,
}

/// Represents a mouse button.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "derive-more", derive(IsVariant))]
#[derive(Debug, PartialOrd, PartialEq, Eq, Clone, Copy, Hash)]
pub enum MouseButton {
    /// Left mouse button.
    Left,
    /// Right mouse button.
    Right,
    /// Middle mouse button.
    Middle,
}

bitflags! {
    /// Represents key modifiers (shift, control, alt, etc.).
    ///
    /// **Note:** `SUPER`, `HYPER`, and `META` can only be read if
    /// [`KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES`] has been enabled with
    /// [`PushKeyboardEnhancementFlags`].
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(transparent))]
    #[derive(Debug, PartialOrd, PartialEq, Eq, Clone, Copy, Hash)]
    pub struct KeyModifiers: u8 {
        const SHIFT = 0b0000_0001;
        const CONTROL = 0b0000_0010;
        const ALT = 0b0000_0100;
        const SUPER = 0b0000_1000;
        const HYPER = 0b0001_0000;
        const META = 0b0010_0000;
        const NONE = 0b0000_0000;
    }
}

impl Display for KeyModifiers {
    /// Formats the key modifiers using the given formatter.
    ///
    /// The key modifiers are joined by a `+` character.
    ///
    /// # Platform-specific Notes
    ///
    /// On macOS, the control, alt, and super keys is displayed as "Control", "Option", and
    /// "Command" respectively. See
    /// <https://support.apple.com/guide/applestyleguide/welcome/1.0/web>.
    ///
    /// On Windows, the super key is displayed as "Windows" and the control key is displayed as
    /// "Ctrl". See
    /// <https://learn.microsoft.com/en-us/style-guide/a-z-word-list-term-collections/term-collections/keys-keyboard-shortcuts>.
    ///
    /// On other platforms, the super key is referred to as "Super" and the control key is
    /// displayed as "Ctrl".
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for modifier in self.iter() {
            if !first {
                f.write_str("+")?;
            }

            first = false;
            match modifier {
                KeyModifiers::SHIFT => f.write_str("Shift")?,
                #[cfg(unix)]
                KeyModifiers::CONTROL => f.write_str("Control")?,
                #[cfg(windows)]
                KeyModifiers::CONTROL => f.write_str("Ctrl")?,
                #[cfg(target_os = "macos")]
                KeyModifiers::ALT => f.write_str("Option")?,
                #[cfg(not(target_os = "macos"))]
                KeyModifiers::ALT => f.write_str("Alt")?,
                #[cfg(target_os = "macos")]
                KeyModifiers::SUPER => f.write_str("Command")?,
                #[cfg(target_os = "windows")]
                KeyModifiers::SUPER => f.write_str("Windows")?,
                #[cfg(not(any(target_os = "macos", target_os = "windows")))]
                KeyModifiers::SUPER => f.write_str("Super")?,
                KeyModifiers::HYPER => f.write_str("Hyper")?,
                KeyModifiers::META => f.write_str("Meta")?,
                _ => unreachable!(),
            }
        }
        Ok(())
    }
}

/// Represents a keyboard event kind.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "derive-more", derive(IsVariant))]
#[derive(Debug, PartialOrd, PartialEq, Eq, Clone, Copy, Hash)]
pub enum KeyEventKind {
    Press,
    Repeat,
    Release,
}

bitflags! {
    /// Represents extra state about the key event.
    ///
    /// **Note:** This state can only be read if
    /// [`KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES`] has been enabled with
    /// [`PushKeyboardEnhancementFlags`].
    #[derive(Debug, PartialOrd, PartialEq, Eq, Clone, Copy, Hash)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(transparent))]
    pub struct KeyEventState: u8 {
        /// The key event origins from the keypad.
        const KEYPAD = 0b0000_0001;
        /// Caps Lock was enabled for this key event.
        ///
        /// **Note:** this is set for the initial press of Caps Lock itself.
        const CAPS_LOCK = 0b0000_0010;
        /// Num Lock was enabled for this key event.
        ///
        /// **Note:** this is set for the initial press of Num Lock itself.
        const NUM_LOCK = 0b0000_0100;
        const NONE = 0b0000_0000;
    }
}

/// Represents a key event.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, PartialOrd, Clone, Copy)]
pub struct KeyEvent {
    /// The key itself.
    pub code: KeyCode,
    /// Additional key modifiers.
    pub modifiers: KeyModifiers,
    /// Kind of event.
    ///
    /// Only set if:
    /// - Unix: [`KeyboardEnhancementFlags::REPORT_EVENT_TYPES`] has been enabled with [`PushKeyboardEnhancementFlags`].
    /// - Windows: always
    pub kind: KeyEventKind,
    /// Keyboard state.
    ///
    /// Only set if [`KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES`] has been enabled with
    /// [`PushKeyboardEnhancementFlags`].
    pub state: KeyEventState,
}

impl KeyEvent {
    pub const fn new(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        }
    }

    pub const fn new_with_kind(
        code: KeyCode,
        modifiers: KeyModifiers,
        kind: KeyEventKind,
    ) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind,
            state: KeyEventState::empty(),
        }
    }

    pub const fn new_with_kind_and_state(
        code: KeyCode,
        modifiers: KeyModifiers,
        kind: KeyEventKind,
        state: KeyEventState,
    ) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind,
            state,
        }
    }

    // modifies the KeyEvent,
    // so that KeyModifiers::SHIFT is present iff
    // an uppercase char is present.
    fn normalize_case(mut self) -> KeyEvent {
        let c = match self.code {
            KeyCode::Char(c) => c,
            _ => return self,
        };

        if c.is_ascii_uppercase() {
            self.modifiers.insert(KeyModifiers::SHIFT);
        } else if self.modifiers.contains(KeyModifiers::SHIFT) {
            self.code = KeyCode::Char(c.to_ascii_uppercase())
        }
        self
    }

    /// Returns whether the key event is a press event.
    pub fn is_press(&self) -> bool {
        matches!(self.kind, KeyEventKind::Press)
    }

    /// Returns whether the key event is a release event.
    pub fn is_release(&self) -> bool {
        matches!(self.kind, KeyEventKind::Release)
    }

    /// Returns whether the key event is a repeat event.
    pub fn is_repeat(&self) -> bool {
        matches!(self.kind, KeyEventKind::Repeat)
    }
}

impl From<KeyCode> for KeyEvent {
    fn from(code: KeyCode) -> Self {
        KeyEvent {
            code,
            modifiers: KeyModifiers::empty(),
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        }
    }
}

impl PartialEq for KeyEvent {
    fn eq(&self, other: &KeyEvent) -> bool {
        let KeyEvent {
            code: lhs_code,
            modifiers: lhs_modifiers,
            kind: lhs_kind,
            state: lhs_state,
        } = self.normalize_case();
        let KeyEvent {
            code: rhs_code,
            modifiers: rhs_modifiers,
            kind: rhs_kind,
            state: rhs_state,
        } = other.normalize_case();
        (lhs_code == rhs_code)
            && (lhs_modifiers == rhs_modifiers)
            && (lhs_kind == rhs_kind)
            && (lhs_state == rhs_state)
    }
}

impl Eq for KeyEvent {}

impl Hash for KeyEvent {
    fn hash<H: Hasher>(&self, hash_state: &mut H) {
        let KeyEvent {
            code,
            modifiers,
            kind,
            state,
        } = self.normalize_case();
        code.hash(hash_state);
        modifiers.hash(hash_state);
        kind.hash(hash_state);
        state.hash(hash_state);
    }
}

/// Represents a media key (as part of [`KeyCode::Media`]).
#[derive(Debug, PartialOrd, PartialEq, Eq, Clone, Copy, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MediaKeyCode {
    /// Play media key.
    Play,
    /// Pause media key.
    Pause,
    /// Play/Pause media key.
    PlayPause,
    /// Reverse media key.
    Reverse,
    /// Stop media key.
    Stop,
    /// Fast-forward media key.
    FastForward,
    /// Rewind media key.
    Rewind,
    /// Next-track media key.
    TrackNext,
    /// Previous-track media key.
    TrackPrevious,
    /// Record media key.
    Record,
    /// Lower-volume media key.
    LowerVolume,
    /// Raise-volume media key.
    RaiseVolume,
    /// Mute media key.
    MuteVolume,
}

impl Display for MediaKeyCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MediaKeyCode::Play => write!(f, "Play"),
            MediaKeyCode::Pause => write!(f, "Pause"),
            MediaKeyCode::PlayPause => write!(f, "Play/Pause"),
            MediaKeyCode::Reverse => write!(f, "Reverse"),
            MediaKeyCode::Stop => write!(f, "Stop"),
            MediaKeyCode::FastForward => write!(f, "Fast Forward"),
            MediaKeyCode::Rewind => write!(f, "Rewind"),
            MediaKeyCode::TrackNext => write!(f, "Next Track"),
            MediaKeyCode::TrackPrevious => write!(f, "Previous Track"),
            MediaKeyCode::Record => write!(f, "Record"),
            MediaKeyCode::LowerVolume => write!(f, "Lower Volume"),
            MediaKeyCode::RaiseVolume => write!(f, "Raise Volume"),
            MediaKeyCode::MuteVolume => write!(f, "Mute Volume"),
        }
    }
}

/// Represents a modifier key (as part of [`KeyCode::Modifier`]).
#[derive(Debug, PartialOrd, PartialEq, Eq, Clone, Copy, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ModifierKeyCode {
    /// Left Shift key.
    LeftShift,
    /// Left Control key. (Control on macOS, Ctrl on other platforms)
    LeftControl,
    /// Left Alt key. (Option on macOS, Alt on other platforms)
    LeftAlt,
    /// Left Super key. (Command on macOS, Windows on Windows, Super on other platforms)
    LeftSuper,
    /// Left Hyper key.
    LeftHyper,
    /// Left Meta key.
    LeftMeta,
    /// Right Shift key.
    RightShift,
    /// Right Control key. (Control on macOS, Ctrl on other platforms)
    RightControl,
    /// Right Alt key. (Option on macOS, Alt on other platforms)
    RightAlt,
    /// Right Super key. (Command on macOS, Windows on Windows, Super on other platforms)
    RightSuper,
    /// Right Hyper key.
    RightHyper,
    /// Right Meta key.
    RightMeta,
    /// Iso Level3 Shift key.
    IsoLevel3Shift,
    /// Iso Level5 Shift key.
    IsoLevel5Shift,
}

impl Display for ModifierKeyCode {
    /// Formats the modifier key using the given formatter.
    ///
    /// # Platform-specific Notes
    ///
    /// On macOS, the control, alt, and super keys are displayed as "Control", "Option", and
    /// "Command" respectively. See
    /// <https://support.apple.com/guide/applestyleguide/welcome/1.0/web>.
    ///
    /// On Windows, the super key is displayed as "Windows" and the control key is displayed as
    /// "Ctrl". See
    /// <https://learn.microsoft.com/en-us/style-guide/a-z-word-list-term-collections/term-collections/keys-keyboard-shortcuts>.
    ///
    /// On other platforms, the super key is referred to as "Super".
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModifierKeyCode::LeftShift => write!(f, "Left Shift"),
            ModifierKeyCode::LeftHyper => write!(f, "Left Hyper"),
            ModifierKeyCode::LeftMeta => write!(f, "Left Meta"),
            ModifierKeyCode::RightShift => write!(f, "Right Shift"),
            ModifierKeyCode::RightHyper => write!(f, "Right Hyper"),
            ModifierKeyCode::RightMeta => write!(f, "Right Meta"),
            ModifierKeyCode::IsoLevel3Shift => write!(f, "Iso Level 3 Shift"),
            ModifierKeyCode::IsoLevel5Shift => write!(f, "Iso Level 5 Shift"),

            #[cfg(target_os = "macos")]
            ModifierKeyCode::LeftControl => write!(f, "Left Control"),
            #[cfg(not(target_os = "macos"))]
            ModifierKeyCode::LeftControl => write!(f, "Left Ctrl"),

            #[cfg(target_os = "macos")]
            ModifierKeyCode::LeftAlt => write!(f, "Left Option"),
            #[cfg(not(target_os = "macos"))]
            ModifierKeyCode::LeftAlt => write!(f, "Left Alt"),

            #[cfg(target_os = "macos")]
            ModifierKeyCode::LeftSuper => write!(f, "Left Command"),
            #[cfg(target_os = "windows")]
            ModifierKeyCode::LeftSuper => write!(f, "Left Windows"),
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            ModifierKeyCode::LeftSuper => write!(f, "Left Super"),

            #[cfg(target_os = "macos")]
            ModifierKeyCode::RightControl => write!(f, "Right Control"),
            #[cfg(not(target_os = "macos"))]
            ModifierKeyCode::RightControl => write!(f, "Right Ctrl"),

            #[cfg(target_os = "macos")]
            ModifierKeyCode::RightAlt => write!(f, "Right Option"),
            #[cfg(not(target_os = "macos"))]
            ModifierKeyCode::RightAlt => write!(f, "Right Alt"),

            #[cfg(target_os = "macos")]
            ModifierKeyCode::RightSuper => write!(f, "Right Command"),
            #[cfg(target_os = "windows")]
            ModifierKeyCode::RightSuper => write!(f, "Right Windows"),
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            ModifierKeyCode::RightSuper => write!(f, "Right Super"),
        }
    }
}

/// Represents a key.
#[derive(Debug, PartialOrd, PartialEq, Eq, Clone, Copy, Hash)]
#[cfg_attr(feature = "derive-more", derive(IsVariant))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum KeyCode {
    /// Backspace key (Delete on macOS, Backspace on other platforms).
    Backspace,
    /// Enter key.
    Enter,
    /// Left arrow key.
    Left,
    /// Right arrow key.
    Right,
    /// Up arrow key.
    Up,
    /// Down arrow key.
    Down,
    /// Home key.
    Home,
    /// End key.
    End,
    /// Page up key.
    PageUp,
    /// Page down key.
    PageDown,
    /// Tab key.
    Tab,
    /// Shift + Tab key.
    BackTab,
    /// Delete key. (Fn+Delete on macOS, Delete on other platforms)
    Delete,
    /// Insert key.
    Insert,
    /// F key.
    ///
    /// `KeyCode::F(1)` represents F1 key, etc.
    #[cfg_attr(feature = "derive-more", is_variant(ignore))]
    F(u8),
    /// A character.
    ///
    /// `KeyCode::Char('c')` represents `c` character, etc.
    #[cfg_attr(feature = "derive-more", is_variant(ignore))]
    Char(char),
    /// Null.
    Null,
    /// Escape key.
    Esc,
    /// Caps Lock key.
    ///
    /// **Note:** this key can only be read if
    /// [`KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES`] has been enabled with
    /// [`PushKeyboardEnhancementFlags`].
    CapsLock,
    /// Scroll Lock key.
    ///
    /// **Note:** this key can only be read if
    /// [`KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES`] has been enabled with
    /// [`PushKeyboardEnhancementFlags`].
    ScrollLock,
    /// Num Lock key.
    ///
    /// **Note:** this key can only be read if
    /// [`KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES`] has been enabled with
    /// [`PushKeyboardEnhancementFlags`].
    NumLock,
    /// Print Screen key.
    ///
    /// **Note:** this key can only be read if
    /// [`KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES`] has been enabled with
    /// [`PushKeyboardEnhancementFlags`].
    PrintScreen,
    /// Pause key.
    ///
    /// **Note:** this key can only be read if
    /// [`KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES`] has been enabled with
    /// [`PushKeyboardEnhancementFlags`].
    Pause,
    /// Menu key.
    ///
    /// **Note:** this key can only be read if
    /// [`KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES`] has been enabled with
    /// [`PushKeyboardEnhancementFlags`].
    Menu,
    /// The "Begin" key (often mapped to the 5 key when Num Lock is turned on).
    ///
    /// **Note:** this key can only be read if
    /// [`KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES`] has been enabled with
    /// [`PushKeyboardEnhancementFlags`].
    KeypadBegin,
    /// A media key.
    ///
    /// **Note:** these keys can only be read if
    /// [`KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES`] has been enabled with
    /// [`PushKeyboardEnhancementFlags`].
    #[cfg_attr(feature = "derive-more", is_variant(ignore))]
    Media(MediaKeyCode),
    /// A modifier key.
    ///
    /// **Note:** these keys can only be read if **both**
    /// [`KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES`] and
    /// [`KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES`] have been enabled with
    /// [`PushKeyboardEnhancementFlags`].
    #[cfg_attr(feature = "derive-more", is_variant(ignore))]
    Modifier(ModifierKeyCode),
}

impl KeyCode {
    /// Returns `true` if the key code is the given function key.
    ///
    /// # Examples
    ///
    /// ```
    /// # use crossterm::event::KeyCode;
    /// assert!(KeyCode::F(1).is_function_key(1));
    /// assert!(!KeyCode::F(1).is_function_key(2));
    /// ```
    pub fn is_function_key(&self, n: u8) -> bool {
        matches!(self, KeyCode::F(m) if *m == n)
    }

    /// Returns `true` if the key code is the given character.
    ///
    /// # Examples
    ///
    /// ```
    /// # use crossterm::event::KeyCode;
    /// assert!(KeyCode::Char('a').is_char('a'));
    /// assert!(!KeyCode::Char('a').is_char('b'));
    /// assert!(!KeyCode::F(1).is_char('a'));
    /// ```
    pub fn is_char(&self, c: char) -> bool {
        matches!(self, KeyCode::Char(m) if *m == c)
    }

    /// Returns the character if the key code is a character key.
    ///
    /// Returns `None` if the key code is not a character key.
    ///
    /// # Examples
    ///
    /// ```
    /// # use crossterm::event::KeyCode;
    /// assert_eq!(KeyCode::Char('a').as_char(), Some('a'));
    /// assert_eq!(KeyCode::F(1).as_char(), None);
    /// ```
    pub fn as_char(&self) -> Option<char> {
        match self {
            KeyCode::Char(c) => Some(*c),
            _ => None,
        }
    }

    /// Returns `true` if the key code is the given media key.
    ///
    /// **Note:** this method requires
    /// [`KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES`] to be enabled with
    /// [`PushKeyboardEnhancementFlags`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use crossterm::event::{KeyCode, MediaKeyCode};
    /// assert!(KeyCode::Media(MediaKeyCode::Play).is_media_key(MediaKeyCode::Play));
    /// assert!(!KeyCode::Media(MediaKeyCode::Play).is_media_key(MediaKeyCode::Pause));
    /// ```
    pub fn is_media_key(&self, media: MediaKeyCode) -> bool {
        matches!(self, KeyCode::Media(m) if *m == media)
    }

    /// Returns `true` if the key code is the given modifier key.
    ///
    /// **Note:** this method requires both
    /// [`KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES`] and
    /// [`KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES`] to be enabled with
    /// [`PushKeyboardEnhancementFlags`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use crossterm::event::{KeyCode, ModifierKeyCode};
    /// assert!(KeyCode::Modifier(ModifierKeyCode::LeftShift).is_modifier(ModifierKeyCode::LeftShift));
    /// assert!(!KeyCode::Modifier(ModifierKeyCode::LeftShift).is_modifier(ModifierKeyCode::RightShift));
    /// ```
    pub fn is_modifier(&self, modifier: ModifierKeyCode) -> bool {
        matches!(self, KeyCode::Modifier(m) if *m == modifier)
    }
}

impl Display for KeyCode {
    /// Formats the `KeyCode` using the given formatter.
    ///
    /// # Platform-specific Notes
    ///
    /// On macOS, the Backspace key is displayed as "Delete", the Delete key is displayed as "Fwd
    /// Del", and the Enter key is displayed as "Return". See
    /// <https://support.apple.com/guide/applestyleguide/welcome/1.0/web>.
    ///
    /// On other platforms, the Backspace key is displayed as "Backspace", the Delete key is
    /// displayed as "Del", and the Enter key is displayed as "Enter".
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // On macOS, the Backspace key is called "Delete" and the Delete key is called "Fwd Del".
            #[cfg(target_os = "macos")]
            KeyCode::Backspace => write!(f, "Delete"),
            #[cfg(target_os = "macos")]
            KeyCode::Delete => write!(f, "Fwd Del"),

            #[cfg(not(target_os = "macos"))]
            KeyCode::Backspace => write!(f, "Backspace"),
            #[cfg(not(target_os = "macos"))]
            KeyCode::Delete => write!(f, "Del"),

            #[cfg(target_os = "macos")]
            KeyCode::Enter => write!(f, "Return"),
            #[cfg(not(target_os = "macos"))]
            KeyCode::Enter => write!(f, "Enter"),
            KeyCode::Left => write!(f, "Left"),
            KeyCode::Right => write!(f, "Right"),
            KeyCode::Up => write!(f, "Up"),
            KeyCode::Down => write!(f, "Down"),
            KeyCode::Home => write!(f, "Home"),
            KeyCode::End => write!(f, "End"),
            KeyCode::PageUp => write!(f, "Page Up"),
            KeyCode::PageDown => write!(f, "Page Down"),
            KeyCode::Tab => write!(f, "Tab"),
            KeyCode::BackTab => write!(f, "Back Tab"),
            KeyCode::Insert => write!(f, "Insert"),
            KeyCode::F(n) => write!(f, "F{}", n),
            KeyCode::Char(c) => match c {
                // special case for non-visible characters
                ' ' => write!(f, "Space"),
                c => write!(f, "{}", c),
            },
            KeyCode::Null => write!(f, "Null"),
            KeyCode::Esc => write!(f, "Esc"),
            KeyCode::CapsLock => write!(f, "Caps Lock"),
            KeyCode::ScrollLock => write!(f, "Scroll Lock"),
            KeyCode::NumLock => write!(f, "Num Lock"),
            KeyCode::PrintScreen => write!(f, "Print Screen"),
            KeyCode::Pause => write!(f, "Pause"),
            KeyCode::Menu => write!(f, "Menu"),
            KeyCode::KeypadBegin => write!(f, "Begin"),
            KeyCode::Media(media) => write!(f, "{}", media),
            KeyCode::Modifier(modifier) => write!(f, "{}", modifier),
        }
    }
}

/// An internal event.
///
/// Encapsulates publicly available `Event` with additional internal
/// events that shouldn't be publicly available to the crate users.
#[derive(Debug, PartialOrd, PartialEq, Hash, Clone, Eq)]
pub(crate) enum InternalEvent {
    /// An event.
    Event(Event),
    /// A cursor position (`col`, `row`).
    #[cfg(unix)]
    CursorPosition(u16, u16),
    /// The progressive keyboard enhancement flags enabled by the terminal.
    #[cfg(unix)]
    KeyboardEnhancementFlags(KeyboardEnhancementFlags),
    /// Attributes and architectural class of the terminal.
    #[cfg(unix)]
    PrimaryDeviceAttributes,
}

#[cfg(test)]
mod tests {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    use super::*;
    use KeyCode::*;
    use MediaKeyCode::*;
    use ModifierKeyCode::*;

    #[test]
    fn test_equality() {
        let lowercase_d_with_shift = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::SHIFT);
        let uppercase_d_with_shift = KeyEvent::new(KeyCode::Char('D'), KeyModifiers::SHIFT);
        let uppercase_d = KeyEvent::new(KeyCode::Char('D'), KeyModifiers::NONE);
        assert_eq!(lowercase_d_with_shift, uppercase_d_with_shift);
        assert_eq!(uppercase_d, uppercase_d_with_shift);
    }

    #[test]
    fn test_hash() {
        let lowercase_d_with_shift_hash = {
            let mut hasher = DefaultHasher::new();
            KeyEvent::new(KeyCode::Char('d'), KeyModifiers::SHIFT).hash(&mut hasher);
            hasher.finish()
        };
        let uppercase_d_with_shift_hash = {
            let mut hasher = DefaultHasher::new();
            KeyEvent::new(KeyCode::Char('D'), KeyModifiers::SHIFT).hash(&mut hasher);
            hasher.finish()
        };
        let uppercase_d_hash = {
            let mut hasher = DefaultHasher::new();
            KeyEvent::new(KeyCode::Char('D'), KeyModifiers::NONE).hash(&mut hasher);
            hasher.finish()
        };
        assert_eq!(lowercase_d_with_shift_hash, uppercase_d_with_shift_hash);
        assert_eq!(uppercase_d_hash, uppercase_d_with_shift_hash);
    }

    #[test]
    fn keycode_display() {
        #[cfg(target_os = "macos")]
        {
            assert_eq!(format!("{}", Backspace), "Delete");
            assert_eq!(format!("{}", Delete), "Fwd Del");
            assert_eq!(format!("{}", Enter), "Return");
        }
        #[cfg(not(target_os = "macos"))]
        {
            assert_eq!(format!("{}", Backspace), "Backspace");
            assert_eq!(format!("{}", Delete), "Del");
            assert_eq!(format!("{}", Enter), "Enter");
        }
        assert_eq!(format!("{}", Left), "Left");
        assert_eq!(format!("{}", Right), "Right");
        assert_eq!(format!("{}", Up), "Up");
        assert_eq!(format!("{}", Down), "Down");
        assert_eq!(format!("{}", Home), "Home");
        assert_eq!(format!("{}", End), "End");
        assert_eq!(format!("{}", PageUp), "Page Up");
        assert_eq!(format!("{}", PageDown), "Page Down");
        assert_eq!(format!("{}", Tab), "Tab");
        assert_eq!(format!("{}", BackTab), "Back Tab");
        assert_eq!(format!("{}", Insert), "Insert");
        assert_eq!(format!("{}", F(1)), "F1");
        assert_eq!(format!("{}", Char('a')), "a");
        assert_eq!(format!("{}", Null), "Null");
        assert_eq!(format!("{}", Esc), "Esc");
        assert_eq!(format!("{}", CapsLock), "Caps Lock");
        assert_eq!(format!("{}", ScrollLock), "Scroll Lock");
        assert_eq!(format!("{}", NumLock), "Num Lock");
        assert_eq!(format!("{}", PrintScreen), "Print Screen");
        assert_eq!(format!("{}", KeyCode::Pause), "Pause");
        assert_eq!(format!("{}", Menu), "Menu");
        assert_eq!(format!("{}", KeypadBegin), "Begin");
    }

    #[test]
    fn media_keycode_display() {
        assert_eq!(format!("{}", Media(Play)), "Play");
        assert_eq!(format!("{}", Media(MediaKeyCode::Pause)), "Pause");
        assert_eq!(format!("{}", Media(PlayPause)), "Play/Pause");
        assert_eq!(format!("{}", Media(Reverse)), "Reverse");
        assert_eq!(format!("{}", Media(Stop)), "Stop");
        assert_eq!(format!("{}", Media(FastForward)), "Fast Forward");
        assert_eq!(format!("{}", Media(Rewind)), "Rewind");
        assert_eq!(format!("{}", Media(TrackNext)), "Next Track");
        assert_eq!(format!("{}", Media(TrackPrevious)), "Previous Track");
        assert_eq!(format!("{}", Media(Record)), "Record");
        assert_eq!(format!("{}", Media(LowerVolume)), "Lower Volume");
        assert_eq!(format!("{}", Media(RaiseVolume)), "Raise Volume");
        assert_eq!(format!("{}", Media(MuteVolume)), "Mute Volume");
    }

    #[test]
    fn modifier_keycode_display() {
        assert_eq!(format!("{}", Modifier(LeftShift)), "Left Shift");
        assert_eq!(format!("{}", Modifier(LeftHyper)), "Left Hyper");
        assert_eq!(format!("{}", Modifier(LeftMeta)), "Left Meta");
        assert_eq!(format!("{}", Modifier(RightShift)), "Right Shift");
        assert_eq!(format!("{}", Modifier(RightHyper)), "Right Hyper");
        assert_eq!(format!("{}", Modifier(RightMeta)), "Right Meta");
        assert_eq!(format!("{}", Modifier(IsoLevel3Shift)), "Iso Level 3 Shift");
        assert_eq!(format!("{}", Modifier(IsoLevel5Shift)), "Iso Level 5 Shift");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn modifier_keycode_display_macos() {
        assert_eq!(format!("{}", Modifier(LeftControl)), "Left Control");
        assert_eq!(format!("{}", Modifier(LeftAlt)), "Left Option");
        assert_eq!(format!("{}", Modifier(LeftSuper)), "Left Command");
        assert_eq!(format!("{}", Modifier(RightControl)), "Right Control");
        assert_eq!(format!("{}", Modifier(RightAlt)), "Right Option");
        assert_eq!(format!("{}", Modifier(RightSuper)), "Right Command");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn modifier_keycode_display_windows() {
        assert_eq!(format!("{}", Modifier(LeftControl)), "Left Ctrl");
        assert_eq!(format!("{}", Modifier(LeftAlt)), "Left Alt");
        assert_eq!(format!("{}", Modifier(LeftSuper)), "Left Windows");
        assert_eq!(format!("{}", Modifier(RightControl)), "Right Ctrl");
        assert_eq!(format!("{}", Modifier(RightAlt)), "Right Alt");
        assert_eq!(format!("{}", Modifier(RightSuper)), "Right Windows");
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    #[test]
    fn modifier_keycode_display_other() {
        assert_eq!(format!("{}", Modifier(LeftControl)), "Left Ctrl");
        assert_eq!(format!("{}", Modifier(LeftAlt)), "Left Alt");
        assert_eq!(format!("{}", Modifier(LeftSuper)), "Left Super");
        assert_eq!(format!("{}", Modifier(RightControl)), "Right Ctrl");
        assert_eq!(format!("{}", Modifier(RightAlt)), "Right Alt");
        assert_eq!(format!("{}", Modifier(RightSuper)), "Right Super");
    }

    #[test]
    fn key_modifiers_display() {
        let modifiers = KeyModifiers::SHIFT | KeyModifiers::CONTROL | KeyModifiers::ALT;

        #[cfg(target_os = "macos")]
        assert_eq!(modifiers.to_string(), "Shift+Control+Option");

        #[cfg(target_os = "windows")]
        assert_eq!(modifiers.to_string(), "Shift+Ctrl+Alt");

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        assert_eq!(modifiers.to_string(), "Shift+Control+Alt");
    }

    const ESC_PRESSED: KeyEvent =
        KeyEvent::new_with_kind(KeyCode::Esc, KeyModifiers::empty(), KeyEventKind::Press);
    const ESC_RELEASED: KeyEvent =
        KeyEvent::new_with_kind(KeyCode::Esc, KeyModifiers::empty(), KeyEventKind::Release);
    const ESC_REPEAT: KeyEvent =
        KeyEvent::new_with_kind(KeyCode::Esc, KeyModifiers::empty(), KeyEventKind::Repeat);
    const MOUSE_CLICK: MouseEvent = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 1,
        row: 1,
        modifiers: KeyModifiers::empty(),
    };

    #[cfg(feature = "derive-more")]
    #[test]
    fn event_is() {
        let event = Event::FocusGained;
        assert!(event.is_focus_gained());
        assert!(event.is_focus_gained());
        assert!(!event.is_key());

        let event = Event::FocusLost;
        assert!(event.is_focus_lost());
        assert!(!event.is_focus_gained());
        assert!(!event.is_key());

        let event = Event::Resize(1, 1);
        assert!(event.is_resize());
        assert!(!event.is_key());

        let event = Event::Key(ESC_PRESSED);
        assert!(event.is_key());
        assert!(event.is_key_press());
        assert!(!event.is_key_release());
        assert!(!event.is_key_repeat());
        assert!(!event.is_focus_gained());

        let event = Event::Key(ESC_RELEASED);
        assert!(event.is_key());
        assert!(!event.is_key_press());
        assert!(event.is_key_release());
        assert!(!event.is_key_repeat());
        assert!(!event.is_focus_gained());

        let event = Event::Key(ESC_REPEAT);
        assert!(event.is_key());
        assert!(!event.is_key_press());
        assert!(!event.is_key_release());
        assert!(event.is_key_repeat());
        assert!(!event.is_focus_gained());

        let event = Event::Mouse(MOUSE_CLICK);
        assert!(event.is_mouse());
        assert!(!event.is_key());

        #[cfg(feature = "bracketed-paste")]
        {
            let event = Event::Paste("".to_string());
            assert!(event.is_paste());
            assert!(!event.is_key());
        }
    }

    #[test]
    fn event_as() {
        let event = Event::FocusGained;
        assert_eq!(event.as_key_event(), None);

        let event = Event::Key(ESC_PRESSED);
        assert_eq!(event.as_key_event(), Some(ESC_PRESSED));
        assert_eq!(event.as_key_press_event(), Some(ESC_PRESSED));
        assert_eq!(event.as_key_release_event(), None);
        assert_eq!(event.as_key_repeat_event(), None);
        assert_eq!(event.as_resize_event(), None);

        let event = Event::Key(ESC_RELEASED);
        assert_eq!(event.as_key_event(), Some(ESC_RELEASED));
        assert_eq!(event.as_key_release_event(), Some(ESC_RELEASED));
        assert_eq!(event.as_key_press_event(), None);
        assert_eq!(event.as_key_repeat_event(), None);
        assert_eq!(event.as_resize_event(), None);

        let event = Event::Key(ESC_REPEAT);
        assert_eq!(event.as_key_event(), Some(ESC_REPEAT));
        assert_eq!(event.as_key_repeat_event(), Some(ESC_REPEAT));
        assert_eq!(event.as_key_press_event(), None);
        assert_eq!(event.as_key_release_event(), None);
        assert_eq!(event.as_resize_event(), None);

        let event = Event::Resize(1, 1);
        assert_eq!(event.as_resize_event(), Some((1, 1)));
        assert_eq!(event.as_key_event(), None);

        let event = Event::Mouse(MOUSE_CLICK);
        assert_eq!(event.as_mouse_event(), Some(MOUSE_CLICK));
        assert_eq!(event.as_key_event(), None);

        #[cfg(feature = "bracketed-paste")]
        {
            let event = Event::Paste("".to_string());
            assert_eq!(event.as_paste_event(), Some(""));
            assert_eq!(event.as_key_event(), None);
        }
    }
}
