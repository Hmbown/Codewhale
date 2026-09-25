//! `execute_tools` — Code Mode Phase 1: run a model-provided JavaScript program
//! that composes read-only tool calls through `tools.call(name, args)`.
//!
//! The program handles loops, branching, filtering, and data movement;
//! intermediate results stay in the VM and only the bounded return value plus
//! a host-owned receipt reach the model. The engine-side precedent is the
//! synthetic interpreter dispatch (`js_execution` / `code_execution`): this
//! tool is engine-injected, never registered, and dispatched in
//! `core::engine::tool_execution`.
//!
//! Authority stays entirely in Rust. Every nested call traverses the same
//! gates a direct call would — registry resolution, the parent turn's
//! deny-lists and authority envelope — plus the Phase-1 profile gates
//! (read-only, auto-approved). Approving the program never approves anything
//! the program might do: a nested call that needs approval aborts the program
//! with a receipt naming it. Nested calls do not take per-tool locks (the
//! program runs under its own exclusive lock instead); see the limitations.
//!
//! KV-cache effect: under the default Direct tool mode this tool is deferred,
//! not eager, so the session-pinned prefix is unchanged until the model
//! activates it via `tool_search` — activation is a declared
//! `change:tool_surface` transition, same as any other deferred tool. Under
//! CodeMode (`[features] code_mode`) it is eager instead; the flag is session
//! config, so the prefix stays stable within a session either way. Program
//! text and nested results live in append-only turn history, never in the
//! prefix.
//!
//! Known limitations (Phase 1):
//! - Read-only composition. Nested calls must satisfy `is_read_only_for` and
//!   resolve `ApprovalRequirement::Auto` (posture-independent: Auto tools run
//!   under every posture, including Never). Anything else aborts the program.
//! - No nested `agent`, `workflow`, `tool_search`, interpreter, or MCP calls,
//!   and no recursive `execute_tools`. Fan-out stays with `workflow`/`task()`.
//! - No approval suspension: a gated call aborts with a receipt instead of
//!   prompting. Per-call approval previews are Phase 2.
//! - Nested reads do not serialize against concurrent sibling top-level
//!   writes. Prefer running `execute_tools` alone in its block when a
//!   consistent snapshot matters.
//! - Rich content blocks (images) from nested results are dropped; text and
//!   JSON payloads pass through bounded. Each nested payload is a
//!   `{content, metadata}` envelope: content parsed as JSON when possible,
//!   metadata verbatim (continuation notices included) or null.
//! - Hidden from Plan mode and refused under a worker authority envelope,
//!   like the other execution surfaces.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde_json::{Value, json};
use tokio::sync::Semaphore;
use tokio::time::timeout;

use codewhale_models::Tool;
use codewhale_workflow_js::{
    BudgetSnapshot, DriverError, ProgressEvent, SpawnedTask, TaskRequest, ToolCallRequest,
    ToolCallResponse, ToolInvoker, WorkflowDriver, WorkflowRunCancel, WorkflowVm,
};

use crate::tools::registry::{ToolRegistry, enforce_tool_authority};
use crate::tools::spec::{
    ApprovalRequirement, ToolContext, ToolError, ToolResult, ToolSpec, required_str,
};

/// Tool name surfaced to the model. Dispatched alongside the synthetic
/// interpreter tools; see `core::engine::tool_execution`.
pub const EXECUTE_TOOLS_TOOL_NAME: &str = "execute_tools";

const EXECUTE_TOOLS_TOOL_TYPE: &str = "execute_tools_20260918";

/// Maximum program source accepted, in bytes.
const MAX_CODE_BYTES: usize = 64 * 1024;
/// Whole-run wall deadline. The watchdog drops the run future; the VM
/// thread then unwinds through the standard cancel cascade.
const RUN_DEADLINE_SECS: u64 = 30;
/// Maximum nested tool calls in flight at once, enforced host-side.
const MAX_CONCURRENT_CALLS: usize = 4;
/// Per nested-call result cap, in serialized bytes.
const PER_CALL_RESULT_CAP_BYTES: usize = 32 * 1024;
/// Model-visible return cap, in serialized bytes.
const RETURN_CAP_BYTES: usize = 16 * 1024;

/// Names refused before any other check, with a message that names the
/// supported alternative. Checked against the requested name; the read-only
/// and auto-approve gates below would refuse most of these anyway, but the
/// explicit list keeps the receipt diagnostic instead of puzzling.
const PROHIBITED_NESTED: &[&str] = &[
    EXECUTE_TOOLS_TOOL_NAME,
    "code_execution",
    "js_execution",
    "agent",
    "workflow",
    "tool_search",
];

