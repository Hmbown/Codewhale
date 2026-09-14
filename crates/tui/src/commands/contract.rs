//! FEAT-015 TUI command-boundary surface.
//!
//! This module holds the TUI-owned pieces of the staged command migration:
//! the pending-frontier projection (D4), the capability facet adapters
//! (D1), boundary-value and localization-key mappings (D3/D8), the envelope
//! construction helper (D1), and the seam helpers (D7-D9). It is deliberately
//! the only new TUI module for the migration surface; the production
//! registry/dispatch stay in `traits.rs` / `mod.rs`.
//!
//! FEAT-015 does NOT migrate any production command. The adapters below wrap
//! App-owned state behind the FEAT-014 contract shapes so later FEATs
//! (FEAT-018+) can adopt them one group at a time. Handlers only ever see
//! `&mut dyn` facets — concrete `App` is never exposed through an envelope.
//!
//! ## Authoritative host-proxy design (D1)
//!
//! `CommandContexts` holds fifteen independently borrowed facet objects, while
//! important behavior (mode transitions, model invalidation, cost accounting,
//! skill refresh) is authoritative on `App`. The adapters therefore share a
//! synchronous TUI-owned host proxy. Each trait call borrows `App` only for the
//! duration of that call and delegates to the real operation; handlers still
//! receive only portable facets and can never name concrete TUI state.
//!
//! ## Dead-code note
//!
//! FEAT-015 intentionally wires no production contextual command. Some bridge
//! helpers remain production-dead until the first slice migrates (FEAT-018+),
//! so this transitional module keeps a bounded dead-code allow.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use codewhale_command_contract::facets::{
    CommandApprovalState, CommandCostContext, CommandMediaContext, CommandMemoryContext,
    CommandModePolicyContext, CommandModelContext, CommandPluginContext,
    CommandPresentationContext, CommandProjectContext, CommandSessionContext,
    CommandSessionControlContext, CommandSessionLifecycleContext, CommandSkillGroupContext,
    CommandSkillsContext, CommandSystemPromptContext, CommandWorkspaceContext, HostedWorkTarget,
    MediaAttachmentReceipt, MemoryDelete, MemoryDeleteScope, MemoryExport, MemoryGetOutcome,
    MemoryHit, MemoryImportOutcome, MemoryReindex, MemoryRememberTarget, MemoryRemembered,
    MemoryStatus, PlanProjection, PlanSections, PlanStep, PlanStepStatus, PluginDetail,
    PluginDiagnostic, PluginDiagnosticLevel, PluginExportReceipt, PluginLegacyScan,
    PluginLegacyTool, PluginManagedCandidate, PluginManagedScan, PluginMarketplaceAddReceipt,
    PluginMarketplaceCandidate, PluginMarketplaceCatalog, PluginMarketplaceInstallPlan,
    PluginMarketplaceState, PluginMcpServerDetail, PluginMcpTransport, PluginMutationOutcome,
    PluginMutationReceipt, PluginSuggestion, PluginSummary, ProjectGoalState, ProjectGoalStatus,
    ProjectShareProjection, RelayProjection, RemoteLink, RemoteOpenOutcome, RemoteRegistryOutcome,
    RemoteSkillEntry, RemoteStartInfo, ResumeImportReceipt, ResumeSource, ReviewOutcome,
    SessionArchiveReceipt, SessionBranchOutcome, SessionForkFromReceipt, SessionForkReceipt,
    SessionNewReceipt, SessionSaveReceipt, SessionSyncPayload, SessionTitleReceipt,
    SkillActivationError, SkillActivationOutcome, SkillBundledTier, SkillEntry,
    SkillMutationOutcome, SkillMutationReceipt, SkillRecommendation, SkillRegistryProjection,
    SkillSourceKind, SkillSyncEntry, SkillSyncOutcome, SkillTargetScope, SnapshotEntry,
    TitleReport, TitleSource, TodoProjection, TreeBodyProjection,
};
#[cfg(test)]
use codewhale_command_contract::handler::ContextParts;
use codewhale_command_contract::handler::{CommandCapabilities, CommandContexts};
use codewhale_command_contract::types::{
    CommandApprovalMode, CommandCurrency, CommandMode, CommandProviderId, CommandReasoningEffort,
};
use codewhale_config::AppMode;
use codewhale_core::request::{Message, SystemPrompt};
use codewhale_execpolicy::ApprovalMode;

use crate::commands::groups::plugins::plugin_network_policy;

use crate::dependencies::ExternalTool as _;
use crate::network_policy::NetworkPolicy;
use crate::pricing::CostCurrency;
use crate::reasoning_preference::ReasoningEffort;
use crate::tui::app::App;
use crate::tui::history::HistoryCell;
use codewhale_localization::{MessageId, tr};

// ---------------------------------------------------------------------------
// Pending frontier projection (D4)
// ---------------------------------------------------------------------------

/// Sorted, unique frontier of command groups that still use concrete-`App`
/// handlers. This is the TUI-visible projection of the checked-in migration
/// topology (`scripts/command-migration-topology.json`); the CI gate performs
/// the authoritative bidirectional source scan against that artifact.
///
/// Not referenced by production dispatch code — the fail-closed Python gate
/// (`scripts/check-command-migration-manifest.py`) reads this exact
/// declaration by source regex and the Rust frontier tests assert it.
#[allow(dead_code)]
pub(crate) const PENDING_GROUPS: &[&str] = &["config", "core", "debug", "session"];

// ---------------------------------------------------------------------------
// Boundary-value mappings (D8)
// ---------------------------------------------------------------------------

/// Map the TUI operating mode onto the portable command boundary value.
pub(crate) fn to_command_mode(mode: AppMode) -> CommandMode {
    match mode {
        AppMode::Agent => CommandMode::Agent,
        AppMode::Plan => CommandMode::Plan,
        AppMode::Operate => CommandMode::Operate,
    }
}

pub(crate) fn from_command_mode(mode: CommandMode) -> AppMode {
    match mode {
        CommandMode::Agent => AppMode::Agent,
        CommandMode::Plan => AppMode::Plan,
        CommandMode::Operate => AppMode::Operate,
    }
}

/// Map the TUI approval posture onto the portable command boundary value.
pub(crate) fn to_command_approval(mode: ApprovalMode) -> CommandApprovalMode {
    match mode {
        ApprovalMode::Auto => CommandApprovalMode::Auto,
        ApprovalMode::Bypass => CommandApprovalMode::Bypass,
        ApprovalMode::Suggest => CommandApprovalMode::Suggest,
        ApprovalMode::Never => CommandApprovalMode::Never,
    }
}

/// Map the TUI reasoning-effort tier onto the portable command boundary value.
pub(crate) fn to_command_effort(effort: ReasoningEffort) -> CommandReasoningEffort {
    match effort {
        ReasoningEffort::Off => CommandReasoningEffort::Off,
        ReasoningEffort::Minimal => CommandReasoningEffort::Minimal,
        ReasoningEffort::Low => CommandReasoningEffort::Low,
        ReasoningEffort::Medium => CommandReasoningEffort::Medium,
        ReasoningEffort::High => CommandReasoningEffort::High,
        ReasoningEffort::XHigh => CommandReasoningEffort::XHigh,
        ReasoningEffort::Ultra => CommandReasoningEffort::Ultra,
        ReasoningEffort::Auto => CommandReasoningEffort::Auto,
        ReasoningEffort::Max => CommandReasoningEffort::Max,
    }
}

/// Map the TUI cost-display currency onto the portable command boundary value.
pub(crate) fn to_command_currency(currency: CostCurrency) -> CommandCurrency {
    match currency {
        CostCurrency::Usd => CommandCurrency::Usd,
        CostCurrency::Cny => CommandCurrency::Cny,
    }
}

fn from_command_currency(currency: CommandCurrency) -> CostCurrency {
    match currency {
        CommandCurrency::Usd => CostCurrency::Usd,
        CommandCurrency::Cny => CostCurrency::Cny,
    }
}

/// Stable provider identity text at the command boundary.
///
/// The TUI persists either the canonical `ApiProvider::as_str()` spelling or —
/// for named custom providers — the exact configured identity text. This
/// function never leaks URLs, credentials, or filesystem paths.
pub(crate) fn to_provider_id(identity: &str) -> CommandProviderId {
    CommandProviderId(identity.to_string())
}

