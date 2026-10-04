//! Native-only Windows launch boundary. LPAC with no capabilities supplies
//! filesystem/network denial; the existing Job supplies lifetime and memory.
//! A fresh profile never inherits ACLs from a retired host. Profiles/assets
//! created here are disposed by their exact owner after the process ends;
//! crash leftovers are not guessed at or swept. Windows may also provide its
//! private AppContainer scratch/registry, separate from Core-owned state.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::io::{self, Read};
use std::mem::{size_of, zeroed};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use windows_sys::Win32::Foundation::{
    ERROR_PIPE_CONNECTED, GetLastError, INVALID_HANDLE_VALUE, LocalFree, WAIT_OBJECT_0,
    WAIT_TIMEOUT,
};
use windows_sys::Win32::Security::Authorization::{
    EXPLICIT_ACCESS_W, GRANT_ACCESS, GetSecurityInfo, REVOKE_ACCESS, SE_FILE_OBJECT,
    SetEntriesInAclW, SetSecurityInfo, TRUSTEE_IS_SID, TRUSTEE_IS_UNKNOWN, TRUSTEE_W,
};
use windows_sys::Win32::Security::Isolation::{
    CreateAppContainerProfile, DeleteAppContainerProfile,
};
use windows_sys::Win32::Security::{
    ACL, CONTAINER_INHERIT_ACE, DACL_SECURITY_INFORMATION, EqualSid, FreeSid, GetTokenInformation,
    OBJECT_INHERIT_ACE, PSID, SECURITY_ATTRIBUTES, SECURITY_CAPABILITIES,
    TOKEN_APPCONTAINER_INFORMATION, TOKEN_GROUPS, TOKEN_QUERY, TokenAppContainerSid,
    TokenCapabilities, TokenIsAppContainer, TokenIsLessPrivilegedAppContainer,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OPEN_REPARSE_POINT, FILE_FLAG_OVERLAPPED,
    FILE_GENERIC_EXECUTE, FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_READ_ATTRIBUTES,
    OPEN_EXISTING, PIPE_ACCESS_INBOUND, PIPE_ACCESS_OUTBOUND, READ_CONTROL, WRITE_DAC,
};
use windows_sys::Win32::System::Pipes::{ConnectNamedPipe, CreateNamedPipeW, PIPE_WAIT};
use windows_sys::Win32::System::Threading::{
    CREATE_NO_WINDOW, CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT, CreateProcessW,
    DeleteProcThreadAttributeList, EXTENDED_STARTUPINFO_PRESENT, GetExitCodeProcess,
    InitializeProcThreadAttributeList, OpenProcessToken,
    PROC_THREAD_ATTRIBUTE_ALL_APPLICATION_PACKAGES_POLICY, PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
    PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES, PROCESS_INFORMATION, ResumeThread,
    STARTF_USESTDHANDLES, STARTUPINFOEXW, UpdateProcThreadAttribute, WaitForSingleObject,
};

use crate::dependencies::HostRuntime;
use crate::fleet::files::WindowsDirectory;
use crate::process_tree::ProcessTree;

const PROBE_SOURCE: &str = include_str!("../../extension-host/src/windows-sandbox-probe.mjs");
const PROBE_DEADLINE: Duration = Duration::from_secs(15);
// Windows SDK winnt.h; the documented LPAC startup attribute opts out of the
// ambient ALL APPLICATION PACKAGES group. Not a fabricated token status.
const PROCESS_CREATION_ALL_APPLICATION_PACKAGES_OPT_OUT: u32 = 0x1;
const MAX_GRANT_ENTRIES: usize = 65_536;
const MAX_GRANT_SCOPES: usize = 1024;
// Serialize Core's read/merge/write ACL operations across old-profile cleanup
// and a new host admission. Never overwrite a concurrently admitted profile.
static ACL_EDITS: Mutex<()> = Mutex::new(());

#[derive(Clone)]
pub(crate) struct NativeSandbox {
    profile: Arc<Profile>,
    _assets: Arc<tempfile::TempDir>,
    pub program: PathBuf,
    data: PathBuf,
}

impl std::fmt::Debug for NativeSandbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeSandbox")
            .field("program", &self.program)
            .finish_non_exhaustive()
    }
}

struct Profile {
    name: Vec<u16>,
    sid: PSID,
    grants: Mutex<BTreeMap<PathBuf, GrantScope>>,
    cleanup_runtime: Option<tokio::runtime::Handle>,
}
struct GrantScope {
    identity: (u32, u64),
    tree: bool,
}
// A retained exact profile SID, never a parsed/guessed orphan identity.
struct RetiredProfile {
    name: Vec<u16>,
    sid: PSID,
    grants: BTreeMap<PathBuf, GrantScope>,
}
// SAFETY: retired state is exclusively owned; the SID is freed only after the
// existing runtime's blocking cleanup worker has finished its exact grants.
unsafe impl Send for RetiredProfile {}
// SAFETY: SID/name are immutable after successful creation. Win32 consumes
// borrowed SID memory synchronously; only the final Arc drop frees it.
unsafe impl Send for Profile {}
unsafe impl Sync for Profile {}

