//! Workspace-confined file operations shared by Fleet artifacts and its ledger.

use std::fs::File;
use std::io::{self, Write};
use std::path::{Component, Path};

pub(crate) fn path_is_confined(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path.components().all(|component| match component {
            Component::Normal(name) => !cfg!(windows) || !name.as_encoded_bytes().contains(&b':'),
            _ => false,
        })
}

fn invalid_path() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "Fleet artifact path must stay within the workspace",
    )
}

#[cfg(unix)]
#[derive(Debug)]
pub(crate) struct WorkspaceFile {
    directory: File,
    filename: std::ffi::CString,
}

#[cfg(unix)]
impl WorkspaceFile {
    pub(crate) fn open(workspace: &Path, relative: &Path, create: bool) -> io::Result<Self> {
        use std::os::fd::{AsRawFd, FromRawFd};
        use std::os::unix::ffi::OsStrExt;
        if !path_is_confined(relative) {
            return Err(invalid_path());
        }
        let workspace = workspace.canonicalize()?;
        // Use the established credential/artifact openat pattern, without
        // touching credentials or creating a second filesystem store.
        // SAFETY: static path and immediate ownership of a successful fd.
        let fd = unsafe {
            libc::open(
                c"/".as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: this fd was just created and has no other owner.
        let mut directory = unsafe { File::from_raw_fd(fd) };
        let parents = relative.parent().ok_or_else(invalid_path)?;
        for (path, may_create) in [(workspace.as_path(), false), (parents, create)] {
            for component in path.components() {
                let Component::Normal(name) = component else {
                    if component == Component::RootDir {
                        continue;
                    }
                    return Err(invalid_path());
                };
                let name = std::ffi::CString::new(name.as_bytes())?;
                let flags = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
                // SAFETY: directory pins the parent; name is one component.
                let mut fd = unsafe { libc::openat(directory.as_raw_fd(), name.as_ptr(), flags) };
                if fd < 0
                    && may_create
                    && io::Error::last_os_error().kind() == io::ErrorKind::NotFound
                {
                    // SAFETY: directory and relative basename remain valid.
                    if unsafe { libc::mkdirat(directory.as_raw_fd(), name.as_ptr(), 0o700) } != 0
                        && io::Error::last_os_error().kind() != io::ErrorKind::AlreadyExists
                    {
                        return Err(io::Error::last_os_error());
                    }
                    // SAFETY: reject a symlink inserted after mkdirat.
                    fd = unsafe { libc::openat(directory.as_raw_fd(), name.as_ptr(), flags) };
                }
                if fd < 0 {
                    return Err(io::Error::last_os_error());
                }
                // SAFETY: fd is freshly owned.
                directory = unsafe { File::from_raw_fd(fd) };
            }
        }
        Ok(Self {
            directory,
            filename: std::ffi::CString::new(
                relative.file_name().ok_or_else(invalid_path)?.as_bytes(),
            )?,
        })
    }

    pub(crate) fn sibling(&self, name: &str) -> io::Result<Self> {
        if !path_is_confined(Path::new(name)) || Path::new(name).components().count() != 1 {
            return Err(invalid_path());
        }
        Ok(Self {
            directory: self.directory.try_clone()?,
            filename: std::ffi::CString::new(name)?,
        })
    }

    pub(crate) fn open_update(&self, create: bool, append: bool) -> io::Result<File> {
        self.open_with_flags(
            libc::O_RDWR
                | if create { libc::O_CREAT } else { 0 }
                | if append { libc::O_APPEND } else { 0 },
        )
    }

    pub(crate) fn open_file(&self) -> io::Result<File> {
        self.open_with_flags(libc::O_RDONLY)
    }

    fn open_with_flags(&self, flags: libc::c_int) -> io::Result<File> {
        use std::os::fd::{AsRawFd, FromRawFd};
        use std::os::unix::fs::MetadataExt;
        // SAFETY: a pinned parent and validated basename; never follows links.
        let fd = unsafe {
            libc::openat(
                self.directory.as_raw_fd(),
                self.filename.as_ptr(),
                flags | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
                0o600,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: fd is freshly owned.
        let file = unsafe { File::from_raw_fd(fd) };
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.nlink() != 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Fleet file must be a regular, non-hard-linked file",
            ));
        }
        Ok(file)
    }

    pub(crate) fn publish(&self, bytes: &[u8]) -> io::Result<()> {
        self.atomic_write(bytes, false)
    }

    pub(crate) fn replace(&self, bytes: &[u8]) -> io::Result<()> {
        self.atomic_write(bytes, true)
    }

    fn atomic_write(&self, bytes: &[u8], replace: bool) -> io::Result<()> {
        use std::os::fd::{AsRawFd, FromRawFd};
        let temporary =
            std::ffi::CString::new(format!(".fleet-write-{}.tmp", uuid::Uuid::new_v4()))
                .expect("generated basename");
        // SAFETY: parent is pinned; exclusive creation cannot follow a link.
        let fd = unsafe {
            libc::openat(
                self.directory.as_raw_fd(),
                temporary.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0o600,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: fd is freshly owned.
        let mut file = unsafe { File::from_raw_fd(fd) };
        let result = (|| {
            file.write_all(bytes)?;
            file.sync_all()?;
            // SAFETY: both basenames are anchored to the same open parent.
            // Replacement changes the directory entry, never a symlink target.
            let published = unsafe {
                if replace {
                    libc::renameat(
                        self.directory.as_raw_fd(),
                        temporary.as_ptr(),
                        self.directory.as_raw_fd(),
                        self.filename.as_ptr(),
                    )
                } else {
                    libc::linkat(
                        self.directory.as_raw_fd(),
                        temporary.as_ptr(),
                        self.directory.as_raw_fd(),
                        self.filename.as_ptr(),
                        0,
                    )
                }
            };
            if published != 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        })();
        // Successful rename already consumed this temporary entry. Never
        // unlink the vacant old name, which another writer could now reuse.
        if !replace || result.is_err() {
            // SAFETY: unlink this call's exclusive temporary basename.
            if unsafe { libc::unlinkat(self.directory.as_raw_fd(), temporary.as_ptr(), 0) } != 0 {
                return Err(io::Error::last_os_error());
            }
        }
        result?;
        self.directory.sync_all()
    }
}

#[cfg(windows)]
#[derive(Debug)]
pub(crate) struct WorkspaceFile {
    // Retaining every ancestor without delete/write sharing prevents a path
    // swap or junction replacement while path-based Windows calls are running.
    _ancestors: Vec<File>,
    directory: std::path::PathBuf,
    filename: std::ffi::OsString,
}

#[cfg(windows)]
impl WorkspaceFile {
    pub(crate) fn open(workspace: &Path, relative: &Path, create: bool) -> io::Result<Self> {
        use std::os::windows::fs::OpenOptionsExt;
        if !path_is_confined(relative) {
            return Err(invalid_path());
        }
        let workspace = workspace.canonicalize()?;
        let mut ancestors = Vec::new();
        let mut directory = std::path::PathBuf::new();
        for (path, may_create) in [
            (workspace.as_path(), false),
            (relative.parent().ok_or_else(invalid_path)?, create),
        ] {
            for component in path.components() {
                directory.push(component.as_os_str());
                if matches!(component, Component::Prefix(_)) {
                    continue;
                }
                let open = || {
                    std::fs::OpenOptions::new()
                        .read(true)
                        .share_mode(0x0000_0001)
                        .custom_flags(0x0220_0000)
                        .open(&directory)
                }; // BACKUP_SEMANTICS | OPEN_REPARSE_POINT
                let file = match open() {
                    Err(error) if may_create && error.kind() == io::ErrorKind::NotFound => {
                        match std::fs::create_dir(&directory) {
                            Ok(()) => {}
                            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                            Err(error) => return Err(error),
                        }
                        open()?
                    }
                    result => result?,
                };
                let metadata = file.metadata()?;
                if !metadata.is_dir() || crate::plugins::metadata_is_link_or_reparse(&metadata) {
                    return Err(invalid_path());
                }
                ancestors.push(file);
            }
        }
        Ok(Self {
            _ancestors: ancestors,
            directory,
            filename: relative.file_name().ok_or_else(invalid_path)?.to_owned(),
        })
    }

    pub(crate) fn sibling(&self, name: &str) -> io::Result<Self> {
        if !path_is_confined(Path::new(name)) || Path::new(name).components().count() != 1 {
            return Err(invalid_path());
        }
        Ok(Self {
            _ancestors: self
                ._ancestors
                .iter()
                .map(File::try_clone)
                .collect::<io::Result<_>>()?,
            directory: self.directory.clone(),
            filename: name.into(),
        })
    }

    pub(crate) fn open_update(&self, create: bool, append: bool) -> io::Result<File> {
        use std::os::windows::fs::OpenOptionsExt;
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .append(append)
            .create(create)
            .truncate(false)
            .share_mode(0x0000_0007)
            .custom_flags(0x0020_0000)
            .open(self.directory.join(&self.filename))?;
        let metadata = file.metadata()?;
        if !metadata.is_file()
            || crate::plugins::metadata_is_link_or_reparse(&metadata)
            || crate::plugins::windows_file_identity(&file)?.links != 1
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Fleet file must be regular and not linked",
            ));
        }
        Ok(file)
    }

