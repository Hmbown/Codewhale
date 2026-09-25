//! Provider/model inventory for routing policy.
//!
//! This is the high-level "what can this user actually run?" object. Auto
//! routing, fleet workers, and sub-agent policy should consume this shape
//! instead of guessing model strings from global defaults.

use serde::Serialize;

use crate::client::system_one::DecisionRouterRoute;
use crate::config::{
    ApiProvider, AutoRouterKind, Config, has_api_key_for, normalize_model_name_for_provider,
    provider_capability,
};
use crate::provider_lake::models_for_provider;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ModelAuthSource {
    Config,
    Env,
    OAuthCli,
    ImportedToken,
    NoAuth,
    KeylessLocal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ModelRouteCandidate {
    pub(crate) provider: ApiProvider,
    pub(crate) provider_name: &'static str,
    pub(crate) provider_display_name: &'static str,
    pub(crate) model: String,
    /// Explicit declarations keep case-sensitive wire identity; bundled aliases
    /// retain the existing case-insensitive convenience lookup.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub(crate) user_declared: bool,
    pub(crate) context_window: u32,
    /// The context window came from the legacy capability fallback (an `_Nk`
    /// name-suffix parse or a vendor-family heuristic), not a route fact
    /// (#5441). Serialized only when true so existing payloads stay stable.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub(crate) context_window_unverified: bool,
    /// Known output ceiling, or `None` when this route publishes none. The
    /// classifier is told "unknown" rather than a fabricated number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) max_output: Option<u32>,
    pub(crate) thinking_supported: bool,
    pub(crate) cache_telemetry_supported: bool,
    pub(crate) auth_source: ModelAuthSource,
    pub(crate) readiness: crate::provider_readiness::ResolvedProviderReadiness,
    pub(crate) default_for_provider: bool,
    pub(crate) tags: Vec<&'static str>,
}

/// Why a declared `[auto.router]` cannot run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AutoRouterSetupIssue {
    /// `kind` is neither `"chat"` nor `"decision"`.
    UnknownKind,
    /// `kind = "decision"` names a provider that serves no decision API.
    UnsupportedDecisionProvider,
    /// `provider` or `model` is missing (or the chat provider is unknown).
    Incomplete,
    /// The route is complete but its credential is missing.
    MissingKey,
}

impl AutoRouterSetupIssue {
    #[must_use]
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::UnknownKind => "unknown [auto.router] kind (expected \"chat\" or \"decision\")",
            Self::UnsupportedDecisionProvider => {
                "decision routers are served by OpenRouter or TypeSafe"
            }
            Self::Incomplete => "[auto.router] needs a known provider and a model",
            Self::MissingKey => "no API key for the router route",
        }
    }
}

