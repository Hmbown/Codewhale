//! Semantic prompt-context shapes for the debug diagnostics slice.
//!
//! No prompt builder or formatter belongs in this module. The TUI host
//! constructs these values from its authoritative report builders; a shared
//! portable renderer consumes them, including from non-command callers.

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DebugPromptSourceMap {
    pub entries: Vec<DebugSourceEntry>,
    pub total_estimated_tokens: usize,
    pub active_context_estimated_tokens: usize,
    pub overflow_guard_estimated_tokens: Option<usize>,
    pub context_window_tokens: Option<u32>,
    pub context_window_source: Option<String>,
    pub budget_used_percent: Option<f64>,
    /// Host-owned pressure policy result; intentionally absent from the public JSON.
    #[serde(skip)]
    pub pressure_label: String,
    /// Verified provenance from the host route ladder; not a public JSON field.
    #[serde(skip)]
    pub context_window_verified: bool,
    pub generated_at: String,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DebugSourceEntry {
    pub source_kind: DebugSourceKind,
    pub label: String,
    pub source_path: Option<String>,
    pub activation_reason: DebugActivationReason,
    pub estimated_tokens: usize,
    pub counting_confidence: DebugCountingConfidence,
    pub authority_tier: Option<u8>,
    pub truncation_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DebugSourceKind {
    Constitution,
    UserConstitution,
    RepoConstitution,
    ProjectContext,
    ProjectContextWarning,
    ProjectContextPack,
    SkillsBlock,
    ContextManagement,
    CompactionRelayTemplate,
    RuntimePolicy,
    AuthorityRecap,
    EnvironmentBlock,
    UserMemory,
    SessionGoal,
    HandoffRelay,
    ToolSchemas,
    UserRequest,
    ConversationHistory,
    ToolResult,
    ModelProviderFact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DebugActivationReason {
    AlwaysOn,
    FilePresent,
    ConfigEnabled,
    RuntimeState,
    PerRequest,
    Omitted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DebugCountingConfidence {
    High,
    Approximate,
}

/// The prompt-json shape retains known/unknown state and full ordered tool
/// fields, not an opaque pre-rendered JSON string from the host.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DebugPromptContext {
    pub schema_version: u8,
    pub provider: String,
    pub model: String,
    pub system_prompt_state: String,
    pub tool_catalog_state: String,
    pub sections: Vec<DebugPromptContextSection>,
    pub tools: Vec<DebugPromptTool>,
    pub source_map: DebugPromptSourceMap,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DebugPromptContextSection {
    pub index: usize,
    pub block_type: String,
    pub cache_control: Option<DebugCacheControl>,
    pub estimated_tokens: usize,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DebugCacheControl {
    #[serde(rename = "type")]
    pub cache_type: String,
}

/// Only the schema field is arbitrary structured JSON, as on the live tool
/// catalog. Optional transport fields retain the core Tool's omission rules.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DebugPromptTool {
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub tool_type: Option<String>,
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_callers: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defer_loading: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_examples: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<DebugCacheControl>,
}