    pub(crate) fn open_file(&self) -> io::Result<File> {
        // Existing protected reader rejects reparse points, hard links and
        // non-regular files, and denies concurrent writes/replacement.
        crate::plugins::manifest::open_bundle_file(&self.directory.join(&self.filename))
    }

    pub(crate) fn publish(&self, bytes: &[u8]) -> io::Result<()> {
        self.atomic_write(bytes, false)
    }

    pub(crate) fn replace(&self, bytes: &[u8]) -> io::Result<()> {
        self.atomic_write(bytes, true)
    }

    fn atomic_write(&self, bytes: &[u8], replace: bool) -> io::Result<()> {
        use std::mem::{offset_of, size_of};
        use std::os::windows::ffi::OsStrExt;
        use std::os::windows::fs::OpenOptionsExt;
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Wdk::Storage::FileSystem::{
            FILE_RENAME_INFORMATION, FileRenameInformation, NtSetInformationFile,
        };
        use windows_sys::Win32::Foundation::RtlNtStatusToDosError;
        use windows_sys::Win32::Storage::FileSystem::{
            DELETE, FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_SHARE_READ,
        };
        use windows_sys::Win32::System::IO::IO_STATUS_BLOCK;

        let mut temporary =
            tempfile::Builder::new()
                .prefix(".fleet-write-")
                .make_in(&self.directory, |path| {
                    std::fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .access_mode(FILE_GENERIC_READ | FILE_GENERIC_WRITE | DELETE)
                        .share_mode(FILE_SHARE_READ)
                        .open(path)
                })?;
        let result = (|| {
            temporary.write_all(bytes)?;
            temporary.as_file().sync_all()?;

            // MoveFileExW (including tempfile::persist) reopens the destination
            // directory with FILE_ADD_FILE, conflicting with our ancestor pins.
            // A native rename with no root handle and a single basename uses
            // the source file's existing parent instead. Keep all ancestor and
            // source handles pinned; never relax their write/delete guards.
            // https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntifs/ns-ntifs-_file_rename_information
            let name = self.filename.encode_wide().collect::<Vec<_>>();
            let name_bytes = name.len() * size_of::<u16>();
            let buffer_size = (offset_of!(FILE_RENAME_INFORMATION, FileName) + name_bytes)
                .max(size_of::<FILE_RENAME_INFORMATION>());
            let mut buffer = vec![0_usize; buffer_size.div_ceil(size_of::<usize>())];
            let rename = buffer.as_mut_ptr().cast::<FILE_RENAME_INFORMATION>();
            // SAFETY: the zeroed buffer is aligned and covers the struct plus
            // the complete UTF-16 basename; no root means the source's parent.
            unsafe {
                (*rename).Anonymous.ReplaceIfExists = replace;
                (*rename).FileNameLength = name_bytes as u32;
                std::ptr::copy_nonoverlapping(
                    name.as_ptr(),
                    (*rename).FileName.as_mut_ptr(),
                    name.len(),
                );
            }
            let mut attempt = 0;
            loop {
                let mut status = IO_STATUS_BLOCK::default();
                // SAFETY: this synchronously opened file has DELETE access;
                // every handle and buffer remains live throughout the call.
                let result = unsafe {
                    NtSetInformationFile(
                        temporary.as_file().as_raw_handle(),
                        &mut status,
                        rename.cast(),
                        buffer_size as u32,
                        FileRenameInformation,
                    )
                };
                if result >= 0 {
                    return Ok(());
                }
                // SAFETY: converts the returned NTSTATUS without dereferencing.
                let error =
                    io::Error::from_raw_os_error(unsafe { RtlNtStatusToDosError(result) as i32 });
                let Some(backoff) = crate::utils::windows_publish_retry_delay(&error, attempt)
                else {
                    return Err(error);
                };
                std::thread::sleep(backoff);
                attempt += 1;
            }
        })();
        match result {
            Ok(()) => {
                // The old name is vacant after the rename; do not unlink an
                // entry another process might create there afterwards.
                temporary.disable_cleanup(true);
                Ok(())
            }
            Err(error) => {
                // Close the source's delete-denying handle before cleanup.
                temporary.into_temp_path().close()?;
                Err(error)
            }
        }
    }
}

