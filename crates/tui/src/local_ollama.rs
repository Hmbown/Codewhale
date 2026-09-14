//! First-run / missing-key adoption of a live local Ollama catalog.
//!
//! Virgin sessions default to the DeepSeek costume (`deepseek-flash`). When a
//! real local daemon answers `GET /api/tags` (or the OpenAI-compat
//! `GET /v1/models` roster), the painted route must switch to a tag that
//! actually exists — never leave DeepSeek flash as the chrome while a live
//! local catalog is sitting on `:11434`.

use std::time::Duration;

use codewhale_config::catalog::{
    CatalogOffering, CatalogSource, ProviderCatalogDelta, base_url_fingerprint, now_unix,
};
use serde::Deserialize;

use crate::config::{ApiProvider, Config, DEFAULT_OLLAMA_BASE_URL};

const TAGS_PROBE_TIMEOUT: Duration = Duration::from_secs(2);

/// Result of a successful local Ollama tags/models probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LiveLocalOllamaCatalog {
    pub(crate) endpoint_v1: String,
    pub(crate) tags: Vec<String>,
}

impl LiveLocalOllamaCatalog {
    /// Prefer the alphabetically first live tag (matches route_runtime's
    /// Ollama default when tags have no `default_for_provider` flag).
    pub(crate) fn preferred_tag(&self) -> Option<&str> {
        self.tags.first().map(String::as_str)
    }
}

/// True when this session should adopt a live local catalog into chrome.
///
/// First-run and missing-key recovery paint DeepSeek by default; a live local
/// roster must replace that costume. An already-keyed hosted route is left alone.
#[must_use]
pub(crate) fn should_adopt_live_local_ollama(app: &crate::tui::app::App) -> bool {
    if app.api_provider == ApiProvider::Ollama {
        // Already on Ollama — route_runtime + #5795 own the tag; don't fight it.
        return false;
    }
    app.onboarding_needs_api_key || app.onboarding_missing_key_recovery
}

/// Resolve the OpenAI-compat Ollama base URL (`…/v1`) from config defaults.
pub(crate) fn ollama_v1_base_url(config: &Config) -> String {
    config
        .provider_config_for(ApiProvider::Ollama)
        .and_then(|entry| entry.base_url.clone())
        .filter(|url| !url.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_OLLAMA_BASE_URL.to_string())
}

/// Strip a trailing `/v1` (with optional slash) so we can hit native `/api/tags`.
pub(crate) fn ollama_native_origin(v1_base: &str) -> String {
    let trimmed = v1_base.trim().trim_end_matches('/');
    if let Some(origin) = trimmed.strip_suffix("/v1") {
        origin.to_string()
    } else {
        trimmed.to_string()
    }
}

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    #[serde(default)]
    models: Vec<OllamaTagModel>,
}

#[derive(Debug, Deserialize)]
struct OllamaTagModel {
    #[serde(default)]
    name: String,
    #[serde(default)]
    model: String,
}

/// Parse native Ollama `GET /api/tags` JSON into sorted unique tag ids.
pub(crate) fn parse_ollama_tags_response(payload: &str) -> anyhow::Result<Vec<String>> {
    let parsed: OllamaTagsResponse = serde_json::from_str(payload)
        .map_err(|err| anyhow::anyhow!("Failed to parse Ollama /api/tags JSON: {err}"))?;
    let mut tags: Vec<String> = parsed
        .models
        .into_iter()
        .filter_map(|row| {
            let name = row.name.trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
            let model = row.model.trim();
            if !model.is_empty() {
                Some(model.to_string())
            } else {
                None
            }
        })
        .collect();
    tags.sort();
    tags.dedup();
    Ok(tags)
}

fn record_ollama_tags_into_lake(endpoint_v1: &str, tags: &[String]) {
    if tags.is_empty() {
        return;
    }
    let fingerprint = base_url_fingerprint(endpoint_v1);
    let fetched_at = now_unix();
    let offerings = tags
        .iter()
        .map(|tag| CatalogOffering {
            provider: "ollama".into(),
            wire_model_id: tag.clone(),
            endpoint_key: "chat".into(),
            source: CatalogSource::Live {
                base_url_fingerprint: fingerprint.clone(),
                fetched_at,
            },
            default_for_provider: false,
            ..Default::default()
        })
        .collect();
    let ticket = crate::provider_catalog_live::begin_refresh_for_identity(
        ApiProvider::Ollama,
        "ollama",
        endpoint_v1,
    );
    let _ = crate::provider_catalog_live::record_success_if_current(
        &ticket,
        ProviderCatalogDelta {
            provider: "ollama".into(),
            base_url_fingerprint: fingerprint,
            fetched_at,
            offerings,
        },
    );
}

async fn fetch_text(url: &str) -> anyhow::Result<String> {
    // The first-run probe can run before any provider client has installed
    // the rustls crypto provider; the shared builder installs it (the bare
    // `reqwest::Client::builder()` panics under `rustls-no-provider`).
    let client = crate::tls::reqwest_client_builder()
        .timeout(TAGS_PROBE_TIMEOUT)
        .build()?;
    let response = client.get(url).send().await?;
    if !response.status().is_success() {
        anyhow::bail!("HTTP {}", response.status());
    }
    Ok(response.text().await?)
}

