use axum::Json;
use axum::extract::{Path, State};
use codewhale_config::ConfigStore;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::config::ApiProvider;

use super::{ApiError, ProviderCredentialState, RuntimeApiState};

/// Largest accepted credential payload. Provider keys are single-line
/// tokens; anything larger is a mistake, not a longer secret.
const MAX_KEY_BYTES: usize = 4 * 1024;

/// Request body cap for the key route — the key plus JSON framing.
pub(super) const PROVIDER_KEY_BODY_LIMIT_BYTES: usize = MAX_KEY_BYTES + 1024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SetProviderKeyRequest {
    key: String,
}

/// Write-only credential entry for native clients (APPS-48).
///
/// `PUT /v1/providers/{id}/key` accepts `{ "key": "…" }`, persists it through
/// the same transactional path as `codewhale auth set` (secret backend plus
/// plaintext-free config metadata, rolled back together on failure), and
/// answers with the redacted receipt: which backend holds the secret and the
/// post-write `credential_state` readback. The key itself — and even its
/// length — never appears in the response, in errors, or in logs.
///
/// There is deliberately no GET: a route that can return a secret can leak
/// one. Clients needing assurance re-read `credential_state` here or on
/// `GET /v1/providers`.
pub(super) async fn set_provider_key(
    State(state): State<RuntimeApiState>,
    Path(id): Path<String>,
    Json(request): Json<SetProviderKeyRequest>,
) -> Result<Json<Value>, ApiError> {
    // Shared with the clear route: unknown id, legacy alias, no credential
    // slot, and — the half #6179 was missing — a credential this route does
    // not own, which must refuse before the write rather than appear to
    // succeed against a source that still wins at request time.
    let (provider, kind) = writable_provider(&state, &id)?;

    let key = request.key;
    let key = key.trim();
    if key.is_empty() {
        return Err(ApiError::bad_request("key must not be empty"));
    }
    if key.len() > MAX_KEY_BYTES || key.chars().any(char::is_control) {
        return Err(ApiError::bad_request(
            "key must be a single-line credential at most 4 KiB",
        ));
    }

    let secrets = crate::config::credential_secret_store().ok_or_else(|| {
        ApiError::internal("no credential store is available in this environment")
    })?;

    let store_path = state.config_path.clone();
    let kind_owned = kind;
    let key_owned = key.to_string();
    let provider_owned = provider;
    let (backend, saved_config_path) = tokio::task::spawn_blocking(move || {
        let mut store = ConfigStore::load(store_path)
            .map_err(|error| ApiError::internal(format!("config store unavailable: {error}")))?;
        let mut credential_store = codewhale_config::credentials::credential_metadata_store(&store)
            .map_err(|error| ApiError::internal(format!("credential store: {error}")))?;
        let target = credential_store.as_mut().unwrap_or(&mut store);
        let slot = codewhale_config::credentials::provider_slot(kind_owned);
        crate::credentials::store::with_provider_write_lock(slot, || {
            codewhale_config::credentials::set_provider_api_key(
                target, &secrets, kind_owned, &key_owned,
            )
        })
        .map_err(|error| {
            // The credential-write errors name paths and backends only — the
            // key material is never embedded in the message.
            ApiError::internal(format!("credential write failed: {error}"))
        })?;
        Ok::<_, ApiError>((
            secrets.backend_name().to_string(),
            target.path().to_path_buf(),
        ))
    })
    .await
    .map_err(|_| ApiError::internal("credential write task failed"))??;

    // Mirror the persisted credential markers into the live config. The
    // durable write may have landed on the user-global document while this
    // server's ambient config is workspace-scoped, and `credential_state`
    // only probes the secret store for an inactive provider when the
    // `auth_mode` save marker is visible — without this mirror
    // `GET /v1/providers` would keep reporting the provider as missing its
    // credential until the next process start. Only marker fields are
    // mirrored; the key itself never enters the runtime config.
    {
        let mut config = state.config.write();
        config.auth_mode = Some("api_key".to_string());
        {
            let entry = config.provider_config_for_mut(provider_owned);
            entry.auth_mode = Some("api_key".to_string());
            entry.external_credentials = None;
            entry.api_key = None;
            if provider_owned == ApiProvider::Xai {
                entry.oauth_credential_generation = None;
            }
        }
        if provider_owned == ApiProvider::Deepseek {
            config.api_key = None;
            if config.default_text_model.is_none() {
                config.default_text_model = config
                    .provider_config_for(ApiProvider::Deepseek)
                    .and_then(|entry| entry.model.clone())
                    .or_else(|| Some("deepseek-v4-pro".to_string()));
            }
        }
    }

    let credential_state: ProviderCredentialState =
        crate::provider_readiness::credential_state_for_provider(
            &state.config.read(),
            provider_owned,
        )
        .into();

    Ok(Json(json!({
        "provider": provider_owned.as_str(),
        "stored": true,
        "backend": backend,
        "credentialState": credential_state,
        "configPath": saved_config_path,
    })))
}

