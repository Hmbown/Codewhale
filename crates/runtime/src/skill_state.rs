//! Persistent enable/disable state shared by runtime/API Skill catalogs.
//!
//! Backs `GET /v1/skills` (`enabled` field per skill) and
//! `POST /v1/skills/{name}` (toggle). Discovery tells us which Skills exist;
//! this store is the final exact-name activation filter shared by prompts,
//! tools, TUI surfaces, sub-agents, and the API. Plugin trust/enablement stays
//! a separate bundle lifecycle gate.
//!
//! Storage shape (TOML at `~/.codewhale/skills_state.toml`, legacy `~/.deepseek/skills_state.toml`):
//!
//! ```toml
//! disabled = ["skill-name-1", "skill-name-2"]
//! ```
//!
//! Reserved `!codewhale-skill-state:1:*` entries retain legacy vetoes and
//! exact enables in that same set. The leading `!` cannot be a discovered
//! skill name. v0.10.0 writers preserve these strings, unlike unknown TOML
//! fields. This preserves new-reader policy through their serialization;
//! it does not make old readers understand distinct Unicode identities or
//! old lossy/no-op toggles express the new per-skill choices. Upgrade all
//! runtimes sharing this directory for consistent activation controls.
//!
//! Default state when the file does not exist: empty list (everything enabled).
//! A present but unreadable or malformed file is an error. Callers may keep
//! native Skills available for recovery, but reviewed plugin Skills must stay
//! hidden until their exact activation state can be read authoritatively.

use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const STATE_FILE_NAME: &str = "skills_state.toml";
const MARKER_PREFIX: &str = "!codewhale-skill-state:";
const ENABLED_PREFIX: &str = "!codewhale-skill-state:1:enabled:";
const HISTORY_PREFIX: &str = "!codewhale-skill-state:1:history:";

#[derive(Debug, Clone)]
pub struct SkillStateStore {
    path: PathBuf,
    disabled: BTreeSet<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct OnDiskState {
    #[serde(default)]
    disabled: Vec<String>,
}

impl SkillStateStore {
    pub fn load_default() -> Result<Self> {
        let path = default_state_path()?;
        Self::load_from(path)
    }

    pub fn load_from(path: PathBuf) -> Result<Self> {
        let disabled = load_disabled(&path)?;
        Ok(Self { path, disabled })
    }

    pub fn is_enabled(&self, skill_name: &str) -> bool {
        self.is_enabled_with_legacy(skill_name, None)
    }

    /// Raw exact denies win even over a retained enable (including a later
    /// v0.10.0 toggle). Enables override only inherited legacy vetoes.
    pub fn is_enabled_with_legacy(&self, skill_name: &str, legacy_name: Option<&str>) -> bool {
        if self.disabled.contains(skill_name) {
            return false;
        }
        if self
            .disabled
            .contains(&format!("{ENABLED_PREFIX}{skill_name}"))
        {
            return true;
        }
        !self
            .disabled
            .contains(&format!("{HISTORY_PREFIX}{skill_name}"))
            && !legacy_name.is_some_and(|legacy| {
                self.disabled.contains(legacy)
                    || self.disabled.contains(&format!("{HISTORY_PREFIX}{legacy}"))
            })
    }

    pub fn set_enabled(&mut self, skill_name: &str, enabled: bool) -> Result<()> {
        self.set_enabled_with_persist(skill_name, enabled, persist_disabled)
    }

    /// Refresh the in-memory snapshot under the same shared lock used by
    /// other Codewhale processes. Long-running Runtime API servers call this
    /// before listing Skills so an external toggle becomes visible without a
    /// restart.
    pub fn refresh(&mut self) -> Result<()> {
        let disabled = load_disabled(&self.path)?;
        self.disabled = disabled;
        Ok(())
    }

    /// Raw exact denies only; inherited vetoes need discovered identity
    /// metadata and cannot be enumerated as a list of effective skill names.
    pub fn disabled(&self) -> Vec<String> {
        self.disabled
            .iter()
            .filter(|name| !name.starts_with(MARKER_PREFIX))
            .cloned()
            .collect()
    }

