//! Apply cloud model patches to a catalog layer map (layer 15: above bundled
//! and live models.dev, below provider `/v1/models`, config, and user rows).
//!
//! Patch semantics:
//! - `Upsert`: only the fields the patch sets shadow the row; a patch for a
//!   row that does not exist is materialized only when it carries a context
//!   window or an explicit `allow_unlisted` assertion (otherwise skipped with
//!   a receipt). An attested id-only row keeps every unstated fact unknown.
//! - `Deprecate`: annotates (the note is carried in `reasoning_options` as a
//!   `{"cloud_facts": {...}}` marker); never removes.
//! - `Hide`: removes the row only when it came from the bundled or live
//!   models.dev layers. Provider-live/config/user rows are never hidden.
//!
//! [`complete_provider_live_row`] is the one seam above this layer: it fills
//! fields a provider-owned row leaves unknown without displacing anything the
//! provider actually said.

use std::collections::BTreeMap;

use serde_json::json;

use super::scope::ScopedFacts;
use super::types::{ModelFact, ModelOp};
use crate::catalog::{CatalogOffering, CatalogSource};
use crate::models_dev::ModelsDevCost;

/// Merge key used by the catalog compiler.
type Key = (String, String);

/// Receipt for one patch that changed nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedPatch {
    pub provider: String,
    pub id: String,
    pub reason: String,
}

/// Apply every model patch in `facts` to `rows`, returning skip receipts.
pub fn apply_model_patches(
    rows: &mut BTreeMap<Key, CatalogOffering>,
    facts: &ScopedFacts,
    fetched_at: u64,
) -> Vec<SkippedPatch> {
    let mut skipped = Vec::new();
    if !facts.is_current_at(crate::catalog::now_unix()) {
        return skipped;
    }
    let source = CatalogSource::CloudFacts {
        facts_version: facts.facts_version,
        key_id: facts.key_id.clone(),
        fetched_at,
        valid_until: facts.valid_until,
    };
    for patch in &facts.models {
        let key = (patch.provider.clone(), patch.id.clone());
        if rows.get(&key).is_some_and(|row| {
            !matches!(
                row.source,
                CatalogSource::Bundled
                    | CatalogSource::CodewhaleBundled { .. }
                    | CatalogSource::ModelsDevLive { .. }
                    | CatalogSource::CloudFacts { .. }
            )
        }) {
            skipped.push(SkippedPatch {
                provider: patch.provider.clone(),
                id: patch.id.clone(),
                reason: "patch ignored: row comes from a higher layer".into(),
            });
            continue;
        }
        match patch.op {
            ModelOp::Hide => match rows.get(&key) {
                Some(row)
                    if matches!(
                        row.source,
                        CatalogSource::Bundled
                            | CatalogSource::CodewhaleBundled { .. }
                            | CatalogSource::ModelsDevLive { .. }
                    ) =>
                {
                    rows.remove(&key);
                }
                Some(_) => skipped.push(SkippedPatch {
                    provider: patch.provider.clone(),
                    id: patch.id.clone(),
                    reason: "hide ignored: row comes from a higher layer".into(),
                }),
                None => skipped.push(SkippedPatch {
                    provider: patch.provider.clone(),
                    id: patch.id.clone(),
                    reason: "hide ignored: no such row".into(),
                }),
            },
            ModelOp::Deprecate => match rows.get_mut(&key) {
                Some(row) => {
                    annotate(row, patch, "deprecated");
                }
                None => skipped.push(SkippedPatch {
                    provider: patch.provider.clone(),
                    id: patch.id.clone(),
                    reason: "deprecate ignored: no such row".into(),
                }),
            },
            ModelOp::Upsert => {
                if let Some(row) = rows.get_mut(&key) {
                    // A capability patch must not refresh or relabel inherited prices.
                    let inherited_price_source = row.pricing_source().clone();
                    // Patches never restate modalities, so their authority
                    // stays with the layer that did.
                    row.modalities_source = Some(row.modalities_source().clone());
                    patch_fields(row, patch);
                    row.cost_source = Some(if patch.pricing.is_some() {
                        source.clone()
                    } else {
                        inherited_price_source
                    });
                    row.source = source.clone();
                } else if patch.context_window.is_some() || patch.allow_unlisted {
                    // A row materializes when the payload says enough to be
                    // worth a row: a context window, or an explicit unlisted
                    // assertion that this id exists. An id-only attested row is
                    // deliberately bare — every limit, price and capability
                    // stays `None` (unknown) rather than being borrowed from a
                    // sibling model or a lower stale layer.
                    let mut row = CatalogOffering {
                        provider: patch.provider.clone(),
                        wire_model_id: patch.id.clone(),
                        endpoint_key: "chat".to_string(),
                        source: source.clone(),
                        ..CatalogOffering::default()
                    };
                    patch_fields(&mut row, patch);
                    if patch.pricing.is_some() {
                        row.cost_source = Some(source.clone());
                    }
                    rows.insert(key, row);
                } else {
                    skipped.push(SkippedPatch {
                        provider: patch.provider.clone(),
                        id: patch.id.clone(),
                        reason: "upsert ignored: new row needs context_window or allow_unlisted"
                            .into(),
                    });
                }
            }
        }
    }
    skipped
}

