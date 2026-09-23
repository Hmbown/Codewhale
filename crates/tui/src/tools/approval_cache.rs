//! Approval fingerprint keys (§5.A).
//!
//! Instead of caching by tool name alone (which would let an approved
//! `exec_shell "cat foo"` silently pass `exec_shell "rm -rf /"`), the
//! approval flow uses a **call fingerprint** — a digest of the tool name
//! and the semantically‑relevant portion of its arguments.
//!
//! ## Two fingerprint shapes
//!
//! There are two key flavours, used for opposite sides of the decision:
//!
//! * [`build_approval_key`] — an **exact** digest of the full arguments.
//!   Used to scope *denials* so that denying one call (e.g. `rm -rf /tmp/x`)
//!   does not also suppress a later, different call to the same tool (#1617).
//!
//!   | Tool           | Exact key                                |
//!   |---------------|------------------------------------------|
//!   | file writes    | `file:<tool_name>:<hash of args>`        |
//!   | shell tools    | `shell:<tool_name>:<hash of args>`       |
//!   | `fetch_url`    | `net:<hostname>`                         |
//!   | everything else| `tool:<tool_name>:<hash of input>`       |
//!
//! * [`build_approval_grouping_key`] — a **lossy / arity-aware** digest.
//!   Used to scope *approvals* so that approving `cargo build` for the
//!   session also covers `cargo build --release` (the v0.8.37 behaviour).
//!
//!   | Tool           | Grouping key                             |
//!   |---------------|------------------------------------------|
//!   | `apply_patch`  | `patch:<hash of file paths>`             |
//!   | shell tools    | `shell:<command prefix>`                 |
//!   | `fetch_url`    | `net:<hostname>`                         |
//!   | everything else| `tool:<tool_name>:<hash of input>`       |
//!
use std::fmt::Write as _;

use serde_json::Value;
use sha2::{Digest, Sha256};

use codewhale_execpolicy::command_safety::classify_command;

/// The fingerprint of a tool call — stable enough to match repeated
/// calls but specific enough to avoid privilege confusion.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ApprovalKey(pub String);

/// Build the approval‑cache key for a tool call.
///
/// The key incorporates the tool name and a canonical digest of the
/// arguments so that denying one call suppresses exact retries, not later
/// invocations of the same tool with different parameters.
#[must_use]
pub fn build_approval_key(tool_name: &str, input: &serde_json::Value) -> ApprovalKey {
    let tool_name = crate::tools::canonical_action::canonical_action_alias(tool_name, input);
    let fingerprint = match tool_name {
        "apply_patch" | "write_file" | "edit_file" | "fim_edit" => {
            format!("file:{tool_name}:{}", hash_json_value(input))
        }
        "exec_shell"
        | "task_shell_start"
        | "exec_shell_wait"
        | "exec_shell_interact"
        | "exec_wait"
        | "exec_interact" => {
            format!("shell:{tool_name}:{}", hash_json_value(input))
        }
        "fetch_url" | "web.fetch" | "web_fetch" => {
            let host = parse_host(input);
            format!("net:{host}")
        }
        _ => format!("tool:{tool_name}:{}", hash_json_value(input)),
    };
    ApprovalKey(fingerprint)
}

/// Build the **grouping** approval key for a tool call.
///
/// Unlike [`build_approval_key`], this collapses argument variants of the
/// same command family onto one key (the v0.8.37 behaviour) so that an
/// "approve for session" decision covers later invocations that differ only
/// by flags. Denials must keep using the exact [`build_approval_key`].
#[must_use]
pub fn build_approval_grouping_key(tool_name: &str, input: &serde_json::Value) -> ApprovalKey {
    let tool_name = crate::tools::canonical_action::canonical_action_alias(tool_name, input);
    let fingerprint = match tool_name {
        "apply_patch" => {
            let paths_hash = hash_patch_paths(input);
            format!("patch:{paths_hash}")
        }
        "exec_shell"
        | "task_shell_start"
        | "exec_shell_wait"
        | "exec_shell_interact"
        | "exec_wait"
        | "exec_interact" => {
            let prefix = command_prefix(input);
            format!("shell:{prefix}")
        }
        "fetch_url" | "web.fetch" | "web_fetch" => {
            let host = parse_host(input);
            format!("net:{host}")
        }
        // MCP tools are reviewed as kinds: a trusted plugin bundle's MCP
        // tools were human-reviewed at trust time, so the session grant the
        // approval card offers (`2` — "approves for the session") is the
        // reviewed kind, `mcp:<tool>`. Hashing the full params here would
        // make every exact-argument variant its own family and silently
        // narrow the granted kind into a one-call grant (the regression the
        // plugin e2e acceptance catches). Shell keeps its command-family
        // key (R2); this arm never widens shell or file tools.
        name if crate::mcp::McpPool::is_mcp_tool(name) => format!("mcp:{name}"),
        _ => format!("tool:{tool_name}:{}", hash_json_value(input)),
    };
    ApprovalKey(fingerprint)
}

