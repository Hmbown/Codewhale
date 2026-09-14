//! Unified read-only inventory for Codewhale extensions.
//!
//! This is deliberately a projection over the existing owners of Hooks,
//! Plugins, Marketplace catalogs, Skills, and MCP. It has no registry, trust
//! database, installer, or network fetch of its own. Future actions emitted by
//! this view must delegate to the existing command/mutation controllers.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fmt::Write as _;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

use super::{
    CommandPaletteAction, ModalKind, ModalView, ViewAction, ViewEvent, render_modal_footer,
    render_underwater_surface, truncate_view_text,
};
use crate::tui::app::App;
use codewhale_localization::{Locale, MessageId, tr};
use codewhale_palette as palette;

fn localize(locale: Locale, id: MessageId, replacements: &[(&str, &str)]) -> String {
    let mut value = tr(locale, id).into_owned();
    for (name, replacement) in replacements {
        value = value.replace(&format!("{{{name}}}"), replacement);
    }
    value
}

/// All extension surfaces in display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionsTab {
    Hooks,
    Plugins,
    Marketplace,
    Skills,
    Mcp,
}

impl ExtensionsTab {
    pub const ALL: [Self; 5] = [
        Self::Hooks,
        Self::Plugins,
        Self::Marketplace,
        Self::Skills,
        Self::Mcp,
    ];

    #[must_use]
    fn label(self, locale: Locale) -> String {
        match self {
            Self::Hooks => tr(locale, MessageId::ExtensionsTabHooks),
            Self::Plugins => tr(locale, MessageId::ExtensionsTabPlugins),
            Self::Marketplace => tr(locale, MessageId::ExtensionsTabMarketplace),
            Self::Skills => tr(locale, MessageId::HelpSkills),
            Self::Mcp => tr(locale, MessageId::ConfigSectionMcp),
        }
        .into_owned()
    }

    const fn index(self) -> usize {
        match self {
            Self::Hooks => 0,
            Self::Plugins => 1,
            Self::Marketplace => 2,
            Self::Skills => 3,
            Self::Mcp => 4,
        }
    }

