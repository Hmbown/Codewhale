use super::*;
use crate::automation_manager::{
    AutomationManager, AutomationStatus, CreateAutomationRequest, run_now_shared,
};
use crate::runtime_threads::{RuntimeThreadManager, RuntimeThreadManagerConfig};
use crate::task_manager::{TaskManager, TaskManagerConfig};

fn fixture_config() -> Config {
    let mut config = Config {
        api_key: Some("local-runtime-binding-fixture".into()),
        base_url: Some("http://127.0.0.1:1/v1".into()),
        ..Config::default()
    };
    config.set_feature("mcp", false).unwrap();
    config.set_feature("subagents", false).unwrap();
    config
}

#[tokio::test]
async fn runtime_store_binding_persists_on_exit_without_a_model_turn() -> anyhow::Result<()> {
    let _environment = crate::test_support::lock_test_env();
    let root = tempfile::tempdir()?;
    let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", root.path());
    let _runtime = crate::test_support::EnvVarGuard::remove("CODEWHALE_RUNTIME_DIR");
    let _legacy = crate::test_support::EnvVarGuard::remove("DEEPSEEK_RUNTIME_DIR");
    let explicit_store = crate::test_support::EnvVarGuard::set(
        "CODEWHALE_RUNTIME_DIR",
        root.path().join("original-runtime"),
    );
    let mut config = fixture_config();
    let sessions = SessionManager::default_location()?;
    let original = crate::session_manager::create_saved_session_with_id_and_mode(
        "legacy-conversation".into(),
        &[text_message("user", "retain this earlier conversation")],
        "deepseek-v4-pro",
        root.path(),
        0,
        None,
        None,
    );
    assert!(original.metadata.runtime_store.is_none());
    sessions.save_session(&original)?;
    sessions.save_checkpoint(&original)?;
    let mut app = Box::new(create_test_app());
    apply_loaded_session_with_goal(&mut app, &mut config, &original, None)
        .map_err(anyhow::Error::msg)?;
    let task_config = TaskManagerConfig::from_runtime(&config, root.path().into(), None, Some(1));
    let tasks = TaskManager::start(
        task_config.clone(),
        config.clone(),
        app.plugin_registry.clone(),
        &original.metadata.id,
        None,
    )
    .await?;
    let binding = tasks
        .session_store_binding()
        .expect("attached Runtime store");
    app.runtime_services.task_manager = Some(tasks.clone());
    let (handle, actor) =
        persistence_actor::spawn_persistence_actor(SessionManager::default_location()?);
    // Match clean exit ordering: no Engine turn, checkpoint or snapshot has
    // been queued by this host before its TaskManager stops.
    tasks.shutdown_and_wait().await?;
    assert!(
        super::super::event_loop::persist_settled_session_on_shutdown(&mut app, &handle)
            .map_err(anyhow::Error::msg)?
    );
    assert!(handle.try_send(PersistRequest::Shutdown));
    actor.await?;
    let saved = sessions.load_session(&original.metadata.id)?;
    assert_eq!(saved.metadata.runtime_store.as_ref(), Some(&binding));
    assert_eq!(saved.metadata.title, original.metadata.title);
    assert_eq!(saved.messages, original.messages);
    assert!(
        sessions
            .load_session_checkpoint(&original.metadata.id)?
            .is_none()
    );
    drop(app);
    drop(tasks);
    drop(explicit_store);
    // Ordinary resume now reopens the same authority without an env override.
    let resumed = TaskManager::start(
        task_config,
        config,
        Arc::new(crate::plugins::PluginRegistry::empty(root.path())),
        &saved.metadata.id,
        saved.metadata.runtime_store.as_ref(),
    )
    .await?;
    assert_eq!(resumed.execution_scope(), binding.execution_scope);
    resumed.shutdown_and_wait().await?;
    Ok(())
}

