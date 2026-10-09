//! Shared test-only helpers.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) use crate::shell_dispatcher::test_env_lock::{
    EnvScopeMembership, EnvScopeTicket, TestEnvLock, current_env_scope_generation,
    current_thread_holds_test_env_lock, env_scope_ticket, join_env_scope, lock_test_env,
    with_test_env_lock,
};

/// Process-wide state root for unit tests that do not intentionally provide an
/// explicit config/settings path.
///
/// The production fallback is the user's real home. That is useful at runtime
/// and unsafe in a parallel test binary: an unguarded save can otherwise read
/// or overwrite the developer's config. Tests that exercise path precedence
/// still hold [`lock_test_env`] and provide explicit temporary environment
/// values; every other test is confined here — enforced by
/// [`guarded_environment_provides_state_paths`], not assumed.
pub(crate) fn isolated_test_state_root() -> &'static Path {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "codewhale-tui-test-state-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap_or_else(|error| {
            panic!(
                "failed to create isolated unit-test state root {}: {error}",
                root.display()
            )
        });
        // Match resolvers that canonicalize their root (macOS aliases /var to
        // /private/var). Every fence must name the same physical test store.
        root.canonicalize().expect("canonical test state root")
    })
}

/// Where the calling test's state should live when it has not sealed the
/// environment itself.
///
/// Two different callers land here. A test that never took [`lock_test_env`]
/// gets the shared root, exactly as before — those tests already coexist there
/// under [`with_test_state_io_lock`]. A test that *holds* the lock but sealed
/// nothing gets a private directory instead: before #5359 it resolved the
/// developer's real home, so it has never shared the process root, and several
/// such tests run full settings transactions. Adding that traffic to the shared
/// root pushed the transaction lock past its deadline and hung unrelated
/// `config_command_*` tests. Keep them isolated from the developer *and* from
/// each other.
pub(crate) fn unsealed_test_state_root() -> PathBuf {
    let shared = isolated_test_state_root();
    if !current_thread_holds_test_env_lock() {
        return shared.to_path_buf();
    }
    // libtest runs each test in a fresh thread. Keep one root for that thread:
    // a settings save resolves its path more than once, while different tests
    // must not inherit each other's files.
    HOLDER_ROOT.with(|cached| {
        cached
            .get_or_init(|| {
                let root = shared.join(format!("env-holder-{:?}", std::thread::current().id()));
                std::fs::create_dir_all(&root).unwrap_or_else(|error| {
                    panic!(
                        "failed to create per-holder test state root {}: {error}",
                        root.display()
                    )
                });
                root
            })
            .clone()
    })
}

thread_local! {
    static HOLDER_ROOT: OnceLock<PathBuf> = const { OnceLock::new() };
}

/// Where a state resolver must put `name` instead of the user's home: the
/// isolated test root when no test has sealed the home (see [`home_is_sealed`]),
/// `None` when one has and the resolver should follow the environment as
/// production does.
///
/// Every resolver that reaches `~/.codewhale` and is reachable from a test
/// needs this fence, because incidental callers (an `App` constructor, a
/// command dispatch) never think about the home at all. Call it first, under
/// `#[cfg(test)]`, so the production path stays byte-for-byte unchanged.
///
/// The directory is under the shared isolated root, not the per-holder one
/// [`unsealed_test_state_root`] hands to lock holders: these resolvers are
/// reached from worker threads as well as the test thread, and a test must see
/// one directory from both. The directory exists when this returns, so callers
/// need no filesystem call of their own on the fenced branch.
pub(crate) fn unsealed_state_dir(name: impl AsRef<Path>) -> Option<PathBuf> {
    if home_is_sealed() {
        return None;
    }
    let dir = isolated_test_state_root().join(name);
    std::fs::create_dir_all(&dir).unwrap_or_else(|error| {
        panic!(
            "failed to create the unsealed test state dir {}: {error}",
            dir.display()
        )
    });
    Some(dir)
}

/// A fixture workspace an OS sandbox can still see (#6305).
///
/// The Linux bwrap wrapper mounts a fresh `--tmpfs /tmp` before it binds the
/// policy's writable roots (`sandbox/bwrap.rs`), and an enforced read-only
/// command has no writable roots at all — nothing re-exposes the host `/tmp`.
/// A fixture rooted there is shadowed inside the sandbox, so the trailing
/// `--chdir <workspace>` lands on a path that no longer exists and bwrap exits
/// with `Can't chdir to /tmp/.tmpXXXXXX`. The tmpfs is deliberate isolation and
/// a security boundary, so the fixture moves instead of the mount.
///
/// Known limitations: this relocates only the directory the sandboxed command
/// chdirs into. It does not make anything else under the host `/tmp` reachable
/// from inside the sandbox, and it says nothing about `/dev` or `/proc`, which
/// bwrap also replaces. Use it for the sandbox probes; plain `tempfile::tempdir`
/// stays correct everywhere else.
// Every caller is `#[cfg(unix)]` (the bwrap probes); on Windows these would
// be dead code and CI builds tests with `-Dwarnings`.
#[cfg(unix)]
pub(crate) fn sandbox_visible_tempdir() -> tempfile::TempDir {
    let root = sandbox_visible_fixture_root();
    tempfile::tempdir_in(root).unwrap_or_else(|error| {
        panic!(
            "failed to create a sandbox-visible fixture workspace in {}: {error}",
            root.display()
        )
    })
}