/// Model-facing definition. `defer_loading` is decided by the catalog (this
/// name is not in the eager set, so it stays deferred); `allowed_callers`
/// mirrors the interpreter tools.
pub fn execute_tools_tool_definition() -> Tool {
    Tool {
        tool_type: Some(EXECUTE_TOOLS_TOOL_TYPE.to_string()),
        name: EXECUTE_TOOLS_TOOL_NAME.to_string(),
        description: "Execute a JavaScript program that composes read-only tool calls via \
             tools.call(name, args) and returns a bounded JSON result. Discover tool \
             names and schemas with tool_search BEFORE writing the program. Phase-1 \
             limits: nested calls must be read-only and auto-approved; writes, \
             shell, subagents, workflows, MCP tools, and nested execute_tools abort \
             the program with a receipt. At most 50 nested calls, 4 concurrent, \
             30s per run, 16 KiB returned. Intermediate results stay in the \
             program; return only what the next decision needs."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "code": {
                    "type": "string",
                    "description": "JavaScript program. The return value (or thrown error) becomes the result; use tools.call(name, argsObject) for tool calls."
                }
            },
            "required": ["code"]
        }),
        allowed_callers: Some(vec!["direct".to_string()]),
        defer_loading: Some(false),
        input_examples: None,
        strict: None,
        cache_control: None,
    }
}

/// One nested call, as recorded by the host — not the script. The receipt is
/// what makes "no failures found" distinguishable from "nothing ran".
#[derive(Debug, Clone, serde::Serialize)]
struct CallReceipt {
    tool: String,
    ok: bool,
    elapsed_ms: u64,
    bytes: usize,
    truncated: bool,
    note: Option<String>,
}

/// [`ToolInvoker`] over a snapshot of the parent turn's registry.
///
/// The snapshot (spec Arcs plus a cloned [`ToolContext`]) is taken at
/// dispatch so the invoker is `'static` for the VM thread. Deny-lists and
/// the authority envelope are re-enforced per call from the cloned context,
/// so a program never outranks the turn that launched it.
pub(crate) struct CodemodeInvoker {
    specs: Vec<Arc<dyn ToolSpec>>,
    context: ToolContext,
    semaphore: Arc<Semaphore>,
    receipts: Mutex<Vec<CallReceipt>>,
}

impl CodemodeInvoker {
    fn new(specs: Vec<Arc<dyn ToolSpec>>, context: ToolContext) -> Self {
        Self {
            specs,
            context,
            semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_CALLS)),
            receipts: Mutex::new(Vec::new()),
        }
    }

    fn record(&self, receipt: CallReceipt) {
        if let Ok(mut receipts) = self.receipts.lock() {
            receipts.push(receipt);
        }
    }

    fn drain(&self) -> Vec<CallReceipt> {
        self.receipts
            .lock()
            .map(|receipts| receipts.clone())
            .unwrap_or_default()
    }

    fn refused(&self, tool: &str, started: Instant, note: String) -> DriverError {
        self.record(CallReceipt {
            tool: tool.to_string(),
            ok: false,
            elapsed_ms: started.elapsed().as_millis() as u64,
            bytes: 0,
            truncated: false,
            note: Some(note.clone()),
        });
        DriverError::Rejected(note)
    }
}

#[async_trait]
impl ToolInvoker for CodemodeInvoker {
    async fn invoke(&self, request: ToolCallRequest) -> Result<ToolCallResponse, DriverError> {
        let _permit = self
            .semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| DriverError::Unavailable("code-mode run shut down".to_string()))?;
        let started = Instant::now();
        let name = request.tool.as_str();

        if PROHIBITED_NESTED.contains(&name) {
            return Err(self.refused(
                name,
                started,
                format!(
                    "`{name}` is not available inside execute_tools programs; use workflow/task() for fan-out and tool_search before writing the program"
                ),
            ));
        }
        if crate::mcp::McpPool::is_mcp_tool(name) {
            return Err(self.refused(
                name,
                started,
                "MCP tools are excluded from Phase-1 code mode".to_string(),
            ));
        }
        let Some(spec) = self.specs.iter().find(|spec| spec.name() == name) else {
            return Err(self.refused(
                name,
                started,
                format!("unknown tool `{name}`; discover names with tool_search before writing the program"),
            ));
        };
        if !spec.is_read_only_for(&request.input) {
            return Err(self.refused(
                name,
                started,
                format!("`{name}` can mutate; Phase-1 code mode executes read-only calls only"),
            ));
        }
        if spec.approval_requirement_for(&request.input) != ApprovalRequirement::Auto {
            return Err(self.refused(
                name,
                started,
                format!(
                    "`{name}` needs approval; Phase-1 code mode executes only auto-approved calls"
                ),
            ));
        }
        if let Err(err) = enforce_tool_authority(name, &request.input, spec.as_ref(), &self.context)
        {
            return Err(self.refused(name, started, err.to_string()));
        }