/// Return the canonical command prefix for the shell command in `input`.
///
/// Uses [`classify_command`] from the arity dictionary so that approving
/// `git status` also covers `git status -s` / `git status --porcelain`
/// without also covering `git push`.
fn command_prefix(input: &serde_json::Value) -> String {
    let cmd = input.get("command").and_then(|v| v.as_str()).unwrap_or("");
    let tokens: Vec<&str> = cmd.split_whitespace().collect();
    if tokens.is_empty() {
        return "<empty>".to_string();
    }
    classify_command(&tokens)
}

/// Hash the sorted set of file paths referenced by a patch input.
///
/// The paths come from [`preflight_apply_patch`] — the same resolver the
/// executor, the permission path (`core/engine.rs`) and auto-review already
/// use — rather than from a second, weaker parser. That matters because this
/// string *is* the scope of an "approve for the session" grant: two patches
/// share a grant exactly when they share this key.
///
/// The previous implementation read only `+++ b/` headers and the
/// `replace`/`changes` array, so it saw no paths at all for the documented
/// `apply_patch{path, patch}` override, for `--no-prefix` diffs, or for
/// delete-only diffs — and collapsed all of them to one shared constant.
/// Approving any one of those pre-approved every later one, to any file
/// (#6247).
///
/// An input the resolver cannot parse gets a digest of the input itself, not
/// a shared constant: an unparseable patch is its own family and matches
/// nothing but a byte-identical repeat.
fn hash_patch_paths(input: &serde_json::Value) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let Ok(preflight) = crate::tools::apply_patch::preflight_apply_patch(input) else {
        return format!("unparsed_{}", hash_json_value(input));
    };

    let mut paths: Vec<&str> = preflight.touched_files.iter().map(String::as_str).collect();

    paths.sort_unstable();
    paths.dedup();

    if paths.is_empty() {
        // The resolver parsed the input but found no target. Fail closed for
        // the same reason as the error arm above: a shared key here is a
        // shared grant.
        return format!("no_target_{}", hash_json_value(input));
    }

    let mut hasher = DefaultHasher::new();
    for path in &paths {
        path.hash(&mut hasher);
    }
    format!("{:x}", hasher.finish())
}

/// Parse the host portion from a URL input.
fn parse_host(input: &serde_json::Value) -> String {
    let url = input.get("url").and_then(|v| v.as_str()).unwrap_or("");

    if let Ok(parsed) = reqwest::Url::parse(url) {
        parsed.host_str().unwrap_or(url).to_string()
    } else {
        url.to_string()
    }
}

fn hash_json_value(value: &Value) -> String {
    let mut canonical = String::new();
    push_canonical_json(value, &mut canonical);

    let digest = Sha256::digest(canonical.as_bytes());
    let mut short = String::with_capacity(16);
    for byte in &digest[..8] {
        write!(&mut short, "{byte:02x}").expect("writing to String cannot fail");
    }
    short
}

/// Maximum nesting depth the canonical serializer descends. Aligned with
/// serde_json's own parse limit so parsed input never truncates; anything
/// deeper emits a fixed marker, keeping keys deterministic.
const MAX_CANONICAL_JSON_DEPTH: usize = 128;

fn push_canonical_json(value: &Value, out: &mut String) {
    push_canonical_json_at(value, out, 0)
}