/// Probability or confidence (0..=1) as basis points, so receipts and the
/// inventory stay `Eq` and never re-render a float.
#[must_use]
pub(crate) fn probability_bp(value: f64) -> u16 {
    if !value.is_finite() {
        return 0;
    }
    // Bounded to 0..=10000 before the cast.
    (value.clamp(0.0, 1.0) * 10_000.0).round() as u16
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ModelInventory {
    pub(crate) active_provider: ApiProvider,
    pub(crate) router_provider: ApiProvider,
    pub(crate) router_model: String,
    /// Thinking tier for the classifier call (None = off) (#auto.router).
    pub(crate) router_thinking: Option<String>,
    /// Classifier call timeout in seconds (default 4; clamped at config load).
    pub(crate) router_timeout_secs: u64,
    /// Whether an explicit legacy `[auto.router]` classifier route is
    /// configured. Absent configuration means legacy Auto stays local/free —
    /// holding a provider key never elects a network classifier by itself.
    pub(crate) router_configured: bool,
    pub(crate) router_available: bool,
    /// `[auto.router] kind` (#6525); an unknown kind leaves the router
    /// unconfigured and sets [`Self::router_setup_issue`].
    pub(crate) router_kind: AutoRouterKind,
    /// Endpoint for a decision router (`None` for chat routers).
    pub(crate) router_decision_route: Option<DecisionRouterRoute>,
    /// Decision-router endpoint override (TypeSafe only).
    pub(crate) router_base_url: Option<String>,
    /// Decision-router confidence floor in basis points (0..=10000).
    pub(crate) router_min_confidence_bp: u16,
    /// Why a declared `[auto.router]` cannot run. `None` when no router is
    /// declared or the router is available. A declared-but-unusable router is
    /// shown as failing, never silently ignored.
    pub(crate) router_setup_issue: Option<AutoRouterSetupIssue>,
    /// `[auto] cross_provider = true` opt-in (#4411). When false (the
    /// default), Auto routing — classifier payload included — is confined to
    /// `active_provider`. The full candidate list still carries every
    /// authenticated provider because pickers and explicit `/model` lookups
    /// legitimately need it; only the Auto paths are scoped.
    pub(crate) cross_provider_auto: bool,
    pub(crate) candidates: Vec<ModelRouteCandidate>,
}

impl ModelInventory {
    pub(crate) fn from_config(config: &Config) -> Self {
        Self::from_config_with_health(
            config,
            &crate::provider_readiness::ProviderReadinessSnapshot::default(),
        )
    }

    pub(crate) fn from_config_with_health(
        config: &Config,
        health: &crate::provider_readiness::ProviderReadinessSnapshot,
    ) -> Self {
        let active_provider = config.api_provider();
        let mut candidates = Vec::new();

        for provider in ApiProvider::all().iter().copied() {
            let Some(auth_source) = auth_source_for_provider(config, provider) else {
                continue;
            };
            let default_model = provider_default_model(config, provider);
            let mut models = Vec::<String>::new();
            if let Some(model) = configured_model_for_provider(config, provider) {
                push_model(&mut models, provider, &model);
            }
            if provider == active_provider {
                let active_model = config.default_model();
                if !active_model.trim().eq_ignore_ascii_case("auto") {
                    push_model(&mut models, provider, &active_model);
                }
            }
            for model in models_for_provider(config, active_provider, provider) {
                push_model(&mut models, provider, &model);
            }
            for declaration in config.custom_models.as_deref().unwrap_or_default() {
                if crate::provider_lake::configured_model_for_route(
                    config,
                    provider,
                    &config.provider_identity_for(provider),
                    &config.base_url_for_route(provider),
                    &declaration.id,
                )
                .is_some()
                    && !models.contains(&declaration.id)
                {
                    models.push(declaration.id.clone());
                }
            }
            if models.is_empty() {
                push_model(&mut models, provider, &default_model);
            }

            for model in models {
                let readiness =
                    crate::provider_readiness::resolve_for_model(config, provider, &model, health);
                let mut capability = provider_capability(provider, &model);
                let mut user_declared = false;
                // #5239/#5441: a candidate whose window came from the legacy
                // capability fallback (a `_Nk` name-suffix parse or a
                // vendor-family heuristic) carries the number *and* the fact
                // that nobody verified it — the auto-router must not read a
                // guessed window as a route capability.
                let mut context_window_unverified =
                    codewhale_models::model_catalog::resolved_context_window(&model).is_none();
                if let Ok(route) =
                    crate::route_runtime::resolve_runtime_route(config, provider, Some(&model))
                {
                    if let Some(context_window) = route.candidate.limits().context_tokens {
                        capability.context_window = context_window.min(u64::from(u32::MAX)) as u32;
                        context_window_unverified = !route.context_window.source.is_verified();
                    }
                    // A concrete offering maximum is a stronger fact than the
                    // static compatibility matrix — and is the only way a
                    // membership route (no static cap) gets a known ceiling.
                    if let Some(max_output) = route
                        .candidate
                        .limits()
                        .output_tokens
                        .and_then(|tokens| u32::try_from(tokens).ok())
                        .filter(|tokens| *tokens > 0)
                    {
                        capability.max_output = Some(max_output);
                    }
                    user_declared = route
                        .candidate
                        .applied_limit_overrides()
                        .iter()
                        .any(|entry| {
                            entry.source
                                == codewhale_config::route::OverrideSource::UserModelMetadata
                        });
                    if user_declared {
                        context_window_unverified = !route.context_window.source.is_verified();
                        capability.context_window = route.context_window.tokens;
                        capability.max_output = route
                            .candidate
                            .limits()
                            .output_tokens
                            .and_then(|value| u32::try_from(value).ok());
                        capability.thinking_supported = route.candidate.capabilities().reasoning
                            == codewhale_config::route::CapabilityState::Supported;
                    }
                    // Do not promote bare `k3` into the global capability
                    // catalog. Its thinking trace contract belongs only to
                    // Kimi Code's exact membership-plan route.
                    if !user_declared
                        && crate::config::is_exact_kimi_code_k3_route(
                            provider,
                            &route.candidate.endpoint().base_url,
                            route.candidate.wire_model_id().as_str(),
                        )
                    {
                        capability.thinking_supported = true;
                    }
                }
                let mut tags = Vec::new();
                if capability.context_window >= 1_000_000 {
                    tags.push("long_context");
                }
                if capability.thinking_supported {
                    tags.push("thinking");
                }
                if matches!(
                    provider,
                    ApiProvider::Ollama | ApiProvider::Sglang | ApiProvider::Vllm
                ) {
                    tags.push("local");
                }
                // Unready routes stay visible (annotated) so an operator can
                // override explicitly, but they are never a silent default.
                let default_for_provider = readiness.can_attempt()
                    && (model == default_model
                        || (!user_declared && model.eq_ignore_ascii_case(&default_model)));
                if default_for_provider {
                    tags.push("default");
                }
                if !readiness.can_attempt() {
                    tags.push("unready");
                }

                candidates.push(ModelRouteCandidate {
                    provider,
                    provider_name: provider.as_str(),
                    provider_display_name: provider.display_name(),
                    default_for_provider,
                    model,
                    user_declared,
                    context_window: capability.context_window,
                    context_window_unverified,
                    max_output: capability.max_output,
                    thinking_supported: capability.thinking_supported,
                    cache_telemetry_supported: capability.cache_telemetry_supported,
                    auth_source: auth_source.clone(),
                    readiness: readiness.clone(),
                    tags,
                });
            }
        }

        // `[auto.router]` is legacy `model = auto` configuration and stays that
        // way — it is NOT a Fleet Router. Explicit configuration still works.
        //
        // What is gone is the implicit half: merely holding a DeepSeek key used
        // to silently elect `deepseek-v4-flash` as a network classifier for
        // every Auto turn, spending a user's tokens on a route they never asked
        // for and privileging one provider. With no explicit `[auto.router]`,
        // legacy Auto is now local/free (heuristic-only).
        let router_table = config.auto.as_ref().and_then(|auto| auto.router.as_ref());
        // Any populated key declares a router: a table holding only
        // `timeout_secs` or `base_url` is a malformed router to diagnose, not
        // an absent one to skip silently.
        let router_declared = router_table.is_some_and(|router| {
            router.provider.is_some()
                || router.model.is_some()
                || router.kind.is_some()
                || router.thinking.is_some()
                || router.timeout_secs.is_some()
                || router.min_confidence.is_some()
                || router.base_url.is_some()
        });
        let router_kind = AutoRouterKind::parse(router_table.and_then(|r| r.kind.as_deref()));
        let router_model_setting = router_table
            .and_then(|router| router.model.as_deref())
            .map(str::trim)
            .filter(|model| !model.is_empty());
        let router_provider_setting = router_table
            .and_then(|router| router.provider.as_deref())
            .map(str::trim)
            .filter(|provider| !provider.is_empty());
        let mut router_setup_issue = None;
        let mut router_decision_route = None;
        let explicit_router = match router_kind {
            None => {
                router_setup_issue = Some(AutoRouterSetupIssue::UnknownKind);
                None
            }
            Some(AutoRouterKind::Chat) => router_provider_setting
                .and_then(ApiProvider::parse)
                .zip(router_model_setting)
                .map(|(provider, model)| {
                    (
                        provider,
                        model.to_string(),
                        router_table
                            .and_then(|router| router.thinking.as_deref())
                            .map(str::trim)
                            .filter(|t| !t.is_empty())
                            .map(str::to_string),
                    )
                }),
            Some(AutoRouterKind::Decision) => {
                match router_provider_setting.map(DecisionRouterRoute::parse) {
                    Some(None) => {
                        router_setup_issue =
                            Some(AutoRouterSetupIssue::UnsupportedDecisionProvider);
                        None
                    }
                    Some(Some(route)) => router_model_setting.map(|model| {
                        router_decision_route = Some(route);
                        // A decision model has no reasoning knob, so the
                        // configured `thinking` is ignored. TypeSafe is not a
                        // chat provider; `router_provider` is only a label
                        // there, and `router_decision_route` is authoritative.
                        (ApiProvider::Openrouter, model.to_string(), None)
                    }),
                    None => None,
                }
            }
        };
        let router_configured = explicit_router.is_some();
        let (router_provider, router_model, router_thinking) = explicit_router
            // Kept only as an inert display/default label for the router fields;
            // `router_available` below is what gates any classifier call.
            .unwrap_or_else(|| (ApiProvider::Deepseek, "deepseek-v4-flash".to_string(), None));
        let router_available = router_configured
            && match router_decision_route {
                Some(route) => route.has_key(config),
                None => has_api_key_for(config, router_provider),
            };
        if router_declared && router_setup_issue.is_none() && !router_available {
            router_setup_issue = Some(if router_configured {
                AutoRouterSetupIssue::MissingKey
            } else {
                AutoRouterSetupIssue::Incomplete
            });
        }

        let cross_provider_auto = config.auto_cross_provider();
        let router_timeout_secs = config.auto_router_timeout_secs();
        let router_min_confidence_bp = probability_bp(config.auto_router_min_confidence());

        Self {
            active_provider,
            router_provider,
            router_configured,
            router_available,
            router_model,
            router_thinking,
            router_timeout_secs,
            router_kind: router_kind.unwrap_or(AutoRouterKind::Chat),
            router_decision_route,
            router_base_url: router_table
                .and_then(|router| router.base_url.as_deref())
                .map(str::trim)
                .filter(|url| !url.is_empty())
                .map(str::to_string),
            router_min_confidence_bp,
            router_setup_issue,
            cross_provider_auto,
            candidates,
        }
    }

    /// Whether Auto routing may select `provider` (#4411).
    pub(crate) fn auto_scope_allows(&self, provider: ApiProvider) -> bool {
        self.cross_provider_auto || provider == self.active_provider
    }

    pub(crate) fn candidate(
        &self,
        provider: ApiProvider,
        model: &str,
    ) -> Option<&ModelRouteCandidate> {
        let model = model.trim();
        self.candidates
            .iter()
            .find(|candidate| candidate.provider == provider && candidate.model == model)
            .or_else(|| {
                self.candidates.iter().find(|candidate| {
                    candidate.provider == provider
                        && !candidate.user_declared
                        && candidate.model.eq_ignore_ascii_case(model)
                })
            })
    }

    pub(crate) fn active_default(&self) -> Option<&ModelRouteCandidate> {
        self.candidates
            .iter()
            .find(|candidate| {
                candidate.provider == self.active_provider && candidate.default_for_provider
            })
            .or_else(|| {
                self.candidates.iter().find(|candidate| {
                    candidate.provider == self.active_provider && candidate.readiness.can_attempt()
                })
            })
            .or_else(|| {
                // Falling through to another provider is a cross-provider Auto
                // route (#4411): allowed only under the persisted opt-in. With
                // it off, an unusable active provider surfaces as "no runnable
                // candidate" instead of silently borrowing another provider's
                // credentials.
                self.cross_provider_auto
                    .then(|| {
                        self.candidates
                            .iter()
                            .find(|candidate| candidate.readiness.can_attempt())
                    })
                    .flatten()
            })
    }

    pub(crate) fn router_context_json(&self) -> String {
        #[derive(Serialize)]
        struct RouterInventoryContext<'a> {
            active_provider: ApiProvider,
            candidates: Vec<RouterCandidateContext<'a>>,
        }

        #[derive(Serialize)]
        struct RouterCandidateContext<'a> {
            provider: ApiProvider,
            provider_name: &'a str,
            provider_display_name: &'a str,
            model: &'a str,
            context_window: u32,
            #[serde(skip_serializing_if = "std::ops::Not::not")]
            context_window_unverified: bool,
            #[serde(skip_serializing_if = "Option::is_none")]
            max_output: Option<u32>,
            thinking_supported: bool,
            cache_telemetry_supported: bool,
            default_for_provider: bool,
            tags: &'a [&'static str],
        }

        // The classifier needs route capabilities, not credentials, endpoint
        // configuration, or provider error text. Filter to runnable candidates
        // and project only non-secret routing facts before serializing.
        //
        // Scope (#4411): without the persisted `[auto] cross_provider` opt-in,
        // the payload names only the active provider's routes. Which other
        // providers a user has credentials for is not something Auto discloses
        // to a classifier by default.
        let candidates = self
            .candidates
            .iter()
            .filter(|candidate| {
                candidate.readiness.can_attempt() && self.auto_scope_allows(candidate.provider)
            })
            .map(|candidate| RouterCandidateContext {
                provider: candidate.provider,
                provider_name: candidate.provider_name,
                provider_display_name: candidate.provider_display_name,
                model: &candidate.model,
                context_window: candidate.context_window,
                context_window_unverified: candidate.context_window_unverified,
                max_output: candidate.max_output,
                thinking_supported: candidate.thinking_supported,
                cache_telemetry_supported: candidate.cache_telemetry_supported,
                default_for_provider: candidate.default_for_provider,
                tags: &candidate.tags,
            })
            .collect();
        serde_json::to_string(&RouterInventoryContext {
            active_provider: self.active_provider,
            candidates,
        })
        .unwrap_or_else(|_| "{}".to_string())
    }
}

