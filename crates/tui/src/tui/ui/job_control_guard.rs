//! Job-control suspend/resume handshake for the TUI (#6169).
//!
//! A full-screen TUI that never handles job control poisons the shell it was
//! started from: once the process group reads a controlling tty it does not
//! own, the kernel stops it (SIGTTIN — or the user stops it with SIGTSTP) with
//! every mode it enabled still active — raw mode, mouse reporting, bracketed
//! paste, the alternate screen — and whatever shell is in the foreground is
//! then fed raw SGR/CUP escape fragments. The startup check
//! `terminal::require_foreground_terminal_owner` guards exactly one moment;
//! this module is the runtime half of the same contract: restore on stop,
//! rebuild on continue, like vim/less/htop.
//!
//! Shape (mirrors `fatal_signal_guard`, deliberately narrow):
//!
//! - **One handler serves both SIGTSTP and SIGTTIN.** The SIGTTIN case is the
//!   one that bites in #6169: a handler that *returns* turns the background
//!   read into `EIO`, which the input pump reports as a dead tty (the forbidden
//!   EIO spin). A handler that never returns cannot: it always finishes with
//!   `raise(SIGSTOP)`, which is uncatchable and unblockable.
//! - **The handler does exactly three things**, all async-signal-safe: write
//!   the fixed restore bytes from `fatal_signal_guard::FATAL_RESTORE_BYTES`
//!   (one byte table serves both process death and suspension — no second
//!   table), `tcsetattr(TCSANOW)` from the cooked snapshot taken at install
//!   time, and stop. **No crossterm call**: crossterm's raw-mode state lives
//!   behind a mutex that the stopped input-pump thread may be holding, so
//!   `disable_raw_mode()` here can deadlock inside a handler (and the process
//!   is unstoppable while it does). No `tracing`, no allocation, no lock.
//! - **The SIGCONT handler is a single atomic store.** Re-entering modes and
//!   repainting happen on the event-loop thread in normal context, where
//!   crossterm is safe to call.
//! - **The resume action is skipped** while another owner has the terminal (the
//!   child-handoff pause) or while this process group is still in the
//!   background (a plain `bg`): re-entering raw mode and the alternate screen
//!   then would steal the shell's tty.
//!
//! Out of scope by design (see the issue's maintainer discussion): a
//! `tcgetpgrp` pre-poll guard and the `restart_detached` liveness lie. Both are
//! check-then-act, and neither can close the race — the background `read(2)`
//! itself is the atomic foreground test.
//!
//! Not recoverable: SIGKILL while stopped (no handler runs), exactly as with
//! the fatal guard. Kill-switch: `CODEWHALE_DISABLE_JOB_CONTROL_GUARD=1`.

#[cfg(unix)]
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

#[cfg(unix)]
use super::fatal_signal_guard::FATAL_RESTORE_BYTES;

/// The teardown the stop handler writes. Deliberately the fatal guard's byte
/// string: one table, so a mode added to one restore path cannot be missing
/// from the other.
#[cfg(unix)]
pub(crate) const SUSPEND_RESTORE_BYTES: &[u8] = FATAL_RESTORE_BYTES;

/// A stop handler has run (and the process has been stopped by SIGSTOP).
const STOPPED_UNDER_HANDLER: u8 = 0b01;
/// SIGCONT arrived after such a stop: the terminal must be rebuilt.
const CONT_SEEN: u8 = 0b10;
/// Both bits: a handler-driven suspend is waiting for its resume repaint.
const PENDING_RESUME: u8 = STOPPED_UNDER_HANDLER | CONT_SEEN;

/// Suspend handshake state. Only ever touched by a single atomic op per site,
/// so the signal handlers stay async-signal-safe.
static SUSPEND_STATE: AtomicU8 = AtomicU8::new(0);

/// True once the restore bytes have been written for the current suspend
/// cycle. Cleared by [`mark_resumed`], so each new suspend restores again.
/// Guards both the byte write and the `tcsetattr` — repeated stops inside one
/// suspend cycle (SIGTTIN, SIGCONT, SIGTTIN again while still backgrounded)
/// must not repeat either.
static RESTORED: AtomicBool = AtomicBool::new(false);

/// Cooked termios snapshot taken at install time, before `enable_raw_mode`.
///
/// A `OnceLock` read from a signal handler is safe here for the same reason it
/// is in `fatal_signal_guard`: the write happens once, on the main thread,
/// before any worker exists, and is never written again.
#[cfg(unix)]
static ORIGINAL_TERMIOS: OnceLock<libc::termios> = OnceLock::new();

