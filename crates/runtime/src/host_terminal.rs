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

use codewhale_config::notifications::NotificationsConfig;

/// Terminal side effects the runtime may request from its host.
pub trait HostTerminal: Send + Sync {
    /// Leave raw mode if it is on, so an interactive child process sees a
    /// cooked terminal. Returns whether raw mode was on (and must be resumed).
    fn suspend_raw_mode(&self) -> bool;

    /// Re-enter raw mode after [`HostTerminal::suspend_raw_mode`] returned
    /// `true`.
    fn resume_raw_mode(&self);

    /// Deliver a model-authored notification (the `notify` tool) now,
    /// honoring the installed notification settings. Returns the delivery
    /// receipt the tool reports to the model.
    fn notify_model(&self, title: &str, body: Option<&str>) -> &'static str;

    /// Record whether the terminal has focus; attention policy reads it.
    fn set_terminal_focused(&self, focused: bool);

    /// Install the process-wide notification method, category gate, sound
    /// policy and attention condition from `[notifications]`.
    fn apply_notification_settings(&self, config: &NotificationsConfig);
}

struct NoHostTerminal;

impl HostTerminal for NoHostTerminal {
    fn suspend_raw_mode(&self) -> bool {
        false
    }

    fn resume_raw_mode(&self) {}

    fn notify_model(&self, _title: &str, _body: Option<&str>) -> &'static str {
        "notification not sent: no terminal host"
    }

    fn set_terminal_focused(&self, _focused: bool) {}

    fn apply_notification_settings(&self, _config: &NotificationsConfig) {}
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
    /// The host that suspended raw mode resumes it.
    host: &'static dyn HostTerminal,
    resume: bool,
}

/// Leave raw mode around an interactive child; resume it when the returned
/// guard drops, only if it was on to begin with.
pub fn suspend_raw_mode() -> RawModeSuspension {
    suspend_raw_mode_on(host())
}

fn suspend_raw_mode_on(host: &'static dyn HostTerminal) -> RawModeSuspension {
    RawModeSuspension {
        host,
        resume: host.suspend_raw_mode(),
    }
}

impl Drop for RawModeSuspension {
    fn drop(&mut self) {
        if self.resume {
            self.host.resume_raw_mode();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    /// Records raw-mode calls. Each test owns its own `static` instance, so
    /// they never share counters.
    struct CountingHost {
        raw: AtomicBool,
        suspends: AtomicUsize,
        resumes: AtomicUsize,
    }

    impl CountingHost {
        const fn new(raw: bool) -> Self {
            Self {
                raw: AtomicBool::new(raw),
                suspends: AtomicUsize::new(0),
                resumes: AtomicUsize::new(0),
            }
        }
    }

    impl HostTerminal for CountingHost {
        fn suspend_raw_mode(&self) -> bool {
            self.suspends.fetch_add(1, Ordering::SeqCst);
            self.raw.swap(false, Ordering::SeqCst)
        }

        fn resume_raw_mode(&self) {
            self.resumes.fetch_add(1, Ordering::SeqCst);
            self.raw.store(true, Ordering::SeqCst);
        }

        fn notify_model(&self, _title: &str, _body: Option<&str>) -> &'static str {
            "unused"
        }

        fn set_terminal_focused(&self, _focused: bool) {}

        fn apply_notification_settings(&self, _config: &NotificationsConfig) {}
    }

    #[test]
    fn no_host_suspension_is_a_no_op() {
        // No host is installed in this test binary: suspending reports raw
        // mode off, so the guard never asks to resume it.
        let guard = suspend_raw_mode();
        assert!(!guard.resume);
    }

    #[test]
    fn raw_mode_that_was_on_is_resumed_exactly_once_when_the_guard_drops() {
        static HOST: CountingHost = CountingHost::new(true);
        let guard = suspend_raw_mode_on(&HOST);
        assert!(
            !HOST.raw.load(Ordering::SeqCst),
            "raw mode is off while suspended"
        );
        assert_eq!(HOST.resumes.load(Ordering::SeqCst), 0);
        drop(guard);
        assert!(
            HOST.raw.load(Ordering::SeqCst),
            "raw mode is back on after the guard"
        );
        assert_eq!(HOST.suspends.load(Ordering::SeqCst), 1);
        assert_eq!(HOST.resumes.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn raw_mode_that_was_off_is_never_turned_on_by_the_guard() {
        static HOST: CountingHost = CountingHost::new(false);
        drop(suspend_raw_mode_on(&HOST));
        assert_eq!(HOST.suspends.load(Ordering::SeqCst), 1);
        assert_eq!(HOST.resumes.load(Ordering::SeqCst), 0);
        assert!(!HOST.raw.load(Ordering::SeqCst));
    }
}
