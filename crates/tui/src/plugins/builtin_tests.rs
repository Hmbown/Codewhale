//! Filesystem-only publication regressions. No plugin or native helper executes.

use super::*;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

const NAME: &str = "fixture";
const FIRST: &[(&str, &[u8])] = &[
    ("plugin.json", br#"{"$schema":"https://agent-plugins.org/schemas/plugin.json","name":"fixture","version":"1.0.0"}"#),
    ("nested/body.txt", b"first bundle"),
];
const SECOND: &[(&str, &[u8])] = &[
    ("plugin.json", br#"{"$schema":"https://agent-plugins.org/schemas/plugin.json","name":"fixture","version":"1.0.0"}"#),
    ("nested/body.txt", b"other bundle"),
];

fn assert_contents(root: &Path, files: &[(&str, &[u8])]) {
    for (path, contents) in files {
        assert_eq!(fs::read(root.join(NAME).join(path)).unwrap(), *contents);
    }
    assert_eq!(
        fs::read(root.join(STAMP_NAME)).unwrap(),
        digest(files).as_bytes()
    );
}

#[test]
fn concurrent_first_startups_share_one_complete_embedded_snapshot() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home");
    fs::create_dir(&home).unwrap();
    let barrier = Arc::new(Barrier::new(4));
    let workers: Vec<_> = (0..4)
        .map(|_| {
            let home = home.clone();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                materialize_at_home(&home).unwrap().unwrap()
            })
        })
        .collect();
    let roots: BTreeSet<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    assert_eq!(roots.len(), 1);
    assert_eq!(
        fs::read_dir(home.join(BUILTIN_DIR_NAME).join(SNAPSHOTS_DIR_NAME))
            .unwrap()
            .count(),
        1
    );
    let root = roots.into_iter().next().unwrap();
    for (path, contents) in COMPUTER_USE_FILES {
        assert_eq!(
            fs::read(root.join(COMPUTER_USE).join(path)).unwrap(),
            *contents
        );
    }
    assert!(!home.join("plugins/state.json").exists());
}

#[test]
fn concurrent_threads_keep_both_versions_complete() {
    let temp = tempfile::tempdir().unwrap();
    let barrier = Arc::new(Barrier::new(12));
    let workers: Vec<_> = (0..12)
        .map(|index| {
            let root = temp.path().to_path_buf();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                let files = if index % 2 == 0 { FIRST } else { SECOND };
                barrier.wait();
                let published = write_bundle(&root, NAME, files).unwrap();
                for _ in 0..8 {
                    assert_contents(&published, files);
                    assert_eq!(write_bundle(&root, NAME, files).unwrap(), published);
                }
                published
            })
        })
        .collect();
    let roots: BTreeSet<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    assert_eq!(roots.len(), 2);
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 2);
}

// Kill/wait even if a parent assertion fails: fixtures must not leave processes.
struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn spawn_child(root: &Path, index: usize, orphan: bool) -> ChildGuard {
    let test_name = format!(
        "{}::publication_child",
        module_path!().split_once("::").unwrap().1
    );
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", &test_name, "--ignored", "--nocapture"])
        .env_clear()
        .env("HOME", root)
        .env("USERPROFILE", root)
        .env("CODEWHALE_HOME", root.join("absent-home"))
        .env("BUILTIN_TEST_ROOT", root)
        .env("BUILTIN_TEST_INDEX", index.to_string())
        .env("BUILTIN_TEST_ORPHAN", if orphan { "yes" } else { "no" })
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());
    #[cfg(windows)]
    if let Some(system_root) = std::env::var_os("SystemRoot") {
        command.env("SystemRoot", system_root);
    }
    ChildGuard(command.spawn().unwrap())
}