#[tokio::test]
async fn runtime_store_binding_exit_preserves_inflight_recovery() -> anyhow::Result<()> {
    let _environment = crate::test_support::lock_test_env();
    let root = tempfile::tempdir()?;
    let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", root.path());
    let sessions = SessionManager::default_location()?;
    for (loading, dispatch) in [(true, false), (false, true)] {
        let original = crate::session_manager::create_saved_session_with_mode(
            &[],
            "deepseek-v4-pro",
            root.path(),
            0,
            None,
            None,
        );
        let path = sessions.save_session(&original)?;
        let checkpoint = sessions.save_checkpoint(&original)?;
        let saved_before = std::fs::read(&path)?;
        let checkpoint_before = std::fs::read(&checkpoint)?;
        let mut app = Box::new(create_test_app());
        app.current_session_id = Some(original.metadata.id.clone());
        app.is_loading = loading;
        app.dispatch_in_flight = dispatch;
        let (handle, actor) =
            persistence_actor::spawn_persistence_actor(SessionManager::default_location()?);
        assert!(
            !super::super::event_loop::persist_settled_session_on_shutdown(&mut app, &handle)
                .map_err(anyhow::Error::msg)?
        );
        assert!(handle.try_send(PersistRequest::Shutdown));
        actor.await?;
        assert_eq!(std::fs::read(path)?, saved_before);
        assert_eq!(std::fs::read(checkpoint)?, checkpoint_before);
    }
    Ok(())
}

