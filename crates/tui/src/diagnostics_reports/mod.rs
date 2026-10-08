//! Portable diagnostics formatting, shared by commands and non-command callers.
//! Request construction and classification stay with their existing TUI owners.

mod context;
mod money;
pub(crate) mod receipts;
mod tool_snapshot;

pub use money::format_cost_amount_precise;

pub use context::{
    context_report_json, format_context_report, format_context_summary, prompt_context_json,
};
pub use tool_snapshot::{render_tool_snapshot_json, render_tool_snapshot_text};
