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

/// Only the first-party account endpoint can receive an account device key.
/// This endpoint supplies a process-only reversible auth transform; it never
/// mutates a user's provider slot or durable config. Running turns retain their
/// materialized client: server-side device/session revocation is authoritative.
fn account_model_access_receipt(config: &crate::config::Config) -> Value {
    let api_base = config.base_url_for_route(ApiProvider::Codewhale);
    let supported = api_base.trim_end_matches('/') == crate::config::DEFAULT_CODEWHALE_BASE_URL
        && super::runtime_account_api_base()
            == codewhale_secrets::account::DEFAULT_ACCOUNT_API_BASE;
    let access = config.account_model_access.read().clone();
    let live = access
        .as_ref()
        .filter(|a| a.expires_at > chrono::Utc::now().timestamp());
    json!({
        "apiBase": if supported { api_base.as_str() } else { "" },
        "supported": supported,
        "configured": config.account_model_api_key(ApiProvider::Codewhale).is_some(),
        "catalogRefreshNeeded": false,
        "sessionId": live.map(|a| a.session_id.as_str()),
        "expiresAt": live.map(|a| a.expires_at),
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SetAccountModelAccessRequest {
    api_base: String,
    session_id: String,
    expected_session_id: Option<String>,
    key: String,
    expires_at: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ClearAccountModelAccessRequest {
    session_id: String,
}

pub(super) async fn get_account_model_access(
    State(state): State<RuntimeApiState>,
) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || {
        Ok(Json(account_model_access_receipt(&state.config.read())))
    })
    .await
    .map_err(|_| ApiError::internal("account access read failed"))?
}

pub(super) async fn set_account_model_access(
    State(state): State<RuntimeApiState>,
    Json(request): Json<SetAccountModelAccessRequest>,
) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || {
        let config = state.config.write();
        let refresh_needed =
            install_account_model_access(&config, state.config_profile.as_deref(), request)?;
        let mut receipt = account_model_access_receipt(&config);
        receipt["catalogRefreshNeeded"] = json!(refresh_needed);
        Ok(Json(receipt))
    })
    .await
    .map_err(|_| ApiError::internal("account access update failed"))?
}

fn install_account_model_access(
    config: &crate::config::Config,
    profile: Option<&str>,
    request: SetAccountModelAccessRequest,
) -> Result<bool, ApiError> {
    let expected_base = crate::config::DEFAULT_CODEWHALE_BASE_URL;
    if request.api_base.trim_end_matches('/') != expected_base
        || config
            .base_url_for_route(ApiProvider::Codewhale)
            .trim_end_matches('/')
            != expected_base
        || super::runtime_account_api_base() != codewhale_secrets::account::DEFAULT_ACCOUNT_API_BASE
    {
        return Err(ApiError::conflict(
            "Account model access requires the first-party Codewhale endpoint.",
        ));
    }
    if !request.key.starts_with("cwc_")
        || request.key.len() < 32
        || request.key.len() > MAX_KEY_BYTES
        || request
            .key
            .chars()
            .any(|c| c.is_control() || c.is_whitespace())
    {
        return Err(ApiError::bad_request("Invalid account device credential."));
    }
    let now = chrono::Utc::now();
    let secrets = codewhale_secrets::account::secure_account_session_secrets()
        .map_err(|_| ApiError::internal("Account session storage is unavailable."))?;
    let account = codewhale_secrets::account::AccountSessionStore::new(
        secrets,
        profile,
        codewhale_secrets::account::DEFAULT_ACCOUNT_API_BASE,
    )
    .runtime_info_at(now)
    .map_err(|_| ApiError::conflict("Sign in again before connecting account models."))?;
    let session_expiry = account
        .expires_at
        .as_deref()
        .and_then(|date| chrono::DateTime::parse_from_rfc3339(date).ok())
        .map(|date| date.timestamp());
    if account.state != codewhale_secrets::account::AccountSessionState::Authenticated
        || account.session_id.as_deref() != Some(request.session_id.as_str())
        || request.expires_at <= now.timestamp()
        || session_expiry.is_none_or(|expiry| request.expires_at > expiry)
    {
        return Err(ApiError::conflict(
            "Account session changed or expired; sign in again.",
        ));
    }
    // Probe the existing resolver without the account layer. Scope to Codewhale
    // so even an inactive unmarked durable slot is checked. Never replace it.
    let mut existing = config.clone();
    existing.account_model_access = Default::default();
    existing.provider = Some("codewhale".into());
    let stored_key_present = match crate::config::credential_secret_store() {
        Some(secrets) => secrets
            .get("codewhale")
            .map_err(|_| {
                ApiError::conflict("The existing Codewhale credential could not be checked.")
            })?
            .is_some(),
        None => false,
    };
    if stored_key_present
        || existing
            .provider_config_for(ApiProvider::Codewhale)
            .is_some_and(|entry| entry.auth.is_some() || entry.api_key_env.is_some())
        || !credential_writeability(&existing, ApiProvider::Codewhale).writable
        || crate::config::has_api_key_for(&existing, ApiProvider::Codewhale)
    {
        return Err(ApiError::conflict(
            "This Codewhale route already has its own credential.",
        ));
    }
    let mut access = config.account_model_access.write();
    let owner = access
        .as_ref()
        .filter(|a| a.expires_at > now.timestamp())
        .map(|a| a.session_id.as_str());
    if owner != request.expected_session_id.as_deref() {
        return Err(ApiError::conflict(
            "Account model access changed; refresh before retrying.",
        ));
    }
    let changed = access.as_ref().is_none_or(|current| {
        current.session_id != request.session_id
            || current.profile.as_deref() != profile
            || current.credential.expose_secret() != request.key
    });
    if changed {
        invalidate_account_catalog(config);
    }
    *access = Some(crate::config::AccountModelAccess {
        session_id: request.session_id,
        credential: crate::credentials::Credential::ApiKey { key: request.key },
        expires_at: request.expires_at,
        profile: profile.map(str::to_string),
    });
    Ok(changed)
}

