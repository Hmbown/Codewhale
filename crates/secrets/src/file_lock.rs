//! Shared file-store write lock. Native uses the same fd-lock 4.0.4 and
//! persistent `<filename>.lock` protocol. Never delete or replace that inode.
//! Older writers bypassing this protocol cannot participate safely.
use crate::SecretsError;
use std::{fs, path::Path};

pub(crate) fn open_private(path: &Path, create: bool) -> Result<fs::File, SecretsError> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() || !meta.is_file() => {
            return Err(std::io::Error::other("The secret store must be a regular file.").into());
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let mut options = fs::OpenOptions::new();
    options
        .read(true)
        .write(create)
        .create(create)
        .truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x0020_0000); // FILE_FLAG_OPEN_REPARSE_POINT
    }
    let file = options.open(path)?;
    let meta = file.metadata()?;
    if !meta.is_file() {
        return Err(std::io::Error::other("The secret store must be a regular file.").into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = meta.permissions().mode() & 0o777;
        if mode & 0o077 != 0 {
            return Err(SecretsError::InsecurePermissions {
                path: path.into(),
                mode,
            });
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return Err(
                std::io::Error::other("The secret store cannot be a reparse point.").into(),
            );
        }
    }
    Ok(file)
}

pub(crate) fn with_write_lock<T>(
    path: &Path,
    operation: impl FnOnce(&Path) -> Result<T, SecretsError>,
) -> Result<T, SecretsError> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut directory = fs::DirBuilder::new();
    directory.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        directory.mode(0o700);
    }
    directory.create(parent)?;
    let filename = path
        .file_name()
        .ok_or_else(|| std::io::Error::other("Invalid secret store path."))?;
    let path = parent.canonicalize()?.join(filename);
    let mut lock_name = path.as_os_str().to_os_string();
    lock_name.push(".lock");
    let mut lock = fd_lock::RwLock::new(open_private(Path::new(&lock_name), true)?);
    let _guard = lock.write()?;
    operation(&path)
}
