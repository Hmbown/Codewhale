//! `/export` command — portable handler over the session-export facet
//! (FEAT-025 D1-D9).
//!
//! Parsing, document rendering, redaction, operation sequencing, and result
//! composition are portable: this module depends only on the external command
//! contract, `chrono`, the shared pure sanitizer in
//! [`codewhale_secrets::sanitize`], and the temporary FEAT-037 `CommandResult`.
//! Concrete `App`, clipboard, filesystem, snapshot, history, and turn-handoff
//! access stays behind `CommandSessionExportContext` (D1). Helpers, tests, and
//! this handler therefore carry no TUI, client, configuration, or filesystem
//! dependency, so the slice can move to `codewhale-commands` unchanged (D8/D10).

use std::fmt::Write as FmtWrite;
use std::path::PathBuf;

use codewhale_command_contract::facets::{
    CommandSessionExportContext, ConversationExportProjection, ExportBlock, ExportMessage,
    HistoryEntry, RestorePointProjection, RestoreSnapshot, TranscriptProjection,
    TurnHandoffProjection,
};
use codewhale_command_contract::handler::{CommandCapabilities, CommandContexts, CommandHandler};
use codewhale_command_contract::metadata::{
    CommandInfo as ContractInfo, RegisterCommand as ContractRegisterCommand,
};
use codewhale_secrets::sanitize::{
    inline_text, is_internal_role, redact_json, redact_url_for_display, sanitize_text,
};
use serde_json::Value;

use super::CommandResult;

pub(in crate::commands) struct ExportCmd;

pub(in crate::commands) const CONTRACT_INFO: ContractInfo = ContractInfo {
    name: "export",
    aliases: &["daochu"],
    usage: "/export [clipboard|file [--force] <path>|turn [clipboard|file [--force] <path>]]",
    description_key: "cmd_export_description",
};

impl ContractRegisterCommand<CommandResult> for ExportCmd {
    fn info() -> &'static ContractInfo {
        &CONTRACT_INFO
    }

    fn handler() -> CommandHandler<CommandResult> {
        CommandHandler::Contextual {
            capabilities: CommandCapabilities::SESSION_EXPORT,
            handler: export_contextual,
        }
    }
}

pub(in crate::commands) fn export_contextual(
    contexts: CommandContexts<'_>,
    arg: Option<&str>,
) -> CommandResult {
    let parts = contexts.into_parts();
    let Some(export) = parts.export.as_deref() else {
        return CommandResult::error("Command capability unavailable: session_export".to_string());
    };
    export_portable(export, arg)
}

