use std::cell::RefCell;
use std::path::{Path, PathBuf};

use codewhale_core::request::{ContentBlock, Message, SystemPrompt};
use codewhale_core::role::Role;

use crate::*;

struct Session;
impl CommandSessionContext for Session {
    fn session_id(&self) -> Option<String> {
        Some("session".into())
    }
    fn api_messages(&self) -> Vec<Message> {
        vec![]
    }
    fn add_message(&mut self, _message: Message) {}
    fn queued_message_count(&self) -> usize {
        0
    }
    fn remove_queued_message(&mut self, _index: usize) -> Result<(), String> {
        Ok(())
    }
    fn total_tokens(&self) -> u64 {
        42
    }
}

struct Model;
impl CommandModelContext for Model {
    fn current_model(&self) -> String {
        "auto".into()
    }
    fn auto_model(&self) -> bool {
        true
    }
    fn set_model_selection(&mut self, _model: String, _provider: Option<CommandProviderId>) {}
    fn reasoning_effort(&self) -> CommandReasoningEffort {
        CommandReasoningEffort::Auto
    }
    fn provider_identity(&self) -> Option<CommandProviderId> {
        None
    }
    fn fallback_chain(&self) -> Vec<CommandProviderId> {
        vec![]
    }
}

struct Cost;
impl CommandCostContext for Cost {
    fn display_currency(&self) -> CommandCurrency {
        CommandCurrency::Usd
    }
    fn session_cost_for_currency(&self, _currency: CommandCurrency) -> f64 {
        1.0
    }
    fn subagent_cost_for_currency(&self, _currency: CommandCurrency) -> f64 {
        0.5
    }
    fn accrue_cost_estimate(&mut self, _amount: f64, _currency: CommandCurrency) {}
    fn record_turn_cost(
        &mut self,
        _amount: f64,
        _currency: CommandCurrency,
        _receipt: Option<String>,
    ) {
    }
}

struct Policy;
impl CommandModePolicyContext for Policy {
    fn mode(&self) -> CommandMode {
        CommandMode::Plan
    }
    fn set_mode(&mut self, _mode: CommandMode) {}
    fn approval_mode(&self) -> CommandApprovalMode {
        CommandApprovalMode::Suggest
    }
    fn allow_shell(&self) -> bool {
        false
    }
    fn set_shell_access(&mut self, _allow: bool) {}
    fn policy_locked(&self) -> bool {
        false
    }
}

struct Prompt;
impl CommandSystemPromptContext for Prompt {
    fn system_prompt(&self) -> Option<SystemPrompt> {
        None
    }
}

struct Skills;
impl CommandSkillsContext for Skills {
    fn active_skill(&self) -> Option<String> {
        None
    }
    fn active_skill_provenance(&self) -> Option<String> {
        None
    }
    fn refresh_skill_cache(&mut self) {}
}

struct Workspace;
impl CommandWorkspaceContext for Workspace {
    fn workspace(&self) -> PathBuf {
        PathBuf::from(".")
    }
    fn work_state_snapshot(&self) -> Result<Option<String>, String> {
        Ok(None)
    }
    fn operation_digest(&mut self) -> Result<String, String> {
        Ok("No active operations or to-do items.".to_string())
    }
}

#[test]
fn all_seven_shapes_are_object_safe() {
    fn session(_: &dyn CommandSessionContext) {}
    fn model(_: &dyn CommandModelContext) {}
    fn cost(_: &dyn CommandCostContext) {}
    fn policy(_: &dyn CommandModePolicyContext) {}
    fn prompt(_: &dyn CommandSystemPromptContext) {}
    fn skills(_: &dyn CommandSkillsContext) {}
    fn workspace(_: &dyn CommandWorkspaceContext) {}

    session(&Session);
    model(&Model);
    cost(&Cost);
    policy(&Policy);
    prompt(&Prompt);
    skills(&Skills);
    workspace(&Workspace);
}

#[test]
fn envelope_carries_independent_facets() {
    let mut session = Session;
    let mut model = Model;
    let parts = CommandContexts::empty()
        .with_session(&mut session)
        .with_model(&mut model)
        .into_parts();
    assert_eq!(parts.session.expect("session").total_tokens(), 42);
    assert!(parts.model.expect("model").auto_model());
    assert!(parts.cost.is_none());
}

fn pure(value: Option<&str>) -> String {
    value.unwrap_or_default().to_owned()
}
fn contextual(_contexts: CommandContexts<'_>, value: Option<&str>) -> String {
    value.unwrap_or_default().to_owned()
}

#[test]
fn handlers_are_plain_function_pointers() {
    let pure_handler = CommandHandler::Pure(pure);
    let contextual_handler = CommandHandler::Contextual {
        capabilities: CommandCapabilities::NONE,
        handler: contextual,
    };
    match pure_handler {
        CommandHandler::Pure(handler) => assert_eq!(handler(Some("x")), "x"),
        _ => unreachable!(),
    }
    match contextual_handler {
        CommandHandler::Contextual {
            capabilities,
            handler,
        } => {
            assert!(capabilities.is_empty());
            assert_eq!(handler(CommandContexts::empty(), Some("y")), "y")
        }
        _ => unreachable!(),
    }
}

struct Sample;
impl RegisterCommand<String> for Sample {
    fn info() -> &'static CommandInfo {
        static INFO: CommandInfo = CommandInfo {
            name: "sample",
            aliases: &["s"],
            usage: "/sample",
            description_key: "command.sample",
        };
        &INFO
    }
    fn handler() -> CommandHandler<String> {
        CommandHandler::Pure(pure)
    }
}

#[test]
fn registration_shape_has_no_app_dependency() {
    assert_eq!(Sample::info().name, "sample");
    assert!(matches!(Sample::handler(), CommandHandler::Pure(_)));
}

// ---------------------------------------------------------------------------
// FEAT-018: presentation, media, and digest capabilities (D2-D5)
// ---------------------------------------------------------------------------

struct Presentation;
impl CommandPresentationContext for Presentation {
    fn translate(&self, key: &str, replacements: &[(&str, &str)]) -> Result<String, String> {
        if key == "automation_usage" {
            return Ok("Usage: /automation [list|show <id>]".to_string());
        }
        if key == "mcp_recommended_unknown_id" {
            let command = replacements
                .iter()
                .find(|(name, _)| *name == "recommendations_command")
                .map(|(_, value)| *value)
                .unwrap_or("/mcp recommendations");
            return Ok(format!("Unknown recommended MCP ID (try {command})"));
        }
        // D3: unknown keys fail safely without echoing the raw lookup key.
        Err("unknown translation key".to_string())
    }
}

struct Media;
impl CommandMediaContext for Media {
    fn attach_media(&mut self, path: &Path) -> Result<MediaAttachmentReceipt, String> {
        if path.extension().and_then(|ext| ext.to_str()) == Some("png") {
            Ok(MediaAttachmentReceipt {
                kind: "image".to_string(),
                path: path.to_path_buf(),
            })
        } else {
            Err("Unsupported attachment type".to_string())
        }
    }
}

struct DigestWorkspace;
impl CommandWorkspaceContext for DigestWorkspace {
    fn workspace(&self) -> PathBuf {
        PathBuf::from(".")
    }
    fn work_state_snapshot(&self) -> Result<Option<String>, String> {
        Ok(None)
    }
    fn operation_digest(&mut self) -> Result<String, String> {
        Ok("No active operations or to-do items.".to_string())
    }
}

#[test]
fn new_capabilities_are_object_safe_and_independently_transportable() {
    fn presentation(_: &dyn CommandPresentationContext) {}
    fn media(_: &dyn CommandMediaContext) {}
    fn digest_workspace(_: &dyn CommandWorkspaceContext) {}

    presentation(&Presentation);
    media(&Media);
    digest_workspace(&DigestWorkspace);
    fn export(_: &dyn CommandSessionExportContext) {}
    export(&FakeExport::default());

    let mut presentation = Presentation;
    let mut media = Media;
    let mut export = FakeExport::default();
    let parts = CommandContexts::empty()
        .with_presentation(&mut presentation)
        .with_media(&mut media)
        .with_export(&mut export)
        .into_parts();
    assert!(parts.presentation.is_some());
    assert!(parts.media.is_some());
    assert!(parts.export.is_some());
    assert!(parts.session.is_none());
}

#[test]
fn translation_contract_resolves_known_keys_and_fails_safely() {
    let presentation = Presentation;
    assert_eq!(
        presentation
            .translate("automation_usage", &[])
            .expect("known key"),
        "Usage: /automation [list|show <id>]"
    );
    assert_eq!(
        presentation
            .translate(
                "mcp_recommended_unknown_id",
                &[("recommendations_command", "/mcp recommendations")],
            )
            .expect("known key with named replacement"),
        "Unknown recommended MCP ID (try /mcp recommendations)"
    );
    let unknown = presentation.translate("no_such_key", &[]);
    assert!(unknown.is_err(), "unknown key must fail safely");
    let err = unknown.unwrap_err();
    assert!(
        !err.contains("no_such_key"),
        "no raw lookup key exposure (D3)"
    );
}

#[test]
fn media_contract_is_atomic_and_returns_only_portable_data() {
    let mut media = Media;
    let ok = media
        .attach_media(Path::new("/tmp/photo.png"))
        .expect("png");
    assert_eq!(ok.kind, "image");
    assert_eq!(ok.path, PathBuf::from("/tmp/photo.png"));

    let err = media.attach_media(Path::new("/tmp/notes.txt")).unwrap_err();
    assert!(!err.is_empty(), "safe error string");
}

#[test]
fn digest_operation_returns_final_text_and_safe_errors() {
    let mut workspace = DigestWorkspace;
    assert_eq!(
        workspace.operation_digest().expect("digest"),
        "No active operations or to-do items."
    );
}

#[test]
fn envelope_rejects_duplicate_new_slots_deterministically() {
    struct SecondPresentation;
    impl CommandPresentationContext for SecondPresentation {
        fn translate(&self, _key: &str, _r: &[(&str, &str)]) -> Result<String, String> {
            Ok(String::new())
        }
    }
    struct SecondMedia;
    impl CommandMediaContext for SecondMedia {
        fn attach_media(&mut self, _p: &Path) -> Result<MediaAttachmentReceipt, String> {
            Err("unused".to_string())
        }
    }

    let mut a = Presentation;
    let mut b = SecondPresentation;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        CommandContexts::empty()
            .with_presentation(&mut a)
            .with_presentation(&mut b);
    }));
    assert!(result.is_err(), "duplicate presentation slot must assert");

    let mut a = Media;
    let mut b = SecondMedia;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        CommandContexts::empty()
            .with_media(&mut a)
            .with_media(&mut b);
    }));
    assert!(result.is_err(), "duplicate media slot must assert");
}

// ---------------------------------------------------------------------------
// Project facet (FEAT-021 D1/D4)
// ---------------------------------------------------------------------------

/// Deterministic fake project facet over portable values only.
struct FakeProject {
    lsp_enabled: bool,
    share: ProjectShareProjection,
    goal: ProjectGoalState,
}

impl FakeProject {
    fn new() -> Self {
        Self {
            lsp_enabled: false,
            share: ProjectShareProjection {
                history_is_empty: true,
                history_len: 0,
                model: "deepseek-chat".to_string(),
                mode_label: "ACT".to_string(),
            },
            goal: ProjectGoalState {
                objective: Some("Ship FEAT-021".to_string()),
                status: ProjectGoalStatus::Active,
                pause_reason: None,
                started_at_elapsed_seconds: Some(42),
                time_used_seconds: 42,
                token_budget: Some(50_000),
                tokens_used: 1_000,
                session_total_tokens: 2_000,
                continuation_count: 3,
                pending_controls: false,
                last_known_objective: None,
                last_known_status: None,
                conversation_present: true,
                is_loading: false,
                goal_continuation_waiting: false,
            },
        }
    }
}

impl CommandProjectContext for FakeProject {
    fn lsp_enabled(&self) -> bool {
        self.lsp_enabled
    }

    fn lsp_set(&mut self, enabled: bool) -> Result<(), String> {
        self.lsp_enabled = enabled;
        Ok(())
    }

    fn share_projection(&self) -> ProjectShareProjection {
        self.share.clone()
    }

    fn goal_state(&self) -> ProjectGoalState {
        self.goal.clone()
    }
}

// ---------------------------------------------------------------------------
// FEAT-019: memory capability, typed outcomes, and workspace scoping (D1-D9)
// ---------------------------------------------------------------------------

/// Deterministic fake memory facet over portable values only. Tracks the
/// workspace argument discipline (D8): only workspace-scoped methods receive
/// the workspace path.
struct FakeMemory {
    hits: Vec<MemoryHit>,
    remembered_result: Option<MemoryRemembered>,
    workspace_id_result: Result<String, String>,
}

