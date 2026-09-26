//! Tool-output pressure reported by `/status`.

use crate::artifacts::ArtifactRecord;

use codewhale_localization::{Locale, MessageId, tr};
use codewhale_models::{ContentBlock, Message};

/// Size above which `/status` counts a raw tool result in the conversation as
/// context pressure. What the model sees of each result is decided by the
/// route's inline budget (`route_budget::route_inline_char_budget`).
pub const RAW_TOOL_OUTPUT_RECEIPT_THRESHOLD_CHARS: usize = 12_000;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolOutputStatus {
    pub raw_large_count: usize,
    pub raw_large_chars: usize,
    pub receipt_count: usize,
    pub artifact_count: usize,
    pub artifact_bytes: u64,
}

pub fn tool_output_status(messages: &[Message], artifacts: &[ArtifactRecord]) -> ToolOutputStatus {
    let mut status = ToolOutputStatus {
        artifact_count: artifacts.len(),
        artifact_bytes: artifacts
            .iter()
            .map(|artifact| artifact.byte_size)
            .sum::<u64>(),
        ..ToolOutputStatus::default()
    };

    for message in messages {
        for block in &message.content {
            if let ContentBlock::ToolResult { content, .. } = block {
                if looks_like_receipt(content) {
                    status.receipt_count += 1;
                } else {
                    let chars = content.chars().count();
                    if chars > RAW_TOOL_OUTPUT_RECEIPT_THRESHOLD_CHARS {
                        status.raw_large_count += 1;
                        status.raw_large_chars = status.raw_large_chars.saturating_add(chars);
                    }
                }
            }
        }
    }

    status
}

pub fn format_tool_output_status(status: &ToolOutputStatus, locale: Locale) -> String {
    let mut parts = Vec::new();
    if status.raw_large_count > 0 {
        parts.push(
            tr(locale, MessageId::StatusToolRawPressure)
                .replace("{count}", &status.raw_large_count.to_string())
                .replace("{chars}", &format_count(status.raw_large_chars)),
        );
    }
    if status.receipt_count > 0 {
        parts.push(
            tr(locale, MessageId::StatusToolCompactReceipts)
                .replace("{count}", &status.receipt_count.to_string()),
        );
    }
    if status.artifact_count > 0 {
        parts.push(
            tr(locale, MessageId::StatusToolArtifacts)
                .replace("{count}", &status.artifact_count.to_string())
                .replace(
                    "{bytes}",
                    &crate::artifacts::format_byte_size(status.artifact_bytes),
                ),
        );
    }
    if parts.is_empty() {
        tr(locale, MessageId::StatusToolNone).into_owned()
    } else {
        parts.join("; ")
    }
}

fn looks_like_receipt(content: &str) -> bool {
    let trimmed = content.trim_start();
    trimmed.starts_with("[TOOL_OUTPUT_RECEIPT]")
        || trimmed.starts_with("[artifact:")
        || trimmed.starts_with("[TOOL_RESULT_TRUNCATED]")
        || trimmed.starts_with("<TOOL_RESULT_REF")
}

fn format_count(value: usize) -> String {
    value.to_string()
}

#[cfg(test)]
mod tests {
    use codewhale_models::Role;
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::artifacts::ArtifactKind;
    use chrono::Utc;

    fn tool_result_message(id: &str, content: &str) -> Message {
        Message {
            role: Role::User,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: id.to_string(),
                content: content.to_string(),
                is_error: None,
                content_blocks: None,
            }],
        }
    }

    fn artifact_record(tool_call_id: &str, raw: &str) -> ArtifactRecord {
        ArtifactRecord {
            id: crate::artifacts::artifact_id_for_tool_call(tool_call_id),
            kind: ArtifactKind::ToolOutput,
            session_id: "session-123".to_string(),
            tool_call_id: tool_call_id.to_string(),
            tool_name: "exec_shell".to_string(),
            created_at: Utc::now(),
            byte_size: raw.len() as u64,
            preview: "checking crate ... error[E0425]".to_string(),
            storage_path: PathBuf::from("artifacts").join("art_call-big.txt"),
        }
    }

    #[test]
    fn status_reports_raw_large_receipts_and_artifacts() {
        let raw = "RAW_STATUS\n".repeat(2_000);
        let receipt = "[TOOL_OUTPUT_RECEIPT]\ntruncation: raw output omitted — full output in the tool details view";
        let messages = vec![
            tool_result_message("call-raw", &raw),
            tool_result_message("call-receipt", receipt),
        ];
        let artifacts = vec![ArtifactRecord {
            storage_path: Path::new("artifacts/art_call-big.txt").to_path_buf(),
            ..artifact_record("call-big", &raw)
        }];

        let status = tool_output_status(&messages, &artifacts);
        assert_eq!(status.raw_large_count, 1);
        assert_eq!(status.receipt_count, 1);
        assert_eq!(status.artifact_count, 1);

        let rendered = format_tool_output_status(&status, Locale::En);
        assert!(rendered.contains("raw over cap"));
        assert!(rendered.contains("compact receipt"));
        assert!(rendered.contains("artifact"));
    }
}