/// `OUT_DIR` is this crate's own build directory, so it follows the Cargo
/// target directory rather than `TMPDIR` — the one path every test binary
/// already owns and that is outside `/tmp` in every normal layout. A target
/// directory deliberately placed under `/tmp` would silently reintroduce
/// #6305, so say so instead of handing back a shadowed path.
#[cfg(unix)]
fn sandbox_visible_fixture_root() -> &'static Path {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        let root = Path::new(env!("OUT_DIR")).join("sandbox-fixtures");
        assert!(
            !root.starts_with("/tmp"),
            "sandbox fixtures need a root outside /tmp, which bwrap replaces with a fresh \
             tmpfs: {} is under it. Point CARGO_TARGET_DIR somewhere else.",
            root.display()
        );
        std::fs::create_dir_all(&root).unwrap_or_else(|error| {
            panic!(
                "failed to create the sandbox fixture root {}: {error}",
                root.display()
            )
        });
        root
    })
}

/// Build a syntactically valid, non-secret JWT fixture without embedding a
/// high-entropy token-shaped literal in Git history.
pub(crate) fn future_test_jwt(label: &str) -> String {
    use base64::Engine as _;

    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(br#"{"exp":9999999999}"#);
    format!("test.{payload}.{label}")
}

fn state_io_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Serialize read/merge/write operations against the process-wide isolated
/// test state root.
///
/// Path isolation protects the developer's files, but parallel tests still
/// share the same temporary files. Settings persistence is a multi-step
/// operation, so it needs this second barrier around the complete I/O
/// transaction rather than only around path resolution.
pub(crate) fn with_test_state_io_lock<T>(operation: impl FnOnce() -> T) -> T {
    let _guard = match state_io_lock().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    operation()
}

/// Build a test phase's future inside this call and box it (#6362).
///
/// In debug builds every inline `async {}` value gets a stack slot in the
/// enclosing poll frame the size of that future's whole state machine, and
/// the slots are never reused, so a body that awaits four phases inline
/// carries all four state machines on its own frame at once (measured at
/// 806 KiB for the runtime-store binding test). Constructing the phase here
/// leaves the caller holding a pointer, and the phase's own temporaries die
/// with its poll frame.
pub(crate) fn boxed_phase<'a, T, M, F>(
    make: M,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = T> + 'a>>
where
    M: FnOnce() -> F,
    F: std::future::Future<Output = T> + 'a,
{
    Box::pin(make())
}

/// Drive a test future on a thread with libtest's default 2 MiB stack,
/// whatever `RUST_MIN_STACK` says (#6362).
///
/// CI exports a 16 MiB `RUST_MIN_STACK` for every test thread, so a test
/// that only fits because of that export never learns it overflowed the
/// stack a contributor's plain `cargo test` gives it. The future is built on
/// the spawned thread (so it need not be `Send`) and pinned before
/// `block_on`, exactly as `#[tokio::test]` drives a current-thread runtime;
/// a panic inside propagates to the caller unchanged. An overflow still
/// aborts the process with "has overflowed its stack": that is the reported
/// symptom, not something this helper can turn into a panic.
pub(crate) fn block_on_default_test_stack<M, F, T>(make: M) -> T
where
    M: FnOnce() -> F + Send + 'static,
    F: std::future::Future<Output = T>,
    T: Send + 'static,
{
    const DEFAULT_TEST_THREAD_STACK: usize = 2 * 1024 * 1024;
    std::thread::Builder::new()
        .name("default-test-stack".into())
        .stack_size(DEFAULT_TEST_THREAD_STACK)
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("current-thread test runtime");
            let future = make();
            tokio::pin!(future);
            runtime.block_on(future)
        })
        .expect("spawn the default-stack test thread")
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
}

/// Restore one environment variable when dropped.
///
/// Callers that mutate process-global environment variables must hold
/// [`lock_test_env`] until after this guard is dropped.
///
/// Every live guard is also recorded in [`guarded_env_keys`], so path
/// resolution can distinguish a test that deliberately redirected `HOME`
/// from one that merely holds the lock to serialize unrelated env access —
/// see [`guarded_environment_provides_state_paths`].
pub(crate) struct EnvVarGuard {
    key: &'static str,
    previous: Option<OsString>,
}