impl FakeMemory {
    fn new() -> Self {
        Self {
            hits: vec![MemoryHit {
                source: PathBuf::from("/mem/source.md"),
                line_start: 3,
                line_end: 5,
                text: "reviewed note".to_string(),
            }],
            remembered_result: Some(MemoryRemembered {
                source: PathBuf::from("/mem/global.md"),
                line_start: 7,
            }),
            workspace_id_result: Ok("owner/repo".to_string()),
        }
    }
}

impl CommandMemoryContext for FakeMemory {
    fn memory_path(&self) -> PathBuf {
        PathBuf::from("/mem/user-memory.md")
    }

    fn memory_enabled(&self) -> bool {
        true
    }

    fn status(&self) -> Result<MemoryStatus, String> {
        Ok(MemoryStatus {
            root: PathBuf::from("/mem/memory"),
            source: PathBuf::from("/mem/memory/global/global.md"),
            index: PathBuf::from("/mem/memory/index.db"),
        })
    }

    fn path(&self) -> Result<PathBuf, String> {
        Ok(PathBuf::from("/mem/memory"))
    }

    fn workspace_id(&self, _workspace: &Path) -> Result<String, String> {
        self.workspace_id_result.clone()
    }

    fn search(
        &self,
        _workspace: &Path,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemoryHit>, String> {
        if query.is_empty() {
            return Ok(Vec::new());
        }
        Ok(self.hits.iter().take(limit).cloned().collect())
    }

    fn remember(
        &self,
        _target: MemoryRememberTarget,
        note: &str,
    ) -> Result<MemoryRemembered, String> {
        if note.is_empty() {
            return Err("empty note".to_string());
        }
        Ok(self.remembered_result.clone().unwrap_or(MemoryRemembered {
            source: PathBuf::from("/mem/global.md"),
            line_start: 1,
        }))
    }

    fn import(&self) -> Result<MemoryImportOutcome, String> {
        Ok(MemoryImportOutcome::Skipped)
    }

    fn get(&self, _workspace: &Path, id: i64) -> Result<MemoryGetOutcome, String> {
        if id == 42 {
            Ok(MemoryGetOutcome::Found(self.hits[0].clone()))
        } else {
            Ok(MemoryGetOutcome::NotFound)
        }
    }

    fn export(&self) -> Result<MemoryExport, String> {
        Ok(MemoryExport {
            content: "# memory\n\n- bullet".to_string(),
        })
    }

    fn reindex(&self) -> Result<MemoryReindex, String> {
        Ok(MemoryReindex { entry_count: 3 })
    }

    fn delete(&self, scope: MemoryDeleteScope) -> Result<MemoryDelete, String> {
        match scope {
            MemoryDeleteScope::All => Ok(MemoryDelete),
            MemoryDeleteScope::Global => Ok(MemoryDelete),
        }
    }

    fn delete_workspace(&self, _workspace: &Path) -> Result<MemoryDelete, String> {
        Ok(MemoryDelete)
    }
}

/// Recording fake that captures remember targets and delete scopes to prove
/// the typed target/scope discipline (D2/D8/D9). Interior mutability lets the
/// contract-level test assert exactly which operations the handler drives.
#[derive(Default)]
struct RecordingMemory {
    remembered_targets: std::cell::RefCell<Vec<MemoryRememberTarget>>,
    delete_scopes: std::cell::RefCell<Vec<String>>,
    workspace_deletes: std::cell::Cell<usize>,
}

impl RecordingMemory {
    fn new() -> Self {
        Self::default()
    }

    fn recorded_targets(&self) -> Vec<MemoryRememberTarget> {
        self.remembered_targets.borrow().clone()
    }

    fn recorded_delete_scopes(&self) -> Vec<String> {
        self.delete_scopes.borrow().clone()
    }

    fn recorded_workspace_deletes(&self) -> usize {
        self.workspace_deletes.get()
    }
}

impl CommandMemoryContext for RecordingMemory {
    fn memory_path(&self) -> PathBuf {
        PathBuf::from("/mem/user-memory.md")
    }

    fn memory_enabled(&self) -> bool {
        true
    }

    fn status(&self) -> Result<MemoryStatus, String> {
        unreachable!("recording fake")
    }

    fn path(&self) -> Result<PathBuf, String> {
        unreachable!("recording fake")
    }

    fn workspace_id(&self, _workspace: &Path) -> Result<String, String> {
        Ok("owner/repo".to_string())
    }

    fn search(
        &self,
        _workspace: &Path,
        _query: &str,
        _limit: usize,
    ) -> Result<Vec<MemoryHit>, String> {
        unreachable!("recording fake")
    }

    fn remember(
        &self,
        target: MemoryRememberTarget,
        _note: &str,
    ) -> Result<MemoryRemembered, String> {
        self.remembered_targets.borrow_mut().push(target);
        Ok(MemoryRemembered {
            source: PathBuf::from("/mem/global.md"),
            line_start: 1,
        })
    }

    fn import(&self) -> Result<MemoryImportOutcome, String> {
        unreachable!("recording fake")
    }

    fn get(&self, _workspace: &Path, _id: i64) -> Result<MemoryGetOutcome, String> {
        unreachable!("recording fake")
    }

    fn export(&self) -> Result<MemoryExport, String> {
        unreachable!("recording fake")
    }

    fn reindex(&self) -> Result<MemoryReindex, String> {
        unreachable!("recording fake")
    }

    fn delete(&self, scope: MemoryDeleteScope) -> Result<MemoryDelete, String> {
        self.delete_scopes.borrow_mut().push(match scope {
            MemoryDeleteScope::All => "all".to_string(),
            MemoryDeleteScope::Global => "global".to_string(),
        });
        Ok(MemoryDelete)
    }

    fn delete_workspace(&self, _workspace: &Path) -> Result<MemoryDelete, String> {
        self.workspace_deletes.set(self.workspace_deletes.get() + 1);
        Ok(MemoryDelete)
    }
}

#[test]
fn project_facet_is_object_safe_and_typed() {
    fn project(_: &dyn CommandProjectContext) {}
    project(&FakeProject::new());

    let mut project = FakeProject::new();
    assert!(!project.lsp_enabled());
    project.lsp_set(true).unwrap();
    assert!(project.lsp_enabled());
    project.lsp_set(false).unwrap();
    assert!(!project.lsp_enabled());
}

#[test]
fn project_share_projection_preserves_semantic_values() {
    let project = FakeProject::new();
    let share = project.share_projection();
    assert!(share.history_is_empty);
    assert_eq!(share.history_len, 0);
    assert_eq!(share.model, "deepseek-chat");
    assert_eq!(share.mode_label, "ACT");
}

#[test]
fn project_goal_state_preserves_semantic_values() {
    let project = FakeProject::new();
    let goal = project.goal_state();
    assert_eq!(goal.objective.as_deref(), Some("Ship FEAT-021"));
    assert_eq!(goal.status, ProjectGoalStatus::Active);
    assert_eq!(goal.pause_reason, None);
    assert_eq!(goal.started_at_elapsed_seconds, Some(42));
    assert_eq!(goal.time_used_seconds, 42);
    assert_eq!(goal.token_budget, Some(50_000));
    assert_eq!(goal.tokens_used, 1_000);
    assert_eq!(goal.session_total_tokens, 2_000);
    assert_eq!(goal.continuation_count, 3);
    assert!(!goal.pending_controls);
    assert_eq!(goal.last_known_objective, None);
    assert_eq!(goal.last_known_status, None);
    assert!(goal.conversation_present);
    assert!(!goal.is_loading);
    assert!(!goal.goal_continuation_waiting);
}

#[test]
fn project_goal_status_variants_are_distinguishable() {
    let paused = ProjectGoalState {
        status: ProjectGoalStatus::Paused,
        pause_reason: Some("user".to_string()),
        ..FakeProject::new().goal
    };
    assert_eq!(paused.status, ProjectGoalStatus::Paused);
    assert_eq!(paused.pause_reason.as_deref(), Some("user"));

    let complete = ProjectGoalState {
        status: ProjectGoalStatus::Complete,
        ..paused
    };
    assert_eq!(complete.status, ProjectGoalStatus::Complete);
    assert_ne!(complete.status, ProjectGoalStatus::Blocked);
}

#[test]
fn project_facet_transports_through_envelope_when_declared() {
    let mut project = FakeProject::new();
    let parts = CommandContexts::empty()
        .with_project(&mut project)
        .into_parts();
    assert!(parts.project.is_some());
    assert!(parts.session.is_none());

    // PROJECT combined with WORKSPACE (init) and PRESENTATION (goal).
    let mut workspace = Workspace;
    let parts = CommandContexts::empty()
        .with_project(&mut project)
        .with_workspace(&mut workspace)
        .into_parts();
    assert!(parts.project.is_some());
    assert!(parts.workspace.is_some());
    assert!(parts.presentation.is_none());
}

#[test]
fn envelope_rejects_duplicate_project_slot_deterministically() {
    let mut a = FakeProject::new();
    let mut b = FakeProject::new();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        CommandContexts::empty()
            .with_project(&mut a)
            .with_project(&mut b);
    }));
    assert!(result.is_err(), "duplicate project slot must assert");
}

#[test]
fn memory_facet_is_object_safe_and_typed() {
    fn memory(_: &dyn CommandMemoryContext) {}
    let fake = FakeMemory::new();
    memory(&fake);

    assert_eq!(fake.memory_path(), PathBuf::from("/mem/user-memory.md"));
    assert!(fake.memory_enabled());
    let status = fake.status().expect("status");
    assert_eq!(status.root, PathBuf::from("/mem/memory"));
    assert_eq!(status.source, PathBuf::from("/mem/memory/global/global.md"));
    assert_eq!(status.index, PathBuf::from("/mem/memory/index.db"));
}

#[test]
fn memory_typed_results_preserve_semantic_distinctions() {
    let fake = FakeMemory::new();

    // Search returns semantic hits, never preformatted messages.
    let hits = fake.search(Path::new("/ws"), "note", 10).expect("search");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].source, PathBuf::from("/mem/source.md"));
    assert_eq!(hits[0].line_start, 3);
    assert_eq!(hits[0].line_end, 5);
    assert_eq!(hits[0].text, "reviewed note");
    assert!(
        fake.search(Path::new("/ws"), "", 10)
            .expect("empty")
            .is_empty()
    );

    // Get distinguishes found from not-found without an error string.
    assert!(matches!(
        fake.get(Path::new("/ws"), 42),
        Ok(MemoryGetOutcome::Found(_))
    ));
    assert_eq!(
        fake.get(Path::new("/ws"), 1).expect("get"),
        MemoryGetOutcome::NotFound
    );

    // Export carries the raw document, not a command response.
    let exported = fake.export().expect("export");
    assert_eq!(exported.content, "# memory\n\n- bullet");

    // Reindex carries the typed count.
    assert_eq!(fake.reindex().expect("reindex").entry_count, 3);

    // Remember distinguishes global from workspace via the typed target.
    let global = fake
        .remember(MemoryRememberTarget::Global, "note")
        .expect("global remember");
    assert_eq!(global.source, PathBuf::from("/mem/global.md"));
    assert_eq!(global.line_start, 7);
    let workspace = fake
        .remember(
            MemoryRememberTarget::Workspace {
                workspace_id: "owner/repo".to_string(),
            },
            "note",
        )
        .expect("workspace remember");
    assert_eq!(workspace.source, PathBuf::from("/mem/global.md"));

    // Import distinguishes imported from skipped.
    assert_eq!(fake.import().expect("import"), MemoryImportOutcome::Skipped);
    assert_eq!(
        MemoryImportOutcome::Imported {
            destination: PathBuf::from("/mem/global.md")
        },
        MemoryImportOutcome::Imported {
            destination: PathBuf::from("/mem/global.md")
        }
    );

    // Remember rejects empty notes with a safe error, never a panic.
    assert!(fake.remember(MemoryRememberTarget::Global, "").is_err());

    // Zero-field delete outcome stays distinguishable.
    assert_eq!(fake.delete(MemoryDeleteScope::All), Ok(MemoryDelete));
}

#[test]
fn memory_delete_and_remember_targets_are_typed_and_scoped() {
    let memory = RecordingMemory::new();
    let _ = memory.delete(MemoryDeleteScope::All);
    let _ = memory.delete(MemoryDeleteScope::Global);
    let _ = memory.delete_workspace(Path::new("/ws"));
    let _ = memory.remember(MemoryRememberTarget::Global, "a");
    let _ = memory.remember(
        MemoryRememberTarget::Workspace {
            workspace_id: "owner/repo".to_string(),
        },
        "b",
    );

    // The non-workspace delete method receives exactly the all/global scopes;
    // workspace deletion goes through the distinct typed method (D8/D9).
    assert_eq!(memory.recorded_delete_scopes(), vec!["all", "global"]);
    assert_eq!(memory.recorded_workspace_deletes(), 1);

    // Remember targets preserve the typed global/workspace distinction.
    assert_eq!(
        memory.recorded_targets(),
        vec![
            MemoryRememberTarget::Global,
            MemoryRememberTarget::Workspace {
                workspace_id: "owner/repo".to_string(),
            },
        ]
    );
}