/// Portable `/export` composed entirely from contract-owned data and facet
/// operations. Parse first, render the selected scope second, then run the
/// destination-specific sequence (D6/D7).
pub(in crate::commands) fn export_portable(
    export: &dyn CommandSessionExportContext,
    arg: Option<&str>,
) -> CommandResult {
    let request = match parse_request(arg) {
        Ok(request) => request,
        Err(err) => return CommandResult::error(err),
    };
    let label = match request.scope {
        ExportScope::Conversation => "Conversation",
        ExportScope::Turn => "Turn handoff",
    };
    let markdown = match request.scope {
        ExportScope::Conversation => render_conversation(export.conversation_projection()),
        ExportScope::Turn => sanitize_turn_handoff(&export.turn_handoff_projection()),
    };

    match request.destination {
        ExportDestination::Clipboard => copy_to_clipboard(export, label, &markdown),
        ExportDestination::File { path, force } => {
            let path = match export.resolve_export_path(&path) {
                Ok(path) => path,
                Err(err) => return CommandResult::error(err),
            };
            match export.write_export_file(&path, markdown.as_bytes(), force) {
                Ok(()) => CommandResult::message(format!(
                    "{label} exported to {}{}",
                    path.display(),
                    if force {
                        " (overwrite explicitly allowed)"
                    } else {
                        ""
                    }
                )),
                Err(err) => CommandResult::error(format!(
                    "Failed to export {label} to {}: {err}",
                    path.display()
                )),
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExportScope {
    Conversation,
    Turn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ExportDestination {
    Clipboard,
    File { path: String, force: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExportRequest {
    scope: ExportScope,
    destination: ExportDestination,
}

fn parse_request(arg: Option<&str>) -> Result<ExportRequest, String> {
    let raw = arg.unwrap_or("").trim();
    if raw.is_empty() || raw.eq_ignore_ascii_case("clipboard") {
        return Ok(ExportRequest {
            scope: ExportScope::Conversation,
            destination: ExportDestination::Clipboard,
        });
    }

    if raw.eq_ignore_ascii_case("turn") {
        return Ok(ExportRequest {
            scope: ExportScope::Turn,
            destination: ExportDestination::Clipboard,
        });
    }

    if let Some(rest) = strip_word(raw, "turn") {
        let rest = rest.trim();
        if rest.is_empty() || rest.eq_ignore_ascii_case("clipboard") {
            return Ok(ExportRequest {
                scope: ExportScope::Turn,
                destination: ExportDestination::Clipboard,
            });
        }
        let destination = if let Some(file_args) = strip_word(rest, "file") {
            parse_file_destination(file_args)?
        } else if rest.eq_ignore_ascii_case("file") {
            return Err(export_usage("missing file path"));
        } else if strip_word(rest, "clipboard").is_some() {
            return Err(export_usage("clipboard does not accept a path"));
        } else {
            // Backward compatibility: `/export turn <path>`.
            ExportDestination::File {
                path: rest.to_string(),
                force: false,
            }
        };
        return Ok(ExportRequest {
            scope: ExportScope::Turn,
            destination,
        });
    }

    if let Some(file_args) = strip_word(raw, "file") {
        return Ok(ExportRequest {
            scope: ExportScope::Conversation,
            destination: parse_file_destination(file_args)?,
        });
    }
    if raw.eq_ignore_ascii_case("file") {
        return Err(export_usage("missing file path"));
    }
    if strip_word(raw, "clipboard").is_some() {
        return Err(export_usage("clipboard does not accept a path"));
    }

    // Backward compatibility: `/export <path>`.
    Ok(ExportRequest {
        scope: ExportScope::Conversation,
        destination: ExportDestination::File {
            path: raw.to_string(),
            force: false,
        },
    })
}

fn parse_file_destination(raw: &str) -> Result<ExportDestination, String> {
    let trimmed = raw.trim();
    let (force, path) = if let Some(path) = strip_word(trimmed, "--force") {
        (true, path.trim())
    } else if trimmed.eq_ignore_ascii_case("--force") {
        (true, "")
    } else {
        (false, trimmed)
    };
    if path.is_empty() {
        return Err(export_usage("missing file path"));
    }
    Ok(ExportDestination::File {
        path: path.to_string(),
        force,
    })
}

fn strip_word<'a>(value: &'a str, word: &str) -> Option<&'a str> {
    let prefix = value.get(..word.len())?;
    if !prefix.eq_ignore_ascii_case(word) {
        return None;
    }
    let rest = value.get(word.len()..)?;
    rest.chars()
        .next()
        .is_some_and(char::is_whitespace)
        .then_some(rest)
}

fn export_usage(reason: &str) -> String {
    format!(
        "{reason}. Usage: /export [clipboard|file [--force] <path>|turn [clipboard|file [--force] <path>]]"
    )
}

fn copy_to_clipboard(
    export: &dyn CommandSessionExportContext,
    label: &str,
    markdown: &str,
) -> CommandResult {
    let terminal_client = export.clipboard_requires_terminal_paste();
    let last_copy = export.write_recovery_copy(markdown);
    let copy_hint = |path: Option<PathBuf>| match path {
        Some(path) => {
            format!("; a copy is at {}", path.display())
        }
        None => String::new(),
    };
    match export.write_clipboard(markdown) {
        Ok(()) if terminal_client => CommandResult::message(format!(
            "{label} sent to the terminal-client clipboard over SSH via tmux/OSC 52 ({} lines){}; terminal support and settings determine whether the client accepts it",
            markdown.lines().count(),
            copy_hint(last_copy)
        )),
        Ok(()) => CommandResult::message(format!(
            "{label} copied to the local clipboard ({} lines; a terminal clipboard fallback may have been used){}",
            markdown.lines().count(),
            copy_hint(last_copy)
        )),
        Err(err) => match last_copy {
            Some(path) => CommandResult::error(format!(
                "Clipboard export failed: {err}. The full export was written to {}; /export file <path> writes it where you choose",
                path.display()
            )),
            None => CommandResult::error(format!(
                "Clipboard export failed: {err}. No file was written; use `/export file <path>` to choose an explicit destination"
            )),
        },
    }
}

/// Render the full-conversation export document from portable projection data.
///
/// Takes the projection by value so the render path can move each block's JSON
/// payload into `push_json` instead of cloning it a second time. The projection
/// itself was already copied once at the facet boundary (F3); cloning the
/// payloads again here would be an avoidable extra copy of every tool input and
/// structured result.
fn render_conversation(projection: ConversationExportProjection) -> String {
    let ConversationExportProjection {
        metadata,
        transcript,
        restore_points,
    } = projection;
    let mut out = String::new();
    out.push_str("# Codewhale conversation export\n\n");
    let _ = writeln!(
        out,
        "- Exported: {}",
        format_export_time(metadata.exported_at_unix)
    );
    let _ = writeln!(out, "- Session: {}", inline_text(&metadata.session_label));
    let _ = writeln!(out, "- Provider: {}", inline_text(&metadata.provider));
    let _ = writeln!(out, "- Model: {}", inline_text(&metadata.model));
    let _ = writeln!(out, "- Mode: {}", metadata.mode);
    let _ = writeln!(
        out,
        "- Workspace: {}",
        inline_text(&metadata.workspace_name)
    );
    let _ = writeln!(out, "- Messages: {}", metadata.message_count);
    out.push_str(
        "\n> Hidden instructions, internal reasoning, and reasoning signatures are omitted. Secret-like values and credential-bearing URLs are redacted as a defense in depth; review the export before sharing it.\n\n",
    );

    render_restore_summary(&mut out, &restore_points);

    match transcript {
        TranscriptProjection::HistoryFallback(entries) => {
            render_history_fallback(&mut out, &entries)
        }
        TranscriptProjection::Authoritative(messages) => {
            for (index, message) in messages.into_iter().enumerate() {
                // Correlation is derived before `message` is consumed by the
                // renderer so the render path can take ownership of its payloads.
                let correlation = correlation_markdown(&restore_points, &message);
                render_message(&mut out, index + 1, message);
                out.push_str(&correlation);
            }
        }
    }
    out
}

fn format_export_time(timestamp: i64) -> String {
    chrono::DateTime::from_timestamp(timestamp, 0)
        .map(|time| time.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
        .unwrap_or_else(|| "unknown".to_string())
}

/// Characters of the snapshot SHA shown as a restore-point id.
const RESTORE_POINT_ID_LEN: usize = 12;

fn render_restore_summary(out: &mut String, projection: &RestorePointProjection) {
    out.push_str("## Restore points\n\n");
    match projection {
        RestorePointProjection::None => {
            out.push_str(
                "No workspace restore points are recorded for this workspace, so nothing in this export can be correlated to a restorable workspace state. Snapshots may be disabled, or no turn has taken one yet.\n\n",
            );
        }
        RestorePointProjection::Unreadable { reason } => {
            let _ = writeln!(
                out,
                "Workspace restore points could not be read ({}). Treat the correlation below as unavailable rather than empty.\n",
                inline_text(reason)
            );
        }
        RestorePointProjection::Recorded { snapshots } if snapshots.is_empty() => {
            out.push_str(
                "A snapshot repository exists for this workspace but records no restore points yet.\n\n",
            );
        }
        RestorePointProjection::Recorded { snapshots } => {
            let _ = writeln!(
                out,
                "The {} most recent workspace restore points, newest first. `/restore <N>` restores by the index in this table and `/restore list` shows the live list.\n",
                snapshots.len()
            );
            out.push_str(
                "> The index is the position at export time. Every new turn records another restore point and shifts it, so re-check `/restore list` before restoring from an older export. The snapshot id does not shift.\n\n",
            );
            out.push_str("| N | Restore point | Recorded (UTC) | Label |\n");
            out.push_str("| --- | --- | --- | --- |\n");
            for (index, snapshot) in snapshots.iter().enumerate() {
                let _ = writeln!(
                    out,
                    "| {} | `{}` | {} | {} |",
                    index + 1,
                    short_restore_id(&snapshot.id),
                    format_snapshot_time(snapshot.timestamp_unix),
                    inline_text(&snapshot.label)
                );
            }
            out.push('\n');
        }
    }
}

/// Append the restore points correlated to a single user message.
///
/// Correlation is by the prompt snippet the host embedded in the snapshot
/// label, produced by the same function the snapshot writer uses. No
/// message-index-to-turn-sequence mapping is invented: a turn sequence and an
/// export message index are different counters, and asserting they line up
/// would be a guess presented as provenance.
fn correlation_markdown(projection: &RestorePointProjection, message: &ExportMessage) -> String {
    let mut out = String::new();
    // F6: exact `Role::User` identity, not the rendered role string. Comparing
    // textually against "user" would also match `Role::Unrecognized("user")`,
    // which the baseline deliberately did not correlate.
    if !message.is_user_role {
        return out;
    }
    let RestorePointProjection::Recorded { snapshots } = projection else {
        return out;
    };
    let Some(snippet) = message.prompt_snippet.as_deref() else {
        return out;
    };

    let matches: Vec<(usize, &RestoreSnapshot)> = snapshots
        .iter()
        .enumerate()
        .filter(|(_, snapshot)| {
            matches!(snapshot.kind.as_str(), "pre-turn" | "post-turn")
                && snapshot.prompt_snippet.as_deref() == Some(snippet)
        })
        .collect();

    if matches.is_empty() {
        out.push_str(
            "- Restore points: none recorded for this message within the listed window.\n\n",
        );
        return out;
    }

    let ambiguous = matches.len() > 1;
    let rendered: Vec<String> = matches
        .iter()
        .map(|(index, snapshot)| {
            let seq = snapshot
                .sequence
                .map(|seq| format!(" turn {seq}"))
                .unwrap_or_default();
            format!(
                "N{} `{}` ({}{})",
                index + 1,
                short_restore_id(&snapshot.id),
                snapshot.kind,
                seq
            )
        })
        .collect();
    let _ = writeln!(out, "- Restore points: {}", rendered.join(", "));
    if ambiguous {
        out.push_str(
            "  - More than one restore point carries this prompt snippet, so the match is ambiguous; compare the recorded times above before restoring.\n",
        );
    }
    out.push('\n');
    out
}

fn short_restore_id(id: &str) -> String {
    id.chars().take(RESTORE_POINT_ID_LEN).collect()
}

fn format_snapshot_time(timestamp: i64) -> String {
    chrono::DateTime::from_timestamp(timestamp, 0)
        .map(|time| time.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
        .unwrap_or_else(|| "unknown".to_string())
}

fn render_message(out: &mut String, index: usize, message: ExportMessage) {
    let role = inline_text(&message.role);
    let _ = writeln!(out, "## {index}. {role}\n");
    if is_internal_role(&message.role) {
        out.push_str("[internal context omitted]\n\n");
        return;
    }
    if message.blocks.is_empty() {
        out.push_str("[no content]\n\n");
        return;
    }
    for (block_index, block) in message.blocks.into_iter().enumerate() {
        render_content_block(out, block_index + 1, block);
    }
}

fn render_content_block(out: &mut String, index: usize, block: ExportBlock) {
    match block {
        ExportBlock::Text { text } => {
            let _ = writeln!(out, "### Content {index}: Text\n");
            push_sanitized_text(out, &text);
        }
        ExportBlock::ImageReference { url } => {
            let _ = writeln!(out, "### Content {index}: Image attachment\n");
            let _ = writeln!(
                out,
                "- Reference: {}\n",
                inline_text(&redact_url_for_display(&url))
            );
        }
        ExportBlock::ImageOmitted => {
            let _ = writeln!(out, "### Content {index}: Image attachment\n");
            out.push_str("- Reference omitted (inline or local image payload)\n\n");
        }
        ExportBlock::InternalReasoning => {
            let _ = writeln!(out, "### Content {index}: Internal reasoning\n");
            out.push_str("[internal reasoning and signature omitted]\n\n");
        }
        ExportBlock::ToolCall {
            id,
            name,
            caller,
            input,
        } => {
            let _ = writeln!(out, "### Content {index}: Tool call\n");
            let _ = writeln!(out, "- ID: {}", inline_text(&id));
            let _ = writeln!(out, "- Name: {}", inline_text(&name));
            if let Some(caller) = caller {
                let _ = writeln!(out, "- Caller type: {}", inline_text(&caller.caller_type));
                if let Some(tool_id) = caller.tool_id.as_deref() {
                    let _ = writeln!(out, "- Caller tool ID: {}", inline_text(tool_id));
                }
            }
            out.push_str("\nInput:\n\n");
            push_json(out, input);
        }
        ExportBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
            structured,
        } => {
            let _ = writeln!(out, "### Content {index}: Tool result\n");
            let _ = writeln!(out, "- Tool call ID: {}", inline_text(&tool_use_id));
            let _ = writeln!(out, "- Error: {is_error}\n");
            out.push_str("Result:\n\n");
            push_sanitized_text(out, &content);
            if let Some(blocks) = structured {
                out.push_str("Structured result blocks:\n\n");
                push_json(out, blocks);
            }
        }
        ExportBlock::ServerToolCall { id, name, input } => {
            let _ = writeln!(out, "### Content {index}: Server tool call\n");
            let _ = writeln!(out, "- ID: {}", inline_text(&id));
            let _ = writeln!(out, "- Name: {}\n", inline_text(&name));
            out.push_str("Input:\n\n");
            push_json(out, input);
        }
        ExportBlock::ToolSearchResult {
            tool_use_id,
            content,
        } => {
            let _ = writeln!(out, "### Content {index}: Tool-search result\n");
            let _ = writeln!(out, "- Tool call ID: {}\n", inline_text(&tool_use_id));
            push_json(out, content);
        }
        ExportBlock::CodeExecutionResult {
            tool_use_id,
            content,
        } => {
            let _ = writeln!(out, "### Content {index}: Code-execution result\n");
            let _ = writeln!(out, "- Tool call ID: {}\n", inline_text(&tool_use_id));
            push_json(out, content);
        }
    }
}

fn render_history_fallback(out: &mut String, entries: &[HistoryEntry]) {
    if entries.is_empty() {
        out.push_str("## Conversation\n\n[empty conversation]\n");
        return;
    }
    out.push_str(
        "> Structured API messages were unavailable; the entries below are a sanitized visible-history fallback.\n\n",
    );
    for (index, entry) in entries.iter().enumerate() {
        let (role, body) = match entry {
            HistoryEntry::Sanitized { role, body } => (role.as_str(), sanitize_text(body)),
            HistoryEntry::Literal { role, body } => (role.as_str(), body.clone()),
        };
        let _ = writeln!(out, "## {}. {}\n", index + 1, inline_text(role));
        push_pre_sanitized_text(out, &body);
    }
}

fn push_sanitized_text(out: &mut String, text: &str) {
    push_pre_sanitized_text(out, &sanitize_text(text));
}

fn push_pre_sanitized_text(out: &mut String, text: &str) {
    if text.trim().is_empty() {
        out.push_str("[empty text]\n\n");
    } else {
        out.push_str(text.trim_end());
        out.push_str("\n\n");
    }
}

fn push_json(out: &mut String, mut value: Value) {
    // Redact in place: the caller hands over ownership, so there is no need to
    // clone the whole payload just to redact it (F3 follow-up).
    redact_json(&mut value, None);
    let json = serde_json::to_string_pretty(&value)
        .unwrap_or_else(|_| "\"[structured content unavailable]\"".to_string());
    let fence = markdown_fence(&json);
    let _ = writeln!(out, "{fence}json\n{json}\n{fence}\n");
}

fn markdown_fence(content: &str) -> String {
    let longest = content
        .split(|ch| ch != '`')
        .map(str::len)
        .max()
        .unwrap_or(0);
    "`".repeat(longest.saturating_add(1).max(3))
}

fn sanitize_turn_handoff(projection: &TurnHandoffProjection) -> String {
    let sanitized = sanitize_text(&projection.markdown);
    if projection.workspace_path.is_empty() {
        sanitized
    } else {
        sanitized.replace(&projection.workspace_path, ".")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::path::Path;

    /// Minimal fake facet: every delegate is deterministic and records calls.
    struct FakeExport {
        conversation: ConversationExportProjection,
        turn: TurnHandoffProjection,
        terminal_paste: bool,
        recovery: Option<PathBuf>,
        clipboard: Result<(), String>,
        resolve: Result<PathBuf, String>,
        write: Result<(), String>,
        calls: RefCell<Vec<String>>,
    }

    impl Default for FakeExport {
        fn default() -> Self {
            Self {
                conversation: conversation_projection(vec![]),
                turn: TurnHandoffProjection {
                    markdown: String::new(),
                    workspace_path: String::new(),
                },
                terminal_paste: false,
                recovery: None,
                clipboard: Ok(()),
                resolve: Ok(PathBuf::from("/resolved/out.md")),
                write: Ok(()),
                calls: RefCell::new(Vec::new()),
            }
        }
    }

    impl CommandSessionExportContext for FakeExport {
        fn conversation_projection(&self) -> ConversationExportProjection {
            self.calls
                .borrow_mut()
                .push("conversation_projection".to_string());
            self.conversation.clone()
        }

        fn turn_handoff_projection(&self) -> TurnHandoffProjection {
            self.calls
                .borrow_mut()
                .push("turn_handoff_projection".to_string());
            self.turn.clone()
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
                .push(format!("write_recovery_copy({markdown})"));
            self.recovery.clone()
        }

        fn write_clipboard(&self, markdown: &str) -> Result<(), String> {
            self.calls
                .borrow_mut()
                .push(format!("write_clipboard({markdown})"));
            self.clipboard.clone()
        }

        fn resolve_export_path(&self, raw: &str) -> Result<PathBuf, String> {
            self.calls
                .borrow_mut()
                .push(format!("resolve_export_path({raw})"));
            self.resolve.clone()
        }

        fn write_export_file(
            &self,
            path: &Path,
            contents: &[u8],
            force: bool,
        ) -> Result<(), String> {
            self.calls.borrow_mut().push(format!(
                "write_export_file({}, {}, {force})",
                path.display(),
                String::from_utf8_lossy(contents)
            ));
            self.write.clone()
        }
    }

    fn conversation_projection(messages: Vec<ExportMessage>) -> ConversationExportProjection {
        ConversationExportProjection {
            metadata: codewhale_command_contract::facets::ExportMetadata {
                session_label: "sess12345678".to_string(),
                provider: "deepseek".to_string(),
                model: "deepseek-v4".to_string(),
                mode: "agent".to_string(),
                workspace_name: "workspace".to_string(),
                message_count: messages.len(),
                exported_at_unix: 1_700_000_000,
            },
            transcript: TranscriptProjection::Authoritative(messages),
            restore_points: RestorePointProjection::None,
        }
    }

    fn user_message(text: &str) -> ExportMessage {
        ExportMessage {
            is_user_role: true,
            role: "user".to_string(),
            blocks: vec![ExportBlock::Text {
                text: text.to_string(),
            }],
            prompt_snippet: Some(text.to_string()),
        }
    }

    fn snapshot(
        id: &str,
        label: &str,
        timestamp: i64,
        kind: &str,
        sequence: Option<u64>,
    ) -> RestoreSnapshot {
        RestoreSnapshot {
            id: id.to_string(),
            label: label.to_string(),
            timestamp_unix: timestamp,
            kind: kind.to_string(),
            sequence,
            prompt_snippet: label
                .split_once(": ")
                .map(|(_, snippet)| snippet.to_string()),
        }
    }

    #[test]
    fn parser_matrix_is_exact() {
        assert_eq!(
            parse_request(None).unwrap(),
            ExportRequest {
                scope: ExportScope::Conversation,
                destination: ExportDestination::Clipboard,
            }
        );
        assert_eq!(
            parse_request(Some("clipboard")).unwrap().scope,
            ExportScope::Conversation
        );
        assert_eq!(
            parse_request(Some("TURN")).unwrap().scope,
            ExportScope::Turn
        );
        assert_eq!(
            parse_request(Some("file --force reports/chat export.md")).unwrap(),
            ExportRequest {
                scope: ExportScope::Conversation,
                destination: ExportDestination::File {
                    path: "reports/chat export.md".to_string(),
                    force: true,
                },
            }
        );
        assert_eq!(
            parse_request(Some("legacy export.md")).unwrap(),
            ExportRequest {
                scope: ExportScope::Conversation,
                destination: ExportDestination::File {
                    path: "legacy export.md".to_string(),
                    force: false,
                },
            }
        );
        assert_eq!(
            parse_request(Some("turn file --force handoff.md")).unwrap(),
            ExportRequest {
                scope: ExportScope::Turn,
                destination: ExportDestination::File {
                    path: "handoff.md".to_string(),
                    force: true,
                },
            }
        );
        assert_eq!(
            parse_request(Some("turn legacy.md")).unwrap(),
            ExportRequest {
                scope: ExportScope::Turn,
                destination: ExportDestination::File {
                    path: "legacy.md".to_string(),
                    force: false,
                },
            }
        );
        assert_eq!(
            parse_request(Some("turn clipboard")).unwrap().destination,
            ExportDestination::Clipboard
        );

        for arg in [
            "file",
            "file --force",
            "clipboard extra.md",
            "turn clipboard extra.md",
        ] {
            assert!(parse_request(Some(arg)).is_err(), "{arg}");
        }
        assert_eq!(
            parse_request(Some("file")).unwrap_err(),
            export_usage("missing file path")
        );
        assert_eq!(
            parse_request(Some("clipboard extra.md")).unwrap_err(),
            export_usage("clipboard does not accept a path")
        );
    }

    #[test]
    fn missing_authority_fails_safely_without_effects() {
        let result = export_contextual(CommandContexts::empty(), Some("clipboard"));
        assert!(result.is_error);
        assert_eq!(
            result.message.as_deref(),
            Some("Error: Command capability unavailable: session_export")
        );
    }

    #[test]
    fn conversation_clipboard_renders_full_document_and_sequences_operations() {
        let fake = FakeExport {
            conversation: conversation_projection(vec![
                ExportMessage {
                    is_user_role: false,
                    role: "system".to_string(),
                    blocks: vec![ExportBlock::Text {
                        text: "hidden policy must never export".to_string(),
                    }],
                    prompt_snippet: None,
                },
                user_message("Please inspect this"),
            ]),
            recovery: Some(PathBuf::from("/home/.codewhale/exports/last-copy.md")),
            ..FakeExport::default()
        };

        let result = export_portable(&fake, Some("clipboard"));

        assert!(!result.is_error, "{:?}", result.message);
        let message = result.message.as_deref().unwrap_or_default();
        assert!(
            message.starts_with("Conversation copied to the local clipboard ("),
            "{message}"
        );
        assert!(
            message.contains(" lines; a terminal clipboard fallback may have been used)"),
            "{message}"
        );
        assert!(
            message.ends_with("; a copy is at /home/.codewhale/exports/last-copy.md"),
            "{message}"
        );
        let read = fake.calls.borrow();
        assert_eq!(read[0], "conversation_projection");
        assert_eq!(read[1], "clipboard_requires_terminal_paste");
        assert!(read[2].starts_with("write_recovery_copy("), "{read:?}");
        assert!(read[3].starts_with("write_clipboard("), "{read:?}");
        assert_eq!(read.len(), 4, "exactly one recovery and one clipboard call");
        let recovery_payload = read[2]
            .trim_start_matches("write_recovery_copy(")
            .trim_end_matches(')');
        let markdown = read[3]
            .trim_start_matches("write_clipboard(")
            .trim_end_matches(')');
        assert_eq!(
            recovery_payload, markdown,
            "both writes receive identical Markdown"
        );
        assert!(markdown.starts_with("# Codewhale conversation export\n\n"));
        assert!(markdown.contains("## 1. system\n\n[internal context omitted]\n\n"));
        assert!(markdown.contains("## 2. user\n\n### Content 1: Text\n\nPlease inspect this\n\n"));
        assert!(!markdown.contains("hidden policy must never export"));
    }

    #[test]
    fn conversation_document_matches_full_golden_equality() {
        let projection = conversation_projection(vec![
            ExportMessage {
                is_user_role: false,
                role: "system".to_string(),
                blocks: vec![ExportBlock::Text {
                    text: "hidden policy must never export".to_string(),
                }],
                prompt_snippet: None,
            },
            user_message("Please inspect this"),
        ]);

        let expected = "# Codewhale conversation export\n\n\
- Exported: 2023-11-14T22:13:20Z\n\
- Session: sess12345678\n\
- Provider: deepseek\n\
- Model: deepseek-v4\n\
- Mode: agent\n\
- Workspace: workspace\n\
- Messages: 2\n\n\
> Hidden instructions, internal reasoning, and reasoning signatures are omitted. Secret-like values and credential-bearing URLs are redacted as a defense in depth; review the export before sharing it.\n\n\
## Restore points\n\n\
No workspace restore points are recorded for this workspace, so nothing in this export can be correlated to a restorable workspace state. Snapshots may be disabled, or no turn has taken one yet.\n\n\
## 1. system\n\n[internal context omitted]\n\n\
## 2. user\n\n### Content 1: Text\n\nPlease inspect this\n\n";

        assert_eq!(render_conversation(projection), expected);
    }

    #[test]
    fn ssh_clipboard_uses_terminal_client_wording() {
        let fake = FakeExport {
            terminal_paste: true,
            conversation: conversation_projection(vec![user_message("hi")]),
            ..FakeExport::default()
        };

        let result = export_portable(&fake, Some("clipboard"));

        assert!(!result.is_error);
        let message = result.message.as_deref().unwrap_or_default();
        assert!(
            message.contains("terminal-client clipboard over SSH via tmux/OSC 52"),
            "{message}"
        );
        assert!(
            !message.contains("a copy is at"),
            "no recovery path present: {message}"
        );
    }

    #[test]
    fn recovery_failure_still_attempts_clipboard() {
        let fake = FakeExport {
            conversation: conversation_projection(vec![user_message("hi")]),
            recovery: None,
            clipboard: Err("no clipboard".to_string()),
            ..FakeExport::default()
        };

        let result = export_portable(&fake, Some("clipboard"));

        assert!(result.is_error);
        assert_eq!(
            result.message.as_deref(),
            Some(
                "Error: Clipboard export failed: no clipboard. No file was written; use `/export file <path>` to choose an explicit destination"
            )
        );
        let read = fake.calls.borrow();
        assert!(
            read.iter()
                .any(|call| call.starts_with("write_recovery_copy("))
        );
        assert!(read.iter().any(|call| call.starts_with("write_clipboard(")));
    }

    #[test]
    fn file_export_renders_resolves_then_writes_once() {
        let fake = FakeExport {
            conversation: conversation_projection(vec![user_message("first export")]),
            resolve: Ok(PathBuf::from("/workspace/transcript.md")),
            write: Ok(()),
            ..FakeExport::default()
        };

        let result = export_portable(&fake, Some("file transcript.md"));

        assert!(!result.is_error, "{:?}", result.message);
        assert_eq!(
            result.message.as_deref(),
            Some("Conversation exported to /workspace/transcript.md")
        );
        let read = fake.calls.borrow();
        assert_eq!(read.len(), 3, "{read:?}");
        assert_eq!(read[0], "conversation_projection");
        assert_eq!(read[1], "resolve_export_path(transcript.md)");
        assert!(read[2].starts_with("write_export_file(/workspace/transcript.md,"));
        assert!(
            !read
                .iter()
                .any(|call| call.contains("clipboard") || call.contains("recovery")),
            "file export must not touch clipboard or recovery: {read:?}"
        );
    }

    #[test]
    fn resolution_failure_prevents_writing() {
        let fake = FakeExport {
            conversation: conversation_projection(vec![user_message("x")]),
            resolve: Err("export paths may not contain `..`".to_string()),
            ..FakeExport::default()
        };

        let result = export_portable(&fake, Some("file ../escape.md"));

        assert!(result.is_error);
        assert_eq!(
            result.message.as_deref(),
            Some("Error: export paths may not contain `..`")
        );
        let read = fake.calls.borrow();
        assert!(
            !read
                .iter()
                .any(|call| call.starts_with("write_export_file(")),
            "no write after resolution failure: {read:?}"
        );
    }

    #[test]
    fn write_failure_wraps_exact_baseline_text() {
        let fake = FakeExport {
            conversation: conversation_projection(vec![user_message("x")]),
            resolve: Ok(PathBuf::from("/workspace/out.md")),
            write: Err("destination already exists: /workspace/out.md".to_string()),
            ..FakeExport::default()
        };

        let result = export_portable(&fake, Some("file out.md"));

        assert!(result.is_error);
        assert_eq!(
            result.message.as_deref(),
            Some(
                "Error: Failed to export Conversation to /workspace/out.md: destination already exists: /workspace/out.md"
            )
        );
    }

    #[test]
    fn forced_file_export_reports_overwrite_suffix() {
        let fake = FakeExport {
            conversation: conversation_projection(vec![user_message("x")]),
            resolve: Ok(PathBuf::from("/workspace/out.md")),
            ..FakeExport::default()
        };

        let result = export_portable(&fake, Some("file --force out.md"));

        assert_eq!(
            result.message.as_deref(),
            Some("Conversation exported to /workspace/out.md (overwrite explicitly allowed)")
        );
        let read = fake.calls.borrow();
        assert!(read[2].ends_with(", true)"), "{:?}", read[2]);
    }

    #[test]
    fn turn_export_sanitizes_then_replaces_nonempty_workspace() {
        let fake = FakeExport {
            turn: TurnHandoffProjection {
                markdown: "# Turn handoff\n\n\u{1b}[31m/Users/me/repo\u{1b}[0m done".to_string(),
                workspace_path: "/Users/me/repo".to_string(),
            },
            ..FakeExport::default()
        };

        let result = export_portable(&fake, Some("turn"));

        assert!(!result.is_error);
        let read = fake.calls.borrow();
        let markdown = read
            .iter()
            .find_map(|call| {
                call.strip_prefix("write_clipboard(")
                    .map(|c| c.trim_end_matches(')'))
            })
            .expect("clipboard payload");
        assert_eq!(markdown, "# Turn handoff\n\n. done");
        assert!(
            !read
                .iter()
                .any(|call| call.starts_with("conversation_projection")),
            "turn-only export must not read conversation snapshots: {read:?}"
        );
    }

    #[test]
    fn turn_export_with_empty_workspace_path_skips_replacement() {
        let fake = FakeExport {
            turn: TurnHandoffProjection {
                markdown: "path stays".to_string(),
                workspace_path: String::new(),
            },
            ..FakeExport::default()
        };
        assert_eq!(sanitize_turn_handoff(&fake.turn), "path stays");
    }

    #[test]
    fn restore_summary_distinguishes_every_state() {
        let mut none = String::new();
        render_restore_summary(&mut none, &RestorePointProjection::None);
        assert!(
            none.contains("No workspace restore points are recorded"),
            "{none}"
        );

        let mut unreadable = String::new();
        render_restore_summary(
            &mut unreadable,
            &RestorePointProjection::Unreadable {
                reason: "permission denied".to_string(),
            },
        );
        assert!(unreadable.contains("could not be read"), "{unreadable}");
        assert!(
            unreadable.contains("unavailable rather than empty"),
            "{unreadable}"
        );

        let mut empty = String::new();
        render_restore_summary(
            &mut empty,
            &RestorePointProjection::Recorded {
                snapshots: Vec::new(),
            },
        );
        assert!(empty.contains("records no restore points yet"), "{empty}");

        let mut recorded = String::new();
        render_restore_summary(
            &mut recorded,
            &RestorePointProjection::Recorded {
                snapshots: vec![
                    snapshot(
                        &"a".repeat(40),
                        "pre-turn:2: second prompt",
                        1_700_000_100,
                        "pre-turn",
                        Some(2),
                    ),
                    snapshot(
                        &"b".repeat(40),
                        "pre-turn:1: first prompt",
                        1_700_000_000,
                        "pre-turn",
                        Some(1),
                    ),
                ],
            },
        );
        assert!(
            recorded.contains(
                "| 1 | `aaaaaaaaaaaa` | 2023-11-14T22:15:00Z | pre-turn:2: second prompt |"
            ),
            "{recorded}"
        );
        assert!(
            recorded.contains(
                "| 2 | `bbbbbbbbbbbb` | 2023-11-14T22:13:20Z | pre-turn:1: first prompt |"
            ),
            "{recorded}"
        );
    }

    #[test]
    fn correlation_matches_ambiguity_absence_and_role_rules() {
        let recorded = RestorePointProjection::Recorded {
            snapshots: vec![
                snapshot(
                    &"e".repeat(40),
                    "pre-turn:9: run the tests",
                    1_700_000_300,
                    "pre-turn",
                    Some(9),
                ),
                snapshot(
                    &"f".repeat(40),
                    "pre-turn:5: run the tests",
                    1_700_000_100,
                    "pre-turn",
                    Some(5),
                ),
                snapshot(
                    &"2".repeat(40),
                    "tool:call_abc: rename the widget",
                    1_700_000_000,
                    "tool",
                    None,
                ),
            ],
        };

        let ambiguous = correlation_markdown(&recorded, &user_message("run the tests"));
        assert!(
            ambiguous.contains("N1 `eeeeeeeeeeee` (pre-turn turn 9)"),
            "{ambiguous}"
        );
        assert!(
            ambiguous.contains("N2 `ffffffffffff` (pre-turn turn 5)"),
            "{ambiguous}"
        );
        assert!(ambiguous.contains("ambiguous"), "{ambiguous}");

        let none = correlation_markdown(&recorded, &user_message("never snapshotted"));
        assert!(none.contains("none recorded for this message"), "{none}");

        let tool_only = correlation_markdown(&recorded, &user_message("rename the widget"));
        assert!(
            tool_only.contains("none recorded for this message"),
            "{tool_only}"
        );

        let assistant = correlation_markdown(
            &recorded,
            &ExportMessage {
                is_user_role: false,
                role: "assistant".to_string(),
                blocks: vec![ExportBlock::Text {
                    text: "run the tests".to_string(),
                }],
                prompt_snippet: Some("run the tests".to_string()),
            },
        );
        assert!(assistant.is_empty(), "{assistant}");
    }

    #[test]
    fn correlation_requires_exact_user_identity_not_the_role_string() {
        let recorded = RestorePointProjection::Recorded {
            snapshots: vec![snapshot(
                "aaaaaaaaaaaa",
                "pre-turn:4: run the tests",
                5,
                "pre-turn",
                Some(4),
            )],
        };
        // Same rendered role string as a real user turn, but not `Role::User`.
        let unrecognized = ExportMessage {
            role: "user".to_string(),
            is_user_role: false,
            blocks: vec![ExportBlock::Text {
                text: "run the tests".to_string(),
            }],
            prompt_snippet: Some("run the tests".to_string()),
        };
        assert!(
            correlation_markdown(&recorded, &unrecognized).is_empty(),
            "a role that only renders as \"user\" must not correlate"
        );

        let real_user = ExportMessage {
            is_user_role: true,
            ..unrecognized
        };
        assert!(
            correlation_markdown(&recorded, &real_user).contains("N1 `aaaaaaaaaaaa`"),
            "an exact user turn still correlates"
        );
    }

    #[test]
    fn history_fallback_marks_literals_and_sanitizes_visible_bodies() {
        let mut out = String::new();
        render_history_fallback(
            &mut out,
            &[
                HistoryEntry::Sanitized {
                    role: "user".to_string(),
                    body: "hello\u{1b}[31m world".to_string(),
                },
                HistoryEntry::Literal {
                    role: "system".to_string(),
                    body: "[internal context omitted]".to_string(),
                },
            ],
        );
        assert!(out.contains("## 1. user\n\nhello world\n\n"), "{out}");
        assert!(
            out.contains("## 2. system\n\n[internal context omitted]\n\n"),
            "{out}"
        );

        let mut empty = String::new();
        render_history_fallback(&mut empty, &[]);
        assert_eq!(empty, "## Conversation\n\n[empty conversation]\n");
    }

    #[test]
    fn json_fence_tracks_longest_backtick_run_and_redacts_secrets() {
        let mut out = String::new();
        push_json(
            &mut out,
            serde_json::json!({"api_key": "literal-secret", "note": "``` inner"}),
        );
        assert!(!out.contains("literal-secret"), "{out}");
        assert!(out.starts_with("````json\n"), "{out}");
        assert!(out.contains("\"api_key\": \"[redacted]\""), "{out}");
    }

    #[test]
    fn empty_content_and_missing_blocks_use_baseline_markers() {
        let mut out = String::new();
        render_message(
            &mut out,
            1,
            ExportMessage {
                is_user_role: false,
                role: "assistant".to_string(),
                blocks: Vec::new(),
                prompt_snippet: None,
            },
        );
        assert!(out.contains("## 1. assistant\n\n[no content]\n\n"), "{out}");

        let mut empty_text = String::new();
        render_content_block(
            &mut empty_text,
            1,
            ExportBlock::Text {
                text: "   ".to_string(),
            },
        );
        assert!(empty_text.ends_with("[empty text]\n\n"), "{empty_text}");
    }

    #[test]
    fn parser_handles_whitespace_case_and_only_leading_force() {
        // Surrounding whitespace is trimmed, keyword matching is ASCII
        // case-insensitive, and only a leading `--force` is honored (the
        // baseline `strip_word` semantics).
        assert_eq!(
            parse_request(Some("  clipboard  ")).unwrap(),
            ExportRequest {
                scope: ExportScope::Conversation,
                destination: ExportDestination::Clipboard,
            }
        );
        assert_eq!(
            parse_request(Some("  TURN  ")).unwrap(),
            ExportRequest {
                scope: ExportScope::Turn,
                destination: ExportDestination::Clipboard,
            }
        );
        assert_eq!(
            parse_request(Some("  File   Report One.md  ")).unwrap(),
            ExportRequest {
                scope: ExportScope::Conversation,
                destination: ExportDestination::File {
                    path: "Report One.md".to_string(),
                    force: false,
                },
            }
        );
        assert_eq!(
            parse_request(Some("turn FILE --Force handoff.md")).unwrap(),
            ExportRequest {
                scope: ExportScope::Turn,
                destination: ExportDestination::File {
                    path: "handoff.md".to_string(),
                    force: true,
                },
            }
        );
        // A trailing `--force` is not a force flag; it becomes part of the
        // literal path exactly like the baseline parser.
        assert_eq!(
            parse_request(Some("file out.md --force")).unwrap(),
            ExportRequest {
                scope: ExportScope::Conversation,
                destination: ExportDestination::File {
                    path: "out.md --force".to_string(),
                    force: false,
                },
            }
        );
        // The turn branch keeps the same usage errors as the conversation one.
        assert_eq!(
            parse_request(Some("turn file")).unwrap_err(),
            export_usage("missing file path")
        );
        assert_eq!(
            parse_request(Some("turn file --force")).unwrap_err(),
            export_usage("missing file path")
        );
        assert_eq!(
            parse_request(Some("turn clipboard extra.md")).unwrap_err(),
            export_usage("clipboard does not accept a path")
        );
    }

    #[test]
    fn header_metadata_keeps_unsaved_and_workspace_fallbacks() {
        let projection = ConversationExportProjection {
            metadata: codewhale_command_contract::facets::ExportMetadata {
                session_label: "unsaved".to_string(),
                provider: "unknown".to_string(),
                model: "unknown".to_string(),
                mode: "agent".to_string(),
                workspace_name: "workspace".to_string(),
                message_count: 0,
                exported_at_unix: 0,
            },
            transcript: TranscriptProjection::Authoritative(Vec::new()),
            restore_points: RestorePointProjection::None,
        };

        let rendered = render_conversation(projection);

        assert!(
            rendered.contains("- Exported: 1970-01-01T00:00:00Z\n"),
            "{rendered}"
        );
        assert!(rendered.contains("- Session: unsaved\n"), "{rendered}");
        assert!(rendered.contains("- Provider: unknown\n"), "{rendered}");
        assert!(rendered.contains("- Model: unknown\n"), "{rendered}");
        assert!(rendered.contains("- Workspace: workspace\n"), "{rendered}");
        assert!(rendered.contains("- Messages: 0\n"), "{rendered}");
    }

    #[test]
    fn every_content_block_variant_renders_exactly() {
        // Image reference: URL credentials and sensitive query values are masked.
        let mut image = String::new();
        render_content_block(
            &mut image,
            1,
            ExportBlock::ImageReference {
                url: "https://alice:pw@example.com/a.png?api_key=hidden&ok=1".to_string(),
            },
        );
        assert_eq!(
            image,
            "### Content 1: Image attachment\n\n- Reference: https://***:***@example.com/a.png?api_key=***&ok=1\n\n"
        );

        // Inline/local image payloads became an omission marker at projection.
        let mut omitted = String::new();
        render_content_block(&mut omitted, 2, ExportBlock::ImageOmitted);
        assert_eq!(
            omitted,
            "### Content 2: Image attachment\n\n- Reference omitted (inline or local image payload)\n\n"
        );

        // Reasoning bodies and signatures are replaced by the baseline marker.
        let mut reasoning = String::new();
        render_content_block(&mut reasoning, 3, ExportBlock::InternalReasoning);
        assert_eq!(
            reasoning,
            "### Content 3: Internal reasoning\n\n[internal reasoning and signature omitted]\n\n"
        );

        // Tool call with caller metadata and redacted JSON input.
        let mut tool_call = String::new();
        render_content_block(
            &mut tool_call,
            4,
            ExportBlock::ToolCall {
                id: "call-1".to_string(),
                name: "fetch_url".to_string(),
                caller: Some(codewhale_command_contract::facets::ToolCallerProjection {
                    caller_type: "code_execution_20250825".to_string(),
                    tool_id: Some("server-tool-1".to_string()),
                }),
                input: serde_json::json!({"api_key": "literal-secret"}),
            },
        );
        assert_eq!(
            tool_call,
            "### Content 4: Tool call\n\n- ID: call-1\n- Name: fetch_url\n- Caller type: code_execution_20250825\n- Caller tool ID: server-tool-1\n\nInput:\n\n```json\n{\n  \"api_key\": \"[redacted]\"\n}\n```\n\n"
        );

        // Tool call without caller metadata omits only the caller lines.
        let mut bare_tool_call = String::new();
        render_content_block(
            &mut bare_tool_call,
            5,
            ExportBlock::ToolCall {
                id: "call-2".to_string(),
                name: "read_file".to_string(),
                caller: None,
                input: serde_json::json!({}),
            },
        );
        assert_eq!(
            bare_tool_call,
            "### Content 5: Tool call\n\n- ID: call-2\n- Name: read_file\n\nInput:\n\n```json\n{}\n```\n\n"
        );

        // Tool result with structured blocks: both the sanitized result text
        // and the redacted structured payload are rendered.
        let mut structured_result = String::new();
        render_content_block(
            &mut structured_result,
            6,
            ExportBlock::ToolResult {
                tool_use_id: "call-1".to_string(),
                content: "tool output line".to_string(),
                is_error: false,
                structured: Some(serde_json::json!([{"session_token": "session-secret"}])),
            },
        );
        assert_eq!(
            structured_result,
            "### Content 6: Tool result\n\n- Tool call ID: call-1\n- Error: false\n\nResult:\n\ntool output line\n\nStructured result blocks:\n\n```json\n[\n  {\n    \"session_token\": \"[redacted]\"\n  }\n]\n```\n\n"
        );

        // Tool result without structured blocks ends after the result text, and
        // an empty body keeps the baseline empty marker.
        let mut plain_result = String::new();
        render_content_block(
            &mut plain_result,
            7,
            ExportBlock::ToolResult {
                tool_use_id: "call-2".to_string(),
                content: String::new(),
                is_error: true,
                structured: None,
            },
        );
        assert_eq!(
            plain_result,
            "### Content 7: Tool result\n\n- Tool call ID: call-2\n- Error: true\n\nResult:\n\n[empty text]\n\n"
        );

        let mut server_call = String::new();
        render_content_block(
            &mut server_call,
            8,
            ExportBlock::ServerToolCall {
                id: "srv-1".to_string(),
                name: "web_search".to_string(),
                input: serde_json::json!({"query": "rust"}),
            },
        );
        assert_eq!(
            server_call,
            "### Content 8: Server tool call\n\n- ID: srv-1\n- Name: web_search\n\nInput:\n\n```json\n{\n  \"query\": \"rust\"\n}\n```\n\n"
        );

        let mut search_result = String::new();
        render_content_block(
            &mut search_result,
            9,
            ExportBlock::ToolSearchResult {
                tool_use_id: "search-1".to_string(),
                content: serde_json::json!({"results": []}),
            },
        );
        assert_eq!(
            search_result,
            "### Content 9: Tool-search result\n\n- Tool call ID: search-1\n\n```json\n{\n  \"results\": []\n}\n```\n\n"
        );

        let mut execution_result = String::new();
        render_content_block(
            &mut execution_result,
            10,
            ExportBlock::CodeExecutionResult {
                tool_use_id: "exec-1".to_string(),
                content: serde_json::json!({"stdout": "ok"}),
            },
        );
        assert_eq!(
            execution_result,
            "### Content 10: Code-execution result\n\n- Tool call ID: exec-1\n\n```json\n{\n  \"stdout\": \"ok\"\n}\n```\n\n"
        );
    }
}
