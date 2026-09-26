//! The `/config` theme editor must survive the arrow keys that drive it.
//!
//! Every arrow key inside the open theme editor live-previews the highlighted
//! choice, and the preview is a `ConfigUpdated` event that sends the host
//! through `refresh_config_view_if_open`. That refresh used to rebuild the
//! view from `ConfigView::new_for_app`, which drops the transient `editing`
//! state: the first arrow key closed the editor, the next one fell through to
//! the non-editing key map (where Left/Right switch category) and the
//! highlight snapped back to the top. Through a real terminal that reads as
//! "the panel closes as soon as I press a direction key".
//!
//! This drives the whole path — terminal decoder, modal host, config write —
//! against the shipped binary, so a regression that only lives in the host
//! glue fails here even if the view-level unit test passes.
//!
//! The expected screen is pinned by `src/tui/goldens/edit_theme_{80x24,120x32}.txt`.

use std::time::Duration;

use super::qa_harness::{
    harness::{Harness, SealedWorkspace, make_sealed_workspace},
    keys,
};

/// The open editor's own title. It only renders while `editing` is set, so it
/// is the exact signal the bug used to clear.
const EDITOR_TITLE: &str = "Edit Theme [theme]";
/// The editor's key legend, likewise editor-only.
const EDITOR_LEGEND: &str = "or click choose";
/// The selection marker on a choice row.
const CHOICE_CURSOR: char = '▸';

fn spawn(workspace: &SealedWorkspace) -> Harness {
    Harness::builder(Harness::cargo_bin("codewhale-tui"))
        .cwd(workspace.workspace())
        .clear_env()
        .seal_home(workspace.home())
        .env("CODEWHALE_DISABLE_MODELS_DEV_FETCH", "1")
        .env("CODEWHALE_NO_UPDATE_CHECK", "1")
        .env("CODEWHALE_TELEMETRY", "0")
        .env("NO_ANIMATIONS", "1")
        .env("RUST_LOG", "warn")
        .args([
            "--workspace",
            workspace.workspace().to_str().unwrap(),
            "--no-project-config",
            "--fresh",
        ])
        .size(24, 80)
        .spawn()
        .expect("start TUI")
}

/// The 1-based index of the highlighted choice, read from the `▸` marker.
///
/// The golden numbers the rows, so this ties the marker to the label the user
/// sees rather than to a row offset that a layout change could move.
fn highlighted_choice(tui: &mut Harness) -> (usize, String) {
    let lines: Vec<String> = (0..tui.frame().rows())
        .map(|row| tui.frame().row(row))
        .collect();
    let line = lines
        .iter()
        .find(|line| line.contains(CHOICE_CURSOR))
        .unwrap_or_else(|| panic!("no highlighted choice on screen:\n{}", tui.frame().text()));
    let after = line.split_once(CHOICE_CURSOR).unwrap().1.trim();
    let (number, label) = after
        .split_once('.')
        .unwrap_or_else(|| panic!("highlighted row has no `N. label` form: {line:?}"));
    let number = number
        .trim()
        .parse::<usize>()
        .unwrap_or_else(|_| panic!("highlighted row has no numeric index: {line:?}"));
    (number, label.trim().to_string())
}

/// Fail with the whole screen when the editor is not open.
fn assert_editor_open(tui: &mut Harness, context: &str) {
    for needle in [EDITOR_TITLE, EDITOR_LEGEND] {
        assert!(
            tui.frame().contains(needle),
            "{context}: the theme editor is gone (missing {needle:?}):\n{}",
            tui.frame().text()
        );
    }
}

#[test]
fn theme_editor_survives_every_arrow_key() {
    let workspace = make_sealed_workspace().expect("sealed workspace");
    let mut tui = spawn(&workspace);
    let timeout = Duration::from_secs(15);

    tui.wait_for_composer(timeout).unwrap();

    // F2 is the advertised bind for the config shell.
    tui.send(keys::key::f2()).unwrap();
    tui.wait_for_text("Search: ", timeout).unwrap();

    // Row 0 of the Appearance tab is Theme; the config golden pins that order.
    tui.send(keys::key::enter()).unwrap();
    tui.wait_for_text(EDITOR_TITLE, timeout).unwrap();
    assert_editor_open(&mut tui, "after Enter");
    let (opened_index, opened_label) = highlighted_choice(&mut tui);

    // Down moves the highlight one choice and keeps the editor open. Before
    // the fix this single key press was enough to close it.
    tui.send(keys::key::down()).unwrap();
    tui.wait_for_idle(Duration::from_millis(250), timeout)
        .unwrap();
    assert_editor_open(&mut tui, "after Down");
    let (down_index, down_label) = highlighted_choice(&mut tui);
    assert_ne!(
        down_index,
        opened_index,
        "Down must move the theme highlight away from {opened_label:?}:\n{}",
        tui.frame().text()
    );
    assert_eq!(
        down_index,
        opened_index + 1,
        "Down must move exactly one choice"
    );
    assert_ne!(
        down_label, opened_label,
        "the label must follow the highlight"
    );

    // Up returns to where it started rather than snapping to the top — the
    // snapped-to-row-0 reading is precisely the reported symptom.
    tui.send(keys::key::up()).unwrap();
    tui.wait_for_idle(Duration::from_millis(250), timeout)
        .unwrap();
    assert_editor_open(&mut tui, "after Up");
    let (up_index, up_label) = highlighted_choice(&mut tui);
    assert_eq!(
        (up_index, up_label.as_str()),
        (opened_index, opened_label.as_str()),
        "Up must return to the opening highlight:\n{}",
        tui.frame().text()
    );

    // The horizontal keys drive the same path and must also keep the editor.
    for (label, key) in [
        ("Right", keys::key::right()),
        ("Left", keys::key::left()),
        ("PageDown", keys::key::page_down()),
        ("PageUp", keys::key::page_up()),
    ] {
        tui.send(key).unwrap();
        tui.wait_for_idle(Duration::from_millis(250), timeout)
            .unwrap();
        assert_editor_open(&mut tui, label);
    }

    // A digit jumps straight to a choice without closing the editor.
    tui.send(keys::key::ch('5')).unwrap();
    tui.wait_for_idle(Duration::from_millis(250), timeout)
        .unwrap();
    assert_editor_open(&mut tui, "after digit jump");
    let (jump_index, _) = highlighted_choice(&mut tui);
    assert_eq!(jump_index, 5, "digit 5 must highlight the fifth choice");

    // Esc cancels the edit: the editor closes, the panel stays.
    tui.send(keys::key::esc()).unwrap();
    tui.wait_for(
        |frame| !frame.contains(EDITOR_TITLE),
        Duration::from_secs(5),
    )
    .unwrap();
    assert!(
        tui.frame().contains("Search: "),
        "Esc must leave the config panel open:\n{}",
        tui.frame().text()
    );

    tui.shutdown();
}