/// Probe local Ollama for a live catalog. Prefers native `/api/tags`, falls
/// back to OpenAI-compat `/v1/models`. Returns `None` when nothing useful
/// answered — never invents a tag.
pub(crate) async fn probe_live_local_ollama_catalog(
    config: &Config,
) -> Option<LiveLocalOllamaCatalog> {
    let endpoint_v1 = ollama_v1_base_url(config);
    let origin = ollama_native_origin(&endpoint_v1);
    let tags_url = format!("{origin}/api/tags");

    let tags = match fetch_text(&tags_url).await {
        Ok(body) => match parse_ollama_tags_response(&body) {
            Ok(tags) if !tags.is_empty() => tags,
            Ok(_) => return None,
            Err(err) => {
                tracing::debug!(
                    target: "local_ollama",
                    error = %err,
                    "GET /api/tags returned unusable body"
                );
                Vec::new()
            }
        },
        Err(err) => {
            tracing::debug!(
                target: "local_ollama",
                error = %err,
                url = %tags_url,
                "GET /api/tags probe failed"
            );
            Vec::new()
        }
    };

    let tags = if tags.is_empty() {
        // Fallback: OpenAI-compat roster (same tags, different shape).
        let models_url = format!("{}/models", endpoint_v1.trim_end_matches('/'));
        match fetch_text(&models_url).await {
            Ok(body) => match crate::client::parse_models_response(&body) {
                Ok(models) if !models.is_empty() => models.into_iter().map(|m| m.id).collect(),
                _ => return None,
            },
            Err(_) => return None,
        }
    } else {
        tags
    };

    record_ollama_tags_into_lake(&endpoint_v1, &tags);
    Some(LiveLocalOllamaCatalog { endpoint_v1, tags })
}

/// Background probe used by the event loop (mirrors `spawn_startup_version_check`).
pub(crate) fn spawn_local_ollama_adoption_probe(
    config: &Config,
    should_probe: bool,
) -> Option<tokio::task::JoinHandle<Option<LiveLocalOllamaCatalog>>> {
    if !should_probe {
        return None;
    }
    #[cfg(test)]
    {
        let _ = config;
        None
    }
    #[cfg(not(test))]
    {
        let config = config.clone();
        Some(tokio::spawn(async move {
            probe_live_local_ollama_catalog(&config).await
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{EnvVarGuard, lock_test_env};

    #[test]
    fn parse_ollama_tags_response_reads_name_field() {
        let body = r#"{"models":[{"name":"qwen2.5:0.5b","model":"qwen2.5:0.5b","size":0}]}"#;
        let tags = parse_ollama_tags_response(body).expect("parse");
        assert_eq!(tags, vec!["qwen2.5:0.5b".to_string()]);
    }

    #[test]
    fn parse_ollama_tags_response_sorts_and_dedups() {
        let body = r#"{"models":[
            {"name":"zeta:tag"},
            {"name":"alpha:tag"},
            {"name":"alpha:tag"}
        ]}"#;
        let tags = parse_ollama_tags_response(body).expect("parse");
        assert_eq!(tags, vec!["alpha:tag".to_string(), "zeta:tag".to_string()]);
    }

    #[test]
    fn ollama_native_origin_strips_v1() {
        assert_eq!(
            ollama_native_origin("http://localhost:11434/v1"),
            "http://localhost:11434"
        );
        assert_eq!(
            ollama_native_origin("http://127.0.0.1:11434/v1/"),
            "http://127.0.0.1:11434"
        );
    }

    #[test]
    fn preferred_tag_is_alphabetically_first_after_sort() {
        let mut tags = vec!["zeta:tag".into(), "alpha:tag".into()];
        tags.sort();
        let catalog = LiveLocalOllamaCatalog {
            endpoint_v1: "http://localhost:11434/v1".into(),
            tags,
        };
        assert_eq!(catalog.preferred_tag(), Some("alpha:tag"));
    }

    #[tokio::test]
    async fn probe_live_local_ollama_catalog_reads_api_tags() {
        let _lock = lock_test_env();
        let _live = crate::provider_lake::lock_live_snapshot();
        let home = tempfile::tempdir().unwrap();
        let _home = EnvVarGuard::set("CODEWHALE_HOME", home.path());
        crate::provider_catalog_live::reset_cache_for_test();
        crate::provider_lake::clear_live_snapshot();

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            use std::io::{Read, Write};
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let body = br#"{"models":[{"name":"qwen2.5:0.5b","model":"qwen2.5:0.5b"}]}"#;
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(header.as_bytes()).unwrap();
            stream.write_all(body).unwrap();
        });

        let endpoint = format!("http://{addr}/v1");
        let mut config = Config::default();
        config.provider_config_for_mut(ApiProvider::Ollama).base_url = Some(endpoint.clone());

        let catalog = probe_live_local_ollama_catalog(&config)
            .await
            .expect("tags probe should succeed");
        assert_eq!(catalog.endpoint_v1, endpoint);
        assert_eq!(catalog.tags, vec!["qwen2.5:0.5b".to_string()]);
        assert_eq!(catalog.preferred_tag(), Some("qwen2.5:0.5b"));
    }
}
