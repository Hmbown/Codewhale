//! Model-backed Auto-Review guardian tier (v0.9.8).
//!
//! The deterministic policy engine (see [`crate::tui::auto_review`]) decides
//! first: configured block rules and the built-in safety floor are hard
//! blocks that never reach a model. Only deterministic *fallback holds* — the
//! `AskUser` outcomes Auto posture would otherwise turn into bare permission
//! denials — escalate to a one-shot reviewer request. A denial returns the
//! rationale to the agent with an explicit "do not work around" instruction;
//! and any reviewer failure (timeout, transport error, unparseable answer)
//! is a denial — fail closed. The reviewer is deliberately stateless: each
//! proposed call stands on its own deterministic context.

use std::time::Duration;

use crate::core::model_client::ModelClient;
use crate::tools::spec::ToolError;
use crate::tui::auto_review::{
    AutoReviewAction, DEFAULT_GUARDIAN_POLICY, ReviewerRiskLevel, ReviewerVerdict,
    parse_reviewer_verdict,
};
use codewhale_models::Role;
use codewhale_models::{
    ContentBlock, Message, MessageRequest, MessageResponse, SystemPrompt, Usage,
    is_incomplete_stop_reason,
};
use tokio_util::sync::CancellationToken;

/// One-shot reviewer deadline. Slow reviewers are denials, surfaced
/// separately from explicit denials (a timeout proves nothing about safety).
const REVIEWER_TIMEOUT: Duration = Duration::from_secs(90);
/// Keep one exact held call comfortably inside every supported model context.
/// Truncating tool input could hide the unsafe part, so oversized reviews deny.
const MAX_REVIEW_CONTEXT_BYTES: usize = 64 * 1024;

/// Unusable-answer reasons that a second attempt cannot improve on. Kept as
/// constants so the retry gate and the failure sites cannot drift apart.
const TIMEOUT_REASON: &str = "the reviewer timed out";
const CONTEXT_LIMIT_REASON: &str = "the exact review context exceeded the guardian limit";

/// Output budget for one guardian answer.
///
/// The guardian answers with one small JSON object and runs with thinking
/// disabled, so a few dozen tokens usually suffice. The budget stays generous
/// because this is a *safety* verdict: an answer clipped at the limit parses
/// as nothing and fails closed, turning a call the model never judged into a
/// denial. Measured 2026-09-18 — with the previous 384-token budget a
/// reasoning model spent the entire allowance on hidden thinking
/// (`finish_reason = length`, empty content) on roughly a quarter of
/// real-world holds, and every one of those became a false denial.
const REVIEWER_MAX_TOKENS: u32 = 1024;
/// Second-attempt budget after an unusable answer. Only the budget changes:
/// the policy, the call under review, and the decision rule stay exact.
const REVIEWER_RETRY_MAX_TOKENS: u32 = 2048;

/// The reviewer's answer, with failure modes separated from explicit denials.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReviewerOutcome {
    Allow {
        risk: ReviewerRiskLevel,
        reason: String,
    },
    Deny {
        risk: ReviewerRiskLevel,
        reason: String,
    },
    /// Timeout, transport error, or unparseable answer. Always a denial.
    Unavailable {
        reason: String,
    },
    Cancelled,
}

impl ReviewerOutcome {
    pub(crate) fn audit_decision(&self) -> &'static str {
        match self {
            Self::Allow { .. } => "allow",
            Self::Deny { .. } => "deny",
            Self::Unavailable { .. } => "unavailable",
            Self::Cancelled => "cancelled",
        }
    }

    pub(crate) fn audit_risk(&self) -> Option<&'static str> {
        match self {
            Self::Allow { risk, .. } | Self::Deny { risk, .. } => Some(risk.as_str()),
            Self::Unavailable { .. } | Self::Cancelled => None,
        }
    }

    pub(crate) fn into_tool_result(self, tool_name: &str) -> Result<String, ToolError> {
        match self {
            Self::Allow { reason, .. } => Ok(reason),
            Self::Deny { reason, .. } => Err(ToolError::permission_denied(format!(
                "Auto-Review guardian denied tool '{tool_name}': {reason}. Do not work around this denial; find a materially safer path or stop."
            ))),
            Self::Unavailable { reason } => Err(ToolError::permission_denied(format!(
                "Auto-Review guardian unavailable ({reason}); the call was denied (fail closed). Switch to Ask to review this call yourself."
            ))),
            Self::Cancelled => Err(ToolError::cancelled(
                "Auto-Review guardian request cancelled",
            )),
        }
    }
}

