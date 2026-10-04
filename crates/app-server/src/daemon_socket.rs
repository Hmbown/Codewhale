//! Authenticated local daemon transport: Unix socket and Windows named pipe.
//!
//! The desktop shell attaches to a long-lived `codewhale app-server --socket`
//! daemon over a local socket instead of a TCP port: local multi-client,
//! peer-credential auth, nothing to firewall (CORE-PROTOCOL spec §5). The
//! wire is *identical* to the `--stdio` transport — newline-delimited
//! JSON-RPC 2.0 driven by the same `crate::run_stdio_loop` — with exactly
//! one addition in front of it: a `daemon/attach` handshake that establishes
//! who this client is and whether it owns the daemon.
//!
//! # Endpoint resolution
//!
//! In precedence order (see [`resolve_socket_path`]):
//!
//! 1. an explicit path (`--socket-path`);
//! 2. `$CODEWHALE_HOME/run/daemon.sock` when `CODEWHALE_HOME` is set — an
//!    explicit home is an isolation boundary, so its daemon must not collide
//!    with the default one;
//! 3. `$XDG_RUNTIME_DIR/codewhale/daemon.sock`;
//! 4. macOS: `~/Library/Application Support/codewhale/daemon.sock`;
//! 5. `~/.codewhale/run/daemon.sock`.
//!
//! Windows derives a local named-pipe endpoint from the selected home and
//! current principal. It shares the same dispatcher and ownership handshake;
//! no supported platform falls back to TCP. Windows runtime proof is separate.
//!
//! # Ownership
//!
//! Hermes' claim model, server-side: a client attaches with `mode: "claim"`
//! (it spawned the daemon and will manage its lifetime) or `mode: "attach"`
//! (it found a healthy daemon and is a guest). Only the current owner may
//! `shutdown` the daemon; guests get `not_daemon_owner`. When the owner
//! disconnects the slot frees, so a relaunched shell can re-claim the daemon
//! it left running — sessions survive UI restarts because the daemon does.

use std::path::{Path, PathBuf};

use codewhale_protocol::RuntimeOwnerReceipt;
use serde::{Deserialize, Serialize};

/// Basename of the daemon socket inside the Codewhale runtime directory.
pub const DAEMON_SOCKET_FILE_NAME: &str = "daemon.sock";

/// Historical reserved endpoint used in unsupported-platform diagnostics.
/// Actual Windows owner endpoints are scoped to the selected home/principal.
pub const WINDOWS_NAMED_PIPE: &str = r"\\.\pipe\codewhale-daemon";

/// JSON-RPC method a client must send first on a daemon-socket connection.
pub const ATTACH_METHOD: &str = "daemon/attach";

/// Longest socket path the kernel accepts (`sun_path` minus the NUL).
pub const MAX_SOCKET_PATH_BYTES: usize = if cfg!(any(target_os = "macos", target_os = "ios")) {
    103
} else {
    107
};

/// Typed failures of the daemon socket transport.
#[derive(Debug, thiserror::Error)]
pub enum DaemonSocketError {
    /// The platform has no daemon socket implementation. Never a silent
    /// fallback: the caller must pick another transport explicitly.
    #[error(
        "the daemon socket transport is not supported on {platform}; the reserved endpoint \
         there is the named pipe {planned_endpoint}, which is not implemented yet"
    )]
    UnsupportedPlatform {
        platform: &'static str,
        planned_endpoint: &'static str,
    },
    /// No home directory (or runtime directory) to derive a default path from.
    #[error(
        "cannot resolve the Codewhale runtime directory for the daemon socket: no home directory"
    )]
    RuntimeDirUnavailable,
    /// `CODEWHALE_HOME` is set but not a usable absolute path.
    #[error("invalid CODEWHALE_HOME override: {0}")]
    InvalidHomeOverride(String),
    /// Unix socket paths are limited to roughly one hundred bytes.
    #[error("daemon socket path {} is {len} bytes; this platform allows at most {max}", path.display())]
    PathTooLong {
        path: PathBuf,
        len: usize,
        max: usize,
    },
    /// Something other than a socket already sits at the path. Refused so a
    /// misconfigured path can never delete a user's file.
    #[error("{} exists and is not a unix socket; refusing to remove it", path.display())]
    NotASocket { path: PathBuf },
    /// A daemon answered on the socket: this one must not replace it.
    #[error(
        "a live listener already answers on {}; refusing to replace it (another codewhale daemon, or something else bound to this path)",
        path.display()
    )]
    AlreadyRunning { path: PathBuf },
    /// The liveness probe neither connected nor was refused within the
    /// budget. Refused rather than clobbered; remove the file by hand if the
    /// old daemon is truly gone.
    #[error("liveness probe of {} timed out; refusing to replace a socket that may be live", path.display())]
    ProbeTimedOut { path: PathBuf },
    /// Filesystem or socket I/O failed.
    #[error("{context} ({})", path.display())]
    Io {
        context: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// The app-server state (config, state store, runtime) failed to build.
    #[error("failed to build daemon state")]
    State(#[source] anyhow::Error),
}

/// How to start the daemon socket transport.
#[derive(Debug, Clone, Default)]
pub struct DaemonSocketOptions {
    /// Explicit socket path; `None` resolves the platform default.
    pub socket_path: Option<PathBuf>,
    /// Explicit config file, like `app-server --config`.
    pub config_path: Option<PathBuf>,
}

/// Inputs to [`resolve_socket_path`], separated from the environment so the
/// precedence rules are a pure, testable function.
#[derive(Debug, Clone, Default)]
pub struct SocketPathInputs {
    /// `--socket-path`.
    pub explicit: Option<PathBuf>,
    /// A valid explicit `CODEWHALE_HOME`.
    pub codewhale_home_override: Option<PathBuf>,
    /// `$XDG_RUNTIME_DIR`, when set and non-empty.
    pub xdg_runtime_dir: Option<PathBuf>,
    /// The user's home directory.
    pub user_home: Option<PathBuf>,
    /// Whether the macOS Application Support layout applies.
    pub macos: bool,
}

impl SocketPathInputs {
    /// Capture the live environment.
    pub fn from_environment(explicit: Option<PathBuf>) -> Result<Self, DaemonSocketError> {
        let codewhale_home_override = codewhale_paths::codewhale_home_override()
            .map_err(|err| DaemonSocketError::InvalidHomeOverride(err.to_string()))?;
        let xdg_runtime_dir = std::env::var_os("XDG_RUNTIME_DIR")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        Ok(Self {
            explicit,
            codewhale_home_override,
            xdg_runtime_dir,
            user_home: codewhale_paths::user_home(),
            macos: cfg!(target_os = "macos"),
        })
    }
}

/// Apply the precedence rules documented at the module level and enforce the
/// kernel's path-length limit.
pub fn resolve_socket_path(inputs: &SocketPathInputs) -> Result<PathBuf, DaemonSocketError> {
    let path = if let Some(explicit) = inputs.explicit.clone() {
        explicit
    } else if let Some(home) = inputs.codewhale_home_override.clone() {
        home.join("run").join(DAEMON_SOCKET_FILE_NAME)
    } else if let Some(runtime_dir) = inputs.xdg_runtime_dir.clone() {
        runtime_dir.join("codewhale").join(DAEMON_SOCKET_FILE_NAME)
    } else {
        let user_home = inputs
            .user_home
            .clone()
            .ok_or(DaemonSocketError::RuntimeDirUnavailable)?;
        if inputs.macos {
            user_home
                .join("Library")
                .join("Application Support")
                .join("codewhale")
                .join(DAEMON_SOCKET_FILE_NAME)
        } else {
            user_home
                .join(codewhale_paths::CODEWHALE_APP_DIR)
                .join("run")
                .join(DAEMON_SOCKET_FILE_NAME)
        }
    };
    let len = path.as_os_str().len();
    if len > MAX_SOCKET_PATH_BYTES {
        return Err(DaemonSocketError::PathTooLong {
            path,
            len,
            max: MAX_SOCKET_PATH_BYTES,
        });
    }
    Ok(path)
}

/// The socket path this host would use with no explicit override.
#[cfg(unix)]
pub fn default_socket_path() -> Result<PathBuf, DaemonSocketError> {
    resolve_socket_path(&SocketPathInputs::from_environment(None)?)
}

/// The socket path this host would use with no explicit override.
#[cfg(not(any(unix, windows)))]
pub fn default_socket_path() -> Result<PathBuf, DaemonSocketError> {
    Err(unsupported_platform())
}

/// Who is on the other end of a daemon-socket connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientIdentity {
    /// Product name of the client, e.g. `codewhale-desktop`.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
}