impl Profile {
    fn create() -> io::Result<Self> {
        let name = wide(OsStr::new(&format!(
            "Codewhale.Native.{}",
            uuid::Uuid::new_v4()
        )))?;
        let mut sid = null_mut();
        // SAFETY: nul-terminated owned strings and an initialized out pointer.
        let result = unsafe {
            CreateAppContainerProfile(
                name.as_ptr(),
                name.as_ptr(),
                name.as_ptr(),
                null(),
                0,
                &mut sid,
            )
        };
        if result < 0 {
            return Err(io::Error::other(format!(
                "CreateAppContainerProfile HRESULT {result:#x}"
            )));
        }
        if sid.is_null() {
            // No existing/user profile is adopted or removed on a collision.
            unsafe {
                DeleteAppContainerProfile(name.as_ptr());
            }
            return Err(io::Error::other("AppContainer profile has no SID"));
        }
        Ok(Self {
            name,
            sid,
            grants: Mutex::new(BTreeMap::new()),
            cleanup_runtime: tokio::runtime::Handle::try_current().ok(),
        })
    }
}

impl Profile {
    fn remember(&self, path: &Path, file: &File, tree: bool) -> io::Result<()> {
        let value = crate::plugins::windows_file_identity(file)?;
        let mut grants = self
            .grants
            .lock()
            .map_err(|_| io::Error::other("profile grant accounting poisoned"))?;
        if let Some(old) = grants.get(path) {
            if old.identity != (value.volume, value.index) || old.tree != tree {
                return Err(io::Error::other(
                    "recorded profile grant identity changed; refusing overwrite",
                ));
            }
            return Ok(());
        }
        if grants.len() == MAX_GRANT_SCOPES {
            return Err(io::Error::other("profile exceeds 1024 exact grant scopes"));
        }
        grants.insert(
            path.to_path_buf(),
            GrantScope {
                identity: (value.volume, value.index),
                tree,
            },
        );
        Ok(())
    }
}
impl Drop for Profile {
    fn drop(&mut self) {
        let grants = std::mem::take(
            self.grants
                .get_mut()
                .unwrap_or_else(|error| error.into_inner()),
        );
        let retired = RetiredProfile {
            name: std::mem::take(&mut self.name),
            sid: self.sid,
            grants,
        };
        // Same existing Tokio scheduler; no new runtime/authority. The worker
        // owns the SID until retirement completes, even during cancellation.
        if let Some(runtime) = self.cleanup_runtime.take() {
            runtime.spawn_blocking(move || retired.dispose());
        } else {
            // Synchronous platform callers have no runtime; work remains
            // bounded to the exact recorded roots, never a profile sweep.
            retired.dispose();
        }
    }
}
impl RetiredProfile {
    fn dispose(self) {
        drop(self);
    }
}
// Drop also retires a task cancelled before its blocking worker starts. Once
// started, spawn_blocking cannot abandon this exclusively owned cleanup.
impl Drop for RetiredProfile {
    fn drop(&mut self) {
        for (path, scope) in &self.grants {
            if let Err(error) = retire_scope(path, scope, self.sid) {
                tracing::warn!(path = %path.display(), "Native exact-profile ACL retirement failed: {error}");
            }
        }
        // The OS profile and SID stay alive until exact grant retirement ends.
        // Crash remnants or refused/replaced roots are not guessed at/swept.
        unsafe {
            let result = DeleteAppContainerProfile(self.name.as_ptr());
            if result < 0 {
                tracing::warn!("Native AppContainer profile disposal failed ({result:#x})");
            }
            FreeSid(self.sid);
        }
    }
}