#[test]
fn capabilities_declare_exact_memory_authority() {
    let workspace = CommandCapabilities::WORKSPACE;
    let memory = CommandCapabilities::MEMORY;
    let workspace_memory = workspace.union(memory);

    assert_eq!(
        workspace_memory,
        CommandCapabilities::WORKSPACE | CommandCapabilities::MEMORY
    );
    assert_ne!(workspace_memory, workspace);
    assert_ne!(workspace_memory, memory);
    assert!(workspace_memory.contains(CommandCapabilities::WORKSPACE));
    assert!(workspace_memory.contains(CommandCapabilities::MEMORY));
    assert!(!workspace.contains(CommandCapabilities::MEMORY));
    assert!(!memory.contains(CommandCapabilities::WORKSPACE));
    assert!(CommandCapabilities::NONE.is_empty());
    assert!(!workspace_memory.contains(CommandCapabilities::NONE));
    assert!(!CommandCapabilities::NONE.contains(CommandCapabilities::NONE));
    // No presentation or media authority is declared for the memory group.
    assert!(!workspace_memory.contains(CommandCapabilities::PRESENTATION));
    assert!(!workspace_memory.contains(CommandCapabilities::MEDIA));
    // Existing capability identities stay stable.
    assert_ne!(CommandCapabilities::SESSION, CommandCapabilities::MODEL);
}

#[test]
fn memory_facet_transports_through_envelope_when_declared() {
    let mut memory = FakeMemory::new();
    let parts = CommandContexts::empty()
        .with_memory(&mut memory)
        .into_parts();
    assert!(parts.memory.is_some());
    assert!(parts.session.is_none());
    assert!(parts.workspace.is_none());

    // Undeclared slots stay absent when the memory facet is carried alone.
    let mut workspace = Workspace;
    let parts = CommandContexts::empty()
        .with_memory(&mut memory)
        .with_workspace(&mut workspace)
        .into_parts();
    assert!(parts.memory.is_some());
    assert!(parts.workspace.is_some());
    assert!(parts.presentation.is_none());
    assert!(parts.media.is_none());
}

#[test]
fn envelope_rejects_duplicate_memory_slot_deterministically() {
    let mut a = FakeMemory::new();
    let mut b = FakeMemory::new();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        CommandContexts::empty()
            .with_memory(&mut a)
            .with_memory(&mut b);
    }));
    assert!(result.is_err(), "duplicate memory slot must assert");
}

// ---------------------------------------------------------------------------
// FEAT-020: plugin capability, portable DTOs, and envelope slot (D1-D11)
// ---------------------------------------------------------------------------

/// Deterministic fake plugin facet over portable values only.
struct FakePlugin {
    summaries: Vec<PluginSummary>,
    detail: Option<PluginDetail>,
    installed: bool,
    managed_candidates: Vec<PluginManagedCandidate>,
}

impl FakePlugin {
    fn new() -> Self {
        Self {
            summaries: vec![PluginSummary {
                name: "demo".to_string(),
                id: "demo@1.0.0".to_string(),
                state_label: "active".to_string(),
                scope: "user".to_string(),
                trust_status: "trusted".to_string(),
                compatibility: "full".to_string(),
                inventory: "skills=1 mcp=0".to_string(),
                active: true,
                trusted: true,
                enabled: true,
            }],
            detail: Some(PluginDetail {
                name: "demo".to_string(),
                id: "demo@1.0.0".to_string(),
                inventory_summary: "skills=1 mcp=0".to_string(),
                version: "1.0.0".to_string(),
                origin: "local".to_string(),
                scope: "user".to_string(),
                state_label: "active".to_string(),
                trust_status: "trusted".to_string(),
                compatibility: "full".to_string(),
                content_hash: "abc".to_string(),
                capability_hash: "def".to_string(),
                canonical_root: PathBuf::from("/plugins/demo"),
                active: true,
                trusted: true,
                enabled: true,
                unsupported_labels: Vec::new(),
                supported_labels: vec!["skills".to_string()],
                skills: vec!["demo:demo-skill".to_string()],
                filesystem_roots: Vec::new(),
                network_hosts: Vec::new(),
                stdio_mcp_servers: 0,
                lifecycle_mutation: false,
                mcp_servers: Vec::new(),
                diagnostics: Vec::new(),
            }),
            installed: false,
            managed_candidates: Vec::new(),
        }
    }
}

impl CommandPluginContext for FakePlugin {
    fn summaries(&self) -> Result<Vec<PluginSummary>, String> {
        Ok(self.summaries.clone())
    }

    fn detail(&self, selector: &str) -> Result<PluginDetail, String> {
        if selector == "demo" {
            self.detail
                .clone()
                .ok_or_else(|| "missing detail".to_string())
        } else {
            Err(format!("no plugin named {selector}"))
        }
    }

    fn registry_diagnostics(&self) -> Vec<PluginDiagnostic> {
        Vec::new()
    }

    fn validation_is_clean(&self) -> bool {
        true
    }

    fn len(&self) -> usize {
        self.summaries.len()
    }

    fn reload(&mut self) -> Result<usize, String> {
        Ok(self.summaries.len())
    }

    fn is_empty(&self) -> bool {
        self.summaries.is_empty()
    }

    fn reload_nudge(&mut self) -> Option<String> {
        None
    }

    fn state_path(&self) -> Option<PathBuf> {
        Some(PathBuf::from("/plugins/state.json"))
    }

    fn suggest(&self, task: &str) -> Result<Vec<PluginSuggestion>, String> {
        if task.len() < 3 {
            return Err("task too short".to_string());
        }
        Ok(vec![PluginSuggestion {
            name: "demo".to_string(),
            state_label: "active".to_string(),
            description: "Demo bundle".to_string(),
            why: vec![task.to_string()],
            next_step: "Already active: /plugin show demo".to_string(),
        }])
    }

    fn trust(&mut self, _selector: &str, token: &str) -> Result<(), String> {
        if token == "abc.def" {
            Ok(())
        } else {
            Err("Review token does not match this bundle content and capability set".to_string())
        }
    }

    fn enable(&mut self, _selector: &str) -> Result<(), String> {
        Ok(())
    }

    fn disable(&mut self, _selector: &str) -> Result<(), String> {
        Ok(())
    }

    fn revoke_trust(&mut self, _selector: &str) -> Result<(), String> {
        Ok(())
    }

    fn install(
        &mut self,
        _source: &str,
        expected_content_hash: Option<&str>,
    ) -> Result<PluginMutationReceipt, String> {
        if let Some(expected) = expected_content_hash
            && expected != "abc"
        {
            return Err("content hash mismatch".to_string());
        }
        self.installed = true;
        Ok(PluginMutationReceipt {
            name: "demo".to_string(),
            path: Some(PathBuf::from("/plugins/demo")),
            content_hash: Some("abc".to_string()),
            installed_content_hash: Some("abc".to_string()),
            outcome: PluginMutationOutcome::Installed,
        })
    }

    fn update(&mut self, _selector: &str) -> Result<PluginMutationReceipt, String> {
        Ok(PluginMutationReceipt {
            name: "demo".to_string(),
            path: None,
            content_hash: None,
            installed_content_hash: None,
            outcome: PluginMutationOutcome::NoChange,
        })
    }

    fn uninstall(&mut self, _selector: &str) -> Result<PluginMutationReceipt, String> {
        Ok(PluginMutationReceipt {
            name: "demo".to_string(),
            path: None,
            content_hash: None,
            installed_content_hash: None,
            outcome: PluginMutationOutcome::Uninstalled,
        })
    }

    fn uninstall_path(&mut self, _name: &str, _plugins_dir: &Path) -> Result<(), String> {
        Ok(())
    }

    fn export(&self, _selector: &str, target: &Path) -> Result<PluginExportReceipt, String> {
        Ok(PluginExportReceipt {
            exported_name: "demo".to_string(),
            target: target.to_path_buf(),
            display_name: Some("Demo Bundle".to_string()),
            wrote_mcp_json: false,
            files_copied: 2,
            skills_normalized: false,
        })
    }

    fn legacy_scan(&self) -> Result<Option<PluginLegacyScan>, String> {
        Ok(None)
    }

    fn managed_scan(&self, _home_override: Option<&Path>) -> Result<PluginManagedScan, String> {
        Ok(PluginManagedScan {
            root: PathBuf::from("/kimi/managed"),
            candidates: self.managed_candidates.clone(),
            rejected: Vec::new(),
        })
    }

    fn managed_install(
        &mut self,
        canonical_path: &Path,
        expected_content_hash: &str,
    ) -> Result<PluginMutationReceipt, String> {
        if expected_content_hash != "abc" {
            return Err("Kimi candidate changed".to_string());
        }
        Ok(PluginMutationReceipt {
            name: "kimi-demo".to_string(),
            path: Some(canonical_path.to_path_buf()),
            content_hash: Some("abc".to_string()),
            installed_content_hash: Some("abc".to_string()),
            outcome: PluginMutationOutcome::Installed,
        })
    }

    fn marketplace_state(&self) -> Result<PluginMarketplaceState, String> {
        Ok(PluginMarketplaceState {
            official: Some(PluginMarketplaceCatalog {
                id: "official".to_string(),
                source_path: None,
                display_name: None,
                description: Some("Built into this release".to_string()),
                format: "codewhale".to_string(),
                tier: "official".to_string(),
                publisher: Some("Codewhale".to_string()),
                total_candidates: 1,
                warning_count: 0,
                candidates: Vec::new(),
                diagnostics: Vec::new(),
            }),
            stored: Vec::new(),
        })
    }

    fn marketplace_add(
        &mut self,
        name: &str,
        _path: &Path,
    ) -> Result<PluginMarketplaceAddReceipt, String> {
        if name == "official" {
            return Err(
                "`official` is the catalog built into Codewhale; pick another name.".to_string(),
            );
        }
        Ok(PluginMarketplaceAddReceipt {
            name: name.to_string(),
            candidate_count: 0,
            warning_count: 0,
            catalog: PluginMarketplaceCatalog {
                id: name.to_string(),
                source_path: None,
                display_name: None,
                description: None,
                format: "kimi".to_string(),
                tier: "community".to_string(),
                publisher: None,
                total_candidates: 0,
                warning_count: 0,
                candidates: Vec::new(),
                diagnostics: Vec::new(),
            },
        })
    }

    fn marketplace_remove(&mut self, _name: &str) -> Result<bool, String> {
        Ok(true)
    }

    fn marketplace_install(
        &mut self,
        _catalog: &str,
        _candidate: &str,
    ) -> Result<PluginMutationReceipt, String> {
        Ok(PluginMutationReceipt {
            name: "market-demo".to_string(),
            path: None,
            content_hash: None,
            installed_content_hash: None,
            outcome: PluginMutationOutcome::Installed,
        })
    }
}

#[test]
fn plugin_facet_is_object_safe_and_typed() {
    fn plugin(_: &dyn CommandPluginContext) {}
    plugin(&FakePlugin::new());

    let plugin = FakePlugin::new();
    assert_eq!(plugin.len(), 1);
    assert!(!plugin.is_empty());
    assert!(plugin.validation_is_clean());
    let summaries = plugin.summaries().unwrap();
    assert_eq!(summaries[0].name, "demo");
    assert_eq!(summaries[0].state_label, "active");
}

#[test]
fn plugin_detail_preserves_semantic_values() {
    let plugin = FakePlugin::new();
    let detail = plugin.detail("demo").unwrap();
    assert_eq!(detail.content_hash, "abc");
    assert_eq!(detail.capability_hash, "def");
    assert_eq!(detail.compatibility, "full");
    assert!(detail.active);
    assert_eq!(detail.skills, vec!["demo:demo-skill"]);
    // Unknown selector fails safely.
    assert!(plugin.detail("nope").is_err());
}

#[test]
fn plugin_mutation_receipts_distinguish_outcomes() {
    let mut plugin = FakePlugin::new();
    let installed = plugin.install("path:/demo", Some("abc")).unwrap();
    assert_eq!(installed.outcome, PluginMutationOutcome::Installed);
    assert_eq!(installed.installed_content_hash.as_deref(), Some("abc"));

    // Exact-hash mismatch fails before any install side effect.
    let err = plugin.install("path:/demo", Some("wrong")).unwrap_err();
    assert!(err.contains("content hash mismatch"));

    let uninstalled = plugin.uninstall("demo").unwrap();
    assert_eq!(uninstalled.outcome, PluginMutationOutcome::Uninstalled);

    plugin.trust("demo", "abc.def").unwrap();
    let trust_err = plugin.trust("demo", "bad.token").unwrap_err();
    assert!(trust_err.contains("Review token does not match"));
}