fn push_model(models: &mut Vec<String>, provider: ApiProvider, model: &str) {
    if provider == ApiProvider::Ollama && crate::config::is_unresolved_local_ollama_model(model) {
        return;
    }
    let Some(model) = normalize_model_name_for_provider(provider, model)
        .or_else(|| crate::config::normalize_custom_model_id(model))
    else {
        return;
    };
    if !models
        .iter()
        .any(|existing| existing.eq_ignore_ascii_case(&model))
    {
        models.push(model);
    }
}

fn configured_model_for_provider(config: &Config, provider: ApiProvider) -> Option<String> {
    config
        .provider_config_for(provider)
        .and_then(|entry| entry.model.clone())
        .map(|model| model.trim().to_string())
        .filter(|model| !model.is_empty())
}

pub(crate) fn provider_default_model(config: &Config, provider: ApiProvider) -> String {
    let configured = configured_model_for_provider(config, provider).or_else(|| {
        (provider == config.api_provider() && config.default_text_model.is_some())
            .then(|| config.default_model())
    });
    let selector = configured.as_deref().filter(|model| {
        !model.trim().eq_ignore_ascii_case("auto")
            && !(provider == ApiProvider::Ollama
                && crate::config::is_unresolved_local_ollama_model(model))
    });
    // Inventory labels must use the executable route's exact endpoint default,
    // not whichever provider-wide snapshot happened to refresh most recently.
    crate::route_runtime::resolve_runtime_route(config, provider, selector)
        .map(|route| route.model)
        .unwrap_or_else(|_| {
            configured.unwrap_or_else(|| {
                provider
                    .kind()
                    .map(|kind| kind.provider().default_model())
                    .unwrap_or(crate::config::DEFAULT_TEXT_MODEL)
                    .to_string()
            })
        })
}