    const fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    const fn previous(self) -> Self {
        Self::ALL[(self.index() + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

/// A real capability contributed by one plugin product.
///
/// Recommendations use the same component vocabulary as installed plugin
/// bundles. An MCP, Skill, browser driver, or sandbox helper is therefore a
/// component of a product, not a parallel kind of install pretending to be a
/// complete plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginProductComponentKind {
    Mcp,
    Skills,
    BrowserDriver,
    SandboxRuntime,
    NativeRuntime,
}

impl PluginProductComponentKind {
    fn label(self, locale: Locale) -> String {
        match self {
            Self::Mcp => tr(locale, MessageId::ConfigSectionMcp),
            Self::Skills => tr(locale, MessageId::HelpSkills),
            Self::BrowserDriver => tr(locale, MessageId::ExtensionsComponentBrowserDriver),
            Self::SandboxRuntime => tr(locale, MessageId::ExtensionsComponentSandboxRuntime),
            Self::NativeRuntime => tr(locale, MessageId::ExtensionsComponentNativeRuntime),
        }
        .into_owned()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginProductComponent {
    pub kind: PluginProductComponentKind,
    pub name: String,
}

/// Marketplace-facing recommendation model.
///
/// `source_reference` is display provenance only. It is intentionally not an
/// install command or executable plan; explicit installation still enters the
/// reviewed plugin installer and trust flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginProduct {
    pub id: String,
    pub name: String,
    pub description: String,
    pub publisher: String,
    pub source_reference: String,
    pub components: Vec<PluginProductComponent>,
    pub maturity: String,
}

impl PluginProduct {
    fn into_row(self, locale: Locale) -> ExtensionItem {
        let mut components = String::new();
        for (index, component) in self.components.iter().enumerate() {
            if index > 0 {
                components.push_str(", ");
            }
            let _ = write!(
                components,
                "{} ({})",
                component.name,
                component.kind.label(locale)
            );
        }
        ExtensionItem {
            id: self.id,
            label: self.name,
            tone: ExtensionTone::Idle,
            description: self.description,
            state: self.maturity,
            detail: localize(
                locale,
                MessageId::ExtensionsProductDetail,
                &[
                    ("publisher", &self.publisher),
                    ("components", &components),
                    ("source", &self.source_reference),
                ],
            ),
            action: None,
            toggle: None,
            remove: None,
        }
    }
}

/// What a row's state *means*, independent of the words it uses to say it.
///
/// Every row on this screen used to paint in one colour, so twenty servers,
/// four of them broken, read as one undifferentiated wall — "incredibly
/// boring, plain, and hard on the eyes because of the sameness". The tone is
/// typed rather than sniffed out of the localized state string, because a
/// screen that only colours correctly in English is not coloured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExtensionTone {
    /// Working: connected, enabled, active.
    Ready,
    /// Wants a person: auth required, disconnected, not yet reviewed.
    Attention,
    /// Broken: an error or a rejected entry.
    Failure,
    /// Deliberately off, or simply not configured.
    #[default]
    Idle,
}

impl ExtensionTone {
    fn ink(self) -> codewhale_palette::ChromeInk {
        use codewhale_palette::ChromeInk;
        match self {
            Self::Ready => ChromeInk::Outcome,
            Self::Attention => ChromeInk::Attention,
            Self::Failure => ChromeInk::Failure,
            Self::Idle => ChromeInk::Metadata,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionItem {
    pub id: String,
    pub label: String,
    pub description: String,
    pub state: String,
    /// Semantic reading of `state`, resolved through the theme's ink grammar.
    pub tone: ExtensionTone,
    pub detail: String,
    pub action: Option<ExtensionAction>,
    /// Reversible on/off toggle for the row (`e`): enable or disable a
    /// plugin or MCP server without leaving the panel.
    pub toggle: Option<ExtensionAction>,
    /// Destructive removal for the row (`d` / Delete / right-click, armed and
    /// confirmed in two steps). Only MCP servers offer it today; plugins keep
    /// their reviewed uninstall flow.
    pub remove: Option<ExtensionAction>,
}

/// Where a row's command lands when the user activates it.
///
/// Every row used to close the panel and drop a slash command into the
/// transcript — inspecting a plugin closed the list and pasted its detail
/// into chat, and a mutation left every other row reading open-time state.
/// Only the row knows which its command is, so the disposition lives on the
/// action: mutations and inspects act in place, and flows that own a
/// different surface (an editor, OAuth login, a composer-bound trust token)
/// still yield the panel to them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowActionDisposition {
    /// Run the command with the panel open; the host refreshes the snapshot
    /// afterwards so every row re-reads live state.
    InPlace,
    /// In place, and the command's text output renders in a pager stacked on
    /// the panel — the inspect path that keeps detail out of the transcript.
    InPlacePager,
    /// The command owns a different surface; the panel yields to it.
    LeavePanel,
}

/// A row affordance. Executable actions route back through the existing slash
/// command controller; status-only actions explain why Enter will not mutate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtensionAction {
    Command {
        label: String,
        command: String,
        disposition: RowActionDisposition,
    },
    Status {
        label: String,
    },
}

impl ExtensionAction {
    fn label(&self) -> &str {
        match self {
            Self::Command { label, .. } | Self::Status { label } => label,
        }
    }

    fn command(&self) -> Option<&str> {
        match self {
            Self::Command { command, .. } => Some(command),
            Self::Status { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionGroup {
    pub id: String,
    pub label: String,
    pub items: Vec<ExtensionItem>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExtensionsTabModel {
    pub groups: Vec<ExtensionGroup>,
    pub problem: Option<String>,
}

/// Read model captured when the modal opens. No source is contacted over the
/// network and no extension process is started while building it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExtensionsSnapshot {
    tabs: [ExtensionsTabModel; 5],
    /// MCP manager generation and initializing flag at capture time. The
    /// open panel reports these on its bounded poll so the host rebuilds the
    /// model only when live state actually moved.
    pub mcp_generation: u64,
    pub mcp_initializing: bool,
}

impl ExtensionsSnapshot {
    #[must_use]
    pub fn from_app(app: &App) -> Self {
        let mut snapshot = Self {
            mcp_generation: app.mcp_snapshot_generation,
            mcp_initializing: app.mcp_initializing,
            ..Self::default()
        };
        snapshot.tabs[ExtensionsTab::Hooks.index()] = hooks_model(app, app.ui_locale);
        snapshot.tabs[ExtensionsTab::Plugins.index()] = plugins_model(app, app.ui_locale);
        snapshot.tabs[ExtensionsTab::Marketplace.index()] = marketplace_model(app, app.ui_locale);
        snapshot.tabs[ExtensionsTab::Skills.index()] = skills_model(app, app.ui_locale);
        snapshot.tabs[ExtensionsTab::Mcp.index()] = mcp_model(app, app.ui_locale);
        snapshot
            .with_recommendations(reviewed_product_catalog(app.ui_locale), app.ui_locale)
            .with_recommended_actions(app)
    }

    #[must_use]
    pub fn with_recommendations(mut self, products: Vec<PluginProduct>, locale: Locale) -> Self {
        if !products.is_empty() {
            self.tabs[ExtensionsTab::Marketplace.index()].groups.insert(
                0,
                ExtensionGroup {
                    id: "recommended".into(),
                    label: tr(locale, MessageId::ExtensionsGroupRecommended).into_owned(),
                    items: products
                        .into_iter()
                        .map(|product| product.into_row(locale))
                        .collect(),
                },
            );
        }
        self
    }

    fn with_recommended_actions(mut self, app: &App) -> Self {
        let configured = crate::mcp::load_config_with_workspace_and_plugins(
            &app.mcp_config_path,
            &app.workspace,
            app.plugin_registry.as_ref(),
        )
        .ok();
        let Some(group) = self.tabs[ExtensionsTab::Marketplace.index()]
            .groups
            .iter_mut()
            .find(|group| group.id == "recommended")
        else {
            return self;
        };

        if configured.is_none() {
            for item in &mut group.items {
                item.action = Some(ExtensionAction::Status {
                    label: tr(app.ui_locale, MessageId::PickerActionUnavailable).into_owned(),
                });
            }
            return self;
        }

        for item in &mut group.items {
            // The first-party row is not an MCP recommendation: the plugin is
            // already in the binary, so the row asks the registry what it
            // wants — trust it, enable it, or open it — through the same
            // ladder the Plugins tab uses.
            if item.id == "codewhale-computer-use" {
                item.action = match app
                    .plugin_registry
                    .list()
                    .into_iter()
                    .find(|plugin| plugin.name() == "computer-use")
                {
                    Some(plugin) => {
                        item.state = localized_plugin_state(app.ui_locale, plugin.state_label());
                        Some(plugin_row_action(app.ui_locale, plugin))
                    }
                    None => Some(ExtensionAction::Status {
                        label: tr(app.ui_locale, MessageId::PickerActionUnavailable).into_owned(),
                    }),
                };
                continue;
            }
            let recommendation = match item.id.as_str() {
                "playwright-browser" => Some(("playwright", "playwright")),
                "chrome-devtools" => Some(("chrome-devtools", "chrome-devtools")),
                "cua-computer-use" => Some(("cua-driver", "cua")),
                _ => None,
            };
            if let Some((server_name, recommendation_id)) = recommendation {
                match configured
                    .as_ref()
                    .and_then(|config| config.servers.get(server_name))
                {
                    None => {
                        item.state =
                            tr(app.ui_locale, MessageId::ExtensionsStateAvailable).into_owned();
                        item.action = Some(ExtensionAction::Command {
                            label: tr(app.ui_locale, MessageId::ExtensionsActionAdd).into_owned(),
                            command: format!("/mcp add recommended {recommendation_id}"),
                            disposition: RowActionDisposition::InPlace,
                        });
                    }
                    Some(server) if !server.is_enabled() => {
                        item.state =
                            tr(app.ui_locale, MessageId::HotbarSetupStatusDisabled).into_owned();
                        item.action = Some(ExtensionAction::Command {
                            label: tr(app.ui_locale, MessageId::ExtensionsActionEnable)
                                .into_owned(),
                            command: format!("/mcp enable {server_name}"),
                            disposition: RowActionDisposition::InPlace,
                        });
                    }
                    Some(_) => {
                        item.state =
                            tr(app.ui_locale, MessageId::PickerActionConfigured).into_owned();
                        item.action = Some(ExtensionAction::Status {
                            label: tr(app.ui_locale, MessageId::PickerActionConfigured)
                                .into_owned(),
                        });
                    }
                }
            } else {
                item.action = Some(ExtensionAction::Status {
                    label: tr(app.ui_locale, MessageId::PickerActionUnavailable).into_owned(),
                });
            }
        }
        self
    }

    fn tab(&self, tab: ExtensionsTab) -> &ExtensionsTabModel {
        &self.tabs[tab.index()]
    }
}

/// Pinned review metadata only. These rows do not contain install commands,
/// do not fetch anything, and do not grant trust. The source-specific plugin
/// manifests produced by the packaging lane remain the installation authority.
fn reviewed_product_catalog(locale: Locale) -> Vec<PluginProduct> {
    vec![
        // Codewhale's own, and the reason this row exists: someone browsing
        // the marketplace for computer use saw Cua and Browser Use and not
        // the plugin that already ships inside the binary.
        PluginProduct {
            id: "codewhale-computer-use".into(),
            name: "Computer Use".into(),
            description: tr(
                locale,
                MessageId::ExtensionsProductCodewhaleComputerUseDescription,
            )
            .into_owned(),
            publisher: "Codewhale".into(),
            source_reference: "crates/tui/plugins/computer-use".into(),
            components: vec![
                PluginProductComponent {
                    kind: PluginProductComponentKind::Mcp,
                    name: "Computer Use MCP".into(),
                },
                PluginProductComponent {
                    kind: PluginProductComponentKind::Skills,
                    name: "Computer Use Skill".into(),
                },
            ],
            maturity: tr(locale, MessageId::ExtensionsStateFirstParty).into_owned(),
        },
        PluginProduct {
            id: "playwright-browser".into(),
            name: "Playwright Browser".into(),
            description: tr(locale, MessageId::ExtensionsProductPlaywrightDescription).into_owned(),
            publisher: "Microsoft".into(),
            source_reference: "microsoft/playwright-mcp".into(),
            components: vec![
                PluginProductComponent {
                    kind: PluginProductComponentKind::Mcp,
                    name: "Playwright MCP".into(),
                },
                PluginProductComponent {
                    kind: PluginProductComponentKind::BrowserDriver,
                    name: "Playwright browser driver".into(),
                },
            ],
            maturity: tr(locale, MessageId::ExtensionsStateReviewedCandidate).into_owned(),
        },
        PluginProduct {
            id: "chrome-devtools".into(),
            name: "Chrome DevTools".into(),
            description: tr(locale, MessageId::ExtensionsProductChromeDescription).into_owned(),
            publisher: "Chrome DevTools".into(),
            source_reference: "ChromeDevTools/chrome-devtools-mcp".into(),
            components: vec![
                PluginProductComponent {
                    kind: PluginProductComponentKind::Mcp,
                    name: "Chrome DevTools MCP".into(),
                },
                PluginProductComponent {
                    kind: PluginProductComponentKind::BrowserDriver,
                    name: "Chrome".into(),
                },
            ],
            maturity: tr(locale, MessageId::ExtensionsStateReviewedCandidate).into_owned(),
        },
        PluginProduct {
            id: "cua-computer-use".into(),
            name: "Cua Computer Use".into(),
            description: tr(locale, MessageId::ExtensionsProductCuaDescription).into_owned(),
            publisher: "Cua".into(),
            source_reference: "trycua/cua".into(),
            components: vec![PluginProductComponent {
                kind: PluginProductComponentKind::NativeRuntime,
                name: "Cua Driver".into(),
            }],
            maturity: tr(locale, MessageId::ExtensionsStateUnderEvaluation).into_owned(),
        },
        PluginProduct {
            id: "browser-use".into(),
            name: "Browser Use".into(),
            description: tr(locale, MessageId::ExtensionsProductBrowserUseDescription).into_owned(),
            publisher: "Browser Use".into(),
            source_reference: "browser-use/browser-use".into(),
            components: vec![
                PluginProductComponent {
                    kind: PluginProductComponentKind::Skills,
                    name: "Browser Use Skill".into(),
                },
                PluginProductComponent {
                    kind: PluginProductComponentKind::BrowserDriver,
                    name: "Browser Use runtime".into(),
                },
            ],
            maturity: tr(locale, MessageId::ExtensionsStateReviewedCandidate).into_owned(),
        },
        PluginProduct {
            id: "anthropic-sandbox-runtime".into(),
            name: "Sandbox Runtime".into(),
            description: tr(locale, MessageId::ExtensionsProductSandboxDescription).into_owned(),
            publisher: "Anthropic Experimental".into(),
            source_reference: "anthropic-experimental/sandbox-runtime".into(),
            components: vec![PluginProductComponent {
                kind: PluginProductComponentKind::SandboxRuntime,
                name: "Sandbox Runtime".into(),
            }],
            maturity: tr(locale, MessageId::ExtensionsStateBetaCandidate).into_owned(),
        },
    ]
}

fn hooks_model(app: &App, locale: Locale) -> ExtensionsTabModel {
    let config = app.hooks.config();
    let configured = config
        .hooks
        .iter()
        .enumerate()
        .map(|(index, hook)| ExtensionItem {
            id: format!("hook-{index}"),
            tone: if config.enabled {
                ExtensionTone::Ready
            } else {
                ExtensionTone::Idle
            },
            label: hook.name.clone().unwrap_or_else(|| {
                localize(
                    locale,
                    MessageId::ExtensionsHookFallback,
                    &[("event", hook.event.as_str())],
                )
            }),
            description: hook.event.as_str().to_string(),
            state: if config.enabled {
                tr(locale, MessageId::ExtensionsStateEnabled)
            } else {
                tr(locale, MessageId::HotbarSetupStatusDisabled)
            }
            .into_owned(),
            detail: localize(
                locale,
                MessageId::ExtensionsHookDetail,
                &[
                    ("timeout", &hook.timeout_secs.to_string()),
                    ("background", &localized_bool(locale, hook.background)),
                    (
                        "continue_on_error",
                        &localized_bool(locale, hook.continue_on_error),
                    ),
                ],
            ),
            action: Some(ExtensionAction::Command {
                label: tr(locale, MessageId::ExtensionsActionEdit).into_owned(),
                command: "/hooks edit".into(),
                disposition: RowActionDisposition::LeavePanel,
            }),
            toggle: None,
            remove: None,
        })
        .collect::<Vec<_>>();
    let problems = config
        .problems
        .iter()
        .enumerate()
        .map(|(index, problem)| ExtensionItem {
            id: format!("hook-problem-{index}"),
            tone: if problem.rejected {
                ExtensionTone::Failure
            } else {
                ExtensionTone::Attention
            },
            label: problem.name.clone().unwrap_or_else(|| {
                tr(locale, MessageId::ExtensionsHooksConfiguration).into_owned()
            }),
            description: problem.detail.clone(),
            state: if problem.rejected {
                tr(locale, MessageId::ExtensionsStateRejected)
            } else {
                tr(locale, MessageId::ExtensionsStateWarning)
            }
            .into_owned(),
            detail: problem.summary(),
            action: Some(ExtensionAction::Command {
                label: tr(locale, MessageId::ExtensionsActionEdit).into_owned(),
                command: "/hooks edit".into(),
                disposition: RowActionDisposition::LeavePanel,
            }),
            toggle: None,
            remove: None,
        })
        .collect::<Vec<_>>();
    let mut groups = Vec::new();
    if !configured.is_empty() {
        groups.push(ExtensionGroup {
            id: "configured".into(),
            label: tr(locale, MessageId::ExtensionsGroupConfigured).into_owned(),
            items: configured,
        });
    }
    if !problems.is_empty() {
        groups.push(ExtensionGroup {
            id: "problems".into(),
            label: tr(locale, MessageId::ExtensionsGroupProblems).into_owned(),
            items: problems,
        });
    }
    // A screen with nothing on it and nothing to press is where "need to be
    // able to add hooks!" comes from. The row that teaches the file is the
    // row that opens it.
    if groups.is_empty() {
        groups.push(ExtensionGroup {
            id: "start".into(),
            label: tr(locale, MessageId::ExtensionsGroupConfigured).into_owned(),
            items: vec![ExtensionItem {
                id: "hooks-add".into(),
                tone: ExtensionTone::Idle,
                label: tr(locale, MessageId::ExtensionsHooksAddLabel).into_owned(),
                description: tr(locale, MessageId::ExtensionsHooksAddDescription).into_owned(),
                state: tr(locale, MessageId::ExtensionsStateAvailable).into_owned(),
                detail: ".codewhale/hooks.toml".into(),
                action: Some(ExtensionAction::Command {
                    label: tr(locale, MessageId::ExtensionsActionEdit).into_owned(),
                    command: "/hooks edit".into(),
                    disposition: RowActionDisposition::LeavePanel,
                }),
                toggle: None,
                remove: None,
            }],
        });
    }
    ExtensionsTabModel {
        groups,
        problem: None,
    }
}

fn inventory_summary(
    inventory: &crate::plugins::manifest::PluginInventory,
    locale: Locale,
) -> String {
    let mut parts = Vec::new();
    if inventory.skills > 0 {
        parts.push(localize(
            locale,
            MessageId::ExtensionsInventorySkills,
            &[("count", &inventory.skills.to_string())],
        ));
    }
    if inventory.mcp_servers > 0 {
        parts.push(localize(
            locale,
            MessageId::ExtensionsInventoryMcp,
            &[("count", &inventory.mcp_servers.to_string())],
        ));
    }
    if inventory.hooks > 0 {
        parts.push(localize(
            locale,
            MessageId::ExtensionsInventoryHooks,
            &[("count", &inventory.hooks.to_string())],
        ));
    }
    if inventory.commands > 0 {
        parts.push(localize(
            locale,
            MessageId::ExtensionsInventoryCommands,
            &[("count", &inventory.commands.to_string())],
        ));
    }
    if inventory.agents > 0 {
        parts.push(localize(
            locale,
            MessageId::ExtensionsInventoryAgents,
            &[("count", &inventory.agents.to_string())],
        ));
    }
    if parts.is_empty() {
        tr(locale, MessageId::ExtensionsInventoryNone).into_owned()
    } else {
        parts.join(", ")
    }
}

fn localized_bool(locale: Locale, value: bool) -> String {
    tr(
        locale,
        if value {
            MessageId::ExtensionsValueYes
        } else {
            MessageId::ExtensionsValueNo
        },
    )
    .into_owned()
}

fn localized_plugin_state(locale: Locale, state: &str) -> String {
    let id = match state {
        "active" => MessageId::CtxInspActive,
        "disabled" => MessageId::HotbarSetupStatusDisabled,
        "enabled-untrusted" => MessageId::ExtensionsStateEnabledUntrusted,
        "unstaged" => MessageId::ExtensionsStateUnstaged,
        "inapplicable" => MessageId::ExtensionsStateInapplicable,
        "unsupported" => MessageId::ExtensionsStateUnsupported,
        "inactive" => MessageId::ExtensionsStateInactive,
        _ => return state.to_string(),
    };
    tr(locale, id).into_owned()
}

fn localized_trust(locale: Locale, trust: &str) -> String {
    let id = match trust {
        "trusted" => MessageId::ExtensionsTrustTrusted,
        "not-reviewed" => MessageId::ExtensionsTrustNotReviewed,
        "content-changed" => MessageId::ExtensionsTrustContentChanged,
        "capabilities-changed" => MessageId::ExtensionsTrustCapabilitiesChanged,
        _ => return trust.to_string(),
    };
    tr(locale, id).into_owned()
}

fn localized_compatibility(locale: Locale, compatibility: &str) -> String {
    let id = match compatibility {
        "full" => MessageId::ExtensionsCompatibilityFull,
        "partial" => MessageId::ExtensionsCompatibilityPartial,
        "unsupported" => MessageId::ExtensionsStateUnsupported,
        _ => return compatibility.to_string(),
    };
    tr(locale, id).into_owned()
}

fn localized_tier(locale: Locale, tier: &str) -> String {
    let id = match tier {
        "community" => MessageId::ExtensionsTierCommunity,
        "official" => MessageId::ExtensionsTierOfficial,
        "curated" => MessageId::ExtensionsTierCurated,
        "partner" => MessageId::ExtensionsTierPartner,
        _ => return tier.to_string(),
    };
    tr(locale, id).into_owned()
}

fn localized_skill_root(locale: Locale, kind: crate::skills::roots::SkillRootKind) -> String {
    use crate::skills::roots::SkillRootKind;

    match kind {
        SkillRootKind::CodeWhaleProject => {
            tr(locale, MessageId::ExtensionsSkillRootProject).into_owned()
        }
        SkillRootKind::CodeWhaleGlobal => {
            tr(locale, MessageId::ExtensionsSkillRootGlobal).into_owned()
        }
        SkillRootKind::CompatibleProject(harness) => localize(
            locale,
            MessageId::ExtensionsSkillRootCompatibleProject,
            &[("harness", harness.label())],
        ),
        SkillRootKind::CompatibleGlobal(harness) => localize(
            locale,
            MessageId::ExtensionsSkillRootCompatibleGlobal,
            &[("harness", harness.label())],
        ),
        SkillRootKind::Configured => {
            tr(locale, MessageId::ExtensionsSkillRootConfigured).into_owned()
        }
        SkillRootKind::BuiltIn => tr(locale, MessageId::ExtensionsGroupBuiltIn).into_owned(),
        SkillRootKind::ReviewedPluginSnapshot => {
            tr(locale, MessageId::ExtensionsSkillRootReviewedPlugin).into_owned()
        }
        SkillRootKind::RegistryCache => {
            tr(locale, MessageId::ExtensionsSkillRootRegistryCache).into_owned()
        }
    }
}

/// The one action a plugin row offers, wherever that row is drawn.
///
/// The Plugins tab and the marketplace's first-party row both need "what does
/// this plugin want from me right now?", and a second copy of the ladder is
/// how the marketplace ends up offering `Enable` for something already active.
fn plugin_row_action(
    locale: Locale,
    plugin: &crate::plugins::types::LoadedPlugin,
) -> ExtensionAction {
    let has_error_diagnostics = plugin
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.level == crate::plugins::types::PluginDiagnosticLevel::Error);
    if has_error_diagnostics {
        ExtensionAction::Command {
            label: tr(locale, MessageId::ExtensionsActionDiagnose).into_owned(),
            command: format!("/plugin validate {}", plugin.name()),
            disposition: RowActionDisposition::InPlacePager,
        }
    } else if plugin.active() {
        ExtensionAction::Command {
            label: tr(locale, MessageId::LaunchHintOpen).into_owned(),
            command: format!("/plugin show {}", plugin.name()),
            disposition: RowActionDisposition::InPlacePager,
        }
    } else if plugin.trusted() && !plugin.enabled {
        ExtensionAction::Command {
            label: tr(locale, MessageId::ExtensionsActionEnable).into_owned(),
            command: format!("/plugin enable {}", plugin.name()),
            disposition: RowActionDisposition::InPlace,
        }
    } else {
        // The command opens the exact-content review with its confirmation
        // control, so this panel yields to that review.
        ExtensionAction::Command {
            label: tr(locale, MessageId::AutomationActionInspect).into_owned(),
            command: format!("/plugin trust {}", plugin.name()),
            disposition: RowActionDisposition::LeavePanel,
        }
    }
}

/// The reversible on/off control for a plugin row. A trusted plugin can be
/// switched off and on from the panel; an untrusted one still goes through
/// its reviewed trust flow first, so no toggle is offered.
fn plugin_row_toggle(
    locale: Locale,
    plugin: &crate::plugins::types::LoadedPlugin,
) -> Option<ExtensionAction> {
    if !plugin.trusted() {
        return None;
    }
    Some(if plugin.enabled {
        ExtensionAction::Command {
            // English fallback until the Extensions vocabulary gains a
            // localized "disable" (#3167 tracks the panel's localization).
            label: "disable".into(),
            command: format!("/plugin disable {}", plugin.name()),
            disposition: RowActionDisposition::InPlace,
        }
    } else {
        ExtensionAction::Command {
            label: tr(locale, MessageId::ExtensionsActionEnable).into_owned(),
            command: format!("/plugin enable {}", plugin.name()),
            disposition: RowActionDisposition::InPlace,
        }
    })
}

/// How a plugin row reads, using the same ladder as [`plugin_row_action`].
fn plugin_row_tone(plugin: &crate::plugins::types::LoadedPlugin) -> ExtensionTone {
    let has_error_diagnostics = plugin
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.level == crate::plugins::types::PluginDiagnosticLevel::Error);
    if has_error_diagnostics {
        ExtensionTone::Failure
    } else if plugin.active() {
        ExtensionTone::Ready
    } else if plugin.trusted() {
        // Trusted and deliberately disabled: off, not wrong.
        ExtensionTone::Idle
    } else {
        // Untrusted is not broken either; it is waiting on a person.
        ExtensionTone::Attention
    }
}

fn plugins_model(app: &App, locale: Locale) -> ExtensionsTabModel {
    let mut by_scope = [Vec::new(), Vec::new(), Vec::new()];
    for plugin in app.plugin_registry.list() {
        let scope = match plugin.scope {
            crate::plugins::types::PluginScope::Builtin => 0,
            crate::plugins::types::PluginScope::User => 1,
            crate::plugins::types::PluginScope::Workspace => 2,
        };
        let diagnostic_count = plugin.diagnostics.len();
        let action = plugin_row_action(locale, plugin);
        let toggle = plugin_row_toggle(locale, plugin);
        by_scope[scope].push(ExtensionItem {
            id: plugin.id.as_str().to_string(),
            tone: plugin_row_tone(plugin),
            label: plugin.name().to_string(),
            description: plugin
                .manifest
                .plugin
                .description
                .clone()
                .unwrap_or_else(|| inventory_summary(&plugin.inventory, locale)),
            state: localized_plugin_state(locale, plugin.state_label()),
            detail: localize(
                locale,
                MessageId::ExtensionsPluginDetail,
                &[
                    ("inventory", &inventory_summary(&plugin.inventory, locale)),
                    (
                        "trust",
                        &localized_trust(locale, plugin.trust_status.as_str()),
                    ),
                    (
                        "compatibility",
                        &localized_compatibility(locale, plugin.compatibility().as_str()),
                    ),
                    ("diagnostics", &diagnostic_count.to_string()),
                ],
            ),
            action: Some(action),
            toggle,
            remove: None,
        });
    }
    let labels = [
        (
            "builtin",
            tr(locale, MessageId::ExtensionsGroupBuiltIn).into_owned(),
        ),
        (
            "user",
            tr(locale, MessageId::ExtensionsGroupUser).into_owned(),
        ),
        (
            "workspace",
            tr(locale, MessageId::ExtensionsGroupWorkspace).into_owned(),
        ),
    ];
    let mut groups = labels
        .into_iter()
        .zip(by_scope)
        .filter(|(_, items)| !items.is_empty())
        .map(|((id, label), items)| ExtensionGroup {
            id: id.into(),
            label,
            items,
        })
        .collect::<Vec<_>>();
    let problems = app
        .plugin_registry
        .diagnostics()
        .iter()
        .enumerate()
        .map(|(index, diagnostic)| ExtensionItem {
            id: format!("plugin-diagnostic-{index}"),
            tone: ExtensionTone::Failure,
            label: diagnostic.code.to_string(),
            description: diagnostic.message.clone(),
            state: if diagnostic.level == crate::plugins::types::PluginDiagnosticLevel::Error {
                tr(locale, MessageId::ExtensionsStateInvalid)
            } else {
                tr(locale, MessageId::ExtensionsStateWarning)
            }
            .into_owned(),
            detail: diagnostic
                .path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| diagnostic.message.clone()),
            action: Some(ExtensionAction::Command {
                label: tr(locale, MessageId::ExtensionsActionDiagnose).into_owned(),
                command: "/plugin validate".into(),
                disposition: RowActionDisposition::InPlacePager,
            }),
            toggle: None,
            remove: None,
        })
        .collect::<Vec<_>>();
    if !problems.is_empty() {
        groups.push(ExtensionGroup {
            id: "problems".into(),
            label: tr(locale, MessageId::ExtensionsGroupProblems).into_owned(),
            items: problems,
        });
    }
    ExtensionsTabModel {
        groups,
        problem: app.plugin_registry.state_error().map(ToString::to_string),
    }
}

fn marketplace_model(app: &App, locale: Locale) -> ExtensionsTabModel {
    use crate::plugins::marketplace::document::{
        CatalogInstallResolution, resolve_candidate_install,
    };
    let Some(store) = crate::plugins::marketplace::store::MarketplaceStore::open(
        app.plugin_registry.state_path(),
    ) else {
        return ExtensionsTabModel {
            groups: Vec::new(),
            problem: Some(tr(locale, MessageId::ExtensionsMarketplaceUnavailable).into_owned()),
        };
    };
    let state = match store.load() {
        Ok(state) => state,
        Err(error) => {
            return ExtensionsTabModel {
                groups: Vec::new(),
                problem: Some(error),
            };
        }
    };
    let groups = state
        .catalogs()
        .values()
        .map(|stored| {
            let catalog = &stored.catalog;
            ExtensionGroup {
                id: catalog.id.as_str().to_string(),
                label: catalog
                    .display_name
                    .clone()
                    .unwrap_or_else(|| catalog.name.clone()),
                items: catalog
                    .candidates
                    .iter()
                    .map(|candidate| {
                        let resolution =
                            resolve_candidate_install(stored, candidate, &app.plugin_registry);
                        if let CatalogInstallResolution::AlreadyPresent { plugin, .. } = &resolution
                        {
                            // A catalog name match is only occupancy. Show and
                            // review the actual local bundle, not catalog claims.
                            return ExtensionItem {
                                id: candidate.id.as_str().to_string(),
                                tone: plugin_row_tone(plugin),
                                label: plugin.name().to_string(),
                                description: plugin
                                    .manifest
                                    .plugin
                                    .description
                                    .clone()
                                    .unwrap_or_default(),
                                state: if plugin.scope
                                    == crate::plugins::types::PluginScope::Builtin
                                {
                                    tr(locale, MessageId::ExtensionsStateFirstParty).into_owned()
                                } else {
                                    localized_plugin_state(locale, plugin.state_label())
                                },
                                detail: plugin.canonical_root.display().to_string(),
                                action: Some(plugin_row_action(locale, plugin)),
                                toggle: None,
                                remove: None,
                            };
                        }
                        let installable =
                            matches!(resolution, CatalogInstallResolution::Supported { .. });
                        ExtensionItem {
                            id: candidate.id.as_str().to_string(),
                            tone: ExtensionTone::Attention,
                            label: candidate
                                .display_name
                                .clone()
                                .unwrap_or_else(|| candidate.name.clone()),
                            description: candidate.description.clone().unwrap_or_default(),
                            state: if candidate.has_errors() {
                                tr(locale, MessageId::ExtensionsStateInvalid)
                            } else if installable {
                                tr(locale, MessageId::ExtensionsStateAvailable)
                            } else {
                                tr(locale, MessageId::AutomationActionInspect)
                            }
                            .into_owned(),
                            detail: {
                                let unknown = tr(locale, MessageId::CmdCostUnknownValue);
                                localize(
                                    locale,
                                    MessageId::ExtensionsMarketplaceDetail,
                                    &[
                                        (
                                            "publisher",
                                            candidate
                                                .provenance
                                                .publisher
                                                .as_deref()
                                                .unwrap_or(unknown.as_ref()),
                                        ),
                                        (
                                            "tier",
                                            &localized_tier(
                                                locale,
                                                candidate.provenance.tier.as_str(),
                                            ),
                                        ),
                                        ("installable", &localized_bool(locale, installable)),
                                    ],
                                )
                            },
                            action: if installable {
                                Some(ExtensionAction::Command {
                                    label: tr(locale, MessageId::ExtensionsActionAdd).into_owned(),
                                    command: format!(
                                        "/plugin marketplace install {} {}",
                                        catalog.id.as_str(),
                                        candidate.name
                                    ),
                                    disposition: RowActionDisposition::InPlace,
                                })
                            } else {
                                Some(ExtensionAction::Status {
                                    label: tr(locale, MessageId::PickerActionUnavailable)
                                        .into_owned(),
                                })
                            },
                            toggle: None,
                            remove: None,
                        }
                    })
                    .collect(),
            }
        })
        .collect();
    ExtensionsTabModel {
        groups,
        problem: None,
    }
}

fn skills_model(app: &App, locale: Locale) -> ExtensionsTabModel {
    use crate::skills::audit::{ParserState, SkillAuditMode, scan_with_configured};

    let home = crate::config::effective_home_dir();
    let audit = scan_with_configured(
        &app.workspace,
        home.as_deref(),
        Some(&app.skills_dir),
        SkillAuditMode::OwnedOnly,
        None,
    );
    let mut groups = Vec::<ExtensionGroup>::new();
    for skill in audit.skills {
        let group_id = format!("{:?}", skill.root.kind);
        let position = groups.iter().position(|group| group.id == group_id);
        let item = ExtensionItem {
            id: format!("{}:{}", group_id, skill.id.canonical_name),
            tone: ExtensionTone::Ready,
            label: skill.name,
            description: skill.description.unwrap_or_default(),
            state: match skill.parser {
                ParserState::Valid => tr(locale, MessageId::HotbarSetupStatusReady),
                ParserState::Warning(_) => tr(locale, MessageId::ExtensionsStateWarning),
                ParserState::Broken(_) | ParserState::Oversized => {
                    tr(locale, MessageId::ExtensionsStateInvalid)
                }
            }
            .into_owned(),
            detail: skill.safe_display_path,
            // Enter opens the skills manager, which is where install, update,
            // remove and trust already live (`views/skills_manager.rs`, 1,000
            // lines, driving `skills::mutation::SkillMutationRequest`). This
            // tab used to dead-end on `action: None` — founder live-test:
            // "skills - no way to delete them or edit or anything either" —
            // even though the manager it needed was one command away. Routing
            // rather than reimplementing: the mutation authority stays in one
            // place.
            action: Some(ExtensionAction::Command {
                label: tr(locale, MessageId::ExtensionsActionManage).into_owned(),
                command: "/skills".into(),
                disposition: RowActionDisposition::LeavePanel,
            }),
            toggle: None,
            remove: None,
        };
        if let Some(position) = position {
            groups[position].items.push(item);
        } else {
            groups.push(ExtensionGroup {
                id: group_id.clone(),
                label: localized_skill_root(locale, skill.root.kind),
                items: vec![item],
            });
        }
    }
    ExtensionsTabModel {
        groups,
        problem: None,
    }
}

fn mcp_model(app: &App, locale: Locale) -> ExtensionsTabModel {
    let configured = crate::mcp::load_config_with_workspace_and_plugins(
        &app.mcp_config_path,
        &app.workspace,
        app.plugin_registry.as_ref(),
    )
    .ok();
    let snapshot = app.mcp_snapshot.as_ref();
    // Configured names are the count authority used by the surrounding shell.
    // Snapshot data enriches those exact rows; it must never independently
    // filter the list down to only the last discovered subset.
    let names = configured.as_ref().map_or_else(
        || {
            snapshot
                .into_iter()
                .flat_map(|snapshot| snapshot.servers.iter().map(|server| server.name.clone()))
                .collect::<BTreeSet<_>>()
        },
        |config| config.servers.keys().cloned().collect::<BTreeSet<_>>(),
    );
    let total = names.len();
    let items: Vec<_> = names
        .into_iter()
        .map(|name| {
            let observed = snapshot
                .and_then(|snapshot| snapshot.servers.iter().find(|server| server.name == name));
            let config = configured
                .as_ref()
                .and_then(|configured| configured.servers.get(&name));
            let enabled = observed
                .map(|server| server.enabled)
                .or_else(|| config.map(crate::mcp::McpServerConfig::is_enabled))
                .unwrap_or(true);
            let initializing = app.mcp_initializing
                && enabled
                && observed.is_none_or(|server| !server.connected && server.error.is_none());
            let state = if !enabled {
                tr(locale, MessageId::HotbarSetupStatusDisabled)
            } else if initializing {
                Cow::Borrowed("connecting")
            } else if observed.is_some_and(|server| server.connected) {
                tr(locale, MessageId::ExtensionsStateConnected)
            } else if observed.is_some_and(|server| server.auth_required) {
                Cow::Owned(crate::tui::session_boot::mcp_auth_required_state_label())
            } else if observed.is_some_and(|server| server.error.is_some()) {
                tr(locale, MessageId::ExtensionsStateError)
            } else if observed.is_none() {
                tr(locale, MessageId::ExtensionsStateNotInspected)
            } else {
                tr(locale, MessageId::PickerActionConfigured)
            }
            .into_owned();
            let oauth_capable = config.is_some_and(crate::mcp::mcp_server_oauth_capable);
            let recovery = match observed {
                Some(server) => server.recovery_kind(oauth_capable),
                None => crate::mcp::mcp_recovery_kind(enabled, false, false, None, oauth_capable),
            };
            let action = match (initializing, recovery) {
                // Still connecting: the state is the whole story.
                (true, _) => ExtensionAction::Status {
                    label: state.clone(),
                },
                // Healthy. A row that needs nothing offers nothing — the
                // actionable rows are the ones worth finding in a list of 20.
                (false, None) => ExtensionAction::Status {
                    label: state.clone(),
                },
                (false, Some(recovery))
                    if crate::mcp::mcp_name_is_command_safe(&name)
                        || matches!(
                            recovery,
                            crate::mcp::McpRecoveryKind::Connect
                                | crate::mcp::McpRecoveryKind::Reconnect
                                | crate::mcp::McpRecoveryKind::Diagnose
                        ) =>
                {
                    ExtensionAction::Command {
                        label: tr(locale, recovery.label_key()).into_owned(),
                        command: recovery.slash_command(&name),
                        // Re-auth hands off to the OAuth login flow; every
                        // other recovery runs against live state the panel
                        // re-reads when it lands.
                        disposition: match recovery {
                            crate::mcp::McpRecoveryKind::Reauth => RowActionDisposition::LeavePanel,
                            _ => RowActionDisposition::InPlace,
                        },
                    }
                }
                (false, Some(_)) => ExtensionAction::Command {
                    label: tr(locale, MessageId::ExtensionsActionDiagnose).into_owned(),
                    command: "/mcp validate".into(),
                    disposition: RowActionDisposition::InPlace,
                },
            };
            let command_safe = crate::mcp::mcp_name_is_command_safe(&name);
            let toggle = command_safe.then(|| {
                if enabled {
                    ExtensionAction::Command {
                        label: "disable".into(),
                        command: format!("/mcp disable {name}"),
                        disposition: RowActionDisposition::InPlace,
                    }
                } else {
                    ExtensionAction::Command {
                        label: tr(locale, MessageId::ExtensionsActionEnable).into_owned(),
                        command: format!("/mcp enable {name}"),
                        disposition: RowActionDisposition::InPlace,
                    }
                }
            });
            let remove = command_safe.then(|| ExtensionAction::Command {
                label: "remove".into(),
                command: format!("/mcp remove {name}"),
                disposition: RowActionDisposition::InPlace,
            });
            ExtensionItem {
                id: name.clone(),
                tone: match (enabled, initializing, recovery) {
                    (false, ..) => ExtensionTone::Idle,
                    (true, true, _) => ExtensionTone::Attention,
                    (true, false, None) => ExtensionTone::Ready,
                    // A server that reports an error is broken; one that only
                    // wants a login or a reconnect is waiting on a person.
                    (true, false, Some(crate::mcp::McpRecoveryKind::Diagnose)) => {
                        ExtensionTone::Failure
                    }
                    (true, false, Some(_)) => ExtensionTone::Attention,
                },
                label: name,
                description: observed.map_or_else(String::new, |server| {
                    localize(
                        locale,
                        MessageId::ExtensionsMcpSummary,
                        &[
                            ("transport", &server.transport),
                            ("tools", &server.tools.len().to_string()),
                            ("resources", &server.resources.len().to_string()),
                        ],
                    )
                }),
                state,
                // The passive snapshot can carry a command line or URL. Do
                // not mirror either into this broad inventory surface.
                detail: observed.map_or_else(
                    || tr(locale, MessageId::ExtensionsMcpNotInspected).into_owned(),
                    |server| {
                        server.error.clone().unwrap_or_else(|| {
                            localize(
                                locale,
                                MessageId::ExtensionsMcpDetail,
                                &[
                                    ("tools", &server.tools.len().to_string()),
                                    ("resources", &server.resources.len().to_string()),
                                    ("prompts", &server.prompts.len().to_string()),
                                ],
                            )
                        })
                    },
                ),
                action: Some(action),
                toggle,
                remove,
            }
        })
        .collect();
    ExtensionsTabModel {
        groups: mcp_groups(locale, items),
        problem: (configured.is_none() && app.mcp_configured_count > total).then(|| {
            localize(
                locale,
                MessageId::ExtensionsMcpRefresh,
                &[("count", &app.mcp_configured_count.to_string())],
            )
        }),
    }
}

/// Group id of the `/mcp` rows whose one action is a login.
const MCP_LOGIN_GROUP_ID: &str = "login";

/// Whether a row's one action is the login flow.
fn mcp_item_needs_login(item: &ExtensionItem) -> bool {
    item.action
        .as_ref()
        .and_then(ExtensionAction::command)
        .is_some_and(|command| command.starts_with("/mcp login "))
}

/// Order the `/mcp` rows by what a person has to do about them. Everything
/// that needs a human leads: with twenty servers configured, the four that
/// failed or want re-auth were impossible to pick out of a flat alphabetical
/// list — founder live-test on the same screen. Within that, the servers
/// that only need a login come first, as their own group, because "failed"
/// is the wrong word for an expired login and the fix is one key (#5926):
/// Enter on the row runs `/mcp login <server>`. Real failures follow with
/// their reason in the detail line; a healthy server renders its state and
/// sorts below.
fn mcp_groups(locale: Locale, items: Vec<ExtensionItem>) -> Vec<ExtensionGroup> {
    let (login, rest): (Vec<_>, Vec<_>) = items.into_iter().partition(mcp_item_needs_login);
    let (attention, healthy): (Vec<_>, Vec<_>) = rest
        .into_iter()
        .partition(|item| item.action.as_ref().is_some_and(|a| a.command().is_some()));
    [
        (
            MCP_LOGIN_GROUP_ID,
            MessageId::ExtensionsGroupNeedsLogin,
            login,
        ),
        (
            "attention",
            MessageId::ExtensionsGroupNeedsAttention,
            attention,
        ),
        ("servers", MessageId::ExtensionsGroupServers, healthy),
    ]
    .into_iter()
    .filter(|(_, _, items)| !items.is_empty())
    .map(|(id, label, items)| ExtensionGroup {
        id: id.into(),
        label: tr(locale, label).into_owned(),
        items,
    })
    .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExtensionsFocus {
    Tabs,
    Search,
    List,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VisibleEntry<'a> {
    Group(&'a ExtensionGroup),
    Item(&'a ExtensionGroup, &'a ExtensionItem),
    Problem(&'a str),
    Empty,
}

#[derive(Default)]
struct HitAreas {
    tabs: Vec<(Rect, ExtensionsTab)>,
    search: Option<Rect>,
    rows: Vec<(Rect, usize)>,
}

pub struct ExtensionsView {
    snapshot: ExtensionsSnapshot,
    locale: Locale,
    active_tab: ExtensionsTab,
    focus: ExtensionsFocus,
    query: String,
    selected: [usize; 5],
    scroll: [usize; 5],
    folded_groups: BTreeSet<String>,
    /// The live theme, captured at open so row ink resolves through the same
    /// grammar the rest of the chrome uses instead of raw palette constants.
    theme: codewhale_palette::UiTheme,
    hits: RefCell<HitAreas>,
    /// Last time `tick` asked the host for a fresh snapshot. Bounds the poll
    /// so a per-frame tick cannot turn into a rebuild every frame.
    last_poll: std::time::Instant,
    /// Row id whose removal is armed. A second `d` / Delete / right-click on
    /// the same row confirms; any navigation or Esc disarms.
    pending_remove: Option<String>,
}

impl ExtensionsView {
    #[must_use]
    pub fn new(app: &App, tab: ExtensionsTab) -> Self {
        let mut view =
            Self::from_snapshot_with_locale(ExtensionsSnapshot::from_app(app), tab, app.ui_locale);
        view.theme = app.ui_theme;
        view
    }

    fn from_snapshot_with_locale(
        snapshot: ExtensionsSnapshot,
        tab: ExtensionsTab,
        locale: Locale,
    ) -> Self {
        let mut view = Self {
            snapshot,
            locale,
            active_tab: tab,
            focus: ExtensionsFocus::List,
            query: String::new(),
            selected: [0; 5],
            scroll: [0; 5],
            folded_groups: BTreeSet::new(),
            theme: codewhale_palette::UI_THEME,
            hits: RefCell::new(HitAreas::default()),
            last_poll: std::time::Instant::now(),
            pending_remove: None,
        };
        // `/mcp` opens on the first server that needs a login, not on that
        // group's heading, so the one key the screen advertises — Enter —
        // runs the login flow straight away (#5926).
        if view
            .snapshot
            .tab(tab)
            .groups
            .first()
            .is_some_and(|group| group.id == MCP_LOGIN_GROUP_ID)
        {
            view.selected[tab.index()] = 1;
        }
        view
    }

    fn fold_key(&self, group: &ExtensionGroup) -> String {
        format!("{}:{}", self.active_tab.index(), group.id)
    }

    fn group_matches(&self, group: &ExtensionGroup, query: &str) -> bool {
        group.label.to_lowercase().contains(query)
            || group.items.iter().any(|item| item_matches(item, query))
    }

    fn visible_entries(&self) -> Vec<VisibleEntry<'_>> {
        let model = self.snapshot.tab(self.active_tab);
        let query = self.query.trim().to_lowercase();
        let searching = !query.is_empty();
        let mut entries = Vec::new();
        if let Some(problem) = model.problem.as_deref() {
            entries.push(VisibleEntry::Problem(problem));
        }
        for group in &model.groups {
            if searching && !self.group_matches(group, &query) {
                continue;
            }
            entries.push(VisibleEntry::Group(group));
            let folded = !searching && self.folded_groups.contains(&self.fold_key(group));
            if folded {
                continue;
            }
            let group_name_matches = searching && group.label.to_lowercase().contains(&query);
            entries.extend(
                group
                    .items
                    .iter()
                    .filter(|item| !searching || group_name_matches || item_matches(item, &query))
                    .map(|item| VisibleEntry::Item(group, item)),
            );
        }
        if entries.is_empty() {
            entries.push(VisibleEntry::Empty);
        }
        entries
    }

    fn clamp_selection(&mut self) {
        let len = self.visible_entries().len();
        let index = self.active_tab.index();
        self.selected[index] = self.selected[index].min(len.saturating_sub(1));
        self.scroll[index] = self.scroll[index].min(self.selected[index]);
    }

    fn move_selection(&mut self, delta: isize) {
        self.pending_remove = None;
        let len = self.visible_entries().len();
        if len == 0 {
            return;
        }
        let index = self.active_tab.index();
        self.selected[index] =
            (self.selected[index] as isize + delta).rem_euclid(len as isize) as usize;
    }

    fn selected_item(&self) -> Option<&ExtensionItem> {
        let selected = self.selected[self.active_tab.index()];
        match self.visible_entries().get(selected).copied() {
            Some(VisibleEntry::Item(_, item)) => Some(item),
            _ => None,
        }
    }

    /// `e`: run the row's reversible on/off command in place.
    fn toggle_selected(&mut self) -> ViewAction {
        self.pending_remove = None;
        match self.selected_item().and_then(|item| item.toggle.as_ref()) {
            Some(ExtensionAction::Command { command, .. }) => {
                ViewAction::Emit(ViewEvent::ExecutePanelCommand {
                    command: command.clone(),
                    pager_title: None,
                })
            }
            _ => ViewAction::None,
        }
    }

    /// `d` / Delete / right-click: arm removal on the first gesture, run the
    /// row's remove command on the second. Rows without a remove command
    /// ignore the gesture.
    fn remove_selected(&mut self) -> ViewAction {
        let Some((id, command)) = self.selected_item().and_then(|item| match &item.remove {
            Some(ExtensionAction::Command { command, .. }) => {
                Some((item.id.clone(), command.clone()))
            }
            _ => None,
        }) else {
            self.pending_remove = None;
            return ViewAction::None;
        };
        if self.pending_remove.as_deref() == Some(id.as_str()) {
            self.pending_remove = None;
            return ViewAction::Emit(ViewEvent::ExecutePanelCommand {
                command,
                pager_title: None,
            });
        }
        self.pending_remove = Some(id);
        ViewAction::None
    }

    fn activate_selected(&mut self) -> ViewAction {
        let selected = self.selected[self.active_tab.index()];
        match self.visible_entries().get(selected).copied() {
            Some(VisibleEntry::Group(group)) => {
                let group = group.clone();
                let key = self.fold_key(&group);
                if !self.folded_groups.remove(&key) {
                    self.folded_groups.insert(key);
                }
                self.clamp_selection();
                ViewAction::None
            }
            Some(VisibleEntry::Item(_, item)) => match item.action.as_ref() {
                Some(ExtensionAction::Command {
                    command,
                    disposition,
                    ..
                }) => match disposition {
                    RowActionDisposition::LeavePanel => {
                        ViewAction::EmitAndClose(ViewEvent::CommandPaletteSelected {
                            action: CommandPaletteAction::ExecuteCommand {
                                command: command.clone(),
                            },
                        })
                    }
                    RowActionDisposition::InPlace => {
                        ViewAction::Emit(ViewEvent::ExecutePanelCommand {
                            command: command.clone(),
                            pager_title: None,
                        })
                    }
                    RowActionDisposition::InPlacePager => {
                        ViewAction::Emit(ViewEvent::ExecutePanelCommand {
                            command: command.clone(),
                            pager_title: Some(item.label.clone()),
                        })
                    }
                },
                _ => ViewAction::None,
            },
            _ => ViewAction::None,
        }
    }

    /// Swap in a fresh read model while keeping everything the user is doing:
    /// active tab, focus, search query, selection, scroll, and folded groups
    /// all survive; the selection only moves when the refreshed list no
    /// longer reaches it.
    pub fn refresh_snapshot(&mut self, snapshot: ExtensionsSnapshot) {
        self.snapshot = snapshot;
        self.clamp_selection();
    }

    fn set_tab(&mut self, tab: ExtensionsTab) {
        self.pending_remove = None;
        self.active_tab = tab;
        self.clamp_selection();
    }

    fn selected_status(&self) -> String {
        if let Some(item) = self.selected_item()
            && self.pending_remove.as_deref() == Some(item.id.as_str())
        {
            return format!(
                "Remove {}? Press d, Enter or right-click again to confirm · Esc cancels",
                item.label
            );
        }
        let index = self.selected[self.active_tab.index()];
        match self.visible_entries().get(index).copied() {
            Some(VisibleEntry::Group(group)) => localize(
                self.locale,
                MessageId::ExtensionsGroupStatus,
                &[("count", &group.items.len().to_string())],
            ),
            Some(VisibleEntry::Item(_, item)) => {
                let action = item
                    .action
                    .as_ref()
                    .map(|action| format!(" · {}", action.label()))
                    .unwrap_or_default();
                format!("{} · {}{action} · {}", item.label, item.state, item.detail)
            }
            Some(VisibleEntry::Problem(problem)) => problem.to_string(),
            Some(VisibleEntry::Empty) | None => {
                tr(self.locale, MessageId::ExtensionsNoItems).into_owned()
            }
        }
    }
}

fn item_matches(item: &ExtensionItem, query: &str) -> bool {
    item.label.to_lowercase().contains(query)
        || item.description.to_lowercase().contains(query)
        || item.state.to_lowercase().contains(query)
        || item.detail.to_lowercase().contains(query)
        || item
            .action
            .as_ref()
            .is_some_and(|action| action.label().to_lowercase().contains(query))
}

impl ModalView for ExtensionsView {
    fn kind(&self) -> ModalKind {
        ModalKind::Extensions
    }

    fn handle_key(&mut self, key: KeyEvent) -> ViewAction {
        // One navigation grammar (grokbuild, the stated authority): Tab and
        // Shift+Tab / BackTab move across the tab bar, always — even during
        // a search, which keeps its query on the new tab. `/` searches, Esc
        // backs out, ↑↓ move, Enter acts. Tab never cycles focus.
        if key.code == KeyCode::BackTab
            || (key.code == KeyCode::Tab && key.modifiers.contains(KeyModifiers::SHIFT))
        {
            self.set_tab(self.active_tab.previous());
            return ViewAction::None;
        }
        if key.code == KeyCode::Tab {
            self.set_tab(self.active_tab.next());
            return ViewAction::None;
        }
        if self.focus == ExtensionsFocus::Search {
            match key.code {
                KeyCode::Esc => {
                    if self.query.is_empty() {
                        self.focus = ExtensionsFocus::List;
                    } else {
                        self.query.clear();
                        self.clamp_selection();
                    }
                }
                KeyCode::Backspace => {
                    self.query.pop();
                    self.clamp_selection();
                }
                KeyCode::Enter | KeyCode::Down => self.focus = ExtensionsFocus::List,
                KeyCode::Char(ch)
                    if !key.modifiers.intersects(
                        KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER,
                    ) =>
                {
                    self.query.push(ch);
                    self.clamp_selection();
                }
                _ => {}
            }
            return ViewAction::None;
        }
        if self.pending_remove.is_some()
            && !matches!(
                key.code,
                KeyCode::Char('d') | KeyCode::Delete | KeyCode::Enter | KeyCode::Char('y')
            )
        {
            // Anything but the confirming key disarms a pending removal.
            self.pending_remove = None;
            if key.code == KeyCode::Esc {
                return ViewAction::None;
            }
        }
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => ViewAction::Close,
            KeyCode::Char('/') => {
                self.focus = ExtensionsFocus::Search;
                ViewAction::None
            }
            KeyCode::Char('e') => {
                self.focus = ExtensionsFocus::List;
                self.toggle_selected()
            }
            KeyCode::Char('d') | KeyCode::Delete => {
                self.focus = ExtensionsFocus::List;
                self.remove_selected()
            }
            KeyCode::Char('y') | KeyCode::Enter if self.pending_remove.is_some() => {
                self.remove_selected()
            }
            // Left/Right and `[`/`]` are the same move for hands that reach
            // for them; the advertised chord is Tab.
            KeyCode::Left | KeyCode::Char('[') | KeyCode::Char('h') => {
                self.set_tab(self.active_tab.previous());
                ViewAction::None
            }
            KeyCode::Right | KeyCode::Char(']') | KeyCode::Char('l') => {
                self.set_tab(self.active_tab.next());
                ViewAction::None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.focus = ExtensionsFocus::List;
                self.move_selection(-1);
                ViewAction::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.focus = ExtensionsFocus::List;
                self.move_selection(1);
                ViewAction::None
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.focus = ExtensionsFocus::List;
                self.activate_selected()
            }
            _ => ViewAction::None,
        }
    }

    fn handle_mouse(&mut self, mouse: MouseEvent) -> ViewAction {
        match mouse.kind {
            // The wheel moves this list, not the transcript behind it.
            MouseEventKind::ScrollUp => {
                self.pending_remove = None;
                self.focus = ExtensionsFocus::List;
                self.move_selection(-1);
                return ViewAction::None;
            }
            MouseEventKind::ScrollDown => {
                self.pending_remove = None;
                self.focus = ExtensionsFocus::List;
                self.move_selection(1);
                return ViewAction::None;
            }
            // Right-click on a row selects it and arms (then confirms) its
            // removal, the same two-step gesture as `d`.
            MouseEventKind::Down(MouseButton::Right) => {
                let row = self
                    .hits
                    .borrow()
                    .rows
                    .iter()
                    .find(|(rect, _)| rect.contains((mouse.column, mouse.row).into()))
                    .map(|(_, row)| *row);
                let Some(row) = row else {
                    self.pending_remove = None;
                    return ViewAction::None;
                };
                self.focus = ExtensionsFocus::List;
                if self.selected[self.active_tab.index()] != row {
                    self.pending_remove = None;
                    self.selected[self.active_tab.index()] = row;
                }
                return self.remove_selected();
            }
            MouseEventKind::Down(MouseButton::Left) => {}
            _ => return ViewAction::None,
        }
        let hits = self.hits.borrow();
        if let Some((_, tab)) = hits
            .tabs
            .iter()
            .find(|(rect, _)| rect.contains((mouse.column, mouse.row).into()))
            .copied()
        {
            drop(hits);
            self.focus = ExtensionsFocus::Tabs;
            self.set_tab(tab);
            return ViewAction::None;
        }
        if hits
            .search
            .is_some_and(|rect| rect.contains((mouse.column, mouse.row).into()))
        {
            drop(hits);
            self.focus = ExtensionsFocus::Search;
            return ViewAction::None;
        }
        if let Some((_, row)) = hits
            .rows
            .iter()
            .find(|(rect, _)| rect.contains((mouse.column, mouse.row).into()))
            .copied()
        {
            drop(hits);
            self.focus = ExtensionsFocus::List;
            self.selected[self.active_tab.index()] = row;
            return self.activate_selected();
        }
        ViewAction::None
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        let body = render_underwater_surface(
            area,
            buf,
            tr(self.locale, MessageId::ExtensionsTitle).into_owned(),
        );
        if body.width == 0 || body.height < 5 {
            return;
        }
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(1),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(body);

        let mut hits = HitAreas::default();
        let mut x = rows[0].x;
        let available = rows[0].right();
        for tab in ExtensionsTab::ALL {
            let label = if body.width < 58 && tab == ExtensionsTab::Marketplace {
                tr(self.locale, MessageId::ExtensionsTabMarketplaceCompact).into_owned()
            } else {
                tab.label(self.locale)
            };
            let width = (label.chars().count() as u16 + 2).min(available.saturating_sub(x));
            if width == 0 {
                break;
            }
            let tab_area = Rect::new(x, rows[0].y, width, 1);
            let active = tab == self.active_tab;
            let focused = active && self.focus == ExtensionsFocus::Tabs;
            let style = if focused {
                Style::default()
                    .fg(palette::WHALE_BG)
                    .bg(palette::WHALE_ACTION)
                    .add_modifier(Modifier::BOLD)
            } else if active {
                Style::default()
                    .fg(palette::WHALE_ACTION)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            } else {
                Style::default().fg(palette::TEXT_MUTED)
            };
            Paragraph::new(Line::from(Span::styled(format!(" {label} "), style)))
                .render(tab_area, buf);
            hits.tabs.push((tab_area, tab));
            x = x.saturating_add(width);
        }

        let search_style = if self.focus == ExtensionsFocus::Search {
            Style::default()
                .fg(palette::WHALE_ACTION)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette::TEXT_MUTED)
        };
        let cursor = if self.focus == ExtensionsFocus::Search {
            "_"
        } else {
            ""
        };
        Paragraph::new(Line::from(vec![
            Span::styled(
                tr(self.locale, MessageId::ExtensionsSearchLabel),
                search_style,
            ),
            Span::styled(
                format!("{}{cursor}", self.query),
                Style::default().fg(palette::TEXT_PRIMARY),
            ),
        ]))
        .render(rows[1], buf);
        hits.search = Some(rows[1]);

        let entries = self.visible_entries();
        let list_height = usize::from(rows[2].height);
        let mut scroll = self.scroll[self.active_tab.index()];
        let selected = self.selected[self.active_tab.index()];
        if selected < scroll {
            scroll = selected;
        } else if selected >= scroll.saturating_add(list_height.max(1)) {
            scroll = selected.saturating_sub(list_height.saturating_sub(1));
        }
        let spacious = area.width >= 64 && area.height >= 16;
        for (visible_offset, (entry_index, entry)) in entries
            .iter()
            .enumerate()
            .skip(scroll)
            .take(list_height)
            .enumerate()
        {
            let row_area = Rect::new(
                rows[2].x,
                rows[2].y.saturating_add(visible_offset as u16),
                rows[2].width,
                1,
            );
            let is_selected = entry_index == selected;
            let style = if is_selected && self.focus == ExtensionsFocus::List {
                Style::default()
                    .fg(palette::WHALE_BG)
                    .bg(palette::WHALE_ACTION)
            } else if is_selected {
                Style::default()
                    .fg(palette::WHALE_ACTION)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(palette::TEXT_PRIMARY)
            };
            // Rows are built as (text, optional ink) pairs. The ink is what
            // stops every row on the screen from reading the same: the action
            // chip is an invitation, the state is a verdict, the description
            // is background. A selected row keeps one style — a highlight the
            // eye can follow beats four colours fighting a fill.
            let mut parts: Vec<(String, Option<codewhale_palette::ChromeInk>)> = Vec::new();
            match entry {
                VisibleEntry::Group(group) => {
                    let folded = self.folded_groups.contains(&self.fold_key(group));
                    parts.push((
                        format!(
                            "{} {} ({})",
                            if folded { "▸" } else { "▾" },
                            group.label,
                            group.items.len()
                        ),
                        None,
                    ));
                }
                VisibleEntry::Item(_, item) => {
                    parts.push(("  ".into(), None));
                    if let Some(action) = item.action.as_ref() {
                        parts.push((
                            format!("[{}] ", action.label()),
                            Some(match action {
                                ExtensionAction::Command { .. } => {
                                    codewhale_palette::ChromeInk::Identity
                                }
                                ExtensionAction::Status { .. } => item.tone.ink(),
                            }),
                        ));
                    }
                    parts.push((item.label.clone(), None));
                    parts.push((format!(" [{}]", item.state), Some(item.tone.ink())));
                    if spacious && !item.description.is_empty() {
                        parts.push((
                            format!(" — {}", item.description),
                            Some(codewhale_palette::ChromeInk::MetadataHint),
                        ));
                    }
                }
                VisibleEntry::Problem(problem) => parts.push((
                    format!("! {problem}"),
                    Some(codewhale_palette::ChromeInk::Failure),
                )),
                VisibleEntry::Empty => parts.push((
                    if self.query.is_empty() {
                        tr(self.locale, MessageId::ExtensionsNoItems).into_owned()
                    } else {
                        localize(
                            self.locale,
                            MessageId::ExtensionsNoMatches,
                            &[("query", &self.query)],
                        )
                    },
                    Some(codewhale_palette::ChromeInk::MetadataHint),
                )),
            }

            // Truncate across the whole row, not per span, so the width bound
            // is the one the flat row always had.
            let joined = parts
                .iter()
                .map(|(text, _)| text.as_str())
                .collect::<String>();
            let clipped = truncate_view_text(&joined, usize::from(row_area.width));
            let spans = if is_selected || clipped.len() != joined.len() {
                vec![Span::styled(clipped, style)]
            } else {
                parts
                    .into_iter()
                    .filter(|(text, _)| !text.is_empty())
                    .map(|(text, ink)| {
                        let span_style = match ink {
                            Some(ink) => Style::default().fg(ink.color(&self.theme)),
                            None => style,
                        };
                        Span::styled(text, span_style)
                    })
                    .collect()
            };
            Paragraph::new(Line::from(spans)).render(row_area, buf);
            hits.rows.push((row_area, entry_index));
        }

        let status = truncate_view_text(&self.selected_status(), usize::from(rows[3].width));
        Paragraph::new(Line::from(Span::styled(
            status,
            Style::default().fg(palette::TEXT_MUTED),
        )))
        .render(rows[3], buf);
        let compact_hints = [
            super::ActionHint::new("Tab", tr(self.locale, MessageId::ExtensionsActionTabs)),
            super::ActionHint::new("/", tr(self.locale, MessageId::SessionsActionSearch)),
            super::ActionHint::new("Esc", tr(self.locale, MessageId::SessionsActionClose)),
        ];
        // Only advertise Enter when Enter does something. A `Status` action is
        // a state, not a verb: a row mid-connect labelled `connecting` produced
        // the hint "Enter connecting", and pressing it did nothing — which is
        // what makes a user press it again.
        let enter_label = match entries.get(selected).copied() {
            Some(VisibleEntry::Item(_, item)) => item
                .action
                .as_ref()
                .filter(|action| action.command().is_some())
                .map(|action| action.label().to_string()),
            _ => Some(tr(self.locale, MessageId::ExtensionsActionFold).into_owned()),
        };
        let mut full_hints = vec![
            super::ActionHint::new("Tab", tr(self.locale, MessageId::ExtensionsActionTabs)),
            super::ActionHint::new("↑↓", tr(self.locale, MessageId::LaunchHintMove)),
        ];
        if let Some(label) = enter_label {
            full_hints.push(super::ActionHint::new("Enter", label));
        }
        if let Some(item) = self.selected_item() {
            if let Some(toggle) = item.toggle.as_ref() {
                full_hints.push(super::ActionHint::new("e", toggle.label().to_string()));
            }
            if let Some(remove) = item.remove.as_ref() {
                full_hints.push(super::ActionHint::new("d", remove.label().to_string()));
            }
        }
        full_hints.push(super::ActionHint::new(
            "/",
            tr(self.locale, MessageId::SessionsActionSearch),
        ));
        full_hints.push(super::ActionHint::new(
            "Esc",
            tr(self.locale, MessageId::SessionsActionClose),
        ));
        render_modal_footer(
            rows[4],
            buf,
            if rows[4].width < 64 {
                &compact_hints
            } else {
                &full_hints
            },
        );
        *self.hits.borrow_mut() = hits;
    }

    fn tick(&mut self) -> ViewAction {
        // MCP rows go live while the panel is open — a retry lands, a login
        // finishes, a diagnosis resolves — and the open-time capture would
        // read stale until reopen. Ask the host for a fresh model at a
        // bounded cadence; it rebuilds only when the generation or the
        // initializing flag actually moved.
        if self.last_poll.elapsed() < std::time::Duration::from_millis(750) {
            return ViewAction::None;
        }
        self.last_poll = std::time::Instant::now();
        ViewAction::Emit(ViewEvent::RefreshExtensions {
            mcp_generation: self.snapshot.mcp_generation,
            mcp_initializing: self.snapshot.mcp_initializing,
        })
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::McpRecoveryKind;

    #[test]
    fn marketplace_shipped_bundle_uses_local_metadata_and_review_action() {
        let _env = crate::test_support::lock_test_env();
        let root = tempfile::tempdir().unwrap();
        let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", root.path());
        let registry = crate::plugins::PluginDiscoveryContext::capture_pre_dotenv()
            .registry_for_workspace(root.path());
        let app = App::new_with_plugin_registry(
            crate::test_support::test_tui_options(root.path()),
            &crate::config::Config::default(),
            registry,
        );
        let model = marketplace_model(&app, Locale::En);
        let group = model
            .groups
            .iter()
            .find(|group| group.id == "codewhale")
            .unwrap();
        let row = group
            .items
            .iter()
            .find(|row| row.label == "computer-use")
            .unwrap();
        let builtin = app.plugin_registry.get("computer-use").unwrap();
        assert_eq!(
            row.description,
            builtin
                .manifest
                .plugin
                .description
                .clone()
                .unwrap_or_default()
        );
        assert_eq!(
            row.state,
            tr(Locale::En, MessageId::ExtensionsStateFirstParty)
        );
        assert!(
            matches!(&row.action, Some(ExtensionAction::Command { command, disposition: RowActionDisposition::LeavePanel, .. }) if command == "/plugin trust computer-use")
        );
        assert!(!builtin.trusted());
        assert!(!builtin.enabled);
        assert_eq!(group.items.iter().filter(|row| matches!(&row.action, Some(ExtensionAction::Command { command, .. }) if command.starts_with("/plugin marketplace install "))).count(), 3);
    }

    #[test]
    fn mcp_item_action_for_stale_oauth_is_login() {
        let recovery =
            crate::mcp::mcp_recovery_kind(true, true, false, Some("401 Unauthorized"), true)
                .expect("stale oauth needs recovery");
        assert_eq!(recovery, McpRecoveryKind::Reauth);
        assert_eq!(recovery.slash_command("github"), "/mcp login github");
        assert_eq!(tr(Locale::En, recovery.label_key()).as_ref(), "re-auth");
    }

    #[test]
    fn a_healthy_server_offers_no_recovery_action() {
        // Founder live-test: "even the ones that are connected say diagnose
        // lol". Enabled, inspected, connected and erroring on nothing is not
        // a state anything repairs.
        assert_eq!(
            crate::mcp::mcp_recovery_kind(true, true, true, None, false),
            None
        );
    }

    #[test]
    fn mcp_item_action_for_disconnected_server_is_reconnect() {
        let recovery = crate::mcp::mcp_recovery_kind(true, true, false, None, false)
            .expect("a disconnected server needs recovery");
        assert_eq!(recovery, McpRecoveryKind::Reconnect);
        // The row names one server, so the command it runs must name it too:
        // reloading all of them leaves the row the user aimed at still pending
        // when the list returns, which reads as the key doing nothing.
        assert_eq!(
            recovery.slash_command("playwright"),
            "/mcp retry playwright"
        );
        assert_eq!(tr(Locale::En, recovery.label_key()).as_ref(), "reconnect");
    }

    /// Four distinct tones, four distinct inks, and none of them read out of
    /// a localized string — a screen that only colours correctly in English
    /// is not coloured.
    #[test]
    fn every_tone_paints_a_distinct_ink() {
        use codewhale_palette::ChromeInk;
        let theme = codewhale_palette::ThemeId::Whale.ui_theme();
        let inks: Vec<ChromeInk> = [
            ExtensionTone::Ready,
            ExtensionTone::Attention,
            ExtensionTone::Failure,
            ExtensionTone::Idle,
        ]
        .into_iter()
        .map(ExtensionTone::ink)
        .collect();
        let colors: std::collections::BTreeSet<String> = inks
            .iter()
            .map(|ink| format!("{:?}", ink.color(&theme)))
            .collect();
        assert_eq!(
            colors.len(),
            4,
            "each tone must be visually separable: {inks:?}"
        );
        assert_eq!(ExtensionTone::default(), ExtensionTone::Idle);
    }

    /// The grokbuild grammar: Tab / Shift+Tab move across the tab bar, even
    /// mid-search, and the query rides along to the new tab.
    #[test]
    fn tab_switches_tabs_and_keeps_the_search_query() {
        use crate::tui::views::ModalView;
        let mut view = ExtensionsView::from_snapshot_with_locale(
            ExtensionsSnapshot::default(),
            ExtensionsTab::Plugins,
            Locale::En,
        );
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        view.handle_key(key(KeyCode::Char('/')));
        view.handle_key(key(KeyCode::Char('g')));
        assert_eq!(view.focus, ExtensionsFocus::Search);

        view.handle_key(key(KeyCode::Tab));
        assert_eq!(view.active_tab, ExtensionsTab::Marketplace);
        assert_eq!(view.query, "g", "the query carries over to the new tab");

        view.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::SHIFT));
        assert_eq!(view.active_tab, ExtensionsTab::Plugins);
        view.handle_key(key(KeyCode::BackTab));
        assert_eq!(view.active_tab, ExtensionsTab::Hooks);

        // Wraps: the last tab's next is the first.
        view.set_tab(ExtensionsTab::Mcp);
        view.handle_key(key(KeyCode::Tab));
        assert_eq!(view.active_tab, ExtensionsTab::Hooks);
    }

    fn mcp_row(name: &str, state: &str, detail: &str, action: ExtensionAction) -> ExtensionItem {
        ExtensionItem {
            id: name.into(),
            label: name.into(),
            description: String::new(),
            state: state.into(),
            tone: ExtensionTone::Attention,
            detail: detail.into(),
            action: Some(action),
            toggle: None,
            remove: None,
        }
    }

    fn login_row(name: &str) -> ExtensionItem {
        mcp_row(
            name,
            &crate::tui::session_boot::mcp_auth_required_state_label(),
            "401 Unauthorized: the session is no longer accepted",
            ExtensionAction::Command {
                label: "re-auth".into(),
                command: McpRecoveryKind::Reauth.slash_command(name),
                disposition: RowActionDisposition::LeavePanel,
            },
        )
    }

    /// The founder's receipt (#5926): seven OAuth servers whose login
    /// expired and one that really failed. The expired logins lead in their
    /// own group with the login command on the row; the real failure keeps
    /// its reason; the connected server sorts last.
    #[test]
    fn mcp_rows_list_expired_logins_first_then_failures_with_their_reason() {
        let rows = vec![
            mcp_row(
                "alpha",
                "connected",
                "3 tools",
                ExtensionAction::Status {
                    label: "connected".into(),
                },
            ),
            mcp_row(
                "supabase",
                "error",
                "OAuth token refresh failed: Failed to parse server response",
                ExtensionAction::Command {
                    label: "diagnose".into(),
                    command: McpRecoveryKind::Diagnose.slash_command("supabase"),
                    disposition: RowActionDisposition::InPlace,
                },
            ),
            login_row("slack"),
            login_row("stripe"),
        ];
        let groups = mcp_groups(Locale::En, rows);
        let shape: Vec<(&str, Vec<&str>)> = groups
            .iter()
            .map(|group| {
                (
                    group.id.as_str(),
                    group.items.iter().map(|item| item.label.as_str()).collect(),
                )
            })
            .collect();
        assert_eq!(
            shape,
            vec![
                ("login", vec!["slack", "stripe"]),
                ("attention", vec!["supabase"]),
                ("servers", vec!["alpha"]),
            ]
        );
        assert_eq!(groups[0].label, "Needs login");
        assert_eq!(groups[1].label, "Needs attention");
        let slack = &groups[0].items[0];
        assert_eq!(
            slack.action.as_ref().and_then(ExtensionAction::command),
            Some("/mcp login slack")
        );
        assert!(!slack.state.contains("failed"), "{}", slack.state);
        assert_eq!(
            groups[1].items[0].detail,
            "OAuth token refresh failed: Failed to parse server response"
        );
    }

    /// Opening `/mcp` lands on the first server that needs a login, so Enter
    /// is the login key, not a fold of the group heading. A tab without a
    /// login group keeps the heading-first default.
    #[test]
    fn mcp_tab_opens_on_the_first_login_row() {
        let mut snapshot = ExtensionsSnapshot::default();
        snapshot.tabs[ExtensionsTab::Mcp.index()] = ExtensionsTabModel {
            groups: mcp_groups(Locale::En, vec![login_row("slack"), login_row("stripe")]),
            problem: None,
        };
        let view = ExtensionsView::from_snapshot_with_locale(
            snapshot.clone(),
            ExtensionsTab::Mcp,
            Locale::En,
        );
        assert_eq!(view.selected[ExtensionsTab::Mcp.index()], 1);
        let entries = view.visible_entries();
        match entries[1] {
            VisibleEntry::Item(group, item) => {
                assert_eq!(group.id, MCP_LOGIN_GROUP_ID);
                assert_eq!(item.label, "slack");
                assert_eq!(
                    item.action.as_ref().and_then(ExtensionAction::command),
                    Some("/mcp login slack")
                );
            }
            other => panic!("expected the first login row, got {other:?}"),
        }

        let plain = ExtensionsView::from_snapshot_with_locale(
            ExtensionsSnapshot::default(),
            ExtensionsTab::Mcp,
            Locale::En,
        );
        assert_eq!(plain.selected[ExtensionsTab::Mcp.index()], 0);
    }

    fn item_with_action(action: ExtensionAction) -> ExtensionItem {
        ExtensionItem {
            id: "row".into(),
            label: "row".into(),
            description: String::new(),
            state: "state".into(),
            tone: ExtensionTone::Idle,
            detail: "detail".into(),
            action: Some(action),
            toggle: None,
            remove: None,
        }
    }

    fn view_on_item(action: ExtensionAction) -> ExtensionsView {
        let mut snapshot = ExtensionsSnapshot::default();
        snapshot.tabs[ExtensionsTab::Plugins.index()] = ExtensionsTabModel {
            groups: vec![ExtensionGroup {
                id: "g".into(),
                label: "g".into(),
                items: vec![item_with_action(action)],
            }],
            problem: None,
        };
        let mut view =
            ExtensionsView::from_snapshot_with_locale(snapshot, ExtensionsTab::Plugins, Locale::En);
        // Land on the item, not its group heading.
        view.selected[ExtensionsTab::Plugins.index()] = 1;
        view
    }

    /// The defect: every row closed the panel and dropped its command into
    /// the transcript. A mutation runs in place — the event carries the
    /// command, and `Emit` (not `EmitAndClose`) is what keeps the panel.
    #[test]
    fn in_place_row_action_emits_without_closing() {
        let mut view = view_on_item(ExtensionAction::Command {
            label: "enable".into(),
            command: "/plugin enable demo".into(),
            disposition: RowActionDisposition::InPlace,
        });
        match view.activate_selected() {
            ViewAction::Emit(ViewEvent::ExecutePanelCommand {
                command,
                pager_title,
            }) => {
                assert_eq!(command, "/plugin enable demo");
                assert_eq!(pager_title, None);
            }
            other => panic!("expected an in-place command, got {other:?}"),
        }
    }

    /// An inspect row keeps the panel open and asks for its text output in a
    /// pager stacked on the panel — the detail belongs to the row, not to a
    /// transcript dump behind the modal.
    #[test]
    fn inspect_row_action_pages_its_output_in_place() {
        let mut view = view_on_item(ExtensionAction::Command {
            label: "open".into(),
            command: "/plugin show demo".into(),
            disposition: RowActionDisposition::InPlacePager,
        });
        match view.activate_selected() {
            ViewAction::Emit(ViewEvent::ExecutePanelCommand {
                command,
                pager_title,
            }) => {
                assert_eq!(command, "/plugin show demo");
                assert_eq!(pager_title.as_deref(), Some("row"));
            }
            other => panic!("expected a paged inspect, got {other:?}"),
        }
    }

    /// A flow that owns another surface — a login, an editor, the composer's
    /// trust token — still yields the panel.
    #[test]
    fn leave_panel_row_action_still_closes() {
        let mut view = view_on_item(ExtensionAction::Command {
            label: "re-auth".into(),
            command: "/mcp login github".into(),
            disposition: RowActionDisposition::LeavePanel,
        });
        match view.activate_selected() {
            ViewAction::EmitAndClose(ViewEvent::CommandPaletteSelected {
                action: CommandPaletteAction::ExecuteCommand { command },
            }) => assert_eq!(command, "/mcp login github"),
            other => panic!("expected the panel to yield, got {other:?}"),
        }
    }

    /// A refresh swaps the read model without disturbing the session: tab,
    /// query, selection, and folds all survive, and the new MCP generation
    /// the poll compares against rides along.
    #[test]
    fn refresh_preserves_view_state_and_tracks_generation() {
        let mut view = view_on_item(ExtensionAction::Command {
            label: "enable".into(),
            command: "/plugin enable demo".into(),
            disposition: RowActionDisposition::InPlace,
        });
        view.query = "de".into();
        let mut fresh = ExtensionsSnapshot {
            mcp_generation: 7,
            mcp_initializing: true,
            ..ExtensionsSnapshot::default()
        };
        fresh.tabs[ExtensionsTab::Plugins.index()] = ExtensionsTabModel {
            groups: vec![ExtensionGroup {
                id: "g".into(),
                label: "g".into(),
                items: vec![item_with_action(ExtensionAction::Status {
                    label: "enabled".into(),
                })],
            }],
            problem: None,
        };
        view.refresh_snapshot(fresh);
        assert_eq!(view.active_tab, ExtensionsTab::Plugins);
        assert_eq!(view.query, "de");
        assert_eq!(view.snapshot.mcp_generation, 7);
        assert!(view.snapshot.mcp_initializing);
        // The refreshed row's action is the new model's, not the stale one.
        let entries = view.visible_entries();
        match entries[view.selected[ExtensionsTab::Plugins.index()]] {
            VisibleEntry::Item(_, item) => {
                assert!(matches!(item.action, Some(ExtensionAction::Status { .. })));
            }
            other => panic!("expected the refreshed item, got {other:?}"),
        }
    }
}
