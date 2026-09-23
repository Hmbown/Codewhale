//! Plugin suggestions for a user task.
//!
//! Ranks installed bundles and locally-added marketplace candidates. A
//! suggestion is never an install, trust, enable, or network side effect.
//!
//! The proactive toast and the `<recommended_plugins>` fragment are driven
//! by the declared-keyword matcher (`match_plugin_for_draft`), not by the
//! score below: there is no host score gate on what the model sees. Scoring
//! only ranks the user-invoked `/plugin suggest` list.

use std::collections::{BTreeMap, BTreeSet};

use crate::skills::install::{RegistryDocument, RegistryEntry};
use crate::skills::recommend::recommend_remote_skills;

use super::marketplace::types::MarketplaceCandidate;
use super::registry::PluginRegistry;
use super::types::LoadedPlugin;

const DEFAULT_LIMIT: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecommendOptions {
    pub limit: usize,
    pub min_score: usize,
    pub include_active: bool,
}

impl Default for RecommendOptions {
    fn default() -> Self {
        Self {
            limit: DEFAULT_LIMIT,
            min_score: 0,
            include_active: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginMatchSource {
    Installed { id: String },
    Marketplace { catalog_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginNextStep {
    AlreadyActive,
    Trust,
    Enable,
    Inspect,
    MarketplaceInstall {
        catalog_id: String,
    },
    /// `/plugin install` only when a catalog entry carries a real source spec.
    SourceInstall {
        spec: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginTaskRecommendation {
    pub name: String,
    pub source: PluginMatchSource,
    pub matched_terms: Vec<String>,
    pub score: usize,
    pub next_step: PluginNextStep,
}

impl PluginTaskRecommendation {
    #[must_use]
    pub fn command(&self) -> String {
        match &self.next_step {
            PluginNextStep::AlreadyActive | PluginNextStep::Inspect => {
                format!("/plugin show {}", self.name)
            }
            PluginNextStep::Trust => format!("/plugin trust {}", self.name),
            PluginNextStep::Enable => format!("/plugin enable {}", self.name),
            PluginNextStep::MarketplaceInstall { catalog_id } => {
                format!("/plugin marketplace install {catalog_id} {}", self.name)
            }
            PluginNextStep::SourceInstall { spec } => format!("/plugin install {spec}"),
        }
    }
}

const RECOMMENDED_PLUGINS_INTRO: &str =
    "Here is a list of plugins that are available but not installed.";
const MAX_RECOMMENDED_PLUGINS: usize = 8;

/// One matcher-driven candidate for the live composer CTA or the
/// append-only `<recommended_plugins>` user fragment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginKeywordMatch {
    pub name: String,
    pub matched_term: Option<String>,
    pub id: String,
    pub next_step: PluginNextStep,
    keywords: Vec<String>,
    domains: Vec<String>,
}

impl PluginKeywordMatch {
    #[must_use]
    pub fn command(&self) -> String {
        PluginTaskRecommendation {
            name: self.name.clone(),
            source: match &self.next_step {
                PluginNextStep::MarketplaceInstall { catalog_id } => {
                    PluginMatchSource::Marketplace {
                        catalog_id: catalog_id.clone(),
                    }
                }
                _ => PluginMatchSource::Installed {
                    id: self.id.clone(),
                },
            },
            matched_terms: Vec::new(),
            score: 0,
            next_step: self.next_step.clone(),
        }
        .command()
    }
}

/// Load the bundled first-party catalog and user-added catalogs from the
/// shared store. Browsing never fetches or installs plugin content.
#[must_use]
pub fn load_marketplace_candidates(
    state_path: Option<&std::path::Path>,
) -> Vec<MarketplaceCandidate> {
    let Some(store) = crate::plugins::marketplace::store::MarketplaceStore::open(state_path) else {
        return Vec::new();
    };
    let Ok(state) = store.load() else {
        return Vec::new();
    };
    state
        .catalogs()
        .values()
        .flat_map(|catalog| catalog.catalog.candidates.iter().cloned())
        .collect()
}

/// Keyword candidates that can still be reviewed: installed-but-idle plugins
/// and uninstalled catalog entries. Already-active plugins are omitted.
#[must_use]
pub fn idle_and_catalog_keyword_matches(
    registry: &PluginRegistry,
    marketplace: &[MarketplaceCandidate],
) -> Vec<PluginKeywordMatch> {
    let installed = registry.list();
    let installed_names = installed
        .iter()
        .map(|plugin| plugin.name().to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    let mut out = Vec::new();
    for plugin in &installed {
        if plugin.active() {
            continue;
        }
        let next_step = if !plugin.trusted() {
            PluginNextStep::Trust
        } else if !plugin.enabled {
            PluginNextStep::Enable
        } else {
            continue;
        };
        let mut keywords = plugin.manifest.plugin.keywords.clone();
        keywords.push(plugin.name().to_string());
        out.push(PluginKeywordMatch {
            matched_term: None,
            name: plugin.name().to_string(),
            id: plugin.id.as_str().to_string(),
            next_step,
            keywords,
            domains: plugin.inventory.network_hosts.clone(),
        });
    }
    for candidate in marketplace {
        if candidate.has_errors() {
            continue;
        }
        // Only plugins are plugin suggestions (#6290 rework): skill entries
        // are installable, but this pool feeds the composer toast and the
        // `<recommended_plugins>` fragment, so a skill must not be dressed as
        // one. This replaces #6274's name suppression, which existed only
        // because the catalog mixed the two kinds.
        if candidate.kind != crate::plugins::marketplace::types::MarketplaceEntryKind::Plugin {
            continue;
        }
        if installed_names.contains(&candidate.name.to_ascii_lowercase()) {
            continue;
        }
        let mut keywords = candidate.keywords.clone();
        keywords.push(candidate.name.clone());
        if let Some(display) = &candidate.display_name {
            keywords.push(display.clone());
        }
        // Categories are deliberately *not* matchable, for the same reason a
        // code-hosting homepage is not (see `matcher::effective_keywords`): a
        // category names the bucket a catalog files the plugin under, not what
        // the plugin is. The bundled catalog buckets read `development`,
        // `productivity`, `design`, `testing`, `security` — ordinary English
        // words, shared by up to 121 entries each — so folding them in made a
        // normal sentence open an unsolicited install prompt. Scored
        // `/plugin suggest` still weighs them (`index_entry_from_marketplace`);
        // that path is user-invoked and ranked, not a proactive interruption.
        let mut domains = Vec::new();
        if let Some(homepage) = &candidate.homepage {
            domains.push(homepage.clone());
        }
        let next_step = match &candidate.install_plan {
            crate::plugins::marketplace::types::MarketplaceInstallPlan::Supported {
                spec, ..
            } if candidate.catalog_id.as_str().is_empty() => {
                PluginNextStep::SourceInstall { spec: spec.clone() }
            }
            _ => PluginNextStep::MarketplaceInstall {
                catalog_id: candidate.catalog_id.as_str().to_string(),
            },
        };
        out.push(PluginKeywordMatch {
            matched_term: None,
            name: candidate.name.clone(),
            id: candidate.id.as_str().to_string(),
            next_step,
            keywords,
            domains,
        });
    }
    out
}

/// One matcher-driven hit for a live draft. Already-active plugins never
/// match. `/plugin install` is returned only when a catalog entry carries a
/// real install spec.
#[must_use]
pub fn match_plugin_for_draft(
    draft: &str,
    registry: &PluginRegistry,
    marketplace: &[MarketplaceCandidate],
    dismissed: &BTreeSet<String>,
) -> Option<PluginKeywordMatch> {
    let mut candidates = idle_and_catalog_keyword_matches(registry, marketplace);
    candidates.retain(|candidate| !dismissed.contains(&candidate.name.to_ascii_lowercase()));
    match_plugin_for_draft_among(draft, &candidates)
}

#[must_use]
pub fn match_plugin_for_draft_among(
    draft: &str,
    candidates: &[PluginKeywordMatch],
) -> Option<PluginKeywordMatch> {
    let keyword_candidates = candidates
        .iter()
        .map(|candidate| crate::plugins::matcher::KeywordCandidate {
            name: candidate.name.as_str(),
            domains: &candidate.domains,
            keywords: &candidate.keywords,
        })
        .collect::<Vec<_>>();
    let (idx, term) = crate::plugins::matcher::match_plugin_keyword(draft, &keyword_candidates)?;
    let mut matched = candidates.get(idx)?.clone();
    matched.matched_term = Some(term);
    Some(matched)
}

/// Per-Engine gate for the append-only `<recommended_plugins>` fragment.
///
/// A plugin id is suggested at most once per Engine lifetime, and dismissals
/// are honored through `Settings`.
///
/// Skill-name suppression (#6274) is gone with the #6290 rework: it existed
/// only because skill entries were catalogued as plugins and then had to be
/// suppressed by name — a snapshot-based check that missed mid-session
/// changes and never applied to the composer toast. Entry kinds now keep
/// skills out of the plugin pool entirely (see `MarketplaceEntryKind`).
#[derive(Debug, Default)]
pub struct RecommendedPluginGate {
    shown: BTreeSet<String>,
}

impl RecommendedPluginGate {
    /// True when this plugin may be suggested now: not already suggested in
    /// this Engine's lifetime. First admission records the plugin id.
    fn admits(&mut self, id: &str) -> bool {
        self.shown.insert(id.to_string())
    }
}

/// Append-only user-turn fragment. Never part of the pinned system prefix.
/// Bounded, omitted when nothing matches.
#[must_use]
pub fn recommended_plugins_user_fragment(
    draft: &str,
    registry: &PluginRegistry,
    marketplace: &[MarketplaceCandidate],
    gate: &mut RecommendedPluginGate,
) -> Option<String> {
    // Called once when composing a user turn, never from the render loop.
    // Read the shared preference so headless and long-lived Engines also
    // honor dismissals recorded by a TUI after Engine startup.
    let settings = crate::settings::Settings::load_read_only().unwrap_or_default();
    let matched = match_plugin_for_draft(
        draft,
        registry,
        marketplace,
        &settings.dismissed_plugin_suggestions,
    )?;
    // Once per Engine lifetime per plugin id. Skill exclusion happens a
    // layer down: skill-kind entries never enter the plugin pool (#6290).
    if !gate.admits(&matched.id) {
        return None;
    }
    let mut listed = vec![matched];
    listed.truncate(MAX_RECOMMENDED_PLUGINS);
    let body = listed
        .iter()
        .map(|plugin| format!("- {} ({})", plugin.name, plugin.id))
        .collect::<Vec<_>>()
        .join("\n");
    Some(format!(
        "<recommended_plugins>\n{RECOMMENDED_PLUGINS_INTRO}\n\n{body}\n</recommended_plugins>"
    ))
}

/// Resolve a model-requested plugin name against installed and catalog
/// entries. Fails closed (None) when the name is unknown.
#[must_use]
pub fn lookup_reviewable_plugin(
    name: &str,
    registry: &PluginRegistry,
    marketplace: &[MarketplaceCandidate],
) -> Option<PluginKeywordMatch> {
    let needle = name.trim();
    if needle.is_empty() {
        return None;
    }
    idle_and_catalog_keyword_matches(registry, marketplace)
        .into_iter()
        .find(|candidate| {
            candidate.name.eq_ignore_ascii_case(needle) || candidate.id.eq_ignore_ascii_case(needle)
        })
}

#[must_use]
pub fn recommend_plugins_for_task(
    task: &str,
    registry: &PluginRegistry,
    marketplace: &[MarketplaceCandidate],
    options: RecommendOptions,
) -> Vec<PluginTaskRecommendation> {
    let installed = registry.list();
    let installed_names = installed
        .iter()
        .map(|plugin| plugin.name().to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    let mut entries = Vec::new();
    for plugin in &installed {
        entries.push(index_entry_from_installed(plugin));
    }
    for candidate in marketplace {
        if candidate.has_errors() {
            continue;
        }
        if candidate.kind != crate::plugins::marketplace::types::MarketplaceEntryKind::Plugin {
            continue;
        }
        if installed_names.contains(&candidate.name.to_ascii_lowercase()) {
            continue;
        }
        entries.push(index_entry_from_marketplace(candidate));
    }
    recommend_from_entries(task, &entries, &installed, options)
}

fn index_entry_from_installed(plugin: &LoadedPlugin) -> (String, RegistryEntry) {
    let mut keywords = plugin.manifest.plugin.keywords.clone();
    keywords.push(plugin.name().to_string());
    let mut description_parts = plugin
        .manifest
        .plugin
        .description
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    for skill in &plugin.skill_snapshots {
        description_parts.push(skill.name.clone());
        description_parts.push(skill.description.clone());
        keywords.push(skill.name.clone());
        keywords.extend(skill.aliases.iter().cloned());
    }
    (
        format!("installed:{}", plugin.name()),
        RegistryEntry {
            source: plugin.id.as_str().to_string(),
            description: (!description_parts.is_empty()).then(|| description_parts.join(" ")),
            keywords,
            domains: plugin.inventory.network_hosts.clone(),
        },
    )
}

fn index_entry_from_marketplace(candidate: &MarketplaceCandidate) -> (String, RegistryEntry) {
    let mut keywords = candidate.keywords.clone();
    keywords.push(candidate.name.clone());
    if let Some(display) = &candidate.display_name {
        keywords.push(display.clone());
    }
    keywords.extend(candidate.categories.iter().cloned());
    (
        format!(
            "marketplace:{}:{}",
            candidate.catalog_id.as_str(),
            candidate.name
        ),
        RegistryEntry {
            source: format!(
                "marketplace:{}:{}",
                candidate.catalog_id.as_str(),
                candidate.name
            ),
            description: candidate.description.clone(),
            keywords,
            domains: Vec::new(),
        },
    )
}

fn recommend_from_entries(
    task: &str,
    entries: &[(String, RegistryEntry)],
    installed: &[&LoadedPlugin],
    options: RecommendOptions,
) -> Vec<PluginTaskRecommendation> {
    if options.limit == 0 {
        return Vec::new();
    }
    let index = RegistryDocument {
        skills: entries.iter().cloned().collect::<BTreeMap<_, _>>(),
    };
    let ranked = recommend_remote_skills(task, &index, options.limit.saturating_mul(2));
    let mut out = Vec::new();
    let mut seen_names = BTreeSet::new();
    for recommendation in ranked {
        if recommendation.score() < options.min_score {
            continue;
        }
        let (source, name, next_step) =
            match recommendation.entry.source.strip_prefix("marketplace:") {
                Some(rest) => {
                    let Some((catalog_id, name)) = rest.split_once(':') else {
                        continue;
                    };
                    (
                        PluginMatchSource::Marketplace {
                            catalog_id: catalog_id.to_string(),
                        },
                        name.to_string(),
                        PluginNextStep::MarketplaceInstall {
                            catalog_id: catalog_id.to_string(),
                        },
                    )
                }
                None => {
                    let Some(plugin) = installed
                        .iter()
                        .find(|plugin| plugin.id.as_str() == recommendation.entry.source)
                    else {
                        continue;
                    };
                    let next_step = if plugin.active() {
                        PluginNextStep::AlreadyActive
                    } else if !plugin.trusted() {
                        PluginNextStep::Trust
                    } else if !plugin.enabled {
                        PluginNextStep::Enable
                    } else {
                        PluginNextStep::Inspect
                    };
                    (
                        PluginMatchSource::Installed {
                            id: plugin.id.as_str().to_string(),
                        },
                        plugin.name().to_string(),
                        next_step,
                    )
                }
            };
        if !options.include_active && next_step == PluginNextStep::AlreadyActive {
            continue;
        }
        let name_key = name.to_ascii_lowercase();
        if !seen_names.insert(name_key) {
            continue;
        }
        out.push(PluginTaskRecommendation {
            name,
            source,
            matched_terms: recommendation.matched_terms.clone(),
            score: recommendation.score(),
            next_step,
        });
        if out.len() >= options.limit {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::marketplace::types::{
        CatalogProvenance, CatalogTier, MarketplaceCandidate, MarketplaceCandidateId,
        MarketplaceCatalogId, MarketplaceEntryKind, MarketplaceInstallPlan, MarketplaceSourceSpec,
    };
    use crate::test_support::{EnvVarGuard, lock_test_env};
    use std::fs;
    use tempfile::TempDir;

    fn write_keyword_bundle(
        root: &std::path::Path,
        name: &str,
        description: &str,
        keywords: &[&str],
    ) {
        let bundle = root.join(".codewhale/plugins").join(name);
        fs::create_dir_all(&bundle).unwrap();
        let keyword_list = keywords
            .iter()
            .map(|keyword| format!("\"{keyword}\""))
            .collect::<Vec<_>>()
            .join(", ");
        fs::write(
            bundle.join("plugin.toml"),
            format!(
                "schema_version = 1\n[plugin]\nname = \"{name}\"\nversion = \"1.0.0\"\ndescription = \"{description}\"\nkeywords = [{keyword_list}]\n"
            ),
        )
        .unwrap();
    }

    fn marketplace_candidate(catalog: &str, name: &str, keywords: &[&str]) -> MarketplaceCandidate {
        MarketplaceCandidate {
            id: MarketplaceCandidateId::new(&MarketplaceCatalogId::new(catalog), name),
            catalog_id: MarketplaceCatalogId::new(catalog),
            kind: MarketplaceEntryKind::Plugin,
            name: name.to_string(),
            display_name: Some(format!("{name} plugin")),
            icon: None,
            description: Some(format!("{name} integration")),
            version: None,
            author: None,
            homepage: None,
            repository: None,
            license: None,
            keywords: keywords.iter().map(|value| (*value).to_string()).collect(),
            categories: Vec::new(),
            source: MarketplaceSourceSpec::GitHub {
                owner: "example".to_string(),
                repo: name.to_string(),
                git_ref: None,
                sha: None,
            },
            install_plan: MarketplaceInstallPlan::Supported {
                spec: format!("github:example/{name}"),
                source_kind: "github".to_string(),
            },
            declared_components: None,
            compatibility: None,
            provenance: CatalogProvenance {
                tier: CatalogTier::Community,
                publisher: None,
                source_url: None,
            },
            when: None,
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn keyword_match_ranks_installed_supabase_plugin() {
        let _lock = lock_test_env();
        let root = TempDir::new().unwrap();
        let _home = EnvVarGuard::set("CODEWHALE_HOME", root.path().join("home"));
        write_keyword_bundle(
            root.path(),
            "supabase",
            "Hosted Postgres and auth",
            &["supabase", "postgres"],
        );
        let registry = crate::plugins::PluginDiscoveryContext::capture_pre_dotenv()
            .registry_for_workspace(root.path());

        let recs = recommend_plugins_for_task(
            "add supabase auth to this app",
            &registry,
            &[],
            RecommendOptions::default(),
        );
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].name, "supabase");
        assert!(recs[0].score > 0);
        assert_eq!(recs[0].next_step, PluginNextStep::Trust);
        assert_eq!(recs[0].command(), "/plugin trust supabase");
    }

    #[test]
    fn marketplace_fills_in_a_missing_plugin() {
        let _lock = lock_test_env();
        let root = TempDir::new().unwrap();
        let _home = EnvVarGuard::set("CODEWHALE_HOME", root.path().join("home"));
        let registry = crate::plugins::PluginDiscoveryContext::capture_pre_dotenv()
            .registry_for_workspace(root.path());
        let catalog = [marketplace_candidate("official", "supabase", &["supabase"])];

        let recs = recommend_plugins_for_task(
            "wire up supabase row level security",
            &registry,
            &catalog,
            RecommendOptions::default(),
        );
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].name, "supabase");
        assert_eq!(
            recs[0].next_step,
            PluginNextStep::MarketplaceInstall {
                catalog_id: "official".to_string()
            }
        );
        assert_eq!(
            recs[0].command(),
            "/plugin marketplace install official supabase"
        );
    }

    #[test]
    fn already_active_plugins_are_skipped_when_active_excluded() {
        let _lock = lock_test_env();
        let root = TempDir::new().unwrap();
        let _home = EnvVarGuard::set("CODEWHALE_HOME", root.path().join("home"));
        write_keyword_bundle(root.path(), "supabase", "Hosted Postgres", &["supabase"]);
        let mut registry = crate::plugins::PluginDiscoveryContext::capture_pre_dotenv()
            .registry_for_workspace(root.path())
            .as_ref()
            .clone();
        registry.trust("supabase").unwrap();
        registry.enable("supabase").unwrap();

        let recs = recommend_plugins_for_task(
            "add supabase auth",
            &registry,
            &[],
            RecommendOptions {
                include_active: false,
                ..RecommendOptions::default()
            },
        );
        assert!(recs.is_empty(), "{recs:?}");
    }

    #[test]
    fn recommended_plugins_fragment_present_for_matching_idle_plugin() {
        let _lock = lock_test_env();
        let root = TempDir::new().unwrap();
        let _home = EnvVarGuard::set("CODEWHALE_HOME", root.path().join("home"));
        write_keyword_bundle(root.path(), "supabase", "Hosted Postgres", &["supabase"]);
        let registry = crate::plugins::PluginDiscoveryContext::capture_pre_dotenv()
            .registry_for_workspace(root.path());

        let fragment = recommended_plugins_user_fragment(
            "add supabase auth to login",
            &registry,
            &[],
            &mut RecommendedPluginGate::default(),
        )
        .expect("idle plugin should produce a fragment");
        assert!(fragment.starts_with("<recommended_plugins>"));
        assert!(fragment.contains("- supabase ("));
        assert!(fragment.contains("</recommended_plugins>"));
        assert!(
            recommended_plugins_user_fragment(
                "fix the failing test",
                &registry,
                &[],
                &mut RecommendedPluginGate::default(),
            )
            .is_none()
        );
    }

    #[test]
    fn recommended_plugins_fragment_suggests_a_plugin_once_per_gate() {
        let _lock = lock_test_env();
        let root = TempDir::new().unwrap();
        let _home = EnvVarGuard::set("CODEWHALE_HOME", root.path().join("home"));
        write_keyword_bundle(root.path(), "supabase", "Hosted Postgres", &["supabase"]);
        let registry = crate::plugins::PluginDiscoveryContext::capture_pre_dotenv()
            .registry_for_workspace(root.path());

        let mut gate = RecommendedPluginGate::default();
        let first = recommended_plugins_user_fragment(
            "add supabase auth to login",
            &registry,
            &[],
            &mut gate,
        )
        .expect("first matching turn suggests the plugin");
        assert!(first.contains("- supabase ("));
        assert!(
            recommended_plugins_user_fragment(
                "add supabase auth to the signup flow",
                &registry,
                &[],
                &mut gate,
            )
            .is_none(),
            "a plugin id is suggested at most once per Engine lifetime (#6274)"
        );
    }

    /// A skill entry in a catalog is installable but is never a plugin
    /// suggestion — the structural replacement for #6274's name suppression.
    #[test]
    fn skill_entries_never_enter_the_plugin_suggestion_pool() {
        let _lock = lock_test_env();
        let root = TempDir::new().unwrap();
        let _home = EnvVarGuard::set("CODEWHALE_HOME", root.path().join("home"));
        let registry = crate::plugins::PluginRegistry::empty(root.path());
        let mut skill = marketplace_candidate("cw2", "test", &["test"]);
        skill.kind = MarketplaceEntryKind::Skill;
        let slice = std::slice::from_ref(&skill);
        assert!(
            idle_and_catalog_keyword_matches(&registry, slice).is_empty(),
            "a skill entry must not be a plugin candidate"
        );
        assert!(
            recommended_plugins_user_fragment(
                "run the test suite",
                &registry,
                slice,
                &mut RecommendedPluginGate::default(),
            )
            .is_none(),
            "a skill entry must not produce a <recommended_plugins> fragment"
        );

        // Control: the same entry as a plugin still matches, so the
        // exclusion is the kind and not a broken fixture.
        skill.kind = MarketplaceEntryKind::Plugin;
        assert!(
            recommended_plugins_user_fragment(
                "run the test suite",
                &registry,
                std::slice::from_ref(&skill),
                &mut RecommendedPluginGate::default(),
            )
            .is_some(),
            "the same entry as a plugin still matches"
        );
    }

    /// A catalog category names the bucket an entry is filed under, not what
    /// the plugin *is* — the same class of thing as a code-hosting homepage,
    /// which the matcher already refuses. Folding categories into the
    /// matchable keyword set made ordinary English in a draft ("productivity",
    /// "development") open an unsolicited install prompt.
    #[test]
    fn catalog_categories_never_fire_a_plugin_suggestion() {
        let _lock = lock_test_env();
        let root = TempDir::new().unwrap();
        let _home = EnvVarGuard::set("CODEWHALE_HOME", root.path().join("home"));
        let registry = crate::plugins::PluginRegistry::empty(root.path());
        let mut candidate = marketplace_candidate("anthropic", "receipts", &["expense"]);
        candidate.display_name = None;
        candidate.categories = vec!["productivity".to_string(), "development".to_string()];
        let slice = std::slice::from_ref(&candidate);
        let candidates = idle_and_catalog_keyword_matches(&registry, slice);
        assert_eq!(candidates.len(), 1, "fixture must supply one candidate");

        for draft in [
            "some notes on productivity today",
            "walk me through the development workflow",
        ] {
            assert!(
                match_plugin_for_draft_among(draft, &candidates).is_none(),
                "a category must not fire an install prompt: {draft}"
            );
        }

        // Control: the declared keyword and the entry name still match, so the
        // exclusion is the category and not a dead fixture.
        for (draft, term) in [
            ("track this expense", "expense"),
            ("open receipts", "receipts"),
        ] {
            let matched = match_plugin_for_draft_among(draft, &candidates)
                .unwrap_or_else(|| panic!("declared term must still match: {draft}"));
            assert_eq!(matched.name, "receipts");
            assert_eq!(matched.matched_term.as_deref(), Some(term));
        }
    }

    #[test]
    fn matcher_driven_cta_skips_already_active_plugins() {
        let _lock = lock_test_env();
        let root = TempDir::new().unwrap();
        let _home = EnvVarGuard::set("CODEWHALE_HOME", root.path().join("home"));
        write_keyword_bundle(root.path(), "supabase", "Hosted Postgres", &["supabase"]);
        let mut registry = crate::plugins::PluginDiscoveryContext::capture_pre_dotenv()
            .registry_for_workspace(root.path())
            .as_ref()
            .clone();
        registry.trust("supabase").unwrap();
        registry.enable("supabase").unwrap();

        assert!(
            match_plugin_for_draft("add supabase auth", &registry, &[], &BTreeSet::new()).is_none(),
            "active plugins must not produce a live CTA"
        );
    }
}
