//! System One decision API on the existing client (#6525).
//!
//! `[auto.router] kind = "decision"` asks a non-generative decision model
//! (TypeSafe's Jev) one typed Choice per turn. The wire is a plain JSON
//! `POST {base}/systemone` — not chat completions — served on two routes:
//!
//! * **OpenRouter** — `{openrouter base}/systemone` with the user's OpenRouter
//!   key, built exactly like every other OpenRouter client.
//! * **TypeSafe direct** — `https://api.typesafe.ai/v1/systemone` with a
//!   TypeSafe key.
//!
//! This is a child of `client` so it reuses the client's auth headers, TLS,
//! redaction and bounded retry plumbing; there is no second HTTP client.
//!
//! Known limits (written down so nobody assumes them):
//! * TypeSafe is **not** an [`ApiProvider`]: it serves no chat route, so it is
//!   the decision router's own endpoint + key (`TYPESAFE_API_KEY`, the
//!   `typesafe` secret-store slot, or `[providers.typesafe] api_key` /
//!   `api_key_env`). Its spend is shown on the Auto receipt as the
//!   provider-reported cost; it is not entered in session cost totals.
//! * The routing call makes one attempt (no retry): it is bounded by the
//!   router timeout, and a retried decision would arrive after the turn has
//!   already fallen back.
//! * Only the `choice` answer shape is parsed; `score` / `noul` are later.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::value::RawValue;

use super::*;
use crate::model_routing::AutoRouterFailure;

/// TypeSafe's direct API base (the `/systemone` path is appended).
pub(crate) const TYPESAFE_DEFAULT_BASE_URL: &str = "https://api.typesafe.ai/v1";
/// Environment variable holding a TypeSafe API key.
pub(crate) const TYPESAFE_API_KEY_ENV: &str = "TYPESAFE_API_KEY";
/// Secret-store slot and `[providers.<name>]` table name for the TypeSafe key.
pub(crate) const TYPESAFE_KEY_NAME: &str = "typesafe";

/// Which endpoint serves a decision router.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DecisionRouterRoute {
    Openrouter,
    Typesafe,
}

impl DecisionRouterRoute {
    /// Parse an `[auto.router] provider` value for a decision router.
    #[must_use]
    pub(crate) fn parse(provider: &str) -> Option<Self> {
        let provider = provider.trim();
        if ApiProvider::parse(provider) == Some(ApiProvider::Openrouter) {
            Some(Self::Openrouter)
        } else if provider.eq_ignore_ascii_case(TYPESAFE_KEY_NAME)
            || provider.eq_ignore_ascii_case("typesafe-ai")
        {
            Some(Self::Typesafe)
        } else {
            None
        }
    }

    #[must_use]
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Openrouter => "openrouter",
            Self::Typesafe => TYPESAFE_KEY_NAME,
        }
    }

    #[must_use]
    pub(crate) fn display_name(self) -> &'static str {
        match self {
            Self::Openrouter => "OpenRouter",
            Self::Typesafe => "TypeSafe",
        }
    }

    /// Whether a credential for this route is present (never returns it).
    #[must_use]
    pub(crate) fn has_key(self, config: &Config) -> bool {
        match self {
            Self::Openrouter => crate::config::has_api_key_for(config, ApiProvider::Openrouter),
            Self::Typesafe => typesafe_api_key(config).is_some(),
        }
    }
}

/// Resolve the TypeSafe key: environment, then the durable secret store, then
/// `[providers.typesafe] api_key` / `api_key_env`.
pub(crate) fn typesafe_api_key(config: &Config) -> Option<String> {
    let non_empty = |value: String| (!value.trim().is_empty()).then(|| value.trim().to_string());
    if let Some(key) = std::env::var(TYPESAFE_API_KEY_ENV).ok().and_then(non_empty) {
        return Some(key);
    }
    if let Some(key) = crate::config::credential_secret_store()
        .and_then(|store| store.get(TYPESAFE_KEY_NAME).ok().flatten())
        .and_then(non_empty)
    {
        return Some(key);
    }
    let entry = config
        .providers
        .as_ref()
        .and_then(|providers| providers.custom_provider_config(TYPESAFE_KEY_NAME))?;
    entry.api_key.clone().and_then(non_empty).or_else(|| {
        entry
            .api_key_env
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .and_then(|name| std::env::var(name).ok())
            .and_then(non_empty)
    })
}

/// A decoded System One response. Unknown fields are ignored.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SystemOneResponse {
    #[serde(default)]
    pub(crate) id: Option<String>,
    #[serde(default)]
    pub(crate) model: Option<String>,
    #[serde(default)]
    pub(crate) answers: BTreeMap<String, SystemOneAnswer>,
    #[serde(default)]
    pub(crate) usage: Option<SystemOneUsage>,
}

/// One answer. Only the `choice` subset is interpreted.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SystemOneAnswer {
    #[serde(rename = "type", default)]
    pub(crate) kind: String,
    #[serde(default)]
    pub(crate) choice: Option<String>,
    #[serde(default)]
    pub(crate) probabilities: BTreeMap<String, Option<f64>>,
    #[serde(default)]
    pub(crate) confidence: Option<f64>,
}

/// Provider usage. OpenRouter adds `cost`; the token-count casing differs
/// between surfaces, so both spellings are accepted.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SystemOneUsage {
    /// Kept verbatim so the receipt never re-renders a float.
    #[serde(default)]
    pub(crate) cost: Option<Box<RawValue>>,
    #[serde(default, alias = "inputTokens")]
    pub(crate) input_tokens: u32,
    #[serde(default, alias = "outputTokens")]
    pub(crate) output_tokens: u32,
}

