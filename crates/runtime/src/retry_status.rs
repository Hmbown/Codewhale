//! Process-wide retry-state surface (#499).
//!
//! Read-side caveat (0.9.4): the renderer this module was written for was
//! the legacy footer's retry banner, which went with `FooterWidget`. The
//! *producer* — `client::send_with_retry` — is still live and still records
//! every retry, and `client`'s own tests read it back through `snapshot`.
//! The read surface (`snapshot`, the countdown, the banner fields) is
//! therefore test-gated (`cfg(any(test, feature = "test-support"))` where
//! possible, so the TUI's tests reach it through the `test-support`
//! feature; `cfg_attr(not(test))` allows where prod constructs); a renderer
//! restores it by dropping the gates. Give the banner a renderer, or
//! delete the producer too — but not half of it.
//!
//! The HTTP retry path in `client::send_with_retry` already times its
//! waits and knows the error category. This module gives the TUI a way
//! to observe that state — `start`, `succeeded`, and `failed` flip a
//! global `RetryState` that the footer / status panel reads each frame.
//!
//! Why a process-wide global: the user-facing TUI runs as one engine
//! per process, and the only retry state we want to surface is the one
//! the user is staring at. Sub-agent retries in background tasks
//! deliberately do **not** light up the foreground banner — they're
//! supposed to be invisible. If a future feature ever needs per-engine
//! retry surfaces, swap this for an `Arc<RwLock<...>>` carried on the
//! `EngineHandle`; the public API stays the same.

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// One in-flight retry attempt. `deadline` is the wall-clock time the
/// next request will fire — the UI subtracts `Instant::now()` from it
/// to render a live countdown.
#[derive(Debug, Clone)]
#[cfg_attr(not(test), allow(dead_code))]
pub struct RetryBanner {
    /// 1-indexed retry attempt number (the first retry is attempt 1).
    pub attempt: u32,
    /// Time at which the next request will be sent.
    pub deadline: Instant,
    /// Short human-readable reason ("rate limited", "server error", …).
    pub reason: String,
}

/// Snapshot of the retry surface for the UI to render.
#[derive(Debug, Clone, Default)]
pub enum RetryState {
    /// No retry in flight. Banner hidden.
    #[default]
    Idle,
    /// A request is sleeping before retrying. Show countdown banner.
    Active(#[cfg_attr(not(test), allow(dead_code))] RetryBanner),
    /// All retries exhausted; show failure row until the next turn
    /// starts. `since` records when the row was set so a future polish
    /// pass can age it out automatically; today the engine clears it on
    /// `TurnStarted`.
    Failed { reason: String, since: Instant },
}

impl RetryState {
    /// Wall-clock seconds remaining on the active banner, or `None` if
    /// not active. Saturates at zero — the renderer should treat any
    /// negative remaining as "firing now".
    #[cfg(any(test, feature = "test-support"))]
    #[must_use]
    pub fn seconds_remaining(&self) -> Option<u64> {
        match self {
            Self::Active(banner) => Some(
                banner
                    .deadline
                    .saturating_duration_since(Instant::now())
                    .as_secs(),
            ),
            _ => None,
        }
    }

    /// Whether the failure row should still be shown. Mirrors the
    /// "until next turn" rule in the issue spec; the engine clears it
    /// explicitly via [`clear`] on `TurnStarted`.
    #[cfg(any(test, feature = "test-support"))]
    #[must_use]
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }
}

/// Lazy-init the cell on first read so callers don't have to initialize
/// process-wide state at boot.
#[cfg(not(any(test, all(feature = "test-thread-scoped-state", debug_assertions))))]
fn with_state<R>(f: impl FnOnce(&mut RetryState) -> R) -> R {
    static STATE: OnceLock<Mutex<RetryState>> = OnceLock::new();
    let mut state = STATE
        .get_or_init(|| Mutex::new(RetryState::Idle))
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    f(&mut state)
}

#[cfg(not(any(test, all(feature = "test-thread-scoped-state", debug_assertions))))]
fn with_rate_limit<R>(f: impl FnOnce(&mut Option<Instant>) -> R) -> R {
    static STATE: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
    let mut state = STATE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    f(&mut state)
}

/// Under test, this state is per-thread.
///
/// Production has exactly one foreground engine per process, so a global is the
/// right model there. The test harness does not: retry state is written as a
/// side effect of *any* client request, by production code that has no test
/// guard to take, so a real request in one test could publish a banner or a
/// provider pause into another test's assertions. Scoping by thread removes the
/// race at its source rather than asking every future test that happens to
/// perform HTTP to remember a lock.
///
/// This changes behavior (a `Retry-After` pause no longer holds across tokio
/// worker threads), so it is *not* part of `test-support`, which only adds
/// helpers. The TUI's tests opt in with `test-thread-scoped-state`, and even
/// then only a build with debug assertions gets it: an optimized build, e.g.
/// `cargo build --release --workspace --all-features`, keeps the process-wide
/// pause. Cargo unifies features across one build, so under
/// `cargo test --workspace` (or a debug `--all-features` build) the debug
/// `codewhale` binary that integration tests spawn gets the per-thread pause
/// too; release and optimized binaries never do.
#[cfg(any(test, all(feature = "test-thread-scoped-state", debug_assertions)))]
fn with_state<R>(f: impl FnOnce(&mut RetryState) -> R) -> R {
    #[allow(clippy::type_complexity)]
    static STATE: OnceLock<Mutex<std::collections::HashMap<std::thread::ThreadId, RetryState>>> =
        OnceLock::new();
    let mut by_thread = STATE
        .get_or_init(|| Mutex::new(std::collections::HashMap::new()))
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    f(by_thread
        .entry(std::thread::current().id())
        .or_insert(RetryState::Idle))
}