/// Ownership intent carried by `daemon/attach`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachMode {
    /// A guest: use the daemon, never stop it.
    #[default]
    Attach,
    /// The daemon's owner: may `shutdown`. Fails if a live owner exists.
    Claim,
}

/// Role granted by a successful attach.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachRole {
    Owner,
    Attached,
}

/// `daemon/attach` params.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AttachFrontend {
    #[default]
    Control,
    Acp,
    Listener,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AttachParams {
    #[serde(default)]
    pub frontend: AttachFrontend,
    #[serde(default)]
    pub listener: Option<crate::RuntimeListenerSelection>,
    #[serde(default)]
    pub scope: Option<crate::RuntimeFrontendScope>,
    #[serde(default)]
    pub acp_model: Option<String>,
    pub client: ClientIdentity,
    #[serde(default)]
    pub mode: AttachMode,
    /// Bundle-skew guard: when set, the daemon refuses the attach unless its
    /// own version string matches exactly.
    #[serde(default)]
    pub expect_daemon_version: Option<String>,
    #[serde(default)]
    pub expect_owner: Option<RuntimeOwnerReceipt>,
}

/// The typed refusal every non-unix entry point returns. Unused in the unix
/// library build by construction; the tests pin its wording on every host.
#[cfg(any(test, not(any(unix, windows))))]
fn unsupported_platform() -> DaemonSocketError {
    DaemonSocketError::UnsupportedPlatform {
        platform: std::env::consts::OS,
        planned_endpoint: WINDOWS_NAMED_PIPE,
    }
}

/// One bounded blocking boundary for held filesystem/process identity work.
/// The worker retains its permit and captured handles if the waiter cancels.
#[cfg(any(unix, windows))]
pub async fn owner_work<T, F>(work: F) -> anyhow::Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> anyhow::Result<T> + Send + 'static,
{
    static SLOTS: std::sync::OnceLock<std::sync::Arc<tokio::sync::Semaphore>> =
        std::sync::OnceLock::new();
    let permit = SLOTS
        .get_or_init(|| std::sync::Arc::new(tokio::sync::Semaphore::new(4)))
        .clone()
        .acquire_owned()
        .await?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        work()
    })
    .await
    .map_err(anyhow::Error::from)?
}

use crate::{
    AppState, AppTransport, JsonRpcError, ParsedStdioLine, ShutdownAuthority, StdioLoopExit,
    StdioLoopPolicy, dispatch_stdio_request_with_writer, jsonrpc_error, jsonrpc_result,
    params_or_object, parse_params, parse_stdio_line, run_stdio_loop, write_stdio_line,
};
use anyhow::Result;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufRead, AsyncWrite, BufReader};
use tokio::sync::watch;
use tokio::task::JoinSet;

/// Facts about this daemon, reported in every attach reply.
#[derive(Debug)]
struct DaemonInfo {
    pid: u32,
    version: &'static str,
    /// Independent effective principal captured from this process.
    #[cfg(unix)]
    uid: u32,
    socket_path: PathBuf,
    started_at: Instant,
}

#[derive(Debug, Default)]
struct ConnectionRegistry {
    next_id: u64,
    connections: HashMap<u64, ClientIdentity>,
    owner: Option<u64>,
}