#[tokio::test]
async fn runtime_store_binding_survives_launch_snapshot_and_resume() -> anyhow::Result<()> {
    let _environment = crate::test_support::lock_test_env();
    let root = tempfile::tempdir()?;
    let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", root.path());
    let _runtime = crate::test_support::EnvVarGuard::remove("CODEWHALE_RUNTIME_DIR");
    let _legacy = crate::test_support::EnvVarGuard::remove("DEEPSEEK_RUNTIME_DIR");
    let config = fixture_config();
    let mut app = Box::new(create_test_app());
    app.workspace = root.path().into();
    let initial_id = super::super::event_loop::ensure_runtime_session_id(&mut app);
    let task_config = TaskManagerConfig::from_runtime(&config, root.path().into(), None, Some(1));
    let tasks = TaskManager::start(
        task_config.clone(),
        config.clone(),
        app.plugin_registry.clone(),
        &initial_id,
        None,
    )
    .await?;
    app.runtime_services.task_manager = Some(tasks.clone());
    let sessions = SessionManager::default_location()?;
    // A saved initial conversation may later be deleted while another launch
    // still refers to its Runtime store.
    let initial = build_session_snapshot(&mut app, &sessions).map_err(anyhow::Error::msg)?;
    sessions.save_session(&initial)?;
    let launch = begin_launch_session(&mut app, None);
    assert!(!launch.is_error, "{:?}", launch.message);
    assert_ne!(app.current_session_id.as_deref(), Some(initial_id.as_str()));
    let saved = build_session_snapshot(&mut app, &sessions).map_err(anyhow::Error::msg)?;
    let binding = saved
        .metadata
        .runtime_store
        .clone()
        .expect("attached host binding");
    assert_eq!(binding.execution_scope, tasks.execution_scope());
    sessions.save_session(&saved)?;
    let mut automations = AutomationManager::open(root.path().join("automations"))?;
    automations.bind_task_manager(&tasks)?;
    let automation = automations.create_automation(CreateAutomationRequest {
        name: "resumed ownership fixture".into(),
        prompt: "local fixture only".into(),
        rrule: "FREQ=HOURLY;INTERVAL=1".into(),
        cwds: vec![root.path().into()],
        model: None,
        model_provider: None,
        model_provider_id: None,
        mode: None,
        allow_shell: Some(false),
        trust_mode: Some(false),
        auto_approve: Some(false),
        delivery_mode: None,
        status: Some(AutomationStatus::Paused),
    })?;
    tasks.shutdown_and_wait().await?;
    drop(app);
    drop(tasks);
    drop(automations);
    sessions.delete_session(&initial_id)?;
    assert!(
        binding.data_dir.is_dir(),
        "transcript deletion cannot erase Runtime authority"
    );
    let loaded = sessions.load_session(&saved.metadata.id)?;
    assert_eq!(loaded.metadata.runtime_store.as_ref(), Some(&binding));
    let mut resumed = Box::new(create_test_app());
    let mut resumed_config = config.clone();
    apply_loaded_session_with_goal(&mut resumed, &mut resumed_config, &loaded, None)
        .map_err(anyhow::Error::msg)?;
    let tasks = TaskManager::start(
        task_config.clone(),
        config.clone(),
        resumed.plugin_registry.clone(),
        &loaded.metadata.id,
        loaded.metadata.runtime_store.as_ref(),
    )
    .await?;
    assert_eq!(
        tasks.execution_scope(),
        automation.execution_scope.as_deref().unwrap()
    );
    resumed.runtime_services.task_manager = Some(tasks.clone());
    let automations = Arc::new(tokio::sync::Mutex::new(AutomationManager::open(
        root.path().join("automations"),
    )?));
    // The real Run-now admission must now create its durable receipt. The
    // configured endpoint is closed loopback and no shell/tool is authorized.
    let run = run_now_shared(&automations, &automation.id, &tasks).await?;
    assert!(run.task_id.is_some(), "{run:?}");
    assert_eq!(
        automations
            .lock()
            .await
            .list_runs(&automation.id, None)?
            .len(),
        1
    );
    assert_eq!(
        automations
            .lock()
            .await
            .get_automation(&automation.id)?
            .execution_scope,
        automation.execution_scope
    );
    tasks.shutdown_and_wait().await?;
    drop(resumed);
    drop(tasks);
    // Reproduce the old resume path: deriving a store from the saved
    // conversation id without its binding opens a foreign scope and cannot run.
    let foreign = TaskManager::start(
        task_config,
        config,
        Arc::new(crate::plugins::PluginRegistry::empty(root.path())),
        &loaded.metadata.id,
        None,
    )
    .await?;
    let foreign_automations = Arc::new(tokio::sync::Mutex::new(AutomationManager::open(
        root.path().join("automations"),
    )?));
    let definition_path = root
        .path()
        .join("automations/automations")
        .join(format!("{}.json", automation.id));
    let before_foreign_run = std::fs::read(&definition_path)?;
    let error = run_now_shared(&foreign_automations, &automation.id, &foreign)
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("another Runtime execution scope"),
        "{error:#}"
    );
    assert_eq!(
        foreign_automations
            .lock()
            .await
            .list_runs(&automation.id, None)?
            .len(),
        1
    );
    assert_eq!(std::fs::read(definition_path)?, before_foreign_run);
    let mut other_app = Box::new(create_test_app());
    other_app.runtime_services.task_manager = Some(foreign.clone());
    other_app.input = "preserve pending input".into();
    let old_id = other_app.current_session_id.clone();
    let error = apply_loaded_session_with_goal(&mut other_app, &mut resumed_config, &loaded, None)
        .unwrap_err();
    assert!(error.contains("Resume it in a new Codewhale process"));
    assert_eq!(other_app.current_session_id, old_id);
    assert_eq!(other_app.input, "preserve pending input");
    foreign.shutdown_and_wait().await?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn runtime_store_binding_retention_does_not_follow_session_directory_symlinks() -> anyhow::Result<()>
{
    let _environment = crate::test_support::lock_test_env();
    let root = tempfile::tempdir()?;
    let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", root.path());
    let sessions = SessionManager::default_location()?;
    let saved = crate::session_manager::create_saved_session_with_id_and_mode(
        "linked-session".into(),
        &[],
        "fixture",
        root.path(),
        0,
        None,
        None,
    );
    sessions.save_session(&saved)?;
    let target = root.path().join("unrelated-directory");
    std::fs::create_dir_all(&target)?;
    std::fs::write(target.join("keep.txt"), "preserve user data")?;
    let link = root.path().join("sessions/linked-session");
    std::os::unix::fs::symlink(&target, &link)?;
    sessions.delete_session("linked-session")?;
    assert!(!link.exists());
    assert_eq!(
        std::fs::read_to_string(target.join("keep.txt"))?,
        "preserve user data"
    );
    Ok(())
}