/// Install the job-control guard. POSIX only; no-op elsewhere.
///
/// Call once on the main thread, after the foreground-ownership check (the
/// termios snapshot below needs the still-cooked tty) and **before**
/// `enable_raw_mode()` — so every mode the TUI goes on to enable has a handler
/// that can undo it.
pub(crate) fn install_job_control_guard() {
    #[cfg(unix)]
    {
        if std::env::var("CODEWHALE_DISABLE_JOB_CONTROL_GUARD")
            .is_ok_and(|value| value == "1" || value == "true")
        {
            tracing::debug!("Job-control guard disabled by CODEWHALE_DISABLE_JOB_CONTROL_GUARD");
            return;
        }
        // Piped/embedded surfaces must never receive escape bytes, and have no
        // job control to participate in.
        if unsafe { libc::isatty(libc::STDOUT_FILENO) } == 0 {
            tracing::debug!("Job-control guard skipped: stdout is not a TTY");
            return;
        }
        // Snapshot the cooked attributes now. In the handler this is the only
        // way back: crossterm's own raw-mode teardown is behind a lock.
        let mut original: libc::termios = unsafe { std::mem::zeroed() };
        // SAFETY: `original` is a fully owned, properly sized `termios`.
        if unsafe { libc::tcgetattr(libc::STDIN_FILENO, &mut original) } != 0 {
            tracing::warn!(
                "Job-control guard: tcgetattr(stdin) failed; terminal attributes will not be restored on suspend"
            );
        } else {
            let _ = ORIGINAL_TERMIOS.set(original);
        }
        for signal in [libc::SIGTSTP, libc::SIGTTIN] {
            // SAFETY: `signal` is a stop-class signal whose default action is
            // "stop"; the handler is async-signal-safe by construction.
            unsafe { install_stop_handler(signal) };
        }
        // SAFETY: SIGCONT's default action is to continue, which we replace.
        unsafe { install_continue_handler() };
        // The SIGTTIN path runs our stop handler while the group is in the
        // BACKGROUND, and `tcsetattr` from a background group raises SIGTTOU
        // (default action: stop) — the process would stop inside the handler
        // before `raise(SIGSTOP)`, and the first `fg` would resume the rest of
        // the handler and immediately stop again. Ignoring SIGTTOU is the
        // standard full-screen-program disposition (vim does the same) and is
        // the only way the background restore can complete. This is set in the
        // installer (normal context), never in a handler.
        unsafe { libc::signal(libc::SIGTTOU, libc::SIG_IGN) };
        tracing::debug!(
            "Job-control guard installed (TSTP/TTIN -> restore + SIGSTOP, CONT -> resume)"
        );
    }
    #[cfg(not(unix))]
    {
        // Windows consoles have no job control; the console-mode cleanup path
        // in `terminal.rs` covers the equivalent "mode leak" surface.
    }
}

/// True while a handler-driven suspend is waiting for its resume repaint.
///
/// This is the event loop's first check each iteration. Deliberately a *peek*,
/// not a take: the state is only cleared by [`mark_resumed`] once the terminal
/// has actually been rebuilt. Consuming it here would lose the resume when the
/// action has to be deferred (a child owns the tty, or we are still a
/// background process group) — and a resume that is lost is a clobbered screen.
pub(crate) fn take_resume() -> bool {
    SUSPEND_STATE.load(Ordering::Acquire) & PENDING_RESUME == PENDING_RESUME
}

/// Acknowledge a completed resume: the next suspend restores from scratch.
///
/// Narrow `fetch_and` rather than a plain store so bits set by a handler racing
/// with this call are not erased outright. A suspend landing inside the
/// load/acknowledge window can at worst lose one repaint; it can never lose the
/// stop, nor leak a mode, because the handler restores *before* stopping.
pub(crate) fn mark_resumed() {
    let _ = SUSPEND_STATE.fetch_and(!PENDING_RESUME, Ordering::AcqRel);
    RESTORED.store(false, Ordering::Release);
}

/// Write the restore bytes, then `tcsetattr`, then stop — nothing else.
///
/// # Safety
///
/// Async-signal-safe by construction: `write(2)`, `tcsetattr(3)`, `raise(2)`
/// and two atomic flag updates on plain `static`s. No crossterm, no `tracing`,
/// no allocation, no lock, no `OnceLock` *initialization* (only a read of one
/// that was filled before any thread existed).
#[cfg(unix)]
unsafe extern "C" fn stop_handler(_signal: libc::c_int) {
    unsafe {
        // One restore per suspend cycle, whatever order the stops arrive in.
        if !RESTORED.swap(true, Ordering::AcqRel) {
            let mut written: usize = 0;
            while written < SUSPEND_RESTORE_BYTES.len() {
                let n = libc::write(
                    libc::STDOUT_FILENO,
                    SUSPEND_RESTORE_BYTES.as_ptr().add(written) as *const libc::c_void,
                    SUSPEND_RESTORE_BYTES.len() - written,
                );
                if n <= 0 {
                    break;
                }
                written += n as usize;
            }
            if written == 0 {
                let _ = libc::write(
                    libc::STDERR_FILENO,
                    SUSPEND_RESTORE_BYTES.as_ptr() as *const libc::c_void,
                    SUSPEND_RESTORE_BYTES.len(),
                );
            }
            // TCSANOW: never TCSADRAIN/TCSAFLUSH, which can block on output a
            // stopped peer will never drain.
            if let Some(original) = ORIGINAL_TERMIOS.get() {
                let _ = libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, original);
            }
        }

        // Record that the stop is ours, then stop for real. SIGSTOP cannot be
        // caught or blocked, so the handler always ends here: SIGTTIN is never
        // allowed to return into the pump as `EIO`.
        SUSPEND_STATE.fetch_or(STOPPED_UNDER_HANDLER, Ordering::Release);
        libc::raise(libc::SIGSTOP);
    }
}

