//! Scope a verified payload to the running binary: per-item `applies_to`,
//! announcement windows, and the provider `base_url` allowlist all live here so
//! consumers see one already-filtered view.

use std::collections::BTreeMap;

use super::types::{
    Announcement, MAX_ANNOUNCEMENT_CHARS, ModelFact, ModelOp, ProviderDefaultFact, ReleaseFact,
};
use super::verify::{VerifiedFacts, item_applies, parse_rfc3339_utc};
use crate::provider_kind::ProviderKind;

/// A verified payload filtered to what applies to this binary right now.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ScopedFacts {
    pub channel: String,
    pub facts_version: u64,
    pub key_id: String,
    pub sha256: String,
    pub published_at: String,
    pub stale: bool,
    /// Signed expiry plus the verification grace window; never extended by 304.
    pub valid_until: Option<u64>,
    pub models: Vec<ModelFact>,
    pub provider_defaults: BTreeMap<String, ProviderDefaultFact>,
    pub release: Option<ReleaseFact>,
    pub announcements: Vec<Announcement>,
    /// Human-readable receipts for every item that was dropped and why.
    pub dropped: Vec<String>,
}

impl ScopedFacts {
    #[must_use]
    pub fn is_current_at(&self, now: u64) -> bool {
        !self.stale && self.valid_until.is_none_or(|expires| now <= expires)
    }

    /// Count of applied patch/default/announcement items, for `/status`.
    #[must_use]
    pub fn item_counts(&self) -> (usize, usize, usize) {
        (
            self.models.len(),
            self.provider_defaults.len(),
            self.announcements.len(),
        )
    }
}

fn cloud_provider(provider: &str) -> bool {
    // Catalog aliases collapse wire variants; signed route facts retain the
    // exact canonical config identity and its distinct endpoint contract.
    ProviderKind::parse_config_identity(provider).is_some_and(|kind| {
        kind.as_str() == provider
            && !matches!(
                kind,
                ProviderKind::Custom
                    | ProviderKind::Ollama
                    | ProviderKind::Sglang
                    | ProviderKind::Vllm
                    | ProviderKind::OpenaiCodex
                    | ProviderKind::Antigravity
            )
    })
}

fn model_id_valid(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 256
        && id.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'/' | b'-')
        })
}

/// Accept only an existing official HTTPS endpoint contract. Signing a fact
/// cannot grant a new host, neighboring API path, userinfo or credential scope.
#[must_use]
pub fn base_url_allowed(provider: &str, candidate: &str) -> bool {
    if !cloud_provider(provider) {
        return false;
    }
    let Some(kind) = ProviderKind::parse_config_identity(provider) else {
        return false;
    };
    let normalized = candidate.trim().trim_end_matches('/').to_ascii_lowercase();
    if !normalized.starts_with("https://")
        || normalized
            .chars()
            .any(|ch| ch.is_whitespace() || ch.is_control() || matches!(ch, '?' | '#' | '@' | '\\'))
    {
        return false;
    }
    match kind {
        ProviderKind::Codewhale => normalized == crate::DEFAULT_CODEWHALE_BASE_URL,
        ProviderKind::Moonshot => matches!(
            normalized.as_str(),
            crate::DEFAULT_MOONSHOT_BASE_URL
                | crate::MOONSHOT_CN_BASE_URL
                | "https://api.kimi.com/coding"
                | "https://api.kimi.com/coding/v1"
        ),
        _ => crate::provider_base_url_is_official(kind, &normalized),
    }
}

