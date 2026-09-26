//! One extension-host process: launch plan, spawn, handshake, channel, exit.
//!
//! Phase 1 has no heartbeat and no auto-restart. When the process exits for
//! any reason, every in-flight call fails with a typed error, the owner
//! registry is revoked wholesale, and the host is marked failed with its
//! stderr tail. Nothing respawns until the next session or a newly enabled
//! plugin.
//!
//! **OS sandbox.** Where Codewhale's default command sandbox is available
//! (Seatbelt on macOS; bubblewrap stays opt-in for shell commands and is not
//! used here) the host runs under a workspace-write profile rooted at
//! `$CODEWHALE_HOME/extension-host/data`: no direct network, writes only
//! there and in the temp dirs, and **no reads** of the Codewhale homes
//! (everything but the bundle, its data dir and plugin code), the Codex and
//! DSH credential homes, and the credential-store default deny-list
//! (`sandbox::read_guard`). Other user-readable files stay readable —
//! including `.env` files, whose filename rule has no Seatbelt subpath form —
//! and Mach services are not restricted, so this is defense-in-depth, not a
//! containment boundary. Elsewhere (Linux, Windows) the host runs unsandboxed
//! with the user's permissions, and `/plugin` says so.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot};

use super::protocol::{
    self, CoreRequest, HostLimits, HostMessage, HostNotification, HostRequest, InitializeParams,
    RegisterResult, error_code,
};

/// Budget for `host/hello` → `host/initialize` → `host/ready`. The design's
/// 2 s target is kept for warm starts (measured ~40 ms); the hard limit is
/// wider so a cold, loaded CI machine does not fail the handshake.
pub const HANDSHAKE_DEADLINE: Duration = Duration::from_secs(5);
pub const ACTIVATE_DEADLINE: Duration = Duration::from_secs(5);
pub const DISPOSE_DEADLINE: Duration = Duration::from_secs(2);
/// Grace between `$/cancel` and resolving a call as cancelled on this side.
pub const CANCEL_GRACE: Duration = Duration::from_millis(500);
const STDERR_TAIL_BYTES: usize = 8 * 1024;
const OUTBOUND_QUEUE: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HostCallError {
    #[error("{message} (extension error {code})")]
    Rpc { code: i64, message: String },
    #[error("extension host exited: {0}")]
    Exited(String),
    #[error("cancelled: {0}")]
    Cancelled(String),
    #[error("timed out after {0:?}")]
    Timeout(Duration),
    #[error("extension host channel is full")]
    Busy,
}

/// How the host is started: the argv (wrapped by the OS sandbox when one is
/// available), its working directory, and which sandbox applies.
#[derive(Debug, Clone)]
pub(crate) struct HostLaunch {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    /// `seatbelt` / `bwrap`, or `None` when the host runs unsandboxed.
    pub sandbox: Option<String>,
    /// Environment the sandbox wrapper adds (`CODEWHALE_SANDBOX`, …).
    pub sandbox_env: Vec<(String, String)>,
}

/// Top-level entries of a Codewhale home the host may read: its own bundle
/// and data (`extension-host`), and plugin code (the staged snapshots live
/// under `plugins/.runtime`). Everything else in a Codewhale home — secrets,
/// tokens, config and its backups, sessions, state, tool outputs, history —
/// is denied.
const HOST_READABLE_HOME_ENTRIES: &[&str] = &["extension-host", "plugins", "builtin-plugins"];

/// Codewhale-home entries denied by name even before they exist, so a store
/// created after the host started is still covered. Existing entries are
/// denied by enumeration (`host_denied_read_paths`).
const HOST_DENIED_HOME_ENTRIES: &[&str] = &[
    "secrets",
    "credentials",
    "tokens",
    "state",
    "state.db",
    "sessions",
    "session-archives",
    "session_index.jsonl",
    "tool_outputs",
    "composer_history.txt",
    "remote-control",
    "integrations",
    "audit.log",
    "logs",
    "memory",
    "mcp.json",
    "mcp.json.bak",
    "config.toml.bak",
    "settings.toml",
];

