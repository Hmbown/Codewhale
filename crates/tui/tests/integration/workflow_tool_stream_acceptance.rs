use std::process::{Command, Output};

fn assert_terminal_stream_error(output: Output, expected_fragment: &str) {
    assert!(
        !output.status.success(),
        "workflow-tool unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("workflow-tool stdout is UTF-8");
    let events = stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str::<serde_json::Value>(line)
                .unwrap_or_else(|error| panic!("invalid stream JSON {line:?}: {error}"))
        })
        .collect::<Vec<_>>();
    let terminal = events.last().expect("terminal error event");
    assert_eq!(terminal["type"], "error", "events={events:?}");
    assert!(
        terminal["error"]
            .as_str()
            .is_some_and(|error| error.contains(expected_fragment)),
        "events={events:?}"
    );
    assert!(
        events.iter().all(|event| event["type"] != "tool_use"),
        "setup failure must happen before tool_use: {events:?}"
    );
}

#[test]
fn invalid_workflow_input_is_terminal_ndjson() {
    let output = Command::new(crate::binary::codewhale())
        .args([
            "workflow-tool",
            "--approval-source",
            "explicit-workflow-command",
            "--input-json",
            "{not-json",
        ])
        .output()
        .expect("run workflow-tool");
    assert_terminal_stream_error(output, "valid Workflow tool input object");
}

#[test]
fn missing_profile_is_terminal_ndjson() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = dir.path().join("config.toml");
    std::fs::write(&config, "provider = \"vllm\"\n").expect("write config");
    let output = Command::new(crate::binary::codewhale())
        .arg("--config")
        .arg(&config)
        .args([
            "--profile",
            "missing-profile",
            "workflow-tool",
            "--approval-source",
            "explicit-workflow-command",
            "--input-json",
            r#"{"action":"run"}"#,
        ])
        .env("CODEWHALE_HOME", dir.path().join("codewhale-home"))
        .output()
        .expect("run workflow-tool with missing profile");
    assert_terminal_stream_error(output, "Profile 'missing-profile' not found");
}

#[test]
fn profile_provider_switch_accepts_source_marked_cli_key_offline() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = dir.path().join("config.toml");
    std::fs::write(
        &config,
        r#"
provider = "deepseek"

[features]
mcp = false

[profiles.anthropic]
provider = "anthropic"
"#,
    )
    .expect("write profile config");
    let output = Command::new(crate::binary::codewhale())
        .arg("--config")
        .arg(&config)
        .args([
            "--profile",
            "anthropic",
            "workflow-tool",
            "--approval-source",
            "explicit-workflow-command",
            "--input-json",
            r#"{"action":"run","script":"phase('offline'); return { ok: true };"}"#,
        ])
        .env("CODEWHALE_HOME", dir.path().join("codewhale-home"))
        .env("DEEPSEEK_API_KEY_SOURCE", "cli")
        .env("CODEWHALE_CLI_API_KEY", "profile-switch-secret")
        .output()
        .expect("run profile-switched workflow-tool");

    assert!(
        output.status.success(),
        "workflow-tool failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("workflow-tool stdout is UTF-8");
    let event_types = stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str::<serde_json::Value>(line)
                .unwrap_or_else(|error| panic!("invalid stream JSON {line:?}: {error}"))
        })
        .map(|event| event["type"].as_str().unwrap_or_default().to_string())
        .collect::<Vec<_>>();
    assert!(event_types.iter().any(|kind| kind == "tool_use"));
    assert!(event_types.iter().any(|kind| kind == "tool_result"));
    assert_eq!(event_types.last().map(String::as_str), Some("done"));
    assert!(!stdout.contains("profile-switch-secret"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("profile-switch-secret"));
}