        match spec
            .execute_rich(request.input.clone(), &self.context)
            .await
        {
            Ok(rich) => {
                let result = rich.into_result();
                // Stable envelope: the tool's text content (parsed as JSON
                // when it is JSON) plus its structured metadata, so
                // continuation and truncation notices survive the bridge.
                let content = serde_json::from_str(&result.content)
                    .unwrap_or_else(|_| Value::String(result.content.clone()));
                let payload = json!({
                    "content": content,
                    "metadata": result.metadata.clone().unwrap_or(Value::Null),
                });
                let raw_len = payload.to_string().len();
                let (bounded, truncated) = bound_json(payload, PER_CALL_RESULT_CAP_BYTES);
                self.record(CallReceipt {
                    tool: name.to_string(),
                    ok: result.success,
                    elapsed_ms: started.elapsed().as_millis() as u64,
                    bytes: raw_len,
                    truncated,
                    note: None,
                });
                Ok(ToolCallResponse {
                    ok: result.success,
                    result: if result.success {
                        bounded
                    } else {
                        Value::String(result.content)
                    },
                })
            }
            Err(err) => {
                let message = err.to_string();
                // Validation-shaped failures mean nothing ran (admission);
                // execution failures ran and failed (agent kind via ok:false);
                // seam breaks are unavailable.
                match err {
                    ToolError::InvalidInput { .. }
                    | ToolError::MissingField { .. }
                    | ToolError::PathEscape { .. }
                    | ToolError::PermissionDenied { .. } => {
                        Err(self.refused(name, started, message))
                    }
                    ToolError::Timeout { .. }
                    | ToolError::Cancelled { .. }
                    | ToolError::NotAvailable { .. } => Err(DriverError::Unavailable(message)),
                    ToolError::ExecutionFailed { .. } => {
                        self.record(CallReceipt {
                            tool: name.to_string(),
                            ok: false,
                            elapsed_ms: started.elapsed().as_millis() as u64,
                            bytes: message.len(),
                            truncated: false,
                            note: None,
                        });
                        Ok(ToolCallResponse {
                            ok: false,
                            result: Value::String(message),
                        })
                    }
                }
            }
        }
    }
}

/// [`WorkflowDriver`] for code-mode runs: `task()` is refused (fan-out stays
/// with `workflow`), the token budget is unconstrained, and progress events
/// feed the run receipt.
pub(crate) struct CodemodeDriver {
    events: Mutex<Vec<ProgressEvent>>,
}

impl Default for CodemodeDriver {
    fn default() -> Self {
        Self {
            events: Mutex::new(Vec::new()),
        }
    }
}