    fn set_enabled_with_persist(
        &mut self,
        skill_name: &str,
        enabled: bool,
        persist: impl FnOnce(&Path, &BTreeSet<String>) -> Result<()>,
    ) -> Result<()> {
        anyhow::ensure!(
            !skill_name.is_empty() && !skill_name.starts_with(MARKER_PREFIX),
            "invalid skill activation identity"
        );
        if let Some(parent) = self
            .path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)
                .with_context(|| format!("create parent dir for {}", self.path.display()))?;
        }
        let lock_path = state_lock_path(&self.path);
        let lock_file = open_state_lock(&lock_path, true)?;
        let mut lock = fd_lock::RwLock::new(lock_file);
        let _guard = lock
            .write()
            .with_context(|| format!("write-lock skill state at {}", self.path.display()))?;

        // Reload while holding the cross-process writer lock. Applying the
        // requested exact-name change to this latest snapshot merges updates
        // from other Runtime API/TUI processes instead of replacing them with
        // the caller's possibly stale in-memory view.
        let previous = load_disabled_unlocked(&self.path)?;
        let mut next = previous.clone();
        // A raw name may also govern an undiscovered legacy collision cohort.
        // Preserve every veto before removing any, without discovery writes.
        next.extend(
            previous
                .iter()
                .filter(|name| !name.is_empty() && !name.starts_with(MARKER_PREFIX))
                .map(|name| format!("{HISTORY_PREFIX}{name}")),
        );
        let exact_enable = format!("{ENABLED_PREFIX}{skill_name}");
        if enabled {
            next.remove(skill_name);
            next.insert(exact_enable);
        } else {
            next.insert(skill_name.to_string());
            next.insert(format!("{HISTORY_PREFIX}{skill_name}"));
            next.remove(&exact_enable);
        }
        if next != previous {
            // Disk is authoritative. Publish to memory only after the atomic
            // write succeeds so a failed persistence attempt cannot make this
            // process report a toggle that no other process can observe.
            persist(&self.path, &next)?;
        }
        self.disabled = next;
        Ok(())
    }
}

fn default_state_path() -> Result<PathBuf> {
    // Listing, prompt construction, and doctor are read-only. The explicit
    // mutation path creates the parent from `persist` when needed.
    Ok(codewhale_config::codewhale_home()
        .context("could not resolve Codewhale state directory")?
        .join(STATE_FILE_NAME))
}

fn load_disabled(path: &Path) -> Result<BTreeSet<String>> {
    let lock_path = state_lock_path(path);
    if path_entry_exists(&lock_path)? {
        let lock_file = open_state_lock(&lock_path, false)?;
        let lock = fd_lock::RwLock::new(lock_file);
        let _guard = lock
            .read()
            .with_context(|| format!("read-lock skill state at {}", path.display()))?;
        return load_disabled_unlocked(path);
    }
    load_disabled_unlocked(path)
}

fn load_disabled_unlocked(path: &Path) -> Result<BTreeSet<String>> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(BTreeSet::new());
        }
        Err(error) => {
            return Err(error).with_context(|| format!("read skill state at {}", path.display()));
        }
    };
    let parsed: OnDiskState =
        toml::from_str(&raw).with_context(|| format!("parse skill state at {}", path.display()))?;
    for entry in &parsed.disabled {
        if entry.starts_with(MARKER_PREFIX) {
            let identity = entry
                .strip_prefix(ENABLED_PREFIX)
                .or_else(|| entry.strip_prefix(HISTORY_PREFIX));
            anyhow::ensure!(
                identity.is_some_and(|name| !name.is_empty() && !name.starts_with(MARKER_PREFIX)),
                "parse skill state at {}: unsupported or malformed activation marker",
                path.display()
            );
        }
    }
    Ok(parsed.disabled.into_iter().collect())
}

fn persist_disabled(path: &Path, disabled: &BTreeSet<String>) -> Result<()> {
    let on_disk = OnDiskState {
        disabled: disabled.iter().cloned().collect(),
    };
    let body = toml::to_string_pretty(&on_disk).context("serialize skill state")?;
    codewhale_config::persistence::atomic_write(path, body.as_bytes())
        .with_context(|| format!("atomically persist skill state at {}", path.display()))
}

fn state_lock_path(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|name| name.to_os_string())
        .unwrap_or_else(|| STATE_FILE_NAME.into());
    name.push(".lock");
    path.with_file_name(name)
}

fn path_entry_exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).with_context(|| format!("inspect {}", path.display())),
    }
}

fn open_state_lock(path: &Path, create: bool) -> Result<fs::File> {
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(true)
        .create(create)
        .truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt as _;
        options.custom_flags(0x0020_0000); // FILE_FLAG_OPEN_REPARSE_POINT
    }
    let file = options
        .open(path)
        .with_context(|| format!("open skill state lock at {}", path.display()))?;
    validate_state_lock(path, &file)?;
    Ok(file)
}

