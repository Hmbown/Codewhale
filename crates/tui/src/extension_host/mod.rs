//! Experimental TypeScript extension host — phase 1 (`[features] extension_host`).
//!
//! Codewhale's Rust core stays closed and authoritative: one turn loop, one
//! event authority, one store, one prompt authority, one approval gate. The
//! extension host (`crates/tui/extension-host`, a Node process running the
//! embedded bundle) is the only extensible surface, and in phase 1 it can do
//! exactly one thing: contribute tools, which become ordinary registry
//! `ToolSpec`s ([`tool::HostToolSpec`]) behind the existing gate.
//!
//! Lifecycle: a reviewed, enabled plugin with a `native` entry (activation
//! policy v4, selected by the flag) makes [`ExtensionHostManager::sync`]
//! spawn the host in the background — never on the first-prompt path — and
//! activate one owner per plugin. Tools join the per-turn registry at the next
//! rebuild, deferred. Disabling, revoking or updating the plugin revokes its
//! registrations synchronously before the host is asked to tear down.
//!
//! Known limitations (phase 1, by design — see the design doc §8):
//! * Tools only: no commands, hooks, skills, prompt sections, MCP, or
//!   `core/call` (the host cannot ask the core to do anything).
//! * No heartbeat and no auto-restart. A dead host fails in-flight calls with
//!   a typed error and stays failed until the next session or until a plugin
//!   the session has not seen before becomes desired. A teardown that times
//!   out or reports leaks is logged in `/plugin`; the plugin's leftover
//!   JavaScript keeps running until the host process ends.
//! * One host per engine process and one trust tier. On macOS (Seatbelt) the
//!   host has no direct network, and cannot read the Codewhale home (except
//!   the bundle, its data dir and plugin code), the Codex and DSH credential
//!   homes, or the default credential stores (`supervisor::plan_launch`).
//!   Other files the user can read — including project `.env` files — stay
//!   readable, and Mach services are not restricted. On Linux and Windows it
//!   runs unsandboxed with the user's permissions. Either way the flag is
//!   Experimental.
//! * The owner token is a bug/staleness guard, not a boundary between
//!   plugins that share the process: one plugin can alter another's
//!   behaviour, which the approval card discloses.
//! * Extension tool names that any name-keyed approval table special-cases
//!   are refused (`registry::core_special_case`), so an extension tool never
//!   shares an approval key, summary or category with a built-in.
//! * The host's process tree (Unix process group / Windows Job Object,
//!   shared with hooks via `crate::process_tree`) is killed as a whole. On
//!   Windows the host is assigned to its job just after spawn, not created
//!   suspended as hooks are. On Unix a plugin child that calls `setsid` leaves
//!   the group and is not killed with it. When the core goes away, the host
//!   kills its own group at stdin EOF, and a watchdog thread does the same
//!   when its parent process changes, even if a plugin blocks the event loop.
//! * One manager per process: `sync` reconciles against the calling engine's
//!   plugin registry, so engines for different workspaces in one process
//!   would revoke each other's plugins. Only one workspace runs per process
//!   today.
//! * The host re-hashes each `native` entry file before importing it; other
//!   files in the staged snapshot are covered by Rust's per-call receipt
//!   check, not re-hashed by the host.

pub(crate) mod protocol;
pub(crate) mod registry;
pub(crate) mod supervisor;
pub(crate) mod tool;

#[cfg(test)]
pub(crate) mod tests;

use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};

use serde_json::json;
use sha2::{Digest, Sha256};

use self::protocol::{
    ActivateParams, ActivateResult, CoreRequest, DeactivateParams, DeactivateResult, EntryRef,
    OwnerRef, RegisterResult,
};
use self::registry::{OwnerRegistry, OwnerState, ToolRegistration};
use self::supervisor::{ACTIVATE_DEADLINE, DISPOSE_DEADLINE, HostEvents, HostProcess};
use crate::plugins::PluginRegistry;
use crate::plugins::activation::{self, PluginActivationCapability};
use crate::plugins::types::PluginAuthority;

