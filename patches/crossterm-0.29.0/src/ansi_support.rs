use std::sync::atomic::{AtomicBool, Ordering};

use crossterm_winapi::{ConsoleMode, Handle};
use parking_lot::Once;
use winapi::um::wincon::ENABLE_VIRTUAL_TERMINAL_PROCESSING;

/// Enable virtual terminal processing.
///
/// This method attempts to enable virtual terminal processing for this
/// console. If there was a problem enabling it, then an error returned.
/// On success, the caller may assume that enabling it was successful.
///
/// When virtual terminal processing is enabled, characters emitted to the
/// console are parsed for VT100 and similar control character sequences
/// that control color and other similar operations.
fn enable_vt_processing() -> std::io::Result<()> {
    let mask = ENABLE_VIRTUAL_TERMINAL_PROCESSING;

    let console_mode = ConsoleMode::from(Handle::current_out_handle()?);
    let old_mode = console_mode.mode()?;

    if old_mode & mask == 0 {
        console_mode.set_mode(old_mode | mask)?;
    }

    Ok(())
}

static SUPPORTS_ANSI_ESCAPE_CODES: AtomicBool = AtomicBool::new(false);
static INITIALIZER: Once = Once::new();

/// Checks if the current terminal supports ANSI escape sequences
pub fn supports_ansi() -> bool {
    INITIALIZER.call_once(|| {
        // Some terminals on Windows like GitBash can't use WinAPI calls directly
        // so when we try to enable the ANSI-flag for Windows this won't work.
        // Because of that we should check first if the TERM-variable is set
        // and see if the current terminal is a terminal who does support ANSI.
        let supported = enable_vt_processing().is_ok()
            || std::env::var("TERM").map_or(false, |term| term != "dumb");

        SUPPORTS_ANSI_ESCAPE_CODES.store(supported, Ordering::SeqCst);
    });

    SUPPORTS_ANSI_ESCAPE_CODES.load(Ordering::SeqCst)
}