impl ConnectionRegistry {
    fn register(&mut self, client: ClientIdentity) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        self.connections.insert(id, client);
        id
    }

    /// Take the owner slot, or report who holds it.
    fn claim(&mut self, id: u64) -> Result<(), ClientIdentity> {
        if let Some(owner_id) = self.owner
            && owner_id != id
            && let Some(owner) = self.connections.get(&owner_id)
        {
            return Err(owner.clone());
        }
        self.owner = Some(id);
        Ok(())
    }

    fn owner(&self) -> Option<ClientIdentity> {
        self.owner.and_then(|id| self.connections.get(&id)).cloned()
    }

    fn remove(&mut self, id: u64) {
        self.connections.remove(&id);
        if self.owner == Some(id) {
            self.owner = None;
        }
    }
}

/// Releases the registry slot (and the owner claim) on drop, whichever
/// way the connection ends.
struct ConnectionGuard {
    frontend: AttachFrontend,
    listener: Option<crate::RuntimeListenerSelection>,
    scope: Option<crate::RuntimeFrontendScope>,
    acp_model: Option<String>,
    registry: Arc<Mutex<ConnectionRegistry>>,
    id: u64,
    role: AttachRole,
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        if let Ok(mut registry) = self.registry.lock() {
            registry.remove(self.id);
        }
    }
}

/// Asks a running [`DaemonSocket::serve`] to stop.
#[derive(Debug, Clone)]
pub struct DaemonShutdownHandle(pub(crate) Arc<watch::Sender<bool>>);

impl DaemonShutdownHandle {
    /// Idempotent; safe to call from any task or signal handler.
    pub fn trigger(&self) {
        self.0.send_replace(true);
    }
}

#[derive(Clone)]
pub(crate) struct ConnectionContext {
    state: AppState,
    registry: Arc<Mutex<ConnectionRegistry>>,
    info: Arc<DaemonInfo>,
    shutdown: DaemonShutdownHandle,
    owner: Option<RuntimeOwnerReceipt>,
}

/// The daemon's connection tasks. A `JoinSet` keeps every finished task
/// until it is joined, so a long-lived daemon used to retain one task per
/// past client (health polls, re-attaches) for its whole lifetime.
/// [`Self::spawn`] is the only way in and reaps finished tasks first, so
/// the set is bounded by the live connections plus those that ended since
/// the last accept. A panicked connection is logged, not fatal.
#[derive(Default)]
pub(crate) struct ConnectionTasks(JoinSet<()>);

impl ConnectionTasks {
    pub(crate) async fn shutdown(&mut self) {
        self.0.shutdown().await;
    }
    pub(crate) fn at_capacity(&mut self) -> bool {
        while let Some(joined) = self.0.try_join_next() {
            if let Err(err) = joined
                && err.is_panic()
            {
                tracing::warn!(error = %err, "daemon connection task panicked");
            }
        }
        self.0.len() >= 64
    }
    pub(crate) fn spawn(&mut self, task: impl std::future::Future<Output = ()> + Send + 'static) {
        if !self.at_capacity() {
            self.0.spawn(task);
        } else {
            tracing::debug!("local control connection ceiling reached; refusing before admission");
        }
    }
}

/// Serve `healthz` and wait for `daemon/attach`; everything else is
/// refused with `attach_required` until the client attaches.
async fn handshake<R, W>(
    context: &ConnectionContext,
    lines: &mut crate::BoundedLines<R>,
    writer: &mut W,
) -> Result<Option<ConnectionGuard>>
where
    R: AsyncBufRead + Unpin,
    W: AsyncWrite + Unpin,
{
    loop {
        let Some(line) = lines.next_line().await? else {
            return Ok(None);
        };
        let request = match parse_stdio_line(&line) {
            ParsedStdioLine::Blank => continue,
            ParsedStdioLine::Rejected(response) => {
                write_stdio_line(writer, &response).await?;
                continue;
            }
            ParsedStdioLine::Request(request) => request,
        };
        let id = request.id.clone();
        match request.method.as_str() {
            "healthz" | "app/healthz" => {
                let response = match dispatch_stdio_request_with_writer(
                    &context.state,
                    writer,
                    &request.method,
                    request.params,
                    AppTransport::Socket,
                )
                .await
                {
                    Ok(dispatch) => jsonrpc_result(id, dispatch.result),
                    Err(err) => jsonrpc_error(id, err),
                };
                write_stdio_line(writer, &response).await?;
            }
            ATTACH_METHOD => match attach(context, request.params).await {
                Ok((result, guard)) => {
                    write_stdio_line(writer, &jsonrpc_result(id, result)).await?;
                    return Ok(Some(guard));
                }
                Err(err) => write_stdio_line(writer, &jsonrpc_error(id, err)).await?,
            },
            other => {
                write_stdio_line(
                    writer,
                    &jsonrpc_error(id, JsonRpcError::attach_required(other)),
                )
                .await?;
            }
        }
    }
}

