//! Real Windows kernel/host receipts. These are not source-only substitutes.
use super::*;
use crate::config::ExtensionHostRuntime;
use crate::extension_host::supervisor::{HostSandbox, MemoryEnforcement};
use crate::extension_host::tests::{FixturePlugins, host_tool};
use crate::extension_host::{ExtensionHostManager, ExtensionHostOptions, HostStatus};
use crate::plugins::activation::TestPolicyGuard;
use crate::tools::spec::ToolContext;
use serde_json::json;

fn runtime(choice: ExtensionHostRuntime, override_path: Option<&Path>) -> Option<HostRuntime> {
    let resolution = crate::dependencies::resolve_extension_host_runtime(
        choice,
        (choice == ExtensionHostRuntime::Node)
            .then_some(override_path)
            .flatten(),
        (choice == ExtensionHostRuntime::Bun)
            .then_some(override_path)
            .flatten(),
    );
    match resolution.selected {
        Some(runtime) => Some(runtime),
        None if std::env::var_os("CODEWHALE_EXT_HOST_TESTS").is_some()
            || std::env::var_os("CODEWHALE_EXT_HOST_BUN_TESTS").is_some() =>
        {
            panic!("required Windows runtime: {}", resolution.failure())
        }
        None => {
            eprintln!(
                "Windows runtime receipt unavailable: {}",
                resolution.failure()
            );
            None
        }
    }
}

#[cfg(test)]
async fn owner_boundary(runtime: HostRuntime) {
    let _policy = TestPolicyGuard::extension_host(true);
    let fixture = FixturePlugins::new(&["secret-probe"]).await;
    let choice = match runtime.kind {
        crate::dependencies::HostRuntimeKind::Node => ExtensionHostRuntime::Node,
        crate::dependencies::HostRuntimeKind::Bun => ExtensionHostRuntime::Bun,
    };
    let registry = fixture.registry();
    let plugin = registry.get("secret-probe").unwrap();
    let source = plugin.base_path.join("index.mjs");
    let staged = plugin.staged_root.as_ref().unwrap().join("index.mjs");
    assert_ne!(source, staged);
    let manager = Arc::new(ExtensionHostManager::new(ExtensionHostOptions {
        runtime: choice,
        node_override: (choice == ExtensionHostRuntime::Node).then_some(runtime.path.clone()),
        bun_override: (choice == ExtensionHostRuntime::Bun).then_some(runtime.path),
        root: Some(fixture.root.clone()),
        ..Default::default()
    }));
    let engine = manager.attach(registry);
    engine.sync().await.unwrap();
    let HostStatus::Ready {
        sandbox, memory, ..
    } = manager.status()
    else {
        panic!(
            "Windows Native failed: {:?}; {:?}",
            manager.status(),
            manager.diagnostics()
        );
    };
    assert_eq!(sandbox, HostSandbox::Wrapped("windows-lpac".into()));
    assert_eq!(memory, MemoryEnforcement::JobObject);
    let context = ToolContext::new(fixture.workspace()).with_plugin_registry(engine.plugin_view());
    let read = host_tool(&engine, fixture.workspace(), "probe_read");
    let write = host_tool(&engine, fixture.workspace(), "probe_write");
    let staged_read = read
        .execute(json!({"path":staged}), &context)
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&staged_read.content).unwrap()["ok"],
        true,
        "reviewed runtime root must be readable by the real Native module"
    );
    for path in [
        source,
        fixture.workspace().join("ungranted.txt"),
        fixture
            .root
            .join("extension-host/builtin-data/private.json"),
    ] {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        if !path.exists() {
            std::fs::write(&path, b"synthetic-private-control").unwrap();
        }
        let result = read.execute(json!({"path":path}), &context).await.unwrap();
        let result: serde_json::Value = serde_json::from_str(&result.content).unwrap();
        assert_eq!(
            result["ok"], false,
            "mutable/ungranted roots must remain unreadable"
        );
        assert!(
            matches!(result["code"].as_str(), Some("EACCES" | "EPERM")),
            "must be an access denial, not a missing file"
        );
    }
    let data = fixture.root.join("extension-host/data/positive-write.txt");
    let result = write.execute(json!({"path":data}), &context).await.unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&result.content).unwrap()["ok"],
        true
    );
    let denied = staged.parent().unwrap().join("forbidden-write.txt");
    let result = write
        .execute(json!({"path":denied}), &context)
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&result.content).unwrap()["ok"],
        false
    );
    assert!(!denied.exists(), "readonly snapshot was modified");
    engine.set_plugins(fixture.disable("secret-probe"));
    engine.sync().await.unwrap();
    assert!(
        read.execute(json!({"path":staged}), &context)
            .await
            .is_err(),
        "revoked caller cannot use an earlier handle"
    );
    manager.shutdown().await;
}