#[cfg(all(test, windows))]
mod windows_publication_tests {
    use super::*;
    use std::os::windows::fs::OpenOptionsExt;

    #[test]
    fn publication_and_replacement_keep_ancestor_write_and_delete_guards() {
        let workspace = tempfile::tempdir().unwrap();
        let parent = workspace.path().join("private");
        let ledger =
            WorkspaceFile::open(workspace.path(), Path::new("private/fleet.jsonl"), true).unwrap();
        let forbidden = std::fs::OpenOptions::new()
            .write(true)
            .custom_flags(0x0220_0000)
            .open(&parent)
            .unwrap_err();
        assert_eq!(forbidden.raw_os_error(), Some(32));
        assert!(std::fs::rename(&parent, workspace.path().join("swapped")).is_err());

        // Both fail with MoveFileExW while the destination parent is pinned.
        ledger.publish(b"first").unwrap();
        ledger.replace(b"compacted").unwrap();
        assert_eq!(
            std::fs::read(parent.join("fleet.jsonl")).unwrap(),
            b"compacted"
        );
        assert_eq!(std::fs::read_dir(&parent).unwrap().count(), 1);
    }

    #[test]
    fn replacement_survives_a_short_lived_reader_without_delete_sharing() {
        let workspace = tempfile::tempdir().unwrap();
        let relative = Path::new("fleet.jsonl");
        let ledger = WorkspaceFile::open(workspace.path(), relative, true).unwrap();
        ledger.publish(b"first").unwrap();
        let held = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0x1 | 0x2)
            .open(workspace.path().join(relative))
            .unwrap();
        let release = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(50));
            drop(held);
        });
        ledger.replace(b"compacted").unwrap();
        release.join().unwrap();
        assert_eq!(
            std::fs::read(workspace.path().join(relative)).unwrap(),
            b"compacted"
        );
    }

    #[test]
    fn immutable_publication_preserves_existing_bytes_and_removes_its_temporary() {
        let workspace = tempfile::tempdir().unwrap();
        let artifact =
            WorkspaceFile::open(workspace.path(), Path::new("receipt.json"), true).unwrap();
        artifact.publish(b"receipt").unwrap();
        assert_eq!(
            artifact.publish(b"replacement").unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(
            std::fs::read(workspace.path().join("receipt.json")).unwrap(),
            b"receipt"
        );
        assert_eq!(std::fs::read_dir(workspace.path()).unwrap().count(), 1);
    }

    #[test]
    fn artifact_paths_cannot_name_windows_alternate_data_streams() {
        for path in ["receipt.json:private", "dir/receipt:private", ":stream"] {
            assert!(
                !path_is_confined(Path::new(path)),
                "accepted stream path {path}"
            );
        }
    }
}

