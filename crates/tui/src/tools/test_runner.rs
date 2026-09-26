//! Cargo test runner tool: `run_tests`.
//!
//! `cargo test` runs workspace code, so this tool follows the same explicit
//! approval policy as the other code-executing tools.

use std::path::Path;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::cargo_failure_summary::summarize_cargo_failure;
use super::shell_output::truncate_head_tail_chars;
use super::spec::{
    ApprovalRequirement, ToolCapability, ToolContext, ToolError, ToolResult, ToolSpec,
    optional_bool, optional_str,
};

use crate::dependencies::ExternalTool;

const MAX_OUTPUT_CHARS: usize = 40_000;

/// Tool for running `cargo test` in the workspace root.
pub struct RunTestsTool;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RunTestsOutput {
    success: bool,
    exit_code: i32,
    stdout: String,
    stderr: String,
    command: String,
}

#[async_trait]
impl ToolSpec for RunTestsTool {
    fn name(&self) -> &'static str {
        "run_tests"
    }

    fn model_visible(&self) -> bool {
        false
    }

    fn description(&self) -> &'static str {
        "Run `cargo test` in the workspace root with optional extra arguments."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "args": {
                    "type": "string",
                    "description": "Optional extra arguments to pass to `cargo test` (shell-style)."
                },
                "all_features": {
                    "type": "boolean",
                    "description": "When true, include `--all-features`."
                },
                "cwd": {
                    "type": "string",
                    "description": "Optional working directory, relative to the workspace, to run `cargo test` in. Must exist inside the workspace."
                }
            },
            "additionalProperties": false
        })
    }

    fn capabilities(&self) -> Vec<ToolCapability> {
        vec![ToolCapability::ExecutesCode, ToolCapability::Sandboxable]
    }

    fn approval_requirement(&self) -> ApprovalRequirement {
        // `run_tests` declares `ToolCapability::ExecutesCode` — match the
        // default approval policy for code-executing tools.
        ApprovalRequirement::Required
    }

    async fn execute(&self, input: Value, context: &ToolContext) -> Result<ToolResult, ToolError> {
        crate::core::engine::tool_catalog::enforce_tool_denial(context, self.name(), &input)?;
        let all_features = optional_bool(&input, "all_features", false)?;
        let extra_args = optional_str(&input, "args")?
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let workdir = match optional_str(&input, "cwd")?
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            None => context.workspace.clone(),
            Some(raw) => context.resolve_existing_dir(raw, "cwd")?,
        };

        let mut args = vec!["test".to_string()];
        if all_features {
            args.push("--all-features".to_string());
        }
        if let Some(extra) = extra_args {
            let split = shlex::split(extra).ok_or_else(|| {
                ToolError::invalid_input("Failed to parse 'args' as shell-style tokens")
            })?;
            args.extend(split);
        }

        let command_str = format_command(&workdir, &args);
        let output = run_cargo(&workdir, &args)?;

        let exit_code = output.status.code().unwrap_or(-1);
        let stdout_raw = String::from_utf8_lossy(&output.stdout);
        let stderr_raw = String::from_utf8_lossy(&output.stderr);
        let stdout = truncate_head_tail_chars(&stdout_raw, MAX_OUTPUT_CHARS).0;
        let stderr = truncate_head_tail_chars(&stderr_raw, MAX_OUTPUT_CHARS).0;

        let result = RunTestsOutput {
            success: output.status.success(),
            exit_code,
            stdout,
            stderr,
            command: command_str,
        };

        let mut tool_result =
            ToolResult::json(&result).map_err(|e| ToolError::execution_failed(e.to_string()))?;
        if let Some(summary) = summarize_cargo_failure(
            &result.command,
            &result.stdout,
            &result.stderr,
            Some(result.exit_code),
        ) {
            tool_result = tool_result.with_metadata(json!({
                "summary": summary.summary,
                "cargo_failure_summary": summary.to_metadata_value(),
            }));
        }
        Ok(tool_result)
    }
}

// === Helpers ===

fn run_cargo(workspace: &Path, args: &[String]) -> Result<std::process::Output, ToolError> {
    let Some(mut cmd) = crate::dependencies::Cargo::command() else {
        return Err(ToolError::not_available(
            "cargo is not installed or not in PATH",
        ));
    };
    cmd.args(args).current_dir(workspace);
    cmd.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ToolError::not_available("cargo is not installed or not in PATH")
        } else {
            ToolError::execution_failed(format!("Failed to run cargo: {e}"))
        }
    })
}