fn retire_scope(root: &Path, scope: &GrantScope, sid: PSID) -> io::Result<()> {
    let _serial = ACL_EDITS.lock().unwrap_or_else(|error| error.into_inner());
    if !root.try_exists()? {
        return Ok(());
    }
    let root_pin = if scope.tree {
        WindowsDirectory::open_acl(root)?
    } else {
        WindowsDirectory::open(
            root.parent()
                .ok_or_else(|| io::Error::other("grant has no parent"))?,
        )?
    };
    let root_file;
    let file = if scope.tree {
        root_pin.acl_handle()?
    } else {
        root_file = acl_file_with_share(root, 1 | 2 | 4)?;
        &root_file
    };
    let value = crate::plugins::windows_file_identity(file)?;
    if (value.volume, value.index) != scope.identity {
        return Err(io::Error::other(
            "recorded grant object was replaced; refusing cleanup",
        ));
    }
    // Remove the parent's inheritable grant before walking, so newly created
    // children cannot inherit this retired SID. MAXIMUM_ALLOWED prevents SDK
    // propagation; every child is independently fenced and updated.
    edit_acl(file, sid, 0, 0, REVOKE_ACCESS)?;
    if !scope.tree {
        return Ok(());
    }
    let mut pending = Vec::new();
    let mut count = 1;
    enqueue_pinned_children(root, &root_pin, &mut pending, count)?;
    let mut failure = None;
    while let Some((path, parent_pin)) = pending.pop() {
        count += 1;
        if count > MAX_GRANT_ENTRIES {
            return Err(io::Error::other(
                "profile retirement exceeds 65536 entries in a recorded root",
            ));
        }
        let mut work = || -> io::Result<()> {
            let metadata = fs::symlink_metadata(&path)?;
            if crate::plugins::metadata_is_link_or_reparse(&metadata) {
                return Err(io::Error::other(
                    "profile retirement refuses links/reparse points",
                ));
            }
            if metadata.is_dir() {
                let directory = parent_pin.open_acl_child(
                    path.file_name()
                        .ok_or_else(|| io::Error::other("grant has no filename"))?,
                )?;
                edit_acl(directory.acl_handle()?, sid, 0, 0, REVOKE_ACCESS)?;
                enqueue_pinned_children(&path, &directory, &mut pending, count)
            } else if metadata.is_file() {
                if parent_pin.child_path(
                    path.file_name()
                        .ok_or_else(|| io::Error::other("grant has no filename"))?,
                )? != path
                {
                    return Err(io::Error::other("grant left its pinned parent"));
                }
                // Retiring this exact SID may overlap a new host's data
                // writes. The physical handle stays pinned, but allow those
                // legitimate writers/renames; never restore an old whole ACL.
                let file = acl_file_with_share(&path, 1 | 2 | 4)?;
                edit_acl(&file, sid, 0, 0, REVOKE_ACCESS)?;
                Ok(())
            } else {
                Err(io::Error::other(
                    "profile retirement refuses a nonregular entry",
                ))
            }
        };
        match work() {
            Ok(()) => {}
            Err(error) => {
                failure = Some(error);
            }
        }
    }
    match failure {
        Some(error) => Err(error),
        None => Ok(()),
    }
}
fn enqueue_children(path: &Path, pending: &mut Vec<PathBuf>, visited: usize) -> io::Result<()> {
    for entry in fs::read_dir(path)? {
        // Bound materialized pending entries, not only already visited paths.
        if visited + pending.len() >= MAX_GRANT_ENTRIES {
            return Err(io::Error::other(
                "profile directory traversal exceeds 65536 entries",
            ));
        }
        pending.push(entry?.path());
    }
    Ok(())
}
fn enqueue_pinned_children(
    path: &Path,
    pin: &WindowsDirectory,
    pending: &mut Vec<(PathBuf, WindowsDirectory)>,
    visited: usize,
) -> io::Result<()> {
    let mut children = Vec::new();
    // Reuse the exact admission/retirement bound, including already pending
    // entries. Each child keeps its actual direct parent pinned until visited.
    enqueue_children(path, &mut children, visited + pending.len())?;
    pending.extend(children.into_iter().map(|child| (child, pin.clone())));
    Ok(())
}
/// Name the admission step in an error, so a refusal says where it stopped.
fn in_step<T>(step: &str, result: io::Result<T>) -> io::Result<T> {
    result.map_err(|error| io::Error::new(error.kind(), format!("{step}: {error}")))
}
fn acl_file(path: &Path) -> io::Result<File> {
    acl_file_with_share(path, 1)
}
fn acl_file_with_share(path: &Path, share: u32) -> io::Result<File> {
    let file = fs::OpenOptions::new()
        .access_mode(READ_CONTROL | WRITE_DAC | FILE_READ_ATTRIBUTES)
        .share_mode(share)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || crate::plugins::metadata_is_link_or_reparse(&metadata)
        || crate::plugins::windows_file_identity(&file)?.links != 1
    {
        return Err(io::Error::other(
            "profile ACL operation refuses a linked/nonregular file",
        ));
    }
    Ok(file)
}

