//! Durable CLI route edits use the same Config owner as Runtime and the TUI.
//! `model` and the legacy `default_text_model` address the saved active route;
//! `default_model` addresses DeepSeek (CN while that route is active). Project root keys retain their scope.

use std::path::Path;

use anyhow::{Context, Result, ensure};

use crate::config::{ApiProvider, Config, ProviderIdentity};
use crate::config_persistence as persistence;

/// Whether a config key names a provider or model selection.
pub fn is_route_key(key: &str) -> bool {
    matches!(
        key,
        "provider" | "model" | "default_model" | "default_text_model"
    ) || provider_model_id(key).is_some()
}

fn provider_model_id(key: &str) -> Option<&str> {
    key.strip_prefix("providers.")?
        .strip_suffix(".model")
        .filter(|id| !id.is_empty())
}

fn parse_config(body: &str) -> Result<Config> {
    toml::from_str(body)
        .map_err(|_| anyhow::anyhow!("Could not parse route configuration; contents omitted"))
}

fn model_identity(config: &Config, key: &str) -> Result<ProviderIdentity> {
    if key == "default_model" {
        let provider = if config.api_provider() == ApiProvider::DeepseekCN {
            ApiProvider::DeepseekCN
        } else {
            ApiProvider::Deepseek
        };
        config.resolve_provider_pin_identity(provider.as_str())
    } else if let Some(id) = provider_model_id(key) {
        // Leaf keys name TOML tables, whose canonical spelling can differ
        // from the public provider selector. Exact custom tables still win.
        let selector = if config
            .providers
            .as_ref()
            .and_then(|providers| providers.custom_provider_config(id))
            .is_some()
        {
            id
        } else if id == "deepseek_cn" {
            ApiProvider::DeepseekCN.as_str()
        } else {
            ApiProvider::all()
                .iter()
                .find(|provider| {
                    provider
                        .metadata()
                        .is_some_and(|metadata| metadata.provider_config_key() == id)
                })
                .map_or(id, |provider| provider.as_str())
        };
        config.resolve_provider_pin_identity(selector)
    } else {
        config.active_provider_identity(config.api_provider())
    }
    .map_err(anyhow::Error::msg)
}

fn model_slot(identity: &ProviderIdentity) -> Result<Vec<&str>> {
    if identity.provider == ApiProvider::Custom && identity.persisted_id().is_none() {
        return Ok(vec!["default_text_model"]);
    }
    let key = match identity.provider {
        ApiProvider::Custom => identity.key.as_str(),
        ApiProvider::DeepseekCN => "deepseek_cn",
        ApiProvider::OllamaCloud if identity.migrated_legacy_ollama_cloud_route => "ollama",
        provider => provider
            .metadata()
            .context("provider model table")?
            .provider_config_key(),
    };
    Ok(vec!["providers", key, "model"])
}

/// Locate the canonical model leaf in a saved document without applying
/// device preferences or launch overrides. Export uses this same identity
/// resolution to omit only root aliases shadowed by that leaf.
pub fn model_slot_for_document(body: &str, key: &str) -> Result<Vec<String>> {
    ensure!(
        is_route_key(key) && key != "provider",
        "Not a model preference key: {key}"
    );
    let config = parse_config(body)?;
    let identity = model_identity(&config, key)?;
    Ok(model_slot(&identity)?
        .into_iter()
        .map(str::to_string)
        .collect())
}

fn document_slot_value(document: &toml::value::Table, slot: &[String]) -> Option<String> {
    let [root, provider, field] = slot else {
        return None;
    };
    document
        .get(root)?
        .as_table()?
        .get(provider)?
        .as_table()?
        .get(field)?
        .as_str()
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .map(str::to_string)
}

fn insert_document_slot_value(
    document: &mut toml::value::Table,
    slot: &[String],
    value: &str,
) -> bool {
    let [root, provider, field] = slot else {
        return false;
    };
    let Some(providers) = document
        .entry(root.clone())
        .or_insert_with(|| toml::Value::Table(toml::value::Table::new()))
        .as_table_mut()
    else {
        return false;
    };
    let Some(entry) = providers
        .entry(provider.clone())
        .or_insert_with(|| toml::Value::Table(toml::value::Table::new()))
        .as_table_mut()
    else {
        return false;
    };
    entry.insert(field.clone(), toml::Value::String(value.to_string()));
    true
}

