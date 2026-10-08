//! `/import-claude` — explicit, reviewable Claude Code migration (#5557).

use crate::commands::CommandResult;
use crate::import_claude::{self, McpCandidateLine};
use crate::tui::app::App;

pub(super) fn import_claude_command(app: &mut App, arg: Option<&str>) -> CommandResult {
    let apply = arg.map(str::trim).is_some_and(|arg| {
        arg.eq_ignore_ascii_case("--apply") || arg.eq_ignore_ascii_case("apply")
    });
    let home = crate::config::effective_home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let (sources, claude, settings) = import_claude::read_sources(&home);

    // MCP candidates come from the same discovery the `/mcp import` consent
    // flow uses, filtered to the Claude sources: provenance stays single-sourced.
    let markets = Vec::new();
    let mcp_candidates =
        crate::mcp::external_import::discover_external_sources(&home, &app.workspace, &markets)
            .into_iter()
            .filter(|candidate| {
                matches!(
                    candidate.source_kind,
                    crate::mcp::external_import::ExternalMcpSourceKind::ClaudeJson
                )
            })
            .map(|candidate| McpCandidateLine {
                summary: candidate.summary,
                hard_blocked: candidate.hard_blocked,
                name: candidate.name,
            })
            .collect::<Vec<_>>();

    let plan = import_claude::build_plan(sources, claude, settings, &home, mcp_candidates);
    if plan.is_empty() {
        return CommandResult {
            message: Some(
                "No Claude configuration found to import (looked for ~/.claude.json and \
                 ~/.claude/settings.json)."
                    .to_string(),
            ),
            action: None,
            is_error: false,
        };
    }

    // The plan is shown before anything is written; the only writes are the
    // report and an *unapplied* bundle file. Applying always goes through a
    // separate consent path (`/mcp import approve <review-token>`, `config import`).
    let imports_dir = codewhale_config::codewhale_home()
        .map(|home| home.join("imports"))
        .unwrap_or_else(|_| std::path::PathBuf::from(".codewhale/imports"));
    let report_path = imports_dir.join("claude-import-report.md");
    let bundle_path = imports_dir.join("claude-portable-bundle.json");
    let wrote_report = codewhale_config::persistence::atomic_write(
        &report_path,
        import_claude::report_markdown(&plan).as_bytes(),
    )
    .is_ok();
    let wrote_bundle = if plan.env_safe.is_empty() {
        false
    } else {
        codewhale_config::persistence::atomic_write(
            &bundle_path,
            import_claude::portable_bundle_json(&plan).as_bytes(),
        )
        .is_ok()
    };

    if apply {
        return apply_plan(&plan, &report_path, wrote_report);
    }

    let mut message = import_claude::render_plan(&plan);
    message.push_str(
        "\n\nNothing above is applied yet. `/import-claude --apply` carries over the \
         standing instructions. Review MCP servers individually with `/mcp import`; hooks and \
         permission rules stay manual because they run code.",
    );
    if wrote_report {
        message.push_str(&format!(
            "\nFull report: {}",
            crate::utils::display_path(&report_path)
        ));
    } else {
        message.push_str("\n(the report could not be written; the plan above is complete)");
    }
    if wrote_bundle {
        message.push_str(&format!(
            "\nPortable bundle (review, then `codewhale config import {}`): apply it there for the consent/rollback path.",
            crate::utils::display_path(&bundle_path)
        ));
    }
    CommandResult {
        message: Some(message),
        action: None,
        is_error: false,
    }
}

