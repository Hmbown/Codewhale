//! One plain-language line per gated tool call (E6).
//!
//! An approval card used to carry the model's raw JSON arguments and a static
//! tool description. [`approval_summary`] derives a short sentence from the
//! tool name and its arguments alone — never from model prose — so every
//! client can show the same first line ("Search the web for 'espresso'") and
//! put the raw arguments behind it. Paths are shown relative to the
//! workspace when they sit inside it.

use std::path::Path;

use codewhale_localization::{Locale, MessageId, tr};
use serde_json::Value;

/// Longest quoted argument a summary carries before it is cut with `…`.
const MAX_QUOTED_CHARS: usize = 80;

/// Summarize a gated tool call for an approval prompt, in English — the
/// wire form every runtime client receives.
#[must_use]
pub fn approval_summary(tool_name: &str, input: &Value, workspace: Option<&Path>) -> String {
    approval_summary_in(Locale::En, tool_name, input, workspace)
}

/// Summarize a gated tool call in `locale`, so a translated approval card
/// leads with the same plain sentence the English one does. Commands, paths,
/// URLs and queries are carried verbatim; only the sentence around them is
/// translated.
#[must_use]
pub fn approval_summary_in(
    locale: Locale,
    tool_name: &str,
    input: &Value,
    workspace: Option<&Path>,
) -> String {
    let name = crate::tools::canonical_action::canonical_action_alias(tool_name, input);
    let text = |key: &str| {
        input
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
    };
    let path = |key: &str| text(key).map(|raw| relative_path(raw, workspace));
    let msg = |id: MessageId| tr(locale, id).into_owned();
    let with = |id: MessageId, slot: &str, value: &str| {
        tr(locale, id).replace(&format!("{{{slot}}}"), value)
    };

    match name {
        "exec_shell" | "task_shell_start" => match text("command") {
            Some(command) => with(
                MessageId::ApprovalSummaryRunCommand,
                "command",
                &clip(command),
            ),
            None => msg(MessageId::ApprovalSummaryRunShell),
        },
        "exec_shell_wait" | "exec_wait" => msg(MessageId::ApprovalSummaryShellWait),
        "exec_shell_interact" | "exec_interact" => msg(MessageId::ApprovalSummaryShellInput),
        "exec_shell_cancel" => msg(MessageId::ApprovalSummaryShellStop),
        "write_file" => match path("path") {
            Some(path) => with(MessageId::ApprovalSummaryWritePath, "path", &path),
            None => msg(MessageId::ApprovalSummaryWriteFile),
        },
        "edit_file" | "fim_edit" => match path("path") {
            Some(path) => with(MessageId::ApprovalSummaryEditPath, "path", &path),
            None => msg(MessageId::ApprovalSummaryEditFile),
        },
        "apply_patch" => patch_summary(locale, input, workspace),
        "read_file" => match path("path") {
            Some(path) => with(MessageId::ApprovalSummaryReadPath, "path", &path),
            None => msg(MessageId::ApprovalSummaryReadFile),
        },
        "list_dir" => match path("path") {
            Some(path) => with(MessageId::ApprovalSummaryListPath, "path", &path),
            None => msg(MessageId::ApprovalSummaryListWorkspace),
        },
        "fetch_url" | "web.fetch" | "web_fetch" => match text("url") {
            Some(url) => with(MessageId::ApprovalSummaryFetchUrl, "url", &clip(url)),
            None => msg(MessageId::ApprovalSummaryFetchPage),
        },
        "web_search" => match text("query").or_else(|| text("q")) {
            Some(query) => with(MessageId::ApprovalSummarySearchQuery, "query", &clip(query)),
            None => msg(MessageId::ApprovalSummarySearchWeb),
        },
        "web.run" => web_run_summary(locale, input),
        name if name.starts_with("mcp_") => mcp_summary(locale, name),
        name => with(MessageId::ApprovalSummaryUseTool, "name", name),
    }
}