/// The host bundle, embedded so every distribution channel carries it.
const BUNDLE: &[u8] = include_bytes!("../../extension-host/dist/codewhale-extension-host.mjs");
const BUNDLE_FILE_NAME: &str = "codewhale-extension-host.mjs";
const MAX_DIAGNOSTICS: usize = 64;

fn hex(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// SHA-256 of the embedded bundle.
#[must_use]
pub fn bundle_sha256() -> &'static str {
    static DIGEST: OnceLock<String> = OnceLock::new();
    DIGEST.get_or_init(|| hex(Sha256::digest(BUNDLE)))
}

/// Write the embedded bundle to `<root>/extension-host/<sha256>/` unless an
/// identical copy is already there, then re-verify the bytes on disk. The
/// file is never overwritten in place: a different build writes a different
/// directory. Blocking.
fn materialize_bundle(root: &Path) -> Result<PathBuf, String> {
    let digest = bundle_sha256();
    let dir = supervisor::bundle_dir(root, digest);
    let path = dir.join(BUNDLE_FILE_NAME);
    let matches = |path: &Path| {
        std::fs::read(path)
            .map(|bytes| hex(Sha256::digest(&bytes)) == digest)
            .unwrap_or(false)
    };
    if !matches(&path) {
        std::fs::create_dir_all(&dir)
            .map_err(|error| format!("cannot create {}: {error}", dir.display()))?;
        let staging = dir.join(format!(
            ".{BUNDLE_FILE_NAME}.{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::write(&staging, BUNDLE)
            .map_err(|error| format!("cannot write {}: {error}", staging.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&staging, std::fs::Permissions::from_mode(0o400));
        }
        if let Err(error) = std::fs::rename(&staging, &path) {
            let _ = std::fs::remove_file(&staging);
            if !matches(&path) {
                return Err(format!("cannot publish {}: {error}", path.display()));
            }
        }
    }
    // Re-check what will actually be executed.
    if !matches(&path) {
        return Err(format!(
            "{} does not match the embedded bundle digest {digest}",
            path.display()
        ));
    }
    Ok(path)
}

/// Options fixed for the life of one manager.
#[derive(Debug, Clone, Default)]
pub struct ExtensionHostOptions {
    /// `[extension_host] node`: tried before every `node` on `PATH`.
    pub node_override: Option<PathBuf>,
    /// Where the bundle is materialized; defaults to the Codewhale home.
    pub root: Option<PathBuf>,
}

/// Observable host state, for `/plugin`, doctor and tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostStatus {
    Idle,
    Starting,
    Ready {
        pid: Option<u32>,
        node_version: String,
        /// `seatbelt` / `bwrap`, or `None` when the host runs unsandboxed.
        sandbox: Option<String>,
    },
    Failed {
        reason: String,
        stderr_tail: String,
    },
}

enum HostSlot {
    Idle,
    Starting,
    Ready(Arc<HostProcess>),
    Failed { reason: String, stderr_tail: String },
}

struct DesiredOwner {
    plugin_name: String,
    authority: PluginAuthority,
    entries: Vec<(PathBuf, String)>,
}

pub(crate) struct ManagerShared {
    options: ExtensionHostOptions,
    registry: Mutex<OwnerRegistry>,
    host: Mutex<HostSlot>,
    host_generation: AtomicU64,
    spawn_attempts: AtomicU64,
    sync_lock: tokio::sync::Mutex<()>,
    diagnostics: Mutex<VecDeque<String>>,
    /// Plugin ids this session has already tried; a new one clears `Failed`.
    seen_plugins: Mutex<BTreeSet<String>>,
}

impl ManagerShared {
    fn diagnostic(&self, message: String) {
        tracing::info!(target: "extension_host", "{message}");
        let mut diagnostics = self.diagnostics.lock().expect("diagnostics lock");
        diagnostics.push_back(message);
        while diagnostics.len() > MAX_DIAGNOSTICS {
            diagnostics.pop_front();
        }
    }

    fn ready_host(&self) -> Option<Arc<HostProcess>> {
        match &*self.host.lock().expect("host lock") {
            HostSlot::Ready(host) if !host.has_exited() => Some(Arc::clone(host)),
            _ => None,
        }
    }

    /// Re-check everything a call depends on, immediately before it is sent:
    /// exact owner generation, the reviewed receipt and staged bytes, the
    /// Native adapter in this build's policy, and a running host.
    pub(crate) async fn live_host_for(
        &self,
        registration: &ToolRegistration,
    ) -> Result<Arc<HostProcess>, String> {
        let authority = {
            let registry = self.registry.lock().expect("registry lock");
            if !registry.is_live(registration.handle, &registration.owner) {
                return Err(format!(
                    "extension tool `{}` from `{}` is no longer registered",
                    registration.name, registration.plugin_name
                ));
            }
            registry
                .authority_for(&registration.owner)
                .ok_or_else(|| "extension owner has no authority".to_string())?
        };
        let policy = activation::extension_host_policy_enabled();
        tokio::task::spawn_blocking(move || {
            let _scope = activation::PolicyScope::propagate(policy);
            crate::plugins::registry::verify_plugin_component_authority(
                &authority,
                PluginActivationCapability::Native,
            )
        })
        .await
        .map_err(|error| format!("authority check failed: {error}"))??;
        self.ready_host()
            .ok_or_else(|| "the extension host is not running".to_string())
    }
}

/// Channel callbacks. Holds a `Weak` so the host process (which owns the
/// callbacks) never keeps the manager alive.
struct Events(Weak<ManagerShared>);

impl HostEvents for Events {
    fn register(&self, params: &protocol::RegisterParams) -> RegisterResult {
        let Some(shared) = self.0.upgrade() else {
            return RegisterResult::Refused {
                refused: "extension host manager is gone".to_string(),
            };
        };
        let result = shared
            .registry
            .lock()
            .expect("registry lock")
            .register_tool(params);
        match result {
            Ok(handle) => RegisterResult::Admitted { handle },
            Err(reason) => {
                shared.diagnostic(format!(
                    "extension `{}` tool `{}` refused: {reason}",
                    params.owner.plugin_id, params.spec.name
                ));
                RegisterResult::Refused { refused: reason }
            }
        }
    }

    fn unregister(&self, params: &protocol::UnregisterParams) {
        if let Some(shared) = self.0.upgrade() {
            shared
                .registry
                .lock()
                .expect("registry lock")
                .unregister(&params.owner, params.handle);
        }
    }

    fn faulted(&self, params: &protocol::FaultedParams) {
        let Some(shared) = self.0.upgrade() else {
            return;
        };
        shared
            .registry
            .lock()
            .expect("registry lock")
            .mark_failed(&params.owner, OwnerState::Faulted(params.error.clone()));
        if let Some(host) = shared.ready_host() {
            host.revoke_calls_of(&params.owner.plugin_id);
        }
        shared.diagnostic(format!(
            "extension `{}` faulted and was disposed: {}",
            params.owner.plugin_id, params.error
        ));
    }

    fn exited(&self, host_generation: u64, reason: String, stderr_tail: String) {
        let Some(shared) = self.0.upgrade() else {
            return;
        };
        if shared.host_generation.load(Ordering::SeqCst) != host_generation {
            return;
        }
        shared
            .registry
            .lock()
            .expect("registry lock")
            .revoke_all(&format!("extension host exited: {reason}"));
        {
            let mut slot = shared.host.lock().expect("host lock");
            if !matches!(&*slot, HostSlot::Failed { .. } | HostSlot::Idle) {
                *slot = HostSlot::Failed {
                    reason: reason.clone(),
                    stderr_tail: stderr_tail.clone(),
                };
            }
        }
        shared.diagnostic(format!("extension host {reason}"));
    }
}

/// Supervises at most one extension host for this engine process.
pub struct ExtensionHostManager {
    shared: Arc<ManagerShared>,
}

impl ExtensionHostManager {
    #[must_use]
    pub fn new(options: ExtensionHostOptions) -> Self {
        Self {
            shared: Arc::new(ManagerShared {
                options,
                registry: Mutex::new(OwnerRegistry::new()),
                host: Mutex::new(HostSlot::Idle),
                host_generation: AtomicU64::new(0),
                spawn_attempts: AtomicU64::new(0),
                sync_lock: tokio::sync::Mutex::new(()),
                diagnostics: Mutex::new(VecDeque::new()),
                seen_plugins: Mutex::new(BTreeSet::new()),
            }),
        }
    }

    #[must_use]
    pub fn status(&self) -> HostStatus {
        match &*self.shared.host.lock().expect("host lock") {
            HostSlot::Idle => HostStatus::Idle,
            HostSlot::Starting => HostStatus::Starting,
            HostSlot::Ready(host) => HostStatus::Ready {
                pid: host.pid,
                node_version: host.node_version.get().cloned().unwrap_or_default(),
                sandbox: host.sandbox.clone(),
            },
            HostSlot::Failed {
                reason,
                stderr_tail,
            } => HostStatus::Failed {
                reason: reason.clone(),
                stderr_tail: stderr_tail.clone(),
            },
        }
    }

    /// How many times this manager has tried to start a host process.
    #[must_use]
    pub fn spawn_attempts(&self) -> u64 {
        self.shared.spawn_attempts.load(Ordering::SeqCst)
    }

    #[must_use]
    pub fn diagnostics(&self) -> Vec<String> {
        self.shared
            .diagnostics
            .lock()
            .expect("diagnostics lock")
            .iter()
            .cloned()
            .collect()
    }

    #[cfg(test)]
    #[must_use]
    pub fn live_tool_names(&self) -> Vec<String> {
        self.shared
            .registry
            .lock()
            .expect("registry lock")
            .live_tools()
            .into_iter()
            .map(|tool| tool.name)
            .collect()
    }

    #[cfg(test)]
    #[must_use]
    pub fn owner_state(&self, plugin_id: &str) -> Option<OwnerState> {
        self.shared
            .registry
            .lock()
            .expect("registry lock")
            .owner(plugin_id)
            .map(|entry| entry.state.clone())
    }

    /// A new session may retry a host that failed in an earlier one.
    pub fn begin_session(&self) {
        {
            let mut slot = self.shared.host.lock().expect("host lock");
            if matches!(&*slot, HostSlot::Failed { .. }) {
                *slot = HostSlot::Idle;
            }
        }
        self.shared
            .registry
            .lock()
            .expect("registry lock")
            .forget_inactive();
        self.shared.seen_plugins.lock().expect("seen lock").clear();
    }

    /// Record the native tool names (the registry before scripts, plugins and
    /// extensions are added) so `registry/register` refuses collisions.
    pub fn note_native_names<'a>(&self, names: impl IntoIterator<Item = &'a str>) {
        self.shared
            .registry
            .lock()
            .expect("registry lock")
            .set_native_names(names);
    }

    /// Add every live extension tool to `tool_registry`, *after* natives and
    /// `~/.codewhale/tools` scripts. A name already present is skipped with a
    /// diagnostic — `ToolRegistry::register` would silently overwrite it.
    /// Returns the names added.
    pub fn install_tools(&self, tool_registry: &mut crate::tools::ToolRegistry) -> Vec<String> {
        let tools = self
            .shared
            .registry
            .lock()
            .expect("registry lock")
            .live_tools();
        if tools.is_empty() {
            return Vec::new();
        }
        let taken: HashSet<String> = tool_registry
            .names()
            .into_iter()
            .map(str::to_ascii_lowercase)
            .collect();
        let mut installed = Vec::new();
        for registration in tools {
            if taken.contains(&registration.name.to_ascii_lowercase()) {
                let origin = tool_registry
                    .get(&registration.name)
                    .map(|existing| existing.registration_origin().into_owned())
                    .unwrap_or_else(|| "another tool".to_string());
                self.shared.diagnostic(format!(
                    "extension tool `{}` from `{}` skipped: the name is already registered by {origin}",
                    registration.name, registration.plugin_name
                ));
                continue;
            }
            installed.push(registration.name.clone());
            tool_registry.register(Arc::new(tool::HostToolSpec::new(
                registration,
                Arc::clone(&self.shared),
            )));
        }
        installed
    }

    /// Kick [`Self::sync`] without waiting (turn builds, session start).
    pub fn sync_in_background(self: &Arc<Self>, plugins: Arc<PluginRegistry>) {
        if tokio::runtime::Handle::try_current().is_err() {
            return;
        }
        let manager = Arc::clone(self);
        tokio::spawn(async move {
            if let Err(error) = manager.sync(plugins).await {
                manager.shared.diagnostic(error);
            }
        });
    }

    /// Reconcile host owners with the reviewed, enabled plugins that declare
    /// `native` entries: revoke what is gone or changed (synchronously, then
    /// ask the host to tear down), spawn the host if needed, activate the rest.
    pub async fn sync(&self, plugins: Arc<PluginRegistry>) -> Result<(), String> {
        let shared = &self.shared;
        let _serial = shared.sync_lock.lock().await;
        let policy = activation::extension_host_policy_enabled();
        let (desired, errors) = tokio::task::spawn_blocking(move || {
            let _scope = activation::PolicyScope::propagate(policy);
            desired_owners(&plugins)
        })
        .await
        .map_err(|error| format!("plugin scan failed: {error}"))?;
        for error in errors {
            shared.diagnostic(error);
        }

        {
            let mut seen = shared.seen_plugins.lock().expect("seen lock");
            let fresh = desired.keys().any(|id| !seen.contains(id));
            seen.extend(desired.keys().cloned());
            if fresh {
                let mut slot = shared.host.lock().expect("host lock");
                if matches!(&*slot, HostSlot::Failed { .. }) {
                    *slot = HostSlot::Idle;
                }
            }
        }

        // 1. Revoke first — never waits for the host.
        let mut revoked: Vec<OwnerRef> = Vec::new();
        let mut to_activate: Vec<(String, DesiredOwner)> = Vec::new();
        {
            let mut registry = shared.registry.lock().expect("registry lock");
            let existing: Vec<(String, String, OwnerState)> = registry
                .owners()
                .map(|entry| {
                    (
                        entry.owner.plugin_id.clone(),
                        entry.content_hash.clone(),
                        entry.state.clone(),
                    )
                })
                .collect();
            for (plugin_id, content_hash, _) in &existing {
                let keep = desired
                    .get(plugin_id)
                    .is_some_and(|want| &want.authority.content_hash == content_hash);
                if !keep {
                    if let Some(owner) = registry.revoke_owner(plugin_id) {
                        revoked.push(owner);
                    }
                    registry.forget_owner(plugin_id);
                }
            }
            for (plugin_id, want) in desired {
                // A failed or faulted activation of these exact bytes is not
                // retried every turn; a content change or a new session is.
                if registry.owner(&plugin_id).is_none() {
                    to_activate.push((plugin_id, want));
                }
            }
        }
        let host = shared.ready_host();
        for owner in revoked {
            shared.diagnostic(format!("extension `{}` revoked", owner.plugin_id));
            if let Some(host) = &host {
                host.revoke_calls_of(&owner.plugin_id);
                match host
                    .request_with_deadline(
                        CoreRequest::Deactivate(DeactivateParams {
                            owner: owner.clone(),
                        }),
                        None,
                        DISPOSE_DEADLINE + std::time::Duration::from_millis(500),
                    )
                    .await
                    .map(serde_json::from_value::<DeactivateResult>)
                {
                    Ok(Ok(ack)) if ack.disposed && ack.leaked.is_empty() => {}
                    Ok(Ok(ack)) => shared.diagnostic(format!(
                        "extension `{}` teardown incomplete (disposed: {}, leaked: {:?})",
                        owner.plugin_id, ack.disposed, ack.leaked
                    )),
                    Ok(Err(error)) => shared.diagnostic(format!(
                        "extension `{}` teardown answer malformed: {error}",
                        owner.plugin_id
                    )),
                    Err(error) => shared.diagnostic(format!(
                        "extension `{}` teardown failed: {error}",
                        owner.plugin_id
                    )),
                }
            }
        }

        if to_activate.is_empty() {
            return Ok(());
        }
        let host = self.ensure_host().await?;
        for (plugin_id, want) in to_activate {
            self.activate_owner(&host, &plugin_id, want).await;
        }
        Ok(())
    }

    async fn activate_owner(&self, host: &Arc<HostProcess>, plugin_id: &str, want: DesiredOwner) {
        let shared = &self.shared;
        let owner = shared.registry.lock().expect("registry lock").begin_owner(
            plugin_id,
            &want.plugin_name,
            want.authority.clone(),
            &want.authority.content_hash,
        );
        let mut failure = None;
        let mut tools = Vec::new();
        for (path, sha256) in &want.entries {
            let request = CoreRequest::Activate(ActivateParams {
                owner: owner.clone(),
                plugin_name: want.plugin_name.clone(),
                entry: EntryRef {
                    path: path.to_string_lossy().into_owned(),
                    sha256: sha256.clone(),
                },
                config: json!({}),
            });
            let outcome = host
                .request_with_deadline(
                    request,
                    Some(plugin_id.to_string()),
                    ACTIVATE_DEADLINE + std::time::Duration::from_secs(1),
                )
                .await;
            match outcome.map(serde_json::from_value::<ActivateResult>) {
                Ok(Ok(ActivateResult::Ok { tools: mut names })) => tools.append(&mut names),
                Ok(Ok(ActivateResult::Failed { diagnostic })) => {
                    failure = Some(diagnostic);
                    break;
                }
                Ok(Err(error)) => {
                    failure = Some(format!("malformed activation answer: {error}"));
                    break;
                }
                Err(error) => {
                    failure = Some(error.to_string());
                    break;
                }
            }
        }
        match failure {
            None => {
                let active = shared
                    .registry
                    .lock()
                    .expect("registry lock")
                    .mark_active(&owner);
                if active {
                    shared.diagnostic(format!(
                        "extension `{}` active (tools: {})",
                        want.plugin_name,
                        tools.join(", ")
                    ));
                }
            }
            Some(reason) => {
                // All-or-nothing: drop anything half-registered on this side,
                // and dispose any entry the host did activate.
                shared
                    .registry
                    .lock()
                    .expect("registry lock")
                    .mark_failed(&owner, OwnerState::Failed(reason.clone()));
                shared.diagnostic(format!(
                    "extension `{}` failed to activate: {reason}",
                    want.plugin_name
                ));
                let _ = host
                    .request_with_deadline(
                        CoreRequest::Deactivate(DeactivateParams { owner }),
                        None,
                        DISPOSE_DEADLINE,
                    )
                    .await;
            }
        }
    }

    async fn ensure_host(&self) -> Result<Arc<HostProcess>, String> {
        let shared = &self.shared;
        {
            let mut slot = shared.host.lock().expect("host lock");
            match &*slot {
                HostSlot::Ready(host) if !host.has_exited() => return Ok(Arc::clone(host)),
                HostSlot::Failed { reason, .. } => {
                    return Err(format!(
                        "extension host is failed ({reason}); it restarts with the next session"
                    ));
                }
                _ => *slot = HostSlot::Starting,
            }
        }
        shared.spawn_attempts.fetch_add(1, Ordering::SeqCst);
        let options = shared.options.clone();
        let prepared = tokio::task::spawn_blocking(move || -> Result<supervisor::HostLaunch, String> {
            let resolution =
                crate::dependencies::resolve_node_for_extension_host(options.node_override.as_deref());
            let Some((node, _)) = resolution.selected else {
                return Err(format!(
                    "the extension host needs Node.js ^22.19 || >=24 (set `[extension_host] node`); {}",
                    resolution.describe_rejections()
                ));
            };
            let root = match options.root {
                Some(root) => root,
                None => codewhale_config::codewhale_home()
                    .map_err(|error| format!("Codewhale home unavailable: {error}"))?,
            };
            let bundle = materialize_bundle(&root)?;
            supervisor::plan_launch(&node, &bundle, &root)
        })
        .await
        .map_err(|error| format!("extension host preparation failed: {error}"))
        .and_then(|result| result);
        let spawned = match prepared {
            Ok(launch) => {
                let generation = shared.host_generation.fetch_add(1, Ordering::SeqCst) + 1;
                let events: Arc<dyn HostEvents> = Arc::new(Events(Arc::downgrade(shared)));
                HostProcess::spawn(generation, &launch, bundle_sha256(), events).await
            }
            Err(error) => Err(error),
        };
        let mut slot = shared.host.lock().expect("host lock");
        match spawned {
            Ok(host) => {
                *slot = HostSlot::Ready(Arc::clone(&host));
                drop(slot);
                shared.diagnostic(format!(
                    "extension host started (pid {}, node {}, sandbox {})",
                    host.pid
                        .map_or_else(|| "?".to_string(), |pid| pid.to_string()),
                    host.node_version.get().map_or("?", String::as_str),
                    host.sandbox.as_deref().unwrap_or("none")
                ));
                Ok(host)
            }
            Err(reason) => {
                *slot = HostSlot::Failed {
                    reason: reason.clone(),
                    stderr_tail: String::new(),
                };
                drop(slot);
                shared.diagnostic(format!("extension host failed to start: {reason}"));
                Err(reason)
            }
        }
    }

    /// Bounded shutdown of the host process, if one is running. Production
    /// has no such call: the host is shared by every engine in the process,
    /// so no single engine's shutdown may stop it. When this process ends the
    /// host sees stdin EOF and kills its own process tree; if a plugin blocks
    /// its event loop, its watchdog thread does so when the parent changes.
    #[cfg(test)]
    pub async fn shutdown(&self) {
        let host = {
            let mut slot = self.shared.host.lock().expect("host lock");
            match std::mem::replace(&mut *slot, HostSlot::Idle) {
                HostSlot::Ready(host) => Some(host),
                other => {
                    *slot = other;
                    None
                }
            }
        };
        if let Some(host) = host {
            self.shared.host_generation.fetch_add(1, Ordering::SeqCst);
            self.shared
                .registry
                .lock()
                .expect("registry lock")
                .revoke_all("extension host shut down");
            host.shutdown().await;
        }
    }

    #[cfg(test)]
    pub(crate) fn host_pid(&self) -> Option<u32> {
        self.shared.ready_host().and_then(|host| host.pid)
    }
}

