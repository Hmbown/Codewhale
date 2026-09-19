//! Keep the host from idling into sleep while a turn is in flight.
//!
//! A suspended host cannot run the engine, so nothing here survives a real
//! suspend — `core::engine::streaming::sleep_gap_detected` already reports that
//! case and the engine re-issues the request (issue #2990). What this module
//! prevents is the avoidable one: an unattended machine idling into sleep in
//! the middle of a turn, which is how a long turn gets lost with no error at
//! all.
//!
//! Scope, stated so nobody expects more than it does:
//!
//! - It holds the platform's *idle-sleep* assertion only. An explicit `sleep`/
//!   `pmset sleepnow`, a closed lid, or a low battery still wins — refusing
//!   those is the machine owner's call, not a running task's.
//! - It is held for the duration of a turn and released on drop, so the host's
//!   power behaviour outside a turn is untouched.
//! - It follows the same gate as the rest of the host-facing chrome
//!   (`EngineConfig::terminal_chrome_enabled`): an interactive TUI turn holds
//!   it, while headless hosts — `exec`, app-server, CI — never do. A dedicated
//!   `[tui]` opt-out key is not implemented yet; the headless gate is the
//!   escape hatch today.
//! - Windows is not implemented. `SetThreadExecutionState` is thread-affine —
//!   the release has to happen on the thread that set it, which a guard that
//!   travels with a turn cannot promise. Rather than ship an untested holder
//!   that might silently never release, this is a no-op there for now.
//!
//! Release is `Drop` and never cached: a leaked inhibitor would keep a laptop
//! awake forever, which is worse than the problem this solves.

use std::process::{Child, Command, Stdio};

/// An idle-sleep assertion held for as long as this value lives.
pub struct SleepGuard {
    /// The platform inhibitor process, when one was started. `None` means the
    /// platform has no implementation, or the process could not be started —
    /// keeping the host awake is best-effort and must never fail a turn.
    #[cfg(unix)]
    child: Option<Child>,
}

impl SleepGuard {
    /// Hold the host awake until the returned guard drops.
    #[must_use]
    pub fn hold() -> Self {
        #[cfg(unix)]
        {
            Self {
                child: start_inhibitor(),
            }
        }
        #[cfg(not(unix))]
        {
            Self {}
        }
    }

    /// The inhibitor's process id, for diagnostics and tests. Absent when the
    /// platform is a no-op or the process did not start.
    #[cfg(all(test, unix))]
    pub(crate) fn inhibitor_pid(&self) -> Option<u32> {
        self.child.as_ref().map(Child::id)
    }
}

#[cfg(unix)]
impl Drop for SleepGuard {
    fn drop(&mut self) {
        let Some(child) = self.child.as_mut() else {
            return;
        };
        // Killing the inhibitor is what releases the assertion; reaping it
        // keeps a zombie out of the process table.
        let _ = child.kill();
        let _ = child.wait();
    }
}

/// `-i` prevents idle sleep. Without `-t` caffeinate runs until it is killed,
/// which is what `Drop` does; macOS releases the assertion with the process.
#[cfg(target_os = "macos")]
fn start_inhibitor() -> Option<Child> {
    spawn("caffeinate", &["-i"])
}

/// `--what=idle` only: an explicit suspend or a closed lid is still honoured.
/// `sleep infinity` is the command whose lifetime holds the block open.
#[cfg(target_os = "linux")]
fn start_inhibitor() -> Option<Child> {
    spawn(
        "systemd-inhibit",
        &[
            "--what=idle",
            "--why=Codewhale turn in flight",
            "--mode=block",
            "sleep",
            "infinity",
        ],
    )
}

/// Everything else Unix (BSD, illumos, …) has no inhibitor this module knows.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "linux"))))]
fn start_inhibitor() -> Option<Child> {
    None
}

#[cfg(unix)]
fn spawn(program: &str, args: &[&str]) -> Option<Child> {
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    /// Whether the process is still there. `kill(pid, 0)` asks the kernel
    /// without touching the process, so this cannot perturb the guard.
    fn alive(pid: u32) -> bool {
        // SAFETY: signal 0 performs the permission/existence check only.
        unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
    }

    #[test]
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn the_inhibitor_lives_exactly_as_long_as_the_guard() {
        let guard = SleepGuard::hold();
        let pid = guard
            .inhibitor_pid()
            .expect("this platform starts an inhibitor");
        assert!(alive(pid), "the inhibitor must be running while held");

        drop(guard);

        // Reaping is synchronous in `Drop`, so the pid is gone immediately —
        // and if it were reused by a new process in this window the test would
        // be racing itself, which is why we assert on the guard's own child.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while alive(pid) {
            assert!(
                std::time::Instant::now() < deadline,
                "a released guard must not leave an inhibitor keeping the host awake"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[test]
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn holding_twice_holds_two_independent_inhibitors() {
        // Turns are serialized, but nothing here should assume it: two guards
        // must not share one process, or the first drop would release both.
        let first = SleepGuard::hold();
        let second = SleepGuard::hold();
        let (a, b) = (
            first.inhibitor_pid().expect("first inhibitor"),
            second.inhibitor_pid().expect("second inhibitor"),
        );
        assert_ne!(a, b, "each guard owns its own inhibitor process");
        drop(first);
        assert!(!alive(a), "the first guard released only its own");
        assert!(alive(b), "the second guard still holds the host awake");
    }
}