impl NativeSandbox {
    /// Blocking, invoked by the existing manager's bounded launch worker.
    pub(crate) fn prepare(
        runtime: &HostRuntime,
        bundle: &Path,
        home: &Path,
        data: &Path,
        memory_cap: u64,
    ) -> Result<Self, String> {
        let work = || -> io::Result<Self> {
            let profile = Arc::new(Profile::create()?);
            let parent = home.join("extension-host").join("native-launch");
            fs::create_dir_all(&parent)?;
            let _parent = in_step("pin launch parent", WindowsDirectory::open(&parent))?;
            let assets = Arc::new(
                tempfile::Builder::new()
                    .prefix("host-")
                    .tempdir_in(&parent)?,
            );
            let program = assets.path().join("runtime.exe");
            // The selected runtime is copied, never granted access in an
            // installation/user directory. Pin its opened bytes while copying.
            let original = runtime.path.canonicalize()?;
            let source_pin = in_step(
                "pin runtime directory",
                WindowsDirectory::open(
                    original
                        .parent()
                        .ok_or_else(|| io::Error::other("runtime has no parent"))?,
                ),
            )?;
            let mut source = in_step(
                "open runtime",
                crate::plugins::manifest::open_bundle_file(&original),
            )?;
            if source.metadata()?.len() > 512 * 1024 * 1024 {
                return Err(io::Error::other(
                    "selected runtime exceeds 512 MiB launch limit",
                ));
            }
            let mut target = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&program)?;
            io::copy(&mut source, &mut target)?;
            target.sync_all()?;
            drop(target);
            drop(source_pin);
            let sandbox = Self {
                profile,
                _assets: assets,
                program,
                data: data.to_path_buf(),
            };
            in_step(
                "grant runtime copy",
                sandbox.grant_tree(sandbox._assets.path(), false),
            )?;
            // Exact canonical bundle file only: never recursively grant its
            // parent, which also contains the Builtin's private data.
            in_step(
                "grant host bundle",
                sandbox.grant_file(bundle, FILE_GENERIC_READ | FILE_GENERIC_EXECUTE, true),
            )?;
            fs::create_dir_all(data.join("tmp"))?;
            in_step("grant data directory", sandbox.grant_tree(data, true))?;
            in_step("isolation probe", sandbox.probe(runtime, memory_cap))?;
            Ok(sandbox)
        };
        work().map_err(|error| format!("Windows Native isolation could not be verified: {error}"))
    }

    /// Only call after the existing Rust Native receipt/hash check. The root
    /// is a reviewed staged snapshot, never the mutable source or a link.
    pub(crate) fn admit_root(&self, root: &Path) -> Result<(), String> {
        self.grant_tree(root, false)
            .map_err(|error| format!("cannot admit reviewed Windows bundle: {error}"))
    }

    fn grant_tree(&self, root: &Path, writable: bool) -> io::Result<()> {
        let _serial = ACL_EDITS.lock().unwrap_or_else(|error| error.into_inner());
        let root_pin = WindowsDirectory::open_acl(root)?;
        self.profile.remember(root, root_pin.acl_handle()?, true)?;
        let access = FILE_GENERIC_READ
            | FILE_GENERIC_EXECUTE
            | if writable {
                FILE_GENERIC_WRITE | windows_sys::Win32::Storage::FileSystem::DELETE
            } else {
                0
            };
        set_acl(
            root_pin.acl_handle()?,
            self.profile.sid,
            access,
            OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE,
        )?;
        let mut pending = Vec::new();
        let mut count = 1;
        enqueue_pinned_children(root, &root_pin, &mut pending, count)?;
        while let Some((path, parent_pin)) = pending.pop() {
            count += 1;
            if count > MAX_GRANT_ENTRIES {
                return Err(io::Error::other("sandbox grant tree exceeds 65536 entries"));
            }
            let metadata = fs::symlink_metadata(&path)?;
            if crate::plugins::metadata_is_link_or_reparse(&metadata) {
                return Err(io::Error::other(
                    "sandbox grant refuses a link/reparse point",
                ));
            }
            if metadata.is_dir() {
                let pin = parent_pin.open_acl_child(
                    path.file_name()
                        .ok_or_else(|| io::Error::other("grant has no filename"))?,
                )?;
                set_acl(
                    pin.acl_handle()?,
                    self.profile.sid,
                    access,
                    OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE,
                )?;
                enqueue_pinned_children(&path, &pin, &mut pending, count)?;
            } else if metadata.is_file() {
                self.grant_pinned_file(&parent_pin, &path, access, false)?;
            } else {
                return Err(io::Error::other(
                    "sandbox grant refuses a non-regular entry",
                ));
            }
        }
        Ok(())
    }

    fn grant_file(&self, path: &Path, access: u32, remember: bool) -> io::Result<()> {
        let _serial = ACL_EDITS.lock().unwrap_or_else(|error| error.into_inner());
        let pin = WindowsDirectory::open(
            path.parent()
                .ok_or_else(|| io::Error::other("file has no parent"))?,
        )?;
        self.grant_pinned_file(&pin, path, access, remember)
    }

    // The owning grant entrypoint holds ACL_EDITS. Tree files reuse the parent
    // chain rather than reopen MAXIMUM_ALLOWED directory objects. The
    // child_path comparison is only a lexical invariant; the protection is the
    // held no-write/no-delete parent chain plus acl_file's no-follow,
    // regular, single-link checks.
    fn grant_pinned_file(
        &self,
        parent_pin: &WindowsDirectory,
        path: &Path,
        access: u32,
        remember: bool,
    ) -> io::Result<()> {
        if parent_pin.child_path(
            path.file_name()
                .ok_or_else(|| io::Error::other("file has no filename"))?,
        )? != path
        {
            return Err(io::Error::other("grant left its pinned parent"));
        }
        let file = acl_file(path)?;
        if remember {
            self.profile.remember(path, &file, false)?;
        }
        set_acl(&file, self.profile.sid, access, 0)
    }

    fn probe(&self, runtime: &HostRuntime, memory_cap: u64) -> io::Result<()> {
        let outside = tempfile::tempdir()?;
        let mut reads = Vec::new();
        for name in [
            "codex-auth.json",
            "dsh-credentials.yaml",
            "builtin-private-state.json",
        ] {
            let path = outside.path().join(name);
            fs::write(&path, b"non-secret-denial-control")?;
            reads.push(path);
        }
        let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
        listener.set_nonblocking(true)?;
        let overrides = [
            (
                "CODEWHALE_WINDOWS_PROBE_INSIDE".to_string(),
                self.data
                    .join(format!(".probe-{}", uuid::Uuid::new_v4()))
                    .to_string_lossy()
                    .into_owned(),
            ),
            (
                "CODEWHALE_WINDOWS_PROBE_OUTSIDE".to_string(),
                outside
                    .path()
                    .join("forbidden-write")
                    .to_string_lossy()
                    .into_owned(),
            ),
            (
                "CODEWHALE_WINDOWS_PROBE_READS".to_string(),
                serde_json::to_string(&reads)?,
            ),
            (
                "CODEWHALE_WINDOWS_PROBE_PORT".to_string(),
                listener.local_addr()?.port().to_string(),
            ),
        ];
        let runtime_env = super::supervisor::runtime_env(runtime.kind);
        let mut args = super::supervisor::runtime_args(runtime);
        if runtime.compiled {
            args.extend(["--tier=plugin".into(), "--windows-sandbox-probe".into()]);
        } else {
            args.extend([
                "--input-type=module".into(),
                "-e".into(),
                format!("{PROBE_SOURCE}\nconsole.log(JSON.stringify(await windowsSandboxProbe()))"),
            ]);
        }
        let child_args = (
            "CODEWHALE_WINDOWS_PROBE_CHILD_ARGS".to_string(),
            serde_json::to_string(&args)?,
        );
        let env = crate::child_env::sanitized_plugin_mcp_env_from(
            std::env::vars_os(),
            runtime_env
                .iter()
                .chain(&overrides)
                .chain(std::iter::once(&child_args))
                .map(|(k, v)| (k.as_str(), v.as_str())),
        );
        let mut probe = self.spawn_inner(&args, &env, memory_cap, false)?;
        // Close input immediately. Fixed output is under 512 bytes; a runtime
        // substitution that floods a pipe cannot evade the wait deadline.
        drop(probe.stdin);
        let status = match probe.child.wait_timeout(PROBE_DEADLINE) {
            Ok(status) => status,
            Err(error) => {
                let _ = probe.child.tree.kill();
                let _ = probe.child.wait_timeout(Duration::from_secs(2));
                return Err(error);
            }
        };
        let mut stdout = Vec::new();
        File::from(probe.stdout)
            .take(4097)
            .read_to_end(&mut stdout)?;
        let mut stderr = Vec::new();
        File::from(probe.stderr)
            .take(4097)
            .read_to_end(&mut stderr)?;
        if !status.success() || stdout.len() > 4096 {
            return Err(io::Error::other(format!(
                "isolation probe failed ({status}): {}",
                String::from_utf8_lossy(&stderr)
            )));
        }
        let expected = serde_json::json!({"version":1,"data_roundtrip":true,"outside_read_denied":true,"outside_write_denied":true,"network_denied":true,"descendant_denied":true});
        if serde_json::from_slice::<serde_json::Value>(&stdout)? != expected
            || listener.accept().is_ok()
        {
            return Err(io::Error::other(
                "sandbox probe returned no exact allow/deny receipt",
            ));
        }
        Ok(())
    }

    pub(crate) fn spawn(
        &self,
        args: &[String],
        env: &[(OsString, OsString)],
        memory_cap: u64,
    ) -> io::Result<Spawned> {
        self.spawn_inner(args, env, memory_cap, true)
    }

    fn spawn_inner(
        &self,
        args: &[String],
        env: &[(OsString, OsString)],
        memory_cap: u64,
        overlapped: bool,
    ) -> io::Result<Spawned> {
        let (stdin, child_stdin) = pipe(false, overlapped)?;
        let (stdout, child_stdout) = pipe(true, overlapped)?;
        let (stderr, child_stderr) = pipe(true, overlapped)?;
        let mut attrs = Attributes::new(3)?;
        let capabilities = SECURITY_CAPABILITIES {
            AppContainerSid: self.profile.sid,
            Capabilities: null_mut(),
            CapabilityCount: 0,
            Reserved: 0,
        };
        let lpac = PROCESS_CREATION_ALL_APPLICATION_PACKAGES_OPT_OUT;
        let handles = [
            child_stdin.as_raw_handle(),
            child_stdout.as_raw_handle(),
            child_stderr.as_raw_handle(),
        ];
        attrs.set(PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES, &capabilities)?;
        attrs.set(PROC_THREAD_ATTRIBUTE_ALL_APPLICATION_PACKAGES_POLICY, &lpac)?;
        attrs.set_slice(PROC_THREAD_ATTRIBUTE_HANDLE_LIST, &handles)?;
        let mut startup: STARTUPINFOEXW = unsafe { zeroed() };
        startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
        startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        startup.StartupInfo.hStdInput = child_stdin.as_raw_handle();
        startup.StartupInfo.hStdOutput = child_stdout.as_raw_handle();
        startup.StartupInfo.hStdError = child_stderr.as_raw_handle();
        startup.lpAttributeList = attrs.ptr();
        let application = wide(self.program.as_os_str())?;
        let mut command = command_line(self.program.as_os_str(), args)?;
        let directory = wide(self.data.as_os_str())?;
        let mut env = env.to_vec();
        let temp = self.data.join("tmp").into_os_string();
        for key in ["TEMP", "TMP", "TMPDIR"] {
            env.retain(|(name, _)| !name.to_string_lossy().eq_ignore_ascii_case(key));
            env.push((key.into(), temp.clone()));
        }
        let environment = environment_block(&env)?;
        let mut info: PROCESS_INFORMATION = unsafe { zeroed() };
        // SAFETY: owned attribute backing/storage/argv/environment outlive
        // this call; HANDLE_LIST is precisely the three inheritable pipe ends.
        let created = unsafe {
            CreateProcessW(
                application.as_ptr(),
                command.as_mut_ptr(),
                null(),
                null(),
                1,
                CREATE_SUSPENDED
                    | CREATE_NO_WINDOW
                    | CREATE_UNICODE_ENVIRONMENT
                    | EXTENDED_STARTUPINFO_PRESENT,
                environment.as_ptr().cast(),
                directory.as_ptr(),
                &startup.StartupInfo,
                &mut info,
            )
        };
        if created == 0 {
            return Err(io::Error::last_os_error());
        }
        let process = unsafe { OwnedHandle::from_raw_handle(info.hProcess) };
        let thread = unsafe { OwnedHandle::from_raw_handle(info.hThread) };
        let setup = || -> io::Result<Arc<ProcessTree>> {
            let tree = Arc::new(ProcessTree::attach_windows_handle(
                info.dwProcessId,
                &process,
            )?);
            tree.limit_process_memory(memory_cap)?;
            verify_token(&process, self.profile.sid)?;
            // No Native byte executes until Job, memory and actual LPAC token
            // identity/capabilities have all been checked by Rust.
            if unsafe { ResumeThread(thread.as_raw_handle()) } != 1 {
                return Err(io::Error::other(
                    "Native main thread did not resume from its exact suspended state",
                ));
            }
            Ok(tree)
        };
        let tree = match setup() {
            Ok(tree) => tree,
            Err(error) => {
                unsafe {
                    windows_sys::Win32::System::Threading::TerminateProcess(
                        process.as_raw_handle(),
                        70,
                    );
                    WaitForSingleObject(process.as_raw_handle(), 2000);
                }
                return Err(error);
            }
        };
        drop((child_stdin, child_stdout, child_stderr, thread));
        Ok(Spawned {
            child: Child {
                process: Arc::new(process),
                tree,
                pid: info.dwProcessId,
                _sandbox: self.clone(),
                reaped: false,
            },
            stdin,
            stdout,
            stderr,
        })
    }
}