/// Human-readable host section for `/plugin`.
pub(crate) fn render_status(manager: &ExtensionHostManager) -> String {
    use std::fmt::Write as _;
    let mut out = String::from("Extension host (experimental): ");
    match manager.status() {
        HostStatus::Idle => out.push_str(
            "not running (starts in the background when a reviewed plugin with host code is enabled)",
        ),
        HostStatus::Starting => out.push_str("starting"),
        HostStatus::Ready {
            pid,
            node_version,
            sandbox,
        } => {
            let _ = write!(
                out,
                "running · pid {} · node {node_version} · {}",
                pid.map_or_else(|| "?".to_string(), |pid| pid.to_string()),
                match sandbox {
                    Some(sandbox) => format!(
                        "{sandbox} sandbox (no direct network; the Codewhale home except plugin code, the Codex and DSH credential homes and the default credential stores are unreadable; other files you can read, such as project .env files, are not protected)"
                    ),
                    None => "UNSANDBOXED: host code runs with your user permissions".to_string(),
                }
            );
        }
        HostStatus::Failed {
            reason,
            stderr_tail,
        } => {
            let _ = write!(out, "failed: {reason} (restarts with the next session)");
            let tail = stderr_tail.trim();
            if !tail.is_empty() {
                let start = tail
                    .char_indices()
                    .rev()
                    .nth(599)
                    .map_or(0, |(index, _)| index);
                let _ = write!(out, "\n  stderr: {}", &tail[start..]);
            }
        }
    }
    let _ = write!(out, "\n  spawn attempts: {}", manager.spawn_attempts());
    let (tools, owners) = {
        let registry = manager.shared.registry.lock().expect("registry lock");
        let owners = registry
            .owners()
            .filter(|entry| entry.state == OwnerState::Active)
            .count();
        (registry.live_tools(), owners)
    };
    if owners > 1 {
        let _ = write!(
            out,
            "\n  {owners} plugins share this one host process and can alter each other's behaviour"
        );
    }
    for tool in tools {
        let _ = write!(
            out,
            "\n  tool {} (extension:{}; needs approval, which your approval mode or a session grant for this exact tool may give)",
            tool.name, tool.plugin_name
        );
    }
    let diagnostics = manager.diagnostics();
    for diagnostic in diagnostics.iter().rev().take(5).rev() {
        let _ = write!(out, "\n  · {diagnostic}");
    }
    out
}