/// Bridge a portable metadata description key onto the TUI localization id.
///
/// The key convention (D3) is mechanical: the contract key equals the
/// snake_case of the [`MessageId`] variant name. The match table is the
/// authoritative bridge; unknown keys fail deterministically.
pub(crate) fn key_to_message_id(key: &'static str) -> Option<MessageId> {
    Some(match key {
        "cmd_advisor_description" => MessageId::CmdAdvisorDescription,
        "cmd_agent_description" => MessageId::CmdAgentDescription,
        "cmd_anchor_description" => MessageId::CmdAnchorDescription,
        "cmd_attach_description" => MessageId::CmdAttachDescription,
        "cmd_auto_description" => MessageId::CmdAutoDescription,
        "cmd_auth_description" => MessageId::CmdAuthDescription,
        "cmd_automation_description" => MessageId::CmdAutomationDescription,
        "cmd_balance_description" => MessageId::CmdBalanceDescription,
        "cmd_branch_description" => MessageId::CmdBranchDescription,
        "cmd_cache_description" => MessageId::CmdCacheDescription,
        "cmd_change_description" => MessageId::CmdChangeDescription,
        "cmd_clear_description" => MessageId::CmdClearDescription,
        "cmd_compact_description" => MessageId::CmdCompactDescription,
        "cmd_config_description" => MessageId::CmdConfigDescription,
        "cmd_constitution_description" => MessageId::CmdConstitutionDescription,
        "cmd_context_description" => MessageId::CmdContextDescription,
        "cmd_cost_description" => MessageId::CmdCostDescription,
        "cmd_diff_description" => MessageId::CmdDiffDescription,
        "cmd_edit_description" => MessageId::CmdEditDescription,
        "cmd_effort_description" => MessageId::CmdEffortDescription,
        "cmd_exit_description" => MessageId::CmdExitDescription,
        "cmd_export_description" => MessageId::CmdExportDescription,
        "cmd_feedback_description" => MessageId::CmdFeedbackDescription,
        "cmd_fleet_description" => MessageId::CmdFleetDescription,
        "cmd_fork_description" => MessageId::CmdForkDescription,
        "cmd_goal_description" => MessageId::CmdGoalDescription,
        "cmd_help_description" => MessageId::CmdHelpDescription,
        "cmd_hf_description" => MessageId::CmdHfDescription,
        "cmd_home_description" => MessageId::CmdHomeDescription,
        "cmd_hooks_description" => MessageId::CmdHooksDescription,
        "cmd_hotbar_description" => MessageId::CmdHotbarDescription,
        "cmd_init_description" => MessageId::CmdInitDescription,
        "cmd_jobs_description" => MessageId::CmdJobsDescription,
        "cmd_dispatch_description" => MessageId::CmdDispatchDescription,
        "cmd_lane_description" => MessageId::CmdLaneDescription,
        "cmd_links_description" => MessageId::CmdLinksDescription,
        "cmd_load_description" => MessageId::CmdLoadDescription,
        "cmd_logout_description" => MessageId::CmdLogoutDescription,
        "cmd_lsp_description" => MessageId::CmdLspDescription,
        "cmd_mcp_description" => MessageId::CmdMcpDescription,
        "cmd_memory_description" => MessageId::CmdMemoryDescription,
        "cmd_mode_description" => MessageId::CmdModeDescription,
        "cmd_model_db_description" => MessageId::CmdModelDbDescription,
        "cmd_model_description" => MessageId::CmdModelDescription,
        "cmd_models_description" => MessageId::CmdModelsDescription,
        "cmd_network_description" => MessageId::CmdNetworkDescription,
        "cmd_new_description" => MessageId::CmdNewDescription,
        "cmd_note_description" => MessageId::CmdNoteDescription,
        "cmd_permissions_description" => MessageId::CmdPermissionsDescription,
        "cmd_pin_description" => MessageId::CmdPinDescription,
        "cmd_plugin_description" => MessageId::CmdPluginDescription,
        "cmd_plugin_detail_description" => MessageId::CmdPluginDetailDescription,
        "cmd_preview_request_description" => MessageId::CmdPreviewRequestDescription,
        "cmd_profile_description" => MessageId::CmdProfileDescription,
        "cmd_provider_description" => MessageId::CmdProviderDescription,
        "cmd_purge_description" => MessageId::CmdPurgeDescription,
        "cmd_queue_description" => MessageId::CmdQueueDescription,
        "cmd_relay_description" => MessageId::CmdRelayDescription,
        "cmd_remote_control_description" => MessageId::CmdRemoteControlDescription,
        "cmd_remote_env_description" => MessageId::CmdRemoteEnvDescription,
        "cmd_rename_description" => MessageId::CmdRenameDescription,
        "cmd_restore_description" => MessageId::CmdRestoreDescription,
        "cmd_resume_description" => MessageId::CmdResumeDescription,
        "cmd_retry_description" => MessageId::CmdRetryDescription,
        "cmd_review_description" => MessageId::CmdReviewDescription,
        "cmd_rlm_description" => MessageId::CmdRlmDescription,
        "cmd_save_description" => MessageId::CmdSaveDescription,
        "cmd_sessions_description" => MessageId::CmdSessionsDescription,
        "cmd_settings_description" => MessageId::CmdSettingsDescription,
        "cmd_setup_description" => MessageId::CmdSetupDescription,
        "cmd_share_description" => MessageId::CmdShareDescription,
        "cmd_sidebar_description" => MessageId::CmdSidebarDescription,
        "cmd_skill_description" => MessageId::CmdSkillDescription,
        "cmd_skills_description" => MessageId::CmdSkillsDescription,
        "cmd_stash_description" => MessageId::CmdStashDescription,
        "cmd_status_description" => MessageId::CmdStatusDescription,
        "cmd_statusline_description" => MessageId::CmdStatuslineDescription,
        "cmd_structcopy_description" => MessageId::CmdStructcopyDescription,
        "cmd_subagents_description" => MessageId::CmdSubagentsDescription,
        "cmd_system_description" => MessageId::CmdSystemDescription,
        "cmd_task_description" => MessageId::CmdTaskDescription,
        "cmd_theme_description" => MessageId::CmdThemeDescription,
        "cmd_title_description" => MessageId::CmdTitleDescription,
        "cmd_tokens_description" => MessageId::CmdTokensDescription,
        "cmd_tools_description" => MessageId::CmdToolsDescription,
        "cmd_translate_description" => MessageId::CmdTranslateDescription,
        "cmd_tree_description" => MessageId::CmdTreeDescription,
        "cmd_trust_description" => MessageId::CmdTrustDescription,
        "cmd_turn_inspect_description" => MessageId::CmdTurnInspectDescription,
        "cmd_undo_description" => MessageId::CmdUndoDescription,
        "cmd_update_description" => MessageId::CmdUpdateDescription,
        "cmd_verbose_description" => MessageId::CmdVerboseDescription,
        "cmd_voice_control_description" => MessageId::CmdVoiceControlDescription,
        "cmd_voice_description" => MessageId::CmdVoiceDescription,
        "cmd_voice_send_description" => MessageId::CmdVoiceSendDescription,
        "cmd_workflow_description" => MessageId::CmdWorkflowDescription,
        "cmd_workflows_description" => MessageId::CmdWorkflowsDescription,
        "cmd_workspace_description" => MessageId::CmdWorkspaceDescription,
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Capability facet adapters (D1)
// ---------------------------------------------------------------------------

/// Shared TUI host hidden behind the portable command facets.
///
/// The envelope needs fifteen independently borrowed facet objects, while the
/// authoritative mutation methods live on `App`. Each adapter therefore owns
/// an `Rc` clone of this synchronous host proxy. Trait calls borrow `App` only
/// for the duration of one method, delegate to the real TUI authority, and
/// return owned values. Command handlers never receive or name `App`.
struct CommandHost<'a> {
    app: RefCell<&'a mut App>,
}

type SharedCommandHost<'a> = Rc<CommandHost<'a>>;

// ---------------------------------------------------------------------------
// Session lifecycle adapter (FEAT-023 D4)
//
// Sole host owner of concrete lifecycle machinery for the nine lifecycle
// commands: App reads/mutations, SessionManager, saved-session creation,
// journal load/branching, filesystem persistence, work-state snapshots/
// publication, picker/view-stack construction, archive/prune, and the core
// `reset_conversation_state` call for `/new`. Every delegate reproduces the
// baseline check/mutation order exactly (blocked transitions fail before I/O,
// branching never rewrites journal history, publication failures retain their
// post-save semantics, archive state updates atomically) and returns portable
// receipts or the exact host-error text the baseline surfaces. The lifecycle
// bodies no longer live in `groups/session/session.rs`; adapter regressions and
// portable-handler tests preserve their host and presentation contracts.
// ---------------------------------------------------------------------------
pub(crate) struct SessionLifecycleAdapter<'a> {
    host: SharedCommandHost<'a>,
}

impl CommandSessionLifecycleContext for SessionLifecycleAdapter<'_> {
    fn transition_blocked(&self) -> bool {
        self.host.app.borrow().session_transition_blocked()
    }

    fn branch_current_leaf_hint(&self) -> Option<String> {
        let app = self.host.app.borrow();
        let session_id = app.current_session_id.as_deref()?;
        let manager = crate::session_manager::SessionManager::default_location().ok()?;
        let mut session = manager.load_session(session_id).ok()?;
        session.ensure_journal();
        session.journal.as_ref()?.leaf_id.clone()
    }

    fn branch_to(&mut self, entry_id: &str) -> Result<SessionBranchOutcome, String> {
        let mut app = self.host.app.borrow_mut();
        let session_id = match app.current_session_id.clone() {
            Some(id) => id,
            None => {
                return Err(
                    "No active session to branch. Resume or create a session first.".to_string(),
                );
            }
        };
        let manager = match crate::session_manager::SessionManager::default_location() {
            Ok(m) => m,
            Err(e) => return Err(format!("could not open sessions directory: {e}")),
        };
        let mut session = match manager.load_session(&session_id) {
            Ok(s) => s,
            Err(e) => return Err(format!("could not load session {session_id}: {e}")),
        };
        session.ensure_journal();
        let journal_len_before = session
            .journal
            .as_ref()
            .map(|j| j.entries.len())
            .unwrap_or(0);
        match session.journal_branch_to(entry_id) {
            Ok(()) => {
                if let Err(e) = manager.save_session(&session) {
                    return Err(format!("branch saved but persist failed: {e}"));
                }
                app.restore_api_messages(
                    session.messages.clone(),
                    &session.journal_message_stamps(),
                );
                let leaf_display = session
                    .leaf_id
                    .clone()
                    .unwrap_or_else(|| "(none)".to_string());
                Ok(SessionBranchOutcome {
                    leaf_display,
                    journal_entries_before: journal_len_before,
                })
            }
            Err(e) => Err(format!(
                "branch failed: {e}. Use `/tree` to see valid entry ids."
            )),
        }
    }

    fn tree_body(&self) -> Result<TreeBodyProjection, String> {
        let app = self.host.app.borrow();
        let manager = match crate::session_manager::SessionManager::default_location() {
            Ok(m) => m,
            Err(e) => return Err(format!("could not open sessions directory: {e}")),
        };
        if let Some(session_id) = app.current_session_id.clone() {
            if let Ok(mut session) = manager.load_session(&session_id) {
                session.ensure_journal();
                if let Some(journal) = session.journal.as_ref() {
                    let rendered = crate::session_tree::render_tree(journal);
                    return Ok(TreeBodyProjection::Journal { rendered });
                }
            }
            if app.api_messages.is_empty() {
                return Ok(TreeBodyProjection::EmptySession);
            }
            let mut rendered =
                String::from("Active branch (linear — journal will be created on save):\n");
            for (i, msg) in app.api_messages.iter().enumerate() {
                let snippet: String = msg
                    .content
                    .iter()
                    .filter_map(|b| match b {
                        codewhale_models::ContentBlock::Text { text, .. } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                let short: String = snippet.chars().take(60).collect();
                let marker = if i + 1 == app.api_messages.len() {
                    "*"
                } else {
                    "●"
                };
                rendered.push_str(&format!("  {marker} [{i}] {}: {short}\n", msg.role));
            }
            Ok(TreeBodyProjection::Linear { rendered })
        } else {
            Ok(TreeBodyProjection::NoSession)
        }
    }

    fn save_session(
        &mut self,
        explicit_path: Option<String>,
    ) -> Result<SessionSaveReceipt, String> {
        let mut app = self.host.app.borrow_mut();
        let explicit_save_path = explicit_path.map(PathBuf::from);

        // Explicit save must report contended Work state instead of falling
        // back to the last automatic snapshot. Reuse the canonical snapshot
        // builder after that preflight so stable IDs also retain lifecycle,
        // title, provider, and window metadata.
        app.work_state_snapshot()
            .map_err(|error| format!("Failed to snapshot Work state: {error}"))?;
        let manager = crate::session_manager::SessionManager::default_location()
            .map_err(|error| format!("could not open sessions directory: {error}"))?;
        let session = crate::tui::ui::build_session_snapshot(&mut app, &manager)?;
        let queue_transition =
            crate::tui::ui::prepare_offline_queue_transition(&app, &session.metadata.id)?;
        let save_path = explicit_save_path.unwrap_or_else(|| {
            let dir = crate::session_manager::default_sessions_dir()
                .unwrap_or_else(|_| app.workspace.clone());
            dir.join(format!("{}.json", session.metadata.id))
        });

        let sessions_dir = save_path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map_or_else(|| app.workspace.clone(), std::path::Path::to_path_buf);

        match std::fs::create_dir_all(&sessions_dir) {
            Ok(()) => {
                let json = match serde_json::to_string_pretty(&session) {
                    Ok(j) => j,
                    Err(e) => return Err(format!("Failed to serialize session: {e}")),
                };
                match crate::utils::write_atomic(&save_path, json.as_bytes()) {
                    Ok(()) => {
                        crate::tui::ui::install_offline_queue_transition(
                            &mut app,
                            queue_transition,
                        );
                        app.current_session_id = Some(session.metadata.id.clone());
                        app.current_session_metadata = Some(session.metadata.clone());
                        app.session_title = Some(session.metadata.title.clone());
                        if let Err(err) = app.publish_pending_work_state() {
                            return Err(format!(
                                "Session saved, but Work views were not published: {err}"
                            ));
                        }
                        Ok(SessionSaveReceipt {
                            display_path: save_path.display().to_string(),
                            truncated_id: crate::session_manager::truncate_id(&session.metadata.id)
                                .to_string(),
                        })
                    }
                    Err(e) => Err(format!("Failed to save session: {e}")),
                }
            }
            Err(e) => Err(format!("Failed to create directory: {e}")),
        }
    }

    fn fork_active(&mut self) -> Result<SessionForkReceipt, String> {
        let mut app = self.host.app.borrow_mut();
        if app.api_messages.is_empty() {
            return Err("Nothing to fork. Send or load a message first.".to_string());
        }

        let manager = match crate::session_manager::SessionManager::default_location() {
            Ok(manager) => manager,
            Err(err) => {
                return Err(format!("could not open sessions directory: {err}"));
            }
        };

        let parent_id = app
            .current_session_id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let mut parent = crate::session_manager::create_saved_session_with_id_and_mode(
            parent_id,
            &app.api_messages,
            &app.model,
            &app.workspace,
            u64::from(app.session.total_tokens),
            app.system_prompt.as_ref(),
            Some(app.mode.label()),
        );
        parent
            .metadata
            .set_model_provider_route(app.api_provider.as_str(), app.provider_id_for_persistence());
        if let Some(cached) = app
            .current_session_metadata
            .as_ref()
            .filter(|metadata| metadata.id == parent.metadata.id)
        {
            parent.metadata.created_at = cached.created_at;
            parent.metadata.title.clone_from(&cached.title);
            parent
                .metadata
                .parent_session_id
                .clone_from(&cached.parent_session_id);
            parent.metadata.forked_from_message_count = cached.forked_from_message_count;
        }
        app.sync_cost_to_metadata(&mut parent.metadata);
        parent.context_references = app.session_context_references.clone();
        parent.artifacts = app.session_artifacts.clone();
        let work_state = match app.work_state_snapshot() {
            Ok(state) => state,
            Err(err) => return Err(format!("Failed to snapshot Work state: {err}")),
        };
        parent.work_state = work_state.clone();
        parent.last_auto_route = app.auto_route_for_persistence();

        if let Err(err) = manager.save_session(&parent) {
            return Err(format!("Failed to save parent session: {err}"));
        }

        let mut forked = crate::session_manager::create_saved_session_with_mode(
            &app.api_messages,
            &app.model,
            &app.workspace,
            u64::from(app.session.total_tokens),
            app.system_prompt.as_ref(),
            Some(app.mode.label()),
        );
        forked
            .metadata
            .set_model_provider_route(app.api_provider.as_str(), app.provider_id_for_persistence());
        forked.metadata.copy_cost_from(&parent.metadata);
        forked.metadata.spawn_depth = parent.metadata.spawn_depth.saturating_add(1);
        // Ensure journal for both sessions: parent already has one from factory, bump forked's journal depth
        if let Some(j) = forked.journal.as_mut() {
            j.spawn_depth = forked.metadata.spawn_depth;
        }
        if let Some(j) = parent.journal.as_mut() {
            j.spawn_depth = parent.metadata.spawn_depth;
        }
        forked.metadata.mark_forked_from(&parent.metadata);
        forked.context_references = app.session_context_references.clone();
        forked.artifacts = app.session_artifacts.clone();
        forked.work_state = work_state;
        forked.last_auto_route = app.auto_route_for_persistence();
        let queue_transition =
            crate::tui::ui::prepare_offline_queue_transition(&app, &forked.metadata.id)?;

        if let Err(err) = manager.save_session(&forked) {
            return Err(format!("Failed to save forked session: {err}"));
        }
        if let Err(err) = app.publish_pending_work_state() {
            return Err(format!(
                "Sessions saved, but Work views were not published: {err}"
            ));
        }

        crate::tui::ui::install_offline_queue_transition(&mut app, queue_transition);
        app.current_session_id = Some(forked.metadata.id.clone());
        app.current_session_metadata = Some(forked.metadata.clone());
        app.session_title = Some(forked.metadata.title.clone());
        // A fork starts as its own session: no inherited tab/window title.
        app.window_title = None;
        let fork_id = forked.metadata.id.clone();
        let parent_label = crate::session_manager::truncate_id(&parent.metadata.id).to_string();
        let fork_label = crate::session_manager::truncate_id(&fork_id).to_string();
        let mode = to_command_mode(app.mode);
        Ok(SessionForkReceipt {
            parent_label,
            fork_label,
            sync: SessionSyncPayload {
                session_id: Some(fork_id),
                messages: app.api_messages.clone(),
                system_prompt: app.system_prompt.clone(),
                model: app.model.clone(),
                workspace: app.workspace.clone(),
                mode,
            },
        })
    }

    fn fork_from(&mut self, session_id_or_prefix: &str) -> Result<SessionForkFromReceipt, String> {
        let mut app = self.host.app.borrow_mut();
        let manager = match crate::session_manager::SessionManager::default_location() {
            Ok(m) => m,
            Err(err) => {
                return Err(format!("could not open sessions directory: {err}"));
            }
        };
        let source = manager
            .load_session(session_id_or_prefix)
            .or_else(|_| manager.load_session_by_prefix(session_id_or_prefix));
        let mut source_session = match source {
            Ok(s) => s,
            Err(e) => {
                return Err(format!(
                    "could not load session '{}': {e}",
                    session_id_or_prefix
                ));
            }
        };
        source_session.ensure_journal();
        let journal = source_session.journal.clone().unwrap_or_else(|| {
            crate::session_tree::SessionJournal::from_messages(
                source_session.messages.clone(),
                source_session.metadata.spawn_depth,
            )
        });
        let forked_journal = journal.fork_from(None).unwrap_or_else(|_| {
            crate::session_tree::SessionJournal::with_spawn_depth(
                source_session.metadata.spawn_depth.saturating_add(1),
            )
        });
        let messages = forked_journal.to_messages();
        let mut forked = crate::session_manager::create_saved_session_with_id_and_mode(
            uuid::Uuid::new_v4().to_string(),
            &messages,
            &source_session.metadata.model,
            &app.workspace,
            source_session.metadata.total_tokens,
            source_session
                .system_prompt
                .as_ref()
                .map(|s| codewhale_models::SystemPrompt::Text(s.clone()))
                .as_ref(),
            source_session.metadata.mode.as_deref(),
        );
        forked.journal = Some(forked_journal);
        forked.leaf_id = forked.journal.as_ref().and_then(|j| j.leaf_id.clone());
        forked.messages = messages;
        forked.metadata.spawn_depth = forked.journal.as_ref().map(|j| j.spawn_depth).unwrap_or(0);
        forked.metadata.parent_session_id = Some(source_session.metadata.id.clone());
        forked.metadata.forked_from_message_count = Some(source_session.metadata.message_count);
        forked.metadata.set_model_provider_route(
            source_session.metadata.model_provider.as_str(),
            source_session.metadata.model_provider_id.as_deref(),
        );
        forked.metadata.copy_cost_from(&source_session.metadata);
        forked.context_references = source_session.context_references.clone();
        forked.artifacts = source_session.artifacts.clone();
        forked.work_state = source_session.work_state.clone();
        forked.last_auto_route = source_session.last_auto_route.clone();
        let queue_transition =
            crate::tui::ui::prepare_offline_queue_transition(&app, &forked.metadata.id)?;
        if let Err(err) = manager.save_session(&forked) {
            return Err(format!("Failed to save forked session: {err}"));
        }
        crate::tui::ui::install_offline_queue_transition(&mut app, queue_transition);
        app.current_session_id = Some(forked.metadata.id.clone());
        app.current_session_metadata = Some(forked.metadata.clone());
        app.session_title = Some(forked.metadata.title.clone());
        // A fork starts as its own session: no inherited tab/window title.
        app.window_title = None;
        let parent_label =
            crate::session_manager::truncate_id(&source_session.metadata.id).to_string();
        let fork_label = crate::session_manager::truncate_id(&forked.metadata.id).to_string();
        let mode = to_command_mode(app.mode);
        Ok(SessionForkFromReceipt {
            parent_label,
            fork_label,
            spawn_depth: forked.metadata.spawn_depth.into(),
            sync: SessionSyncPayload {
                session_id: Some(forked.metadata.id.clone()),
                messages: forked.messages.clone(),
                system_prompt: forked
                    .system_prompt
                    .as_ref()
                    .map(|s| codewhale_models::SystemPrompt::Text(s.clone())),
                model: forked.metadata.model.clone(),
                workspace: app.workspace.clone(),
                mode,
            },
        })
    }

    fn fresh_session(&mut self, force: bool) -> Result<SessionNewReceipt, String> {
        let mut app = self.host.app.borrow_mut();
        if !force {
            let mut blockers: Vec<&'static str> = Vec::new();
            if !app.input.trim().is_empty() {
                blockers.push("the composer has unsent text");
            }
            if !app.queued_messages.is_empty() || app.queued_draft.is_some() {
                blockers.push("queued messages are pending");
            }
            if !blockers.is_empty() {
                return Err(format!(
                    "Cannot start a new session while {}. Run `/new --force` to discard pending work and start a fresh session.",
                    blockers.join(", ")
                ));
            }
        }

        let new_id = uuid::Uuid::new_v4().to_string();
        let queue_transition = crate::tui::ui::prepare_offline_queue_transition(&app, &new_id)?;
        if !crate::commands::groups::core::reset_conversation_state(&mut app) {
            return Err(
                "Could not start a new session because Work state is busy; retry in a moment."
                    .to_string(),
            );
        }
        crate::tui::ui::install_offline_queue_transition(&mut app, queue_transition);
        app.clear_input();
        app.session_artifacts.clear();
        app.session_context_references.clear();
        app.tool_evidence.clear();
        app.current_session_id = Some(new_id.clone());
        app.current_session_metadata = None;
        app.session_title = Some(crate::session_manager::DEFAULT_SESSION_TITLE.to_string());
        // A new session has no tab/window title override yet; the `title`
        // config default still applies.
        app.window_title = None;
        app.scroll_to_bottom();
        let mode = to_command_mode(app.mode);
        Ok(SessionNewReceipt {
            truncated_id: crate::session_manager::truncate_id(&new_id).to_string(),
            sync: SessionSyncPayload {
                session_id: Some(new_id),
                messages: Vec::new(),
                system_prompt: None,
                model: app.model.clone(),
                workspace: app.workspace.clone(),
                mode,
            },
        })
    }

    fn load_session(&mut self, path: &str) -> Result<PathBuf, String> {
        let app = self.host.app.borrow();
        let load_path = if path.contains('/') || path.contains('\\') {
            PathBuf::from(path)
        } else {
            app.workspace.join(path)
        };

        let content = match std::fs::read_to_string(&load_path) {
            Ok(c) => c,
            Err(e) => {
                return Err(format!("Failed to read session file: {e}"));
            }
        };

        let _session: crate::session_manager::SavedSession = match serde_json::from_str(&content) {
            Ok(s) => s,
            Err(e) => {
                return Err(format!("Failed to parse session file: {e}"));
            }
        };
        Ok(load_path)
    }

    fn open_picker(&mut self, preselected: Option<String>) {
        let mut app = self.host.app.borrow_mut();
        // Materialize the picker inputs before mutating the view stack so the
        // `RefCell` borrow of `App` is not simultaneously mutable and shared.
        let workspace = app.workspace.clone();
        let ui_locale = app.ui_locale;
        match preselected {
            Some(session_id) => {
                app.view_stack.push(
                    crate::tui::session_picker::SessionPickerView::new_selecting(
                        &workspace,
                        ui_locale,
                        &session_id,
                    ),
                );
            }
            None => {
                app.view_stack
                    .push(crate::tui::session_picker::SessionPickerView::new(
                        &workspace, ui_locale,
                    ));
            }
        }
    }

    fn set_archived(
        &mut self,
        session_id: &str,
        archived: bool,
    ) -> Result<SessionArchiveReceipt, String> {
        let verb = if archived { "archive" } else { "unarchive" };
        let mut app = self.host.app.borrow_mut();
        let manager = match crate::session_manager::SessionManager::default_location() {
            Ok(manager) => manager,
            Err(err) => {
                return Err(format!("could not open sessions directory: {err}"));
            }
        };
        match manager.set_session_archived(
            session_id,
            archived,
            crate::session_manager::SessionMutator::Owner,
        ) {
            Ok(metadata) => {
                if let Some(cached) = app.current_session_metadata.as_mut()
                    && cached.id == metadata.id
                {
                    cached.archived = metadata.archived;
                }
                Ok(SessionArchiveReceipt {
                    truncated_id: crate::session_manager::truncate_id(&metadata.id).to_string(),
                    title: metadata.title,
                })
            }
            Err(err) => Err(format!("{verb} failed: {err}")),
        }
    }

    fn prune_sessions(&mut self, days: u64) -> Result<usize, String> {
        let app = self.host.app.borrow();
        let manager = match crate::session_manager::SessionManager::default_location() {
            Ok(m) => m,
            Err(err) => {
                return Err(format!("could not open sessions directory: {err}"));
            }
        };

        let max_age = std::time::Duration::from_secs(days.saturating_mul(24 * 60 * 60));
        // Never prune the active session, even if its timestamp is stale (a
        // just-resumed session isn't re-saved until its first post-resume write).
        let keep = app.current_session_id.as_deref();
        manager
            .prune_sessions_older_than_keeping(max_age, keep)
            .map_err(|err| format!("prune failed: {err}"))
    }
}

// ---------------------------------------------------------------------------
// FEAT-024 Phase 4: relocated host machinery for the control slice.
//
// These helpers were extracted from the legacy `/remote-env` command body
// when that file became portable; they are host-owned and stay in TUI (the
// future movable group never names them).
// ---------------------------------------------------------------------------

const HOSTED_WORK_URL: &str = "https://app.codewhale.net/work";
const MAX_GIT_VALUE_BYTES: usize = 4 * 1024;

/// Validated hosted-work Git target (repo slug + checked-out branch).
#[derive(Debug, Clone, PartialEq, Eq)]
struct RemoteEnvTarget {
    repo: String,
    branch: String,
}

/// Resolve the hosted-work launcher target for a workspace: read the origin
/// URL and symbolic branch, normalize the repository slug against the
/// allowlist, and encode the launcher URL. Credentials never appear in the
/// returned values.
fn resolve_target(workspace: &Path) -> Option<RemoteEnvTarget> {
    let origin = read_git_value(
        workspace,
        &["config", "--local", "--get", "remote.origin.url"],
    )?;
    let repo = normalize_repo_slug(&origin)?;
    let branch = read_git_value(workspace, &["symbolic-ref", "--quiet", "--short", "HEAD"])?;
    if !valid_branch_name(&branch) {
        return None;
    }
    Some(RemoteEnvTarget { repo, branch })
}

fn hosted_work_url(repo: &str, branch: &str) -> String {
    format!(
        "{HOSTED_WORK_URL}?repo={}&branch={}",
        urlencoding::encode(repo),
        urlencoding::encode(branch),
    )
}

fn read_git_value(workspace: &Path, args: &[&str]) -> Option<String> {
    let mut command = crate::dependencies::Git::command()?;
    let output = command.arg("-C").arg(workspace).args(args).output().ok()?;
    if !output.status.success()
        || output.stdout.is_empty()
        || output.stdout.len() > MAX_GIT_VALUE_BYTES
    {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?;
    let value = value.trim_end_matches(&['\r', '\n'][..]);
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn valid_branch_name(branch: &str) -> bool {
    if branch.is_empty() || branch.len() > MAX_GIT_VALUE_BYTES {
        return false;
    }
    let Some(mut command) = crate::dependencies::Git::command() else {
        return false;
    };
    command
        .args(["check-ref-format", "--branch"])
        .arg(branch)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn normalize_repo_slug(origin: &str) -> Option<String> {
    let origin = origin.trim();
    if origin.is_empty()
        || origin.len() > MAX_GIT_VALUE_BYTES
        || origin.chars().any(char::is_control)
    {
        return None;
    }

    let (host, path) = if starts_with_ascii_case(origin, "https://") {
        split_url_origin(&origin["https://".len()..], UrlScheme::Https)?
    } else if starts_with_ascii_case(origin, "ssh://") {
        split_url_origin(&origin["ssh://".len()..], UrlScheme::Ssh)?
    } else {
        split_scp_origin(origin)?
    };
    if !matches!(
        host.to_ascii_lowercase().as_str(),
        "github.com" | "cnb.cool"
    ) {
        return None;
    }
    normalize_repo_path(path)
}

#[derive(Debug, Clone, Copy)]
enum UrlScheme {
    Https,
    Ssh,
}

fn starts_with_ascii_case(value: &str, prefix: &str) -> bool {
    value
        .get(..prefix.len())
        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(prefix))
}

fn split_url_origin(origin: &str, scheme: UrlScheme) -> Option<(&str, &str)> {
    let (authority, path) = origin.split_once('/')?;
    if authority.is_empty() || path.is_empty() {
        return None;
    }
    let host_port = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let host = match host_port.rsplit_once(':') {
        Some((host, port))
            if !host.is_empty()
                && !port.is_empty()
                && port.bytes().all(|byte| byte.is_ascii_digit())
                && (matches!(scheme, UrlScheme::Ssh) || port == "443") =>
        {
            host
        }
        Some(_) => return None,
        None => host_port,
    };
    (!host.is_empty()).then_some((host, path))
}

fn split_scp_origin(origin: &str) -> Option<(&str, &str)> {
    let (authority, path) = origin.split_once(':')?;
    let (_, host) = authority.rsplit_once('@')?;
    if host.is_empty() || path.is_empty() {
        return None;
    }
    Some((host, path))
}

fn normalize_repo_path(path: &str) -> Option<String> {
    if path.chars().any(|ch| matches!(ch, '?' | '#' | '\\')) {
        return None;
    }
    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let mut parts = path.split('/');
    let namespace = parts.next()?;
    let repository = parts.next()?;
    if parts.next().is_some()
        || !valid_repo_component(namespace)
        || !valid_repo_component(repository)
    {
        return None;
    }
    Some(format!("{namespace}/{repository}"))
}

fn valid_repo_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 255
        && !matches!(value, "." | "..")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

// ---------------------------------------------------------------------------
// Session control adapter (FEAT-024 D4/D5)
//
// Sole host owner of concrete control machinery for the six control commands:
// relay snapshot reads (goal/plan/work/todo/compact-template), rename/title
// persistence (sanitization, checkpoint recovery, live synchronization, save,
// publication, redraw), resume routing/imports, remote-control state and the
// synchronous single-attempt browser launch, and hosted-work Git target
// resolution. Every delegate reproduces the baseline check/mutation order
// exactly (transition gate before resume I/O, save before publication,
// browser launch without retry/deferral) and returns portable
// projections/receipts or the exact host-error text the baseline surfaces.
// No `SessionManager`, saved-session/container type, `SessionPickerView`,
// remote-control service, Git wrapper, configuration, model/history type,
// lock, or host callback crosses the facet.
// ---------------------------------------------------------------------------
pub(crate) struct SessionControlAdapter<'a> {
    host: SharedCommandHost<'a>,
}

impl CommandSessionControlContext for SessionControlAdapter<'_> {
    fn transition_blocked(&self) -> bool {
        self.host.app.borrow().session_transition_blocked()
    }

    fn relay_projection(&self) -> RelayProjection {
        let app = self.host.app.borrow();
        let plan = match app.plan_state.try_lock() {
            Ok(plan) => {
                let snapshot = plan.snapshot();
                if snapshot.is_empty() {
                    PlanProjection::Absent
                } else {
                    PlanProjection::Sections(plan_snapshot_to_sections(&snapshot))
                }
            }
            Err(_) => PlanProjection::Busy,
        };
        let todos = match app.work_state_snapshot() {
            Ok(Some(state)) => match crate::todo_snapshot::todo_snapshot_body(&state.todos) {
                Some(body) => TodoProjection::Body(body),
                None => TodoProjection::Absent,
            },
            Ok(None) => TodoProjection::Absent,
            Err(_) => TodoProjection::Unavailable,
        };
        RelayProjection {
            compact_template: crate::prompts::COMPACT_TEMPLATE.to_string(),
            workspace: app.workspace.display().to_string(),
            mode: app.mode.label().to_string(),
            model: app.model_display_label(),
            goal_objective: app.goal.objective.clone(),
            goal_token_budget: app.goal.token_budget,
            todos,
            plan,
        }
    }

    fn open_resume_picker(&mut self) {
        let mut app = self.host.app.borrow_mut();
        let picker =
            crate::tui::session_picker::SessionPickerView::new(&app.workspace, app.ui_locale);
        app.view_stack.push(picker);
    }

    fn resolve_resume_source(&mut self, raw: &str) -> Result<ResumeSource, String> {
        // Baseline order: direct path (or `.json` existing path) first, then
        // workspace-relative, then session id/prefix, then inline container.
        let raw_path = PathBuf::from(raw);
        if raw_path.is_file() || (raw.ends_with(".json") && Path::new(raw).exists()) {
            return Ok(ResumeSource::File(raw_path));
        }
        let workspace_relative = {
            let app = self.host.app.borrow();
            let ws_path = app.workspace.join(raw);
            ws_path.is_file().then_some(ws_path)
        };
        if let Some(ws_path) = workspace_relative {
            return Ok(ResumeSource::File(ws_path));
        }
        let manager = match crate::session_manager::SessionManager::default_location() {
            Ok(m) => m,
            Err(e) => return Err(format!("could not open sessions directory: {e}")),
        };
        match manager
            .load_session(raw)
            .or_else(|_| manager.load_session_by_prefix(raw))
        {
            Ok(sess) => {
                let path = manager
                    .sessions_dir()
                    .join(format!("{}.json", sess.metadata.id));
                Ok(ResumeSource::Session {
                    load_path: path.exists().then_some(path),
                    truncated_id: crate::session_manager::truncate_id(&sess.metadata.id)
                        .to_string(),
                    title: sess.metadata.title,
                })
            }
            Err(e) => {
                if let Ok(container) = crate::session_tree::SessionImportContainer::from_json(raw) {
                    let mut app = self.host.app.borrow_mut();
                    let receipt = import_session_container(&mut app, container)?;
                    Ok(ResumeSource::Imported(receipt))
                } else {
                    Ok(ResumeSource::NotFound {
                        raw: raw.to_string(),
                        error: e.to_string(),
                    })
                }
            }
        }
    }

    fn import_session_file(&mut self, path: PathBuf) -> Result<ResumeImportReceipt, String> {
        let mut app = self.host.app.borrow_mut();
        import_foreign_file(&mut app, &path)
    }

    fn sanitize_session_title(&self, raw_title: &str) -> String {
        crate::session_manager::sanitize_session_title(raw_title)
    }

    fn rename_session(&mut self, new_title: &str) -> Result<SessionTitleReceipt, String> {
        let mut app = self.host.app.borrow_mut();
        let session_id = match &app.current_session_id {
            Some(id) => id.clone(),
            None => {
                return Err(
                    "No active session. Send a message first to start a session.".to_string(),
                );
            }
        };
        let manager = match crate::session_manager::SessionManager::default_location() {
            Ok(m) => m,
            Err(e) => return Err(format!("Could not open sessions directory: {e}")),
        };

        // Mirrors the baseline `/rename` write path exactly: load (with
        // first-snapshot recovery), sync live state, snapshot Work state,
        // carry context/artifacts/route/cost/model/workspace/mode metadata,
        // persist, then publish. Publication failures keep their post-save
        // partial-success semantics.
        let mut session = match manager.load_session(&session_id) {
            Ok(s) => s,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                match live_session_before_first_snapshot(&manager, &session_id, &app) {
                    Some(s) => s,
                    None => return Err(format!("Could not load session: {err}")),
                }
            }
            Err(e) => return Err(format!("Could not load session: {e}")),
        };
        session = crate::session_manager::update_session(
            session,
            &app.api_messages,
            u64::from(app.session.total_tokens),
            app.system_prompt.as_ref(),
        );
        session.work_state = match app.work_state_snapshot() {
            Ok(state) => state,
            Err(err) => {
                return Err(format!(
                    "Could not snapshot Work state before rename: {err}"
                ));
            }
        };
        session.context_references = app.session_context_references.clone();
        session.artifacts = app.session_artifacts.clone();
        session.last_auto_route = app.auto_route_for_persistence();
        session.metadata.model = app.model_selection_for_persistence();
        session
            .metadata
            .set_model_provider_route(app.api_provider.as_str(), app.provider_id_for_persistence());
        session.metadata.workspace.clone_from(&app.workspace);
        session.metadata.mode = Some(app.mode.as_setting().to_string());
        app.sync_cost_to_metadata(&mut session.metadata);
        session.metadata.title = new_title.to_string();

        match manager.save_session(&session) {
            Ok(_) => {
                app.current_session_metadata = Some(session.metadata.clone());
                app.session_title = Some(new_title.to_string());
                if let Err(err) = app.publish_pending_work_state() {
                    return Err(format!(
                        "Session renamed, but Work views were not published: {err}"
                    ));
                }
                Ok(SessionTitleReceipt {
                    title: new_title.to_string(),
                })
            }
            Err(e) => Err(format!("Could not save session: {e}")),
        }
    }

    fn title_report(&self) -> TitleReport {
        let app = self.host.app.borrow();
        let source = if app.window_title.is_some() {
            TitleSource::Session
        } else if app.title_default.is_some() {
            TitleSource::ConfigDefault
        } else {
            TitleSource::None
        };
        TitleReport {
            effective: app.window_title_prefix().unwrap_or("unset").to_string(),
            source,
        }
    }

    fn set_window_title(&mut self, title: String) -> Result<(), String> {
        let mut app = self.host.app.borrow_mut();
        persist_window_title(&mut app, Some(title))
    }

    fn clear_window_title(&mut self) -> Result<(), String> {
        let mut app = self.host.app.borrow_mut();
        persist_window_title(&mut app, None)
    }

    fn remote_status(&self) -> String {
        self.host.app.borrow().remote_control.status_line()
    }

    fn remote_link(&self) -> Option<RemoteLink> {
        let app = self.host.app.borrow();
        let url = app.remote_control.run_url()?.to_string();
        Some(RemoteLink {
            computer_url: app.remote_control.computer_url().map(str::to_string),
            url,
        })
    }

    fn remote_browser_open(&self) -> RemoteOpenOutcome {
        let app = self.host.app.borrow();
        let Some(url) = app.remote_control.run_url().map(str::to_string) else {
            return RemoteOpenOutcome::NoLink;
        };
        // Synchronous single attempt through the authoritative URL-opening
        // helper; never retried and never deferred to an external-URL action.
        let launched = crate::utils::open_url(&url).is_ok();
        map_browser_open_result(url, launched)
    }

    fn remote_start_info(&self) -> RemoteStartInfo {
        let app = self.host.app.borrow();
        RemoteStartInfo {
            connecting: app.is_loading || app.dispatch_in_flight,
        }
    }

    fn remote_stop_refusal(&self) -> Option<String> {
        self.host.app.borrow().remote_control.stop_refusal().clone()
    }

    fn resolve_hosted_work_target(&self) -> Option<HostedWorkTarget> {
        let app = self.host.app.borrow();
        let target = resolve_target(&app.workspace)?;
        let url = hosted_work_url(&target.repo, &target.branch);
        Some(HostedWorkTarget {
            url,
            repo: target.repo,
            branch: target.branch,
        })
    }
}

/// Persist an already sanitized window title through the baseline host path.
/// Manager resolution intentionally precedes active-session lookup, matching
/// the original `/title` error precedence for both set and clear operations.
fn persist_window_title(app: &mut App, title: Option<String>) -> Result<(), String> {
    let manager = match crate::session_manager::SessionManager::default_location() {
        Ok(manager) => manager,
        Err(error) => return Err(format!("Could not open sessions directory: {error}")),
    };
    let session_id = match &app.current_session_id {
        Some(id) => id.clone(),
        None => {
            return Err("No active session. Send a message first to start a session.".to_string());
        }
    };
    let mut session = match manager.load_session(&session_id) {
        Ok(session) => session,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            match live_session_before_first_snapshot(&manager, &session_id, app) {
                Some(session) => session,
                None => return Err(format!("Could not load session: {error}")),
            }
        }
        Err(error) => return Err(format!("Could not load session: {error}")),
    };
    session = crate::session_manager::update_session(
        session,
        &app.api_messages,
        u64::from(app.session.total_tokens),
        app.system_prompt.as_ref(),
    );
    session.work_state = match app.work_state_snapshot() {
        Ok(state) => state,
        Err(error) => {
            return Err(format!(
                "Could not snapshot Work state before setting title: {error}"
            ));
        }
    };
    session.context_references = app.session_context_references.clone();
    session.artifacts = app.session_artifacts.clone();
    session.last_auto_route = app.auto_route_for_persistence();
    session.metadata.model = app.model_selection_for_persistence();
    session
        .metadata
        .set_model_provider_route(app.api_provider.as_str(), app.provider_id_for_persistence());
    session.metadata.workspace.clone_from(&app.workspace);
    session.metadata.mode = Some(app.mode.as_setting().to_string());
    app.sync_cost_to_metadata(&mut session.metadata);
    session.window_title.clone_from(&title);

    match manager.save_session(&session) {
        Ok(_) => {
            app.window_title = title;
            // The render loop syncs the resolved prefix into the terminal
            // title; force a frame so the change lands immediately.
            app.needs_redraw = true;
            if let Err(error) = app.publish_pending_work_state() {
                return Err(format!(
                    "Window title saved, but Work views were not published: {error}"
                ));
            }
            Ok(())
        }
        Err(error) => Err(format!("Could not save session: {error}")),
    }
}

/// Map one synchronous browser-launch attempt to its portable outcome.
/// Split out so the delegate's success/failure branches are unit-provable
/// without spawning a real browser (utils tests cover the launcher itself).
fn map_browser_open_result(url: String, launched: bool) -> RemoteOpenOutcome {
    if launched {
        RemoteOpenOutcome::Opened { url }
    } else {
        RemoteOpenOutcome::LaunchFailed { url }
    }
}

fn plan_snapshot_to_sections(snapshot: &crate::tools::plan::PlanSnapshot) -> PlanSections {
    PlanSections {
        title: snapshot.title.clone(),
        objective: snapshot.objective.clone(),
        context_summary: snapshot.context_summary.clone(),
        explanation: snapshot.explanation.clone(),
        sources_used: snapshot.sources_used.clone(),
        critical_files: snapshot.critical_files.clone(),
        constraints: snapshot.constraints.clone(),
        recommended_approach: snapshot.recommended_approach.clone(),
        verification_plan: snapshot.verification_plan.clone(),
        risks_and_unknowns: snapshot.risks_and_unknowns.clone(),
        handoff_packet: snapshot.handoff_packet.clone(),
        items: snapshot
            .items
            .iter()
            .map(|item| PlanStep {
                status: match &item.status {
                    crate::tools::plan::StepStatus::Pending => PlanStepStatus::Pending,
                    crate::tools::plan::StepStatus::InProgress => PlanStepStatus::InProgress,
                    crate::tools::plan::StepStatus::Completed => PlanStepStatus::Completed,
                },
                text: item.step.clone(),
            })
            .collect(),
    }
}

/// Recover the session document for a live turn that has not completed (and
/// therefore persisted) its first snapshot yet (#5430). Mirrors the legacy
/// `/rename`/`/title` recovery exactly.
fn live_session_before_first_snapshot(
    manager: &crate::session_manager::SessionManager,
    session_id: &str,
    app: &App,
) -> Option<crate::session_manager::SavedSession> {
    if let Ok(Some(checkpoint)) = manager.load_session_checkpoint(session_id) {
        return Some(checkpoint);
    }
    Some(
        crate::session_manager::create_saved_session_with_id_and_mode(
            session_id.to_string(),
            &app.api_messages,
            &app.model_selection_for_persistence(),
            &app.workspace,
            u64::from(app.session.total_tokens),
            app.system_prompt.as_ref(),
            Some(app.mode.as_setting()),
        ),
    )
}

/// `/resume <file>` import: read, parse a container or plain saved session,
/// and apply it atomically. Errors are the exact baseline text.
fn import_foreign_file(app: &mut App, path: &Path) -> Result<ResumeImportReceipt, String> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            return Err(format!(
                "failed to read import file {}: {e}",
                path.display()
            ));
        }
    };
    if let Ok(container) = crate::session_tree::SessionImportContainer::from_json(&content) {
        return import_session_container(app, container);
    }
    if let Ok(foreign) = serde_json::from_str::<crate::session_manager::SavedSession>(&content) {
        let container = foreign.export_container("foreign");
        return import_session_container(app, container);
    }
    Err(format!(
        "File {} is not a recognized session export",
        path.display()
    ))
}

/// Apply a parsed foreign container: persist, mutate the active session,
/// select it in a fresh picker, and return the portable receipt.
fn import_session_container(
    app: &mut App,
    container: crate::session_tree::SessionImportContainer,
) -> Result<ResumeImportReceipt, String> {
    let manager = match crate::session_manager::SessionManager::default_location() {
        Ok(m) => m,
        Err(e) => return Err(format!("could not open sessions directory: {e}")),
    };
    let model = app.model.clone();
    let workspace = app.workspace.clone();
    let imported =
        match crate::session_manager::SavedSession::import_foreign(container, workspace, model) {
            Ok(s) => s,
            Err(e) => return Err(format!("foreign import failed: {e}")),
        };
    let new_id = imported.metadata.id.clone();
    let queue_transition = crate::tui::ui::prepare_offline_queue_transition(app, &new_id)?;
    if let Err(e) = manager.save_session(&imported) {
        return Err(format!("imported session could not be saved: {e}"));
    }
    crate::tui::ui::install_offline_queue_transition(app, queue_transition);
    app.current_session_id = Some(new_id.clone());
    app.current_session_metadata = Some(imported.metadata.clone());
    app.restore_api_messages(
        imported.messages.clone(),
        &imported.journal_message_stamps(),
    );
    let picker = crate::tui::session_picker::SessionPickerView::new_selecting(
        &app.workspace,
        app.ui_locale,
        &new_id,
    );
    app.view_stack.push(picker);
    Ok(ResumeImportReceipt {
        truncated_id: crate::session_manager::truncate_id(&new_id).to_string(),
        entry_count: imported
            .journal
            .as_ref()
            .map(|journal| journal.entries.len())
            .unwrap_or(0),
        leaf_display: imported.leaf_id.as_deref().unwrap_or("(none)").to_string(),
    })
}

/// Session identity, messages, queue operations, and token totals.
pub(crate) struct SessionAdapter<'a> {
    host: SharedCommandHost<'a>,
}

impl CommandSessionContext for SessionAdapter<'_> {
    fn session_id(&self) -> Option<String> {
        self.host.app.borrow().current_session_id.clone()
    }

    fn api_messages(&self) -> Vec<Message> {
        self.host.app.borrow().api_messages.clone()
    }

    fn add_message(&mut self, message: Message) {
        self.host.app.borrow_mut().push_api_message(message);
    }

    fn queued_message_count(&self) -> usize {
        self.host.app.borrow().queued_message_count()
    }

    fn remove_queued_message(&mut self, index: usize) -> Result<(), String> {
        self.host
            .app
            .borrow_mut()
            .remove_queued_message(index)
            .map(|_| ())
            .ok_or_else(|| format!("queued message index {index} out of bounds"))
    }

    fn total_tokens(&self) -> u64 {
        u64::from(self.host.app.borrow().session.total_tokens)
    }
}

/// Model selection, provider identity, effort, and fallback chain.
pub(crate) struct ModelAdapter<'a> {
    host: SharedCommandHost<'a>,
}