pub(crate) struct Spawned {
    pub child: Child,
    pub stdin: OwnedHandle,
    pub stdout: OwnedHandle,
    pub stderr: OwnedHandle,
}

pub(crate) struct Child {
    process: Arc<OwnedHandle>,
    pub tree: Arc<ProcessTree>,
    pub pid: u32,
    _sandbox: NativeSandbox,
    reaped: bool,
}

impl Child {
    pub(crate) async fn wait(&mut self) -> io::Result<std::process::ExitStatus> {
        let process = Arc::clone(&self.process);
        let sandbox = self._sandbox.clone();
        let status = tokio::task::spawn_blocking(move || {
            let _sandbox = sandbox;
            wait(&process, u32::MAX)
        })
        .await
        .map_err(io::Error::other)??;
        self.reaped = true;
        Ok(status)
    }
    pub(crate) async fn kill(&mut self) -> io::Result<()> {
        self.tree.kill()?;
        self.wait().await.map(|_| ())
    }
    fn wait_timeout(&mut self, after: Duration) -> io::Result<std::process::ExitStatus> {
        let status = wait(
            &self.process,
            after.as_millis().min(u32::MAX as u128 - 1) as u32,
        )?;
        self.reaped = true;
        // The fixed probe must not retain background descendants/pipe handles.
        let _ = self.tree.kill();
        Ok(status)
    }
}

