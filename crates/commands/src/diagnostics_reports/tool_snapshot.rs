//! Single portable renderer for the prepared-tool request projection.
//!
//! The authoritative request capture and bounding policy remain TUI-owned.
//! This module formats the already-bounded, contract-owned semantic snapshot;
//! no registry, App, engine or client is reachable from it.

use codewhale_command_contract::facets::{
    DebugBoundedString as BoundedString, DebugEvidence as Evidence,
    DebugProviderAvailability as ProviderAvailability, DebugToolProvenance, DebugToolSnapshot,
    DebugToolVisibility,
};

#[must_use]
pub fn render_tool_snapshot_text(snapshot: &DebugToolSnapshot) -> String {
    let mut out = String::new();
    out.push_str("Prepared Model-Client Tool Request (read-only)\n");
    out.push_str(&format!("Capture source: {}\n", snapshot.capture_source));
    out.push_str(&format!("Delivery: {}\n", snapshot.delivery_status));
    out.push_str(&format!(
        "Turn: {}\nTurn truncated: {}\n",
        json_string(&snapshot.turn_id.value),
        yes_no(snapshot.turn_id.truncated)
    ));
    out.push_str(&format!("Step: {}\n", snapshot.step));
    if let Some(terminal) = &snapshot.terminal {
        out.push_str("Terminal diagnostics (observed facts; effective_max_steps null means uncapped, other nulls mean unknown):\n");
        if let Ok(json) = serde_json::to_string_pretty(terminal) {
            out.push_str(&json);
            out.push('\n');
        }
    }
    out.push_str(&format!(
        "Tools field: {}\nTool count: {}\n",
        if snapshot.tools_field_present {
            "present"
        } else {
            "absent"
        },
        snapshot.tool_count
    ));
    out.push_str(&format!(
        "Rendered tools: {}; omitted by render bound: {}\n",
        snapshot.rendered_tool_count, snapshot.omitted_tool_count
    ));
    out.push_str(&format!(
        "Model-client payload measurement: {}\n",
        snapshot.payload_measurement_status
    ));
    out.push_str(&format_optional_usize(
        "Model-client tool JSON bytes",
        snapshot.payload_json_bytes,
    ));
    out.push_str(&format_optional_string(
        "Active tool catalog digest (same digest as the request manifest)",
        snapshot.active_tool_catalog_sha256.as_deref(),
    ));
    out.push_str(
            "Provider-wire tool payload: unavailable (the provider adapter may transform or omit model-client fields)\n",
        );
    match &snapshot.provider {
        ProviderAvailability::Available { provider, model } => out.push_str(&format!(
            "Provider: {} (resolved model client)\nModel: {}\n",
            json_string(provider),
            json_string(model)
        )),
        ProviderAvailability::Unavailable { reason } => {
            out.push_str(&format!("Provider: unavailable ({reason})\n"));
        }
        ProviderAvailability::Unknown => {
            out.push_str("Provider: unknown (no model-client receipt captured)\n");
        }
    }
    out.push_str(&format!(
        "Registry facts: {}\n",
        if snapshot.registry_facts_present {
            "captured"
        } else {
            "not captured"
        }
    ));
    match &snapshot.registry_tool_count {
        Evidence::Known { value } => {
            out.push_str(&format!("Registered tools: {value}\n"));
        }
        Evidence::Unknown { reason } => {
            out.push_str(&format!("Registered tools: unknown ({reason})\n"));
        }
    }
    match &snapshot.registry_only_tools {
        Evidence::Known { value } => {
            let rendered = value
                .rendered
                .iter()
                .map(|entry| entry.value.as_str())
                .collect::<Vec<_>>();
            out.push_str(&format!(
                    "Model-visible tools not in this request: {}\n  names: {}\n  omitted by render bound: {}\n",
                    value.count,
                    serde_json::to_string(&rendered).unwrap_or_else(|_| "unavailable".to_string()),
                    value.omitted
                ));
        }
        Evidence::Unknown { reason } => {
            out.push_str(&format!(
                "Model-visible tools not in this request: unknown ({reason})\n"
            ));
        }
    }
    out.push_str(&format!(
        "Unavailable for this request: {}\n",
        snapshot.unavailable_for_this_request.join(", ")
    ));

    for tool in &snapshot.tools {
        out.push_str(&format!(
            "\n{}. {}\n",
            tool.ordinal,
            json_string(&tool.name.value)
        ));
        out.push_str(&format!(
            "   name truncated: {}\n",
            yes_no(tool.name.truncated)
        ));
        render_bounded_evidence(&mut out, "type", &tool.tool_type);
        out.push_str(&format!(
            "   description: {}\n   description truncated: {}\n",
            json_string(&tool.description.value),
            yes_no(tool.description.truncated)
        ));
        out.push_str(&format!(
            "   input schema JSON: {}\n   input schema truncated: {}\n",
            tool.input_schema_json.value,
            yes_no(tool.input_schema_json.truncated)
        ));
        match &tool.allowed_callers {
            Evidence::Known { value } => {
                let rendered = value
                    .rendered
                    .iter()
                    .map(|entry| entry.value.as_str())
                    .collect::<Vec<_>>();
                out.push_str(&format!(
                        "   allowed callers: {}\n   allowed callers count: {}\n   allowed callers omitted: {}\n   allowed callers truncated: {}\n",
                        serde_json::to_string(&rendered).unwrap_or_else(|_| "unavailable".to_string()),
                        value.count,
                        value.omitted,
                        yes_no(value.rendered.iter().any(|entry| entry.truncated))
                    ));
            }
            Evidence::Unknown { reason } => {
                out.push_str(&format!("   allowed callers: unknown ({reason})\n"));
            }
        }
        render_bool_evidence(&mut out, "deferred loading", &tool.defer_loading);
        render_bool_evidence(&mut out, "strict", &tool.strict);
        match &tool.input_examples {
            Evidence::Known { value } => out.push_str(&format!(
                "   input examples: present ({} value(s), {})\n",
                value.count, value.values
            )),
            Evidence::Unknown { reason } => {
                out.push_str(&format!("   input examples: unknown ({reason})\n"));
            }
        }
        render_bounded_evidence(&mut out, "cache control type", &tool.cache_control_type);
        out.push_str(&format!(
            "   request state: {}\n   in request: {}\n",
            visibility_label(tool.visibility),
            yes_no(visibility_in_request(tool.visibility))
        ));
        match &tool.provenance {
            Evidence::Known { value } => {
                out.push_str(&format!("   provenance: {}\n", provenance_label(*value)));
            }
            Evidence::Unknown { reason } => {
                out.push_str(&format!("   provenance: unknown ({reason})\n"));
            }
        }
        render_bounded_evidence(&mut out, "MCP server", &tool.mcp_server);
        match &tool.capabilities {
            Evidence::Known { value } => {
                let rendered = value
                    .rendered
                    .iter()
                    .map(|entry| entry.value.as_str())
                    .collect::<Vec<_>>();
                out.push_str(&format!(
                    "   capabilities: {}\n   capabilities count: {}\n   capabilities omitted: {}\n",
                    serde_json::to_string(&rendered).unwrap_or_else(|_| "unavailable".to_string()),
                    value.count,
                    value.omitted
                ));
            }
            Evidence::Unknown { reason } => {
                out.push_str(&format!("   capabilities: unknown ({reason})\n"));
            }
        }
        render_bounded_evidence(&mut out, "approval", &tool.approval);
        render_bool_evidence(&mut out, "model visible", &tool.model_visible);
    }
    out
}