async fn attach(
    context: &ConnectionContext,
    params: Value,
) -> Result<(Value, ConnectionGuard), JsonRpcError> {
    let params: AttachParams = parse_params(params_or_object(params))?;
    if params.frontend == AttachFrontend::Acp
        && (context.state.owner_frontend.is_none()
            || !context
                .state
                .captured_routing
                .as_ref()
                .is_some_and(|routing| routing.acp))
    {
        return Err(JsonRpcError::invalid_params(
            "held owner has no narrowed ACP projection; cross-profile attachment refused",
        ));
    }
    if (params.frontend == AttachFrontend::Listener) != params.listener.is_some() {
        return Err(JsonRpcError::invalid_params(
            "listener selection does not match frontend",
        ));
    }
    if let Some(listener) = params.listener.as_ref() {
        listener.validate_bounds().map_err(|_| {
            JsonRpcError::invalid_params("selected frontend input exceeds its bounds")
        })?;
        if context.state.owner_frontend.is_none() {
            return Err(JsonRpcError::invalid_params(
                "held owner has no selected listener projection",
            ));
        }
    }
    if params.scope.is_some()
        && (params.frontend == AttachFrontend::Listener || context.state.owner_frontend.is_none())
        || params.acp_model.is_some() && params.frontend != AttachFrontend::Acp
    {
        return Err(JsonRpcError::invalid_params(
            "selected scope does not match captured frontend",
        ));
    }
    if let Some(scope) = params.scope.as_ref() {
        scope.validate_bounds().map_err(|_| {
            JsonRpcError::invalid_params("selected frontend scope exceeds its bounds")
        })?;
    }
    if params
        .acp_model
        .as_ref()
        .is_some_and(|model| model.trim().is_empty() || model.len() > 1024)
    {
        return Err(JsonRpcError::invalid_params("invalid selected ACP model"));
    }
    if let Some(owner) = context.owner.as_ref() {
        if params.expect_owner.as_ref() != Some(owner) {
            return Err(JsonRpcError::invalid_params(
                "captured owner generation or store does not match",
            ));
        }
        if params.mode == AttachMode::Claim {
            return Err(JsonRpcError::not_daemon_owner());
        }
    }
    if params.client.name.trim().is_empty() {
        return Err(JsonRpcError::invalid_params(
            "client.name must not be empty",
        ));
    }
    if let Some(expected) = params.expect_daemon_version.as_deref()
        && expected != context.info.version
    {
        return Err(JsonRpcError::daemon_version_skew(
            expected,
            context.info.version,
        ));
    }

    if let Some(frontend) = context.state.owner_frontend.as_ref() {
        let selection = match params.frontend {
            AttachFrontend::Listener => Some(crate::RuntimeOwnerFrontendSelection::Listener(
                params.listener.clone().expect("validated listener"),
            )),
            AttachFrontend::Acp => Some(crate::RuntimeOwnerFrontendSelection::Acp {
                scope: params.scope.clone(),
                model: params.acp_model.clone(),
            }),
            AttachFrontend::Control => params
                .scope
                .clone()
                .map(crate::RuntimeOwnerFrontendSelection::Control),
        };
        if let Some(selection) = selection {
            frontend
                .validate_selection(&selection)
                .await
                .map_err(|error| JsonRpcError::invalid_params(error.to_string()))?;
        }
    }
    let mut registry = context
        .registry
        .lock()
        .map_err(|_| JsonRpcError::internal("daemon connection registry poisoned"))?;
    let id = registry.register(params.client.clone());
    let role = match params.mode {
        AttachMode::Attach => AttachRole::Attached,
        AttachMode::Claim => match registry.claim(id) {
            Ok(()) => AttachRole::Owner,
            Err(owner) => {
                registry.remove(id);
                let owner = serde_json::to_value(owner)
                    .map_err(|err| JsonRpcError::internal(err.to_string()))?;
                return Err(JsonRpcError::daemon_already_claimed(&owner));
            }
        },
    };
    let owner = registry.owner();
    let connections = registry.connections.len();
    drop(registry);

    let info = &context.info;
    let result = json!({
        "attached": true,
        "connection_id": id,
        "role": role,
        "transport": AppTransport::Socket.label(),
        "daemon": {
            "service": crate::legacy_deepseek_compat::SERVICE_NAME,
            "pid": info.pid,
            "version": info.version,
            "socket_path": info.socket_path.display().to_string(),
            "uptime_ms": u64::try_from(info.started_at.elapsed().as_millis()).unwrap_or(u64::MAX),
        },
        "owner": owner,
        "connections": connections,
        "owner_receipt": context.owner,
        "runtime_routing": context.state.captured_routing,
    });
    Ok((
        result,
        ConnectionGuard {
            frontend: params.frontend,
            listener: params.listener,
            scope: params.scope,
            acp_model: params.acp_model,
            registry: Arc::clone(&context.registry),
            id,
            role,
        },
    ))
}

pub(crate) struct AuthorizedPeer {
    pub(crate) pid: u32,
    pub(crate) start: String,
    #[cfg(windows)]
    pub(crate) process: codewhale_config::windows_identity::WindowsPeerProcess,
}

impl AuthorizedPeer {
    pub(crate) fn check(&self) -> Result<()> {
        #[cfg(unix)]
        anyhow::ensure!(
            codewhale_config::private_directory::unix_process_start(self.pid)? == self.start,
            "local control peer generation changed"
        );
        #[cfg(windows)]
        {
            self.process.check_current_user()?;
            anyhow::ensure!(
                self.process.pid() == self.pid,
                "Windows kernel peer PID changed"
            );
            anyhow::ensure!(
                self.process.start() == self.start,
                "local control peer generation changed"
            );
        }
        Ok(())
    }
}

pub(crate) async fn connection_context(
    state: AppState,
    path: PathBuf,
    shutdown: DaemonShutdownHandle,
    owner: Option<RuntimeOwnerReceipt>,
) -> Result<ConnectionContext> {
    Ok(ConnectionContext {
        state,
        registry: Arc::new(Mutex::new(ConnectionRegistry::default())),
        info: Arc::new(DaemonInfo {
            pid: std::process::id(),
            version: env!("CARGO_PKG_VERSION"),
            #[cfg(unix)]
            uid: codewhale_config::private_directory::PrivateDirectory::current_user_id(),
            socket_path: path,
            started_at: Instant::now(),
        }),
        shutdown,
        owner,
    })
}