impl Drop for Child {
    fn drop(&mut self) {
        if !self.reaped {
            let _ = self.tree.kill();
        }
    }
}

fn wait(process: &OwnedHandle, timeout: u32) -> io::Result<std::process::ExitStatus> {
    use std::os::windows::process::ExitStatusExt;
    match unsafe { WaitForSingleObject(process.as_raw_handle(), timeout) } {
        WAIT_OBJECT_0 => {
            let mut status = 0;
            if unsafe { GetExitCodeProcess(process.as_raw_handle(), &mut status) } == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(std::process::ExitStatus::from_raw(status))
        }
        WAIT_TIMEOUT => Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "Windows sandbox probe exceeded launch deadline",
        )),
        _ => Err(io::Error::last_os_error()),
    }
}

fn verify_token(process: &OwnedHandle, sid: PSID) -> io::Result<()> {
    let mut token = null_mut();
    if unsafe { OpenProcessToken(process.as_raw_handle(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let token = unsafe { OwnedHandle::from_raw_handle(token) };
    // Both flags must be exactly 1. Report each observation, so a refusal
    // says which property the kernel did not confirm.
    let mut observed = Vec::new();
    let mut confirmed = true;
    for (name, class) in [
        ("TokenIsAppContainer", TokenIsAppContainer),
        (
            "TokenIsLessPrivilegedAppContainer",
            TokenIsLessPrivilegedAppContainer,
        ),
    ] {
        let mut value = 0_u32;
        let mut read = 0;
        let queried = unsafe {
            GetTokenInformation(
                token.as_raw_handle(),
                class,
                (&mut value as *mut u32).cast(),
                size_of::<u32>() as u32,
                &mut read,
            )
        } != 0;
        if queried {
            observed.push(format!("{name}={value}"));
        } else {
            observed.push(format!(
                "{name} query failed: {}",
                io::Error::last_os_error()
            ));
        }
        confirmed &= queried && value == 1;
    }
    if !confirmed {
        return Err(io::Error::other(format!(
            "Windows refused the required LPAC token ({})",
            observed.join(", ")
        )));
    }
    // Variable-size token buffers are aligned for their SDK structs.
    for class in [TokenAppContainerSid, TokenCapabilities] {
        let mut size = 0;
        unsafe {
            GetTokenInformation(token.as_raw_handle(), class, null_mut(), 0, &mut size);
        }
        if size == 0 || size > 64 * 1024 {
            return Err(io::Error::other("invalid sandbox token size"));
        }
        let mut buffer = vec![0_usize; (size as usize).div_ceil(size_of::<usize>())];
        if unsafe {
            GetTokenInformation(
                token.as_raw_handle(),
                class,
                buffer.as_mut_ptr().cast(),
                size,
                &mut size,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        let valid = unsafe {
            if class == TokenAppContainerSid {
                let value = &*buffer.as_ptr().cast::<TOKEN_APPCONTAINER_INFORMATION>();
                !value.TokenAppContainer.is_null() && EqualSid(value.TokenAppContainer, sid) != 0
            } else {
                (*buffer.as_ptr().cast::<TOKEN_GROUPS>()).GroupCount == 0
            }
        };
        if !valid {
            return Err(io::Error::other(
                "sandbox token has unexpected identity/capabilities",
            ));
        }
    }
    Ok(())
}

fn set_acl(file: &File, sid: PSID, access: u32, inheritance: u32) -> io::Result<()> {
    edit_acl(file, sid, access, inheritance, GRANT_ACCESS)
}
fn edit_acl(
    file: &File,
    sid: PSID,
    access: u32,
    inheritance: u32,
    mode: windows_sys::Win32::Security::Authorization::ACCESS_MODE,
) -> io::Result<()> {
    // The owning grant/retirement entrypoint holds ACL_EDITS while opening
    // and editing its exact objects, including all shared pinned ancestors.
    let mut old_acl: *mut ACL = null_mut();
    let mut descriptor = null_mut();
    let result = unsafe {
        GetSecurityInfo(
            file.as_raw_handle(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            &mut old_acl,
            null_mut(),
            &mut descriptor,
        )
    };
    if result != 0 {
        return Err(io::Error::from_raw_os_error(result as i32));
    }
    let descriptor = LocalAllocation(descriptor);
    if old_acl.is_null() {
        return Err(io::Error::other("refusing to replace an unrestricted DACL"));
    }
    let entry = EXPLICIT_ACCESS_W {
        grfAccessPermissions: access,
        grfAccessMode: mode,
        grfInheritance: inheritance,
        Trustee: TRUSTEE_W {
            pMultipleTrustee: null_mut(),
            MultipleTrusteeOperation: 0,
            TrusteeForm: TRUSTEE_IS_SID,
            TrusteeType: TRUSTEE_IS_UNKNOWN,
            ptstrName: sid.cast(),
        },
    };
    let mut acl = null_mut();
    let result = unsafe { SetEntriesInAclW(1, &entry, old_acl, &mut acl) };
    if result != 0 {
        return Err(io::Error::from_raw_os_error(result as i32));
    }
    let acl = LocalAllocation(acl.cast());
    let result = unsafe {
        SetSecurityInfo(
            file.as_raw_handle(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            acl.0.cast(),
            null(),
        )
    };
    drop((acl, descriptor));
    if result != 0 {
        return Err(io::Error::from_raw_os_error(result as i32));
    }
    Ok(())
}

struct LocalAllocation(*mut core::ffi::c_void);
impl Drop for LocalAllocation {
    fn drop(&mut self) {
        unsafe {
            LocalFree(self.0);
        }
    }
}

struct Attributes {
    bytes: Vec<usize>,
    initialized: bool,
}
impl Attributes {
    fn new(count: u32) -> io::Result<Self> {
        let mut size = 0;
        unsafe {
            InitializeProcThreadAttributeList(null_mut(), count, 0, &mut size);
        }
        if size == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut value = Self {
            bytes: vec![0; size.div_ceil(size_of::<usize>())],
            initialized: false,
        };
        if unsafe { InitializeProcThreadAttributeList(value.ptr(), count, 0, &mut size) } == 0 {
            return Err(io::Error::last_os_error());
        }
        value.initialized = true;
        Ok(value)
    }
    fn ptr(&mut self) -> *mut core::ffi::c_void {
        self.bytes.as_mut_ptr().cast()
    }
    fn set<T>(&mut self, attribute: u32, value: &T) -> io::Result<()> {
        self.set_raw(attribute, (value as *const T).cast(), size_of::<T>())
    }
    fn set_slice<T>(&mut self, attribute: u32, values: &[T]) -> io::Result<()> {
        self.set_raw(
            attribute,
            values.as_ptr().cast(),
            std::mem::size_of_val(values),
        )
    }
    fn set_raw(
        &mut self,
        attribute: u32,
        value: *const core::ffi::c_void,
        size: usize,
    ) -> io::Result<()> {
        if unsafe {
            UpdateProcThreadAttribute(
                self.ptr(),
                0,
                attribute as usize,
                value,
                size,
                null_mut(),
                null(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}
impl Drop for Attributes {
    fn drop(&mut self) {
        if self.initialized {
            unsafe {
                DeleteProcThreadAttributeList(self.ptr());
            }
        }
    }
}

fn pipe(output: bool, overlapped: bool) -> io::Result<(OwnedHandle, OwnedHandle)> {
    let name = wide(OsStr::new(&format!(
        r"\\.\pipe\Codewhale.Native.{}",
        uuid::Uuid::new_v4()
    )))?;
    // Parent endpoints are overlapped/noninheritable; exactly the synchronous
    // child endpoints are included in the process attribute HANDLE_LIST.
    let handle = unsafe {
        CreateNamedPipeW(
            name.as_ptr(),
            (if output {
                PIPE_ACCESS_INBOUND
            } else {
                PIPE_ACCESS_OUTBOUND
            }) | if overlapped { FILE_FLAG_OVERLAPPED } else { 0 }
                | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_WAIT,
            1,
            8192,
            8192,
            0,
            null(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    let parent = unsafe { OwnedHandle::from_raw_handle(handle) };
    let security = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: null_mut(),
        bInheritHandle: 1,
    };
    let handle = unsafe {
        CreateFileW(
            name.as_ptr(),
            if output {
                FILE_GENERIC_WRITE
            } else {
                FILE_GENERIC_READ
            },
            0,
            &security,
            OPEN_EXISTING,
            0,
            null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    let child = unsafe { OwnedHandle::from_raw_handle(handle) };
    if unsafe { ConnectNamedPipe(parent.as_raw_handle(), null_mut()) } == 0
        && unsafe { GetLastError() } != ERROR_PIPE_CONNECTED
    {
        return Err(io::Error::last_os_error());
    }
    Ok((parent, child))
}

fn wide(value: &OsStr) -> io::Result<Vec<u16>> {
    let mut value: Vec<_> = value.encode_wide().collect();
    if value.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Windows argv/env contains NUL",
        ));
    }
    value.push(0);
    Ok(value)
}

fn command_line(program: &OsStr, args: &[String]) -> io::Result<Vec<u16>> {
    // The documented CommandLineToArgvW/MS CRT quote+backslash rules. An
    // explicit application path means PATH/first-token parsing is never authority.
    let mut result = Vec::new();
    for arg in std::iter::once(program).chain(args.iter().map(OsStr::new)) {
        let units = wide(arg)?;
        if !result.is_empty() {
            result.push(b' ' as u16);
        }
        result.push(b'"' as u16);
        let mut slashes = 0;
        for unit in units.into_iter().take_while(|unit| *unit != 0) {
            if unit == b'\\' as u16 {
                slashes += 1;
                continue;
            }
            if unit == b'"' as u16 {
                result.extend(std::iter::repeat_n(b'\\' as u16, slashes * 2 + 1));
            } else {
                result.extend(std::iter::repeat_n(b'\\' as u16, slashes));
            }
            slashes = 0;
            result.push(unit);
        }
        result.extend(std::iter::repeat_n(b'\\' as u16, slashes * 2));
        result.push(b'"' as u16);
    }
    result.push(0);
    Ok(result)
}

fn environment_block(env: &[(OsString, OsString)]) -> io::Result<Vec<u16>> {
    let mut entries = env.iter().collect::<Vec<_>>();
    entries.sort_by_key(|(key, _)| key.to_string_lossy().to_uppercase());
    let mut result = Vec::new();
    for (key, value) in entries {
        let key = wide(key)?;
        if key.len() == 1 || key.contains(&(b'=' as u16)) {
            return Err(io::Error::other("invalid Windows environment key"));
        }
        result.extend(&key[..key.len() - 1]);
        result.push(b'=' as u16);
        result.extend(wide(value)?);
    }
    if result.is_empty() {
        result.push(0);
    }
    result.push(0);
    Ok(result)
}

#[cfg(test)]
#[path = "windows_tests.rs"]
mod tests;