fn guarded_env_keys() -> &'static Mutex<std::collections::HashMap<&'static str, usize>> {
    static KEYS: OnceLock<Mutex<std::collections::HashMap<&'static str, usize>>> = OnceLock::new();
    KEYS.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

fn register_guarded_env_key(key: &'static str) {
    let mut keys = match guarded_env_keys().lock() {
        Ok(keys) => keys,
        Err(poisoned) => poisoned.into_inner(),
    };
    *keys.entry(key).or_insert(0) += 1;
}

fn unregister_guarded_env_key(key: &'static str) {
    let mut keys = match guarded_env_keys().lock() {
        Ok(keys) => keys,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(count) = keys.get_mut(key) {
        *count -= 1;
        if *count == 0 {
            keys.remove(key);
        }
    }
}

/// Whether some live [`EnvVarGuard`] currently covers `key`.
pub(crate) fn env_var_currently_guarded(key: &str) -> bool {
    match guarded_env_keys().lock() {
        Ok(keys) => keys.contains_key(key),
        Err(poisoned) => poisoned.into_inner().contains_key(key),
    }
}

/// Whether the calling test actually provided the state-path environment it
/// is about to resolve.
///
/// Holding [`lock_test_env`] alone is not that: many tests hold the lock only
/// to serialize access to unrelated variables (`TERM_PROGRAM`, API keys) and
/// have provided no temporary paths at all. Trusting the lock routed those
/// tests to the developer's real `~/.codewhale` state, which is exactly the
/// leak the isolated root exists to prevent (#5359). A test earns environment
/// resolution by holding the lock *and* either setting one of the explicit
/// override variables or redirecting `HOME`/`USERPROFILE` through
/// [`EnvVarGuard`].
pub(crate) fn guarded_environment_provides_state_paths() -> bool {
    if !current_thread_holds_test_env_lock() {
        return false;
    }
    let guarded_path_is_present = |var: &str| {
        env_var_currently_guarded(var)
            && std::env::var_os(var)
                .is_some_and(|value| value.to_str().is_none_or(|text| !text.trim().is_empty()))
    };
    [
        "CODEWHALE_HOME",
        "CODEWHALE_CONFIG_PATH",
        "DEEPSEEK_CONFIG_PATH",
        "HOME",
        "USERPROFILE",
    ]
    .iter()
    .any(|var| guarded_path_is_present(var))
}

/// Whether the calling test's live seal pins the user's *home* — the root that
/// user state (sessions, snapshots, plugin bundles, logs) resolves under — as
/// opposed to merely redirecting a config file.
///
/// The owner and workers enrolled through [`join_env_scope`] follow the seal.
/// Unrelated parallel tests and workers without that scope use the isolated
/// root: following another test's environment would race its restoration to
/// the developer's real profile. Pass [`env_scope_ticket`] to workers that need
/// their owner's home.
///
/// Stricter than [`guarded_environment_provides_state_paths`], which also
/// accepts a guarded config-path override: such a test still resolves
/// `~/.codewhale` for everything else. `CODEWHALE_HOME` outranks `HOME` when it
/// is set, so a guarded `HOME` seals nothing while an unguarded
/// `CODEWHALE_HOME` is in the ambient environment.
pub(crate) fn home_is_sealed() -> bool {
    if !current_thread_holds_test_env_lock() {
        return false;
    }
    let present = |var: &str| {
        std::env::var_os(var)
            .is_some_and(|value| value.to_str().is_none_or(|text| !text.trim().is_empty()))
    };
    if present("CODEWHALE_HOME") {
        return env_var_currently_guarded("CODEWHALE_HOME");
    }
    ["HOME", "USERPROFILE"]
        .iter()
        .any(|var| env_var_currently_guarded(var) && present(var))
}

impl EnvVarGuard {
    pub(crate) fn set(key: &'static str, value: impl AsRef<OsStr>) -> Self {
        debug_assert!(
            current_thread_holds_test_env_lock(),
            "EnvVarGuard::set({key}) requires lock_test_env()"
        );
        let previous = std::env::var_os(key);
        // SAFETY: callers hold the process-wide test env mutex.
        unsafe { std::env::set_var(key, value) };
        register_guarded_env_key(key);
        Self { key, previous }
    }

    pub(crate) fn remove(key: &'static str) -> Self {
        debug_assert!(
            current_thread_holds_test_env_lock(),
            "EnvVarGuard::remove({key}) requires lock_test_env()"
        );
        let previous = std::env::var_os(key);
        // SAFETY: callers hold the process-wide test env mutex.
        unsafe { std::env::remove_var(key) };
        register_guarded_env_key(key);
        Self { key, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        // Withdraw the claim *before* restoring the value: a reader on another
        // thread that still saw the key as guarded would otherwise follow the
        // restored — ambient, possibly real — path for the instant in between.
        unregister_guarded_env_key(self.key);
        // SAFETY: callers hold the process-wide test env mutex until after this
        // guard is dropped.
        unsafe {
            if let Some(value) = self.previous.take() {
                std::env::set_var(self.key, value);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }
}

/// Seal the user's home onto a directory the test owns, for the life of one
/// test.
///
/// This is the one way a test takes the user's state out of play. It pins
/// `HOME`, `USERPROFILE` and `CODEWHALE_HOME` through [`EnvVarGuard`] — so
/// [`guarded_environment_provides_state_paths`] recognises the seal — and
/// removes the aliases that would otherwise route around them
/// (`HOMEDRIVE`/`HOMEPATH`, `CODEWHALE_CONFIG_PATH`, `DEEPSEEK_CONFIG_PATH`,
/// `DEEPSEEK_HOME`). Pinning `HOME` alone is not a seal: an ambient
/// `CODEWHALE_HOME` takes precedence over it, so a test that "scoped" only
/// `HOME` still wrote the developer's real profile.
///
/// It holds the process-wide environment lock, which is not reentrant: keep
/// one seal per test, and build any fixture that takes the lock itself
/// (settings, diagnostics harnesses) after it. Anything that reads or writes
/// the user's state — instructions, skills, sessions, snapshots, settings,
/// credentials — then lands in the seal.
pub(crate) struct SealedHome {
    // Drop order matters: restore the environment, then delete the directory,
    // then release the lock.
    _vars: Vec<EnvVarGuard>,
    _dir: Option<tempfile::TempDir>,
    home: PathBuf,
    codewhale_home: PathBuf,
    _lock: TestEnvLock,
}

impl SealedHome {
    /// Seal onto fresh, empty directories: `<tmp>/home` and
    /// `<tmp>/codewhale-home`.
    pub(crate) fn new() -> Self {
        let lock = lock_test_env();
        let dir = tempfile::TempDir::new().expect("sealed home tempdir");
        let home = dir.path().join("home");
        let codewhale_home = dir.path().join("codewhale-home");
        std::fs::create_dir_all(&home).expect("sealed home dir");
        std::fs::create_dir_all(&codewhale_home).expect("sealed codewhale home dir");
        Self::pin(lock, Some(dir), home, codewhale_home)
    }

    /// Seal onto a home the test already owns: `HOME` is `home` and the
    /// Codewhale home is `home/.codewhale`, the layout an unset
    /// `CODEWHALE_HOME` resolves to. For tests whose fixtures are built under
    /// one tempdir that also plays the home.
    pub(crate) fn at(home: &Path) -> Self {
        let lock = lock_test_env();
        let codewhale_home = home.join(".codewhale");
        Self::pin(lock, None, home.to_path_buf(), codewhale_home)
    }

    fn pin(
        lock: TestEnvLock,
        dir: Option<tempfile::TempDir>,
        home: PathBuf,
        codewhale_home: PathBuf,
    ) -> Self {
        // `CODEWHALE_HOME` first: it is dropped first, so the seal never
        // outlives the override that outranks `HOME`.
        let vars = vec![
            EnvVarGuard::set("CODEWHALE_HOME", &codewhale_home),
            EnvVarGuard::set("HOME", &home),
            EnvVarGuard::set("USERPROFILE", &home),
            EnvVarGuard::remove("HOMEDRIVE"),
            EnvVarGuard::remove("HOMEPATH"),
            EnvVarGuard::remove("CODEWHALE_CONFIG_PATH"),
            EnvVarGuard::remove("DEEPSEEK_CONFIG_PATH"),
            EnvVarGuard::remove("DEEPSEEK_HOME"),
        ];
        Self {
            _vars: vars,
            _dir: dir,
            home,
            codewhale_home,
            _lock: lock,
        }
    }

    /// The sealed `HOME` / `USERPROFILE`.
    pub(crate) fn home(&self) -> &Path {
        &self.home
    }

    /// The sealed `CODEWHALE_HOME`.
    pub(crate) fn codewhale_home(&self) -> &Path {
        &self.codewhale_home
    }
}

/// Find the byte position of the first divergence between two strings,
/// returning a windowed view (`±32 bytes` around the divergence) so failures
/// in cache-prefix-stability tests show *which* bytes drifted, not just that
/// they did. Returns `None` when the strings are byte-identical.
pub(crate) fn first_divergence(a: &str, b: &str) -> Option<(usize, String, String)> {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let max = a_bytes.len().min(b_bytes.len());
    for i in 0..max {
        if a_bytes[i] != b_bytes[i] {
            let lo = i.saturating_sub(32);
            let a_hi = (i + 32).min(a_bytes.len());
            let b_hi = (i + 32).min(b_bytes.len());
            let a_ctx = String::from_utf8_lossy(&a_bytes[lo..a_hi]).into_owned();
            let b_ctx = String::from_utf8_lossy(&b_bytes[lo..b_hi]).into_owned();
            return Some((i, a_ctx, b_ctx));
        }
    }
    if a_bytes.len() != b_bytes.len() {
        return Some((
            max,
            format!("(len={})", a_bytes.len()),
            format!("(len={})", b_bytes.len()),
        ));
    }
    None
}

/// Assert two strings are byte-identical, panicking with a windowed diff
/// around the first divergence when they aren't. Used by the prefix-cache
/// stability harness (#263, #280) to pin construction surfaces that land in
/// DeepSeek's KV cache prefix.
#[track_caller]
pub(crate) fn assert_byte_identical(label: &str, a: &str, b: &str) {
    if let Some((pos, a_ctx, b_ctx)) = first_divergence(a, b) {
        panic!(
            "{label}: prompt construction is non-deterministic — first diff at byte {pos}\n\
             ── side A (±32B) ──\n{a_ctx:?}\n── side B (±32B) ──\n{b_ctx:?}",
        );
    }
}

// ── Shared App/TuiOptions fixtures (#3923) ──────────────────────────────
//
// Before this module owned them, `create_test_app` was copy-pasted across 28
// test modules, each spelling out the full `TuiOptions` literal — 87 literals
// in all. The copies had drifted: different modules pinned different locales,
// currencies, and onboarding flags without anyone having chosen that, which is
// the non-hermeticity behind the intermittent `config_command_allow_shell_*`
// failures. Adding a `TuiOptions` field meant editing up to 87 sites.
//
// Express intentional differences by mutating the returned value at the call
// site, so the difference is visible as a deliberate line of test code rather
// than hidden inside another near-identical literal.

/// Default `TuiOptions` for tests, pinned to the deepseek-v4-pro fixture route.
/// Mark `workspace` trusted in the test's config, creating it first so the
/// trust key is the canonical path. Repository-supplied commands and skills
/// load only in a trusted workspace.
pub(crate) fn trust_workspace(workspace: &Path) {
    std::fs::create_dir_all(workspace).expect("create test workspace");
    crate::config::save_workspace_trust(workspace).expect("trust test workspace");
}

pub(crate) fn test_tui_options(workspace: impl AsRef<Path>) -> crate::tui::app::TuiOptions {
    let workspace = workspace.as_ref().to_path_buf();
    crate::tui::app::TuiOptions {
        model: "deepseek-v4-pro".to_string(),
        workspace,
        config_path: None,
        config_profile: None,
        allow_shell: false,
        screen_mode: crate::tui::app::ScreenMode::Fullscreen,
        use_mouse_capture: false,
        mouse_capture_preference: false,
        use_bracketed_paste: true,
        max_subagents: 1,
        skills_dir: PathBuf::from("."),
        memory_path: PathBuf::from("memory.md"),
        notes_path: PathBuf::from("notes.txt"),
        mcp_config_path: PathBuf::from("mcp.json"),
        use_memory: false,
        // Majority-of-fixtures defaults, measured across the 89 literals this
        // helper replaced. Modules that need the other value say so explicitly.
        start_in_agent_mode: false,
        skip_onboarding: true,
        yolo: false,
        resume_session_id: None,
        initial_input: None,
        startup_notice: None,
    }
}

/// Build an `App` whose observable state does not depend on the developer's
/// machine.
///
/// `App::new` consults real persisted settings (provider/model maps,
/// auto-model, route limits, locale, currency), so an un-pinned fixture
/// computes against whatever the developer last configured. Every pin below
/// exists because some test was observed to depend on it. This fixture models
/// a session after the user has chosen a Startup action; direct `App::new`
/// tests remain the clean-launch authority.
pub(crate) fn test_app_with_options(options: crate::tui::app::TuiOptions) -> crate::tui::app::App {
    let config = crate::config::Config::default();
    let mut app = crate::tui::app::App::new(options, &config);

    // Shared behavior tests operate on the live session surface. Do not make
    // the production startup conditional for them: clean launches are covered
    // by direct `App::new` tests that retain the Tideline Startup Hero.
    app.launch.visible = false;

    // Deterministic presentation regardless of host locale.
    app.cost_currency = crate::pricing::CostCurrency::Usd;
    app.ui_locale = codewhale_localization::Locale::En;
    // Transcript tests must not depend on a concurrently swapped settings
    // home. Tests for hidden reasoning opt out explicitly.
    app.show_thinking = true;
    // Pin the route identity: without this, a machine with customized
    // settings computes context-window assertions against a different model
    // than the requested deepseek-v4-pro.
    app.set_provider_identity(crate::config::ProviderKind::Deepseek, "deepseek");
    app.billing_presentation = crate::route_billing::BillingPresentation::Metered;
    app.model = "deepseek-v4-pro".to_string();
    app.auto_model = false;
    app.last_effective_model = None;
    app.active_route_limits = None;
    app.active_context_window_override = None;
    // Fixtures replace `app.workspace` freely. Do not retain `App::new`'s real
    // process cwd as a second discovery root: parallel tests and a large
    // developer checkout can otherwise consume the bounded mention index
    // before the fixture workspace is scanned.
    app.composer.mention_cwd = None;
    // `App::new` derives onboarding state from the real `~/.codewhale`, and a
    // pending step makes `ui::frame::render` take its onboarding early return
    // before it assigns `last_prompt_area` or any other chrome geometry. CI
    // has no such state, so a layout test written against that machine passes
    // there and fails on any developer box mid-onboarding — for no product
    // reason. Shared fixtures render the ordinary session surface; onboarding
    // has its own tests that set this state deliberately.
    app.onboarding = crate::tui::app::OnboardingState::None;
    app
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    fn env_snapshot(keys: &[&str]) -> Vec<Option<OsString>> {
        let _lock = lock_test_env();
        keys.iter().map(std::env::var_os).collect()
    }

    #[test]
    fn sealed_home_pins_every_alias_and_restores_the_environment() {
        const KEYS: [&str; 8] = [
            "HOME",
            "USERPROFILE",
            "HOMEDRIVE",
            "HOMEPATH",
            "CODEWHALE_HOME",
            "CODEWHALE_CONFIG_PATH",
            "DEEPSEEK_CONFIG_PATH",
            "DEEPSEEK_HOME",
        ];
        let before = env_snapshot(&KEYS);
        {
            let seal = SealedHome::new();
            let now = |key: &str| std::env::var_os(key);
            assert_eq!(now("HOME").as_deref(), Some(seal.home().as_os_str()));
            assert_eq!(now("USERPROFILE").as_deref(), Some(seal.home().as_os_str()));
            assert_eq!(
                now("CODEWHALE_HOME").as_deref(),
                Some(seal.codewhale_home().as_os_str())
            );
            for alias in [
                "HOMEDRIVE",
                "HOMEPATH",
                "CODEWHALE_CONFIG_PATH",
                "DEEPSEEK_CONFIG_PATH",
                "DEEPSEEK_HOME",
            ] {
                assert_eq!(now(alias), None, "{alias} would route around the seal");
            }
            assert!(guarded_environment_provides_state_paths());
            assert!(home_is_sealed());
            // A resolver that follows the environment lands inside the seal.
            let sessions = crate::session_manager::default_sessions_dir().expect("sessions dir");
            assert_eq!(sessions, seal.codewhale_home().join("sessions"));
        }
        {
            let owned = tempfile::tempdir().expect("owned home");
            let seal = SealedHome::at(owned.path());
            assert_eq!(seal.home(), owned.path());
            assert_eq!(seal.codewhale_home(), owned.path().join(".codewhale"));
        }
        assert_eq!(env_snapshot(&KEYS), before, "the seal must restore the env");
    }

    /// The fence on every resolver a test reaches without ever thinking about
    /// the home: an unsealed test must land in the isolated root, whatever the
    /// ambient `HOME` / `CODEWHALE_HOME` say. Add a resolver here when it grows
    /// a `#[cfg(test)]` fence — and fence the next one before a test finds it.
    #[test]
    fn unsealed_state_resolvers_stay_inside_the_isolated_root() {
        // Hold the barrier to keep the fixture's environment stable.
        let _lock = lock_test_env();
        assert!(!home_is_sealed());
        let root = isolated_test_state_root();
        let inside = |label: &str, path: &Path| {
            assert!(
                path.starts_with(root),
                "{label} resolved outside the isolated test root: {}",
                path.display()
            );
        };

        inside(
            "sessions",
            &crate::session_manager::default_sessions_dir().expect("sessions dir"),
        );
        inside(
            "snapshots",
            &crate::snapshot::snapshot_dir_for(Path::new("/nonexistent/codewhale-fence-probe"))
                .expect("snapshot dir"),
        );
        for bundle in crate::plugins::builtin::materialized_dirs() {
            inside("built-in plugins", &bundle);
        }
        let audit = crate::audit::audit_log_path().expect("audit log path");
        assert!(
            audit.starts_with(std::env::temp_dir()),
            "the audit log resolved outside the temp dir: {}",
            audit.display()
        );
    }

    #[test]
    fn home_seal_is_available_only_to_its_owner_and_enrolled_workers() {
        let seal = SealedHome::new();
        let sessions = seal.codewhale_home().join("sessions");
        let ticket = env_scope_ticket().expect("seal owns an environment scope");

        std::thread::spawn(move || {
            assert!(!home_is_sealed(), "a foreign worker cannot follow the seal");
            assert!(
                crate::session_manager::default_sessions_dir()
                    .expect("isolated sessions dir")
                    .starts_with(isolated_test_state_root())
            );

            let membership = join_env_scope(Some(ticket)).expect("enroll worker");
            assert!(home_is_sealed());
            assert_eq!(
                crate::session_manager::default_sessions_dir().expect("sealed sessions dir"),
                sessions
            );
            drop(membership);
            assert!(!home_is_sealed(), "leaving the scope withdraws the seal");
        })
        .join()
        .expect("home seal worker");
    }

    /// Tripwire for the leak this module exists to prevent: run a sample of
    /// the tests that once wrote `~/.codewhale` in a child process whose
    /// *ambient* `HOME` and `CODEWHALE_HOME` are a seeded, read-only sentinel,
    /// then require both that they pass and that the sentinel is byte-for-byte
    /// as it was. A test that writes the ambient home without sealing its own
    /// fails here by name, instead of silently rewriting a developer's profile.
    ///
    /// The sample covers one test per resolver class that leaked
    /// (`App::new`, command dispatch, sessions, snapshots, plugin bundles,
    /// credentials, the `/import-claude` report). To sweep a whole family,
    /// set `CODEWHALE_TEST_AMBIENT_HOME_FILTER` to a libtest filter such as
    /// `commands::`; that takes minutes, which is why it is not the default.
    #[cfg(unix)]
    #[test]
    fn ambient_home_survives_a_sample_of_state_touching_tests() {
        use std::os::unix::fs::PermissionsExt;

        const PROBE_ENV: &str = "CODEWHALE_TEST_AMBIENT_HOME_PROBE";
        const FILTER_ENV: &str = "CODEWHALE_TEST_AMBIENT_HOME_FILTER";
        const SAMPLE: &[&str] = &[
            "commands::contract::tests::bundle_construction_performs_no_eager_work",
            "commands::contract::tests::skill_group_snapshot_list_and_restore_roundtrip",
            "commands::debug_diagnostics_host_tests::test_context_report_subcommands_return_source_map",
            "commands::debug_mutation_host_tests::test_patch_undo_requests_session_resync_after_restore",
            "commands::groups::core::core::tests::test_clear_resets_all_state",
            "commands::groups::plugins::tests::kimi_managed_import_refuses_linked_children",
            "commands::session_lifecycle_regression_tests::fork_saves_parent_and_switches_to_child_session",
            "commands::session_lifecycle_regression_tests::test_save_creates_file_and_sets_session_id",
            "commands::tests::every_registered_command_dispatches_to_a_handler",
            "commands::tests::every_command_alias_dispatches_to_a_handler",
            "commands::tests::feat020_plugin_dispatches_through_public_seam",
        ];

        // The child runs the sample; it must not recurse into this probe.
        if std::env::var_os(PROBE_ENV).is_some() {
            return;
        }

        fn walk(dir: &Path, visit: &mut dyn FnMut(&Path, &std::fs::Metadata)) {
            let mut entries: Vec<_> = std::fs::read_dir(dir)
                .expect("read sentinel dir")
                .map(|entry| entry.expect("sentinel entry").path())
                .collect();
            entries.sort();
            for path in entries {
                let metadata = std::fs::symlink_metadata(&path).expect("sentinel metadata");
                visit(&path, &metadata);
                if metadata.is_dir() {
                    walk(&path, visit);
                }
            }
        }
        fn listing(root: &Path) -> Vec<String> {
            let mut out = Vec::new();
            walk(root, &mut |path, metadata| {
                let mtime = metadata
                    .modified()
                    .ok()
                    .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                    .map_or(0, |elapsed| elapsed.as_nanos());
                out.push(format!(
                    "{} dir={} len={} mtime={mtime} contents={:?}",
                    path.strip_prefix(root).unwrap_or(path).display(),
                    metadata.is_dir(),
                    metadata.len(),
                    metadata
                        .is_file()
                        .then(|| std::fs::read(path).expect("read sentinel file"))
                ));
            });
            out
        }
        fn set_readonly(root: &Path, readonly: bool) {
            let mode = |dir: bool| match (dir, readonly) {
                (true, true) => 0o500,
                (true, false) => 0o700,
                (false, true) => 0o400,
                (false, false) => 0o600,
            };
            let apply = |path: &Path, dir: bool| {
                std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode(dir)))
                    .expect("set sentinel permissions");
            };
            walk(root, &mut |path, metadata| {
                if !metadata.file_type().is_symlink() {
                    apply(path, metadata.is_dir());
                }
            });
            apply(root, true);
        }

        let sentinel = tempfile::tempdir().expect("sentinel tempdir");
        let home = sentinel.path().join("home");
        let codewhale_home = sentinel.path().join("codewhale-home");
        // Content a leaking test would read, rewrite or migrate.
        for (relative, body) in [
            (
                "home/.claude/CLAUDE.md",
                "# someone's Claude instructions\n",
            ),
            ("home/.claude.json", "{\"mcpServers\":{}}\n"),
            ("home/.codewhale/instructions.md", "# real instructions\n"),
            ("home/.deepseek/config.toml", "model = \"keep-me\"\n"),
            ("home/.deepseek/sessions/legacy.json", "{}\n"),
            ("codewhale-home/instructions.md", "# real instructions\n"),
            ("codewhale-home/sessions/keep.json", "{}\n"),
        ] {
            let path = sentinel.path().join(relative);
            std::fs::create_dir_all(path.parent().expect("sentinel parent")).expect("seed dir");
            std::fs::write(path, body).expect("seed file");
        }
        assert!(home.is_dir() && codewhale_home.is_dir());
        let before = listing(sentinel.path());
        set_readonly(sentinel.path(), true);

        let filter = std::env::var(FILTER_ENV).ok().filter(|f| !f.is_empty());
        let mut child = std::process::Command::new(std::env::current_exe().expect("test binary"));
        child.arg("--test-threads=4");
        match &filter {
            Some(filter) => child.arg(filter),
            None => child.arg("--exact").args(SAMPLE),
        };
        let output = child
            .env(PROBE_ENV, "1")
            .env("HOME", &home)
            .env("USERPROFILE", &home)
            .env("CODEWHALE_HOME", &codewhale_home)
            .env_remove("HOMEDRIVE")
            .env_remove("HOMEPATH")
            .env_remove("CODEWHALE_CONFIG_PATH")
            .env_remove("DEEPSEEK_CONFIG_PATH")
            .env_remove("DEEPSEEK_HOME")
            .output();

        // Restore write access first so a failure still cleans up after itself.
        set_readonly(sentinel.path(), false);
        let output = output.expect("run the sample under a read-only ambient home");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let report = format!("stdout:\n{stdout}\nstderr:\n{stderr}");

        assert!(
            output.status.success(),
            "a test failed under a read-only ambient home — it reaches the user's \
             state without sealing it (use SealedHome, or fence the resolver):\n{report}"
        );
        if filter.is_none() {
            assert!(
                stdout.contains(&format!("running {} tests", SAMPLE.len())),
                "the sample names drifted from real tests; update SAMPLE:\n{report}"
            );
        }
        assert_eq!(
            listing(sentinel.path()),
            before,
            "a test wrote the ambient home:\n{report}"
        );
    }

    #[test]
    fn ambient_codewhale_home_is_not_a_test_seal() {
        let _lock = lock_test_env();
        let _ambient = EnvVarGuard::set("CODEWHALE_HOME", "/tmp/ambient-codewhale-home");
        unregister_guarded_env_key("CODEWHALE_HOME");

        let sealed = guarded_environment_provides_state_paths();

        register_guarded_env_key("CODEWHALE_HOME");
        assert!(!sealed, "ambient developer state must remain confined");
    }

    #[test]
    fn a_config_path_guard_or_a_home_shadowed_by_the_ambient_override_is_not_a_home_seal() {
        let _lock = lock_test_env();
        let dir = tempfile::tempdir().expect("tempdir");
        {
            // Redirecting the config file leaves `~/.codewhale` in play.
            let _config = EnvVarGuard::set("CODEWHALE_CONFIG_PATH", dir.path().join("config.toml"));
            let _no_home_override = EnvVarGuard::remove("CODEWHALE_HOME");
            assert!(guarded_environment_provides_state_paths());
            assert!(!home_is_sealed());
        }
        {
            // `CODEWHALE_HOME` outranks `HOME`: guarding only `HOME` while an
            // ambient `CODEWHALE_HOME` is set seals nothing.
            let _ambient = EnvVarGuard::set("CODEWHALE_HOME", dir.path().join("ambient"));
            unregister_guarded_env_key("CODEWHALE_HOME");
            let _home = EnvVarGuard::set("HOME", dir.path());
            let sealed = home_is_sealed();
            register_guarded_env_key("CODEWHALE_HOME");
            assert!(!sealed, "an ambient CODEWHALE_HOME shadows a guarded HOME");
        }
        {
            let _no_home_override = EnvVarGuard::remove("CODEWHALE_HOME");
            let _home = EnvVarGuard::set("HOME", dir.path());
            assert!(home_is_sealed());
        }
    }

    #[test]
    fn removing_overrides_does_not_seal_the_ambient_home() {
        let _lock = lock_test_env();
        let _codewhale_home = EnvVarGuard::remove("CODEWHALE_HOME");
        let _codewhale_config = EnvVarGuard::remove("CODEWHALE_CONFIG_PATH");
        let _deepseek_config = EnvVarGuard::remove("DEEPSEEK_CONFIG_PATH");

        assert!(
            !guarded_environment_provides_state_paths(),
            "removing an override must not expose the developer's HOME"
        );
    }

    #[test]
    fn removing_home_variables_does_not_seal_a_missing_path() {
        let _lock = lock_test_env();
        let _home = EnvVarGuard::remove("HOME");
        let _userprofile = EnvVarGuard::remove("USERPROFILE");

        assert!(
            !guarded_environment_provides_state_paths(),
            "removing HOME variables must keep state in the isolated test root"
        );
        assert_eq!(
            crate::config_persistence::config_toml_path(None)
                .expect("resolve isolated config path"),
            unsealed_test_state_root().join(codewhale_config::CONFIG_FILE_NAME)
        );
    }

    #[test]
    fn lock_without_sealed_paths_does_not_use_developer_config() {
        let _lock = lock_test_env();
        let path = crate::config_persistence::config_toml_path(None)
            .expect("resolve isolated config path");
        let root = isolated_test_state_root();
        assert!(
            path.starts_with(root),
            "holding lock_test_env without an EnvVarGuard must not read ~/.codewhale ({})",
            path.display()
        );
        assert_eq!(
            path,
            unsealed_test_state_root().join(codewhale_config::CONFIG_FILE_NAME)
        );
    }

    #[test]
    fn unguarded_state_writes_use_isolated_test_root() {
        const PROBE_ENV: &str = "CODEWHALE_TEST_STATE_ISOLATION_PROBE";
        const RECEIPT_ENV: &str = "CODEWHALE_TEST_STATE_ISOLATION_RECEIPT";

        if std::env::var_os(PROBE_ENV).is_some() {
            let config_path =
                crate::config_persistence::persist_root_bool_key(None, "allow_shell", true)
                    .expect("write isolated config");
            let direct_config_path =
                crate::config::save_workspace_trust(Path::new("/tmp/codewhale-test-workspace"))
                    .expect("write through direct default config path");
            crate::settings::Settings::default()
                .save()
                .expect("write isolated settings");
            let settings_path =
                crate::settings::Settings::path().expect("resolve isolated settings");
            let root = isolated_test_state_root();
            assert!(config_path.starts_with(root), "{}", config_path.display());
            assert!(
                settings_path.starts_with(root),
                "{}",
                settings_path.display()
            );
            assert!(
                direct_config_path.starts_with(root),
                "{}",
                direct_config_path.display()
            );
            let receipt = std::env::var_os(RECEIPT_ENV).expect("receipt path");
            std::fs::write(
                receipt,
                format!(
                    "{}\n{}\n{}\n{}\n",
                    root.display(),
                    config_path.display(),
                    settings_path.display(),
                    direct_config_path.display()
                ),
            )
            .expect("write isolation receipt");
            return;
        }

        let sentinel = tempfile::tempdir().expect("sentinel home");
        let user_state = sentinel.path().join(".codewhale");
        std::fs::create_dir_all(&user_state).expect("create sentinel state");
        let config_path = user_state.join("config.toml");
        let settings_path = user_state.join("settings.toml");
        let config_sentinel = b"# developer config sentinel\n";
        let settings_sentinel = b"# developer settings sentinel\n";
        std::fs::write(&config_path, config_sentinel).expect("seed config");
        std::fs::write(&settings_path, settings_sentinel).expect("seed settings");
        let receipt_path = sentinel.path().join("receipt.txt");

        let output = std::process::Command::new(std::env::current_exe().expect("test binary"))
            .arg("--exact")
            .arg("test_support::tests::unguarded_state_writes_use_isolated_test_root")
            .arg("--test-threads=1")
            .env(PROBE_ENV, "1")
            .env(RECEIPT_ENV, &receipt_path)
            .env("HOME", sentinel.path())
            .env("USERPROFILE", sentinel.path())
            .env_remove("CODEWHALE_HOME")
            .env_remove("CODEWHALE_CONFIG_PATH")
            .env_remove("DEEPSEEK_CONFIG_PATH")
            .output()
            .expect("run isolated-state probe");
        assert!(
            output.status.success(),
            "probe failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        assert_eq!(
            std::fs::read(&config_path).expect("read config sentinel"),
            config_sentinel
        );
        assert_eq!(
            std::fs::read(&settings_path).expect("read settings sentinel"),
            settings_sentinel
        );

        let receipt = std::fs::read_to_string(&receipt_path).expect("read isolation receipt");
        let mut paths = receipt.lines().map(PathBuf::from);
        let isolated_root = paths.next().expect("root receipt");
        let written_config = paths.next().expect("config receipt");
        let written_settings = paths.next().expect("settings receipt");
        let direct_config = paths.next().expect("direct config receipt");
        assert!(!isolated_root.starts_with(sentinel.path()));
        assert!(written_config.starts_with(&isolated_root));
        assert!(written_settings.starts_with(&isolated_root));
        assert!(direct_config.starts_with(&isolated_root));
        assert!(written_config.exists());
        assert!(written_settings.exists());
    }

    #[test]
    fn config_path_read_waits_for_foreign_env_redirect_to_restore() {
        let (started_tx, started_rx) = mpsc::channel();
        let (tx, rx) = mpsc::channel();
        let redirected = std::env::temp_dir().join(format!(
            "codewhale-config-path-read-barrier-{}",
            std::process::id()
        ));

        let reader = {
            let lock = lock_test_env();
            let redirect = EnvVarGuard::set("DEEPSEEK_CONFIG_PATH", &redirected);
            let reader = std::thread::spawn(move || {
                started_tx.send(()).expect("signal config path read start");
                tx.send(crate::config_persistence::config_toml_path(None))
                    .expect("send resolved config path");
            });

            started_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("reader reached config path resolution");
            assert!(
                rx.recv_timeout(Duration::from_millis(50)).is_err(),
                "a foreign reader observed the temporary config redirect"
            );
            drop(redirect);
            drop(lock);
            reader
        };

        let resolved = rx
            .recv_timeout(Duration::from_secs(2))
            .expect("reader resumed after the redirect was restored")
            .expect("resolve config path");
        reader.join().expect("reader thread");
        assert_ne!(resolved, redirected);
    }

    #[test]
    fn settings_save_waits_for_foreign_state_io_transaction() {
        let (holder_ready_tx, holder_ready_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let holder = std::thread::spawn(move || {
            with_test_state_io_lock(|| {
                holder_ready_tx.send(()).expect("signal state lock held");
                release_rx.recv().expect("release state lock");
            });
        });
        holder_ready_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("holder acquired state I/O lock");

        let (started_tx, started_rx) = mpsc::channel();
        let (saved_tx, saved_rx) = mpsc::channel();
        let writer = std::thread::spawn(move || {
            started_tx.send(()).expect("signal settings save start");
            saved_tx
                .send(crate::settings::Settings::default().save())
                .expect("send settings save result");
        });
        started_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("writer reached settings save");
        assert!(
            saved_rx.recv_timeout(Duration::from_millis(50)).is_err(),
            "settings save did not wait for an in-flight state transaction"
        );

        release_tx.send(()).expect("release holder");
        holder.join().expect("holder thread");
        saved_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("settings save resumed")
            .expect("settings save succeeded");
        writer.join().expect("writer thread");
    }
}