fn auth_source_for_provider(config: &Config, provider: ApiProvider) -> Option<ModelAuthSource> {
    let credential_state =
        crate::provider_readiness::credential_state_for_provider(config, provider);
    match credential_state {
        crate::provider_readiness::CredentialState::NoAuth => {
            return Some(ModelAuthSource::NoAuth);
        }
        crate::provider_readiness::CredentialState::Local => {
            return Some(ModelAuthSource::KeylessLocal);
        }
        crate::provider_readiness::CredentialState::ImportedToken => {
            return Some(ModelAuthSource::ImportedToken);
        }
        crate::provider_readiness::CredentialState::MissingKey
        | crate::provider_readiness::CredentialState::MissingLogin
        | crate::provider_readiness::CredentialState::ExternalConsent
        | crate::provider_readiness::CredentialState::Legacy => return None,
        crate::provider_readiness::CredentialState::Saved => {}
    }

    if provider == ApiProvider::Custom {
        let configured = config.provider_config_for(provider)?;
        if configured
            .api_key_env
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .is_some_and(|name| std::env::var(name).is_ok_and(|value| !value.trim().is_empty()))
        {
            return Some(ModelAuthSource::Env);
        }
        return (configured.api_key.as_deref().is_some_and(|value| {
            crate::config::classify_config_api_key_value(value)
                == crate::config::ConfigApiKeyValueKind::Literal
        }) || crate::config::explicit_cli_api_key_override().is_some())
        .then_some(ModelAuthSource::Config);
    }
    if provider_uses_oauth_cli(config, provider) {
        return Some(ModelAuthSource::OAuthCli);
    }
    if config
        .provider_config_for(provider)
        .and_then(|entry| entry.api_key_env.as_deref())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .is_some_and(|name| std::env::var(name).is_ok_and(|value| !value.trim().is_empty()))
    {
        return Some(ModelAuthSource::Env);
    }
    if !config.should_skip_secret_store_for_provider(provider) && env_has_key_for(provider) {
        return Some(ModelAuthSource::Env);
    }
    Some(ModelAuthSource::Config)
}

fn provider_uses_oauth_cli(config: &Config, provider: ApiProvider) -> bool {
    if config.provider_uses_custom_endpoint(provider) {
        return false;
    }
    match provider {
        ApiProvider::OpenaiCodex => true,
        ApiProvider::Xai => config
            .provider_config_for(provider)
            .and_then(|entry| entry.auth_mode.as_deref())
            .is_some_and(crate::oauth::auth_mode_uses_xai_oauth),
        _ => false,
    }
}

fn env_has_key_for(provider: ApiProvider) -> bool {
    env_keys_for_provider(provider)
        .iter()
        .any(|key| std::env::var(key).is_ok_and(|value| !value.trim().is_empty()))
}