#[tokio::test(flavor = "current_thread")]
async fn windows_native_lpac_node_owner_boundary() {
    if let Some(runtime) = runtime(ExtensionHostRuntime::Node, None) {
        owner_boundary(runtime).await;
    }
}

#[tokio::test(flavor = "current_thread")]
async fn windows_native_lpac_bun_owner_boundary() {
    if let Some(runtime) = runtime(ExtensionHostRuntime::Bun, None) {
        owner_boundary(runtime).await;
    }
}

#[tokio::test(flavor = "current_thread")]
async fn windows_native_lpac_compiled_owner_boundary() {
    let Some(binary) = std::env::var_os("CODEWHALE_COMPILED_HOST_TEST_BINARY") else {
        assert!(
            std::env::var_os("CODEWHALE_EXT_HOST_TESTS").is_none(),
            "required Windows compiled-host receipt is missing"
        );
        eprintln!(
            "compiled Windows receipt unavailable: compile the canonical bundle and name its image"
        );
        return;
    };
    let runtime = runtime(ExtensionHostRuntime::Bun, Some(Path::new(&binary))).unwrap();
    assert!(
        runtime.compiled,
        "test must use the exact canonical compiled-host filename/identity"
    );
    owner_boundary(runtime).await;
}

#[test]
fn windows_native_profiles_are_distinct_and_acl_grant_refuses_junctions() {
    let first = Profile::create().unwrap();
    let second = Profile::create().unwrap();
    assert_ne!(first.name, second.name);
    assert_eq!(
        unsafe { EqualSid(first.sid, second.sid) },
        0,
        "retired profile ACLs cannot be inherited by a new host"
    );
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let link = root.path().join("junction");
    let outside_file = outside.path().join("ungranted.txt");
    std::fs::write(&outside_file, b"outside control").unwrap();
    // Junction creation needs no symlink privilege; every failure is a red
    // Windows receipt, never a silent environment skip.
    let status = std::process::Command::new("cmd.exe")
        .args(["/d", "/c", "mklink", "/J"])
        .arg(&link)
        .arg(outside.path())
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "cannot create the actual junction control"
    );
    assert!(WindowsDirectory::open(&link).is_err());
    let sandbox = NativeSandbox {
        profile: Arc::new(first),
        _assets: Arc::new(tempfile::tempdir().unwrap()),
        program: PathBuf::new(),
        data: root.path().to_path_buf(),
    };
    let result = sandbox.admit_root(root.path());
    // Remove only the exact junction we created before tempfile cleanup.
    std::fs::remove_dir(&link).unwrap();
    assert!(
        result.is_err(),
        "grant traversal must refuse reparse points"
    );
    assert_no_profile_grant(outside.path(), sandbox.profile.sid);
    assert_no_profile_grant(&outside_file, sandbox.profile.sid);
}