#[cfg(all(not(unix), not(windows)))]
#[derive(Debug)]
pub(crate) struct WorkspaceFile;
#[cfg(all(not(unix), not(windows)))]
impl WorkspaceFile {
    pub(crate) fn open(_: &Path, _: &Path, _: bool) -> io::Result<Self> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Confined Fleet artifact I/O is unavailable on this platform",
        ))
    }
    pub(crate) fn sibling(&self, _: &str) -> io::Result<Self> {
        unreachable!()
    }
    pub(crate) fn open_update(&self, _: bool, _: bool) -> io::Result<File> {
        unreachable!()
    }
    pub(crate) fn replace(&self, _: &[u8]) -> io::Result<()> {
        unreachable!()
    }
    pub(crate) fn open_file(&self) -> io::Result<File> {
        unreachable!()
    }
    pub(crate) fn publish(&self, _: &[u8]) -> io::Result<()> {
        unreachable!()
    }
}

/// Compare already opened lock handles; a replaced lock must never create two
/// independent critical sections for the same live ledger.
pub(crate) fn same_file(left: &File, right: &File) -> io::Result<bool> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let a = left.metadata()?;
        let b = right.metadata()?;
        Ok(a.dev() == b.dev() && a.ino() == b.ino())
    }
    #[cfg(windows)]
    {
        let a = crate::plugins::windows_file_identity(left)?;
        let b = crate::plugins::windows_file_identity(right)?;
        Ok(a.volume == b.volume && a.index == b.index)
    }
    #[cfg(all(not(unix), not(windows)))]
    {
        let _ = (left, right);
        unreachable!()
    }
}
