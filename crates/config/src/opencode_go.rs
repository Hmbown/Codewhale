//! OpenCode Go's provider-owned transport roster.
//!
//! Reviewed against <https://opencode.ai/docs/go/#endpoints> on 2026-09-12.
//! Previously accepted Chat ids remain compatible absent explicit deprecation.
//! Membership proves a wire protocol, not live availability, limits or pricing.

/// Every documented Go wire id, in stable picker order.
#[must_use]
pub fn opencode_go_models() -> Vec<&'static str> {
    crate::catalog::reviewed::constants::OPENCODE_GO_MODELS.to_vec()
}

/// Canonicalize only IDs whose Go protocol is known. Unknown models cannot
/// silently fall through to Chat, another provider, or the default model.
#[must_use]
pub fn opencode_go_model_id(model: &str) -> Option<&'static str> {
    let normalized = model.trim().to_ascii_lowercase().replace(['_', ' '], "-");
    let normalized = normalized
        .strip_prefix("opencode-go/")
        .unwrap_or(&normalized);
    let reviewed = crate::catalog::reviewed::bundled_reviewed();
    let normalized = reviewed
        .go_aliases
        .get(normalized)
        .map_or(normalized, String::as_str);
    crate::catalog::reviewed::constants::OPENCODE_GO_MODELS
        .iter()
        .copied()
        .find(|id| *id == normalized)
}

/// The documented endpoint for a Go model, independent of stale catalog metadata.
#[must_use]
pub fn opencode_go_endpoint_key(model: &str) -> Option<&'static str> {
    let canonical = opencode_go_model_id(model)?;
    crate::catalog::reviewed::reviewed_transport("opencode-go", canonical)
        .map(|row| row.endpoint_key.as_str())
}