fn web_run_summary(locale: Locale, input: &Value) -> String {
    let first = |key: &str, field: &str| {
        input
            .get(key)
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(|item| item.get(field))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    };
    let count = |key: &str| input.get(key).and_then(Value::as_array).map_or(0, Vec::len);
    let more = |key: &str| match count(key) {
        0 | 1 => String::new(),
        n => tr(locale, MessageId::ApprovalSummaryMore).replace("{count}", &(n - 1).to_string()),
    };
    let with = |id: MessageId, slot: &str, value: &str| {
        tr(locale, id).replace(&format!("{{{slot}}}"), &clip(value))
    };
    if let Some(query) = first("search_query", "q") {
        return with(MessageId::ApprovalSummarySearchQuery, "query", &query)
            + &more("search_query");
    }
    if let Some(query) = first("image_query", "q") {
        return with(MessageId::ApprovalSummaryImageQuery, "query", &query) + &more("image_query");
    }
    if let Some(target) = first("open", "ref_id") {
        return with(MessageId::ApprovalSummaryOpenTarget, "target", &target) + &more("open");
    }
    if count("click") > 0 {
        return tr(locale, MessageId::ApprovalSummaryFollowLink).into_owned();
    }
    if let Some(pattern) = first("find", "pattern") {
        return with(MessageId::ApprovalSummaryFindOnPage, "pattern", &pattern);
    }
    if count("screenshot") > 0 {
        return tr(locale, MessageId::ApprovalSummaryScreenshot).into_owned();
    }
    tr(locale, MessageId::ApprovalSummaryBrowse).into_owned()
}

fn patch_summary(locale: Locale, input: &Value, workspace: Option<&Path>) -> String {
    let apply_patch = || tr(locale, MessageId::ApprovalSummaryApplyPatch).into_owned();
    let Ok(preflight) = crate::tools::apply_patch::preflight_apply_patch(input) else {
        return apply_patch();
    };
    let mut paths: Vec<String> = preflight
        .touched_files
        .iter()
        .map(|raw| relative_path(raw, workspace))
        .collect();
    paths.sort_unstable();
    paths.dedup();
    match paths.as_slice() {
        [] => apply_patch(),
        [one] => tr(locale, MessageId::ApprovalSummaryEditPath).replace("{path}", one),
        [first, rest @ ..] => tr(locale, MessageId::ApprovalSummaryEditPathMore)
            .replace("{path}", first)
            .replace("{count}", &rest.len().to_string()),
    }
}

fn mcp_summary(locale: Locale, name: &str) -> String {
    // `mcp_<server>_<tool>`; server names may themselves hold `_`, so this is
    // presentation only and never a policy decision.
    let rest = name.trim_start_matches("mcp_");
    match rest.split_once('_') {
        Some((server, tool)) if !server.is_empty() && !tool.is_empty() => {
            tr(locale, MessageId::ApprovalSummaryMcpTool)
                .replace("{tool}", tool)
                .replace("{server}", server)
        }
        _ => tr(locale, MessageId::ApprovalSummaryUseName).replace("{name}", rest),
    }
}

/// Show `raw` relative to `workspace` when it names a path inside it.
fn relative_path(raw: &str, workspace: Option<&Path>) -> String {
    let candidate = Path::new(raw);
    if let Some(workspace) = workspace
        && candidate.has_root()
        && let Ok(relative) = candidate.strip_prefix(workspace)
    {
        let shown = relative.display().to_string();
        return if shown.is_empty() {
            ".".to_string()
        } else {
            clip(&shown)
        };
    }
    // A relative path is already workspace-relative; drop a leading `./`.
    if let Ok(relative) = candidate.strip_prefix(".")
        && !relative.as_os_str().is_empty()
    {
        return clip(&relative.display().to_string());
    }
    clip(raw)
}

