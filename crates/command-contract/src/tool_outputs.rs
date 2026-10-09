//! Pure output-pressure display; observation remains host-owned.
use crate::config_policy::StatusToolOutputs as ToolOutputStatus;

pub struct ToolOutputLabels {
    pub raw_pressure: String,
    pub compact_receipts: String,
    pub artifacts: String,
    pub none: String,
}

pub fn format_tool_output_status(status: &ToolOutputStatus, labels: &ToolOutputLabels) -> String {
    let mut parts = Vec::new();
    if status.raw_large_count > 0 {
        parts.push(
            labels
                .raw_pressure
                .clone()
                .replace("{count}", &status.raw_large_count.to_string())
                .replace("{chars}", &status.raw_large_chars.to_string()),
        );
    }
    if status.receipt_count > 0 {
        parts.push(
            labels
                .compact_receipts
                .clone()
                .replace("{count}", &status.receipt_count.to_string()),
        );
    }
    if status.artifact_count > 0 {
        parts.push(
            labels
                .artifacts
                .clone()
                .replace("{count}", &status.artifact_count.to_string())
                .replace(
                    "{bytes}",
                    &codewhale_protocol::display::format_byte_size(status.artifact_bytes),
                ),
        );
    }
    if parts.is_empty() {
        labels.none.clone()
    } else {
        parts.join("; ")
    }
}
