//! Offline coverage of shipped defaults, not arbitrary live/account model rosters.
//! Rate authority stays in the production audit; the fixture stores only exact,
//! source-reviewed omissions. Never regenerate it from observed audit outcomes.

use std::collections::{BTreeMap, BTreeSet};

use codewhale_config::ProviderKind;
use codewhale_config::descriptors::{DescriptorWire, bundled_provider_descriptors};
use codewhale_config::provider::all_providers;
use codewhale_config::route::{LogicalModelRef, RouteRequest, RouteResolver};

use super::*;
use crate::config::{Config, ProviderConfig};
use crate::cost_status::EffectiveRouteEnvelope;
use crate::test_support::EnvVarGuard;

// Identity, exact wire ID, exact endpoint, protocol, captured billing mode.
// Keep literal model/endpoint values in reviewed exemptions: referring to an
// expanding default/roster constant would automatically bless a new omission.
type RouteKey = [String; 5];

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewedOmissions {
    reason: String,
    review: String,
    routes: Vec<RouteKey>,
}

fn reviewed_omissions() -> BTreeMap<RouteKey, UnpricedReason> {
    let groups: Vec<ReviewedOmissions> =
        serde_json::from_str(include_str!("default_coverage_unpriced.json"))
            .expect("reviewed default coverage fixture");
    let mut reviewed = BTreeMap::new();
    for group in groups {
        let reason = UnpricedReason::from_label(&group.reason);
        assert_eq!(reason.label(), group.reason, "unknown review reason");
        assert!(!group.review.trim().is_empty(), "missing source review");
        assert!(!group.routes.is_empty(), "empty review group");
        for key in group.routes {
            assert!(
                reviewed.insert(key.clone(), reason).is_none(),
                "duplicate default coverage exemption: {key:?}"
            );
        }
    }
    reviewed
}

#[test]
fn shipped_default_routes_have_reviewed_pricing_coverage() {
    let _live = crate::provider_lake::lock_live_snapshot();
    // Explicit table endpoints below bypass endpoint overrides. These remaining
    // variables can still change billing products or legacy Ollama migration.
    let _env: Vec<_> = [
        "MINIMAX_API_KEY",
        "XIAOMI_MIMO_MODE",
        "XIAOMI_MIMO_BASE_URL",
        "XIAOMI_MIMO_TOKEN_PLAN_API_KEY",
        "MIMO_TOKEN_PLAN_API_KEY",
        "XIAOMI_MIMO_API_KEY",
        "XIAOMI_API_KEY",
        "MIMO_API_KEY",
        "OLLAMA_BASE_URL",
    ]
    .into_iter()
    .map(EnvVarGuard::remove)
    .collect();
    let _cloud = EnvVarGuard::set("CODEWHALE_DISABLE_CLOUD_FACTS", "1");
    crate::provider_lake::clear_live_snapshot();
    crate::provider_catalog_live::reset_cache_for_test();

    let resolver = RouteResolver::new();
    let mut defaults = Vec::new();
    for entry in all_providers() {
        if entry.kind() == ProviderKind::Antigravity {
            continue; // Retired tombstone, not a shipped runnable choice.
        }
        let candidate = resolver
            .resolve(&RouteRequest {
                explicit_provider: Some(entry.kind()),
                ..Default::default()
            })
            .unwrap_or_else(|error| panic!("{} default cannot resolve: {error}", entry.id()));
        let provider = ApiProvider::from_kind(entry.kind());
        let mut config = Config {
            provider: Some(entry.id().to_string()),
            ..Default::default()
        };
        config.provider_config_for_mut(provider).base_url =
            Some(candidate.endpoint().base_url.clone());
        defaults.push((config, provider, candidate));
    }
    let built_in_count = defaults.len();
    for descriptor in bundled_provider_descriptors() {
        assert_eq!(descriptor.wire, DescriptorWire::OpenaiCompatible);
        let mut config = Config {
            provider: Some(descriptor.id.clone()),
            ..Default::default()
        };
        *config.provider_config_for_mut(ApiProvider::Custom) = ProviderConfig {
            kind: Some("openai-compatible".to_string()),
            base_url: Some(descriptor.base_url.clone()),
            model: Some(descriptor.default_model.clone()),
            ..Default::default()
        };
        config
            .resolve_provider_identity(&descriptor.id)
            .expect("named compatible default must retain a valid identity");
        let candidate = resolver
            .resolve(&RouteRequest {
                explicit_provider: Some(ProviderKind::Custom),
                model_selector: Some(LogicalModelRef::from(descriptor.default_model.clone())),
                base_url_override: Some(config.base_url_for_route(ApiProvider::Custom)),
                ..Default::default()
            })
            .expect("named compatible bootstrap route must resolve");
        defaults.push((config, ApiProvider::Custom, candidate));
    }

    // A deliberate breadth receipt, including the generic Custom placeholder.
    // Inventory still comes from production owners, not these expected counts.
    assert_eq!(built_in_count, 51, "review changed shipped-default breadth");
    assert_eq!(
        defaults.len() - built_in_count,
        8,
        "review compatible-default breadth"
    );
    let recorded_at = Utc.with_ymd_and_hms(2026, 9, 8, 12, 0, 0).unwrap();
    // Both ordinary token classes are actually used. Cache and long-context
    // tiers have separate existing tests; this guard makes no all-tier claim.
    let usage = Usage {
        input_tokens: 1_000,
        output_tokens: 100,
        ..Default::default()
    };
    let mut seen = BTreeSet::new();
    let mut unpriced = BTreeMap::new();
    for (config, provider, candidate) in defaults {
        let base_url = config.base_url_for_route(provider);
        // Config normalizes the optional trailing separator before dispatch.
        // Retain the exact host/path comparison and audit that runtime form.
        assert_eq!(
            base_url,
            candidate.endpoint().base_url.trim_end_matches('/')
        );
        let route = EffectiveRouteEnvelope::capture(
            Some(&config),
            provider,
            config.provider_identity_for(provider),
            candidate.wire_model_id().as_str(),
            Some(&base_url),
            recorded_at,
        );
        assert!(
            route.provider_live_pricing.is_none(),
            "offline default captured live pricing"
        );
        let key = [
            route.provider_identity.clone(),
            route.model.clone(),
            base_url,
            serde_json::to_value(candidate.protocol())
                .unwrap()
                .as_str()
                .unwrap()
                .to_string(),
            serde_json::to_value(route.billing_mode)
                .unwrap()
                .as_str()
                .unwrap()
                .to_string(),
        ];
        assert!(seen.insert(key.clone()), "duplicate default route: {key:?}");
        let audit = route.audit(&usage);
        if let Some(reason) = audit.unpriced_reason {
            assert!(
                audit.estimate.is_none(),
                "unpriced route has an estimate: {key:?}"
            );
            unpriced.insert(key, reason);
        } else {
            assert!(
                audit.usd_priced || audit.cny_priced,
                "no authoritative currency: {key:?}"
            );
            assert!(
                audit
                    .provenance
                    .is_some_and(|source| source != PricingProvenance::Unknown)
            );
            assert!(
                audit.unpriced_classes.is_empty(),
                "used token class missing: {key:?}"
            );
            assert!(
                audit
                    .estimate
                    .is_some_and(CostEstimate::is_finite_nonnegative),
                "invalid price: {key:?}"
            );
        }
    }
    assert_eq!(
        unpriced,
        reviewed_omissions(),
        "New or changed omissions need exact source review; newly priced/removed routes must retire stale exemptions"
    );
}