fn clip(value: &str) -> String {
    // Keep line breaks visible: `a\nb` joined with a space would read as one
    // command with arguments on an approval card.
    let single_line = value
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ⏎ ");
    if single_line.chars().count() <= MAX_QUOTED_CHARS {
        return single_line;
    }
    let mut clipped: String = single_line.chars().take(MAX_QUOTED_CHARS - 1).collect();
    clipped.push('…');
    clipped
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn web_run_search_names_the_query() {
        let summary = approval_summary(
            "web.run",
            &json!({"search_query": [{"q": "best espresso machine"}, {"q": "reviews"}]}),
            None,
        );
        assert_eq!(
            summary,
            "Search the web for 'best espresso machine' (+1 more)"
        );
    }

    #[test]
    fn file_paths_are_workspace_relative() {
        let workspace = Path::new("/work/repo");
        assert_eq!(
            approval_summary(
                "write_file",
                &json!({"path": "/work/repo/notes/espresso.md", "content": "x"}),
                Some(workspace),
            ),
            "Write notes/espresso.md"
        );
        // Outside the workspace stays absolute, so the card never hides where
        // a write lands.
        assert_eq!(
            approval_summary("edit_file", &json!({"path": "/etc/hosts"}), Some(workspace)),
            "Edit /etc/hosts"
        );
        // The model-facing `File{action}` family resolves to the same line.
        assert_eq!(
            approval_summary(
                "File",
                &json!({"action": "write", "path": "/work/repo/a.txt"}),
                Some(workspace),
            ),
            "Write a.txt"
        );
    }

    #[test]
    fn shell_and_fallbacks_are_plain_and_bounded() {
        assert_eq!(
            approval_summary(
                "exec_shell",
                &json!({"command": "cargo  test  -p tui"}),
                None
            ),
            "Run `cargo test -p tui`"
        );
        assert_eq!(
            approval_summary(
                "exec_shell",
                &json!({"command": "cargo test\n\nrm -rf target"}),
                None
            ),
            "Run `cargo test ⏎ rm -rf target`",
            "a second command line never reads as arguments of the first"
        );
        let long = "x".repeat(500);
        let summary = approval_summary("exec_shell", &json!({ "command": long }), None);
        assert!(summary.chars().count() < 100, "{summary}");
        assert_eq!(
            approval_summary("mcp_github_create_issue", &json!({}), None),
            "Use create_issue from github"
        );
        assert_eq!(
            approval_summary("some_tool", &json!({"a": 1}), None),
            "Use the some_tool tool"
        );
    }

    /// A translated card leads with the same sentence, translated around the
    /// verbatim command, path and query (experience mark 4).
    #[test]
    fn summaries_translate_the_sentence_and_keep_the_arguments_verbatim() {
        let workspace = Path::new("/work/repo");
        assert_eq!(
            approval_summary_in(
                Locale::De,
                "exec_shell",
                &json!({"command": "cargo test"}),
                None
            ),
            "`cargo test` ausführen"
        );
        assert_eq!(
            approval_summary_in(
                Locale::Ja,
                "write_file",
                &json!({"path": "/work/repo/notes/espresso.md"}),
                Some(workspace),
            ),
            "notes/espresso.md を書き込み"
        );
        assert_eq!(
            approval_summary_in(
                Locale::ZhHans,
                "web.run",
                &json!({"search_query": [{"q": "espresso"}, {"q": "grinder"}]}),
                None,
            ),
            "在网上搜索“espresso”（另 1 项）"
        );
        assert_eq!(
            approval_summary_in(Locale::Fr, "mcp_github_create_issue", &json!({}), None),
            "Utiliser create_issue de github"
        );
        // Every summary a non-English pack produces is its own sentence, never
        // the English one leaking through the fallback.
        for (tool, input) in [
            ("exec_shell", json!({})),
            ("write_file", json!({})),
            ("edit_file", json!({"path": "a.rs"})),
            ("read_file", json!({})),
            ("list_dir", json!({})),
            ("fetch_url", json!({})),
            ("web_search", json!({"query": "x"})),
            ("web.run", json!({})),
            ("some_tool", json!({})),
        ] {
            let english = approval_summary(tool, &input, None);
            for locale in [
                Locale::De,
                Locale::Ja,
                Locale::Ru,
                Locale::Hi,
                Locale::ZhHant,
            ] {
                let translated = approval_summary_in(locale, tool, &input, None);
                assert_ne!(translated, english, "{locale:?} {tool}");
                assert!(!translated.contains('{'), "{locale:?} {tool}: {translated}");
            }
        }
    }
}