/// One guardian review and its provider usage, when a request was dispatched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReviewerResult {
    pub(crate) outcome: ReviewerOutcome,
    pub(crate) usage: Option<Usage>,
}

impl ReviewerResult {
    fn finish(outcome: ReviewerOutcome, usage: Option<Usage>) -> Self {
        Self { outcome, usage }
    }

    fn unavailable(reason: impl Into<String>, usage: Option<Usage>) -> Self {
        Self::finish(
            ReviewerOutcome::Unavailable {
                reason: reason.into(),
            },
            usage,
        )
    }
}

/// Ask the model guardian for one decision. `context_text` carries the
/// deterministic hold and the call under review; the system prompt is fixed.
///
/// An unusable answer (clipped output, unparseable reply, transport failure)
/// is retried once with a larger output budget: the guardian never actually
/// decided, and a fail-closed denial would read as a decision it did make.
/// Failures that would repeat identically — a timeout, or a context over the
/// guardian limit — are not retried.
pub(crate) async fn consult_reviewer(
    client: &dyn ModelClient,
    context_text: &str,
    cancel_token: &CancellationToken,
) -> ReviewerResult {
    let first =
        consult_reviewer_once(client, context_text, cancel_token, REVIEWER_MAX_TOKENS).await;
    if !worth_retrying(&first) {
        return first;
    }
    let retry =
        consult_reviewer_once(client, context_text, cancel_token, REVIEWER_RETRY_MAX_TOKENS).await;
    ReviewerResult::finish(retry.outcome, merge_usage(first.usage, retry.usage))
}

/// One guardian attempt at the given output budget.
async fn consult_reviewer_once(
    client: &dyn ModelClient,
    context_text: &str,
    cancel_token: &CancellationToken,
    max_tokens: u32,
) -> ReviewerResult {
    if context_text.len() > MAX_REVIEW_CONTEXT_BYTES {
        return ReviewerResult::unavailable(CONTEXT_LIMIT_REASON, None);
    }
    let request = MessageRequest {
        model: client.model().to_string(),
        messages: vec![Message {
            role: Role::User,
            content: vec![ContentBlock::Text {
                text: context_text.to_string(),
                cache_control: None,
            }],
        }],
        max_tokens,
        system: Some(SystemPrompt::Text(DEFAULT_GUARDIAN_POLICY.to_string())),
        tools: None,
        tool_choice: None,
        metadata: None,
        thinking: None,
        // Disable reasoning for the guardian. The verdict is one small JSON
        // object; hidden thinking would spend the output budget first and
        // clip the answer. `off` is the tier spelling the client layer
        // documents per provider (providers that do not document these
        // controls are left untouched), and the same tier the translation
        // path uses for its own small decisions.
        reasoning_effort: Some("off".to_string()),
        stream: Some(false),
        temperature: Some(0.0),
        top_p: None,
    };
    if cancel_token.is_cancelled() {
        return ReviewerResult::finish(ReviewerOutcome::Cancelled, None);
    }
    let response = tokio::select! {
        biased;
        _ = cancel_token.cancelled() => {
            return ReviewerResult::finish(ReviewerOutcome::Cancelled, None);
        }
        response = tokio::time::timeout(REVIEWER_TIMEOUT, client.create_message_uncached(request)) => response,
    };
    let response = match response {
        Err(_) => return ReviewerResult::unavailable(TIMEOUT_REASON, None),
        // Provider errors can include response bodies or credential-shaped
        // details. The guardian needs only the fail-closed outcome.
        Ok(Err(_)) => return ReviewerResult::unavailable("the reviewer request failed", None),
        Ok(Ok(response)) => response,
    };
    let outcome = verdict_from_response(&response);
    ReviewerResult::finish(outcome, Some(response.usage))
}