fn format_command(workspace: &Path, args: &[String]) -> String {
    format!(
        "(cd {} && cargo {})",
        workspace.display(),
        args.iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(" ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};
    use tempfile::tempdir;

    static NEXT_CARGO_PROJECT: AtomicU64 = AtomicU64::new(0);

    fn cargo_available() -> bool {
        Command::new("cargo")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn init_cargo_project(root: &Path) -> std::path::PathBuf {
        let project_dir = root.join("project");
        let package_name = format!(
            "eval_project_{}_{}",
            std::process::id(),
            NEXT_CARGO_PROJECT.fetch_add(1, Ordering::Relaxed)
        );
        fs::create_dir_all(&project_dir).expect("create project dir");
        let status = crate::dependencies::Cargo::command()
            .expect("cargo not found")
            .args(["init", "--lib", "--vcs", "none", "-q"])
            .arg("--name")
            .arg(package_name)
            .current_dir(&project_dir)
            .status()
            .expect("cargo should spawn");
        assert!(status.success(), "cargo init failed");
        project_dir
    }

    /// `run_tests` is `ToolCapability::ExecutesCode`, so it must follow the
    /// explicit-approval policy that applies to other code-executing tools.
    #[test]
    fn run_tests_requires_user_approval() {
        let tool = RunTestsTool;
        assert_eq!(
            tool.approval_requirement(),
            ApprovalRequirement::Required,
            "run_tests must gate cargo test behind user approval"
        );
    }

    #[tokio::test]
    async fn run_tests_succeeds_on_fresh_project() {
        if !cargo_available() {
            return;
        }
        let tmp = tempdir().expect("tempdir");
        // Release jobs commonly export one CARGO_TARGET_DIR for the whole
        // workspace. Give concurrent nested Cargo fixtures distinct package
        // identities so their test artifacts cannot replace each other.
        let project_dir = init_cargo_project(tmp.path());

        let ctx = ToolContext::new(&project_dir);
        let tool = RunTestsTool;
        let result = tool.execute(json!({}), &ctx).await.expect("execute");
        assert!(result.success);

        let parsed: RunTestsOutput =
            serde_json::from_str(&result.content).expect("tool result should be json");
        assert!(
            parsed.success,
            "nested cargo test unexpectedly failed:\n{}",
            parsed.stderr
        );
        assert_eq!(parsed.exit_code, 0);
        assert!(parsed.command.contains("cargo test"));
    }

    #[tokio::test]
    async fn run_tests_reports_failures_without_hard_error() {
        if !cargo_available() {
            return;
        }
        let tmp = tempdir().expect("tempdir");
        let project_dir = init_cargo_project(tmp.path());

        let lib_rs = project_dir.join("src/lib.rs");
        let failing = r#"
pub fn add(a: i32, b: i32) -> i32 { a + b }

#[cfg(test)]
mod tests {
    #[test]
    fn fails() {
        assert_eq!(2 + 2, 5);
    }
}
"#;
        fs::write(&lib_rs, failing).expect("write failing test");

        let ctx = ToolContext::new(&project_dir);
        let tool = RunTestsTool;
        let result = tool.execute(json!({}), &ctx).await.expect("execute");
        assert!(result.success);

        let parsed: RunTestsOutput =
            serde_json::from_str(&result.content).expect("tool result should be json");
        assert!(
            !parsed.success,
            "nested cargo test unexpectedly passed:\nstdout:\n{}\nstderr:\n{}",
            parsed.stdout, parsed.stderr
        );
        assert_ne!(parsed.exit_code, 0);
        let metadata = result.metadata.expect("metadata");
        assert_eq!(
            metadata["cargo_failure_summary"]["kind"],
            json!("test_failure")
        );
        assert!(
            metadata["cargo_failure_summary"]["summary"]
                .as_str()
                .unwrap()
                .contains("Failing tests:")
        );
    }

    #[test]
    fn truncation_adds_note() {
        let long = "x".repeat(MAX_OUTPUT_CHARS + 128) + "test result: FAILED";
        let (truncated, _, _) = truncate_head_tail_chars(&long, MAX_OUTPUT_CHARS);
        assert!(truncated.contains("output truncated"));
        assert!(truncated.ends_with("test result: FAILED"));
    }

    /// A child parked at a workspace root that is not the project root (the
    /// #6296 verifier) runs the suite where the manifest lives instead of
    /// failing on cwd.
    #[tokio::test]
    async fn run_tests_cwd_scopes_cargo_to_subdir() {
        if !cargo_available() {
            return;
        }
        let tmp = tempdir().expect("tempdir");
        let project_dir = init_cargo_project(tmp.path());

        let ctx = ToolContext::new(tmp.path());
        let result = RunTestsTool
            .execute(json!({"cwd": "project"}), &ctx)
            .await
            .expect("cwd-scoped execute");
        assert!(result.success);

        let parsed: RunTestsOutput =
            serde_json::from_str(&result.content).expect("tool result should be json");
        assert!(
            parsed.success,
            "nested cargo test unexpectedly failed:\\n{}",
            parsed.stderr
        );
        // `resolve_existing_dir` returns the canonical path, which on Windows
        // carries the `\\?\` verbatim prefix the raw tempdir lacks (#6346).
        let scoped_dir = project_dir.canonicalize().expect("canonical project dir");
        assert!(
            parsed.command.contains(&scoped_dir.display().to_string()),
            "cargo must run in the scoped dir, ran: {}",
            parsed.command
        );
    }

    #[tokio::test]
    async fn run_tests_cwd_fails_closed_with_a_named_fallback() {
        let tmp = tempdir().expect("tempdir");
        let ctx = ToolContext::new(tmp.path());

        let escape = RunTestsTool
            .execute(json!({"cwd": "../escape"}), &ctx)
            .await
            .expect_err("workspace escape must be refused");
        assert!(escape.to_string().contains("escapes workspace"), "{escape}");

        let missing = RunTestsTool
            .execute(json!({"cwd": "no-such-dir"}), &ctx)
            .await
            .expect_err("missing dir must be refused");
        let message = missing.to_string();
        assert!(message.contains("not an existing directory"), "{message}");
        assert!(message.contains("drop `cwd`"), "{message}");
    }
}