#[cfg(unix)]
fn validate_state_lock(path: &Path, file: &fs::File) -> Result<()> {
    use std::os::unix::fs::MetadataExt as _;

    let metadata = file
        .metadata()
        .with_context(|| format!("inspect skill state lock at {}", path.display()))?;
    anyhow::ensure!(
        metadata.is_file() && metadata.nlink() == 1,
        "skill state lock at {} must be one regular, non-hard-linked file",
        path.display()
    );
    Ok(())
}

#[cfg(windows)]
fn validate_state_lock(path: &Path, file: &fs::File) -> Result<()> {
    use std::os::windows::fs::MetadataExt as _;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    let metadata = file
        .metadata()
        .with_context(|| format!("inspect skill state lock at {}", path.display()))?;
    anyhow::ensure!(
        metadata.is_file() && metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT == 0,
        "skill state lock at {} must be a regular, non-reparse file",
        path.display()
    );
    Ok(())
}

#[cfg(all(not(unix), not(windows)))]
fn validate_state_lock(path: &Path, file: &fs::File) -> Result<()> {
    anyhow::ensure!(
        file.metadata()
            .with_context(|| format!("inspect skill state lock at {}", path.display()))?
            .is_file(),
        "skill state lock at {} must be a regular file",
        path.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn fresh() -> (TempDir, SkillStateStore) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(STATE_FILE_NAME);
        let store = SkillStateStore::load_from(path).unwrap();
        (dir, store)
    }

    #[test]
    fn missing_file_defaults_to_everything_enabled() {
        let (_dir, store) = fresh();
        assert!(store.is_enabled("anything"));
        assert!(store.disabled().is_empty());
    }

    #[test]
    fn disable_then_reload_persists() {
        let (dir, mut store) = fresh();
        store.set_enabled("foo", false).unwrap();
        assert!(!store.is_enabled("foo"));

        let reloaded = SkillStateStore::load_from(dir.path().join(STATE_FILE_NAME)).unwrap();
        assert!(!reloaded.is_enabled("foo"));
        assert!(reloaded.is_enabled("bar"));
    }

    #[test]
    fn enable_removes_from_disabled_list() {
        let (_dir, mut store) = fresh();
        store.set_enabled("foo", false).unwrap();
        store.set_enabled("foo", true).unwrap();
        assert!(store.is_enabled("foo"));
        assert!(store.disabled().is_empty());
    }

    #[test]
    fn explicit_enable_records_a_choice_then_repeated_toggle_is_noop() {
        let (dir, mut store) = fresh();
        store.set_enabled("foo", true).unwrap();
        assert!(store.disabled().is_empty());
        let path = dir.path().join(STATE_FILE_NAME);
        let before = fs::read(&path).unwrap();
        store
            .set_enabled_with_persist("foo", true, |_, _| {
                panic!("repeating the same choice must not rewrite the store")
            })
            .unwrap();
        assert_eq!(fs::read(path).unwrap(), before);
    }

    // The released v0.10.0 serde shape and exact-set writer semantics at
    // 1be1a703b975fc0a6c125886c761141341615a32. This intentionally does not
    // interpret markers or use the upgraded loader/writer.
    fn released_writer_toggle(path: &Path, name: &str, enabled: bool) -> bool {
        #[derive(Deserialize, Serialize)]
        struct ReleasedState {
            #[serde(default)]
            disabled: Vec<String>,
        }
        let old: ReleasedState = toml::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        let mut next: BTreeSet<String> = old.disabled.into_iter().collect();
        let changed = if enabled {
            next.remove(name)
        } else {
            next.insert(name.to_string())
        };
        if changed {
            fs::write(
                path,
                toml::to_string_pretty(&ReleasedState {
                    disabled: next.into_iter().collect(),
                })
                .unwrap(),
            )
            .unwrap();
        }
        changed
    }

    #[test]
    fn legacy_veto_and_exact_choices_survive_released_writer_serialization() {
        for legacy in ["skill", "pdf", "demo:skill"] {
            let (dir, mut store) = fresh();
            let path = dir.path().join(STATE_FILE_NAME);
            let original = format!("disabled = [\"{legacy}\"]\n");
            fs::write(&path, &original).unwrap();
            store.refresh().unwrap();
            let a = format!("{legacy}-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
            let b = format!("{legacy}-bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
            assert!(!store.is_enabled_with_legacy(&a, Some(legacy)));
            assert!(!store.is_enabled_with_legacy(&b, Some(legacy)));
            assert_eq!(
                fs::read_to_string(&path).unwrap(),
                original,
                "reads never migrate"
            );

            store.set_enabled(&a, true).unwrap();
            assert!(released_writer_toggle(&path, "unrelated", false));
            store.refresh().unwrap();
            assert!(store.is_enabled_with_legacy(&a, Some(legacy)));
            assert!(!store.is_enabled_with_legacy(&b, Some(legacy)));
            // Old serialization cannot erase the history when removing L.
            assert!(released_writer_toggle(&path, legacy, true));
            store.refresh().unwrap();
            assert!(!store.is_enabled_with_legacy(&b, Some(legacy)));
            assert!(!store.is_enabled(legacy));

            store.set_enabled(legacy, true).unwrap();
            assert!(
                store.is_enabled(legacy),
                "literal ASCII identity has its own exception"
            );
            assert!(!store.is_enabled_with_legacy(&b, Some(legacy)));
            assert!(released_writer_toggle(&path, legacy, false));
            store.refresh().unwrap();
            assert!(
                !store.is_enabled(legacy),
                "later raw exact disable beats an exception"
            );
            // The old lossy/no-op toggle cannot express a new per-skill
            // revocation. Preserve this limit rather than claim parity.
            assert!(!released_writer_toggle(&path, legacy, false));
            store.refresh().unwrap();
            assert!(store.is_enabled_with_legacy(&a, Some(legacy)));

            store.set_enabled(&a, false).unwrap();
            released_writer_toggle(&path, legacy, true);
            store.refresh().unwrap();
            assert!(!store.is_enabled_with_legacy(&a, Some(legacy)));
            assert!(!store.is_enabled_with_legacy(&b, Some(legacy)));
        }
    }

    #[test]
    fn malformed_or_unknown_activation_markers_preserve_bytes_and_memory() {
        let (dir, mut store) = fresh();
        let path = dir.path().join(STATE_FILE_NAME);
        store.set_enabled("kept", false).unwrap();
        for marker in [
            "!codewhale-skill-state:1:enabled:",
            "!codewhale-skill-state:2:enabled:kept",
            "!codewhale-skill-state:1:unknown:kept",
            "!codewhale-skill-state:1:history:!codewhale-skill-state:1:enabled:kept",
        ] {
            let bytes = format!("disabled = [\"{marker}\"]\n");
            fs::write(&path, &bytes).unwrap();
            assert!(store.refresh().is_err());
            assert!(store.set_enabled("kept", true).is_err());
            assert!(!store.is_enabled("kept"));
            assert_eq!(fs::read_to_string(&path).unwrap(), bytes);
        }
        fs::write(&path, "disabled = [\"unrelated opaque entry\"]\n").unwrap();
        store.refresh().unwrap();
        store.set_enabled("foo", false).unwrap();
        assert!(!store.is_enabled("unrelated opaque entry"));
    }

    #[test]
    fn failed_exact_enable_does_not_publish_or_replace_legacy_policy() {
        let (dir, mut store) = fresh();
        let path = dir.path().join(STATE_FILE_NAME);
        let original = b"disabled = [\"skill\"]\n";
        fs::write(&path, original).unwrap();
        store.refresh().unwrap();
        let name = "skill-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let result = store.set_enabled_with_persist(name, true, |_, _| {
            anyhow::bail!("injected persistence failure")
        });
        assert!(result.is_err());
        assert!(!store.is_enabled_with_legacy(name, Some("skill")));
        assert_eq!(fs::read(path).unwrap(), original);
    }

    #[test]
    fn malformed_file_fails_closed() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(STATE_FILE_NAME);
        fs::write(&path, b"this is not toml = { broken").unwrap();
        let error = SkillStateStore::load_from(path.clone()).unwrap_err();
        assert!(error.to_string().contains("parse skill state"));
        assert_eq!(
            fs::read(&path).unwrap(),
            b"this is not toml = { broken",
            "a malformed authority file must remain untouched for recovery"
        );
    }

    #[test]
    fn disabled_list_is_deterministic_order() {
        let (_dir, mut store) = fresh();
        store.set_enabled("zeta", false).unwrap();
        store.set_enabled("alpha", false).unwrap();
        store.set_enabled("mu", false).unwrap();
        assert_eq!(
            store.disabled(),
            vec!["alpha".to_string(), "mu".to_string(), "zeta".to_string()]
        );
    }

    #[test]
    fn stale_stores_merge_independent_toggles() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(STATE_FILE_NAME);
        let mut first = SkillStateStore::load_from(path.clone()).unwrap();
        let mut second = SkillStateStore::load_from(path.clone()).unwrap();

        first.set_enabled("alpha", false).unwrap();
        second.set_enabled("beta", false).unwrap();

        let persisted = SkillStateStore::load_from(path).unwrap();
        assert!(!persisted.is_enabled("alpha"));
        assert!(!persisted.is_enabled("beta"));
    }

    #[test]
    fn stale_enable_request_reloads_before_noop_decision() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(STATE_FILE_NAME);
        let mut disabler = SkillStateStore::load_from(path.clone()).unwrap();
        let mut stale_enabler = SkillStateStore::load_from(path.clone()).unwrap();

        disabler.set_enabled("alpha", false).unwrap();
        stale_enabler.set_enabled("alpha", true).unwrap();

        assert!(stale_enabler.is_enabled("alpha"));
        assert!(
            SkillStateStore::load_from(path)
                .unwrap()
                .is_enabled("alpha")
        );
    }

    #[test]
    fn refresh_observes_external_process_snapshot() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(STATE_FILE_NAME);
        let mut writer = SkillStateStore::load_from(path.clone()).unwrap();
        let mut reader = SkillStateStore::load_from(path).unwrap();

        writer.set_enabled("alpha", false).unwrap();
        assert!(reader.is_enabled("alpha"));
        reader.refresh().unwrap();
        assert!(!reader.is_enabled("alpha"));
    }

    #[test]
    fn failed_persist_does_not_advance_in_memory_state() {
        let (_dir, mut store) = fresh();
        store.set_enabled("alpha", false).unwrap();

        let error = store
            .set_enabled_with_persist("beta", false, |_, _| {
                anyhow::bail!("injected persistence failure")
            })
            .unwrap_err();

        assert!(error.to_string().contains("injected persistence failure"));
        assert!(!store.is_enabled("alpha"));
        assert!(store.is_enabled("beta"));
    }

    #[test]
    fn cross_process_toggles_serialize_and_merge() {
        const CHILD_PATH: &str = "CODEWHALE_TEST_SKILL_STATE_PATH";
        const CHILD_NAME: &str = "CODEWHALE_TEST_SKILL_STATE_NAME";
        const TEST_NAME: &str = "skill_state::tests::cross_process_toggles_serialize_and_merge";

        if let (Some(path), Some(name)) =
            (std::env::var_os(CHILD_PATH), std::env::var_os(CHILD_NAME))
        {
            let path = PathBuf::from(path);
            let name = name.to_string_lossy().into_owned();
            let mut store = SkillStateStore::load_from(path.clone()).unwrap();
            fs::write(path.with_file_name(format!("{name}.ready")), b"ready").unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while ["alpha", "beta"]
                .iter()
                .any(|peer| !path.with_file_name(format!("{peer}.ready")).exists())
            {
                assert!(
                    std::time::Instant::now() < deadline,
                    "peer skill-state process did not reach the mutation barrier"
                );
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            store.set_enabled(&name, false).unwrap();
            return;
        }

        use std::process::{Command, Stdio};
        use wait_timeout::ChildExt as _;

        let dir = TempDir::new().unwrap();
        let path = dir.path().join(STATE_FILE_NAME);
        let executable = std::env::current_exe().expect("current test executable");
        let mut children = ["alpha", "beta"].map(|name| {
            Command::new(&executable)
                .args(["--exact", TEST_NAME, "--nocapture", "--test-threads=1"])
                .env(CHILD_PATH, &path)
                .env(CHILD_NAME, name)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("spawn isolated skill-state writer")
        });
        for child in &mut children {
            let status = match child
                .wait_timeout(std::time::Duration::from_secs(15))
                .expect("wait for isolated skill-state writer")
            {
                Some(status) => status,
                None => {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("isolated skill-state writer timed out");
                }
            };
            assert!(status.success(), "isolated skill-state writer failed");
        }

        let persisted = SkillStateStore::load_from(path).unwrap();
        assert!(!persisted.is_enabled("alpha"));
        assert!(!persisted.is_enabled("beta"));
    }
}