/// Whether a second attempt could plausibly decide differently.
///
/// Only answer-level failures qualify: a clipped or unparseable reply never
/// reached a verdict, and a larger budget can produce one. A timeout already
/// spent the full deadline, and an oversized context cannot shrink, so both
/// would repeat identically.
fn worth_retrying(result: &ReviewerResult) -> bool {
    matches!(
        &result.outcome,
        ReviewerOutcome::Unavailable { reason }
            if reason.as_str() != TIMEOUT_REASON && reason.as_str() != CONTEXT_LIMIT_REASON
    )
}

/// Sum both attempts' usage: a retry is real spend, not a correction.
fn merge_usage(first: Option<Usage>, second: Option<Usage>) -> Option<Usage> {
    match (first, second) {
        (Some(first), Some(second)) => Some(Usage {
            input_tokens: first.input_tokens.saturating_add(second.input_tokens),
            output_tokens: first.output_tokens.saturating_add(second.output_tokens),
            prompt_cache_hit_tokens: sum_optional(
                first.prompt_cache_hit_tokens,
                second.prompt_cache_hit_tokens,
            ),
            prompt_cache_miss_tokens: sum_optional(
                first.prompt_cache_miss_tokens,
                second.prompt_cache_miss_tokens,
            ),
            prompt_cache_write_tokens: sum_optional(
                first.prompt_cache_write_tokens,
                second.prompt_cache_write_tokens,
            ),
            reasoning_tokens: sum_optional(first.reasoning_tokens, second.reasoning_tokens),
            reasoning_replay_tokens: sum_optional(
                first.reasoning_replay_tokens,
                second.reasoning_replay_tokens,
            ),
            server_tool_use: second.server_tool_use.or(first.server_tool_use),
        }),
        (Some(only), None) | (None, Some(only)) => Some(only),
        (None, None) => None,
    }
}