impl CommandModelContext for ModelAdapter<'_> {
    fn current_model(&self) -> String {
        self.host.app.borrow().model.clone()
    }

    fn auto_model(&self) -> bool {
        self.host.app.borrow().auto_model
    }

    fn set_model_selection(&mut self, model: String, provider: Option<CommandProviderId>) {
        let mut app = self.host.app.borrow_mut();
        if let Some(provider) = provider {
            let identity = provider.0;
            let provider = crate::config::ApiProvider::parse(&identity)
                .unwrap_or(crate::config::ApiProvider::Custom);
            app.set_provider_identity(provider, identity);
        }
        app.set_model_selection(model);
    }

    fn reasoning_effort(&self) -> CommandReasoningEffort {
        to_command_effort(self.host.app.borrow().reasoning_effort)
    }

    fn provider_identity(&self) -> Option<CommandProviderId> {
        let app = self.host.app.borrow();
        let identity = app.provider_identity_for_persistence();
        (!identity.trim().is_empty()).then(|| to_provider_id(identity))
    }

    fn fallback_chain(&self) -> Vec<CommandProviderId> {
        self.host
            .app
            .borrow()
            .fallback_chain_entries()
            .into_iter()
            .map(|(_, provider, _)| to_provider_id(provider.as_str()))
            .collect()
    }
}

/// Cost display and accounting operations delegated to App's cost authority.
pub(crate) struct CostAdapter<'a> {
    host: SharedCommandHost<'a>,
}

fn command_cost_estimate(amount: f64, currency: CommandCurrency) -> crate::pricing::CostEstimate {
    match currency {
        CommandCurrency::Usd => crate::pricing::CostEstimate {
            usd: amount,
            cny: 0.0,
        },
        CommandCurrency::Cny => crate::pricing::CostEstimate {
            usd: 0.0,
            cny: amount,
        },
    }
}

impl CommandCostContext for CostAdapter<'_> {
    fn display_currency(&self) -> CommandCurrency {
        let app = self.host.app.borrow();
        to_command_currency(app.cost_display_currency(app.cost_currency))
    }

    fn session_cost_for_currency(&self, currency: CommandCurrency) -> f64 {
        self.host
            .app
            .borrow()
            .session_cost_for_currency(from_command_currency(currency))
    }

    fn subagent_cost_for_currency(&self, currency: CommandCurrency) -> f64 {
        self.host
            .app
            .borrow()
            .subagent_cost_for_currency(from_command_currency(currency))
    }

    fn accrue_cost_estimate(&mut self, amount: f64, currency: CommandCurrency) {
        self.host
            .app
            .borrow_mut()
            .accrue_session_cost_estimate(command_cost_estimate(amount, currency));
    }

    fn record_turn_cost(
        &mut self,
        amount: f64,
        currency: CommandCurrency,
        route_receipt: Option<String>,
    ) {
        let mut app = self.host.app.borrow_mut();
        app.accrue_session_cost_estimate(command_cost_estimate(amount, currency));
        if let Some(receipt) = route_receipt {
            app.record_turn_cost_route_receipt(receipt);
        }
    }
}

/// Operating mode, approval posture, shell access, and policy lock.
pub(crate) struct ModePolicyAdapter<'a> {
    host: SharedCommandHost<'a>,
}

impl CommandModePolicyContext for ModePolicyAdapter<'_> {
    fn mode(&self) -> CommandMode {
        to_command_mode(self.host.app.borrow().mode)
    }

    fn set_mode(&mut self, mode: CommandMode) {
        self.host.app.borrow_mut().set_mode(from_command_mode(mode));
    }

    fn approval_mode(&self) -> CommandApprovalMode {
        to_command_approval(self.host.app.borrow().approval_mode)
    }

    fn allow_shell(&self) -> bool {
        self.host.app.borrow().allow_shell
    }

    fn set_shell_access(&mut self, allow: bool) {
        self.host.app.borrow_mut().set_agent_shell_access(allow);
    }

    fn policy_locked(&self) -> bool {
        self.host.app.borrow().approval_policy_locked()
    }
}

/// Read access to the effective system prompt.
pub(crate) struct SystemPromptAdapter<'a> {
    host: SharedCommandHost<'a>,
}

impl CommandSystemPromptContext for SystemPromptAdapter<'_> {
    fn system_prompt(&self) -> Option<SystemPrompt> {
        self.host.app.borrow().system_prompt.clone()
    }
}

/// Active skill identity and authoritative skill-cache refresh.
pub(crate) struct SkillsAdapter<'a> {
    host: SharedCommandHost<'a>,
}

impl CommandSkillsContext for SkillsAdapter<'_> {
    fn active_skill(&self) -> Option<String> {
        self.host.app.borrow().active_skill.clone()
    }

    fn active_skill_provenance(&self) -> Option<String> {
        self.host
            .app
            .borrow()
            .active_skill_provenance
            .as_ref()
            .map(|authority| authority.plugin_name.clone())
    }

    fn refresh_skill_cache(&mut self) {
        self.host.app.borrow_mut().refresh_skill_cache();
    }
}

/// Workspace path and bounded serialized work-state snapshot.
pub(crate) struct WorkspaceAdapter<'a> {
    host: SharedCommandHost<'a>,
}

impl CommandWorkspaceContext for WorkspaceAdapter<'_> {
    fn workspace(&self) -> PathBuf {
        self.host.app.borrow().workspace.clone()
    }

    fn work_state_snapshot(&self) -> Result<Option<String>, String> {
        self.host.app.borrow().work_state_snapshot().map(|state| {
            state.and_then(|state| crate::todo_snapshot::todo_snapshot_body(&state.todos))
        })
    }

    fn operation_digest(&mut self) -> Result<String, String> {
        let app = self.host.app.borrow();
        let Some(work) = app.runtime_services.work.as_ref() else {
            return Ok("No active operations or to-do items.".to_string());
        };
        match work.capture(app.current_session_id.as_deref()) {
            Ok(snapshot) => Ok(crate::work_graph::format_operation_digest(
                snapshot.as_ref(),
            )),
            Err(error) => Err(format!(
                "Operation digest is temporarily unavailable: {error}"
            )),
        }
    }
}

/// Stable-key translation adapter (FEAT-018 D3).
///
/// Maps stable snake_case utility message keys to the current catalog and
/// preserves the existing English fallback for intentionally incomplete locale
/// packs. Unknown keys and invalid replacement contracts fail safely; a raw
/// lookup key is never exposed.
pub(crate) struct PresentationAdapter<'a> {
    host: SharedCommandHost<'a>,
}

impl CommandPresentationContext for PresentationAdapter<'_> {
    fn translate(&self, key: &str, replacements: &[(&str, &str)]) -> Result<String, String> {
        let Some(message_id) = key_to_utility_message_id(key)
            .or_else(|| key_to_project_message_id(key))
            .or_else(|| key_to_plugin_message_id(key))
            .or_else(|| key_to_session_message_id(key))
        else {
            return Err("unknown translation key".to_string());
        };
        let locale = self.host.app.borrow().ui_locale;
        let template = tr(locale, message_id);
        apply_named_replacements(&template, replacements)
            .ok_or_else(|| "invalid translation replacement contract".to_string())
    }
}

/// Resolve a stable session-control message key to the current catalog id
/// (FEAT-024 D6). Only `/remote-env` makes runtime catalog calls; the other
/// five control commands keep their metadata-only `description_key` usage.
pub(crate) fn key_to_session_message_id(key: &str) -> Option<MessageId> {
    Some(match key {
        "cmd_remote_env_overview" => MessageId::CmdRemoteEnvOverview,
        "cmd_remote_env_opening" => MessageId::CmdRemoteEnvOpening,
        "cmd_remote_env_unavailable" => MessageId::CmdRemoteEnvUnavailable,
        "cmd_remote_env_source_custody_policy" => MessageId::CmdRemoteEnvSourceCustodyPolicy,
        "cmd_remote_env_browser_label" => MessageId::CmdRemoteEnvBrowserLabel,
        _ => return None,
    })
}

/// Resolve a stable plugin message key to the current catalog id (FEAT-020 D5).
///
/// Every plugin-group catalog message uses a stable snake_case key; the TUI
/// adapter maps it to the current `MessageId` value and preserves the
/// authoritative English fallback. Unknown keys fail safely.
pub(crate) fn key_to_plugin_message_id(key: &str) -> Option<MessageId> {
    Some(match key {
        "cmd_plugin_action_failed" => MessageId::CmdPluginActionFailed,
        "cmd_plugin_bundle_detail" => MessageId::CmdPluginBundleDetail,
        "cmd_plugin_bundle_diagnostics_header" => MessageId::CmdPluginBundleDiagnosticsHeader,
        "cmd_plugin_bundle_list_header" => MessageId::CmdPluginBundleListHeader,
        "cmd_plugin_bundle_mutation_success" => MessageId::CmdPluginBundleMutationSuccess,
        "cmd_plugin_bundle_none_found" => MessageId::CmdPluginBundleNoneFound,
        "cmd_plugin_bundle_not_found" => MessageId::CmdPluginBundleNotFound,
        "cmd_plugin_bundle_reloaded" => MessageId::CmdPluginBundleReloaded,
        "cmd_plugin_bundle_usage" => MessageId::CmdPluginBundleUsage,
        "cmd_plugin_detail_description" => MessageId::CmdPluginDetailDescription,
        "cmd_plugin_detail_approval" => MessageId::CmdPluginDetailApproval,
        "cmd_plugin_detail_path" => MessageId::CmdPluginDetailPath,
        "cmd_plugin_detail_schema" => MessageId::CmdPluginDetailSchema,
        "cmd_plugin_legacy_list_header" => MessageId::CmdPluginLegacyListHeader,
        "cmd_plugin_none_found" => MessageId::CmdPluginNoneFound,
        "cmd_plugin_not_found" => MessageId::CmdPluginNotFound,
        "plugin_kimi_applicable" => MessageId::PluginKimiApplicable,
        "plugin_kimi_candidate_changed" => MessageId::PluginKimiCandidateChanged,
        "plugin_kimi_candidate_details" => MessageId::PluginKimiCandidateDetails,
        "plugin_kimi_candidate_missing" => MessageId::PluginKimiCandidateMissing,
        "plugin_kimi_candidate_summary" => MessageId::PluginKimiCandidateSummary,
        "plugin_kimi_directory_name_mismatch" => MessageId::PluginKimiDirectoryNameMismatch,
        "plugin_kimi_entry_canonicalize_failed" => MessageId::PluginKimiEntryCanonicalizeFailed,
        "plugin_kimi_entry_inspect_failed" => MessageId::PluginKimiEntryInspectFailed,
        "plugin_kimi_entry_limit" => MessageId::PluginKimiEntryLimit,
        "plugin_kimi_entry_links_refused" => MessageId::PluginKimiEntryLinksRefused,
        "plugin_kimi_entry_outside_root" => MessageId::PluginKimiEntryOutsideRoot,
        "plugin_kimi_entry_read_failed" => MessageId::PluginKimiEntryReadFailed,
        "plugin_kimi_hash_unavailable" => MessageId::PluginKimiHashUnavailable,
        "plugin_kimi_home_missing" => MessageId::PluginKimiHomeMissing,
        "plugin_kimi_inspection_footer" => MessageId::PluginKimiInspectionFooter,
        "plugin_kimi_license_unspecified" => MessageId::PluginKimiLicenseUnspecified,
        "plugin_kimi_managed_root_heading" => MessageId::PluginKimiManagedRootHeading,
        "plugin_kimi_manifest_invalid" => MessageId::PluginKimiManifestInvalid,
        "plugin_kimi_manifest_must_be_file" => MessageId::PluginKimiManifestMustBeFile,
        "plugin_kimi_manifest_unreadable" => MessageId::PluginKimiManifestUnreadable,
        "plugin_kimi_marketplace_gzip_tarball" => MessageId::PluginKimiMarketplaceGzipTarball,
        "kimi_zip_unsupported" => MessageId::PluginKimiMarketplaceZipUnsupported,
        "kimi_remote_archive_unsupported" => MessageId::PluginKimiMarketplaceRemoteUnsupported,
        "kimi_gzip_tarball_url" => MessageId::PluginKimiMarketplaceGzipTarball,
        "plugin_kimi_marketplace_remote_unsupported" => {
            MessageId::PluginKimiMarketplaceRemoteUnsupported
        }
        "plugin_kimi_marketplace_zip_unsupported" => MessageId::PluginKimiMarketplaceZipUnsupported,
        "plugin_kimi_mismatch_removed" => MessageId::PluginKimiMismatchRemoved,
        "plugin_kimi_mismatch_rollback_failed" => MessageId::PluginKimiMismatchRollbackFailed,
        "plugin_kimi_none_found" => MessageId::PluginKimiNoneFound,
        "plugin_kimi_not_applicable" => MessageId::PluginKimiNotApplicable,
        "plugin_kimi_rejected_heading" => MessageId::PluginKimiRejectedHeading,
        "plugin_kimi_rollback_destination_missing" => {
            MessageId::PluginKimiRollbackDestinationMissing
        }
        "plugin_kimi_root_canonicalize_failed" => MessageId::PluginKimiRootCanonicalizeFailed,
        "plugin_kimi_root_inspect_failed" => MessageId::PluginKimiRootInspectFailed,
        "plugin_kimi_root_list_failed" => MessageId::PluginKimiRootListFailed,
        "plugin_kimi_root_must_be_directory" => MessageId::PluginKimiRootMustBeDirectory,
        "plugin_kimi_usage" => MessageId::PluginKimiUsage,
        "plugin_kimi_user_plugin_directory" => MessageId::PluginKimiUserPluginDirectory,
        _ => return None,
    })
}

/// Resolve a stable utility message key to the current catalog id.
fn key_to_utility_message_id(key: &str) -> Option<MessageId> {
    Some(match key {
        "automation_usage" => MessageId::AutomationUsage,
        "mcp_recommended_unknown_id" => MessageId::McpRecommendedUnknownId,
        "mcp_recommendations_heading" => MessageId::McpRecommendationsHeading,
        "mcp_recommendations_safety" => MessageId::McpRecommendationsSafety,
        "mcp_recommendation_github" => MessageId::McpRecommendationGithub,
        "mcp_recommendation_chrome" => MessageId::McpRecommendationChrome,
        "mcp_recommendation_playwright" => MessageId::McpRecommendationPlaywright,
        "mcp_recommendation_cua" => MessageId::McpRecommendationCua,
        "mcp_recommendation_container_use" => MessageId::McpRecommendationContainerUse,
        _ => return None,
    })
}

/// Resolve a stable project message key to the current catalog id (FEAT-021 D5).
///
/// Only `/goal` uses runtime translations (`GoalControlAccepted`,
/// `GoalStatusIdleHint`); all four description keys resolve through the
/// metadata bridge (`key_to_message_id`) and do not require the presentation
/// facet.
pub(crate) fn key_to_project_message_id(key: &str) -> Option<MessageId> {
    Some(match key {
        "goal_control_accepted" => MessageId::GoalControlAccepted,
        "goal_status_idle_hint" => MessageId::GoalStatusIdleHint,
        _ => return None,
    })
}

/// Replace `{name}` placeholders with the supplied named values.
///
/// Returns `None` when the replacement set does not exactly cover every
/// placeholder in the template (missing, extra, or duplicate names).
fn apply_named_replacements(template: &str, replacements: &[(&str, &str)]) -> Option<String> {
    let supplied: std::collections::BTreeMap<&str, &str> = replacements.iter().copied().collect();
    if supplied.len() != replacements.len() {
        return None; // duplicate replacement name
    }
    let mut placeholders = std::collections::BTreeSet::new();
    let mut cursor = 0usize;
    while let Some(start) = template[cursor..].find('{') {
        let start = cursor + start;
        let Some(end) = template[start + 1..].find('}') else {
            break;
        };
        let end = start + 1 + end;
        let name = &template[start + 1..end];
        if !name.is_empty() {
            placeholders.insert(name);
        }
        cursor = end + 1;
    }
    if placeholders != supplied.keys().copied().collect() {
        return None;
    }
    let mut out = template.to_string();
    for (name, value) in replacements {
        out = out.replace(&format!("{{{name}}}"), value);
    }
    Some(out)
}

/// Atomic composer/media adapter (FEAT-018 D4).
///
/// Performs media validation and composer insertion as one host operation by
/// delegating to the authoritative image-validation and attachment behavior.
pub(crate) struct MediaAdapter<'a> {
    host: SharedCommandHost<'a>,
}

impl CommandMediaContext for MediaAdapter<'_> {
    fn attach_media(&mut self, resolved_path: &Path) -> Result<MediaAttachmentReceipt, String> {
        let Ok(path) = resolved_path.canonicalize() else {
            return Err(format!("Attachment not found: {}", resolved_path.display()));
        };
        if !path.is_file() {
            return Err(format!("Attachment is not a file: {}", path.display()));
        }
        let Some(kind) = media_kind(&path) else {
            return Err(
                "Unsupported attachment type. /attach is for image/video paths; use @path for \
                 text files or directories."
                    .to_string(),
            );
        };
        if kind == "image"
            && let Err(error) = crate::image_attach::attach_image_from_path(&path)
        {
            return Err(error.to_string());
        }
        let mut app = self.host.app.borrow_mut();
        app.insert_media_attachment(kind, &path, None);
        Ok(MediaAttachmentReceipt {
            kind: kind.to_string(),
            path,
        })
    }
}

/// Classify a media path by extension (image or video).
fn media_kind(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "tif" | "tiff" | "ppm" => Some("image"),
        "mp4" | "mov" | "m4v" | "webm" | "avi" | "mkv" => Some("video"),
        _ => None,
    }
}

/// Memory host-data adapter (FEAT-019 D1).
///
/// Derives the authoritative native store exactly like the legacy `/memory`
/// handler (`from_global_path` on the app memory path, falling back to a
/// `memory` root beside it) and converts every host value/error to a portable
/// contract value before it crosses the boundary. All methods are `&self` and
/// borrow `App` only for the duration of one call; workspace state is passed
/// per call and never retained by the facet (D8).
pub(crate) struct MemoryAdapter<'a> {
    host: SharedCommandHost<'a>,
}

/// Derive the authoritative native-memory store from the resolved user-memory
/// file path, mirroring the pre-migration `/memory` handler exactly.
fn native_store_from_memory_path(memory_path: &Path) -> crate::native_memory::NativeMemoryStore {
    if let Some(store) = crate::native_memory::NativeMemoryStore::from_global_path(memory_path) {
        return store;
    }
    let root = memory_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("memory");
    crate::native_memory::NativeMemoryStore::new(root)
}

/// Convert a TUI-owned native hit into the portable contract hit. Only the
/// semantic fields the handler consumes for rendering cross the boundary (D2).
fn portable_hit(hit: crate::native_memory::MemoryHit) -> MemoryHit {
    MemoryHit {
        source: hit.source,
        line_start: hit.line_start,
        line_end: hit.line_end,
        text: hit.text,
    }
}

impl CommandMemoryContext for MemoryAdapter<'_> {
    fn memory_path(&self) -> PathBuf {
        self.host.app.borrow().memory_path.clone()
    }

    fn memory_enabled(&self) -> bool {
        self.host.app.borrow().use_memory
    }

    fn status(&self) -> Result<MemoryStatus, String> {
        let app = self.host.app.borrow();
        let store = native_store_from_memory_path(&app.memory_path);
        Ok(MemoryStatus {
            root: store.root().to_path_buf(),
            source: store.global_path(),
            index: store.index_path(),
        })
    }

    fn path(&self) -> Result<PathBuf, String> {
        let app = self.host.app.borrow();
        Ok(native_store_from_memory_path(&app.memory_path)
            .root()
            .to_path_buf())
    }

    fn workspace_id(&self, workspace: &Path) -> Result<String, String> {
        match crate::native_memory::NativeMemoryStore::workspace_id(workspace) {
            Ok(Some(id)) => Ok(id),
            Ok(None) => {
                Err("workspace memory requires a git repository with an origin".to_string())
            }
            Err(err) => Err(format!("failed to resolve workspace identity: {err}")),
        }
    }

    fn search(
        &self,
        workspace: &Path,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemoryHit>, String> {
        let app = self.host.app.borrow();
        let store = native_store_from_memory_path(&app.memory_path);
        match store.search_for_workspace(workspace, query, limit) {
            Ok(hits) => Ok(hits.into_iter().map(portable_hit).collect()),
            Err(err) => Err(err.to_string()),
        }
    }

    fn remember(
        &self,
        target: MemoryRememberTarget,
        note: &str,
    ) -> Result<MemoryRemembered, String> {
        let app = self.host.app.borrow();
        let store = native_store_from_memory_path(&app.memory_path);
        let (scope, workspace_id) = match target {
            MemoryRememberTarget::Global => (crate::native_memory::MemoryScope::Global, None),
            MemoryRememberTarget::Workspace { workspace_id } => (
                crate::native_memory::MemoryScope::Workspace,
                Some(workspace_id),
            ),
        };
        match store.remember(scope, workspace_id.as_deref(), note) {
            Ok(hit) => Ok(MemoryRemembered {
                source: hit.source,
                line_start: hit.line_start,
            }),
            Err(err) => Err(err.to_string()),
        }
    }

    fn import(&self) -> Result<MemoryImportOutcome, String> {
        let app = self.host.app.borrow();
        let store = native_store_from_memory_path(&app.memory_path);
        let legacy_path = store
            .root()
            .parent()
            .map(|parent| parent.join("memory.md"))
            .unwrap_or_else(|| app.memory_path.clone());
        match store.import_legacy(&legacy_path) {
            Ok(true) => Ok(MemoryImportOutcome::Imported {
                destination: store.global_path(),
            }),
            Ok(false) => Ok(MemoryImportOutcome::Skipped),
            Err(err) => Err(err.to_string()),
        }
    }

    fn get(&self, workspace: &Path, id: i64) -> Result<MemoryGetOutcome, String> {
        let app = self.host.app.borrow();
        let store = native_store_from_memory_path(&app.memory_path);
        match store.get_for_workspace(workspace, id) {
            Ok(Some(hit)) => Ok(MemoryGetOutcome::Found(portable_hit(hit))),
            Ok(None) => Ok(MemoryGetOutcome::NotFound),
            Err(err) => Err(err.to_string()),
        }
    }

    fn export(&self) -> Result<MemoryExport, String> {
        let app = self.host.app.borrow();
        let store = native_store_from_memory_path(&app.memory_path);
        match store.export() {
            Ok(content) => Ok(MemoryExport { content }),
            Err(err) => Err(err.to_string()),
        }
    }

    fn reindex(&self) -> Result<MemoryReindex, String> {
        let app = self.host.app.borrow();
        let store = native_store_from_memory_path(&app.memory_path);
        match store.reindex() {
            Ok(entry_count) => Ok(MemoryReindex { entry_count }),
            Err(err) => Err(err.to_string()),
        }
    }

    fn delete(&self, scope: MemoryDeleteScope) -> Result<MemoryDelete, String> {
        let app = self.host.app.borrow();
        let store = native_store_from_memory_path(&app.memory_path);
        let result = match scope {
            MemoryDeleteScope::All => store.delete_all(None, None),
            MemoryDeleteScope::Global => {
                store.delete_all(Some(crate::native_memory::MemoryScope::Global), None)
            }
        };
        result.map(|()| MemoryDelete).map_err(|err| err.to_string())
    }

    fn delete_workspace(&self, workspace: &Path) -> Result<MemoryDelete, String> {
        let app = self.host.app.borrow();
        let store = native_store_from_memory_path(&app.memory_path);
        match crate::native_memory::NativeMemoryStore::workspace_id(workspace) {
            Ok(Some(id)) => store
                .delete_all(
                    Some(crate::native_memory::MemoryScope::Workspace),
                    Some(&id),
                )
                .map(|()| MemoryDelete)
                .map_err(|err| err.to_string()),
            Ok(None) => {
                Err("workspace memory requires a git repository with an origin".to_string())
            }
            Err(err) => Err(format!("failed to resolve workspace identity: {err}")),
        }
    }
}

// ---------------------------------------------------------------------------
// Project host adapter (FEAT-021 D1/D3)
// ---------------------------------------------------------------------------

/// Concrete TUI host mapping for the project command group (FEAT-021 D1/D3).
///
/// The only place that touches `App` goal/share/LSP state, `config::config`
/// (cross-group LSP bridge), and the session manager. Every method borrows
/// `App` for one call and converts host values to portable contract values
/// before returning; the `/init` workspace path flows through the existing
/// `WORKSPACE` facet (D2), so no init-specific method exists here.
pub(crate) struct ProjectAdapter<'a> {
    host: SharedCommandHost<'a>,
}

/// Map the TUI-owned goal status onto the portable project status.
fn portable_goal_status(status: crate::tools::goal::GoalStatus) -> ProjectGoalStatus {
    match status {
        crate::tools::goal::GoalStatus::Active => ProjectGoalStatus::Active,
        crate::tools::goal::GoalStatus::Paused => ProjectGoalStatus::Paused,
        crate::tools::goal::GoalStatus::Complete => ProjectGoalStatus::Complete,
        crate::tools::goal::GoalStatus::Blocked => ProjectGoalStatus::Blocked,
    }
}

/// Map the durable session goal status onto the portable project status.
fn portable_session_goal_status(
    status: crate::session_manager::SessionGoalStatus,
) -> ProjectGoalStatus {
    match status {
        crate::session_manager::SessionGoalStatus::Active => ProjectGoalStatus::Active,
        crate::session_manager::SessionGoalStatus::Paused => ProjectGoalStatus::Paused,
        crate::session_manager::SessionGoalStatus::Complete => ProjectGoalStatus::Complete,
        crate::session_manager::SessionGoalStatus::Blocked => ProjectGoalStatus::Blocked,
    }
}

impl CommandProjectContext for ProjectAdapter<'_> {
    fn lsp_enabled(&self) -> bool {
        self.host.app.borrow().lsp_enabled
    }

    fn lsp_set(&mut self, enabled: bool) -> Result<(), String> {
        // Cross-group LSP behavior stays host-side (D3): the adapter owns the
        // `config::config::lsp_command` invocation. The portable handler
        // composes the byte-identical user-facing message from the typed
        // state, so the formatted result is intentionally not forwarded.
        let mut app = self.host.app.borrow_mut();
        let arg = if enabled { "on" } else { "off" };
        let _ = crate::commands::groups::config::config::lsp_command(&mut app, Some(arg));
        Ok(())
    }

    fn share_projection(&self) -> ProjectShareProjection {
        let app = self.host.app.borrow();
        ProjectShareProjection {
            history_is_empty: app.history.is_empty(),
            history_len: app.history.len(),
            model: app.model.clone(),
            mode_label: app.mode.label().to_string(),
        }
    }

    fn goal_state(&self) -> ProjectGoalState {
        let app = self.host.app.borrow();
        let pending_controls = !app.pending_goal_controls.is_empty();
        let last_known = app.last_known_goal_state.as_ref();
        ProjectGoalState {
            objective: app.goal.objective.clone(),
            status: portable_goal_status(app.goal.status),
            pause_reason: app
                .goal
                .pause_reason
                .map(|reason| reason.label().to_string()),
            started_at_elapsed_seconds: app.goal.started_at.map(|t| t.elapsed().as_secs()),
            time_used_seconds: app.goal.time_used_seconds,
            token_budget: app.goal.token_budget,
            tokens_used: app.goal.tokens_used,
            session_total_tokens: app.session.total_conversation_tokens,
            continuation_count: app.goal.continuation_count,
            pending_controls,
            last_known_objective: last_known.map(|goal| goal.objective.clone()),
            last_known_status: last_known.map(|goal| portable_session_goal_status(goal.status)),
            conversation_present: !app.api_messages.is_empty(),
            is_loading: app.is_loading,
            goal_continuation_waiting: app.goal_continuation_waiting,
        }
    }
}

// ---------------------------------------------------------------------------
// Skill group adapter (FEAT-022 D1/D3)
// ---------------------------------------------------------------------------

/// The single new skills-specific host adapter.
///
/// Owns every concrete skills touch: `App` skill state, `crate::skills`
/// discovery/mutation/install/recommend services, `crate::plugins` authority
/// verification, `SnapshotRepo`, config/network policy, and the async bridge
/// (`tokio::task::block_in_place`). Portable handlers never name these
/// subsystems (D3); every method returns portable contract values or safe
/// error text (D1).
pub(crate) struct SkillGroupAdapter<'a> {
    host: SharedCommandHost<'a>,
}

/// Bridge a sync slash-command handler back into the async ecosystem.
///
/// We are on the TUI's thread, which is part of the multi-threaded runtime;
/// `block_in_place` + `Handle::current().block_on` bridges sync handlers back
/// into the async ecosystem. Mirrors `groups/skills/skills.rs::run_async`;
/// the legacy copy is removed in Phase 4 when the handlers are ported.
fn run_async<F, T>(future: F) -> T
where
    F: std::future::Future<Output = T>,
{
    tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(future))
}

/// Read the active config knobs for the installer (network policy, max size,
/// registry URL). `Config::load` is cheap and `App` does not carry a `Config`;
/// on parse failure we fall back to defaults so the user still gets a
/// network-gated install rather than a silent crash. Mirrors
/// `groups/skills/skills.rs::installer_settings`.
fn installer_settings() -> (NetworkPolicy, u64, String) {
    let cfg = crate::config::Config::load(None, None).unwrap_or_default();
    let network = cfg
        .network
        .clone()
        .map(|policy| policy.into_runtime())
        .unwrap_or_default();
    let skills_cfg = cfg.skills.as_ref();
    let max_size = skills_cfg
        .and_then(|s| s.max_install_size_bytes)
        .unwrap_or(crate::skills::install::DEFAULT_MAX_SIZE_BYTES);
    let registry_url = skills_cfg
        .and_then(|s| s.registry_url.clone())
        .unwrap_or_else(|| crate::skills::install::DEFAULT_REGISTRY_URL.to_string());
    (network, max_size, registry_url)
}

