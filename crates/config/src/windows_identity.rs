//! Existing Windows current-user token/SID custody, shared by owned storage and IPC.

use anyhow::{Context, Result, bail};
use std::os::windows::io::FromRawHandle as _;

#[cfg(windows)]
pub struct CurrentWindowsUser {
    _token: std::os::windows::io::OwnedHandle,
    token_info: Vec<usize>,
}

#[cfg(windows)]
impl CurrentWindowsUser {
    pub fn open() -> Result<Self> {
        // SAFETY: Windows supplies a process pseudo-handle; no ownership transfer.
        Self::from_process(unsafe { windows_sys::Win32::System::Threading::GetCurrentProcess() })
    }

    fn from_process(process: windows_sys::Win32::Foundation::HANDLE) -> Result<Self> {
        use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE};
        use windows_sys::Win32::Security::{
            GetTokenInformation, TOKEN_QUERY, TOKEN_USER, TokenUser,
        };
        use windows_sys::Win32::System::Threading::OpenProcessToken;

        let mut token: HANDLE = std::ptr::null_mut();
        // SAFETY: the pseudo-process handle is valid and `token` is writable.
        if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) } == 0 {
            return Err(std::io::Error::last_os_error())
                .context("opening current Windows user token");
        }
        let mut needed = 0;
        // SAFETY: a null buffer/zero length asks for the required size.
        let _ =
            unsafe { GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut needed) };
        if needed < std::mem::size_of::<TOKEN_USER>() as u32 || needed > 65536 {
            let error = std::io::Error::from_raw_os_error(unsafe { GetLastError() } as i32);
            // SAFETY: the token is owned on this error path.
            unsafe { CloseHandle(token) };
            return Err(error).context("sizing current Windows user token information");
        }
        let words = (needed as usize).div_ceil(std::mem::size_of::<usize>());
        let mut token_info = vec![0usize; words];
        // SAFETY: the aligned buffer contains at least `needed` writable bytes.
        if unsafe {
            GetTokenInformation(
                token,
                TokenUser,
                token_info.as_mut_ptr().cast(),
                needed,
                &mut needed,
            )
        } == 0
        {
            let error = std::io::Error::last_os_error();
            // SAFETY: the token is owned on this error path.
            unsafe { CloseHandle(token) };
            return Err(error).context("reading current Windows user token information");
        }
        let user = unsafe { &*token_info.as_ptr().cast::<TOKEN_USER>() };
        if user.User.Sid.is_null() {
            // SAFETY: the token is owned on this error path.
            unsafe { CloseHandle(token) };
            bail!("current Windows user token has no SID");
        }
        // SAFETY: the token was opened above and is transferred exactly once.
        let token = unsafe { std::os::windows::io::OwnedHandle::from_raw_handle(token) };
        Ok(Self {
            _token: token,
            token_info,
        })
    }

    pub fn sid_string(&self) -> Result<String> {
        use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
        let mut text = std::ptr::null_mut();
        // SAFETY: the validated SID remains held and the output is writable.
        if unsafe { ConvertSidToStringSidW(self.sid(), &mut text) } == 0 {
            return Err(std::io::Error::last_os_error()).context("encoding current Windows SID");
        }
        let _text = WindowsLocalAllocation(text.cast());
        anyhow::ensure!(!text.is_null(), "Windows SID text unavailable");
        let mut units = Vec::new();
        for index in 0..192 {
            let unit = unsafe { *text.add(index) };
            if unit == 0 {
                return String::from_utf16(&units).map_err(Into::into);
            }
            units.push(unit);
        }
        bail!("Windows SID text exceeds its fixed bound")
    }

    pub fn sid(&self) -> windows_sys::Win32::Security::PSID {
        use windows_sys::Win32::Security::TOKEN_USER;
        // SAFETY: the aligned token buffer remains owned by `self`.
        unsafe { (*self.token_info.as_ptr().cast::<TOKEN_USER>()).User.Sid }
    }
}

#[cfg(windows)]
pub(crate) struct WindowsLocalAllocation(pub(crate) *mut core::ffi::c_void);

#[cfg(windows)]
impl Drop for WindowsLocalAllocation {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: Windows allocated this block for a LocalFree caller.
            unsafe { windows_sys::Win32::Foundation::LocalFree(self.0) };
        }
    }
}

/// The actual kernel-selected peer process and token, held across control admission.
/// A display PID or a same-user SID is never sufficient to select an owner.
pub struct WindowsPeerProcess {
    process: std::os::windows::io::OwnedHandle,
    user: CurrentWindowsUser,
    pid: u32,
    start: String,
}