/// Where the credential for a route comes from, as a *class* and never as a
/// value, a path, or an environment variable name.
///
/// This exists so a client can disable its own credential control with a
/// truthful reason before submitting, instead of letting a write fail late or —
/// worse — appear to succeed against a source Codewhale does not own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ProviderCredentialSource {
    /// Codewhale's own durable secret backend. The only writable source.
    SecretStore,
    /// A literal value sitting in a config document.
    Config,
    /// An external consent or auth-command source (OAuth, `auth_source`).
    ExternalAuth,
    /// The route takes no credential at all.
    None,
}

/// Whether this route's credential can be written through the runtime API, and
/// the reason when it cannot. The reason is user-facing copy.
pub(super) struct CredentialWriteability {
    pub(super) source: ProviderCredentialSource,
    pub(super) writable: bool,
    pub(super) reason: Option<&'static str>,
}

/// Classify a route's credential ownership without reading any credential.
///
/// Deliberately structural: it consults declared auth mode, consent state and
/// the *kind* of any configured `api_key` value, and never resolves a secret,
/// an environment value, or an auth command.
pub(super) fn credential_writeability(
    config: &crate::config::Config,
    provider: ApiProvider,
) -> CredentialWriteability {
    let auth_mode = config.auth_mode_for_provider(provider);
    if codewhale_config::auth_mode_disables_api_key(auth_mode.as_deref()) {
        return CredentialWriteability {
            source: ProviderCredentialSource::None,
            writable: false,
            reason: Some("This route is configured to send no credential."),
        };
    }
    if provider.kind().is_none() {
        return CredentialWriteability {
            source: ProviderCredentialSource::None,
            writable: false,
            reason: Some("This route has no credential slot."),
        };
    }
    // An active external consent owns the credential. Overwriting the key slot
    // would not change what the route sends, so a write here must refuse
    // rather than report a success the user cannot observe.
    if config
        .external_credential_consent_status(provider)
        .is_some_and(|status| status.route_state == "active")
    {
        return CredentialWriteability {
            source: ProviderCredentialSource::ExternalAuth,
            writable: false,
            reason: Some(
                "This route signs in through an external consent. Sign out of it before setting a key.",
            ),
        };
    }
    // A literal key in a config document is a plaintext credential Codewhale
    // did not put there. Writing the secret store would leave the literal in
    // place and still winning, so refuse and name the file-owned source.
    if let Some(entry) = config.provider_config_for(provider)
        && let Some(existing) = entry.api_key.as_deref()
        && codewhale_config::classify_config_api_key_value(existing)
            == codewhale_config::ConfigApiKeyValueKind::Literal
    {
        return CredentialWriteability {
            source: ProviderCredentialSource::Config,
            writable: false,
            reason: Some(
                "This route's key is set literally in a config file. Remove it there before managing it here.",
            ),
        };
    }
    CredentialWriteability {
        source: ProviderCredentialSource::SecretStore,
        writable: true,
        reason: None,
    }
}

/// Shared provider validation for both credential routes.
fn writable_provider(
    state: &RuntimeApiState,
    id: &str,
) -> Result<(ApiProvider, codewhale_config::ProviderKind), ApiError> {
    let provider = ApiProvider::parse(id)
        .ok_or_else(|| ApiError::bad_request(format!("Unknown provider id '{id}'")))?;
    if provider == ApiProvider::DeepseekCN {
        return Err(ApiError::bad_request(
            "provider 'deepseek-cn' is a legacy alias; use 'deepseek' instead",
        ));
    }
    let kind = provider
        .kind()
        .ok_or_else(|| ApiError::bad_request("provider has no credential slot"))?;
    let writeability = credential_writeability(&state.config.read(), provider);
    if !writeability.writable {
        return Err(ApiError::conflict(
            writeability
                .reason
                .unwrap_or("This route's credential is not managed by Codewhale."),
        ));
    }
    Ok((provider, kind))
}