/// Scrub root model aliases from a serialized config document for export.
///
/// Root `model`/`default_text_model` address the *active* route on import and
/// `default_model` addresses DeepSeek (CN when that route is active), so a raw root alias exported next to
/// the canonical `[providers.<id>].model` leaf fails the importer's replay
/// check. This rewrites `document` in place:
///
/// - A root alias the active route still consumes is shadowed state once the
///   route's canonical leaf is exported; it is dropped.
/// - A root alias the active route ignores but DeepSeek recognizes is
///   DeepSeek's saved fallback; it moves to `providers.deepseek.model` unless
///   that slot already carries a value. Any other unrecognized value is dead
///   state that would only conflict on import and is dropped.
/// - `default_model` folds into the selected DeepSeek region's model slot when
///   nothing live occupies that slot, and is dropped otherwise.
///
/// Only route-selection keys are parsed as `Config` here. Export documents
/// intentionally preserve unknown or differently typed local-authority
/// extras, and reparsing them merely to locate the model slot would fail the
/// whole export.
pub fn scrub_root_model_aliases_for_export(document: &mut toml::value::Table) -> Result<()> {
    let mut scratch = toml::value::Table::new();
    for key in [
        "provider",
        "model",
        "default_text_model",
        "defaultTextModel",
        "base_url",
        "baseUrl",
        "providers",
    ] {
        if let Some(value) = document.get(key) {
            scratch.insert(key.to_string(), value.clone());
        }
    }
    let body = toml::to_string(&toml::Value::Table(scratch))
        .context("serializing route selection for export")?;
    // Slot resolution reuses the exact document identity rules; the extra
    // parse below only exists so the ownership test can scope a Config clone.
    let active_slot = model_slot_for_document(&body, "model")?;
    let deepseek_slot = model_slot_for_document(&body, "default_model")?;
    let config = parse_config(&body)?;
    let identity = model_identity(&config, "model")?;

    if document_slot_value(document, &active_slot).is_some() {
        for root_key in ["model", "default_text_model"] {
            let Some(value) = document
                .get(root_key)
                .and_then(toml::Value::as_str)
                .map(str::to_owned)
            else {
                continue;
            };
            // Same ownership test as `unset`: scope to the active route with
            // its canonical leaf cleared and ask whether this root value is
            // what the route would then resolve. A foreign DeepSeek root
            // ignored by the active vendor remains DeepSeek's fallback.
            let mut scoped = config.clone();
            scoped.scope_to_provider_identity(&identity);
            scoped.set_provider_model_override(identity.provider, None);
            scoped.legacy_model = None;
            scoped.default_text_model = Some(value.to_string());
            let wire_model = crate::config::wire_model_for_provider_route(
                identity.provider,
                &scoped.active_route_base_url(),
                &value,
            );
            if scoped.default_model() == wire_model {
                document.remove(root_key);
                continue;
            }
            if crate::config::normalize_model_name(&value).is_none() {
                document.remove(root_key);
                continue;
            }
            if document_slot_value(document, &deepseek_slot).is_some() {
                document.remove(root_key);
                continue;
            }
            ensure!(
                insert_document_slot_value(document, &deepseek_slot, &value),
                "Cannot export a root model alias into a non-table provider slot"
            );
            document.remove(root_key);
        }
    }

    match document.get("default_model").and_then(toml::Value::as_str) {
        Some(value) => {
            let value = value.trim().to_string();
            // A root alias the DeepSeek route still consumes lands in this
            // same slot on import; the live choice wins over the dead alias.
            let root_covers_slot = active_slot == deepseek_slot
                && ["model", "default_text_model"]
                    .iter()
                    .any(|key| document.get(*key).and_then(toml::Value::as_str).is_some());
            let folded = !value.is_empty()
                && !root_covers_slot
                && document_slot_value(document, &deepseek_slot).is_none();
            if folded {
                ensure!(
                    insert_document_slot_value(document, &deepseek_slot, &value),
                    "Cannot export default_model into a non-table provider slot"
                );
            }
            document.remove("default_model");
        }
        None => {
            ensure!(
                !document.contains_key("default_model"),
                "Cannot export a non-string default_model without losing its value"
            );
        }
    }
    Ok(())
}

fn project_root_key<'a>(path: &Path, key: &'a str) -> Option<&'a str> {
    (codewhale_config::config_path_is_workspace_scoped(path)
        && matches!(key, "model" | "default_text_model"))
    .then_some(key)
}