fn await_file(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !path.exists() {
        assert!(
            Instant::now() < deadline,
            "child did not reach {}",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
#[ignore = "subprocess entry point, exercised by the two process tests"]
fn publication_child() {
    let root = PathBuf::from(std::env::var_os("BUILTIN_TEST_ROOT").unwrap());
    let index: usize = std::env::var("BUILTIN_TEST_INDEX")
        .unwrap()
        .parse()
        .unwrap();
    if std::env::var("BUILTIN_TEST_ORPHAN").unwrap() == "yes" {
        let stage = tempfile::Builder::new()
            .prefix(".staging-fixture-")
            .tempdir_in(root.join("cache"))
            .unwrap();
        fs::write(stage.path().join("partial"), b"interrupted writer").unwrap();
        fs::write(root.join("orphan-path"), stage.path().to_str().unwrap()).unwrap();
        fs::write(root.join("ready-0"), b"ready").unwrap();
        // The parent kills this writer before it can publish or run TempDir cleanup.
        std::thread::sleep(Duration::from_secs(30));
        panic!("parent did not terminate interrupted publisher");
    }
    fs::write(root.join(format!("ready-{index}")), b"ready").unwrap();
    await_file(&root.join("start"));
    let files = if index.is_multiple_of(2) {
        FIRST
    } else {
        SECOND
    };
    for _ in 0..8 {
        let published = write_bundle(&root.join("cache"), NAME, files).unwrap();
        assert_contents(&published, files);
    }
    fs::write(root.join(format!("done-{index}")), b"done").unwrap();
}

#[test]
fn concurrent_processes_converge_without_replacing_other_versions() {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir(temp.path().join("cache")).unwrap();
    let mut children: Vec<_> = (0..4)
        .map(|index| spawn_child(temp.path(), index, false))
        .collect();
    for index in 0..4 {
        await_file(&temp.path().join(format!("ready-{index}")));
    }
    fs::write(temp.path().join("start"), b"go").unwrap();
    for (index, child) in children.iter_mut().enumerate() {
        await_file(&temp.path().join(format!("done-{index}")));
        assert!(child.0.wait().unwrap().success());
    }
    let cache = temp.path().join("cache");
    assert_eq!(fs::read_dir(&cache).unwrap().count(), 2);
    for files in [FIRST, SECOND] {
        assert_contents(&write_bundle(&cache, NAME, files).unwrap(), files);
    }
    assert!(!temp.path().join("absent-home").exists());
}

#[test]
fn killed_writer_leaves_only_its_unexposed_stage() {
    let temp = tempfile::tempdir().unwrap();
    let cache = temp.path().join("cache");
    fs::create_dir(&cache).unwrap();
    let mut child = spawn_child(temp.path(), 0, true);
    await_file(&temp.path().join("ready-0"));
    child.0.kill().unwrap();
    child.0.wait().unwrap();
    let orphan = PathBuf::from(fs::read_to_string(temp.path().join("orphan-path")).unwrap());
    let published = write_bundle(&cache, NAME, FIRST).unwrap();
    assert_contents(&published, FIRST);
    assert_eq!(
        fs::read(orphan.join("partial")).unwrap(),
        b"interrupted writer"
    );
    assert_eq!(fs::read_dir(&cache).unwrap().count(), 2);
    assert_ne!(published, orphan);
}

#[test]
fn exclusive_publication_preserves_even_an_empty_destination() {
    let temp = tempfile::tempdir().unwrap();
    let stage = temp.path().join("stage");
    let destination = temp.path().join("destination");
    fs::create_dir(&stage).unwrap();
    fs::write(stage.join("complete"), b"complete").unwrap();
    fs::create_dir(&destination).unwrap();
    assert_eq!(
        publish_snapshot(&stage, &destination).unwrap_err().kind(),
        io::ErrorKind::AlreadyExists
    );
    assert_eq!(fs::read_dir(destination).unwrap().count(), 0);
    assert!(stage.join("complete").is_file());
}

#[test]
fn partial_published_destination_is_never_repaired_or_replaced() {
    let temp = tempfile::tempdir().unwrap();
    let destination = temp.path().join(format!("{NAME}-{}", digest(FIRST)));
    fs::create_dir(&destination).unwrap();
    fs::write(destination.join(STAMP_NAME), digest(FIRST)).unwrap();
    assert!(write_bundle(temp.path(), NAME, FIRST).is_err());
    assert_eq!(fs::read_dir(destination).unwrap().count(), 1);
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
}

#[test]
fn matching_stamp_does_not_bless_changed_missing_or_extra_content() {
    for mutation in ["bytes", "missing", "extra-file", "extra-directory", "stamp"] {
        let temp = tempfile::tempdir().unwrap();
        let published = write_bundle(temp.path(), NAME, FIRST).unwrap();
        let body = published.join(NAME).join("nested/body.txt");
        match mutation {
            "bytes" => fs::write(&body, b"other bundle").unwrap(), // Same length.
            "missing" => fs::remove_file(&body).unwrap(),
            "extra-file" => fs::write(published.join(NAME).join("extra"), b"extra").unwrap(),
            "extra-directory" => fs::create_dir(published.join(NAME).join("extra")).unwrap(),
            "stamp" => fs::write(published.join(STAMP_NAME), digest(SECOND)).unwrap(),
            _ => unreachable!(),
        }
        assert!(
            write_bundle(temp.path(), NAME, FIRST).is_err(),
            "{mutation}"
        );
        assert!(published.exists());
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
        if mutation == "bytes" {
            assert_eq!(fs::read(&body).unwrap(), b"other bundle");
        }
    }
}

#[cfg(unix)]
#[test]
fn executable_mode_changes_and_hardlinks_fail_closed() {
    use std::os::unix::fs::PermissionsExt as _;
    for mode in [0o644, 0o700] {
        let temp = tempfile::tempdir().unwrap();
        let files: &[(&str, &[u8])] = &[("bin/darwin/accessibility", b"fixture only")];
        let published = write_bundle(temp.path(), NAME, files).unwrap();
        let file = published.join(NAME).join("bin/darwin/accessibility");
        if mode == 0o644 {
            fs::set_permissions(&file, fs::Permissions::from_mode(mode)).unwrap();
        } else {
            fs::hard_link(&file, temp.path().join("linked-helper")).unwrap();
        }
        assert!(write_bundle(temp.path(), NAME, files).is_err());
    }
    let temp = tempfile::tempdir().unwrap();
    let published = write_bundle(temp.path(), NAME, FIRST).unwrap();
    fs::set_permissions(
        published.join(NAME).join("plugin.json"),
        fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    assert!(write_bundle(temp.path(), NAME, FIRST).is_err());
}

#[cfg(unix)]
#[test]
fn linked_cache_snapshot_directory_and_file_are_rejected() {
    use std::os::unix::fs::symlink;
    for level in ["cache", "snapshot", "directory", "file"] {
        let temp = tempfile::tempdir().unwrap();
        let cache = temp.path().join("cache");
        fs::create_dir(&cache).unwrap();
        let published = write_bundle(&cache, NAME, FIRST).unwrap();
        let original = match level {
            "cache" => cache.clone(),
            "snapshot" => published.clone(),
            "directory" => published.join(NAME).join("nested"),
            "file" => published.join(NAME).join("nested/body.txt"),
            _ => unreachable!(),
        };
        let moved = temp.path().join("moved");
        fs::rename(&original, &moved).unwrap();
        symlink(&moved, &original).unwrap();
        assert!(write_bundle(&cache, NAME, FIRST).is_err(), "{level}");
        assert!(
            fs::symlink_metadata(&original)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(moved.exists());
    }
}

#[cfg(unix)]
#[test]
fn linked_home_pins_its_selected_directory_without_creating_missing_targets() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().unwrap();
    let first_home = temp.path().join("first-home");
    let second_home = temp.path().join("second-home");
    let selected = temp.path().join("selected-home");
    fs::create_dir(&first_home).unwrap();
    fs::create_dir(&second_home).unwrap();
    symlink(&first_home, &selected).unwrap();

    let first = materialize_at_home(&selected).unwrap().unwrap();
    assert!(first.starts_with(first_home.canonicalize().unwrap()));
    assert_eq!(materialize_at_home(&first_home).unwrap().unwrap(), first);
    let manifest = first.join(COMPUTER_USE).join("plugin.json");
    let original = fs::read(&manifest).unwrap();

    // Retargeting the user's alias cannot redirect an already captured root.
    fs::remove_file(&selected).unwrap();
    symlink(&second_home, &selected).unwrap();
    let second = materialize_at_home(&selected).unwrap().unwrap();
    assert!(second.starts_with(second_home.canonicalize().unwrap()));
    assert_ne!(second, first);
    assert_eq!(fs::read(&manifest).unwrap(), original);
    assert_eq!(materialize_at_home(&first_home).unwrap().unwrap(), first);

    fs::remove_file(&selected).unwrap();
    let missing = temp.path().join("missing-home");
    symlink(&missing, &selected).unwrap();
    assert!(materialize_at_home(&selected).unwrap().is_none());
    assert!(!missing.exists());
}

#[cfg(unix)]
#[test]
fn linked_builtin_descendants_of_an_aliased_home_are_rejected() {
    use std::os::unix::fs::symlink;
    for level in ["builtin", "snapshots"] {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let selected = temp.path().join("selected-home");
        let target = temp.path().join("target");
        fs::create_dir(&target).unwrap();
        let linked = match level {
            "builtin" => {
                fs::create_dir(&home).unwrap();
                home.join(BUILTIN_DIR_NAME)
            }
            "snapshots" => {
                fs::create_dir_all(home.join(BUILTIN_DIR_NAME)).unwrap();
                home.join(BUILTIN_DIR_NAME).join(SNAPSHOTS_DIR_NAME)
            }
            _ => unreachable!(),
        };
        symlink(&home, &selected).unwrap();
        symlink(&target, &linked).unwrap();
        assert!(materialize_at_home(&selected).is_err(), "{level}");
        assert_eq!(fs::read_dir(target).unwrap().count(), 0);
    }
}

#[cfg(windows)]
#[test]
fn junction_cache_snapshot_and_nested_directory_are_rejected() {
    for level in ["cache", "snapshot", "directory"] {
        let temp = tempfile::tempdir().unwrap();
        let cache = temp.path().join("cache");
        fs::create_dir(&cache).unwrap();
        let published = write_bundle(&cache, NAME, FIRST).unwrap();
        let original = match level {
            "cache" => cache.clone(),
            "snapshot" => published.clone(),
            "directory" => published.join(NAME).join("nested"),
            _ => unreachable!(),
        };
        let moved = temp.path().join("moved");
        fs::rename(&original, &moved).unwrap();
        let result = Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&original)
            .arg(&moved)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(write_bundle(&cache, NAME, FIRST).is_err(), "{level}");
        assert!(metadata_is_link_or_reparse(
            &fs::symlink_metadata(&original).unwrap()
        ));
        assert!(moved.exists());
    }
}

#[test]
fn embedded_paths_cannot_escape_or_alias_the_inventory() {
    for path in ["../outside", "/absolute", "", "./dot"] {
        let temp = tempfile::tempdir().unwrap();
        assert!(
            write_bundle(temp.path(), NAME, &[(path, b"bad")]).is_err(),
            "{path}"
        );
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
    }
    let temp = tempfile::tempdir().unwrap();
    assert!(write_bundle(temp.path(), NAME, &[("same", b"a"), ("same", b"b")]).is_err());
    assert!(write_bundle(temp.path(), "../outside", FIRST).is_err());
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
}

#[test]
fn materialization_preserves_legacy_tree_receipts_and_missing_home() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home");
    assert!(materialize_at_home(&home).unwrap().is_none());
    assert!(!home.exists());
    let legacy = home.join(BUILTIN_DIR_NAME).join(COMPUTER_USE);
    fs::create_dir_all(&legacy).unwrap();
    fs::write(legacy.join("legacy"), b"old live bundle").unwrap();
    fs::create_dir(home.join("plugins")).unwrap();
    let state = home.join("plugins/state.json");
    fs::write(&state, b"existing receipts").unwrap();
    let published = materialize_at_home(&home).unwrap().unwrap();
    let stamp_time = fs::metadata(published.join(STAMP_NAME))
        .unwrap()
        .modified()
        .unwrap();
    let stamp_bytes = fs::read(published.join(STAMP_NAME)).unwrap();
    assert_eq!(materialize_at_home(&home).unwrap().unwrap(), published);
    // Reuse re-stamps the snapshot's mtime (plugin-state GC reads it as "last
    // started") and nothing else: the stamp's bytes are the digest, untouched.
    assert!(
        fs::metadata(published.join(STAMP_NAME))
            .unwrap()
            .modified()
            .unwrap()
            >= stamp_time
    );
    assert_eq!(fs::read(published.join(STAMP_NAME)).unwrap(), stamp_bytes);
    assert!(published.join(COMPUTER_USE).join("plugin.json").is_file());
    assert_eq!(fs::read(legacy.join("legacy")).unwrap(), b"old live bundle");
    assert_eq!(fs::read(state).unwrap(), b"existing receipts");
}