#[cfg(any(test, all(feature = "test-thread-scoped-state", debug_assertions)))]
fn with_rate_limit<R>(f: impl FnOnce(&mut Option<Instant>) -> R) -> R {
    #[allow(clippy::type_complexity)]
    static STATE: OnceLock<
        Mutex<std::collections::HashMap<std::thread::ThreadId, Option<Instant>>>,
    > = OnceLock::new();
    let mut by_thread = STATE
        .get_or_init(|| Mutex::new(std::collections::HashMap::new()))
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    f(by_thread.entry(std::thread::current().id()).or_default())
}

/// Read snapshot for renderers. No production renderer exists since the
/// legacy footer went away; `client` retry tests are the only readers.
#[cfg(any(test, feature = "test-support"))]
#[must_use]
pub fn snapshot() -> RetryState {
    with_state(|state| state.clone())
}

/// Extend the provider-wide rate-limit pause window. This is separate from
/// the footer banner so one successful concurrent request cannot clear another
/// request's active `Retry-After` window.
pub fn note_rate_limit(delay: Duration) {
    let deadline = Instant::now() + delay;
    with_rate_limit(|current| {
        if current.is_none_or(|existing| existing < deadline) {
            *current = Some(deadline);
        }
    });
}

/// Remaining provider-wide rate-limit pause, if any.
#[must_use]
pub fn rate_limit_remaining() -> Option<Duration> {
    let now = Instant::now();
    with_rate_limit(|current| match *current {
        Some(deadline) if deadline > now => Some(deadline.duration_since(now)),
        Some(_) => {
            *current = None;
            None
        }
        None => None,
    })
}

/// Mark an in-flight retry. `attempt` is the number of the *upcoming*
/// retry (1 for the first); `delay` is how long the client will sleep
/// before firing.
pub fn start(attempt: u32, delay: Duration, reason: impl Into<String>) {
    let banner = RetryBanner {
        attempt,
        deadline: Instant::now() + delay,
        reason: reason.into(),
    };
    with_state(|state| *state = RetryState::Active(banner));
}

/// Mark the retry chain as having succeeded. Hides the banner.
pub fn succeeded() {
    with_state(|state| *state = RetryState::Idle);
}

/// Mark the retry chain as having exhausted retries. The renderer keeps
/// the failure row until [`clear`] (typically called on `TurnStarted`).
pub fn failed(reason: impl Into<String>) {
    with_state(|state| {
        *state = RetryState::Failed {
            reason: reason.into(),
            since: Instant::now(),
        };
    });
}

/// Reset to idle. Called on `TurnStarted` so the previous turn's
/// failure row doesn't bleed into the next turn.
pub fn clear() {
    with_state(|state| *state = RetryState::Idle);
}

#[cfg(any(test, feature = "test-support"))]
pub fn clear_rate_limit() {
    with_rate_limit(|current| *current = None);
}

/// Test helper: serialize tests that touch the global state so cargo's
/// parallel runner can't observe a torn read. The guard is exported so
/// tests in *other* modules (e.g. footer rendering tests) can hold the
/// same lock as the ones in `retry_status::tests`.
#[cfg(any(test, feature = "test-support"))]
pub fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: Mutex<()> = Mutex::new(());
    GUARD.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Acquire the cross-module test guard from [`super::test_guard`] and
    /// reset state to `Idle` before yielding to the test body.
    fn setup() -> std::sync::MutexGuard<'static, ()> {
        let g = test_guard();
        clear();
        clear_rate_limit();
        g
    }

    #[test]
    fn idle_by_default_after_clear() {
        let _g = setup();
        assert!(matches!(snapshot(), RetryState::Idle));
        assert_eq!(snapshot().seconds_remaining(), None);
    }

    #[test]
    fn start_then_succeeded_returns_to_idle() {
        let _g = setup();
        start(1, Duration::from_secs(5), "rate limited");
        let s = snapshot();
        assert!(matches!(s, RetryState::Active(_)));
        let remaining = s.seconds_remaining().unwrap();
        assert!(remaining <= 5, "{remaining}");
        succeeded();
        assert!(matches!(snapshot(), RetryState::Idle));
    }

    #[test]
    fn failed_persists_until_clear() {
        let _g = setup();
        failed("upstream 500");
        let s = snapshot();
        assert!(s.is_failed());
        if let RetryState::Failed { reason, .. } = s {
            assert_eq!(reason, "upstream 500");
        } else {
            panic!("expected Failed");
        }
        clear();
        assert!(matches!(snapshot(), RetryState::Idle));
    }

    #[test]
    fn deadline_in_past_yields_zero_remaining() {
        let _g = setup();
        // Bypass `start` so we can plant a deadline already in the past.
        with_state(|state| {
            *state = RetryState::Active(RetryBanner {
                attempt: 2,
                deadline: Instant::now() - Duration::from_secs(1),
                reason: "test".into(),
            });
        });
        assert_eq!(snapshot().seconds_remaining(), Some(0));
        clear();
    }

    #[test]
    fn rate_limit_deadline_survives_banner_clear() {
        let _g = setup();
        note_rate_limit(Duration::from_secs(5));
        start(1, Duration::from_secs(5), "rate limited");
        succeeded();
        assert!(
            rate_limit_remaining().is_some(),
            "provider-wide rate limit pause must not be cleared by an unrelated success"
        );
        clear_rate_limit();
    }
}
