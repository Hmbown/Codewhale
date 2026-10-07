//! Plan hand-off: when a Plan turn leaves open To-do steps, ask how to go on.
//!
//! The question uses the modal `request_user_input` already draws. It is
//! host-owned: no tool call is waiting in the engine, so the answer is applied
//! by the TUI (permission, mode, follow-up message) instead of being returned
//! as a tool result.

use codewhale_config::AppMode;
use codewhale_execpolicy::ApprovalMode;
use codewhale_localization::{Locale, MessageId, tr};

use crate::core::events::TurnOutcomeStatus;
use crate::tools::todo::TodoListSnapshot;
use crate::tools::user_input::{
    UserInputOption, UserInputQuestion, UserInputRequest, UserInputResponse,
};
use crate::tui::app::ToolEvidence;
use crate::tui::history::is_checklist_tool_name;

/// Request id of the hand-off question on the view stack.
pub(crate) const REQUEST_ID: &str = "codewhale:plan-handoff";
const QUESTION_ID: &str = "plan_handoff";

/// What the person chose to do with a finished plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PlanHandoffChoice {
    /// Leave Plan for Work with this permission and start on the plan.
    Work(ApprovalMode),
    /// Stay in Plan and send this feedback as the next message.
    Revise(String),
    /// Stay in Plan; nothing is sent.
    KeepPlanning,
}

/// Whether a finished turn left a plan to hand off: it ran in Plan, completed,
/// wrote the To-do list, and that list still has open steps.
pub(crate) fn plan_ready(
    mode: AppMode,
    status: TurnOutcomeStatus,
    evidence: &[ToolEvidence],
    todos: &TodoListSnapshot,
) -> bool {
    mode == AppMode::Plan
        && status == TurnOutcomeStatus::Completed
        && evidence
            .iter()
            .any(|entry| is_checklist_tool_name(&entry.tool_name))
        && todos.items.iter().any(|item| !item.status.is_settled())
}

/// The hand-off question, in the person's language.
pub(crate) fn request(locale: Locale) -> UserInputRequest {
    let option = |label: MessageId, description: MessageId| UserInputOption {
        label: tr(locale, label).into_owned(),
        description: tr(locale, description).into_owned(),
    };
    UserInputRequest {
        questions: vec![UserInputQuestion {
            header: tr(locale, MessageId::PlanHandoffHeader).into_owned(),
            id: QUESTION_ID.to_string(),
            question: tr(locale, MessageId::PlanHandoffQuestion).into_owned(),
            options: vec![
                option(
                    MessageId::PlanHandoffWorkAsk,
                    MessageId::PlanHandoffWorkAskDetail,
                ),
                option(
                    MessageId::PlanHandoffWorkAuto,
                    MessageId::PlanHandoffWorkAutoDetail,
                ),
                option(
                    MessageId::PlanHandoffKeepPlanning,
                    MessageId::PlanHandoffKeepPlanningDetail,
                ),
            ],
            allow_free_text: true,
            multi_select: false,
        }],
    }
}

