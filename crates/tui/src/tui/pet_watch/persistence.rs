//! Session-owned pet sidecar, using the existing confined artifact I/O. A
//! writer lock plus content revision rejects concurrent or external edits.
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::artifacts::{open_session_relative, write_session_relative_immutable};
use crate::fleet::files::{WorkspaceFile, same_file};

const MAX_BYTES: usize = 8 * 1024 * 1024;
const HABITAT: &str = "artifacts/pet/habitat.json";

pub struct Store {
    data: WorkspaceFile,
    lock: WorkspaceFile,
    original_lock: File,
    expected: Option<[u8; 32]>,
}

impl Store {
    pub fn open(session: &str) -> io::Result<Self> {
        let data = open_session_relative(session, Path::new(HABITAT), true)?;
        let lock = open_session_relative(session, Path::new("artifacts/pet/habitat.lock"), true)?;
        let original_lock = lock.open_update(true, false)?;
        Ok(Self {
            data,
            lock,
            original_lock,
            expected: None,
        })
    }

    fn with_lock<T>(&self, action: impl FnOnce() -> io::Result<T>) -> io::Result<T> {
        let file = self.lock.open_update(false, false)?;
        if !same_file(&file, &self.original_lock)? {
            return Err(io::Error::other("Pet habitat lock was replaced"));
        }
        let mut lock = fd_lock::RwLock::new(file);
        // Never wait behind another process in the world worker.
        let _guard = lock.try_write()?;
        action()
    }

    fn read(&self) -> io::Result<Option<Vec<u8>>> {
        let file = match self.data.open_file() {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let mut bytes = Vec::new();
        file.take(MAX_BYTES as u64 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > MAX_BYTES {
            return Err(io::Error::other("Pet habitat exceeds 8 MiB"));
        }
        Ok(Some(bytes))
    }

    pub fn load(&mut self) -> io::Result<Option<String>> {
        let bytes = self.with_lock(|| self.read())?;
        let text = bytes
            .as_ref()
            .map(|b| {
                String::from_utf8(b.clone())
                    .map_err(|_| io::Error::other("Pet habitat is not UTF-8"))
            })
            .transpose()?;
        self.expected = bytes.map(|b| Sha256::digest(b).into());
        Ok(text)
    }

    pub fn save(&mut self, text: &str) -> io::Result<()> {
        if text.len() > MAX_BYTES {
            return Err(io::Error::other("Pet habitat exceeds 8 MiB"));
        }
        self.with_lock(|| {
            let current = self.read()?.map(|b| <[u8; 32]>::from(Sha256::digest(b)));
            if current != self.expected {
                return Err(io::Error::other("Another writer changed the pet habitat"));
            }
            self.data.replace(text.as_bytes())
        })?;
        self.expected = Some(Sha256::digest(text.as_bytes()).into());
        Ok(())
    }
}

pub fn export(session: &str, text: &str) -> io::Result<PathBuf> {
    if text.len() > MAX_BYTES {
        return Err(io::Error::other("Pet replay exceeds 8 MiB"));
    }
    let relative = PathBuf::from(format!(
        "artifacts/pet/replay-{}.json",
        uuid::Uuid::new_v4()
    ));
    write_session_relative_immutable(session, &relative, text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(root: &Path) -> Store {
        let data = WorkspaceFile::open(root, Path::new("habitat.json"), true).unwrap();
        let lock = WorkspaceFile::open(root, Path::new("habitat.lock"), true).unwrap();
        let original_lock = lock.open_update(true, false).unwrap();
        Store {
            data,
            lock,
            original_lock,
            expected: None,
        }
    }

    #[test]
    fn stale_writer_cannot_replace_a_newer_recording_or_external_edit() {
        let root = tempfile::tempdir().unwrap();
        let mut first = store(root.path());
        let mut second = store(root.path());
        assert!(first.load().unwrap().is_none());
        assert!(second.load().unwrap().is_none());
        first.save("first recording").unwrap();
        assert!(second.save("stale recording").is_err());
        assert_eq!(
            std::fs::read_to_string(root.path().join("habitat.json")).unwrap(),
            "first recording"
        );
        std::fs::write(root.path().join("habitat.json"), "external edit").unwrap();
        assert!(first.save("lost edit").is_err());
        assert_eq!(first.load().unwrap().as_deref(), Some("external edit"));
    }

    #[test]
    fn invalid_and_oversized_habitats_remain_intact() {
        let root = tempfile::tempdir().unwrap();
        let mut files = store(root.path());
        let path = root.path().join("habitat.json");
        std::fs::write(&path, [0xff]).unwrap();
        assert!(files.load().is_err());
        assert!(files.save("replacement").is_err());
        assert_eq!(std::fs::read(&path).unwrap(), [0xff]);
        std::fs::write(&path, vec![b' '; MAX_BYTES + 1]).unwrap();
        assert!(files.load().is_err());
        assert!(files.save("replacement").is_err());
        assert_eq!(
            std::fs::metadata(&path).unwrap().len(),
            MAX_BYTES as u64 + 1
        );
    }

    #[cfg(unix)]
    #[test]
    fn private_atomic_files_reject_links_and_replaced_locks() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let root = tempfile::tempdir().unwrap();
        let mut files = store(root.path());
        files.save("private").unwrap();
        let path = root.path().join("habitat.json");
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let outside = root.path().join("outside");
        std::fs::rename(&path, &outside).unwrap();
        symlink(&outside, &path).unwrap();
        assert!(files.load().is_err());
        assert!(files.save("replacement").is_err());
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), "private");
        std::fs::remove_file(&path).unwrap();
        std::fs::hard_link(&outside, &path).unwrap();
        assert!(files.load().is_err());
        std::fs::remove_file(root.path().join("habitat.lock")).unwrap();
        let _new_owner = store(root.path());
        assert!(files.save("split lock").is_err());
    }
}