/// Does this payload explicitly assert `(provider, id)` exists even when the
/// provider's own roster omits it?
///
/// This is the *only* thing that may override a roster's omission. The client
/// keeps no history of past rosters, so it cannot tell a never-listed preview
/// from a retired model by itself — and must not guess. Absent the assertion,
/// the roster stays authoritative for every id it does and does not list.
///
/// `facts` must be a scoped view ([`super::scope::scoped_view`]), which is
/// where the assertion is restricted to an `Upsert` in a payload that expires.
#[must_use]
pub fn is_unlisted_attested(facts: &ScopedFacts, provider: &str, id: &str) -> bool {
    facts.models.iter().any(|patch| {
        patch.allow_unlisted
            && patch.op == ModelOp::Upsert
            && patch.provider == provider
            && patch.id == id
    })
}

/// Fill fields a provider-owned live row does not state with signed values.
///
/// A `/v1/models` roster that answers with ids alone has not said "context and
/// reasoning support are unknown" — it has said nothing about them. Layer
/// precedence still holds where the two disagree: this only writes fields the
/// provider row leaves `None`, and it never changes the row's own `source`,
/// which stays provider-live. Returns whether anything was filled.
///
/// Deliberately excluded:
/// - Rows from any other layer. Bundled/Models.dev rows are patched by
///   [`apply_model_patches`]; config and user rows are the user's authority.
/// - Pricing. A filled price would have to carry a `CloudFacts` price source on
///   a provider-live row, and `fresh_dispatch_pricing_quote_at` admits a cloud
///   quote only when the whole row is `CloudFacts` — so the price would render
///   without being billable. Cloud prices therefore keep applying only where no
///   fresh roster owns the row.
/// - Capabilities the payload has no field for. Nothing is inferred.
pub fn complete_provider_live_row(row: &mut CatalogOffering, facts: &ScopedFacts) -> bool {
    if !matches!(row.source, CatalogSource::Live { .. })
        || !facts.is_current_at(crate::catalog::now_unix())
    {
        return false;
    }
    let Some(patch) = facts.models.iter().find(|patch| {
        patch.op == ModelOp::Upsert
            && patch.provider == row.provider
            && patch.id == row.wire_model_id
    }) else {
        return false;
    };
    let mut filled = false;
    let mut limit = row.limit.clone().unwrap_or_default();
    if limit.context.is_none()
        && let Some(context) = patch.context_window
    {
        limit.context = Some(context);
        filled = true;
    }
    if limit.output.is_none()
        && let Some(output) = patch.max_output
    {
        limit.output = Some(output);
        filled = true;
    }
    if filled {
        row.limit = Some(limit);
    }
    if row.reasoning.is_none()
        && let Some(reasoning) = patch.reasoning
    {
        row.reasoning = Some(reasoning);
        filled = true;
    }
    filled
}

fn patch_fields(row: &mut CatalogOffering, patch: &ModelFact) {
    if patch.context_window.is_some() || patch.max_output.is_some() {
        let mut limit = row.limit.clone().unwrap_or_default();
        if let Some(context) = patch.context_window {
            limit.context = Some(context);
        }
        if let Some(output) = patch.max_output {
            limit.output = Some(output);
        }
        row.limit = Some(limit);
    }
    if let Some(pricing) = &patch.pricing {
        // A price block has one authority. Missing classes stay unknown instead
        // of silently mixing an old row's prices with newly signed rates.
        row.cost = Some(ModelsDevCost {
            input: pricing.input_per_m,
            output: pricing.output_per_m,
            cache_read: pricing.cache_read_per_m,
            cache_write: None,
        });
    }
    if patch.reasoning.is_some() {
        row.reasoning = patch.reasoning;
    }
    if patch.display_name.is_some() || patch.note.is_some() {
        annotate(row, patch, "upsert");
    }
}

fn annotate(row: &mut CatalogOffering, patch: &ModelFact, kind: &str) {
    row.reasoning_options
        .retain(|value| value.get("cloud_facts").is_none());
    row.reasoning_options.push(json!({
        "cloud_facts": {
            "op": kind,
            "display_name": patch.display_name,
            "deprecated_at": patch.deprecated_at,
            "replacement": patch.replacement,
            "note": patch.note,
        }
    }));
}