#[test]
fn plugin_managed_and_marketplace_values_are_portable() {
    let mut plugin = FakePlugin::new();
    let scan = plugin.managed_scan(None).unwrap();
    assert_eq!(scan.root, PathBuf::from("/kimi/managed"));
    assert!(scan.candidates.is_empty());

    plugin.managed_candidates.push(PluginManagedCandidate {
        name: "kimi-demo".to_string(),
        version: "1.0.0".to_string(),
        license: Some("MIT".to_string()),
        canonical_path: PathBuf::from("/kimi/managed/kimi-demo"),
        content_hash: "abc".to_string(),
        capability_hash: "def".to_string(),
        inventory: "skills=1".to_string(),
        applicable: true,
    });
    let scan = plugin.managed_scan(None).unwrap();
    assert_eq!(scan.candidates[0].name, "kimi-demo");
    assert_eq!(scan.candidates[0].license.as_deref(), Some("MIT"));

    let state = plugin.marketplace_state().unwrap();
    let official = state.official.as_ref().expect("fake official catalog");
    assert_eq!(official.id, "official");
    assert_eq!(official.tier, "official");
    assert!(state.stored.is_empty());

    let add = plugin
        .marketplace_add("custom", Path::new("/catalog.json"))
        .unwrap();
    assert_eq!(add.name, "custom");
    assert_eq!(add.catalog.format, "kimi");

    let err = plugin
        .marketplace_add("official", Path::new("/x.json"))
        .unwrap_err();
    assert!(err.contains("built into Codewhale"));
}

#[test]
fn plugin_suggest_is_read_only_and_safe() {
    let plugin = FakePlugin::new();
    let err = plugin.suggest("ab").unwrap_err();
    assert!(err.contains("too short"));
    let suggestions = plugin.suggest("translate").unwrap();
    assert_eq!(suggestions[0].name, "demo");
    assert_eq!(
        suggestions[0].next_step,
        "Already active: /plugin show demo"
    );
}

#[test]
fn plugin_facet_transports_through_envelope_when_declared() {
    let mut plugin = FakePlugin::new();
    let parts = CommandContexts::empty()
        .with_plugin(&mut plugin)
        .into_parts();
    assert!(parts.plugin.is_some());
    assert!(parts.session.is_none());
    assert!(parts.memory.is_none());

    // Undeclared slots stay absent when the plugin facet is carried alone.
    let mut workspace = Workspace;
    let parts = CommandContexts::empty()
        .with_plugin(&mut plugin)
        .with_workspace(&mut workspace)
        .into_parts();
    assert!(parts.plugin.is_some());
    assert!(parts.workspace.is_some());
    assert!(parts.presentation.is_none());
}

#[test]
fn envelope_rejects_duplicate_plugin_slot_deterministically() {
    let mut a = FakePlugin::new();
    let mut b = FakePlugin::new();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        CommandContexts::empty()
            .with_plugin(&mut a)
            .with_plugin(&mut b);
    }));
    assert!(result.is_err(), "duplicate plugin slot must assert");
}

#[test]
fn plugin_capability_bit_is_stable_and_distinct() {
    let plugin = CommandCapabilities::PLUGIN;
    assert_eq!(plugin, CommandCapabilities::PLUGIN);
    assert!(plugin.contains(CommandCapabilities::PLUGIN));
    assert!(!plugin.contains(CommandCapabilities::MEMORY));
    assert!(!plugin.contains(CommandCapabilities::PROJECT));
    assert!(!plugin.contains(CommandCapabilities::SKILL_GROUP));
    assert!(!plugin.contains(CommandCapabilities::WORKSPACE));

    let plugin_workspace = CommandCapabilities::PLUGIN.union(CommandCapabilities::WORKSPACE);
    assert!(plugin_workspace.contains(CommandCapabilities::PLUGIN));
    assert!(plugin_workspace.contains(CommandCapabilities::WORKSPACE));
    assert!(!plugin_workspace.contains(CommandCapabilities::MEMORY));

    // The plugin group declares exactly WORKSPACE | PRESENTATION | PLUGIN.
    let exact = CommandCapabilities::WORKSPACE
        .union(CommandCapabilities::PRESENTATION)
        .union(CommandCapabilities::PLUGIN);
    assert!(exact.contains(CommandCapabilities::PLUGIN));
    assert!(exact.contains(CommandCapabilities::PRESENTATION));
    assert!(!exact.contains(CommandCapabilities::MEDIA));
    assert!(!exact.contains(CommandCapabilities::MEMORY));
    assert!(!exact.contains(CommandCapabilities::PROJECT));
    assert!(!exact.contains(CommandCapabilities::SKILL_GROUP));
    assert!(!exact.contains(CommandCapabilities::SKILLS));
}

// FEAT-022: skill-group facet (CommandSkillGroupContext)
// ---------------------------------------------------------------------------

struct FakeSkillGroup {
    projection: SkillRegistryProjection,
    activation_result: Result<SkillActivationOutcome, SkillActivationError>,
    receipt: SkillMutationReceipt,
    remote: Result<RemoteRegistryOutcome, String>,
    sync: Result<SkillSyncOutcome, String>,
    review: Result<ReviewOutcome, String>,
    snapshots: Vec<SnapshotEntry>,
    restore_ok: bool,
    approval: CommandApprovalState,
}

impl FakeSkillGroup {
    fn new() -> Self {
        Self {
            projection: SkillRegistryProjection {
                workspace: "/ws".into(),
                skills_dir: "/ws/.codewhale/skills".into(),
                mode_label: "compatible".into(),
                dirs: vec!["/ws/.codewhale/skills".into()],
                entries: vec![SkillEntry {
                    name: "demo".into(),
                    description: "Demo skill".into(),
                    source: SkillSourceKind::Native,
                    path: Some("/ws/.codewhale/skills/demo/SKILL.md".into()),
                    bundled_tier: None,
                }],
                warnings: vec!["one warning".into()],
                total: 1,
            },
            activation_result: Ok(SkillActivationOutcome {
                name: "demo".into(),
                description: "Demo skill".into(),
            }),
            receipt: SkillMutationReceipt {
                name: "demo".into(),
                safe_target_path: "/ws/.codewhale/skills/demo".into(),
                outcome: SkillMutationOutcome::Installed,
            },
            remote: Ok(RemoteRegistryOutcome::Loaded {
                entries: vec![RemoteSkillEntry {
                    name: "demo".into(),
                    description: Some("Remote demo".into()),
                    source: "github.com/acme/skills".into(),
                }],
            }),
            sync: Ok(SkillSyncOutcome::Done {
                total: 1,
                downloaded: 1,
                fresh: 0,
                failed: 0,
                entries: vec![SkillSyncEntry::Downloaded {
                    name: "demo".into(),
                    path: "/cache/demo".into(),
                }],
            }),
            review: Ok(ReviewOutcome::Ready),
            snapshots: vec![SnapshotEntry {
                id: "abcdef123456".into(),
                label: "pre-turn:1".into(),
                timestamp: 1_700_000_000,
            }],
            restore_ok: true,
            approval: CommandApprovalState {
                yolo: true,
                trust_mode: false,
            },
        }
    }
}

impl CommandSkillGroupContext for FakeSkillGroup {
    fn skill_registry_projection(&self) -> SkillRegistryProjection {
        self.projection.clone()
    }

    fn activate_skill(
        &mut self,
        _name: &str,
    ) -> Result<SkillActivationOutcome, SkillActivationError> {
        self.activation_result.clone()
    }

    fn install_skill(
        &mut self,
        _scope: Option<SkillTargetScope>,
        _spec: &str,
    ) -> Result<SkillMutationReceipt, String> {
        Ok(self.receipt.clone())
    }

    fn update_skill(
        &mut self,
        _scope: Option<SkillTargetScope>,
        _name: &str,
    ) -> Result<SkillMutationReceipt, String> {
        Ok(self.receipt.clone())
    }

    fn uninstall_skill(
        &mut self,
        _scope: Option<SkillTargetScope>,
        _name: &str,
    ) -> Result<SkillMutationReceipt, String> {
        Ok(self.receipt.clone())
    }

    fn trust_skill(
        &mut self,
        _scope: Option<SkillTargetScope>,
        _name: &str,
    ) -> Result<SkillMutationReceipt, String> {
        Ok(self.receipt.clone())
    }

    fn fetch_remote_registry(&mut self) -> Result<RemoteRegistryOutcome, String> {
        self.remote.clone()
    }

    fn recommend_skills(&mut self, task: &str) -> Result<Vec<SkillRecommendation>, String> {
        Ok(vec![SkillRecommendation {
            name: format!("rec-{task}"),
            description: Some("Recommended".into()),
            matched_terms: vec!["term".into()],
        }])
    }

    fn sync_registry(&mut self) -> Result<SkillSyncOutcome, String> {
        self.sync.clone()
    }

    fn run_review(&mut self) -> Result<ReviewOutcome, String> {
        self.review.clone()
    }

    fn snapshot_list(&mut self, _limit: usize) -> Result<Vec<SnapshotEntry>, String> {
        Ok(self.snapshots.clone())
    }

    fn restore_snapshot(&mut self, _id: &str) -> Result<(), String> {
        if self.restore_ok {
            Ok(())
        } else {
            Err("Restore failed: boom".into())
        }
    }

    fn approval_state(&self) -> CommandApprovalState {
        self.approval
    }
}

#[test]
fn skill_group_facet_is_object_safe_and_typed() {
    fn project(_: &dyn CommandSkillGroupContext) {}
    project(&FakeSkillGroup::new());

    let group = FakeSkillGroup::new();
    let projection = group.skill_registry_projection();
    assert_eq!(projection.total, 1);
    assert_eq!(projection.entries[0].name, "demo");
    assert!(group.approval_state().yolo);
}

#[test]
fn skill_registry_projection_preserves_semantic_values() {
    let group = FakeSkillGroup::new();
    let projection = group.skill_registry_projection();
    assert_eq!(projection.workspace, "/ws");
    assert_eq!(projection.skills_dir, "/ws/.codewhale/skills");
    assert_eq!(projection.mode_label, "compatible");
    assert_eq!(projection.dirs, vec!["/ws/.codewhale/skills"]);
    assert_eq!(projection.warnings, vec!["one warning"]);
    assert_eq!(projection.entries.len(), 1);
    let entry = &projection.entries[0];
    assert_eq!(entry.name, "demo");
    assert_eq!(entry.description, "Demo skill");
    assert_eq!(entry.source, SkillSourceKind::Native);
    assert_eq!(
        entry.path.as_deref(),
        Some("/ws/.codewhale/skills/demo/SKILL.md")
    );
    assert_eq!(entry.bundled_tier, None);
}

#[test]
fn skill_bundled_tier_headings_are_stable() {
    assert_eq!(SkillBundledTier::CoreAgentic.heading(), "Core agentic");
    assert_eq!(
        SkillBundledTier::FormatTooling.heading(),
        "Format & tooling"
    );
}

#[test]
fn skill_mutation_receipt_preserves_outcome_variants() {
    let installed = FakeSkillGroup::new().receipt;
    assert_eq!(installed.name, "demo");
    assert_eq!(installed.outcome, SkillMutationOutcome::Installed);

    let denied = SkillMutationReceipt {
        outcome: SkillMutationOutcome::NetworkDenied("acme.com".into()),
        ..installed.clone()
    };
    assert_eq!(
        denied.outcome,
        SkillMutationOutcome::NetworkDenied("acme.com".into())
    );

    let approval = SkillMutationReceipt {
        outcome: SkillMutationOutcome::NeedsApproval("acme.com".into()),
        ..installed.clone()
    };
    assert_eq!(
        approval.outcome,
        SkillMutationOutcome::NeedsApproval("acme.com".into())
    );

    assert_ne!(installed.outcome, denied.outcome);
    assert_ne!(installed.outcome, approval.outcome);
    assert_ne!(denied.outcome, approval.outcome);
}

#[test]
fn skill_source_kind_variants_are_distinguishable() {
    let native = SkillSourceKind::Native;
    let plugin = SkillSourceKind::Plugin {
        plugin_name: "acme".into(),
        plugin_id: "acme-1".into(),
    };
    assert_ne!(native, plugin);
    assert_eq!(
        plugin,
        SkillSourceKind::Plugin {
            plugin_name: "acme".into(),
            plugin_id: "acme-1".into(),
        }
    );
}

#[test]
fn remote_registry_outcome_variants_are_distinguishable() {
    let loaded = RemoteRegistryOutcome::Loaded {
        entries: vec![RemoteSkillEntry {
            name: "demo".into(),
            description: None,
            source: "acme".into(),
        }],
    };
    let approval = RemoteRegistryOutcome::NeedsApproval("acme.com".into());
    let denied = RemoteRegistryOutcome::Denied("acme.com".into());
    assert_ne!(loaded, approval);
    assert_ne!(loaded, denied);
    assert_ne!(approval, denied);
}

