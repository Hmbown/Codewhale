//! Process-tree containment for a spawned child and everything it starts.
//!
//! Moved out of `hooks/executor.rs` (where it was `HookProcessTree` /
//! `WindowsHookJob`) so the hook runner and the extension host share one
//! implementation. Killing only the immediate child can leave the real work
//! alive (a hook's shell runtime, a host plugin's `child_process`), so:
//!
//! * **Unix:** the child must be spawned as the leader of its own process
//!   group (`process_group(0)`); the tree is that group, and it is SIGKILLed
//!   as a whole.
//! * **Windows:** the child is assigned to a Job Object configured with
//!   `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, so closing the job kills every
//!   process in it. Hooks are created suspended and resumed only after the
//!   assignment, so no descendant can escape; the extension host is not
//!   (tokio's `Command` does not expose a suspended spawn), which leaves a
//!   window of microseconds before assignment in which Node has not yet run
//!   any plugin code.
//!
//! Dropping the guard kills the tree. That is deliberate: the guard's lifetime
//! is the tree's lifetime.

#[cfg(windows)]
use windows::Win32::Foundation::{CloseHandle, HANDLE};
#[cfg(windows)]
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject, TerminateJobObject,
};
#[cfg(windows)]
use windows::core::PCWSTR;

/// Owns the process tree rooted at one spawned child.
pub(crate) struct ProcessTree {
    #[cfg(unix)]
    pgid: libc::pid_t,
    #[cfg(windows)]
    job: WindowsJob,
}

// SAFETY (Windows): the job handle is an owned kernel handle; it is only
// used through `&self` calls that the OS serializes, and closed once in Drop.
#[cfg(windows)]
unsafe impl Send for ProcessTree {}
#[cfg(windows)]
unsafe impl Sync for ProcessTree {}

impl ProcessTree {
    /// Contain a `std::process::Child` (spawned with `process_group(0)` on Unix).
    pub(crate) fn attach(child: &std::process::Child) -> std::io::Result<Self> {
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            Self::attach_parts(child.id(), child.as_raw_handle())
        }
        #[cfg(not(windows))]
        {
            Self::attach_parts(child.id())
        }
    }

    /// Contain a `tokio::process::Child` (spawned with `process_group(0)` on
    /// Unix). Fails if the child has already been reaped.
    pub(crate) fn attach_tokio(child: &tokio::process::Child) -> std::io::Result<Self> {
        let pid = child
            .id()
            .ok_or_else(|| std::io::Error::other("child already exited"))?;
        #[cfg(windows)]
        {
            let handle = child
                .raw_handle()
                .ok_or_else(|| std::io::Error::other("child already exited"))?;
            Self::attach_parts(pid, handle)
        }
        #[cfg(not(windows))]
        {
            Self::attach_parts(pid)
        }
    }

    #[cfg(windows)]
    fn attach_parts(_pid: u32, handle: std::os::windows::io::RawHandle) -> std::io::Result<Self> {
        Ok(Self {
            job: WindowsJob::attach(handle)?,
        })
    }

    #[cfg(not(windows))]
    #[allow(clippy::unnecessary_wraps)]
    fn attach_parts(pid: u32) -> std::io::Result<Self> {
        #[cfg(unix)]
        {
            Ok(Self {
                pgid: pid as libc::pid_t,
            })
        }
        #[cfg(not(unix))]
        {
            let _ = pid;
            Ok(Self {})
        }
    }

    /// Kill every process in the tree. A tree that is already gone is not an
    /// error. On failure the caller should fall back to killing the child.
    pub(crate) fn kill(&self) -> std::io::Result<()> {
        #[cfg(unix)]
        {
            // SAFETY: kill(2) dereferences no pointers; a negative pid names
            // the process group.
            let result = unsafe { libc::kill(-self.pgid, libc::SIGKILL) };
            if result != 0 {
                let error = std::io::Error::last_os_error();
                if error.raw_os_error() != Some(libc::ESRCH) {
                    return Err(error);
                }
            }
            Ok(())
        }
        #[cfg(windows)]
        {
            self.job.terminate()
        }
        #[cfg(not(any(unix, windows)))]
        {
            Err(std::io::Error::other(
                "process-tree containment is not supported on this platform",
            ))
        }
    }
}

impl Drop for ProcessTree {
    fn drop(&mut self) {
        #[cfg(unix)]
        // SAFETY: kill(2) dereferences no pointers.
        unsafe {
            // The leader may have exited while a descendant still holds an
            // inherited pipe. Reaping the group keeps lifetimes bounded.
            let _ = libc::kill(-self.pgid, libc::SIGKILL);
        }
        // On Windows, dropping `WindowsJob` closes a Job Object configured
        // with JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE.
    }
}

#[cfg(windows)]
struct WindowsJob {
    handle: HANDLE,
}

#[cfg(windows)]
impl WindowsJob {
    fn attach(child: std::os::windows::io::RawHandle) -> std::io::Result<Self> {
        // SAFETY: returned handle is owned by the new wrapper.
        let handle = unsafe { CreateJobObjectW(None, PCWSTR::null()).map_err(windows_io_error)? };
        let job = Self { handle };
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

        // SAFETY: `limits` is live with matching size; both handles are live.
        unsafe {
            SetInformationJobObject(
                job.handle,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const core::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
            .map_err(windows_io_error)?;
            AssignProcessToJobObject(job.handle, HANDLE(child)).map_err(windows_io_error)?;
        }
        Ok(job)
    }

    fn terminate(&self) -> std::io::Result<()> {
        // SAFETY: `self.handle` is a live owned job handle.
        unsafe { TerminateJobObject(self.handle, 1).map_err(windows_io_error) }
    }
}

#[cfg(windows)]
impl Drop for WindowsJob {
    fn drop(&mut self) {
        // SAFETY: `self.handle` is owned here; Drop runs once.
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

#[cfg(windows)]
pub(crate) fn windows_io_error(error: windows::core::Error) -> std::io::Error {
    std::io::Error::other(error)
}