fn sum_optional(first: Option<u32>, second: Option<u32>) -> Option<u32> {
    match (first, second) {
        (Some(first), Some(second)) => Some(first.saturating_add(second)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn verdict_from_response(response: &MessageResponse) -> ReviewerOutcome {
    if is_incomplete_stop_reason(response.stop_reason.as_deref()) {
        return ReviewerOutcome::Unavailable {
            reason: "the reviewer answer was incomplete".to_string(),
        };
    }
    let text: String = response
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    match parse_reviewer_verdict(&text) {
        Some(ReviewerVerdict {
            action: AutoReviewAction::Allow,
            risk,
            reason,
        }) if risk.may_auto_run() => ReviewerOutcome::Allow { risk, reason },
        Some(ReviewerVerdict {
            action: AutoReviewAction::Allow,
            risk,
            ..
        }) => ReviewerOutcome::Deny {
            risk,
            reason: format!(
                "the reviewer classified the call as {} risk, which Auto-Review cannot run automatically",
                risk.as_str()
            ),
        },
        Some(ReviewerVerdict {
            action: AutoReviewAction::Block,
            risk,
            reason,
        }) => ReviewerOutcome::Deny { risk, reason },
        Some(_) | None => ReviewerOutcome::Unavailable {
            reason: format!("the reviewer answer was unparseable ({} chars)", text.len()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm_client::mock::MockLlmClient;

    fn response(text: &str, usage: Usage) -> MessageResponse {
        MessageResponse {
            id: "review".to_string(),
            r#type: "message".to_string(),
            role: "assistant".to_string(),
            content: vec![ContentBlock::Text {
                text: text.to_string(),
                cache_control: None,
            }],
            model: "mock-model".to_string(),
            stop_reason: Some("end_turn".to_string()),
            stop_sequence: None,
            container: None,
            usage,
        }
    }

    #[tokio::test]
    async fn guardian_rechecks_identical_calls_instead_of_reusing_a_cached_allow() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        let client = crate::client::DeepSeekClient::new(&crate::config::Config {
            api_key: Some("test-guardian-cache-key".to_string()),
            base_url: Some(server.uri()),
            ..Default::default()
        })
        .unwrap();
        for (decision, risk) in [("allow", "low"), ("deny", "high")] {
            server.reset().await;
            let verdict = serde_json::json!({
                "decision": decision,
                "risk_level": risk,
                "reason": "current authorization evidence",
            });
            Mock::given(method("POST"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "id": "guardian-fresh",
                    "object": "chat.completion",
                    "model": "deepseek-v4-pro",
                    "choices": [{"index": 0, "message": {
                        "role": "assistant", "content": verdict.to_string(),
                    }, "finish_reason": "stop"}],
                    "usage": {"prompt_tokens": 9, "completion_tokens": 3, "total_tokens": 12},
                })))
                .mount(&server)
                .await;
            let result = consult_reviewer(
                &client,
                "the same proposed call under current policy",
                &CancellationToken::new(),
            )
            .await;
            assert_eq!(result.outcome.audit_decision(), decision);
            assert_eq!(result.usage.unwrap().output_tokens, 3);
            assert_eq!(server.received_requests().await.unwrap().len(), 1);
        }
    }

    #[tokio::test]
    async fn reviewer_records_usage_and_keeps_context_untrusted() {
        let usage = Usage {
            input_tokens: 17,
            output_tokens: 9,
            ..Usage::default()
        };
        let mock = MockLlmClient::new(Vec::new());
        mock.push_message_response(response(
            r#"{"risk_level":"low","decision":"allow","reason":"bounded and authorized"}"#,
            usage.clone(),
        ));

        let result = consult_reviewer(
            &mock,
            r#"{"proposed_tool_call":{"tool":"exec_shell"}}"#,
            &CancellationToken::new(),
        )
        .await;

        assert_eq!(
            result.outcome,
            ReviewerOutcome::Allow {
                risk: ReviewerRiskLevel::Low,
                reason: "bounded and authorized".to_string()
            }
        );
        assert_eq!(result.usage, Some(usage));
        let request = mock.last_request().expect("reviewer request");
        assert_eq!(request.tools, None, "guardian requests never expose tools");
        assert_eq!(
            request.reasoning_effort.as_deref(),
            Some("off"),
            "the guardian must not spend its output budget on hidden thinking"
        );
        assert_eq!(request.max_tokens, REVIEWER_MAX_TOKENS);
        let SystemPrompt::Text(system) = request.system.expect("guardian policy") else {
            panic!("guardian system prompt must be text");
        };
        assert!(system.contains("Never infer user intent"));
        assert!(!system.contains("Prefer reversible work."));
    }

    #[tokio::test]
    async fn reviewer_distinguishes_denial_malformed_and_incomplete_answers() {
        let deny = MockLlmClient::new(Vec::new());
        deny.push_message_response(response(
            r#"{"risk_level":"high","decision":"deny","reason":"destination is not authorized"}"#,
            Usage::default(),
        ));
        assert_eq!(
            consult_reviewer(&deny, "context", &CancellationToken::new())
                .await
                .outcome,
            ReviewerOutcome::Deny {
                risk: ReviewerRiskLevel::High,
                reason: "destination is not authorized".to_string()
            }
        );
        assert_eq!(
            deny.call_count(),
            1,
            "an explicit denial is a decision, not an unusable answer"
        );
        assert!(matches!(
            verdict_from_response(&response(
                r#"{"risk_level":"high","decision":"allow","reason":"the request mentions it"}"#,
                Usage::default(),
            )),
            ReviewerOutcome::Deny {
                risk: ReviewerRiskLevel::High,
                ..
            }
        ));

        // An unusable first answer is retried with a larger budget instead of
        // denying a call the guardian never actually judged. Both attempts'
        // usage is kept — a retry is real spend.
        let malformed = MockLlmClient::new(Vec::new());
        malformed.push_message_response(response("allow it", Usage::default()));
        malformed.push_message_response(response(
            r#"{"risk_level":"low","decision":"allow","reason":"bounded and authorized"}"#,
            Usage {
                input_tokens: 11,
                output_tokens: 5,
                ..Usage::default()
            },
        ));
        let malformed_result =
            consult_reviewer(&malformed, "context", &CancellationToken::new()).await;
        assert_eq!(
            malformed_result.outcome,
            ReviewerOutcome::Allow {
                risk: ReviewerRiskLevel::Low,
                reason: "bounded and authorized".to_string()
            }
        );
        assert_eq!(malformed.call_count(), 2);
        assert_eq!(malformed_result.usage.unwrap().output_tokens, 5);
        let retry_requests = malformed.captured_requests();
        assert_eq!(retry_requests.len(), 2);
        assert_eq!(retry_requests[0].max_tokens, REVIEWER_MAX_TOKENS);
        assert_eq!(
            retry_requests[1].max_tokens, REVIEWER_RETRY_MAX_TOKENS,
            "the second attempt gets the larger budget"
        );
        assert_eq!(retry_requests[1].reasoning_effort.as_deref(), Some("off"));

        let incomplete = MockLlmClient::new(Vec::new());
        let mut incomplete_response = response(
            r#"{"risk_level":"low","decision":"allow","reason":"looks safe"}"#,
            Usage::default(),
        );
        incomplete_response.stop_reason = Some("max_tokens".to_string());
        incomplete.push_message_response(incomplete_response);
        incomplete.push_message_response(response(
            r#"{"risk_level":"medium","decision":"allow","reason":"bounded workspace edit"}"#,
            Usage::default(),
        ));
        let incomplete_result =
            consult_reviewer(&incomplete, "context", &CancellationToken::new()).await;
        assert_eq!(
            incomplete_result.outcome,
            ReviewerOutcome::Allow {
                risk: ReviewerRiskLevel::Medium,
                reason: "bounded workspace edit".to_string()
            },
            "a clipped answer must not become a fail-closed denial"
        );
        assert_eq!(incomplete.call_count(), 2);

        // A guardian that stays unusable on both attempts still fails closed:
        // the retry softens the clipping failure, it does not open the gate.
        let hopeless = MockLlmClient::new(Vec::new());
        hopeless.push_message_response(response("not json", Usage::default()));
        hopeless.push_message_response(response("still not json", Usage::default()));
        let hopeless_result =
            consult_reviewer(&hopeless, "context", &CancellationToken::new()).await;
        assert!(matches!(
            hopeless_result.outcome,
            ReviewerOutcome::Unavailable { .. }
        ));
        assert_eq!(hopeless.call_count(), 2);
    }

    #[tokio::test]
    async fn reviewer_cancellation_aborts_without_calling_provider() {
        let mock = MockLlmClient::new(Vec::new());
        let cancel = CancellationToken::new();
        cancel.cancel();

        let result = consult_reviewer(&mock, "context", &cancel).await;

        assert_eq!(result.outcome, ReviewerOutcome::Cancelled);
        assert!(result.usage.is_none());
        assert_eq!(mock.call_count(), 0);
    }

    #[tokio::test]
    async fn reviewer_denies_oversized_exact_context_without_calling_provider() {
        let mock = MockLlmClient::new(Vec::new());
        let context = "x".repeat(MAX_REVIEW_CONTEXT_BYTES + 1);

        let result = consult_reviewer(&mock, &context, &CancellationToken::new()).await;

        assert_eq!(
            result.outcome,
            ReviewerOutcome::Unavailable {
                reason: "the exact review context exceeded the guardian limit".to_string()
            }
        );
        assert!(result.usage.is_none());
        assert_eq!(mock.call_count(), 0);
    }
}