#[test]
fn skill_sync_outcome_preserves_all_entry_variants() {
    let outcome = SkillSyncOutcome::Done {
        total: 4,
        downloaded: 1,
        fresh: 1,
        failed: 2,
        entries: vec![
            SkillSyncEntry::Downloaded {
                name: "a".into(),
                path: "/cache/a".into(),
            },
            SkillSyncEntry::Fresh { name: "b".into() },
            SkillSyncEntry::Failed {
                name: "c".into(),
                reason: "boom".into(),
            },
            SkillSyncEntry::Denied {
                name: "d".into(),
                host: "acme.com".into(),
            },
            SkillSyncEntry::NeedsApproval {
                name: "e".into(),
                host: "acme.com".into(),
            },
        ],
    };
    let SkillSyncOutcome::Done {
        total,
        downloaded,
        fresh,
        failed,
        entries,
    } = &outcome
    else {
        panic!("expected Done");
    };
    assert_eq!(*total, 4);
    assert_eq!(*downloaded, 1);
    assert_eq!(*fresh, 1);
    assert_eq!(*failed, 2);
    assert_eq!(entries.len(), 5);
    assert!(matches!(entries[0], SkillSyncEntry::Downloaded { .. }));
    assert!(matches!(entries[1], SkillSyncEntry::Fresh { .. }));
    assert!(matches!(entries[2], SkillSyncEntry::Failed { .. }));
    assert!(matches!(entries[3], SkillSyncEntry::Denied { .. }));
    assert!(matches!(entries[4], SkillSyncEntry::NeedsApproval { .. }));
}

#[test]
fn skill_sync_registry_policy_variants_are_distinguishable() {
    let approval = SkillSyncOutcome::RegistryNeedsApproval("acme.com".into());
    let denied = SkillSyncOutcome::RegistryDenied("acme.com".into());
    assert_ne!(approval, denied);
    assert!(matches!(
        approval,
        SkillSyncOutcome::RegistryNeedsApproval(host) if host == "acme.com"
    ));
    assert!(matches!(
        denied,
        SkillSyncOutcome::RegistryDenied(host) if host == "acme.com"
    ));
}

#[test]
fn skill_activation_error_variants_are_distinguishable() {
    let mut group = FakeSkillGroup::new();
    group.activation_result = Err(SkillActivationError::NotFound {
        requested: "missing".into(),
        available: vec!["demo".into()],
        warnings: vec![],
    });
    let not_found = group.activate_skill("missing").unwrap_err();
    match &not_found {
        SkillActivationError::NotFound {
            requested,
            available,
            ..
        } => {
            assert_eq!(requested, "missing");
            assert_eq!(available, &vec!["demo".to_string()]);
        }
        _ => panic!("expected NotFound"),
    }

    let mut group = FakeSkillGroup::new();
    group.activation_result = Err(SkillActivationError::PluginRejected {
        name: "plug".into(),
        reason: "authority revoked".into(),
    });
    let rejected = group.activate_skill("plug").unwrap_err();
    match rejected {
        SkillActivationError::PluginRejected { name, reason } => {
            assert_eq!(name, "plug");
            assert_eq!(reason, "authority revoked");
        }
        _ => panic!("expected PluginRejected"),
    }
}

#[test]
fn review_outcome_variants_are_distinguishable() {
    let mut group = FakeSkillGroup::new();
    group.review = Ok(ReviewOutcome::NotFound {
        skills_dir: "/ws/skills".into(),
        global_dir: "/home/u/.codewhale/skills".into(),
        warnings: vec!["w".into()],
    });
    let outcome = group.run_review().unwrap();
    match outcome {
        ReviewOutcome::NotFound {
            skills_dir,
            global_dir,
            warnings,
        } => {
            assert_eq!(skills_dir, "/ws/skills");
            assert_eq!(global_dir, "/home/u/.codewhale/skills");
            assert_eq!(warnings, vec!["w".to_string()]);
        }
        _ => panic!("expected NotFound"),
    }
}

#[test]
fn snapshot_and_approval_values_preserve_semantics() {
    let mut group = FakeSkillGroup::new();
    let snapshots = group.snapshot_list(20).unwrap();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].id, "abcdef123456");
    assert_eq!(snapshots[0].label, "pre-turn:1");
    assert_eq!(snapshots[0].timestamp, 1_700_000_000);

    let approval = group.approval_state();
    assert!(approval.yolo);
    assert!(!approval.trust_mode);
}

#[test]
fn skill_group_facet_transports_through_envelope_when_declared() {
    let mut group = FakeSkillGroup::new();
    let parts = CommandContexts::empty()
        .with_skill_group(&mut group)
        .into_parts();
    assert!(parts.skill_group.is_some());
    assert!(parts.session.is_none());
    assert!(parts.project.is_none());

    // /skill combines skill_group with SKILLS for baseline cache refreshes.
    let mut skills = Skills;
    let parts = CommandContexts::empty()
        .with_skill_group(&mut group)
        .with_skills(&mut skills)
        .into_parts();
    assert!(parts.skill_group.is_some());
    assert!(parts.skills.is_some());
    assert!(parts.workspace.is_none());
}

#[test]
fn envelope_rejects_duplicate_skill_group_slot_deterministically() {
    let mut a = FakeSkillGroup::new();
    let mut b = FakeSkillGroup::new();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        CommandContexts::empty()
            .with_skill_group(&mut a)
            .with_skill_group(&mut b);
    }));
    assert!(result.is_err(), "duplicate skill_group slot must assert");
}

/// Regression: the shared FEAT-015 `CommandSkillsContext` surface is unchanged
/// (getters + cache refresh only, no setter) and still transports through the
/// envelope alongside the new skill-group facet (D2).
#[test]
fn shared_skills_facet_surface_remains_read_only_and_transportable() {
    let mut skills = Skills;
    let active = skills.active_skill();
    assert_eq!(active, None);
    assert_eq!(skills.active_skill_provenance(), None);
    skills.refresh_skill_cache();

    let mut group = FakeSkillGroup::new();
    let parts = CommandContexts::empty()
        .with_skills(&mut skills)
        .with_skill_group(&mut group)
        .into_parts();
    assert!(parts.skills.is_some());
    assert!(parts.skill_group.is_some());
}

// ---------------------------------------------------------------------------
// FEAT-023: session lifecycle contract (D2/D3/D6).
// ---------------------------------------------------------------------------

#[test]
fn lifecycle_capability_is_stable_distinct_and_non_conflicting() {
    let lifecycle = CommandCapabilities::SESSION_LIFECYCLE;
    for existing in [
        CommandCapabilities::NONE,
        CommandCapabilities::SESSION,
        CommandCapabilities::MODEL,
        CommandCapabilities::COST,
        CommandCapabilities::MODE_POLICY,
        CommandCapabilities::SYSTEM_PROMPT,
        CommandCapabilities::SKILLS,
        CommandCapabilities::WORKSPACE,
        CommandCapabilities::PRESENTATION,
        CommandCapabilities::MEDIA,
        CommandCapabilities::MEMORY,
        CommandCapabilities::PROJECT,
        CommandCapabilities::SKILL_GROUP,
        CommandCapabilities::PLUGIN,
    ] {
        assert_ne!(lifecycle, existing, "SESSION_LIFECYCLE must not collide");
    }
    assert!(!CommandCapabilities::NONE.contains(lifecycle));
    assert!(lifecycle.contains(lifecycle));
    assert!(
        lifecycle
            .union(CommandCapabilities::SESSION)
            .contains(lifecycle)
    );
    assert!(
        lifecycle
            .union(CommandCapabilities::SESSION)
            .contains(CommandCapabilities::SESSION)
    );
}

/// Deterministic fake lifecycle facet: every delegate returns canned portable
/// values or error text so the contract transport is exercised exactly.
#[derive(Default)]
struct FakeLifecycle {
    blocked: bool,
    leaf_hint: Option<String>,
    branch_outcome: Option<SessionBranchOutcome>,
    branch_error: Option<String>,
    tree: Option<Result<TreeBodyProjection, String>>,
    save: Option<Result<SessionSaveReceipt, String>>,
    fork_active: Option<Result<SessionForkReceipt, String>>,
    fork_from: Option<Result<SessionForkFromReceipt, String>>,
    fresh: Option<Result<SessionNewReceipt, String>>,
    load: Option<Result<PathBuf, String>>,
    picker: Option<String>,
    archived: Option<Result<SessionArchiveReceipt, String>>,
    prune: Option<Result<usize, String>>,
}

impl CommandSessionLifecycleContext for FakeLifecycle {
    fn transition_blocked(&self) -> bool {
        self.blocked
    }
    fn branch_current_leaf_hint(&self) -> Option<String> {
        self.leaf_hint.clone()
    }
    fn branch_to(&mut self, entry_id: &str) -> Result<SessionBranchOutcome, String> {
        if let Some(err) = &self.branch_error {
            return Err(err.clone());
        }
        self.branch_outcome
            .clone()
            .ok_or_else(|| format!("unexpected branch_to({entry_id}) on empty fake"))
    }
    fn tree_body(&self) -> Result<TreeBodyProjection, String> {
        self.tree
            .clone()
            .unwrap_or(Ok(TreeBodyProjection::NoSession))
    }
    fn save_session(
        &mut self,
        explicit_path: Option<String>,
    ) -> Result<SessionSaveReceipt, String> {
        self.save
            .clone()
            .ok_or_else(|| format!("unexpected save_session({explicit_path:?}) on empty fake"))?
    }
    fn fork_active(&mut self) -> Result<SessionForkReceipt, String> {
        self.fork_active
            .clone()
            .ok_or_else(|| "unexpected fork_active() on empty fake".to_string())?
    }
    fn fork_from(&mut self, id: &str) -> Result<SessionForkFromReceipt, String> {
        self.fork_from
            .clone()
            .ok_or_else(|| format!("unexpected fork_from({id}) on empty fake"))?
    }
    fn fresh_session(&mut self, force: bool) -> Result<SessionNewReceipt, String> {
        self.fresh
            .clone()
            .ok_or_else(|| format!("unexpected fresh_session({force}) on empty fake"))?
    }
    fn load_session(&mut self, path: &str) -> Result<PathBuf, String> {
        self.load
            .clone()
            .ok_or_else(|| format!("unexpected load_session({path}) on empty fake"))?
    }
    fn open_picker(&mut self, preselected: Option<String>) {
        self.picker = preselected;
    }
    fn set_archived(
        &mut self,
        session_id: &str,
        archived: bool,
    ) -> Result<SessionArchiveReceipt, String> {
        self.archived.clone().ok_or_else(|| {
            format!("unexpected set_archived({session_id}, {archived}) on empty fake")
        })?
    }
    fn prune_sessions(&mut self, days: u64) -> Result<usize, String> {
        self.prune
            .clone()
            .ok_or_else(|| format!("unexpected prune_sessions({days}) on empty fake"))?
    }
}

fn lifecycle_sync_payload(session_id: Option<&str>) -> SessionSyncPayload {
    SessionSyncPayload {
        session_id: session_id.map(str::to_string),
        messages: vec![Message {
            role: Role::User,
            content: vec![ContentBlock::Text {
                text: "hello lifecycle".to_string(),
                cache_control: None,
            }],
        }],
        system_prompt: Some(SystemPrompt::Text("prompt".to_string())),
        model: "lifecycle-model".to_string(),
        workspace: PathBuf::from("/workspace/lifecycle"),
        mode: CommandMode::Plan,
    }
}

