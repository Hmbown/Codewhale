//! Single portable formatter for the authoritative context source map and prompt schema.
//! Pressure and route-source verification are already classified by the host
//! and skipped by serde, so JSON retains its original public schema.

use codewhale_command_contract::facets::{DebugPromptContext, DebugPromptSourceMap};
use std::fmt::Write as _;

pub fn format_context_report(report: &DebugPromptSourceMap) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "Context Source Map");
    let _ = writeln!(
        out,
        "Estimated active context: {} tokens",
        report.active_context_estimated_tokens
    );
    write_overflow_guard_line(&mut out, report);
    match (report.context_window_tokens, report.budget_used_percent) {
        (Some(window), Some(percent)) => {
            let source = report
                .context_window_source
                .as_deref()
                .unwrap_or("fallback");
            // An unverified rung is a guess about the window printed on this
            // same line; it must not claim a fixed 128K default the capability
            // matrix may not hold. A label from no known rung is no evidence
            // either, so it reads the same way.
            let source_label = if report.context_window_verified {
                source.to_string()
            } else {
                format!(
                    "{source} (unverified — nothing describes this model, so this window is a guess)"
                )
            };
            let _ = writeln!(
                out,
                "Window: {window} tokens ({percent:.1}% used, {}; source: {})",
                report.pressure_label.as_str(),
                source_label
            );
        }
        _ => {
            let _ = writeln!(out, "Window: unknown");
        }
    }
    // #5134: the source label says where the window came from but not how to
    // change it. Name the key here so the report answers the question it
    // provokes.
    let _ = writeln!(
        out,
        "Change the window: set `context_window` on the active `[providers.<name>]` table in config.toml (docs/CONFIGURATION.md, \"Context length\")."
    );
    let _ = writeln!(
        out,
        "Source-entry total: {} tokens",
        report.total_estimated_tokens
    );
    let _ = writeln!(
        out,
        "Manage standing law: /constitution (status/preview), /constitution repo (repo-local law), /setup report (readiness)."
    );
    let _ = writeln!(out);
    let _ = writeln!(out, "Sources:");
    for entry in &report.entries {
        let path = entry
            .source_path
            .as_deref()
            .map(|path| format!(" [{path}]"))
            .unwrap_or_default();
        let tier = entry
            .authority_tier
            .map(|tier| format!(", tier {tier}"))
            .unwrap_or_default();
        let omitted = entry
            .truncation_reason
            .as_deref()
            .map(|reason| format!(" - {reason}"))
            .unwrap_or_default();
        let _ = writeln!(
            out,
            "- {:?}: {}{} - {} tokens ({:?}{}){}",
            entry.source_kind,
            entry.label,
            path,
            entry.estimated_tokens,
            entry.counting_confidence,
            tier,
            omitted
        );
    }
    let _ = writeln!(out);
    let _ = write!(out, "{}", report.note);
    out
}

/// Secondary line: the inflated overflow-guard figure, labeled so nobody
/// reads it as the pressure the meter and the compaction gate act on.
fn write_overflow_guard_line(out: &mut String, report: &DebugPromptSourceMap) {
    if let Some(guard) = report.overflow_guard_estimated_tokens {
        let _ = writeln!(
            out,
            "Overflow guard: {guard} tokens (conservative 1.5x estimate that blocks oversized requests; not the pressure the meter and auto-compaction read)"
        );
    }
}

pub fn format_context_summary(report: &DebugPromptSourceMap) -> String {
    let mut entries = report.entries.clone();
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.estimated_tokens));
    let top = entries
        .iter()
        .take(5)
        .map(|entry| format!("{} ({})", entry.label, entry.estimated_tokens))
        .collect::<Vec<_>>()
        .join(", ");

    let mut out = String::new();
    let _ = writeln!(out, "Context Summary");
    let _ = writeln!(out, "Pressure: {}", report.pressure_label.as_str());
    let _ = writeln!(
        out,
        "Estimated active context: {} tokens",
        report.active_context_estimated_tokens
    );
    if let Some(percent) = report.budget_used_percent {
        let _ = writeln!(out, "Budget used: {percent:.1}%");
    }
    write_overflow_guard_line(&mut out, report);
    let _ = write!(out, "Top sources: {top}");
    out
}

pub fn context_report_json(report: &DebugPromptSourceMap) -> String {
    serde_json::to_string_pretty(report).unwrap_or_else(|err| {
        format!("{{\"error\":\"failed to serialize context report: {err}\"}}")
    })
}

#[must_use]
pub fn prompt_context_json(context: &DebugPromptContext) -> String {
    serde_json::to_string_pretty(context).unwrap_or_else(|error| {
        format!(r#"{{"error":"failed to serialize prompt context: {error}"}}"#)
    })
}