pub(super) async fn clear_account_model_access(
    State(state): State<RuntimeApiState>,
    Json(request): Json<ClearAccountModelAccessRequest>,
) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || {
        let config = state.config.write();
        remove_account_model_access(&config, &request.session_id)?;
        Ok(Json(account_model_access_receipt(&config)))
    })
    .await
    .map_err(|_| ApiError::internal("account access removal failed"))?
}

fn remove_account_model_access(
    config: &crate::config::Config,
    session_id: &str,
) -> Result<(), ApiError> {
    let mut access = config.account_model_access.write();
    if access.as_ref().is_some_and(|a| a.session_id != session_id) {
        return Err(ApiError::conflict(
            "Account model access belongs to a different session.",
        ));
    }
    if access.is_some() {
        invalidate_account_catalog(config);
    }
    *access = None;
    Ok(())
}

// Beginning a generation already invalidates this account-only memory roster
// and all older in-flight tickets. No network request or second cache is needed.
fn invalidate_account_catalog(config: &crate::config::Config) {
    crate::provider_catalog_live::begin_refresh_for_identity(
        ApiProvider::Codewhale,
        "codewhale",
        &config.base_url_for_route(ApiProvider::Codewhale),
    );
}

pub(super) fn invalidate_stale_account_catalog(config: &crate::config::Config) {
    let bound = config.account_model_access.read().is_some();
    if bound
        && config
            .account_model_api_key(ApiProvider::Codewhale)
            .is_none()
    {
        invalidate_account_catalog(config);
    }
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
    /// The expiring account transform; disconnect through account sign-out.
    AccountSession,
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
    let account_bound = config.account_model_access.read().is_some();
    if account_bound
        && matches!(
            crate::config::resolve_credential_source(config, provider).source,
            crate::credentials::CredentialSource::AccountSession
        )
    {
        return CredentialWriteability {
            source: ProviderCredentialSource::AccountSession,
            writable: false,
            reason: Some("This route uses your Codewhale account. Sign out to disconnect it."),
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

    fn saved_account_session(session_id: &str) -> codewhale_secrets::account::AccountSessionStore {
        let store = codewhale_secrets::account::AccountSessionStore::new(
            codewhale_secrets::account::secure_account_session_secrets().unwrap(),
            None,
            codewhale_secrets::account::DEFAULT_ACCOUNT_API_BASE,
        );
        store
            .save(codewhale_secrets::account::AccountAuthBundle {
                token_type: "Bearer".into(),
                access_token: "fixture-access-token-for-account-models".into(),
                refresh_token: "fixture-refresh-token-for-account-models".into(),
                session: Some(codewhale_secrets::account::AccountSession {
                    id: session_id.into(),
                    status: "active".into(),
                    expires_at: (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339(),
                    refresh_expires_at: (chrono::Utc::now() + chrono::Duration::days(1))
                        .to_rfc3339(),
                    ..Default::default()
                }),
                user: Some(codewhale_secrets::account::AccountUser {
                    id: "fixture-user".into(),
                    ..Default::default()
                }),
            })
            .unwrap();
        store
    }

    fn account_request(owner: Option<&str>) -> SetAccountModelAccessRequest {
        SetAccountModelAccessRequest {
            api_base: crate::config::DEFAULT_CODEWHALE_BASE_URL.into(),
            session_id: "fixture-session".into(),
            expected_session_id: owner.map(str::to_string),
            key: "cwc_fixture_device_credential_not_real".into(),
            expires_at: chrono::Utc::now().timestamp() + 1800,
        }
    }

    #[test]
    fn account_overlay_resolves_without_disk_residue_and_logout_revokes_clones() {
        let _env = crate::test_support::lock_test_env();
        let tmp = tempfile::tempdir().unwrap();
        let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", tmp.path());
        let _backend = crate::test_support::EnvVarGuard::set("CODEWHALE_SECRET_BACKEND", "file");
        let _base = crate::test_support::EnvVarGuard::set(
            "CODEWHALE_CLOUD_API_BASE",
            codewhale_secrets::account::DEFAULT_ACCOUNT_API_BASE,
        );
        let _api_base = crate::test_support::EnvVarGuard::set(
            "CODEWHALE_API_BASE",
            crate::config::DEFAULT_CODEWHALE_BASE_URL,
        );
        let _api_key = crate::test_support::EnvVarGuard::set("CODEWHALE_API_KEY", "");
        let store = saved_account_session("fixture-session");
        let config = Config {
            provider: Some("codewhale".into()),
            ..Default::default()
        };
        let cloned_before_install = config.clone();
        assert!(install_account_model_access(&config, None, account_request(None)).unwrap());
        assert_eq!(
            cloned_before_install
                .active_route_api_key_read_only()
                .unwrap(),
            "cwc_fixture_device_credential_not_real"
        );
        assert!(crate::config::has_api_key_for(
            &config,
            ApiProvider::Codewhale
        ));
        assert!(!format!("{config:?}").contains("cwc_fixture"));
        assert!(
            !account_model_access_receipt(&config)
                .to_string()
                .contains("cwc_fixture")
        );
        assert!(config.provider_config_for(ApiProvider::Codewhale).is_none());
        assert!(
            crate::config::credential_secret_store()
                .unwrap()
                .get("codewhale")
                .unwrap()
                .is_none()
        );
        use codewhale_config::catalog::{
            CatalogStatus, ProviderCatalogDelta, base_url_fingerprint,
        };
        let endpoint = crate::config::DEFAULT_CODEWHALE_BASE_URL;
        let ticket = crate::provider_catalog_live::begin_refresh_for_identity(
            ApiProvider::Codewhale,
            "codewhale",
            endpoint,
        );
        let delta = || ProviderCatalogDelta {
            provider: "codewhale".into(),
            base_url_fingerprint: base_url_fingerprint(endpoint),
            fetched_at: 1,
            offerings: vec![],
        };
        assert_eq!(
            crate::provider_catalog_live::record_success_if_current(&ticket, delta()),
            Some(CatalogStatus::Fresh)
        );
        assert!(
            !install_account_model_access(&config, None, account_request(Some("fixture-session")))
                .unwrap()
        );
        assert!(
            crate::provider_catalog_live::cached_entry_for_route(
                ApiProvider::Codewhale,
                "codewhale",
                endpoint
            )
            .unwrap()
            .is_some()
        );
        let mut extended = account_request(Some("fixture-session"));
        extended.expires_at += 60;
        assert!(!install_account_model_access(&config, None, extended).unwrap());
        assert_eq!(
            crate::provider_catalog_live::record_success_if_current(&ticket, delta()),
            Some(CatalogStatus::Fresh)
        );
        store.clear().unwrap();
        invalidate_stale_account_catalog(&config);
        assert!(
            crate::provider_catalog_live::cached_entry_for_route(
                ApiProvider::Codewhale,
                "codewhale",
                endpoint
            )
            .unwrap()
            .is_none()
        );
        assert_eq!(
            crate::provider_catalog_live::record_success_if_current(&ticket, delta()),
            None
        );
        assert!(
            cloned_before_install
                .active_route_api_key_read_only()
                .is_err()
        );
        assert!(
            install_account_model_access(&config, None, account_request(Some("fixture-session")))
                .is_err()
        );
    }

    #[test]
    fn account_overlay_refuses_endpoint_credentials_and_stale_session_ownership() {
        let _env = crate::test_support::lock_test_env();
        let tmp = tempfile::tempdir().unwrap();
        let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", tmp.path());
        let _backend = crate::test_support::EnvVarGuard::set("CODEWHALE_SECRET_BACKEND", "file");
        let _base = crate::test_support::EnvVarGuard::set(
            "CODEWHALE_CLOUD_API_BASE",
            codewhale_secrets::account::DEFAULT_ACCOUNT_API_BASE,
        );
        let _api_base = crate::test_support::EnvVarGuard::set(
            "CODEWHALE_API_BASE",
            crate::config::DEFAULT_CODEWHALE_BASE_URL,
        );
        let _api_key = crate::test_support::EnvVarGuard::set("CODEWHALE_API_KEY", "");
        saved_account_session("fixture-session");
        let mut config = Config {
            provider: Some("codewhale".into()),
            ..Default::default()
        };
        config
            .provider_config_for_mut(ApiProvider::Codewhale)
            .base_url = Some("https://api.codewhale.net/other/v1".into());
        assert!(install_account_model_access(&config, None, account_request(None)).is_err());
        config
            .provider_config_for_mut(ApiProvider::Codewhale)
            .base_url = None;
        config
            .provider_config_for_mut(ApiProvider::Codewhale)
            .api_key = Some("existing-user-key".into());
        assert!(install_account_model_access(&config, None, account_request(None)).is_err());
        config
            .provider_config_for_mut(ApiProvider::Codewhale)
            .api_key = None;
        let mut expired = account_request(None);
        expired.expires_at = chrono::Utc::now().timestamp() - 1;
        assert!(install_account_model_access(&config, None, expired).is_err());
        install_account_model_access(&config, None, account_request(None)).unwrap();
        assert!(install_account_model_access(&config, None, account_request(None)).is_err());
        install_account_model_access(&config, None, account_request(Some("fixture-session")))
            .unwrap();
        assert!(remove_account_model_access(&config, "stale-session").is_err());
        assert!(
            config
                .account_model_api_key(ApiProvider::Codewhale)
                .is_some()
        );
        config
            .provider_config_for_mut(ApiProvider::Codewhale)
            .api_key = Some("new-user-key".into());
        assert_eq!(
            config.active_route_api_key_read_only().unwrap(),
            "new-user-key"
        );
        config
            .provider_config_for_mut(ApiProvider::Codewhale)
            .api_key = None;
        config
            .provider_config_for_mut(ApiProvider::Codewhale)
            .base_url = Some("https://api.codewhale.net/other/v1".into());
        assert!(
            config
                .account_model_api_key(ApiProvider::Codewhale)
                .is_none()
        );
    }

    #[test]
    fn account_overlay_clear_is_shared_and_preserves_manual_config() {
        let _env = crate::test_support::lock_test_env();
        let mut config = Config::default();
        config
            .provider_config_for_mut(ApiProvider::Codewhale)
            .api_key = Some("manual-key".into());
        *config.account_model_access.write() = Some(crate::config::AccountModelAccess {
            session_id: "current".into(),
            credential: crate::credentials::Credential::ApiKey {
                key: "cwc_fixture".into(),
            },
            expires_at: chrono::Utc::now().timestamp() + 60,
            profile: None,
        });
        let clone = config.clone();
        assert!(remove_account_model_access(&config, "stale").is_err());
        assert!(clone.account_model_access.read().is_some());
        remove_account_model_access(&config, "current").unwrap();
        assert!(clone.account_model_access.read().is_none());
        assert_eq!(
            config
                .provider_config_for(ApiProvider::Codewhale)
                .unwrap()
                .api_key
                .as_deref(),
            Some("manual-key")
        );
    }

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