/// Carry over what can be carried over safely, and say exactly what was done.
///
/// `--apply` is the consent: the plan is printed first by the bare command, and
/// this path never overwrites an existing file, never imports MCP servers,
/// and never touches hooks or permission rules — those run code, so
/// they stay a human decision.
fn apply_plan(
    plan: &import_claude::ClaudeImportPlan,
    report_path: &std::path::Path,
    wrote_report: bool,
) -> CommandResult {
    let mut done: Vec<String> = Vec::new();
    let mut manual: Vec<String> = Vec::new();

    // 1. Standing instructions: a plain file copy, and only when the
    //    destination does not already exist. Clobbering the operator's own
    //    instructions would be the one unrecoverable thing here.
    if plan.has_claude_md {
        match codewhale_config::codewhale_home() {
            Ok(home) => {
                let destination = home.join("instructions.md");
                let source = crate::config::effective_home_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
                    .join(".claude")
                    .join("CLAUDE.md");
                if destination.exists() {
                    manual.push(format!(
                        "{} already exists — left untouched; merge from {} by hand if you want both.",
                        crate::utils::display_path(&destination),
                        crate::utils::display_path(&source)
                    ));
                } else {
                    match std::fs::read(&source) {
                        Ok(bytes)
                            if codewhale_config::persistence::atomic_write(
                                &destination,
                                &bytes,
                            )
                            .is_ok() =>
                        {
                            done.push(format!(
                                "Standing instructions copied to {}.",
                                crate::utils::display_path(&destination)
                            ));
                        }
                        _ => manual.push(format!(
                            "Could not copy {} — copy it to {} by hand.",
                            crate::utils::display_path(&source),
                            crate::utils::display_path(&destination)
                        )),
                    }
                }
            }
            Err(_) => manual.push(
                "Could not resolve the Codewhale home, so standing instructions were not copied."
                    .to_string(),
            ),
        }
    }

    // MCP approval must bind the exact bytes and current managed-config
    // revision shown by the shared preview. Blanket migration cannot grant it.
    for candidate in &plan.mcp_candidates {
        manual.push(if candidate.hard_blocked {
            format!("MCP `{}` is disabled at its source and was not imported.", candidate.name)
        } else {
            format!("MCP `{}` needs a separate review: run /mcp import, then use its displayed approval command. It will be imported OFF.", candidate.name)
        });
    }

    // 3. What stays human, always.
    if !plan.hook_events.is_empty() {
        manual.push(format!(
            "{} hook event(s) still need mapping with /hooks — hooks run code, so they are never imported for you.",
            plan.hook_events.len()
        ));
    }
    if !plan.permissions_allow.is_empty()
        || !plan.permissions_ask.is_empty()
        || !plan.permissions_deny.is_empty()
    {
        manual.push(
            "Permission rules are listed in the report; apply the ones you want with /permissions."
                .to_string(),
        );
    }

    let mut message = String::new();
    if done.is_empty() {
        message.push_str("Nothing was applied.\n");
    } else {
        message.push_str("Applied:\n");
        for line in &done {
            message.push_str(&format!("  · {line}\n"));
        }
    }
    if !manual.is_empty() {
        message.push_str("\nStill yours to do:\n");
        for line in &manual {
            message.push_str(&format!("  · {line}\n"));
        }
    }
    if !done.is_empty() {
        message.push_str("\nRun /mcp reload to connect the imported servers after reviewing them.");
    }
    if wrote_report {
        message.push_str(&format!(
            "\nFull report: {}",
            crate::utils::display_path(report_path)
        ));
    }
    CommandResult {
        message: Some(message),
        action: None,
        is_error: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::tui::app::TuiOptions;
    use std::fs;
    use tempfile::TempDir;

    /// A home with a Claude `CLAUDE.md`, plus a Codewhale home to import into.
    fn sandbox() -> (TempDir, App) {
        let temp = TempDir::new().expect("tempdir");
        let home = temp.path().join("home");
        let claude = home.join(".claude");
        fs::create_dir_all(&claude).expect("claude dir");
        fs::write(claude.join("CLAUDE.md"), b"be excellent\n").expect("CLAUDE.md");
        fs::write(claude.join("settings.json"), b"{}").expect("settings");
        fs::write(home.join(".claude.json"), b"{}").expect("claude.json");

        let workspace = temp.path().join("ws");
        fs::create_dir_all(&workspace).expect("workspace");
        let options = TuiOptions {
            config_path: Some(temp.path().join("config.toml")),
            skills_dir: temp.path().join("skills"),
            memory_path: temp.path().join("memory.md"),
            notes_path: temp.path().join("notes.txt"),
            mcp_config_path: temp.path().join("mcp.json"),
            ..crate::test_support::test_tui_options(&workspace)
        };
        let app = App::new(options, &Config::default());
        (temp, app)
    }

    #[test]
    fn the_bare_command_applies_nothing_and_says_how_to_apply() {
        let _lock = crate::test_support::lock_test_env();
        let (temp, mut app) = sandbox();
        let home = temp.path().join("home");
        let _home = crate::test_support::EnvVarGuard::set("HOME", &home);
        let _profile = crate::test_support::EnvVarGuard::set("USERPROFILE", &home);
        let cw_home = temp.path().join("cw");
        let _cw = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", &cw_home);

        let result = import_claude_command(&mut app, None);
        let message = result.message.expect("a plan");
        assert!(
            message.contains("/import-claude --apply"),
            "the plan must say how to apply it: {message}"
        );
        assert!(
            !cw_home.join("instructions.md").exists(),
            "the bare command must not write instructions.md"
        );
    }

    #[test]
    fn apply_carries_the_standing_instructions_and_never_clobbers_them() {
        let _lock = crate::test_support::lock_test_env();
        let (temp, mut app) = sandbox();
        let home = temp.path().join("home");
        let _home = crate::test_support::EnvVarGuard::set("HOME", &home);
        let _profile = crate::test_support::EnvVarGuard::set("USERPROFILE", &home);
        let cw_home = temp.path().join("cw");
        let _cw = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", &cw_home);

        let result = import_claude_command(&mut app, Some("--apply"));
        let message = result.message.expect("an outcome");
        let destination = cw_home.join("instructions.md");
        assert!(destination.exists(), "instructions were copied: {message}");
        assert_eq!(
            fs::read_to_string(&destination).expect("read"),
            "be excellent\n"
        );

        // Running it again must not overwrite the operator's own file.
        fs::write(&destination, b"mine now\n").expect("overwrite");
        let again = import_claude_command(&mut app, Some("--apply"));
        let message = again.message.expect("an outcome");
        assert_eq!(
            fs::read_to_string(&destination).expect("read"),
            "mine now\n",
            "an existing instructions.md is never clobbered: {message}"
        );
        assert!(
            message.contains("already exists"),
            "and the reason is said out loud: {message}"
        );
    }
}