/// One atomic store. Nothing else — every mode change happens on the event loop
/// thread, in normal context.
#[cfg(unix)]
unsafe extern "C" fn continue_handler(_signal: libc::c_int) {
    SUSPEND_STATE.fetch_or(CONT_SEEN, Ordering::Release);
}

/// Install [`stop_handler`] for one stop-class signal via `sigaction`.
///
/// # Safety
///
/// `signal` must be a signal whose default action is "stop".
#[cfg(unix)]
unsafe fn install_stop_handler(signal: libc::c_int) {
    unsafe {
        // Zero the whole struct then set our two fields; the remaining members
        // (empty signal mask, per-OS plumbing) are exactly what a zeroed
        // default means, and the wrapper fills in what it owns. SA_RESTART
        // keeps the input pump's interrupted read restarted after resume.
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = stop_handler as *const () as libc::sighandler_t;
        action.sa_flags = libc::SA_RESTART;
        if libc::sigaction(signal, &action, std::ptr::null_mut()) != 0 {
            tracing::warn!(signal, "job-control guard install failed");
        }
    }
}

/// # Safety
///
/// Installed only from [`install_job_control_guard`], on the main thread.
#[cfg(unix)]
unsafe fn install_continue_handler() {
    unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = continue_handler as *const () as libc::sighandler_t;
        action.sa_flags = libc::SA_RESTART;
        if libc::sigaction(libc::SIGCONT, &action, std::ptr::null_mut()) != 0 {
            tracing::warn!(signal = libc::SIGCONT, "job-control guard install failed");
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    /// Serializes the state-machine test against any other test that might poke
    /// the same statics. There is exactly one such test today; the lock keeps
    /// that true if a second one is added.
    static STATE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn job_control_restore_bytes_are_the_fatal_guard_table() {
        // One byte table for death and suspension: if this ever forks into a
        // second table, a mode can be restored on one path and leaked on the
        // other.
        assert_eq!(SUSPEND_RESTORE_BYTES, FATAL_RESTORE_BYTES);
        // The teardown the suspend path cannot do without.
        let bytes = String::from_utf8_lossy(SUSPEND_RESTORE_BYTES).to_string();
        for mode in [
            "?1000l", // mouse tracking
            "?1002l", // button-event mouse tracking
            "?1003l", // any-motion mouse tracking
            "?1006l", // SGR mouse encoding
            "?2004l", // bracketed paste
            "?1049l", // alternate screen
        ] {
            assert!(
                bytes.contains(mode),
                "suspend restore must reset {mode}; got: {bytes:?}"
            );
        }
    }

    #[test]
    fn job_control_state_bits_are_disjoint() {
        // The resume test is a mask compare; aliasing bits would make a stop
        // with no SIGCONT look resumable.
        assert_eq!(STOPPED_UNDER_HANDLER & CONT_SEEN, 0);
        assert_eq!(PENDING_RESUME, STOPPED_UNDER_HANDLER | CONT_SEEN);
    }

    #[test]
    fn job_control_state_machine_is_ordered_and_idempotent() {
        let _guard = STATE_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        SUSPEND_STATE.store(0, Ordering::Release);
        RESTORED.store(false, Ordering::Release);

        // Nothing suspended: no resume.
        assert!(!take_resume());
        mark_resumed();
        assert!(!take_resume());

        // A bare SIGCONT (somebody else's `kill -CONT`) is not a resume: no
        // handler-driven stop is on record.
        SUSPEND_STATE.fetch_or(CONT_SEEN, Ordering::Release);
        assert!(!take_resume());
        mark_resumed();
        assert!(!take_resume());

        // The real handshake: the stop handler records the stop, SIGCONT
        // records the continue, the loop sees both.
        SUSPEND_STATE.fetch_or(STOPPED_UNDER_HANDLER, Ordering::Release);
        assert!(!take_resume(), "stopped without SIGCONT is not resumable");
        SUSPEND_STATE.fetch_or(CONT_SEEN, Ordering::Release);
        assert!(take_resume());
        // Peeking is idempotent: the action may have to be deferred, so the
        // state must survive until it actually runs.
        assert!(take_resume());

        // Acknowledging resumes the cycle: no stale resume remains.
        mark_resumed();
        assert!(!take_resume());
        assert!(!RESTORED.load(Ordering::Acquire));
        mark_resumed();
        assert!(!take_resume());
    }
}