#[test]
fn lifecycle_facet_is_object_safe_and_transports_every_outcome() {
    // Object safety: usable behind a single `dyn` reference.
    fn accepts_dyn(_: &dyn CommandSessionLifecycleContext) {}
    fn accepts_dyn_mut(_: &mut dyn CommandSessionLifecycleContext) {}

    let mut fake = FakeLifecycle {
        blocked: true,
        leaf_hint: Some("entry-42".to_string()),
        branch_outcome: Some(SessionBranchOutcome {
            leaf_display: "entry-43".to_string(),
            journal_entries_before: 7,
        }),
        tree: Some(Ok(TreeBodyProjection::Journal {
            rendered: "rendered journal".to_string(),
        })),
        save: Some(Ok(SessionSaveReceipt {
            display_path: "/tmp/session.json".to_string(),
            truncated_id: "abc123".to_string(),
        })),
        fork_active: Some(Ok(SessionForkReceipt {
            parent_label: "parent".to_string(),
            fork_label: "child".to_string(),
            sync: lifecycle_sync_payload(Some("child")),
        })),
        fork_from: Some(Ok(SessionForkFromReceipt {
            parent_label: "source".to_string(),
            fork_label: "sibling".to_string(),
            spawn_depth: 3,
            sync: lifecycle_sync_payload(Some("sibling")),
        })),
        fresh: Some(Ok(SessionNewReceipt {
            truncated_id: "new-id".to_string(),
            sync: lifecycle_sync_payload(Some("new-id")),
        })),
        load: Some(Ok(PathBuf::from("/tmp/loaded.json"))),
        archived: Some(Ok(SessionArchiveReceipt {
            truncated_id: "arch-1".to_string(),
            title: "Archive Title".to_string(),
        })),
        prune: Some(Ok(3)),
        ..FakeLifecycle::default()
    };
    accepts_dyn(&fake);
    accepts_dyn_mut(&mut fake);

    assert!(fake.transition_blocked());
    assert_eq!(fake.branch_current_leaf_hint().as_deref(), Some("entry-42"));
    let branch = fake.branch_to("entry-43").expect("branch ok");
    assert_eq!(branch.leaf_display, "entry-43");
    assert_eq!(branch.journal_entries_before, 7);
    match fake.tree_body().expect("tree ok") {
        TreeBodyProjection::Journal { rendered } => assert_eq!(rendered, "rendered journal"),
        other => panic!("expected Journal projection, got {other:?}"),
    }
    let save = fake
        .save_session(Some("/tmp/session.json".to_string()))
        .expect("save ok");
    assert_eq!(save.display_path, "/tmp/session.json");
    assert_eq!(save.truncated_id, "abc123");
    let active = fake.fork_active().expect("active fork ok");
    assert_eq!(active.parent_label, "parent");
    assert_eq!(active.fork_label, "child");
    assert_eq!(active.sync.session_id.as_deref(), Some("child"));
    assert_eq!(active.sync.messages.len(), 1);
    assert_eq!(active.sync.mode, CommandMode::Plan);
    let explicit = fake.fork_from("source").expect("explicit fork ok");
    assert_eq!(explicit.spawn_depth, 3);
    assert_eq!(
        explicit.sync.workspace,
        PathBuf::from("/workspace/lifecycle")
    );
    let fresh = fake.fresh_session(true).expect("fresh ok");
    assert_eq!(fresh.truncated_id, "new-id");
    assert_eq!(fresh.sync.messages.len(), 1);
    let loaded = fake.load_session("loaded.json").expect("load ok");
    assert_eq!(loaded, PathBuf::from("/tmp/loaded.json"));
    fake.open_picker(Some("arch-1".to_string()));
    assert_eq!(fake.picker.as_deref(), Some("arch-1"));
    let archived = fake.set_archived("arch-1", true).expect("archive ok");
    assert_eq!(archived.truncated_id, "arch-1");
    assert_eq!(archived.title, "Archive Title");
    assert_eq!(fake.prune_sessions(30).expect("prune ok"), 3);
}

#[test]
fn lifecycle_error_text_and_empty_states_transport_exactly() {
    let mut fake = FakeLifecycle {
        branch_error: Some("could not load session x: boom".to_string()),
        tree: Some(Err("could not open sessions directory: boom".to_string())),
        save: Some(Err("Failed to save session: boom".to_string())),
        load: Some(Err("Failed to read session file: boom".to_string())),
        archived: Some(Err("archive failed: boom".to_string())),
        prune: Some(Err("prune failed: boom".to_string())),
        ..FakeLifecycle::default()
    };
    assert_eq!(
        fake.branch_to("x").unwrap_err(),
        "could not load session x: boom"
    );
    assert_eq!(
        fake.tree_body().unwrap_err(),
        "could not open sessions directory: boom"
    );
    assert_eq!(
        fake.save_session(None).unwrap_err(),
        "Failed to save session: boom"
    );
    assert_eq!(
        fake.load_session("missing.json").unwrap_err(),
        "Failed to read session file: boom"
    );
    assert_eq!(
        fake.set_archived("a", false).unwrap_err(),
        "archive failed: boom"
    );
    assert_eq!(fake.prune_sessions(7).unwrap_err(), "prune failed: boom");

    let mut empty = FakeLifecycle::default();
    assert!(!empty.transition_blocked());
    assert_eq!(empty.branch_current_leaf_hint(), None);
    assert!(matches!(
        empty.tree_body().expect("default tree"),
        TreeBodyProjection::NoSession
    ));
    empty.open_picker(None);
    assert_eq!(empty.picker, None);
}

#[test]
fn envelope_lifecycle_slot_is_independent_and_rejects_duplicates() {
    let mut first = FakeLifecycle::default();
    let mut second = FakeLifecycle::default();

    let parts = CommandContexts::empty()
        .with_lifecycle(&mut first)
        .into_parts();
    assert!(
        parts.lifecycle.is_some(),
        "lifecycle slot must be present when declared"
    );
    assert!(
        parts.session.is_none() && parts.plugin.is_none() && parts.skill_group.is_none(),
        "unrelated slots must stay absent (exact exposure)"
    );

    let bare = CommandContexts::empty().into_parts();
    assert!(
        bare.lifecycle.is_none(),
        "undeclared lifecycle stays absent"
    );

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        CommandContexts::empty()
            .with_lifecycle(&mut first)
            .with_lifecycle(&mut second);
    }));
    assert!(
        result.is_err(),
        "duplicate lifecycle slot must assert deterministically"
    );

    // Reading through the dyn facet works after insertion.
    first.blocked = true;
    let inserted = CommandContexts::empty().with_lifecycle(&mut first);
    let lifecycle = inserted.into_parts().lifecycle.expect("inserted lifecycle");
    assert!(lifecycle.transition_blocked());
}

// ---------------------------------------------------------------------------
// FEAT-024: session control contract (D2/D3/D6/D7).
// ---------------------------------------------------------------------------

#[test]
fn control_capability_is_stable_distinct_and_non_conflicting() {
    let control = CommandCapabilities::SESSION_CONTROL;
    for existing in [
        CommandCapabilities::NONE,
        CommandCapabilities::SESSION,
        CommandCapabilities::MODEL,
        CommandCapabilities::COST,
        CommandCapabilities::MODE_POLICY,
        CommandCapabilities::SYSTEM_PROMPT,
        CommandCapabilities::SKILLS,
        CommandCapabilities::WORKSPACE,
        CommandCapabilities::PRESENTATION,
        CommandCapabilities::MEDIA,
        CommandCapabilities::MEMORY,
        CommandCapabilities::PROJECT,
        CommandCapabilities::SKILL_GROUP,
        CommandCapabilities::PLUGIN,
        CommandCapabilities::SESSION_LIFECYCLE,
    ] {
        assert_ne!(control, existing, "SESSION_CONTROL must not collide");
    }
    assert!(!CommandCapabilities::NONE.contains(control));
    assert!(control.contains(control));
    assert!(
        control
            .union(CommandCapabilities::PRESENTATION)
            .contains(control)
    );
    assert!(
        control
            .union(CommandCapabilities::PRESENTATION)
            .contains(CommandCapabilities::PRESENTATION)
    );
    assert!(!CommandCapabilities::SESSION_LIFECYCLE.contains(control));
    assert!(!control.contains(CommandCapabilities::SESSION_LIFECYCLE));
    // Storage remains u16-backed by construction: bit 14 (1 << 14 = 16384)
    // fits the backing `u16` without the speculative widening FEAT-023's
    // maintainer review ruled out.
}

/// Deterministic fake control facet: every delegate returns canned portable
/// values or error text so the contract transport is exercised exactly.
#[derive(Default)]
struct FakeControl {
    blocked: bool,
    relay: Option<RelayProjection>,
    resume: Option<Result<ResumeSource, String>>,
    import: Option<Result<ResumeImportReceipt, String>>,
    sanitized_title: Option<String>,
    rename: Option<Result<SessionTitleReceipt, String>>,
    title_report: Option<TitleReport>,
    set_title: Option<Result<(), String>>,
    clear_title: Option<Result<(), String>>,
    remote_status: Option<String>,
    remote_link: Option<Option<RemoteLink>>,
    browser_open: Option<RemoteOpenOutcome>,
    start_info: Option<RemoteStartInfo>,
    stop_refusal: Option<Option<String>>,
    hosted: Option<Option<HostedWorkTarget>>,
}

impl CommandSessionControlContext for FakeControl {
    fn transition_blocked(&self) -> bool {
        self.blocked
    }
    fn relay_projection(&self) -> RelayProjection {
        self.relay
            .clone()
            .expect("unexpected relay_projection() on empty fake")
    }
    fn open_resume_picker(&mut self) {}
    fn resolve_resume_source(&mut self, raw: &str) -> Result<ResumeSource, String> {
        self.resume.clone().unwrap_or_else(|| {
            Err(format!(
                "unexpected resolve_resume_source({raw}) on empty fake"
            ))
        })
    }
    fn import_session_file(&mut self, path: PathBuf) -> Result<ResumeImportReceipt, String> {
        self.import.clone().unwrap_or_else(|| {
            Err(format!(
                "unexpected import_session_file({path:?}) on empty fake"
            ))
        })
    }
    fn sanitize_session_title(&self, raw: &str) -> String {
        self.sanitized_title
            .clone()
            .unwrap_or_else(|| raw.to_string())
    }
    fn rename_session(&mut self, title: &str) -> Result<SessionTitleReceipt, String> {
        self.rename
            .clone()
            .unwrap_or_else(|| Err(format!("unexpected rename_session({title}) on empty fake")))
    }
    fn title_report(&self) -> TitleReport {
        self.title_report
            .clone()
            .expect("unexpected title_report() on empty fake")
    }
    fn set_window_title(&mut self, title: String) -> Result<(), String> {
        self.set_title.clone().unwrap_or_else(|| {
            Err(format!(
                "unexpected set_window_title({title}) on empty fake"
            ))
        })
    }
    fn clear_window_title(&mut self) -> Result<(), String> {
        self.clear_title
            .clone()
            .unwrap_or_else(|| Err("unexpected clear_window_title() on empty fake".to_string()))
    }
    fn remote_status(&self) -> String {
        self.remote_status
            .clone()
            .expect("unexpected remote_status() on empty fake")
    }
    fn remote_link(&self) -> Option<RemoteLink> {
        self.remote_link
            .clone()
            .expect("unexpected remote_link() on empty fake")
    }
    fn remote_browser_open(&self) -> RemoteOpenOutcome {
        self.browser_open
            .clone()
            .expect("unexpected remote_browser_open() on empty fake")
    }
    fn remote_start_info(&self) -> RemoteStartInfo {
        self.start_info
            .clone()
            .expect("unexpected remote_start_info() on empty fake")
    }
    fn remote_stop_refusal(&self) -> Option<String> {
        self.stop_refusal
            .clone()
            .expect("unexpected remote_stop_refusal() on empty fake")
    }
    fn resolve_hosted_work_target(&self) -> Option<HostedWorkTarget> {
        self.hosted
            .clone()
            .expect("unexpected resolve_hosted_work_target() on empty fake")
    }
}

fn control_relay_projection() -> RelayProjection {
    RelayProjection {
        compact_template: "# Session relay".to_string(),
        workspace: "/workspace/control".to_string(),
        mode: "operate".to_string(),
        model: "control-model".to_string(),
        goal_objective: Some("ship the slice".to_string()),
        goal_token_budget: Some(42_000),
        todos: TodoProjection::Body("- [ ] port relay".to_string()),
        plan: PlanProjection::Sections(PlanSections {
            title: Some("Plan title".to_string()),
            items: vec![PlanStep {
                status: PlanStepStatus::InProgress,
                text: "port the control slice".to_string(),
            }],
            ..PlanSections::default()
        }),
    }
}

