//! The one port through which runtime code causes terminal side effects.
//!
//! The terminal UI is the only owner of the terminal: it alone toggles raw
//! mode and writes escape sequences. Runtime code (tools, the engine, the
//! runtime API) never links a terminal library; when it needs a terminal
//! effect it asks the installed [`HostTerminal`]. The composition root
//! installs the TUI's implementation once at startup, for every host it
//! launches today (interactive TUI, `exec`, the runtime API server), so
//! behavior is unchanged from when the calls were inline.
//!
//! With nothing installed every method is a no-op. That is the intended
//! future shape for stdio hosts (ACP, MCP server, app-server), whose stdout is
//! a JSON-RPC stream that must never receive terminal bytes.
//!
//! Known limitations: the install is process-global and first-wins; a
//! process cannot swap hosts after startup, and tests that need the real
//! terminal behavior must go through the TUI's own implementation.

use std::sync::OnceLock;

/// Terminal side effects the runtime may request from its host.
pub trait HostTerminal: Send + Sync {
    /// Leave raw mode if it is on, so an interactive child process sees a
    /// cooked terminal. Returns whether raw mode was on (and must be resumed).
    fn suspend_raw_mode(&self) -> bool;

    /// Re-enter raw mode after [`HostTerminal::suspend_raw_mode`] returned
    /// `true`.
    fn resume_raw_mode(&self);
}

struct NoHostTerminal;

impl HostTerminal for NoHostTerminal {
    fn suspend_raw_mode(&self) -> bool {
        false
    }

    fn resume_raw_mode(&self) {}
}

static HOST: OnceLock<Box<dyn HostTerminal>> = OnceLock::new();

/// Install the process's terminal host. The first install wins; later calls
/// are ignored and return `false`.
pub fn install(host: Box<dyn HostTerminal>) -> bool {
    HOST.set(host).is_ok()
}

/// The installed terminal host, or a no-op host when none is installed.
#[must_use]
pub fn host() -> &'static dyn HostTerminal {
    match HOST.get() {
        Some(host) => host.as_ref(),
        None => &NoHostTerminal,
    }
}

/// Raw mode suspended for the lifetime of this guard (issue #1690).
///
/// Created by [`suspend_raw_mode`]; dropping it resumes raw mode only if it
/// was on when the guard was created.
#[must_use = "raw mode is resumed when the guard drops"]
pub struct RawModeSuspension {
    resume: bool,
}

/// Leave raw mode around an interactive child; resume it when the returned
/// guard drops, only if it was on to begin with.
pub fn suspend_raw_mode() -> RawModeSuspension {
    RawModeSuspension {
        resume: host().suspend_raw_mode(),
    }
}

impl Drop for RawModeSuspension {
    fn drop(&mut self) {
        if self.resume {
            host().resume_raw_mode();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_host_suspension_is_a_no_op() {
        // No host is installed in this test binary: suspending reports raw
        // mode off, so the guard never asks to resume it.
        let guard = suspend_raw_mode();
        assert!(!guard.resume);
    }
}
