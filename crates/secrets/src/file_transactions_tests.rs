use super::*;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn transaction_child() {
    let Some(path) = std::env::var_os("CW_CORE_STORE_CHILD") else {
        return;
    };
    let path = PathBuf::from(path);
    fs::write(path.with_extension("ready"), b"ready").unwrap();
    let store = FileKeyringStore::new(&path);
    match std::env::var("CW_CORE_STORE_OPERATION").unwrap().as_str() {
        "set" => store.set("child", "fixture-child").unwrap(),
        "delete" => store.delete("delete-me").unwrap(),
        "migrate" => {
            FileKeyringStore::migrate_legacy_file_if_needed(&path, &path.with_extension("legacy"))
                .unwrap()
        }
        _ => unreachable!(),
    }
}

#[test]
fn file_transactions_serialize_process_set_delete_and_legacy_migration() {
    for operation in ["set", "delete", "migrate"] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secrets.json");
        let store = FileKeyringStore::new(&path);
        store.set("delete-me", "fixture").unwrap();
        FileKeyringStore::new(path.with_extension("legacy"))
            .set("migrated", "fixture")
            .unwrap();
        let mut child = file_lock::with_write_lock(&path, |path| {
            let mut child = Command::new(std::env::current_exe()?)
                .args([
                    "--exact",
                    "file_transactions_tests::transaction_child",
                    "--nocapture",
                ])
                .env("CW_CORE_STORE_CHILD", path)
                .env("CW_CORE_STORE_OPERATION", operation)
                .stdout(Stdio::null())
                .spawn()?;
            let deadline = Instant::now() + Duration::from_secs(5);
            while !path.with_extension("ready").exists() {
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(5));
            }
            std::thread::sleep(Duration::from_millis(100));
            assert!(child.try_wait()?.is_none(), "writer bypassed shared lock");
            let store = FileKeyringStore::new(path);
            let mut blob = store.load_unlocked()?;
            blob.entries
                .insert("parent".into(), "fixture-parent".into());
            blob.extra
                .insert("metadata".into(), serde_json::json!({"future":true}));
            store.store_unlocked(&blob)?;
            Ok(child)
        })
        .unwrap();
        assert!(child.wait().unwrap().success());
        let blob = store.load_unlocked().unwrap();
        assert_eq!(blob.entries["parent"], "fixture-parent");
        assert_eq!(blob.extra["metadata"]["future"], true);
        match operation {
            "set" => assert_eq!(blob.entries["child"], "fixture-child"),
            "delete" => assert!(!blob.entries.contains_key("delete-me")),
            "migrate" => assert_eq!(blob.entries["migrated"], "fixture"),
            _ => unreachable!(),
        }
    }
}

#[test]
fn file_transaction_failure_does_not_commit_and_releases_lock() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("secrets.json");
    let store = FileKeyringStore::new(&path);
    store.set("keep", "fixture").unwrap();
    let before = fs::read(&path).unwrap();
    assert!(
        store
            .mutate::<()>(|blob| {
                blob.entries.clear();
                Err(std::io::Error::other("fixture failure").into())
            })
            .is_err()
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    store.set("next", "fixture").unwrap();
}

#[cfg(unix)]
#[test]
fn file_transaction_rejects_symlink_store_and_lock_without_touching_targets() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("secrets.json");
    let target = dir.path().join("target");
    fs::write(&target, b"keep").unwrap();
    symlink(&target, &path).unwrap();
    let store = FileKeyringStore::new(&path);
    assert!(store.set("key", "fixture").is_err());
    assert!(store.get("key").is_err());
    fs::remove_file(&path).unwrap();
    fs::remove_file(path.with_extension("json.lock")).unwrap();
    symlink(&target, path.with_extension("json.lock")).unwrap();
    assert!(store.set("key", "fixture").is_err());
    assert_eq!(fs::read(target).unwrap(), b"keep");
}

#[test]
fn account_companion_survives_refresh_but_not_account_switch_or_logout() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("secrets.json");
    let store = FileKeyringStore::new(&path);
    let slot = account::account_auth_slot("default", account::DEFAULT_ACCOUNT_API_BASE);
    let companion = slot.replace("-auth-", "-device-");
    let mut bundle = serde_json::json!({"schemaVersion":1,"apiBase":account::DEFAULT_ACCOUNT_API_BASE,"bundle":{"tokenType":"Bearer","accessToken":"fixture-a","refreshToken":"fixture-r","user":{"id":"account"},"session":{"id":"session"}}});
    store.set(&slot, &bundle.to_string()).unwrap();
    store.set(&companion, "opaque-fixture").unwrap();
    bundle["bundle"]["accessToken"] = "fixture-b".into();
    store.set(&slot, &bundle.to_string()).unwrap();
    assert!(store.get(&companion).unwrap().is_some());
    bundle["bundle"]["session"]["id"] = "different".into();
    store.set(&slot, &bundle.to_string()).unwrap();
    assert!(store.get(&companion).unwrap().is_none());
    store.set(&companion, "opaque-fixture").unwrap();
    store.delete(&slot).unwrap();
    assert!(store.get(&companion).unwrap().is_none());
}
