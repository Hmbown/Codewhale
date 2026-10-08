//! Recursive Language Model presentation and persistent Python context.
//!
//! The serving Engine captures one call-local route, policy, service and
//! cancellation receipt. `llm_query` and `rlm_query` submit bounded invocations
//! to the same `Engine::run_turn`; this module owns no provider/code loop.
//!
//! Long input stays in `_context` (`_ctx` / `content` compatibility aliases).
//! Core's session keeps initial metadata plus every code/result round until
//! FINAL, the strict partial-response refusal or the bounded deadline/cap.
//! Python kernels retain variables and pipes, never their borrowed dispatcher
//! or caller. The persistent `rlm` action surface is caller-session scoped;
//! `share_session=true` explicitly refuses. No HTTP sidecar is created.

use codewhale_models::Usage;

pub mod bridge;
pub mod prompt;
pub mod session;
pub mod turn;

pub(crate) use bridge::RlmBridge;
pub use prompt::rlm_system_prompt;
pub use turn::{RlmTermination, RlmTurnResult};

fn add_usage_with_prompt_cache(total: &mut Usage, delta: &Usage) {
    total.input_tokens = total.input_tokens.saturating_add(delta.input_tokens);
    total.output_tokens = total.output_tokens.saturating_add(delta.output_tokens);
    total.prompt_cache_hit_tokens =
        add_optional_usage(total.prompt_cache_hit_tokens, delta.prompt_cache_hit_tokens);
    total.prompt_cache_miss_tokens = add_optional_usage(
        total.prompt_cache_miss_tokens,
        delta.prompt_cache_miss_tokens,
    );
    total.prompt_cache_write_tokens = add_optional_usage(
        total.prompt_cache_write_tokens,
        delta.prompt_cache_write_tokens,
    );
    total.reasoning_tokens = add_optional_usage(total.reasoning_tokens, delta.reasoning_tokens);
    total.reasoning_replay_tokens =
        add_optional_usage(total.reasoning_replay_tokens, delta.reasoning_replay_tokens);
    if let Some(delta_server) = delta.server_tool_use.as_ref() {
        let total_server = total.server_tool_use.get_or_insert_default();
        total_server.code_execution_requests = add_optional_usage(
            total_server.code_execution_requests,
            delta_server.code_execution_requests,
        );
        total_server.tool_search_requests = add_optional_usage(
            total_server.tool_search_requests,
            delta_server.tool_search_requests,
        );
    }
}

fn add_optional_usage(total: Option<u32>, delta: Option<u32>) -> Option<u32> {
    match (total, delta) {
        (Some(total), Some(delta)) => Some(total.saturating_add(delta)),
        (None, Some(delta)) => Some(delta),
        (Some(total), None) => Some(total),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_usage_with_prompt_cache_preserves_cache_counts() {
        let mut total = Usage {
            input_tokens: 100,
            output_tokens: 10,
            prompt_cache_hit_tokens: Some(80),
            prompt_cache_miss_tokens: Some(20),
            ..Usage::default()
        };
        let delta = Usage {
            input_tokens: 50,
            output_tokens: 5,
            prompt_cache_hit_tokens: Some(30),
            prompt_cache_miss_tokens: Some(20),
            reasoning_tokens: Some(4),
            reasoning_replay_tokens: Some(3),
            server_tool_use: Some(codewhale_models::ServerToolUsage {
                code_execution_requests: Some(2),
                tool_search_requests: Some(1),
            }),
            ..Usage::default()
        };

        add_usage_with_prompt_cache(&mut total, &delta);

        assert_eq!(total.input_tokens, 150);
        assert_eq!(total.output_tokens, 15);
        assert_eq!(total.prompt_cache_hit_tokens, Some(110));
        assert_eq!(total.prompt_cache_miss_tokens, Some(40));
        assert_eq!(total.reasoning_tokens, Some(4));
        assert_eq!(total.reasoning_replay_tokens, Some(3));
        assert_eq!(
            total.server_tool_use,
            Some(codewhale_models::ServerToolUsage {
                code_execution_requests: Some(2),
                tool_search_requests: Some(1),
            })
        );
    }
}