/// Inspect an anyhow chain and surface a one-line hint pointing at the most
/// common cause of a registry fetch failure (DNS, refused, TLS, HTTP status,
/// timeout). Mirrors `groups/skills/skills.rs::registry_fetch_error_hint`.
fn registry_fetch_error_hint(err: &anyhow::Error) -> Option<&'static str> {
    let msg = format!("{err:#}").to_lowercase();
    if msg.contains("dns")
        || msg.contains("name resolution")
        || msg.contains("getaddrinfo")
        || msg.contains("nodename nor servname")
    {
        Some(
            "Hint: DNS lookup failed. Check internet/DNS connectivity, or override the registry URL in [skills] of ~/.codewhale/config.toml.",
        )
    } else if msg.contains("connection refused")
        || msg.contains("connection reset")
        || msg.contains("connection aborted")
    {
        Some(
            "Hint: connection refused/reset. The registry host may be unreachable from this network (corporate proxy, firewall, offline).",
        )
    } else if msg.contains("tls")
        || msg.contains("certificate")
        || msg.contains("ssl")
        || msg.contains("handshake")
    {
        Some(
            "Hint: TLS handshake failed. The system trust store may be missing the registry's CA, or a TLS-intercepting proxy is rewriting the certificate.",
        )
    } else if msg.contains(" 404") || msg.contains("not found") {
        Some(
            "Hint: registry URL returned 404. Verify the registry URL in [skills] of ~/.codewhale/config.toml.",
        )
    } else if msg.contains(" 401") || msg.contains(" 403") || msg.contains("forbidden") {
        Some(
            "Hint: registry returned an auth error. The registry may require credentials or have been moved.",
        )
    } else if msg.contains(" 429") || msg.contains("rate limit") || msg.contains("too many") {
        Some("Hint: rate-limited by the registry. Try again in a moment.")
    } else if msg.contains("timed out") || msg.contains("timeout") {
        Some("Hint: request timed out. Network may be slow or the registry host may be down.")
    } else {
        None
    }
}

/// Append the actionable hint to a registry fetch error. Mirrors
/// `groups/skills/skills.rs::format_registry_error`.
fn format_registry_error(prefix: &str, err: &anyhow::Error) -> String {
    let mut out = format!("{prefix}: {err:#}");
    if let Some(hint) = registry_fetch_error_hint(err) {
        out.push_str("\n\n");
        out.push_str(hint);
    }
    out
}

/// Discover the enabled visible skills for the current App state.
fn discover_visible(app: &App) -> crate::skills::SkillRegistry {
    crate::skills::discover_for_workspace_and_dir_with_mode_and_plugins(
        &app.workspace,
        &app.skills_dir,
        crate::skills::SkillDiscoveryMode::from_codewhale_only(app.skills_scan_codewhale_only),
        Some(app.plugin_registry.as_ref()),
    )
    .into_enabled()
}

/// Map a TUI skill to its portable projection entry.
fn portable_skill_entry(skill: &crate::skills::Skill) -> SkillEntry {
    let source = match &skill.source {
        crate::skills::SkillSource::Native => SkillSourceKind::Native,
        crate::skills::SkillSource::Plugin {
            plugin_id,
            plugin_name,
            ..
        } => SkillSourceKind::Plugin {
            plugin_name: plugin_name.clone(),
            plugin_id: plugin_id.clone(),
        },
    };
    let path = match &skill.source {
        crate::skills::SkillSource::Native => Some(skill.path.display().to_string()),
        crate::skills::SkillSource::Plugin { .. } => None,
    };
    let bundled_tier = crate::skills::bundled_skill_tier(&skill.name).map(|tier| match tier {
        crate::skills::BundledSkillTier::CoreAgentic => SkillBundledTier::CoreAgentic,
        crate::skills::BundledSkillTier::FormatTooling => SkillBundledTier::FormatTooling,
    });
    SkillEntry {
        name: skill.name.clone(),
        description: skill.description.clone(),
        source,
        path,
        bundled_tier,
    }
}

/// Map a TUI mutation receipt to its portable receipt.
fn portable_mutation_receipt(
    receipt: &crate::skills::mutation::SkillMutationReceipt,
) -> SkillMutationReceipt {
    use crate::skills::mutation::SkillMutationOutcome as TuiOutcome;
    let outcome = match &receipt.outcome {
        TuiOutcome::Installed => SkillMutationOutcome::Installed,
        TuiOutcome::Updated => SkillMutationOutcome::Updated,
        TuiOutcome::NoChange => SkillMutationOutcome::NoChange,
        TuiOutcome::Removed => SkillMutationOutcome::Removed,
        TuiOutcome::Trusted => SkillMutationOutcome::Trusted,
        TuiOutcome::Imported => SkillMutationOutcome::Imported,
        TuiOutcome::AlreadyPresent => SkillMutationOutcome::AlreadyPresent,
        TuiOutcome::NeedsApproval(host) => SkillMutationOutcome::NeedsApproval(host.clone()),
        TuiOutcome::NetworkDenied(host) => SkillMutationOutcome::NetworkDenied(host.clone()),
    };
    SkillMutationReceipt {
        name: receipt.name.clone(),
        safe_target_path: receipt.safe_target_path.clone(),
        outcome,
    }
}

/// Map a portable target scope to the TUI scope.
fn portable_scope(
    scope: Option<SkillTargetScope>,
) -> Option<crate::skills::mutation::SkillTargetScope> {
    use crate::skills::mutation::SkillTargetScope as TuiScope;
    scope.map(|s| match s {
        SkillTargetScope::Project => TuiScope::Project,
        SkillTargetScope::Global => TuiScope::Global,
    })
}

/// Map a curated registry document to portable entries.
fn portable_registry_entries(
    doc: &crate::skills::install::RegistryDocument,
) -> Vec<RemoteSkillEntry> {
    doc.skills
        .iter()
        .map(|(name, entry)| RemoteSkillEntry {
            name: name.clone(),
            description: entry.description.clone(),
            source: entry.source.clone(),
        })
        .collect()
}

/// Message shown when a network-policy host requires approval. Moved
/// verbatim from `groups/skills/skills.rs`; the legacy copy is removed in
/// Phase 4. Rendered by the portable handler from the typed outcome.
fn needs_approval_message(host: &str) -> String {
    format!(
        "Network policy requires approval for {host}.\n\
         Add it to your allow list with `/network allow {host}` (or set [network].default = \"allow\" in ~/.codewhale/config.toml), then retry."
    )
}

/// Message shown when a network-policy host is denied. Moved verbatim from
/// `groups/skills/skills.rs`; the legacy copy is removed in Phase 4.
fn network_denied_message(host: &str) -> String {
    format!(
        "Network policy denied access to {host}.\n\
         Remove the deny entry from ~/.codewhale/config.toml under [network] or contact your administrator."
    )
}

impl CommandSkillGroupContext for SkillGroupAdapter<'_> {
    fn skill_registry_projection(&self) -> SkillRegistryProjection {
        let app = self.host.app.borrow();
        let mode =
            crate::skills::SkillDiscoveryMode::from_codewhale_only(app.skills_scan_codewhale_only);
        let dirs = crate::skills::skill_directories_for_workspace_and_dir(
            &app.workspace,
            &app.skills_dir,
            mode,
        );
        let registry = discover_visible(&app);
        let mode_label = match mode {
            crate::skills::SkillDiscoveryMode::Compatible => "compatible",
            crate::skills::SkillDiscoveryMode::CodeWhaleOnly => "codewhale-only",
        };
        SkillRegistryProjection {
            workspace: app.workspace.display().to_string(),
            skills_dir: app.skills_dir.display().to_string(),
            mode_label: mode_label.to_string(),
            dirs: dirs.iter().map(|dir| dir.display().to_string()).collect(),
            entries: registry.list().iter().map(portable_skill_entry).collect(),
            warnings: registry.warnings().to_vec(),
            total: registry.len(),
        }
    }

    fn activate_skill(
        &mut self,
        name: &str,
    ) -> Result<SkillActivationOutcome, SkillActivationError> {
        let registry = {
            let app = self.host.app.borrow();
            discover_visible(&app)
        };
        if let Some(skill) = registry.get(name) {
            let plugin_provenance = match &skill.source {
                crate::skills::SkillSource::Native => None,
                crate::skills::SkillSource::Plugin { authority, .. } => {
                    if let Err(reason) = crate::plugins::registry::verify_plugin_component_authority(
                        authority,
                        crate::plugins::activation::PluginActivationCapability::Skills,
                    ) {
                        return Err(SkillActivationError::PluginRejected {
                            name: skill.name.clone(),
                            reason,
                        });
                    }
                    Some(authority.as_ref().clone())
                }
            };
            let skill = skill.clone();
            let instruction = format!(
                "You are now using a skill. Follow these instructions:\n\n# Skill: {}\n\n{}\n\n---\n\nNow respond to the user's request following the above skill instructions.",
                skill.name, skill.body
            );
            let mut app = self.host.app.borrow_mut();
            app.add_message(HistoryCell::System {
                content: format!("Activated skill: {}\n\n{}", skill.name, skill.description),
            });
            app.active_skill = Some(instruction);
            app.active_skill_provenance = plugin_provenance;
            Ok(SkillActivationOutcome {
                name: skill.name,
                description: skill.description,
            })
        } else {
            let available: Vec<String> = registry.list().iter().map(|s| s.name.clone()).collect();
            Err(SkillActivationError::NotFound {
                requested: name.to_string(),
                available,
                warnings: registry.warnings().to_vec(),
            })
        }
    }

    fn install_skill(
        &mut self,
        scope: Option<SkillTargetScope>,
        spec: &str,
    ) -> Result<SkillMutationReceipt, String> {
        use crate::skills::mutation::{MutationContext, SkillMutationRequest};
        let source = match crate::skills::install::InstallSource::parse(spec) {
            Ok(source) => source,
            Err(err) => return Err(format!("Invalid install source: {err}")),
        };
        let target =
            portable_scope(scope).unwrap_or(crate::skills::mutation::SkillTargetScope::Global);
        let workspace = self.host.app.borrow().workspace.clone();
        let home = crate::config::effective_home_dir();
        let (network, max_size, registry_url) = installer_settings();
        let outcome = run_async(async move {
            let ctx = MutationContext {
                workspace: &workspace,
                home: home.as_deref(),
                configured_skills_dir: None,
                network: &network,
                max_size,
                registry_url: &registry_url,
            };
            crate::skills::mutation::execute(
                SkillMutationRequest::InstallRemote { source, target },
                &ctx,
            )
            .await
        });
        match outcome {
            Ok(receipt) => Ok(portable_mutation_receipt(&receipt)),
            Err(err) => Err(format!("Install failed: {err:#}")),
        }
    }

    fn update_skill(
        &mut self,
        scope: Option<SkillTargetScope>,
        name: &str,
    ) -> Result<SkillMutationReceipt, String> {
        use crate::skills::mutation::{MutationContext, SkillMutationRequest};
        let workspace = self.host.app.borrow().workspace.clone();
        let home = crate::config::effective_home_dir();
        let (network, max_size, registry_url) = installer_settings();
        let owned_name = name.to_string();
        let scope = portable_scope(scope);
        let outcome = run_async(async move {
            let ctx = MutationContext {
                workspace: &workspace,
                home: home.as_deref(),
                configured_skills_dir: None,
                network: &network,
                max_size,
                registry_url: &registry_url,
            };
            crate::skills::mutation::execute(
                SkillMutationRequest::UpdateByName {
                    name: owned_name,
                    scope,
                    expected_digest: None,
                },
                &ctx,
            )
            .await
        });
        match outcome {
            Ok(receipt) => Ok(portable_mutation_receipt(&receipt)),
            Err(err) => Err(format!("Update failed: {err:#}")),
        }
    }

    fn uninstall_skill(
        &mut self,
        scope: Option<SkillTargetScope>,
        name: &str,
    ) -> Result<SkillMutationReceipt, String> {
        use crate::skills::mutation::{MutationContext, SkillMutationRequest};
        let workspace = self.host.app.borrow().workspace.clone();
        let home = crate::config::effective_home_dir();
        let (network, max_size, registry_url) = installer_settings();
        let ctx = MutationContext {
            workspace: &workspace,
            home: home.as_deref(),
            configured_skills_dir: None,
            network: &network,
            max_size,
            registry_url: &registry_url,
        };
        match crate::skills::mutation::execute_sync(
            SkillMutationRequest::RemoveByName {
                name: name.to_string(),
                scope: portable_scope(scope),
                expected_digest: None,
            },
            &ctx,
        ) {
            Ok(receipt) => Ok(portable_mutation_receipt(&receipt)),
            Err(err) => Err(format!("Uninstall failed: {err:#}")),
        }
    }

    fn trust_skill(
        &mut self,
        scope: Option<SkillTargetScope>,
        name: &str,
    ) -> Result<SkillMutationReceipt, String> {
        use crate::skills::mutation::{MutationContext, SkillMutationRequest};
        let workspace = self.host.app.borrow().workspace.clone();
        let home = crate::config::effective_home_dir();
        let (network, max_size, registry_url) = installer_settings();
        let ctx = MutationContext {
            workspace: &workspace,
            home: home.as_deref(),
            configured_skills_dir: None,
            network: &network,
            max_size,
            registry_url: &registry_url,
        };
        match crate::skills::mutation::execute_sync(
            SkillMutationRequest::TrustByName {
                name: name.to_string(),
                scope: portable_scope(scope),
                expected_digest: None,
            },
            &ctx,
        ) {
            Ok(receipt) => Ok(portable_mutation_receipt(&receipt)),
            Err(err) => Err(format!("Trust failed: {err:#}")),
        }
    }

    fn fetch_remote_registry(&mut self) -> Result<RemoteRegistryOutcome, String> {
        let (network, _max_size, registry_url) = installer_settings();
        let registry = run_async(async move {
            crate::skills::install::fetch_registry(&network, &registry_url).await
        });
        match registry {
            Ok(crate::skills::install::RegistryFetchResult::Loaded(doc)) => {
                Ok(RemoteRegistryOutcome::Loaded {
                    entries: portable_registry_entries(&doc),
                })
            }
            Ok(crate::skills::install::RegistryFetchResult::NeedsApproval(host)) => {
                Ok(RemoteRegistryOutcome::NeedsApproval(host))
            }
            Ok(crate::skills::install::RegistryFetchResult::Denied(host)) => {
                Ok(RemoteRegistryOutcome::Denied(host))
            }
            Err(err) => Err(format_registry_error("Failed to fetch registry", &err)),
        }
    }

    fn recommend_skills(&mut self, task: &str) -> Result<Vec<SkillRecommendation>, String> {
        let (network, _max_size, registry_url) = installer_settings();
        let registry = run_async(async move {
            crate::skills::install::fetch_registry(&network, &registry_url).await
        });
        match registry {
            Ok(crate::skills::install::RegistryFetchResult::Loaded(doc)) => {
                let recommendations =
                    crate::skills::recommend::recommend_remote_skills(task, &doc, 3);
                Ok(recommendations
                    .into_iter()
                    .map(|recommendation| SkillRecommendation {
                        name: recommendation.name.to_string(),
                        description: recommendation.entry.description.clone(),
                        matched_terms: recommendation.matched_terms.clone(),
                    })
                    .collect())
            }
            Ok(crate::skills::install::RegistryFetchResult::NeedsApproval(host)) => {
                Err(needs_approval_message(&host))
            }
            Ok(crate::skills::install::RegistryFetchResult::Denied(host)) => {
                Err(network_denied_message(&host))
            }
            Err(err) => Err(format_registry_error("Failed to fetch registry", &err)),
        }
    }

    fn sync_registry(&mut self) -> Result<SkillSyncOutcome, String> {
        use crate::skills::install::{SkillSyncOutcome as TuiSyncOutcome, SyncResult};
        let (network, max_size, registry_url) = installer_settings();
        let cache_dir = crate::skills::install::default_cache_skills_dir();
        let result = run_async(async move {
            crate::skills::install::sync_registry(&network, &registry_url, &cache_dir, max_size)
                .await
        });
        match result {
            Ok(SyncResult::RegistryDenied(host)) => Ok(SkillSyncOutcome::RegistryDenied(host)),
            Ok(SyncResult::RegistryNeedsApproval(host)) => {
                Ok(SkillSyncOutcome::RegistryNeedsApproval(host))
            }
            Ok(SyncResult::Done { outcomes }) => {
                let total = outcomes.len();
                let mut downloaded = 0usize;
                let mut fresh = 0usize;
                let mut failed = 0usize;
                let entries = outcomes
                    .into_iter()
                    .map(|outcome| match outcome {
                        TuiSyncOutcome::Downloaded { name, path } => {
                            downloaded += 1;
                            SkillSyncEntry::Downloaded {
                                name,
                                path: path.display().to_string(),
                            }
                        }
                        TuiSyncOutcome::Fresh { name } => {
                            fresh += 1;
                            SkillSyncEntry::Fresh { name }
                        }
                        TuiSyncOutcome::Failed { name, reason } => {
                            failed += 1;
                            SkillSyncEntry::Failed { name, reason }
                        }
                        TuiSyncOutcome::Denied { name, host } => {
                            failed += 1;
                            SkillSyncEntry::Denied { name, host }
                        }
                        TuiSyncOutcome::NeedsApproval { name, host } => {
                            failed += 1;
                            SkillSyncEntry::NeedsApproval { name, host }
                        }
                    })
                    .collect();
                Ok(SkillSyncOutcome::Done {
                    total,
                    downloaded,
                    fresh,
                    failed,
                    entries,
                })
            }
            Err(err) => Err(format_registry_error("Sync failed", &err)),
        }
    }

    fn run_review(&mut self) -> Result<ReviewOutcome, String> {
        let skills_dir = self.host.app.borrow().skills_dir.clone();
        let registry = crate::skills::SkillRegistry::discover(&skills_dir).into_enabled();
        let mut warnings: Vec<String> = registry.warnings().to_vec();
        let mut skill = registry.get("review").cloned();

        let global_dir = crate::skills::default_skills_dir();
        if skill.is_none() && global_dir != skills_dir {
            let registry = crate::skills::SkillRegistry::discover(&global_dir).into_enabled();
            if warnings.is_empty() {
                warnings = registry.warnings().to_vec();
            } else if !registry.warnings().is_empty() {
                warnings.extend(registry.warnings().iter().cloned());
            }
            skill = registry.get("review").cloned();
        }

        match skill {
            Some(skill) => {
                // Host-side side effects (D2): session-message insertion and
                // active-skill mutation are authoritative App operations; the
                // portable handler renders no success message (baseline emits
                // only the SendMessage action) and never touches App.
                let instruction = format!(
                    "You are now using a skill. Follow these instructions:\n\n# Skill: {}\n\n{}\n\n---\n\nNow respond to the user's request following the above skill instructions.",
                    skill.name, skill.body
                );
                let mut app = self.host.app.borrow_mut();
                app.add_message(HistoryCell::System {
                    content: format!("Activated skill: {}\n\n{}", skill.name, skill.description),
                });
                app.active_skill = Some(instruction);
                app.active_skill_provenance = None;
                Ok(ReviewOutcome::Ready)
            }
            None => Ok(ReviewOutcome::NotFound {
                skills_dir: skills_dir.display().to_string(),
                global_dir: global_dir.display().to_string(),
                warnings,
            }),
        }
    }

    fn snapshot_list(&mut self, limit: usize) -> Result<Vec<SnapshotEntry>, String> {
        let workspace = self.host.app.borrow().workspace.clone();
        let repo = match crate::snapshot::SnapshotRepo::open_or_init(&workspace) {
            Ok(repo) => repo,
            Err(err) => {
                return Err(format!(
                    "Snapshot repo unavailable for {}: {err}",
                    workspace.display(),
                ));
            }
        };
        let snapshots = match repo.list(limit) {
            Ok(snapshots) => snapshots,
            Err(err) => return Err(format!("Failed to list snapshots: {err}")),
        };
        Ok(snapshots
            .into_iter()
            .map(|snapshot| SnapshotEntry {
                id: snapshot.id.0,
                label: snapshot.label,
                timestamp: snapshot.timestamp,
            })
            .collect())
    }

    fn restore_snapshot(&mut self, id: &str) -> Result<(), String> {
        let workspace = self.host.app.borrow().workspace.clone();
        let repo = match crate::snapshot::SnapshotRepo::open_or_init(&workspace) {
            Ok(repo) => repo,
            Err(err) => {
                return Err(format!(
                    "Snapshot repo unavailable for {}: {err}",
                    workspace.display(),
                ));
            }
        };
        repo.restore(&crate::snapshot::SnapshotId(id.to_string()))
            .map_err(|err| format!("Restore failed: {err}"))
    }

    fn approval_state(&self) -> CommandApprovalState {
        let app = self.host.app.borrow();
        CommandApprovalState {
            yolo: app.yolo,
            trust_mode: app.trust_mode,
        }
    }
}

// ---------------------------------------------------------------------------
// Plugin host adapter (FEAT-020 D1/D11)
// ---------------------------------------------------------------------------

/// Plugin host-data adapter (FEAT-020 D1/D11).
///
/// Owns every concrete plugin service the live `/plugin` branch closure
/// consumes: registry reads/mutations, the async mutation/network-policy
/// bridge (D11), export, legacy executable-tool scan, Kimi managed import,
/// and the marketplace store. That store projects the bundled first-party
/// catalog alongside locally added catalogs for every host surface.
/// Every method borrows `App` only for the duration of one call and converts
/// host values to portable contract values before returning. Handlers receive
/// only the portable facet and never name `PluginRegistry`, `LoadedPlugin`,
/// `Config`, or another concrete host service.
pub(crate) struct PluginAdapter<'a> {
    host: SharedCommandHost<'a>,
}

/// Convert a TUI-owned diagnostic to the portable contract diagnostic.
fn portable_diagnostic(diagnostic: &crate::plugins::types::PluginDiagnostic) -> PluginDiagnostic {
    PluginDiagnostic {
        level: match diagnostic.level {
            crate::plugins::types::PluginDiagnosticLevel::Warning => PluginDiagnosticLevel::Warning,
            crate::plugins::types::PluginDiagnosticLevel::Error => PluginDiagnosticLevel::Error,
        },
        code: diagnostic.code.to_string(),
        message: diagnostic.message.clone(),
        path: diagnostic.path.clone(),
    }
}

/// Convert a TUI marketplace diagnostic into the portable contract diagnostic.
fn portable_marketplace_diagnostic(
    diagnostic: &crate::plugins::marketplace::types::MarketplaceDiagnostic,
) -> PluginDiagnostic {
    PluginDiagnostic {
        level: match diagnostic.level {
            crate::plugins::types::PluginDiagnosticLevel::Warning => PluginDiagnosticLevel::Warning,
            crate::plugins::types::PluginDiagnosticLevel::Error => PluginDiagnosticLevel::Error,
        },
        code: diagnostic.code.clone(),
        message: diagnostic.message.clone(),
        path: None,
    }
}

/// Convert a TUI-owned loaded plugin into the portable list summary.
fn portable_summary(plugin: &crate::plugins::types::LoadedPlugin) -> PluginSummary {
    PluginSummary {
        name: plugin.name().to_string(),
        id: plugin.id.as_str().to_string(),
        state_label: plugin.state_label().to_string(),
        scope: plugin.scope.as_str().to_string(),
        trust_status: plugin.trust_status.as_str().to_string(),
        compatibility: plugin.compatibility().as_str().to_string(),
        inventory: plugin.inventory.summary(),
        active: plugin.active(),
        trusted: plugin.trusted(),
        enabled: plugin.enabled,
    }
}

/// Convert one TUI MCP server config into the portable review detail.
fn portable_mcp_server(name: &str, server: &crate::mcp::McpServerConfig) -> PluginMcpServerDetail {
    let transport = if server.url.is_some() {
        PluginMcpTransport::Http
    } else if server.command.is_some() {
        PluginMcpTransport::Stdio
    } else {
        PluginMcpTransport::Invalid
    };
    let mut env = server
        .env
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect::<Vec<_>>();
    env.sort_unstable();
    let mut env_headers = server
        .env_headers
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect::<Vec<_>>();
    env_headers.sort_unstable();
    PluginMcpServerDetail {
        name: name.to_string(),
        transport,
        command: server.command.clone(),
        argv: server.args.clone(),
        cwd: server.cwd.clone(),
        env,
        url: server.url.clone(),
        env_headers,
        bearer_token_env_var: server.bearer_token_env_var.clone(),
        connect_timeout_secs: server.connect_timeout,
        execute_timeout_secs: server.execute_timeout,
        read_timeout_secs: server.read_timeout,
        required: server.required,
        enabled_tools: server.enabled_tools.clone(),
        disabled_tools: server.disabled_tools.clone(),
        enabled: server.is_enabled(),
    }
}

/// Convert a TUI-owned loaded plugin into the portable full detail.
fn portable_detail(plugin: &crate::plugins::types::LoadedPlugin) -> PluginDetail {
    let mcp_servers = plugin
        .manifest
        .mcp_servers
        .as_ref()
        .map(|servers| {
            let mut list = servers
                .iter()
                .map(|(name, server)| portable_mcp_server(name, server))
                .collect::<Vec<_>>();
            list.sort_by(|a, b| a.name.cmp(&b.name));
            list
        })
        .unwrap_or_default();
    PluginDetail {
        name: plugin.name().to_string(),
        id: plugin.id.as_str().to_string(),
        inventory_summary: plugin.inventory.summary(),
        version: plugin.manifest.plugin.version.clone(),
        origin: plugin.origin.as_str().to_string(),
        scope: plugin.scope.as_str().to_string(),
        state_label: plugin.state_label().to_string(),
        trust_status: plugin.trust_status.as_str().to_string(),
        compatibility: plugin.compatibility().as_str().to_string(),
        content_hash: plugin.content_hash.clone(),
        capability_hash: plugin.capability_hash.clone(),
        canonical_root: plugin.canonical_root.clone(),
        active: plugin.active(),
        trusted: plugin.trusted(),
        enabled: plugin.enabled,
        unsupported_labels: plugin
            .inventory
            .unsupported_labels()
            .into_iter()
            .map(str::to_string)
            .collect(),
        supported_labels: plugin
            .inventory
            .supported_labels()
            .into_iter()
            .map(str::to_string)
            .collect(),
        skills: plugin
            .skill_snapshots
            .iter()
            .map(|skill| format!("{}:{}", plugin.name(), skill.name))
            .collect(),
        filesystem_roots: plugin.inventory.filesystem_roots.clone(),
        network_hosts: plugin.inventory.network_hosts.clone(),
        stdio_mcp_servers: plugin.inventory.stdio_mcp_servers,
        lifecycle_mutation: plugin.inventory.lifecycle_mutation,
        mcp_servers,
        diagnostics: plugin.diagnostics.iter().map(portable_diagnostic).collect(),
    }
}

/// Convert a TUI mutation receipt into the portable contract receipt.
fn portable_plugin_mutation_receipt(
    receipt: &crate::plugins::mutation::PluginMutationReceipt,
) -> PluginMutationReceipt {
    let outcome = match &receipt.outcome {
        crate::plugins::mutation::PluginMutationOutcome::Installed => {
            PluginMutationOutcome::Installed
        }
        crate::plugins::mutation::PluginMutationOutcome::Updated => PluginMutationOutcome::Updated,
        crate::plugins::mutation::PluginMutationOutcome::NoChange => {
            PluginMutationOutcome::NoChange
        }
        crate::plugins::mutation::PluginMutationOutcome::Uninstalled => {
            PluginMutationOutcome::Uninstalled
        }
        crate::plugins::mutation::PluginMutationOutcome::NeedsApproval(host) => {
            PluginMutationOutcome::NeedsApproval(host.clone())
        }
        crate::plugins::mutation::PluginMutationOutcome::NetworkDenied(host) => {
            PluginMutationOutcome::NetworkDenied(host.clone())
        }
    };
    PluginMutationReceipt {
        name: receipt.name.clone(),
        path: receipt.path.clone(),
        content_hash: receipt.content_hash.clone(),
        installed_content_hash: receipt.installed_content_hash.clone(),
        outcome,
    }
}

/// Convert a TUI export receipt into the portable contract receipt.
fn portable_export_receipt(
    receipt: &crate::plugins::export::PluginExportReceipt,
) -> PluginExportReceipt {
    PluginExportReceipt {
        exported_name: receipt.exported_name.clone(),
        target: receipt.target.clone(),
        display_name: receipt.display_name.clone(),
        wrote_mcp_json: receipt.wrote_mcp_json,
        files_copied: receipt.files_copied as u64,
        skills_normalized: receipt.skills_normalized,
    }
}

