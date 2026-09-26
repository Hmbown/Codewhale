//! Background work that finished since the last notice (#6565).
//!
//! Every terminal transition of background work (a sub-agent, a background
//! shell, a durable task) lands here with the name every other surface shows.
//! The notice for a batch names all of it, not only the last child to finish,
//! and names each item by what a person recognises (the agent's name, the
//! shell's command), never an internal id.
//!
//! When the notice fires is decided by [`ready_to_flush`]. In `final-only`
//! mode it waits only for *finite* work: running agents and queued or running
//! durable tasks. A background shell can run forever (a dev server, a
//! watcher), so a running shell never holds a notice back.

use std::time::Duration;

use codewhale_localization::{Locale, MessageId, tr};

use crate::config::SubagentCompletionNotification;
use crate::notify::payload::NotificationPayload;
use crate::tools::subagent::SubAgentStatus;

/// Finished background shells kept listed (muted) in the dock.
pub const MAX_FINISHED_SHELLS: usize = 8;
/// Names listed in a batch notice before the rest are counted.
const MAX_NAMED: usize = 4;
/// Characters of a shell command kept as its name.
const SHELL_NAME_CHARS: usize = 48;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinishedKind {
    Agent,
    Shell,
    Task,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinishedOutcome {
    Done,
    Failed,
    Stopped,
}

impl FinishedOutcome {
    #[must_use]
    pub fn from_agent(status: &SubAgentStatus) -> Self {
        match status {
            SubAgentStatus::Completed | SubAgentStatus::Running => Self::Done,
            SubAgentStatus::Failed(_) | SubAgentStatus::BudgetExhausted => Self::Failed,
            SubAgentStatus::Cancelled | SubAgentStatus::Interrupted(_) => Self::Stopped,
        }
    }
}

/// One piece of background work that reached a terminal state.
#[derive(Debug, Clone, PartialEq)]
pub struct FinishedWork {
    pub kind: FinishedKind,
    /// The name every other surface shows: the agent's label, the shell's
    /// command, the task's summary.
    pub name: String,
    pub outcome: FinishedOutcome,
    /// The agent's exact status, so a single-agent notice keeps its
    /// status-specific headline ("Agent cancelled", "Budget exhausted").
    pub agent_status: Option<SubAgentStatus>,
    /// The agent's result as reported, for the preview headline.
    pub result: Option<String>,
    /// One line of facts for a shell or task ("exit 0 · 12s", "killed").
    pub summary: Option<String>,
    pub elapsed: Duration,
}

impl FinishedWork {
    #[must_use]
    pub fn agent(name: &str, status: &SubAgentStatus, result: &str, elapsed: Duration) -> Self {
        Self {
            kind: FinishedKind::Agent,
            name: name.to_string(),
            outcome: FinishedOutcome::from_agent(status),
            agent_status: Some(status.clone()),
            result: Some(result.to_string()),
            summary: None,
            elapsed,
        }
    }

    #[must_use]
    pub fn shell(
        command: &str,
        outcome: FinishedOutcome,
        summary: String,
        elapsed: Duration,
    ) -> Self {
        let command = command.split_whitespace().collect::<Vec<_>>().join(" ");
        let name = if command.chars().count() > SHELL_NAME_CHARS {
            let kept: String = command.chars().take(SHELL_NAME_CHARS - 1).collect();
            format!("{kept}…")
        } else {
            command
        };
        Self {
            kind: FinishedKind::Shell,
            name,
            outcome,
            agent_status: None,
            result: None,
            summary: Some(summary),
            elapsed,
        }
    }

    #[must_use]
    pub fn task(name: &str, outcome: FinishedOutcome, summary: String, elapsed: Duration) -> Self {
        Self {
            kind: FinishedKind::Task,
            name: name.trim().to_string(),
            outcome,
            agent_status: None,
            result: None,
            summary: Some(summary),
            elapsed,
        }
    }

    /// The preview line for this item: an agent's result headline (with `…`
    /// and a pointer to the full result when the headline is not all of it),
    /// or a shell or task's facts.
    fn preview(&self, locale: Locale) -> Option<String> {
        if let Some(summary) = self.summary.as_deref() {
            return Some(summary.to_string());
        }
        let result = self.result.as_deref()?;
        let headline = crate::agent_roster::result_headline(result)?;
        let bounded = crate::tui::notifications::text_summary(&headline)?;
        let whole = result
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with("<codewhale:"))
            .collect::<Vec<_>>()
            .join(" ");
        if bounded == whole {
            return Some(bounded);
        }
        let bounded = bounded.trim_end_matches("...").trim_end();
        Some(format!(
            "{bounded} … {}",
            tr(locale, MessageId::NotificationFullResultPointer)
        ))
    }

    fn headline_id(&self) -> MessageId {
        match (self.kind, self.outcome) {
            (FinishedKind::Agent, _) => crate::tui::notifications::subagent_terminal_label(
                self.agent_status
                    .as_ref()
                    .unwrap_or(&SubAgentStatus::Completed),
            ),
            (FinishedKind::Shell, FinishedOutcome::Done) => MessageId::NotificationShellFinished,
            (FinishedKind::Shell, FinishedOutcome::Failed) => MessageId::NotificationShellFailed,
            (FinishedKind::Shell, FinishedOutcome::Stopped) => MessageId::NotificationShellStopped,
            (FinishedKind::Task, FinishedOutcome::Done) => MessageId::NotificationTaskFinished,
            (FinishedKind::Task, FinishedOutcome::Failed) => MessageId::NotificationTaskFailed,
            (FinishedKind::Task, FinishedOutcome::Stopped) => MessageId::NotificationTaskStopped,
        }
    }
}