/// Paths the host process must never read, even though the sandbox otherwise
/// grants full-disk read: the curated credential-store defaults; every entry
/// of Codewhale's homes (the runtime home, the ambient `~/.codewhale`, and the
/// legacy `~/.deepseek`) except [`HOST_READABLE_HOME_ENTRIES`]; and the Codex
/// and DSH homes whose credential files Codewhale itself reads. Blocking.
pub(crate) fn host_denied_read_paths(home: &Path) -> Vec<PathBuf> {
    let mut paths = crate::sandbox::read_guard::ReadDenylist::build(true, &[], &[]).subtree_paths();
    let mut push = |path: PathBuf| {
        if !paths.contains(&path) {
            paths.push(path);
        }
    };
    let user_home = codewhale_paths::user_home();
    let mut roots = vec![home.to_path_buf()];
    roots.extend(codewhale_config::codewhale_home().ok());
    if let Some(user) = &user_home {
        roots.push(user.join(codewhale_config::CODEWHALE_APP_DIR));
        roots.push(user.join(".deepseek"));
    }
    for root in roots {
        let mut names: Vec<std::ffi::OsString> = HOST_DENIED_HOME_ENTRIES
            .iter()
            .chain(std::iter::once(&codewhale_config::CONFIG_FILE_NAME))
            .map(std::ffi::OsString::from)
            .collect();
        if let Ok(entries) = std::fs::read_dir(&root) {
            names.extend(
                entries
                    .filter_map(Result::ok)
                    .map(|entry| entry.file_name()),
            );
        }
        // Seatbelt matches the kernel-resolved path, and a name that does not
        // exist yet cannot be canonicalized later, so deny it under both the
        // given and the resolved spelling of its (existing) root.
        let resolved = std::fs::canonicalize(&root).ok();
        for name in names {
            let readable = name
                .to_str()
                .is_some_and(|name| HOST_READABLE_HOME_ENTRIES.contains(&name));
            if !readable {
                if let Some(resolved) = &resolved {
                    push(resolved.join(&name));
                }
                push(root.join(name));
            }
        }
    }
    // Codex's home (ChatGPT OAuth tokens in `auth.json`) and the DSH home
    // (`.credentials.yaml`), wherever the environment points them.
    if let Some(codex_home) = crate::oauth::auth_file_path().parent() {
        push(codex_home.to_path_buf());
    }
    if let Some(user) = &user_home {
        push(user.join(".codex"));
        push(user.join(".dsh"));
    }
    if let Some(dsh_home) = codewhale_config::default_dsh_credentials_path().parent() {
        push(dsh_home.to_path_buf());
    }
    paths
}

