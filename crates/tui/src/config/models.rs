//! Named model aliases and curated rosters.
//! Provider default seeds are generated from the shared descriptor owner;
//! model aliases and completion rosters are generated from the same reviewed catalog.

pub use codewhale_config::catalog::reviewed::constants::*;
pub use codewhale_config::descriptors::defaults::*;

pub const XIAOMI_MIMO_PAY_AS_YOU_GO_BASE_URL: &str = "https://api.xiaomimimo.com/v1";

pub const XIAOMI_MIMO_TOKEN_PLAN_CN_BASE_URL: &str = "https://token-plan-cn.xiaomimimo.com/v1";
pub const XIAOMI_MIMO_TOKEN_PLAN_SGP_BASE_URL: &str = DEFAULT_XIAOMI_MIMO_BASE_URL;
pub const XIAOMI_MIMO_TOKEN_PLAN_AMS_BASE_URL: &str = "https://token-plan-ams.xiaomimimo.com/v1";

pub const KIMI_CODE_MEMBERSHIP_PLAN_CONSOLE_URL: &str =
    codewhale_config::provider::KIMI_CODE_MEMBERSHIP_PLAN_CONSOLE_URL;
// The K3 contract constants (`KIMI_CODE_K3_CONTEXT_WINDOW_TOKENS`,
// `KIMI_K3_CONTEXT_WINDOW_TOKENS`, and the distinct default/direct output
// limits) retain the existing `codewhale_models` typed contract. Intrinsic
// projections use the reviewed catalog; route floors remain Rust policy.
// Re-export only the route-owned floor, which existing `crate::config` call
// sites import.
pub use codewhale_models::KIMI_CODE_K3_CONTEXT_WINDOW_TOKENS;

/// True when `model` is the pre-refresh local-Ollama placeholder, not a tag.
#[must_use]
pub fn is_unresolved_local_ollama_model(model: &str) -> bool {
    model.trim().eq_ignore_ascii_case(DEFAULT_OLLAMA_MODEL)
}

/// Conservative offline floor for an OAuth model absent from a fresh Codex
/// roster. Fresh account-scoped cache metadata overrides this in route_runtime.
pub const OPENAI_CODEX_EFFECTIVE_CONTEXT_WINDOW_TOKENS: u32 = 128_000;

pub use codewhale_config::opencode_go_models;
