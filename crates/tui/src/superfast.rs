//! Superfast Decision Gate — a small, off-by-default "System One" front-door
//! classifier for the agent turn loop.
//!
//! Every user message normally wakes a large, slow, expensive model just to
//! decide intent and whether a tool is needed. The Decision Gate asks a small,
//! fast decision model (Von, or any Jev-compatible server) those routine
//! questions in one non-generating pass, and derives a conservative routing
//! recommendation. A later, validated step could send each turn to the cheapest
//! correct path; this first increment only measures and logs.
//!
//! Design contract (identical to the reference implementation):
//!   - Off by default. Nothing runs unless `SUPERFAST_ENABLED` is set to a
//!     truthy value. A user who does nothing sees the exact current behavior.
//!   - Shadow mode. The gate classifies the turn and logs its recommendation
//!     through `tracing`, but never changes routing, never skips the model
//!     call, and never alters any user-visible behavior.
//!   - Fail open. Any error, timeout, non-2xx response, unreachable backend,
//!     or malformed body yields "no opinion" and the agent continues exactly
//!     as if the gate were off. It never throws into the loop and never adds
//!     latency to the real turn (the request runs on a detached task with a
//!     short timeout).
//!   - No heavy new dependencies. It talks to the decision backend with a
//!     plain HTTP POST using `reqwest`, which the crate already depends on.
//!     The decision model itself is installed out of band, not bundled.
//!
//! The Decision Gate concept and the reference implementation are by Andrea
//! Bruno, released under Creative Commons Attribution 4.0 (CC BY 4.0). See
//! the harness-superfast white paper for the full design. The decision models
//! (Von, OpenJev, Laya) are third-party open models; only the integration
//! architecture and the routing method here are covered by that attribution.

use std::env;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use codewhale_core::request::{ContentBlock, Message};
use codewhale_core::role::Role;

/// Master switch. The gate never runs unless this env var is truthy.
const ENABLED_VAR: &str = "SUPERFAST_ENABLED";
/// Full URL of the decision endpoint.
const ENDPOINT_VAR: &str = "SUPERFAST_ENDPOINT";
/// Model id sent in the request body.
const MODEL_VAR: &str = "SUPERFAST_MODEL";
/// Hard timeout for a single decision call, in milliseconds.
const TIMEOUT_VAR: &str = "SUPERFAST_TIMEOUT_MS";

const DEFAULT_ENDPOINT: &str = "http://localhost:8000/v1/systemone";
const DEFAULT_MODEL: &str = "von-1.2.0";
const DEFAULT_TIMEOUT_MS: u64 = 150;

/// Conservative routing recommendation derived from a turn's answers. Only a
/// decisive set of numbers produces a fast route; anything else is `Unknown`,
/// which means "fall back to the full model exactly as today".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    NeedsTool,
    AnswerFromContext,
    PlainChat,
    Unknown,
}

impl Route {
    fn as_str(self) -> &'static str {
        match self {
            Route::NeedsTool => "needs_tool",
            Route::AnswerFromContext => "answer_from_context",
            Route::PlainChat => "plain_chat",
            Route::Unknown => "unknown",
        }
    }
}