/// Plan the host launch. Blocking (creates the data dir, canonicalizes the
/// deny-list); call from `spawn_blocking`.
pub(crate) fn plan_launch(node: &Path, bundle: &Path, home: &Path) -> Result<HostLaunch, String> {
    use crate::sandbox::{CommandSpec, SandboxManager, SandboxPolicy, SandboxType};
    let data = home.join("extension-host").join("data");
    std::fs::create_dir_all(&data)
        .map_err(|error| format!("cannot create {}: {error}", data.display()))?;
    let args: Vec<String> = [
        "--max-old-space-size=256",
        "--disable-proto=throw",
        "--no-addons",
    ]
    .into_iter()
    .map(str::to_string)
    .chain(std::iter::once(bundle.to_string_lossy().into_owned()))
    .collect();
    let unsandboxed = HostLaunch {
        program: node.to_path_buf(),
        args: args.clone(),
        cwd: data.clone(),
        sandbox: None,
        sandbox_env: Vec::new(),
    };
    if cfg!(windows) {
        // The Windows helper is process containment only; ProcessTree already
        // provides that, and it must not be reported as isolation.
        return Ok(unsandboxed);
    }
    let spec = CommandSpec::program(&node.to_string_lossy(), args, data, Duration::ZERO)
        .with_policy(SandboxPolicy::WorkspaceWrite {
            writable_roots: Vec::new(),
            network_access: false,
            exclude_tmpdir: false,
            exclude_slash_tmp: false,
        });
    let mut manager = SandboxManager::new();
    manager.set_denied_read_subpaths(host_denied_read_paths(home));
    let env = manager.prepare(&spec);
    if matches!(env.sandbox_type, SandboxType::None) {
        return Ok(unsandboxed);
    }
    let mut command = env.command.into_iter();
    let program = command
        .next()
        .ok_or("sandbox wrapper produced an empty command")?;
    Ok(HostLaunch {
        program: PathBuf::from(program),
        args: command.collect(),
        cwd: env.cwd,
        sandbox: Some(env.sandbox_type.to_string()),
        sandbox_env: env.env.into_iter().collect(),
    })
}

/// Callbacks from the channel into the manager.
pub(crate) trait HostEvents: Send + Sync + 'static {
    fn register(&self, params: &protocol::RegisterParams) -> RegisterResult;
    fn unregister(&self, params: &protocol::UnregisterParams);
    fn faulted(&self, params: &protocol::FaultedParams);
    fn exited(&self, host_generation: u64, reason: String, stderr_tail: String);
}

/// Receives one request's outcome.
pub(crate) type CallReceiver = oneshot::Receiver<Result<Value, HostCallError>>;

struct PendingCall {
    tx: oneshot::Sender<Result<Value, HostCallError>>,
    /// Plugin whose revocation cancels this call.
    owner: Option<String>,
    revoked: bool,
}

#[derive(Default)]
struct Handshake {
    hello: Option<oneshot::Sender<protocol::HelloParams>>,
    ready: Option<oneshot::Sender<()>>,
}

pub(crate) struct HostProcess {
    pub pid: Option<u32>,
    pub node_version: std::sync::OnceLock<String>,
    /// `seatbelt` / `bwrap`, or `None` when unsandboxed.
    pub sandbox: Option<String>,
    tree: Arc<crate::process_tree::ProcessTree>,
    outbound: mpsc::Sender<Vec<u8>>,
    pending: Arc<Mutex<HashMap<u64, PendingCall>>>,
    next_id: AtomicU64,
    stderr_tail: Arc<Mutex<VecDeque<u8>>>,
    exited: tokio::sync::watch::Receiver<bool>,
}

fn push_tail(tail: &Mutex<VecDeque<u8>>, bytes: &[u8]) {
    let mut tail = tail.lock().expect("stderr tail lock");
    tail.extend(bytes);
    while tail.len() > STDERR_TAIL_BYTES {
        tail.pop_front();
    }
}