/// Convert one TUI legacy tool entry into the portable value.
fn portable_legacy_tool(
    path: &Path,
    metadata: &crate::tools::plugin::PluginMetadata,
) -> PluginLegacyTool {
    PluginLegacyTool {
        name: metadata.name.clone(),
        description: metadata.description.clone(),
        approval: match metadata.approval {
            crate::tools::spec::ApprovalRequirement::Auto => "auto",
            crate::tools::spec::ApprovalRequirement::Suggest => "suggest",
            crate::tools::spec::ApprovalRequirement::Required => "required",
        }
        .to_string(),
        input_schema: Some(
            serde_json::to_string_pretty(&metadata.input_schema).unwrap_or_default(),
        ),
        path: path.to_path_buf(),
    }
}

/// Convert one TUI marketplace candidate into the portable value.
fn portable_marketplace_candidate(
    entry: &crate::plugins::marketplace::store::StoredMarketplaceCatalog,
    candidate: &crate::plugins::marketplace::types::MarketplaceCandidate,
    registry: &crate::plugins::PluginRegistry,
) -> PluginMarketplaceCandidate {
    use crate::plugins::marketplace::document::{
        CatalogInstallResolution, resolve_candidate_install,
    };
    let install_plan = match resolve_candidate_install(entry, candidate, registry) {
        CatalogInstallResolution::Supported { spec, source_kind } => {
            PluginMarketplaceInstallPlan::Supported { spec, source_kind }
        }
        CatalogInstallResolution::AlreadyPresent { plugin, reason } => {
            PluginMarketplaceInstallPlan::AlreadyPresent {
                selector: plugin.id.as_str().to_string(),
                reason,
            }
        }
        CatalogInstallResolution::Unsupported { reason } => {
            PluginMarketplaceInstallPlan::Unsupported { reason }
        }
        CatalogInstallResolution::HasErrors { diagnostics } => {
            PluginMarketplaceInstallPlan::Unsupported {
                reason: diagnostics,
            }
        }
    };
    PluginMarketplaceCandidate {
        name: candidate.name.clone(),
        display_name: candidate.display_name.clone(),
        version: candidate.version.clone(),
        tier: candidate.provenance.tier.as_str().to_string(),
        compatibility: candidate
            .compatibility
            .as_ref()
            .map(|c| c.as_str().to_string()),
        install_plan,
        description: candidate.description.clone(),
        homepage: candidate.homepage.clone(),
        repository: candidate.repository.clone(),
        author: candidate.author.clone(),
        license: candidate.license.clone(),
        keywords: candidate.keywords.clone(),
        when: candidate.when.as_ref().map(|when| format!("{when:?}")),
        diagnostics: candidate
            .diagnostics
            .iter()
            .map(portable_marketplace_diagnostic)
            .collect(),
        has_errors: candidate.has_errors(),
    }
}

/// Convert one stored TUI marketplace catalog (with its source path).
fn portable_marketplace_catalog_with_source(
    entry: &crate::plugins::marketplace::store::StoredMarketplaceCatalog,
    registry: &crate::plugins::PluginRegistry,
) -> PluginMarketplaceCatalog {
    let catalog = &entry.catalog;
    PluginMarketplaceCatalog {
        id: catalog.id.as_str().to_string(),
        source_path: Some(entry.source_path.clone()),
        display_name: catalog.display_name.clone(),
        description: catalog.description.clone(),
        format: catalog.format.as_str().to_string(),
        tier: catalog.provenance.tier.as_str().to_string(),
        publisher: catalog.provenance.publisher.clone(),
        total_candidates: catalog.total_candidates(),
        warning_count: catalog.warning_count(),
        candidates: catalog
            .candidates
            .iter()
            .map(|candidate| portable_marketplace_candidate(entry, candidate, registry))
            .collect(),
        diagnostics: catalog
            .diagnostics
            .iter()
            .map(portable_marketplace_diagnostic)
            .collect(),
    }
}

/// Kimi managed-plugin scan (host-side, FEAT-020 D1). Mirrors the legacy
/// `/plugin import kimi` scan exactly: only immediate canonical children of
/// `~/.kimi-code/plugins/managed`, rejecting symlinks/reparse points,
/// non-directories, and children that escape the root. Returns portable
/// candidate values; rejection reasons cross as safe text.
fn scan_managed_plugins_portable(
    home_override: Option<&Path>,
) -> Result<PluginManagedScan, String> {
    use std::fs;
    use std::path::PathBuf;

    const MAX_MANAGED_CHILDREN: usize = 128;
    const KIMI_PLUGIN_JSON_NAME: &str = crate::plugins::agent_plugin::KIMI_PLUGIN_JSON_NAME;

    struct Candidate {
        name: String,
        version: String,
        license: Option<String>,
        canonical_path: PathBuf,
        content_hash: String,
        capability_hash: String,
        inventory: String,
        applicable: bool,
    }

    fn inspect_candidate(canonical_path: &Path) -> Result<Candidate, String> {
        let manifest_path = canonical_path.join(KIMI_PLUGIN_JSON_NAME);
        let metadata = fs::symlink_metadata(&manifest_path).map_err(|error| {
            format!(
                "Kimi manifest unreadable at {}: {}",
                canonical_path.display(),
                error
            )
        })?;
        if crate::plugins::metadata_is_link_or_reparse(&metadata) || !metadata.is_file() {
            return Err(format!(
                "Kimi manifest must be a regular file at {}",
                canonical_path.display()
            ));
        }
        let validated = crate::plugins::manifest::PluginManifest::validate_from_path(
            &manifest_path,
        )
        .map_err(|error| {
            format!(
                "Kimi manifest invalid at {}: {error}",
                canonical_path.display()
            )
        })?;
        let name = validated.manifest.plugin.name.clone();
        if canonical_path.file_name().and_then(|part| part.to_str()) != Some(name.as_str()) {
            return Err(format!(
                "Kimi directory name `{}` does not match manifest name `{}`",
                canonical_path.display(),
                name
            ));
        }
        Ok(Candidate {
            name,
            version: validated.manifest.plugin.version.clone(),
            license: validated.manifest.plugin.license.clone(),
            canonical_path: validated.canonical_root,
            content_hash: validated.content_hash,
            capability_hash: validated.capability_hash,
            inventory: validated.inventory.summary(),
            applicable: validated.applicable,
        })
    }

    let home = match home_override {
        Some(home) => home.to_path_buf(),
        None => crate::config::effective_home_dir().ok_or_else(|| {
            tr(
                codewhale_localization::Locale::En,
                codewhale_localization::MessageId::PluginKimiHomeMissing,
            )
            .into_owned()
            .to_string()
        })?,
    };
    let configured_root = home.join(".kimi-code/plugins/managed");
    let metadata = match fs::symlink_metadata(&configured_root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(PluginManagedScan {
                root: configured_root,
                candidates: Vec::new(),
                rejected: Vec::new(),
            });
        }
        Err(error) => {
            let root_text = escape_review_text(&configured_root.display().to_string());
            let error_text = escape_review_text(&error.to_string());
            return Err(tr(
                codewhale_localization::Locale::En,
                codewhale_localization::MessageId::PluginKimiRootInspectFailed,
            )
            .replace("{root}", &root_text)
            .replace("{error}", &error_text));
        }
    };
    if crate::plugins::metadata_is_link_or_reparse(&metadata) || !metadata.is_dir() {
        let root_text = escape_review_text(&configured_root.display().to_string());
        return Err(tr(
            codewhale_localization::Locale::En,
            codewhale_localization::MessageId::PluginKimiRootMustBeDirectory,
        )
        .replace("{root}", &root_text));
    }
    let canonical_root = configured_root.canonicalize().map_err(|error| {
        let root_text = escape_review_text(&configured_root.display().to_string());
        let error_text = escape_review_text(&error.to_string());
        tr(
            codewhale_localization::Locale::En,
            codewhale_localization::MessageId::PluginKimiRootCanonicalizeFailed,
        )
        .replace("{root}", &root_text)
        .replace("{error}", &error_text)
    })?;
    let mut entries = fs::read_dir(&canonical_root)
        .map_err(|error| {
            let root_text = escape_review_text(&canonical_root.display().to_string());
            let error_text = escape_review_text(&error.to_string());
            tr(
                codewhale_localization::Locale::En,
                codewhale_localization::MessageId::PluginKimiRootListFailed,
            )
            .replace("{root}", &root_text)
            .replace("{error}", &error_text)
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            let error_text = escape_review_text(&error.to_string());
            tr(
                codewhale_localization::Locale::En,
                codewhale_localization::MessageId::PluginKimiEntryReadFailed,
            )
            .replace("{error}", &error_text)
        })?;
    if entries.len() > MAX_MANAGED_CHILDREN {
        return Err(tr(
            codewhale_localization::Locale::En,
            codewhale_localization::MessageId::PluginKimiEntryLimit,
        )
        .replace("{count}", &entries.len().to_string())
        .replace("{max}", &MAX_MANAGED_CHILDREN.to_string()));
    }
    entries.sort_by_key(fs::DirEntry::file_name);

    let mut candidates = Vec::new();
    let mut rejected = Vec::new();
    for entry in entries {
        let path = entry.path();
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) => {
                let path_text = escape_review_text(&path.display().to_string());
                let error_text = escape_review_text(&error.to_string());
                rejected.push(
                    tr(
                        codewhale_localization::Locale::En,
                        codewhale_localization::MessageId::PluginKimiEntryInspectFailed,
                    )
                    .replace("{path}", &path_text)
                    .replace("{error}", &error_text),
                );
                continue;
            }
        };
        if crate::plugins::metadata_is_link_or_reparse(&metadata) {
            let path_text = escape_review_path(&path);
            rejected.push(
                tr(
                    codewhale_localization::Locale::En,
                    codewhale_localization::MessageId::PluginKimiEntryLinksRefused,
                )
                .replace("{path}", &path_text),
            );
            continue;
        }
        if !metadata.is_dir() {
            continue;
        }
        let canonical_path = match path.canonicalize() {
            Ok(path) if path.parent() == Some(canonical_root.as_path()) => path,
            Ok(canonical_path) => {
                let path_text = escape_review_text(&path.display().to_string());
                let canonical_text = escape_review_text(&canonical_path.display().to_string());
                rejected.push(
                    tr(
                        codewhale_localization::Locale::En,
                        codewhale_localization::MessageId::PluginKimiEntryOutsideRoot,
                    )
                    .replace("{path}", &path_text)
                    .replace("{canonical_path}", &canonical_text),
                );
                continue;
            }
            Err(error) => {
                let path_text = escape_review_text(&path.display().to_string());
                let error_text = escape_review_text(&error.to_string());
                rejected.push(
                    tr(
                        codewhale_localization::Locale::En,
                        codewhale_localization::MessageId::PluginKimiEntryCanonicalizeFailed,
                    )
                    .replace("{path}", &path_text)
                    .replace("{error}", &error_text),
                );
                continue;
            }
        };
        match inspect_candidate(&canonical_path) {
            Ok(candidate) => candidates.push(candidate),
            Err(error) => rejected.push(error),
        }
    }
    candidates.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(PluginManagedScan {
        root: canonical_root,
        candidates: candidates
            .into_iter()
            .map(|candidate| PluginManagedCandidate {
                name: candidate.name,
                version: candidate.version,
                license: candidate.license,
                canonical_path: candidate.canonical_path,
                content_hash: candidate.content_hash,
                capability_hash: candidate.capability_hash,
                inventory: candidate.inventory,
                applicable: candidate.applicable,
            })
            .collect(),
        rejected,
    })
}

/// Escape review text exactly like the plugin render helpers (FEAT-020 D2).
fn escape_review_text(value: &str) -> String {
    crate::commands::groups::plugins::render::escape_review_text(value)
}

/// Escape a review path exactly like the plugin render helpers (FEAT-020 D2).
fn escape_review_path(path: &Path) -> String {
    crate::commands::groups::plugins::render::escape_review_path(path)
}

impl CommandPluginContext for PluginAdapter<'_> {
    fn summaries(&self) -> Result<Vec<PluginSummary>, String> {
        let app = self.host.app.borrow();
        Ok(app
            .plugin_registry
            .list()
            .iter()
            .map(|plugin| portable_summary(plugin))
            .collect())
    }

    fn detail(&self, selector: &str) -> Result<PluginDetail, String> {
        let app = self.host.app.borrow();
        let plugin = app
            .plugin_registry
            .get(selector)
            .ok_or_else(|| format!("no plugin named {selector}"))?;
        Ok(portable_detail(plugin))
    }

    fn registry_diagnostics(&self) -> Vec<PluginDiagnostic> {
        self.host
            .app
            .borrow()
            .plugin_registry
            .diagnostics()
            .iter()
            .map(portable_diagnostic)
            .collect()
    }

    fn validation_is_clean(&self) -> bool {
        self.host.app.borrow().plugin_registry.validation_is_clean()
    }

    fn len(&self) -> usize {
        self.host.app.borrow().plugin_registry.len()
    }

    fn is_empty(&self) -> bool {
        self.host.app.borrow().plugin_registry.is_empty()
    }

    fn reload(&mut self) -> Result<usize, String> {
        let mut app = self.host.app.borrow_mut();
        let workspace = app.workspace.clone();
        app.plugin_registry = app.plugin_registry.rediscover_for_workspace(&workspace);
        app.refresh_skill_cache();
        Ok(app.plugin_registry.len())
    }

    fn reload_nudge(&mut self) -> Option<String> {
        let mut app = self.host.app.borrow_mut();
        let registry = app.plugin_registry.clone();
        crate::plugins::plugin_reload_nudge(registry.as_ref(), &mut app.plugin_reload_nudge_stamp)
            .map(str::to_string)
    }

    fn state_path(&self) -> Option<PathBuf> {
        self.host
            .app
            .borrow()
            .plugin_registry
            .state_path()
            .map(Path::to_path_buf)
    }

    fn suggest(&self, task: &str) -> Result<Vec<PluginSuggestion>, String> {
        let task = task.trim();
        if task.chars().count() < 3 {
            return Err("Usage: /plugin suggest <task of at least 3 characters>".to_string());
        }
        let app = self.host.app.borrow();
        let marketplace = crate::plugins::recommend::load_marketplace_candidates(
            app.plugin_registry.state_path(),
        );
        let recommendations = crate::plugins::recommend::recommend_plugins_for_task(
            task,
            app.plugin_registry.as_ref(),
            &marketplace,
            crate::plugins::recommend::RecommendOptions::default(),
        );
        Ok(recommendations
            .into_iter()
            .map(|recommendation| {
                let description = match &recommendation.source {
                    crate::plugins::recommend::PluginMatchSource::Installed { id } => app
                        .plugin_registry
                        .get(id)
                        .and_then(|plugin| plugin.manifest.plugin.description.clone())
                        .filter(|description| !description.trim().is_empty())
                        .unwrap_or_else(|| "No description provided.".to_string()),
                    crate::plugins::recommend::PluginMatchSource::Marketplace { catalog_id } => {
                        marketplace
                            .iter()
                            .find(|candidate| {
                                candidate.name.eq_ignore_ascii_case(&recommendation.name)
                                    && candidate.catalog_id.as_str() == catalog_id
                            })
                            .and_then(|candidate| candidate.description.clone())
                            .filter(|description| !description.trim().is_empty())
                            .unwrap_or_else(|| "Catalog plugin.".to_string())
                    }
                };
                let state_label = match &recommendation.source {
                    crate::plugins::recommend::PluginMatchSource::Installed { id } => app
                        .plugin_registry
                        .get(id)
                        .map(|plugin| plugin.state_label().to_string())
                        .unwrap_or_else(|| "installed".to_string()),
                    crate::plugins::recommend::PluginMatchSource::Marketplace { .. } => {
                        "not installed".to_string()
                    }
                };
                PluginSuggestion {
                    name: recommendation.name.clone(),
                    state_label,
                    description,
                    why: recommendation.matched_terms.clone(),
                    next_step: recommendation.command(),
                }
            })
            .collect())
    }

    fn trust(&mut self, selector: &str, token: &str) -> Result<(), String> {
        let expected = {
            let app = self.host.app.borrow();
            app.plugin_registry
                .get(selector)
                .map(crate::plugins::types::LoadedPlugin::review_token)
                .ok_or_else(|| format!("no plugin named {selector}"))?
        };
        if token != expected {
            return Err(
                "Review token does not match this bundle content and capability set; run `/plugin trust <name>` again"
                    .to_string(),
            );
        }
        {
            let mut app = self.host.app.borrow_mut();
            std::sync::Arc::make_mut(&mut app.plugin_registry).trust(selector)?;
            app.refresh_skill_cache();
        }
        Ok(())
    }

    fn enable(&mut self, selector: &str) -> Result<(), String> {
        let needs_review = self
            .host
            .app
            .borrow()
            .plugin_registry
            .get(selector)
            .is_some_and(|plugin| !plugin.trusted());
        if needs_review {
            // Enabling is the natural entry point; open the capability review
            // instead of an opaque denial (matches the legacy handler).
            return Err("plugin requires review before enabling".to_string());
        }
        let mut app = self.host.app.borrow_mut();
        std::sync::Arc::make_mut(&mut app.plugin_registry).enable(selector)?;
        app.refresh_skill_cache();
        Ok(())
    }

    fn disable(&mut self, selector: &str) -> Result<(), String> {
        let mut app = self.host.app.borrow_mut();
        std::sync::Arc::make_mut(&mut app.plugin_registry).disable(selector)?;
        app.refresh_skill_cache();
        app.active_skill = None;
        app.active_skill_provenance = None;
        Ok(())
    }

    fn revoke_trust(&mut self, selector: &str) -> Result<(), String> {
        let mut app = self.host.app.borrow_mut();
        std::sync::Arc::make_mut(&mut app.plugin_registry).revoke_trust(selector)?;
        app.refresh_skill_cache();
        app.active_skill = None;
        app.active_skill_provenance = None;
        Ok(())
    }

    fn install(
        &mut self,
        source: &str,
        expected_content_hash: Option<&str>,
    ) -> Result<PluginMutationReceipt, String> {
        use crate::plugins::install::PluginInstallSource;
        use crate::plugins::mutation::{
            PluginMutationContext, PluginMutationOutcome, PluginMutationRequest,
        };

        let plugin_source = PluginInstallSource::parse(source).map_err(|error| {
            format!(
                "Invalid plugin install source `{source}`: {error:#}\n\
                 Expected a local path, github:owner/repo, an HTTPS tarball URL, or builtin:<name>."
            )
        })?;
        let network = plugin_network_policy();
        let expected_content_hash = expected_content_hash.map(str::to_string);
        let expected_for_request = expected_content_hash.clone();
        let mut app = self.host.app.borrow_mut();
        let registry = std::sync::Arc::make_mut(&mut app.plugin_registry);
        let outcome = run_async(async move {
            let ctx = PluginMutationContext {
                network: &network,
                max_size: crate::plugins::install::DEFAULT_MAX_SIZE_BYTES,
            };
            let request = match expected_for_request {
                Some(expected_content_hash) => PluginMutationRequest::InstallExact {
                    source: plugin_source,
                    expected_content_hash,
                },
                None => PluginMutationRequest::Install {
                    source: plugin_source,
                },
            };
            crate::plugins::mutation::execute(request, &ctx, registry).await
        });
        match outcome {
            Ok(receipt) => {
                let portable = portable_plugin_mutation_receipt(&receipt);
                // Rediscover and refresh the skill cache after any install.
                if matches!(receipt.outcome, PluginMutationOutcome::Installed) {
                    let workspace = app.workspace.clone();
                    app.plugin_registry = app.plugin_registry.rediscover_for_workspace(&workspace);
                    app.refresh_skill_cache();
                }
                Ok(portable)
            }
            Err(error) => Err(format!("Plugin install failed: {error:#}")),
        }
    }

    fn update(&mut self, selector: &str) -> Result<PluginMutationReceipt, String> {
        use crate::plugins::mutation::{
            PluginMutationContext, PluginMutationOutcome, PluginMutationRequest,
        };
        let network = plugin_network_policy();
        let selector_owned = selector.to_string();
        let mut app = self.host.app.borrow_mut();
        let registry = std::sync::Arc::make_mut(&mut app.plugin_registry);
        let outcome = run_async(async move {
            let ctx = PluginMutationContext {
                network: &network,
                max_size: crate::plugins::install::DEFAULT_MAX_SIZE_BYTES,
            };
            crate::plugins::mutation::execute(
                PluginMutationRequest::Update {
                    selector: selector_owned,
                },
                &ctx,
                registry,
            )
            .await
        });
        match outcome {
            Ok(receipt) => {
                let portable = portable_plugin_mutation_receipt(&receipt);
                if matches!(receipt.outcome, PluginMutationOutcome::Updated) {
                    let workspace = app.workspace.clone();
                    app.plugin_registry = app.plugin_registry.rediscover_for_workspace(&workspace);
                    app.refresh_skill_cache();
                }
                Ok(portable)
            }
            Err(error) => Err(format!("Plugin update failed: {error:#}")),
        }
    }

    fn uninstall(&mut self, selector: &str) -> Result<PluginMutationReceipt, String> {
        use crate::plugins::mutation::{
            PluginMutationContext, PluginMutationOutcome, PluginMutationRequest,
        };
        let network = plugin_network_policy();
        let selector_owned = selector.to_string();
        let mut app = self.host.app.borrow_mut();
        let registry = std::sync::Arc::make_mut(&mut app.plugin_registry);
        let outcome = run_async(async move {
            let ctx = PluginMutationContext {
                network: &network,
                max_size: crate::plugins::install::DEFAULT_MAX_SIZE_BYTES,
            };
            crate::plugins::mutation::execute(
                PluginMutationRequest::Uninstall {
                    selector: selector_owned,
                },
                &ctx,
                registry,
            )
            .await
        });
        match outcome {
            Ok(receipt) => {
                let portable = portable_plugin_mutation_receipt(&receipt);
                if matches!(receipt.outcome, PluginMutationOutcome::Uninstalled) {
                    let workspace = app.workspace.clone();
                    app.plugin_registry = app.plugin_registry.rediscover_for_workspace(&workspace);
                    app.refresh_skill_cache();
                    app.active_skill = None;
                    app.active_skill_provenance = None;
                }
                Ok(portable)
            }
            Err(error) => Err(format!("Plugin uninstall failed: {error:#}")),
        }
    }

    fn uninstall_path(&mut self, name: &str, plugins_dir: &Path) -> Result<(), String> {
        // File-level rollback removal for a bundle whose content hash
        // mismatched; no registry resolution, rediscovery, or skill side
        // effects (FEAT-020 D1 — the `crate::plugins` call stays host-side).
        crate::plugins::install::uninstall(name, plugins_dir).map_err(|error| format!("{error:#}"))
    }

    fn export(&self, selector: &str, target: &Path) -> Result<PluginExportReceipt, String> {
        let app = self.host.app.borrow();
        let plugin = app
            .plugin_registry
            .get(selector)
            .ok_or_else(|| format!("no plugin named {selector}"))?
            .clone();
        let existing_names: std::collections::BTreeSet<String> = app
            .plugin_registry
            .list()
            .iter()
            .map(|other| other.name().to_string())
            .filter(|name| name != plugin.name())
            .collect();
        let target = if target.is_absolute() {
            target.to_path_buf()
        } else {
            app.workspace.join(target)
        };
        crate::plugins::export::export_plugin_bundle(&plugin, &target, &existing_names)
            .map(|receipt| portable_export_receipt(&receipt))
            .map_err(|error| format!("Export of `{}` failed: {}", plugin.name(), error))
    }

    fn legacy_scan(&self) -> Result<Option<PluginLegacyScan>, String> {
        let app = self.host.app.borrow();
        let Some(dir) = app
            .legacy_plugin_tools_dir
            .clone()
            .or_else(default_codewhale_tools_dir)
        else {
            return Ok(None);
        };
        if !dir.exists() {
            return Ok(None);
        }
        let tools = crate::tools::plugin::scan_plugin_dir(&dir)
            .into_iter()
            .map(|(path, metadata)| portable_legacy_tool(&path, &metadata))
            .collect();
        Ok(Some(PluginLegacyScan { dir, tools }))
    }

    fn managed_scan(&self, home_override: Option<&Path>) -> Result<PluginManagedScan, String> {
        scan_managed_plugins_portable(home_override)
    }

    fn managed_install(
        &mut self,
        canonical_path: &Path,
        expected_content_hash: &str,
    ) -> Result<PluginMutationReceipt, String> {
        use crate::plugins::install::PluginInstallSource;
        use crate::plugins::mutation::{
            PluginMutationContext, PluginMutationOutcome, PluginMutationRequest,
        };
        let network = plugin_network_policy();
        let expected_content_hash = expected_content_hash.to_string();
        let path = canonical_path.to_path_buf();
        let mut app = self.host.app.borrow_mut();
        let registry = std::sync::Arc::make_mut(&mut app.plugin_registry);
        let outcome = run_async(async move {
            let ctx = PluginMutationContext {
                network: &network,
                max_size: crate::plugins::install::DEFAULT_MAX_SIZE_BYTES,
            };
            crate::plugins::mutation::execute(
                PluginMutationRequest::InstallExact {
                    source: PluginInstallSource::LocalPath(path),
                    expected_content_hash,
                },
                &ctx,
                registry,
            )
            .await
        });
        match outcome {
            Ok(receipt) => {
                let portable = portable_plugin_mutation_receipt(&receipt);
                if matches!(receipt.outcome, PluginMutationOutcome::Installed) {
                    let workspace = app.workspace.clone();
                    app.plugin_registry = app.plugin_registry.rediscover_for_workspace(&workspace);
                    app.refresh_skill_cache();
                }
                Ok(portable)
            }
            Err(error) => Err(format!("Plugin install failed: {error:#}")),
        }
    }

    fn marketplace_state(&self) -> Result<PluginMarketplaceState, String> {
        let app = self.host.app.borrow();
        let store = crate::plugins::marketplace::store::MarketplaceStore::open(
            app.plugin_registry.state_path(),
        )
        .ok_or_else(|| {
            "This plugin registry has no persistence store, so marketplace catalogs cannot be saved."
                .to_string()
        })?;
        let state = store.load()?;
        let stored = state
            .catalogs()
            .values()
            .map(|entry| portable_marketplace_catalog_with_source(entry, &app.plugin_registry))
            .collect();
        Ok(PluginMarketplaceState {
            official: None,
            stored,
        })
    }

    fn marketplace_add(
        &mut self,
        name: &str,
        path: &Path,
    ) -> Result<PluginMarketplaceAddReceipt, String> {
        let app = self.host.app.borrow();
        let store = crate::plugins::marketplace::store::MarketplaceStore::open(
            app.plugin_registry.state_path(),
        )
        .ok_or_else(|| {
            "This plugin registry has no persistence store, so marketplace catalogs cannot be saved."
                .to_string()
        })?;
        let raw_path = path.to_string_lossy();
        let loaded = crate::plugins::marketplace::document::load_catalog_document(
            name,
            &app.workspace,
            &raw_path,
        )?;
        let candidate_count = loaded.candidate_count;
        let warning_count = loaded.warning_count;
        let portable_catalog =
            portable_marketplace_catalog_with_source(&loaded.entry, &app.plugin_registry);
        store.add(&loaded.entry.catalog.id.clone(), loaded.entry)?;
        Ok(PluginMarketplaceAddReceipt {
            name: name.to_string(),
            candidate_count,
            warning_count,
            catalog: portable_catalog,
        })
    }

    fn marketplace_remove(&mut self, name: &str) -> Result<bool, String> {
        let app = self.host.app.borrow();
        let store = crate::plugins::marketplace::store::MarketplaceStore::open(
            app.plugin_registry.state_path(),
        )
        .ok_or_else(|| {
            "This plugin registry has no persistence store, so marketplace catalogs cannot be saved."
                .to_string()
        })?;
        store.remove(name)
    }

    fn marketplace_install(
        &mut self,
        catalog: &str,
        candidate: &str,
    ) -> Result<PluginMutationReceipt, String> {
        let app = self.host.app.borrow();
        let store = crate::plugins::marketplace::store::MarketplaceStore::open(
            app.plugin_registry.state_path(),
        )
        .ok_or_else(|| {
            "This plugin registry has no persistence store, so marketplace catalogs cannot be saved."
                .to_string()
        })?;
        let state = store.load()?;
        let catalog_text = escape_review_text(catalog);
        let candidate_text = escape_review_text(candidate);
        let entry = state.get(catalog).cloned().ok_or_else(|| {
            format!("No marketplace named `{catalog_text}`. Use /plugin marketplace list.")
        })?;
        let candidate_entry = entry.catalog.candidate_by_name(candidate).ok_or_else(|| {
            format!("No candidate `{candidate_text}` in marketplace `{catalog_text}`.")
        })?;
        let spec = match crate::plugins::marketplace::document::resolve_candidate_install(
            &entry,
            candidate_entry,
            &app.plugin_registry,
        ) {
            crate::plugins::marketplace::document::CatalogInstallResolution::Supported {
                spec,
                ..
            } => spec,
            crate::plugins::marketplace::document::CatalogInstallResolution::AlreadyPresent {
                reason,
                ..
            } => {
                return Err(escape_review_text(&reason));
            }
            crate::plugins::marketplace::document::CatalogInstallResolution::Unsupported {
                reason,
            } => {
                let localized = key_to_plugin_message_id(&reason)
                    .map(|message_id| tr(app.ui_locale, message_id).into_owned())
                    .unwrap_or(reason);
                return Err(format!(
                    "Candidate `{candidate_text}` cannot be installed by Codewhale: {}",
                    escape_review_text(&localized)
                ));
            }
            crate::plugins::marketplace::document::CatalogInstallResolution::HasErrors {
                diagnostics,
            } => {
                return Err(format!(
                    "Candidate `{candidate_text}` has parse errors and cannot be installed:\n{}",
                    escape_review_text(&diagnostics)
                ));
            }
        };
        drop(app);
        self.install(&spec, None)
    }
}