fn push_canonical_json_at(value: &Value, out: &mut String, depth: usize) {
    if depth > MAX_CANONICAL_JSON_DEPTH {
        out.push_str("maxdepth");
        return;
    }
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(value) => {
            out.push_str("bool:");
            out.push_str(if *value { "true" } else { "false" });
        }
        Value::Number(value) => {
            out.push_str("number:");
            // Avoid allocating via value.to_string().
            if let Some(n) = value.as_f64() {
                let _ = write!(out, "{n}");
            } else if let Some(n) = value.as_i64() {
                let _ = write!(out, "{n}");
            } else if let Some(n) = value.as_u64() {
                let _ = write!(out, "{n}");
            } else {
                out.push_str(&value.to_string());
            }
        }
        Value::String(value) => {
            out.push_str("string:");
            // Emit JSON-encoded string without an intermediate allocation.
            out.push('"');
            for ch in value.chars() {
                match ch {
                    '"' => out.push_str("\\\""),
                    '\\' => out.push_str("\\\\"),
                    '\n' => out.push_str("\\n"),
                    '\r' => out.push_str("\\r"),
                    '\t' => out.push_str("\\t"),
                    c if c.is_control() => {
                        let _ = write!(out, "\\u{:04x}", c as u32);
                    }
                    c => out.push(c),
                }
            }
            out.push('"');
        }
        Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                push_canonical_json_at(item, out, depth + 1);
            }
            out.push(']');
        }
        Value::Object(map) => {
            let mut entries = map.iter().collect::<Vec<_>>();
            entries.sort_by_key(|(key, _)| *key);

            out.push('{');
            for (index, (key, value)) in entries.into_iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                let encoded_key =
                    serde_json::to_string(key).expect("serializing an object key cannot fail");
                out.push_str(&encoded_key);
                out.push(':');
                push_canonical_json_at(value, out, depth + 1);
            }
            out.push('}');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn different_commands_different_keys() {
        let key_a = build_approval_key("exec_shell", &json!({"command": "ls"}));
        let key_b = build_approval_key("exec_shell", &json!({"command": "rm -rf /tmp"}));
        assert_ne!(key_a, key_b);
    }

    #[test]
    fn same_command_same_key() {
        let key_a = build_approval_key("exec_shell", &json!({"command": "cargo build --release"}));
        let key_b = build_approval_key("exec_shell", &json!({"command": "cargo build --release"}));
        assert_eq!(key_a, key_b);
    }

    #[test]
    fn pathological_nesting_yields_a_stable_key() {
        let mut value = Value::String("leaf".to_string());
        for _ in 0..150 {
            let mut map = serde_json::Map::new();
            map.insert("t".to_string(), value);
            value = Value::Object(map);
        }
        let key_a = build_approval_key("exec_shell", &value);
        let key_b = build_approval_key("exec_shell", &value);
        assert_eq!(key_a, key_b, "truncated keys must stay deterministic");
    }

    #[test]
    fn shell_keys_include_full_command_arguments() {
        let key_a = build_approval_key("exec_shell", &json!({"command": "cargo build"}));
        let key_b = build_approval_key("exec_shell", &json!({"command": "cargo build --release"}));
        assert_ne!(key_a, key_b);
    }

    #[test]
    fn grouping_key_collapses_shell_flag_variants() {
        let key_a = build_approval_grouping_key("exec_shell", &json!({"command": "cargo build"}));
        let key_b =
            build_approval_grouping_key("exec_shell", &json!({"command": "cargo build --release"}));
        assert_eq!(
            key_a, key_b,
            "approving a command family must cover later flag variants"
        );
    }

    #[test]
    fn grouping_key_grants_mcp_tools_as_reviewed_kinds() {
        // A session grant for a reviewed plugin MCP tool is the kind
        // (`mcp:<tool>`), not the exact arguments: the plugin e2e acceptance
        // approves the echo kind once and later variants of the same reviewed
        // tool must not re-prompt. R2's shell command-family scoping is
        // untouched — this is the MCP arm only.
        let key_a = build_approval_grouping_key(
            "mcp_plugin-4-demo-local_echo",
            &json!({"text": "acceptance", "hang": false}),
        );
        let key_b = build_approval_grouping_key(
            "mcp_plugin-4-demo-local_echo",
            &json!({"text": "acceptance", "hang": true}),
        );
        assert_eq!(
            key_a, key_b,
            "a reviewed MCP kind grant covers argument variants of that tool"
        );
        let key_c = build_approval_grouping_key("mcp_plugin-4-demo-local_kick", &json!({"x": 1}));
        assert_ne!(key_a, key_c, "a different MCP tool is a different kind");
        // The exact-call key stays per-arguments so denials still suppress
        // only exact retries.
        let exact_a = build_approval_key(
            "mcp_plugin-4-demo-local_echo",
            &json!({"text": "acceptance", "hang": false}),
        );
        let exact_b = build_approval_key(
            "mcp_plugin-4-demo-local_echo",
            &json!({"text": "acceptance", "hang": true}),
        );
        assert_ne!(exact_a, exact_b, "denial keys remain argument-exact");
    }

    #[test]
    fn grouping_key_still_separates_distinct_commands() {
        let key_a = build_approval_grouping_key("exec_shell", &json!({"command": "git status"}));
        let key_b = build_approval_grouping_key("exec_shell", &json!({"command": "git push"}));
        assert_ne!(key_a, key_b);
    }

    /// #6247. The `path` override is the documented way to patch without
    /// diff headers (`apply_patch.rs` tells the model "Ensure the patch
    /// includes ---/+++ headers or provide `path`"), and a bare hunk has no
    /// `+++` line at all. Before the fix both of these produced the constant
    /// `patch:no_files`, so one session grant covered every later one.
    #[test]
    fn grouping_key_scopes_a_path_override_to_its_own_file() {
        let hunk = "@@ -1 +1 @@\n-old\n+new\n";
        let benign = build_approval_grouping_key(
            "apply_patch",
            &json!({"path": ".env.example", "patch": hunk}),
        );
        let sensitive = build_approval_grouping_key(
            "apply_patch",
            &json!({"path": ".codewhale/settings.json", "patch": hunk}),
        );
        assert_ne!(
            benign, sensitive,
            "approving a patch to one file must never cover a patch to another"
        );
        assert!(
            !format!("{benign:?}").contains("no_files"),
            "a resolvable target must never collapse to the shared constant"
        );
    }

    /// The executor's `normalize_diff_path` accepts a prefix-less header, so
    /// the fingerprint must too — otherwise a `--no-prefix` diff is a second
    /// route to the shared key.
    #[test]
    fn grouping_key_reads_prefix_less_diff_headers() {
        let prefixed = build_approval_grouping_key(
            "apply_patch",
            &json!({"patch": "--- a/src/auth.rs\n+++ b/src/auth.rs\n@@ -1 +1 @@\n-a\n+b\n"}),
        );
        let bare = build_approval_grouping_key(
            "apply_patch",
            &json!({"patch": "--- src/auth.rs\n+++ src/auth.rs\n@@ -1 +1 @@\n-a\n+b\n"}),
        );
        assert_eq!(
            prefixed, bare,
            "the same target written two legal ways is one approval family"
        );
        let other = build_approval_grouping_key(
            "apply_patch",
            &json!({"patch": "--- src/billing.rs\n+++ src/billing.rs\n@@ -1 +1 @@\n-a\n+b\n"}),
        );
        assert_ne!(bare, other, "different targets are different families");
    }

    /// Fail closed: an input the resolver cannot parse is its own family, not
    /// a member of a shared one. Two different unparseable inputs must not
    /// share a grant.
    #[test]
    fn grouping_key_fails_closed_on_an_unresolvable_patch() {
        let a = build_approval_grouping_key("apply_patch", &json!({"patch": "not a diff at all"}));
        let b = build_approval_grouping_key("apply_patch", &json!({"patch": "also not a diff"}));
        assert_ne!(a, b, "unparseable inputs must not share an approval family");
        for key in [&a, &b] {
            let rendered = format!("{key:?}");
            assert!(
                !rendered.contains("no_files"),
                "the shared constant must not survive anywhere: {rendered}"
            );
        }
    }

    #[test]
    fn grouping_key_collapses_patch_body_for_same_path() {
        let key_a = build_approval_grouping_key(
            "apply_patch",
            &json!({"replace": [{"path": "a.rs", "content": "x"}]}),
        );
        let key_b = build_approval_grouping_key(
            "apply_patch",
            &json!({"replace": [{"path": "a.rs", "content": "y"}]}),
        );
        assert_eq!(
            key_a, key_b,
            "approving a patch family must cover later edits to the same path"
        );
    }

    #[test]
    fn grouping_key_treats_replace_and_legacy_changes_as_the_same_path_set() {
        let canonical = build_approval_grouping_key(
            "apply_patch",
            &json!({"replace": [{"path": "a.rs", "content": "new"}]}),
        );
        let legacy = build_approval_grouping_key(
            "apply_patch",
            &json!({"changes": [{"path": "a.rs", "content": "new"}]}),
        );

        assert_eq!(canonical, legacy);
    }

    #[test]
    fn denial_key_stays_exact_while_grouping_key_collapses() {
        let exact_a = build_approval_key("exec_shell", &json!({"command": "cargo build"}));
        let exact_b =
            build_approval_key("exec_shell", &json!({"command": "cargo build --release"}));
        assert_ne!(exact_a, exact_b, "denials must remain exact-call scoped");

        let group_a = build_approval_grouping_key("exec_shell", &json!({"command": "cargo build"}));
        let group_b =
            build_approval_grouping_key("exec_shell", &json!({"command": "cargo build --release"}));
        assert_eq!(group_a, group_b, "approvals must group by command family");
    }

    #[test]
    fn patch_keys_differ_by_path() {
        let key_a = build_approval_key(
            "apply_patch",
            &json!({"replace": [{"path": "a.rs", "content": "x"}]}),
        );
        let key_b = build_approval_key(
            "apply_patch",
            &json!({"replace": [{"path": "b.rs", "content": "x"}]}),
        );
        assert_ne!(key_a, key_b);
    }

    #[test]
    fn patch_keys_differ_by_body_for_same_path() {
        let key_a = build_approval_key(
            "apply_patch",
            &json!({"replace": [{"path": "a.rs", "content": "x"}]}),
        );
        let key_b = build_approval_key(
            "apply_patch",
            &json!({"replace": [{"path": "a.rs", "content": "y"}]}),
        );
        assert_ne!(key_a, key_b);
    }

    #[test]
    fn net_keys_differ_by_host() {
        let key_a = build_approval_key("fetch_url", &json!({"url": "https://example.com"}));
        let key_b = build_approval_key("fetch_url", &json!({"url": "https://other.org"}));
        assert_ne!(key_a, key_b);
    }

    #[test]
    fn generic_tool_keys_include_arguments() {
        let key_a = build_approval_key("read_file", &json!({"path": "a.txt"}));
        let key_b = build_approval_key("read_file", &json!({"path": "b.txt"}));
        assert_ne!(key_a, key_b);
        assert!(key_a.0.starts_with("tool:read_file:"));
    }

    #[test]
    fn generic_tool_same_arguments_reuse_key() {
        let input = json!({"path": "a.txt"});
        let key_a = build_approval_key("edit_file", &input);
        let key_b = build_approval_key("edit_file", &input);
        assert_eq!(key_a, key_b);
    }

    #[test]
    fn input_hash_is_stable_across_object_key_order() {
        let key_a = build_approval_key("write_file", &json!({"path": "a.txt", "content": "x"}));
        let key_b = build_approval_key("write_file", &json!({"content": "x", "path": "a.txt"}));
        assert_eq!(key_a, key_b);
    }

    #[test]
    fn lowercase_primitives_share_legacy_approval_keys() {
        let shell = json!({"command": "cargo test"});
        assert_eq!(
            build_approval_key("bash", &shell),
            build_approval_key("exec_shell", &shell)
        );
        let write = json!({"path": "a.txt", "content": "x"});
        assert_eq!(
            build_approval_key("write", &write),
            build_approval_key("write_file", &write)
        );
        let edit = json!({
            "path": "a.txt",
            "edits": [{"oldText": "x", "newText": "y"}]
        });
        assert_eq!(
            build_approval_key("edit", &edit),
            build_approval_key("edit_file", &edit)
        );
    }

    #[test]
    fn canonical_json_omits_trailing_commas() {
        let mut canonical = String::new();
        push_canonical_json(&json!({"b": [true, false], "a": {"x": 1}}), &mut canonical);

        assert_eq!(
            canonical,
            r#"{"a":{"x":number:1},"b":[bool:true,bool:false]}"#
        );
        assert!(!canonical.contains(",]"));
        assert!(!canonical.contains(",}"));
    }
}