/// Build the scoped view for `current` at `now_unix`.
#[must_use]
pub fn scoped_view(
    verified: &VerifiedFacts,
    current: &semver::Version,
    now_unix: u64,
) -> ScopedFacts {
    let facts = &verified.facts;
    let mut out = ScopedFacts {
        channel: facts.channel.clone(),
        facts_version: facts.facts_version,
        key_id: verified.key_id.clone(),
        sha256: verified.sha256.clone(),
        published_at: facts.published_at.clone(),
        stale: verified.stale,
        valid_until: facts
            .not_after
            .as_deref()
            .and_then(parse_rfc3339_utc)
            .map(|expires| expires.saturating_add(super::verify::NOT_AFTER_GRACE_SECS)),
        ..ScopedFacts::default()
    };

    for model in &facts.models {
        if !cloud_provider(&model.provider)
            || !model_id_valid(&model.id)
            || model.context_window == Some(0)
            || model.max_output == Some(0)
        {
            out.dropped
                .push("model patch has unsupported provider, identity, or limits".into());
            continue;
        }
        if !item_applies(model.applies_to.as_deref(), current) {
            out.dropped.push(format!(
                "model {}/{}: applies_to {:?} does not match",
                model.provider,
                model.id,
                model.applies_to.as_deref().unwrap_or("")
            ));
            continue;
        }
        let mut kept = model.clone();
        // An unlisted assertion overrides a provider roster's own omission, so
        // it must be an `Upsert` and it must expire: without `not_after` the
        // claim would outlive any ability to withdraw it by publishing. The
        // rest of the patch still applies; only the assertion is discarded.
        if kept.allow_unlisted && (kept.op != ModelOp::Upsert || out.valid_until.is_none()) {
            kept.allow_unlisted = false;
            out.dropped.push(format!(
                "model {}/{}: allow_unlisted needs an upsert in a payload with not_after",
                model.provider, model.id
            ));
        }
        out.models.push(kept);
    }

    for (provider, fact) in &facts.provider_defaults {
        if !cloud_provider(provider) {
            out.dropped.push("unsupported provider default".into());
            continue;
        }
        if !item_applies(fact.applies_to.as_deref(), current) {
            out.dropped.push(format!(
                "provider_defaults.{provider}: applies_to {:?} does not match",
                fact.applies_to.as_deref().unwrap_or("")
            ));
            continue;
        }
        let mut kept = ProviderDefaultFact {
            default_model: fact
                .default_model
                .as_deref()
                .map(str::trim)
                .filter(|m| {
                    model_id_valid(m)
                        && !m.eq_ignore_ascii_case("auto")
                        && !m.eq_ignore_ascii_case("unknown")
                })
                .map(str::to_string),
            base_url: None,
            applies_to: None,
        };
        if let Some(url) = fact.base_url.as_deref().map(str::trim) {
            if base_url_allowed(provider, url) {
                kept.base_url = Some(url.to_string());
            } else {
                out.dropped.push(format!(
                    "provider_defaults.{provider}.base_url: outside the official HTTPS endpoint contract"
                ));
            }
        }
        if kept.default_model.is_none() && kept.base_url.is_none() {
            continue;
        }
        out.provider_defaults.insert(provider.clone(), kept);
    }

    if let Some(release) = &facts.release {
        if item_applies(release.applies_to.as_deref(), current) {
            out.release = Some(release.clone());
        } else {
            out.dropped.push(format!(
                "release: applies_to {:?} does not match",
                release.applies_to.as_deref().unwrap_or("")
            ));
        }
    }

    for announcement in &facts.announcements {
        let id = announcement.id.trim();
        if id.is_empty() || announcement.text.trim().is_empty() {
            out.dropped.push("announcement without id/text".into());
            continue;
        }
        if announcement.text.chars().count() > MAX_ANNOUNCEMENT_CHARS {
            out.dropped
                .push(format!("announcement {id}: text too long"));
            continue;
        }
        if !item_applies(announcement.applies_to.as_deref(), current) {
            out.dropped
                .push(format!("announcement {id}: applies_to does not match"));
            continue;
        }
        if [
            announcement.starts_at.as_deref(),
            announcement.expires_at.as_deref(),
        ]
        .into_iter()
        .flatten()
        .any(|value| parse_rfc3339_utc(value).is_none())
        {
            out.dropped
                .push(format!("announcement {id}: invalid time window"));
            continue;
        }
        if let Some(starts) = announcement
            .starts_at
            .as_deref()
            .and_then(parse_rfc3339_utc)
            && now_unix < starts
        {
            out.dropped.push(format!("announcement {id}: not started"));
            continue;
        }
        if let Some(expires) = announcement
            .expires_at
            .as_deref()
            .and_then(parse_rfc3339_utc)
            && now_unix >= expires
        {
            out.dropped.push(format!("announcement {id}: expired"));
            continue;
        }
        out.announcements.push(announcement.clone());
    }

    out
}
