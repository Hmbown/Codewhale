//! OpenCode Go's provider-owned transport roster.
//!
//! Reviewed against <https://opencode.ai/docs/go/#endpoints> on 2026-09-12.
//! Previously accepted Chat ids remain compatible absent explicit deprecation.
//! Membership proves a wire protocol, not live availability, limits or pricing.

pub(crate) const MODEL_GROUPS: &[(&str, &[&str])] = &[
    (
        "chat",
        &[
            crate::DEFAULT_OPENCODE_GO_MODEL,
            crate::OPENCODE_GO_GROK_4_5_MODEL,
            crate::OPENCODE_GO_GLM_5_2_MODEL,
            crate::OPENCODE_GO_GLM_5_1_MODEL,
            crate::OPENCODE_GO_KIMI_K3_MODEL,
            crate::OPENCODE_GO_KIMI_K2_7_CODE_MODEL,
            crate::OPENCODE_GO_KIMI_K2_6_MODEL,
            crate::OPENCODE_GO_DEEPSEEK_V4_FLASH_MODEL,
            crate::OPENCODE_GO_MIMO_V2_5_MODEL,
            crate::OPENCODE_GO_MIMO_V2_5_PRO_MODEL,
            "glm-5.3-flash",
            "glm-5.3",
            "longcat-2.0",
            "deepseek-v4-flash-vision-exp",
            "hy4-preview",
            "hy3",
            "omen-alpha",
            "deepseek-v4.1-flash",
        ],
    ),
    (
        "responses",
        &[
            "grok-4.6",
            "gpt-5.6-luna",
            "muse-spark-1.3-contributor",
            "muse-spark-1.2-contributor",
        ],
    ),
    (
        "messages",
        &[
            "minimax-m3",
            "minimax-m2.7",
            "minimax-m2.5",
            "qwen3.8-max",
            "qwen3.8-flash",
            "qwen3.7-max",
            "qwen3.7-plus",
            "qwen3.6-plus",
        ],
    ),
];

/// Every documented Go wire id, in stable picker order.
#[must_use]
pub fn opencode_go_models() -> Vec<&'static str> {
    MODEL_GROUPS
        .iter()
        .flat_map(|(_, models)| models.iter().copied())
        .collect()
}

/// Canonicalize only IDs whose Go protocol is known. Unknown models cannot
/// silently fall through to Chat, another provider, or the default model.
#[must_use]
pub fn opencode_go_model_id(model: &str) -> Option<&'static str> {
    let normalized = model.trim().to_ascii_lowercase().replace(['_', ' '], "-");
    let normalized = normalized
        .strip_prefix("opencode-go/")
        .unwrap_or(&normalized);
    let normalized = match normalized {
        "grok-4-5" => "grok-4.5",
        "glm-5-2" => "glm-5.2",
        "glm-5-1" => "glm-5.1",
        "kimi-k2-7-code" => "kimi-k2.7-code",
        "kimi-k2-6" => "kimi-k2.6",
        "deepseek-v4pro" => "deepseek-v4-pro",
        "deepseek-v4flash" => "deepseek-v4-flash",
        "mimo-v2-5" => "mimo-v2.5",
        "mimo-v2-5-pro" => "mimo-v2.5-pro",
        other => other,
    };
    MODEL_GROUPS
        .iter()
        .flat_map(|(_, models)| models.iter().copied())
        .find(|candidate| *candidate == normalized)
}

/// The documented endpoint for a Go model, independent of stale catalog metadata.
#[must_use]
pub fn opencode_go_endpoint_key(model: &str) -> Option<&'static str> {
    let canonical = opencode_go_model_id(model)?;
    MODEL_GROUPS
        .iter()
        .find_map(|(endpoint, models)| models.contains(&canonical).then_some(*endpoint))
}