#[test]
fn windows_argv_and_environment_keep_exact_values_and_reject_nul() {
    let args = vec![
        String::new(),
        "C:\\folder with space\\".into(),
        "a\"b".into(),
        "鲸鱼".into(),
    ];
    let line = command_line(OsStr::new("C:\\runtime folder\\node.exe"), &args).unwrap();
    assert_eq!(
        String::from_utf16(&line[..line.len() - 1]).unwrap(),
        "\"C:\\runtime folder\\node.exe\" \"\" \"C:\\folder with space\\\\\" \"a\\\"b\" \"鲸鱼\""
    );
    assert!(command_line(OsStr::new("node.exe"), &["x\0y".into()]).is_err());
    assert!(environment_block(&[("A".into(), "value\0tail".into())]).is_err());
    assert!(environment_block(&[("A=B".into(), "value".into())]).is_err());
    let env = environment_block(&[
        ("Z".into(), "last".into()),
        ("a".into(), "first=literal".into()),
    ])
    .unwrap();
    assert_eq!(
        String::from_utf16(&env).unwrap(),
        "a=first=literal\0Z=last\0\0"
    );
}

// Check the actual external ACL, not only the resolver's return value. The
// current user still owns these controls; our AppContainer SID must never gain
// an allow ACE through SetSecurityInfo's descendant propagation.
fn assert_no_profile_grant(path: &Path, profile: PSID) {
    assert_eq!(
        acl_snapshot(path, None),
        acl_snapshot(path, Some(profile)),
        "outside control {} received the sandbox SID",
        path.display()
    );
}
fn acl_snapshot(path: &Path, exclude: Option<PSID>) -> Vec<Vec<u8>> {
    use windows_sys::Win32::Security::{ACCESS_ALLOWED_ACE, ACE_HEADER, GetAce};
    use windows_sys::Win32::System::SystemServices::ACCESS_ALLOWED_ACE_TYPE;
    let directory;
    let file;
    let handle = if path.is_dir() {
        directory = WindowsDirectory::open(path).unwrap();
        directory.acl_handle().unwrap().as_raw_handle()
    } else {
        file = fs::OpenOptions::new()
            .access_mode(READ_CONTROL | FILE_READ_ATTRIBUTES)
            .share_mode(1)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)
            .unwrap();
        file.as_raw_handle()
    };
    let mut dacl = null_mut();
    let mut descriptor = null_mut();
    let error = unsafe {
        GetSecurityInfo(
            handle,
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            &mut dacl,
            null_mut(),
            &mut descriptor,
        )
    };
    assert_eq!(error, 0, "cannot inspect control ACL");
    let _descriptor = LocalAllocation(descriptor);
    let dacl = std::ptr::NonNull::new(dacl).expect("control must have an actual DACL");
    let mut result = Vec::new();
    for index in 0..unsafe { dacl.as_ref().AceCount as u32 } {
        let mut ace = null_mut();
        assert_ne!(unsafe { GetAce(dacl.as_ptr(), index, &mut ace) }, 0);
        let ace = std::ptr::NonNull::new(ace).expect("GetAce must return an actual ACE");
        let header = unsafe { ace.cast::<ACE_HEADER>().as_ref() };
        assert!(header.AceSize as usize >= std::mem::size_of::<ACE_HEADER>());
        if header.AceType as u32 == ACCESS_ALLOWED_ACE_TYPE {
            assert!(header.AceSize as usize >= std::mem::size_of::<ACCESS_ALLOWED_ACE>());
            let sid = unsafe {
                std::ptr::addr_of_mut!((*ace.cast::<ACCESS_ALLOWED_ACE>().as_ptr()).SidStart)
            };
            if exclude.is_some_and(|profile| unsafe { EqualSid(profile, sid.cast()) } != 0) {
                continue;
            }
        }
        result.push(
            unsafe {
                std::slice::from_raw_parts(ace.cast::<u8>().as_ptr(), header.AceSize as usize)
            }
            .to_vec(),
        );
    }
    result
}

