//! Account-scoped roster for official Sign in with ChatGPT plan use.
//!
//! This replaces external Codex CLI cache/app-server discovery. Network access
//! uses the existing Codewhale provider client; only secret-free model metadata
//! is cached, keyed by the verified issuer, issued client ID, and subject.
//! A missing account roster offers no models. Public catalog rows and legacy
//! Codex credentials never prove permission to use a ChatGPT plan.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::config::{Config, ProviderKind};

const MAX_MODEL_CACHE_BYTES: u64 = 4 * 1024 * 1024;
const MODEL_CACHE_MAX_AGE: Duration = Duration::hours(24);
const MAX_FUTURE_CLOCK_SKEW: Duration = Duration::minutes(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CodexModelCacheFreshness {
    Fresh,
    Missing,
    Stale,
    Invalid,
}

impl CodexModelCacheFreshness {
    #[must_use]
    pub(crate) const fn picker_label(self) -> &'static str {
        match self {
            Self::Fresh => "ChatGPT OAuth",
            Self::Missing => "OAuth roster missing",
            Self::Stale => "OAuth roster stale",
            Self::Invalid => "OAuth roster invalid",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CodexModelRoster {
    pub(crate) models: Vec<CodexModelMetadata>,
    pub(crate) freshness: CodexModelCacheFreshness,
    pub(crate) fetched_at: Option<DateTime<Utc>>,
    pub(crate) observed_at: Option<DateTime<Utc>>,
    pub(crate) source: &'static str,
    pub(crate) observation_persisted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CodexModelMetadata {
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) display_name: Option<String>,
    pub(crate) context_window: Option<u32>,
    pub(crate) reasoning: Option<bool>,
    pub(crate) efforts: Vec<String>,
}

impl CodexModelRoster {
    fn fallback(freshness: CodexModelCacheFreshness, fetched_at: Option<DateTime<Utc>>) -> Self {
        Self {
            models: Vec::new(),
            freshness,
            fetched_at,
            observed_at: None,
            source: "chatgpt_plan_api",
            observation_persisted: false,
        }
    }

    #[must_use]
    pub(crate) fn model_ids(&self) -> Vec<String> {
        self.models.iter().map(|model| model.id.clone()).collect()
    }

    #[must_use]
    pub(crate) fn metadata_for(&self, id: &str) -> Option<&CodexModelMetadata> {
        self.models
            .iter()
            .find(|model| model.id.eq_ignore_ascii_case(id.trim()))
    }

    #[must_use]
    pub(crate) fn preferred_model_id(&self) -> Option<&str> {
        (self.freshness == CodexModelCacheFreshness::Fresh)
            .then(|| self.models.first().map(|model| model.id.as_str()))
            .flatten()
    }
}

#[derive(Serialize, Deserialize)]
struct CatalogSnapshot {
    fetched_at: DateTime<Utc>,
    models: Vec<CodexModelMetadata>,
}

type RosterCacheKey = (PathBuf, Option<SystemTime>, u64);
static ROSTER_MEMO: Mutex<Option<(RosterCacheKey, CodexModelRoster)>> = Mutex::new(None);

/// An unscoped completion/catalog cannot borrow another account's roster.
#[must_use]
pub(crate) fn model_roster() -> CodexModelRoster {
    CodexModelRoster::fallback(CodexModelCacheFreshness::Missing, None)
}

#[must_use]
pub(crate) fn model_roster_for(config: &Config) -> CodexModelRoster {
    let Some(path) = snapshot_path(config) else {
        return model_roster();
    };
    let key = match std::fs::symlink_metadata(&path) {
        Ok(metadata) if !metadata.file_type().is_file() => {
            return CodexModelRoster::fallback(CodexModelCacheFreshness::Invalid, None);
        }
        Ok(metadata) => (path.clone(), metadata.modified().ok(), metadata.len()),
        Err(_) => (path.clone(), None, 0),
    };
    let now = Utc::now();
    if let Ok(memo) = ROSTER_MEMO.lock()
        && let Some((cached_key, roster)) = memo.as_ref()
        && *cached_key == key
        && roster.freshness == CodexModelCacheFreshness::Fresh
        && roster
            .fetched_at
            .is_some_and(|fetched| now.signed_duration_since(fetched) <= MODEL_CACHE_MAX_AGE)
    {
        return roster.clone();
    }
    let roster = load_snapshot(&path, now);
    if let Ok(mut memo) = ROSTER_MEMO.lock() {
        *memo = Some((key, roster.clone()));
    }
    roster
}

fn registration_key(issuer: &str, client_id: &str, subject: &str) -> String {
    let mut identity = Sha256::new();
    identity.update(b"codewhale-chatgpt-plan-roster-v1\0");
    for value in [issuer, client_id, subject] {
        identity.update((value.len() as u64).to_le_bytes());
        identity.update(value.as_bytes());
    }
    identity
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn snapshot_path(config: &Config) -> Option<PathBuf> {
    let identity = config
        .builtin_provider_identity(ProviderKind::OpenaiCodex)
        .ok()?;
    if config.provider_uses_custom_endpoint(&identity) {
        return None;
    }
    let registration = crate::oauth::official_chatgpt_registration(config).ok()?;
    let catalog_path = crate::models_dev_live::cache_path()?;
    Some(catalog_path.parent()?.join(format!(
        "chatgpt-plan-{}.json",
        registration_key(
            &registration.issuer,
            &registration.client_id,
            &registration.subject
        )
    )))
}

fn load_snapshot(path: &Path, now: DateTime<Utc>) -> CodexModelRoster {
    let bytes = match read_cache_bytes(path) {
        Ok(bytes) => bytes,
        Err(freshness) => return CodexModelRoster::fallback(freshness, None),
    };
    let snapshot: CatalogSnapshot = match serde_json::from_slice(&bytes) {
        Ok(snapshot) => snapshot,
        Err(_) => return CodexModelRoster::fallback(CodexModelCacheFreshness::Invalid, None),
    };
    let age = now.signed_duration_since(snapshot.fetched_at);
    if age < -MAX_FUTURE_CLOCK_SKEW
        || snapshot.models.iter().any(|model| {
            !crate::provider_lake::valid_catalog_model_id(&model.id)
                || model
                    .display_name
                    .as_ref()
                    .is_some_and(|name| name.len() > 512 || name.chars().any(char::is_control))
                || model.efforts.len() > 16
                || model.efforts.iter().any(|effort| !valid_effort(effort))
                || model
                    .context_window
                    .is_some_and(|window| !(1..=16_000_000).contains(&window))
        })
    {
        return CodexModelRoster::fallback(
            CodexModelCacheFreshness::Invalid,
            Some(snapshot.fetched_at),
        );
    }
    if age > MODEL_CACHE_MAX_AGE {
        return CodexModelRoster::fallback(
            CodexModelCacheFreshness::Stale,
            Some(snapshot.fetched_at),
        );
    }
    CodexModelRoster {
        models: snapshot.models,
        freshness: CodexModelCacheFreshness::Fresh,
        fetched_at: Some(snapshot.fetched_at),
        observed_at: None,
        source: "chatgpt_plan_api",
        observation_persisted: true,
    }
}

fn valid_effort(effort: &str) -> bool {
    !effort.is_empty()
        && effort.len() <= 32
        && effort
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

/// Fetch through the existing provider client. A registration change during
/// the request cannot publish an old account's models under a new account.
pub(crate) async fn update_from_chatgpt(config: &Config) -> Result<CodexModelRoster, &'static str> {
    let config = config.clone();
    let prepared = config.clone();
    #[cfg(test)]
    let ticket = crate::test_support::env_scope_ticket();
    let (path, client) = tokio::task::spawn_blocking(move || {
        #[cfg(test)]
        let _membership = crate::test_support::join_env_scope(ticket);
        let path = snapshot_path(&prepared).ok_or("chatgpt_plan_permission_required")?;
        let client = crate::client::CodewhaleClient::for_catalog_refresh(&prepared)
            .map_err(|_| "chatgpt_plan_credentials_unavailable")?;
        Ok::<_, &'static str>((path, client))
    })
    .await
    .map_err(|_| "chatgpt_plan_credentials_unavailable")??;
    let available = tokio::time::timeout(std::time::Duration::from_secs(20), client.list_models())
        .await
        .map_err(|_| "chatgpt_models_timeout")?
        .map_err(|_| "chatgpt_models_unavailable")?;
    // Catalog ordering and labels are provider facts. The basic official
    // listing does not establish context limits or reasoning effort tiers.
    if available.iter().any(|model| {
        !crate::provider_lake::valid_catalog_model_id(&model.id)
            || model
                .display_name
                .as_ref()
                .is_some_and(|name| name.len() > 512 || name.chars().any(char::is_control))
    }) {
        return Err("chatgpt_models_invalid_response");
    }
    let models = available
        .into_iter()
        .map(|model| CodexModelMetadata {
            id: model.id,
            display_name: model.display_name,
            context_window: None,
            reasoning: None,
            efforts: Vec::new(),
        })
        .collect();
    let snapshot = CatalogSnapshot {
        fetched_at: Utc::now(),
        models,
    };
    #[cfg(test)]
    let ticket = crate::test_support::env_scope_ticket();
    tokio::task::spawn_blocking(move || {
        #[cfg(test)]
        let _membership = crate::test_support::join_env_scope(ticket);
        if snapshot_path(&config).as_ref() != Some(&path) {
            return Err("refresh_credentials_changed");
        }
        let encoded = serde_json::to_vec(&snapshot).map_err(|_| "cache_write_failed")?;
        if encoded.len() as u64 > MAX_MODEL_CACHE_BYTES {
            return Err("chatgpt_models_response_too_large");
        }
        codewhale_config::persistence::atomic_write(&path, &encoded)
            .map_err(|_| "cache_write_failed")?;
        if let Ok(mut memo) = ROSTER_MEMO.lock() {
            *memo = None;
        }
        Ok(load_snapshot(&path, Utc::now()))
    })
    .await
    .map_err(|_| "cache_write_failed")?
}

fn read_cache_bytes(path: &Path) -> Result<Vec<u8>, CodexModelCacheFreshness> {
    let path_metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(CodexModelCacheFreshness::Missing);
        }
        Err(_) => return Err(CodexModelCacheFreshness::Invalid),
    };
    if !path_metadata.file_type().is_file() || path_metadata.len() > MAX_MODEL_CACHE_BYTES {
        return Err(CodexModelCacheFreshness::Invalid);
    }
    let mut file = match open_cache_file(path) {
        Ok(file) => file,
        Err(_) => return Err(CodexModelCacheFreshness::Invalid),
    };
    let metadata = match file.metadata() {
        Ok(metadata) => metadata,
        Err(_) => return Err(CodexModelCacheFreshness::Invalid),
    };
    if !metadata.file_type().is_file() || metadata.len() > MAX_MODEL_CACHE_BYTES {
        return Err(CodexModelCacheFreshness::Invalid);
    }

    let mut bytes = Vec::with_capacity(metadata.len().min(MAX_MODEL_CACHE_BYTES) as usize);
    if file
        .by_ref()
        .take(MAX_MODEL_CACHE_BYTES + 1)
        .read_to_end(&mut bytes)
        .is_err()
        || bytes.len() as u64 > MAX_MODEL_CACHE_BYTES
    {
        return Err(CodexModelCacheFreshness::Invalid);
    }
    Ok(bytes)
}

#[cfg(test)]
pub(crate) fn install_test_chatgpt_roster(config: &Config, ids: &[&str]) -> anyhow::Result<()> {
    install_test_chatgpt_roster_with_metadata(
        config,
        ids.iter()
            .map(|id| CodexModelMetadata {
                id: (*id).to_string(),
                display_name: None,
                context_window: None,
                reasoning: None,
                efforts: Vec::new(),
            })
            .collect(),
    )
}

#[cfg(test)]
pub(crate) fn install_test_chatgpt_roster_with_metadata(
    config: &Config,
    models: Vec<CodexModelMetadata>,
) -> anyhow::Result<()> {
    let path = snapshot_path(config)
        .ok_or_else(|| anyhow::anyhow!("test needs an owned ChatGPT registration"))?;
    let snapshot = CatalogSnapshot {
        fetched_at: Utc::now(),
        models,
    };
    codewhale_config::persistence::atomic_write(&path, &serde_json::to_vec(&snapshot)?)?;
    if let Ok(mut memo) = ROSTER_MEMO.lock() {
        *memo = None;
    }
    Ok(())
}

fn open_cache_file(path: &Path) -> std::io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW);
    options.open(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(id: &str) -> CodexModelMetadata {
        CodexModelMetadata {
            id: id.to_string(),
            display_name: Some(format!("Label for {id}")),
            context_window: None,
            reasoning: None,
            efforts: Vec::new(),
        }
    }

    fn save(path: &Path, fetched_at: DateTime<Utc>, models: Vec<CodexModelMetadata>) {
        std::fs::write(
            path,
            serde_json::to_vec(&CatalogSnapshot { fetched_at, models }).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn registration_scope_separates_accounts_workspaces_and_issuers() {
        let key = registration_key("https://auth.openai.com", "oaiapp_workspace_a", "account_a");
        for other in [
            registration_key("https://auth.openai.com", "oaiapp_workspace_a", "account_b"),
            registration_key("https://auth.openai.com", "oaiapp_workspace_b", "account_a"),
            registration_key("https://other.example", "oaiapp_workspace_a", "account_a"),
        ] {
            assert_ne!(key, other);
        }
        assert!(!key.contains("account_a"));
        assert!(!key.contains("workspace_a"));
    }

    #[test]
    fn own_roster_follows_selected_registration_and_disappears_after_sign_out() {
        let _env = crate::test_support::lock_test_env();
        let directory = tempfile::tempdir().unwrap();
        let directory_path = directory.path().canonicalize().unwrap();
        let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", &directory_path);
        let mut account_a = Config::default();
        crate::oauth::install_test_chatgpt_registration_for(
            &mut account_a,
            "account-a",
            "oaiapp_workspace_a",
        )
        .unwrap();
        install_test_chatgpt_roster(&account_a, &["z-first", "a-second"]).unwrap();
        let path_a = snapshot_path(&account_a).unwrap();
        assert_eq!(
            model_roster_for(&account_a).model_ids(),
            ["z-first", "a-second"]
        );

        let mut account_b = Config::default();
        crate::oauth::install_test_chatgpt_registration_for(
            &mut account_b,
            "account-b",
            "oaiapp_workspace_a",
        )
        .unwrap();
        assert_ne!(snapshot_path(&account_b).unwrap(), path_a);
        assert_eq!(
            model_roster_for(&account_b).freshness,
            CodexModelCacheFreshness::Missing
        );
        install_test_chatgpt_roster(&account_b, &["b-only"]).unwrap();
        assert_eq!(model_roster_for(&account_b).model_ids(), ["b-only"]);
        assert_eq!(
            model_roster_for(&account_a).model_ids(),
            ["z-first", "a-second"]
        );

        let mut workspace_b = Config::default();
        crate::oauth::install_test_chatgpt_registration_for(
            &mut workspace_b,
            "account-a",
            "oaiapp_workspace_b",
        )
        .unwrap();
        assert!(model_roster_for(&workspace_b).models.is_empty());
        let generation = account_a
            .provider_config_for(&account_a.test_identity_for_kind(ProviderKind::OpenaiCodex))
            .unwrap()
            .oauth_credential_generation
            .as_ref()
            .unwrap()
            .clone();
        let path = codewhale_config::chatgpt_oauth_generation_path(&generation).unwrap();
        std::fs::remove_file(path).unwrap();
        assert!(model_roster_for(&account_a).models.is_empty());
        assert!(path_a.exists());
        assert_eq!(model_roster_for(&account_b).model_ids(), ["b-only"]);
    }

    #[test]
    fn own_snapshot_preserves_provider_order_and_labels_without_inventing_limits() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("roster.json");
        let now = Utc::now();
        save(&path, now, vec![model("z-first"), model("a-second")]);
        let roster = load_snapshot(&path, now);
        assert_eq!(roster.model_ids(), ["z-first", "a-second"]);
        assert_eq!(roster.preferred_model_id(), Some("z-first"));
        let metadata = roster.metadata_for("a-second").unwrap();
        assert_eq!(metadata.display_name.as_deref(), Some("Label for a-second"));
        assert_eq!(metadata.context_window, None);
        assert_eq!(metadata.reasoning, None);
        assert!(metadata.efforts.is_empty());
        assert_eq!(roster.source, "chatgpt_plan_api");
        assert!(roster.observation_persisted);
    }

    #[test]
    fn missing_stale_future_and_unsafe_snapshots_offer_no_entitlements() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("roster.json");
        let now = Utc::now();
        assert!(load_snapshot(&path, now).model_ids().is_empty());
        save(
            &path,
            now - MODEL_CACHE_MAX_AGE - Duration::seconds(1),
            vec![model("old")],
        );
        let stale = load_snapshot(&path, now);
        assert_eq!(stale.freshness, CodexModelCacheFreshness::Stale);
        assert!(stale.models.is_empty());
        save(
            &path,
            now + MAX_FUTURE_CLOCK_SKEW + Duration::seconds(1),
            vec![model("future")],
        );
        assert_eq!(
            load_snapshot(&path, now).freshness,
            CodexModelCacheFreshness::Invalid
        );
        let mut unsafe_label = model("safe-id");
        unsafe_label.display_name = Some("Unsafe\u{1b}[31m".to_string());
        save(&path, now, vec![unsafe_label]);
        assert_eq!(
            load_snapshot(&path, now).freshness,
            CodexModelCacheFreshness::Invalid
        );
        assert!(model_roster().models.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn cache_symlinks_are_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("target.json");
        let path = directory.path().join("roster.json");
        let now = Utc::now();
        save(&target, now, vec![model("safe")]);
        std::os::unix::fs::symlink(target, &path).unwrap();
        assert_eq!(
            load_snapshot(&path, now).freshness,
            CodexModelCacheFreshness::Invalid
        );
    }
}
