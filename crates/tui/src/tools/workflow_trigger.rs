//! Automatic Workflow trigger and suppression heuristics (#4127).
//!
//! Soft-auto model: the **agent** decides to use Workflow without the operator
//! saying the word "workflow". Policy here answers "should we orchestrate?" —
//! the parent prompt still **tells the operator** the intended shape and may
//! ask setup questions via `request_user_input` (TUI modal) before calling
//! `workflow` / `plan`.
//!
//! This remains Act/Agent guidance rather than a prose classifier at the host
//! boundary. Operate keeps small tasks in the parent and uses the existing
//! Workflow plan for multi-step delegation through the same sub-agents.

/// Signals the parent can supply without full conversation replay.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkflowTriggerSignals {
    /// Approximate open file / edit scope count for the current ask.
    pub distinct_file_scopes: usize,
    /// True when the operator is mid interactive multi-turn design/chat.
    pub highly_interactive: bool,
    /// True when the ask requires writes but no clear phase/child decomposition.
    pub risky_writes_unclear_decomposition: bool,
    /// Estimated child count if Workflow launched now.
    pub estimated_children: usize,
    /// Soft cap from `[workflow].auto_start_child_limit` (default 16).
    pub auto_start_child_limit: usize,
    /// Approximate parent context tokens in use (for high-volume signal).
    pub context_tokens: usize,
    /// Threshold above which high context volume favors Workflow.
    pub high_context_token_threshold: usize,
}

impl WorkflowTriggerSignals {
    #[must_use]
    pub fn product_defaults() -> Self {
        Self {
            distinct_file_scopes: 0,
            highly_interactive: false,
            risky_writes_unclear_decomposition: false,
            estimated_children: 0,
            auto_start_child_limit: 16,
            context_tokens: 0,
            high_context_token_threshold: 80_000,
        }
    }
}

/// Decision for automatic Workflow launch / recommendation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowTriggerDecision {
    /// Launch or recommend Workflow.
    Trigger { reason: &'static str },
    /// Suppress automatic Workflow; prefer direct tools / single agent.
    Suppress { reason: &'static str },
}

impl WorkflowTriggerDecision {
    #[must_use]
    pub fn should_trigger(&self) -> bool {
        matches!(self, Self::Trigger { .. })
    }

    #[cfg(test)]
    #[must_use]
    pub fn reason(&self) -> &'static str {
        match self {
            Self::Trigger { reason } | Self::Suppress { reason } => reason,
        }
    }
}

/// Evaluate whether automatic Workflow is appropriate for this user ask.
///
/// Suppression wins over trigger when both could apply (noisy auto-orchestration
/// is worse than missing a fan-out). Act/Agent soft-auto guidance should stay
/// aligned with these rules. Operate uses a direct worker for one bounded
/// independent task and a Workflow plan for multi-step delegation.
#[must_use]
pub fn evaluate_workflow_trigger(
    user_text: &str,
    signals: &WorkflowTriggerSignals,
) -> WorkflowTriggerDecision {
    let text = user_text.trim();
    let lower = text.to_ascii_lowercase();

    // --- Hard suppressions (AC) ---
    if signals.highly_interactive {
        return WorkflowTriggerDecision::Suppress {
            reason: "highly interactive task — keep turn-by-turn",
        };
    }
    if signals.risky_writes_unclear_decomposition {
        return WorkflowTriggerDecision::Suppress {
            reason: "risky writes without clear decomposition",
        };
    }
    if signals.estimated_children > 0
        && signals.auto_start_child_limit > 0
        && signals.estimated_children > signals.auto_start_child_limit
    {
        return WorkflowTriggerDecision::Suppress {
            reason: "estimated children exceed auto_start_child_limit",
        };
    }
    if child_overhead_exceeds_benefit(&lower, signals) {
        return WorkflowTriggerDecision::Suppress {
            reason: "child overhead greater than benefit",
        };
    }
    if is_simple_command_or_factual_question(&lower, text) {
        return WorkflowTriggerDecision::Suppress {
            reason: "simple command or factual question",
        };
    }
    if is_one_file_edit(&lower, signals) {
        return WorkflowTriggerDecision::Suppress {
            reason: "one-file edit — use direct tools",
        };
    }

    // --- Triggers (AC) ---
    if signals.distinct_file_scopes >= 3 {
        return WorkflowTriggerDecision::Trigger {
            reason: "independent scopes across multiple files",
        };
    }
    if signals.context_tokens >= signals.high_context_token_threshold {
        return WorkflowTriggerDecision::Trigger {
            reason: "high context volume favors staged Workflow",
        };
    }
    if has_fanout_language(&lower) {
        return WorkflowTriggerDecision::Trigger {
            reason: "audit/sweep/compare/fan-out language",
        };
    }
    if has_staged_work_language(&lower) {
        return WorkflowTriggerDecision::Trigger {
            reason: "staged multi-phase work",
        };
    }
    if has_independent_verification_language(&lower) {
        return WorkflowTriggerDecision::Trigger {
            reason: "independent verification pass",
        };
    }

    WorkflowTriggerDecision::Suppress {
        reason: "no automatic Workflow trigger matched",
    }
}