impl CodemodeDriver {
    fn log_lines(&self) -> Vec<String> {
        self.events
            .lock()
            .map(|events| {
                events
                    .iter()
                    .filter_map(|event| match event {
                        ProgressEvent::Log { message } => Some(message.clone()),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[async_trait]
impl WorkflowDriver for CodemodeDriver {
    async fn spawn_task(&self, _request: TaskRequest) -> Result<SpawnedTask, DriverError> {
        Err(DriverError::Rejected(
            "task() is unavailable in execute_tools programs; tools.call() composes direct tool calls"
                .to_string(),
        ))
    }

    fn budget(&self) -> BudgetSnapshot {
        BudgetSnapshot {
            total: None,
            spent: 0,
        }
    }

    fn progress(&self, event: ProgressEvent) {
        if let Ok(mut events) = self.events.lock() {
            events.push(event);
        }
    }

    fn cancel_all(&self) {}
}

/// Bound a JSON value to `cap` serialized bytes, replacing oversize payloads
/// with a preview that stays valid JSON.
fn bound_json(value: Value, cap: usize) -> (Value, bool) {
    let raw = value.to_string();
    if raw.len() <= cap {
        return (value, false);
    }
    let preview: String = raw.chars().take(cap / 2).collect();
    (
        json!({
            "_truncated": true,
            "bytes": raw.len(),
            "preview": preview,
        }),
        true,
    )
}

fn receipt_payload(
    success: bool,
    body: Value,
    invoker: &CodemodeInvoker,
    driver: &CodemodeDriver,
) -> ToolResult {
    let calls = invoker.drain();
    let content = json!({
        "success": success,
        "body": body,
        "nested_calls": calls.len(),
        "calls": calls,
        "log": driver.log_lines(),
    })
    .to_string();
    ToolResult {
        content,
        success,
        metadata: None,
    }
}

/// Execute one `execute_tools` call: validate, run the program under the run
/// deadline, and return the bounded program value plus the host-owned
/// receipt. A script failure is a `success: false` payload, not a host
/// error — only VM and deadline failures are `Err`.
pub async fn execute_tools_tool(
    input: &Value,
    registry: &ToolRegistry,
    context: &ToolContext,
) -> Result<ToolResult, ToolError> {
    let code = required_str(input, "code")?;
    if code.trim().is_empty() {
        return Err(ToolError::missing_field("code"));
    }
    if code.len() > MAX_CODE_BYTES {
        return Err(ToolError::invalid_input(format!(
            "code exceeds {MAX_CODE_BYTES} bytes"
        )));
    }
    let invoker = Arc::new(CodemodeInvoker::new(registry.all(), context.clone()));
    let driver = Arc::new(CodemodeDriver::default());
    let outcome = timeout(
        Duration::from_secs(RUN_DEADLINE_SECS),
        WorkflowVm::new().run_tools_script(
            code,
            Value::Null,
            driver.clone(),
            invoker.clone(),
            WorkflowRunCancel::new(),
        ),
    )
    .await;
    let program_result = match outcome {
        Err(_) => {
            return Err(ToolError::Timeout {
                seconds: RUN_DEADLINE_SECS,
            });
        }
        Ok(Err(err)) => {
            return Ok(receipt_payload(
                false,
                json!({ "error": err.to_string() }),
                &invoker,
                &driver,
            ));
        }
        Ok(Ok(value)) => value,
    };
    let (bounded, truncated) = bound_json(program_result, RETURN_CAP_BYTES);
    Ok(receipt_payload(
        true,
        json!({ "return": bounded, "return_truncated": truncated }),
        &invoker,
        &driver,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_advertises_execute_tools_deferred_outside_plan() {
        use codewhale_config::AppMode;
        use std::collections::HashSet;
        let empty = HashSet::new();
        for mode in [AppMode::Agent, AppMode::Operate] {
            let mut catalog = Vec::new();
            crate::core::engine::tool_catalog::ensure_advanced_tooling(
                &mut catalog,
                mode,
                &empty,
                ToolMode::Direct,
            );
            let tool = catalog
                .iter()
                .find(|tool| tool.name == EXECUTE_TOOLS_TOOL_NAME)
                .unwrap_or_else(|| panic!("{mode:?} catalog must advertise execute_tools"));
            assert_eq!(tool.defer_loading, Some(true), "{mode:?} must defer it");
        }
        let mut catalog = Vec::new();
        crate::core::engine::tool_catalog::ensure_advanced_tooling(
            &mut catalog,
            AppMode::Plan,
            &empty,
            ToolMode::Direct,
        );
        assert!(
            catalog
                .iter()
                .all(|tool| tool.name != EXECUTE_TOOLS_TOOL_NAME)
        );
    }

    #[test]
    fn definition_is_deferred_direct_only() {
        let tool = execute_tools_tool_definition();
        assert_eq!(tool.name, EXECUTE_TOOLS_TOOL_NAME);
        assert_eq!(tool.tool_type.as_deref(), Some(EXECUTE_TOOLS_TOOL_TYPE));
        assert!(tool.description.contains("tools.call"));
        // Catalog decides deferral; the name is absent from the eager set,
        // so injection marks it deferred like the interpreter tools.
        assert!(
            !crate::core::engine::tool_catalog::DEFAULT_ACTIVE_NATIVE_TOOLS
                .contains(&tool.name.as_str())
        );
        assert_eq!(tool.allowed_callers, Some(vec!["direct".to_string()]));
    }

    #[test]
    fn bound_json_keeps_small_values_verbatim() {
        let (value, truncated) = bound_json(json!({"a": 1}), 1024);
        assert!(!truncated);
        assert_eq!(value, json!({"a": 1}));
    }

    #[test]
    fn bound_json_truncates_to_valid_json_with_preview() {
        let big = "x".repeat(100);
        let (value, truncated) = bound_json(json!({ "blob": big }), 64);
        assert!(truncated);
        assert_eq!(value["bytes"], json!(111));
        assert!(value["preview"].as_str().is_some());
    }

    use crate::core::engine::tool_catalog::ToolMode;
    use crate::tools::file_tool::{ReadTool, WriteTool};
    use crate::tools::registry::ToolRegistryBuilder;

    fn workspace_with_note() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let note = dir.path().join("note.txt");
        std::fs::write(&note, "alpha\nbeta\n").unwrap();
        (dir, note)
    }

    #[tokio::test]
    async fn nested_read_passes_gates_and_records_receipt() {
        let (_dir, note) = workspace_with_note();
        let context = ToolContext::new(note.parent().unwrap());
        let invoker = CodemodeInvoker::new(vec![Arc::new(ReadTool)], context);
        let response = invoker
            .invoke(ToolCallRequest {
                tool: "read".to_string(),
                input: json!({ "path": note.to_string_lossy() }),
            })
            .await
            .unwrap();
        assert!(response.ok);
        assert!(response.result.to_string().contains("alpha"));
        let receipts = invoker.drain();
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].tool, "read");
        assert!(receipts[0].ok);
    }

    #[tokio::test]
    async fn nested_write_is_refused_and_writes_nothing() {
        let (_dir, note) = workspace_with_note();
        let target = note.parent().unwrap().join("evil.txt");
        let context = ToolContext::new(note.parent().unwrap());
        let invoker = CodemodeInvoker::new(vec![Arc::new(WriteTool)], context);
        let err = invoker
            .invoke(ToolCallRequest {
                tool: "write".to_string(),
                input: json!({ "path": target.to_string_lossy(), "content": "x" }),
            })
            .await
            .unwrap_err();
        assert!(
            matches!(&err, DriverError::Rejected(message) if message.contains("read-only")),
            "unexpected: {err:?}"
        );
        assert!(!target.exists());
        assert_eq!(invoker.drain().len(), 1);
    }

    #[tokio::test]
    async fn prohibited_nested_names_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let context = ToolContext::new(dir.path());
        let invoker = CodemodeInvoker::new(vec![], context);
        for name in ["agent", "workflow", "execute_tools", "tool_search", "nope"] {
            let err = invoker
                .invoke(ToolCallRequest {
                    tool: name.to_string(),
                    input: json!({}),
                })
                .await
                .unwrap_err();
            assert!(matches!(err, DriverError::Rejected(_)), "{name}: {err:?}");
        }
    }

    #[tokio::test]
    async fn program_composes_nested_read_and_returns_receipt() {
        let (_dir, note) = workspace_with_note();
        let workspace = note.parent().unwrap().to_path_buf();
        let context = ToolContext::new(workspace.clone());
        let registry = ToolRegistryBuilder::new()
            .with_tool(Arc::new(ReadTool))
            .build(context.clone());
        let path = note.to_string_lossy().replace('\\', "\\\\");
        let code = format!(
            "const r = await tools.call('read', {{ path: '{path}' }}); return {{ hasAlpha: JSON.stringify(r).includes('alpha') }};"
        );
        let result = execute_tools_tool(&json!({ "code": code }), &registry, &context)
            .await
            .unwrap();
        assert!(result.success, "{}", result.content);
        let body: Value = serde_json::from_str(&result.content).unwrap();
        assert_eq!(body["nested_calls"], 1);
        assert_eq!(body["body"]["return"]["hasAlpha"], true);
    }

    #[tokio::test]
    async fn program_loads_skills_at_runtime_through_load_skill() {
        // Skills-as-tools composes with code mode: `load_skill` is
        // read-only and auto-approved, so a program can list and load
        // skills at runtime without widening its authority.
        // A configured skills dir, not a project root: project skills load
        // only in a trusted workspace, which this composition test is not
        // about.
        let dir = tempfile::tempdir().unwrap();
        let skills_root = dir.path().join("configured-skills");
        let skill_dir = skills_root.join("greet");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: greet\ndescription: Say hello\n---\n# Greet\nSay hello warmly.\n",
        )
        .unwrap();
        let context = ToolContext::new(dir.path()).with_skills_config(&skills_root, false);
        let registry = ToolRegistryBuilder::new()
            .with_tool(Arc::new(crate::tools::skill::LoadSkillTool))
            .build(context.clone());
        let code = "const list = await tools.call('load_skill', { name: 'list' }); \
             const body = await tools.call('load_skill', { name: 'greet' }); \
             return { listed: JSON.stringify(list).includes('greet'), \
             loaded: JSON.stringify(body).includes('warmly') };";
        let result = execute_tools_tool(&json!({ "code": code }), &registry, &context)
            .await
            .unwrap();
        assert!(result.success, "{}", result.content);
        let body: Value = serde_json::from_str(&result.content).unwrap();
        assert_eq!(body["nested_calls"], 2);
        assert_eq!(body["body"]["return"]["listed"], true);
        assert_eq!(body["body"]["return"]["loaded"], true);
    }
}
