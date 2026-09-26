//! Canonical provider-credential writes shared by the CLI (`auth set`),
//! the runtime API secret route, and any future host. Owning this here keeps
//! every writer on the same transactional discipline: snapshot the prior
//! secret, write the durable backend, refuse plaintext config fallback, and
//! roll both stores back if either leg fails.

use anyhow::{Context, Result};

use crate::provider_kind::ProviderKind;
use crate::{ConfigStore, Secrets};

/// Resolve the store for credential-adjacent writes: provider selection,
/// `auth_mode` markers, and the plaintext-free metadata that accompanies a
/// saved key.
///
/// Credentials and their metadata are user-global — a key saved while
/// working in one repo must be visible from every other repo, and the secret
/// store already is. When the ambient config path is a workspace-scoped
/// document (`<repo>/.codewhale/config.toml`), credential writes must not
/// bind the provider or write auth markers there: the binding would be
/// invisible from every other repo and would invite plaintext keys into a
/// committable repo file. Returns a store loaded on the user-global document
/// in that case, or `None` when the ambient store is already correctly
/// scoped, so key + provider binding + auth markers share one user-global
/// scope by default.
pub fn credential_metadata_store(store: &ConfigStore) -> Result<Option<ConfigStore>> {
    if !crate::config_path_is_workspace_scoped(store.path()) {
        return Ok(None);
    }
    let global = crate::default_config_path()?;
    ConfigStore::load(Some(global)).map(Some)
}

/// The secret-store slot a provider's key occupies. Shared-account families
/// (SiliconFlow China, the Model Studio variants) collapse onto one slot;
/// see [`ProviderKind::secret_store_slot`].
#[must_use]
pub fn provider_slot(provider: ProviderKind) -> &'static str {
    provider.secret_store_slot()
}

/// Remove any plaintext `api_key` left in the config for `provider`.
pub fn clear_provider_api_key_from_config(store: &mut ConfigStore, provider: ProviderKind) {
    store.config.providers.for_provider_mut(provider).api_key = None;
}

/// Plaintext-free metadata that accompanies a saved key.
///
/// Saving a credential never writes a model: every provider resolves an
/// unset model to its own default, and model choice belongs to the
/// model/config commands.
pub fn prepare_provider_api_key_metadata(store: &mut ConfigStore, provider: ProviderKind) {
    store.config.auth_mode = Some("api_key".to_string());
    let provider_config = store.config.providers.for_provider_mut(provider);
    provider_config.auth_mode = Some("api_key".to_string());
    provider_config.external_credentials = None;
    if provider == ProviderKind::Xai {
        provider_config.oauth_credential_generation = None;
    }
}

/// Persist a provider credential to the durable secret store without silently
/// downgrading a backend failure to plaintext config storage.
///
/// Returns `true` when the key landed in the secret store (config then holds
/// metadata only). Callers must not print or echo `api_key`.
pub fn set_provider_api_key(
    store: &mut ConfigStore,
    secrets: &Secrets,
    provider: ProviderKind,
    api_key: &str,
) -> Result<bool> {
    // #6528: strip pasted invisible characters and whitespace in one place.
    let api_key = codewhale_secrets::normalize_api_key(api_key);
    anyhow::ensure!(!api_key.is_empty(), "Refusing to save an empty API key.");
    let api_key = api_key.as_str();
    if provider == ProviderKind::Xai {
        return crate::with_xai_oauth_revocation_transaction(|| {
            set_provider_api_key_unlocked(store, secrets, provider, api_key)
        });
    }
    set_provider_api_key_unlocked(store, secrets, provider, api_key)
}