fn child_overhead_exceeds_benefit(lower: &str, signals: &WorkflowTriggerSignals) -> bool {
    // Tiny asks or explicit single-step language — spawn cost dominates.
    if signals.estimated_children == 1 {
        return true;
    }
    if lower.len() < 24 && !has_fanout_language(lower) && !has_staged_work_language(lower) {
        return true;
    }
    let tiny = [
        "fix typo",
        "rename variable",
        "one liner",
        "one-liner",
        "quick peek",
        "just check",
    ];
    tiny.iter().any(|needle| lower.contains(needle))
}

fn is_simple_command_or_factual_question(lower: &str, original: &str) -> bool {
    if lower.starts_with('/') {
        // Slash commands are UI routing, not orchestration.
        return true;
    }
    let factual_prefixes = [
        "what is ",
        "what's ",
        "whats ",
        "who is ",
        "when is ",
        "where is ",
        "how many ",
        "which ",
        "define ",
        "explain ",
    ];
    if factual_prefixes.iter().any(|p| lower.starts_with(p)) && original.len() < 160 {
        return true;
    }
    let simple_cmds = [
        "run tests",
        "run the tests",
        "cargo test",
        "cargo check",
        "git status",
        "git log",
        "git diff",
        "ls",
        "pwd",
        "show version",
        "print version",
    ];
    if simple_cmds
        .iter()
        .any(|c| lower == *c || lower.starts_with(&format!("{c} ")))
    {
        return true;
    }
    // Short yes/no or status pings.
    matches!(
        lower.trim_end_matches(['?', '.', '!']),
        "ok" | "thanks" | "thank you" | "status" | "ping" | "hello" | "hi"
    )
}

fn is_one_file_edit(lower: &str, signals: &WorkflowTriggerSignals) -> bool {
    if signals.distinct_file_scopes == 1 {
        let editish = [
            "edit ",
            "fix ",
            "patch ",
            "update ",
            "change ",
            "rewrite ",
            "in this file",
            "this file",
            "only this file",
            "single file",
            "one file",
        ];
        return editish.iter().any(|n| lower.contains(n));
    }
    // Explicit single-file phrasing without scope signal.
    lower.contains("only this file")
        || lower.contains("just this file")
        || lower.contains("single file")
        || (lower.contains("one file") && !has_fanout_language(lower))
}

fn has_fanout_language(lower: &str) -> bool {
    const NEEDLES: &[&str] = &[
        "audit",
        "sweep",
        "compare",
        "fan-out",
        "fan out",
        "fanout",
        "in parallel",
        "parallel across",
        "across the codebase",
        "across packages",
        "across crates",
        "every crate",
        "all packages",
        "all modules",
        "multi-repo",
        "multi repo",
    ];
    NEEDLES.iter().any(|n| lower.contains(n))
}

fn has_staged_work_language(lower: &str) -> bool {
    const NEEDLES: &[&str] = &[
        "phase 1",
        "phase 2",
        "first implement",
        "then verify",
        "implement then",
        "staged",
        "multi-phase",
        "multi phase",
        "plan then execute",
        "explore then implement",
        "scout then",
    ];
    NEEDLES.iter().any(|n| lower.contains(n))
}

