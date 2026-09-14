use super::*;
use tempfile::tempdir;

fn git(root: &Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("git");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn repository(root: &Path) {
    git(root, &["init", "--quiet"]);
    git(root, &["config", "user.name", "Delivery test"]);
    git(root, &["config", "user.email", "delivery@example.invalid"]);
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/lib.rs"), "baseline\n").unwrap();
    git(root, &["add", "--", "src/lib.rs"]);
    git(root, &["commit", "--quiet", "-m", "baseline"]);
}

fn worker(root: &Path, write: bool, paths: &[&str], scope: &[&str]) -> (SubAgentManager, String) {
    let mut manager = SubAgentManager::new(root.to_path_buf(), 2);
    let id = manager.insert_test_running_agent("delivery", root);
    let record = manager.worker_records.get_mut(&id).unwrap();
    record.spec.runtime_profile.permissions.write = write;
    record.spec.launch_manifest = Some(ChildLaunchManifest {
        owner_session: "workspace".into(),
        child_id: id.clone(),
        profile: record.spec.runtime_profile.clone(),
        prompt: "produce report".into(),
        cwd: Some(root.display().to_string()),
        worktree: false,
        writable_roots: scope.iter().map(|path| (*path).into()).collect(),
        writable_files: Vec::new(),
        coordination_contracts: Vec::new(),
        expected_artifact: None,
        deliverables: paths.iter().map(|path| (*path).into()).collect(),
        token_budget: None,
        resume_identity: None,
        generation: 1,
        resume_from_agent_id: None,
    });
    record.delivery_evidence = DeliveryEvidence::capture(&record.spec);
    if write && !scope.is_empty() {
        manager
            .coordination
            .register_claim(
                WriteScopeClaim {
                    owner: id.clone(),
                    roots: scope.iter().map(|path| (*path).into()).collect(),
                    exact_files: Vec::new(),
                    contracts: Vec::new(),
                },
                false,
                |_| false,
            )
            .unwrap();
    }
    (manager, id)
}

fn complete(manager: &mut SubAgentManager, id: &str, report: &str) -> AgentRunVerificationSummary {
    let mut result = manager.get_result(id).unwrap();
    result.status = SubAgentStatus::Completed;
    result.result = Some(report.into());
    manager.complete_worker_from_result(id, &result);
    manager.worker_records[id].verification.clone()
}

#[test]
fn declared_deliverables_narrow_default_scope_and_reject_invalid_input() {
    let request = parse_spawn_request(&json!({
        "type": "implement", "prompt": "write outputs", "deliverables": ["tmp/a/report.md", "tmp/b/report.md"]
    })).unwrap();
    assert!(request.write_roots.is_empty());
    assert_eq!(request.exact_files, ["tmp/a/report.md", "tmp/b/report.md"]);
    for paths in [
        json!([""]),
        json!(["../report.md"]),
        json!(["/tmp/report.md"]),
        json!([".git/config"]),
        json!(["."]),
        json!([1]),
        json!("report.md"),
    ] {
        assert!(
            parse_spawn_request(
                &json!({"type":"implement", "prompt":"report", "deliverables": paths})
            )
            .is_err()
        );
    }
    assert!(
        parse_spawn_request(
            &json!({"type":"implement", "prompt":"report", "deliverables": vec!["x.md"; 17]})
        )
        .is_err()
    );
    assert!(
        delivery::declared_paths(&[], Some("review findings"))
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        delivery::declared_paths(&[], Some("report.md")).unwrap(),
        ["report.md"]
    );
}

#[test]
fn deliverable_verdict_distinguishes_present_missing_empty_directory_and_scope() {
    let tmp = tempdir().unwrap();
    fs::write(tmp.path().join("present.md"), "report").unwrap();
    fs::write(tmp.path().join("empty.md"), "").unwrap();
    fs::create_dir(tmp.path().join("directory")).unwrap();
    for (path, status) in [
        ("present.md", "present"),
        ("missing.md", "missing"),
        ("empty.md", "empty"),
        ("directory", "not_file"),
    ] {
        assert_eq!(
            delivery::check_deliverable(tmp.path(), path, true).status,
            status
        );
    }
    assert_eq!(
        delivery::check_deliverable(tmp.path(), "present.md", false).status,
        "out_of_scope"
    );
}

#[cfg(unix)]
#[test]
fn deliverables_refuse_leaf_and_parent_symlink_escape() {
    let tmp = tempdir().unwrap();
    let outside = tempdir().unwrap();
    fs::write(outside.path().join("secret.md"), "outside").unwrap();
    std::os::unix::fs::symlink(outside.path().join("secret.md"), tmp.path().join("leaf.md"))
        .unwrap();
    std::os::unix::fs::symlink(outside.path(), tmp.path().join("linked")).unwrap();
    for path in ["leaf.md", "linked/secret.md"] {
        assert_eq!(
            delivery::check_deliverable(tmp.path(), path, true).status,
            "invalid_path"
        );
        assert!(delivery::safe_deliverable_path(tmp.path(), path).is_err());
    }
}

#[test]
fn missing_declared_deliverable_is_visible_in_terminal_sentinel() {
    let tmp = tempdir().unwrap();
    let (mut manager, id) = worker(tmp.path(), true, &["report.md"], &["."]);
    let verification = complete(&mut manager, &id, "Finished the research.");
    assert_eq!(verification.status, "deliverable_missing");
    assert_eq!(verification.deliverables[0].status, "missing");
    let mut result = manager.get_result(&id).unwrap();
    result.status = SubAgentStatus::Completed;
    let completion = subagent_completion_with_verification(
        "workspace",
        &result,
        None,
        Some(&verification),
        None,
    );
    assert!(completion.payload.contains("deliverable_missing"));
    assert!(completion.payload.contains("report.md"));
}

#[test]
fn declared_output_is_not_an_undeclared_edit_and_undeclared_worker_stays_self_reported() {
    let tmp = tempdir().unwrap();
    repository(tmp.path());
    let (mut manager, id) = worker(tmp.path(), true, &["report.md"], &["."]);
    fs::write(tmp.path().join("report.md"), "findings").unwrap();
    let verification = complete(&mut manager, &id, "Report ready.");
    assert_eq!(verification.status, "deliverables_present");
    assert_eq!(verification.deliverables[0].bytes, Some(8));
    let (mut manager, id) = worker(tmp.path(), false, &[], &[]);
    assert_eq!(
        complete(&mut manager, &id, "No changes.").status,
        "self_report_only"
    );
}

#[test]
fn change_like_prose_and_line_citations_are_never_edit_claims() {
    for report in [
        "Fixed behavior is documented in src/lib.rs:12-19.",
        "CHANGES: None\nThe added guard is at src/lib.rs:12-19",
        "CHANGES: src/lib.rs:12-19",
        "CHANGES:\n- Reviewed src/lib.rs:12-19.",
        "CHANGES:\n- Reviewed src/lib.rs:12-19!",
        "CHANGES:\n- Reviewed src/lib.rs:12-19:",
        "CHANGES: [src/lib.rs:12-19](src/lib.rs#L12-L19)",
        "CHANGES: [source](src/lib.rs:12-19)",
    ] {
        assert!(
            delivery::explicit_change_paths(report).is_empty(),
            "{report}"
        );
    }
    assert_eq!(
        delivery::explicit_change_paths("CHANGES:\n- src/lib.rs\n- report.md"),
        BTreeSet::from(["src/lib.rs".into(), "report.md".into()])
    );
    let tmp = tempdir().unwrap();
    repository(tmp.path());
    let (mut manager, id) = worker(tmp.path(), false, &[], &[]);
    assert_eq!(
        complete(&mut manager, &id, "CHANGES: src/lib.rs").status,
        "self_report_only"
    );
}

#[test]
fn unchanged_dirty_file_does_not_satisfy_a_new_edit_claim() {
    let tmp = tempdir().unwrap();
    repository(tmp.path());
    fs::write(tmp.path().join("src/lib.rs"), "existing dirty work\n").unwrap();
    let (mut manager, id) = worker(tmp.path(), true, &[], &["src"]);
    let verification = complete(&mut manager, &id, "CHANGES: src/lib.rs");
    assert_eq!(verification.status, "claim_mismatch");
    assert!(verification.summary.contains("declared but unchanged"));
    assert!(verification.summary.contains("src/lib.rs"));
}

#[test]
fn five_heading_output_declares_changed_files_without_claiming_evidence_or_risks() {
    let tmp = tempdir().unwrap();
    repository(tmp.path());
    let (mut manager, id) = worker(tmp.path(), true, &[], &["src"]);
    fs::write(tmp.path().join("src/lib.rs"), "updated\n").unwrap();
    manager
        .worker_records
        .get_mut(&id)
        .unwrap()
        .delivery_evidence
        .observed_writes
        .insert("src/lib.rs".into());
    let report = "### SUMMARY\n\nUpdated the parser.\n\n\
        ### EVIDENCE\n\n- Reviewed src/reference.rs:12-19.\n\n\
        ### CHANGES\n\n- `src/lib.rs` — adjusted the parser\n\n\
        ### RISKS\n\n- src/consumer.rs still needs a separate review\n\n\
        ### BLOCKERS\n\nNone.\n";
    for report in [
        report.to_string(),
        report.replace("### CHANGES", "### changes"),
    ] {
        assert_eq!(
            delivery::explicit_change_paths(&report),
            BTreeSet::from(["src/lib.rs".into()])
        );
        assert_ne!(
            complete(&mut manager, &id, &report).status,
            "claim_mismatch"
        );
    }
}

#[test]
fn changes_bullet_descriptions_do_not_invent_file_claims() {
    let tmp = tempdir().unwrap();
    repository(tmp.path());
    let (mut manager, id) = worker(tmp.path(), true, &[], &["src"]);
    fs::write(tmp.path().join("src/lib.rs"), "updated\n").unwrap();
    manager
        .worker_records
        .get_mut(&id)
        .unwrap()
        .delivery_evidence
        .observed_writes
        .insert("src/lib.rs".into());
    for declaration in [
        "- `src/lib.rs` — updated parsing.",
        "- src/lib.rs: Updated parsing.",
        "- src/lib.rs updated parsing; reviewed notes.md.",
        "- [src/lib.rs](src/lib.rs) - Updated parsing! See notes.md.",
        "- 'src/lib.rs' — matches src/reference.rs:12-19.",
    ] {
        let report = format!(
            "### SUMMARY\n\nUpdated the parser.\n\n\
             ### EVIDENCE\n\n- Reviewed src/reference.rs:12-19.\n\n\
             ### CHANGES\n\n{declaration}\n\n\
             ### RISKS\n\n- Check notes.md separately.\n\n\
             ### BLOCKERS\n\nNone.\n"
        );
        assert_eq!(
            delivery::explicit_change_paths(&report),
            BTreeSet::from(["src/lib.rs".into()]),
            "{declaration}"
        );
        assert_ne!(
            complete(&mut manager, &id, &report).status,
            "claim_mismatch",
            "{declaration}"
        );
    }
}

#[test]
fn explicit_change_path_lists_preserve_quoted_spaces_and_punctuation() {
    let expected = BTreeSet::from([
        "src/lib.rs".into(),
        "src/with spaces.rs".into(),
        "src/another file.rs".into(),
        "src/trailing.".into(),
    ]);
    for label in ["CHANGES:", "Changed files:", "Files changed:"] {
        let report = format!(
            "{label} src/lib.rs, `src/with spaces.rs`; \"src/another file.rs\" 'src/trailing.'"
        );
        assert_eq!(delivery::explicit_change_paths(&report), expected);
    }
    assert!(delivery::explicit_change_paths("CHANGES: Updated parsing.").is_empty());
    assert!(
        delivery::explicit_change_paths("CHANGES: src/reference.rs:12-19. — reviewed only.")
            .is_empty()
    );
}

#[test]
fn modification_of_already_dirty_file_is_measured_against_spawn_content() {
    let tmp = tempdir().unwrap();
    repository(tmp.path());
    fs::write(tmp.path().join("src/lib.rs"), "existing dirty work\n").unwrap();
    let (mut manager, id) = worker(tmp.path(), true, &[], &["src"]);
    fs::write(
        tmp.path().join("src/lib.rs"),
        "worker changed this further\n",
    )
    .unwrap();
    assert_eq!(
        complete(&mut manager, &id, "CHANGES: src/lib.rs").status,
        "self_report_only"
    );
}

#[test]
fn committed_change_is_compared_with_exact_spawn_head_without_timestamp_guessing() {
    let tmp = tempdir().unwrap();
    repository(tmp.path());
    let (mut manager, id) = worker(tmp.path(), true, &[], &["src"]);
    fs::write(tmp.path().join("src/lib.rs"), "worker commit\n").unwrap();
    git(tmp.path(), &["add", "--", "src/lib.rs"]);
    git(tmp.path(), &["commit", "--quiet", "-m", "worker"]);
    assert_eq!(
        complete(&mut manager, &id, "CHANGES: src/lib.rs").status,
        "self_report_only"
    );
}

#[test]
fn an_observed_write_without_a_declaration_is_flagged_but_external_changes_are_not() {
    let tmp = tempdir().unwrap();
    repository(tmp.path());
    let (mut manager, id) = worker(tmp.path(), true, &[], &["src"]);
    fs::write(tmp.path().join("src/lib.rs"), "new change\n").unwrap();
    manager
        .worker_records
        .get_mut(&id)
        .unwrap()
        .delivery_evidence
        .observed_writes
        .insert("src/lib.rs".into());
    assert_eq!(
        complete(&mut manager, &id, "CHANGES: None").status,
        "claim_mismatch"
    );
    for scope in ["reports", "."] {
        let (mut manager, id) = worker(tmp.path(), true, &[], &[scope]);
        fs::write(
            tmp.path().join("src/lib.rs"),
            format!("external {scope} change\n"),
        )
        .unwrap();
        assert!(
            manager.worker_records[&id]
                .delivery_evidence
                .observed_writes
                .is_empty()
        );
        assert_eq!(
            complete(&mut manager, &id, "CHANGES: None").status,
            "self_report_only"
        );
    }
}

#[test]
fn disjoint_sibling_write_paths_admit_and_ancestor_overlap_names_actual_remedy() {
    let mut ledger = CoordinationLedger::default();
    let claim = |owner: &str, path: &str| WriteScopeClaim {
        owner: owner.into(),
        roots: vec![path.into()],
        exact_files: Vec::new(),
        contracts: Vec::new(),
    };
    ledger
        .register_claim(claim("a", "tmp/scan/a"), false, |_| true)
        .unwrap();
    ledger
        .register_claim(claim("b", "tmp/scan/b"), false, |_| true)
        .unwrap();
    let error = ledger
        .register_claim(claim("broad", "tmp/scan"), false, |_| true)
        .unwrap_err();
    for text in [
        "tmp/scan/a",
        "tmp/scan",
        "disjoint sibling",
        "exact_files",
        "write_authority=read_only",
    ] {
        assert!(error.contains(text), "{error}");
    }
}

#[cfg(unix)]
#[tokio::test]
#[allow(clippy::print_stderr)] // Test receipt must distinguish refused probes from exercised isolation.
async fn enforced_readonly_python_queries_sqlite_under_a_live_peer_write_claim() {
    let tmp = tempdir().unwrap();
    let database = rusqlite::Connection::open(tmp.path().join("fixture.sqlite")).unwrap();
    database
        .execute_batch(
            "CREATE TABLE fixture(value TEXT); INSERT INTO fixture VALUES ('peer-read-receipt');",
        )
        .unwrap();
    drop(database);
    fs::write(tmp.path().join("peer.txt"), "preserve peer bytes").unwrap();
    fs::write(tmp.path().join("own.txt"), "own bytes").unwrap();
    let manager = new_shared_subagent_manager(tmp.path().to_path_buf(), 4);
    {
        let mut guard = manager.write().await;
        guard.insert_test_running_agent("analysis", tmp.path());
        guard.insert_test_running_agent("peer", tmp.path());
        for (owner, file) in [("agent_analysis", "own.txt"), ("agent_peer", "peer.txt")] {
            guard
                .coordination
                .register_claim(
                    WriteScopeClaim {
                        owner: owner.into(),
                        roots: Vec::new(),
                        exact_files: vec![file.into()],
                        contracts: Vec::new(),
                    },
                    false,
                    |_| true,
                )
                .unwrap();
        }
        assert_eq!(
            guard.live_peer_shared_write_claim_owners("agent_analysis"),
            ["agent_peer"]
        );
    }
    let python = [
        "/usr/bin/python3",
        "/opt/homebrew/bin/python3",
        "/usr/local/bin/python3",
    ]
    .into_iter()
    .find(|path| Path::new(path).is_file())
    .expect("Python fixture runtime");
    for role in [FleetRole::Builder, FleetRole::Scout] {
        let mut runtime = super::tests::stub_runtime();
        runtime.manager = Arc::clone(&manager);
        runtime.context = ToolContext::new(tmp.path());
        runtime.context.auto_approve = true;
        runtime.context.elevated_sandbox_policy =
            Some(crate::sandbox::SandboxPolicy::DangerFullAccess);
        #[cfg(target_os = "linux")]
        runtime
            .context
            .shell_manager
            .lock()
            .unwrap()
            .set_prefer_bwrap(true);
        let available = runtime
            .context
            .shell_manager
            .lock()
            .unwrap()
            .configured_sandbox_type()
            .is_some();
        runtime.worker_profile = WorkerRuntimeProfile::for_role(role.clone());
        let registry = SubAgentToolRegistry::new_with_owner(
            runtime,
            role,
            "agent_analysis".into(),
            "analysis".into(),
            Some(vec!["bash".into()]),
            Arc::new(Mutex::new(TodoList::new())),
            Arc::new(Mutex::new(PlanState::default())),
        );
        let script = "import sqlite3; c=sqlite3.connect('file:fixture.sqlite?mode=ro', uri=True); print(c.execute('SELECT value FROM fixture').fetchone()[0])";
        let query = json!({"command": format!("{python} -I -B -c {}", shell_words::quote(script)), "read_only": true});
        let read = registry.execute("agent_analysis", "bash", query).await;
        if !available {
            let error = read.unwrap_err().to_string();
            assert!(error.contains("native read-only enforcement"), "{error}");
            eprintln!("UNRUN: native child+peer Python probe; unavailable sandbox was refused");
            continue;
        }
        let read = read.unwrap();
        assert!(read.contains("peer-read-receipt"), "{read}");
        let write = json!({"command": format!("{python} -I -B -c {}", shell_words::quote("open('peer.txt', 'w').write('corrupt')")), "read_only": true});
        let error = registry
            .execute("agent_analysis", "bash", write)
            .await
            .unwrap_err()
            .to_string();
        assert!(
            !error.contains("blocking peers"),
            "read-only execution must reach native enforcement: {error}"
        );
        assert_eq!(
            fs::read_to_string(tmp.path().join("peer.txt")).unwrap(),
            "preserve peer bytes"
        );
        assert_eq!(
            manager
                .read()
                .await
                .get_result("agent_peer")
                .unwrap()
                .status,
            SubAgentStatus::Running
        );
        eprintln!(
            "NATIVE_READONLY_ENFORCED: child SQLite query passed under a live peer claim; mutation denied"
        );
    }
}