/// True when `SUPERFAST_ENABLED` is set to a truthy value.
fn enabled() -> bool {
    env::var(ENABLED_VAR).ok().is_some_and(|v| {
        matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

/// Text of the last user message, or `None` when there is none or it is blank.
fn last_user_text(messages: &[Message]) -> Option<String> {
    let message = messages.iter().rev().find(|m| m.role == Role::User)?;
    let text = message
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// Fire the shadow gate for a turn. Returns immediately: the classification
/// runs on a detached task and only logs. This is a no-op when the gate is
/// disabled, when there is no user text, or when no tokio runtime is present.
/// It never blocks or alters the real model request.
pub fn spawn_shadow_gate(messages: &[Message]) {
    if !enabled() {
        return;
    }
    let Some(state) = last_user_text(messages) else {
        return;
    };
    // A detached task needs a runtime handle; if there is none (for example a
    // bare unit test), skip rather than panic.
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        return;
    };
    let endpoint = env::var(ENDPOINT_VAR).unwrap_or_else(|_| DEFAULT_ENDPOINT.to_string());
    let model = env::var(MODEL_VAR).unwrap_or_else(|_| DEFAULT_MODEL.to_string());
    let timeout_ms = env::var(TIMEOUT_VAR)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|t| *t > 0)
        .unwrap_or(DEFAULT_TIMEOUT_MS);

    handle.spawn(async move {
        let started = Instant::now();
        let route = classify(&state, &endpoint, &model, timeout_ms).await;
        let latency_ms = started.elapsed().as_millis();
        match route {
            Some(route) => tracing::info!(
                target: "superfast",
                route = route.as_str(),
                latency_ms,
                "decision gate (shadow) recommendation"
            ),
            None => tracing::debug!(
                target: "superfast",
                latency_ms,
                "decision gate (shadow) no opinion (fail-open)"
            ),
        }
    });
}

/// Ask the decision backend and derive a conservative route. Returns `None`
/// on any error, timeout, non-2xx response, or malformed body (fail open).
async fn classify(state: &str, endpoint: &str, model: &str, timeout_ms: u64) -> Option<Route> {
    let body = json!({
        "model": model,
        "state": state,
        "questions": {
            "needs_tool": {
                "type": "noul",
                "instructions": "Does answering this request require taking an action with a tool (reading, writing, running, searching), rather than replying from what is already known?"
            },
            "answerable_from_context": {
                "type": "noul",
                "instructions": "Can this request be answered from information already present in the conversation, without any new investigation?"
            },
            "intent": {
                "type": "choice",
                "instructions": "Classify the primary intent of the user request.",
                "criteria": {
                    "code_change": "Create, edit, or delete code or files.",
                    "code_question": "Explain or reason about code without changing it.",
                    "command": "Run a command or operation.",
                    "chat": "Casual conversation or a question needing no tools.",
                    "other": "None of the above."
                }
            }
        }
    });

    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(timeout_ms))
        .build()
        .ok()?;
    let res = client.post(endpoint).json(&body).send().await.ok()?;
    if !res.status().is_success() {
        return None;
    }
    let parsed: Value = res.json().await.ok()?;
    let answers = parsed.get("answers")?.as_object()?;
    Some(derive_route(answers))
}

/// Read a noul probability only when it is a real, finite value in [0, 1].
/// Anything else (absent, NaN, Infinity, out of range, wrong type) is treated
/// as "no evidence", so a mis-scaled or missing answer can never produce a
/// decisive fast route.
fn read_noul(answers: &serde_json::Map<String, Value>, key: &str) -> Option<f64> {
    let value = answers.get(key)?.get("noul")?.as_f64()?;
    (value.is_finite() && (0.0..=1.0).contains(&value)).then_some(value)
}

/// Read a calibrated confidence only when it is a real, finite value in [0, 1].
fn read_confidence(answers: &serde_json::Map<String, Value>, key: &str) -> Option<f64> {
    let value = answers.get(key)?.get("confidence")?.as_f64()?;
    (value.is_finite() && (0.0..=1.0).contains(&value)).then_some(value)
}

/// Derive a conservative route. The gate only recommends a fast route when the
/// relevant numbers are decisive; otherwise it says `Unknown` so the caller
/// falls back to the normal path.
fn derive_route(answers: &serde_json::Map<String, Value>) -> Route {
    let needs_tool = read_noul(answers, "needs_tool");
    let from_context = read_noul(answers, "answerable_from_context");

    // Decisive "needs a tool" wins first — the harness must not skip work.
    if needs_tool.is_some_and(|nt| nt >= 0.85) {
        return Route::NeedsTool;
    }

    // Strongly answerable from context, with a present and low tool-need signal.
    if from_context.is_some_and(|fc| fc >= 0.85) && needs_tool.is_some_and(|nt| nt <= 0.3) {
        return Route::AnswerFromContext;
    }

    // Clearly chat, with a calibrated intent and a present, low tool-need signal.
    let intent_is_chat = answers
        .get("intent")
        .and_then(|intent| intent.get("choice"))
        .and_then(|choice| choice.as_str())
        == Some("chat");
    if intent_is_chat
        && read_confidence(answers, "intent").is_some_and(|c| c >= 0.5)
        && needs_tool.is_some_and(|nt| nt <= 0.2)
    {
        return Route::PlainChat;
    }

    Route::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answers(raw: Value) -> serde_json::Map<String, Value> {
        raw.as_object().unwrap().clone()
    }

    #[test]
    fn decisive_tool_need_routes_to_needs_tool() {
        let a = answers(json!({ "needs_tool": { "noul": 0.9 } }));
        assert_eq!(derive_route(&a), Route::NeedsTool);
    }

    #[test]
    fn answerable_from_context_with_low_tool_need() {
        let a = answers(json!({
            "needs_tool": { "noul": 0.1 },
            "answerable_from_context": { "noul": 0.9 }
        }));
        assert_eq!(derive_route(&a), Route::AnswerFromContext);
    }

    #[test]
    fn clear_chat_with_calibrated_intent_and_very_low_tool_need() {
        let a = answers(json!({
            "needs_tool": { "noul": 0.05 },
            "intent": { "choice": "chat", "confidence": 0.8 }
        }));
        assert_eq!(derive_route(&a), Route::PlainChat);
    }

    #[test]
    fn non_finite_and_out_of_range_are_no_evidence() {
        // NaN/Infinity cannot be expressed in strict JSON, but a wrong-type or
        // out-of-range value must not route fast.
        let a = answers(json!({
            "needs_tool": { "noul": 5.0 },
            "answerable_from_context": { "noul": "high" }
        }));
        assert_eq!(derive_route(&a), Route::Unknown);
    }

    #[test]
    fn empty_answers_is_unknown() {
        let a = answers(json!({}));
        assert_eq!(derive_route(&a), Route::Unknown);
    }

    #[test]
    fn last_user_text_picks_last_user_message_text() {
        let messages = vec![
            Message {
                role: Role::User,
                content: vec![ContentBlock::Text {
                    text: "first".to_string(),
                    cache_control: None,
                }],
            },
            Message {
                role: Role::Assistant,
                content: vec![ContentBlock::Text {
                    text: "reply".to_string(),
                    cache_control: None,
                }],
            },
            Message {
                role: Role::User,
                content: vec![ContentBlock::Text {
                    text: "  latest question  ".to_string(),
                    cache_control: None,
                }],
            },
        ];
        assert_eq!(
            last_user_text(&messages).as_deref(),
            Some("latest question")
        );
    }

    #[test]
    fn last_user_text_none_when_no_user_message() {
        let messages = vec![Message {
            role: Role::Assistant,
            content: vec![ContentBlock::Text {
                text: "only assistant".to_string(),
                cache_control: None,
            }],
        }];
        assert_eq!(last_user_text(&messages), None);
    }
}
