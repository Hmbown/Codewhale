//! `/structcopy` command — human-only structural copy (#2033).
//!
//! Copies exactly one bounded, human-selected session object (one transcript
//! item, one tool call+result pair, the current plan snapshot, or one
//! existing Workflow run projection) as deterministic, versioned canonical
//! JSON with a top-level receipt. The default target is the clipboard; an
//! explicit `stdout` argument is the only text-view path.
//!
//! Contract:
//! - Human-only. This is a slash command, never a model-visible tool, event,
//!   or authority, and it writes nothing back into App/session/plan/workflow
//!   state (see the registry/catalog contract test).
//! - Read-only projection over existing state. Redaction reuses the shared
//!   sanitizer seams in `codewhale_sanitize::sanitize` (`redact_json` for
//!   values, `sanitize_text` for keys and status labels, which
//!   `redact_json` does not reach) plus a strict pass that strips URL
//!   userinfo/query/fragment entirely and folds the workspace and home
//!   prefixes to labels, removes other absolute paths, and handles generic
//!   authority URLs. The workflow object reuses the bounded
//!   `WorkflowRunSummary` projection.
//! - Hard caps on final encoded bytes, array items, string bytes, object key
//!   bytes, and nesting depth; grapheme-safe truncation; recursively sorted
//!   keys; exact full-tree original counts and exact retained counts in the
//!   receipt. If receipt metadata alone cannot fit the byte cap, the command
//!   fails closed and emits nothing.
//!
//! What this deliberately does **not** claim:
//! - It is not a general PII scrubber. Workspace/home paths retain a useful
//!   labelled suffix; other absolute POSIX, drive-letter, and UNC paths are
//!   replaced outright.
//! - Redaction is pattern-based (the shared sanitizer's private-key/bearer/
//!   JWT/URL/secret regexes plus this module's strict URL pass). A secret that
//!   matches none of those patterns and sits under a non-sensitive key is
//!   copied as-is.
//! - Delivery to the clipboard is not confirmed. Terminal-client transports
//!   (tmux / OSC 52) are queued on a background writer; the receipt says
//!   "queued", not "delivered".

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as FmtWrite;

use serde_json::{Value, json};
use unicode_segmentation::UnicodeSegmentation;

use codewhale_command_contract::facets::*;
use codewhale_command_contract::handler::{CommandCapabilities, CommandContexts, CommandHandler};
use codewhale_command_contract::metadata::{CommandInfo, RegisterCommand};
use codewhale_command_contract::outcome::StructcopyCommandResult as CommandResult;
use codewhale_sanitize::sanitize::{is_sensitive_key, redact_json, sanitize_text};

pub(in crate::commands) const COMMAND_INFO: CommandInfo = CommandInfo {
    name: "structcopy",
    aliases: &[],
    usage: "/structcopy <turn <n>|tool <call-id>|plan|workflow <run-id>> [stdout]",
    description_key: "cmd_structcopy_description",
};
pub(in crate::commands) const CAPABILITIES: CommandCapabilities =
    CommandCapabilities::SESSION_STRUCTCOPY.union(CommandCapabilities::PRESENTATION);

pub(in crate::commands) struct StructcopyCmd;
impl RegisterCommand<CommandResult> for StructcopyCmd {
    fn info() -> &'static CommandInfo {
        &COMMAND_INFO
    }
    fn handler() -> CommandHandler<CommandResult> {
        CommandHandler::Contextual {
            capabilities: CAPABILITIES,
            handler: execute_structcopy,
        }
    }
}

/// Versioned envelope identity carried in every receipt.
pub(in crate::commands) const SCHEMA_ID: &str = "codewhale/structcopy/v1";
/// Redaction contract label so consumers can tell which seams ran.
pub(in crate::commands) const REDACTION_CONTRACT: &str =
    "export-sanitize/v1+typed-markers/v1+strict-url/v2+path-redact/v2";
/// Marker substituted for subtrees cut by the depth cap. Structural markers
/// are inserted after bounding and are intentionally exempt from
/// `max_string_bytes`; they are still counted as retained bytes.
pub(in crate::commands) const DEPTH_OMISSION_MARKER: &str = "omitted:depth_cap";
/// Marker substituted for a URL token that cannot be parsed and therefore
/// cannot be proven free of userinfo/query/fragment. Fail closed.
pub(in crate::commands) const URL_OMISSION_MARKER: &str = "redacted:url";
/// Marker substituted for an absolute filesystem path outside the labelled
/// workspace/home roots. Paths are privacy-bearing even when they contain no
/// conventional secret token.
pub(in crate::commands) const PATH_OMISSION_MARKER: &str = "redacted:absolute_path";
pub(in crate::commands) const BEARER_REDACTION_MARKER: &str = "redacted:bearer";
pub(in crate::commands) const SENSITIVE_VALUE_REDACTION_MARKER: &str = "redacted:sensitive_value";

/// Selectors are echoed into the receipt and into status messages, so they
/// get their own tight cap independent of the payload string cap.
pub(in crate::commands) const MAX_SELECTOR_BYTES: usize = 256;
/// Hard caps enforced on every emitted artifact. The byte cap stays well
/// under the OSC 52 clipboard ceiling (100 KiB) so the default clipboard
/// target always fits its weakest transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::commands) struct Caps {
    pub(in crate::commands) max_output_bytes: usize,
    pub(in crate::commands) max_array_items: usize,
    pub(in crate::commands) max_string_bytes: usize,
    pub(in crate::commands) max_depth: usize,
}

pub(in crate::commands) const DEFAULT_CAPS: Caps = Caps {
    max_output_bytes: 48 * 1024,
    max_array_items: 64,
    max_string_bytes: 2 * 1024,
    max_depth: 12,
};

/// Object keys are bounded separately from values: they are short by nature,
/// they participate in collision handling, and they are rewritten once during
/// redaction rather than per byte-cap retry.
pub(in crate::commands) const MAX_KEY_BYTES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::commands) enum CopyKind {
    Turn(usize),
    Tool(String),
    Plan,
    Workflow(String),
}