/// A plugin was enabled, disabled, trusted, revoked or removed: reconcile the
/// host now, so a disabled plugin's calls are cancelled and its code torn
/// down without waiting for the next turn. No-op with the flag off.
pub fn plugins_changed(plugins: Arc<PluginRegistry>) {
    if activation::extension_host_policy_enabled() {
        manager().sync_in_background(plugins);
    }
}

/// The `/plugin` section, or `None` when the experimental host is off.
#[must_use]
pub fn status_report() -> Option<String> {
    activation::extension_host_policy_enabled().then(|| render_status(&manager()))
}

/// Reviewed, enabled plugins with `native` entries, keyed by plugin id, read
/// from Codewhale's immutable staged snapshot. Blocking.
fn desired_owners(plugins: &PluginRegistry) -> (BTreeMap<String, DesiredOwner>, Vec<String>) {
    let (sources, mut errors) = crate::plugins::runtime::active_component_sources(
        plugins,
        PluginActivationCapability::Native,
    );
    let mut desired: BTreeMap<String, DesiredOwner> = BTreeMap::new();
    let mut broken: BTreeSet<String> = BTreeSet::new();
    for source in sources {
        let plugin_id = source.authority.plugin_id.as_str().to_string();
        let is_module = source
            .path
            .extension()
            .is_some_and(|extension| extension == "mjs" || extension == "js");
        let bytes = if is_module {
            std::fs::read(&source.path).map_err(|error| error.to_string())
        } else {
            Err("a native entry must be one .mjs or .js ES module file".to_string())
        };
        match bytes {
            Ok(bytes) => desired
                .entry(plugin_id)
                .or_insert_with(|| DesiredOwner {
                    plugin_name: source.plugin_name.clone(),
                    authority: source.authority.clone(),
                    entries: Vec::new(),
                })
                .entries
                .push((source.path.clone(), hex(Sha256::digest(&bytes)))),
            Err(reason) => {
                errors.push(format!(
                    "Plugin `{}` native entry {} was denied: {reason}",
                    source.plugin_name,
                    source.path.display()
                ));
                broken.insert(plugin_id);
            }
        }
    }
    // All-or-nothing per plugin: one unusable entry keeps the whole plugin out.
    for plugin_id in broken {
        desired.remove(&plugin_id);
    }
    (desired, errors)
}