fn env_keys_for_provider(provider: ApiProvider) -> &'static [&'static str] {
    provider.env_vars()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_env_keys_follow_provider_metadata() {
        for provider in ApiProvider::all() {
            assert_eq!(env_keys_for_provider(*provider), provider.env_vars());
        }
    }

    #[test]
    fn inventory_includes_only_usable_authenticated_providers() {
        let _env_lock = crate::test_support::lock_test_env();
        let _deepseek = crate::test_support::EnvVarGuard::set("DEEPSEEK_API_KEY", "ds-key");
        let _zai = crate::test_support::EnvVarGuard::set("ZAI_API_KEY", "zai-key");
        let _minimax = crate::test_support::EnvVarGuard::remove("MINIMAX_API_KEY");
        let config = Config {
            provider: Some("zai".to_string()),
            default_text_model: Some("deepseek-v4-pro".to_string()),
            ..Default::default()
        };

        let inventory = ModelInventory::from_config(&config);

        // A DeepSeek key alone no longer elects a network classifier: with no
        // explicit `[auto.router]`, legacy Auto stays local/free.
        assert!(!inventory.router_configured);
        assert!(!inventory.router_available);
        assert!(
            inventory
                .candidate(ApiProvider::Zai, crate::config::ZAI_GLM_5_2_MODEL)
                .is_some()
        );
        assert!(
            inventory
                .candidates
                .iter()
                .all(|candidate| candidate.provider != ApiProvider::Minimax)
        );
    }

    #[test]
    fn inventory_marks_local_providers_keyless() {
        let _env_lock = crate::test_support::lock_test_env();
        let _deepseek = crate::test_support::EnvVarGuard::remove("DEEPSEEK_API_KEY");
        let mut config = Config::default();
        config.set_provider_model_override(ApiProvider::Ollama, Some("local-tag:latest".into()));

        let inventory = ModelInventory::from_config(&config);

        assert!(
            inventory
                .candidates
                .iter()
                .any(|candidate| candidate.provider == ApiProvider::Ollama
                    && candidate.auth_source == ModelAuthSource::KeylessLocal)
        );
    }

    #[test]
    fn inventory_never_marks_ollama_cloud_keyless_or_local() {
        let _env_lock = crate::test_support::lock_test_env();
        let _cloud_env = crate::test_support::EnvVarGuard::remove("OLLAMA_CLOUD_API_KEY");
        let _official_env = crate::test_support::EnvVarGuard::remove("OLLAMA_API_KEY");
        let config = Config {
            provider: Some("ollama-cloud".to_string()),
            providers: Some(crate::config::ProvidersConfig {
                ollama_cloud: crate::config::ProviderConfig {
                    api_key: Some("cloud-key".to_string()),
                    ..Default::default()
                },
                ..Default::default()
            }),
            ..Default::default()
        };

        let inventory = ModelInventory::from_config(&config);
        let candidate = inventory
            .candidate(
                ApiProvider::OllamaCloud,
                crate::config::DEFAULT_OLLAMA_CLOUD_MODEL,
            )
            .expect("authenticated Ollama Cloud candidate");
        assert_eq!(candidate.auth_source, ModelAuthSource::Config);
        assert!(!candidate.tags.contains(&"local"));
        assert_ne!(candidate.readiness.label(), "local · not checked");
    }

    #[test]
    fn inventory_never_admits_kimi_cli_oauth_import() {
        let _env_lock = crate::test_support::lock_test_env();
        let temp = tempfile::tempdir().expect("Kimi import fixture root");
        let kimi_home = temp.path().join("kimi-code");
        std::fs::create_dir_all(kimi_home.join("credentials")).expect("Kimi credential directory");
        let expires_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock after epoch")
            .as_secs_f64()
            + 3600.0;
        let credential_path = kimi_home.join("credentials/kimi-code.json");
        let credential_raw = serde_json::json!({
            "access_token": "unexpired-user-owned-token",
            "refresh_token": "must-not-be-used",
            "expires_at": expires_at,
        })
        .to_string();
        std::fs::write(&credential_path, &credential_raw).expect("write Kimi import fixture");
        let _kimi_home = crate::test_support::EnvVarGuard::set(
            "KIMI_CODE_HOME",
            kimi_home.to_str().expect("utf8 path"),
        );
        let config = Config {
            provider: Some("moonshot".to_string()),
            providers: Some(crate::config::ProvidersConfig {
                moonshot: crate::config::ProviderConfig {
                    auth_mode: Some("kimi_oauth".to_string()),
                    ..Default::default()
                },
                ..Default::default()
            }),
            ..Default::default()
        };

        let inventory = ModelInventory::from_config(&config);
        assert!(
            inventory
                .candidates
                .iter()
                .all(|candidate| candidate.provider != ApiProvider::Moonshot),
            "unsupported Kimi CLI OAuth must not enter the routing inventory"
        );
        assert_eq!(
            std::fs::read_to_string(credential_path).expect("Kimi file remains untouched"),
            credential_raw
        );
    }

    #[test]
    fn inventory_uses_kimi_code_k3_route_context_not_generic_fallback() {
        let config = Config {
            provider: Some("moonshot".to_string()),
            providers: Some(crate::config::ProvidersConfig {
                moonshot: crate::config::ProviderConfig {
                    api_key: Some("test-kimi-key".to_string()),
                    base_url: Some(crate::config::DEFAULT_KIMI_CODE_BASE_URL.to_string()),
                    model: Some(crate::config::KIMI_CODE_K3_MODEL.to_string()),
                    ..Default::default()
                },
                ..Default::default()
            }),
            ..Default::default()
        };

        let inventory = ModelInventory::from_config(&config);
        let candidate = inventory
            .candidate(ApiProvider::Moonshot, crate::config::KIMI_CODE_K3_MODEL)
            .expect("configured Kimi Code K3 route");

        assert_eq!(candidate.context_window, 262_144);
        assert!(candidate.thinking_supported);
        assert!(candidate.tags.contains(&"thinking"));
        assert!(!candidate.tags.contains(&"long_context"));
    }

    /// #5441: the auto-router inventory carries a `_Nk` name-suffix window
    /// together with the fact that nobody verified it, so a classifier never
    /// reads a naming convention as a route capability.
    #[test]
    fn router_inventory_marks_name_suffix_windows_unverified() {
        let config = Config {
            provider: Some("vllm".to_string()),
            providers: Some(crate::config::ProvidersConfig {
                vllm: crate::config::ProviderConfig {
                    base_url: Some("http://localhost:8000/v1".to_string()),
                    model: Some("qwen3-32b-256k".to_string()),
                    ..Default::default()
                },
                ..Default::default()
            }),
            ..Default::default()
        };

        let inventory = ModelInventory::from_config(&config);
        let candidate = inventory
            .candidate(ApiProvider::Vllm, "qwen3-32b-256k")
            .expect("configured self-hosted route");
        assert_eq!(candidate.context_window, 256_000);
        assert!(
            candidate.context_window_unverified,
            "a name-suffix window must not enter the router payload as a fact"
        );

        let payload = inventory.router_context_json();
        assert!(
            payload.contains("\"context_window_unverified\":true"),
            "payload must serialize the marker: {payload}"
        );
    }

    #[test]
    fn inventory_includes_custom_api_key_env_route() {
        let _env_lock = crate::test_support::lock_test_env();
        let _custom_key = crate::test_support::EnvVarGuard::set("ACME_CUSTOM_KEY", "custom-key");
        let config = Config {
            provider: Some("acme".to_string()),
            providers: Some(crate::config::ProvidersConfig {
                custom: std::collections::HashMap::from([(
                    "acme".to_string(),
                    crate::config::ProviderConfig {
                        kind: Some("openai-compatible".to_string()),
                        base_url: Some("https://api.acme.test/v1".to_string()),
                        model: Some("acme-coder".to_string()),
                        api_key_env: Some("ACME_CUSTOM_KEY".to_string()),
                        ..Default::default()
                    },
                )]),
                ..Default::default()
            }),
            ..Default::default()
        };

        let inventory = ModelInventory::from_config(&config);
        assert!(
            inventory
                .candidates
                .iter()
                .any(|candidate| candidate.provider == ApiProvider::Custom
                    && candidate.model == "acme-coder"
                    && candidate.auth_source == ModelAuthSource::Env)
        );
    }

    #[test]
    fn inventory_router_timeout_secs_respects_config_with_clamp() {
        let _env_lock = crate::test_support::lock_test_env();

        // Unset: the legacy default (4 s) survives.
        let config = Config {
            ..Default::default()
        };
        assert_eq!(ModelInventory::from_config(&config).router_timeout_secs, 4);

        // Explicit value is honored.
        let config = Config {
            auto: Some(crate::config::AutoConfig {
                router: Some(crate::config::AutoRouterConfig {
                    provider: Some("custom".to_string()),
                    model: Some("local-router".to_string()),
                    thinking: None,
                    timeout_secs: Some(15),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(ModelInventory::from_config(&config).router_timeout_secs, 15);

        // Out-of-range values clamp to the safety ceiling, never to zero.
        let config = Config {
            auto: Some(crate::config::AutoConfig {
                router: Some(crate::config::AutoRouterConfig {
                    provider: Some("custom".to_string()),
                    model: Some("local-router".to_string()),
                    thinking: None,
                    timeout_secs: Some(9_999),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(
            ModelInventory::from_config(&config).router_timeout_secs,
            crate::config::MAX_AUTO_ROUTER_TIMEOUT_SECS
        );

        // Zero means "use the default", not an instant timeout.
        let config = Config {
            auto: Some(crate::config::AutoConfig {
                router: Some(crate::config::AutoRouterConfig {
                    provider: Some("custom".to_string()),
                    model: Some("local-router".to_string()),
                    thinking: None,
                    timeout_secs: Some(0),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(ModelInventory::from_config(&config).router_timeout_secs, 4);
    }

    #[test]
    fn inventory_ignores_unresolved_command_and_secret_auth_metadata() {
        let _env_lock = crate::test_support::lock_test_env();
        let temp = tempfile::tempdir().expect("isolated credential home");
        let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", temp.path());
        let _backend = crate::test_support::EnvVarGuard::set("CODEWHALE_SECRET_BACKEND", "file");
        let _deepseek = crate::test_support::EnvVarGuard::remove("DEEPSEEK_API_KEY");
        let _openai = crate::test_support::EnvVarGuard::remove("OPENAI_API_KEY");
        let _xai = crate::test_support::EnvVarGuard::remove("XAI_API_KEY");
        let mut providers = crate::config::ProvidersConfig::default();
        providers.openai.auth = Some(codewhale_config::ProviderAuthSourceToml {
            source: codewhale_config::AuthSourceKind::Command,
            command: vec!["secret-tool".to_string(), "lookup".to_string()],
            timeout_ms: Some(2000),
            secret_id: None,
        });
        providers.xai.auth = Some(codewhale_config::ProviderAuthSourceToml {
            source: codewhale_config::AuthSourceKind::Secret,
            command: Vec::new(),
            timeout_ms: None,
            secret_id: Some("codewhale/xai".to_string()),
        });
        let config = Config {
            provider: Some("openai".to_string()),
            providers: Some(providers),
            ..Default::default()
        };

        let inventory = ModelInventory::from_config(&config);
        assert!(inventory.candidates.iter().all(|candidate| !matches!(
            candidate.provider,
            ApiProvider::Openai | ApiProvider::Xai
        )));
    }

    #[test]
    fn auto_router_config_overrides_default_classifier_route() {
        let config = Config {
            auto: Some(crate::config::AutoConfig {
                cost_saving: None,
                cross_provider: None,
                router: Some(crate::config::AutoRouterConfig {
                    provider: Some("zai".to_string()),
                    model: Some("glm-5-turbo".to_string()),
                    thinking: Some("low".to_string()),
                    timeout_secs: None,
                    ..Default::default()
                }),
            }),
            ..Default::default()
        };

        let inventory = ModelInventory::from_config(&config);
        assert!(inventory.router_configured);
        assert_eq!(inventory.router_provider, ApiProvider::Zai);
        assert_eq!(inventory.router_model, "glm-5-turbo");
        assert_eq!(inventory.router_thinking.as_deref(), Some("low"));
    }

    /// A DeepSeek key must never, on its own, turn on a network classifier.
    /// `[auto.router]` stays legacy `model = auto` configuration; absent it,
    /// legacy Auto is local/free.
    #[test]
    fn a_deepseek_key_alone_never_elects_an_implicit_flash_classifier() {
        let _env_lock = crate::test_support::lock_test_env();
        let _deepseek = crate::test_support::EnvVarGuard::set("DEEPSEEK_API_KEY", "ds-key");
        let config = Config {
            provider: Some("deepseek".to_string()),
            ..Default::default()
        };

        let inventory = ModelInventory::from_config(&config);

        assert!(
            !inventory.router_configured,
            "no [auto.router] means no configured classifier"
        );
        assert!(
            !inventory.router_available,
            "holding a DeepSeek key must not silently select deepseek-v4-flash as a classifier"
        );
    }

    #[test]
    fn an_explicit_legacy_auto_router_still_works_when_its_key_is_present() {
        let _env_lock = crate::test_support::lock_test_env();
        let _zai = crate::test_support::EnvVarGuard::set("ZAI_API_KEY", "zai-key");
        let config = Config {
            auto: Some(crate::config::AutoConfig {
                cost_saving: None,
                router: Some(crate::config::AutoRouterConfig {
                    provider: Some("zai".to_string()),
                    model: Some("glm-5-turbo".to_string()),
                    thinking: None,
                    timeout_secs: None,
                    ..Default::default()
                }),
                cross_provider: None,
            }),
            ..Default::default()
        };

        let inventory = ModelInventory::from_config(&config);

        assert!(inventory.router_configured);
        assert!(inventory.router_available);
        assert_eq!(inventory.router_model, "glm-5-turbo");
    }

    #[test]
    fn inventory_marks_explicit_no_auth_separately_from_keyless_local() {
        let mut providers = crate::config::ProvidersConfig::default();
        providers.vllm.auth_mode = Some("none".to_string());
        providers.vllm.model = Some("local-model".to_string());
        let config = Config {
            provider: Some("vllm".to_string()),
            providers: Some(providers),
            ..Default::default()
        };

        let inventory = ModelInventory::from_config(&config);
        let candidate = inventory
            .candidates
            .iter()
            .find(|candidate| {
                candidate.provider == ApiProvider::Vllm && candidate.model == "local-model"
            })
            .expect("vLLM no-auth candidate");

        assert_eq!(candidate.auth_source, ModelAuthSource::NoAuth);
        assert_eq!(
            candidate.readiness,
            crate::provider_readiness::ResolvedProviderReadiness::NoAuthUnchecked
        );
    }

    #[test]
    fn unready_candidates_are_never_provider_defaults() {
        use crate::provider_readiness::ResolvedProviderReadiness;

        let candidate = ModelRouteCandidate {
            provider: ApiProvider::Openai,
            provider_name: "openai",
            provider_display_name: "OpenAI",
            model: "gpt-5.5".to_string(),
            context_window: 128_000,
            context_window_unverified: false,
            user_declared: false,
            max_output: Some(16_384),
            thinking_supported: true,
            cache_telemetry_supported: false,
            auth_source: ModelAuthSource::Config,
            readiness: ResolvedProviderReadiness::MissingLogin,
            default_for_provider: false,
            tags: vec!["unready"],
        };
        assert!(!candidate.readiness.can_attempt());
        assert!(!candidate.default_for_provider);
        assert!(candidate.tags.contains(&"unready"));
    }

    #[test]
    fn active_default_never_falls_back_to_unready_candidate() {
        let inventory = ModelInventory {
            active_provider: ApiProvider::Openai,
            router_provider: ApiProvider::Deepseek,
            router_model: "deepseek-v4-flash".to_string(),
            router_thinking: None,
            router_timeout_secs: 4,
            router_configured: false,
            router_available: false,
            router_kind: AutoRouterKind::Chat,
            router_decision_route: None,
            router_base_url: None,
            router_min_confidence_bp: 5_000,
            router_setup_issue: None,
            cross_provider_auto: false,
            candidates: vec![ModelRouteCandidate {
                provider: ApiProvider::Openai,
                provider_name: "openai",
                provider_display_name: "OpenAI",
                model: "unsupported-model".to_string(),
                context_window: 1,
                context_window_unverified: false,
                user_declared: false,
                max_output: Some(1),
                thinking_supported: false,
                cache_telemetry_supported: false,
                auth_source: ModelAuthSource::Config,
                readiness: crate::provider_readiness::ResolvedProviderReadiness::InvalidRoute,
                default_for_provider: false,
                tags: vec!["unready"],
            }],
        };

        assert!(inventory.active_default().is_none());
    }

    #[test]
    fn router_context_is_runnable_and_redacts_auth_and_failure_details() {
        let _env_lock = crate::test_support::lock_test_env();
        let _deepseek = crate::test_support::EnvVarGuard::set("DEEPSEEK_API_KEY", "ds-key");
        let mut inventory = ModelInventory::from_config(&Config::default());
        let candidate = inventory
            .candidates
            .iter_mut()
            .find(|candidate| candidate.provider == ApiProvider::Deepseek)
            .expect("DeepSeek inventory candidate");
        candidate.readiness =
            crate::provider_readiness::ResolvedProviderReadiness::SavedLastCheckFailed {
                category: crate::error_taxonomy::ErrorCategory::Authentication,
                message: "Bearer super-secret-router-token".to_string(),
            };
        inventory.candidates.push(ModelRouteCandidate {
            provider: ApiProvider::Openai,
            provider_name: "openai",
            provider_display_name: "OpenAI",
            model: "unsupported-model".to_string(),
            context_window: 1,
            context_window_unverified: false,
            user_declared: false,
            max_output: Some(1),
            thinking_supported: false,
            cache_telemetry_supported: false,
            auth_source: ModelAuthSource::Config,
            readiness: crate::provider_readiness::ResolvedProviderReadiness::InvalidRoute,
            default_for_provider: false,
            tags: vec!["unready"],
        });

        let json = inventory.router_context_json();

        assert!(json.contains("deepseek-v4"));
        assert!(!json.contains("super-secret-router-token"));
        assert!(!json.contains("auth_source"));
        assert!(!json.contains("unsupported-model"));
    }

    #[test]
    fn router_context_names_only_the_active_provider_by_default() {
        // #4411: a Z.ai session with a DeepSeek key in the environment must
        // not disclose the DeepSeek routes — or the fact that a DeepSeek
        // credential exists — to the classifier.
        let _env_lock = crate::test_support::lock_test_env();
        let _deepseek = crate::test_support::EnvVarGuard::set("DEEPSEEK_API_KEY", "ds-key");
        let _zai = crate::test_support::EnvVarGuard::set("ZAI_API_KEY", "zai-key");
        let config = Config {
            provider: Some("zai".to_string()),
            ..Default::default()
        };

        let inventory = ModelInventory::from_config(&config);
        assert!(
            inventory
                .candidates
                .iter()
                .any(|candidate| candidate.provider == ApiProvider::Deepseek),
            "the full inventory still knows about DeepSeek for pickers/explicit routes"
        );

        let json = inventory.router_context_json();
        let payload: serde_json::Value =
            serde_json::from_str(&json).expect("router context is JSON");
        let providers: Vec<&str> = payload["candidates"]
            .as_array()
            .expect("candidate array")
            .iter()
            .map(|candidate| candidate["provider_name"].as_str().expect("provider name"))
            .collect();

        assert!(!providers.is_empty(), "active provider routes must remain");
        assert!(
            providers.iter().all(|provider| *provider == "zai"),
            "classifier payload leaked another provider: {json}"
        );
        assert!(!json.contains("deepseek"), "{json}");
    }

    #[test]
    fn router_context_includes_other_providers_under_persisted_opt_in() {
        let _env_lock = crate::test_support::lock_test_env();
        let _deepseek = crate::test_support::EnvVarGuard::set("DEEPSEEK_API_KEY", "ds-key");
        let _zai = crate::test_support::EnvVarGuard::set("ZAI_API_KEY", "zai-key");
        let config = Config {
            provider: Some("zai".to_string()),
            auto: Some(crate::config::AutoConfig {
                cost_saving: None,
                cross_provider: Some(true),
                router: None,
            }),
            ..Default::default()
        };

        let json = ModelInventory::from_config(&config).router_context_json();

        assert!(json.contains("\"zai\""), "{json}");
        assert!(json.contains("deepseek"), "{json}");
    }

    #[test]
    fn implicit_deepseek_classifier_is_out_of_scope_for_another_active_provider() {
        // #4411: the default classifier route is DeepSeek flash. Calling it
        // from a Z.ai session would send the turn's prompt to a second
        // provider, so it stays unavailable without an explicit opt-in.
        let _env_lock = crate::test_support::lock_test_env();
        let _deepseek = crate::test_support::EnvVarGuard::set("DEEPSEEK_API_KEY", "ds-key");
        let _zai = crate::test_support::EnvVarGuard::set("ZAI_API_KEY", "zai-key");
        let zai = Config {
            provider: Some("zai".to_string()),
            ..Default::default()
        };
        assert!(!ModelInventory::from_config(&zai).router_available);

        // `cross_provider = true` widens which candidates Auto may pick; it is
        // NOT a classifier election. With the implicit DeepSeek-flash default
        // removed, no network classifier runs without an explicit
        // `[auto.router]` route — a scope opt-in alone stays local/free.
        let opted_in = Config {
            auto: Some(crate::config::AutoConfig {
                cost_saving: None,
                cross_provider: Some(true),
                router: None,
            }),
            ..zai.clone()
        };
        let widened = ModelInventory::from_config(&opted_in);
        assert!(!widened.router_available);
        assert!(widened.auto_scope_allows(ApiProvider::Deepseek));

        // An explicitly configured `[auto.router]` is itself a persisted
        // opt-in for that classifier route.
        let explicit_router = Config {
            auto: Some(crate::config::AutoConfig {
                cost_saving: None,
                cross_provider: None,
                router: Some(crate::config::AutoRouterConfig {
                    provider: Some("deepseek".to_string()),
                    model: Some("deepseek-v4-flash".to_string()),
                    thinking: None,
                    timeout_secs: None,
                    ..Default::default()
                }),
            }),
            ..zai.clone()
        };
        assert!(ModelInventory::from_config(&explicit_router).router_available);

        // A DeepSeek session gets no free classifier either: with the
        // implicit flash default removed, only an explicit `[auto.router]`
        // elects a network classifier, active provider or not.
        let deepseek = Config {
            provider: Some("deepseek".to_string()),
            ..Default::default()
        };
        assert!(!ModelInventory::from_config(&deepseek).router_available);
    }

    #[test]
    fn declared_inventory_ids_remain_case_distinct() {
        let _env = crate::test_support::lock_test_env();
        let home = tempfile::tempdir().unwrap();
        let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", home.path());
        let mut config: Config = toml::from_str(include_str!(
            "../../config/tests/fixtures/custom_models.toml"
        ))
        .unwrap();
        let declaration = config.custom_models.as_mut().unwrap().first_mut().unwrap();
        declaration.id = "Preview-fixture".into();
        let mut other = declaration.clone();
        other.id = "preview-fixture".into();
        other.limit.as_mut().unwrap().context = Some(128000);
        config.custom_models.as_mut().unwrap().push(other);
        config.set_provider_api_key_override(ApiProvider::Deepseek, Some("fixture-key".into()));
        let inventory = ModelInventory::from_config(&config);
        for (id, context) in [("Preview-fixture", 96000), ("preview-fixture", 128000)] {
            let candidate = inventory.candidate(ApiProvider::Deepseek, id).unwrap();
            assert!(candidate.user_declared);
            assert_eq!(candidate.model, id);
            assert_eq!(candidate.context_window, context);
        }
        assert!(
            inventory
                .candidate(ApiProvider::Deepseek, "PREVIEW-FIXTURE")
                .is_none()
        );
    }

    #[test]
    fn ollama_inventory_default_uses_only_the_fresh_exact_endpoint_roster() {
        use codewhale_config::catalog::{
            CatalogOffering, CatalogRefreshError, CatalogSource, ProviderCatalogDelta,
            base_url_fingerprint, now_unix,
        };

        let _env = crate::test_support::lock_test_env();
        let _live = crate::provider_lake::lock_live_snapshot();
        let home = tempfile::tempdir().unwrap();
        let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", home.path());
        crate::provider_catalog_live::reset_cache_for_test();
        crate::provider_lake::clear_live_snapshot();
        let mut config = Config {
            provider: Some("ollama".to_string()),
            ..Default::default()
        };
        let endpoint = "http://localhost:11445/v1";
        config.provider_config_for_mut(ApiProvider::Ollama).base_url = Some(endpoint.into());
        assert_eq!(
            provider_default_model(&config, ApiProvider::Ollama),
            "unknown"
        );
        assert!(
            ModelInventory::from_config(&config)
                .candidates
                .iter()
                .all(|row| { row.provider != ApiProvider::Ollama || row.model != "unknown" })
        );
        let fingerprint = base_url_fingerprint(endpoint);
        let now = now_unix();
        let ticket = crate::provider_catalog_live::begin_refresh_for_identity(
            ApiProvider::Ollama,
            "ollama",
            endpoint,
        );
        crate::provider_catalog_live::record_success_if_current(
            &ticket,
            ProviderCatalogDelta {
                provider: "ollama".into(),
                base_url_fingerprint: fingerprint.clone(),
                fetched_at: now,
                offerings: vec![CatalogOffering {
                    provider: "ollama".into(),
                    wire_model_id: "qwen2.5:0.5b".into(),
                    endpoint_key: "chat".into(),
                    source: CatalogSource::Live {
                        base_url_fingerprint: fingerprint.clone(),
                        fetched_at: now,
                    },
                    ..Default::default()
                }],
            },
        );
        assert_eq!(
            provider_default_model(&config, ApiProvider::Ollama),
            "qwen2.5:0.5b"
        );
        let inventory = ModelInventory::from_config(&config);
        assert!(inventory.candidates.iter().any(|row| {
            row.provider == ApiProvider::Ollama
                && row.model == "qwen2.5:0.5b"
                && row.default_for_provider
        }));
        let mut other = config.clone();
        other.provider_config_for_mut(ApiProvider::Ollama).base_url =
            Some("http://localhost:11446/v1".into());
        assert_eq!(
            provider_default_model(&other, ApiProvider::Ollama),
            "unknown"
        );
        crate::provider_catalog_live::record_failure_if_current(
            &ticket,
            "ollama",
            &fingerprint,
            CatalogRefreshError::Network,
        );
        assert_eq!(
            provider_default_model(&config, ApiProvider::Ollama),
            "unknown"
        );
        config.set_provider_model_override(ApiProvider::Ollama, Some("chosen:tag".into()));
        assert_eq!(
            provider_default_model(&config, ApiProvider::Ollama),
            "chosen:tag"
        );
        crate::provider_catalog_live::reset_cache_for_test();
        crate::provider_lake::clear_live_snapshot();
    }
}

#[cfg(test)]
mod decision_router_inventory_tests {
    use super::*;

    fn with_router(router: crate::config::AutoRouterConfig, openrouter_key: bool) -> Config {
        Config {
            provider: Some("deepseek".to_string()),
            providers: Some(crate::config::ProvidersConfig {
                openrouter: crate::config::ProviderConfig {
                    api_key: openrouter_key.then(|| "or-test-key".to_string()),
                    ..Default::default()
                },
                ..Default::default()
            }),
            auto: Some(crate::config::AutoConfig {
                cost_saving: None,
                cross_provider: None,
                router: Some(router),
            }),
            ..Default::default()
        }
    }

    fn decision(provider: &str) -> crate::config::AutoRouterConfig {
        crate::config::AutoRouterConfig {
            kind: Some("decision".to_string()),
            provider: Some(provider.to_string()),
            model: Some("typesafe/jev-1.13".to_string()),
            ..Default::default()
        }
    }

    /// Tuple fields drop in order: restore the environment, then unlock.
    fn hermetic() -> (
        crate::test_support::EnvVarGuard,
        crate::test_support::EnvVarGuard,
        crate::test_support::TestEnvLock,
    ) {
        let lock = crate::test_support::lock_test_env();
        (
            crate::test_support::EnvVarGuard::remove("OPENROUTER_API_KEY"),
            crate::test_support::EnvVarGuard::remove("TYPESAFE_API_KEY"),
            lock,
        )
    }

    #[test]
    fn decision_router_on_openrouter_needs_the_openrouter_key() {
        let _env = hermetic();
        let with_key = ModelInventory::from_config(&with_router(decision("openrouter"), true));
        assert!(with_key.router_configured);
        assert!(with_key.router_available);
        assert_eq!(with_key.router_kind, AutoRouterKind::Decision);
        assert_eq!(
            with_key.router_decision_route,
            Some(DecisionRouterRoute::Openrouter)
        );
        assert_eq!(
            with_key.router_thinking, None,
            "decision models ignore thinking"
        );
        assert_eq!(with_key.router_min_confidence_bp, 5_000);
        assert_eq!(with_key.router_setup_issue, None);

        let without_key = ModelInventory::from_config(&with_router(decision("openrouter"), false));
        assert!(without_key.router_configured);
        assert!(!without_key.router_available);
        assert_eq!(
            without_key.router_setup_issue,
            Some(AutoRouterSetupIssue::MissingKey)
        );
    }

    #[test]
    fn unknown_kind_or_unsupported_decision_provider_is_not_configured() {
        let _env = hermetic();
        let bogus = crate::config::AutoRouterConfig {
            kind: Some("bogus".to_string()),
            ..decision("openrouter")
        };
        let inventory = ModelInventory::from_config(&with_router(bogus, true));
        assert!(!inventory.router_configured);
        assert!(!inventory.router_available);
        assert_eq!(
            inventory.router_setup_issue,
            Some(AutoRouterSetupIssue::UnknownKind)
        );

        let zai = ModelInventory::from_config(&with_router(decision("zai"), true));
        assert!(!zai.router_configured);
        assert_eq!(
            zai.router_setup_issue,
            Some(AutoRouterSetupIssue::UnsupportedDecisionProvider)
        );
    }

    #[test]
    fn a_router_table_with_only_tuning_keys_is_declared_and_incomplete() {
        let _env = hermetic();
        let tuning_only = [
            crate::config::AutoRouterConfig {
                timeout_secs: Some(3),
                ..Default::default()
            },
            crate::config::AutoRouterConfig {
                min_confidence: Some(0.6),
                ..Default::default()
            },
            crate::config::AutoRouterConfig {
                thinking: Some("off".to_string()),
                ..Default::default()
            },
            crate::config::AutoRouterConfig {
                base_url: Some("https://api.typesafe.ai/v1".to_string()),
                ..Default::default()
            },
        ];
        for router in tuning_only {
            let inventory = ModelInventory::from_config(&with_router(router.clone(), true));
            assert!(!inventory.router_available, "{router:?}");
            assert_eq!(
                inventory.router_setup_issue,
                Some(AutoRouterSetupIssue::Incomplete),
                "{router:?}"
            );
        }
    }

    #[test]
    fn min_confidence_is_clamped() {
        let _env = hermetic();
        for (configured, expected) in [
            (Some(2.0), 10_000),
            (Some(-1.0), 0),
            (Some(0.35), 3_500),
            (None, 5_000),
        ] {
            let router = crate::config::AutoRouterConfig {
                min_confidence: configured,
                ..decision("openrouter")
            };
            assert_eq!(
                ModelInventory::from_config(&with_router(router, true)).router_min_confidence_bp,
                expected,
                "{configured:?}"
            );
        }
    }
}