fn tail_string(tail: &Mutex<VecDeque<u8>>) -> String {
    let tail = tail.lock().expect("stderr tail lock");
    let bytes: Vec<u8> = tail.iter().copied().collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

impl HostProcess {
    /// Spawn the host and complete the handshake. `expected_sha256` is the
    /// digest of the bundle this process materialized; the host's
    /// self-reported digest must match (a consistency check, not
    /// anti-substitution: the control is that Rust chooses what to exec).
    pub(crate) async fn spawn(
        generation: u64,
        launch: &HostLaunch,
        expected_sha256: &str,
        events: Arc<dyn HostEvents>,
    ) -> Result<Arc<Self>, String> {
        let mut command = tokio::process::Command::new(&launch.program);
        crate::utils::suppress_tokio_console_window(&mut command);
        command
            .args(&launch.args)
            .current_dir(&launch.cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        // Scrubbed environment: no credentials, no ambient proxy URLs.
        command.env_clear();
        let parent_pid = std::process::id().to_string();
        // On Unix the host leads its own process group (below), so it may kill
        // that group when the core goes away (stdin EOF, or a parent change
        // seen by its watchdog thread).
        let own_group = if cfg!(unix) { "1" } else { "0" };
        let overrides = launch
            .sandbox_env
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .chain([
                ("CODEWHALE_HOST_PARENT_PID", parent_pid.as_str()),
                ("CODEWHALE_HOST_PROCESS_GROUP", own_group),
            ]);
        for (key, value) in
            crate::child_env::sanitized_plugin_mcp_env_from(std::env::vars_os(), overrides)
        {
            command.env(key, value);
        }
        #[cfg(unix)]
        command.process_group(0);

        let mut child = command
            .spawn()
            .map_err(|error| format!("failed to start {}: {error}", launch.program.display()))?;
        let pid = child.id();
        let tree = match crate::process_tree::ProcessTree::attach_tokio(&child) {
            Ok(tree) => Arc::new(tree),
            Err(error) => {
                let _ = child.start_kill();
                return Err(format!("failed to contain the extension host: {error}"));
            }
        };
        let stdin = child.stdin.take().ok_or("host stdin unavailable")?;
        let stdout = child.stdout.take().ok_or("host stdout unavailable")?;
        let stderr = child.stderr.take().ok_or("host stderr unavailable")?;

        let (outbound, mut outbound_rx) = mpsc::channel::<Vec<u8>>(OUTBOUND_QUEUE);
        let pending: Arc<Mutex<HashMap<u64, PendingCall>>> = Arc::default();
        let stderr_tail: Arc<Mutex<VecDeque<u8>>> = Arc::default();
        let (exited_tx, exited_rx) = tokio::sync::watch::channel(false);
        let (hello_tx, hello_rx) = oneshot::channel();
        let (ready_tx, ready_rx) = oneshot::channel();
        let handshake = Arc::new(Mutex::new(Handshake {
            hello: Some(hello_tx),
            ready: Some(ready_tx),
        }));

        // Writer: the only task that touches stdin. Dropping every sender
        // closes stdin, which the host treats as "core is gone".
        tokio::spawn(async move {
            let mut stdin = stdin;
            while let Some(frame) = outbound_rx.recv().await {
                if stdin.write_all(&frame).await.is_err() || stdin.flush().await.is_err() {
                    break;
                }
            }
        });

        // stderr: a bounded tail for diagnostics, chunks into tracing. Read
        // in fixed-size chunks, never by line: a plugin writing endless
        // output without a newline must not grow this process's memory.
        {
            let tail = Arc::clone(&stderr_tail);
            tokio::spawn(async move {
                let mut stderr = stderr;
                let mut chunk = vec![0_u8; 4096];
                while let Ok(read) = stderr.read(&mut chunk).await {
                    if read == 0 {
                        break;
                    }
                    push_tail(&tail, &chunk[..read]);
                    tracing::debug!(
                        target: "extension_host",
                        "host stderr: {}",
                        String::from_utf8_lossy(&chunk[..read]).trim_end()
                    );
                }
            });
        }

        // Reader: every frame is validated strictly; a framing or protocol
        // violation kills the host (a plugin wrote to the channel, or the
        // host is not ours).
        let (kill_tx, mut kill_rx) = mpsc::channel::<String>(1);
        {
            let pending = Arc::clone(&pending);
            let outbound = outbound.clone();
            let events = Arc::clone(&events);
            let handshake = Arc::clone(&handshake);
            let kill_tx = kill_tx.clone();
            tokio::spawn(async move {
                let mut stdout = stdout;
                loop {
                    let value = match protocol::read_frame(&mut stdout).await {
                        Ok(Some(value)) => value,
                        Ok(None) => break,
                        Err(error) => {
                            let _ = kill_tx.try_send(format!("channel framing violation: {error}"));
                            break;
                        }
                    };
                    let message = match protocol::parse_host_message(value) {
                        Ok(message) => message,
                        Err(error) => {
                            let _ = kill_tx.try_send(format!("protocol violation: {error}"));
                            break;
                        }
                    };
                    handle_host_message(message, &pending, &outbound, events.as_ref(), &handshake);
                }
            });
        }

        // Exit watcher: owns the child. On exit, fail everything and report.
        {
            let pending = Arc::clone(&pending);
            let tail = Arc::clone(&stderr_tail);
            let events = Arc::clone(&events);
            let tree = Arc::clone(&tree);
            tokio::spawn(async move {
                let reason = tokio::select! {
                    status = child.wait() => match status {
                        Ok(status) => format!("exited with {status}"),
                        Err(error) => format!("wait failed: {error}"),
                    },
                    Some(reason) = kill_rx.recv() => {
                        let _ = tree.kill();
                        let _ = child.kill().await;
                        reason
                    }
                };
                // The leader is gone; take anything it left behind with it.
                let _ = tree.kill();
                let drained: Vec<PendingCall> = pending
                    .lock()
                    .expect("pending lock")
                    .drain()
                    .map(|(_, call)| call)
                    .collect();
                for call in drained {
                    let _ = call.tx.send(Err(HostCallError::Exited(reason.clone())));
                }
                let _ = exited_tx.send(true);
                // Give the stderr task a moment to capture the last lines.
                tokio::time::sleep(Duration::from_millis(50)).await;
                events.exited(generation, reason, tail_string(&tail));
            });
        }

        let host = Arc::new(Self {
            pid,
            node_version: std::sync::OnceLock::new(),
            sandbox: launch.sandbox.clone(),
            tree,
            outbound,
            pending,
            next_id: AtomicU64::new(1),
            stderr_tail,
            exited: exited_rx,
        });

        let handshake_result = tokio::time::timeout(HANDSHAKE_DEADLINE, async {
            let hello = hello_rx
                .await
                .map_err(|_| "host exited before host/hello".to_string())?;
            if hello.protocol.min > protocol::PROTOCOL_VERSION
                || hello.protocol.max < protocol::PROTOCOL_VERSION
            {
                return Err(format!(
                    "host speaks protocol {}..={}, core speaks {}",
                    hello.protocol.min,
                    hello.protocol.max,
                    protocol::PROTOCOL_VERSION
                ));
            }
            if hello.bundle_sha256 != expected_sha256 {
                return Err(format!(
                    "host bundle digest {} does not match the materialized bundle {}",
                    hello.bundle_sha256, expected_sha256
                ));
            }
            let initialize = CoreRequest::Initialize(InitializeParams {
                protocol: protocol::PROTOCOL_VERSION,
                limits: HostLimits {
                    max_frame: protocol::MAX_FRAME as u64,
                    max_inflight: protocol::MAX_INFLIGHT as u64,
                    dispose_deadline_ms: DISPOSE_DEADLINE.as_millis() as u64,
                    activate_deadline_ms: ACTIVATE_DEADLINE.as_millis() as u64,
                },
            });
            host.request(initialize, None)
                .await
                .map_err(|error| format!("host/initialize failed: {error}"))?;
            ready_rx
                .await
                .map_err(|_| "host exited before host/ready".to_string())?;
            Ok(hello.node_version)
        })
        .await;
        match handshake_result {
            Ok(Ok(node_version)) => {
                let _ = host.node_version.set(node_version);
                Ok(host)
            }
            Ok(Err(reason)) => {
                let _ = kill_tx.try_send(reason.clone());
                Err(format!(
                    "{reason}; stderr: {}",
                    tail_string(&host.stderr_tail)
                ))
            }
            Err(_) => {
                let reason = format!("handshake exceeded {HANDSHAKE_DEADLINE:?}");
                let _ = kill_tx.try_send(reason.clone());
                Err(format!(
                    "{reason}; stderr: {}",
                    tail_string(&host.stderr_tail)
                ))
            }
        }
    }

    #[must_use]
    pub fn has_exited(&self) -> bool {
        *self.exited.borrow()
    }

    fn send_frame(&self, value: &Value) -> Result<(), HostCallError> {
        let frame = protocol::encode_frame(value).map_err(|error| HostCallError::Rpc {
            code: error_code::INVALID_PARAMS,
            message: error.to_string(),
        })?;
        self.outbound.try_send(frame).map_err(|error| match error {
            mpsc::error::TrySendError::Full(_) => HostCallError::Busy,
            mpsc::error::TrySendError::Closed(_) => {
                HostCallError::Exited("channel closed".to_string())
            }
        })
    }

    /// Send a request; the returned id can be cancelled with [`Self::cancel`].
    pub(crate) fn start_request(
        &self,
        request: CoreRequest,
        owner: Option<String>,
    ) -> Result<(u64, CallReceiver), HostCallError> {
        if self.has_exited() {
            return Err(HostCallError::Exited("already exited".to_string()));
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.lock().expect("pending lock");
            if pending.len() >= protocol::MAX_INFLIGHT {
                return Err(HostCallError::Busy);
            }
            pending.insert(
                id,
                PendingCall {
                    tx,
                    owner,
                    revoked: false,
                },
            );
        }
        if let Err(error) = self.send_frame(&request.to_value(id)) {
            self.pending.lock().expect("pending lock").remove(&id);
            return Err(error);
        }
        Ok((id, rx))
    }

    pub(crate) async fn request(
        &self,
        request: CoreRequest,
        owner: Option<String>,
    ) -> Result<Value, HostCallError> {
        let (_, rx) = self.start_request(request, owner)?;
        rx.await
            .unwrap_or_else(|_| Err(HostCallError::Exited("channel closed".to_string())))
    }

    /// Request with a deadline; on expiry the call is cancelled host-side.
    pub(crate) async fn request_with_deadline(
        &self,
        request: CoreRequest,
        owner: Option<String>,
        deadline: Duration,
    ) -> Result<Value, HostCallError> {
        let (id, rx) = self.start_request(request, owner)?;
        match tokio::time::timeout(deadline, rx).await {
            Ok(result) => {
                result.unwrap_or_else(|_| Err(HostCallError::Exited("channel closed".to_string())))
            }
            Err(_) => {
                self.cancel(id);
                self.pending.lock().expect("pending lock").remove(&id);
                Err(HostCallError::Timeout(deadline))
            }
        }
    }

    /// Fire `$/cancel`. Best effort: a full or closed channel is fine, the
    /// caller resolves its side on its own schedule.
    pub(crate) fn cancel(&self, id: u64) {
        let _ = self.send_frame(&protocol::cancel_value(id));
    }

    /// Forget a request without cancelling it (its answer will be dropped).
    pub(crate) fn forget(&self, id: u64) {
        self.pending.lock().expect("pending lock").remove(&id);
    }

    /// Revocation: cancel every in-flight call owned by `plugin_id`; each
    /// resolves as cancelled when the host answers or after `CANCEL_GRACE`,
    /// whichever is first — revocation never waits on the host.
    pub(crate) fn revoke_calls_of(self: &Arc<Self>, plugin_id: &str) {
        let ids: Vec<u64> = {
            let mut pending = self.pending.lock().expect("pending lock");
            pending
                .iter_mut()
                .filter(|(_, call)| call.owner.as_deref() == Some(plugin_id))
                .map(|(id, call)| {
                    call.revoked = true;
                    *id
                })
                .collect()
        };
        for id in ids {
            self.cancel(id);
            let pending = Arc::clone(&self.pending);
            tokio::spawn(async move {
                tokio::time::sleep(CANCEL_GRACE).await;
                if let Some(call) = pending.lock().expect("pending lock").remove(&id) {
                    let _ = call.tx.send(Err(HostCallError::Cancelled(
                        "extension was revoked".to_string(),
                    )));
                }
            });
        }
    }

    /// Bounded shutdown: `host/shutdown` (2 s), close stdin, then kill the
    /// process group at 3 s total.
    #[cfg(test)]
    pub(crate) async fn shutdown(&self) {
        let _ = tokio::time::timeout(
            Duration::from_secs(2),
            self.request(CoreRequest::Shutdown, None),
        )
        .await;
        let mut exited = self.exited.clone();
        let waited = tokio::time::timeout(Duration::from_secs(1), async {
            while !*exited.borrow() {
                if exited.changed().await.is_err() {
                    break;
                }
            }
        })
        .await;
        if waited.is_err() {
            let _ = self.tree.kill();
        }
    }
}

impl Drop for HostProcess {
    fn drop(&mut self) {
        // The exit watcher also holds the tree; kill explicitly so dropping
        // the last handle to a live host never leaves it running.
        if !self.has_exited() {
            let _ = self.tree.kill();
        }
    }
}

fn handle_host_message(
    message: HostMessage,
    pending: &Mutex<HashMap<u64, PendingCall>>,
    outbound: &mpsc::Sender<Vec<u8>>,
    events: &dyn HostEvents,
    handshake: &Mutex<Handshake>,
) {
    let send = |value: Value| {
        if let Ok(frame) = protocol::encode_frame(&value) {
            let _ = outbound.try_send(frame);
        }
    };
    match message {
        HostMessage::Response { id, outcome } => {
            let Some(call) = pending.lock().expect("pending lock").remove(&id) else {
                tracing::debug!(target: "extension_host", id, "dropping late host response");
                return;
            };
            let result = if call.revoked {
                Err(HostCallError::Cancelled(
                    "extension was revoked".to_string(),
                ))
            } else {
                match outcome {
                    Ok(value) => Ok(value),
                    Err(error) if error.code == error_code::CANCELLED => {
                        Err(HostCallError::Cancelled(error.message))
                    }
                    Err(error) => Err(HostCallError::Rpc {
                        code: error.code,
                        message: error.message,
                    }),
                }
            };
            let _ = call.tx.send(result);
        }
        HostMessage::Request { id, request } => match request {
            HostRequest::Register(params) => {
                let result = events.register(&params);
                send(protocol::response_ok(
                    id,
                    serde_json::to_value(result).unwrap_or_else(|_| json!({"refused": "internal"})),
                ));
            }
            HostRequest::Unregister(params) => {
                events.unregister(&params);
                send(protocol::response_ok(id, json!({})));
            }
        },
        HostMessage::Notification(notification) => match notification {
            HostNotification::Hello(hello) => {
                if let Some(tx) = handshake.lock().expect("handshake lock").hello.take() {
                    let _ = tx.send(hello);
                }
            }
            HostNotification::Ready => {
                if let Some(tx) = handshake.lock().expect("handshake lock").ready.take() {
                    let _ = tx.send(());
                }
            }
            HostNotification::Faulted(params) => events.faulted(&params),
            HostNotification::Log(log) => {
                let plugin = log.plugin_id.as_deref().unwrap_or("host");
                match log.level.as_str() {
                    "error" => tracing::warn!(target: "extension_host", plugin, "{}", log.msg),
                    "warn" => tracing::info!(target: "extension_host", plugin, "{}", log.msg),
                    _ => tracing::debug!(target: "extension_host", plugin, "{}", log.msg),
                }
            }
            // Phase 1 has no host-originated requests to cancel.
            HostNotification::Cancel(_) => {}
        },
    }
}

/// Where the embedded bundle is written: `<root>/extension-host/<sha256>/`.
#[must_use]
pub fn bundle_dir(root: &Path, sha256: &str) -> PathBuf {
    root.join("extension-host").join(sha256)
}
