//! Non-interactive exec agent assembly: the `run_exec_agent` pipeline
//! that resolves the CLI route, builds the engine configuration, spawns
//! the engine, and drives the exec output stream to completion.
//!
//! Extracted verbatim from `lib.rs` (#5586, the issue's prescribed
//! engine-config-assembly cut). The two functions were crate-private in
//! the root and are `pub(crate)` here purely so the root's glob re-export
//! keeps the dispatch site and tests resolving unchanged.

use super::*;

/// Resolve the headless `exec` model-step ceiling.
///
/// Omission leaves model steps uncapped. Clap rejects `--max-turns 0`;
/// explicit positive values retain the documented finite range.
pub(crate) fn exec_max_steps(max_turns: Option<u32>) -> u32 {
    crate::core::engine::turn_budget::resolve_max_model_steps(max_turns)
}

type ExecSettlementProbe = std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<crate::core::ops::SubAgentSettlement>> + Send>,
>;

/// Read the existing Engine stream through the one-shot host's final boundary.
/// Successful parent receipts remain pending while admitted children or their
/// completion inbox can still produce another normal Engine turn. This owns
/// only the deferred output receipt, never child execution or a turn loop.
pub(crate) struct ExecAgentEvents {
    handle: crate::core::engine::EngineHandle,
    deadline: tokio::time::Instant,
    terminal: Option<crate::core::events::Event>,
    probe: Option<ExecSettlementProbe>,
    next_probe_at: tokio::time::Instant,
    in_flight_usage: codewhale_models::Usage,
}

impl ExecAgentEvents {
    pub(crate) fn new(handle: crate::core::engine::EngineHandle, deadline: Instant) -> Self {
        Self {
            handle,
            deadline: deadline.into(),
            terminal: None,
            probe: None,
            next_probe_at: tokio::time::Instant::now(),
            in_flight_usage: codewhale_models::Usage::default(),
        }
    }

    fn stop_settlement(
        &mut self,
        status: crate::core::events::TurnOutcomeStatus,
        error: String,
    ) -> crate::core::events::Event {
        use crate::core::events::Event;
        self.handle
            .cancel_with_reason(crate::core::engine::CancelReason::External);
        // Cancellation is out of band; shutdown remains in the existing
        // Engine mailbox so it also cancels detached session children.
        let _ = self.handle.try_send(crate::core::ops::Op::Shutdown);
        self.probe = None;
        let mut terminal = self.terminal.take().expect("pending parent receipt");
        if let Event::TurnComplete {
            status: terminal_status,
            error: terminal_error,
            usage,
            parent_route_usage,
            routed_usage_dropped_records,
            ..
        } = &mut terminal
        {
            *terminal_status = status;
            *terminal_error = Some(error);
            crate::core::turn::add_usage_to(usage, &self.in_flight_usage);
            crate::core::turn::add_usage_to(parent_route_usage, &self.in_flight_usage);
            // The host cannot prove usage settlement after abandoning the
            // inbox. Keep reported usage and explicitly mark coverage partial.
            *routed_usage_dropped_records = routed_usage_dropped_records.saturating_add(1);
        }
        self.in_flight_usage = codewhale_models::Usage::default();
        terminal
    }

    pub(crate) async fn next(&mut self) -> Option<crate::core::events::Event> {
        use crate::core::events::{Event, TurnOutcomeStatus};
        // Keep the streamed event on the stack instead of allocating another
        // box for every token merely to equalize the two small control arms.
        #[allow(clippy::large_enum_variant)]
        enum Input {
            Event(Option<Event>),
            Probe(Result<crate::core::ops::SubAgentSettlement>),
            Poll,
        }
        loop {
            if matches!(
                self.terminal,
                Some(Event::TurnComplete { status, .. }) if status != TurnOutcomeStatus::Completed
            ) {
                return self.terminal.take();
            }
            let settling = self.terminal.is_some();
            if settling && self.handle.is_cancelled() {
                return Some(self.stop_settlement(
                    TurnOutcomeStatus::Interrupted,
                    "Headless exec cancelled while settling children; recorded usage is partial."
                        .to_string(),
                ));
            }
            if settling && tokio::time::Instant::now() >= self.deadline {
                return Some(self.stop_settlement(
                    TurnOutcomeStatus::Failed,
                    "Headless exec wall-clock budget exhausted while settling children; recorded usage is partial."
                        .to_string(),
                ));
            }
            if settling && self.probe.is_none() && tokio::time::Instant::now() >= self.next_probe_at
            {
                let handle = self.handle.clone();
                self.probe = Some(Box::pin(
                    async move { handle.get_subagent_settlement().await },
                ));
            }
            let probing = self.probe.is_some();
            let wake_at = self.deadline.min(if probing {
                tokio::time::Instant::now() + Duration::from_millis(250)
            } else {
                self.next_probe_at
            });
            let input = {
                let mut events = self.handle.rx_event.write().await;
                tokio::select! {
                    biased;
                    // Drain queued SessionUpdated/TurnComplete events before
                    // accepting the later actor-owned idle receipt.
                    event = events.recv() => Input::Event(event),
                    result = async { self.probe.as_mut().expect("active probe").await }, if probing => Input::Probe(result),
                    () = tokio::time::sleep_until(wake_at), if settling => Input::Poll,
                }
            };
            match input {
                Input::Poll => {}
                Input::Probe(Ok(snapshot)) if snapshot.is_settled() => {
                    self.probe = None;
                    return self.terminal.take();
                }
                Input::Probe(Ok(_)) => {
                    self.probe = None;
                    self.next_probe_at = tokio::time::Instant::now() + Duration::from_millis(250);
                }
                Input::Probe(Err(error)) => {
                    return Some(self.stop_settlement(
                        TurnOutcomeStatus::Failed,
                        format!(
                            "Cannot verify child settlement: {error}; recorded usage is partial."
                        ),
                    ));
                }
                Input::Event(None) if settling => {
                    return Some(self.stop_settlement(
                        TurnOutcomeStatus::Failed,
                        "Engine event channel closed before child settlement; recorded usage is partial."
                            .to_string(),
                    ));
                }
                Input::Event(None) => return None,
                Input::Event(Some(mut event)) => {
                    match &mut event {
                        Event::TurnComplete {
                            usage,
                            parent_route_usage,
                            routed_usage_dropped_records,
                            status,
                            error,
                            ..
                        } => {
                            if let Some(Event::TurnComplete {
                                usage: prior_usage,
                                parent_route_usage: prior_parent_usage,
                                routed_usage_dropped_records: prior_dropped,
                                ..
                            }) = self.terminal.take()
                            {
                                crate::core::turn::add_usage_to(usage, &prior_usage);
                                crate::core::turn::add_usage_to(
                                    parent_route_usage,
                                    &prior_parent_usage,
                                );
                                *routed_usage_dropped_records =
                                    routed_usage_dropped_records.saturating_add(prior_dropped);
                            }
                            self.in_flight_usage = codewhale_models::Usage::default();
                            if *status == TurnOutcomeStatus::Completed && error.is_none() {
                                self.terminal = Some(event);
                                self.next_probe_at = tokio::time::Instant::now();
                                continue;
                            }
                        }
                        Event::TurnUsage { usage, .. } => {
                            crate::core::turn::add_usage_to(&mut self.in_flight_usage, usage);
                        }
                        Event::Error { envelope, .. }
                            if settling && exec_error_event_is_fatal(envelope) =>
                        {
                            let terminal = self.stop_settlement(
                                TurnOutcomeStatus::Failed,
                                format!(
                                    "{}; child settlement stopped and recorded usage is partial.",
                                    envelope.message
                                ),
                            );
                            self.terminal = Some(terminal);
                        }
                        _ => {}
                    }
                    return Some(event);
                }
            }
        }
    }
}

