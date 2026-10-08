//! Provider model offerings (#3084).
//!
//! A [`ProviderModelOffering`] binds a provider to a canonical model, the
//! provider-owned wire id that serves it, and the endpoint key. This is the
//! seam that proves the #2608 invariant: the SAME canonical model can be served
//! by multiple providers under DIFFERENT wire ids (some aggregator-prefixed),
//! and a prefix never implies provider ownership.
//!
//! Catalog-derived offerings from [`crate::catalog::bundled_catalog_offerings`]
//! remain the general bundled source of truth. [`bundled_offerings`] contains
//! only transport facts that Models.dev cannot express, such as a single
//! provider routing different models over different wire protocols.

use serde::{Deserialize, Serialize};

use super::candidate::PricingSku;
use super::capabilities::RouteCapabilities;
use super::ids::{ModelId, ProviderId, WireModelId};

/// Token limits for one resolved route/offering.
///
/// These are optional because hosted catalogs, local runtimes, and custom
/// endpoints can legitimately omit some or all limit facts. Callers should
/// treat `None` as unknown, not zero.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteLimits {
    /// Total context window (input + output), in tokens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_tokens: Option<u64>,
    /// Input-token limit, when the provider reports it separately.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
    /// Output-token cap for the route/offering, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
}

impl RouteLimits {
    /// Whether at least one limit fact is known.
    #[must_use]
    pub const fn has_known_limit(self) -> bool {
        self.context_tokens.is_some() || self.input_tokens.is_some() || self.output_tokens.is_some()
    }
}

/// One provider's way of serving a (possibly canonical) model.
///
/// `Eq` is intentionally NOT derived: [`PricingSku::Token`] carries `f64` rates,
/// so the offering is only `PartialEq`. No caller keys a set/map on offerings.
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderModelOffering {
    /// Provider serving this offering.
    pub provider: ProviderId,
    /// Canonical model identity, if this offering maps to one.
    pub canonical_model: Option<ModelId>,
    /// Provider-owned wire id sent on the request (verbatim).
    pub wire_model_id: WireModelId,
    /// Endpoint key the offering is served on.
    pub endpoint_key: String,
    /// Whether this is the provider's default offering.
    pub default_for_provider: bool,
    /// Provider/offering-scoped token limits, when known.
    pub limits: RouteLimits,
    /// Provider/model-scoped capability facts. Unknown is preserved rather
    /// than inferred from the wire protocol.
    pub capabilities: RouteCapabilities,
    /// Coarse route-facing pricing meter for this offering (#3085).
    ///
    /// Projected from the offering's sourced cost at the layer that owns it
    /// (`CatalogOffering::to_offering` → [`crate::pricing::route_pricing_sku`]).
    /// The resolver carries this verbatim onto the candidate; it is
    /// [`PricingSku::UnknownOrStale`] whenever no price was sourced — never a
    /// fabricated zero (the #2608 / #3085 honesty rule).
    pub pricing: PricingSku,
}

/// Endpoint key for a Zen catalog row Models.dev marks `deprecated`. It names
/// no protocol, so the resolver refuses the model locally with this reason
/// instead of sending a request Zen no longer serves.
pub const OPENCODE_ZEN_DEPRECATED_ENDPOINT_KEY: &str = "deprecated";

/// Endpoint key OpenCode Zen serves a model on, from the AI SDK package
/// OpenCode's own catalog names for it.
///
/// Models.dev's `opencode` provider is Zen's published catalog: its provider
/// default is `@ai-sdk/openai-compatible`, and a model served over another
/// wire overrides that with `provider.npm`. The package is the wire fact, so it
/// is mapped exactly and never guessed from a model-id family, which does not
/// hold on Zen (qwen3.8-flash is Messages while qwen3.8-max is Chat). Google's
/// package maps to `"google"` and any other to `"unproven"`; neither resolves
/// to a protocol, so the resolver fails closed and names the endpoint.
#[must_use]
pub fn opencode_zen_endpoint_key_for_npm(npm: Option<&str>) -> &'static str {
    match npm.map(str::trim) {
        Some("@ai-sdk/openai") => "responses",
        Some("@ai-sdk/anthropic") => "messages",
        Some("@ai-sdk/openai-compatible") => "chat",
        Some("@ai-sdk/google") => "google",
        _ => "unproven",
    }
}

/// Logical default plus every documented Zen wire id, for picker fallbacks
/// when Models.dev is stale or failed. `gpt-5.6` is the user-facing default;
/// `gpt-5.6-sol` is the proven Responses wire id.
#[must_use]
pub fn opencode_zen_picker_models() -> Vec<&'static str> {
    crate::catalog::reviewed::constants::completion_names("opencode-zen").to_vec()
}

/// Codewhale API bootstrap rows used only when the account's live
/// `GET {base}/models` cannot be fetched.
///
/// The account catalog is authoritative: it lists exactly the providers the
/// customer connected, and each row states its own protocol. These three rows
/// exist so a route can still be selected offline; every consumer that shows
/// models must say the list is a fallback, not the account's catalog.
pub use crate::catalog::reviewed::constants::CODEWHALE_FALLBACK_MODELS;

/// Endpoint key for one Codewhale API model id.
///
/// The live catalog states the protocol per model in `codewhale.protocol`;
/// this is the offline inference used for the bootstrap rows and for a model
/// id the local catalog has never seen. Only the `anthropic/` namespace routes
/// to `{base}/messages`; everything else is OpenAI Chat Completions at
/// `{base}/chat/completions`. The id alone carries no signal for the
/// Responses surface — a `responses` row only ever comes from the catalog's
/// stated `codewhale.protocol`, never from a model name.
#[must_use]
pub fn codewhale_endpoint_key_for_model(model: &str) -> &'static str {
    if model.trim().to_ascii_lowercase().starts_with("anthropic/") {
        "messages"
    } else {
        "chat"
    }
}

/// Return curated provider/model transport facts as owned offering rows.
///
/// OpenCode Zen's official catalog serves models over three protocol families.
/// These rows intentionally carry no inferred limits, pricing, or canonical
/// identity: their sole claim is the documented wire model and endpoint key.
#[must_use]
pub fn bundled_offerings() -> Vec<ProviderModelOffering> {
    crate::catalog::reviewed::bundled_reviewed()
        .transports
        .iter()
        .map(crate::catalog::reviewed::ReviewedTransport::to_offering)
        .collect()
}