/// `DELETE /v1/providers/{id}/key` — remove a Codewhale-owned credential.
///
/// Refuses for exactly the sources `PUT` refuses for, and for the same reason:
/// a route that reports "cleared" for a credential it cannot reach has lied
/// about a security action. The secret-store leg is reported separately,
/// because the config write lands first and the backend can still refuse.
pub(super) async fn clear_provider_key(
    State(state): State<RuntimeApiState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let (provider, kind) = writable_provider(&state, &id)?;

    let secrets = crate::config::credential_secret_store().ok_or_else(|| {
        ApiError::internal("no credential store is available in this environment")
    })?;

    let store_path = state.config_path.clone();
    let outcome = tokio::task::spawn_blocking(move || {
        let mut store = ConfigStore::load(store_path)
            .map_err(|error| ApiError::internal(format!("config store unavailable: {error}")))?;
        let mut credential_store = codewhale_config::credentials::credential_metadata_store(&store)
            .map_err(|error| ApiError::internal(format!("credential store: {error}")))?;
        let target = credential_store.as_mut().unwrap_or(&mut store);
        let slot = codewhale_config::credentials::provider_slot(kind);
        crate::credentials::store::with_provider_write_lock(slot, || {
            codewhale_config::credentials::clear_provider_api_key(target, &secrets, kind)
        })
        .map_err(|error| {
            // Clear errors name slots and paths only; no key material can
            // reach this message because none was read.
            ApiError::internal(format!("credential clear failed: {error}"))
        })
    })
    .await
    .map_err(|_| ApiError::internal("credential clear task failed"))??;

    // Mirror the cleared markers into the live config for the same reason the
    // write path mirrors them: the durable clear may have landed on the
    // user-global document while this server's ambient config is
    // workspace-scoped, and `credential_state` would otherwise keep reporting
    // the provider as configured until the next process start.
    {
        let mut config = state.config.write();
        let entry = config.provider_config_for_mut(provider);
        entry.api_key = None;
        if provider == ApiProvider::Xai {
            entry.auth_mode = None;
            entry.external_credentials = None;
            entry.oauth_credential_generation = None;
        }
        if provider == ApiProvider::Deepseek {
            config.api_key = None;
        }
    }

    let credential_state: ProviderCredentialState =
        crate::provider_readiness::credential_state_for_provider(&state.config.read(), provider)
            .into();

    if let Some(error) = outcome.secret_store_error {
        return Err(ApiError::internal(format!(
            "the config entry was cleared, but the secret store refused to delete {}: {error}",
            outcome.slot
        )));
    }

    Ok(Json(json!({
        "provider": provider.as_str(),
        "cleared": true,
        "credentialState": credential_state,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    /// A route Codewhale owns is writable, and says its source is the store it
    /// would actually write.
    #[test]
    fn a_codewhale_owned_route_is_writable_through_the_secret_store() {
        let config = Config::default();
        let writeability = credential_writeability(&config, ApiProvider::Openai);
        assert_eq!(writeability.source, ProviderCredentialSource::SecretStore);
        assert!(writeability.writable);
        assert!(writeability.reason.is_none());
    }

    /// The case #6179 exists for: a literal key in a config file still wins at
    /// request time, so a write here must refuse rather than report a success
    /// the user cannot observe. The reason names the file-owned source.
    #[test]
    fn a_literal_config_key_refuses_the_write_and_says_why() {
        let mut config = Config::default();
        config.provider_config_for_mut(ApiProvider::Openai).api_key =
            Some("sk-literal-in-a-config-file".to_string());

        let writeability = credential_writeability(&config, ApiProvider::Openai);
        assert_eq!(writeability.source, ProviderCredentialSource::Config);
        assert!(!writeability.writable);
        let reason = writeability.reason.expect("a refusal must name its reason");
        assert!(reason.contains("config file"), "{reason}");
        // The reason is copy, not a credential: it can never carry the value.
        assert!(!reason.contains("sk-literal-in-a-config-file"));
    }

    /// The secret-store sentinel is routing metadata, not a credential, so it
    /// must not be mistaken for a file-owned literal and refused.
    #[test]
    fn the_secret_store_sentinel_is_not_a_file_owned_key() {
        let mut config = Config::default();
        config.provider_config_for_mut(ApiProvider::Openai).api_key =
            Some(codewhale_config::API_KEYRING_SENTINEL.to_string());

        let writeability = credential_writeability(&config, ApiProvider::Openai);
        assert_eq!(writeability.source, ProviderCredentialSource::SecretStore);
        assert!(writeability.writable);
    }

    /// A route declared to send no credential has nothing to manage, and says
    /// so instead of offering a control that would do nothing.
    #[test]
    fn a_no_auth_route_reports_no_credential_source() {
        let mut config = Config::default();
        config
            .provider_config_for_mut(ApiProvider::Openai)
            .auth_mode = Some("none".to_string());

        let writeability = credential_writeability(&config, ApiProvider::Openai);
        assert_eq!(writeability.source, ProviderCredentialSource::None);
        assert!(!writeability.writable);
        assert!(writeability.reason.is_some());
    }
}