fn saved_config(store: &codewhale_config::ConfigStore) -> Result<Config> {
    let rendered;
    let body = if let Some(original) = store.original_body() {
        original
    } else {
        rendered = store.rendered_body()?;
        &rendered
    };
    let mut config = parse_config(body)?;
    if config.route_preferences_version.is_none()
        && crate::config::is_home_config_path(store.path())
    {
        config.apply_saved_selection(
            &crate::settings::Settings::load_legacy_route_preferences_read_only()?,
        );
    }
    Ok(config)
}

/// Read the saved route without applying launch overrides or credentials.
pub fn get(path: &Path, key: &str) -> Result<Option<String>> {
    ensure!(is_route_key(key), "Not a route preference key: {key}");
    let store = codewhale_config::ConfigStore::load(Some(path.to_path_buf()))?;
    if let Some(key) = project_root_key(store.path(), key) {
        return Ok(store.config.get_value(key));
    }
    let mut config = saved_config(&store)?;
    if key == "provider" {
        return Ok(Some(
            config.provider.unwrap_or_else(|| "deepseek".to_string()),
        ));
    }
    let identity = model_identity(&config, key)?;
    config.scope_to_provider_identity(&identity);
    Ok(config
        .provider_config_for(identity.provider)
        .and_then(|entry| entry.model.clone())
        .or_else(|| {
            (provider_model_id(key).is_none()
                && (config.default_text_model.is_some() || config.legacy_model.is_some()))
            .then(|| config.default_model())
        }))
}

/// One saved snapshot for CLI route reports, including exact legacy identities.
/// The source distinguishes an explicit model from the provider default.
pub fn selected_route(path: &Path) -> Result<(String, String, codewhale_config::ModelSource)> {
    let store = codewhale_config::ConfigStore::load(Some(path.to_path_buf()))?;
    let config = saved_config(&store)?;
    let identity = config
        .active_provider_identity(config.api_provider())
        .map_err(anyhow::Error::msg)?;
    let provider = identity.key;
    let source = if config
        .provider_config_for(identity.provider)
        .and_then(|entry| entry.model.as_ref())
        .is_some()
    {
        codewhale_config::ModelSource::ProviderConfig
    } else if config.default_text_model.is_some() || config.legacy_model.is_some() {
        if store.config.default_text_model.is_none() && store.config.model.is_some() {
            codewhale_config::ModelSource::RootModel
        } else {
            codewhale_config::ModelSource::RootDefaultTextModel
        }
    } else {
        codewhale_config::ModelSource::ProviderDefault
    };
    Ok((provider, config.default_model(), source))
}

/// Save one explicit route preference after atomically adopting legacy choices.
pub fn set(path: &Path, key: &str, value: &str) -> Result<()> {
    persistence::mutate_config_document(path, |doc| set_document(path, doc, key, value))
}

/// Prepare a validated snapshot for a caller's preview and atomic save.
/// Migration changes only this document; this function never writes a file.
pub fn prepare_document(path: &Path, raw: &str) -> Result<toml_edit::DocumentMut> {
    let mut doc = raw
        .parse::<toml_edit::DocumentMut>()
        .map_err(|_| anyhow::anyhow!("Could not parse route configuration; contents omitted"))?;
    parse_config(raw)?;
    persistence::migrate_legacy_route_preferences(path, &mut doc)?;
    Ok(doc)
}

/// Edit one route selection in an already-prepared candidate without saving.
/// Callers must prepare migration first and atomically save the final snapshot.
pub fn set_document(
    path: &Path,
    doc: &mut toml_edit::DocumentMut,
    key: &str,
    value: &str,
) -> Result<()> {
    ensure!(is_route_key(key), "Not a route preference key: {key}");
    let value = value.trim();
    ensure!(
        !value.is_empty() && !value.chars().any(char::is_control),
        "Route preference must be nonempty and contain no control characters"
    );
    if let Some(key) = project_root_key(path, key) {
        return persistence::set_document_value(doc, &[key], value);
    }
    let config = parse_config(&doc.to_string())?;
    if key == "provider" {
        let identity = config
            .resolve_provider_pin_identity(value)
            .map_err(anyhow::Error::msg)?;
        persistence::set_document_value(
            doc,
            &["provider"],
            identity.persisted_id().unwrap_or(&identity.key),
        )?;
        // Same root-alias authority as the Runtime/TUI provider writer: a CLI
        // switch must not leave the incoming route holding the outgoing one's
        // fallback, and must not delete a choice to get there.
        return persistence::reconcile_root_model_aliases(doc, &config, &identity);
    }
    let identity = model_identity(&config, key)?;
    persistence::set_provider_model_document(
        doc,
        identity.provider,
        identity.persisted_id().unwrap_or(&identity.key),
        value,
    )
}

