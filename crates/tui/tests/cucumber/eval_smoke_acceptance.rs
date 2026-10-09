//! Gherkin acceptance test: eval smoke test.
//!
//! Verifies that the binary loads and the eval harness reports step-level
//! success for a shell command after Layer 4.2 registry cleanup. Follows the
//! proven `core_session_command_extraction.rs` pattern.
//!
//! NOTE: This is an eval smoke test, not a command-surface verification
//! (AT-004) test. It confirms the binary starts and runs eval correctly.
//! For AT-004 command-surface coverage (help, palette, completion), see the
//! focused unit tests in command_palette.rs, widgets/mod.rs, and
//! commands/mod.rs.

use std::process::{Command, ExitStatus};

use cucumber::{World as _, given, then, when, writer::Stats as _};
use serde_json::Value;
use tempfile::TempDir;

const FEATURE_NAME: &str = "Eval smoke test (binary load and eval step reporting)";
const FEATURE_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/features/eval_smoke.feature"
);
const SMOKE_SCENARIO: &str = "Binary loads and reports step-level success via eval";

#[derive(Debug, Default, cucumber::World)]
struct EvalSmokeWorld {
    _record_dir: Option<TempDir>,
    report: Option<Value>,
    exit_status: Option<ExitStatus>,
}

#[given("a clean CodeWhale evaluation workspace")]
fn clean_codewhale_evaluation_workspace(world: &mut EvalSmokeWorld) {
    world._record_dir = Some(TempDir::new().expect("evaluation TempDir"));
}

#[when("the evaluation harness runs a shell command")]
fn eval_harness_runs_shell_command(world: &mut EvalSmokeWorld) {
    let record_dir = world
        ._record_dir
        .as_ref()
        .expect("evaluation workspace should exist");

    let output = Command::new(crate::binary::codewhale())
        .args([
            "eval",
            "--json",
            "--shell-command",
            "echo eval-smoke-test",
            "--record",
        ])
        .arg(record_dir.path())
        .output()
        .expect("codewhale eval should start");

    // Capture stdout/stderr for diagnostics
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let report: Value = serde_json::from_str(&stdout).unwrap_or_else(|err| {
        panic!("eval --json should emit valid JSON: {err}\nstdout:\n{stdout}\nstderr:\n{stderr}")
    });

    world.exit_status = Some(output.status);
    world.report = Some(report);
}

#[then("the binary exits without crashing")]
fn binary_exits_without_crashing(world: &mut EvalSmokeWorld) {
    let status = world
        .exit_status
        .expect("exit status should have been captured");

    // The eval harness exits with code 1 when `metrics.success` is false
    // (run_eval in main.rs uses `bail!("...")` for non-successful scenarios).
    // This is expected behavior: the eval runs a multi-step scenario offline
    // (List, Read, Search, Edit, ApplyPatch, ExecShell) and the overall
    // metrics.success reflects all steps, not just Bash. The Bash
    // step itself succeeds — see `json_report_contains_execution_steps`.
    //
    // What we verify here:
    //   1. The process ran to completion (was killed by no signal)
    //   2. A known exit code was produced (not a crash/hang)
    //   3. Step-level success is validated by the next Gherkin step.
    let exit_code = status.code().expect("process should have terminated");
    assert_no_signal_crash(&status);
    assert!(
        exit_code == 0 || exit_code == 1,
        "codewhale eval exited with unexpected code {exit_code} (expected 0 or 1)"
    );

    let report = world.report.as_ref().expect("eval report should exist");
    let steps = report
        .get("steps")
        .and_then(|value| value.as_array())
        .expect("eval report should have a 'steps' array");
    assert!(
        !steps.is_empty(),
        "eval report should have at least one step"
    );
}

#[then("the JSON report contains execution steps")]
fn json_report_contains_execution_steps(world: &mut EvalSmokeWorld) {
    let report = world.report.as_ref().expect("eval report should exist");
    let steps = report
        .get("steps")
        .and_then(|value| value.as_array())
        .expect("eval report should have a 'steps' array");

    // Find the Bash step and verify it contains the expected output
    let exec_step = steps
        .iter()
        .find(|step| step.get("kind").and_then(|v| v.as_str()) == Some("Bash"))
        .expect("eval report should have a Bash step");

    let step_success = exec_step
        .get("success")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    assert!(step_success, "Bash step should succeed, got: {exec_step:?}");

    let output = exec_step
        .get("output")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(
        output.contains("eval-smoke-test"),
        "Bash output should contain the shell command echo, got: {output}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn eval_smoke_binary_loads_and_reports_steps() {
    let writer = EvalSmokeWorld::cucumber()
        .fail_on_skipped()
        .with_default_cli()
        .filter_run(FEATURE_PATH, move |feature, _, scenario| {
            feature.name == FEATURE_NAME && scenario.name == SMOKE_SCENARIO
        })
        .await;
    assert_eq!(
        writer.failed_steps(),
        0,
        "scenario failed: {SMOKE_SCENARIO}"
    );
    assert_eq!(
        writer.skipped_steps(),
        0,
        "scenario skipped steps: {SMOKE_SCENARIO}"
    );
    assert_eq!(
        writer.passed_steps(),
        4,
        "scenario did not run: {SMOKE_SCENARIO}"
    );
}

/// Assert the process was not killed by a signal (Unix-only check).
#[cfg(unix)]
fn assert_no_signal_crash(status: &ExitStatus) {
    use std::os::unix::process::ExitStatusExt;
    assert!(
        status.signal().is_none(),
        "codewhale eval was killed by signal {} (crash?)",
        status.signal().unwrap()
    );
}

/// No-op on non-Unix platforms where `ExitStatusExt` is unavailable.
#[cfg(not(unix))]
fn assert_no_signal_crash(_status: &ExitStatus) {}
