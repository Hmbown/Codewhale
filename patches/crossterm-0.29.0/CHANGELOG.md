# Unreleased

# Version 0.29

## Added ⭐

- Copy to clipboard using OSC52 (#974)
- Derive standard traits for "SetCursorStyle" (#909)
- Add query_keyboard_enhancement_flags to read enabled flags (#958)
- Add is_* and as_* methods to the event enums (#949)
- Add a feature flag for derive_more impls (#970)
- Update rustix to 1.0 (#982)

## Breaking ⚠️

- Correctly fix KeyModifiers Display impl Properly adding + in between modifiers (#979)


# Version 0.28.1

## Fixed 🐛

- Fix broken build on linux when using `use-dev-tty` with (#906)

## Breaking ⚠️

- Fix desync with mio and signalhook between repo and published crate. (upgrade to mio 1.0)

# Version 0.28

## Added ⭐

- Capture double click mouse events on windows (#826)
- (De)serialize Reset color (#824)
- Add functions to allow constructing `Attributes` in a const context (#817)
- Implement `Display` for `KeyCode` and `KeyModifiers` (#862)

## Changed ⚙️

- Use Rustix by default instead of libc. Libc can be re-enabled if necessary with the `libc` feature flag (#892)
- `FileDesc` now requires a lifetime annotation.
- Improve available color detection (#885)
- Speed up `SetColors` by ~15-25% (#879)
- Remove unsafe and unnecessary size argument from `FileDesc::read()` (#821)

## Breaking ⚠️

- Fix duplicate bit masks for caps lock and num lock (#863).
  This breaks serialization of `KeyEventState`

# Version 0.27.1

## Added ⭐
- Add support for (de)serializing `Reset` `Color`

# Version 0.27

## Added ⭐

- Add `NO_COLOR` support (https://no-color.org/)
- Add option to force overwrite `NO_COLOR` (#802)
- Add support for scroll left/right events on windows and unix systems (#788).
- Add `window_size` function to fetch pixel width/height of screen for more sophisticated rendering in terminals.
- Add support for deserializing hex color strings to `Color` e.g #fffff.

## Changed ⚙️

- Make the events module an optional feature `events` (to make crossterm more lightweight) (#776)

## Breaking ⚠️

- Set minimum rustc version to 1.58 (#798)
- Change all error types to `std::io::Result` (#765)

# Version 0.26.1

## Added ⭐

- Add synchronized output/update control (#756)
- Add kitty report alternate keys functionality (#754)
- Updates dev dependencies.

## Fixed 🐛
- Fix icorrect return in kitty keyboard enhancement check (#751)
- Fix panic when using `use-dev-tty` feature flag (#762)

# Version 0.26.0
## Added ⭐

- Add `SetCursorStyle` to set the cursor apearance and visibility. (#742)
- Add a function to check if kitty keyboard enhancement protocol is available. (#732)
- Add filedescriptors poll in order to move away from mio in the future (can be used via `use-dev-tty`). (#735)

## Fixed 🐛
- Improved F1-F4 handling for kitty keyboard protocol. (#736)
- Improved parsing of event types/modifiers with certain keys for kitty protocol. (#716)

## Breaking ⚠️
- Remove `SetCursorShape` in favour of `SetCursorStyle`.  (#742)
- Make Windows resize event match `terminal::size` (#714)
- Rust 1.58 or later is now required.
- Add key release event for windows. (#745)

# Version 0.25.0
BREAKING: `Copy` trait is removed from `Event`, you can keep it by removing the "bracked-paste" feature flag. However this flag might be standardized in the future.
We removed the `Copy` from `Event` because the new `Paste` event, which contains a pasted string into the terminal, which is a non-copy string.

- Add ability to paste a string in into the terminal and fetch the pasted string via events (see `Event::Paste` and `EnableBracketedPaste `).
- Add support for functional key codes from kitty keyboard protocol. Try out by `PushKeyboardEnhancementFlags`. This protocol allows for:
  - See: https://sw.kovidgoyal.net/kitty/keyboard-protocol/#modifiers
  - Press, Repeat, Release event kinds.
  - SUPER, HYPER, META modifiers.
  - Media keycodes
  - Right/left SHIFT, Control, Alt, Super, Hyper, Meta
  - IsoLevel3Shift, IsoLevel5Shift
  - Capslock, scroll lock, numlock
  - Printscreen, pauze, menue, keyboard begin.
- Create `SetStyle` command to allow setting various styling in one command.
- Terminal Focus events (see `Event::FocusGained` and `Event::FocusLost`)

# Version 0.24.0
- Add  DoubleUnderlined, Undercurled, Underdots the text, Underdotted, Underdashes, Underdashed attributes and allow coloring their foreground / background color.
- Fix windows unicode character parsing, this fixed various key combinations and support typing unicode characters.
- Consistency and better documentation on mouse cursor operations (BREAKING CHANGE).
  - MoveTo, MoveToColumn, MoveToRow are 0-based. (left top most cell is 0,0). Moving like this is absolute
  - MoveToNextLine, MoveToPreviousLine, MoveUp, MoveDown, MoveRight, MoveLeft are 1-based,. Moving like this is relative. Moving 1 left means moving 1 left. Moving 0 to the left is not possible, wikipedia states that most terminals will just default to 1.
- terminal::size returns error when previously it returned (0,0).
- Remove println from serialisation code.
- Fix mouse up for middle and right buttons.
- Fix escape codes on Git-Bash + Windows Terminal / Alacritty / WezTerm.
- Add support for cursor keys in application mode.
# Version 0.23.2
- Update signal-hook and mio to version 0.8.

# Version 0.23.1
- Fix control key parsing problem.

# Version 0.23
- Update dependencies.
- Add 0 check for all cursor functions to prevent undefined behaviour.
- Add CSIu key parsing for unix.
- Improve control character window key parsing supporting (e.g. CTRL [ and ])
- Update library to 2021 edition.

# Version 0.22.1
- Update yanked version crossterm-winapi and move to  crossterm-winapi 0.9.0.
- Changed panic to error when calling disable-mouse capture without setting it first.
- Update bitflags dependency.

# Version 0.22
- Fix serde Color serialisation/deserialization inconsistency.
- Update crossterm-winapi 0.8.1 to fix panic for certain mouse events

# Version 0.21
- Expose `is_raw` function.
- Add 'purge' option on unix system, this clears the entire screen buffer.
- Improve serialisation for color enum values.

# Version 0.20
- Update from signal-hook with 'mio-feature flag' to signal-hook-mio 0.2.1.
- Manually implements Eq, PartialEq and Hash for KeyEvent improving equality checks and hash calculation.
- `crossterm::ErrorKind` to `io::Error`.
- Added Cursor Shape Support.
- Add support for function keys F13...F20.
- Support taking any Display in `SetTitle` command.
- Remove lazy_static dependency.
- Remove extra Clone bounds in the style module.
 - Add `MoveToRow` command.
 - Remove writer parameter from execute_winapi

# Version 0.19
- Use single thread for async event reader.
- Patch timeout handling for event polling this was not working correctly.
- Add unix support for more key combinations mainly complex ones with ALT/SHIFT/CTRL.
- Derive `PartialEq` and `Eq` for ContentStyle
- Fix windows resize event size, this used to be the buffer size but is screen size now.
- Change `Command::ansi_code` to `Command::write_ansi`, this way the ansi code will be written to given formatter.

# Version 0.18.2
- Fix panic when only setting bold and redirecting stdout.
- Use `tty_fd` for set/get terminal attributes

# Version 0.18.1
- Fix enabling ANSI support when stdout is redirected
- Update crossterm-winapi to 0.6.2

# Version 0.18.0
- Fix get position bug
- Fix windows 8 or lower write to user-given stdout instead of stdout.
- Make MoveCursor(Left/Right/Up/Dow) command with input 0 not move.
- Switch to futures-core to reduce dependencies.
- Command API restricts to only accept `std::io::Write`
- Make `supports_ansi` public
- Implement ALT + numbers windows systems.

# Version 0.17.7
- Fix cursor position retrieval bug linux.

# Version 0.17.6
- Add functionality to retrieve color based on passed ansi code.
- Switch from 'futures' to 'futures-util' crate to reduce dependency count
- Mio 0.7 update
- signal-hook update
- Make windows raw_mode act on CONIN$
- Added From<(u8, u8, u8)> Trait to Color::Rgb Enum
- Implement Color::try_from()
- Implement styler traits for `&'a str`

# Version 0.17.5
- Improved support of keymodifier for linux, arrow keys, function keys, home keys etc.
- Add `SetTitle` command to change the terminal title.
- Mio 0.7 update

# Version 0.17.4
- Add macros for `Colorize` and `Styler` impls, add an impl for `String`
- Add shift modifier to uppercase char events on unix

# Version 0.17.3
- Fix get terminal size mac os, this did not report the correct size.

# Version 0.17.2
- Windows unicode support

# Version 0.17.1
- Reverted bug in 0.17.0: "Make terminal size function fallback to `STDOUT_FILENO` if `/dev/tty` is missing.".
- Support for querying whether the current instance is a TTY.

# Version 0.17
- Impl Display for MoveToColumn, MoveToNextLine, MoveToPreviousLine
- Make unix event reader always use `/dev/tty`.
- Direct write command ansi_codes into formatter instead of double allocation.
- Add NONE flag to KeyModifiers
- Add support for converting chars to StylizedContent
- Make terminal size function fallback to `STDOUT_FILENO` if `/dev/tty` is missing.

# Version 0.16.0
- Change attribute vector in `ContentStyle` to bitmask.
- Add `SetAttributes` command.
- Add `Attributes` type, which is a bitfield of enabled attributes.
- Remove `exit()`, was useless.

# Version 0.15.0
- Fix CTRL + J key combination. This used to return an ENTER event.
- Add a generic implementation `Command` for `&T: Command`. This allows commands to be queued by reference, as well as by value.
- Remove unnecessary `Clone` trait bounds from `StyledContent`.
- Add `StyledContent::style_mut`.
- Handle error correctly for `execute!` and `queue!`.
- Fix minor syntax bug in `execute!` and `queue!`.
- Change `ContentStyle::apply` to take self by value instead of reference, to prevent an unnecessary extra clone.
- Added basic trait implementations (`Debug`, `Clone`, `Copy`, etc) to all of the command structs
- `ResetColor` uses `&'static str` instead of `String`

# Version 0.14.2
- Fix TIOCGWINSZ for FreeBSD

# Version 0.14.1
- Made windows cursor position relative to the window instead absolute to the screen buffer windows.
- Fix windows bug with `queue` macro were it consumed a type and required an type to be `Copy`.

# Version 0.14

- Replace the `input` module with brand new `event` module
    - Terminal Resize Events
    - Advanced modifier (SHIFT | ALT | CTRL) support for both mouse and key events and
    - futures Stream  (feature 'event-stream')
    - Poll/read API
    - It's **highly recommended** to read the
    [Upgrade from 0.13 to 0.14](https://github.com/crossterm-rs/crossterm/wiki/Upgrade-from-0.13-to-0.14)
        documentation
- Replace `docs/UPGRADE.md` with the [Upgrade Paths](https://github.com/crossterm-rs/crossterm/wiki#upgrade-paths)
  documentation
- Add `MoveToColumn`, `MoveToPreviousLine`, `MoveToNextLine` commands
- Merge `screen` module into `terminal`
    - Remove `screen::AlternateScreen`
    - Remove `screen::Rawscreen`
      * Move and rename `Rawscreen::into_raw_mode` and `Rawscreen::disable_raw_mode` to `terminal::enable_raw_mode` and `terminal::disable_raw_mode`
    - Move `screen::EnterAlternateScreen` and `screen::LeaveAlternateScreen` to `terminal::EnterAlternateScreen` and `terminal::LeaveAlternateScreen`
    - Replace `utils::Output` command with `style::Print` command
- Fix enable/disable mouse capture commands on Windows
- Allow trailing comma `queue!` & `execute!` macros

# Version 0.13.3

- Remove thread from AsyncReader on Windows.
- Improve HANDLE management windows.

# Version 0.13.2

- New `input::stop_reading_thread()` function
  - Temporary workaround for the UNIX platform to stop the background
    reading thread and close the file descriptor
  - This function will be removed in the next version

# Version 0.13.1

- Async Reader fix, join background thread and avoid looping forever on windows.

# Version 0.13.0

**Major API-change, removed old-api**

- Remove `Crossterm` type
- Remove `TerminalCursor`, `TerminalColor`, `Terminal`
- Remove `cursor()`, `color()` , `terminal()`
- Remove re-exports at root, accessible via `module::types` (`cursor::MoveTo`)
- `input` module
    - Derive 'Copy' for 'KeyEvent'
    - Add the `EnableMouseCapture` and `EnableMouseCapture` commands
- `cursor` module
    - Introduce static function `crossterm::cursor::position` in place of `TerminalCursor::pos`
    - Rename `Goto` to `MoveTo`
    - Rename `Up` to `MoveLeft`
    - Rename `Right` to `MoveRight`
    - Rename `Down` to `MoveDown`
    - Rename `BlinkOn` to `EnableBlinking`
    - Rename `BlinkOff` to `DisableBlinking`
    - Rename `ResetPos` to `ResetPosition`
    - Rename `SavePos` to `SavePosition`
- `terminal`
     - Introduce static function `crossterm::terminal::size` in place of `Terminal::size`
     - Introduce static function `crossterm::terminal::exit` in place of `Terminal::exit`
- `style module`
    - Rename `ObjectStyle` to `ContentStyle`. Now full names are used for methods
    - Rename `StyledObject` to `StyledContent` and made members private
    - Rename `PrintStyledFont` to `PrintStyledContent`
    - Rename `attr` method to `attribute`.
    - Rename `Attribute::NoInverse` to `NoReverse`
    - Update documentation
    - Made `Colored` private, user should use commands instead
    - Rename `SetFg` -> `SetForegroundColor`
    - Rename `SetBg` -> `SetBackgroundColor`
    - Rename `SetAttr` -> `SetAttribute`
    - Rename `ContentStyle::fg_color` -> `ContentStyle::foreground_color`
    - Rename `ContentStyle::bg_color` -> `ContentStyle::background_color`
    - Rename `ContentStyle::attrs` -> `ContentStyle::attributes`
- Improve documentation
- Unix terminal size calculation with TPUT

# Version 0.12.1

- Move all the `crossterm_` crates code was moved to the `crossterm` crate
  - `crossterm_cursor` is in the `cursor` module, etc.
  - All these modules are public
- No public API breaking changes

# Version 0.12.0

- Following crates are deprecated and no longer maintained
  - `crossterm_cursor`
  - `crossterm_input`
  - `crossterm_screen`
  - `crossterm_style`
  - `crossterm_terminal`
  - `crossterm_utils`

## `crossterm_cursor` 0.4.0

- Fix examples link ([PR #6](https://github.com/crossterm-rs/crossterm-cursor/pull/6))
- Sync documentation style ([PR #7](https://github.com/crossterm-rs/crossterm-cursor/pull/7))
- Remove all references to the crossterm book ([PR #8](https://github.com/crossterm-rs/crossterm-cursor/pull/8))
- Replace `RAW_MODE_ENABLED` with `is_raw_mode_enabled` ([PR #9](https://github.com/crossterm-rs/crossterm-cursor/pull/9))
- Use `SyncReader` & `InputEvent::CursorPosition` for `pos_raw()` ([PR #10](https://github.com/crossterm-rs/crossterm-cursor/pull/10))

## `crossterm_input` 0.5.0

- Sync documentation style ([PR #4](https://github.com/crossterm-rs/crossterm-input/pull/4))
- Sync `SyncReader::next()` Windows and UNIX behavior ([PR #5](https://github.com/crossterm-rs/crossterm-input/pull/5))
- Remove all references to the crossterm book ([PR #6](https://github.com/crossterm-rs/crossterm-input/pull/6))
- Mouse coordinates synchronized with the cursor ([PR #7](https://github.com/crossterm-rs/crossterm-input/pull/7))
  - Upper/left reported as `(0, 0)`
- Fix bug that read sync didn't block (Windows) ([PR #8](https://github.com/crossterm-rs/crossterm-input/pull/8))
- Refactor UNIX readers ([PR #9](https://github.com/crossterm-rs/crossterm-input/pull/9))
  - AsyncReader produces mouse events
  - One reading thread per application, not per `AsyncReader`
  - Cursor position no longer consumed by another `AsyncReader`
  - Implement sync reader for read_char (requires raw mode)
  - Fix `SIGTTIN` when executed under the LLDB
  - Add mio for reading from FD and more efficient polling (UNIX only)
- Sync UNIX and Windows vertical mouse position ([PR #11](https://github.com/crossterm-rs/crossterm-input/pull/11))
  - Top is always reported as `0`

## `crossterm_screen` 0.3.2

- `to_alternate` switch back to main screen if it fails to switch into raw mode ([PR #4](https://github.com/crossterm-rs/crossterm-screen/pull/4))
- Improve the documentation ([PR #5](https://github.com/crossterm-rs/crossterm-screen/pull/5))
  - Public API
  - Include the book content in the documentation
- Remove all references to the crossterm book ([PR #6](https://github.com/crossterm-rs/crossterm-screen/pull/6))
- New commands introduced ([PR #7](https://github.com/crossterm-rs/crossterm-screen/pull/7))
  - `EnterAlternateScreen`
  - `LeaveAlternateScreen`
- Sync Windows and UNIX raw mode behavior ([PR #8](https://github.com/crossterm-rs/crossterm-screen/pull/8))

## `crossterm_style` 0.5.2

- Refactor ([PR #2](https://github.com/crossterm-rs/crossterm-style/pull/2))
  - Added unit tests
  - Improved documentation and added book page to `lib.rs`
  - Fixed bug with `SetBg` command, WinApi logic
  - Fixed bug with `StyledObject`, used stdout for resetting terminal color
  - Introduced `ResetColor` command
- Sync documentation style ([PR #3](https://github.com/crossterm-rs/crossterm-style/pull/3))
- Remove all references to the crossterm book ([PR #4](https://github.com/crossterm-rs/crossterm-style/pull/4))
- Windows 7 grey/white foreground/intensity swapped ([PR #5](https://github.com/crossterm-rs/crossterm-style/pull/5))

## `crossterm_terminal` 0.3.2

- Removed `crossterm_cursor::sys` dependency ([PR #2](https://github.com/crossterm-rs/crossterm-terminal/pull/2))
- Internal refactoring & documentation ([PR #3](https://github.com/crossterm-rs/crossterm-terminal/pull/3))
- Removed all references to the crossterm book ([PR #4](https://github.com/crossterm-rs/crossterm-terminal/pull/4))

## `crossterm_utils` 0.4.0

- Add deprecation note ([PR #3](https://github.com/crossterm-rs/crossterm-utils/pull/3))
- Remove all references to the crossterm book ([PR #4](https://github.com/crossterm-rs/crossterm-utils/pull/4))
- Remove unsafe static mut ([PR #5](https://github.com/crossterm-rs/crossterm-utils/pull/5))
  - `sys::unix::RAW_MODE_ENABLED` replaced with `sys::unix::is_raw_mode_enabled()` (breaking)
  - New `lazy_static` dependency

## `crossterm_winapi` 0.3.0

- Make read sync block for windows systems ([PR #2](https://github.com/crossterm-rs/crossterm-winapi/pull/2))

# Version 0.11.1

- Maintenance release
- All sub-crates were moved to their own repositories in the `crossterm-rs` organization

# Version 0.11.0

As a preparation for crossterm 0.1.0 we have moved crossterm to an organisation called 'crossterm-rs'.

### Code Quality

- Code Cleanup: [warning-cleanup], [crossterm_style-cleanup], [crossterm_screen-cleanup], [crossterm_terminal-cleanup], [crossterm_utils-cleanup], [2018-cleanup], [api-cleanup-1], [api-cleanup-2], [api-cleanup-3]
- Examples: [example-cleanup_1], [example-cleanup_2], [example-fix], [commandbar-fix], [snake-game-improved]
- Fixed all broken tests and added tests

### Important Changes

- Return written bytes: [return-written-bytes]
- Added derives: `Debug` for `ObjectStyle`  [debug-derive], Serialize/Deserialize for key events [serde]
- Improved error handling:
    - Return `crossterm::Result` from all api's: [return_crossterm_result]
         * `TerminalCursor::pos()` returns `Result<(u16, u16)>`
         * `Terminal::size()` returns `Result<(u16, u16)>`
         * `TerminalCursor::move_*` returns `crossterm::Result`
         * `ExecutableCommand::queue` returns `crossterm::Result`
         * `QueueableCommand::queue` returns `crossterm::Result`
         * `get_available_color_count` returns no result
         * `RawScreen::into_raw_mode` returns `crossterm::Result` instead of `io::Result`
         * `RawScreen::disable_raw_mode` returns `crossterm::Result` instead of `io::Result`
         * `AlternateScreen::to_alternate` returns `crossterm::Result` instead of `io::Result`
         * `TerminalInput::read_line` returns `crossterm::Result` instead of `io::Result`
         * `TerminalInput::read_char` returns `crossterm::Result` instead of `io::Result`
         * Maybe I forgot something, a lot of functions have changed
     - Removed all unwraps/expects from library
- Add KeyEvent::Enter and KeyEvent::Tab: [added-key-event-enter], [added-key-event-tab]
- Sync set/get terminal size behaviour: [fixed-get-set-terminal-size]
- Method renames:
    * `AsyncReader::stop_reading()` to `stop()`
    * `RawScreen::disable_raw_mode_on_drop` to `keep_raw_mode_on_drop`
    * `TerminalCursor::reset_position()` to `restore_position()`
    * `Command::get_anis_code()` to `ansi_code()`
    * `available_color_count` to `available_color_count()`
    * `Terminal::terminal_size` to `Terminal::size`
    * `Console::get_handle` to `Console::handle`
- All `i16` values for indexing: set size, set cursor pos, scrolling synced to `u16` values
- Command API takes mutable self instead of self

[serde]: https://github.com/crossterm-rs/crossterm/pull/190

[debug-derive]: https://github.com/crossterm-rs/crossterm/pull/192
[example-fix]: https://github.com/crossterm-rs/crossterm/pull/193
[commandbar-fix]: https://github.com/crossterm-rs/crossterm/pull/204

[warning-cleanup]: https://github.com/crossterm-rs/crossterm/pull/198
[example-cleanup_1]: https://github.com/crossterm-rs/crossterm/pull/196
[example-cleanup_2]: https://github.com/crossterm-rs/crossterm/pull/225
[snake-game-improved]: https://github.com/crossterm-rs/crossterm/pull/231
[crossterm_style-cleanup]: https://github.com/crossterm-rs/crossterm/pull/208
[crossterm_screen-cleanup]: https://github.com/crossterm-rs/crossterm/pull/209
[crossterm_terminal-cleanup]: https://github.com/crossterm-rs/crossterm/pull/210
[crossterm_utils-cleanup]: https://github.com/crossterm-rs/crossterm/pull/211
[2018-cleanup]: https://github.com/crossterm-rs/crossterm/pull/222
[wild-card-cleanup]: https://github.com/crossterm-rs/crossterm/pull/224

[api-cleanup-1]: https://github.com/crossterm-rs/crossterm/pull/235
[api-cleanup-2]: https://github.com/crossterm-rs/crossterm/pull/238
[api-cleanup-3]: https://github.com/crossterm-rs/crossterm/pull/240

[return-written-bytes]: https://github.com/crossterm-rs/crossterm/pull/212

[return_crossterm_result]: https://github.com/crossterm-rs/crossterm/pull/232
[added-key-event-tab]: https://github.com/crossterm-rs/crossterm/pull/239
[added-key-event-enter]: https://github.com/crossterm-rs/crossterm/pull/236
[fixed-get-set-terminal-size]: https://github.com/crossterm-rs/crossterm/pull/242

# Version 0.10.1

# Version 0.10.0 ~ yanked
- Implement command API, to have better performance and more control over how and when commands are executed. [PR](https://github.com/crossterm-rs/crossterm/commit/1a60924abd462ab169b6706aab68f4cca31d7bc2), [issue](https://github.com/crossterm-rs/crossterm/issues/171)
- Fix showing, hiding cursor windows implementation
- Remove some of the parsing logic from windows keys to ansi codes to key events [PR](https://github.com/crossterm-rs/crossterm/commit/762c3a9b8e3d1fba87acde237f8ed09e74cd9ecd)
- Made terminal size 1-based [PR](https://github.com/crossterm-rs/crossterm/commit/d689d7e8ed46a335474b8262bd76f21feaaf0c50)
- Add some derives

# Version 0.9.6

- Copy for KeyEvent
- CTRL + Left, Down, Up, Right key support
- SHIFT + Left, Down, Up, Right key support
- Fixed UNIX cursor position bug [issue](https://github.com/crossterm-rs/crossterm/issues/140), [PR](https://github.com/crossterm-rs/crossterm/pull/152)

# Version 0.9.5

- Prefetch buffer size for more efficient windows input reads. [PR](https://github.com/crossterm-rs/crossterm/pull/144)

# Version 0.9.4

- Reset foreground and background color individually. [PR](https://github.com/crossterm-rs/crossterm/pull/138)
- Backtap input support. [PR](https://github.com/crossterm-rs/crossterm/pull/129)
- Corrected white/grey and added dark grey.
- Fixed getting cursor position with raw screen enabled. [PR](https://github.com/crossterm-rs/crossterm/pull/134)
- Removed one redundant stdout lock

# Version 0.9.3

- Removed println from `SyncReader`

## Version 0.9.2

- Terminal size linux was not 0-based
- Windows mouse input event position was 0-based and should be 1-based
- Result, ErrorKind are made re-exported
- Fixed some special key combination detections for UNIX systems
- Made FreeBSD compile

## Version 0.9.1

- Fixed libc compile error

## Version 0.9.0 (yanked)

This release is all about moving to a stabilized API for 1.0.

- Major refactor and cleanup.
- Improved performance;
    - No locking when writing to stdout.
    - UNIX doesn't have any dynamic dispatch anymore.
    - Windows has improved the way to check if ANSI modes are enabled.
    - Removed lot's of complex API calls: `from_screen`, `from_output`
    - Removed `Arc<TerminalOutput>` from all internal Api's.
- Removed termios dependency for UNIX systems.
- Upgraded deps.
- Removed about 1000 lines of code
    - `TerminalOutput`
    - `Screen`
    - unsafe code
    - Some duplicated code introduced by a previous refactor.
- Raw modes UNIX systems improved
- Added `NoItalic` attribute

## Version 0.8.2

- Bug fix for sync reader UNIX.

## Version 0.8.1

- Added public re-exports for input.

# Version 0.8.0

- Introduced KeyEvents
- Introduced MouseEvents
- Upgraded crossterm_winapi 0.2

# Version 0.7.0

- Introduced more `Attributes`
- Introduced easier ways to style text [issue 87](https://github.com/crossterm-rs/crossterm/issues/87).
- Removed `ColorType` since it was unnecessary.

# Version 0.6.0

- Introduced feature flags; input, cursor, style, terminal, screen.
- All modules are moved to their own crate.
- Introduced crossterm workspace
- Less dependencies.
- Improved namespaces.

[PR 84](https://github.com/crossterm-rs/crossterm/pull/84)

# Version 0.5.5

- Error module is made public [PR 78](https://github.com/crossterm-rs/crossterm/pull/78).

# Version 0.5.4

- WinApi rewrite and correctly error handled [PR 67](https://github.com/crossterm-rs/crossterm/pull/67)
- Windows attribute support [PR 62](https://github.com/crossterm-rs/crossterm/pull/62)
- Readline bug fix windows systems [PR 62](https://github.com/crossterm-rs/crossterm/pull/62)
- Error handling improvement.
- General refactoring, all warnings removed.
- Documentation improvement.

# Version 0.5.1

- Documentation refactor.
- Fixed broken API documentation [PR 53](https://github.com/crossterm-rs/crossterm/pull/53).

# Version 0.5.0

- Added ability to pause the terminal [issue](https://github.com/crossterm-rs/crossterm/issues/39)
- RGB support for Windows 10 systems
- ANSI color value (255) color support
- More convenient API, no need to care about `Screen` unless working with when working with alternate or raw screen [PR](https://github.com/crossterm-rs/crossterm/pull/44)
- Implemented Display for styled object

# Version 0.4.3

- Fixed bug [issue 41](https://github.com/crossterm-rs/crossterm/issues/41)

# Version 0.4.2

- Added functionality to make a styled object writable to screen [issue 33](https://github.com/crossterm-rs/crossterm/issues/33)
- Added unit tests.
- Bugfix with getting terminal size unix.
- Bugfix with returning written bytes [pull request 31](https://github.com/crossterm-rs/crossterm/pull/31)
- removed methods calls: `as_any()` and `as_any_mut()` from `TerminalOutput`

# Version 0.4.1

- Fixed resizing of ansi terminal with and height where in the wrong order.

# Version 0.4.0

- Input support (read_line, read_char, read_async, read_until_async)
- Styling module improved
- Everything is multithreaded (`Send`, `Sync`)
- Performance enhancements: removed mutexes, removed state manager, removed context type removed unnecessarily RC types.
- Bug fix resetting console color.
- Bug fix whit undoing raw modes.
- More correct error handling.
- Overall command improvement.
- Overall refactor of code.

# Version 0.3.0

This version has some braking changes check [upgrade manual](UPGRADE%20Manual.md) for more information about what is changed.
I think you should not switch to version `0.3.0` if you aren't going to use the AlternateScreen feature.
Because you will have some work to get to the new version of crossterm depending on your situation.

Some Features crossterm 0.3.0
- Alternate Screen for windows and unix systems.
- Raw screen for unix and windows systems [Issue 5](https://github.com/crossterm-rs/crossterm/issues/5)..
- Hiding an showing the cursor.
- Control over blinking of the terminal cursor (only some terminals are supporting this).
- The terminal state will be set to its original state when process ends [issue7](https://github.com/crossterm-rs/crossterm/issues/7).
- exit the current process.

## Alternate screen

This create supports alternate screen for both windows and unix systems. You can use

*Nix style applications often utilize an alternate screen buffer, so that they can modify the entire contents of the buffer, without affecting the application that started them.
The alternate buffer is exactly the dimensions of the window, without any scrollback region.
For an example of this behavior, consider when vim is launched from bash.
Vim uses the entirety of the screen to edit the file, then returning to bash leaves the original buffer unchanged.

I Highly recommend you to check the `examples/program_examples/first_depth_search` for seeing this in action.

## Raw screen

This crate now supports raw screen for both windows and unix systems.
What exactly is raw state:
- No line buffering.
   Normally the terminals uses line buffering. This means that the input will be send to the terminal line by line.
   With raw mode the input will be send one byte at a time.
- Input
  All input has to be written manually by the programmer.
- Characters
  The characters are not processed by the terminal driver, but are sent straight through.
  Special character have no meaning, like backspace will not be interpret as backspace but instead will be directly send to the terminal.
With these modes you can easier design the terminal screen.

## Some functionalities added

- Hiding and showing terminal cursor
- Enable or disabling blinking of the cursor for unix systems (this is not widely supported)
- Restoring the terminal to original modes.
- Added a [wrapper](https://github.com/crossterm-rs/crossterm/blob/master/src/shared/crossterm.rs) for managing all the functionalities of crossterm `Crossterm`.
- Exit the current running process

## Examples
Added [examples](https://github.com/crossterm-rs/crossterm/tree/master/examples) for each version of the crossterm version.
Also added a folder with some [real life examples](https://github.com/crossterm-rs/crossterm/tree/master/examples/program_examples).

## Context

What is the `Context`  all about? This `Context` has several reasons why it is introduced into `crossterm version 0.3.0`.
These points are related to the features like `Alternatescreen` and managing the terminal state.

- At first `Terminal state`:

    Because this is a terminal manipulating library there will be made changes to terminal when running an process.
    If you stop the process you want the terminal back in its original state.
    Therefore, I need to track the changes made to the terminal.

- At second `Handle to the console`

    In Rust we can use `stdout()` to get an handle to the current default console handle.
    For example when in unix systems you want to print something to the main screen you can use the following code:

        write!(std::io::stdout(), "{}", "some text").

    But things change when we are in alternate screen modes.
    We can not simply use `stdout()` to get a handle to the alternate screen, since this call returns the current default console handle (handle to mainscreen).

    Because of that we need to store an handle to the current screen.
    This handle could be used to put into alternate screen modes and back into main screen modes.
    Through this stored handle Crossterm can execute its command and write on and to the current screen whether it be alternate screen or main screen.

    For unix systems we store the handle gotten from `stdout()` for windows systems that are not supporting ANSI escape codes we store WinApi `HANDLE` struct witch will provide access to the current screen.

So to recap this `Context` struct is a wrapper for a type that manges terminal state changes.
When this `Context` goes out of scope all changes made will be undone.
Also is this `Context` is a wrapper for access to the current console screen.

Because Crossterm needs access to the above to types quite often I have chosen to add those two in one struct called `Context` so that this type could be shared throughout library.
Check this link for more info: [cleanup of rust code](https://stackoverflow.com/questions/48732387/how-can-i-run-clean-up-code-in-a-rust-library).
More info over writing to alternate screen buffer on windows and unix see this [link](https://github.com/crossterm-rs/crossterm/issues/17)

__Now the user has to pass an context type to the modules of Crossterm like this:__

      let context = Context::new();

      let cursor = cursor(&context);
      let terminal = terminal(&context);
      let color = color(&context);

Because this looks a little odd I will provide a type widths will manage the `Context` for you. You can call the different modules like the following:

      let crossterm = Crossterm::new();
      let color = crossterm.color();
      let cursor = crossterm.cursor();
      let terminal = crossterm.terminal();


### Alternate screen
When you want to switch to alternate screen there are a couple of things to keep in mind for it to work correctly.
First off some code of how to switch to Alternate screen, for more info check the [alternate screen example](https://github.com/crossterm-rs/crossterm/blob/master/examples/alternate_screen.rs).

_Create alternate screen from `Context`_

        // create context.
        let context = crossterm::Context::new();
        // create instance of Alternatescreen by the given context, this will also switch to it.
        let mut screen = crossterm::AlternateScreen::from(context.clone());
        // write to the alternate screen.
        write!(screen,  "test");

_Create alternate screen from `Crossterm`:_

        // create context.
        let crossterm = ::crossterm::Crossterm::new();
        // create instance of Alternatescreen by the given reference to crossterm, this will also switch to it.
        let mut screen = crossterm::AlternateScreen::from(&crossterm);
        // write to the alternate screen.
        write!(screen,  "test");

like demonstrated above, to get the functionalities of `cursor(), color(), terminal()` also working on alternate screen.
You need to pass it the same `Context` as you have passed to the previous three called functions,
If you don't use the same `Context` in `cursor(), color(), terminal()` than these modules will be using the main screen and you will not see anything at the alternate screen. If you use the [Crossterm](https://github.com/crossterm-rs/crossterm/blob/master/src/shared/crossterm.rs) type you can get the `Context` from it by calling the crossterm.get_context() whereafter you can create the AlternateScreen from it.

# Version 0.2.2

- Bug see [issue 15](https://github.com/crossterm-rs/crossterm/issues/15)

# Version 0.2.1

- Default ANSI escape codes for windows machines, if windows does not support ANSI switch back to WinApi.
- method grammar mistake fixed [Issue 3](https://github.com/crossterm-rs/crossterm/issues/3)
- Some Refactorings in method names see [issue 4](https://github.com/crossterm-rs/crossterm/issues/4)
- Removed bin reference from crate [Issue 6](https://github.com/crossterm-rs/crossterm/issues/6)
- Get position unix fixed [issue 8](https://github.com/crossterm-rs/crossterm/issues/8)

# Version 0.2

- 256 color support.
- Text Attributes like: bold, italic, underscore and crossed word etc.
- Custom ANSI color code input to set fore- and background color for unix.
- Storing the current cursor position and resetting to that stored cursor position later.
- Resizing the terminal.