fn set_provider_api_key_unlocked(
    store: &mut ConfigStore,
    secrets: &Secrets,
    provider: ProviderKind,
    api_key: &str,
) -> Result<bool> {
    let original_config = store.config.clone();
    prepare_provider_api_key_metadata(store, provider);
    let slot = provider_slot(provider);
    // A readable prior value is required before a secret-store write so a
    // later config failure can restore the exact prior state. If the backend
    // cannot provide that snapshot, fail before changing the config file.
    let prior_secret = secrets.get(slot);
    let secret_store_saved = match prior_secret.as_ref().map_err(|error| error.to_string()) {
        Ok(_) => match secrets.set(slot, api_key) {
            Ok(()) => {
                clear_provider_api_key_from_config(store, provider);
                true
            }
            Err(err) => {
                store.config = original_config;
                return Err(anyhow::anyhow!(
                    "Secret storage write failed for {slot}: {err}. Refusing to write the API key in plaintext to {}. Fix the configured secret backend and retry; Codewhale did not change that file.",
                    crate::quote_os_path(store.path())
                ));
            }
        },
        Err(error) => {
            store.config = original_config;
            return Err(anyhow::anyhow!(
                "Secret storage snapshot failed for {slot}: {error}. Refusing to write the API key in plaintext to {}. Fix the configured secret backend and retry; Codewhale did not change that file.",
                crate::quote_os_path(store.path())
            ));
        }
    };
    if let Err(error) = store.save() {
        store.config = original_config;
        if secret_store_saved {
            let current = secrets
                .get(slot)
                .map_err(|rollback| anyhow::anyhow!(
                    "{error}; additionally could not verify secret-store rollback for {slot}: {rollback}"
                ))?;
            if current.as_deref() == Some(api_key) {
                match prior_secret.expect("snapshot succeeded before secret write") {
                    Some(previous) => secrets.set(slot, &previous),
                    None => secrets.delete(slot),
                }
                .map_err(|rollback| anyhow::anyhow!(
                    "{error}; additionally failed to restore prior secret-store state for {slot}: {rollback}"
                ))?;
            }
        }
        return Err(error);
    }
    crate::scrub_plaintext_api_keys_from_config_backup(store.path())
        .context("failed to scrub plaintext API keys from config backup")?;
    Ok(secret_store_saved)
}

/// What a credential clear actually accomplished.
///
/// The secret-store leg can fail after the config leg has already been
/// persisted. Reporting that separately is the point: a caller that prints
/// "cleared" while the key is still sitting in the keyring has lied about a
/// security-relevant action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClearOutcome {
    /// The secret-store slot the clear targeted.
    pub slot: &'static str,
    /// `None` when the secret store accepted the delete; otherwise the backend
    /// error, already stringified so it carries no credential material.
    pub secret_store_error: Option<String>,
}

impl ClearOutcome {
    /// True only when both the config and the secret store were cleared.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.secret_store_error.is_none()
    }
}

/// Remove a provider credential from config and the durable secret store.
///
/// Shared by `codewhale auth clear` and the runtime API's credential route so
/// both get the same ordering and the same rollback: the config document is
/// snapshotted and restored if its save fails, and the secret store is only
/// touched once the config write has landed. A secret-store failure is
/// returned rather than swallowed, because the config no longer advertises a
/// key that the backend may still hold.
///
/// This deliberately does not clear external-consent or environment-sourced
/// credentials: Codewhale does not own those, and a caller must refuse the
/// request instead of implying it revoked something it cannot reach.
pub fn clear_provider_api_key(
    store: &mut ConfigStore,
    secrets: &Secrets,
    provider: ProviderKind,
) -> Result<ClearOutcome> {
    let slot = provider_slot(provider);
    let original_config = store.config.clone();
    clear_provider_api_key_from_config(store, provider);
    // Only xAI carries OAuth generation and consent state alongside the key,
    // and `codewhale auth clear` has always cleared those three together. Every
    // other provider keeps its `auth_mode` marker deliberately: the route is
    // still an API-key route, it simply has no key now, which is exactly the
    // `missing` credential state a client needs to see.
    if provider == ProviderKind::Xai {
        let xai = store.config.providers.for_provider_mut(provider);
        xai.oauth_credential_generation = None;
        xai.auth_mode = None;
        xai.external_credentials = None;
    }
    if let Err(error) = store.save() {
        store.config = original_config;
        return Err(error);
    }
    let secret_store_error = secrets.delete(slot).err().map(|error| error.to_string());
    Ok(ClearOutcome {
        slot,
        secret_store_error,
    })
}