impl CopyKind {
    fn display_label(
        &self,
        presentation: &dyn CommandPresentationContext,
    ) -> Result<String, String> {
        let key = match self {
            CopyKind::Turn(_) => "cmd_structcopy_kind_turn",
            CopyKind::Tool(_) => "cmd_structcopy_kind_tool",
            CopyKind::Plan => "cmd_structcopy_kind_plan",
            CopyKind::Workflow(_) => "cmd_structcopy_kind_workflow",
        };
        presentation.translate(key, &[])
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CopyRequest {
    kind: CopyKind,
    stdout: bool,
}

pub(in crate::commands) fn execute_structcopy(
    contexts: CommandContexts<'_>,
    arg: Option<&str>,
) -> CommandResult {
    let parts = contexts.into_parts();
    let Some(copy) = parts.structcopy else {
        return CommandResult::error("Command capability unavailable: session_structcopy");
    };
    let Some(presentation) = parts.presentation else {
        return CommandResult::error("Command capability unavailable: presentation");
    };
    execute_portable(copy, presentation, arg, &DEFAULT_CAPS).unwrap_or_else(CommandResult::error)
}

fn execute_portable(
    copy: &dyn CommandSessionStructcopyContext,
    presentation: &dyn CommandPresentationContext,
    arg: Option<&str>,
    caps: &Caps,
) -> Result<CommandResult, String> {
    let request = match parse_request(arg) {
        Ok(request) => request,
        Err(()) => {
            return Err(presentation.translate(
                "cmd_structcopy_usage_error",
                &[("usage", COMMAND_INFO.usage)],
            )?);
        }
    };
    let label = request.kind.display_label(presentation)?;
    let json = render_copy(copy, presentation, &request.kind, caps)?;
    if request.stdout {
        return Ok(CommandResult::message(json));
    }
    let bytes = json.len().to_string();
    match copy.write_clipboard(&json) {
        Ok(transport) => {
            let key = match transport {
                StructcopyTransport::Native => "cmd_structcopy_clipboard_accepted",
                StructcopyTransport::TerminalQueued => "cmd_structcopy_clipboard_queued",
            };
            Ok(CommandResult::message(
                presentation.translate(key, &[("kind", &label), ("bytes", &bytes)])?,
            ))
        }
        Err(error) => {
            Err(presentation.translate("cmd_structcopy_clipboard_failed", &[("error", &error)])?)
        }
    }
}

fn parse_request(arg: Option<&str>) -> Result<CopyRequest, ()> {
    let raw = arg.unwrap_or("").trim();
    if raw.is_empty() {
        return Err(());
    }
    let mut tokens: Vec<&str> = raw.split_whitespace().collect();
    let mut stdout = false;
    if tokens
        .last()
        .is_some_and(|last| last.eq_ignore_ascii_case("stdout"))
    {
        stdout = true;
        tokens.pop();
    }
    let kind = match tokens.as_slice() {
        ["plan"] => CopyKind::Plan,
        ["turn", index] => {
            let index = index
                .parse::<usize>()
                .ok()
                .filter(|index| *index >= 1)
                .ok_or(())?;
            CopyKind::Turn(index)
        }
        ["tool", call_id] => CopyKind::Tool((*call_id).to_string()),
        ["workflow", run_id] => CopyKind::Workflow((*run_id).to_string()),
        _ => return Err(()),
    };
    Ok(CopyRequest { kind, stdout })
}

// === Object selection (read-only; unavailable objects are reported, never
// fabricated) ===

fn build_payload(
    copy: &dyn CommandSessionStructcopyContext,
    kind: &CopyKind,
) -> Result<(&'static str, Value, Value), StructcopyError> {
    match kind {
        CopyKind::Turn(index) => Ok((
            "turn",
            json!(index),
            message_payload(&copy.transcript_item(*index)?),
        )),
        CopyKind::Tool(call_id) => {
            let pair = copy.tool_pair(call_id)?;
            let result = match pair.result {
                Some(result) => {
                    json!({"found":true,"is_error":optional_bool(result.is_error),"content":result.content,"content_blocks":result.content_blocks})
                }
                None => json!({"found":false}),
            };
            Ok((
                "tool",
                json!(call_id),
                json!({"call_id":call_id,"name":pair.name,"input":pair.input,"result":result}),
            ))
        }
        CopyKind::Plan => Ok((
            "plan",
            Value::Null,
            serde_json::to_value(copy.plan_snapshot()?)
                .map_err(|error| StructcopyError::Preparation(error.to_string()))?,
        )),
        CopyKind::Workflow(run_id) => Ok((
            "workflow",
            json!(run_id),
            serde_json::to_value(copy.workflow_projection(run_id)?)
                .map_err(|error| StructcopyError::Preparation(error.to_string()))?,
        )),
    }
}

fn message_payload(message: &StructcopyTranscript) -> Value {
    match &message.content {
        StructcopyContent::InternalContext => {
            json!({"index":message.index,"role":message.role,"omission_code":"internal_context"})
        }
        StructcopyContent::Visible(blocks) => {
            json!({"index":message.index,"role":message.role,"content":blocks.iter().map(block_payload).collect::<Vec<_>>()})
        }
    }
}

pub(in crate::commands) fn optional_bool(value: Option<bool>) -> Value {
    value.map_or(Value::Null, Value::Bool)
}

fn block_payload(block: &StructcopyBlock) -> Value {
    match block {
        StructcopyBlock::Text(text) => json!({"type":"text","text":text}),
        StructcopyBlock::ThinkingOmitted => {
            json!({"type":"thinking","omission_code":"internal_reasoning_and_signature"})
        }
        StructcopyBlock::ToolUse {
            id,
            name,
            input,
            caller_type,
        } => json!({"type":"tool_use","id":id,"caller_type":caller_type,"name":name,"input":input}),
        StructcopyBlock::ToolResult {
            tool_use_id,
            result,
        } => {
            json!({"type":"tool_result","tool_use_id":tool_use_id,"is_error":optional_bool(result.is_error),"content":result.content,"content_blocks":result.content_blocks})
        }
        StructcopyBlock::ImageUrl(url) => json!({"type":"image","url":url}),
        StructcopyBlock::ImageOmitted => {
            json!({"type":"image","omission_code":"inline_or_local_image_payload"})
        }
        StructcopyBlock::ServerToolUse { id, name, input } => {
            json!({"type":"server_tool_use","id":id,"name":name,"input":input})
        }
        StructcopyBlock::ToolSearchToolResult {
            tool_use_id,
            content,
        } => json!({"type":"tool_search_tool_result","tool_use_id":tool_use_id,"content":content}),
        StructcopyBlock::CodeExecutionToolResult {
            tool_use_id,
            content,
        } => {
            json!({"type":"code_execution_tool_result","tool_use_id":tool_use_id,"content":content})
        }
    }
}

fn selection_error(
    presentation: &dyn CommandPresentationContext,
    kind: &CopyKind,
    error: StructcopyError,
) -> Result<String, String> {
    let label = kind.display_label(presentation)?;
    match error {
        StructcopyError::Unavailable => {
            presentation.translate("cmd_structcopy_unavailable", &[("kind", &label)])
        }
        StructcopyError::Busy => presentation.translate("cmd_structcopy_busy", &[("kind", &label)]),
        StructcopyError::Preparation(error) => presentation.translate(
            "cmd_structcopy_prepare_failed",
            &[("kind", &label), ("error", &error)],
        ),
    }
}

// === Redaction (composed from existing central seams) ===

/// The strongest existing central redaction, applied before any bounding or
/// serialization and after key normalization.
///
/// [`redact_json`] replaces values under secret-shaped keys, and runs
/// [`sanitize_text`] over every string *value* — stripping ANSI/control
/// bytes and masking PEM blocks, `Bearer` tokens, JWTs, credential-bearing
/// URLs, and the config layer's known secret patterns. It does **not** touch
/// object *keys*, so this pass runs [`sanitize_text`] over keys as well,
/// then folds workspace/home prefixes to labels and strips URL
/// userinfo/query/fragment outright.
///
/// Keys are also sorted, bounded, and de-collided here. Original and retained
/// key counts are kept separately so omitted subtrees cannot inflate claims
/// about the emitted object.
fn redact_payload(value: &mut Value, labels: &PathLabels, keys: &mut KeyStats) {
    // Normalize keys first so ANSI/control obfuscation cannot hide a
    // sensitive-key hint from classification. `strict_strings` classifies
    // both the original and normalized key; the shared export pass then runs
    // over the normalized tree as defense in depth.
    let mut path = Vec::new();
    strict_strings(value, labels, keys, &mut path);
    redact_json(value, None);
    normalize_redaction_codes(value);
}

/// Prefix folding for useful filesystem paths. These prefixes are recognised:
/// the workspace root (both as configured and as canonicalized, which differ
/// on macOS where `/var` symlinks to `/private/var`) and `$HOME` /
/// `%USERPROFILE%`. The later strict pass removes every remaining absolute
/// POSIX, drive-letter, or UNC path.
pub(in crate::commands) struct PathLabels {
    /// `(prefix, label)` sorted longest-first so that a workspace nested
    /// inside `$HOME` folds to `<workspace>` rather than `<home>/…`.
    pub(in crate::commands) labels: Vec<(String, &'static str)>,
}

impl PathLabels {
    pub(in crate::commands) fn new(roots: &StructcopyPathRoots) -> Self {
        let mut workspace_forms: Vec<String> = Vec::new();
        for form in std::iter::once(&roots.workspace).chain(roots.canonical_workspace.iter()) {
            if form.len() > 1 && !workspace_forms.contains(form) {
                workspace_forms.push(form.clone());
            }
        }
        let mut labels: Vec<(String, &'static str)> = workspace_forms
            .iter()
            .map(|form| (form.clone(), "<workspace>"))
            .collect();
        if let Some(home) = &roots.home
            && home.len() > 3
            && !workspace_forms.contains(home)
        {
            labels.push((home.clone(), "<home>"));
        }
        labels.sort_by(|left, right| {
            right
                .0
                .len()
                .cmp(&left.0.len())
                .then_with(|| left.0.cmp(&right.0))
        });
        Self { labels }
    }

    pub(in crate::commands) fn apply(&self, text: &str) -> String {
        let mut out = text.to_string();
        for (prefix, label) in &self.labels {
            out = replace_path_root(&out, prefix, label);
        }
        out
    }
}

/// Replace a configured root only when it ends on a path-component boundary.
/// A lexical prefix such as `/opt/app` must not label `/opt/application`; the
/// latter remains foreign and is removed by the absolute-path scrubber.
fn replace_path_root(text: &str, root: &str, label: &str) -> String {
    if root.is_empty() {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0usize;
    while let Some(offset) = text[cursor..].find(root) {
        let start = cursor + offset;
        let end = start + root.len();
        out.push_str(&text[cursor..start]);
        let component_boundary = text[end..]
            .chars()
            .next()
            .is_none_or(|ch| matches!(ch, '/' | '\\'));
        if component_boundary {
            out.push_str(label);
        } else {
            out.push_str(root);
        }
        cursor = end;
    }
    out.push_str(&text[cursor..]);
    out
}

/// Per-object-key accounting. Computed once during redaction and reported in
/// the receipt so a renamed or truncated key is never silent.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct KeyStats {
    entries: BTreeMap<Vec<String>, KeyFlags>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct KeyFlags {
    truncated: bool,
    deduped: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct RetainedKeyStats {
    total: u64,
    truncated: u64,
    deduped: u64,
}

impl KeyStats {
    fn original_total(&self) -> u64 {
        u64::try_from(self.entries.len()).unwrap_or(u64::MAX)
    }
}

fn strict_strings(
    value: &mut Value,
    labels: &PathLabels,
    keys: &mut KeyStats,
    path: &mut Vec<String>,
) {
    match value {
        Value::String(text) => *text = scrub_string(text, labels),
        Value::Array(items) => {
            for (index, item) in items.iter_mut().enumerate() {
                path.push(format!("i:{index}"));
                strict_strings(item, labels, keys, path);
                path.pop();
            }
        }
        Value::Object(map) => {
            // Take the map, rewrite each key, and reinsert. Entries are
            // processed in sorted original-key order so collision suffixes
            // are assigned deterministically regardless of insertion order.
            let mut entries: Vec<(String, Value)> = std::mem::take(map).into_iter().collect();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            for (key, mut item) in entries {
                let scrubbed = flatten_ws(&scrub_string(&key, labels));
                let (bounded, was_truncated) =
                    truncate_string_grapheme_safe(&scrubbed, MAX_KEY_BYTES);
                let (unique, collision_truncated) = unique_object_key(map, &bounded);
                let sensitive = is_sensitive_key(&key) || is_sensitive_key(&scrubbed);
                if sensitive {
                    item = Value::String("[redacted]".to_string());
                } else {
                    path.push(key_path_segment(&unique));
                    strict_strings(&mut item, labels, keys, path);
                    path.pop();
                }
                path.push(key_path_segment(&unique));
                keys.entries.insert(
                    path.clone(),
                    KeyFlags {
                        truncated: was_truncated || collision_truncated,
                        deduped: unique != bounded,
                    },
                );
                path.pop();
                map.insert(unique, item);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn key_path_segment(key: &str) -> String {
    format!("k:{}:{key}", key.len())
}

fn collect_retained_key_stats(
    value: &Value,
    original: &KeyStats,
    path: &mut Vec<String>,
    retained: &mut RetainedKeyStats,
) {
    match value {
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                path.push(format!("i:{index}"));
                collect_retained_key_stats(item, original, path, retained);
                path.pop();
            }
        }
        Value::Object(map) => {
            for (key, item) in map {
                path.push(key_path_segment(key));
                if let Some(flags) = original.entries.get(path) {
                    retained.total += 1;
                    if flags.truncated {
                        retained.truncated += 1;
                    }
                    if flags.deduped {
                        retained.deduped += 1;
                    }
                }
                collect_retained_key_stats(item, original, path, retained);
                path.pop();
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

/// Deterministic collision handling for keys that collapsed onto each other
/// after scrubbing or truncation.
///
/// Termination is structural rather than hopeful: the numeric reserve is
/// sized for the largest suffix this call can produce, so `base` is fixed and
/// the `map.len() + 1` candidates `base~2 … base~(len+2)` are pairwise
/// distinct. A map holding `len` keys cannot occupy all of them.
///
/// When `MAX_KEY_BYTES` is smaller than the reserve the suffix still wins:
/// losing a key to a silent overwrite is worse than exceeding a key cap by a
/// few bytes, and the per-key flags record that it happened.
fn unique_object_key(map: &serde_json::Map<String, Value>, requested: &str) -> (String, bool) {
    if !map.contains_key(requested) {
        return (requested.to_string(), false);
    }
    let highest = map.len().saturating_add(2);
    let reserve = 1 + decimal_width(highest);
    let base_cap = MAX_KEY_BYTES.saturating_sub(reserve);
    let (base, collision_truncated) = truncate_string_grapheme_safe(requested, base_cap);
    for index in 2..=highest {
        let candidate = format!("{base}~{index}");
        if !map.contains_key(&candidate) {
            return (candidate, collision_truncated);
        }
    }
    unreachable!(
        "map of {} keys cannot occupy {} distinct candidates",
        map.len(),
        highest - 1
    )
}

fn decimal_width(mut value: usize) -> usize {
    let mut width = 1;
    while value >= 10 {
        value /= 10;
        width += 1;
    }
    width
}

/// Collapse every run of whitespace to a single space. Used for object keys,
/// where control layout is a structural hazard rather than data.
fn flatten_ws(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(in crate::commands) fn scrub_string(text: &str, labels: &PathLabels) -> String {
    // `sanitize_text` first: it strips ANSI and control bytes, so the URL
    // scan below cannot be fooled by an escape sequence spliced into a
    // scheme. It is idempotent, so re-running it over values that
    // `redact_json` already sanitized is safe.
    let sanitized = sanitize_text(text);
    let bearer_safe = redact_loose_bearers(&sanitized);
    let labelled = labels.apply(&bearer_safe);
    scrub_paths(&scrub_urls(&labelled))
}

/// Convert the prose placeholders owned by the shared sanitizer into stable
/// language-neutral codes. Structural JSON is a machine artifact and must not
/// change with the UI locale.
fn normalize_redaction_codes(value: &mut Value) {
    match value {
        Value::String(text) => {
            *text = text
                .replace("[redacted private key]", "redacted:private_key")
                .replace("Bearer [redacted]", BEARER_REDACTION_MARKER)
                .replace("[redacted token]", "redacted:token")
                .replace("[redacted]", SENSITIVE_VALUE_REDACTION_MARKER);
        }
        Value::Array(items) => {
            for item in items {
                normalize_redaction_codes(item);
            }
        }
        Value::Object(map) => {
            for item in map.values_mut() {
                normalize_redaction_codes(item);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn redact_loose_bearers(text: &str) -> String {
    // Selectors cannot carry the whitespace used by a conventional
    // `Bearer <token>` header. Delimiter variants are still secret-shaped;
    // redact their entire line tail so token punctuation cannot terminate a
    // regex early and expose the remainder.
    let lowered = text.to_ascii_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0usize;
    while let Some(offset) = ["bearer-", "bearer_", "bearer:", "bearer="]
        .iter()
        .filter_map(|prefix| lowered[cursor..].find(prefix))
        .min()
    {
        let start = cursor + offset;
        out.push_str(&text[cursor..start]);
        let end = text[start..]
            .find('\n')
            .map(|line_end| start + line_end)
            .unwrap_or(text.len());
        out.push_str(BEARER_REDACTION_MARKER);
        cursor = end;
    }
    out.push_str(&text[cursor..]);
    out
}

/// Trailing characters that are punctuation or wrappers around a URL rather
/// than part of it. Trimming generously is safe in both directions: the
/// trimmed tail is re-appended verbatim and can hold no credential, while a
/// tail left attached would be swallowed by the query/fragment strip.
const URL_TRAILING_PUNCTUATION: &[char] = &[
    '.', ',', ';', ':', '!', '?', ')', ']', '}', '>', '"', '\'', '`', '*', '_', '\\',
];

/// Strip URL userinfo, query, and fragment entirely, leaving a
/// `scheme://host[:port]/path` label.
///
/// The shared sanitizer has already masked credentials in URLs it recognised;
/// this pass enforces the stricter structural-copy contract that no
/// userinfo, query string, or fragment may survive at all — including for
/// URLs that are punctuation-wrapped (`(https://…)`, `<https://…>`,
/// `"https://…"`), embedded mid-token, or uppercased. A token that starts
/// with a syntactically valid `scheme://` prefix but does not parse is replaced outright rather than
/// passed through, because an unparseable URL cannot be proven credential
/// free.
fn scrub_urls(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0usize;
    while let Some(offset) = next_url_start(&text[cursor..]) {
        let start = cursor + offset;
        out.push_str(&text[cursor..start]);
        let rest = &text[start..];
        // A scheme prefix contains no whitespace, so `end` is always > 0 and
        // the cursor strictly advances.
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        out.push_str(&scrub_url_token(&rest[..end]));
        cursor = start + end;
    }
    out.push_str(&text[cursor..]);
    out
}

fn next_url_start(text: &str) -> Option<usize> {
    for (separator, _) in text.match_indices("://") {
        let before = &text[..separator];
        let start = before
            .char_indices()
            .rev()
            .take_while(|(_, ch)| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '-' | '.'))
            .map(|(index, _)| index)
            .last()
            .unwrap_or(separator);
        let scheme = &text[start..separator];
        if scheme
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphabetic())
        {
            return Some(start);
        }
    }
    None
}

fn scrub_url_token(token: &str) -> String {
    let trimmed = token.trim_end_matches(URL_TRAILING_PUNCTUATION);
    let suffix = &token[trimmed.len()..];
    let Ok(mut parsed) = url::Url::parse(trimmed) else {
        return format!("{URL_OMISSION_MARKER}{suffix}");
    };
    // `set_username`/`set_password` only fail for cannot-be-a-base URLs.
    // Failing closed keeps the "no userinfo survives" claim literally true.
    if parsed.set_username("").is_err() || parsed.set_password(None).is_err() {
        return format!("{URL_OMISSION_MARKER}{suffix}");
    }
    parsed.set_query(None);
    parsed.set_fragment(None);
    format!("{parsed}{suffix}")
}

fn scrub_paths(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0usize;
    while let Some(start) = next_absolute_path_start(text, cursor) {
        out.push_str(&text[cursor..start]);
        // An unquoted absolute path can legally contain spaces. Stop at the
        // line boundary rather than risk leaking the tail of such a path;
        // losing adjacent prose is safer than emitting a customer/user name.
        let end = text[start..]
            .find('\n')
            .map(|offset| start + offset)
            .unwrap_or(text.len());
        out.push_str(PATH_OMISSION_MARKER);
        cursor = end;
    }
    out.push_str(&text[cursor..]);
    out
}

fn next_absolute_path_start(text: &str, from: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut index = from;
    while index < bytes.len() {
        let boundary = index == 0
            || text[..index]
                .chars()
                .next_back()
                .is_some_and(|ch| !ch.is_alphanumeric() && !matches!(ch, '_' | '/' | '\\'));
        if boundary {
            let labelled_root =
                text[..index].ends_with("<workspace>") || text[..index].ends_with("<home>");
            let url_separator = index > 0
                && index + 1 < bytes.len()
                && bytes[index - 1] == b':'
                && bytes[index + 1] == b'/';
            let previous_is_slash = index > 0 && bytes[index - 1] == b'/';
            let posix =
                bytes[index] == b'/' && !previous_is_slash && !url_separator && !labelled_root;
            let drive = index + 2 < bytes.len()
                && bytes[index].is_ascii_alphabetic()
                && bytes[index + 1] == b':'
                && matches!(bytes[index + 2], b'/' | b'\\');
            let unc = index + 1 < bytes.len() && bytes[index] == b'\\' && bytes[index + 1] == b'\\';
            if posix || drive || unc {
                return Some(index);
            }
        }
        index += text[index..].chars().next()?.len_utf8();
    }
    None
}

// === Bounding (hard caps + exact accounting) ===

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(in crate::commands) struct BoundStats {
    /// Strings present in the full redacted tree, at every depth.
    pub(in crate::commands) strings_total: u64,
    /// Strings actually present in the emitted payload, including the
    /// structural markers substituted for depth-omitted subtrees.
    pub(in crate::commands) strings_retained: u64,
    pub(in crate::commands) strings_truncated: u64,
    pub(in crate::commands) string_bytes_original: u64,
    pub(in crate::commands) string_bytes_retained: u64,
    /// Array elements present in the full redacted tree, at every depth —
    /// including elements inside subtrees that the depth cap later omits.
    pub(in crate::commands) array_items_original: u64,
    pub(in crate::commands) array_items_retained: u64,
    pub(in crate::commands) depth_omissions: u64,
}

/// Exact full-tree original counts. Deliberately depth-unbounded: the
/// receipt's `*_original` numbers describe the whole redacted object, so
/// that a subtree removed by the depth cap still shows up in the difference
/// between original and retained.
pub(in crate::commands) fn collect_original_counts(value: &Value, stats: &mut BoundStats) {
    match value {
        Value::String(text) => {
            stats.strings_total += 1;
            stats.string_bytes_original += text.len() as u64;
        }
        Value::Array(items) => {
            stats.array_items_original += items.len() as u64;
            for item in items {
                collect_original_counts(item, stats);
            }
        }
        Value::Object(map) => {
            for item in map.values() {
                collect_original_counts(item, stats);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn bound_value(
    value: &mut Value,
    caps: &Caps,
    stats: &mut BoundStats,
    reasons: &mut BTreeSet<&'static str>,
    depth: usize,
) {
    match value {
        Value::String(text) => {
            let (truncated, was_truncated) =
                truncate_string_grapheme_safe(text, caps.max_string_bytes);
            if was_truncated {
                *text = truncated;
                stats.strings_truncated += 1;
                reasons.insert("string_bytes_cap");
            }
            stats.strings_retained += 1;
            stats.string_bytes_retained += text.len() as u64;
        }
        Value::Array(items) => {
            if depth >= caps.max_depth {
                omit_for_depth(value, stats, reasons);
                return;
            }
            if items.len() > caps.max_array_items {
                items.truncate(caps.max_array_items);
                reasons.insert("array_items_cap");
            }
            stats.array_items_retained += items.len() as u64;
            for item in items {
                bound_value(item, caps, stats, reasons, depth + 1);
            }
        }
        Value::Object(map) => {
            if depth >= caps.max_depth {
                omit_for_depth(value, stats, reasons);
                return;
            }
            for item in map.values_mut() {
                bound_value(item, caps, stats, reasons, depth + 1);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

/// Replace a too-deep subtree with the structural marker. The marker is a
/// string that really is emitted, so it counts toward the retained totals —
/// otherwise `string_bytes_retained` would understate the artifact it
/// describes.
fn omit_for_depth(value: &mut Value, stats: &mut BoundStats, reasons: &mut BTreeSet<&'static str>) {
    stats.depth_omissions += 1;
    reasons.insert("depth_cap");
    *value = Value::String(DEPTH_OMISSION_MARKER.to_string());
    stats.strings_retained += 1;
    stats.string_bytes_retained += DEPTH_OMISSION_MARKER.len() as u64;
}

/// UTF-8/grapheme-safe truncation: never splits a grapheme cluster, and the
/// retained bytes (including the ellipsis marker) never exceed the cap.
///
/// When `max_bytes` is below the ellipsis's own 3 bytes there is no way to
/// emit both content and a truncation marker inside the cap. The honest
/// answer is the empty string plus `true`: the caller records a truncation,
/// and no partial content escapes under a cap it does not fit.
pub(in crate::commands) fn truncate_string_grapheme_safe(
    text: &str,
    max_bytes: usize,
) -> (String, bool) {
    if text.len() <= max_bytes {
        return (text.to_string(), false);
    }
    if max_bytes < '…'.len_utf8() {
        return (String::new(), true);
    }
    let budget = max_bytes - '…'.len_utf8();
    let mut out = String::new();
    for grapheme in UnicodeSegmentation::graphemes(text, true) {
        if out.len() + grapheme.len() > budget {
            break;
        }
        out.push_str(grapheme);
    }
    out.push('…');
    (out, true)
}

// === Canonical serialization (deterministic, recursively sorted keys) ===

fn canonical_string(value: &Value) -> String {
    let mut out = String::new();
    write_canonical(value, &mut out);
    out
}

fn write_canonical(value: &Value, out: &mut String) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(flag) => out.push_str(if *flag { "true" } else { "false" }),
        Value::Number(number) => {
            let _ = write!(out, "{number}");
        }
        Value::String(text) => {
            let encoded = serde_json::to_string(text).unwrap_or_else(|_| "\"\"".to_string());
            out.push_str(&encoded);
        }
        Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_canonical(item, out);
            }
            out.push(']');
        }
        Value::Object(map) => {
            let mut entries: Vec<(&String, &Value)> = map.iter().collect();
            entries.sort_by(|left, right| left.0.cmp(right.0));
            out.push('{');
            for (index, (key, item)) in entries.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                let encoded = serde_json::to_string(key).unwrap_or_else(|_| "\"\"".to_string());
                out.push_str(&encoded);
                out.push(':');
                write_canonical(item, out);
            }
            out.push('}');
        }
    }
}

// === Envelope assembly ===

pub(in crate::commands) fn render_copy(
    copy: &dyn CommandSessionStructcopyContext,
    presentation: &dyn CommandPresentationContext,
    kind: &CopyKind,
    caps: &Caps,
) -> Result<String, String> {
    let (kind_label, mut selector, mut payload) = match build_payload(copy, kind) {
        Ok(payload) => payload,
        Err(error) => return Err(selection_error(presentation, kind, error)?),
    };
    let labels = PathLabels::new(&copy.path_roots());

    // The selector is echoed verbatim into the receipt, so it goes through
    // the same redaction as the payload and gets its own tight byte bound.
    let mut selector_keys = KeyStats::default();
    redact_payload(&mut selector, &labels, &mut selector_keys);
    bound_selector(&mut selector);

    let mut keys = KeyStats::default();
    redact_payload(&mut payload, &labels, &mut keys);

    // Fit the byte cap by tightening the content caps before ever
    // considering a payload omission.
    let mut effective = *caps;
    for _ in 0..4 {
        let encoded = encode_attempt(
            kind_label, &selector, &payload, &effective, caps, &keys, false,
        );
        if encoded.len() <= caps.max_output_bytes {
            return Ok(encoded);
        }
        effective.max_string_bytes = (effective.max_string_bytes / 2).max(64);
        effective.max_array_items = (effective.max_array_items / 2).max(1);
        effective.max_depth = effective.max_depth.saturating_sub(2).max(2);
    }

    // Last resort: emit receipt metadata only. If even that exceeds the cap,
    // fail closed rather than emit an over-cap artifact.
    let encoded = encode_attempt(
        kind_label, &selector, &payload, &effective, caps, &keys, true,
    );
    if encoded.len() <= caps.max_output_bytes {
        return Ok(encoded);
    }
    Err(presentation.translate(
        "cmd_structcopy_receipt_too_large",
        &[("bytes", &caps.max_output_bytes.to_string())],
    )?)
}

/// Bound the selector independently of the payload caps. Selectors are
/// scalars, so this only has to handle the string case.
fn bound_selector(selector: &mut Value) {
    if let Value::String(text) = selector {
        let (bounded, _) = truncate_string_grapheme_safe(text, MAX_SELECTOR_BYTES);
        *text = bounded;
    }
}

fn encode_attempt(
    kind_label: &str,
    selector: &Value,
    payload: &Value,
    effective: &Caps,
    hard: &Caps,
    keys: &KeyStats,
    omit_payload: bool,
) -> String {
    let mut candidate = payload.clone();
    let mut stats = BoundStats::default();
    let mut reasons = BTreeSet::new();
    collect_original_counts(&candidate, &mut stats);
    bound_value(&mut candidate, effective, &mut stats, &mut reasons, 0);
    let mut retained_keys = RetainedKeyStats::default();
    collect_retained_key_stats(&candidate, keys, &mut Vec::new(), &mut retained_keys);
    if effective != hard {
        reasons.insert("caps_tightened_output_bytes_cap");
    }
    if retained_keys.truncated > 0 {
        reasons.insert("object_key_bytes_cap");
    }
    if retained_keys.deduped > 0 {
        reasons.insert("object_key_collision");
    }
    let emitted = if omit_payload {
        // Nothing from the bounding pass was emitted, so every retained
        // counter and every bounding reason would be a claim about an
        // artifact that does not exist. Originals stay; the rest resets.
        reasons.clear();
        reasons.insert("payload_omitted_output_bytes_cap");
        stats.strings_retained = 0;
        stats.strings_truncated = 0;
        stats.string_bytes_retained = 0;
        stats.array_items_retained = 0;
        stats.depth_omissions = 0;
        retained_keys = RetainedKeyStats::default();
        Value::Null
    } else {
        candidate
    };
    let envelope = assemble_envelope(
        kind_label,
        selector,
        &emitted,
        &stats,
        keys,
        &retained_keys,
        &reasons,
        effective,
        hard,
    );
    canonical_string(&envelope)
}

#[allow(clippy::too_many_arguments)]
fn assemble_envelope(
    kind: &str,
    selector: &Value,
    payload: &Value,
    stats: &BoundStats,
    original_keys: &KeyStats,
    retained_keys: &RetainedKeyStats,
    reasons: &BTreeSet<&'static str>,
    effective: &Caps,
    hard: &Caps,
) -> Value {
    json!({
        "object": payload,
        "receipt": {
            "schema": SCHEMA_ID,
            "human_only": true,
            "kind": kind,
            "selector": selector,
            "redaction": REDACTION_CONTRACT,
            // `caps` is the declared contract; `applied_caps` is what this
            // artifact was actually bounded with. They differ whenever the
            // output-byte cap forced a tightening pass.
            "caps": caps_value(hard),
            "applied_caps": caps_value(effective),
            "counts": {
                "strings_total": stats.strings_total,
                "strings_retained": stats.strings_retained,
                "strings_truncated": stats.strings_truncated,
                "string_bytes_original": stats.string_bytes_original,
                "string_bytes_retained": stats.string_bytes_retained,
                "array_items_original": stats.array_items_original,
                "array_items_retained": stats.array_items_retained,
                "depth_omissions": stats.depth_omissions,
                "object_keys_original": original_keys.original_total(),
                "object_keys_retained": retained_keys.total,
                "object_keys_truncated": retained_keys.truncated,
                "object_keys_deduped": retained_keys.deduped,
                "payload_bytes": canonical_string(payload).len(),
            },
            "reasons": reasons.iter().copied().collect::<Vec<_>>(),
        }
    })
}

fn caps_value(caps: &Caps) -> Value {
    json!({
        "max_output_bytes": caps.max_output_bytes,
        "max_array_items": caps.max_array_items,
        "max_string_bytes": caps.max_string_bytes,
        "max_key_bytes": MAX_KEY_BYTES,
        "max_depth": caps.max_depth,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn no_labels() -> PathLabels {
        PathLabels { labels: Vec::new() }
    }
    /// `unique_object_key` must terminate and preserve every value even when
    /// the key cap leaves no room at all for a base.
    #[test]
    fn key_dedup_terminates_under_a_degenerate_cap() {
        let mut map = serde_json::Map::new();
        for _ in 0..12 {
            let (key, _) = unique_object_key(&map, "");
            assert!(!map.contains_key(&key), "reused key {key:?}");
            map.insert(key, Value::Null);
        }
        assert_eq!(map.len(), 12, "every insert must survive");

        // Deterministic across runs with the same inputs.
        let mut replay = serde_json::Map::new();
        for _ in 0..12 {
            let (key, _) = unique_object_key(&replay, "");
            replay.insert(key, Value::Null);
        }
        let left: Vec<&String> = map.keys().collect();
        let right: Vec<&String> = replay.keys().collect();
        assert_eq!(left, right);

        assert_eq!(decimal_width(0), 1);
        assert_eq!(decimal_width(9), 1);
        assert_eq!(decimal_width(10), 2);
        assert_eq!(decimal_width(999), 3);
        assert_eq!(decimal_width(1000), 4);
    }

    /// URLs do not arrive as tidy whitespace-delimited tokens. Wrapped,
    /// embedded, uppercased, and malformed forms must all lose their
    /// userinfo, query, and fragment.
    #[test]
    fn urls_lose_userinfo_query_and_fragment_in_hostile_shapes() {
        let labels = no_labels();
        let cases = [
            "(https://u:p@host.test/a?q=1#f)",
            "<https://u:p@host.test/a?q=1#f>",
            "\"https://u:p@host.test/a?q=1#f\"",
            "'https://u:p@host.test/a?q=1#f'",
            "see https://u:p@host.test/a?q=1#f.",
            "see https://u:p@host.test/a?q=1#f, then",
            "[link](https://u:p@host.test/a?q=1#f)",
            "prefixhttps://u:p@host.test/a?q=1#f",
            "HTTPS://U:P@HOST.TEST/a?q=1#f",
            "ws://u:p@host.test/a?q=1#f",
            "ftp://u:p@host.test/a?q=1#f",
            "postgres://u:p@host.test/db?sslkey=secret#f",
            "mongodb://u:p@host.test/db?authSource=admin#f",
            "redis://u:p@host.test/0?token=secret#f",
            "amqp://u:p@host.test/vhost?token=secret#f",
            "ssh://u:p@host.test/repo?identity=secret#f",
            "socks5://u:p@host.test/path?token=secret#f",
            "trailing`https://u:p@host.test/a?q=1#f`",
            "a=https://u:p@host.test/a?q=1#f&b=2",
        ];
        for case in cases {
            let scrubbed = scrub_string(case, &labels);
            for forbidden in [
                "u:p@",
                "q=1",
                "#f",
                "P@HOST",
                "sslkey=secret",
                "authSource=admin",
                "token=secret",
                "identity=secret",
            ] {
                assert!(
                    !scrubbed.contains(forbidden),
                    "{case:?} kept {forbidden:?}: {scrubbed}"
                );
            }
            assert!(
                scrubbed.contains("host.test") || scrubbed.contains(URL_OMISSION_MARKER),
                "{case:?} -> {scrubbed}"
            );
        }

        // Two URLs in one string: both are scrubbed, order preserved.
        let both = scrub_string(
            "first https://a:b@one.test/x?y=1#z then https://c:d@two.test/w?v=2#u end",
            &labels,
        );
        assert!(both.contains("one.test"), "{both}");
        assert!(both.contains("two.test"), "{both}");
        assert!(both.starts_with("first "), "{both}");
        assert!(both.ends_with(" end"), "{both}");
        for forbidden in ["a:b@", "c:d@", "y=1", "v=2", "#z", "#u"] {
            assert!(!both.contains(forbidden), "kept {forbidden:?}: {both}");
        }

        // Unparseable but scheme-prefixed: fail closed, do not pass through.
        for hostile in [
            "https://",
            "https://[not-an-ipv6:1]/x?token=leak#f",
            "http://user:pw@:99999/x?token=leak",
        ] {
            let scrubbed = scrub_string(hostile, &labels);
            assert!(!scrubbed.contains("token=leak"), "{hostile} -> {scrubbed}");
            assert!(!scrubbed.contains("user:pw@"), "{hostile} -> {scrubbed}");
        }

        // An ANSI escape spliced into a scheme must not hide the URL from
        // the scanner: `sanitize_text` runs first.
        let hidden = scrub_string("htt\u{1b}[0mps://u:p@host.test/a?q=1#f", &labels);
        assert!(!hidden.contains("u:p@"), "{hidden}");
        assert!(!hidden.contains("q=1"), "{hidden}");

        // Text with no URL is untouched.
        assert_eq!(
            scrub_string("plain text, no url", &labels),
            "plain text, no url"
        );
    }

    #[test]
    fn path_labels_require_component_boundaries_and_preserve_repeated_roots() {
        let labels = PathLabels {
            labels: vec![
                ("/opt/app".to_string(), "<workspace>"),
                ("/Users/alice".to_string(), "<home>"),
            ],
        };

        assert_eq!(labels.apply("/opt/app"), "<workspace>");
        assert_eq!(labels.apply("/opt/app/src"), "<workspace>/src");
        assert_eq!(labels.apply(r"/opt/app\src"), r"<workspace>\src");
        assert_eq!(
            labels.apply("/opt/app/a and /opt/app/b"),
            "<workspace>/a and <workspace>/b"
        );
        assert_eq!(labels.apply("/Users/alice"), "<home>");
        assert_eq!(
            labels.apply("/Users/alice/project and /Users/alice/other"),
            "<home>/project and <home>/other"
        );

        for collision in [
            "/opt/application/customer",
            "/opt/app-old/customer",
            "/Users/alice-old/private",
            "/Users/alice2/private",
        ] {
            assert_eq!(
                labels.apply(collision),
                collision,
                "near-prefix path must not receive a trusted label"
            );
            assert_eq!(
                scrub_string(collision, &labels),
                PATH_OMISSION_MARKER,
                "near-prefix path must remain foreign and be redacted"
            );
        }
    }

    struct RecordingCopy {
        events: std::cell::RefCell<Vec<String>>,
        error: Option<StructcopyError>,
        transport: Result<StructcopyTransport, String>,
    }
    impl RecordingCopy {
        fn new() -> Self {
            Self {
                events: Default::default(),
                error: None,
                transport: Ok(StructcopyTransport::Native),
            }
        }
        fn observe(&self, event: String) -> Result<(), StructcopyError> {
            self.events.borrow_mut().push(event);
            self.error.clone().map_or(Ok(()), Err)
        }
    }
    impl CommandSessionStructcopyContext for RecordingCopy {
        fn transcript_item(&self, index: usize) -> Result<StructcopyTranscript, StructcopyError> {
            self.observe(format!("turn:{index}"))?;
            Ok(StructcopyTranscript {
                index,
                role: "user".into(),
                content: StructcopyContent::Visible(vec![StructcopyBlock::Text("visible".into())]),
            })
        }
        fn tool_pair(&self, id: &str) -> Result<StructcopyToolPair, StructcopyError> {
            self.observe(format!("tool:{id}"))?;
            Ok(StructcopyToolPair {
                name: "fetch".into(),
                input: json!({"api_key":"private"}),
                result: None,
            })
        }
        fn plan_snapshot(&self) -> Result<StructcopyPlan, StructcopyError> {
            self.observe("plan".into())?;
            Ok(StructcopyPlan {
                title: Some("plan".into()),
                ..Default::default()
            })
        }
        fn workflow_projection(&self, id: &str) -> Result<StructcopyWorkflow, StructcopyError> {
            self.observe(format!("workflow:{id}"))?;
            Err(StructcopyError::Unavailable)
        }
        fn path_roots(&self) -> StructcopyPathRoots {
            self.events.borrow_mut().push("roots".into());
            StructcopyPathRoots {
                workspace: "/work".into(),
                canonical_workspace: None,
                home: None,
            }
        }
        fn write_clipboard(&self, text: &str) -> Result<StructcopyTransport, String> {
            let payload: Value =
                serde_json::from_str(text).expect("only rendered JSON may reach clipboard");
            assert_eq!(payload["receipt"]["schema"], SCHEMA_ID);
            assert!(!text.contains("private"));
            self.events.borrow_mut().push("clipboard".into());
            self.transport.clone()
        }
    }
    struct Labels;
    impl CommandPresentationContext for Labels {
        fn translate(&self, key: &str, _: &[(&str, &str)]) -> Result<String, String> {
            Ok(key.into())
        }
    }

    #[test]
    fn structcopy_portable_selects_one_observation_and_writes_only_after_rendering() {
        for (args, selected) in [
            ("turn +1", "turn:1"),
            ("tool call-id", "tool:call-id"),
            ("plan", "plan"),
        ] {
            for stdout in [false, true] {
                let copy = RecordingCopy::new();
                let args = format!("{args}{}", if stdout { " STDOUT" } else { "" });
                let result = execute_portable(&copy, &Labels, Some(&args), &DEFAULT_CAPS).unwrap();
                assert!(!result.is_error);
                assert!(result.action.is_none());
                let mut expected = vec![selected, "roots"];
                if stdout {
                    assert!(serde_json::from_str::<Value>(&result.message.unwrap()).is_ok());
                } else {
                    expected.push("clipboard");
                    assert_eq!(
                        result.message.as_deref(),
                        Some("cmd_structcopy_clipboard_accepted")
                    );
                }
                assert_eq!(*copy.events.borrow(), expected);
            }
        }
    }

    #[test]
    fn structcopy_portable_rejections_never_write_clipboard() {
        for args in [
            None,
            Some(""),
            Some("turn 0"),
            Some("TURN 1"),
            Some("plan extra"),
            Some("tool"),
            Some("workflow"),
            Some("stdout plan"),
        ] {
            let copy = RecordingCopy::new();
            assert!(execute_portable(&copy, &Labels, args, &DEFAULT_CAPS).is_err());
            assert!(copy.events.borrow().is_empty());
        }
        for error in [
            StructcopyError::Unavailable,
            StructcopyError::Busy,
            StructcopyError::Preparation("broken".into()),
        ] {
            let mut copy = RecordingCopy::new();
            copy.error = Some(error);
            assert!(execute_portable(&copy, &Labels, Some("plan"), &DEFAULT_CAPS).is_err());
            assert_eq!(*copy.events.borrow(), ["plan"]);
        }
        let copy = RecordingCopy::new();
        assert!(execute_portable(&copy, &Labels, Some("workflow missing"), &DEFAULT_CAPS).is_err());
        assert_eq!(*copy.events.borrow(), ["workflow:missing"]);
        let copy = RecordingCopy::new();
        assert!(
            execute_portable(
                &copy,
                &Labels,
                Some("turn 1"),
                &Caps {
                    max_output_bytes: 1,
                    ..DEFAULT_CAPS
                }
            )
            .is_err()
        );
        assert_eq!(*copy.events.borrow(), ["turn:1", "roots"]);
    }

    #[test]
    fn structcopy_portable_transport_receipts_and_missing_facets_are_honest() {
        for (transport, key, error) in [
            (
                Ok(StructcopyTransport::TerminalQueued),
                "cmd_structcopy_clipboard_queued",
                false,
            ),
            (
                Err("clipboard unavailable".into()),
                "cmd_structcopy_clipboard_failed",
                true,
            ),
        ] {
            let mut copy = RecordingCopy::new();
            copy.transport = transport;
            let result = execute_structcopy(
                CommandContexts::empty()
                    .with_structcopy(&mut copy)
                    .with_presentation(&mut Labels),
                Some("plan"),
            );
            assert_eq!(result.is_error, error);
            assert!(result.action.is_none());
            assert_eq!(
                result.message,
                Some(if error {
                    format!("Error: {key}")
                } else {
                    key.into()
                })
            );
            assert_eq!(*copy.events.borrow(), ["plan", "roots", "clipboard"]);
        }
        let mut copy = RecordingCopy::new();
        assert!(execute_structcopy(CommandContexts::empty(), Some("plan")).is_error);
        assert!(
            execute_structcopy(
                CommandContexts::empty().with_presentation(&mut Labels),
                Some("plan")
            )
            .is_error
        );
        assert!(
            execute_structcopy(
                CommandContexts::empty().with_structcopy(&mut copy),
                Some("plan")
            )
            .is_error
        );
        assert!(copy.events.borrow().is_empty());
    }
}