/// Attach the durable automation store headless `exec` inspects.
///
/// Headless exec builds its catalog from the same tool surface the TUI and the
/// Runtime host do, so it advertises `automation` and `send_later` whether or
/// not the store behind them is attached. Left unattached, every call failed
/// "AutomationManager is not attached" — the tool was real and the service was
/// missing. This opens the same store those two hosts open: a shared directory
/// guarded per transaction by its own file locks (`AutomationManager::open`),
/// so it adds no second store, no scheduler, and no second scheduling
/// authority.
///
/// What exec deliberately does not take is the Runtime's task-execution lease.
/// That lease is exclusive (`TaskExecutionLease::new`) and a one-shot host must
/// neither contend with it nor recover work from the process that holds it. So
/// the manager returned here is *unbound*: inspection works, and everything
/// that would promise dispatch is refused by the tool's own admission check
/// (`tools::automation::require_dispatch_owner`) rather than persisting a
/// schedule nothing would honor.
///
/// Fleet worker subprocesses get nothing, keeping the narrowed envelope they
/// were launched with alongside the empty plugin registry and disabled
/// subagents. A store that cannot be opened is reported, never swallowed:
/// both other hosts fail startup on it, so exec does too instead of
/// advertising an automation surface it silently cannot serve.
pub(crate) fn exec_automation_services(
    fleet_authority_active: bool,
) -> Result<Option<crate::automation_manager::SharedAutomationManager>> {
    if fleet_authority_active {
        return Ok(None);
    }
    let service = crate::automation_manager::AutomationManager::default_location()
        .context("open the automation store for headless exec")?;
    Ok(Some(std::sync::Arc::new(tokio::sync::Mutex::new(service))))
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_exec_agent(
    config: &Config,
    model: &str,
    prompt: &str,
    workspace: PathBuf,
    max_subagents: usize,
    auto_approve: bool,
    allow_sandbox_elevation: bool,
    explicit_sandbox: Option<&str>,
    trust_mode: bool,
    json_output: bool,
    resume_session: Option<session_manager::SavedSession>,
    force_configured_route: bool,
    output_format: ExecOutputFormat,
    max_turns: u32,
    max_tool_calls: Option<u32>,
    allowed_tools: Option<Vec<String>>,
    disallowed_tools: Option<Vec<String>>,
    append_system_prompt: Option<String>,
    tool_authority_json: Option<String>,
    plugin_registry: std::sync::Arc<crate::plugins::PluginRegistry>,
) -> Result<()> {
    use crate::compaction::CompactionConfig;
    use crate::core::engine::{EngineConfig, spawn_engine};
    use crate::core::events::Event;
    use crate::core::ops::Op;
    use crate::tools::plan::new_shared_plan_state;
    use crate::tools::todo::new_shared_todo_list;
    use codewhale_config::AppMode;
    use codewhale_execpolicy::ApprovalMode;

    // Headless exec registers the model-facing notify tool too. Project the
    // final merged config before tool setup so `off`, quiet/category gates,
    // and explicit `always` are truthful outside the interactive TUI. With no
    // focus-reporting channel, fail closed to focused; only explicit `always`
    // may authorize a headless desktop notification.
    crate::tui::notifications::set_terminal_focused(true);
    let _ = crate::tui::notifications::settings(config);

    validate_exec_tool_authority_resume(tool_authority_json.as_deref(), resume_session.is_some())?;
    let fleet_authority = tool_authority_json
        .as_deref()
        .map(crate::tools::spec::ToolAuthorityEnvelope::from_json)
        .transpose()
        .map_err(anyhow::Error::msg)?;
    let fleet_authority_active = fleet_authority.is_some();
    let outer_network_access = fleet_authority
        .as_ref()
        .and_then(|authority| authority.network_access);
    let outer_shell_authority = fleet_authority
        .as_ref()
        .map(|authority| authority.shell)
        .unwrap_or_default();
    if let Some(envelope) = fleet_authority {
        crate::tools::spec::install_process_tool_authority(envelope).map_err(anyhow::Error::msg)?;
    }

    let fleet_capture = match (
        std::env::var("CODEWHALE_FLEET_CAPTURE_ID").ok(),
        std::env::var_os("CODEWHALE_FLEET_CAPTURE_DIR"),
    ) {
        (Some(id), Some(dir)) => {
            uuid::Uuid::parse_str(&id).context("invalid Fleet session capture id")?;
            anyhow::ensure!(
                resume_session.is_none(),
                "Fleet capture cannot resume a session"
            );
            let manager = SessionManager::new(PathBuf::from(dir))?;
            match manager.load_session(&id) {
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                _ => anyhow::bail!("Fleet session capture id is already in use or unavailable"),
            }
            Some((id, manager))
        }
        (None, None) => None,
        _ => anyhow::bail!("incomplete Fleet session capture destination"),
    };

    let route = resolve_cli_exec_route(config, model, prompt, force_configured_route).await?;
    let execution_config = config_for_cli_route(config, &route);
    let auto_model = route.auto_model;
    let effective_provider = route.provider;
    let effective_model = route.model;
    let validated_route = crate::route_runtime::resolve_runtime_route(
        &execution_config,
        effective_provider,
        Some(&effective_model),
    )
    .map_err(anyhow::Error::msg)?
    .validate()
    .map_err(anyhow::Error::msg)?;
    let effective_provider_name = validated_route.identity.key.clone();
    let effective_provider_id = validated_route.identity.exact_id.clone();
    let (effective_provider_kind, effective_stream_provider_id) =
        exec_stream_provider_route(&validated_route.identity);
    let route_source = if auto_model {
        "auto_resolver"
    } else {
        "explicit_or_configured"
    }
    .to_string();
    let exec_started = Instant::now();
    let prompt_sha256 = format!("sha256:{}", crate::hashing::sha256_hex(prompt.as_bytes()));
    let binary_sha256 = current_binary_sha256();
    let approval_posture = if auto_approve { "auto_tools" } else { "ask" }.to_string();
    let sandbox_posture = explicit_sandbox.unwrap_or("configured_default").to_string();
    let active_route_limits =
        crate::route_budget::known_route_limits(validated_route.candidate.limits());
    let max_subagents = if max_subagents == config.max_subagents_for_provider(config.api_provider())
    {
        execution_config
            .max_subagents_for_provider(effective_provider)
            .clamp(1, MAX_SUBAGENTS)
    } else {
        max_subagents
    };
    // A FIXED model with `--reasoning-effort auto` (the exact shape a Fleet
    // worker subprocess launches with: `--model <exact> --reasoning-effort
    // auto`) is still Auto. `auto_model` is a *model* decision and is false
    // here, so deriving the auto flag from it left this path both raw and
    // non-auto: the literal string `"auto"` travelled to the engine while the
    // receipt claimed no Auto was in play.
    let reasoning_effort_auto = route.auto_controls_reasoning;
    // Resolve Auto against this run's prompt at the CLI boundary, exactly like
    // `run_one_shot`/`run_one_shot_json` and the interactive launch path do,
    // so the tier the engine (and the receipt below) sees is concrete.
    let effective_reasoning_effort = route.reasoning_effort.and_then(|effort| {
        cli_reasoning_effort_value_for_prompt(&execution_config, &effective_model, effort, prompt)
    });

    let settings = crate::settings::Settings::load().unwrap_or_default();
    let auto_compact_enabled = if crate::settings::Settings::auto_compact_explicitly_configured() {
        settings.auto_compact
    } else {
        crate::route_budget::auto_compact_default_for_route(
            effective_provider,
            &effective_model,
            active_route_limits,
        )
    };
    let compaction = CompactionConfig {
        enabled: auto_compact_enabled,
        model: effective_model.clone(),
        effective_context_window: Some(crate::route_budget::route_context_window_tokens(
            effective_provider,
            &effective_model,
            active_route_limits,
        )),
        token_threshold: crate::route_budget::compaction_threshold_for_route_at_percent(
            effective_provider,
            &effective_model,
            active_route_limits,
            settings.auto_compact_threshold_percent,
        ),
        summary_instructions: execution_config.compaction_summary_instructions(),
        retained_user_message_tokens: execution_config.compaction_retained_user_message_tokens(),
        ..Default::default()
    };

    let network_policy = exec_network_policy(&execution_config, outer_network_access);

    let lsp_config = (!fleet_authority_active)
        .then(|| {
            execution_config
                .lsp
                .clone()
                .map(crate::config::LspConfigToml::into_runtime)
        })
        .flatten();
    let mut engine_features = execution_config.features();
    apply_fleet_engine_feature_caps(
        &mut engine_features,
        fleet_authority_active,
        outer_network_access,
        outer_shell_authority,
    );
    if crate::core::allowlist_is_native_file_and_shell_only(allowed_tools.as_deref()) {
        engine_features.disable(crate::features::Feature::Mcp);
    }
    let engine_plugin_registry = if fleet_authority_active {
        std::sync::Arc::new(crate::plugins::PluginRegistry::empty(&workspace))
    } else {
        plugin_registry
    };
    let exec_allow_shell = crate::tools::spec::fleet_exec_shell_enabled(
        fleet_authority_active,
        outer_shell_authority,
        disallowed_tools.as_deref(),
    ) || (!fleet_authority_active
        && (auto_approve || execution_config.allow_shell()));
    let persist_services_enabled = cfg!(unix)
        && !fleet_authority_active
        && exec_allow_shell
        && explicit_sandbox
            .is_some_and(|sandbox| sandbox.eq_ignore_ascii_case("danger-full-access"));
    let exec_shell_manager = crate::tools::shell::new_shared_shell_manager(workspace.clone());
    let exec_automations = exec_automation_services(fleet_authority_active)?;
    let runtime_services = crate::tools::spec::RuntimeToolServices {
        shell_manager: Some(exec_shell_manager.clone()),
        persist_services_enabled,
        automations: exec_automations,
        media_originals_dir: crate::media_originals::default_store_dir(),
        ..crate::tools::spec::RuntimeToolServices::default()
    };

    let engine_config = EngineConfig {
        model: effective_model.clone(),
        active_route_limits,
        workspace: workspace.clone(),
        session_id: fleet_capture.as_ref().map(|(id, _)| id.clone()),
        subagent_state_root: None,
        plugin_registry: Some(std::sync::Arc::clone(&engine_plugin_registry)),
        allow_shell: exec_allow_shell,
        trust_mode,
        notes_path: execution_config.notes_path(),
        mcp_config_path: execution_config.mcp_config_path(),
        // Non-interactive exec has no user-level MCP OAuth callback
        // overrides; the loopback default applies.
        mcp_oauth_callback_port: None,
        mcp_oauth_callback_url: None,
        skills_dir: execution_config.skills_dir(),
        skills_scan_codewhale_only: execution_config.skills_config().scan_codewhale_only(),
        instructions: {
            let mut instrs: Vec<crate::prompts::InstructionSource> = execution_config
                .instructions_paths()
                .into_iter()
                .map(Into::into)
                .collect();
            if let Some(ref extra) = append_system_prompt {
                instrs.push(crate::prompts::InstructionSource::Inline {
                    name: "cli:append-system-prompt".into(),
                    content: extra.clone(),
                });
            }
            instrs
        },
        project_context_pack_enabled: execution_config.project_context_pack_enabled(),
        translation_enabled: false,
        max_steps: max_turns,
        max_subagents,
        max_admitted_subagents: execution_config
            .max_admitted_subagents_for_provider(effective_provider)
            .max(max_subagents),
        launch_concurrency: execution_config.launch_concurrency_for_provider(effective_provider),
        subagents_enabled: !fleet_authority_active
            && execution_config.subagents_enabled_for_provider(effective_provider),
        features: engine_features,
        auto_review_policy: execution_config.auto_review_policy(),
        compaction: compaction.clone(),
        todos: new_shared_todo_list(),
        plan_state: new_shared_plan_state(),
        goal_state: crate::tools::goal::new_shared_goal_state(),
        max_spawn_depth: if fleet_authority_active {
            0
        } else {
            execution_config.subagent_max_spawn_depth_for_provider(effective_provider)
        },
        subagent_token_budget: execution_config
            .subagent_token_budget_for_provider(effective_provider),
        network_policy,
        snapshots_enabled: !fleet_authority_active && execution_config.snapshots_config().enabled,
        snapshots_max_workspace_bytes: execution_config
            .snapshots_config()
            .max_workspace_gb
            .saturating_mul(1024 * 1024 * 1024),
        lsp_config,
        runtime_services,
        subagent_model_overrides: execution_config.subagent_model_overrides(),
        fleet_roster: std::sync::Arc::new(crate::fleet::identity::load_effective_roster(
            &execution_config.fleet_config(),
            &workspace,
            Some(engine_plugin_registry.as_ref()),
        )),
        subagent_api_timeout: std::time::Duration::from_secs(
            execution_config.subagent_api_timeout_secs_for_provider(effective_provider),
        ),
        stream_chunk_timeout: std::time::Duration::from_secs(
            execution_config.stream_chunk_timeout_secs(),
        ),
        turn_wall_clock: execution_config.turn_wall_clock(),
        stream_max_content_bytes: execution_config.stream_max_content_bytes(),
        stream_max_duration: execution_config.stream_max_duration(),
        subagent_heartbeat_timeout: std::time::Duration::from_secs(
            execution_config.subagent_heartbeat_timeout_secs_for_provider(effective_provider),
        ),
        prefer_bwrap: execution_config.prefer_bwrap.unwrap_or(false),
        bwrap_extensions: crate::sandbox::BwrapMountExtensions {
            read_only_roots: execution_config.bwrap_ro_roots.clone(),
            device_roots: execution_config.bwrap_dev_roots.clone(),
        },
        read_denylist: execution_config.read_denylist(),
        memory_enabled: execution_config.memory_enabled(),
        memory_path: execution_config.memory_path(),
        speech_output_dir: execution_config.speech_output_dir(),
        vision_config: execution_config.vision_model_config(),
        strict_tool_mode: execution_config.strict_tool_mode.unwrap_or(false),
        goal_objective: None,
        goal_token_budget: None,
        goal_status: crate::tools::goal::GoalStatus::Active,
        goal_max_continuations: execution_config.goal_max_continuations(),
        goal_continuation_delay_seconds: execution_config.goal_continuation_delay_seconds(),
        reasoning_only_max_reprompts: execution_config.reasoning_only_max_reprompts(),
        reasoning_only_reprompt_message: Some(
            execution_config
                .reasoning_only_reprompt_message()
                .to_string(),
        ),
        allowed_tools: allowed_tools.clone(),
        disallowed_tools: disallowed_tools.clone(),
        max_tool_calls,
        hook_executor: None,
        locale_tag: codewhale_localization::resolve_locale(&settings.locale)
            .tag()
            .to_string(),
        workshop: {
            crate::tools::large_output_router::WorkshopConfig::install_active(
                config.workshop.as_ref(),
            );
            config.workshop.clone()
        },
        search_provider: execution_config.search_provider(),
        search_api_key: execution_config
            .search
            .as_ref()
            .and_then(|s| s.api_key.clone()),
        search_base_url: execution_config
            .search
            .as_ref()
            .and_then(|s| s.base_url.clone()),
        tools_always_load: if fleet_authority_active {
            std::collections::HashSet::new()
        } else {
            execution_config.tools_always_load()
        },
        user_input_limits: execution_config.user_input_limits(),
        user_input_timeout: execution_config.user_input_timeout(),
        goal_max_steps: None,
        tools: if fleet_authority_active {
            None
        } else {
            execution_config.tools.clone()
        },
        verbosity: execution_config.verbosity.clone(),
        workspace_follow_symlinks: settings.workspace_follow_symlinks,
        exec_policy_engine: execution_config.exec_policy_engine.clone(),
        terminal_chrome_enabled: false,
        advisor_config: execution_config
            .advisor
            .as_ref()
            .map(crate::tools::subagent::AdvisorConfig::from_toml)
            .unwrap_or_else(crate::tools::subagent::AdvisorConfig::disabled),
    };

    let engine_handle = spawn_engine(engine_config, &execution_config);
    // The Full Access posture travels in the op's auto_approve/approval_mode
    // fields; modes no longer carry permission.
    let mode = AppMode::Agent;

    let resuming_session = resume_session.is_some();
    let mut loaded_session_id = None;
    if let Some(saved) = resume_session {
        let saved_id = saved.metadata.id.clone();
        if saved.metadata.workspace != workspace && output_format == ExecOutputFormat::Text {
            eprintln!(
                "Warning: session {} was created in a different workspace ({}). Resuming anyway.",
                truncate_id(&saved_id),
                saved.metadata.workspace.display(),
            );
        }

        engine_handle
            .send(Op::SyncSession {
                session_id: Some(saved_id.clone()),
                messages: saved.messages,
                system_prompt: saved.system_prompt.map(SystemPrompt::Text),
                system_prompt_override: false,
                model: saved.metadata.model,
                workspace: saved.metadata.workspace,
                mode,
            })
            .await?;
        loaded_session_id = Some(saved_id.clone());
        if output_format == ExecOutputFormat::Text && !json_output {
            eprintln!("{}", exec_resumed_session_line(&saved_id));
        }
    }

    // Lifecycle outbox (`[lifecycle_outbox]`): headless `codewhale exec`
    // gets the same turn boundaries as the interactive TUI. Disabled
    // (all emits no-op) when the config has no path.
    let lifecycle_outbox = config
        .lifecycle_outbox
        .as_ref()
        .map(|outbox| {
            codewhale_hooks::LifecycleOutbox::new(
                outbox.path.clone(),
                outbox.webhook_url.clone(),
                outbox.webhook_token.clone(),
            )
        })
        .unwrap_or_else(codewhale_hooks::LifecycleOutbox::disabled);
    // Wall clock for the outbox `turn_end` duration. `exec` never receives
    // a TurnStarted engine event, so the start is marked at the same
    // `Op::SendMessage` boundary where `turn_start` is emitted below.
    let exec_turn_started_at = Instant::now();

    engine_handle
        .send(Op::SendMessage {
            max_output_tokens: None,
            content: prompt.to_string(),
            images: Vec::new(),
            mode,
            route: Box::new(validated_route.into_resolved()),
            compaction: Box::new(compaction.clone()),
            initial_routed_usage: Box::default(),
            goal_objective: None,
            goal_token_budget: None,
            goal_status: crate::tools::goal::GoalStatus::Active,
            allowed_tools: allowed_tools.clone(),
            dynamic_tools: Vec::new(),
            hook_executor: None,
            reasoning_effort: effective_reasoning_effort,
            reasoning_effort_auto,
            auto_model,
            allow_shell: auto_approve || execution_config.allow_shell(),
            trust_mode,
            auto_approve,
            translation_enabled: false,
            approval_mode: if auto_approve {
                ApprovalMode::Bypass
            } else {
                execution_config
                    .approval_policy
                    .as_deref()
                    .and_then(ApprovalMode::from_config_value)
                    .unwrap_or_default()
            },
            verbosity: execution_config.verbosity.clone(),
            provenance: crate::core::ops::UserInputProvenance::ExternalUser,
        })
        .await?;

    // Lifecycle outbox: the clean headless turn-start boundary. `exec` has
    // no TurnStarted engine event; the message submission above is exactly
    // where the engine begins the turn. No-op when the feature is disabled.
    lifecycle_outbox.emit(codewhale_hooks::LifecycleEvent {
        event: "turn_start".to_string(),
        kind: "turn.started".to_string(),
        thread_id: loaded_session_id.clone().unwrap_or_default(),
        turn_id: None,
        item_id: None,
        payload: serde_json::json!({
            "model": codewhale_hooks::bounded_text(
                &effective_model,
                codewhale_hooks::OUTBOX_DETAIL_MAX_CHARS,
            ),
            "workspace": workspace.display().to_string(),
        }),
    });

    let mut summary = ExecSummary {
        mode: "agent".to_string(),
        provider: effective_provider_name.clone(),
        model: effective_model.clone(),
        prompt: prompt.to_string(),
        ..ExecSummary::default()
    };
    let can_elevate_sandbox =
        exec_sandbox_elevation_authorized(allow_sandbox_elevation, explicit_sandbox);
    let mut sandbox_denied = false;
    let mut approval_required = false;
    let mut tool_error_seen = false;
    let mut last_error_category = None;
    let mut reported_sandbox_contract = false;

    let should_persist_session = resuming_session || output_format == ExecOutputFormat::StreamJson;
    let mut latest_session_id = loaded_session_id;
    let mut latest_messages: Vec<Message> = Vec::new();
    let mut latest_system_prompt: Option<SystemPrompt> = None;
    let mut latest_model = effective_model;
    let mut latest_workspace = workspace.clone();
    let mut tool_starts: HashMap<String, (Instant, String)> = HashMap::new();
    let mut turn_usage_seq: u32 = 0;

    let mut stdout = io::stdout();
    let mut ends_with_newline = false;
    // One absolute host deadline includes every autonomous child fan-in turn;
    // child-specific shorter deadlines remain enforced by their runtime.
    let mut events = ExecAgentEvents::new(
        engine_handle.clone(),
        exec_turn_started_at + execution_config.turn_wall_clock(),
    );
    loop {
        let Some(event) = events.next().await else {
            break;
        };

        match event {
            Event::MessageDelta { content, .. } => {
                summary.output.push_str(&content);
                if output_format == ExecOutputFormat::StreamJson {
                    emit_exec_stream_event(&ExecStreamEvent::Content { content })?;
                } else if !json_output {
                    print!("{content}");
                    stdout.flush()?;
                }
                ends_with_newline = summary.output.ends_with('\n');
            }
            Event::MessageComplete { .. }
                if output_format == ExecOutputFormat::Text
                    && !json_output
                    && !ends_with_newline =>
            {
                println!();
            }
            Event::ThinkingDelta { .. } => {
                // Exec stream-json intentionally omits reasoning deltas; the
                // TUI transcript retains its existing Activity Detail surface.
            }
            Event::ToolProjectionWarning {
                provider,
                omitted_tool_names,
                omitted_tool_count,
            } if !json_output => {
                eprintln!(
                    "{}",
                    crate::core::events::tool_projection_warning_message(
                        &provider,
                        &omitted_tool_names,
                        omitted_tool_count,
                    )
                );
            }
            Event::ToolCallStarted { id, name, input } => {
                let started_at = chrono::Utc::now().to_rfc3339();
                tool_starts.insert(id.clone(), (Instant::now(), started_at.clone()));
                if output_format == ExecOutputFormat::StreamJson {
                    emit_exec_stream_event(&ExecStreamEvent::ToolUse {
                        name,
                        id,
                        input,
                        started_at,
                    })?;
                } else if !json_output {
                    let summary = summarize_tool_args(&input);
                    if let Some(summary) = summary {
                        eprintln!("tool: {name} ({summary})");
                    } else {
                        eprintln!("tool: {name}");
                    }
                }
            }
            Event::ToolCallComplete {
                id, name, result, ..
            } => {
                let (duration_ms, started_at) = tool_starts
                    .remove(&id)
                    .map(|(started, timestamp)| {
                        (
                            u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
                            timestamp,
                        )
                    })
                    .unwrap_or_else(|| (0, chrono::Utc::now().to_rfc3339()));
                let receipt_name = name.clone();
                match result {
                    Ok(output) => {
                        tool_error_seen |= !output.success;
                        summary.tools.push(ExecToolEntry {
                            name: name.clone(),
                            success: output.success,
                            output: output.content.clone(),
                        });
                        if output_format == ExecOutputFormat::StreamJson {
                            emit_exec_stream_event(&ExecStreamEvent::ToolResult {
                                id,
                                name: receipt_name,
                                output: output.content,
                                status: if output.success {
                                    "success".to_string()
                                } else {
                                    "error".to_string()
                                },
                                started_at,
                                completed_at: chrono::Utc::now().to_rfc3339(),
                                duration_ms,
                                side_effect_status: output
                                    .metadata
                                    .as_ref()
                                    .and_then(|metadata| metadata.get("side_effect_status"))
                                    .and_then(serde_json::Value::as_str)
                                    .unwrap_or("unknown")
                                    .to_string(),
                                error_category: (!output.success).then(|| {
                                    output
                                        .metadata
                                        .as_ref()
                                        .and_then(|metadata| metadata.get("error_category"))
                                        .and_then(serde_json::Value::as_str)
                                        .unwrap_or("tool_reported_failure")
                                        .to_string()
                                }),
                                truncated: output
                                    .metadata
                                    .as_ref()
                                    .and_then(|metadata| metadata.get("truncated"))
                                    .and_then(serde_json::Value::as_bool),
                                artifact: tool_artifact_receipt(output.metadata.as_ref()),
                                result_metadata: output.metadata,
                            })?;
                        } else if !json_output {
                            if name == "exec_shell" && !output.content.trim().is_empty() {
                                eprintln!("tool {name} completed");
                                eprintln!(
                                    "--- stdout/stderr ---\n{}\n---------------------",
                                    output.content
                                );
                            } else {
                                eprintln!(
                                    "tool {name} completed: {}",
                                    summarize_tool_output(&output.content)
                                );
                            }
                        }
                    }
                    Err(err) => {
                        tool_error_seen = true;
                        let error_text = err.to_string();
                        summary.tools.push(ExecToolEntry {
                            name: name.clone(),
                            success: false,
                            output: error_text.clone(),
                        });
                        if output_format == ExecOutputFormat::StreamJson {
                            emit_exec_stream_event(&ExecStreamEvent::ToolResult {
                                id,
                                name: receipt_name,
                                output: error_text,
                                status: "error".to_string(),
                                started_at,
                                completed_at: chrono::Utc::now().to_rfc3339(),
                                duration_ms,
                                side_effect_status: "not_started_or_unknown".to_string(),
                                error_category: Some(tool_error_receipt_category(&err).to_string()),
                                truncated: None,
                                artifact: None,
                                result_metadata: None,
                            })?;
                        } else if !json_output {
                            eprintln!("tool {name} failed: {err}");
                        }
                    }
                }
            }
            Event::AgentSpawned { id, prompt, .. }
                if output_format == ExecOutputFormat::Text && !json_output =>
            {
                eprintln!("sub-agent {id} spawned: {}", summarize_tool_output(&prompt));
            }
            Event::AgentProgress { id, status, .. }
                if output_format == ExecOutputFormat::Text && !json_output =>
            {
                eprintln!("sub-agent {id}: {status}");
            }
            Event::AgentComplete {
                id,
                result,
                outcome,
                ..
            } if output_format == ExecOutputFormat::Text && !json_output => {
                eprintln!(
                    "sub-agent {id} {}: {}",
                    outcome
                        .as_ref()
                        .map(crate::tools::subagent::subagent_status_name)
                        .unwrap_or("settled (outcome unconfirmed)"),
                    summarize_tool_output(&result)
                );
            }
            Event::AgentSpawned {
                id,
                parent_run_id,
                spawn_depth,
                model,
                route_source,
                ..
            } if output_format == ExecOutputFormat::StreamJson => {
                emit_exec_stream_event(&ExecStreamEvent::AgentSpawned {
                    id,
                    model,
                    spawn_depth,
                    parent_run_id,
                    route_source,
                })?;
            }
            Event::AgentSpawned { .. }
            | Event::AgentProgress { .. }
            | Event::AgentComplete { .. } => {}
            Event::WorkflowUi { run_id, event, .. }
                if output_format == ExecOutputFormat::StreamJson =>
            {
                emit_exec_stream_event(&ExecStreamEvent::WorkflowEvent { run_id, event })?;
            }
            Event::ApprovalRequired { id, .. } => {
                if auto_approve {
                    let _ = engine_handle.approve_tool_call(id).await;
                } else {
                    approval_required = true;
                    let _ = engine_handle.deny_tool_call(id).await;
                }
            }
            Event::ElevationRequired {
                tool_id,
                tool_name,
                denial_reason,
                ..
            } => {
                if can_elevate_sandbox {
                    let policy = crate::sandbox::SandboxPolicy::DangerFullAccess;
                    let _ = engine_handle.retry_tool_with_policy(tool_id, policy).await;
                } else {
                    sandbox_denied = true;
                    approval_required = true;
                    summary.outcomes.push(ExecOutcome {
                        kind: "sandbox_denied".to_string(),
                        outcome: "approval_required".to_string(),
                        tool_name: tool_name.clone(),
                        reason: denial_reason.clone(),
                    });
                    if !reported_sandbox_contract {
                        eprintln!(
                            "sandbox denied {tool_name}: {denial_reason}; --auto approves tools but does not elevate sandbox access — use --sandbox danger-full-access or --allow-sandbox-elevation to opt in"
                        );
                        reported_sandbox_contract = true;
                    }
                    if output_format == ExecOutputFormat::StreamJson {
                        emit_exec_stream_event(&ExecStreamEvent::SandboxDenied {
                            tool_id: tool_id.clone(),
                            tool_name,
                            reason: denial_reason,
                            outcome: "approval_required".to_string(),
                        })?;
                    }
                    let _ = engine_handle.deny_tool_call(tool_id).await;
                }
            }
            Event::Error {
                envelope,
                recoverable: _,
            } => {
                // Only a non-recoverable envelope may force the run summary
                // into failure. Recoverable warnings (stream-stall notices,
                // transient retry noise) are still streamed for visibility,
                // but the terminal TurnComplete event carries the
                // authoritative turn outcome — letting a warning set
                // `summary.error` here would exit an otherwise-successful
                // `exec` run non-zero.
                if exec_error_event_is_fatal(&envelope) {
                    last_error_category = Some(envelope.category);
                    summary.error_category = Some(envelope.category.to_string());
                    summary.error = Some(envelope.message.clone());
                }
                if output_format == ExecOutputFormat::StreamJson {
                    emit_exec_stream_event(&ExecStreamEvent::Error {
                        error: envelope.message,
                    })?;
                } else if !json_output {
                    eprintln!("error: {}", envelope.message);
                }
            }
            Event::TurnUsage {
                usage, duration_ms, ..
            } => {
                if output_format == ExecOutputFormat::StreamJson {
                    turn_usage_seq = turn_usage_seq.saturating_add(1);
                    emit_exec_stream_event(&ExecStreamEvent::TurnUsage {
                        turn: turn_usage_seq,
                        input_tokens: usage.input_tokens,
                        output_tokens: usage.output_tokens,
                        reasoning_tokens: usage.reasoning_tokens,
                        prompt_cache_hit_tokens: usage.prompt_cache_hit_tokens,
                        prompt_cache_miss_tokens: usage.prompt_cache_miss_tokens,
                        prompt_cache_write_tokens: usage.prompt_cache_write_tokens,
                        reasoning_replay_tokens: usage.reasoning_replay_tokens,
                        duration_ms,
                    })?;
                }
            }
            Event::TurnComplete {
                status,
                error,
                usage,
                tool_catalog,
                ..
            } => {
                let (terminal_status, terminal_error) = (status, error);
                #[cfg(unix)]
                let (mut terminal_status, mut terminal_error) = (terminal_status, terminal_error);
                if matches!(
                    terminal_status,
                    crate::core::events::TurnOutcomeStatus::Completed
                ) && terminal_error.is_none()
                {
                    #[cfg(unix)]
                    match exec_shell_manager.lock() {
                        Ok(mut manager) => match manager.commit_persistent_services() {
                            Ok(receipts) => {
                                for receipt in &receipts {
                                    if output_format == ExecOutputFormat::StreamJson {
                                        emit_exec_stream_event(
                                            &ExecStreamEvent::ServiceReleased {
                                                task_id: receipt.task_id.clone(),
                                                pid: receipt.pid,
                                                process_group_id: receipt.process_group_id,
                                                ownership: receipt.ownership.clone(),
                                            },
                                        )?;
                                    } else if !json_output {
                                        eprintln!(
                                            "persistent service released: {} pid={} pgid={} ownership={}",
                                            receipt.task_id,
                                            receipt.pid,
                                            receipt.process_group_id,
                                            receipt.ownership
                                        );
                                    }
                                }
                                summary.released_services.extend(receipts);
                            }
                            Err(error) => {
                                manager.abort_persistent_services();
                                terminal_status = crate::core::events::TurnOutcomeStatus::Failed;
                                terminal_error = Some(format!(
                                    "Persistent service ownership transfer failed: {error}"
                                ));
                            }
                        },
                        Err(_) => {
                            terminal_status = crate::core::events::TurnOutcomeStatus::Failed;
                            terminal_error = Some(
                                "Persistent service ownership transfer failed: shell manager lock poisoned"
                                    .to_string(),
                            );
                        }
                    }
                } else if let Ok(mut manager) = exec_shell_manager.lock() {
                    manager.abort_persistent_services();
                }
                summary.status = Some(format!("{terminal_status:?}").to_lowercase());
                if terminal_error.is_some() {
                    summary.error = terminal_error;
                }
                if sandbox_denied
                    && summary.error.is_none()
                    && matches!(
                        terminal_status,
                        crate::core::events::TurnOutcomeStatus::Failed
                    )
                {
                    summary.error = Some(
                        "exec turn failed after sandbox denial; explicit sandbox elevation was not authorized"
                            .to_string(),
                    );
                }
                // Lifecycle outbox: the clean headless turn-end boundary.
                // `terminal_status` is authoritative here — persistent-service
                // handoff failures above already demoted it to Failed, and
                // `summary.error` includes the sandbox-denial augmentation.
                // No-op when the feature is disabled.
                {
                    let outbox_status = format!("{terminal_status:?}").to_lowercase();
                    let kind = match terminal_status {
                        crate::core::events::TurnOutcomeStatus::Completed => "turn.completed",
                        crate::core::events::TurnOutcomeStatus::Failed => "turn.failed",
                        crate::core::events::TurnOutcomeStatus::Interrupted => "turn.interrupted",
                    };
                    lifecycle_outbox.emit(codewhale_hooks::LifecycleEvent {
                        event: "turn_end".to_string(),
                        kind: kind.to_string(),
                        thread_id: latest_session_id.clone().unwrap_or_default(),
                        turn_id: None,
                        item_id: None,
                        payload: serde_json::json!({
                            "status": outbox_status,
                            "duration_ms": exec_turn_started_at.elapsed().as_millis() as u64,
                            "workspace": latest_workspace.display().to_string(),
                            "error": summary.error.as_deref().map(|message| {
                                codewhale_hooks::bounded_text(
                                    message,
                                    codewhale_hooks::OUTBOX_DETAIL_MAX_CHARS,
                                )
                            }),
                        }),
                    });
                }
                if last_error_category.is_none() {
                    last_error_category = summary
                        .error
                        .as_deref()
                        .map(crate::error_taxonomy::classify_error_message);
                    summary.error_category =
                        last_error_category.map(|category| category.to_string());
                }
                let termination_reason = crate::core::termination::classify_turn_termination(
                    terminal_status,
                    last_error_category,
                    tool_error_seen,
                    approval_required,
                );
                summary.termination_reason = Some(termination_reason.as_str().to_string());
                // State the exit class here rather than inferring it later
                // from the process exit code: `Canceled` exits 130, the same
                // value the SIGINT path uses, so a code-based derivation would
                // report every Esc-cancelled turn as a signal. A no-op unless
                // this process was armed.
                if !termination_reason.is_success() {
                    codewhale_telemetry::set_exit_class(codewhale_telemetry::ExitClass::Error);
                }
                let saved_session_id = if should_persist_session && !latest_messages.is_empty() {
                    match persist_exec_session(
                        &latest_messages,
                        &latest_model,
                        PersistedProviderRoute {
                            kind: effective_provider.as_str(),
                            id: effective_provider_id.as_deref(),
                        },
                        &latest_workspace,
                        &latest_system_prompt,
                        latest_session_id.as_deref(),
                        u64::from(usage.input_tokens) + u64::from(usage.output_tokens),
                        fleet_capture.as_ref().map(|(_, manager)| manager),
                    ) {
                        Ok(id) => {
                            if output_format == ExecOutputFormat::Text && !json_output {
                                eprintln!("{}", exec_saved_session_line(&id));
                            }
                            Some(id)
                        }
                        Err(err) => {
                            if output_format == ExecOutputFormat::Text && !json_output {
                                eprintln!("warning: failed to save exec session: {err}");
                            }
                            None
                        }
                    }
                } else {
                    None
                };
                if output_format == ExecOutputFormat::StreamJson {
                    if let Some(id) = saved_session_id.as_ref() {
                        emit_exec_stream_event(&ExecStreamEvent::SessionCapture {
                            content: exec_stream_session_ref(id),
                            saved_session_id: id.clone(),
                        })?;
                    }
                    // Resolved output ceiling and its provenance, surfaced so a
                    // wrong ceiling is visible in the receipt rather than
                    // requiring packet capture.
                    let codewhale_max_output_tokens =
                        crate::route_budget::effective_max_output_tokens_for_route(
                            effective_provider,
                            &latest_model,
                            active_route_limits,
                        );
                    let codewhale_max_output_tokens_source =
                        crate::route_budget::output_ceiling_source(
                            effective_provider,
                            &latest_model,
                        )
                        .as_str();
                    // The deliverable is the final assistant reply of the
                    // session, not the cumulative stream output: a
                    // multi-step turn streams pre-tool commentary first,
                    // and that commentary is not part of the answer.
                    let final_answer = exec_stream_final_answer_text(
                        &latest_messages,
                        !summary.output.trim().is_empty(),
                    )
                    .unwrap_or_default();
                    emit_exec_stream_event(&ExecStreamEvent::Metadata {
                        meta: Box::new(ExecStreamMeta {
                            receipt_kind: "terminal",
                            provider: effective_provider_kind.clone(),
                            provider_id: effective_stream_provider_id.clone(),
                            model: latest_model.clone(),
                            route_source: route_source.clone(),
                            input_tokens: Some(usage.input_tokens),
                            output_tokens: Some(usage.output_tokens),
                            prompt_cache_hit_tokens: usage.prompt_cache_hit_tokens,
                            prompt_cache_miss_tokens: usage.prompt_cache_miss_tokens,
                            prompt_cache_write_tokens: usage.prompt_cache_write_tokens,
                            reasoning_tokens: usage.reasoning_tokens,
                            codewhale_max_output_tokens: Some(codewhale_max_output_tokens),
                            codewhale_max_output_tokens_source: Some(
                                codewhale_max_output_tokens_source,
                            ),
                            duration_ms: u64::try_from(exec_started.elapsed().as_millis())
                                .unwrap_or(u64::MAX),
                            retry_count: None,
                            approval_posture: approval_posture.clone(),
                            sandbox_posture: sandbox_posture.clone(),
                            binary_sha256: binary_sha256.clone(),
                            config_sha256: None,
                            prompt_sha256: prompt_sha256.clone(),
                            tool_catalog_sha256: tool_catalog.as_ref().and_then(|catalog| {
                                serde_json::to_vec(catalog).ok().map(|bytes| {
                                    format!("sha256:{}", crate::hashing::sha256_hex(&bytes))
                                })
                            }),
                            input_analysis: exec_stream_input_analysis(
                                &latest_messages,
                                latest_system_prompt.as_ref(),
                            ),
                            visible_final_answer_chars: final_answer.chars().count(),
                            visible_final_answer_excerpt: exec_stream_final_answer_excerpt(
                                &final_answer,
                            ),
                            resume_command: saved_session_id
                                .as_deref()
                                .map(exec_stream_resume_hint)
                                .unwrap_or_default(),
                            session_id: saved_session_id
                                .as_deref()
                                .map(exec_stream_session_ref)
                                .unwrap_or_default(),
                            workspace: latest_workspace.display().to_string(),
                            message_count: latest_messages.len(),
                            status: summary.status.clone(),
                            termination_reason: summary.termination_reason.clone(),
                            error_category: summary.error_category.clone(),
                            error: summary.error.clone(),
                        }),
                    })?;
                    emit_exec_stream_event(&ExecStreamEvent::Done)?;
                }
                let _ =
                    tokio::time::timeout(Duration::from_secs(2), engine_handle.send(Op::Shutdown))
                        .await;
                break;
            }
            Event::SessionUpdated {
                session_id,
                messages,
                system_prompt,
                model,
                workspace,
            } => {
                latest_session_id = Some(session_id);
                latest_messages = messages;
                latest_system_prompt = system_prompt;
                latest_model = model;
                latest_workspace = workspace;
            }
            // #3027: surface the engine's max-steps notice in text mode so a
            // --max-turns run that stops early says why instead of going quiet.
            Event::Status { message }
                if output_format == ExecOutputFormat::Text
                    && !json_output
                    && message.contains("Maximum model steps") =>
            {
                eprintln!("{message}");
            }
            _ => {}
        }
    }

    if summary.status.is_none() {
        if let Ok(mut manager) = exec_shell_manager.lock() {
            manager.abort_persistent_services();
        }
        let error = summary.error.clone().unwrap_or_else(|| {
            "Engine event channel closed before a terminal turn receipt".to_string()
        });
        let category = last_error_category
            .unwrap_or_else(|| crate::error_taxonomy::classify_error_message(&error));
        let termination_reason = crate::core::termination::classify_turn_termination(
            crate::core::events::TurnOutcomeStatus::Failed,
            Some(category),
            tool_error_seen,
            approval_required,
        );
        summary.status = Some("failed".to_string());
        summary.error_category = Some(category.to_string());
        summary.termination_reason = Some(termination_reason.as_str().to_string());
        summary.error = Some(error.clone());
        // Lifecycle outbox: the engine channel closed before a terminal
        // turn receipt. Every emitted `turn_start` still gets its matching
        // `turn_end` so a supervisor never sees an orphaned in-progress
        // turn. No-op when the feature is disabled.
        lifecycle_outbox.emit(codewhale_hooks::LifecycleEvent {
            event: "turn_end".to_string(),
            kind: "turn.failed".to_string(),
            thread_id: latest_session_id.clone().unwrap_or_default(),
            turn_id: None,
            item_id: None,
            payload: serde_json::json!({
                "status": "failed",
                "duration_ms": exec_turn_started_at.elapsed().as_millis() as u64,
                "workspace": latest_workspace.display().to_string(),
                "error": codewhale_hooks::bounded_text(
                    &error,
                    codewhale_hooks::OUTBOX_DETAIL_MAX_CHARS,
                ),
            }),
        });
        if output_format == ExecOutputFormat::StreamJson {
            emit_exec_stream_event(&ExecStreamEvent::Error { error })?;
        }
    }

    // Drain the terminal receipt before either returning or taking the explicit
    // retryable-failure process exit below. Outbox failures cannot change the
    // authoritative turn outcome.
    if let Err(error) = lifecycle_outbox.flush(Duration::from_secs(2)).await {
        tracing::warn!(target: "lifecycle_outbox", %error, "exec lifecycle outbox did not drain before exit");
    }

    if json_output {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    }

    if let Some(error) = summary.error.as_ref()
        && !error.trim().is_empty()
    {
        // Distinguish retryable infrastructure failures (provider/transport,
        // after all in-session retries are exhausted) from genuine task
        // failures so supervisors and bench harnesses can tell them apart at
        // the process level without parsing the stream. Genuine failures
        // keep the historical `bail!` → exit 1 path.
        let exit_code = exec_failure_exit_code(summary.error_category.as_deref());
        if exit_code != 1 {
            eprintln!("Error: exec turn failed: {error}");
            let _ = io::stdout().flush();
            std::process::exit(exit_code);
        }
        bail!("exec turn failed: {error}");
    }

    if matches!(
        summary.status.as_deref(),
        Some("failed" | "canceled" | "interrupted")
    ) {
        let status = summary.status.as_deref().unwrap_or("unknown");
        bail!("exec turn ended with status {status}");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ExecAgentEvents, exec_automation_services};
    use crate::core::engine::mock_engine_handle;
    use crate::core::events::{Event, TurnOutcomeStatus};
    use crate::core::ops::{Op, SubAgentSettlement};
    use codewhale_models::Usage;
    use std::time::{Duration, Instant};

    fn completed_parent(input_tokens: u32) -> Event {
        let usage = Usage {
            input_tokens,
            ..Usage::default()
        };
        Event::TurnComplete {
            usage: usage.clone(),
            parent_route_usage: usage,
            routed_usage_dropped_records: 0,
            status: TurnOutcomeStatus::Completed,
            error: None,
            tool_catalog: None,
            base_url: None,
        }
    }

    async fn reply_to_probe(
        operations: &mut tokio::sync::mpsc::Receiver<Op>,
        snapshot: SubAgentSettlement,
    ) {
        let Op::GetSubAgentSettlement { tx } = operations.recv().await.expect("host probe") else {
            panic!("host must not shut down while child work remains");
        };
        tx.lock().unwrap().take().unwrap().send(snapshot).unwrap();
    }

    #[tokio::test]
    async fn headless_success_waits_for_children_workflow_phases_and_parent_fan_in() {
        let mut engine = mock_engine_handle();
        let mut events = ExecAgentEvents::new(
            engine.handle.clone(),
            Instant::now() + Duration::from_secs(3),
        );
        engine.tx_event.send(completed_parent(11)).await.unwrap();
        let actor = async {
            reply_to_probe(
                &mut engine.rx_op,
                SubAgentSettlement {
                    running_children: 1,
                    running_workflows: 1,
                    pending_completions: 0,
                },
            )
            .await;
            reply_to_probe(
                &mut engine.rx_op,
                SubAgentSettlement {
                    running_children: 0,
                    running_workflows: 1,
                    pending_completions: 0,
                },
            )
            .await;
            reply_to_probe(
                &mut engine.rx_op,
                SubAgentSettlement {
                    running_children: 0,
                    running_workflows: 0,
                    pending_completions: 1,
                },
            )
            .await;
            engine
                .tx_event
                .send(Event::MessageDelta {
                    content: "child findings reviewed".into(),
                    index: 0,
                })
                .await
                .unwrap();
            engine.tx_event.send(completed_parent(7)).await.unwrap();
            reply_to_probe(&mut engine.rx_op, SubAgentSettlement::default()).await;
        };
        let host = async {
            assert!(
                matches!(events.next().await, Some(Event::MessageDelta { content, .. }) if content == "child findings reviewed")
            );
            let Some(Event::TurnComplete { usage, status, .. }) = events.next().await else {
                panic!("settled parent receipt")
            };
            assert_eq!(status, TurnOutcomeStatus::Completed);
            assert_eq!(
                usage.input_tokens, 18,
                "both parent turns are accounted once"
            );
        };
        tokio::time::timeout(Duration::from_secs(4), async { tokio::join!(actor, host) })
            .await
            .unwrap();
        assert!(
            !engine.handle.is_cancelled(),
            "ordinary success must not cancel children"
        );
        assert!(
            engine.rx_op.try_recv().is_err(),
            "the event reader does not send early Shutdown"
        );
    }

    #[tokio::test]
    async fn headless_child_settlement_deadline_bounds_a_stalled_engine_probe() {
        let mut engine = mock_engine_handle();
        let mut events = ExecAgentEvents::new(
            engine.handle.clone(),
            Instant::now() + Duration::from_millis(30),
        );
        engine.tx_event.send(completed_parent(13)).await.unwrap();
        let event = tokio::time::timeout(Duration::from_secs(1), events.next())
            .await
            .unwrap();
        let Some(Event::TurnComplete {
            status,
            error,
            usage,
            routed_usage_dropped_records,
            ..
        }) = event
        else {
            panic!("bounded failure receipt")
        };
        assert_eq!(status, TurnOutcomeStatus::Failed);
        assert!(error.unwrap().contains("wall-clock budget exhausted"));
        assert_eq!(usage.input_tokens, 13);
        assert_eq!(routed_usage_dropped_records, 1);
        assert!(engine.handle.is_cancelled());
        let mut shutdown = false;
        while let Ok(op) = engine.rx_op.try_recv() {
            shutdown |= matches!(op, Op::Shutdown);
        }
        assert!(shutdown, "shutdown must also cancel detached session tasks");
    }

    #[tokio::test]
    async fn headless_failed_or_interrupted_parent_skips_child_settlement() {
        for terminal_status in [TurnOutcomeStatus::Failed, TurnOutcomeStatus::Interrupted] {
            let mut engine = mock_engine_handle();
            let mut events = ExecAgentEvents::new(
                engine.handle.clone(),
                Instant::now() + Duration::from_secs(30),
            );
            let mut terminal = completed_parent(3);
            if let Event::TurnComplete { status, .. } = &mut terminal {
                *status = terminal_status;
            }
            engine.tx_event.send(terminal).await.unwrap();
            assert!(
                matches!(events.next().await, Some(Event::TurnComplete { status, .. }) if status == terminal_status)
            );
            assert!(
                engine.rx_op.try_recv().is_err(),
                "failure cannot admit another settling turn"
            );
        }
    }

    #[tokio::test]
    async fn headless_fatal_fan_in_error_cancels_before_releasing_terminal_receipt() {
        let engine = mock_engine_handle();
        let mut events = ExecAgentEvents::new(
            engine.handle.clone(),
            Instant::now() + Duration::from_secs(30),
        );
        engine.tx_event.send(completed_parent(5)).await.unwrap();
        engine
            .tx_event
            .send(Event::error(crate::error_taxonomy::ErrorEnvelope::fatal(
                "fan-in route unavailable",
            )))
            .await
            .unwrap();
        assert!(matches!(events.next().await, Some(Event::Error { .. })));
        assert!(engine.handle.is_cancelled());
        assert!(
            matches!(events.next().await, Some(Event::TurnComplete { status: TurnOutcomeStatus::Failed, error: Some(error), .. }) if error.contains("fan-in route unavailable"))
        );
    }

    #[tokio::test]
    async fn headless_cancel_during_child_wait_returns_interrupted() {
        let mut engine = mock_engine_handle();
        let mut events = ExecAgentEvents::new(
            engine.handle.clone(),
            Instant::now() + Duration::from_secs(30),
        );
        engine.tx_event.send(completed_parent(5)).await.unwrap();
        let cancel = async {
            reply_to_probe(
                &mut engine.rx_op,
                SubAgentSettlement {
                    running_children: 1,
                    running_workflows: 0,
                    pending_completions: 0,
                },
            )
            .await;
            engine.handle.cancel();
        };
        let host = async {
            assert!(matches!(
                events.next().await,
                Some(Event::TurnComplete {
                    status: TurnOutcomeStatus::Interrupted,
                    ..
                })
            ));
        };
        tokio::time::timeout(Duration::from_secs(1), async { tokio::join!(cancel, host) })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn headless_closed_engine_cannot_reuse_an_earlier_success_receipt() {
        let mut engine = mock_engine_handle();
        let mut events = ExecAgentEvents::new(
            engine.handle.clone(),
            Instant::now() + Duration::from_secs(30),
        );
        engine.tx_event.send(completed_parent(5)).await.unwrap();
        engine.close_event_stream();
        assert!(
            matches!(events.next().await, Some(Event::TurnComplete { status: TurnOutcomeStatus::Failed, error: Some(error), .. }) if error.contains("channel closed"))
        );
    }

    /// The reproduced defect: headless exec advertised `automation` while
    /// attaching no store, so every call — including the read-only `list` and
    /// `read` — failed "AutomationManager is not attached". Exec must attach
    /// the same durable store the TUI and the Runtime open.
    #[test]
    fn headless_exec_attaches_the_shared_automation_store() {
        let _lock = crate::test_support::lock_test_env();
        let tmp = tempfile::TempDir::new().expect("tempdir");
        // SAFETY: serialised by lock_test_env.
        unsafe {
            std::env::set_var("CODEWHALE_AUTOMATIONS_DIR", tmp.path());
        }
        let attached = exec_automation_services(false).expect("open store");
        // SAFETY: cleanup under the same lock.
        unsafe {
            std::env::remove_var("CODEWHALE_AUTOMATIONS_DIR");
        }
        let attached = attached.expect("exec attaches the automation store");
        let manager = attached.blocking_lock();
        // Reads work against the shared store...
        assert!(
            manager.list_automations().is_ok(),
            "an attached store must serve inspection"
        );
        // ...while the exec host stays outside the Runtime's exclusive
        // task-execution lease, so it claims no dispatch ownership.
        assert!(
            manager.execution_scope().is_none(),
            "a one-shot host must not claim an execution scope"
        );
    }

    /// A Fleet worker keeps the narrowed envelope it was launched with.
    #[test]
    fn fleet_workers_get_no_automation_store() {
        assert!(
            exec_automation_services(true)
                .expect("no store to open")
                .is_none()
        );
    }

    /// A store that cannot be opened is reported, not swallowed into a silent
    /// "not attached" at the first tool call.
    #[test]
    fn an_unopenable_store_fails_loudly() {
        let _lock = crate::test_support::lock_test_env();
        let tmp = tempfile::NamedTempFile::new().expect("temp file");
        // A regular file cannot host the store's directories.
        let blocked = tmp.path().join("automations");
        // SAFETY: serialised by lock_test_env.
        unsafe {
            std::env::set_var("CODEWHALE_AUTOMATIONS_DIR", &blocked);
        }
        let result = exec_automation_services(false);
        // SAFETY: cleanup under the same lock.
        unsafe {
            std::env::remove_var("CODEWHALE_AUTOMATIONS_DIR");
        }
        let err = result.expect_err("opening the store must fail");
        assert!(
            format!("{err:#}").contains("automation store for headless exec"),
            "the failure must name what could not be opened: {err:#}"
        );
    }
}
