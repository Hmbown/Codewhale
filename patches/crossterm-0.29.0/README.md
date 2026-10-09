<h1 align="center"><img width="440" src="docs/crossterm_full.png" /></h1>

[![Donate](https://img.shields.io/badge/Donate-PayPal-green.svg)](https://www.paypal.com/cgi-bin/webscr?cmd=_s-xclick&hosted_button_id=Z8QK6XU749JB2) ![Travis][s7] [![Latest Version][s1]][l1] [![MIT][s2]][l2] [![docs][s3]][l3] ![Lines of Code][s6] [![Join us on Discord][s5]][l5]

# Cross-platform Terminal Manipulation Library 

Crossterm is a pure-rust, terminal manipulation library that makes it possible to write cross-platform text-based interfaces (see [features](#features)). It supports all UNIX and Windows terminals down to Windows 7 (not all terminals are tested,
see [Tested Terminals](#tested-terminals) for more info).

## Table of Contents

- [Cross-platform Terminal Manipulation Library](#cross-platform-terminal-manipulation-library)
  - [Table of Contents](#table-of-contents)
  - [Features](#features)
    - [Tested Terminals](#tested-terminals)
  - [Getting Started](#getting-started)
    - [Feature Flags](#feature-flags)
    - [Dependency Justification](#dependency-justification)
    - [Other Resources](#other-resources)
  - [Used By](#used-by)
  - [Contributing](#contributing)
  - [Authors](#authors)
  - [License](#license)

## Features

- Cross-platform
- Multi-threaded (send, sync)
- Detailed documentation
- Few dependencies
- Full control over writing and flushing output buffer
- Is tty
- Cursor 
    - Move the cursor N times (up, down, left, right)
    - Move to previous / next line
    - Move to column
    - Set/get the cursor position
    - Store the cursor position and restore to it later
    - Hide/show the cursor
    - Enable/disable cursor blinking (not all terminals do support this feature)
- Styled output 
    - Foreground color (16 base colors)
    - Background color (16 base colors)
    - 256 (ANSI) color support (Windows 10 and UNIX only)
    - RGB color support (Windows 10 and UNIX only)
    - Text attributes like bold, italic, underscore, crossed, etc
- Terminal 
    - Clear (all lines, current line, from cursor down and up, until new line)
    - Scroll up, down
    - Set/get the terminal size
    - Exit current process
    - Alternate screen
    - Raw screen   
    - Set terminal title
    - Enable/disable line wrapping
- Event 
    - Input Events 
    - Mouse Events (press, release, position, button, drag)
    - Terminal Resize Events
    - Advanced modifier (SHIFT | ALT | CTRL) support for both mouse and key events and
    - futures Stream  (feature 'event-stream')
    - Poll/read API
    
<!--
WARNING: Do not change following heading title as it's used in the URL by other crates!
-->

### Tested Terminals

- Console Host
    - Windows 10 (Pro)
    - Windows 8.1 (N)
- Windows Terminal
    - Windows 10 x86_64 (Enterprise)
    - Windows 11 arm64 (Enterprise)
- Ubuntu Desktop Terminal
    - Ubuntu 23.04 64-bit
    - Ubuntu 17.10
    - Pop!_OS ( Ubuntu ) 20.04
- (Arch, Manjaro) KDE Konsole
- (Arch, NixOS) Kitty
- Linux Mint
- (OpenSuse) Alacritty
- (Chrome OS) Crostini
- Apple
    - macOS Monterey 12.7.1 (Intel-Chip)
    - macOS Sonama 14.4 (M1 Max, Apple Silicon-Chip)

This crate supports all UNIX terminals and Windows terminals down to Windows 7; however, not all of the
terminals have been tested. If you have used this library for a terminal other than the above list without
issues, then feel free to add it to the above list - I really would appreciate it!

## Getting Started
_see the [examples directory](examples/) and [documentation](https://docs.rs/crossterm/) for more advanced examples._

<details>
<summary>
Click to show Cargo.toml.
</summary>

```toml
[dependencies]
crossterm = "0.27"
```

</details>
<p></p>

```rust
use std::io::{stdout, Write};

use crossterm::{
    execute,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    ExecutableCommand,
    event,
};

fn main() -> std::io::Result<()> {
    // using the macro
    execute!(
        stdout(),
        SetForegroundColor(Color::Blue),
        SetBackgroundColor(Color::Red),
        Print("Styled text here."),
        ResetColor
    )?;

    // or using functions
    stdout()
        .execute(SetForegroundColor(Color::Blue))?
        .execute(SetBackgroundColor(Color::Red))?
        .execute(Print("Styled text here."))?
        .execute(ResetColor)?;
    
    Ok(())
}
```

Checkout this [list](https://docs.rs/crossterm/latest/crossterm/index.html#supported-commands) with all possible commands.

### Feature Flags

```toml
[dependencies.crossterm]
version = "0.27"
features = ["event-stream"] 
```

| Feature        | Description                                  |
|:---------------|:---------------------------------------------|
| `event-stream` | `futures::Stream` producing `Result<Event>`. |
| `serde`        | (De)serializing of events.                   |
| `events`        | Reading input/system events (enabled by default) |
| `filedescriptor` | Use raw filedescriptor for all events rather then mio dependency |
| `derive-more`  | Adds `is_*` helper functions for event types |
| `osc52`        | Enables crossterm::clipboard                 |


To use crossterm as a very thin layer you can disable the `events` feature or use `filedescriptor` feature. 
This can disable `mio` / `signal-hook` / `signal-hook-mio` dependencies.

### Dependency Justification

| Dependency     | Used for                                                                         | Included                              |
|:---------------|:---------------------------------------------------------------------------------|:--------------------------------------|
| `bitflags`     | `KeyModifiers`, those are differ based on input.                                 | always                                |
| `parking_lot`  | locking `RwLock`s with a timeout, const mutexes.                                 | always                                |
| `libc`         | UNIX terminal_size/raw modes/set_title and several other low level functionality. | optional (`events` feature), UNIX only |
| `Mio`          | event readiness polling, waking up poller                                        | optional (`events` feature), UNIX only |
| `signal-hook`  | signal-hook is used to handle terminal resize SIGNAL with Mio.                   |  optional (`events` feature),UNIX only |
| `winapi`       | Used for low-level windows system calls which ANSI codes can't replace           | windows only                          |
| `futures-core` | For async stream of events                                                       | only with `event-stream` feature flag |
| `serde`        | ***ser***ializing and ***de***serializing of events                              | only with `serde` feature flag        |
| `derive_more`  | Adds `is_*` helper functions for event types                                     | optional (`derive-more` feature), included by default |
| `base64`       | Encoding clipboard data for OSC52 sequences in crossterm::clipboard              | only with `osc52` feature flag        |

### Other Resources

- [API documentation](https://docs.rs/crossterm/)
- [Deprecated examples repository](https://github.com/crossterm-rs/examples)

## Used By

- [Broot](https://dystroy.org/broot/)
- [Cursive](https://github.com/gyscos/Cursive)
- [Ratatui](https://github.com/ratatui/ratatui)
- [Rust-sloth](https://github.com/ecumene/rust-sloth)
- [Rusty-rain](https://github.com/cowboy8625/rusty-rain)

## Contributing
  
We highly appreciate when anyone contributes to this crate. Before you do, please,
read the [Contributing](docs/CONTRIBUTING.md) guidelines. 

## Authors

* **Timon Post** - *Project Owner & creator*

## License

This project, `crossterm` and all its sub-crates: `crossterm_screen`, `crossterm_cursor`, `crossterm_style`,
`crossterm_input`, `crossterm_terminal`, `crossterm_winapi`, `crossterm_utils` are licensed under the MIT
License - see the [LICENSE](https://github.com/crossterm-rs/crossterm/blob/master/LICENSE) file for details.

[s1]: https://img.shields.io/crates/v/crossterm.svg
[l1]: https://crates.io/crates/crossterm

[s2]: https://img.shields.io/badge/license-MIT-blue.svg
[l2]: ./LICENSE

[s3]: https://docs.rs/crossterm/badge.svg
[l3]: https://docs.rs/crossterm/

[s3]: https://docs.rs/crossterm/badge.svg
[l3]: https://docs.rs/crossterm/

[s5]: https://img.shields.io/discord/560857607196377088.svg?logo=discord
[l5]: https://discord.gg/K4nyTDB

[s6]: https://tokei.rs/b1/github/crossterm-rs/crossterm?category=code
[s7]: https://travis-ci.org/crossterm-rs/crossterm.svg?branch=master