pub fn render_tool_snapshot_json(
    snapshot: &DebugToolSnapshot,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(snapshot)
}

const fn visibility_label(value: DebugToolVisibility) -> &'static str {
    match value {
        DebugToolVisibility::Active => "active",
        DebugToolVisibility::Deferred => "deferred",
        DebugToolVisibility::InRequest => "in-request",
    }
}

const fn visibility_in_request(value: DebugToolVisibility) -> bool {
    matches!(
        value,
        DebugToolVisibility::Active
            | DebugToolVisibility::Deferred
            | DebugToolVisibility::InRequest
    )
}

const fn provenance_label(value: DebugToolProvenance) -> &'static str {
    match value {
        DebugToolProvenance::Builtin => "builtin",
        DebugToolProvenance::Plugin => "plugin",
        DebugToolProvenance::Mcp => "mcp",
        DebugToolProvenance::Synthetic => "synthetic",
        DebugToolProvenance::Unknown => "unknown",
    }
}

fn render_bounded_evidence(out: &mut String, label: &str, evidence: &Evidence<BoundedString>) {
    match evidence {
        Evidence::Known { value } => out.push_str(&format!(
            "   {label}: {}\n   {label} truncated: {}\n",
            json_string(&value.value),
            yes_no(value.truncated)
        )),
        Evidence::Unknown { reason } => {
            out.push_str(&format!("   {label}: unknown ({reason})\n"));
        }
    }
}

fn render_bool_evidence(out: &mut String, label: &str, evidence: &Evidence<bool>) {
    match evidence {
        Evidence::Known { value } => out.push_str(&format!("   {label}: {value}\n")),
        Evidence::Unknown { reason } => {
            out.push_str(&format!("   {label}: unknown ({reason})\n"));
        }
    }
}

fn json_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"unavailable\"".to_string())
}

fn format_optional_usize(label: &str, value: Option<usize>) -> String {
    value.map_or_else(
        || format!("{label}: unavailable\n"),
        |value| format!("{label}: {value}\n"),
    )
}

fn format_optional_string(label: &str, value: Option<&str>) -> String {
    value.map_or_else(
        || format!("{label}: unavailable\n"),
        |value| format!("{label}: {value}\n"),
    )
}

const fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}