#[test]
fn windows_profile_retirement_removes_only_its_grants_and_inherited_data_on_restarts() {
    use windows_sys::Win32::Security::GetLengthSid;
    let data = tempfile::tempdir().unwrap();
    let original_file = data.path().join("existing-state.json");
    fs::write(&original_file, b"retained user state").unwrap();
    let original_root_acl = acl_snapshot(data.path(), None);
    let original_file_acl = acl_snapshot(&original_file, None);
    let make = || NativeSandbox {
        profile: Arc::new(Profile::create().unwrap()),
        _assets: Arc::new(tempfile::tempdir().unwrap()),
        program: PathBuf::new(),
        data: data.path().to_path_buf(),
    };
    // A concurrent/new profile's exact grants must survive old-profile cleanup.
    let other = make();
    other.grant_tree(data.path(), true).unwrap();
    for turn in 0..8 {
        let current = make();
        current.grant_tree(data.path(), true).unwrap();
        let root_without_current = acl_snapshot(data.path(), Some(current.profile.sid));
        let file_without_current = acl_snapshot(&original_file, Some(current.profile.sid));
        // Creation after admission exercises inherited grants, not only the
        // immutable list of files that happened to exist during prepare.
        let nested = data.path().join(format!("new-state-{turn}"));
        fs::create_dir(&nested).unwrap();
        let created = nested.join("state.json");
        fs::write(&created, b"new persisted state").unwrap();
        let length = unsafe { GetLengthSid(current.profile.sid) } as usize;
        let mut sid_copy = vec![0_usize; length.div_ceil(std::mem::size_of::<usize>())];
        unsafe {
            std::ptr::copy_nonoverlapping(
                current.profile.sid as *const u8,
                sid_copy.as_mut_ptr().cast::<u8>(),
                length,
            );
        }
        let retired_sid = sid_copy.as_mut_ptr().cast();
        assert_ne!(
            acl_snapshot(&created, None),
            acl_snapshot(&created, Some(retired_sid)),
            "control must really inherit the current profile grant"
        );
        // A new/live owner may still have a data writer open. Exact ACL
        // retirement must not fail only because legitimate writes continue.
        let writer = fs::OpenOptions::new()
            .write(true)
            .open(&original_file)
            .unwrap();
        drop(current); // no Tokio context here: exact retirement is synchronous
        drop(writer);
        assert_eq!(acl_snapshot(data.path(), None), root_without_current);
        assert_eq!(acl_snapshot(&original_file, None), file_without_current);
        assert_no_profile_grant(&nested, retired_sid);
        assert_no_profile_grant(&created, retired_sid);
        assert_eq!(fs::read(&created).unwrap(), b"new persisted state");
        assert_eq!(fs::read(&original_file).unwrap(), b"retained user state");
    }
    drop(other);
    assert_eq!(acl_snapshot(data.path(), None), original_root_acl);
    assert_eq!(acl_snapshot(&original_file, None), original_file_acl);
}

#[test]
fn windows_profile_directory_budget_and_recorded_identity_refuse_before_overwrite() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("first"), b"one").unwrap();
    fs::write(root.path().join("second"), b"two").unwrap();
    // Budget includes pending paths, so even a flat directory fails before
    // growing the allocation past the bound. Admission and retirement share
    // this exact production helper.
    let mut pending = vec![PathBuf::from("already-pending"); MAX_GRANT_ENTRIES - 3];
    assert!(enqueue_children(root.path(), &mut pending, 2).is_err());
    assert_eq!(2 + pending.len(), MAX_GRANT_ENTRIES);
    let profile = Profile::create().unwrap();
    let path = root.path().join("first");
    let original = File::open(&path).unwrap();
    profile.remember(&path, &original, false).unwrap();
    let identity = profile.grants.lock().unwrap().get(&path).unwrap().identity;
    drop(original);
    fs::rename(&path, root.path().join("moved-original")).unwrap();
    fs::write(&path, b"different object").unwrap();
    let replacement = File::open(&path).unwrap();
    assert!(profile.remember(&path, &replacement, false).is_err());
    assert_eq!(
        profile.grants.lock().unwrap().get(&path).unwrap().identity,
        identity
    );
    // No ACL was granted in this accounting-only check. Do not schedule an
    // irrelevant retirement against the deliberately replaced fixture path.
    profile.grants.lock().unwrap().clear();
}