#[test]
fn control_facet_is_object_safe_and_transports_every_outcome() {
    // Object safety: usable behind a single `dyn` reference.
    fn accepts_dyn(_: &dyn CommandSessionControlContext) {}
    fn accepts_dyn_mut(_: &mut dyn CommandSessionControlContext) {}

    let mut fake = FakeControl {
        blocked: true,
        relay: Some(control_relay_projection()),
        resume: Some(Ok(ResumeSource::Session {
            load_path: Some(PathBuf::from("/tmp/sessions/abc123.json")),
            truncated_id: "abc123".to_string(),
            title: "Control Session".to_string(),
        })),
        import: Some(Ok(ResumeImportReceipt {
            truncated_id: "imp-9".to_string(),
            entry_count: 12,
            leaf_display: "leaf-3".to_string(),
        })),
        sanitized_title: Some("Renamed".to_string()),
        rename: Some(Ok(SessionTitleReceipt {
            title: "Renamed".to_string(),
        })),
        title_report: Some(TitleReport {
            effective: "task-7".to_string(),
            source: TitleSource::Session,
        }),
        set_title: Some(Ok(())),
        clear_title: Some(Ok(())),
        remote_status: Some("live".to_string()),
        remote_link: Some(Some(RemoteLink {
            url: "https://remote.example/s".to_string(),
            computer_url: Some("https://remote.example/c".to_string()),
        })),
        browser_open: Some(RemoteOpenOutcome::Opened {
            url: "https://remote.example/s".to_string(),
        }),
        start_info: Some(RemoteStartInfo { connecting: true }),
        stop_refusal: Some(None),
        hosted: Some(Some(HostedWorkTarget {
            url: "https://app.codewhale.net/work?repo=A%2FB".to_string(),
            repo: "A/B".to_string(),
            branch: "main".to_string(),
        })),
    };
    accepts_dyn(&fake);
    accepts_dyn_mut(&mut fake);

    assert!(fake.transition_blocked());
    let relay = fake.relay_projection();
    assert_eq!(relay.model, "control-model");
    assert_eq!(relay.goal_token_budget, Some(42_000));
    assert!(matches!(relay.todos, TodoProjection::Body(_)));
    match relay.plan {
        PlanProjection::Sections(sections) => {
            assert_eq!(sections.title.as_deref(), Some("Plan title"));
            assert_eq!(sections.items.len(), 1);
            assert_eq!(sections.items[0].status, PlanStepStatus::InProgress);
        }
        other => panic!("expected Sections plan, got {other:?}"),
    }
    let resolved = fake
        .resolve_resume_source("abc123")
        .expect("resume resolution ok");
    match resolved {
        ResumeSource::Session {
            load_path, title, ..
        } => {
            assert_eq!(load_path, Some(PathBuf::from("/tmp/sessions/abc123.json")));
            assert_eq!(title, "Control Session");
        }
        other => panic!("expected Session resolution, got {other:?}"),
    }
    let imported = fake
        .import_session_file(PathBuf::from("/tmp/import.json"))
        .expect("import ok");
    assert_eq!(imported.truncated_id, "imp-9");
    assert_eq!(imported.entry_count, 12);
    assert_eq!(imported.leaf_display, "leaf-3");
    assert_eq!(fake.sanitize_session_title("raw"), "Renamed");
    let renamed = fake.rename_session("Renamed").expect("rename ok");
    assert_eq!(renamed.title, "Renamed");
    let report = fake.title_report();
    assert_eq!(report.effective, "task-7");
    assert!(matches!(report.source, TitleSource::Session));
    fake.set_window_title("task-7".to_string()).expect("set ok");
    fake.clear_window_title().expect("clear ok");
    assert_eq!(fake.remote_status(), "live");
    let link = fake.remote_link().expect("link present");
    assert_eq!(link.url, "https://remote.example/s");
    assert!(matches!(
        fake.remote_browser_open(),
        RemoteOpenOutcome::Opened { .. }
    ));
    assert!(fake.remote_start_info().connecting);
    assert_eq!(fake.remote_stop_refusal(), None);
    let hosted = fake.resolve_hosted_work_target().expect("target present");
    assert_eq!(hosted.repo, "A/B");
    assert_eq!(hosted.branch, "main");
}

#[test]
fn control_error_and_empty_states_transport_exactly() {
    let mut fake = FakeControl {
        blocked: false,
        resume: Some(Err("could not open sessions directory: boom".to_string())),
        import: Some(Err(
            "File x.json is not a recognized session export".to_string()
        )),
        rename: Some(Err("Could not save session: boom".to_string())),
        set_title: Some(Err("Could not save session: boom".to_string())),
        clear_title: Some(Err("Could not save session: boom".to_string())),
        remote_link: Some(None),
        browser_open: Some(RemoteOpenOutcome::NoLink),
        stop_refusal: Some(Some(
            "stop refused while a remote turn is active".to_string(),
        )),
        hosted: Some(None),
        ..FakeControl::default()
    };
    assert!(!fake.transition_blocked());
    assert_eq!(
        fake.resolve_resume_source("x").unwrap_err(),
        "could not open sessions directory: boom"
    );
    assert_eq!(
        fake.import_session_file(PathBuf::from("x.json"))
            .unwrap_err(),
        "File x.json is not a recognized session export"
    );
    assert_eq!(
        fake.rename_session("t").unwrap_err(),
        "Could not save session: boom"
    );
    assert_eq!(
        fake.set_window_title("task".to_string()).unwrap_err(),
        "Could not save session: boom"
    );
    assert_eq!(
        fake.clear_window_title().unwrap_err(),
        "Could not save session: boom"
    );
    assert_eq!(fake.remote_link(), None);
    assert!(matches!(
        fake.remote_browser_open(),
        RemoteOpenOutcome::NoLink
    ));
    assert_eq!(
        fake.remote_stop_refusal().as_deref(),
        Some("stop refused while a remote turn is active")
    );
    assert_eq!(fake.resolve_hosted_work_target(), None);

    // Empty-state variants: absent to-do/plan and no effective title transport.
    fake.relay = Some(RelayProjection {
        todos: TodoProjection::Absent,
        plan: PlanProjection::Absent,
        ..control_relay_projection()
    });
    let relay = fake.relay_projection();
    assert!(matches!(relay.todos, TodoProjection::Absent));
    assert!(matches!(relay.plan, PlanProjection::Absent));
    fake.title_report = Some(TitleReport {
        effective: "unset".to_string(),
        source: TitleSource::None,
    });
    assert!(matches!(fake.title_report().source, TitleSource::None));
    fake.clear_title = Some(Ok(()));
    fake.clear_window_title().expect("cleared");
}

#[test]
fn envelope_control_slot_is_independent_and_rejects_duplicates() {
    let mut first = FakeControl::default();
    let mut second = FakeControl::default();
    let mut lifecycle = FakeLifecycle::default();

    let parts = CommandContexts::empty()
        .with_control(&mut first)
        .with_lifecycle(&mut lifecycle)
        .into_parts();
    assert!(
        parts.control.is_some(),
        "control slot must be present when declared"
    );
    assert!(
        parts.lifecycle.is_some(),
        "lifecycle slot may coexist with control"
    );
    assert!(
        parts.session.is_none()
            && parts.plugin.is_none()
            && parts.skill_group.is_none()
            && parts.presentation.is_none(),
        "unrelated slots must stay absent (exact exposure)"
    );

    let bare = CommandContexts::empty().into_parts();
    assert!(bare.control.is_none(), "undeclared control stays absent");

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        CommandContexts::empty()
            .with_control(&mut first)
            .with_control(&mut second);
    }));
    assert!(
        result.is_err(),
        "duplicate control slot must assert deterministically"
    );

    // Reading through the dyn facet works after insertion.
    first.blocked = true;
    let inserted = CommandContexts::empty().with_control(&mut first);
    let control = inserted.into_parts().control.expect("inserted control");
    assert!(control.transition_blocked());
}

#[test]
fn control_surface_does_not_widen_session_or_lifecycle_facets() {
    // The basic session and lifecycle facets still expose exactly their own
    // method surface alongside the new control slot: all three may populate an
    // envelope at once without colliding, and control does not add behavior to
    // the existing facets.
    let mut session = Session;
    let mut lifecycle = FakeLifecycle::default();
    let mut control = FakeControl {
        blocked: true,
        ..FakeControl::default()
    };

    let mut parts = CommandContexts::empty()
        .with_session(&mut session)
        .with_lifecycle(&mut lifecycle)
        .with_control(&mut control)
        .into_parts();
    assert_eq!(
        parts.session.as_deref().unwrap().session_id().as_deref(),
        Some("session")
    );
    assert!(!parts.lifecycle.as_deref_mut().unwrap().transition_blocked());
    assert!(parts.control.as_deref_mut().unwrap().transition_blocked());
}

// ---------------------------------------------------------------------------
// FEAT-025: session export contract (D1/D3/D5/D6/D7/D8/D9).
// ---------------------------------------------------------------------------

#[test]
fn export_capability_is_stable_distinct_and_non_conflicting() {
    let export = CommandCapabilities::SESSION_EXPORT;
    let existing = [
        CommandCapabilities::SESSION,
        CommandCapabilities::MODEL,
        CommandCapabilities::COST,
        CommandCapabilities::MODE_POLICY,
        CommandCapabilities::SYSTEM_PROMPT,
        CommandCapabilities::SKILLS,
        CommandCapabilities::WORKSPACE,
        CommandCapabilities::PRESENTATION,
        CommandCapabilities::MEDIA,
        CommandCapabilities::MEMORY,
        CommandCapabilities::PROJECT,
        CommandCapabilities::SKILL_GROUP,
        CommandCapabilities::PLUGIN,
        CommandCapabilities::SESSION_LIFECYCLE,
        CommandCapabilities::SESSION_CONTROL,
    ];
    let mut union = CommandCapabilities::NONE;
    for capability in existing {
        assert_ne!(
            export, capability,
            "SESSION_EXPORT must not collide with an existing capability"
        );
        union = union.union(capability);
    }
    assert!(
        !union.contains(export),
        "SESSION_EXPORT must be a bit outside every existing capability (bits 0-14)"
    );
    assert!(!CommandCapabilities::NONE.contains(export));
    assert!(!CommandCapabilities::NONE.contains(CommandCapabilities::NONE));
    assert!(export.contains(export));
    assert!(
        export
            .union(CommandCapabilities::SESSION_CONTROL)
            .contains(export)
    );
    assert!(
        export
            .union(CommandCapabilities::SESSION_CONTROL)
            .contains(CommandCapabilities::SESSION_CONTROL)
    );
    assert!(!CommandCapabilities::SESSION_CONTROL.contains(export));
    assert!(!export.contains(CommandCapabilities::SESSION_CONTROL));
    // Storage remains `u16`-backed: bit 15 (1 << 15 = 32768) fits without the
    // speculative widening FEAT-023's maintainer review ruled out.
    assert_eq!(
        std::mem::size_of::<CommandCapabilities>(),
        std::mem::size_of::<u16>(),
        "CommandCapabilities storage must stay u16"
    );
}

/// Canary: after FEAT-025 the `u16` capability space is *exactly* full.
///
/// This is deliberate capacity documentation, not a health check. When FEAT-026
/// (session structcopy) adds its own facet it must widen the backing storage to
/// `u32`, and this test is expected to be updated in that commit. Until then it
/// guarantees that no capability bit is silently reused, and that anyone who
/// adds a seventeenth capability is told why `1 << 16` on a `u16` will not do.
#[test]
fn export_capability_space_is_exactly_full() {
    let all = [
        CommandCapabilities::SESSION,
        CommandCapabilities::MODEL,
        CommandCapabilities::COST,
        CommandCapabilities::MODE_POLICY,
        CommandCapabilities::SYSTEM_PROMPT,
        CommandCapabilities::SKILLS,
        CommandCapabilities::WORKSPACE,
        CommandCapabilities::PRESENTATION,
        CommandCapabilities::MEDIA,
        CommandCapabilities::MEMORY,
        CommandCapabilities::PROJECT,
        CommandCapabilities::SKILL_GROUP,
        CommandCapabilities::PLUGIN,
        CommandCapabilities::SESSION_LIFECYCLE,
        CommandCapabilities::SESSION_CONTROL,
        CommandCapabilities::SESSION_EXPORT,
    ];

    let mut union = CommandCapabilities::NONE;
    for (index, capability) in all.iter().enumerate() {
        assert_eq!(
            capability.bits_for_test(),
            1u16 << index,
            "capability {index} must occupy exactly bit {index}"
        );
        union = union.union(*capability);
    }

    assert_eq!(
        all.len(),
        u16::BITS as usize,
        "the declared capability count must consume the whole u16 space"
    );
    assert_eq!(
        union.bits_for_test(),
        u16::MAX,
        "bits 0-15 are fully allocated; FEAT-026 must widen the storage to u32"
    );
}

/// Deterministic fake export facet: every delegate returns canned portable
/// values or host error text, and effectful delegates record their calls so a
/// later phase can assert sequencing without a real host.
#[derive(Default)]
struct FakeExport {
    projection: Option<ConversationExportProjection>,
    turn: Option<TurnHandoffProjection>,
    terminal_paste: bool,
    recovery: Option<Option<PathBuf>>,
    clipboard: Option<Result<(), String>>,
    resolved: Option<Result<PathBuf, String>>,
    write: Option<Result<(), String>>,
    calls: RefCell<Vec<String>>,
}

impl CommandSessionExportContext for FakeExport {
    fn conversation_projection(&self) -> ConversationExportProjection {
        self.calls
            .borrow_mut()
            .push("conversation_projection".to_string());
        self.projection
            .clone()
            .expect("unexpected conversation_projection() on empty fake")
    }
    fn turn_handoff_projection(&self) -> TurnHandoffProjection {
        self.calls
            .borrow_mut()
            .push("turn_handoff_projection".to_string());
        self.turn
            .clone()
            .expect("unexpected turn_handoff_projection() on empty fake")
    }
    fn clipboard_requires_terminal_paste(&self) -> bool {
        self.calls
            .borrow_mut()
            .push("clipboard_requires_terminal_paste".to_string());
        self.terminal_paste
    }
    fn write_recovery_copy(&self, markdown: &str) -> Option<PathBuf> {
        self.calls
            .borrow_mut()
            .push(format!("write_recovery_copy:{markdown}"));
        self.recovery
            .clone()
            .expect("unexpected write_recovery_copy() on empty fake")
    }
    fn write_clipboard(&self, markdown: &str) -> Result<(), String> {
        self.calls
            .borrow_mut()
            .push(format!("write_clipboard:{markdown}"));
        self.clipboard
            .clone()
            .unwrap_or_else(|| Err("unexpected write_clipboard() on empty fake".to_string()))
    }
    fn resolve_export_path(&self, raw: &str) -> Result<PathBuf, String> {
        self.calls
            .borrow_mut()
            .push(format!("resolve_export_path:{raw}"));
        self.resolved.clone().unwrap_or_else(|| {
            Err(format!(
                "unexpected resolve_export_path({raw}) on empty fake"
            ))
        })
    }
    fn write_export_file(&self, path: &Path, contents: &[u8], force: bool) -> Result<(), String> {
        self.calls.borrow_mut().push(format!(
            "write_export_file:{}:{}:{force}",
            path.display(),
            contents.len()
        ));
        self.write
            .clone()
            .unwrap_or_else(|| Err("unexpected write_export_file() on empty fake".to_string()))
    }
}