/// Resolve the default Codewhale tools directory (mirrors the legacy handler).
fn default_codewhale_tools_dir() -> Option<PathBuf> {
    codewhale_config::codewhale_home()
        .ok()
        .map(|home| home.join("tools"))
}

// ---------------------------------------------------------------------------
// Envelope construction (D1)
// ---------------------------------------------------------------------------

/// Owns fifteen facet objects sharing one synchronous TUI host proxy.
///
/// Handlers borrow only these adapters. Every method delegates to the real App
/// authority and releases its `RefCell` borrow before returning, so facets can
/// be called sequentially without exposing TUI types across the boundary.
pub(crate) struct CommandContextBundle<'a> {
    session: SessionAdapter<'a>,
    model: ModelAdapter<'a>,
    cost: CostAdapter<'a>,
    mode_policy: ModePolicyAdapter<'a>,
    system_prompt: SystemPromptAdapter<'a>,
    skills: SkillsAdapter<'a>,
    workspace: WorkspaceAdapter<'a>,
    presentation: PresentationAdapter<'a>,
    media: MediaAdapter<'a>,
    project: ProjectAdapter<'a>,
    memory: MemoryAdapter<'a>,
    skill_group: SkillGroupAdapter<'a>,
    plugin: PluginAdapter<'a>,
    lifecycle: SessionLifecycleAdapter<'a>,
    control: SessionControlAdapter<'a>,
}

impl<'a> CommandContextBundle<'a> {
    /// Expose exactly the capabilities declared by the command registration.
    pub(crate) fn contexts(&mut self, capabilities: CommandCapabilities) -> CommandContexts<'_> {
        let mut contexts = CommandContexts::empty();
        if capabilities.contains(CommandCapabilities::SESSION) {
            contexts = contexts.with_session(&mut self.session);
        }
        if capabilities.contains(CommandCapabilities::MODEL) {
            contexts = contexts.with_model(&mut self.model);
        }
        if capabilities.contains(CommandCapabilities::COST) {
            contexts = contexts.with_cost(&mut self.cost);
        }
        if capabilities.contains(CommandCapabilities::MODE_POLICY) {
            contexts = contexts.with_mode_policy(&mut self.mode_policy);
        }
        if capabilities.contains(CommandCapabilities::SYSTEM_PROMPT) {
            contexts = contexts.with_system_prompt(&mut self.system_prompt);
        }
        if capabilities.contains(CommandCapabilities::SKILLS) {
            contexts = contexts.with_skills(&mut self.skills);
        }
        if capabilities.contains(CommandCapabilities::WORKSPACE) {
            contexts = contexts.with_workspace(&mut self.workspace);
        }
        if capabilities.contains(CommandCapabilities::PRESENTATION) {
            contexts = contexts.with_presentation(&mut self.presentation);
        }
        if capabilities.contains(CommandCapabilities::MEDIA) {
            contexts = contexts.with_media(&mut self.media);
        }
        if capabilities.contains(CommandCapabilities::MEMORY) {
            contexts = contexts.with_memory(&mut self.memory);
        }
        if capabilities.contains(CommandCapabilities::PROJECT) {
            contexts = contexts.with_project(&mut self.project);
        }
        if capabilities.contains(CommandCapabilities::SKILL_GROUP) {
            contexts = contexts.with_skill_group(&mut self.skill_group);
        }
        if capabilities.contains(CommandCapabilities::PLUGIN) {
            contexts = contexts.with_plugin(&mut self.plugin);
        }
        if capabilities.contains(CommandCapabilities::SESSION_LIFECYCLE) {
            contexts = contexts.with_lifecycle(&mut self.lifecycle);
        }
        if capabilities.contains(CommandCapabilities::SESSION_CONTROL) {
            contexts = contexts.with_control(&mut self.control);
        }
        contexts
    }

    /// Test-only: consume the bundle into independent facet parts.
    #[cfg(test)]
    pub(crate) fn parts(&mut self) -> ContextParts<'_> {
        let all_test_capabilities = CommandCapabilities::SESSION
            .union(CommandCapabilities::MODEL)
            .union(CommandCapabilities::COST)
            .union(CommandCapabilities::MODE_POLICY)
            .union(CommandCapabilities::SYSTEM_PROMPT)
            .union(CommandCapabilities::SKILLS)
            .union(CommandCapabilities::WORKSPACE)
            .union(CommandCapabilities::PRESENTATION)
            .union(CommandCapabilities::MEDIA)
            .union(CommandCapabilities::MEMORY)
            .union(CommandCapabilities::PROJECT)
            .union(CommandCapabilities::SKILL_GROUP)
            .union(CommandCapabilities::PLUGIN)
            .union(CommandCapabilities::SESSION_LIFECYCLE)
            .union(CommandCapabilities::SESSION_CONTROL);
        self.contexts(all_test_capabilities).into_parts()
    }
}

impl App {
    /// Build an App-free capability envelope backed by authoritative TUI
    /// operations. The shared proxy is synchronous and local to one dispatch.
    pub(crate) fn command_contexts(&mut self) -> CommandContextBundle<'_> {
        let host = Rc::new(CommandHost {
            app: RefCell::new(self),
        });
        CommandContextBundle {
            session: SessionAdapter { host: host.clone() },
            model: ModelAdapter { host: host.clone() },
            cost: CostAdapter { host: host.clone() },
            mode_policy: ModePolicyAdapter { host: host.clone() },
            system_prompt: SystemPromptAdapter { host: host.clone() },
            skills: SkillsAdapter { host: host.clone() },
            workspace: WorkspaceAdapter { host: host.clone() },
            presentation: PresentationAdapter { host: host.clone() },
            media: MediaAdapter { host: host.clone() },
            project: ProjectAdapter { host: host.clone() },
            memory: MemoryAdapter { host: host.clone() },
            skill_group: SkillGroupAdapter { host: host.clone() },
            plugin: PluginAdapter { host: host.clone() },
            lifecycle: SessionLifecycleAdapter { host: host.clone() },
            control: SessionControlAdapter { host },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codewhale_localization::Locale;
    use codewhale_models::Role;
    use tempfile::TempDir;

    fn test_app() -> App {
        crate::test_support::test_app_with_options(crate::test_support::test_tui_options(
            PathBuf::from("."),
        ))
    }

    /// A 1x1 PNG for media adapter tests.
    const PNG_1X1: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f,
        0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];