impl SystemOneUsage {
    /// The provider-reported cost as its verbatim JSON decimal, when it is a
    /// finite, non-negative number.
    #[must_use]
    pub(crate) fn reported_cost(&self) -> Option<String> {
        let raw = self.cost.as_ref()?.get().trim();
        let value: f64 = raw.parse().ok()?;
        (value.is_finite() && value >= 0.0).then(|| raw.to_string())
    }
}

impl CodewhaleClient {
    /// Build the client that serves `route`.
    ///
    /// OpenRouter is the ordinary OpenRouter client (its key, base URL,
    /// attribution headers). TypeSafe re-points a clone of the active route's
    /// client — keeping its retry, TLS and redaction policy — at the TypeSafe
    /// endpoint with the TypeSafe key only, and drops the active provider's
    /// concurrency permit so a routing call never holds a chat slot.
    pub(crate) fn for_decision_route(
        config: &Config,
        route: DecisionRouterRoute,
        base_url_override: Option<&str>,
    ) -> Result<Self> {
        match route {
            DecisionRouterRoute::Openrouter => {
                let mut scoped = config.clone();
                scoped.provider = Some(ApiProvider::Openrouter.as_str().to_string());
                // The decision model id is not a chat route; it travels in the
                // JSON body only.
                scoped.default_text_model = None;
                Self::new(&scoped)
            }
            DecisionRouterRoute::Typesafe => {
                let key = typesafe_api_key(config).with_context(|| {
                    format!("TypeSafe API key not configured ({TYPESAFE_API_KEY_ENV})")
                })?;
                let base_url = base_url_override
                    .map(str::trim)
                    .filter(|url| !url.is_empty())
                    .unwrap_or(TYPESAFE_DEFAULT_BASE_URL)
                    .trim_end_matches('/')
                    .to_string();
                validate_base_url_security(&base_url, false)?;
                let mut client = Self::new(config)?;
                client.http_client = Self::http_client_builder_with_auth_mode(
                    &key,
                    &HashMap::new(),
                    ApiProvider::Custom,
                    &base_url,
                    WireFormat::ChatCompletions,
                    false,
                    false,
                )?
                .build()?;
                client.base_url = base_url;
                // `Self::new` froze the redaction set from the chat provider's
                // secrets; the TypeSafe key is none of them, so add it before
                // any decision body is built from untrusted context.
                let mut secrets = client.model_bound_secret_values.as_ref().clone();
                push_model_bound_secret(&mut secrets, Some(&key));
                client.model_bound_secret_values = Arc::new(secrets);
                client.api_key = key;
                client.request_concurrency = None;
                client.remote_control_inference_participant = false;
                Ok(client)
            }
        }
    }

    /// `POST {base}/systemone` once, isolated like the Auto chat classifier:
    /// no global retry banners, no shared token bucket, no response cache.
    ///
    /// Only a failure class leaves this function — provider error bodies can
    /// echo the prompt and must never reach receipts.
    ///
    /// `dispatched` is set once both permits are held and the request is
    /// handed to the transport, so a caller whose deadline cancels this
    /// future can tell a possibly-billed request from one never sent.
    pub(crate) async fn system_one_decide(
        &self,
        body: &Value,
        dispatched: &std::sync::atomic::AtomicBool,
    ) -> std::result::Result<SystemOneResponse, AutoRouterFailure> {
        let mut isolated = self.clone();
        isolated.isolated_request_state = true;
        isolated.rate_limiter = Arc::new(AsyncMutex::new(TokenBucket::from_env()));
        isolated.retry.max_retries = 0;
        let _inference = isolated.acquire_remote_control_inference_permit().await;
        let _permit = isolated.acquire_provider_request_permit().await;
        let url = api_url(&isolated.base_url, "systemone");
        dispatched.store(true, std::sync::atomic::Ordering::Release);
        let response = isolated
            .send_json_with_retry(&url, body)
            .await
            .map_err(|error| router_failure_from_error(&error))?;
        let bytes = response
            .bytes()
            .await
            .map_err(|_| AutoRouterFailure::Transport)?;
        serde_json::from_slice(&bytes).map_err(|_| AutoRouterFailure::InvalidAnswer)
    }
}

/// Collapse a client error into a non-secret failure class. The HTTP status
/// is kept where the error carries it; the body never is.
pub(crate) fn router_failure_from_error(error: &anyhow::Error) -> AutoRouterFailure {
    let Some(error) = error.downcast_ref::<LlmError>() else {
        return AutoRouterFailure::Transport;
    };
    match error {
        LlmError::RateLimited { .. } => AutoRouterFailure::Http { status: 429 },
        LlmError::ServerError { status, .. } | LlmError::InvalidRequest { status, .. } => {
            AutoRouterFailure::Http { status: *status }
        }
        LlmError::AuthenticationError(_) => AutoRouterFailure::Http { status: 401 },
        LlmError::AuthorizationError(_) => AutoRouterFailure::Http { status: 403 },
        LlmError::QuotaExhausted(_) => AutoRouterFailure::QuotaExhausted,
        LlmError::ModelError(_)
        | LlmError::ContextLengthError(_)
        | LlmError::ContentPolicyError(_) => AutoRouterFailure::Rejected,
        LlmError::Other(message) => message
            .strip_prefix("HTTP ")
            .and_then(|rest| rest.split(':').next())
            .and_then(|status| status.trim().parse::<u16>().ok())
            .map_or(AutoRouterFailure::Transport, |status| {
                AutoRouterFailure::Http { status }
            }),
        LlmError::NetworkError(_) | LlmError::Timeout(_) | LlmError::ParseError(_) => {
            AutoRouterFailure::Transport
        }
    }
}