fn has_independent_verification_language(lower: &str) -> bool {
    const NEEDLES: &[&str] = &[
        "independent verification",
        "verify independently",
        "separate verifier",
        "second pair of eyes",
        "review in parallel",
        "verify in parallel",
        "independent review",
    ];
    NEEDLES.iter().any(|n| lower.contains(n))
}

/// Reachability probe so the soft-auto surface stays linked in release builds.
///
/// Returns `true` when a canonical fan-out ask would trigger Workflow under
/// product defaults (used by registry/tool wiring smoke tests).
#[must_use]
pub fn soft_auto_policy_is_linked() -> bool {
    evaluate_workflow_trigger(
        "audit every crate for unsafe blocks",
        &WorkflowTriggerSignals::product_defaults(),
    )
    .should_trigger()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signals() -> WorkflowTriggerSignals {
        WorkflowTriggerSignals::product_defaults()
    }

    #[test]
    fn suppresses_one_file_edits() {
        let mut s = signals();
        s.distinct_file_scopes = 1;
        let d = evaluate_workflow_trigger("fix the typo in this file", &s);
        assert!(!d.should_trigger(), "{d:?}");
        assert!(d.reason().contains("one-file"));
    }

    #[test]
    fn suppresses_simple_commands_and_factual_questions() {
        let s = signals();
        for ask in [
            "cargo test",
            "git status",
            "what is a worktree?",
            "how many crates are there?",
            "/help",
            "thanks",
        ] {
            let d = evaluate_workflow_trigger(ask, &s);
            assert!(!d.should_trigger(), "expected suppress for {ask:?}: {d:?}");
        }
    }

    #[test]
    fn product_defaults_match_workflow_child_limit() {
        assert_eq!(signals().auto_start_child_limit, 16);
    }

    #[test]
    fn suppresses_highly_interactive_and_unclear_risky_writes() {
        let mut s = signals();
        s.highly_interactive = true;
        assert!(!evaluate_workflow_trigger("redesign the product with me", &s).should_trigger());

        s = signals();
        s.risky_writes_unclear_decomposition = true;
        assert!(!evaluate_workflow_trigger("make it better somehow", &s).should_trigger());
    }

    #[test]
    fn suppresses_when_child_overhead_dominates() {
        let mut s = signals();
        s.estimated_children = 1;
        assert!(!evaluate_workflow_trigger("quick peek at main.rs", &s).should_trigger());

        s = signals();
        s.estimated_children = 20;
        s.auto_start_child_limit = 8;
        let d = evaluate_workflow_trigger("audit the whole monorepo", &s);
        assert!(!d.should_trigger(), "{d:?}");
        assert!(d.reason().contains("auto_start_child_limit"));
    }

    #[test]
    fn triggers_on_fanout_and_staged_language() {
        let s = signals();
        for ask in [
            "audit every crate for unsafe blocks",
            "sweep the codebase for TODO debt",
            "compare the two provider implementations in parallel",
            "phase 1 explore then phase 2 implement",
            "run an independent verification of the release notes",
        ] {
            let d = evaluate_workflow_trigger(ask, &s);
            assert!(d.should_trigger(), "expected trigger for {ask:?}: {d:?}");
        }
    }

    #[test]
    fn triggers_on_independent_scopes_and_high_context() {
        let mut s = signals();
        s.distinct_file_scopes = 5;
        assert!(
            evaluate_workflow_trigger("touch the related modules carefully", &s).should_trigger()
        );

        s = signals();
        s.context_tokens = 120_000;
        assert!(evaluate_workflow_trigger("continue the migration plan", &s).should_trigger());
    }

    #[test]
    fn suppression_wins_over_fanout_language_when_interactive() {
        let mut s = signals();
        s.highly_interactive = true;
        let d = evaluate_workflow_trigger("let's design an audit sweep together", &s);
        assert!(!d.should_trigger(), "{d:?}");
        assert!(d.reason().contains("interactive"));
    }
}