#[test]
fn runtime_store_binding_rejects_foreign_missing_or_overridden_store() -> anyhow::Result<()> {
    let _environment = crate::test_support::lock_test_env();
    let root = tempfile::tempdir()?;
    let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", root.path());
    let _runtime = crate::test_support::EnvVarGuard::remove("CODEWHALE_RUNTIME_DIR");
    let _legacy = crate::test_support::EnvVarGuard::remove("DEEPSEEK_RUNTIME_DIR");
    let config = fixture_config();
    let cfg = RuntimeThreadManagerConfig::for_session(root.path().join("tasks"), "original");
    let runtime = RuntimeThreadManager::open(config.clone(), root.path().into(), cfg.clone())?;
    let binding = runtime.session_store_binding();
    drop(runtime);
    let state_path = binding.data_dir.join("state.json");
    let before = std::fs::read(&state_path)?;
    let mut wrong = binding.clone();
    wrong.execution_scope = "0".repeat(64);
    let open = |binding: &crate::runtime_threads::RuntimeStoreBinding| {
        RuntimeThreadManager::open_for_session(
            config.clone(),
            root.path().into(),
            cfg.clone(),
            Arc::new(crate::plugins::PluginRegistry::empty(root.path())),
            Some(binding),
        )
    };
    assert!(
        open(&wrong)
            .err()
            .unwrap()
            .to_string()
            .contains("ownership does not match")
    );
    assert_eq!(std::fs::read(&state_path)?, before);
    wrong.data_dir = root.path().join("missing-store");
    assert!(open(&wrong).is_err());
    assert!(
        !wrong.data_dir.exists(),
        "saved binding cannot create a replacement authority"
    );
    let _override = crate::test_support::EnvVarGuard::set(
        "CODEWHALE_RUNTIME_DIR",
        root.path().join("foreign-override"),
    );
    assert!(
        open(&binding)
            .err()
            .unwrap()
            .to_string()
            .contains("override conflicts")
    );
    Ok(())
}

#[test]
fn missing_runtime_store_recovers_without_reusing_authority_or_resurrecting_stale_binding()
-> anyhow::Result<()> {
    let _environment = crate::test_support::lock_test_env();
    let root = tempfile::tempdir()?;
    let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", root.path());
    let _runtime = crate::test_support::EnvVarGuard::remove("CODEWHALE_RUNTIME_DIR");
    let _legacy = crate::test_support::EnvVarGuard::remove("DEEPSEEK_RUNTIME_DIR");
    let sessions = SessionManager::default_location()?;
    let mut saved = crate::session_manager::create_saved_session_with_id_and_mode(
        "interrupted".into(),
        &[text_message("user", "retain my work")],
        "deepseek-v4-pro",
        root.path(),
        0,
        None,
        None,
    );
    let missing = crate::runtime_threads::RuntimeStoreBinding {
        data_dir: root.path().join("sessions/previous/runtime"),
        execution_scope: "0".repeat(64),
    };
    saved.metadata.runtime_store = Some(missing.clone());
    sessions.save_session(&saved)?;
    let stale = saved.clone();
    let manager = RuntimeThreadManager::open_for_session(
        fixture_config(),
        root.path().into(),
        RuntimeThreadManagerConfig::for_session(root.path().join("tasks"), "interrupted"),
        Arc::new(crate::plugins::PluginRegistry::empty(root.path())),
        Some(&missing),
    )?;
    let recovered = manager.session_store_binding();
    assert_ne!(recovered.execution_scope, missing.execution_scope);
    assert_ne!(recovered.data_dir, missing.data_dir);
    assert!(!missing.data_dir.exists());
    assert!(
        RuntimeThreadManager::open_for_session(
            fixture_config(),
            root.path().into(),
            RuntimeThreadManagerConfig::for_session(root.path().join("tasks"), "interrupted"),
            Arc::new(crate::plugins::PluginRegistry::empty(root.path())),
            Some(&missing),
        )
        .is_err(),
        "concurrent recovery cannot mint a second owner"
    );
    // Losing the process before the repaired binding is saved must leave a
    // retryable, session-scoped store, not an orphan or a second authority.
    drop(manager);
    let manager = RuntimeThreadManager::open_for_session(
        fixture_config(),
        root.path().into(),
        RuntimeThreadManagerConfig::for_session(root.path().join("tasks"), "interrupted"),
        Arc::new(crate::plugins::PluginRegistry::empty(root.path())),
        Some(&missing),
    )?;
    assert_eq!(manager.session_store_binding(), recovered);
    let other_recovery = RuntimeThreadManager::open_for_session(
        fixture_config(),
        root.path().into(),
        RuntimeThreadManagerConfig::for_session(root.path().join("tasks"), "other-interrupted"),
        Arc::new(crate::plugins::PluginRegistry::empty(root.path())),
        Some(&missing),
    )?;
    assert_ne!(
        other_recovery.session_store_binding().data_dir,
        recovered.data_dir
    );
    assert_ne!(
        other_recovery.session_store_binding().execution_scope,
        recovered.execution_scope
    );
    saved.metadata.runtime_store = Some(recovered.clone());
    sessions.save_session(&saved)?;
    sessions.save_session(&stale)?;
    sessions.save_checkpoint(&stale)?;
    let durable = sessions.load_session("interrupted")?;
    assert_eq!(durable.metadata.runtime_store, Some(recovered.clone()));
    assert_eq!(durable.messages, stale.messages);
    let competing = RuntimeThreadManager::open_for_session(
        fixture_config(),
        root.path().into(),
        RuntimeThreadManagerConfig::for_session(root.path().join("tasks"), "competing"),
        Arc::new(crate::plugins::PluginRegistry::empty(root.path())),
        None,
    )?;
    let mut competing_snapshot = stale.clone();
    competing_snapshot.metadata.runtime_store = Some(competing.session_store_binding());
    assert!(sessions.save_session(&competing_snapshot).is_err());
    assert!(sessions.save_checkpoint(&competing_snapshot).is_err());
    assert_eq!(
        sessions.load_session("interrupted")?.metadata.runtime_store,
        Some(recovered),
        "another valid owner cannot overwrite the completed recovery"
    );
    Ok(())
}