static GLOBAL: OnceLock<Arc<ExtensionHostManager>> = OnceLock::new();

#[cfg(test)]
thread_local! {
    static TEST_MANAGER: std::cell::RefCell<Option<Arc<ExtensionHostManager>>> =
        const { std::cell::RefCell::new(None) };
}

/// Configure the process-wide manager once, at boot, from user config.
pub fn configure(options: ExtensionHostOptions) {
    let _ = GLOBAL.set(Arc::new(ExtensionHostManager::new(options)));
}

/// The manager for this engine process (one host per process and tier).
#[must_use]
pub fn manager() -> Arc<ExtensionHostManager> {
    #[cfg(test)]
    if let Some(manager) = TEST_MANAGER.with(|cell| cell.borrow().clone()) {
        return manager;
    }
    Arc::clone(
        GLOBAL.get_or_init(|| Arc::new(ExtensionHostManager::new(ExtensionHostOptions::default()))),
    )
}

/// Test-only: route [`manager`] on this thread to `manager`.
#[cfg(test)]
pub(crate) struct TestManagerGuard(Option<Arc<ExtensionHostManager>>);

#[cfg(test)]
impl TestManagerGuard {
    pub(crate) fn install(manager: Arc<ExtensionHostManager>) -> Self {
        Self(TEST_MANAGER.with(|cell| cell.replace(Some(manager))))
    }
}

#[cfg(test)]
impl Drop for TestManagerGuard {
    fn drop(&mut self) {
        TEST_MANAGER.with(|cell| *cell.borrow_mut() = self.0.take());
    }
}