impl WindowsPeerProcess {
    pub fn open_current_user(pid: u32) -> Result<Self> {
        use std::os::windows::io::AsRawHandle as _;
        use windows_sys::Win32::Storage::FileSystem::SYNCHRONIZE;
        use windows_sys::Win32::System::Threading::{
            GetProcessId, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };
        anyhow::ensure!(pid > 0, "invalid Windows peer PID");
        let process =
            unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE, 0, pid) };
        anyhow::ensure!(!process.is_null(), "Windows peer process unavailable");
        let process = unsafe { std::os::windows::io::OwnedHandle::from_raw_handle(process) };
        anyhow::ensure!(
            unsafe { GetProcessId(process.as_raw_handle()) } == pid,
            "Windows kernel peer PID changed"
        );
        let user = CurrentWindowsUser::from_process(process.as_raw_handle())?;
        let start = process_creation(process.as_raw_handle())?;
        let value = Self {
            process,
            user,
            pid,
            start,
        };
        value.check_current_user()?;
        Ok(value)
    }
    pub fn pid(&self) -> u32 {
        self.pid
    }
    pub fn start(&self) -> &str {
        &self.start
    }
    pub fn principal(&self) -> Result<String> {
        self.user.sid_string()
    }
    pub fn check_current_user(&self) -> Result<()> {
        use std::os::windows::io::AsRawHandle as _;
        use windows_sys::Win32::Security::EqualSid;
        use windows_sys::Win32::System::Threading::GetExitCodeProcess;
        let mut status = 0;
        anyhow::ensure!(
            unsafe { GetExitCodeProcess(self.process.as_raw_handle(), &mut status) } != 0
                && status == 259,
            "Windows peer process has exited"
        );
        let current = CurrentWindowsUser::open()?;
        let peer = CurrentWindowsUser::from_process(self.process.as_raw_handle())?;
        anyhow::ensure!(
            unsafe { EqualSid(current.sid(), peer.sid()) } != 0
                && unsafe { EqualSid(self.user.sid(), peer.sid()) } != 0,
            "Windows peer principal changed"
        );
        anyhow::ensure!(
            process_creation(self.process.as_raw_handle())? == self.start,
            "Windows peer generation changed"
        );
        Ok(())
    }
}

fn process_creation(process: windows_sys::Win32::Foundation::HANDLE) -> Result<String> {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::Threading::GetProcessTimes;
    let (mut creation, mut exit, mut kernel, mut user) = (
        FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        },
        FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        },
        FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        },
        FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        },
    );
    anyhow::ensure!(
        unsafe { GetProcessTimes(process, &mut creation, &mut exit, &mut kernel, &mut user) } != 0,
        "Windows process creation identity unavailable"
    );
    Ok(format!(
        "windows:{}:{}",
        creation.dwHighDateTime, creation.dwLowDateTime
    ))
}

/// One current-user ACL builder, immediately adopted by private storage and
/// named-pipe creation. Both commit the same protected owner/DACL policy.
pub struct OwnerOnlyAcl {
    user: CurrentWindowsUser,
    acl: WindowsLocalAllocation,
}
impl OwnerOnlyAcl {
    pub fn new(access: u32, inherit_to_children: bool) -> Result<Self> {
        use windows_sys::Win32::Security::Authorization::{
            EXPLICIT_ACCESS_W, SET_ACCESS, SetEntriesInAclW, TRUSTEE_IS_SID, TRUSTEE_IS_USER,
            TRUSTEE_W,
        };
        use windows_sys::Win32::Security::{NO_INHERITANCE, SUB_CONTAINERS_AND_OBJECTS_INHERIT};
        let user = CurrentWindowsUser::open()?;
        let entry = EXPLICIT_ACCESS_W {
            grfAccessPermissions: access,
            grfAccessMode: SET_ACCESS,
            grfInheritance: if inherit_to_children {
                SUB_CONTAINERS_AND_OBJECTS_INHERIT
            } else {
                NO_INHERITANCE
            },
            Trustee: TRUSTEE_W {
                pMultipleTrustee: std::ptr::null_mut(),
                MultipleTrusteeOperation: 0,
                TrusteeForm: TRUSTEE_IS_SID,
                TrusteeType: TRUSTEE_IS_USER,
                ptstrName: user.sid().cast(),
            },
        };
        let mut acl = std::ptr::null_mut();
        let status = unsafe { SetEntriesInAclW(1, &entry, std::ptr::null(), &mut acl) };
        anyhow::ensure!(
            status == 0,
            "building current-user-only ACL failed ({status})"
        );
        Ok(Self {
            user,
            acl: WindowsLocalAllocation(acl.cast()),
        })
    }
    pub(crate) fn user_sid(&self) -> windows_sys::Win32::Security::PSID {
        self.user.sid()
    }
    pub(crate) fn acl(&self) -> *mut windows_sys::Win32::Security::ACL {
        self.acl.0.cast()
    }
    pub fn with_security_attributes<T>(
        &self,
        create: impl FnOnce(*mut core::ffi::c_void) -> Result<T>,
    ) -> Result<T> {
        use windows_sys::Win32::Security::{
            InitializeSecurityDescriptor, SE_DACL_PROTECTED, SECURITY_ATTRIBUTES,
            SECURITY_DESCRIPTOR, SetSecurityDescriptorControl, SetSecurityDescriptorDacl,
            SetSecurityDescriptorOwner,
        };
        let mut descriptor = SECURITY_DESCRIPTOR::default();
        let pointer = (&mut descriptor as *mut SECURITY_DESCRIPTOR).cast();
        anyhow::ensure!(
            unsafe { InitializeSecurityDescriptor(pointer, 1) } != 0,
            "initializing owner security descriptor"
        );
        anyhow::ensure!(
            unsafe { SetSecurityDescriptorOwner(pointer, self.user.sid(), 0) } != 0,
            "setting owner SID"
        );
        anyhow::ensure!(
            unsafe { SetSecurityDescriptorDacl(pointer, 1, self.acl(), 0) } != 0,
            "setting owner-only DACL"
        );
        anyhow::ensure!(
            unsafe { SetSecurityDescriptorControl(pointer, SE_DACL_PROTECTED, SE_DACL_PROTECTED) }
                != 0,
            "protecting owner-only DACL"
        );
        let mut attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: pointer,
            bInheritHandle: 0,
        };
        create((&mut attributes as *mut SECURITY_ATTRIBUTES).cast())
    }
}
