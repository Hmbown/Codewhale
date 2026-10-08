//! Persisted, operator-declared input to the existing catalog. These records
//! describe one exact route; they never create credentials or model aliases.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Deserializer, Serialize};

use super::{CatalogOffering, CatalogSource, base_url_fingerprint};
use crate::models_dev::{ModelsDevCost, ModelsDevLimit, ModelsDevModalities};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfiguredModel {
    pub provider: String,
    pub base_url: String,
    /// Exact, case-sensitive wire identity. The label is never sent instead.
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "deserialize_limit")]
    pub limit: Option<ModelsDevLimit>,
    /// USD per million tokens, using the same shape as the catalog.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "deserialize_cost")]
    pub cost: Option<ModelsDevCost>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "deserialize_modalities")]
    pub modalities: Option<ModelsDevModalities>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachment: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_output: Option<bool>,
    /// Keep future metadata intact through typed config saves.
    #[serde(flatten)]
    pub extras: BTreeMap<String, toml::Value>,
}

impl ConfiguredModel {
    pub fn matches_route(&self, provider: &str, base_url: &str) -> bool {
        !base_url.contains(['@', '?', '#'])
            && self.provider == provider
            && base_url_fingerprint(&self.base_url) == base_url_fingerprint(base_url)
    }

    /// Unknown fields stay unknown; no sibling, alias, or provider fact is
    /// inherited. Source is assigned here and cannot be supplied by the file.
    pub fn to_catalog_offering(&self) -> CatalogOffering {
        CatalogOffering {
            provider: self.provider.clone(),
            wire_model_id: self.id.clone(),
            endpoint_key: "chat".into(),
            limit: self.limit.clone(),
            cost: self.cost.clone(),
            modalities: self.modalities.clone(),
            attachment: self.attachment,
            reasoning: self.reasoning,
            tool_call: self.tool_call,
            structured_output: self.structured_output,
            source: CatalogSource::ConfigOverride,
            ..CatalogOffering::default()
        }
    }
}

pub fn validate_configured_models(models: &[ConfiguredModel]) -> anyhow::Result<()> {
    let mut identities = BTreeSet::new();
    for (index, model) in models.iter().enumerate() {
        // Error messages name fields, never echo untrusted values or URLs.
        let invalid = |field| anyhow::anyhow!("custom_models[{index}].{field} is invalid");
        for (field, value) in [("provider", &model.provider), ("id", &model.id)] {
            if value.is_empty()
                || value.len() > 256
                || value.chars().any(char::is_whitespace)
                || value.chars().any(char::is_control)
            {
                return Err(invalid(field));
            }
        }
        if model.id.eq_ignore_ascii_case("auto") {
            return Err(invalid("id"));
        }
        let Some((scheme, rest)) = model.base_url.split_once("://") else {
            return Err(invalid("base_url"));
        };
        if !matches!(scheme.to_ascii_lowercase().as_str(), "http" | "https")
            || rest.split('/').next().is_none_or(str::is_empty)
            || model.base_url.contains(['@', '?', '#'])
            || model
                .base_url
                .chars()
                .any(|c| c.is_whitespace() || c.is_control())
        {
            return Err(invalid("base_url"));
        }
        if model
            .display_name
            .as_ref()
            .is_some_and(|name| name.trim().is_empty() || name.chars().any(char::is_control))
        {
            return Err(invalid("display_name"));
        }
        if let Some(limit) = &model.limit
            && ([limit.context, limit.input, limit.output]
                .into_iter()
                .flatten()
                .any(|value| value == 0 || value > u64::from(u32::MAX))
                || limit.context.is_some_and(|context| {
                    limit.input.is_some_and(|input| input > context)
                        || limit.output.is_some_and(|output| output > context)
                }))
        {
            return Err(invalid("limit"));
        }
        if model
            .cost
            .as_ref()
            .is_some_and(|cost| !crate::pricing::catalog_cost_is_valid(cost))
        {
            return Err(invalid("cost"));
        }
        if model.extras.keys().any(|key| {
            matches!(
                key.as_str(),
                "source"
                    | "canonical_model"
                    | "aliases"
                    | "api_key"
                    | "auth"
                    | "headers"
                    | "endpoint_key"
                    | "default_for_provider"
            )
        }) {
            return Err(invalid("metadata authority"));
        }
        if !identities.insert((
            model.provider.clone(),
            base_url_fingerprint(&model.base_url),
            model.id.clone(),
        )) {
            return Err(invalid("duplicate route"));
        }
    }
    Ok(())
}

pub fn deserialize_configured_models<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<ConfiguredModel>>, D::Error>
where
    D: Deserializer<'de>,
{
    let models = Option::<Vec<ConfiguredModel>>::deserialize(deserializer)?;
    validate_configured_models(models.as_deref().unwrap_or_default())
        .map_err(serde::de::Error::custom)?;
    Ok(models)
}

// Nested units and limits have precise meanings. Reject unrecognized keys
// instead of silently dropping a currency, tier, or other pricing condition.
fn deserialize_known<'de, D: Deserializer<'de>, T: serde::de::DeserializeOwned>(
    d: D,
    fields: &[&str],
) -> Result<Option<T>, D::Error> {
    let value = Option::<toml::Value>::deserialize(d)?;
    value
        .map(|value| {
            if value
                .as_table()
                .is_none_or(|table| table.keys().any(|key| !fields.contains(&key.as_str())))
            {
                return Err(serde::de::Error::custom(
                    "unsupported nested custom model metadata field",
                ));
            }
            value
                .try_into()
                .map_err(|_| serde::de::Error::custom("invalid custom model metadata"))
        })
        .transpose()
}
fn deserialize_limit<'de, D: Deserializer<'de>>(d: D) -> Result<Option<ModelsDevLimit>, D::Error> {
    deserialize_known(d, &["context", "input", "output"])
}
fn deserialize_cost<'de, D: Deserializer<'de>>(d: D) -> Result<Option<ModelsDevCost>, D::Error> {
    deserialize_known(d, &["input", "output", "cache_read", "cache_write"])
}
fn deserialize_modalities<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<Option<ModelsDevModalities>, D::Error> {
    deserialize_known(d, &["input", "output"])
}