/// Clear a canonical selection without allowing archived Settings to restore it.
pub fn unset(path: &Path, key: &str) -> Result<()> {
    ensure!(is_route_key(key), "Not a route preference key: {key}");
    persistence::mutate_config_document(path, |doc| {
        if key == "provider" || project_root_key(path, key).is_some() {
            persistence::unset_document_value(doc, &[key])?;
            return Ok(());
        }
        let config = parse_config(&doc.to_string())?;
        let identity = model_identity(&config, key)?;
        persistence::unset_document_value(doc, &model_slot(&identity)?)?;
        // Clear relevant legacy fallbacks as well, or deleting the canonical
        // leaf would restore an older choice on reload. Root fields belong to
        // the active route, with DeepSeek's historical default as an exception.
        if matches!(
            identity.provider,
            ApiProvider::Deepseek | ApiProvider::DeepseekCN
        ) || config
            .active_provider_identity(config.api_provider())
            .is_ok_and(|active| active == identity)
        {
            let mut scoped = config.clone();
            scoped.scope_to_provider_identity(&identity);
            scoped.set_provider_model_override(identity.provider, None);
            scoped.legacy_model = None;
            for root_key in ["default_text_model", "model"] {
                let Some(model) = doc.get(root_key).and_then(toml_edit::Item::as_str) else {
                    continue;
                };
                scoped.default_text_model = Some(model.to_string());
                let wire_model = crate::config::wire_model_for_provider_route(
                    identity.provider,
                    &scoped.active_route_base_url(),
                    model,
                );
                // Reuse Config's root-model guards: a foreign DeepSeek root
                // ignored by the active vendor remains that provider's fallback.
                if scoped.default_model() == wire_model {
                    persistence::unset_document_value(doc, &[root_key])?;
                }
            }
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{EnvVarGuard, lock_test_env};

    fn document(path: &Path) -> toml::Value {
        toml::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn route_edits_adopt_legacy_selection_once_and_preserve_settings() -> Result<()> {
        let _env = lock_test_env();
        let home = tempfile::tempdir()?;
        let _home = EnvVarGuard::set("CODEWHALE_HOME", home.path());
        let _path = EnvVarGuard::remove("CODEWHALE_CONFIG_PATH");
        let _legacy_path = EnvVarGuard::remove("DEEPSEEK_CONFIG_PATH");
        let path = home.path().join("config.toml");
        std::fs::write(
            &path,
            "provider = \"deepseek\"\ndefault_text_model = \"deepseek-v4-pro\"\n[providers.zai]\nmodel = \"GLM-5.2\"\n",
        )?;
        let settings_path = home.path().join("settings.toml");
        let settings = "default_provider = \"zai\"\n[provider_models]\nzai = \"GLM-5.3\"\n";
        std::fs::write(&settings_path, settings)?;
        let before = std::fs::read(&path)?;
        assert_eq!(get(&path, "provider")?.as_deref(), Some("zai"));
        assert_eq!(get(&path, "model")?.as_deref(), Some("GLM-5.3"));
        assert_eq!(std::fs::read(&path)?, before);

        let mut candidate = prepare_document(&path, &std::fs::read_to_string(&path)?)?;
        set_document(&path, &mut candidate, "model", "GLM-5.2")?;
        assert_eq!(std::fs::read(&path)?, before);
        set(&path, "model", "GLM-5.2")?;
        assert_eq!(
            document(&path),
            toml::from_str::<toml::Value>(&candidate.to_string())?
        );
        assert_eq!(
            document(&path)["route_preferences_version"].as_integer(),
            Some(1)
        );
        assert_eq!(
            get(&path, "default_text_model")?.as_deref(),
            Some("GLM-5.2")
        );
        assert_eq!(
            get(&path, "providers.zai.model")?.as_deref(),
            Some("GLM-5.2")
        );
        set(&path, "default_model", "deepseek-v4-flash")?;
        assert_eq!(
            get(&path, "default_model")?.as_deref(),
            Some("deepseek-v4-flash")
        );
        set(&path, "provider", "deepseek")?;
        assert_eq!(get(&path, "model")?.as_deref(), Some("deepseek-v4-flash"));
        unset(&path, "providers.deepseek.model")?;
        assert!(get(&path, "default_model")?.is_none());
        unset(&path, "providers.zai.model")?;
        assert!(get(&path, "providers.zai.model")?.is_none());
        unset(&path, "provider")?;
        assert_eq!(get(&path, "provider")?.as_deref(), Some("deepseek"));
        assert_eq!(std::fs::read_to_string(settings_path)?, settings);
        // An unrelated typed store write must preserve the migration receipt.
        let mut store = codewhale_config::ConfigStore::load(Some(path.clone()))?;
        store.config.set_value("verbosity", "quiet")?;
        store.save()?;
        assert_eq!(
            document(&path)["route_preferences_version"].as_integer(),
            Some(1)
        );
        Ok(())
    }

    #[test]
    fn route_edits_keep_exact_named_provider_keys_and_reject_unknown_routes() -> Result<()> {
        let _env = lock_test_env();
        let home = tempfile::tempdir()?;
        let _home = EnvVarGuard::set("CODEWHALE_HOME", home.path());
        let _path = EnvVarGuard::remove("CODEWHALE_CONFIG_PATH");
        let _legacy_path = EnvVarGuard::remove("DEEPSEEK_CONFIG_PATH");
        let path = home.path().join("config.toml");
        std::fs::write(
            &path,
            r#"provider = "Team.A"
[providers."Team.A"]
kind = "openai-compatible"
base_url = "http://127.0.0.1:9/v1"
model = "Model-X"
[providers."team.a"]
kind = "openai-compatible"
base_url = "http://127.0.0.1:10/v1"
model = "Other-X"
"#,
        )?;
        set(&path, "providers.Team.A.model", "Model-Y")?;
        assert_eq!(get(&path, "model")?.as_deref(), Some("Model-Y"));
        assert_eq!(
            get(&path, "providers.team.a.model")?.as_deref(),
            Some("Other-X")
        );
        let before = std::fs::read(&path)?;
        assert!(set(&path, "providers.TEAM.A.model", "Model-Z").is_err());
        assert!(set(&path, "provider", "unconfigured-route").is_err());
        assert_eq!(std::fs::read(&path)?, before);
        unset(&path, "providers.Team.A.model")?;
        assert!(get(&path, "providers.Team.A.model")?.is_none());
        assert_eq!(
            document(&path)["providers"]["team.a"]["model"].as_str(),
            Some("Other-X")
        );
        Ok(())
    }

    #[test]
    fn cli_provider_edits_share_the_runtime_writer_root_alias_authority() -> Result<()> {
        let _env = lock_test_env();
        let home = tempfile::tempdir()?;
        let _home = EnvVarGuard::set("CODEWHALE_HOME", home.path());
        let _path = EnvVarGuard::remove("CODEWHALE_CONFIG_PATH");
        let _legacy_path = EnvVarGuard::remove("DEEPSEEK_CONFIG_PATH");
        let _cloud = EnvVarGuard::set("CODEWHALE_DISABLE_CLOUD_FACTS", "1");
        let _overrides: Vec<_> = [
            "CODEWHALE_MODEL",
            "DEEPSEEK_MODEL",
            "DEEPSEEK_DEFAULT_TEXT_MODEL",
            "CODEWHALE_PROVIDER",
            "DEEPSEEK_PROVIDER",
            "CODEWHALE_BASE_URL",
            "DEEPSEEK_BASE_URL",
            "CODEWHALE_PROFILE",
            "DEEPSEEK_PROFILE",
            "ZAI_MODEL",
            "ZAI_BASE_URL",
        ]
        .into_iter()
        .map(EnvVarGuard::remove)
        .collect();
        let path = home.path().join("config.toml");

        // The incoming route owns its own leaf, so the outgoing root fallback
        // is inert saved state. A CLI switch must not delete it, and switching
        // back must still find it.
        std::fs::write(
            &path,
            "route_preferences_version = 1\nprovider = \"zai\"\ndefault_text_model = \"GLM-4.6\"\n[providers.deepseek]\nmodel = \"deepseek-v4-pro\"\n",
        )?;
        set(&path, "provider", "deepseek")?;
        assert_eq!(
            document(&path)["default_text_model"].as_str(),
            Some("GLM-4.6")
        );
        let switched = Config::load(Some(path.clone()), None)
            .expect("a CLI provider switch must remain loadable");
        assert_eq!(switched.api_provider(), ApiProvider::Deepseek);
        assert_eq!(switched.default_model(), "deepseek-v4-pro");
        set(&path, "provider", "zai")?;
        assert_eq!(
            Config::load(Some(path.clone()), None)
                .expect("switching back must remain loadable")
                .default_model(),
            "GLM-4.6"
        );

        // With no leaf on the incoming route the alias is what `Config::load`
        // rejects. Move it to the route that owns it rather than drop it.
        std::fs::write(
            &path,
            "route_preferences_version = 1\nprovider = \"volcengine\"\ndefault_text_model = \"ark-private-id\"\n",
        )?;
        set(&path, "provider", "deepseek")?;
        let doc = document(&path);
        assert!(doc.get("default_text_model").is_none());
        assert_eq!(
            doc["providers"]["volcengine"]["model"].as_str(),
            Some("ark-private-id")
        );
        Config::load(Some(path.clone()), None)
            .expect("a CLI switch must not commit an unloadable config");
        set(&path, "provider", "volcengine")?;
        assert_eq!(
            Config::load(Some(path), None)
                .expect("switching back must remain loadable")
                .default_model(),
            "ark-private-id"
        );
        Ok(())
    }

    #[test]
    fn project_model_edits_keep_root_fields_and_skip_device_migration() -> Result<()> {
        let _env = lock_test_env();
        let root = tempfile::tempdir()?;
        let home = root.path().join("home");
        std::fs::create_dir_all(&home)?;
        let _home = EnvVarGuard::set("CODEWHALE_HOME", &home);
        let _path = EnvVarGuard::remove("CODEWHALE_CONFIG_PATH");
        let _legacy_path = EnvVarGuard::remove("DEEPSEEK_CONFIG_PATH");
        std::fs::write(home.join("settings.toml"), "default_provider = \"zai\"\n")?;
        let project = root.path().join("project/.codewhale");
        std::fs::create_dir_all(&project)?;
        // An absolute config outside the process workspace is project-scoped
        // only when its parent is a checkout, not merely named `.codewhale`.
        std::fs::create_dir(root.path().join("project/.git"))?;
        let path = project.join("config.toml");
        std::fs::write(&path, "model = \"project-old\"\n")?;
        set(&path, "model", "project-new")?;
        assert_eq!(get(&path, "model")?.as_deref(), Some("project-new"));
        assert!(document(&path).get("providers").is_none());
        assert!(document(&path).get("route_preferences_version").is_none());
        unset(&path, "model")?;
        assert!(document(&path).get("model").is_none());
        Ok(())
    }

    #[test]
    fn saved_routes_preserve_regional_and_legacy_table_identity() -> Result<()> {
        let _env = lock_test_env();
        let home = tempfile::tempdir()?;
        let _home = EnvVarGuard::set("CODEWHALE_HOME", home.path());
        let _path = EnvVarGuard::remove("CODEWHALE_CONFIG_PATH");
        let _legacy_path = EnvVarGuard::remove("DEEPSEEK_CONFIG_PATH");
        let _cloud = EnvVarGuard::set("CODEWHALE_DISABLE_CLOUD_FACTS", "1");
        let _overrides: Vec<_> = [
            "CODEWHALE_MODEL",
            "DEEPSEEK_MODEL",
            "DEEPSEEK_DEFAULT_TEXT_MODEL",
            "CODEWHALE_PROVIDER",
            "DEEPSEEK_PROVIDER",
            "CODEWHALE_BASE_URL",
            "DEEPSEEK_BASE_URL",
            "CODEWHALE_PROFILE",
            "DEEPSEEK_PROFILE",
            "ZAI_MODEL",
            "ZAI_BASE_URL",
            "OLLAMA_MODEL",
            "OLLAMA_CLOUD_MODEL",
            "OLLAMA_BASE_URL",
            "OLLAMA_CLOUD_BASE_URL",
        ]
        .into_iter()
        .map(EnvVarGuard::remove)
        .collect();
        let path = home.path().join("config.toml");
        for (provider, table, endpoint, model) in [
            (
                "deepseek-cn",
                "deepseek_cn",
                "https://api.deepseek.cn",
                "deepseek-v4-flash",
            ),
            (
                "ollama",
                "ollama",
                "https://ollama.com/v1",
                "saved-cloud-model",
            ),
        ] {
            std::fs::write(
                &path,
                format!(
                    "route_preferences_version = 1\nprovider = '{provider}'\n[providers.{table}]\nbase_url = '{endpoint}'\n"
                ),
            )?;
            set(&path, "model", model)?;
            assert_eq!(document(&path)["provider"].as_str(), Some(provider));
            assert_eq!(
                document(&path)["providers"][table]["model"].as_str(),
                Some(model)
            );
            assert_eq!(get(&path, "model")?.as_deref(), Some(model));
            assert_eq!(
                selected_route(&path)?,
                (
                    if provider == "ollama" {
                        "ollama-cloud"
                    } else {
                        provider
                    }
                    .to_string(),
                    model.to_string(),
                    codewhale_config::ModelSource::ProviderConfig
                )
            );
            unset(&path, "model")?;
            assert!(document(&path)["providers"][table].get("model").is_none());
            let leaf = format!("providers.{table}.model");
            set(&path, &leaf, model)?;
            assert_eq!(get(&path, &leaf)?.as_deref(), Some(model));
            assert_eq!(
                Config::load(Some(path.clone()), None)?.default_model(),
                model
            );
            assert_eq!(document(&path)["provider"].as_str(), Some(provider));
            unset(&path, &leaf)?;
            assert!(get(&path, &leaf)?.is_none());
            assert!(document(&path)["providers"][table].get("model").is_none());
        }

        std::fs::write(
            &path,
            "route_preferences_version = 1\nprovider = 'zai'\nmodel = 'GLM-5.3'\n",
        )?;
        assert_eq!(get(&path, "model")?.as_deref(), Some("GLM-5.3"));
        assert_eq!(
            Config::load(Some(path.clone()), None)?.default_model(),
            "GLM-5.3"
        );
        assert_eq!(
            selected_route(&path)?.2,
            codewhale_config::ModelSource::RootModel
        );
        // A foreign active-route root must never become the DeepSeek default.
        assert_eq!(
            get(&path, "default_model")?.as_deref(),
            Some(crate::config::DEFAULT_TEXT_MODEL)
        );
        unset(&path, "providers.zai.model")?;
        assert!(document(&path).get("model").is_none());
        assert_eq!(
            Config::load(Some(path.clone()), None)?.default_model(),
            selected_route(&path)?.1
        );

        for (provider, root, leaf) in [
            (
                "deepseek",
                "default_text_model = 'deepseek-v4-flash'\nmodel = 'deepseek-v4-flash-vision-exp'",
                "deepseek-v4-pro",
            ),
            (
                "zai",
                "default_text_model = 'GLM-5.1'\nmodel = 'GLM-5.2'",
                "GLM-5.3",
            ),
        ] {
            std::fs::write(
                &path,
                format!(
                    "route_preferences_version = 1\nprovider = '{provider}'\n{root}\n[providers.{provider}]\nmodel = '{leaf}'\n"
                ),
            )?;
            assert_eq!(
                Config::load(Some(path.clone()), None)?.default_model(),
                leaf
            );
            unset(&path, &format!("providers.{provider}.model"))?;
            let doc = document(&path);
            assert!(doc.get("model").is_none());
            assert!(doc.get("default_text_model").is_none());
            assert!(doc["providers"][provider].get("model").is_none());
            let loaded = Config::load(Some(path.clone()), None)?;
            assert_eq!(loaded.default_model(), selected_route(&path)?.1);
            assert!(loaded.legacy_model.is_none());
        }

        // Clearing Z.ai cannot erase the independent DeepSeek root fallback.
        std::fs::write(
            &path,
            "route_preferences_version = 1\nprovider = 'zai'\ndefault_text_model = 'deepseek-v4-flash'\nmodel = 'GLM-5.1'\n[providers.zai]\nmodel = 'GLM-5.2'\n",
        )?;
        unset(&path, "providers.zai.model")?;
        assert_eq!(
            document(&path)["default_text_model"].as_str(),
            Some("deepseek-v4-flash")
        );
        assert!(document(&path).get("model").is_none());
        let loaded = Config::load(Some(path.clone()), None)?;
        assert_ne!(loaded.default_model(), "GLM-5.1");
        assert_eq!(loaded.default_model(), selected_route(&path)?.1);
        Ok(())
    }
}
