//! User preference and impression receipts through the real terminal host.
//! Every process uses a sealed HOME; commands never submit a model request.

use std::time::Duration;

use super::qa_harness::{
    harness::{Harness, SealedWorkspace, make_sealed_workspace},
    keys,
};

const TIMEOUT: Duration = Duration::from_secs(10);

fn launch(workspace: &SealedWorkspace) -> Harness {
    let mut tui = Harness::builder(Harness::cargo_bin("codewhale-tui"))
        .cwd(workspace.workspace())
        .clear_env()
        .seal_home(workspace.home())
        .env("CODEWHALE_DISABLE_MODELS_DEV_FETCH", "1")
        .env("CODEWHALE_NO_UPDATE_CHECK", "1")
        .env("CODEWHALE_TELEMETRY", "0")
        .env("NO_ANIMATIONS", "1")
        .args([
            "--workspace",
            workspace.workspace().to_str().unwrap(),
            "--no-project-config",
            "--fresh",
        ])
        .size(32, 100)
        .spawn()
        .unwrap();
    tui.wait_for_composer(TIMEOUT).unwrap();
    tui
}

fn command(tui: &mut Harness, input: &str, expected: &str) {
    tui.paste(input).unwrap();
    tui.wait_for_text(&format!("❯ {input}"), TIMEOUT).unwrap();
    tui.send(keys::key::enter()).unwrap();
    tui.wait_for_text("Type a message", TIMEOUT).unwrap();
    tui.wait_for_text(expected, TIMEOUT).unwrap();
}

fn clear_draft(tui: &mut Harness) {
    tui.paste("unsent draft").unwrap();
    tui.wait_for_text("unsent draft", TIMEOUT).unwrap();
    tui.send(keys::key::ctrl('u')).unwrap();
    tui.wait_for_text("Type a message", TIMEOUT).unwrap();
    tui.wait_for_idle(Duration::from_millis(150), TIMEOUT)
        .unwrap();
}

fn settings(workspace: &SealedWorkspace) -> toml::Value {
    let path = workspace.home().join(".codewhale/settings.toml");
    toml::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn impressions(workspace: &SealedWorkspace) -> i64 {
    settings(workspace)
        .get("behavioral_tip_impressions")
        .and_then(|counts| counts.get("cleared_input_restore"))
        .and_then(toml::Value::as_integer)
        .unwrap_or(0)
}

#[test]
fn contextual_tips_opt_out_survives_restart_and_preserves_caps() {
    let workspace = make_sealed_workspace().unwrap();
    let mut tui = launch(&workspace);
    command(
        &mut tui,
        "/config contextual_tips off --save",
        "contextual_tips = false",
    );
    assert_eq!(
        settings(&workspace)["contextual_tips"].as_bool(),
        Some(false)
    );
    clear_draft(&mut tui);
    assert!(!tui.frame().contains("Cleared"));
    assert_eq!(impressions(&workspace), 0);
    tui.shutdown();

    let mut tui = launch(&workspace);
    command(
        &mut tui,
        "/config contextual_tips",
        "contextual_tips = false",
    );
    clear_draft(&mut tui);
    assert!(!tui.frame().contains("Cleared"));
    assert_eq!(impressions(&workspace), 0);

    // The existing Settings row supports pointer activation as well as the
    // command route. One click selects; the second activates the same row.
    tui.send(keys::key::f2()).unwrap();
    tui.wait_for_text("Settings", TIMEOUT).unwrap();
    for ch in "tips".chars() {
        tui.send(ch.to_string()).unwrap();
    }
    tui.wait_for_text("Search: tips", TIMEOUT).unwrap();
    tui.wait_for_text("Contextual tips", TIMEOUT).unwrap();
    let frame = tui.frame();
    let row = (0..frame.rows())
        .find(|&row| frame.row(row).contains("Contextual tips"))
        .unwrap();
    tui.send(keys::mouse::click(row, 4)).unwrap();
    tui.wait_for_idle(Duration::from_millis(150), TIMEOUT)
        .unwrap();
    assert_eq!(
        settings(&workspace)["contextual_tips"].as_bool(),
        Some(false)
    );
    tui.send(keys::mouse::click(row, 4)).unwrap();
    tui.wait_for(
        |frame| {
            (0..frame.rows()).any(|row| {
                let text = frame.row(row);
                text.contains("Contextual tips") && text.contains("On")
            })
        },
        TIMEOUT,
    )
    .unwrap();
    assert_eq!(
        settings(&workspace)["contextual_tips"].as_bool(),
        Some(true)
    );
    for _ in 0..2 {
        tui.send(keys::key::esc()).unwrap();
        tui.wait_for_idle(Duration::from_millis(150), TIMEOUT)
            .unwrap();
    }
    clear_draft(&mut tui);
    tui.wait_for_text("Cleared", TIMEOUT).unwrap();
    assert_eq!(impressions(&workspace), 1);
    clear_draft(&mut tui);
    assert_eq!(impressions(&workspace), 1);
    command(
        &mut tui,
        "/config contextual_tips off --save",
        "contextual_tips = false",
    );
    command(
        &mut tui,
        "/config contextual_tips on --save",
        "contextual_tips = true",
    );
    clear_draft(&mut tui);
    assert_eq!(
        impressions(&workspace),
        1,
        "toggle must not reset the session cap"
    );
    tui.shutdown();

    for expected in [2, 2] {
        let mut tui = launch(&workspace);
        command(
            &mut tui,
            "/config contextual_tips",
            "contextual_tips = true",
        );
        let before = impressions(&workspace);
        clear_draft(&mut tui);
        assert_eq!(impressions(&workspace), expected);
        if before == 2 {
            assert!(
                !tui.frame().contains("Cleared"),
                "lifetime cap still suppresses the tip"
            );
        }
        tui.shutdown();
    }
}

#[test]
fn contextual_tips_failed_save_is_visible_and_preserves_settings() {
    let workspace = make_sealed_workspace().unwrap();
    let mut tui = launch(&workspace);
    command(
        &mut tui,
        "/config contextual_tips",
        "contextual_tips = true",
    );
    let path = workspace.home().join(".codewhale/settings.toml");
    let malformed = "contextual_tips = [private_fixture_payload\n";
    std::fs::write(&path, malformed).unwrap();
    command(
        &mut tui,
        "/config contextual_tips off --save",
        "could not be saved",
    );
    assert!(!tui.frame().contains("private_fixture_payload"));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), malformed);
    command(
        &mut tui,
        "/config contextual_tips",
        "contextual_tips = false",
    );
    clear_draft(&mut tui);
    assert!(!tui.frame().contains("Cleared"));
    assert_eq!(std::fs::read_to_string(path).unwrap(), malformed);
    tui.shutdown();
}