/// Read the answer to [`request`]. A typed response is feedback on the plan;
/// anything unrecognized or empty keeps planning, the choice that changes
/// nothing.
pub(crate) fn choice(locale: Locale, response: &UserInputResponse) -> PlanHandoffChoice {
    let Some(answer) = response.answers.first() else {
        return PlanHandoffChoice::KeepPlanning;
    };
    let picked = |id: MessageId| answer.label == tr(locale, id);
    if picked(MessageId::PlanHandoffWorkAsk) {
        PlanHandoffChoice::Work(ApprovalMode::Suggest)
    } else if picked(MessageId::PlanHandoffWorkAuto) {
        PlanHandoffChoice::Work(ApprovalMode::Auto)
    } else if picked(MessageId::PlanHandoffKeepPlanning) {
        PlanHandoffChoice::KeepPlanning
    } else {
        let feedback = answer.value.trim();
        if feedback.is_empty() {
            PlanHandoffChoice::KeepPlanning
        } else {
            PlanHandoffChoice::Revise(feedback.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::todo::{TodoItem, TodoStatus};
    use crate::tools::user_input::UserInputAnswer;

    fn evidence(tool_name: &str) -> Vec<ToolEvidence> {
        vec![ToolEvidence {
            tool_name: tool_name.to_string(),
            summary: String::new(),
        }]
    }

    fn todos(statuses: &[TodoStatus]) -> TodoListSnapshot {
        TodoListSnapshot {
            items: statuses
                .iter()
                .zip(1u32..)
                .map(|(status, id)| TodoItem {
                    id,
                    content: format!("step {id}"),
                    status: *status,
                })
                .collect(),
            ..TodoListSnapshot::default()
        }
    }

    fn answer(label: &str, value: &str) -> UserInputResponse {
        UserInputResponse {
            answers: vec![UserInputAnswer {
                id: QUESTION_ID.to_string(),
                label: label.to_string(),
                value: value.to_string(),
            }],
        }
    }

    #[test]
    fn a_completed_plan_turn_with_open_todo_steps_is_ready() {
        let open = todos(&[TodoStatus::Completed, TodoStatus::Pending]);
        let wrote = evidence("todo_write");
        assert!(plan_ready(
            AppMode::Plan,
            TurnOutcomeStatus::Completed,
            &wrote,
            &open
        ));

        // Work and Operate turns write the same list while doing the work.
        for mode in [AppMode::Agent, AppMode::Operate] {
            assert!(!plan_ready(
                mode,
                TurnOutcomeStatus::Completed,
                &wrote,
                &open
            ));
        }
        // An interrupted or failed turn did not finish its plan.
        for status in [TurnOutcomeStatus::Interrupted, TurnOutcomeStatus::Failed] {
            assert!(!plan_ready(AppMode::Plan, status, &wrote, &open));
        }
        // A Plan turn that only answered a question left no new plan, even
        // when an older list is still open.
        assert!(!plan_ready(
            AppMode::Plan,
            TurnOutcomeStatus::Completed,
            &evidence("read"),
            &open
        ));
        assert!(!plan_ready(
            AppMode::Plan,
            TurnOutcomeStatus::Completed,
            &[],
            &open
        ));
        // Nothing left to start.
        assert!(!plan_ready(
            AppMode::Plan,
            TurnOutcomeStatus::Completed,
            &wrote,
            &todos(&[TodoStatus::Completed, TodoStatus::Cancelled])
        ));
        assert!(!plan_ready(
            AppMode::Plan,
            TurnOutcomeStatus::Completed,
            &wrote,
            &todos(&[])
        ));
    }

    #[test]
    fn every_locale_asks_a_valid_question_and_reads_its_own_answers() {
        for locale in Locale::shipped_complete() {
            let asked = request(*locale);
            asked
                .validate()
                .unwrap_or_else(|error| panic!("{}: {error:?}", locale.tag()));
            let labels: Vec<&str> = asked.questions[0]
                .options
                .iter()
                .map(|option| option.label.as_str())
                .collect();
            let expected = [
                PlanHandoffChoice::Work(ApprovalMode::Suggest),
                PlanHandoffChoice::Work(ApprovalMode::Auto),
                PlanHandoffChoice::KeepPlanning,
            ];
            assert_eq!(labels.len(), expected.len(), "{}", locale.tag());
            for (label, expected) in labels.iter().zip(expected) {
                // The modal answers an option with its label as the value.
                assert_eq!(
                    choice(*locale, &answer(label, label)),
                    expected,
                    "{} {label}",
                    locale.tag()
                );
                // "Other" is the modal's own free-text row.
                assert_ne!(*label, "Other", "{}", locale.tag());
            }
        }
    }

    #[test]
    fn a_typed_response_is_feedback_and_an_empty_one_keeps_planning() {
        assert_eq!(
            choice(Locale::En, &answer("Other", "  cover the tests too ")),
            PlanHandoffChoice::Revise("cover the tests too".to_string())
        );
        assert_eq!(
            choice(Locale::En, &answer("Other", "   ")),
            PlanHandoffChoice::KeepPlanning
        );
        assert_eq!(
            choice(Locale::En, &UserInputResponse { answers: vec![] }),
            PlanHandoffChoice::KeepPlanning
        );
    }
}