fn export_metadata() -> ExportMetadata {
    ExportMetadata {
        session_label: "abc123".to_string(),
        provider: "deepseek".to_string(),
        model: "deepseek-chat".to_string(),
        mode: "ACT".to_string(),
        workspace_name: "workspace".to_string(),
        message_count: 2,
        exported_at_unix: 1_760_000_000,
    }
}

fn export_recorded_snapshot() -> RestoreSnapshot {
    RestoreSnapshot {
        id: "0123456789abcdef".to_string(),
        label: "pre-turn:3: fix parser".to_string(),
        timestamp_unix: 1_759_999_000,
        kind: "pre-turn".to_string(),
        sequence: Some(3),
        prompt_snippet: Some("fix parser".to_string()),
    }
}

#[test]
fn export_facet_is_object_safe_and_transports_every_outcome() {
    // Object safety: usable behind a single `dyn` reference.
    fn accepts_dyn(_: &dyn CommandSessionExportContext) {}
    fn accepts_dyn_mut(_: &mut dyn CommandSessionExportContext) {}

    let mut fake = FakeExport {
        projection: Some(ConversationExportProjection {
            metadata: export_metadata(),
            transcript: TranscriptProjection::Authoritative(vec![ExportMessage {
                is_user_role: false,
                role: "assistant".to_string(),
                prompt_snippet: Some("fix parser".to_string()),
                blocks: vec![
                    ExportBlock::Text {
                        text: "visible".to_string(),
                    },
                    ExportBlock::ImageReference {
                        url: "https://example.test/a.png".to_string(),
                    },
                    ExportBlock::ImageOmitted,
                    ExportBlock::InternalReasoning,
                    ExportBlock::ToolCall {
                        id: "tool-1".to_string(),
                        name: "read".to_string(),
                        caller: Some(ToolCallerProjection {
                            caller_type: "direct".to_string(),
                            tool_id: Some("caller-1".to_string()),
                        }),
                        input: serde_json::json!({"path": "a.txt"}),
                    },
                    ExportBlock::ToolResult {
                        tool_use_id: "tool-1".to_string(),
                        content: "ok".to_string(),
                        is_error: false,
                        structured: Some(serde_json::json!([{"type": "text", "text": "ok"}])),
                    },
                    ExportBlock::ServerToolCall {
                        id: "server-1".to_string(),
                        name: "web_search".to_string(),
                        input: serde_json::json!({"q": "rust"}),
                    },
                    ExportBlock::ToolSearchResult {
                        tool_use_id: "search-1".to_string(),
                        content: serde_json::json!({"results": []}),
                    },
                    ExportBlock::CodeExecutionResult {
                        tool_use_id: "code-1".to_string(),
                        content: serde_json::json!({"stdout": "hi"}),
                    },
                ],
            }]),
            restore_points: RestorePointProjection::Recorded {
                snapshots: vec![export_recorded_snapshot()],
            },
        }),
        turn: Some(TurnHandoffProjection {
            markdown: "# turn handoff".to_string(),
            workspace_path: "/workspace/example".to_string(),
        }),
        terminal_paste: true,
        recovery: Some(Some(PathBuf::from(
            "/home/u/.codewhale/exports/last-copy.md",
        ))),
        clipboard: Some(Ok(())),
        resolved: Some(Ok(PathBuf::from("/workspace/example/out.md"))),
        write: Some(Ok(())),
        ..FakeExport::default()
    };
    accepts_dyn(&fake);
    accepts_dyn_mut(&mut fake);

    let projection = fake.conversation_projection();
    assert_eq!(projection.metadata.session_label, "abc123");
    assert_eq!(projection.metadata.provider, "deepseek");
    assert_eq!(projection.metadata.model, "deepseek-chat");
    assert_eq!(projection.metadata.mode, "ACT");
    assert_eq!(projection.metadata.workspace_name, "workspace");
    assert_eq!(projection.metadata.message_count, 2);
    assert_eq!(projection.metadata.exported_at_unix, 1_760_000_000);
    let TranscriptProjection::Authoritative(messages) = projection.transcript else {
        panic!("expected authoritative transcript");
    };
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].role, "assistant");
    assert_eq!(messages[0].prompt_snippet.as_deref(), Some("fix parser"));
    assert_eq!(messages[0].blocks.len(), 9);
    let ExportBlock::ToolCall {
        caller: Some(caller),
        input,
        ..
    } = &messages[0].blocks[4]
    else {
        panic!("expected tool call with caller");
    };
    assert_eq!(caller.caller_type, "direct");
    assert_eq!(caller.tool_id.as_deref(), Some("caller-1"));
    assert_eq!(input["path"], "a.txt");
    let ExportBlock::ToolResult {
        is_error,
        structured,
        ..
    } = &messages[0].blocks[5]
    else {
        panic!("expected tool result");
    };
    assert!(!is_error);
    assert!(structured.is_some());
    let RestorePointProjection::Recorded { snapshots } = projection.restore_points else {
        panic!("expected recorded restore points");
    };
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].id, "0123456789abcdef");
    assert_eq!(snapshots[0].label, "pre-turn:3: fix parser");
    assert_eq!(snapshots[0].timestamp_unix, 1_759_999_000);
    assert_eq!(snapshots[0].kind, "pre-turn");
    assert_eq!(snapshots[0].sequence, Some(3));
    assert_eq!(snapshots[0].prompt_snippet.as_deref(), Some("fix parser"));

    let turn = fake.turn_handoff_projection();
    assert_eq!(turn.markdown, "# turn handoff");
    assert_eq!(turn.workspace_path, "/workspace/example");

    assert!(fake.clipboard_requires_terminal_paste());
    assert_eq!(
        fake.write_recovery_copy("# md"),
        Some(PathBuf::from("/home/u/.codewhale/exports/last-copy.md"))
    );
    assert!(fake.write_clipboard("# md").is_ok());
    assert_eq!(
        fake.resolve_export_path("out.md").expect("resolved"),
        PathBuf::from("/workspace/example/out.md")
    );
    assert!(
        fake.write_export_file(Path::new("/workspace/example/out.md"), b"# md", false)
            .is_ok()
    );
    // Effectful delegates were exercised exactly once each, in call order.
    let expected: Vec<String> = [
        "conversation_projection",
        "turn_handoff_projection",
        "clipboard_requires_terminal_paste",
        "write_recovery_copy:# md",
        "write_clipboard:# md",
        "resolve_export_path:out.md",
        "write_export_file:/workspace/example/out.md:4:false",
    ]
    .into_iter()
    .map(str::to_string)
    .collect();
    assert_eq!(fake.calls.borrow().as_slice(), expected.as_slice());
}

#[test]
fn export_error_and_empty_states_transport_exactly() {
    let fake = FakeExport {
        projection: Some(ConversationExportProjection {
            metadata: export_metadata(),
            transcript: TranscriptProjection::HistoryFallback(vec![
                HistoryEntry::Sanitized {
                    role: "user".to_string(),
                    body: "visible history".to_string(),
                },
                HistoryEntry::Literal {
                    role: "system".to_string(),
                    body: "[internal context omitted]".to_string(),
                },
            ]),
            restore_points: RestorePointProjection::Unreadable {
                reason: "permission denied".to_string(),
            },
        }),
        recovery: Some(None),
        clipboard: Some(Err("clipboard unavailable".to_string())),
        resolved: Some(Err("export paths may not contain `..`".to_string())),
        write: Some(Err("destination already exists".to_string())),
        ..FakeExport::default()
    };

    let projection = fake.conversation_projection();
    let TranscriptProjection::HistoryFallback(entries) = projection.transcript else {
        panic!("expected history fallback");
    };
    assert_eq!(entries.len(), 2);
    assert!(matches!(
        &entries[0],
        HistoryEntry::Sanitized { role, body }
            if role == "user" && body == "visible history"
    ));
    assert!(matches!(
        &entries[1],
        HistoryEntry::Literal { role, body }
            if role == "system" && body == "[internal context omitted]"
    ));
    let RestorePointProjection::Unreadable { reason } = projection.restore_points else {
        panic!("expected unreadable restore points");
    };
    assert_eq!(reason, "permission denied");

    assert!(!fake.clipboard_requires_terminal_paste());
    assert_eq!(fake.write_recovery_copy("# md"), None);
    assert_eq!(
        fake.write_clipboard("# md").unwrap_err(),
        "clipboard unavailable"
    );
    assert_eq!(
        fake.resolve_export_path("../out.md").unwrap_err(),
        "export paths may not contain `..`"
    );
    assert_eq!(
        fake.write_export_file(Path::new("/tmp/out.md"), b"x", false)
            .unwrap_err(),
        "destination already exists"
    );
}

#[test]
fn export_projection_distinguishes_restore_states() {
    let states = [
        RestorePointProjection::None,
        RestorePointProjection::Unreadable {
            reason: "boom".to_string(),
        },
        RestorePointProjection::Recorded { snapshots: vec![] },
        RestorePointProjection::Recorded {
            snapshots: vec![export_recorded_snapshot()],
        },
    ];
    assert!(matches!(&states[0], RestorePointProjection::None));
    assert!(matches!(
        &states[1],
        RestorePointProjection::Unreadable { reason } if reason == "boom"
    ));
    let RestorePointProjection::Recorded { snapshots } = &states[2] else {
        panic!("expected recorded state");
    };
    assert!(snapshots.is_empty(), "existing-but-empty stays distinct");
    let RestorePointProjection::Recorded { snapshots } = &states[3] else {
        panic!("expected recorded state");
    };
    assert_eq!(snapshots.len(), 1);
}

#[test]
fn export_projection_omission_markers_carry_no_hidden_payload() {
    // D9: the projection has no field for a reasoning body, reasoning
    // signature, or inline/local image payload. Omission markers are data-free
    // unit variants, so prohibited payloads cannot be transported even by
    // accident.
    let block = ExportBlock::InternalReasoning;
    let ExportBlock::InternalReasoning = block else {
        panic!("internal reasoning must be a payload-free marker");
    };
    let block = ExportBlock::ImageOmitted;
    let ExportBlock::ImageOmitted = block else {
        panic!("omitted image must be a payload-free marker");
    };

    const HIDDEN_REASONING: &str = "signed-thinking-secret-body";
    const HIDDEN_SIGNATURE: &str = "sig_1234567890";
    const HIDDEN_IMAGE: &str = "data:image/png;base64,QUJD";

    let projection = ConversationExportProjection {
        metadata: export_metadata(),
        transcript: TranscriptProjection::Authoritative(vec![ExportMessage {
            is_user_role: false,
            role: "assistant".to_string(),
            prompt_snippet: None,
            blocks: vec![ExportBlock::InternalReasoning, ExportBlock::ImageOmitted],
        }]),
        restore_points: RestorePointProjection::None,
    };
    let rendered = format!("{projection:?}");
    assert!(!rendered.contains(HIDDEN_REASONING));
    assert!(!rendered.contains(HIDDEN_SIGNATURE));
    assert!(!rendered.contains(HIDDEN_IMAGE));
    assert!(rendered.contains("InternalReasoning"));
    assert!(rendered.contains("ImageOmitted"));
}

#[test]
fn envelope_export_slot_is_independent_and_rejects_duplicates() {
    let mut first = FakeExport::default();
    let mut second = FakeExport::default();
    let mut control = FakeControl::default();

    let parts = CommandContexts::empty()
        .with_export(&mut first)
        .with_control(&mut control)
        .into_parts();
    assert!(
        parts.export.is_some(),
        "export slot must be present when declared"
    );
    assert!(
        parts.control.is_some(),
        "control slot may coexist with export"
    );
    assert!(
        parts.session.is_none()
            && parts.lifecycle.is_none()
            && parts.plugin.is_none()
            && parts.skill_group.is_none(),
        "unrelated slots must stay absent (exact exposure)"
    );

    let bare = CommandContexts::empty().into_parts();
    assert!(bare.export.is_none(), "undeclared export stays absent");

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        CommandContexts::empty()
            .with_export(&mut first)
            .with_export(&mut second);
    }));
    assert!(
        result.is_err(),
        "duplicate export slot must assert deterministically"
    );

    // Reading through the dyn facet works after insertion.
    let mut projection = FakeExport {
        terminal_paste: true,
        ..FakeExport::default()
    };
    let inserted = CommandContexts::empty().with_export(&mut projection);
    let export = inserted.into_parts().export.expect("inserted export");
    assert!(export.clipboard_requires_terminal_paste());
}