pub(crate) async fn run_authorized_connection<R, W>(
    context: ConnectionContext,
    read: R,
    mut writer: W,
    peer: AuthorizedPeer,
) where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let peer = Arc::new(peer);
    let check = peer.clone();
    if owner_work(move || check.check()).await.is_err() {
        return;
    }
    let mut lines = crate::BoundedLines::new(BufReader::new(read));
    let guard = match tokio::time::timeout(
        Duration::from_secs(5),
        handshake(&context, &mut lines, &mut writer),
    )
    .await
    {
        Err(_) => return,
        Ok(result) => match result {
            Ok(Some(guard)) => guard,
            Ok(None) => return,
            Err(error) => {
                tracing::debug!(%error,"local control handshake ended");
                return;
            }
        },
    };
    let check = peer.clone();
    if owner_work(move || check.check()).await.is_err() {
        return;
    }
    if guard.frontend != AttachFrontend::Control || guard.scope.is_some() {
        let Some(frontend) = context.state.owner_frontend.as_ref() else {
            return;
        };
        let selection = match guard.frontend {
            AttachFrontend::Listener => crate::RuntimeOwnerFrontendSelection::Listener(
                guard.listener.clone().expect("admitted listener"),
            ),
            AttachFrontend::Acp => crate::RuntimeOwnerFrontendSelection::Acp {
                scope: guard.scope.clone(),
                model: guard.acp_model.clone(),
            },
            AttachFrontend::Control => crate::RuntimeOwnerFrontendSelection::Control(
                guard.scope.clone().expect("admitted scope"),
            ),
        };
        let _claim = (guard, peer);
        if let Err(error) = frontend
            .serve(
                selection,
                context.state.clone(),
                Box::new(lines.into_inner()),
                Box::new(writer),
            )
            .await
        {
            tracing::debug!(%error,"captured frontend connection ended");
        }
        return;
    }
    let policy = StdioLoopPolicy {
        transport: AppTransport::Socket,
        shutdown: match guard.role {
            AttachRole::Owner => ShutdownAuthority::Granted,
            AttachRole::Attached => ShutdownAuthority::Denied,
        },
    };
    match run_stdio_loop(&context.state, lines, writer, policy, Some((guard, peer))).await {
        Ok(StdioLoopExit::Shutdown) => context.shutdown.trigger(),
        Ok(StdioLoopExit::InputClosed) => {}
        Err(error) => tracing::debug!(%error,"local control connection ended"),
    }
}

#[cfg(unix)]
mod platform {
    use std::fs::File;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::time::Duration;

    use anyhow::Result;
    use codewhale_config::private_directory::{PrivateDirectory, PrivateSocketIdentity};
    use tokio::net::{UnixListener, UnixStream};
    use tokio::sync::watch;

    #[cfg(test)]
    use super::{ClientIdentity, ConnectionRegistry};
    use super::{
        ConnectionContext, ConnectionTasks, DaemonShutdownHandle, DaemonSocketError,
        DaemonSocketOptions, SocketPathInputs, resolve_socket_path,
    };
    use crate::{AppState, AppTransport, build_state_off_runtime};

    /// How long the stale-socket probe waits for a connect to resolve.
    const PROBE_TIMEOUT: Duration = Duration::from_secs(1);

    /// Removes the socket file when the server stops, however it stops.
    struct SocketFileGuard {
        parent: Arc<PrivateDirectory>,
        name: String,
        identity: PrivateSocketIdentity,
        receipt: Option<(String, File)>,
        retired: bool,
    }

    impl SocketFileGuard {
        fn retirement(&mut self) -> Option<impl FnOnce() -> Result<()> + Send + 'static> {
            if self.retired {
                return None;
            }
            self.retired = true;
            let parent = self.parent.clone();
            let name = self.name.clone();
            let identity = self.identity;
            let receipt = self.receipt.take();
            Some(move || {
                if let Some((name, file)) = receipt {
                    parent.retire_private_receipt(&name, &file)?;
                }
                parent.retire_socket(&name, identity)?;
                Ok(())
            })
        }

