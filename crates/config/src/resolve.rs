//! Ranked settings resolution with provenance.
//!
//! One rank table for every setting: the highest-ranked layer that names a
//! scalar wins outright (scalars replace, never merge); map layers
//! deep-merge with the highest rank winning per leaf; secrets travel by
//! reference only — a [`SecretRef`] names the source and id, never the
//! value, so a resolved row can say *which* secret won without carrying it.
//!
//! Layers with no producer today (managed policy files, project settings)
//! simply contribute no entries; the rank is the contract, and an empty
//! layer abstains rather than vetoing.

use std::collections::BTreeMap;

/// Where a setting value came from, highest rank first.
///
/// The order is the product rule: policy beats the CLI flag, the flag beats
/// a session override, overrides beat files, and files beat the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Layer {
    /// Organization / managed policy. No file producer yet.
    ManagedPolicy,
    /// Per-run `-c key=value` CLI flag (lands with the config CLI slice).
    CliFlag,
    /// Session override (`/set`, `/config <key> <value>` without `--save`).
    SessionOverride,
    /// Project settings. No file producer yet.
    ProjectConfig,
    /// `settings.toml` (user config).
    UserConfig,
    /// The schema default.
    Default,
}

impl Layer {
    /// Stable wire name for receipts and row labels.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Layer::ManagedPolicy => "policy",
            Layer::CliFlag => "cli",
            Layer::SessionOverride => "session",
            Layer::ProjectConfig => "project",
            Layer::UserConfig => "user",
            Layer::Default => "default",
        }
    }
}

/// A secret by reference: the layer that resolved it plus where to find it.
/// The value itself never enters the resolver, a row, or a receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretRef {
    /// Which layer named this secret.
    pub layer: Layer,
    /// Where it lives: `env:DASHSCOPE_API_KEY`, `keychain:codewhale/...`, …
    pub reference: String,
}

impl SecretRef {
    #[must_use]
    pub fn new(layer: Layer, reference: impl Into<String>) -> Self {
        Self {
            layer,
            reference: reference.into(),
        }
    }
}

/// The winning layer for one scalar key, with every layer that named it.
/// Empty input means the schema default wins unopposed.
#[must_use]
pub fn resolve_scalar<'a>(
    candidates: impl IntoIterator<Item = (Layer, &'a str)>,
) -> (Layer, Vec<Layer>) {
    let mut best: Option<Layer> = None;
    let mut named: Vec<Layer> = Vec::new();
    for (layer, _) in candidates {
        if !named.contains(&layer) {
            named.push(layer);
        }
        if best.is_none_or(|top| layer < top) {
            best = Some(layer);
        }
    }
    named.sort();
    (best.unwrap_or(Layer::Default), named)
}

/// Deep-merge map layers: every key present anywhere survives, and per key
/// the highest-ranked layer's value wins. Lower layers contribute only the
/// keys nobody above them named.
#[must_use]
pub fn merge_maps(layers: &BTreeMap<Layer, BTreeMap<String, String>>) -> BTreeMap<String, String> {
    let mut merged = BTreeMap::new();
    let mut order: Vec<Layer> = layers.keys().copied().collect();
    order.sort();
    for layer in order {
        if let Some(map) = layers.get(&layer) {
            for (key, value) in map {
                merged.entry(key.clone()).or_insert_with(|| value.clone());
            }
        }
    }
    merged
}

/// Which layer supplied each merged key. Same walk as [`merge_maps`], kept
/// as a separate pass so a row can name the winner per key.
#[must_use]
pub fn merge_provenance(
    layers: &BTreeMap<Layer, BTreeMap<String, String>>,
) -> BTreeMap<String, Layer> {
    let mut provenance = BTreeMap::new();
    let mut order: Vec<Layer> = layers.keys().copied().collect();
    order.sort();
    for layer in order {
        if let Some(map) = layers.get(&layer) {
            for key in map.keys() {
                provenance.entry(key.clone()).or_insert(layer);
            }
        }
    }
    provenance
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rank_order_is_policy_over_cli_over_session_over_files_over_default() {
        let layers = [
            Layer::ManagedPolicy,
            Layer::CliFlag,
            Layer::SessionOverride,
            Layer::ProjectConfig,
            Layer::UserConfig,
            Layer::Default,
        ];
        let mut sorted = layers;
        sorted.sort();
        assert_eq!(sorted, layers);
    }

    #[test]
    fn scalar_resolution_picks_the_highest_ranked_layer() {
        let (winner, named) = resolve_scalar([
            (Layer::Default, "bottom"),
            (Layer::UserConfig, "user"),
            (Layer::SessionOverride, "session"),
        ]);
        assert_eq!(winner, Layer::SessionOverride);
        assert_eq!(
            named,
            vec![Layer::SessionOverride, Layer::UserConfig, Layer::Default]
        );
    }

    #[test]
    fn scalar_resolution_with_no_candidates_falls_back_to_default() {
        let (winner, named) = resolve_scalar([]);
        assert_eq!(winner, Layer::Default);
        assert!(named.is_empty());
    }

    #[test]
    fn scalar_values_replace_never_merge() {
        // The loser's text is evidence only; the winner's value is untouched.
        let (winner, _) = resolve_scalar([(Layer::UserConfig, "user"), (Layer::CliFlag, "cli")]);
        assert_eq!(winner, Layer::CliFlag);
    }

    #[test]
    fn map_merge_keeps_every_key_with_highest_rank_winning_per_key() {
        let layers = BTreeMap::from([
            (
                Layer::UserConfig,
                BTreeMap::from([
                    ("a".to_string(), "user-a".to_string()),
                    ("b".to_string(), "user-b".to_string()),
                ]),
            ),
            (
                Layer::ProjectConfig,
                BTreeMap::from([("b".to_string(), "project-b".to_string())]),
            ),
        ]);
        let merged = merge_maps(&layers);
        assert_eq!(merged.get("a").map(String::as_str), Some("user-a"));
        assert_eq!(merged.get("b").map(String::as_str), Some("project-b"));
        let provenance = merge_provenance(&layers);
        assert_eq!(provenance.get("a"), Some(&Layer::UserConfig));
        assert_eq!(provenance.get("b"), Some(&Layer::ProjectConfig));
    }

    #[test]
    fn secret_refs_carry_no_value() {
        let secret = SecretRef::new(Layer::UserConfig, "env:DASHSCOPE_API_KEY");
        let debug = format!("{secret:?}");
        assert!(debug.contains("env:DASHSCOPE_API_KEY"));
        assert_eq!(secret.layer, Layer::UserConfig);
    }

    #[test]
    fn layer_names_are_stable() {
        assert_eq!(Layer::ManagedPolicy.name(), "policy");
        assert_eq!(Layer::CliFlag.name(), "cli");
        assert_eq!(Layer::SessionOverride.name(), "session");
        assert_eq!(Layer::ProjectConfig.name(), "project");
        assert_eq!(Layer::UserConfig.name(), "user");
        assert_eq!(Layer::Default.name(), "default");
    }
}