    #[test]
    fn pending_groups_is_sorted_unique_and_matches_checked_in_frontier() {
        let mut sorted = PENDING_GROUPS.to_vec();
        sorted.sort_unstable();
        assert_eq!(PENDING_GROUPS, sorted.as_slice(), "frontier must be sorted");
        let unique: std::collections::BTreeSet<&str> = PENDING_GROUPS.iter().copied().collect();
        assert_eq!(
            unique.len(),
            PENDING_GROUPS.len(),
            "frontier must be unique"
        );

        let topology: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../scripts/command-migration-topology.json"
        ))
        .expect("checked-in topology must be valid JSON");
        let frontier = topology["frontier"]
            .as_array()
            .expect("topology frontier")
            .iter()
            .map(|entry| entry.as_str().expect("string frontier entry"))
            .collect::<Vec<_>>();
        assert_eq!(PENDING_GROUPS, frontier.as_slice());
    }

    #[test]
    fn boundary_mappings_cover_every_variant() {
        for mode in [AppMode::Agent, AppMode::Plan, AppMode::Operate] {
            let command = to_command_mode(mode);
            assert_eq!(from_command_mode(command), mode);
        }
        for approval in [
            ApprovalMode::Auto,
            ApprovalMode::Bypass,
            ApprovalMode::Suggest,
            ApprovalMode::Never,
        ] {
            let _ = to_command_approval(approval);
        }
        for effort in [
            ReasoningEffort::Off,
            ReasoningEffort::Minimal,
            ReasoningEffort::Low,
            ReasoningEffort::Medium,
            ReasoningEffort::High,
            ReasoningEffort::XHigh,
            ReasoningEffort::Ultra,
            ReasoningEffort::Auto,
            ReasoningEffort::Max,
        ] {
            let _ = to_command_effort(effort);
        }
        for currency in [CostCurrency::Usd, CostCurrency::Cny] {
            let command = to_command_currency(currency);
            assert_eq!(from_command_currency(command), currency);
        }
    }

    #[test]
    fn key_to_message_id_resolves_convention_keys_and_rejects_unknown() {
        assert_eq!(
            key_to_message_id("cmd_balance_description"),
            Some(MessageId::CmdBalanceDescription)
        );
        assert_eq!(
            key_to_message_id("cmd_voice_control_description"),
            Some(MessageId::CmdVoiceControlDescription)
        );
        assert_eq!(key_to_message_id("cmd_nonexistent_description"), None);
        assert_eq!(key_to_message_id(""), None);
    }

    #[test]
    fn cost_adapter_delegates_totals_high_water_and_route_receipt_to_app() {
        let mut app = test_app();
        app.cost_currency = CostCurrency::Usd;
        {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let cost = parts.cost.as_mut().expect("cost facet");
            cost.accrue_cost_estimate(3.0, CommandCurrency::Usd);
            cost.record_turn_cost(
                4.0,
                CommandCurrency::Cny,
                Some("provider=deepseek model=x".to_string()),
            );
            assert_eq!(cost.session_cost_for_currency(CommandCurrency::Usd), 3.0);
            assert_eq!(cost.session_cost_for_currency(CommandCurrency::Cny), 4.0);
        }
        assert_eq!(app.session_cost_for_currency(CostCurrency::Usd), 3.0);
        assert_eq!(app.session_cost_for_currency(CostCurrency::Cny), 4.0);
        assert_eq!(
            app.displayed_session_cost_for_currency(CostCurrency::Usd),
            3.0
        );
        assert!(
            app.session
                .cost_route_receipts
                .contains("provider=deepseek model=x")
        );
    }

    #[test]
    fn session_adapter_delegates_message_and_queue_operations_to_app() {
        let mut app = test_app();
        app.current_session_id = Some("s1".to_string());
        app.session.total_tokens = 42;
        app.queue_message(crate::tui::app::QueuedMessage {
            display: "q".to_string(),
            skill_instruction: None,
            skill_provenance: None,
            history_echoed: false,
        });
        {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let session = parts.session.as_mut().expect("session facet");
            assert_eq!(session.session_id().as_deref(), Some("s1"));
            session.add_message(Message {
                role: Role::User,
                content: vec![],
            });
            assert_eq!(session.api_messages().len(), 1);
            assert_eq!(session.queued_message_count(), 1);
            assert!(session.remove_queued_message(0).is_ok());
            assert!(session.remove_queued_message(5).is_err());
            assert_eq!(session.total_tokens(), 42);
        }
        assert_eq!(app.api_messages.len(), 1);
        assert_eq!(app.queued_message_count(), 0);
    }

    #[test]
    fn model_adapter_delegates_selection_and_route_invalidation_to_app() {
        let mut app = test_app();
        app.last_effective_model = Some("stale-model".to_string());
        {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let model = parts.model.as_mut().expect("model facet");
            model.set_model_selection("auto".to_string(), Some(to_provider_id("deepseek")));
            assert!(model.auto_model());
            assert_eq!(model.current_model(), "auto");
            assert_eq!(
                model.provider_identity().map(|id| id.0).as_deref(),
                Some("deepseek")
            );
        }
        assert!(app.last_effective_model.is_none());
        assert_eq!(app.provider_identity_for_persistence(), "deepseek");
    }

    #[test]
    fn mode_policy_adapter_delegates_mode_and_shell_policy_to_app() {
        let mut app = test_app();
        app.set_agent_shell_access(false);
        {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let policy = parts.mode_policy.as_mut().expect("mode facet");
            policy.set_mode(CommandMode::Operate);
            policy.set_shell_access(true);
            assert!(policy.allow_shell());
            assert_eq!(policy.mode(), CommandMode::Operate);
        }
        assert_eq!(
            app.mode,
            AppMode::Operate,
            "adapter delegates to App authority"
        );
        assert!(app.allow_shell);
    }

    #[test]
    fn system_prompt_adapter_returns_owned_prompt() {
        let mut app = test_app();
        app.system_prompt = Some(SystemPrompt::Text("system".to_string()));
        let mut bundle = app.command_contexts();
        let parts = bundle.parts();
        assert!(
            parts
                .system_prompt
                .expect("system prompt facet")
                .system_prompt()
                .is_some()
        );
    }

    #[test]
    fn workspace_adapter_returns_path_and_snapshot() {
        let mut app = test_app();
        let expected = app.workspace.clone();
        let mut bundle = app.command_contexts();
        let parts = bundle.parts();
        let workspace = parts.workspace.expect("workspace facet");
        assert_eq!(workspace.workspace(), expected);
        assert!(workspace.work_state_snapshot().is_ok());
    }

    #[test]
    fn envelope_exposes_all_facets_without_app_in_handler_surface() {
        let mut app = test_app();
        let mut bundle = app.command_contexts();
        let parts = bundle.parts();
        assert!(parts.session.is_some());
        assert!(parts.model.is_some());
        assert!(parts.cost.is_some());
        assert!(parts.mode_policy.is_some());
        assert!(parts.system_prompt.is_some());
        assert!(parts.skills.is_some());
        assert!(parts.workspace.is_some());
        assert!(parts.presentation.is_some());
        assert!(parts.media.is_some());
    }

    // -----------------------------------------------------------------------
    // FEAT-018 adapter tests: presentation (D3), media (D4), digest (D5)
    // -----------------------------------------------------------------------

    #[test]
    fn presentation_adapter_resolves_utility_keys_with_english_fallback() {
        let mut app = test_app();
        app.ui_locale = Locale::En;
        let mut bundle = app.command_contexts();
        let mut parts = bundle.parts();
        let presentation = parts.presentation.as_mut().expect("presentation facet");

        // automation_usage has no placeholders.
        let usage = presentation
            .translate("automation_usage", &[])
            .expect("automation usage key");
        assert!(
            usage.contains("/automation"),
            "expected usage text, got {usage}"
        );

        // mcp_recommended_unknown_id needs {recommendations_command}.
        let unknown = presentation
            .translate(
                "mcp_recommended_unknown_id",
                &[("recommendations_command", "/mcp recommendations")],
            )
            .expect("mcp unknown-id key");
        assert!(
            unknown.contains("/mcp recommendations"),
            "expected replacement text, got {unknown}"
        );

        // mcp_recommendation_github needs {endpoint}, {login_command}, {add_command}.
        let github = presentation
            .translate(
                "mcp_recommendation_github",
                &[
                    ("endpoint", "https://api.githubcopilot.com/mcp/"),
                    ("login_command", "/mcp login github"),
                    ("add_command", "/mcp add recommended github"),
                ],
            )
            .expect("github recommendation key");
        assert!(
            github.contains("https://api.githubcopilot.com/mcp/"),
            "{github}"
        );
        assert!(
            !github.contains("{endpoint}"),
            "placeholder must be replaced"
        );
    }

    #[test]
    fn presentation_adapter_rejects_unknown_keys_and_invalid_replacements() {
        let mut app = test_app();
        app.ui_locale = Locale::En;
        let mut bundle = app.command_contexts();
        let mut parts = bundle.parts();
        let presentation = parts.presentation.as_mut().expect("presentation facet");

        let unknown = presentation.translate("no_such_key", &[]);
        assert!(unknown.is_err(), "unknown key must fail safely");
        let err = unknown.unwrap_err();
        assert!(
            !err.contains("no_such_key"),
            "no raw lookup key exposure (D3): {err}"
        );

        // Missing required replacement.
        assert!(
            presentation
                .translate("mcp_recommendation_github", &[])
                .is_err()
        );
        // Extra replacement not present in the template.
        assert!(
            presentation
                .translate("automation_usage", &[("no_such_placeholder", "value")],)
                .is_err()
        );
        // Duplicate replacement names.
        assert!(
            presentation
                .translate(
                    "mcp_recommendation_github",
                    &[
                        ("endpoint", "a"),
                        ("endpoint", "b"),
                        ("login_command", "c"),
                        ("add_command", "d"),
                    ],
                )
                .is_err()
        );
    }

    #[test]
    fn media_adapter_attaches_valid_image_and_preserves_confirm() {
        let tmpdir = tempfile::TempDir::new().expect("tempdir");
        let image_path = tmpdir.path().join("photo.png");
        std::fs::write(&image_path, PNG_1X1).expect("write image fixture");

        let mut app = test_app();
        let mut bundle = app.command_contexts();
        let mut parts = bundle.parts();
        let media = parts.media.as_mut().expect("media facet");
        let receipt = media
            .attach_media(&image_path)
            .expect("valid image attaches");
        assert_eq!(receipt.kind, "image");
        assert_eq!(receipt.path, image_path.canonicalize().expect("canonical"));
        assert!(
            app.input.contains("[Attached image:"),
            "composer must contain the attachment reference"
        );
    }

    #[test]
    fn media_adapter_rejects_invalid_media_atomically() {
        let tmpdir = tempfile::TempDir::new().expect("tempdir");

        // Missing path.
        let mut app = test_app();
        {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let media = parts.media.as_mut().expect("media facet");
            let missing = tmpdir.path().join("missing.png");
            let err = media.attach_media(&missing).unwrap_err();
            assert!(err.contains("Attachment not found"), "{err}");
        }
        assert!(
            app.input.is_empty(),
            "refused attachment must not reach composer"
        );

        // Directory is not a file.
        {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let media = parts.media.as_mut().expect("media facet");
            let dir = tmpdir.path().to_path_buf();
            let err = media.attach_media(&dir).unwrap_err();
            assert!(err.contains("Attachment is not a file"), "{err}");
        }
        assert!(app.input.is_empty());

        // Unsupported extension.
        std::fs::write(tmpdir.path().join("notes.txt"), b"text").expect("write fixture");
        {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let media = parts.media.as_mut().expect("media facet");
            let err = media
                .attach_media(&tmpdir.path().join("notes.txt"))
                .unwrap_err();
            assert!(err.contains("Unsupported attachment type"), "{err}");
        }
        assert!(app.input.is_empty());

        // Corrupt image with a valid extension.
        std::fs::write(tmpdir.path().join("bad.png"), b"not an image").expect("write fixture");
        {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let media = parts.media.as_mut().expect("media facet");
            let err = media
                .attach_media(&tmpdir.path().join("bad.png"))
                .unwrap_err();
            assert!(!err.is_empty(), "corrupt image must fail");
        }
        assert!(app.input.is_empty());
    }

    #[test]
    fn media_adapter_attaches_valid_video_reference() {
        // A real (non-image) media file with a video extension passes the
        // extension gate without byte validation, matching baseline /attach.
        let tmpdir = tempfile::TempDir::new().expect("tempdir");
        let video_path = tmpdir.path().join("clip.mp4");
        std::fs::write(&video_path, b"not a real mp4 but extension-gated").expect("write");

        let mut app = test_app();
        let mut bundle = app.command_contexts();
        let mut parts = bundle.parts();
        let media = parts.media.as_mut().expect("media facet");
        let receipt = media
            .attach_media(&video_path)
            .expect("video path attaches by extension");
        assert_eq!(receipt.kind, "video");
        assert!(app.input.contains("[Attached video:"), "{}", app.input);
    }

    #[test]
    fn workspace_digest_adapter_preserves_no_active_and_failure_semantics() {
        let mut app = test_app();
        app.runtime_services.work = None;
        {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let workspace = parts.workspace.as_mut().expect("workspace facet");
            assert_eq!(
                workspace.operation_digest().expect("no-runtime digest"),
                "No active operations or to-do items."
            );
        }
    }

    #[test]
    fn bundle_construction_performs_no_eager_work() {
        let mut app = test_app();
        let input_before = app.input.clone();
        {
            let mut bundle = app.command_contexts();
            let parts = bundle.parts();
            // Merely constructing the bundle must not mutate composer state or
            // perform capability work; the adapters only run on method calls.
            let _ = parts.media.is_some();
            let _ = parts.presentation.is_some();
            let _ = parts.memory.is_some();
            let _ = parts.project.is_some();
        }
        assert_eq!(app.input, input_before, "no eager composer mutation");
    }
    // FEAT-021 project adapter tests
    // ---------------------------------------------------------------------

    #[test]
    fn key_to_project_message_id_resolves_goal_runtime_keys_and_rejects_unknown() {
        // FEAT-021 D5: only /goal uses runtime translations via the project
        // key map; unknown keys fail safely.
        assert_eq!(
            key_to_project_message_id("goal_control_accepted"),
            Some(MessageId::GoalControlAccepted)
        );
        assert_eq!(
            key_to_project_message_id("goal_status_idle_hint"),
            Some(MessageId::GoalStatusIdleHint)
        );
        assert_eq!(key_to_project_message_id("goal_bogus_key"), None);
        assert_eq!(key_to_project_message_id(""), None);
    }

    #[test]
    fn presentation_translate_resolves_project_keys_with_locale_and_fallback() {
        // The presentation facet resolves the project runtime keys through the
        // current catalog (authoritative English fallback preserved).
        let mut app = test_app();
        let mut bundle = app.command_contexts();
        let mut parts = bundle.parts();
        let presentation = parts.presentation.as_mut().expect("presentation facet");
        let accepted = presentation
            .translate("goal_control_accepted", &[])
            .expect("goal_control_accepted must resolve");
        assert!(
            accepted.contains("Goal control saved"),
            "English fallback text expected: {accepted}"
        );
        let hint = presentation
            .translate("goal_status_idle_hint", &[])
            .expect("goal_status_idle_hint must resolve");
        assert!(hint.contains("not running now"), "hint: {hint}");
        assert!(
            presentation.translate("goal_bogus", &[]).is_err(),
            "unknown key must fail safely"
        );
    }

    // -----------------------------------------------------------------------
    // FEAT-019: memory adapter mappings (D6/D9)
    // -----------------------------------------------------------------------

    /// App with an isolated temp memory file; memory feature enabled or not.
    fn memory_test_app(tmpdir: &TempDir, use_memory: bool) -> App {
        let options = crate::test_support::test_tui_options(tmpdir.path());
        let options = crate::tui::app::TuiOptions {
            memory_path: tmpdir.path().join("memory.md"),
            use_memory,
            ..options
        };
        crate::test_support::test_app_with_options(options)
    }

    /// Give a temp workspace a git origin so workspace identity resolves.
    fn git_origin(workspace: &Path) {
        let init = std::process::Command::new("git")
            .arg("-C")
            .arg(workspace)
            .args(["init", "-q"])
            .status()
            .unwrap();
        assert!(init.success(), "git init must succeed");
        let remote = std::process::Command::new("git")
            .arg("-C")
            .arg(workspace)
            .args(["remote", "add", "origin", "https://example.test/repo.git"])
            .status()
            .unwrap();
        assert!(remote.success(), "git remote add must succeed");
    }

    #[test]
    fn memory_adapter_maps_path_and_enablement() {
        let tmp = TempDir::new().unwrap();
        let mut enabled = memory_test_app(&tmp, true);
        let mut bundle = enabled.command_contexts();
        let memory = bundle.parts().memory.expect("memory facet must be present");
        assert_eq!(memory.memory_path(), tmp.path().join("memory.md"));
        assert!(memory.memory_enabled());

        let mut disabled = memory_test_app(&tmp, false);
        let mut bundle = disabled.command_contexts();
        let memory = bundle.parts().memory.expect("memory facet must be present");
        assert!(!memory.memory_enabled());
    }

    #[test]
    fn memory_adapter_status_and_path_map_native_store() {
        let tmp = TempDir::new().unwrap();
        let mut app = memory_test_app(&tmp, true);
        let mut bundle = app.command_contexts();
        let memory = bundle.parts().memory.expect("memory facet");

        // Fallback root derivation mirrors the legacy handler: a plain
        // `memory.md` file is not a native global source, so the root is the
        // sibling `memory` directory.
        let status = memory.status().expect("status");
        assert_eq!(status.root, tmp.path().join("memory"));
        assert_eq!(
            status.source,
            tmp.path().join("memory").join("global").join("MEMORY.md")
        );
        assert_eq!(
            status.index,
            tmp.path().join("memory").join("index.sqlite3")
        );
        assert_eq!(memory.path().expect("path"), tmp.path().join("memory"));
    }

    #[test]
    fn memory_adapter_workspace_identity_resolves_and_preserves_errors() {
        let tmp = TempDir::new().unwrap();
        git_origin(tmp.path());
        let mut app = memory_test_app(&tmp, true);
        let mut bundle = app.command_contexts();
        let memory = bundle.parts().memory.expect("memory facet");
        // A git origin resolves to a stable workspace identity (sha256 digest).
        let id = memory.workspace_id(tmp.path()).expect("workspace id");
        assert!(!id.is_empty());
        assert_eq!(id, memory.workspace_id(tmp.path()).expect("stable id"));

        // A plain directory without git origin preserves the established error.
        let plain = TempDir::new().unwrap();
        let err = memory
            .workspace_id(plain.path())
            .expect_err("missing origin");
        assert_eq!(
            err,
            "workspace memory requires a git repository with an origin"
        );
    }

    #[test]
    fn project_adapter_maps_lsp_state() {
        let mut app = test_app();
        app.lsp_enabled = false;
        assert!(!app.lsp_enabled);
        {
            let mut bundle = app.command_contexts();
            let project = bundle
                .parts()
                .project
                .expect("project facet must be present");
            assert!(!project.lsp_enabled());

            project.lsp_set(true).unwrap();
            assert!(project.lsp_enabled());
            project.lsp_set(false).unwrap();
            assert!(!project.lsp_enabled());
        }
        assert!(!app.lsp_enabled);
    }

    #[test]
    fn project_adapter_share_projection_maps_history_model_and_mode() {
        let mut app = test_app();
        app.model = "deepseek-v4-pro".to_string();
        app.mode = codewhale_config::AppMode::Agent;
        let mut bundle = app.command_contexts();
        let project = bundle
            .parts()
            .project
            .expect("project facet must be present");

        // Empty history → empty share branch.
        let share = project.share_projection();
        assert!(share.history_is_empty);
        assert_eq!(share.history_len, 0);

        // Populated history → length and labels match host exactly.
        app.history.push(crate::tui::history::HistoryCell::User {
            content: "hello".to_string(),
        });
        app.history
            .push(crate::tui::history::HistoryCell::Assistant {
                content: "world".to_string(),
                streaming: false,
            });
        let mut bundle = app.command_contexts();
        let project = bundle
            .parts()
            .project
            .expect("project facet must be present");
        let share = project.share_projection();
        assert!(!share.history_is_empty);
        assert_eq!(share.history_len, 2);
        assert_eq!(share.model, "deepseek-v4-pro");
        assert_eq!(share.mode_label, codewhale_config::AppMode::Agent.label());
    }

    #[test]
    fn project_adapter_goal_projection_preserves_visible_and_effective_state() {
        let mut app = test_app();
        app.goal.objective = Some("Ship FEAT-021".to_string());
        app.goal.status = crate::tools::goal::GoalStatus::Active;
        app.goal.time_used_seconds = 42;
        app.goal.token_budget = Some(50_000);
        app.goal.tokens_used = 1_000;
        app.goal.continuation_count = 3;
        app.session.total_conversation_tokens = 2_000;
        app.goal_continuation_waiting = true;
        app.is_loading = false;
        app.api_messages.push(codewhale_models::Message {
            role: codewhale_models::Role::User,
            content: vec![codewhale_models::ContentBlock::Text {
                text: "work".to_string(),
                cache_control: None,
            }],
        });

        let mut bundle = app.command_contexts();
        let project = bundle
            .parts()
            .project
            .expect("project facet must be present");
        let goal = project.goal_state();
        assert_eq!(goal.objective.as_deref(), Some("Ship FEAT-021"));
        assert_eq!(goal.status, ProjectGoalStatus::Active);
        assert_eq!(goal.time_used_seconds, 42);
        assert_eq!(goal.token_budget, Some(50_000));
        assert_eq!(goal.tokens_used, 1_000);
        assert_eq!(goal.session_total_tokens, 2_000);
        assert_eq!(goal.continuation_count, 3);
        assert!(!goal.pending_controls);
        assert!(goal.goal_continuation_waiting);
        assert!(goal.conversation_present);

        // Pending controls flip the effective source to the durable state.
        app.pending_goal_controls
            .push_back(crate::tui::app::PendingGoalControl {
                goal_id: None,
                intent: crate::tui::app::GoalControlIntent::SetStatus {
                    status: crate::tools::goal::GoalStatus::Paused,
                    clear: false,
                },
                dispatched: false,
            });
        app.last_known_goal_state = Some(crate::session_manager::SessionGoalState {
            schema_version: 1,
            goal_id: None,
            last_gap_fingerprint: None,
            repeated_gap_count: 0,
            last_gap_pass: None,
            objective: "Durable objective".to_string(),
            status: crate::session_manager::SessionGoalStatus::Paused,
            token_budget: None,
            tokens_used: 0,
            time_used_seconds: 0,
            continuation_count: 0,
            elapsed_seconds: 0,
            pause_reason: None,
        });
        let mut bundle = app.command_contexts();
        let project = bundle
            .parts()
            .project
            .expect("project facet must be present");
        let goal = project.goal_state();
        assert!(goal.pending_controls);
        assert_eq!(
            goal.last_known_objective.as_deref(),
            Some("Durable objective")
        );
        assert_eq!(goal.last_known_status, Some(ProjectGoalStatus::Paused));
    }

    #[test]
    fn project_adapter_exposure_matches_main_envelope_model() {
        // main's envelope always populates every adapter (no capability
        // bitmask yet); the project facet is present and usable, and the
        // handlers destructure only the facets they need.
        let mut app = test_app();
        let mut bundle = app.command_contexts();
        let parts = bundle.parts();
        assert!(parts.project.is_some());
        assert!(parts.workspace.is_some());
        assert!(parts.presentation.is_some());
    }

    #[test]
    fn memory_adapter_search_remember_get_export_reindex_work() {
        let tmp = TempDir::new().unwrap();
        let mut app = memory_test_app(&tmp, true);
        let mut bundle = app.command_contexts();
        let memory = bundle.parts().memory.expect("memory facet");

        // Global remember produces a portable remembered location.
        let remembered = memory
            .remember(MemoryRememberTarget::Global, "alpha note")
            .expect("remember global");
        assert!(remembered.source.ends_with("global/MEMORY.md"));
        assert_eq!(remembered.line_start, 2);

        // Workspace remember targets the workspace scope with the typed id.
        git_origin(tmp.path());
        let workspace_id = memory.workspace_id(tmp.path()).expect("id");
        let workspace_note = memory
            .remember(
                MemoryRememberTarget::Workspace { workspace_id },
                "workspace-only note",
            )
            .expect("remember workspace");
        assert!(
            workspace_note
                .source
                .to_string_lossy()
                .contains("workspace")
        );

        // Search finds workspace-scoped content only for the given workspace.
        let hits = memory
            .search(tmp.path(), "workspace-only", 10)
            .expect("search");
        assert_eq!(hits.len(), 1);
        assert!(hits[0].text.contains("workspace-only note"));
        assert_eq!(hits[0].line_start, 2);
        // Empty results stay a typed empty vec, never an error.
        assert!(
            memory
                .search(tmp.path(), "zzz-no-match", 10)
                .expect("empty search")
                .is_empty()
        );

        // Get distinguishes found from not-found (first rowid is 1).
        match memory.get(tmp.path(), 1) {
            Ok(MemoryGetOutcome::Found(hit)) => assert!(!hit.text.is_empty()),
            other => panic!("expected found entry, got {other:?}"),
        }
        assert_eq!(
            memory.get(tmp.path(), 999_999).expect("get"),
            MemoryGetOutcome::NotFound
        );

        // Export carries the document; reindex reports the typed count.
        let exported = memory.export().expect("export");
        assert!(exported.content.contains("alpha note"));
        assert!(exported.content.contains("workspace-only note"));
        assert!(memory.reindex().expect("reindex").entry_count >= 1);
    }

    #[test]
    fn memory_adapter_import_distinguishes_imported_from_skipped() {
        let tmp = TempDir::new().unwrap();
        let legacy = tmp.path().join("memory.md");
        std::fs::write(&legacy, "# legacy\n\n- imported line").unwrap();
        let mut app = memory_test_app(&tmp, true);
        let mut bundle = app.command_contexts();
        let memory = bundle.parts().memory.expect("memory facet");

        let imported = memory.import().expect("import");
        let MemoryImportOutcome::Imported { destination } = imported else {
            panic!("first import must be imported");
        };
        assert!(destination.ends_with("global/MEMORY.md"));

        // Idempotent: an existing global source reports skipped.
        assert_eq!(
            memory.import().expect("second"),
            MemoryImportOutcome::Skipped
        );
    }

    #[test]
    fn memory_adapter_deletes_are_scoped_and_preserve_other_memory() {
        let tmp = TempDir::new().unwrap();
        git_origin(tmp.path());
        let mut app = memory_test_app(&tmp, true);
        let mut bundle = app.command_contexts();
        let memory = bundle.parts().memory.expect("memory facet");

        memory
            .remember(MemoryRememberTarget::Global, "keep global")
            .expect("global");
        let workspace_id = memory.workspace_id(tmp.path()).expect("id");
        memory
            .remember(
                MemoryRememberTarget::Workspace { workspace_id },
                "remove workspace",
            )
            .expect("workspace");

        // Workspace deletion removes only the workspace scope.
        memory
            .delete_workspace(tmp.path())
            .expect("workspace delete");
        assert!(
            memory
                .search(tmp.path(), "remove workspace", 10)
                .expect("search")
                .is_empty()
        );
        assert_eq!(
            memory.search(tmp.path(), "keep global", 10).unwrap().len(),
            1
        );

        // Global deletion removes the global scope but keeps the workspace one.
        memory
            .remember(
                MemoryRememberTarget::Workspace {
                    workspace_id: memory.workspace_id(tmp.path()).expect("id"),
                },
                "workspace survivor",
            )
            .expect("workspace again");
        memory
            .delete(MemoryDeleteScope::Global)
            .expect("global delete");
        assert!(
            memory
                .search(tmp.path(), "keep global", 10)
                .expect("search")
                .is_empty()
        );
        assert_eq!(
            memory
                .search(tmp.path(), "workspace survivor", 10)
                .unwrap()
                .len(),
            1
        );

        // All deletion removes every scope.
        memory.delete(MemoryDeleteScope::All).expect("all delete");
        assert!(
            memory
                .search(tmp.path(), "workspace survivor", 10)
                .expect("search")
                .is_empty()
        );
    }

    #[test]
    fn memory_adapter_preserves_workspace_delete_error_text() {
        let tmp = TempDir::new().unwrap();
        let mut app = memory_test_app(&tmp, true);
        let mut bundle = app.command_contexts();
        let memory = bundle.parts().memory.expect("memory facet");
        let err = memory
            .delete_workspace(tmp.path())
            .expect_err("missing origin");
        assert_eq!(
            err,
            "workspace memory requires a git repository with an origin"
        );
    }

    #[test]
    fn envelope_exposes_only_declared_capabilities() {
        let tmp = TempDir::new().unwrap();
        let mut app = memory_test_app(&tmp, true);
        let mut bundle = app.command_contexts();

        // Memory-only: memory present, workspace/session absent.
        let parts = bundle.contexts(CommandCapabilities::MEMORY).into_parts();
        assert!(parts.memory.is_some());
        assert!(parts.workspace.is_none());
        assert!(parts.session.is_none());

        // Workspace-only: memory absent.
        let parts = bundle.contexts(CommandCapabilities::WORKSPACE).into_parts();
        assert!(parts.workspace.is_some());
        assert!(parts.memory.is_none());

        // Workspace | MEMORY: both present, presentation/media absent.
        let parts = bundle
            .contexts(CommandCapabilities::WORKSPACE.union(CommandCapabilities::MEMORY))
            .into_parts();
        assert!(parts.workspace.is_some());
        assert!(parts.memory.is_some());
        assert!(parts.presentation.is_none());
        assert!(parts.media.is_none());

        // Lifecycle-only: lifecycle present and every unrelated slot absent.
        let parts = bundle
            .contexts(CommandCapabilities::SESSION_LIFECYCLE)
            .into_parts();
        assert!(parts.lifecycle.is_some());
        assert!(parts.session.is_none());
        assert!(parts.model.is_none());
        assert!(parts.cost.is_none());
        assert!(parts.mode_policy.is_none());
        assert!(parts.system_prompt.is_none());
        assert!(parts.skills.is_none());
        assert!(parts.workspace.is_none());
        assert!(parts.presentation.is_none());
        assert!(parts.media.is_none());
        assert!(parts.memory.is_none());
        assert!(parts.project.is_none());
        assert!(parts.skill_group.is_none());
        assert!(parts.plugin.is_none());

        // Control-only: control present and every unrelated slot absent.
        let parts = bundle
            .contexts(CommandCapabilities::SESSION_CONTROL)
            .into_parts();
        assert!(parts.control.is_some());
        assert!(parts.session.is_none());
        assert!(parts.model.is_none());
        assert!(parts.cost.is_none());
        assert!(parts.mode_policy.is_none());
        assert!(parts.system_prompt.is_none());
        assert!(parts.skills.is_none());
        assert!(parts.workspace.is_none());
        assert!(parts.presentation.is_none());
        assert!(parts.media.is_none());
        assert!(parts.memory.is_none());
        assert!(parts.project.is_none());
        assert!(parts.skill_group.is_none());
        assert!(parts.plugin.is_none());
        assert!(parts.lifecycle.is_none());

        // `/remote-env`: exactly control plus presentation.
        let parts = bundle
            .contexts(CommandCapabilities::SESSION_CONTROL.union(CommandCapabilities::PRESENTATION))
            .into_parts();
        assert!(parts.control.is_some());
        assert!(parts.presentation.is_some());
        assert!(parts.session.is_none());
        assert!(parts.workspace.is_none());
        assert!(parts.lifecycle.is_none());
        assert!(parts.plugin.is_none());

        // Unrelated capability: memory, lifecycle, and control all absent.
        let parts = bundle.contexts(CommandCapabilities::SESSION).into_parts();
        assert!(parts.session.is_some());
        assert!(parts.memory.is_none());
        assert!(parts.lifecycle.is_none());
        assert!(parts.control.is_none());
    }

    // ─── FEAT-022 skill-group adapter tests ───────────────────────────────────

    /// Pins HOME to a tempdir for the duration of the test under the
    /// crate-wide env mutex (keeps global skill/snapshot discovery hermetic).
    struct ScopedHome {
        prev: Option<std::ffi::OsString>,
        _home: TempDir,
        _guard: crate::test_support::TestEnvLock,
    }
    impl Drop for ScopedHome {
        fn drop(&mut self) {
            // SAFETY: process-wide lock still held.
            unsafe {
                match self.prev.take() {
                    Some(v) => std::env::set_var("HOME", v),
                    None => std::env::remove_var("HOME"),
                }
            }
        }
    }
    fn scoped_home(_workspace: &TempDir) -> ScopedHome {
        let guard = crate::test_support::lock_test_env();
        let prev = std::env::var_os("HOME");
        let home = TempDir::new().expect("home tempdir");
        // SAFETY: serialised by the global env lock.
        unsafe {
            std::env::set_var("HOME", home.path());
        }
        ScopedHome {
            prev,
            _home: home,
            _guard: guard,
        }
    }

    fn skill_test_app(tmp: &TempDir, skills_dir: &Path) -> App {
        let mut options = crate::test_support::test_tui_options(tmp.path());
        options.skills_dir = skills_dir.to_path_buf();
        crate::test_support::test_app_with_options(options)
    }

    fn write_skill(dir: &Path, name: &str) {
        let skill_dir = dir.join(name);
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: {name} skill\n---\n{name} instructions"),
        )
        .unwrap();
    }

    #[test]
    fn skill_group_projection_maps_native_skills_and_dirs() {
        let tmp = TempDir::new().unwrap();
        let _home = scoped_home(&tmp);
        let skills_dir = tmp.path().join("skills");
        write_skill(&skills_dir, "demo");
        let mut app = skill_test_app(&tmp, &skills_dir);
        let mut bundle = app.command_contexts();
        let group = bundle
            .parts()
            .skill_group
            .expect("skill_group facet must be present");
        let projection = group.skill_registry_projection();
        assert_eq!(projection.total, 1);
        assert_eq!(projection.entries.len(), 1);
        assert_eq!(projection.entries[0].name, "demo");
        assert_eq!(projection.entries[0].description, "demo skill");
        assert_eq!(projection.entries[0].source, SkillSourceKind::Native);
        assert!(projection.entries[0].path.is_some());
        assert_eq!(projection.skills_dir, skills_dir.display().to_string());
        assert!(!projection.dirs.is_empty());
        assert!(projection.warnings.is_empty());
    }

    #[test]
    fn skill_group_projection_reports_empty_registry() {
        let tmp = TempDir::new().unwrap();
        let _home = scoped_home(&tmp);
        let skills_dir = tmp.path().join("skills");
        std::fs::create_dir_all(&skills_dir).unwrap();
        let mut app = skill_test_app(&tmp, &skills_dir);
        let mut bundle = app.command_contexts();
        let group = bundle
            .parts()
            .skill_group
            .expect("skill_group facet must be present");
        let projection = group.skill_registry_projection();
        assert_eq!(projection.total, 0);
        assert!(projection.entries.is_empty());
    }

    #[test]
    fn skill_group_activation_sets_active_skill_and_history() {
        let tmp = TempDir::new().unwrap();
        let _home = scoped_home(&tmp);
        let skills_dir = tmp.path().join("skills");
        write_skill(&skills_dir, "demo");
        let mut app = skill_test_app(&tmp, &skills_dir);
        {
            let mut bundle = app.command_contexts();
            let group = bundle
                .parts()
                .skill_group
                .expect("skill_group facet must be present");
            let outcome = group.activate_skill("demo").unwrap();
            assert_eq!(outcome.name, "demo");
            assert_eq!(outcome.description, "demo skill");
        }
        assert!(app.active_skill.is_some());
        assert!(
            app.active_skill
                .as_deref()
                .unwrap()
                .contains("# Skill: demo")
        );
        assert!(app.active_skill_provenance.is_none());
        assert!(!app.history.is_empty());
    }

    #[test]
    fn skill_group_activation_looks_up_exact_name() {
        // The `/skill new` -> skill-creator alias is handler-side parsing
        // (Phase 4); the delegate performs an exact host lookup.
        let tmp = TempDir::new().unwrap();
        let _home = scoped_home(&tmp);
        let skills_dir = tmp.path().join("skills");
        write_skill(&skills_dir, "skill-creator");
        let mut app = skill_test_app(&tmp, &skills_dir);
        {
            let mut bundle = app.command_contexts();
            let group = bundle
                .parts()
                .skill_group
                .expect("skill_group facet must be present");
            let outcome = group.activate_skill("skill-creator").unwrap();
            assert_eq!(outcome.name, "skill-creator");
        }
        assert!(app.active_skill.is_some());
    }

    #[test]
    fn skill_group_activation_not_found_lists_available() {
        let tmp = TempDir::new().unwrap();
        let _home = scoped_home(&tmp);
        let skills_dir = tmp.path().join("skills");
        write_skill(&skills_dir, "demo");
        let mut app = skill_test_app(&tmp, &skills_dir);
        {
            let mut bundle = app.command_contexts();
            let group = bundle
                .parts()
                .skill_group
                .expect("skill_group facet must be present");
            let err = group.activate_skill("missing").unwrap_err();
            match err {
                SkillActivationError::NotFound {
                    requested,
                    available,
                    ..
                } => {
                    assert_eq!(requested, "missing");
                    assert!(available.contains(&"demo".to_string()));
                }
                _ => panic!("expected NotFound"),
            }
        }
        assert!(app.active_skill.is_none());
    }

    #[test]
    fn skill_group_install_invalid_source_returns_safe_error() {
        let tmp = TempDir::new().unwrap();
        let _home = scoped_home(&tmp);
        let skills_dir = tmp.path().join("skills");
        std::fs::create_dir_all(&skills_dir).unwrap();
        let mut app = skill_test_app(&tmp, &skills_dir);
        {
            let mut bundle = app.command_contexts();
            let group = bundle
                .parts()
                .skill_group
                .expect("skill_group facet must be present");
            let err = group.install_skill(None, "   ").unwrap_err();
            assert!(err.contains("Invalid install source"), "{err}");
        }
    }

    #[test]
    fn skill_group_review_ready_sets_side_effects() {
        let tmp = TempDir::new().unwrap();
        let _home = scoped_home(&tmp);
        let skills_dir = tmp.path().join("skills");
        write_skill(&skills_dir, "review");
        let mut app = skill_test_app(&tmp, &skills_dir);
        {
            let mut bundle = app.command_contexts();
            let group = bundle
                .parts()
                .skill_group
                .expect("skill_group facet must be present");
            let outcome = group.run_review().unwrap();
            assert_eq!(outcome, ReviewOutcome::Ready);
        }
        assert!(app.active_skill.is_some());
        assert!(app.active_skill_provenance.is_none());
        assert!(!app.history.is_empty());
    }

    #[test]
    fn skill_group_review_not_found_reports_searched_dirs() {
        let tmp = TempDir::new().unwrap();
        let _home = scoped_home(&tmp);
        let skills_dir = tmp.path().join("skills");
        std::fs::create_dir_all(&skills_dir).unwrap();
        let mut app = skill_test_app(&tmp, &skills_dir);
        {
            let mut bundle = app.command_contexts();
            let group = bundle
                .parts()
                .skill_group
                .expect("skill_group facet must be present");
            let outcome = group.run_review().unwrap();
            match outcome {
                ReviewOutcome::NotFound {
                    skills_dir: found_dir,
                    global_dir,
                    warnings,
                } => {
                    assert_eq!(found_dir, skills_dir.display().to_string());
                    assert_eq!(
                        global_dir,
                        crate::skills::default_skills_dir().display().to_string()
                    );
                    assert!(warnings.is_empty());
                }
                _ => panic!("expected NotFound"),
            }
        }
        assert!(app.active_skill.is_none());
    }

    #[test]
    fn skill_group_snapshot_list_and_restore_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let _home = scoped_home(&tmp);
        let skills_dir = tmp.path().join("skills");
        let file = tmp.path().join("a.txt");
        let repo = crate::snapshot::SnapshotRepo::open_or_init(tmp.path()).unwrap();
        std::fs::write(&file, b"v1").unwrap();
        repo.snapshot("pre-turn:1").unwrap();
        std::fs::write(&file, b"v2").unwrap();
        let mut app = skill_test_app(&tmp, &skills_dir);
        {
            let mut bundle = app.command_contexts();
            let group = bundle
                .parts()
                .skill_group
                .expect("skill_group facet must be present");
            let entries = group.snapshot_list(20).unwrap();
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].label, "pre-turn:1");
            assert!(!entries[0].id.is_empty());
            group.restore_snapshot(&entries[0].id).unwrap();
        }
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "v1");
    }

    #[test]
    fn skill_group_approval_state_reflects_app_posture() {
        let tmp = TempDir::new().unwrap();
        let _home = scoped_home(&tmp);
        let skills_dir = tmp.path().join("skills");
        let mut app = skill_test_app(&tmp, &skills_dir);
        app.yolo = true;
        app.trust_mode = false;
        {
            let mut bundle = app.command_contexts();
            let group = bundle
                .parts()
                .skill_group
                .expect("skill_group facet must be present");
            let state = group.approval_state();
            assert!(state.yolo);
            assert!(!state.trust_mode);
        }
        app.yolo = false;
        app.trust_mode = true;
        {
            let mut bundle = app.command_contexts();
            let group = bundle
                .parts()
                .skill_group
                .expect("skill_group facet must be present");
            let state = group.approval_state();
            assert!(!state.yolo);
            assert!(state.trust_mode);
        }
    }

    #[test]
    fn portable_scope_maps_both_scopes_and_none() {
        use crate::skills::mutation::SkillTargetScope as TuiScope;
        assert_eq!(
            portable_scope(Some(SkillTargetScope::Project)),
            Some(TuiScope::Project)
        );
        assert_eq!(
            portable_scope(Some(SkillTargetScope::Global)),
            Some(TuiScope::Global)
        );
        assert_eq!(portable_scope(None), None);
    }

    #[test]
    fn portable_mutation_receipt_maps_distinct_outcomes() {
        use crate::skills::audit::SkillActionKind;
        use crate::skills::mutation::{
            SkillMutationOutcome as TuiOutcome, SkillMutationReceipt as TuiReceipt,
        };
        use crate::skills::roots::SkillScope;
        let make = |outcome: TuiOutcome| TuiReceipt {
            action: SkillActionKind::Install,
            name: "demo".to_string(),
            scope: SkillScope::Global,
            safe_target_path: "/tmp/demo".to_string(),
            before_digest: None,
            after_digest: None,
            outcome,
        };
        let installed = portable_mutation_receipt(&make(TuiOutcome::Installed));
        assert_eq!(installed.outcome, SkillMutationOutcome::Installed);
        assert_eq!(installed.name, "demo");
        assert_eq!(installed.safe_target_path, "/tmp/demo");

        let approval =
            portable_mutation_receipt(&make(TuiOutcome::NeedsApproval("acme.com".to_string())));
        assert_eq!(
            approval.outcome,
            SkillMutationOutcome::NeedsApproval("acme.com".to_string())
        );

        let denied =
            portable_mutation_receipt(&make(TuiOutcome::NetworkDenied("acme.com".to_string())));
        assert_eq!(
            denied.outcome,
            SkillMutationOutcome::NetworkDenied("acme.com".to_string())
        );
        assert_ne!(installed.outcome, denied.outcome);
    }

    #[test]
    fn skill_group_adapter_exposure_matches_main_envelope_model() {
        // The envelope populates the skill_group slot alongside the other
        // adapters; handlers destructure only their declared facets (D4).
        let mut app = test_app();
        let mut bundle = app.command_contexts();
        let parts = bundle.parts();
        assert!(parts.skill_group.is_some());
        assert!(parts.project.is_some());
        assert!(parts.skills.is_some());
    }

    // ------------------------------------------------------------------
    // FEAT-020 plugin adapter tests
    // ------------------------------------------------------------------

    fn plugin_test_app(tmpdir: &TempDir) -> App {
        let options = crate::test_support::test_tui_options(tmpdir.path());
        let mut app = crate::test_support::test_app_with_options(options);
        app.ui_locale = Locale::En;
        app
    }

    /// Write a minimal plugin bundle into the temp workspace's
    /// `.codewhale/plugins` so the adapter can read real host data.
    fn write_demo_bundle(root: &Path) {
        let bundle = root.join(".codewhale/plugins/demo");
        std::fs::create_dir_all(bundle.join("skills/hello")).unwrap();
        std::fs::write(
            bundle.join("plugin.toml"),
            "schema_version = 1\n[plugin]\nname = \"demo\"\nversion = \"1.0.0\"\ndescription = \"Import spreadsheet data safely\"\n[skills]\npath = \"skills\"\n",
        )
        .unwrap();
        std::fs::write(
            bundle.join("skills/hello/SKILL.md"),
            "---\nname: hello\ndescription: hello\n---\nbody\n",
        )
        .unwrap();
    }

    #[test]
    fn plugin_adapter_summaries_and_detail_project_host_data() {
        let tmp = TempDir::new().unwrap();
        write_demo_bundle(tmp.path());
        let mut app = plugin_test_app(&tmp);
        // Discover only the demo bundle: the host's real `~/.codewhale/plugins`
        // and materialized builtin plugins must not leak diagnostics into this
        // assertion (they did on a shared CI agent).
        let plugin_config = crate::plugins::discovery::DiscoveryConfig {
            workspace: tmp.path().to_path_buf(),
            user_plugins_dir: tmp.path().join("user-plugins"),
            workspace_plugins_dir: tmp.path().join(".codewhale/plugins"),
            builtin_plugin_dirs: Vec::new(),
            state_path: tmp.path().join("user-plugins/state.json"),
        };
        let discovery = crate::plugins::PluginDiscoveryContext::from_config_and_environment(
            &plugin_config,
            crate::plugins::HostEnvironment::capture(),
        );
        app.plugin_registry = discovery.registry_for_workspace(tmp.path());
        let mut bundle = app.command_contexts();
        let mut parts = bundle
            .contexts(
                CommandCapabilities::WORKSPACE
                    .union(CommandCapabilities::PRESENTATION)
                    .union(CommandCapabilities::PLUGIN),
            )
            .into_parts();
        let plugin = parts.plugin.as_deref_mut().unwrap();

        let summaries = plugin.summaries().unwrap();
        assert!(!summaries.is_empty());
        let summary = summaries
            .iter()
            .find(|s| s.name == "demo")
            .expect("demo summary");
        assert_eq!(summary.compatibility, "full");
        assert!(
            summary.inventory.starts_with("skills=1"),
            "inventory summary: {}",
            summary.inventory
        );

        let detail = plugin.detail("demo").unwrap();
        assert_eq!(detail.name, "demo");
        assert_eq!(detail.version, "1.0.0");
        assert_eq!(detail.skills, vec!["demo:hello"]);
        assert_eq!(detail.trust_status, "not-reviewed");

        // Unknown selector fails safely.
        assert!(plugin.detail("nope").is_err());
        // Registry diagnostics empty for a clean bundle.
        assert!(plugin.registry_diagnostics().is_empty());
        assert!(plugin.validation_is_clean());
    }

    #[test]
    fn plugin_adapter_registry_mutations_and_suggest_are_behavior_faithful() {
        let tmp = TempDir::new().unwrap();
        write_demo_bundle(tmp.path());
        let mut app = plugin_test_app(&tmp);
        // Discover only the demo bundle: the host's real `~/.codewhale/plugins`
        // and materialized builtin plugins must not leak diagnostics into this
        // assertion (they did on a shared CI agent).
        let plugin_config = crate::plugins::discovery::DiscoveryConfig {
            workspace: tmp.path().to_path_buf(),
            user_plugins_dir: tmp.path().join("user-plugins"),
            workspace_plugins_dir: tmp.path().join(".codewhale/plugins"),
            builtin_plugin_dirs: Vec::new(),
            state_path: tmp.path().join("user-plugins/state.json"),
        };
        let discovery = crate::plugins::PluginDiscoveryContext::from_config_and_environment(
            &plugin_config,
            crate::plugins::HostEnvironment::capture(),
        );
        app.plugin_registry = discovery.registry_for_workspace(tmp.path());
        // Capture the review token before borrowing the mutable facet.
        let demo = app.plugin_registry.get("demo").unwrap();
        let token = format!("{}.{}", demo.content_hash, demo.capability_hash);

        let mut bundle = app.command_contexts();
        let mut parts = bundle.contexts(CommandCapabilities::PLUGIN).into_parts();
        let plugin = parts.plugin.as_deref_mut().unwrap();

        // Read-only suggest does not mutate anything.
        let before = plugin.len();
        let _ = plugin.suggest("spreadsheet");
        assert_eq!(plugin.len(), before);
        assert_eq!(plugin.summaries().unwrap().len(), before);

        // enable on an untrusted bundle routes to review (safe error), not a mutation.
        let err = plugin.enable("demo").unwrap_err();
        assert!(err.contains("requires review"));

        // trust with a wrong token fails safely.
        assert!(plugin.trust("demo", "bogus.token").is_err());

        // trust with the exact token succeeds.
        plugin.trust("demo", &token).unwrap();
        assert!(plugin.detail("demo").unwrap().trusted);

        // enable now succeeds.
        plugin.enable("demo").unwrap();
        assert!(plugin.detail("demo").unwrap().enabled);

        // disable clears active skill and marks disabled.
        plugin.disable("demo").unwrap();
        assert!(!plugin.detail("demo").unwrap().enabled);

        // revoke_trust flips trust back off.
        plugin.revoke_trust("demo").unwrap();
        assert!(!plugin.detail("demo").unwrap().trusted);
    }

    #[test]
    fn plugin_adapter_exposure_is_exactly_declared_capabilities() {
        let tmp = TempDir::new().unwrap();
        let mut app = plugin_test_app(&tmp);
        let mut bundle = app.command_contexts();

        // Plugin-only: plugin present, everything else absent.
        let parts = bundle.contexts(CommandCapabilities::PLUGIN).into_parts();
        assert!(parts.plugin.is_some());
        assert!(parts.workspace.is_none());
        assert!(parts.presentation.is_none());
        assert!(parts.memory.is_none());

        // Workspace | PRESENTATION | PLUGIN: all three present, media/memory absent.
        let parts = bundle
            .contexts(
                CommandCapabilities::WORKSPACE
                    .union(CommandCapabilities::PRESENTATION)
                    .union(CommandCapabilities::PLUGIN),
            )
            .into_parts();
        assert!(parts.plugin.is_some());
        assert!(parts.workspace.is_some());
        assert!(parts.presentation.is_some());
        assert!(parts.media.is_none());
        assert!(parts.memory.is_none());

        // Undeclared capability: plugin absent.
        let parts = bundle.contexts(CommandCapabilities::SESSION).into_parts();
        assert!(parts.session.is_some());
        assert!(parts.plugin.is_none());
    }

    // ---------------------------------------------------------------------------
    // FEAT-023 Phase 3: SessionLifecycleAdapter tests (Tasks 3.2/3.4).
    // Every delegate is exercised over the real App with an isolated CODEWHALE_HOME
    // so SessionManager writes stay inside the temp directory. The bundle borrows
    // `App` for its whole life, so each test scopes the facet and re-reads `App`
    // only after dropping it (adapters borrow through the host `RefCell` at call
    // time, but the bundle itself holds the `&mut App`).
    // ---------------------------------------------------------------------------

    fn lifecycle_test_app(tmpdir: &TempDir) -> App {
        let options = crate::test_support::test_tui_options(tmpdir.path());
        App::new(options, &crate::config::Config::default())
    }

    /// Point CODEWHALE_HOME at `tmp/home` with a pre-created sessions directory so
    /// `SessionManager::default_location()` resolves inside the temp sandbox.
    fn lifecycle_home_guard(tmpdir: &TempDir) -> crate::test_support::EnvVarGuard {
        let home = tmpdir.path().join("home");
        let sessions = home.join("sessions");
        std::fs::create_dir_all(&sessions).expect("create sandbox sessions dir");
        crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", &home)
    }

    fn user_message(text: &str) -> Message {
        Message {
            role: codewhale_models::Role::User,
            content: vec![codewhale_models::ContentBlock::Text {
                text: text.to_string(),
                cache_control: None,
            }],
        }
    }

    #[test]
    fn lifecycle_dispatch_transition_blocking_wins_over_io() {
        let tmpdir = TempDir::new().unwrap();
        let _lock = crate::test_support::lock_test_env();
        let mut app = lifecycle_test_app(&tmpdir);
        app.is_loading = true;
        app.current_session_id = Some("active-session".to_string());
        app.api_messages.push(user_message("in flight"));

        for (command, expected) in [
            ("/fork", "Cannot fork a session"),
            ("/fork other-session", "Cannot fork a session"),
            ("/load does-not-exist.json", "Cannot load a session"),
            ("/new", "Cannot start a new session"),
            ("/branch entry-1", "Cannot branch"),
        ] {
            let result = crate::commands::execute(command, &mut app);
            assert!(result.is_error, "{command}: {result:?}");
            assert!(result.action.is_none(), "{command}: {result:?}");
            assert!(
                result
                    .message
                    .as_deref()
                    .is_some_and(|text| text.contains(expected)),
                "{command}: {result:?}"
            );
            assert_eq!(app.current_session_id.as_deref(), Some("active-session"));
            assert_eq!(app.api_messages.len(), 1);
        }
    }

    #[test]
    fn lifecycle_adapter_save_and_fork_roundtrip_preserves_history() {
        let tmpdir = TempDir::new().unwrap();
        let _lock = crate::test_support::lock_test_env();
        let _home = lifecycle_home_guard(&tmpdir);
        let mut app = lifecycle_test_app(&tmpdir);
        app.api_messages.push(user_message("try another path"));

        let save_path = tmpdir.path().join("parent.json");
        {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.lifecycle.as_deref_mut().expect("lifecycle slot");
            let saved = facet
                .save_session(Some(save_path.display().to_string()))
                .expect("save ok");
            assert!(save_path.exists());
            assert!(!saved.display_path.is_empty());
            assert!(!saved.truncated_id.is_empty());
        }
        let parent_id = app
            .current_session_id
            .clone()
            .expect("save sets session id");
        {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.lifecycle.as_deref_mut().expect("lifecycle slot");
            let forked = facet.fork_active().expect("fork ok");
            assert!(!forked.parent_label.is_empty());
            assert!(!forked.fork_label.is_empty());
            assert!(forked.sync.session_id.is_some());
            assert_eq!(forked.sync.messages.len(), 1);
            assert_eq!(forked.sync.workspace, tmpdir.path());
        }
        let child_id = app
            .current_session_id
            .clone()
            .expect("fork switches session");
        assert_ne!(child_id, parent_id);

        let manager = crate::session_manager::SessionManager::default_location().unwrap();
        let parent = manager.load_session(&parent_id).expect("parent loadable");
        let child = manager.load_session(&child_id).expect("child loadable");
        assert_eq!(parent.messages.len(), 1, "parent history preserved");
        assert_eq!(
            child.metadata.parent_session_id.as_deref(),
            Some(parent_id.as_str())
        );
        assert_eq!(child.metadata.forked_from_message_count, Some(1));
    }

    #[test]
    fn lifecycle_adapter_explicit_fork_reports_spawn_depth_and_preserves_source() {
        let tmpdir = TempDir::new().unwrap();
        let _lock = crate::test_support::lock_test_env();
        let _home = lifecycle_home_guard(&tmpdir);
        let mut app = lifecycle_test_app(&tmpdir);
        app.api_messages.push(user_message("parent turn"));
        {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.lifecycle.as_deref_mut().expect("lifecycle slot");
            let saved = facet.save_session(None).expect("save into managed dir");
            assert!(!saved.truncated_id.is_empty());
        }
        let parent_id = app
            .current_session_id
            .clone()
            .expect("save sets session id");
        let source_len = {
            let manager = crate::session_manager::SessionManager::default_location().unwrap();
            manager
                .load_session(&parent_id)
                .expect("saved parent")
                .messages
                .len()
        };
        {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.lifecycle.as_deref_mut().expect("lifecycle slot");
            let forked = facet.fork_from(&parent_id).expect("explicit fork ok");
            assert_eq!(forked.spawn_depth, 1);
            assert_eq!(
                forked.parent_label,
                crate::session_manager::truncate_id(&parent_id)
            );
            assert_eq!(forked.sync.messages.len(), 1);
        }
        let manager = crate::session_manager::SessionManager::default_location().unwrap();
        let reloaded = manager
            .load_session(&parent_id)
            .expect("source still loadable");
        assert_eq!(
            reloaded.messages.len(),
            source_len,
            "source history never rewritten by forking"
        );
    }

    #[test]
    fn lifecycle_adapter_new_session_is_all_or_nothing_when_work_state_is_busy() {
        let tmpdir = TempDir::new().unwrap();
        let _lock = crate::test_support::lock_test_env();
        let _home = lifecycle_home_guard(&tmpdir);
        let mut app = lifecycle_test_app(&tmpdir);
        app.current_session_id = Some("current-session".to_string());
        app.api_messages.push(user_message("work"));
        let todos = app.todos.clone();
        let _held = todos.try_lock().expect("hold todos lock");

        {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.lifecycle.as_deref_mut().expect("lifecycle slot");
            let err = facet.fresh_session(true).expect_err("busy work state");
            assert!(err.contains("Work state is busy"), "{err}");
        }
        assert_eq!(app.api_messages.len(), 1);
        assert_eq!(app.current_session_id.as_deref(), Some("current-session"));
    }

    #[test]
    fn lifecycle_adapter_new_session_blocks_unsent_input_without_force() {
        let tmpdir = TempDir::new().unwrap();
        let _lock = crate::test_support::lock_test_env();
        let _home = lifecycle_home_guard(&tmpdir);
        let mut app = lifecycle_test_app(&tmpdir);
        app.current_session_id = Some("old-session".to_string());
        app.input = "draft text".to_string();
        {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.lifecycle.as_deref_mut().expect("lifecycle slot");
            let err = facet.fresh_session(false).expect_err("blocker text");
            assert!(err.contains("/new --force"), "{err}");
        }
        assert_eq!(app.input, "draft text");
        assert_eq!(app.current_session_id.as_deref(), Some("old-session"));

        {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.lifecycle.as_deref_mut().expect("lifecycle slot");
            let ok = facet.fresh_session(true).expect("force discards draft");
            assert_ne!(app.current_session_id.as_deref(), Some("old-session"));
            assert!(app.input.is_empty());
            assert!(!ok.truncated_id.is_empty());
            assert!(ok.sync.messages.is_empty());
        }
    }

    #[test]
    fn lifecycle_adapter_load_validates_shape_without_applying_state() {
        let tmpdir = TempDir::new().unwrap();
        let _lock = crate::test_support::lock_test_env();
        let _home = lifecycle_home_guard(&tmpdir);
        let mut app = lifecycle_test_app(&tmpdir);
        app.api_messages.push(user_message("checkpoint"));
        let save_path = tmpdir.path().join("checkpoint.json");
        {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.lifecycle.as_deref_mut().expect("lifecycle slot");
            facet
                .save_session(Some(save_path.display().to_string()))
                .expect("seed session file");
        }
        let before = app.api_messages.clone();
        {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.lifecycle.as_deref_mut().expect("lifecycle slot");
            let missing = facet
                .load_session("does-not-exist.json")
                .expect_err("missing file");
            assert!(missing.contains("Failed to read session file"), "{missing}");
            let bad = tmpdir.path().join("bad.json");
            std::fs::write(&bad, "not json").unwrap();
            let parse = facet
                .load_session(bad.display().to_string().as_str())
                .expect_err("invalid json");
            assert!(parse.contains("Failed to parse session file"), "{parse}");
            let resolved = facet
                .load_session(save_path.display().to_string().as_str())
                .expect("valid session resolves");
            assert_eq!(resolved, save_path);
        }
        assert_eq!(
            app.api_messages, before,
            "no state applied by /load delegate"
        );
    }

    #[test]
    fn lifecycle_adapter_picker_archive_and_prune_behavior() {
        let tmpdir = TempDir::new().unwrap();
        let _lock = crate::test_support::lock_test_env();
        let _home = lifecycle_home_guard(&tmpdir);
        let mut app = lifecycle_test_app(&tmpdir);
        let before_kind = app.view_stack.top_kind();
        {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.lifecycle.as_deref_mut().expect("lifecycle slot");
            facet.open_picker(None);
            facet.open_picker(Some("pick-me".to_string()));
        }
        {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.lifecycle.as_deref_mut().expect("lifecycle slot");
            facet.save_session(None).expect("seed archive target");
        }
        let archived_id = app.current_session_id.clone().unwrap();
        {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.lifecycle.as_deref_mut().expect("lifecycle slot");
            let receipt = facet.set_archived(&archived_id, true).expect("archive ok");
            assert_eq!(
                receipt.truncated_id,
                crate::session_manager::truncate_id(&archived_id)
            );
            assert!(!receipt.title.is_empty());
            let restored = facet.set_archived(&archived_id, false).expect("restore ok");
            assert_eq!(restored.truncated_id, receipt.truncated_id);
            let pruned = facet.prune_sessions(36500).expect("prune runs");
            assert_eq!(pruned, 0, "no inactive session older than the window");
        }
        assert_ne!(
            app.view_stack.top_kind(),
            before_kind,
            "picker pushed a view"
        );
        let manager = crate::session_manager::SessionManager::default_location().unwrap();
        assert!(
            manager.load_session(&archived_id).is_ok(),
            "active session survives pruning"
        );
    }

    #[test]
    fn lifecycle_adapter_tree_projections_cover_all_states() {
        let tmpdir = TempDir::new().unwrap();
        let _lock = crate::test_support::lock_test_env();
        let _home = lifecycle_home_guard(&tmpdir);

        // No active session.
        let mut no_session_app = lifecycle_test_app(&tmpdir);
        {
            let mut bundle = no_session_app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.lifecycle.as_deref_mut().expect("lifecycle slot");
            assert!(matches!(
                facet.tree_body().expect("tree ok"),
                TreeBodyProjection::NoSession
            ));
        }

        // Active session with no messages and no saved journal.
        let mut empty_app = lifecycle_test_app(&tmpdir);
        empty_app.current_session_id = Some("empty-session".to_string());
        {
            let mut bundle = empty_app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.lifecycle.as_deref_mut().expect("lifecycle slot");
            assert!(matches!(
                facet.tree_body().expect("tree ok"),
                TreeBodyProjection::EmptySession
            ));
        }

        // Linear transcript before the journal exists.
        let mut linear_app = lifecycle_test_app(&tmpdir);
        linear_app.current_session_id = Some("linear-session".to_string());
        linear_app
            .api_messages
            .push(user_message("first message with a long tail"));
        {
            let mut bundle = linear_app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.lifecycle.as_deref_mut().expect("lifecycle slot");
            match facet.tree_body().expect("tree ok") {
                TreeBodyProjection::Linear { rendered } => {
                    assert!(rendered.contains("Active branch (linear"), "{rendered}");
                    assert!(rendered.contains("[0]"), "{rendered}");
                }
                other => panic!("expected Linear projection, got {other:?}"),
            }
        }

        // Journal projection once the session is saved with messages.
        let mut journal_app = lifecycle_test_app(&tmpdir);
        journal_app
            .api_messages
            .push(user_message("journaled turn"));
        {
            let mut bundle = journal_app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.lifecycle.as_deref_mut().expect("lifecycle slot");
            facet.save_session(None).expect("seed journaled session");
            match facet.tree_body().expect("tree ok") {
                TreeBodyProjection::Journal { rendered } => {
                    assert!(!rendered.is_empty());
                }
                other => panic!("expected Journal projection, got {other:?}"),
            }
        }
    }

    // -------------------------------------------------------------------
    // FEAT-024 Phase 3: SessionControlAdapter tests (Tasks 3.2/3.4/3.6).
    // The bundle borrows `App` for its whole life, so each test scopes the
    // facet (via a bound `parts` value) and re-reads `App` only after
    // dropping it.
    // -------------------------------------------------------------------

    fn control_test_app(tmpdir: &TempDir) -> App {
        lifecycle_test_app(tmpdir)
    }

    fn control_home_guard(tmpdir: &TempDir) -> crate::test_support::EnvVarGuard {
        lifecycle_home_guard(tmpdir)
    }

    fn save_control_session(tmpdir: &TempDir, id: &str) {
        let manager = crate::session_manager::SessionManager::default_location().unwrap();
        let mut session = crate::session_manager::create_saved_session_with_mode(
            &[],
            "deepseek-v4-pro",
            tmpdir.path(),
            0,
            None,
            None,
        );
        session.metadata.id = id.to_string();
        session.metadata.title = "Control Session".to_string();
        manager.save_session(&session).unwrap();
    }

    #[test]
    fn control_relay_projection_maps_authoritative_snapshot() {
        let tmpdir = TempDir::new().unwrap();
        let _lock = crate::test_support::lock_test_env();
        let mut app = control_test_app(&tmpdir);
        app.goal.objective = Some("ship the control slice".to_string());
        app.goal.token_budget = Some(42_000);
        let expected_workspace = app.workspace.display().to_string();
        let expected_mode = app.mode.label().to_string();
        let expected_model = app.model_display_label();

        {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.control.as_deref_mut().expect("control slot");
            let projection = facet.relay_projection();
            assert_eq!(projection.workspace, expected_workspace);
            assert_eq!(projection.mode, expected_mode);
            assert_eq!(projection.model, expected_model);
            assert_eq!(
                projection.goal_objective.as_deref(),
                Some("ship the control slice")
            );
            assert_eq!(projection.goal_token_budget, Some(42_000));
            assert_eq!(
                projection.compact_template.trim(),
                crate::prompts::COMPACT_TEMPLATE.trim()
            );
            assert!(matches!(projection.todos, TodoProjection::Absent));
            assert!(matches!(projection.plan, PlanProjection::Absent));
        }

        // Plan state held by another owner -> busy state is represented,
        // never a panic or lock wait.
        {
            let plan_state = app.plan_state.clone();
            let guard = plan_state.try_lock().unwrap();
            {
                let mut bundle = app.command_contexts();
                let mut parts = bundle.parts();
                let facet = parts.control.as_deref_mut().expect("control slot");
                assert!(matches!(
                    facet.relay_projection().plan,
                    PlanProjection::Busy
                ));
            }
            drop(guard);
        }

        // Seeded plan sections transport the status label mapping.
        {
            let plan_state = app.plan_state.clone();
            let mut plan = plan_state.try_lock().unwrap();
            plan.update(crate::tools::plan::UpdatePlanArgs {
                title: Some("Relay Plan".to_string()),
                plan: vec![crate::tools::plan::PlanItemArg {
                    step: "port the control slice".to_string(),
                    status: crate::tools::plan::StepStatus::InProgress,
                }],
                ..crate::tools::plan::UpdatePlanArgs::default()
            });
        }
        {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.control.as_deref_mut().expect("control slot");
            match facet.relay_projection().plan {
                PlanProjection::Sections(sections) => {
                    assert_eq!(sections.title.as_deref(), Some("Relay Plan"));
                    assert_eq!(sections.items.len(), 1);
                    assert_eq!(sections.items[0].status, PlanStepStatus::InProgress);
                    assert_eq!(sections.items[0].text, "port the control slice");
                }
                other => panic!("expected Sections plan after update, got {other:?}"),
            }
        }
    }

    #[test]
    fn control_hosted_work_target_resolves_and_never_echoes_credentials() {
        let tmpdir = TempDir::new().unwrap();
        let _lock = crate::test_support::lock_test_env();
        let secret = "top-secret-token";
        let mut app = control_test_app(&tmpdir);
        init_control_git_repo(
            tmpdir.path(),
            &format!("https://hunter:{secret}@github.com/Hmbown/CodeWhale.git"),
            "main",
        );

        {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.control.as_deref_mut().expect("control slot");
            let target = facet.resolve_hosted_work_target().expect("target");
            assert_eq!(target.repo, "Hmbown/CodeWhale");
            assert_eq!(target.branch, "main");
            assert_eq!(
                target.url,
                "https://app.codewhale.net/work?repo=Hmbown%2FCodeWhale&branch=main"
            );
            assert!(!target.url.contains(secret));
            assert!(!target.repo.contains(secret));
        }

        // Unsupported host resolves to None.
        init_control_git_repo(tmpdir.path(), "git@gitlab.com:acme/widgets.git", "main");
        {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.control.as_deref_mut().expect("control slot");
            assert_eq!(facet.resolve_hosted_work_target(), None);
        }
    }

    fn init_control_git_repo(dir: &Path, origin: &str, branch: &str) {
        let init = std::process::Command::new("git")
            .args(["init", "--quiet"])
            .arg(dir)
            .status()
            .expect("run git init");
        assert!(init.success());
        let set_origin = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["config", "--local", "remote.origin.url", origin])
            .status()
            .expect("set origin");
        assert!(set_origin.success());
        let set_branch = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["symbolic-ref", "HEAD"])
            .arg(format!("refs/heads/{branch}"))
            .status()
            .expect("set branch");
        assert!(set_branch.success());
    }

    #[test]
    fn control_rename_session_persists_title_and_preserves_order() {
        let tmpdir = TempDir::new().unwrap();
        let _lock = crate::test_support::lock_test_env();
        let _home = control_home_guard(&tmpdir);
        save_control_session(&tmpdir, "rename-1");
        let mut app = control_test_app(&tmpdir);
        app.current_session_id = Some("rename-1".to_string());

        let receipt = {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.control.as_deref_mut().expect("control slot");
            facet.rename_session("Brand New Title").expect("rename ok")
        };
        assert_eq!(receipt.title, "Brand New Title");
        assert_eq!(app.session_title.as_deref(), Some("Brand New Title"));
        let manager = crate::session_manager::SessionManager::default_location().unwrap();
        let reloaded = manager.load_session("rename-1").unwrap();
        assert_eq!(reloaded.metadata.title, "Brand New Title");

        // The facet exposes the authoritative sanitizer while the portable
        // handler owns empty and length policy.
        let sanitized = {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.control.as_deref_mut().expect("control slot");
            facet.sanitize_session_title("\u{1b}\u{7}\u{200b}")
        };
        assert!(sanitized.is_empty());
        assert_eq!(app.window_title, None);
    }

    #[test]
    fn control_rename_recovers_first_snapshot_from_checkpoint() {
        let tmpdir = TempDir::new().unwrap();
        let _lock = crate::test_support::lock_test_env();
        let _home = control_home_guard(&tmpdir);
        let manager = crate::session_manager::SessionManager::default_location().unwrap();
        let mut checkpoint = crate::session_manager::create_saved_session_with_mode(
            &[],
            "deepseek-v4-pro",
            tmpdir.path(),
            0,
            None,
            None,
        );
        checkpoint.metadata.id = "midturn-1".to_string();
        manager.save_checkpoint(&checkpoint).unwrap();

        let mut app = control_test_app(&tmpdir);
        app.current_session_id = Some("midturn-1".to_string());
        app.api_messages = vec![user_message("first turn still streaming")];

        let receipt = {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.control.as_deref_mut().expect("control slot");
            facet.rename_session("Midturn Rename").expect("rename ok")
        };
        assert_eq!(receipt.title, "Midturn Rename");
        assert_eq!(app.session_title.as_deref(), Some("Midturn Rename"));
        let persisted = manager.load_session("midturn-1").unwrap();
        assert_eq!(persisted.metadata.title, "Midturn Rename");
        assert_eq!(persisted.messages.len(), 1);
    }

    #[test]
    fn control_rename_errors_match_the_baseline() {
        let tmpdir = TempDir::new().unwrap();
        let _lock = crate::test_support::lock_test_env();
        let _home = control_home_guard(&tmpdir);
        let mut app = control_test_app(&tmpdir);
        app.current_session_id = None;
        let err = {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.control.as_deref_mut().expect("control slot");
            facet.rename_session("Anything").unwrap_err()
        };
        assert!(err.contains("No active session"));
    }

    #[test]
    fn control_title_report_set_and_clear_preserve_semantics() {
        let tmpdir = TempDir::new().unwrap();
        let _lock = crate::test_support::lock_test_env();
        let _home = control_home_guard(&tmpdir);
        save_control_session(&tmpdir, "title-1");
        let mut app = control_test_app(&tmpdir);
        app.current_session_id = Some("title-1".to_string());

        // No session window title and no config default -> unset, no source.
        let report = {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.control.as_deref_mut().expect("control slot");
            facet.title_report()
        };
        assert_eq!(report.effective, "unset");
        assert!(matches!(report.source, TitleSource::None));

        // Set a window title: session name untouched, redraw requested.
        {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.control.as_deref_mut().expect("control slot");
            facet
                .set_window_title("parallel-task".to_string())
                .expect("set ok");
        }
        assert_eq!(app.window_title.as_deref(), Some("parallel-task"));
        assert!(app.needs_redraw);
        assert_eq!(
            app.session_title, None,
            "/title never changes the session name"
        );
        let manager = crate::session_manager::SessionManager::default_location().unwrap();
        let reloaded = manager.load_session("title-1").unwrap();
        assert_eq!(reloaded.window_title.as_deref(), Some("parallel-task"));
        assert_eq!(reloaded.metadata.title, "Control Session");

        // Control-char-only input is normalized to empty before the portable
        // handler applies its exact user-facing validation message.
        let sanitized = {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.control.as_deref_mut().expect("control slot");
            facet.sanitize_session_title("\u{1b}\u{7}\u{200b}")
        };
        assert!(sanitized.is_empty());

        // Clear removes the session-level title.
        {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.control.as_deref_mut().expect("control slot");
            facet.clear_window_title().expect("clear ok");
        }
        assert_eq!(app.window_title, None);
    }

    #[test]
    fn control_resume_gate_picker_and_resolution_routes() {
        let tmpdir = TempDir::new().unwrap();
        let _lock = crate::test_support::lock_test_env();
        let _home = control_home_guard(&tmpdir);
        save_control_session(&tmpdir, "resume-target-1");
        let mut app = control_test_app(&tmpdir);

        // Transition gate mirrors the host state.
        app.is_loading = true;
        {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.control.as_deref_mut().expect("control slot");
            assert!(facet.transition_blocked());
        }
        app.is_loading = false;

        // Bare resume pushes the picker.
        assert!(app.view_stack.is_empty());
        {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.control.as_deref_mut().expect("control slot");
            facet.open_resume_picker();
        }
        assert!(!app.view_stack.is_empty());

        // Full id and prefix resolve to the durable session file.
        let by_id = {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.control.as_deref_mut().expect("control slot");
            facet
                .resolve_resume_source("resume-target-1")
                .expect("resolve ok")
        };
        match by_id {
            ResumeSource::Session {
                load_path,
                truncated_id,
                title,
            } => {
                assert!(load_path.as_ref().is_some_and(|p| p.exists()));
                assert!(!truncated_id.is_empty());
                assert_eq!(title, "Control Session");
            }
            other => panic!("expected Session resolution, got {other:?}"),
        }
        let by_prefix = {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.control.as_deref_mut().expect("control slot");
            facet
                .resolve_resume_source("resume-target")
                .expect("prefix ok")
        };
        assert!(matches!(by_prefix, ResumeSource::Session { .. }));

        // A readable file resolves as the direct-file route.
        let export_file = tmpdir.path().join("session-export.json");
        std::fs::write(&export_file, "{}").unwrap();
        let as_file = {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.control.as_deref_mut().expect("control slot");
            facet
                .resolve_resume_source(&export_file.display().to_string())
                .expect("file ok")
        };
        assert!(matches!(as_file, ResumeSource::File(_)));

        // Unknown input resolves to NotFound with the raw value for the
        // handler's exact fallback message.
        let missing = {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.control.as_deref_mut().expect("control slot");
            facet
                .resolve_resume_source("not-a-real-session-xyz")
                .expect("notfound ok")
        };
        match missing {
            ResumeSource::NotFound { raw, error } => {
                assert_eq!(raw, "not-a-real-session-xyz");
                assert!(!error.is_empty());
            }
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn control_resume_import_rejects_unrecognized_and_applies_foreign() {
        let tmpdir = TempDir::new().unwrap();
        let _lock = crate::test_support::lock_test_env();
        let _home = control_home_guard(&tmpdir);
        let mut app = control_test_app(&tmpdir);

        let bad_file = tmpdir.path().join("not-an-export.json");
        std::fs::write(&bad_file, "not session json at all").unwrap();
        let err = {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.control.as_deref_mut().expect("control slot");
            facet.import_session_file(bad_file).unwrap_err()
        };
        assert!(err.contains("is not a recognized session export"), "{err}");

        // A real export container round-trips through import and mutates the
        // active session atomically.
        let manager = crate::session_manager::SessionManager::default_location().unwrap();
        let mut source = crate::session_manager::create_saved_session_with_mode(
            &[],
            "deepseek-v4-pro",
            tmpdir.path(),
            0,
            None,
            None,
        );
        source.metadata.id = "foreign-source".to_string();
        source.metadata.title = "Foreign Name".to_string();
        let container = source.export_container("foreign");
        let json = serde_json::to_string(&container).expect("serialize container");
        let import_file = tmpdir.path().join("foreign-export.json");
        std::fs::write(&import_file, &json).unwrap();

        let receipt = {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.control.as_deref_mut().expect("control slot");
            facet.import_session_file(import_file).expect("import ok")
        };
        assert!(!receipt.truncated_id.is_empty());
        assert_eq!(receipt.entry_count, 0);
        assert_eq!(receipt.leaf_display, "(none)");
        let imported_id = app.current_session_id.clone().expect("active session");
        let saved = manager
            .load_session(&imported_id)
            .expect("import persisted");
        // Host import_foreign rebuilds the document with default metadata
        // (fresh id/title), matching the baseline import path exactly.
        assert_eq!(saved.metadata.title, "New Session");
        assert_ne!(saved.metadata.id, "foreign-source");
        assert!(
            manager
                .sessions_dir()
                .join(format!("{imported_id}.json"))
                .exists()
        );
    }

    #[test]
    fn control_remote_state_and_routing_are_deterministic() {
        let tmpdir = TempDir::new().unwrap();
        let _lock = crate::test_support::lock_test_env();
        let mut app = control_test_app(&tmpdir);

        // Off state: status line, no link, no browser open.
        {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.control.as_deref_mut().expect("control slot");
            assert_eq!(facet.remote_status(), "Remote control: off");
            assert_eq!(facet.remote_link(), None);
            assert!(matches!(
                facet.remote_browser_open(),
                RemoteOpenOutcome::NoLink
            ));
            assert_eq!(facet.remote_stop_refusal(), None);
        }

        // Start wording distinguishes the active-turn copy.
        app.is_loading = true;
        {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.control.as_deref_mut().expect("control slot");
            assert!(facet.remote_start_info().connecting);
        }
        app.is_loading = false;
        {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.control.as_deref_mut().expect("control slot");
            assert!(!facet.remote_start_info().connecting);
        }

        // A live advertised link composes without spawning a browser.
        app.remote_control.install_live_link_for_test(
            "https://app.codewhale.net/session?run=run-1",
            Some("https://app.codewhale.net/settings"),
        );
        let link = {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.control.as_deref_mut().expect("control slot");
            facet.remote_link().expect("live link")
        };
        assert_eq!(link.url, "https://app.codewhale.net/session?run=run-1");
        assert_eq!(
            link.computer_url.as_deref(),
            Some("https://app.codewhale.net/settings")
        );
    }

    #[test]
    fn control_remote_stop_refusal_guards_active_turns() {
        let tmpdir = TempDir::new().unwrap();
        let _lock = crate::test_support::lock_test_env();
        let mut app = control_test_app(&tmpdir);
        app.remote_control
            .activate_prompt("run-1", "turn-1")
            .unwrap();
        let refusal = {
            let mut bundle = app.command_contexts();
            let mut parts = bundle.parts();
            let facet = parts.control.as_deref_mut().expect("control slot");
            facet.remote_stop_refusal().expect("refusal present")
        };
        assert!(refusal.contains("active remote turn"), "{refusal}");
    }

    #[test]
    fn control_browser_open_outcome_mapping_is_exact() {
        let url = "https://app.codewhale.net/session?run=run-9".to_string();
        assert!(matches!(
            map_browser_open_result(url.clone(), true),
            RemoteOpenOutcome::Opened { url: u } if u == url
        ));
        assert!(matches!(
            map_browser_open_result(url.clone(), false),
            RemoteOpenOutcome::LaunchFailed { url: u } if u == url
        ));
    }
}
