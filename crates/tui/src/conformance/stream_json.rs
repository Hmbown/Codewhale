//! The one normalized stream-event format both the `sse` and `events`
//! families speak.
//!
//! It is exactly the serde shape `codewhale_models::StreamEvent` already
//! deserializes (Anthropic-style `{"type": "content_block_delta", ...}`), so a
//! scripted provider script and a parser golden are the same language and a
//! TypeScript implementation needs no Rust type to read either. `StreamEvent`
//! is `Deserialize`-only; this module is its serializer, and
//! [`round_trips`] proves the two directions agree for every golden line.

use codewhale_models::{ContentBlockStart, Delta, StreamEvent};
use serde_json::{Value, json};

pub(super) fn to_json(event: &StreamEvent) -> Value {
    match event {
        StreamEvent::ToolProjectionWarning {
            provider,
            omitted_tool_names,
            omitted_tool_count,
        } => json!({
            "type": "tool_projection_warning",
            "provider": provider,
            "omitted_tool_names": omitted_tool_names,
            "omitted_tool_count": omitted_tool_count,
        }),
        StreamEvent::MessageStart { message } => json!({
            "type": "message_start",
            "message": serde_json::to_value(message).expect("MessageResponse serializes"),
        }),
        StreamEvent::ContentBlockStart {
            index,
            content_block,
        } => json!({
            "type": "content_block_start",
            "index": index,
            "content_block": block_start_json(content_block),
        }),
        StreamEvent::ContentBlockDelta { index, delta } => json!({
            "type": "content_block_delta",
            "index": index,
            "delta": delta_json(delta),
        }),
        StreamEvent::ContentBlockStop { index } => json!({
            "type": "content_block_stop",
            "index": index,
        }),
        StreamEvent::MessageDelta { delta, usage } => {
            let mut value = json!({
                "type": "message_delta",
                "delta": {
                    "stop_reason": delta.stop_reason,
                    "stop_sequence": delta.stop_sequence,
                },
            });
            if let Some(usage) = usage {
                value["usage"] = serde_json::to_value(usage).expect("Usage serializes");
            }
            value
        }
        StreamEvent::MessageStop => json!({ "type": "message_stop" }),
        StreamEvent::Ping => json!({ "type": "ping" }),
        StreamEvent::Error { error } => json!({ "type": "error", "error": error }),
    }
}

fn block_start_json(block: &ContentBlockStart) -> Value {
    match block {
        ContentBlockStart::Text { text } => json!({ "type": "text", "text": text }),
        ContentBlockStart::Thinking { thinking } => {
            json!({ "type": "thinking", "thinking": thinking })
        }
        ContentBlockStart::ToolUse {
            id,
            name,
            input,
            caller,
            thought_signature,
        } => {
            let mut value = json!({
                "type": "tool_use",
                "id": id,
                "name": name,
                "input": input,
            });
            if let Some(caller) = caller {
                value["caller"] = serde_json::to_value(caller).expect("ToolCaller serializes");
            }
            if let Some(signature) = thought_signature {
                value["thought_signature"] = json!(signature);
            }
            value
        }
        ContentBlockStart::ServerToolUse { id, name, input } => json!({
            "type": "server_tool_use",
            "id": id,
            "name": name,
            "input": input,
        }),
    }
}

fn delta_json(delta: &Delta) -> Value {
    match delta {
        Delta::TextDelta { text } => json!({ "type": "text_delta", "text": text }),
        Delta::ThinkingDelta { thinking } => {
            json!({ "type": "thinking_delta", "thinking": thinking })
        }
        Delta::InputJsonDelta { partial_json } => {
            json!({ "type": "input_json_delta", "partial_json": partial_json })
        }
        Delta::SignatureDelta { signature } => {
            json!({ "type": "signature_delta", "signature": signature })
        }
        Delta::ReasoningStateDelta { state } => json!({
            "type": "reasoning_state_delta",
            "state": serde_json::to_value(state).expect("OpaqueReasoningState serializes"),
        }),
    }
}

pub(super) fn from_json(value: &Value) -> anyhow::Result<StreamEvent> {
    Ok(serde_json::from_value(value.clone())?)
}

/// `value` deserializes into a `StreamEvent` that serializes back to itself.
pub(super) fn round_trips(value: &Value) -> Result<(), String> {
    let event = from_json(value).map_err(|error| format!("{value} does not parse: {error}"))?;
    let back = to_json(&event);
    if super::golden::canonical(&back) == super::golden::canonical(value) {
        Ok(())
    } else {
        Err(format!("{value} round-trips to {back}"))
    }
}