#[tokio::test]
async fn picker_recovers_missing_store_into_the_idle_host_and_persists_before_returning()
-> anyhow::Result<()> {
    let _environment = crate::test_support::lock_test_env();
    let root = tempfile::tempdir()?;
    let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", root.path());
    let _runtime = crate::test_support::EnvVarGuard::remove("CODEWHALE_RUNTIME_DIR");
    let _legacy = crate::test_support::EnvVarGuard::remove("DEEPSEEK_RUNTIME_DIR");
    let sessions = SessionManager::default_location()?;
    let mut config = fixture_config();
    let mut saved = crate::session_manager::create_saved_session_with_id_and_mode(
        "picker-interrupted".into(),
        &[text_message("user", "retain my work")],
        "deepseek-v4-pro",
        root.path(),
        0,
        None,
        None,
    );
    saved.metadata.runtime_store = Some(crate::runtime_threads::RuntimeStoreBinding {
        data_dir: root.path().join("sessions/previous/runtime"),
        execution_scope: "0".repeat(64),
    });
    sessions.save_session(&saved)?;
    let mut app = Box::new(create_test_app());
    let tasks = TaskManager::start(
        TaskManagerConfig::from_runtime(&config, root.path().into(), None, Some(1)),
        config.clone(),
        app.plugin_registry.clone(),
        "picker-current",
        None,
    )
    .await?;
    let binding = tasks.session_store_binding().expect("current host");
    app.runtime_services.task_manager = Some(tasks.clone());
    app.current_session_id = Some("picker-current".into());
    app.api_messages
        .push(text_message("user", "current conversation"));
    let current_messages = app.api_messages.clone();
    let plan_state = app.plan_state.clone();
    let held = plan_state
        .try_lock()
        .expect("hold Work state during recovery");
    assert!(apply_loaded_session_with_goal(&mut app, &mut config, &saved, None).is_err());
    assert_eq!(app.current_session_id.as_deref(), Some("picker-current"));
    assert_eq!(app.api_messages, current_messages);
    assert_eq!(
        sessions
            .load_session("picker-interrupted")?
            .metadata
            .runtime_store
            .as_ref(),
        Some(&binding),
        "binding repair survives a contended UI restore"
    );
    drop(held);
    apply_loaded_session_with_goal(&mut app, &mut config, &saved, None)
        .map_err(anyhow::Error::msg)?;
    assert_eq!(
        app.current_session_id.as_deref(),
        Some("picker-interrupted")
    );
    let durable = sessions.load_session("picker-interrupted")?;
    assert_eq!(durable.metadata.runtime_store.as_ref(), Some(&binding));
    assert_eq!(durable.messages, saved.messages);
    assert_eq!(
        app.current_session_metadata.as_ref().unwrap().runtime_store,
        Some(binding)
    );
    sessions.save_session(&saved)?;
    assert_eq!(
        sessions
            .load_session("picker-interrupted")?
            .metadata
            .runtime_store,
        durable.metadata.runtime_store
    );
    tasks.shutdown_and_wait().await?;
    Ok(())
}