        async fn retire(&mut self) -> Result<()> {
            if let Some(retire) = self.retirement() {
                tokio::spawn(async move { super::owner_work(retire).await })
                    .await
                    .map_err(anyhow::Error::from)??;
            }
            Ok(())
        }
    }

    impl Drop for SocketFileGuard {
        fn drop(&mut self) {
            let Some(retire) = self.retirement() else {
                return;
            };
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                handle.spawn(async move {
                    if let Err(error) = super::owner_work(retire).await {
                        tracing::warn!(%error, "private endpoint retirement is uncertain; retaining it");
                    }
                });
            } else if let Err(error) = retire() {
                tracing::warn!(%error, "private endpoint retirement is uncertain; retaining it");
            }
        }
    }

    /// A bound, not yet serving, daemon socket.
    pub struct DaemonSocket {
        listener: UnixListener,
        path: PathBuf,
        state: AppState,
        shutdown: Arc<watch::Sender<bool>>,
        socket_file: SocketFileGuard,
        owner: Option<super::RuntimeOwnerReceipt>,
    }

    impl DaemonSocket {
        /// Where clients connect.
        #[must_use]
        pub fn local_path(&self) -> &Path {
            &self.path
        }

        /// A handle that stops [`Self::serve`] from outside (signals, tests).
        #[must_use]
        pub fn shutdown_handle(&self) -> DaemonShutdownHandle {
            DaemonShutdownHandle(Arc::clone(&self.shutdown))
        }

        /// Accept clients until the owner sends `shutdown` or the handle is
        /// triggered. Removes the socket file on the way out.
        pub async fn serve(self) -> Result<(), DaemonSocketError> {
            let Self {
                listener,
                path,
                state,
                shutdown,
                socket_file,
                owner,
            } = self;
            let mut socket_file = socket_file;
            let context = super::connection_context(
                state,
                path.clone(),
                DaemonShutdownHandle(Arc::clone(&shutdown)),
                owner,
            )
            .await
            .map_err(DaemonSocketError::State)?;
            let mut shutdown_rx = shutdown.subscribe();
            let mut connections = ConnectionTasks::default();

            loop {
                if *shutdown_rx.borrow() {
                    break;
                }
                tokio::select! {
                    accepted = listener.accept() => match accepted {
                        Ok((stream, _)) => {
                            connections.spawn(handle_connection(context.clone(), stream));
                        }
                        Err(err) => {
                            tracing::warn!(error = %err, "daemon socket accept failed");
                            tokio::time::sleep(Duration::from_millis(50)).await;
                        }
                    },
                    changed = shutdown_rx.changed() => {
                        if changed.is_err() || *shutdown_rx.borrow() {
                            break;
                        }
                    }
                }
            }

            // The owner's `shutdown` reply was flushed before its loop
            // returned, so aborting what is left loses nothing a client
            // still needs.
            connections.shutdown().await;
            drop(listener);
            socket_file
                .retire()
                .await
                .map_err(DaemonSocketError::State)?;
            Ok(())
        }
    }

    /// Resolve the path, clear a stale socket, bind with `0600`, and build
    /// the shared app state. Does not accept anything until
    /// [`DaemonSocket::serve`].
    pub async fn bind_daemon_socket(
        options: DaemonSocketOptions,
    ) -> Result<DaemonSocket, DaemonSocketError> {
        bind_with_owner(options, None).await
    }

    pub(crate) async fn bind_captured_owner(
        state: AppState,
        owner: super::RuntimeOwnerReceipt,
    ) -> Result<DaemonSocket, DaemonSocketError> {
        let options = DaemonSocketOptions {
            socket_path: Some(owner.socket_path.clone()),
            config_path: None,
        };
        bind_with_owner(options, Some((state, owner))).await
    }

    async fn bind_with_owner(
        options: DaemonSocketOptions,
        captured: Option<(AppState, super::RuntimeOwnerReceipt)>,
    ) -> Result<DaemonSocket, DaemonSocketError> {
        let path = resolve_socket_path(&SocketPathInputs::from_environment(options.socket_path)?)?;
        let parent_path = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .ok_or(DaemonSocketError::RuntimeDirUnavailable)?
            .to_path_buf();
        let parent = Arc::new(
            super::owner_work(move || PrivateDirectory::admit(&parent_path))
                .await
                .map_err(DaemonSocketError::State)?,
        );
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(DaemonSocketError::RuntimeDirUnavailable)?
            .to_string();
        clear_stale_socket(&path, &parent, &name).await?;
        let check_parent = parent.clone();
        if !super::owner_work(move || check_parent.is_at_selected_path())
            .await
            .map_err(DaemonSocketError::State)?
        {
            return Err(DaemonSocketError::State(anyhow::anyhow!(
                "private endpoint parent changed"
            )));
        }

        let listener = UnixListener::bind(&path).map_err(|source| DaemonSocketError::Io {
            context: "failed to bind the daemon socket",
            path: path.clone(),
            source,
        })?;
        let inspect_parent = parent.clone();
        let inspect_name = name.clone();
        let identity = super::owner_work(move || inspect_parent.socket_identity(&inspect_name))
            .await
            .map_err(DaemonSocketError::State)?
            .ok_or_else(|| {
                DaemonSocketError::State(anyhow::anyhow!("bound socket identity is unavailable"))
            })?;
        let socket_file = SocketFileGuard {
            parent: parent.clone(),
            name,
            identity,
            receipt: None,
            retired: false,
        };
        let protect_parent = parent.clone();
        let protect_name = socket_file.name.clone();
        super::owner_work(move || {
            anyhow::ensure!(
                protect_parent.is_at_selected_path()?,
                "private endpoint parent changed during bind"
            );
            protect_parent.protect_socket(&protect_name, identity)
        })
        .await
        .map_err(DaemonSocketError::State)?;

        let (state, owner) = match captured {
            Some((state, owner)) => (state, Some(owner)),
            None => (
                build_state_off_runtime(options.config_path, None, AppTransport::Socket)
                    .await
                    .map_err(DaemonSocketError::State)?,
                None,
            ),
        };
        let mut socket_file = socket_file;
        if let Some(owner) = owner.as_ref() {
            let receipt_name = format!("{}.owner.json", socket_file.name);
            let bytes = serde_json::to_vec(owner)
                .map_err(|error| DaemonSocketError::State(error.into()))?;
            if bytes.len() > 16384 {
                return Err(DaemonSocketError::State(anyhow::anyhow!(
                    "owner receipt exceeds private publication limit"
                )));
            }
            let parent = parent.clone();
            let name = receipt_name.clone();
            let expected = owner.clone();
            let guard = socket_file;
            socket_file = super::owner_work(move || {
                let mut guard = guard;
                anyhow::ensure!(
                    parent.is_at_selected_path()?,
                    "private owner parent changed before publication"
                );
                if let Some((bytes, old)) = parent.read_private_receipt(&name, 16384)? {
                    let previous: super::RuntimeOwnerReceipt = serde_json::from_slice(&bytes)?;
                    anyhow::ensure!(
                        previous.version == expected.version
                            && previous.data_dir == expected.data_dir
                            && previous.execution_scope == expected.execution_scope
                            && previous.socket_path == expected.socket_path,
                        "stale owner receipt belongs to a different selected store"
                    );
                    anyhow::ensure!(
                        parent.retire_private_receipt(&name, &old)?,
                        "stale owner receipt changed"
                    );
                }
                parent.write_owned_file(&name, &bytes, false)?;
                let (_, file) = parent
                    .read_private_receipt(&name, 16384)?
                    .ok_or_else(|| anyhow::anyhow!("published owner receipt unavailable"))?;
                guard.receipt = Some((name, file));
                anyhow::ensure!(
                    parent.is_at_selected_path()?,
                    "private owner parent changed during publication; retaining uncertain receipt"
                );
                Ok(guard)
            })
            .await
            .map_err(DaemonSocketError::State)?;
        }
        let (shutdown, _) = watch::channel(false);
        Ok(DaemonSocket {
            listener,
            path,
            state,
            shutdown: Arc::new(shutdown),
            socket_file,
            owner,
        })
    }

    /// Only a definite refusal permits exact captured endpoint retirement.
    async fn clear_stale_socket(
        path: &Path,
        parent: &Arc<PrivateDirectory>,
        name: &str,
    ) -> Result<(), DaemonSocketError> {
        let inspect_parent = parent.clone();
        let inspect_name = name.to_string();
        let Some(identity) =
            super::owner_work(move || inspect_parent.socket_identity(&inspect_name))
                .await
                .map_err(|error| {
                    if error
                        .downcast_ref::<std::io::Error>()
                        .is_some_and(|error| error.kind() == std::io::ErrorKind::InvalidInput)
                    {
                        DaemonSocketError::NotASocket {
                            path: path.to_path_buf(),
                        }
                    } else {
                        DaemonSocketError::State(error)
                    }
                })?
        else {
            return Ok(());
        };
        match tokio::time::timeout(PROBE_TIMEOUT, UnixStream::connect(path)).await {
            Ok(Ok(_live)) => Err(DaemonSocketError::AlreadyRunning {
                path: path.to_path_buf(),
            }),
            Ok(Err(error)) if error.kind() == std::io::ErrorKind::ConnectionRefused => {
                let retire_parent = parent.clone();
                let retire_name = name.to_string();
                if super::owner_work(move || retire_parent.retire_socket(&retire_name, identity))
                    .await
                    .map_err(DaemonSocketError::State)?
                {
                    Ok(())
                } else {
                    Err(DaemonSocketError::State(anyhow::anyhow!(
                        "stale endpoint changed; refusing replacement"
                    )))
                }
            }
            Ok(Err(source)) => Err(DaemonSocketError::Io {
                context: "uncertain daemon endpoint probe; refusing replacement",
                path: path.to_path_buf(),
                source,
            }),
            Err(_) => Err(DaemonSocketError::ProbeTimedOut {
                path: path.to_path_buf(),
            }),
        }
    }

    async fn handle_connection(context: ConnectionContext, stream: UnixStream) {
        let credential = match stream.peer_cred() {
            Ok(peer) if peer.uid() == context.info.uid => peer,
            _ => return,
        };
        let Some(pid) = credential.pid().and_then(|pid| u32::try_from(pid).ok()) else {
            return;
        };
        let start = match super::owner_work(move || {
            codewhale_config::private_directory::unix_process_start(pid)
        })
        .await
        {
            Ok(start) => start,
            Err(_) => return,
        };
        let (read, write) = stream.into_split();
        super::run_authorized_connection(
            context,
            read,
            write,
            super::AuthorizedPeer { pid, start },
        )
        .await;
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn client(name: &str) -> ClientIdentity {
            ClientIdentity {
                name: name.to_string(),
                version: None,
                pid: None,
            }
        }

        #[tokio::test]
        async fn finished_connection_tasks_are_reaped() {
            let mut connections = ConnectionTasks::default();
            let mut handles = Vec::new();
            for _ in 0..3 {
                handles.push(connections.0.spawn(async {}));
            }
            handles.push(
                connections
                    .0
                    .spawn(async { panic!("fixture connection panic") }),
            );
            tokio::time::timeout(Duration::from_secs(5), async {
                while !handles.iter().all(tokio::task::AbortHandle::is_finished) {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("fixture connections finish");

            // Accepting the next client reaps every finished connection,
            // including the panicked one, and keeps only the new live one.
            connections.spawn(std::future::pending::<()>());
            assert_eq!(connections.0.len(), 1, "only the live connection is kept");
            connections.0.abort_all();
        }

        #[tokio::test]
        async fn cancelled_owner_waiter_retains_worker_capacity_until_real_completion() {
            let (started, mut observed) = tokio::sync::mpsc::unbounded_channel();
            let mut releases = Vec::new();
            let mut waiters = Vec::new();
            for _ in 0..4 {
                let (release, released) = std::sync::mpsc::channel();
                releases.push(release);
                let started = started.clone();
                waiters.push(tokio::spawn(super::super::owner_work(move || {
                    started.send(()).unwrap();
                    let _ = released.recv();
                    Ok(())
                })));
            }
            for _ in 0..4 {
                tokio::time::timeout(Duration::from_secs(5), observed.recv())
                    .await
                    .unwrap()
                    .unwrap();
            }
            waiters[0].abort();
            let _ = (&mut waiters[0]).await;
            let (next_started, mut next_observed) = tokio::sync::mpsc::unbounded_channel();
            let next = tokio::spawn(super::super::owner_work(move || {
                next_started.send(()).unwrap();
                Ok(())
            }));
            tokio::task::yield_now().await;
            assert!(matches!(
                next_observed.try_recv(),
                Err(tokio::sync::mpsc::error::TryRecvError::Empty)
            ));
            releases.remove(0).send(()).unwrap();
            tokio::time::timeout(Duration::from_secs(5), next_observed.recv())
                .await
                .unwrap()
                .unwrap();
            for release in releases {
                release.send(()).unwrap();
            }
            for waiter in waiters.into_iter().skip(1) {
                waiter.await.unwrap().unwrap();
            }
            next.await.unwrap().unwrap();
        }

        #[test]
        fn registry_claim_is_exclusive_until_the_owner_leaves() {
            let mut registry = ConnectionRegistry::default();
            let first = registry.register(client("desktop-a"));
            let second = registry.register(client("desktop-b"));

            assert!(registry.claim(first).is_ok());
            assert_eq!(registry.claim(second), Err(client("desktop-a")));
            assert!(
                registry.claim(first).is_ok(),
                "re-claim by the owner is idempotent"
            );

            registry.remove(first);
            assert_eq!(registry.owner(), None);
            assert!(registry.claim(second).is_ok());
            assert_eq!(registry.owner(), Some(client("desktop-b")));
        }

        #[test]
        fn removing_a_guest_keeps_the_owner() {
            let mut registry = ConnectionRegistry::default();
            let owner = registry.register(client("owner"));
            let guest = registry.register(client("guest"));
            registry.claim(owner).expect("claim");
            registry.remove(guest);
            assert_eq!(registry.owner(), Some(client("owner")));
            assert_eq!(registry.connections.len(), 1);
        }
    }
}

#[cfg(not(any(unix, windows)))]
mod platform {
    use std::path::Path;

    use super::{
        DaemonShutdownHandle, DaemonSocketError, DaemonSocketOptions, unsupported_platform,
    };

    /// Placeholder until the Windows named pipe lands; cannot be constructed.
    pub struct DaemonSocket {
        never: std::convert::Infallible,
    }

    impl DaemonSocket {
        #[must_use]
        pub fn local_path(&self) -> &Path {
            match self.never {}
        }

        #[must_use]
        pub fn shutdown_handle(&self) -> DaemonShutdownHandle {
            match self.never {}
        }

        pub async fn serve(self) -> Result<(), DaemonSocketError> {
            match self.never {}
        }
    }

    /// Always [`DaemonSocketError::UnsupportedPlatform`] here.
    pub async fn bind_daemon_socket(
        _options: DaemonSocketOptions,
    ) -> Result<DaemonSocket, DaemonSocketError> {
        Err(unsupported_platform())
    }
}

#[cfg(windows)]
use crate::daemon_windows as platform;
pub use platform::{DaemonSocket, bind_daemon_socket};

/// `codewhale app-server --socket`: bind, announce, serve until the owner's
/// `shutdown` or a termination signal.
pub async fn run_daemon_socket(options: DaemonSocketOptions) -> anyhow::Result<()> {
    let daemon = bind_daemon_socket(options).await?;
    let path: &Path = daemon.local_path();
    tracing::info!(path = %path.display(), "codewhale daemon listening on unix socket");
    eprintln!("codewhale daemon: listening on {}", path.display());

    let handle = daemon.shutdown_handle();
    tokio::spawn(async move {
        crate::shutdown_signal().await;
        handle.trigger();
    });
    daemon.serve().await?;
    Ok(())
}

#[cfg(any(unix, windows))]
pub(crate) use platform::bind_captured_owner;

#[cfg(unix)]
pub async fn capture_process_start(pid: u32) -> anyhow::Result<String> {
    owner_work(move || codewhale_config::private_directory::unix_process_start(pid)).await
}

#[cfg(windows)]
pub fn default_socket_path() -> Result<PathBuf, DaemonSocketError> {
    crate::daemon_windows::selected_pipe_path(None).map_err(DaemonSocketError::State)
}

#[cfg(windows)]
pub async fn capture_process_start(pid: u32) -> anyhow::Result<String> {
    owner_work(move || {
        Ok(
            codewhale_config::windows_identity::WindowsPeerProcess::open_current_user(pid)?
                .start()
                .to_string(),
        )
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs() -> SocketPathInputs {
        SocketPathInputs {
            explicit: None,
            codewhale_home_override: None,
            xdg_runtime_dir: None,
            user_home: Some(PathBuf::from("/home/whale")),
            macos: false,
        }
    }

    #[test]
    fn explicit_path_wins() {
        let resolved = resolve_socket_path(&SocketPathInputs {
            explicit: Some(PathBuf::from("/tmp/x.sock")),
            codewhale_home_override: Some(PathBuf::from("/iso")),
            xdg_runtime_dir: Some(PathBuf::from("/run/user/1000")),
            ..inputs()
        })
        .expect("resolve");
        assert_eq!(resolved, PathBuf::from("/tmp/x.sock"));
    }

    #[test]
    fn explicit_codewhale_home_isolates_the_daemon() {
        let resolved = resolve_socket_path(&SocketPathInputs {
            codewhale_home_override: Some(PathBuf::from("/iso/home")),
            xdg_runtime_dir: Some(PathBuf::from("/run/user/1000")),
            ..inputs()
        })
        .expect("resolve");
        assert_eq!(resolved, PathBuf::from("/iso/home/run/daemon.sock"));
    }

    #[test]
    fn xdg_runtime_dir_beats_home_layouts() {
        let resolved = resolve_socket_path(&SocketPathInputs {
            xdg_runtime_dir: Some(PathBuf::from("/run/user/1000")),
            macos: true,
            ..inputs()
        })
        .expect("resolve");
        assert_eq!(
            resolved,
            PathBuf::from("/run/user/1000/codewhale/daemon.sock")
        );
    }

    #[test]
    fn macos_defaults_to_application_support() {
        let resolved = resolve_socket_path(&SocketPathInputs {
            macos: true,
            user_home: Some(PathBuf::from("/Users/whale")),
            ..inputs()
        })
        .expect("resolve");
        assert_eq!(
            resolved,
            PathBuf::from("/Users/whale/Library/Application Support/codewhale/daemon.sock")
        );
    }

    #[test]
    fn linux_defaults_to_dot_codewhale_run() {
        let resolved = resolve_socket_path(&inputs()).expect("resolve");
        assert_eq!(
            resolved,
            PathBuf::from("/home/whale/.codewhale/run/daemon.sock")
        );
    }

    #[test]
    fn no_home_is_a_typed_error() {
        let err = resolve_socket_path(&SocketPathInputs {
            user_home: None,
            ..inputs()
        })
        .expect_err("must fail");
        assert!(
            matches!(err, DaemonSocketError::RuntimeDirUnavailable),
            "{err}"
        );
    }

    #[test]
    fn over_long_paths_are_refused_before_bind() {
        let long = PathBuf::from(format!(
            "/{}/daemon.sock",
            "d".repeat(MAX_SOCKET_PATH_BYTES)
        ));
        let err = resolve_socket_path(&SocketPathInputs {
            explicit: Some(long.clone()),
            ..inputs()
        })
        .expect_err("must fail");
        match err {
            DaemonSocketError::PathTooLong { path, len, max } => {
                assert_eq!(path, long);
                assert!(len > max);
                assert_eq!(max, MAX_SOCKET_PATH_BYTES);
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn unsupported_platform_error_names_the_named_pipe() {
        let err = unsupported_platform();
        let text = err.to_string();
        assert!(text.contains(WINDOWS_NAMED_PIPE), "{text}");
        assert!(text.contains("not implemented"), "{text}");
    }

    #[test]
    fn attach_mode_defaults_to_guest() {
        let params: AttachParams =
            serde_json::from_value(serde_json::json!({ "client": { "name": "x" } }))
                .expect("parse");
        assert_eq!(params.mode, AttachMode::Attach);
        assert_eq!(params.expect_daemon_version, None);
    }
}