/// Whether the pending batch should be announced now.
///
/// - `off` drains the batch without a notice.
/// - `always` announces every item as it arrives.
/// - `final-only` waits while finite work is still live (running agents, a
///   running workflow, queued or running durable tasks). Running shells are
///   open-ended and never count. A batch with no agent in it also waits for
///   the parent turn to go idle, so a shell the model is handling mid-turn is
///   reported by the turn, not by a second notice.
#[must_use]
pub fn ready_to_flush(
    mode: SubagentCompletionNotification,
    batch: &[FinishedWork],
    finite_work_live: bool,
    parent_busy: bool,
) -> bool {
    if batch.is_empty() {
        return false;
    }
    match mode {
        SubagentCompletionNotification::Off | SubagentCompletionNotification::Always => true,
        SubagentCompletionNotification::FinalOnly => {
            !finite_work_live
                && (!parent_busy || batch.iter().any(|item| item.kind == FinishedKind::Agent))
        }
    }
}

/// One notice for a batch of finished background work.
///
/// A single item keeps its own headline ("Agent complete", "Shell failed"),
/// names itself in the detail and previews its result. A batch counts what
/// finished ("3 finished", or "2 done · 1 failed" when something did not
/// succeed), names each item (the first few, then `+N`), and previews the
/// item most worth reading: the first that did not succeed, else the first.
#[must_use]
pub fn background_finished_payload(
    locale: Locale,
    batch: &[FinishedWork],
    include_summary: bool,
) -> Option<NotificationPayload> {
    let elapsed = batch.iter().map(|item| item.elapsed).max()?;
    let headline = match batch {
        [single] => tr(locale, single.headline_id()).into_owned(),
        many => {
            let done = many
                .iter()
                .filter(|item| item.outcome == FinishedOutcome::Done)
                .count();
            if done == many.len() {
                tr(locale, MessageId::NotificationBackgroundFinished)
                    .replace("{count}", &many.len().to_string())
            } else {
                tr(locale, MessageId::NotificationBackgroundMixed)
                    .replace("{done}", &done.to_string())
                    .replace("{failed}", &(many.len() - done).to_string())
            }
        }
    };
    let headline =
        crate::tui::notifications::completion_status(&headline, include_summary, elapsed, None);
    let mut names = batch
        .iter()
        .take(MAX_NAMED)
        .map(|item| item.name.as_str())
        .collect::<Vec<_>>()
        .join(" · ");
    if batch.len() > MAX_NAMED {
        names.push_str(&format!(" · +{}", batch.len() - MAX_NAMED));
    }
    let featured = batch
        .iter()
        .find(|item| item.outcome != FinishedOutcome::Done)
        .unwrap_or(&batch[0]);
    let preview = featured.preview(locale).map(|preview| {
        if batch.len() > 1 {
            format!("{}: {preview}", featured.name)
        } else {
            preview
        }
    });
    Some(NotificationPayload::subagent_terminal(&headline, &names).with_preview(preview.as_deref()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SubagentCompletionNotification as Mode;

    fn agent(name: &str, status: SubAgentStatus, result: &str) -> FinishedWork {
        FinishedWork::agent(name, &status, result, Duration::from_secs(30))
    }

    #[test]
    fn final_only_waits_for_finite_work_but_never_for_a_running_shell() {
        let batch = vec![agent("explore", SubAgentStatus::Completed, "Done.")];
        // Another agent is still running: hold.
        assert!(!ready_to_flush(Mode::FinalOnly, &batch, true, false));
        // Only an `npm run dev` shell is still running: shells are not finite
        // work, so the caller reports no finite work and the notice fires.
        assert!(ready_to_flush(Mode::FinalOnly, &batch, false, false));
        // An agent batch does not wait for the parent turn.
        assert!(ready_to_flush(Mode::FinalOnly, &batch, false, true));
        // A shell-only batch waits for the parent turn to go idle.
        let shells = vec![FinishedWork::shell(
            "cargo build",
            FinishedOutcome::Done,
            "exit 0 · 12s".to_string(),
            Duration::from_secs(12),
        )];
        assert!(!ready_to_flush(Mode::FinalOnly, &shells, false, true));
        assert!(ready_to_flush(Mode::FinalOnly, &shells, false, false));
        assert!(ready_to_flush(Mode::Always, &batch, true, true));
        assert!(ready_to_flush(Mode::Off, &batch, true, true));
        assert!(!ready_to_flush(Mode::Always, &[], false, false));
    }

    #[test]
    fn a_batch_names_everything_that_finished_and_counts_failures() {
        let batch = vec![
            agent("explore", SubAgentStatus::Completed, "Found 3 call sites."),
            agent(
                "review",
                SubAgentStatus::Failed("boom".to_string()),
                "provider returned 429",
            ),
            agent(
                "audit docs",
                SubAgentStatus::Completed,
                "All links resolve.",
            ),
        ];
        let payload = background_finished_payload(Locale::En, &batch, false).expect("payload");
        assert_eq!(payload.headline(), "2 done · 1 failed");
        assert_eq!(payload.detail(), Some("explore · review · audit docs"));
        assert_eq!(payload.preview(), Some("review: provider returned 429"));

        let done = vec![
            agent("explore", SubAgentStatus::Completed, "One."),
            agent("audit docs", SubAgentStatus::Completed, "Two."),
            agent("review", SubAgentStatus::Completed, "Three."),
        ];
        let payload = background_finished_payload(Locale::En, &done, false).expect("payload");
        assert_eq!(payload.headline(), "3 finished");
        assert_eq!(payload.preview(), Some("explore: One."));

        let many = (0..6)
            .map(|n| agent(&format!("lane {n}"), SubAgentStatus::Completed, "ok."))
            .collect::<Vec<_>>();
        let payload = background_finished_payload(Locale::En, &many, false).expect("payload");
        assert_eq!(
            payload.detail(),
            Some("lane 0 · lane 1 · lane 2 · lane 3 · +2")
        );
        assert!(background_finished_payload(Locale::En, &[], false).is_none());
    }

    #[test]
    fn a_cut_preview_says_so_and_points_at_the_full_result() {
        let single = vec![agent(
            "audit docs",
            SubAgentStatus::Completed,
            "## Summary\n\nThree links are stale. Two are in README.md.",
        )];
        let payload = background_finished_payload(Locale::En, &single, false).expect("payload");
        assert_eq!(payload.headline(), "Agent complete");
        assert_eq!(payload.detail(), Some("audit docs"));
        assert_eq!(
            payload.preview(),
            Some("Three links are stale. … open Codewhale for the full result")
        );
        // A result that is only its headline is shown as it is.
        let whole = vec![agent("explore", SubAgentStatus::Completed, "Found it.")];
        let payload = background_finished_payload(Locale::En, &whole, false).expect("payload");
        assert_eq!(payload.preview(), Some("Found it."));
    }

    #[test]
    fn shells_are_named_by_their_command_with_exit_facts() {
        for (outcome, summary, headline) in [
            (FinishedOutcome::Done, "exit 0 · 12s", "Shell finished"),
            (FinishedOutcome::Failed, "failed · exit 2", "Shell failed"),
            (FinishedOutcome::Stopped, "killed", "Shell stopped"),
            (FinishedOutcome::Stopped, "timed out", "Shell stopped"),
        ] {
            let batch = vec![FinishedWork::shell(
                "npm   test --\n  --watch=false",
                outcome,
                summary.to_string(),
                Duration::from_secs(12),
            )];
            let payload = background_finished_payload(Locale::En, &batch, false).expect("payload");
            assert_eq!(payload.headline(), headline);
            assert_eq!(payload.detail(), Some("npm test -- --watch=false"));
            assert_eq!(payload.preview(), Some(summary));
        }
    }
}
