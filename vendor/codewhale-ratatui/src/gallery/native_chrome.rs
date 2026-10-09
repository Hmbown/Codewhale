//! Current Codewhale TUI footer chrome, rendered through portable host data.
use std::time::Duration;

use ratatui::{buffer::Buffer, layout::Rect};

use super::Entry;
use crate::{
    NativeComposer, NativeComposerDensity, Paint, Theme, WorkflowProgress, WorkflowRun,
    WorkflowRunState,
};

fn composer(area: Rect, buf: &mut Buffer, theme: &Theme) {
    NativeComposer::new("Keep the composer compact and the workflow progress visible.")
        .focused(true)
        .can_submit(true)
        .submit_hint("Enter send  Shift+Enter newline")
        .paint(area, buf, theme);
}
fn composer_narrow(area: Rect, buf: &mut Buffer, theme: &Theme) {
    NativeComposer::new("Review the changes, then continue.")
        .focused(true)
        .can_submit(true)
        .density(NativeComposerDensity::Compact)
        .submit_hint("Enter send")
        .paint(area, buf, theme);
}
fn composer_quiet(area: Rect, buf: &mut Buffer, theme: &Theme) {
    NativeComposer::new("")
        .enclosed(false)
        .density(NativeComposerDensity::Compact)
        .focused(true)
        .paint(area, buf, theme);
}
fn composer_target(area: Rect, buf: &mut Buffer, theme: &Theme) {
    NativeComposer::new("Check the selected files.\nKeep the draft until I answer.")
        .focused(true)
        .can_submit(true)
        .target("To reviewer")
        .submit_hint("Enter send")
        .density(NativeComposerDensity::Spacious)
        .paint(area, buf, theme);
}
fn workflow_live(area: Rect, buf: &mut Buffer, theme: &Theme) {
    WorkflowProgress::new(vec![
        WorkflowRun::new("Compare Cline with Codewhale", WorkflowRunState::Running)
            .outcomes(4, 1, 0, 10)
            .elapsed(Duration::from_secs(134))
            .tokens(1_234_567),
    ])
    .paint(area, buf, theme);
}
fn workflow_settled(area: Rect, buf: &mut Buffer, theme: &Theme) {
    WorkflowProgress::new(vec![
        WorkflowRun::new("Inspect the files", WorkflowRunState::Succeeded)
            .outcomes(3, 0, 0, 3)
            .elapsed(Duration::from_secs(59)),
        WorkflowRun::new("Release-readiness audit", WorkflowRunState::Failed)
            .outcomes(0, 2, 0, 2)
            .elapsed(Duration::from_millis(355))
            .reason("Authorization failed: sign in again. No task produced a result."),
        WorkflowRun::new("Port the fixture suite", WorkflowRunState::Degraded)
            .outcomes(3, 1, 0, 4)
            .elapsed(Duration::from_secs(134))
            .reason("Timed out waiting for the model after 600s. Retried once."),
    ])
    .paint(area, buf, theme);
}
fn workflow_queued(area: Rect, buf: &mut Buffer, theme: &Theme) {
    WorkflowProgress::new(vec![
        WorkflowRun::new("Compare the modules", WorkflowRunState::Running)
            .outcomes(0, 1, 0, 25)
            .elapsed(Duration::from_secs(5))
            .queued(2)
            .tokens(48_400),
    ])
    .paint(area, buf, theme);
}
fn workflow_narrow(area: Rect, buf: &mut Buffer, theme: &Theme) {
    WorkflowProgress::new(vec![
        WorkflowRun::new(
            "Read-only release-readiness audit for Codewhale",
            WorkflowRunState::Failed,
        )
        .outcomes(0, 2, 0, 2)
        .elapsed(Duration::from_millis(355))
        .reason("Authorization failed: sign in again."),
    ])
    .paint(area, buf, theme);
}

pub(crate) fn entries() -> Vec<Entry> {
    vec![
        Entry {
            name: "native-composer",
            width: 100,
            height: 4,
            draw: composer,
        },
        Entry {
            name: "native-composer-narrow",
            width: 40,
            height: 4,
            draw: composer_narrow,
        },
        Entry {
            name: "native-composer-quiet",
            width: 60,
            height: 2,
            draw: composer_quiet,
        },
        Entry {
            name: "native-composer-target",
            width: 80,
            height: 6,
            draw: composer_target,
        },
        Entry {
            name: "workflow-progress-live",
            width: 110,
            height: 1,
            draw: workflow_live,
        },
        Entry {
            name: "workflow-progress-settled",
            width: 140,
            height: 3,
            draw: workflow_settled,
        },
        Entry {
            name: "workflow-progress-queued",
            width: 120,
            height: 1,
            draw: workflow_queued,
        },
        Entry {
            name: "workflow-progress-narrow",
            width: 40,
            height: 1,
            draw: workflow_narrow,
        },
    ]
}
