//! `/model` picker modal: pick a model and thinking-effort tier (#39, #2026).
//!
//! The picker intentionally presents model and thinking as independent choices
//! instead of collapsing them into preset route names. The "auto" option is
//! always available; custom (unrecognized) model ids appear as a separate row.
//! Pass-through providers fall back to only "auto" plus the current custom row.
//!
//! On apply we emit a [`ViewEvent::ModelPickerApplied`] with the resolved
//! model id and effort tier.

use std::cell::{Ref, RefCell};
use std::collections::BTreeMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Widget},
};

use codewhale_config::catalog::{CatalogRefreshError, CatalogSource, CatalogStatus};
use codewhale_config::model_reference::ModelReferenceCard;
use codewhale_config::pricing::OfferingPricing;

use crate::codex_model_cache::{
    self, CodexModelCacheFreshness, CodexModelMetadata, CodexModelRoster,
};
use crate::config::{ApiProvider, Config, DEEPSEEK_ALIAS_REPLACEMENT};
use crate::model_profile::{
    CapabilityOverride, SupportState, resolved_capability_profile_for_route_with_overrides,
    resolved_capability_profile_with_overrides,
};
use crate::model_registry;
use crate::models_dev_live::{self, ModelsDevFreshness};
use crate::provider_lake::{
    catalog_offering_for_model, catalog_offering_for_model_identity, configured_providers,
};
use crate::reasoning_preference::ReasoningEffort;
use crate::settings::PinnedModel;
use crate::tui::app::{App, StatusToastLevel};
use crate::tui::menu_style;
use crate::tui::views::fleet_detail::{FleetRouteSelection, FleetRouteTarget};
use crate::tui::views::{
    ActionHint, ListDetailLayout, ModalKind, ModalView, ViewAction, ViewEvent, render_modal_footer,
    render_underwater_surface,
};
use crate::utils::format_context_window;
use codewhale_localization::{Locale, MessageId, tr};
use codewhale_palette as palette;

/// Thinking-effort rows shown for DeepSeek-style providers, in the order
/// DeepSeek behaviorally distinguishes them.
const DEFAULT_PICKER_EFFORTS: &[ReasoningEffort] = &[
    ReasoningEffort::Auto,
    ReasoningEffort::Off,
    ReasoningEffort::High,
    ReasoningEffort::Max,
];
/// First-party DeepSeek routes document a real `low` wire tier alongside
/// `high`/`max` (#52), so their picker exposes the cheaper tier the generic
/// default list cannot claim for routes where low collapses onto high.
const DEEPSEEK_PICKER_EFFORTS: &[ReasoningEffort] = &[
    ReasoningEffort::Auto,
    ReasoningEffort::Off,
    ReasoningEffort::Low,
    ReasoningEffort::High,
    ReasoningEffort::Max,
];
/// Kimi Code K3 accepts route-specific low and medium controls at the
/// official membership endpoint. Medium becomes K3's nested high wire effort,
/// but keeping the selected intent visible is important for recovery and
/// route receipts.
const KIMI_CODE_K3_PICKER_EFFORTS: &[ReasoningEffort] = &[
    ReasoningEffort::Auto,
    ReasoningEffort::Off,
    ReasoningEffort::Low,
    ReasoningEffort::Medium,
    ReasoningEffort::High,
    ReasoningEffort::Max,
];
const CODEX_PICKER_EFFORTS: &[ReasoningEffort] = &[
    ReasoningEffort::Low,
    ReasoningEffort::Medium,
    ReasoningEffort::High,
    ReasoningEffort::Max,
];
/// Auto model routing has no concrete provider dialect yet, so retain the
/// complete preference vocabulary and defer normalization to dispatch.
const AUTO_MODEL_PICKER_EFFORTS: &[ReasoningEffort] = &[
    ReasoningEffort::Auto,
    ReasoningEffort::Off,
    ReasoningEffort::Low,
    ReasoningEffort::Medium,
    ReasoningEffort::High,
    ReasoningEffort::Max,
];

/// `/model` catalog views (#4115).
///
/// Configured stays the calm default: the current route, pins and Fleet
/// models, recently used routes, and one default per credentialed provider
/// (#6533). Typing searches every provider and a cross-provider selection
/// switches its route transactionally, so `/provider` is never a
/// prerequisite. Discoverability views (New / Coding / Cheap / Long context)
/// never auto-select a surprising route — the active model remains the
/// selection until the operator moves. "New" sorts by catalog refresh time;
/// recency of *use* lives in the default view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModelListView {
    Configured,
    Catalog,
    New,
    Coding,
    Cheap,
    LongContext,
}

impl ModelListView {
    const ALL: [Self; 6] = [
        Self::Configured,
        Self::Catalog,
        Self::New,
        Self::Coding,
        Self::Cheap,
        Self::LongContext,
    ];

    fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|view| *view == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    fn from_memory_name(name: &str) -> Option<Self> {
        match name {
            "configured" => Some(Self::Configured),
            "catalog" => Some(Self::Catalog),
            // `recent` is the pre-#6533 name of the same view.
            "new" | "recent" => Some(Self::New),
            "coding" => Some(Self::Coding),
            "cheap" => Some(Self::Cheap),
            "long_context" => Some(Self::LongContext),
            _ => None,
        }
    }

    fn memory_name(self) -> &'static str {
        match self {
            Self::Configured => "configured",
            Self::Catalog => "catalog",
            Self::New => "new",
            Self::Coding => "coding",
            Self::Cheap => "cheap",
            Self::LongContext => "long_context",
        }
    }

    /// Short chrome / action label for this view.
    fn title_label(self) -> &'static str {
        match self {
            Self::Configured => "configured",
            Self::Catalog => "catalog",
            Self::New => "new",
            Self::Coding => "coding",
            Self::Cheap => "cheap",
            Self::LongContext => "long ctx",
        }
    }

    /// Views that browse beyond the conservative configured-provider set.
    fn is_discoverability(self) -> bool {
        !matches!(self, Self::Configured)
    }

    fn browses_all_providers(self) -> bool {
        self.is_discoverability()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pane {
    Model,
    Effort,
}

/// What applying a row means: the session's own route (`/model`), or a pin
/// on one Fleet editor row, where Enter hands the absolute route back to the
/// editor instead of switching the session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModelPickerPurpose {
    Session,
    FleetProfileRoute {
        editor_id: uuid::Uuid,
        initial_reasoning: Option<ReasoningEffort>,
    },
    FleetRoute {
        target: FleetRouteTarget,
        editor_id: uuid::Uuid,
        initial_reasoning: Option<ReasoningEffort>,
        allow_inherit: bool,
    },
}

#[derive(Debug, Clone, Copy)]
struct PaneRenderState {
    pane: Pane,
    selected: usize,
    focused: bool,
}

pub struct ModelPickerView {
    initial_model: String,
    /// Exact runtime value before the picker opened. Keep this raw so choosing
    /// the canonical replacement for a retired alias performs a real migration
    /// instead of being misclassified as "unchanged".
    previous_model: String,
    initial_provider: ApiProvider,
    initial_provider_identity: String,
    /// Raw preference before the picker opened. An absent explicit preference
    /// is represented by Auto so applying a visible fixed-route tier is still
    /// recognized as an intentional picker choice.
    initial_effort: ReasoningEffort,
    /// Working raw preference. Model-row navigation only changes how this is
    /// projected into the visible route-specific effort rows.
    selected_effort_request: ReasoningEffort,
    active_accepts_custom_model_ids: bool,
    query: String,
    /// Working selection (separate from the initial values so we can offer a
    /// clean Esc-to-cancel without mutating App state).
    selected_model_idx: usize,
    selected_effort_idx: usize,
    focus: Pane,
    /// True when the active model is one we don't list — we still show it
    /// so the picker doesn't quietly forget the user's chosen IDs.
    show_custom_model_row: bool,
    model_rows: Vec<ModelPickerRow>,
    /// Static route facts used to validate custom/current rows at apply time.
    route_config: Config,
    /// Session-local provider checks used by custom/current rows. Catalog rows
    /// resolve the same snapshot during construction.
    provider_health: crate::provider_readiness::ProviderReadinessSnapshot,
    view: ModelListView,
    /// Other providers considered "configured" (#3830), shown by default
    /// alongside `initial_provider`'s own rows without requiring the user to
    /// type a search query first. Uses the same definition as the
    /// `/provider` manager's default view
    /// (`crate::config::provider_is_configured_for_active`): active
    /// provider, working credentials/OAuth, or an explicit
    /// `[providers.<name>]` entry. Self-hosted providers (Ollama/Sglang/
    /// Vllm) don't qualify just because routing to them doesn't require a
    /// key.
    configured_providers: Vec<ApiProvider>,
    row_hitboxes: RefCell<Vec<(Rect, Pane, usize)>>,
    last_mouse_selected: Option<(Pane, usize)>,
    hovered_row: Option<(Pane, usize)>,
    /// UI locale captured from the app at construction (#4057 wave 2).
    locale: Locale,
    pinned_models: Vec<PinnedModel>,
    // Navigation only changes selection. Catalog projections are rebuilt when
    // the query, view, sort, pins or readiness/catalog snapshot changes.
    projection: RefCell<Option<ModelPickerProjection>>,
    sort: Option<ModelSort>,
    column_hitboxes: RefCell<Vec<(Rect, ModelSortColumn)>>,
    pane_hitboxes: RefCell<Vec<(Rect, Pane)>>,
    catalog_action_hitbox: RefCell<Option<Rect>>,
    catalog_action_hovered: bool,
    purpose: ModelPickerPurpose,
    assignment_context: Option<(String, String)>,
    /// Receipt of the last ⇧P / ⇧F / refresh with its typed toast level. The
    /// picker covers the status line, so this line is the only place those
    /// actions can confirm or refuse (#6500). Cleared by the next key.
    notice: Option<(String, StatusToastLevel)>,
    /// Theme the notice resolves its semantic ink through.
    theme: codewhale_palette::UiTheme,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModelSortColumn {
    Model,
    Provider,
    Context,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ModelSort {
    column: ModelSortColumn,
    descending: bool,
}

struct ModelPickerProjection {
    query: String,
    view: ModelListView,
    sort: Option<ModelSort>,
    indices: Vec<usize>,
    rows: Vec<PaneRow>,
    custom: Option<(String, ApiProvider)>,
}

struct VisibleModelRows<'a> {
    catalog: &'a [ModelPickerRow],
    indices: Ref<'a, [usize]>,
}

impl<'a> VisibleModelRows<'a> {
    fn len(&self) -> usize {
        self.indices.len()
    }
    fn get(&self, index: usize) -> Option<&'a ModelPickerRow> {
        self.indices.get(index).map(|index| &self.catalog[*index])
    }
    fn iter(&self) -> impl ExactSizeIterator<Item = &'a ModelPickerRow> + '_ {
        self.indices.iter().map(|index| &self.catalog[*index])
    }
}

impl std::ops::Index<usize> for VisibleModelRows<'_> {
    type Output = ModelPickerRow;
    fn index(&self, index: usize) -> &Self::Output {
        &self.catalog[self.indices[index]]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ModelPickerRow {
    id: String,
    provider: Option<ApiProvider>,
    /// Concrete persistence identity. `Custom` alone cannot identify a named
    /// custom route, so pins must carry this exact key when present.
    provider_identity: Option<String>,
    hint: String,
    metadata: EffectivePickerMetadata,
    selectable: bool,
    /// Why this route cannot be attempted, kept structured so the scannable
    /// row can show the reason without re-parsing the prose `hint`. `None`
    /// whenever the route is attemptable.
    blocked_reason: Option<String>,
    /// Where this row sits in the default view (#6533); `None` keeps it
    /// behind search and the catalog views.
    default_rank: Option<(DefaultSection, usize)>,
    /// The route holds a credential (saved key, imported token, or an
    /// observed success), as opposed to merely being routable.
    credentialed: bool,
    /// The route's fresh live roster does not list this id.
    not_listed: Option<NotListed>,
}

/// Sections of the default view, in display order (#6533).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum DefaultSection {
    Current,
    Pinned,
    Recent,
    ProviderDefault,
}

/// Most rows the default view's recent section shows.
const RECENT_ROW_LIMIT: usize = 8;

/// A fresh live roster for the row's exact route omits its id. Only a fresh
/// roster may say so: a stale one lags the provider (and would condemn ids it
/// simply never saw). The row stays selectable and nothing is rewritten.
#[derive(Debug, Clone, PartialEq, Eq)]
struct NotListed {
    provider: String,
    /// `YYYY-MM-DD` of the roster fetch.
    checked: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct EffectivePickerMetadata {
    display_name: Option<String>,
    declared_input_price: Option<String>,
    context_window: Option<u32>,
    /// The context window came through the legacy provider fallback rather
    /// than an offering, catalog row, roster, or operator override — shown,
    /// but never as a verified capability (#5239, #5441).
    context_window_unverified: bool,
    max_output: Option<u32>,
    /// The output ceiling is an assumed floor for a route that publishes no
    /// ceiling we can stand behind (unknown Anthropic-family models),
    /// clamped but never labeled "documented" (#5440).
    max_output_unverified: bool,
    tool_calls: Option<bool>,
    reasoning: bool,
    reasoning_unknown: bool,
    vision: SupportState,
    pricing: PickerPricing,
    source: Option<CatalogSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
enum PickerPricing {
    /// The route explicitly does not expose authoritative token pricing.
    Unavailable,
    Known(String),
    #[default]
    Unknown,
}

impl ModelPickerView {
    #[must_use]
    /// The same picker, opened by the Fleet editor for one of its rows: Enter
    /// hands the absolute route to the editor (`FleetRoutePicked`) instead of
    /// switching the session.
    pub fn new_for_fleet_route(
        app: &App,
        config: &Config,
        target: FleetRouteTarget,
        editor_id: uuid::Uuid,
        selection: FleetRouteSelection,
    ) -> Self {
        Self::new_for_assignment(
            app,
            config,
            ModelPickerPurpose::FleetRoute {
                target,
                editor_id,
                initial_reasoning: selection.reasoning,
                allow_inherit: selection.allow_inherit,
            },
            selection,
        )
    }

    pub fn new_for_fleet_profile(
        app: &App,
        config: &Config,
        editor_id: uuid::Uuid,
        selection: FleetRouteSelection,
    ) -> Self {
        Self::new_for_assignment(
            app,
            config,
            ModelPickerPurpose::FleetProfileRoute {
                editor_id,
                initial_reasoning: selection.reasoning,
            },
            selection,
        )
    }

    pub fn with_assignment_context(
        mut self,
        title: impl Into<String>,
        scope: impl Into<String>,
    ) -> Self {
        self.assignment_context = Some((title.into(), scope.into()));
        self
    }

    fn new_for_assignment(
        app: &App,
        config: &Config,
        purpose: ModelPickerPurpose,
        selection: FleetRouteSelection,
    ) -> Self {
        let mut picker = Self::new(app, config);
        picker.purpose = purpose;
        // Session browsing memory is unrelated to the row being edited.
        picker.query.clear();
        picker.view = ModelListView::Configured;
        picker.sort = None;
        picker.initial_provider_identity = selection
            .provider
            .unwrap_or_else(|| app.provider_identity_for_persistence().to_string());
        picker.initial_provider =
            ApiProvider::parse(&picker.initial_provider_identity).unwrap_or(ApiProvider::Custom);
        picker.initial_model = selection.model.unwrap_or_else(|| "auto".to_string());
        picker.previous_model = picker.initial_model.clone();
        picker.initial_effort = selection.reasoning.unwrap_or(ReasoningEffort::Auto);
        picker.selected_effort_request = picker.initial_effort;
        picker.apply_fleet_route_rows(app, config);
        *picker.projection.borrow_mut() = None;
        picker.selected_model_idx = {
            let rows = picker.visible_model_rows();
            rows.iter()
                .position(|row| {
                    row.id == picker.initial_model
                        && model_row_matches_route(
                            row,
                            picker.initial_provider,
                            &picker.initial_provider_identity,
                        )
                })
                .unwrap_or(rows.len())
        };
        let visible_len = picker.visible_model_rows().len();
        picker.show_custom_model_row = picker.selected_model_idx >= visible_len;
        *picker.projection.borrow_mut() = None;
        picker.select_effort_for_current_model();
        picker
    }

    fn apply_fleet_route_rows(&mut self, app: &App, config: &Config) {
        let allow_inherit = match self.purpose {
            ModelPickerPurpose::Session => return,
            ModelPickerPurpose::FleetRoute { allow_inherit, .. } => allow_inherit,
            ModelPickerPurpose::FleetProfileRoute { .. } => true,
        };
        self.route_config.provider = Some(self.initial_provider_identity.clone());
        self.configured_providers = configured_providers(config, self.initial_provider)
            .into_iter()
            .filter(|provider| *provider != self.initial_provider)
            .collect();
        self.active_accepts_custom_model_ids =
            config.model_ids_pass_through_for_provider(self.initial_provider);
        if self.initial_model != "auto" {
            push_provider_model_rows(
                &mut self.model_rows,
                self.initial_provider,
                (self.initial_provider == ApiProvider::Custom)
                    .then_some(self.initial_provider_identity.as_str()),
                vec![self.initial_model.clone()],
                self.initial_provider,
                config,
                &codex_model_cache::model_roster(),
                &app.provider_health,
            );
        }
        self.model_rows
            .retain(|row| allow_inherit || row.id != "auto");
        for row in &mut self.model_rows {
            if row.id == "auto" && row.provider.is_none() {
                row.hint = format!("{}/{}", app.provider_identity_for_persistence(), app.model);
                row.selectable = true;
                row.blocked_reason = None;
            }
        }
        // Same sections as the session picker, anchored on the row being
        // edited rather than on the session's own route.
        let current = (self.initial_model != "auto").then_some((
            self.initial_provider_identity.as_str(),
            self.initial_model.as_str(),
        ));
        assign_default_sections(
            &mut self.model_rows,
            app,
            config,
            current,
            &self.pinned_models,
        );
    }

    pub fn new(app: &App, config: &Config) -> Self {
        let initial_model = if app.auto_model {
            "auto".to_string()
        } else {
            picker_visible_model_id(app.api_provider, &app.model, app.accepts_custom_model_ids())
                .to_string()
        };
        let previous_model = if app.auto_model {
            "auto".to_string()
        } else {
            app.model.clone()
        };
        let model_rows = picker_model_rows_for_app(app, config);
        let configured_providers: Vec<_> = configured_providers(config, app.api_provider)
            .into_iter()
            .filter(|provider| *provider != app.api_provider)
            .collect();
        let mut default_visible_rows: Vec<_> = model_rows
            .iter()
            .filter(|row| model_row_visible_in_view(row, ModelListView::Configured))
            .collect();
        // Selection indices must be calculated in the same order that the
        // configured view renders. Pinned rows are sorted to the top by
        // `visible_model_rows`; using the unsorted construction order here
        // made the cursor land on a different row (or look unselected) after
        // a pin reordered the list.
        let pins = picker_pins_for_app(app);
        sort_model_rows_for_view(
            &mut default_visible_rows,
            |row| *row,
            ModelListView::Configured,
            &pins,
        );
        let mut selected_model_idx = default_visible_rows.iter().position(|row| {
            row.id == initial_model
                && model_row_matches_route(
                    row,
                    app.api_provider,
                    app.provider_identity_for_persistence(),
                )
        });
        let show_custom_model_row = selected_model_idx.is_none();
        if show_custom_model_row {
            selected_model_idx = Some(default_visible_rows.len());
        }
        let selected_model_idx = selected_model_idx.unwrap_or(0);

        let initial_effort = app
            .reasoning_effort_preference
            .unwrap_or(ReasoningEffort::Auto);
        let selected_effort_request = app
            .reasoning_effort_preference
            .unwrap_or(app.reasoning_effort);
        let effort_rows = picker_efforts_for_route(
            app.api_provider,
            &config.active_route_base_url(),
            &initial_model,
            app.auto_model,
        );
        let normalized = normalize_picker_effort(
            selected_effort_request,
            app.api_provider,
            &config.active_route_base_url(),
            &initial_model,
            app.auto_model,
        );
        let selected_effort_idx = effort_rows
            .iter()
            .position(|e| *e == normalized)
            .unwrap_or_else(|| {
                default_picker_effort_idx(
                    app.api_provider,
                    &config.active_route_base_url(),
                    &initial_model,
                    app.auto_model,
                )
            });

        let mut view = Self {
            initial_model,
            previous_model,
            initial_provider: app.api_provider,
            initial_provider_identity: app.provider_identity_for_persistence().to_string(),
            initial_effort,
            selected_effort_request,
            active_accepts_custom_model_ids: app.accepts_custom_model_ids(),
            query: String::new(),
            selected_model_idx,
            selected_effort_idx,
            focus: Pane::Model,
            show_custom_model_row,
            model_rows,
            route_config: config.clone(),
            provider_health: app.provider_health.clone(),
            view: ModelListView::Configured,
            configured_providers,
            row_hitboxes: RefCell::new(Vec::new()),
            last_mouse_selected: None,
            hovered_row: None,
            locale: app.ui_locale,
            pinned_models: pins,
            projection: RefCell::new(None),
            sort: None,
            column_hitboxes: RefCell::new(Vec::new()),
            pane_hitboxes: RefCell::new(Vec::new()),
            catalog_action_hitbox: RefCell::new(None),
            catalog_action_hovered: false,
            purpose: ModelPickerPurpose::Session,
            assignment_context: None,
            notice: None,
            theme: app.ui_theme,
        };
        view.restore_memory(app.model_picker_memory.as_ref());
        view
    }

    /// Restore the browsing context from the last dismissed picker (#4109):
    /// the named catalog view and, when the remembered row still exists in
    /// that view, the highlighted row. The active model remains the selection
    /// when nothing was remembered or the row is gone.
    fn restore_memory(&mut self, memory: Option<&crate::tui::app::ModelPickerMemory>) {
        let Some(memory) = memory else {
            return;
        };
        if let Some(view_name) = memory.view.as_deref() {
            if let Some(view) = ModelListView::from_memory_name(view_name) {
                self.view = view;
            }
        } else if memory.catalog_view {
            self.view = ModelListView::Catalog;
        }
        // Older picker memory stores only a model id. An ambiguous id must
        // not move the selection onto a different configured route. Resolve
        // the active row again because a restored view can change row order.
        let position = {
            let rows = self.visible_model_rows();
            let remembered = memory.selected_row_id.as_deref().and_then(|remembered_id| {
                let mut matches = rows
                    .iter()
                    .enumerate()
                    .filter(|(_, row)| row.id == remembered_id);
                let first = matches.next().map(|(index, _)| index);
                first.filter(|_| matches.next().is_none())
            });
            remembered.or_else(|| {
                rows.iter().position(|row| {
                    row.id == self.initial_model
                        && model_row_matches_route(
                            row,
                            self.initial_provider,
                            &self.initial_provider_identity,
                        )
                })
            })
        };
        if let Some(position) = position {
            self.selected_model_idx = position;
            self.select_effort_for_current_model();
        }
        self.clamp_model_selection();
    }

    fn ensure_projection(&self) {
        if self.projection.borrow().as_ref().is_some_and(|cached| {
            cached.query == self.query && cached.view == self.view && cached.sort == self.sort
        }) {
            return;
        }
        let query = self.query.trim();
        let mut indices: Vec<usize> = self
            .model_rows
            .iter()
            .enumerate()
            .filter_map(|(index, row)| {
                let visible = if query.is_empty() {
                    model_row_visible_in_view(row, self.view)
                } else {
                    model_row_matches_query(row, query, self.initial_provider)
                };
                visible.then_some(index)
            })
            .collect();
        if let Some(sort) = self.sort {
            sort_model_indices(&mut indices, &self.model_rows, sort);
        } else if query.is_empty() {
            sort_model_rows_for_view(
                &mut indices,
                |index| &self.model_rows[*index],
                self.view,
                &self.pinned_models,
            );
        } else {
            let query_lower = query.to_ascii_lowercase();
            indices.sort_by_cached_key(|index| {
                let row = &self.model_rows[*index];
                let provider_matches = row.provider.is_some_and(|provider| {
                    row.provider_identity.as_deref().is_some_and(|identity| {
                        identity.to_ascii_lowercase().contains(&query_lower)
                    }) || provider
                        .as_str()
                        .to_ascii_lowercase()
                        .contains(&query_lower)
                        || provider
                            .display_name()
                            .to_ascii_lowercase()
                            .contains(&query_lower)
                });
                let id = row.id.to_ascii_lowercase();
                let id_rank = if id == query_lower {
                    0
                } else if id.starts_with(&query_lower) {
                    1
                } else {
                    2
                };
                (
                    usize::from(!provider_matches),
                    id_rank,
                    usize::from(
                        row.provider.is_some() && row.provider != Some(self.initial_provider),
                    ),
                    id,
                )
            });
        }
        let visible: Vec<_> = indices
            .iter()
            .map(|index| &self.model_rows[*index])
            .collect();
        let route_labels = route_labels_for_rows(&visible);
        // The default view is ordered by section (current, pinned, recent,
        // defaults), so family headers would split it; only the catalog groups.
        let grouped =
            self.sort.is_none() && query.is_empty() && self.view == ModelListView::Catalog;
        let mut rows: Vec<_> = visible
            .iter()
            .map(|row| (row, pin_for_row(&self.pinned_models, row)))
            .map(|(row, pin)| PaneRow {
                primary: if self.purpose != ModelPickerPurpose::Session
                    && row.id == "auto"
                    && row.provider.is_none()
                {
                    tr(self.locale, MessageId::FleetRouteInherited).into_owned()
                } else {
                    row.metadata
                        .display_name
                        .as_ref()
                        .map(|label| format!("{label} ({})", row.id))
                        .unwrap_or_else(|| row.id.clone())
                },
                route: row
                    .provider
                    .map(|provider| {
                        route_labels
                            .get(row_provider_identity(row).unwrap_or(provider.as_str()))
                            .cloned()
                            .unwrap_or_else(|| provider.display_name().to_string())
                    })
                    .unwrap_or_default(),
                meta: if row.provider.is_none() {
                    vec![row.hint.clone()]
                } else {
                    // A pin or Fleet membership leads the chips so a squeezed
                    // row never sheds it: without it ⇧P / ⇧F changed nothing
                    // the person could see (#6500). A not-listed notice still
                    // leads when there is no pin.
                    let mut chips = model_row_meta_chips(row);
                    if let Some(not_listed) = &row.not_listed {
                        chips.insert(
                            0,
                            tr(self.locale, MessageId::ModelPickerNotListed)
                                .replace("{provider}", &not_listed.provider)
                                .replace("{date}", &not_listed.checked),
                        );
                    }
                    if let Some(pin) = pin {
                        chips.insert(
                            0,
                            pin.label.clone().unwrap_or_else(|| {
                                tr(self.locale, MessageId::ModelPickerPinnedChip).into_owned()
                            }),
                        );
                    }
                    chips
                },
                // Pinned rows sort above every group, so they draw no family
                // header: otherwise the family's header repeats where its
                // unpinned rows begin.
                family: (grouped && pin.is_none())
                    .then(|| {
                        row.provider.and_then(|provider| {
                            catalog_family_for_identity(
                                provider,
                                row.provider_identity.as_deref(),
                                &row.id,
                            )
                        })
                    })
                    .flatten(),
                active: row.id == self.initial_model
                    && model_row_matches_route(
                        row,
                        self.initial_provider,
                        &self.initial_provider_identity,
                    ),
                locked: !row.selectable,
                dim: row.not_listed.is_some(),
            })
            .collect();
        let custom = self.custom_model_row_for_visible(&visible);
        if let Some((model, provider)) = custom.as_ref() {
            rows.push(PaneRow {
                primary: model.clone(),
                route: provider.display_name().to_string(),
                meta: vec![
                    if query.is_empty() {
                        "current (custom)"
                    } else {
                        "custom route"
                    }
                    .to_string(),
                ],
                ..PaneRow::default()
            });
        }
        *self.projection.borrow_mut() = Some(ModelPickerProjection {
            query: self.query.clone(),
            view: self.view,
            sort: self.sort,
            indices,
            rows,
            custom,
        });
    }

    fn visible_model_rows(&self) -> VisibleModelRows<'_> {
        self.ensure_projection();
        VisibleModelRows {
            catalog: &self.model_rows,
            indices: Ref::map(self.projection.borrow(), |projection| {
                projection.as_ref().unwrap().indices.as_slice()
            }),
        }
    }

    fn model_row_count(&self) -> usize {
        self.ensure_projection();
        self.projection.borrow().as_ref().unwrap().rows.len()
    }

    /// Resolve the currently highlighted row to a model id.
    fn resolved_model(&self) -> String {
        let rows = self.visible_model_rows();
        if self.selected_model_idx < rows.len() {
            return rows[self.selected_model_idx].id.clone();
        }
        self.custom_model_row()
            .map(|(model, _)| model)
            .unwrap_or_else(|| self.initial_model.clone())
    }

    fn selected_model_is_selectable(&self) -> bool {
        if matches!(
            self.purpose,
            ModelPickerPurpose::FleetRoute {
                allow_inherit: false,
                ..
            }
        ) && self.resolved_model() == "auto"
        {
            return false;
        }
        let rows = self.visible_model_rows();
        if let Some(row) = rows.get(self.selected_model_idx) {
            return row.selectable;
        }
        self.custom_model_row().is_some_and(|(model, provider)| {
            crate::provider_readiness::resolve_for_model(
                &self.route_config,
                provider,
                &model,
                &self.provider_health,
            )
            .can_attempt()
        })
    }

    /// Feedback when Enter/apply is pressed on a locked (unauthenticated) model.
    /// Surfaces the readiness reason instead of a silent no-op, and routes the
    /// user toward provider authentication/setup when possible.
    fn explain_unselectable_selection(&self) -> ViewAction {
        let rows = self.visible_model_rows();
        let Some(row) = rows.get(self.selected_model_idx) else {
            return ViewAction::None;
        };
        let reason = if row.hint.trim().is_empty() {
            "This model is not available with the current provider credentials.".to_string()
        } else {
            row.hint.clone()
        };
        // The provider auth event identifies only an enum. Sending Custom
        // would open the first custom route's key editor, not this row's.
        if row.provider == Some(ApiProvider::Custom) {
            let identity = row_provider_identity(row).unwrap_or("custom");
            return ViewAction::Emit(ViewEvent::StatusMessage {
                message: format!(
                    "! {identity}/{} is locked — {reason}. Open /provider and select {identity} to repair or authenticate this route.",
                    row.id
                ),
            });
        }
        let message = format!(
            "! {} is locked — {reason}. Open /provider to authenticate, then refresh.",
            row.id
        );
        // The ordinary setup wizard switches the session after auth. A
        // Fleet edit must keep that session route intact.
        if self.purpose != ModelPickerPurpose::Session {
            return ViewAction::Emit(ViewEvent::StatusMessage { message });
        }
        // Prefer opening provider setup so the user can remediate in one step.
        if let Some(provider) = row.provider {
            return ViewAction::Emit(ViewEvent::ModelPickerNeedsAuth {
                provider,
                model: row.id.clone(),
                reason: message,
            });
        }
        ViewAction::Emit(ViewEvent::StatusMessage { message })
    }

    /// Exact route identity of the highlighted row, when it names one.
    fn resolved_provider_identity(&self) -> Option<String> {
        let rows = self.visible_model_rows();
        rows.get(self.selected_model_idx)?.provider_identity.clone()
    }

    fn resolved_provider(&self) -> Option<ApiProvider> {
        let rows = self.visible_model_rows();
        if self.selected_model_idx < rows.len() {
            return rows[self.selected_model_idx].provider;
        }
        self.custom_model_row()
            .map(|(_, provider)| provider)
            .or(Some(self.initial_provider))
    }

    fn resolved_effort(&self) -> ReasoningEffort {
        let efforts = self.current_efforts();
        efforts[self
            .selected_effort_idx
            .min(efforts.len().saturating_sub(1))]
    }

    fn current_efforts(&self) -> Vec<ReasoningEffort> {
        if matches!(
            self.purpose,
            ModelPickerPurpose::FleetRoute {
                allow_inherit: false,
                ..
            }
        ) {
            return vec![ReasoningEffort::Auto];
        }
        if let ModelPickerPurpose::FleetRoute {
            initial_reasoning, ..
        }
        | ModelPickerPurpose::FleetProfileRoute {
            initial_reasoning, ..
        } = self.purpose
            && self.resolved_model() == "auto"
        {
            return vec![initial_reasoning.unwrap_or(ReasoningEffort::Auto)];
        }
        let provider = self.resolved_provider().unwrap_or(self.initial_provider);
        let model = self.resolved_model();
        let base_url = self.resolved_base_url_for_provider(provider, &model);
        picker_efforts_for_route(
            provider,
            &base_url,
            &model,
            model.trim().eq_ignore_ascii_case("auto"),
        )
    }

    fn resolved_base_url_for_provider(&self, provider: ApiProvider, model: &str) -> String {
        if provider == ApiProvider::Custom
            && let Some(identity) = self.resolved_provider_identity()
        {
            return self
                .route_config
                .base_url_for_route_identity(provider, &identity);
        }
        crate::route_runtime::resolve_runtime_route(&self.route_config, provider, Some(model))
            .map(|route| route.candidate.endpoint().base_url.clone())
            .unwrap_or_else(|_| provider.default_base_url().to_string())
    }

    fn custom_model_row(&self) -> Option<(String, ApiProvider)> {
        self.ensure_projection();
        self.projection.borrow().as_ref().unwrap().custom.clone()
    }

    fn custom_model_row_for_visible(
        &self,
        visible_rows: &[&ModelPickerRow],
    ) -> Option<(String, ApiProvider)> {
        let query = self.query.trim();
        if query.is_empty() {
            return self
                .show_custom_model_row
                .then(|| (self.initial_model.clone(), self.initial_provider));
        }
        if let Some((provider, model)) = self.provider_qualified_custom_query(query) {
            if visible_rows.iter().any(|row| {
                row.provider == Some(provider) && row.id.eq_ignore_ascii_case(model.trim())
            }) {
                return None;
            }
            if self.provider_accepts_custom_model(provider, &model) {
                return Some((model, provider));
            }
            return None;
        }
        if !self.active_accepts_custom_model_ids {
            return None;
        }
        if visible_rows.iter().any(|row| {
            row.provider == Some(self.initial_provider) && row.id.eq_ignore_ascii_case(query)
        }) {
            return None;
        }
        Some((query.to_string(), self.initial_provider))
    }

    fn provider_qualified_custom_query(&self, query: &str) -> Option<(ApiProvider, String)> {
        for (provider_key, model) in provider_query_splits(query) {
            let Some(provider) = ApiProvider::parse(provider_key) else {
                continue;
            };
            if provider != self.initial_provider
                && !self.view.browses_all_providers()
                && !self.configured_providers.contains(&provider)
            {
                continue;
            }
            let model = model.trim();
            if model.is_empty() {
                continue;
            }
            return Some((provider, model.to_string()));
        }
        None
    }

    fn provider_accepts_custom_model(&self, provider: ApiProvider, model: &str) -> bool {
        (provider == self.initial_provider && self.active_accepts_custom_model_ids)
            || (provider != self.initial_provider
                && self
                    .route_config
                    .model_ids_pass_through_for_provider(provider))
            || crate::config::normalize_model_name_for_provider(provider, model).is_some()
    }

    fn clamp_model_selection(&mut self) {
        let count = self.model_row_count();
        if count == 0 {
            self.selected_model_idx = 0;
        } else if self.selected_model_idx >= count {
            self.selected_model_idx = count - 1;
        }
    }

    fn update_query(&mut self, next: String) {
        self.query = next;
        self.selected_model_idx = 0;
        self.clamp_model_selection();
        self.select_effort_for_current_model();
    }

    fn select_effort_for_current_model(&mut self) {
        let provider = self.resolved_provider().unwrap_or(self.initial_provider);
        let model = self.resolved_model();
        let model_is_auto = model.trim().eq_ignore_ascii_case("auto");
        let base_url = self.resolved_base_url_for_provider(provider, &model);
        let normalized = normalize_picker_effort(
            self.selected_effort_request,
            provider,
            &base_url,
            &model,
            model_is_auto,
        );
        self.selected_effort_idx =
            picker_efforts_for_route(provider, &base_url, &model, model_is_auto)
                .iter()
                .position(|candidate| *candidate == normalized)
                .unwrap_or_else(|| {
                    default_picker_effort_idx(provider, &base_url, &model, model_is_auto)
                });
    }

    /// Both panes rotate rather than stop at the ends. Thinking is four to six
    /// rows, so a hard stop at the bottom reads as a dead key rather than as a
    /// boundary; the model list wraps for the same reason.
    fn move_up(&mut self) -> bool {
        match self.focus {
            Pane::Model => {
                let count = self.model_row_count();
                if count == 0 {
                    return false;
                }
                self.selected_model_idx = wrapping_prev(self.selected_model_idx, count);
                self.select_effort_for_current_model();
                true
            }
            Pane::Effort => {
                let count = self.current_efforts().len();
                if count == 0 {
                    return false;
                }
                self.selected_effort_idx = wrapping_prev(self.selected_effort_idx, count);
                self.selected_effort_request = self.resolved_effort();
                true
            }
        }
    }

    fn move_down(&mut self) -> bool {
        match self.focus {
            Pane::Model => {
                let count = self.model_row_count();
                if count == 0 {
                    return false;
                }
                self.selected_model_idx = wrapping_next(self.selected_model_idx, count);
                self.select_effort_for_current_model();
                true
            }
            Pane::Effort => {
                let count = self.current_efforts().len();
                if count == 0 {
                    return false;
                }
                self.selected_effort_idx = wrapping_next(self.selected_effort_idx, count);
                self.selected_effort_request = self.resolved_effort();
                true
            }
        }
    }

    /// Apply one [`list_nav`](crate::tui::list_nav) motion (#6290), returning
    /// whether it was consumed. Steps wrap; pages travel [`MODEL_PAGE`] rows
    /// and clamp. The region axis toggles between the model and effort panes.
    fn apply_motion(&mut self, motion: crate::tui::list_nav::Motion) -> bool {
        use crate::tui::list_nav::Motion;
        if matches!(motion, Motion::RegionPrev | Motion::RegionNext) {
            if self.can_edit_effort() {
                self.toggle_focus();
            }
            return true;
        }
        let (current, len) = match self.focus {
            Pane::Model => (self.selected_model_idx, self.model_row_count()),
            Pane::Effort => (self.selected_effort_idx, self.current_efforts().len()),
        };
        if len == 0 {
            return false;
        }
        let Some(next) = crate::tui::list_nav::apply(current, len, MODEL_PAGE, motion) else {
            return false;
        };
        match self.focus {
            Pane::Model => {
                self.selected_model_idx = next;
                self.select_effort_for_current_model();
            }
            Pane::Effort => {
                self.selected_effort_idx = next;
                self.selected_effort_request = self.resolved_effort();
            }
        }
        true
    }

    fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            Pane::Model => Pane::Effort,
            Pane::Effort => Pane::Model,
        };
    }

    fn toggle_view(&mut self) {
        self.view = self.view.next();
        self.selected_model_idx = 0;
        self.clamp_model_selection();
        self.select_effort_for_current_model();
    }

    fn build_event_with_startup_default(&self, save_as_startup_default: bool) -> ViewEvent {
        let resolved_provider = self.resolved_provider().unwrap_or(self.initial_provider);
        let provider = (resolved_provider != self.initial_provider).then_some(resolved_provider);
        // The selected row's own identity, never the config's currently
        // selected custom route: applying a row must switch to the route that
        // row describes (#6016). Only the typed custom-model row, which names
        // no route, falls back to the configured identity.
        let provider_id = (resolved_provider == ApiProvider::Custom).then(|| {
            self.resolved_provider_identity()
                .unwrap_or_else(|| self.route_config.provider_identity_for(resolved_provider))
        });
        ViewEvent::ModelPickerApplied {
            model: self.resolved_model(),
            provider,
            provider_id,
            effort: self.selected_effort_request,
            previous_model: self.previous_model.clone(),
            previous_effort: self.initial_effort,
            save_as_startup_default,
        }
    }

    /// The event Enter (or the startup-default chord) emits, by purpose. A
    /// Fleet row gets its absolute route — provider resolved, `Custom` named
    /// by its exact identity — and has no startup default to save.
    fn build_apply_event(&self, save_as_startup_default: bool) -> ViewEvent {
        let (initial_reasoning, allow_inherit) = match self.purpose {
            ModelPickerPurpose::Session => {
                return self.build_event_with_startup_default(save_as_startup_default);
            }
            ModelPickerPurpose::FleetRoute {
                initial_reasoning,
                allow_inherit,
                ..
            } => (initial_reasoning, allow_inherit),
            ModelPickerPurpose::FleetProfileRoute {
                initial_reasoning, ..
            } => (initial_reasoning, true),
        };
        let provider = self.resolved_provider().unwrap_or(self.initial_provider);
        let provider_id = (provider == ApiProvider::Custom).then(|| {
            self.resolved_provider_identity()
                .unwrap_or_else(|| self.route_config.provider_identity_for(provider))
        });
        let model = self.resolved_model();
        let reasoning = if !allow_inherit {
            None
        } else if model == "auto" || self.selected_effort_request == self.initial_effort {
            initial_reasoning
        } else {
            Some(self.selected_effort_request)
        };
        match self.purpose {
            ModelPickerPurpose::FleetRoute {
                target, editor_id, ..
            } => ViewEvent::FleetRoutePicked {
                target,
                editor_id,
                provider,
                provider_id,
                model,
                reasoning,
            },
            ModelPickerPurpose::FleetProfileRoute { editor_id, .. } => {
                ViewEvent::FleetProfileRoutePicked {
                    editor_id,
                    provider,
                    provider_id,
                    model,
                    reasoning,
                }
            }
            ModelPickerPurpose::Session => unreachable!("session handled above"),
        }
    }

    /// Footer label for Enter: apply to the session, or assign to a Fleet row.
    fn apply_action_id(&self) -> MessageId {
        match self.purpose {
            ModelPickerPurpose::Session => MessageId::PickerActionApply,
            ModelPickerPurpose::FleetRoute { .. }
            | ModelPickerPurpose::FleetProfileRoute { .. } => MessageId::PickerActionAssignRoute,
        }
    }

    fn can_edit_effort(&self) -> bool {
        match self.purpose {
            ModelPickerPurpose::Session => true,
            ModelPickerPurpose::FleetProfileRoute { .. } => self.resolved_model() != "auto",
            // Shortlisted rows pin a model only; inherited routes retain
            // their existing reasoning until a concrete model is selected.
            ModelPickerPurpose::FleetRoute { allow_inherit, .. } => {
                allow_inherit && self.resolved_model() != "auto"
            }
        }
    }

    fn set_sort(&mut self, sort: Option<ModelSort>) {
        let selected = self
            .visible_model_rows()
            .indices
            .get(self.selected_model_idx)
            .copied();
        let was_custom = selected.is_none() && self.custom_model_row().is_some();
        self.sort = sort;
        self.last_mouse_selected = None;
        self.ensure_projection();
        if let Some(selected) = selected {
            let position = self
                .visible_model_rows()
                .indices
                .iter()
                .position(|index| *index == selected);
            if let Some(position) = position {
                self.selected_model_idx = position;
            }
        } else if was_custom {
            let visible_len = self.visible_model_rows().len();
            self.selected_model_idx = visible_len;
        }
        self.clamp_model_selection();
        self.select_effort_for_current_model();
    }

    fn sort_column(&mut self, column: ModelSortColumn) {
        let descending = self
            .sort
            .is_some_and(|sort| sort.column == column && !sort.descending);
        self.set_sort(Some(ModelSort { column, descending }));
    }

    fn cycle_sort(&mut self) {
        use ModelSortColumn::{Context, Model, Provider};
        let next = match self.sort {
            None => Some(ModelSort {
                column: Model,
                descending: false,
            }),
            Some(ModelSort {
                column,
                descending: false,
            }) => Some(ModelSort {
                column,
                descending: true,
            }),
            Some(ModelSort {
                column: Model,
                descending: true,
            }) => Some(ModelSort {
                column: Provider,
                descending: false,
            }),
            Some(ModelSort {
                column: Provider,
                descending: true,
            }) => Some(ModelSort {
                column: Context,
                descending: false,
            }),
            Some(ModelSort {
                column: Context,
                descending: true,
            }) => None,
        };
        self.set_sort(next);
    }

    fn render_sort_columns(&self, area: Rect, buf: &mut Buffer, columns: ModelRowColumns) {
        let label = |name: &str, column| match self.sort.filter(|sort| sort.column == column) {
            Some(sort) => format!("{name} {}", if sort.descending { "↓" } else { "↑" }),
            None => name.to_string(),
        };
        let row = PaneRow {
            primary: label("Model", ModelSortColumn::Model),
            route: label("Provider", ModelSortColumn::Provider),
            meta: vec![label("Context", ModelSortColumn::Context)],
            ..PaneRow::default()
        };
        let style = Style::default().fg(palette::TEXT_MUTED).bold();
        Paragraph::new(Line::from(picker_row_spans(
            &row,
            " ",
            usize::from(area.width),
            columns,
            style,
            style,
        )))
        .render(area, buf);
        let fitted = columns.resolve(usize::from(area.width));
        let mut x = usize::from(area.x) + ROW_PREFIX_WIDTH;
        let right = usize::from(area.right());
        for (width, column) in [
            (fitted.primary, ModelSortColumn::Model),
            (fitted.route, ModelSortColumn::Provider),
            (fitted.meta, ModelSortColumn::Context),
        ] {
            if width > 0 {
                if x < right {
                    self.column_hitboxes.borrow_mut().push((
                        Rect::new(x as u16, area.y, width.min(right - x) as u16, 1),
                        column,
                    ));
                }
                x += width + COLUMN_GAP;
            }
        }
    }

    fn render_pane(
        &self,
        area: Rect,
        buf: &mut Buffer,
        title: &str,
        rows: &[PaneRow],
        state: PaneRenderState,
    ) {
        self.pane_hitboxes.borrow_mut().push((area, state.pane));
        // A short stacked picker gives the focused pane the working space.
        // The other pane remains a clickable summary with its actual choice.
        if area.height == 1 && !state.focused {
            let summary = rows.get(state.selected).map_or_else(
                || title.to_string(),
                |row| format!("{title}: {}", row.primary),
            );
            Paragraph::new(crate::tui::ui_text::semantic_truncate(
                &summary,
                usize::from(area.width),
            ))
            .style(Style::default().fg(palette::TEXT_MUTED))
            .render(area, buf);
            if !rows.is_empty() {
                self.row_hitboxes
                    .borrow_mut()
                    .push((area, state.pane, state.selected));
            }
            return;
        }
        let header_height = if state.pane == Pane::Model && area.height >= 3 {
            2
        } else {
            1
        };
        let visible_height = usize::from(area.height.saturating_sub(header_height));
        let (start, end) = pane_row_window(state.selected, rows, visible_height);
        let title = if rows.len() > visible_height && visible_height > 0 {
            if start + 1 == end {
                // A scrollable pane whose visible window spans exactly one row
                // renders a single position (`Model 2/3`), not a degenerate
                // `2-2/3` range (#3995).
                format!(" {title} {}/{} ", end, rows.len())
            } else {
                format!(" {title} {}-{}/{} ", start + 1, end, rows.len())
            }
        } else {
            format!(" {title} ")
        };
        Block::default()
            .style(Style::default().bg(palette::WHALE_BG))
            .render(area, buf);
        let title_area = Rect { height: 1, ..area };
        Paragraph::new(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                title,
                Style::default()
                    .fg(if state.focused {
                        palette::TEXT_PRIMARY
                    } else {
                        palette::TEXT_MUTED
                    })
                    .bold(),
            ),
        ]))
        .render(title_area, buf);
        let inner = Rect {
            y: area.y.saturating_add(header_height),
            height: area.height.saturating_sub(header_height),
            ..area
        };

        // Column widths are measured over the rows actually on screen, so the
        // route column lands at one predictable offset for the whole page
        // instead of drifting with whatever long id happens to be scrolled in.
        let mut columns =
            ModelRowColumns::for_page(&rows[start.min(rows.len())..end.min(rows.len())]);
        if header_height == 2 {
            columns.primary = columns.primary.max(7);
            columns.route = columns.route.max(10);
            columns.meta = columns.meta.max(9);
            self.render_sort_columns(Rect::new(area.x, area.y + 1, area.width, 1), buf, columns);
        }

        let mut lines = Vec::with_capacity(end.saturating_sub(start));
        let pane_height = usize::from(inner.height);
        for (idx, row) in rows.iter().enumerate().skip(start).take(end - start) {
            // Family headers consume pane lines too: stop building (and stop
            // recording hitboxes) as soon as the pane is full, so rendering
            // never addresses the buffer past its bounds.
            if lines.len() >= pane_height {
                break;
            }
            let is_selected = idx == state.selected;
            // Only the focused pane owns the keyboard cursor. Unavailable
            // routes retain a width-safe attention mark and warning ink.
            let locked = row.locked;
            let focused = is_selected && state.focused;
            let marker = if focused {
                crate::tui::glyphs::SELECTION
            } else if locked {
                crate::tui::glyphs::ATTENTION
            } else if row.active {
                crate::tui::glyphs::CURRENT
            } else {
                " "
            };
            let hovered = self.hovered_row == Some((state.pane, idx)) && !focused;
            let label_style = if focused {
                menu_style::selected_row_style()
            } else if hovered {
                menu_style::hovered_row_style().fg(if locked {
                    palette::TEXT_MUTED
                } else {
                    palette::TEXT_PRIMARY
                })
            } else if is_selected {
                Style::default()
                    .fg(palette::TEXT_MUTED)
                    .bg(palette::SURFACE_ELEVATED)
            } else if locked || row.dim {
                Style::default()
                    .fg(palette::TEXT_MUTED)
                    .add_modifier(Modifier::DIM)
            } else {
                Style::default().fg(palette::TEXT_PRIMARY)
            };
            let hint_style = if focused {
                menu_style::selected_row_bg_style().fg(palette::SELECTION_TEXT)
            } else if locked || row.dim {
                label_style.fg(palette::TEXT_MUTED)
            } else if hovered {
                menu_style::hovered_row_style().fg(palette::TEXT_MUTED)
            } else {
                Style::default().fg(palette::TEXT_MUTED)
            };
            // Provider → family → model grouping: a dim family header is
            // drawn when the catalog states a family and it differs from the
            // previous visible row's (families sort contiguously). Unknown
            // families draw nothing.
            if family_header_before(rows, idx) && pane_height > 1 {
                lines.push(Line::from(Span::styled(
                    format!("  ─ {}", row.family.as_deref().unwrap_or_default()),
                    Style::default().fg(palette::TEXT_DIM),
                )));
            }
            // The hitbox points at the row's own line (after any family
            // header), so mouse/scan targets and keyboard targets agree.
            let row_y = inner.y.saturating_add(lines.len() as u16);
            self.row_hitboxes.borrow_mut().push((
                Rect::new(inner.x, row_y, inner.width, 1),
                state.pane,
                idx,
            ));
            if focused || hovered {
                buf.set_style(Rect::new(inner.x, row_y, inner.width, 1), label_style);
            }
            let spans = picker_row_spans(
                row,
                marker,
                usize::from(inner.width),
                columns,
                label_style,
                hint_style,
            );
            lines.push(Line::from(spans).style(if focused || hovered {
                label_style
            } else {
                Style::default()
            }));
        }
        if rows.is_empty() {
            // A search that matches nothing must say so, not render a bare
            // empty box (#3757 UX review).
            let message = if self.query.is_empty() {
                tr(self.locale, MessageId::RouteNoModels).into_owned()
            } else {
                tr(self.locale, MessageId::RouteNoModelMatch).replace("{query}", &self.query)
            };
            lines.push(Line::from(Span::styled(
                message,
                Style::default().fg(palette::TEXT_MUTED),
            )));
        }
        // Family headers can push the visible rows past the viewport; clip
        // to the area so rendering never indexes the buffer out of bounds
        // (ratatui-core 0.1.0 panics instead of clipping).
        if lines.len() > usize::from(inner.height) {
            lines.truncate(usize::from(inner.height));
        }
        Paragraph::new(lines).render(inner, buf);
    }
}

fn family_header_before(rows: &[PaneRow], index: usize) -> bool {
    let row = &rows[index];
    row.family.as_deref().is_some_and(|family| {
        rows.get(index.wrapping_sub(1)).is_none_or(|previous| {
            previous.family.as_deref() != Some(family) || previous.route != row.route
        })
    })
}

fn pane_row_window(selected: usize, rows: &[PaneRow], height: usize) -> (usize, usize) {
    if rows.is_empty() || height == 0 {
        return (0, 0);
    }
    let selected = selected.min(rows.len() - 1);
    let cost = |index| 1 + usize::from(height > 1 && family_header_before(rows, index));
    // Keep the selection near the middle, counting actual painted lines.
    let mut start = selected;
    let mut above = 0;
    while start > 0 && above + cost(start - 1) <= height.saturating_sub(cost(selected)) / 2 {
        start -= 1;
        above += cost(start);
    }
    let mut end = start;
    let mut used = 0;
    while end < rows.len() && used + cost(end) <= height {
        used += cost(end);
        end += 1;
    }
    // Fill the space above when we reach the end of the list.
    while start > 0 && used + cost(start - 1) <= height {
        start -= 1;
        used += cost(start);
    }
    (start, end)
}

/// Widest Thinking row plus its marker: `max  (extra-high reasoning)`.
const EFFORT_PANE_WIDTH: u16 = 30;

/// Give the model list the width the Thinking pane cannot use.
///
/// The generic list/detail split caps the list at 52 columns and hands the
/// remainder to the detail pane. Thinking rows are a fixed, short vocabulary,
/// so on a wide terminal most of the row went to a pane with nothing to put
/// there while the model rows — which carry the id, route and metadata that
/// tell near-identical routes apart — were squeezed into half a screen.
fn widen_model_pane(layout: ListDetailLayout) -> ListDetailLayout {
    if layout.stacked {
        return layout;
    }
    let gap = layout
        .detail
        .x
        .saturating_sub(layout.list.x.saturating_add(layout.list.width));
    let total = layout.list.width + gap + layout.detail.width;
    let detail_width = layout.detail.width.min(EFFORT_PANE_WIDTH);
    let list_width = total.saturating_sub(gap + detail_width);
    ListDetailLayout {
        list: Rect {
            width: list_width,
            ..layout.list
        },
        detail: Rect {
            x: layout.list.x + list_width + gap,
            width: detail_width,
            ..layout.detail
        },
        stacked: false,
    }
}

/// One rendered row in either picker pane, split into the columns the row is
/// laid out from.
///
/// Model rows fill all three: the wire id (`primary`), the route identity that
/// separates same-named models on different endpoints (`route`), and the facts
/// that actually vary between neighbouring rows (`meta`). Thinking-effort rows
/// leave `route` empty and keep their descriptive `meta`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct PaneRow {
    primary: String,
    route: String,
    /// Metadata as separable units. Kept as a list so a squeezed column sheds
    /// whole facts instead of rendering half a word.
    meta: Vec<String>,
    /// Catalog model family (e.g. `deepseek`, `glm`) for section headers.
    /// None = the catalog did not state a family (no header is drawn).
    family: Option<String>,
    /// The route this session is already on.
    active: bool,
    locked: bool,
    /// Muted ink for a selectable row with a caveat (not listed by a fresh
    /// roster); unlike `locked` it keeps the ordinary marker.
    dim: bool,
}

impl PaneRow {
    fn effort(primary: String, meta: String) -> Self {
        Self {
            primary,
            route: String::new(),
            meta: if meta.is_empty() {
                Vec::new()
            } else {
                vec![meta]
            },
            family: None,
            active: false,
            locked: false,
            dim: false,
        }
    }

    fn meta_width(&self) -> usize {
        self.meta
            .iter()
            .map(|chip| unicode_width::UnicodeWidthStr::width(chip.as_str()))
            .sum::<usize>()
            + self.meta.len().saturating_sub(1) * 3
    }
}

/// Per-page column offsets for a picker pane.
///
/// Rows used to render as `label  (one long parenthesised hint)`, which meant
/// the hint was dropped whole whenever it did not fit — and at every real
/// terminal width it never fit, so a dozen DeepSeek routes all rendered as
/// nothing but their near-identical ids. Fixed columns fix that: each field
/// gets a measured share of the row and is truncated on its own, so the
/// distinguishing token is always on screen at a predictable offset.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ModelRowColumns {
    primary: usize,
    route: usize,
    meta: usize,
}

/// Width reserved for the marker glyph itself. The lock is a two-column emoji
/// while `▸` and `●` are one, so the cell is padded to the widest of them —
/// otherwise a single locked row shifts every column on its line by one.
const MARKER_CELL_WIDTH: usize = 2;
/// ` ▸  ` — one leading space, the marker cell, one trailing space.
const ROW_PREFIX_WIDTH: usize = MARKER_CELL_WIDTH + 2;
/// Blank cells between two columns.
const COLUMN_GAP: usize = 2;
/// Below this a route column tells the user nothing, so the space goes to the
/// id instead.
const MIN_ROUTE_WIDTH: usize = 6;
/// Below this the metadata column cannot hold even a context-window token.
const MIN_META_WIDTH: usize = 4;

impl ModelRowColumns {
    /// Measure the natural width each column wants, over the rows on screen.
    fn for_page(rows: &[PaneRow]) -> Self {
        let widest = |pick: fn(&PaneRow) -> usize| rows.iter().map(pick).max().unwrap_or(0);
        Self {
            primary: widest(|row| unicode_width::UnicodeWidthStr::width(row.primary.as_str())),
            route: widest(|row| unicode_width::UnicodeWidthStr::width(row.route.as_str())),
            meta: widest(PaneRow::meta_width),
        }
    }

    /// Fit the measured widths into the width actually available.
    ///
    /// When everything fits, every column keeps its natural width. When it does
    /// not, the scarce space is divided rather than handed to whichever column
    /// comes first: the id used to take everything and the metadata was dropped
    /// whole, which is precisely how a dozen near-identical routes ended up
    /// rendering as nothing but their shared prefix.
    fn resolve(self, width: usize) -> Self {
        let available = width.saturating_sub(ROW_PREFIX_WIDTH);
        if available == 0 {
            return Self::default();
        }
        let gaps = COLUMN_GAP * (usize::from(self.route > 0) + usize::from(self.meta > 0));
        let content = available.saturating_sub(gaps);
        if content == 0 {
            return Self {
                primary: available,
                route: 0,
                meta: 0,
            };
        }
        if self.primary + self.route + self.meta <= content {
            return self;
        }
        // With no route column there is nothing to protect from a long id, so
        // the id keeps its natural width and the trailing metadata yields — a
        // clipped model id is worse than a hidden hint.
        if self.route == 0 {
            let primary = self.primary.min(content);
            let meta = self.meta.min(content.saturating_sub(primary));
            return Self {
                primary,
                route: 0,
                meta: if meta < MIN_META_WIDTH { 0 } else { meta },
            };
        }

        // Floors first, so no column that has something to say disappears
        // entirely; then each takes the smaller of its natural width and its
        // share. Metadata is the densest per column and gets the tightest cap.
        let mut meta = if self.meta == 0 {
            0
        } else {
            self.meta
                .min((content / 4).max(MIN_META_WIDTH.min(content)))
        };
        let after_meta = content.saturating_sub(meta);
        let mut route = if self.route == 0 {
            0
        } else {
            self.route
                .min((after_meta / 3).max(MIN_ROUTE_WIDTH.min(after_meta)))
        };
        let mut primary = after_meta.saturating_sub(route);

        // The id's share is whatever the other two did not take, which can
        // exceed the longest id on the page. Hand that surplus back rather than
        // padding blank space next to a metadata column that is shedding facts.
        if primary > self.primary {
            let mut slack = primary - self.primary;
            primary = self.primary;
            for (column, natural) in [(&mut meta, self.meta), (&mut route, self.route)] {
                let gain = slack.min(natural.saturating_sub(*column));
                *column += gain;
                slack -= gain;
            }
            primary += slack;
        }

        Self {
            primary,
            route,
            meta,
        }
    }
}

/// Truncate an identifier from the middle, keeping both ends.
///
/// Model ids and route names share their heads and differ in their tails:
/// `deepseek-ai/DeepSeek-V4-Pro` and `deepseek-ai/DeepSeek-V4-Flash` are
/// identical for twenty characters and only separate at the very end. Clipping
/// the tail therefore deletes the one token that tells them apart — both rows
/// render as `deepseek-ai/DeepSee...`. Keeping a slice of each end costs one
/// column for the ellipsis and preserves the variant.
fn fit_identifier(text: &str, width: usize) -> String {
    use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

    if UnicodeWidthStr::width(text) <= width {
        return text.to_string();
    }
    // Too narrow to seat a head, an ellipsis and a meaningful tail; fall back
    // to the plain head-first form rather than emit punctuation soup.
    if width < 8 {
        return fit_text(text, width);
    }

    let budget = width - 1;
    // The tail is the discriminating end, so it gets the larger share.
    let tail_budget = (budget * 3) / 5;
    let head_budget = budget - tail_budget;

    let mut head = String::new();
    let mut used = 0usize;
    for ch in text.chars() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + ch_width > head_budget {
            break;
        }
        used += ch_width;
        head.push(ch);
    }

    let mut tail: Vec<char> = Vec::new();
    let mut used = 0usize;
    for ch in text.chars().rev() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + ch_width > tail_budget {
            break;
        }
        used += ch_width;
        tail.push(ch);
    }
    tail.reverse();

    let mut out = head;
    out.push('…');
    out.extend(tail);
    out
}

/// Lay a row out into aligned, individually-truncated columns.
///
/// Primary and secondary ink follow the pane's focus; unavailable routes
/// retain a semantic warning mark independently of selection.
fn picker_row_spans<'a>(
    row: &'a PaneRow,
    marker: &'static str,
    width: usize,
    columns: ModelRowColumns,
    label_style: Style,
    hint_style: Style,
) -> Vec<Span<'a>> {
    use unicode_width::UnicodeWidthStr;

    let columns = columns.resolve(width);
    let marker_pad = MARKER_CELL_WIDTH.saturating_sub(UnicodeWidthStr::width(marker));
    let mut spans = vec![
        Span::styled(" ", label_style),
        Span::styled(
            marker,
            if row.locked {
                label_style.fg(palette::STATUS_WARNING)
            } else {
                label_style
            },
        ),
        Span::styled(" ".repeat(marker_pad + 1), label_style),
    ];
    let mut used = ROW_PREFIX_WIDTH;

    let primary = fit_identifier(&row.primary, columns.primary.max(1));
    used += UnicodeWidthStr::width(primary.as_str());
    spans.push(Span::styled(primary, label_style));

    // Pad to the column edge only when something follows; a trailing run of
    // spaces would otherwise extend the selected row's highlight past its text.
    let pad_to = |spans: &mut Vec<Span<'a>>, used: &mut usize, target: usize| {
        if *used < target {
            spans.push(Span::styled(" ".repeat(target - *used), label_style));
            *used = target;
        }
    };

    if columns.route > 0 && !row.route.is_empty() {
        pad_to(&mut spans, &mut used, ROW_PREFIX_WIDTH + columns.primary);
        spans.push(Span::styled(" ".repeat(COLUMN_GAP), label_style));
        used += COLUMN_GAP;
        let route = fit_identifier(&row.route, columns.route);
        used += UnicodeWidthStr::width(route.as_str());
        spans.push(Span::styled(route, hint_style));
    }

    if !row.meta.is_empty() {
        let column_edge = if columns.route > 0 && !row.route.is_empty() {
            ROW_PREFIX_WIDTH + columns.primary + COLUMN_GAP + columns.route
        } else {
            ROW_PREFIX_WIDTH + columns.primary
        };
        // Take the smaller of the column's share and the physical remainder, so
        // a row that ended early cannot overrun the pane.
        let remaining = width
            .saturating_sub(column_edge)
            .saturating_sub(COLUMN_GAP)
            .min(columns.meta.max(MIN_META_WIDTH));
        let meta = fit_meta_chips(&row.meta, remaining);
        if !meta.is_empty() {
            pad_to(&mut spans, &mut used, column_edge);
            spans.push(Span::styled(" ".repeat(COLUMN_GAP), label_style));
            spans.push(Span::styled(meta, hint_style));
        }
    }

    spans
}

fn fit_text(text: &str, width: usize) -> String {
    use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

    if UnicodeWidthStr::width(text) <= width {
        return text.to_string();
    }
    if width == 0 {
        return String::new();
    }
    if width <= 3 {
        return ".".repeat(width);
    }

    let mut out = String::new();
    let target = width - 3;
    let mut used = 0usize;
    for ch in text.chars() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + ch_width > target {
            break;
        }
        used += ch_width;
        out.push(ch);
    }
    out.push_str("...");
    out
}

pub(crate) fn provider_scoped_model_completion_ids(app: &App) -> Vec<String> {
    // Slash completions inline the current custom model so `/model <current>`
    // stays visible even when it is outside the provider catalog.
    provider_scoped_model_ids_for_app(app, true)
}

/// The pin (Fleet model or the person's own) naming this exact row, if any.
fn pin_for_row<'a>(pins: &'a [PinnedModel], row: &ModelPickerRow) -> Option<&'a PinnedModel> {
    pins.iter().find(|pin| pin_names_row(pin, row))
}

/// One route match for sorting, marking and labelling alike. Built-in
/// provider slugs are case-insensitive aliases; a named custom provider's
/// identity is its exact config key, so `TeamA` and `teama` stay two routes.
/// Model ids are exact.
fn pin_names_row(pin: &PinnedModel, row: &ModelPickerRow) -> bool {
    let named_custom = row.provider == Some(ApiProvider::Custom);
    row_provider_identity(row).is_some_and(|provider| {
        if named_custom {
            provider == pin.provider
        } else {
            provider.eq_ignore_ascii_case(&pin.provider)
        }
    }) && row.id == pin.model
}

/// The pins the picker sorts and labels by: the fleet's models first (the
/// selected Fleet's operator and every pinned member, labelled with the roles
/// each fills — design §10 F1), then the person's own pins.
fn picker_pins_for_app(app: &App) -> Vec<PinnedModel> {
    // A selected fleet that cannot be read contributes no pins; ⇧F on any
    // row then surfaces that store error instead of writing past it.
    crate::fleet::members::fleet_models(&app.workspace)
        .unwrap_or_default()
        .into_iter()
        .map(|member| PinnedModel {
            provider: member.provider.clone(),
            model: member.model.clone(),
            label: Some(format!("fleet · {}", member.roles_label())),
        })
        .chain(app.pinned_models.iter().cloned())
        .collect()
}

fn picker_model_rows_for_app(app: &App, config: &Config) -> Vec<ModelPickerRow> {
    let mut rows = Vec::new();
    let auto_hint = auto_picker_hint(app, config);
    push_auto_model_row(&mut rows, app, config, &auto_hint);
    // One snapshot supplies both IDs, capabilities, and freshness so a cache
    // replacement cannot produce mixed-generation picker rows.
    let codex_roster = codex_model_cache::model_roster();
    let mut active_model_ids = if app.api_provider == ApiProvider::OpenaiCodex {
        let mut models = vec!["auto".to_string()];
        for id in codex_roster.model_ids() {
            push_model_id(&mut models, &id);
        }
        if let Some(model) = app
            .provider_models
            .get(app.provider_identity_for_persistence())
            .map(|model| model.trim())
            .filter(|model| !model.is_empty())
        {
            push_model_id(
                &mut models,
                picker_visible_model_id(app.api_provider, model, app.accepts_custom_model_ids()),
            );
        }
        models
    } else {
        provider_scoped_model_ids_for_app(app, false)
    };
    push_recent_model_ids(
        &mut active_model_ids,
        app,
        app.provider_identity_for_persistence(),
    );
    push_configured_provider_model(
        &mut active_model_ids,
        config,
        app.api_provider,
        app.provider_identity_for_persistence(),
    );
    push_provider_model_rows(
        &mut rows,
        app.api_provider,
        (app.api_provider == ApiProvider::Custom).then(|| app.provider_identity_for_persistence()),
        active_model_ids,
        app.api_provider,
        config,
        &codex_roster,
        &app.provider_health,
    );

    for provider in ApiProvider::sorted_for_display() {
        // Every named custom route shares `ApiProvider::Custom`, so the enum
        // alone can neither name a route nor say which ones are already
        // listed. Enumerate the configured tables by exact identity (#6016):
        // a session resumed on another provider still sees every custom route
        // it has configured, and one custom route never stands in for another.
        if provider == ApiProvider::Custom {
            for identity in inactive_custom_route_identities(app, config) {
                push_inactive_route_rows(
                    &mut rows,
                    app,
                    config,
                    provider,
                    &identity,
                    &codex_roster,
                );
            }
            continue;
        }
        if provider == app.api_provider {
            continue;
        }
        push_inactive_route_rows(
            &mut rows,
            app,
            config,
            provider,
            provider.as_str(),
            &codex_roster,
        );
    }

    // The fleet comes first (design §10 F1): every model the person added
    // to the selected Fleet rides the pin machinery ahead of their own pins,
    // labelled with the roles it fills, so the list leads with what they
    // chose rather than with a provider's alphabet.
    let pins = picker_pins_for_app(app);
    for row in &mut rows {
        if let Some(pin) = pin_for_row(&pins, row) {
            let pinned = tr(app.ui_locale, MessageId::ModelPickerPinnedChip);
            let label = pin.label.as_deref().unwrap_or(&pinned);
            row.hint = format!(
                "{label} · exact {} / {} · {}",
                pin.provider, pin.model, row.hint
            );
        }
    }

    for pin in &pins {
        let provider = ApiProvider::parse(&pin.provider).unwrap_or(ApiProvider::Custom);
        if rows.iter().any(|row| {
            row_provider_identity(row).is_some_and(|identity| identity == pin.provider)
                && row.id == pin.model
        }) {
            continue;
        }
        let metadata = effective_picker_metadata_for_identity(
            config,
            Some(provider),
            Some(&pin.provider),
            &pin.model,
        );
        // Bypass the ordinary `(enum provider, model)` de-duplication here:
        // two named Custom routes may intentionally expose the same model id.
        rows.push(ModelPickerRow {
            id: pin.model.clone(),
            provider: Some(provider),
            provider_identity: Some(pin.provider.clone()),
            hint: format!(
                "stale pinned · exact {} / {} · unavailable; repair or remove",
                pin.provider, pin.model
            ),
            metadata,
            selectable: false,
            blocked_reason: Some("stale pin".to_string()),
            default_rank: None,
            credentialed: false,
            not_listed: None,
        });
    }

    let current = (!app.auto_model).then(|| {
        (
            app.provider_identity_for_persistence(),
            picker_visible_model_id(app.api_provider, &app.model, app.accepts_custom_model_ids()),
        )
    });
    assign_default_sections(&mut rows, app, config, current, &pins);
    rows
}

/// Place rows into the default view's sections (#6533): the current route,
/// then pins and Fleet models, then up to [`RECENT_ROW_LIMIT`] recently used
/// routes by decayed use, then one default per credentialed route. A leftover
/// `[providers.X].model` is that default only when the route is current or
/// recently used; an unused route offers its built-in default instead, so a
/// test model saved months ago no longer rides along. User-declared
/// `[[models]]` rows keep their place. Everything else stays behind search.
fn assign_default_sections(
    rows: &mut [ModelPickerRow],
    app: &App,
    config: &Config,
    current: Option<(&str, &str)>,
    pins: &[PinnedModel],
) {
    let now = chrono::Utc::now();
    let guard = app.route_usage.read().ok();
    let empty = crate::model_relevance::RouteUsageIndex::default();
    let usage = guard.as_deref().unwrap_or(&empty);
    let find = |rows: &[ModelPickerRow], identity: &str, model: &str| {
        rows.iter().position(|row| {
            row_provider_identity(row) == Some(identity) && row.id.eq_ignore_ascii_case(model)
        })
    };
    for row in rows.iter_mut() {
        row.default_rank = None;
        // `auto` is the current route when auto routing is on; otherwise it
        // follows the pins, ahead of recent use.
        if row.provider.is_none() {
            row.default_rank = Some(if current.is_none() {
                (DefaultSection::Current, 0)
            } else {
                (DefaultSection::Pinned, usize::MAX)
            });
        }
    }
    if let Some((identity, model)) = current
        && let Some(index) = find(rows, identity, model)
    {
        rows[index].default_rank = Some((DefaultSection::Current, 0));
    }
    for (rank, pin) in pins.iter().enumerate() {
        // Exact, as the pin labels match: a pin names one wire spelling.
        if let Some(row) = rows.iter_mut().find(|row| {
            row_provider_identity(row) == Some(pin.provider.as_str()) && row.id == pin.model
        }) {
            row.default_rank
                .get_or_insert((DefaultSection::Pinned, rank));
        }
    }
    let mut recent = 0;
    for route in usage.ranked(now) {
        if recent >= RECENT_ROW_LIMIT {
            break;
        }
        if let Some(index) = find(rows, &route.identity, &route.model)
            && rows[index].default_rank.is_none()
        {
            rows[index].default_rank = Some((DefaultSection::Recent, recent));
            recent += 1;
        }
    }
    let mut routes: Vec<(String, ApiProvider)> = Vec::new();
    for row in rows.iter() {
        if let (Some(provider), Some(identity)) = (row.provider, row_provider_identity(row))
            && !routes.iter().any(|(seen, _)| seen == identity)
        {
            routes.push((identity.to_string(), provider));
        }
    }
    for (order, (identity, provider)) in routes.iter().enumerate() {
        let in_use = current.is_some_and(|(current, _)| current == identity)
            || usage.identity_used(identity, now);
        let configured = || {
            app.provider_models
                .get(identity)
                .map(String::as_str)
                .or_else(|| {
                    route_provider_config(config, *provider, identity)
                        .and_then(|entry| entry.model.as_deref())
                })
                .map(str::to_string)
        };
        let default = if in_use || *provider == ApiProvider::Custom {
            configured()
        } else {
            provider
                .kind()
                .map(|kind| kind.provider().default_model().to_string())
        };
        let Some(default) = default
            .as_deref()
            .map(str::trim)
            .filter(|model| !model.is_empty() && !model.eq_ignore_ascii_case("auto"))
            .map(|model| {
                picker_visible_model_id(
                    *provider,
                    model,
                    config.model_ids_pass_through_for_provider(*provider),
                )
                .to_string()
            })
        else {
            continue;
        };
        if let Some(index) = find(rows, identity, &default)
            && rows[index].default_rank.is_none()
            && rows[index].credentialed
        {
            rows[index].default_rank = Some((DefaultSection::ProviderDefault, order));
        }
    }
    for row in rows.iter_mut() {
        if row.default_rank.is_none()
            && matches!(row.metadata.source, Some(CatalogSource::ConfigOverride))
        {
            row.default_rank = Some((DefaultSection::ProviderDefault, usize::MAX));
        }
    }
}

/// Model ids this exact route was used with recently, so a model picked
/// through `--model` or another client still has a row to rank.
fn push_recent_model_ids(models: &mut Vec<String>, app: &App, identity: &str) {
    if let Ok(usage) = app.route_usage.read() {
        for model in usage.models_for_identity(identity, chrono::Utc::now()) {
            push_model_id(models, &model);
        }
    }
}

/// Every configured custom route except the one this session is actually on.
///
/// Ordered by identity so the picker's row order is stable across rebuilds;
/// case-distinct tables stay distinct routes, exactly as the catalog and
/// credential stores treat them.
fn inactive_custom_route_identities(app: &App, config: &Config) -> Vec<String> {
    let active =
        (app.api_provider == ApiProvider::Custom).then(|| app.provider_identity_for_persistence());
    let mut identities: Vec<String> = config
        .providers
        .as_ref()
        .map(|providers| {
            providers
                .custom
                .iter()
                .filter(|(_, entry)| entry.is_openai_compatible_custom())
                .map(|(identity, _)| identity.clone())
                .collect()
        })
        .unwrap_or_default();
    identities.sort();
    identities.retain(|identity| active != Some(identity.as_str()));
    identities
}

/// Rows for one route that is not the session's active route.
///
/// `identity` is the exact persistence key — the `[providers.<name>]` table
/// for a named custom route, the provider slug otherwise — and every lookup
/// here is made against it, so two routes that expose the same model id keep
/// separate rows, separate remembered choices, and separate enablement.
fn push_inactive_route_rows(
    rows: &mut Vec<ModelPickerRow>,
    app: &App,
    config: &Config,
    provider: ApiProvider,
    identity: &str,
    codex_roster: &CodexModelRoster,
) {
    let mut model_ids = if provider == ApiProvider::OpenaiCodex {
        codex_roster.model_ids()
    } else {
        provider_catalog_model_ids(
            provider,
            identity,
            &config.base_url_for_route_identity(provider, identity),
        )
    };
    if let Some(model) = app
        .provider_models
        .get(identity)
        .map(|model| model.trim())
        .filter(|model| !model.is_empty())
    {
        push_model_id(
            &mut model_ids,
            picker_visible_model_id(
                provider,
                model,
                config.model_ids_pass_through_for_provider(provider),
            ),
        );
    }
    push_recent_model_ids(&mut model_ids, app, identity);
    push_configured_provider_model(&mut model_ids, config, provider, identity);
    push_provider_model_rows(
        rows,
        provider,
        (provider == ApiProvider::Custom).then_some(identity),
        model_ids,
        app.api_provider,
        config,
        codex_roster,
        &app.provider_health,
    );
}

/// The `[providers.…]` table that owns this exact route.
///
/// [`Config::provider_config_for`] resolves `Custom` through the *selected*
/// `provider = "<name>"`, which cannot describe a custom route the session is
/// not on — and must never answer for one (#6016).
fn route_provider_config<'a>(
    config: &'a Config,
    provider: ApiProvider,
    identity: &str,
) -> Option<&'a crate::config::ProviderConfig> {
    if provider == ApiProvider::Custom {
        return config
            .providers
            .as_ref()?
            .custom_provider_config(identity.trim());
    }
    config.provider_config_for(provider)
}

fn push_provider_model_rows(
    rows: &mut Vec<ModelPickerRow>,
    provider: ApiProvider,
    provider_identity: Option<&str>,
    mut model_ids: Vec<String>,
    active_provider: ApiProvider,
    config: &Config,
    codex_roster: &CodexModelRoster,
    provider_health: &crate::provider_readiness::ProviderReadinessSnapshot,
) {
    let identity = provider_identity
        .map(str::to_string)
        .unwrap_or_else(|| config.provider_identity_for(provider));
    // Readiness resolves a custom route's endpoint, auth class and credentials
    // through the selected `provider = "<name>"`, so an inactive named route
    // would otherwise be judged by the active route's credentials. Scope one
    // copy to this exact identity — the same re-pointing the provider
    // dashboard uses for its per-route rows.
    let scoped_config;
    let config = if provider == ApiProvider::Custom
        && config.provider.as_deref().map(str::trim) != Some(identity.as_str())
    {
        scoped_config = {
            let mut scoped = config.clone();
            scoped.provider = Some(identity.clone());
            scoped
        };
        &scoped_config
    } else {
        config
    };
    let base_url = config.base_url_for_route_identity(provider, &identity);
    let fresh_roster = fresh_roster_listing(provider, &identity, &base_url);
    for declaration in config.custom_models.as_deref().unwrap_or_default() {
        if crate::provider_lake::configured_model_for_route(
            config,
            provider,
            &identity,
            &base_url,
            &declaration.id,
        )
        .is_some()
            && !model_ids.contains(&declaration.id)
        {
            model_ids.push(declaration.id.clone());
        }
    }
    for id in model_ids {
        if id == "auto" {
            continue;
        }
        let readiness =
            crate::provider_readiness::resolve_for_model(config, provider, &id, provider_health);
        let selectable = readiness.can_attempt();
        let readiness_label = readiness.label();
        let roster_entry = if provider == ApiProvider::OpenaiCodex {
            codex_roster.metadata_for(&id)
        } else {
            None
        };
        let codex_metadata = if codex_roster.freshness == CodexModelCacheFreshness::Fresh {
            roster_entry
        } else {
            None
        };
        let codex_freshness = roster_entry.map(|_| codex_roster.freshness);
        let metadata = effective_picker_metadata_with_codex(
            config,
            Some(provider),
            provider_identity,
            &id,
            codex_metadata,
        );
        let provider_catalog_receipt = provider_catalog_receipt_for_route(
            provider,
            provider_identity,
            config,
            metadata.source.as_ref(),
        );
        let mut hint = render_picker_model_hint(
            &id,
            Some(provider),
            &metadata,
            codex_freshness,
            provider_catalog_receipt.as_ref(),
        );
        if metadata.display_name.is_some() {
            hint = format!("{id} · {hint}");
        }
        hint = format!("{readiness_label} · {hint}");
        if provider != active_provider {
            hint = format!("switch route · {hint}");
        }
        let blocked_reason = (!selectable).then(|| readiness_label.to_string());
        let credentialed = matches!(
            readiness,
            crate::provider_readiness::ResolvedProviderReadiness::SavedUnchecked
                | crate::provider_readiness::ResolvedProviderReadiness::ImportedTokenUnchecked
                | crate::provider_readiness::ResolvedProviderReadiness::Ready
                | crate::provider_readiness::ResolvedProviderReadiness::ConnectionCheckedModelUnchecked
                | crate::provider_readiness::ResolvedProviderReadiness::SavedLastCheckFailed { .. }
        );
        let not_listed = fresh_roster.as_ref().and_then(|(listed, checked)| {
            (!listed.iter().any(|listed| listed.eq_ignore_ascii_case(&id))).then(|| NotListed {
                provider: if provider == ApiProvider::Custom {
                    identity.clone()
                } else {
                    provider.display_name().to_string()
                },
                checked: checked.clone(),
            })
        });
        let before = rows.len();
        push_model_row(
            rows,
            id.clone(),
            Some(provider),
            provider_identity.map(str::to_string),
            hint,
            metadata,
            selectable,
            blocked_reason,
        );
        if rows.len() > before
            && let Some(row) = rows.last_mut()
        {
            row.credentialed = credentialed;
            row.not_listed = not_listed;
        }
    }
}

/// Ids a FRESH live roster lists for this exact route, with the fetch date.
/// `None` when no fresh roster exists — a stale, failed, or absent roster
/// cannot prove an id is gone (the same rule as
/// `provider_catalog_live::pin_missing_from_fresh_roster`, #6035). Attested
/// unlisted ids count as listed.
fn fresh_roster_listing(
    provider: ApiProvider,
    identity: &str,
    base_url: &str,
) -> Option<(Vec<String>, String)> {
    if provider == ApiProvider::OpenaiCodex {
        return None;
    }
    // Reading the entry first loads a roster an earlier process persisted.
    let entry = crate::provider_catalog_live::cached_entry_for_route(provider, identity, base_url)
        .ok()
        .flatten()?;
    if crate::provider_catalog_live::status_for_route(provider, identity, base_url)
        != CatalogStatus::Fresh
    {
        return None;
    }
    let mut listed = crate::provider_lake::catalog_models_for_route(provider, identity, base_url);
    for offering in &entry.offerings {
        listed.push(picker_visible_model_id(provider, &offering.wire_model_id, false).to_string());
        if let Some(canonical) = &offering.canonical_model {
            listed.push(canonical.clone());
        }
    }
    let checked = chrono::DateTime::from_timestamp(i64::try_from(entry.fetched_at).ok()?, 0)?
        .format("%Y-%m-%d")
        .to_string();
    Some((listed, checked))
}

fn provider_catalog_receipt_for_route(
    provider: ApiProvider,
    provider_identity: Option<&str>,
    config: &Config,
    source: Option<&CatalogSource>,
) -> Option<(CatalogStatus, bool)> {
    let identity = provider_identity.unwrap_or_else(|| provider.as_str());
    // A custom route owns its catalog only on Baseten's endpoint, whose
    // account-scoped roster no snapshot can serve (#6289).
    let owns_provider_catalog = matches!(
        provider,
        ApiProvider::Openrouter
            | ApiProvider::Telecomjs
            | ApiProvider::Edenai
            | ApiProvider::Zenmux
    ) || (provider == ApiProvider::Custom
        && codewhale_config::catalog::endpoint_is_baseten(
            &config.base_url_for_route_identity(provider, identity),
        ));
    if !owns_provider_catalog {
        return None;
    }

    let base_url = config.base_url_for_route_identity(provider, identity);
    let endpoint_matches = match source {
        Some(CatalogSource::Live {
            base_url_fingerprint,
            ..
        }) => *base_url_fingerprint == codewhale_config::catalog::base_url_fingerprint(&base_url),
        // A bundled/template fallback has no endpoint claim to compare. Its
        // exact-scope status still matters: a first refresh failure must be
        // visible even though no live row exists yet.
        _ => true,
    };
    Some((
        crate::provider_catalog_live::status_for_route(provider, identity, &base_url),
        endpoint_matches,
    ))
}

fn push_auto_model_row(rows: &mut Vec<ModelPickerRow>, app: &App, config: &Config, hint: &str) {
    let readiness = crate::provider_readiness::resolve_for_model(
        config,
        app.api_provider,
        "auto",
        &app.provider_health,
    );
    let metadata = effective_picker_metadata(config, None, "auto");
    let selectable = readiness.can_attempt();
    let blocked_reason = (!selectable).then(|| readiness.label().to_string());
    push_model_row(
        rows,
        "auto".to_string(),
        None,
        None,
        format!("{} · {hint}", readiness.label()),
        metadata,
        selectable,
        blocked_reason,
    );
}

fn auto_picker_hint(app: &App, config: &Config) -> String {
    let inventory = crate::model_inventory::ModelInventory::from_config(config);
    // #4411: the classifier only sees other providers under the persisted
    // `[auto] cross_provider` opt-in, so the default hint says active provider
    // only and names the classifier route it will actually call.
    let hint_id = match (inventory.router_available, inventory.cross_provider_auto) {
        (true, true) => MessageId::ModelPickerAutoNetworkHint,
        (true, false) => MessageId::ModelPickerAutoNetworkActiveProviderHint,
        (false, _) => MessageId::ModelPickerAutoLocalHint,
    };
    let mut hint = app
        .tr(hint_id)
        .into_owned()
        .replace("{provider}", inventory.router_provider.display_name())
        .replace("{model}", &inventory.router_model);
    if let (Some(provider), Some(model)) = (
        app.last_effective_provider,
        app.last_effective_model.as_deref(),
    ) {
        let provider_label = if provider == ApiProvider::Custom {
            app.last_effective_provider_identity
                .as_deref()
                .unwrap_or_else(|| app.provider_identity_for_persistence())
        } else {
            provider.display_name()
        };
        let last = app
            .tr(MessageId::ModelPickerAutoLastRoute)
            .replace("{provider}", provider_label)
            .replace("{model}", model);
        hint.push_str(" · ");
        hint.push_str(&last);
    }
    hint
}

fn push_configured_provider_model(
    models: &mut Vec<String>,
    config: &Config,
    provider: ApiProvider,
    identity: &str,
) {
    if let Some(model) = route_provider_config(config, provider, identity)
        .and_then(|entry| entry.model.as_deref())
        .map(str::trim)
        .filter(|model| !model.is_empty())
    {
        push_model_id(
            models,
            picker_visible_model_id(
                provider,
                model,
                config.model_ids_pass_through_for_provider(provider),
            ),
        );
    }
}

fn provider_catalog_model_ids(
    provider: ApiProvider,
    identity: &str,
    base_url: &str,
) -> Vec<String> {
    let mut models = Vec::new();
    for id in crate::provider_lake::catalog_models_for_route(provider, identity, base_url) {
        // Cached IDs belong to this exact endpoint. The configured/current
        // model is appended separately so users can still select saved IDs.
        push_model_id(&mut models, picker_visible_model_id(provider, &id, false));
    }
    models
}

fn provider_scoped_model_ids_for_app(app: &App, include_current_model: bool) -> Vec<String> {
    // `include_current_model` is for completion surfaces that do not have a
    // separate custom/current-model row.
    let mut models = Vec::new();
    push_model_id(&mut models, "auto");
    for id in provider_catalog_model_ids(
        app.api_provider,
        app.provider_identity_for_persistence(),
        &app.active_route_base_url,
    ) {
        push_model_id(&mut models, &id);
    }

    if app.api_provider != ApiProvider::OpenaiCodex
        && codewhale_config::catalog::configured::validate_configured_models(&app.configured_models)
            .is_ok()
    {
        for declaration in app.configured_models.iter().filter(|declaration| {
            declaration.matches_route(
                app.provider_identity_for_persistence(),
                &app.active_route_base_url,
            )
        }) {
            if !models.contains(&declaration.id) {
                models.push(declaration.id.clone());
            }
        }
    }

    if let Some(model) = app
        .provider_models
        .get(app.provider_identity_for_persistence())
        .map(|model| model.trim())
        .filter(|model| !model.is_empty())
    {
        push_model_id(
            &mut models,
            picker_visible_model_id(app.api_provider, model, app.accepts_custom_model_ids()),
        );
    }

    if include_current_model && !app.auto_model {
        push_model_id(
            &mut models,
            picker_visible_model_id(
                app.api_provider,
                app.model.trim(),
                app.accepts_custom_model_ids(),
            ),
        );
    }

    models
}

fn push_model_id(models: &mut Vec<String>, model: &str) {
    let model = model.trim();
    if model.is_empty() {
        return;
    }
    if !models
        .iter()
        .any(|existing| existing.eq_ignore_ascii_case(model))
    {
        models.push(model.to_string());
    }
}

/// Migrate retired aliases out of first-party DeepSeek model choices. Custom
/// endpoints and aggregators own their namespaces, where `deepseek-reasoner`
/// can remain a native wire id.
fn picker_visible_model_id(
    provider: ApiProvider,
    model: &str,
    preserve_endpoint_model_ids: bool,
) -> &str {
    if !preserve_endpoint_model_ids
        && matches!(
            provider,
            ApiProvider::Deepseek | ApiProvider::DeepseekCN | ApiProvider::DeepseekAnthropic
        )
        && (model.eq_ignore_ascii_case("deepseek-chat")
            || model.eq_ignore_ascii_case("deepseek-reasoner"))
    {
        DEEPSEEK_ALIAS_REPLACEMENT
    } else {
        model
    }
}

fn provider_query_splits(query: &str) -> Vec<(&str, &str)> {
    let trimmed = query.trim();
    let mut splits = Vec::new();
    if let Some((provider, model)) = trimmed.split_once(':') {
        splits.push((provider.trim(), model.trim()));
    }
    if let Some(idx) = trimmed.find(char::is_whitespace) {
        let (provider, model) = trimmed.split_at(idx);
        splits.push((provider.trim(), model.trim()));
    }
    splits
}

fn push_model_row(
    rows: &mut Vec<ModelPickerRow>,
    id: String,
    provider: Option<ApiProvider>,
    provider_identity: Option<String>,
    hint: String,
    metadata: EffectivePickerMetadata,
    selectable: bool,
    blocked_reason: Option<String>,
) {
    if rows.iter().any(|row| {
        row.id == id
            && row.provider == provider
            && match (
                row.provider_identity.as_deref(),
                provider_identity.as_deref(),
            ) {
                (Some(left), Some(right)) => left == right,
                (None, None) => true,
                _ => false,
            }
    }) {
        return;
    }
    rows.push(ModelPickerRow {
        id,
        provider,
        provider_identity,
        hint,
        metadata,
        selectable,
        blocked_reason,
        default_rank: None,
        credentialed: false,
        not_listed: None,
    });
}

/// Compact Models.dev freshness chip for the picker chrome (#4139).
///
/// Fresh/live rows stay unmarked; stale and failed caches get an explicit
/// suffix so users know the live layer is still visible but not current.
fn catalog_freshness_title_suffix() -> &'static str {
    catalog_freshness_title_suffix_for(models_dev_live::status().freshness)
}

fn catalog_freshness_title_suffix_for(freshness: ModelsDevFreshness) -> &'static str {
    match freshness {
        ModelsDevFreshness::Stale => " · cached catalog",
        ModelsDevFreshness::Failed => " · refresh failed; catalog available",
        ModelsDevFreshness::Bundled | ModelsDevFreshness::Live => "",
    }
}

/// Cross-field search (#4141): match a query against the provider name
/// (provider key + display name), the display model name, and the wire model
/// id, mirroring `ProviderDashboardRow::matches_query` so the two pickers behave
/// consistently. `row.id` is both the model's display name and the id it is
/// sent to the provider as, so matching it covers the display model name and
/// the wire model id. The compact hint is only searched for the active
/// provider / `auto` rows, preserving the existing cross-provider behavior.
fn model_row_matches_query(
    row: &ModelPickerRow,
    query: &str,
    initial_provider: ApiProvider,
) -> bool {
    let query = query.trim().to_ascii_lowercase();
    if query.is_empty() {
        return true;
    }
    let normalized_query = normalize_picker_search_text(&query);
    let matches = |candidate: &str| {
        let candidate = candidate.to_ascii_lowercase();
        candidate.contains(&query)
            || normalize_picker_search_text(&candidate).contains(&normalized_query)
    };
    let provider_matches = row.provider.is_some_and(|provider| {
        row.provider_identity.as_deref().is_some_and(matches)
            || matches(provider.as_str())
            || matches(provider.display_name())
    });
    provider_matches
        || row.metadata.display_name.as_deref().is_some_and(matches)
        || matches(&row.id)
        || ((row.provider.is_none() || row.provider == Some(initial_provider))
            && matches(&row.hint))
}

fn normalize_picker_search_text(text: &str) -> String {
    text.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Route-identity labels for a set of rows, disambiguated where two providers
/// answer to the same display name.
///
/// `Deepseek` and `DeepseekAnthropic` are both spelled "DeepSeek", so a picker
/// listing both showed two rows of literally identical text for two genuinely
/// different endpoints. When a display name is not unique among the rows on
/// offer, the provider's own id — the `[providers.<id>]` key the user would
/// edit — supplies the discriminator, with the leading run it already shares
/// with the display name removed so the suffix is the part that differs.
fn route_labels_for_rows(rows: &[&ModelPickerRow]) -> BTreeMap<String, String> {
    let mut by_display: BTreeMap<&'static str, Vec<ApiProvider>> = BTreeMap::new();
    for provider in rows
        .iter()
        .filter_map(|row| row.provider)
        .filter(|provider| *provider != ApiProvider::Custom)
    {
        let bucket = by_display.entry(provider.display_name()).or_default();
        if !bucket.contains(&provider) {
            bucket.push(provider);
        }
    }
    let mut labels = BTreeMap::new();
    for (display, providers) in by_display {
        let ambiguous = providers.len() > 1;
        for provider in providers {
            let label = match ambiguous.then(|| route_discriminator(display, provider.as_str())) {
                Some(Some(suffix)) => format!("{display} {suffix}"),
                // The canonical route — the one whose id is just the display
                // name — keeps the bare name; provider ids are unique, so at
                // most one member of a group can land here and the labels stay
                // distinct.
                Some(None) | None => display.to_string(),
            };
            labels.insert(provider.as_str().to_string(), label);
        }
    }
    for row in rows
        .iter()
        .filter(|row| row.provider == Some(ApiProvider::Custom))
    {
        let Some(identity) = row_provider_identity(row) else {
            continue;
        };
        // Custom tables are labeled by their `[providers.<id>]` key: there
        // are no compiled display names anymore (#6289).
        labels
            .entry(identity.to_string())
            .or_insert_with(|| identity.to_string());
    }
    labels
}

/// The part of a provider id that is not already carried by its display name.
fn route_discriminator(display: &str, provider_id: &str) -> Option<String> {
    let squash = |text: &str| -> String {
        text.chars()
            .filter(|c| c.is_alphanumeric())
            .collect::<String>()
    };
    let display_key = squash(display).to_ascii_lowercase();
    let id_key = squash(provider_id).to_ascii_lowercase();
    if display_key.is_empty() || !id_key.starts_with(&display_key) {
        return None;
    }
    // Walk the raw id until the display name's alphanumerics are consumed; what
    // remains is the endpoint-specific tail (`-anthropic`, `-CN`, …).
    // Count CHARACTERS, not bytes: `display_key.len()` is a byte length, and
    // for a non-ASCII display name it exceeds the alphanumeric char count, so
    // the loop would over-consume and the discriminator would be wrong or
    // empty (2026-08-04 review).
    let display_key_chars = display_key.chars().count();
    let mut consumed = 0usize;
    let mut tail = provider_id;
    for (offset, ch) in provider_id.char_indices() {
        if consumed == display_key_chars {
            tail = &provider_id[offset..];
            break;
        }
        if ch.is_alphanumeric() {
            consumed += 1;
        }
        tail = &provider_id[offset + ch.len_utf8()..];
    }
    let tail = tail.trim_matches(|c: char| !c.is_alphanumeric());
    (!tail.is_empty()).then(|| tail.to_string())
}

/// The handful of facts that actually differ between neighbouring model rows,
/// in the order they earn their space.
///
/// Everything the old prose hint carried but that reads the same on nearly
/// every row — `tools`, `no vision`, `price unknown`, `bundled` — is dropped
/// here: a token repeated on forty rows cannot tell them apart, and it is what
/// pushed the differentiating tokens off the end of the line. Facts the
/// registry does not know are omitted rather than guessed.
/// The picker section label for a provider/model row.
///
/// Catalog families are useful grouping metadata, but they are not model names.
/// DeepSeek has published both `deepseek` and `deepseek-thinking` as family
/// values for its current V4 models, so keep its picker heading stable and
/// provider-facing rather than exposing either implementation detail.
fn catalog_family_for_identity(
    provider: ApiProvider,
    provider_identity: Option<&str>,
    model_id: &str,
) -> Option<String> {
    if provider == ApiProvider::Deepseek {
        return Some(provider.display_name().to_string());
    }
    catalog_offering_for_model_identity(provider, provider_identity, model_id)
        .and_then(|offering| offering.family)
}

fn model_row_meta_chips(row: &ModelPickerRow) -> Vec<String> {
    if row.metadata.source == Some(CatalogSource::ConfigOverride) {
        // The qualifier comes first so compact rows cannot shed it while
        // keeping an unverified declaration visible as a provider fact.
        return vec![
            "user declared (unverified)".to_string(),
            row.metadata
                .context_window
                .map(|value| format_context_window(u64::from(value)))
                .unwrap_or_else(|| "context unknown".into()),
            row.metadata
                .max_output
                .map(|value| format!("{} out", format_context_window(u64::from(value))))
                .unwrap_or_else(|| "output unknown".into()),
            match &row.metadata.pricing {
                PickerPricing::Known(price) => format!("estimate {price}"),
                _ => "price unknown".into(),
            },
            if row.metadata.reasoning_unknown {
                "reasoning unknown"
            } else if row.metadata.reasoning {
                "reasoning"
            } else {
                "no reasoning"
            }
            .into(),
            match row.metadata.tool_calls {
                Some(true) => "tools declared",
                Some(false) => "no tools",
                None => "tools unknown",
            }
            .into(),
            match row.metadata.vision {
                SupportState::Supported => "vision declared",
                SupportState::Unsupported => "text only",
                SupportState::Unknown => "vision unknown",
            }
            .into(),
        ];
    }
    let mut chips = Vec::new();
    if let Some(context_window) = row.metadata.context_window {
        chips.push(format_context_window(u64::from(context_window)));
    }
    // The reasoning stance is the most decision-relevant fact for a coding
    // harness, so it sits before the limits/modality chips — the chip budget
    // sheds from the tail, and a squeezed row must never lose the stance.
    chips.push(
        if row.metadata.reasoning {
            "reasoning"
        } else {
            "no reasoning"
        }
        .to_string(),
    );
    // #5239/#5441: an unverified window still drives budgets, but the chip
    // must not lend it a verified reading. Honesty rides as its own chip
    // *after* the stance so a squeezed row sheds the marker before it ever
    // sheds the stance; the full "(unverified)" prose lives in the hint
    // line, which always renders it.
    if row.metadata.context_window_unverified {
        chips.push("unverified ctx".to_string());
    }
    if let Some(max_output) = row.metadata.max_output {
        // #5440: an assumed floor is shown as such, never as a documented
        // ceiling.
        let suffix = if row.metadata.max_output_unverified {
            " (assumed floor)"
        } else {
            ""
        };
        chips.push(format!(
            "{} out{suffix}",
            format_context_window(u64::from(max_output))
        ));
    }
    // Modality and tool facts are shown only when the catalog genuinely knows
    // them — an unknown is never rendered as a claim.
    match row.metadata.vision {
        SupportState::Supported => chips.push("vision".to_string()),
        SupportState::Unsupported => chips.push("text only".to_string()),
        SupportState::Unknown => {}
    }
    if let Some(tool_calls) = row.metadata.tool_calls {
        chips.push(if tool_calls {
            "tools".to_string()
        } else {
            "no tools".to_string()
        });
    }
    if let Some(reason) = row.blocked_reason.as_deref() {
        chips.push(reason.to_string());
    }
    chips
}

/// Join metadata chips, dropping the lowest-priority ones until the result
/// fits. Truncating mid-chip would render a half-word fact, so whole chips are
/// shed instead.
fn fit_meta_chips(chips: &[String], width: usize) -> String {
    for take in (1..=chips.len()).rev() {
        let joined = chips[..take].join(" · ");
        if unicode_width::UnicodeWidthStr::width(joined.as_str()) <= width {
            return joined;
        }
    }
    // A single chip that still does not fit is prose (an `auto` explanation or
    // an effort description) rather than a fact token, so it is truncated
    // instead of dropped — but only when the column can hold something worth
    // reading.
    match chips.first() {
        Some(first) if width >= MIN_META_WIDTH => fit_text(first, width),
        _ => String::new(),
    }
}

/// Whether a model row shows in the active catalog view (#3830 / #4115).
fn model_row_visible_in_view(row: &ModelPickerRow, view: ModelListView) -> bool {
    match view {
        ModelListView::Configured => model_row_visible_by_default(row),
        ModelListView::Catalog => true,
        ModelListView::New
        | ModelListView::Coding
        | ModelListView::Cheap
        | ModelListView::LongContext => {
            // Discoverability views browse the full lake but hide the synthetic
            // `auto` row — it is not a catalog offering.
            row.provider.is_some() || row.id != "auto"
        }
    }
}

/// Whether a model row shows up without the user typing a search query:
/// `auto` and every row placed in a default-view section (#6533). The rest of
/// every roster, the active one included, is one search or ⇧A away.
fn model_row_visible_by_default(row: &ModelPickerRow) -> bool {
    row.provider.is_none() || row.default_rank.is_some()
}

fn model_row_matches_route(row: &ModelPickerRow, provider: ApiProvider, identity: &str) -> bool {
    row.provider.is_none()
        || (row.provider == Some(provider) && row_provider_identity(row) == Some(identity))
}

fn sort_model_rows_for_view<'a, T>(
    rows: &mut [T],
    model_row: impl Fn(&T) -> &'a ModelPickerRow,
    view: ModelListView,
    pins: &[PinnedModel],
) {
    use std::cmp::Reverse;
    // Same route match as marking and labelling (`pin_names_row`), so a row
    // never sorts into the pinned block without carrying its pin.
    let pin_rank = |row: &ModelPickerRow| {
        pins.iter()
            .position(|pin| pin_names_row(pin, row))
            .unwrap_or(usize::MAX)
    };
    match view {
        // Sections in order; within one, a row a fresh roster no longer lists
        // goes last, then the section's own rank.
        ModelListView::Configured => rows.sort_by_cached_key(|item| {
            let row = model_row(item);
            (
                row.default_rank.map(|(section, _)| section),
                row.not_listed.is_some(),
                row.default_rank.map(|(_, rank)| rank),
                row_group_key(row),
                Reverse(model_version_key(&row.id)),
                row.id.clone(),
            )
        }),
        ModelListView::Catalog => rows.sort_by_cached_key(|item| {
            let row = model_row(item);
            (
                pin_rank(row),
                row_group_key(row),
                Reverse(model_version_key(&row.id)),
                row.id.clone(),
            )
        }),
        ModelListView::New => rows.sort_by_cached_key(|item| {
            let row = model_row(item);
            (Reverse(offering_fetched_at(row)), row.id.clone())
        }),
        ModelListView::Coding => rows.sort_by_cached_key(|item| {
            let row = model_row(item);
            (Reverse(coding_score(row)), row.id.clone())
        }),
        ModelListView::Cheap => {
            // Catalog lookup/pricing parsing happens once per row. Unknown
            // prices stay last; f64 retains its existing partial-order behavior.
            let prices: BTreeMap<_, _> = rows
                .iter()
                .map(|item| {
                    let row = model_row(item);
                    (
                        (
                            row_provider_identity(row).unwrap_or_default().to_string(),
                            row.id.clone(),
                        ),
                        input_price_per_million(row),
                    )
                })
                .collect();
            rows.sort_by(|left, right| {
                let left = model_row(left);
                let right = model_row(right);
                let price = |row: &ModelPickerRow| {
                    prices[&(
                        row_provider_identity(row).unwrap_or_default().to_string(),
                        row.id.clone(),
                    )]
                };
                match (price(left), price(right)) {
                    (Some(l), Some(r)) => l.partial_cmp(&r).unwrap_or(std::cmp::Ordering::Equal),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                }
                .then_with(|| left.id.cmp(&right.id))
            });
        }
        ModelListView::LongContext => rows.sort_by_cached_key(|item| {
            let row = model_row(item);
            (Reverse(context_tokens(row)), row.id.clone())
        }),
    }
}

fn sort_model_indices(indices: &mut [usize], rows: &[ModelPickerRow], sort: ModelSort) {
    // Precompute owned text once, keeping navigation independent of catalog size.
    let keys: BTreeMap<_, _> = indices
        .iter()
        .map(|index| {
            let row = &rows[*index];
            (
                *index,
                (
                    row.id.to_ascii_lowercase(),
                    row_provider_identity(row)
                        .unwrap_or_default()
                        .to_ascii_lowercase(),
                ),
            )
        })
        .collect();
    indices.sort_by(|left, right| {
        let a = &rows[*left];
        let b = &rows[*right];
        let order = match sort.column {
            ModelSortColumn::Model => keys[left].0.cmp(&keys[right].0),
            ModelSortColumn::Provider => keys[left].1.cmp(&keys[right].1),
            ModelSortColumn::Context => a.metadata.context_window.cmp(&b.metadata.context_window),
        };
        let order = if sort.descending {
            order.reverse()
        } else {
            order
        };
        // Auto remains reachable at the top; missing context stays last in
        // either direction rather than pretending to be a zero-sized model.
        a.provider
            .is_some()
            .cmp(&b.provider.is_some())
            .then_with(|| {
                if sort.column == ModelSortColumn::Context {
                    a.metadata
                        .context_window
                        .is_none()
                        .cmp(&b.metadata.context_window.is_none())
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .then(order)
            .then_with(|| keys[left].cmp(&keys[right]))
    });
}

/// Stable grouping key so a provider's families render as one contiguous
/// block each, which is what the family-header logic already assumes when it
/// only compares against the previous row.
fn row_group_key(row: &ModelPickerRow) -> (String, String) {
    let provider = row_provider_identity(row)
        .map(str::to_ascii_lowercase)
        .or_else(|| {
            row.provider
                .map(|provider| provider.as_str().to_ascii_lowercase())
        })
        .unwrap_or_default();
    let family = row
        .provider
        .and_then(|provider| {
            catalog_family_for_identity(provider, row.provider_identity.as_deref(), &row.id)
        })
        .unwrap_or_default()
        .to_ascii_lowercase();
    (provider, family)
}

/// Version ordinal for "newest first" inside a family.
///
/// Model ids carry their version as dotted or dashed numbers (`GLM-5.3`,
/// `deepseek-v4-pro`, `gpt-5.6-terra`), so the comparison is on the numeric
/// components in order, not on the string — otherwise `GLM-5.10` would sort
/// below `GLM-5.2`. Ids with no digits compare equal and fall through to the
/// alphabetical tiebreak.
fn model_version_key(id: &str) -> Vec<u32> {
    let mut parts = Vec::new();
    let mut current: Option<u32> = None;
    for ch in id.chars() {
        if let Some(digit) = ch.to_digit(10) {
            current = Some(
                current
                    .unwrap_or(0)
                    .saturating_mul(10)
                    .saturating_add(digit),
            );
        } else if let Some(value) = current.take() {
            parts.push(value);
        }
    }
    if let Some(value) = current {
        parts.push(value);
    }
    parts
}

fn row_provider_identity(row: &ModelPickerRow) -> Option<&str> {
    row.provider_identity.as_deref().or_else(|| {
        row.provider
            .filter(|provider| *provider != ApiProvider::Custom)
            .map(ApiProvider::as_str)
    })
}

fn offering_for_row(row: &ModelPickerRow) -> Option<codewhale_config::catalog::CatalogOffering> {
    let provider = row.provider?;
    catalog_offering_for_model_identity(provider, row.provider_identity.as_deref(), &row.id)
}

fn offering_fetched_at(row: &ModelPickerRow) -> u64 {
    match offering_for_row(row).map(|o| o.source) {
        Some(
            CatalogSource::Live { fetched_at, .. } | CatalogSource::CloudFacts { fetched_at, .. },
        ) => fetched_at,
        _ => 0,
    }
}

fn context_tokens(row: &ModelPickerRow) -> u64 {
    row.metadata.context_window.map(u64::from).unwrap_or(0)
}

fn input_price_per_million(row: &ModelPickerRow) -> Option<f64> {
    if matches!(row.metadata.source, Some(CatalogSource::ConfigOverride)) {
        return row
            .metadata
            .declared_input_price
            .as_ref()
            .and_then(|price| price.parse().ok());
    }
    if matches!(row.metadata.pricing, PickerPricing::Unavailable) {
        return None;
    }
    offering_for_row(row)
        .and_then(|offering| OfferingPricing::from_catalog_offering(&offering))
        .and_then(|pricing| pricing.input_per_million)
}

fn coding_score(row: &ModelPickerRow) -> u32 {
    let mut score = 0_u32;
    if let Some(offering) = offering_for_row(row) {
        let text_ok = offering.modalities.as_ref().is_none_or(|modalities| {
            modalities.output.is_empty()
                || modalities
                    .output
                    .iter()
                    .any(|m| m.eq_ignore_ascii_case("text"))
        });
        if text_ok {
            score += 40;
        }
    }
    if row.metadata.tool_calls == Some(true) {
        score += 40;
    }
    if row.metadata.reasoning {
        score += 10;
    }
    if row.metadata.context_window.unwrap_or(0) >= 100_000 {
        score += 10;
    }
    score
}

fn effective_picker_metadata(
    config: &Config,
    provider: Option<ApiProvider>,
    id: &str,
) -> EffectivePickerMetadata {
    effective_picker_metadata_for_identity(config, provider, None, id)
}

fn effective_picker_metadata_for_identity(
    config: &Config,
    provider: Option<ApiProvider>,
    provider_identity: Option<&str>,
    id: &str,
) -> EffectivePickerMetadata {
    effective_picker_metadata_with_codex(config, provider, provider_identity, id, None)
}

fn effective_picker_metadata_with_codex(
    config: &Config,
    provider: Option<ApiProvider>,
    provider_identity: Option<&str>,
    id: &str,
    codex_metadata: Option<&CodexModelMetadata>,
) -> EffectivePickerMetadata {
    let offering = provider.and_then(|provider| {
        let identity = provider_identity
            .map(str::to_string)
            .unwrap_or_else(|| config.provider_identity_for(provider));
        let base_url = config.base_url_for_route_identity(provider, &identity);
        crate::provider_lake::configured_catalog_offering_for_route(
            config, provider, &identity, &base_url, id,
        )
    });
    let card = offering.as_ref().map(ModelReferenceCard::from_offering);
    let registry = model_registry::lookup(id);

    let Some(provider) = provider else {
        return EffectivePickerMetadata {
            context_window: registry.as_ref().and_then(|meta| meta.context_window),
            context_window_unverified: false,
            max_output: registry.as_ref().and_then(|meta| meta.max_output),
            max_output_unverified: false,
            tool_calls: None,
            reasoning: registry
                .as_ref()
                .is_some_and(|meta| meta.supports_reasoning),
            vision: SupportState::Unknown,
            pricing: if crate::pricing::has_pricing_for_model(id) {
                PickerPricing::Known("priced".to_string())
            } else {
                PickerPricing::Unknown
            },
            display_name: None,
            reasoning_unknown: false,
            declared_input_price: None,
            source: None,
        };
    };

    let identity = provider_identity
        .map(str::to_string)
        .unwrap_or_else(|| config.provider_identity_for(provider));
    let context_override = if provider == ApiProvider::Custom {
        config
            .providers
            .as_ref()
            .and_then(|providers| providers.custom_provider_config(&identity))
            .and_then(|entry| entry.context_window)
            .filter(|window| *window > 0)
    } else {
        config.context_window_for_provider_config(provider)
    };
    let base_url = config.base_url_for_route_identity(provider, &identity);
    if let Some(declared) =
        crate::provider_lake::configured_model_for_route(config, provider, &identity, &base_url, id)
    {
        return EffectivePickerMetadata {
            display_name: declared.display_name.clone(),
            declared_input_price: declared
                .cost
                .as_ref()
                .and_then(|cost| cost.input)
                .map(|price| price.to_string()),
            context_window: context_override.or_else(|| {
                declared
                    .limit
                    .as_ref()
                    .and_then(|limit| limit.context)
                    .and_then(|value| u32::try_from(value).ok())
            }),
            max_output: declared
                .limit
                .as_ref()
                .and_then(|limit| limit.output)
                .and_then(|value| u32::try_from(value).ok()),
            tool_calls: declared.tool_call,
            reasoning: declared.reasoning.unwrap_or(false),
            reasoning_unknown: declared.reasoning.is_none(),
            vision: codewhale_config::models_dev::image_input_support(declared.modalities.as_ref()),
            pricing: card
                .as_ref()
                .filter(|card| card.price_label() != "unknown")
                .map_or(PickerPricing::Unknown, |card| {
                    PickerPricing::Known(card.price_label())
                }),
            source: Some(CatalogSource::ConfigOverride),
            ..EffectivePickerMetadata::default()
        };
    }
    if offering.is_none()
        && provider != ApiProvider::OpenaiCodex
        && provider.kind().is_none_or(|kind| {
            codewhale_config::provider_preserves_custom_base_url_model(kind, &base_url)
        })
    {
        return EffectivePickerMetadata {
            context_window: context_override,
            ..EffectivePickerMetadata::default()
        };
    }
    let overrides = CapabilityOverride {
        context_window: context_override,
        ..CapabilityOverride::default()
    };
    let profile = offering.as_ref().map_or_else(
        || resolved_capability_profile_with_overrides(provider, id, overrides.clone()),
        |offering| {
            let route_offering = offering.to_offering();
            resolved_capability_profile_for_route_with_overrides(
                provider,
                id,
                route_offering.capabilities,
                route_offering.limits,
                overrides.clone(),
            )
        },
    );
    let card_context = card
        .as_ref()
        .and_then(|card| card.context_window)
        .map(|tokens| tokens.min(u64::from(u32::MAX)) as u32);
    let preserves_unknown_limits = offering.is_some()
        || (provider == ApiProvider::Together
            && id.eq_ignore_ascii_case(crate::config::TOGETHER_INKLING_MODEL));
    let context_window = if context_override.is_some() {
        profile.context_window
    } else if provider == ApiProvider::OpenaiCodex {
        codex_metadata.and_then(|metadata| metadata.context_window)
    } else if preserves_unknown_limits {
        card_context
    } else {
        profile.context_window
    };
    let card_output = card
        .as_ref()
        .and_then(|card| card.max_output)
        .map(|tokens| tokens.min(u64::from(u32::MAX)) as u32);
    // The Codex cache does not publish a route-owned output ceiling. The
    // profile's current value is inherited from the same-id OpenAI API model,
    // so omitting it is more truthful than claiming that API limit for OAuth.
    let max_output = if provider == ApiProvider::OpenaiCodex {
        None
    } else if preserves_unknown_limits {
        card_output
    } else {
        profile.max_output
    };
    let profile_tool_calls = match profile.native_tool_calls {
        SupportState::Supported => Some(true),
        SupportState::Unsupported => Some(false),
        SupportState::Unknown => None,
    };
    let tool_calls = if provider == ApiProvider::OpenaiCodex {
        codex_metadata.and(profile_tool_calls)
    } else {
        offering
            .as_ref()
            .and_then(|offering| offering.tool_call)
            .or(profile_tool_calls)
    };
    let reasoning = if provider == ApiProvider::OpenaiCodex {
        codex_metadata
            .map(|metadata| {
                metadata
                    .reasoning
                    .unwrap_or_else(|| profile.supports_reasoning())
            })
            .unwrap_or(false)
    } else {
        offering
            .as_ref()
            .and_then(|offering| offering.reasoning)
            .unwrap_or_else(|| profile.supports_reasoning())
    };
    let vision = profile.image_input;
    let card_price = card.as_ref().and_then(|card| {
        let label = card.price_label();
        (label != "unknown").then_some(label)
    });
    let pricing = if provider == ApiProvider::OpenaiCodex {
        PickerPricing::Unavailable
    } else if let Some(label) = card_price {
        PickerPricing::Known(label)
    } else if crate::pricing::has_pricing_for_provider(provider, id) {
        PickerPricing::Known("priced".to_string())
    } else {
        PickerPricing::Unknown
    };

    // Honesty rungs (#5239, #5440, #5441). A window that reached the picker
    // only through the legacy provider fallback — no offering, no catalog
    // row, no Codex roster, no operator override — is a guess (possibly an
    // `_Nk` name-suffix parse), and an unknown Anthropic-family model's
    // output ceiling is an assumed floor. Both still drive budgets; both
    // must say what they are instead of borrowing a verified label.
    let context_window_unverified = context_window.is_some()
        && context_override.is_none()
        && provider != ApiProvider::OpenaiCodex
        && !preserves_unknown_limits
        && codewhale_models::model_catalog::resolved_context_window(id).is_none();
    let max_output_unverified = max_output.is_some()
        && matches!(
            provider,
            ApiProvider::Anthropic | ApiProvider::MinimaxAnthropic | ApiProvider::Openmodel
        )
        && !preserves_unknown_limits
        && codewhale_models::max_output_tokens_for_model(id).is_none();

    EffectivePickerMetadata {
        context_window,
        context_window_unverified,
        max_output,
        max_output_unverified,
        tool_calls,
        reasoning,
        vision,
        pricing,
        display_name: None,
        reasoning_unknown: false,
        declared_input_price: None,
        source: card.map(|card| card.source),
    }
}

fn render_picker_model_hint(
    id: &str,
    provider: Option<ApiProvider>,
    metadata: &EffectivePickerMetadata,
    codex_freshness: Option<CodexModelCacheFreshness>,
    provider_catalog_receipt: Option<&(CatalogStatus, bool)>,
) -> String {
    debug_assert_ne!(id, "auto", "Auto rows use the context-aware picker hint");

    let mut parts = Vec::new();

    // `k3` and `kimi-k3` are the same underlying model on two different
    // products, so bare ids read as a confusing duplicate. Name the route:
    // bare `k3` is the Kimi Code membership route (validated pairing with
    // the coding endpoint, #4687), `kimi-k3` is the direct open platform.
    if provider == Some(ApiProvider::Moonshot) {
        match id.trim().to_ascii_lowercase().as_str() {
            "k3" => parts.push("Kimi Code plan route".to_string()),
            "kimi-k3" | "moonshotai/kimi-k3" => parts.push("Moonshot direct route".to_string()),
            _ => {}
        }
    }

    if let Some(context_window) = metadata.context_window {
        // The ChatGPT/Codex OAuth roster reports account-scoped windows (e.g.
        // 272K for gpt-5.x) that differ from the API route's limits by
        // deliberate policy. Label the value as route-scoped so it reads as a
        // route fact, not a wrong generic model limit (TUI-DOG-016).
        if provider == Some(ApiProvider::OpenaiCodex) {
            parts.push(format!(
                "{} ctx · ChatGPT route",
                format_context_window(u64::from(context_window))
            ));
        } else if provider == Some(ApiProvider::Moonshot)
            && id.trim().eq_ignore_ascii_case("k3")
            && context_window == codewhale_models::KIMI_CODE_K3_CONTEXT_WINDOW_TOKENS
        {
            // The membership route's real window is plan-tier dependent
            // (256K on lower tiers, up to 1M on higher ones); this default
            // is the safe floor, raisable via the provider's
            // `context_window` setting when the plan includes 1M.
            parts.push(format!(
                "{} ctx (plan floor; raise via context_window)",
                format_context_window(u64::from(context_window))
            ));
        } else {
            let suffix = if metadata.context_window_unverified {
                " (unverified)"
            } else {
                ""
            };
            parts.push(format!(
                "{} ctx{}",
                format_context_window(u64::from(context_window)),
                suffix
            ));
        }
    }

    if let Some(max_output) = metadata.max_output {
        let suffix = if metadata.max_output_unverified {
            " (assumed floor)"
        } else {
            ""
        };
        parts.push(format!(
            "{} out{}",
            format_context_window(u64::from(max_output)),
            suffix
        ));
    }

    match metadata.tool_calls {
        Some(true) => parts.push("tools".to_string()),
        Some(false) => parts.push("no tools".to_string()),
        None => {}
    }

    if metadata.reasoning {
        parts.push("reasoning".to_string());
    }

    match metadata.vision {
        SupportState::Supported => parts.push("vision".to_string()),
        SupportState::Unsupported => parts.push("no vision".to_string()),
        SupportState::Unknown => {}
    }

    match &metadata.pricing {
        PickerPricing::Unavailable => {}
        PickerPricing::Known(label) => parts.push(label.clone()),
        PickerPricing::Unknown => parts.push("price unknown".to_string()),
    }
    let provider_live_source = matches!(metadata.source.as_ref(), Some(CatalogSource::Live { .. }));
    match metadata.source.as_ref() {
        Some(CatalogSource::Live { .. }) => {
            parts.push(provider_catalog_source_label(provider_catalog_receipt))
        }
        Some(CatalogSource::ModelsDevLive { .. }) => parts.push("live".to_string()),
        Some(CatalogSource::Bundled | CatalogSource::CodewhaleBundled { .. }) => {
            parts.push("bundled".to_string())
        }
        Some(CatalogSource::CloudFacts { .. }) => parts.push("signed facts".to_string()),
        Some(CatalogSource::ConfigOverride | CatalogSource::UserOverride) => {
            parts.push("user declared (unverified)".to_string())
        }
        None => {}
    }
    if !provider_live_source
        && let Some((CatalogStatus::Failed { reason }, _)) = provider_catalog_receipt
    {
        parts.push(format!(
            "refresh failed ({})",
            catalog_refresh_error_label(*reason)
        ));
    }
    if provider == Some(ApiProvider::OpenaiCodex) {
        parts.push(match codex_freshness {
            Some(freshness) => freshness.picker_label().to_string(),
            None => "custom · OAuth roster unconfirmed".to_string(),
        });
    }

    if parts.is_empty() {
        "provider model".to_string()
    } else {
        parts.join(" · ")
    }
}

fn provider_catalog_source_label(receipt: Option<&(CatalogStatus, bool)>) -> String {
    let Some((status, endpoint_matches)) = receipt else {
        return "catalog freshness unknown".to_string();
    };
    if !endpoint_matches {
        return "catalog from different endpoint".to_string();
    }
    match status {
        CatalogStatus::Fresh => "live".to_string(),
        CatalogStatus::Stale { age_secs } => {
            let age_hours = age_secs.saturating_add(3_599) / 3_600;
            format!("stale catalog ({age_hours}h)")
        }
        CatalogStatus::Failed { reason } => {
            format!("refresh failed ({})", catalog_refresh_error_label(*reason))
        }
        CatalogStatus::Unknown => "catalog freshness unknown".to_string(),
    }
}

fn catalog_refresh_error_label(error: CatalogRefreshError) -> &'static str {
    match error {
        CatalogRefreshError::Unauthorized => "unauthorized",
        CatalogRefreshError::Forbidden => "forbidden",
        CatalogRefreshError::NotFound => "not found",
        CatalogRefreshError::RateLimited => "rate limited",
        CatalogRefreshError::InvalidResponse => "invalid response",
        CatalogRefreshError::EmptyList => "empty list",
        CatalogRefreshError::Network => "network error",
    }
}

impl ModelPickerView {
    /// Rebuild model rows from a fresh app/config snapshot (readiness + catalog).
    pub fn re_resolve_from_app(&mut self, app: &App, config: &Config) {
        self.theme = app.ui_theme;
        let selected = self
            .visible_model_rows()
            .get(self.selected_model_idx)
            .map(|row| {
                (
                    row_provider_identity(row).map(str::to_string),
                    row.id.clone(),
                )
            });
        self.provider_health = app.provider_health.clone();
        self.route_config = config.clone();
        self.pinned_models = picker_pins_for_app(app);
        self.model_rows = picker_model_rows_for_app(app, config);
        self.apply_fleet_route_rows(app, config);
        *self.projection.get_mut() = None;
        self.last_mouse_selected = None;
        self.configured_providers = configured_providers(config, self.initial_provider)
            .into_iter()
            .filter(|provider| *provider != self.initial_provider)
            .collect();
        // Re-anchor to the same exact provider/model after pin sorting changes;
        // preserving only the numeric index can select a different model.
        let reanchored = selected.and_then(|(provider, model)| {
            self.visible_model_rows().iter().position(|row| {
                row.id == model && row_provider_identity(row).map(str::to_owned) == provider
            })
        });
        if let Some(position) = reanchored {
            self.selected_model_idx = position;
            return;
        }
        // Keep selection stable when the row still exists.
        let visible_len = self.visible_model_rows().len();
        if self.selected_model_idx >= visible_len + usize::from(self.show_custom_model_row) {
            self.selected_model_idx = visible_len.saturating_sub(1);
        }
    }
}

impl ModelPickerView {
    /// Exact route of the highlighted catalog row: the target of ⇧P, ⇧F and
    /// Alt+↑↓. `None` on provider-less rows (`auto`) and the custom row.
    fn highlighted_route(&self) -> Option<(ApiProvider, Option<String>, String)> {
        let rows = self.visible_model_rows();
        let row = rows.get(self.selected_model_idx)?;
        Some((row.provider?, row.provider_identity.clone(), row.id.clone()))
    }

    fn emit_pin_move(&self, delta: isize) -> ViewAction {
        let Some((provider, provider_id, model)) = self.highlighted_route() else {
            return ViewAction::None;
        };
        ViewAction::Emit(ViewEvent::ModelPickerMovePin {
            provider,
            provider_id,
            model,
            delta,
        })
    }

    /// Show an action receipt inside the picker (see `notice`).
    pub fn set_notice(&mut self, text: String, level: StatusToastLevel) {
        self.notice = Some((text, level));
    }
}

impl ModalView for ModelPickerView {
    fn kind(&self) -> ModalKind {
        ModalKind::ModelPicker
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn handle_key(&mut self, key: KeyEvent) -> ViewAction {
        self.last_mouse_selected = None;
        self.hovered_row = None;
        self.notice = None;
        // Movement keys come from the shared vocabulary (#6290); the match
        // below owns only the picker's own verbs. The live filter means the
        // typing-safe set — no letter aliases to eat the query.
        if let Some(motion) = crate::tui::list_nav::motion_while_typing(&key)
            && self.apply_motion(motion)
        {
            return ViewAction::None;
        }
        match key.code {
            KeyCode::Char('s' | 'S') if key.modifiers == KeyModifiers::CONTROL => {
                self.cycle_sort();
                ViewAction::None
            }
            // Esc carries the browsing context out so the next open can
            // restore it (#4109 picker memory).
            KeyCode::Esc if !self.query.is_empty() => {
                self.update_query(String::new());
                ViewAction::None
            }
            KeyCode::Esc if self.purpose != ModelPickerPurpose::Session => {
                let editor_id = match self.purpose {
                    ModelPickerPurpose::FleetRoute { editor_id, .. }
                    | ModelPickerPurpose::FleetProfileRoute { editor_id, .. } => editor_id,
                    ModelPickerPurpose::Session => unreachable!(),
                };
                ViewAction::EmitAndClose(ViewEvent::FleetAssignmentPickerDismissed { editor_id })
            }
            KeyCode::Esc => ViewAction::EmitAndClose(ViewEvent::ModelPickerDismissed {
                catalog_view: self.view.browses_all_providers(),
                view: self.view.memory_name().to_string(),
                selected_row_id: {
                    let rows = self.visible_model_rows();
                    rows.get(self.selected_model_idx).map(|row| row.id.clone())
                },
            }),
            KeyCode::Enter if self.model_row_count() == 0 => ViewAction::None,
            KeyCode::Enter if !self.selected_model_is_selectable() => {
                // Never silently ignore Enter on locked models — surface the
                // readiness reason and offer provider setup.
                self.explain_unselectable_selection()
            }
            KeyCode::Enter => ViewAction::EmitAndClose(self.build_apply_event(false)),
            // Shift+D makes the visible provider/model pair the startup
            // default. Plain Enter deliberately stays session-local, so a
            // one-off route comparison cannot silently change the next launch.
            KeyCode::Char(ch)
                if key.modifiers.contains(KeyModifiers::SHIFT)
                    && self.query.is_empty()
                    && ch.eq_ignore_ascii_case(&'d')
                    && self.selected_model_is_selectable() =>
            {
                ViewAction::EmitAndClose(self.build_apply_event(true))
            }
            // Only an empty query explains a locked row. While searching,
            // ⇧D is query text: it used to open provider auth for a row that
            // was not locked at all (#6500).
            KeyCode::Char(ch)
                if key.modifiers.contains(KeyModifiers::SHIFT)
                    && self.query.is_empty()
                    && ch.eq_ignore_ascii_case(&'d') =>
            {
                self.explain_unselectable_selection()
            }
            // ⇧P / ⇧F act on the highlighted row, searching or not: a search
            // is how a model is found, and the verbs used to become query
            // text the moment one was typed (#6500). The filter ignores case,
            // so only a custom id being typed (no catalog row highlighted)
            // still receives the capital letter. Some terminals report the
            // chord as lowercase + SHIFT, hence `eq_ignore_ascii_case`.
            KeyCode::Char(ch)
                if key.modifiers == KeyModifiers::SHIFT
                    && ch.eq_ignore_ascii_case(&'p')
                    && (self.query.is_empty() || self.highlighted_route().is_some()) =>
            {
                let Some((provider, provider_id, model)) = self.highlighted_route() else {
                    return ViewAction::None;
                };
                ViewAction::Emit(ViewEvent::ModelPickerTogglePin {
                    provider,
                    provider_id,
                    model,
                })
            }
            KeyCode::Char(ch)
                if key.modifiers == KeyModifiers::SHIFT
                    && ch.eq_ignore_ascii_case(&'f')
                    && self.purpose == ModelPickerPurpose::Session
                    && (self.query.is_empty() || self.highlighted_route().is_some()) =>
            {
                let Some((provider, provider_id, model)) = self.highlighted_route() else {
                    return ViewAction::None;
                };
                ViewAction::Emit(ViewEvent::ModelPickerToggleFleet {
                    provider,
                    provider_id,
                    model,
                })
            }
            KeyCode::Up if key.modifiers.contains(KeyModifiers::ALT) && self.query.is_empty() => {
                self.emit_pin_move(-1)
            }
            KeyCode::Down if key.modifiers.contains(KeyModifiers::ALT) && self.query.is_empty() => {
                self.emit_pin_move(1)
            }
            // Cycle catalog views (#4115) without shadowing a typed provider
            // name such as `anthropic` or `azure`.
            KeyCode::Char('A') if key.modifiers == KeyModifiers::SHIFT && self.query.is_empty() => {
                self.toggle_view();
                ViewAction::None
            }
            KeyCode::Char(ch)
                if self.focus == Pane::Model
                    && !key
                        .modifiers
                        .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                let mut query = self.query.clone();
                query.push(ch);
                self.update_query(query);
                ViewAction::None
            }
            KeyCode::Backspace if self.focus == Pane::Model && !self.query.is_empty() => {
                let mut query = self.query.clone();
                query.pop();
                self.update_query(query);
                ViewAction::None
            }
            // Explicit readiness + catalog refresh (safe, non-destructive).
            // Plain `r` remains a route-search character.
            KeyCode::Char('r') | KeyCode::Char('R')
                if key.modifiers == crossterm::event::KeyModifiers::CONTROL =>
            {
                ViewAction::Emit(ViewEvent::ModelPickerRefresh)
            }
            _ => ViewAction::None,
        }
    }

    fn handle_mouse(&mut self, mouse: MouseEvent) -> ViewAction {
        let over_catalog = self
            .catalog_action_hitbox
            .borrow()
            .is_some_and(|rect| rect.contains((mouse.column, mouse.row).into()));
        if mouse.kind == MouseEventKind::Moved {
            self.catalog_action_hovered = over_catalog;
        }
        if over_catalog && mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            self.toggle_view();
            self.catalog_action_hovered = false;
            self.last_mouse_selected = None;
            return ViewAction::None;
        }
        match mouse.kind {
            MouseEventKind::Moved => {
                self.hovered_row =
                    self.row_hitboxes
                        .borrow()
                        .iter()
                        .find_map(|(rect, pane, idx)| {
                            rect.contains((mouse.column, mouse.row).into())
                                .then_some((*pane, *idx))
                        });
                ViewAction::None
            }
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                self.last_mouse_selected = None;
                self.hovered_row = None;
                let pane = self.pane_hitboxes.borrow().iter().find_map(|(rect, pane)| {
                    rect.contains(ratatui::layout::Position::new(mouse.column, mouse.row))
                        .then_some(*pane)
                });
                let Some(pane) = pane else {
                    return ViewAction::None;
                };
                self.focus = pane;
                if mouse.kind == MouseEventKind::ScrollUp {
                    self.move_up();
                } else {
                    self.move_down();
                }
                ViewAction::None
            }
            MouseEventKind::Down(MouseButton::Left) => {
                let column = self
                    .column_hitboxes
                    .borrow()
                    .iter()
                    .find_map(|(rect, column)| {
                        rect.contains(ratatui::layout::Position::new(mouse.column, mouse.row))
                            .then_some(*column)
                    });
                if let Some(column) = column {
                    // Sorting acts on the Model pane, so the header click
                    // moves focus there too — otherwise the next keystroke or
                    // wheel event edits the pane that previously had focus.
                    self.focus = Pane::Model;
                    self.sort_column(column);
                    return ViewAction::None;
                }
                let clicked = self
                    .row_hitboxes
                    .borrow()
                    .iter()
                    .find_map(|(rect, pane, idx)| {
                        rect.contains(ratatui::layout::Position::new(mouse.column, mouse.row))
                            .then_some((*pane, *idx))
                    });
                let Some((pane, idx)) = clicked else {
                    return ViewAction::None;
                };
                let apply = self.last_mouse_selected == Some((pane, idx))
                    && self.focus == pane
                    && match pane {
                        Pane::Model => self.selected_model_idx == idx,
                        Pane::Effort => self.selected_effort_idx == idx,
                    };
                self.focus = pane;
                match pane {
                    Pane::Model => {
                        self.selected_model_idx = idx.min(self.model_row_count().saturating_sub(1));
                        self.select_effort_for_current_model();
                    }
                    Pane::Effort => {
                        self.selected_effort_idx =
                            idx.min(self.current_efforts().len().saturating_sub(1));
                        self.selected_effort_request = self.resolved_effort();
                    }
                }
                self.last_mouse_selected = Some((pane, idx));
                if apply && self.selected_model_is_selectable() {
                    ViewAction::EmitAndClose(self.build_apply_event(false))
                } else if apply {
                    self.explain_unselectable_selection()
                } else {
                    ViewAction::None
                }
            }
            _ => ViewAction::None,
        }
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.render_route(area, buf);
    }
}

impl ModelPickerView {
    fn render_route(&self, area: Rect, buf: &mut Buffer) {
        self.row_hitboxes.borrow_mut().clear();
        self.column_hitboxes.borrow_mut().clear();
        self.pane_hitboxes.borrow_mut().clear();
        *self.catalog_action_hitbox.borrow_mut() = None;
        let view_action: std::borrow::Cow<'static, str> = match self.view {
            // The default view is a short list; say how much sits behind it.
            ModelListView::Configured => format!(
                "{} ({})",
                tr(self.locale, MessageId::RouteBrowseCatalog),
                self.model_rows
                    .iter()
                    .filter(|row| row.provider.is_some())
                    .count()
            )
            .into(),
            other => other.next().title_label().into(),
        };
        let title = self
            .assignment_context
            .as_ref()
            .map(|(role, _)| format!("Model · {role}"))
            .unwrap_or_else(|| {
                tr(self.locale, MessageId::RouteSurfaceTitle)
                    .replace("{view}", self.view.title_label())
            });
        // The catalog is a visible action on the existing title rail, with
        // no extra row taken from short terminals. Keep the shortcut too.
        let action_label = crate::tui::ui_text::semantic_truncate(
            &view_action,
            usize::from(area.width.saturating_sub(20)),
        );
        let action_width = unicode_width::UnicodeWidthStr::width(action_label.as_str()) as u16;
        let show_action = area.width >= 28
            && area.height > 0
            && (self.assignment_context.is_none()
                || unicode_width::UnicodeWidthStr::width(title.as_str())
                    + usize::from(action_width)
                    + 8
                    <= usize::from(area.width));
        let title = if show_action {
            crate::tui::ui_text::semantic_truncate(
                &title,
                usize::from(area.width.saturating_sub(action_width + 8)),
            )
        } else {
            title
        };
        let inner = render_underwater_surface(area, buf, title);
        if show_action {
            let action = Rect::new(
                inner.right().saturating_sub(action_width),
                area.y + u16::from(area.height >= 24),
                action_width,
                1,
            );
            *self.catalog_action_hitbox.borrow_mut() = Some(action);
            Paragraph::new(action_label)
                .style(if self.catalog_action_hovered {
                    menu_style::hovered_row_style()
                } else {
                    Style::default().fg(palette::WHALE_ACTION).underlined()
                })
                .render(action, buf);
        }
        // ⇧A and ⇧D are query text while a search is typed; advertise them
        // only when they act.
        let searching = !self.query.is_empty();
        let mut footer_hints = vec![
            ActionHint::new("↑↓", tr(self.locale, MessageId::PickerActionMove)),
            ActionHint::new("Tab", tr(self.locale, MessageId::PickerActionSwitch)),
            ActionHint::new(
                tr(self.locale, MessageId::RouteActionType),
                tr(self.locale, MessageId::RouteActionSearchAnyModel),
            ),
            ActionHint::new("Enter", tr(self.locale, self.apply_action_id())),
        ];
        if !searching {
            footer_hints.push(ActionHint::new("⇧A", view_action));
        }
        if inner.height >= 16 {
            footer_hints.push(ActionHint::new(
                "Ctrl+S",
                tr(self.locale, MessageId::SessionsActionSort),
            ));
        }
        if !self.can_edit_effort() {
            footer_hints.remove(1);
        }
        // A Fleet row has no startup default to save; the chord is a
        // session-route action only.
        if self.purpose == ModelPickerPurpose::Session && inner.height >= 16 && !searching {
            footer_hints.insert(
                4,
                ActionHint::new(
                    "⇧D",
                    tr(self.locale, MessageId::PickerActionSetStartupDefault),
                ),
            );
        }
        // Keep compact route modals focused on the core browse/apply actions;
        // wider shells have room to disclose the pin action too.
        // ⇧P / ⇧F act only on a highlighted catalog route; without one
        // (typing a custom id) they are query text, so they are not offered.
        if inner.width >= 72 && inner.height >= 16 && self.highlighted_route().is_some() {
            if self.purpose == ModelPickerPurpose::Session {
                footer_hints.push(ActionHint::new(
                    "⇧F",
                    tr(self.locale, MessageId::PickerActionFleet),
                ));
            }
            footer_hints.push(ActionHint::new(
                "⇧P",
                tr(self.locale, MessageId::PickerActionPin),
            ));
        }
        footer_hints.push(ActionHint::new(
            "Esc",
            tr(self.locale, MessageId::PickerActionCancel),
        ));
        let content = render_modal_footer(inner, buf, &footer_hints);

        let shell = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                ratatui::layout::Constraint::Length(1),
                ratatui::layout::Constraint::Min(1),
            ])
            .split(content);
        Paragraph::new(Line::from(vec![
            Span::styled(
                self.assignment_context
                    .as_ref()
                    .map(|(_, scope)| format!("{scope} · "))
                    .unwrap_or_else(|| {
                        format!("{} ", tr(self.locale, MessageId::RouteProviderLabel))
                    }),
                Style::default().fg(palette::TEXT_MUTED),
            ),
            Span::styled(
                self.resolved_provider()
                    .unwrap_or(self.initial_provider)
                    .display_name(),
                Style::default().fg(palette::TEXT_PRIMARY),
            ),
            Span::styled(
                self.visible_model_rows()
                    .get(self.selected_model_idx)
                    .and_then(|row| row.blocked_reason.as_deref())
                    .map(|reason| format!(" · ! {reason}"))
                    .unwrap_or_default(),
                Style::default().fg(palette::STATUS_WARNING),
            ),
            Span::styled(
                self.notice
                    .as_ref()
                    .map(|(text, _)| format!(" · {text}"))
                    .unwrap_or_default(),
                self.notice
                    .as_ref()
                    .map(|(_, level)| palette::chrome_style(&self.theme, level.ink()))
                    .unwrap_or_default(),
            ),
            Span::styled(
                if self.assignment_context.is_some() {
                    ""
                } else {
                    catalog_freshness_title_suffix()
                },
                Style::default().fg(palette::TEXT_MUTED),
            ),
        ]))
        .render(shell[0], buf);

        let mut layout = widen_model_pane(ListDetailLayout::split(shell[1], 24));
        if !self.can_edit_effort() {
            layout.list = shell[1];
        } else if layout.stacked && shell[1].height < 12 {
            let model_height = if self.focus == Pane::Model {
                shell[1].height.saturating_sub(1)
            } else {
                u16::from(shell[1].height > 0)
            };
            layout.list = Rect::new(shell[1].x, shell[1].y, shell[1].width, model_height);
            layout.detail = Rect::new(
                shell[1].x,
                shell[1].y + model_height,
                shell[1].width,
                shell[1].height.saturating_sub(model_height),
            );
        }

        self.ensure_projection();
        let projection = self.projection.borrow();
        let model_rows = &projection.as_ref().unwrap().rows;
        let model_title = if self.query.trim().is_empty() {
            format!("Model · {}", self.view.title_label())
        } else {
            format!("Model: {}", self.query.trim())
        };
        self.render_pane(
            layout.list,
            buf,
            &model_title,
            model_rows,
            PaneRenderState {
                pane: Pane::Model,
                selected: self.selected_model_idx,
                focused: self.focus == Pane::Model,
            },
        );

        if !self.can_edit_effort() {
            return;
        }
        let effort_provider = self.resolved_provider().unwrap_or(self.initial_provider);
        let current_efforts = self.current_efforts();
        let selected_effort_idx = self
            .selected_effort_idx
            .min(current_efforts.len().saturating_sub(1));
        let effort_rows: Vec<PaneRow> = current_efforts
            .iter()
            .map(|effort| {
                let label = effort
                    .display_label_for_provider(effort_provider)
                    .to_string();
                let hint = match effort {
                    ReasoningEffort::Auto => "choose per turn".to_string(),
                    ReasoningEffort::Off => "no extra reasoning".to_string(),
                    ReasoningEffort::Minimal => "minimal reasoning".to_string(),
                    ReasoningEffort::Low => "lighter reasoning".to_string(),
                    ReasoningEffort::Medium => "balanced reasoning".to_string(),
                    ReasoningEffort::High => "deeper reasoning".to_string(),
                    ReasoningEffort::XHigh => "extra-high reasoning".to_string(),
                    ReasoningEffort::Ultra => "ultra reasoning".to_string(),
                    ReasoningEffort::Max => "maximum reasoning".to_string(),
                };
                PaneRow::effort(label, hint)
            })
            .collect();
        self.render_pane(
            layout.detail,
            buf,
            "Thinking",
            &effort_rows,
            PaneRenderState {
                pane: Pane::Effort,
                selected: selected_effort_idx,
                focused: self.focus == Pane::Effort,
            },
        );
    }
}

/// Rows one PageUp/PageDown travels. Pages clamp at the ends per the shared
/// vocabulary instead of wrapping (#6290).
const MODEL_PAGE: usize = 5;

/// Previous index in a list that rotates: 0 wraps to the last row.
/// `count` must be non-zero.
fn wrapping_prev(index: usize, count: usize) -> usize {
    if index == 0 {
        count - 1
    } else {
        (index - 1).min(count - 1)
    }
}

/// Next index in a list that rotates: the last row wraps to 0.
/// `count` must be non-zero.
fn wrapping_next(index: usize, count: usize) -> usize {
    if index + 1 >= count { 0 } else { index + 1 }
}

pub(crate) fn picker_efforts_for_route(
    provider: ApiProvider,
    base_url: &str,
    wire_model: &str,
    model_is_auto: bool,
) -> Vec<ReasoningEffort> {
    if model_is_auto {
        return AUTO_MODEL_PICKER_EFFORTS.to_vec();
    }
    // Exact-route overrides still win over catalog metadata: Kimi Code K3 and
    // OpenAI Codex have wire dialects the generic Models.dev shape does not
    // fully describe.
    if crate::config::is_exact_kimi_code_k3_route(provider, base_url, wire_model) {
        return KIMI_CODE_K3_PICKER_EFFORTS.to_vec();
    }
    if provider == ApiProvider::OpenaiCodex {
        // The OAuth roster publishes a per-model ladder, and the models differ:
        // gpt-5.6-sol/terra go up to `ultra`, gpt-5.6-luna stops at `max`, and
        // gpt-5.5 and older stop at `xhigh`. Returning one static list for the
        // whole provider offered tiers a model does not have and hid tiers it
        // does. Fall back to the static ladder only when the roster is missing
        // or published no levels for this model.
        return codex_picker_efforts(wire_model).unwrap_or_else(|| CODEX_PICKER_EFFORTS.to_vec());
    }
    if let Some(catalog_efforts) = catalog_picker_efforts(provider, wire_model) {
        return catalog_efforts;
    }
    if matches!(
        provider,
        crate::config::ApiProvider::Deepseek | crate::config::ApiProvider::DeepseekCN
    ) {
        return DEEPSEEK_PICKER_EFFORTS.to_vec();
    }
    DEFAULT_PICKER_EFFORTS.to_vec()
}

/// Thinking tiers for one Codex model, taken from the OAuth roster's
/// `supported_reasoning_levels`. `None` when the roster does not describe the
/// model, so the caller keeps the static Codex ladder rather than inventing
/// tiers.
fn codex_picker_efforts(wire_model: &str) -> Option<Vec<ReasoningEffort>> {
    let roster = crate::codex_model_cache::model_roster();
    let metadata = roster.metadata_for(wire_model)?;
    let mut efforts = Vec::new();
    for raw in &metadata.efforts {
        if let Some(effort) = catalog_effort_value(raw)
            && !efforts.contains(&effort)
        {
            efforts.push(effort);
        }
    }
    (!efforts.is_empty()).then_some(efforts)
}

/// Build thinking-tier rows from Models.dev `reasoning_options` when present.
///
/// Expected shape (already parsed onto the catalog offering):
/// `[{ "type": "effort", "values": ["high", "max"] }]`.
/// Non-effort option types (e.g. MiniMax `thinking`) are mapped when their
/// values collapse cleanly onto our tier vocabulary; unknown values are
/// skipped. Returns `None` when the catalog has no usable effort list so the
/// caller can keep the provider default rather than inventing tiers.
fn catalog_picker_efforts(provider: ApiProvider, wire_model: &str) -> Option<Vec<ReasoningEffort>> {
    let offering = catalog_offering_for_model(provider, wire_model)?;
    let mut efforts = Vec::new();
    let mut saw_effort_list = false;
    for option in &offering.reasoning_options {
        let option_type = option
            .get("type")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        // Prefer explicit effort lists; also accept thinking-mode lists whose
        // values map onto our tiers (adaptive→auto, disabled→off, always_on→max).
        if option_type != "effort" && option_type != "thinking" {
            continue;
        }
        let Some(values) = option.get("values").and_then(|value| value.as_array()) else {
            continue;
        };
        saw_effort_list = true;
        for value in values {
            let Some(raw) = value.as_str() else {
                continue;
            };
            if let Some(effort) = catalog_effort_value(raw)
                && !efforts.contains(&effort)
            {
                efforts.push(effort);
            }
        }
    }
    if !saw_effort_list || efforts.is_empty() {
        return None;
    }
    // Always offer Auto when the catalog published discrete tiers so the
    // operator can still leave the choice to the route default. Do not invent
    // Off unless the catalog said so — some models are always-on.
    if !efforts.contains(&ReasoningEffort::Auto) {
        efforts.insert(0, ReasoningEffort::Auto);
    }
    Some(efforts)
}

fn catalog_effort_value(raw: &str) -> Option<ReasoningEffort> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "off" | "disabled" | "false" => Some(ReasoningEffort::Off),
        "none" => Some(ReasoningEffort::Off), // Muse "none" maps to Off in our enum but display as "none"
        "minimal" | "minimum" => Some(ReasoningEffort::Minimal),
        "low" | "light" => Some(ReasoningEffort::Low),
        "medium" | "mid" => Some(ReasoningEffort::Medium),
        "high" => Some(ReasoningEffort::High),
        "xhigh" => Some(ReasoningEffort::XHigh),
        "ultra" | "ultracode" => Some(ReasoningEffort::Ultra),
        "max" | "maximum" => Some(ReasoningEffort::Max),
        "auto" | "automatic" | "adaptive" => Some(ReasoningEffort::Auto),
        "always_on" | "always-on" => Some(ReasoningEffort::Max),
        _ => None,
    }
}

fn normalize_picker_effort(
    effort: ReasoningEffort,
    provider: ApiProvider,
    base_url: &str,
    wire_model: &str,
    model_is_auto: bool,
) -> ReasoningEffort {
    let normalized = if model_is_auto {
        effort
    } else {
        effort.normalize_for_route(provider, base_url, wire_model)
    };
    let efforts = picker_efforts_for_route(provider, base_url, wire_model, model_is_auto);
    if efforts.contains(&normalized) {
        return normalized;
    }
    // Catalog-driven lists may keep Low/Medium that route normalization would
    // otherwise collapse. Prefer the operator's exact choice when the picker
    // still shows it.
    if efforts.contains(&effort) {
        return effort;
    }
    default_picker_effort(provider, &efforts)
}

fn default_picker_effort(provider: ApiProvider, efforts: &[ReasoningEffort]) -> ReasoningEffort {
    let preferred = if provider == ApiProvider::OpenaiCodex {
        ReasoningEffort::Medium
    } else {
        ReasoningEffort::High
    };
    if efforts.contains(&preferred) {
        preferred
    } else {
        efforts
            .iter()
            .copied()
            .find(|effort| *effort != ReasoningEffort::Auto && *effort != ReasoningEffort::Off)
            .or_else(|| efforts.first().copied())
            .unwrap_or(preferred)
    }
}

fn default_picker_effort_idx(
    provider: ApiProvider,
    base_url: &str,
    wire_model: &str,
    model_is_auto: bool,
) -> usize {
    let efforts = picker_efforts_for_route(provider, base_url, wire_model, model_is_auto);
    let default_effort = default_picker_effort(provider, &efforts);
    efforts
        .iter()
        .position(|effort| *effort == default_effort)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_model_picker_and_runtime_share_exact_persisted_metadata() {
        let _env = crate::test_support::lock_test_env();
        let _catalog = crate::provider_lake::lock_live_snapshot();
        let mut config: Config = toml::from_str(include_str!(
            "../../../config/tests/fixtures/custom_models.toml"
        ))
        .unwrap();
        let id = "deepseek-v4.1-flash-expires-on-0910";
        let base = "https://models.example.test/v1";
        let metadata = effective_picker_metadata(&config, Some(ApiProvider::Deepseek), id);
        assert_eq!(metadata.display_name.as_deref(), Some("Temporary preview"));
        assert_eq!(metadata.context_window, Some(96000));
        assert_eq!(metadata.max_output, Some(8000));
        assert_eq!(metadata.tool_calls, Some(true));
        assert_eq!(metadata.source, Some(CatalogSource::ConfigOverride));
        let mut row = model_row(ApiProvider::Deepseek, true);
        row.id = id.into();
        row.metadata = metadata.clone();
        let chips = model_row_meta_chips(&row);
        assert_eq!(chips[0], "user declared (unverified)");
        assert!(chips.iter().any(|chip| chip.starts_with("estimate ")));
        assert!(fit_meta_chips(&chips, 30).starts_with("user declared"));
        assert!(
            render_picker_model_hint(id, Some(ApiProvider::Deepseek), &metadata, None, None)
                .contains("user declared")
        );
        assert!(
            crate::provider_lake::configured_catalog_models_for_route(
                &config,
                ApiProvider::Deepseek,
                "deepseek",
                base
            )
            .iter()
            .any(|model| model == id)
        );
        let route =
            crate::route_runtime::resolve_runtime_route(&config, ApiProvider::Deepseek, Some(id))
                .unwrap();
        assert_eq!(route.model, id);
        assert_eq!(Some(route.context_window.tokens), metadata.context_window);
        assert_eq!(
            route.context_window.source,
            crate::route_runtime::ContextWindowSource::UserDeclared
        );
        assert!(!route.context_window.source.is_verified());
        assert_eq!(
            route.candidate.capabilities().image_input,
            SupportState::Unknown
        );
        assert_eq!(
            route.candidate.capabilities().native_tool_calls,
            SupportState::Unknown
        );
        assert_eq!(
            crate::route_budget::route_output_limit_tokens(Some(route.candidate.limits())),
            metadata.max_output
        );
        // /load replaces the same metadata alongside the provider route.
        let mut reloaded = config.clone();
        reloaded.custom_models.as_mut().unwrap()[0].reasoning = None;
        reloaded.custom_models.as_mut().unwrap()[0].limit = None;
        reloaded.custom_models.as_mut().unwrap()[0].cost = None;
        reloaded.custom_models.as_mut().unwrap()[0].tool_call = None;
        reloaded.custom_models.as_mut().unwrap()[0].modalities = None;
        config.refresh_provider_routes_from(&reloaded);
        let unknown = effective_picker_metadata(&config, Some(ApiProvider::Deepseek), id);
        // An automatic request allowance is policy, not discovered metadata.
        // Its limits are covered by route_budget; the picker must stay unknown.
        assert_eq!(unknown.context_window, None);
        assert_eq!(unknown.max_output, None);
        assert_eq!(unknown.tool_calls, None);
        assert_eq!(unknown.vision, SupportState::Unknown);
        assert_eq!(unknown.pricing, PickerPricing::Unknown);
        row.metadata = unknown;
        assert!(model_row_meta_chips(&row).contains(&"reasoning unknown".to_string()));
    }

    fn model_row(provider: ApiProvider, in_default_view: bool) -> ModelPickerRow {
        ModelPickerRow {
            id: "model".to_string(),
            provider: Some(provider),
            provider_identity: None,
            hint: String::new(),
            metadata: EffectivePickerMetadata::default(),
            selectable: true,
            blocked_reason: None,
            default_rank: in_default_view.then_some((DefaultSection::Current, 0)),
            credentialed: false,
            not_listed: None,
        }
    }

    /// #6523 review: named custom providers are exact config keys, so a pin
    /// on `TeamA` must not mark the same model under `teama`; built-in slugs
    /// stay case-insensitive aliases.
    #[test]
    fn pin_matching_is_exact_for_named_custom_providers_only() {
        let pin = |provider: &str| PinnedModel {
            provider: provider.to_string(),
            model: "model".to_string(),
            label: None,
        };
        let custom = |identity: &str| ModelPickerRow {
            provider_identity: Some(identity.to_string()),
            ..model_row(ApiProvider::Custom, true)
        };
        assert!(pin_names_row(&pin("TeamA"), &custom("TeamA")));
        assert!(!pin_names_row(&pin("TeamA"), &custom("teama")));
        assert!(!pin_names_row(&pin("teama"), &custom("TeamA")));

        let builtin = model_row(ApiProvider::Deepseek, true);
        assert!(pin_names_row(&pin("deepseek"), &builtin));
        assert!(pin_names_row(&pin("DeepSeek"), &builtin));
    }

    /// The catalog sort ranks pins with the same exact match that marks them:
    /// a `TeamA` pin must not lift an unmarked `teama` row into the pinned
    /// block above the groups.
    #[test]
    fn catalog_sort_ranks_only_rows_the_pin_names() {
        let pins = vec![PinnedModel {
            provider: "TeamA".to_string(),
            model: "model".to_string(),
            label: None,
        }];
        let custom = |identity: &str| ModelPickerRow {
            provider_identity: Some(identity.to_string()),
            ..model_row(ApiProvider::Custom, true)
        };
        let owned = [
            model_row(ApiProvider::Deepseek, true),
            custom("teama"),
            custom("TeamA"),
        ];
        let mut rows: Vec<&ModelPickerRow> = owned.iter().collect();
        sort_model_rows_for_view(&mut rows, |row| *row, ModelListView::Catalog, &pins);

        // Pinned `TeamA` leads; `teama` falls back to plain group order
        // (after `deepseek`) instead of riding the pin into the top block.
        let order: Vec<Option<&str>> = rows
            .iter()
            .map(|row| row.provider_identity.as_deref())
            .collect();
        assert_eq!(order, vec![Some("TeamA"), None, Some("teama")]);
        assert!(pin_for_row(&pins, rows[0]).is_some());
        assert!(pin_for_row(&pins, rows[2]).is_none());
    }

    #[test]
    fn locked_model_keeps_keyboard_focus_visible_without_becoming_selectable() {
        let mut picker = test_picker();
        picker.model_rows[0].selectable = false;
        picker.model_rows[0].blocked_reason = Some("missing key".to_string());
        let area = Rect::new(0, 0, 100, 32);
        let mut buf = Buffer::empty(area);
        picker.render(area, &mut buf);
        let hit = picker
            .row_hitboxes
            .borrow()
            .iter()
            .find(|(_, pane, idx)| *pane == Pane::Model && *idx == 0)
            .unwrap()
            .0;
        assert_eq!(buf[(hit.right() - 1, hit.y)].bg, palette::SELECTION_BG);
        assert!(!picker.model_rows[0].selectable);
    }

    #[test]
    fn workbench_hover_preserves_model_selection_and_clears_on_keyboard_input() {
        let mut picker = test_picker();
        picker
            .model_rows
            .push(model_row(ApiProvider::Deepseek, true));
        let area = Rect::new(0, 0, 100, 32);
        let mut buf = Buffer::empty(area);
        picker.render(area, &mut buf);
        let hit = picker
            .row_hitboxes
            .borrow()
            .iter()
            .find(|(_, pane, idx)| *pane == Pane::Model && *idx == 1)
            .unwrap()
            .0;
        picker.handle_mouse(MouseEvent {
            kind: MouseEventKind::Moved,
            column: hit.x,
            row: hit.y,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(picker.hovered_row, Some((Pane::Model, 1)));
        assert_eq!(picker.selected_model_idx, 0);
        picker.render(area, &mut buf);
        assert_eq!(buf[(hit.right() - 1, hit.y)].bg, palette::SURFACE_ELEVATED);
        picker.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(picker.hovered_row, None);
    }

    fn test_picker() -> ModelPickerView {
        ModelPickerView {
            initial_model: "model".to_string(),
            previous_model: "model".to_string(),
            initial_provider: ApiProvider::Openai,
            initial_provider_identity: ApiProvider::Openai.as_str().to_string(),
            initial_effort: ReasoningEffort::Auto,
            selected_effort_request: ReasoningEffort::Auto,
            active_accepts_custom_model_ids: false,
            query: String::new(),
            selected_model_idx: 0,
            selected_effort_idx: 0,
            focus: Pane::Model,
            show_custom_model_row: false,
            model_rows: vec![model_row(ApiProvider::Openai, true)],
            route_config: Config::default(),
            provider_health: Default::default(),
            view: ModelListView::Configured,
            configured_providers: Vec::new(),
            row_hitboxes: RefCell::new(Vec::new()),
            last_mouse_selected: None,
            hovered_row: None,
            locale: Locale::En,
            pinned_models: Vec::new(),
            projection: RefCell::new(None),
            sort: None,
            column_hitboxes: RefCell::new(Vec::new()),
            pane_hitboxes: RefCell::new(Vec::new()),
            catalog_action_hitbox: RefCell::new(None),
            catalog_action_hovered: false,
            purpose: ModelPickerPurpose::Session,
            assignment_context: None,
            notice: None,
            theme: codewhale_palette::UI_THEME,
        }
    }

    /// Opened for a Fleet row, Enter hands the editor the absolute route
    /// instead of switching the session; the startup-default chord is the
    /// same pick.
    #[test]
    fn catalog_header_click_matches_keyboard_at_compact_and_wide_sizes() {
        for (width, height) in [(40, 12), (80, 24), (140, 40)] {
            let mut mouse_picker = test_picker();
            let mut key_picker = test_picker();
            let area = Rect::new(0, 0, width, height);
            mouse_picker.render(area, &mut Buffer::empty(area));
            let hit = mouse_picker
                .catalog_action_hitbox
                .borrow()
                .expect("catalog action");
            assert!(area.contains((hit.x, hit.y).into()));
            assert!(hit.right() <= area.right());
            assert!(matches!(
                mouse_picker.handle_mouse(MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    column: hit.x,
                    row: hit.y,
                    modifiers: KeyModifiers::NONE,
                }),
                ViewAction::None
            ));
            key_picker.handle_key(KeyEvent::new(KeyCode::Char('A'), KeyModifiers::SHIFT));
            assert_eq!(mouse_picker.view, key_picker.view);
            assert_eq!(
                mouse_picker.selected_model_idx,
                key_picker.selected_model_idx
            );
            let empty = Rect::new(0, 0, 0, 0);
            mouse_picker.render(empty, &mut Buffer::empty(empty));
            assert!(mouse_picker.catalog_action_hitbox.borrow().is_none());
        }
    }

    #[test]
    fn fleet_purpose_enter_hands_the_absolute_route_to_the_editor() {
        let mut picker = test_picker();
        picker.purpose = ModelPickerPurpose::FleetRoute {
            target: FleetRouteTarget::Member(1),
            editor_id: uuid::Uuid::nil(),
            initial_reasoning: None,
            allow_inherit: true,
        };
        let action = picker.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(
            matches!(
                &action,
                ViewAction::EmitAndClose(ViewEvent::FleetRoutePicked {
                    target: FleetRouteTarget::Member(1),
                    provider: ApiProvider::Openai,
                    provider_id: None,
                    model,
                    editor_id,
                    reasoning: None,
                }) if model == "model" && editor_id.is_nil()
            ),
            "{action:?}"
        );
        assert!(matches!(
            picker.handle_key(KeyEvent::new(KeyCode::Char('D'), KeyModifiers::SHIFT)),
            ViewAction::EmitAndClose(ViewEvent::FleetRoutePicked { .. })
        ));
        // The session-purpose picker is untouched by the new purpose.
        let mut session = test_picker();
        assert!(matches!(
            session.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            ViewAction::EmitAndClose(ViewEvent::ModelPickerApplied { .. })
        ));
    }

    #[test]
    fn profile_role_picker_assigns_only_its_owner_and_search_cancels_before_closing() {
        let editor_id = uuid::Uuid::new_v4();
        let mut picker = test_picker().with_assignment_context("reviewer", "Choose where to save");
        picker.purpose = ModelPickerPurpose::FleetProfileRoute {
            editor_id,
            initial_reasoning: None,
        };
        assert!(
            matches!(picker.build_apply_event(false), ViewEvent::FleetProfileRoutePicked {
            editor_id: owner, provider: ApiProvider::Openai, model, ..
        } if owner == editor_id && model == "model")
        );
        for (width, height) in [(40, 12), (80, 24), (140, 40)] {
            let area = Rect::new(0, 0, width, height);
            let mut buf = Buffer::empty(area);
            picker.render(area, &mut buf);
            let text: String = buf.content().iter().map(|cell| cell.symbol()).collect();
            assert!(text.contains("reviewer"), "{text}");
            assert!(text.contains("Choose where to save"), "{text}");
        }
        picker.update_query("model".into());
        assert!(matches!(
            picker.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            ViewAction::None
        ));
        assert!(picker.query.is_empty());
        assert!(
            matches!(picker.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)), ViewAction::EmitAndClose(ViewEvent::FleetAssignmentPickerDismissed { editor_id: owner }) if owner == editor_id)
        );
        picker.model_rows[0].selectable = false;
        *picker.projection.get_mut() = None;
        assert!(matches!(
            picker.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            ViewAction::Emit(ViewEvent::StatusMessage { .. })
        ));
    }

    #[test]
    fn fleet_locked_builtin_explains_setup_without_switching_the_session() {
        let mut picker = test_picker();
        picker.purpose = ModelPickerPurpose::FleetRoute {
            target: FleetRouteTarget::Operator,
            editor_id: uuid::Uuid::nil(),
            initial_reasoning: None,
            allow_inherit: true,
        };
        picker.model_rows[0].selectable = false;
        *picker.projection.get_mut() = None;
        assert!(
            matches!(picker.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)), ViewAction::Emit(ViewEvent::StatusMessage { message }) if message.contains("/provider"))
        );
    }

    #[test]
    fn fleet_picker_keeps_the_rows_exact_pin_and_effort_after_refresh() {
        let _env = crate::test_support::lock_test_env();
        let _live = crate::provider_lake::lock_live_snapshot();
        let (config, mut app, home, _workspace) =
            resumed_openrouter_session_with_named_custom_routes();
        let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", home.path());
        crate::provider_catalog_live::reset_cache_for_test();
        crate::provider_lake::clear_live_snapshot();
        app.model_picker_memory = Some(crate::tui::app::ModelPickerMemory {
            catalog_view: true,
            view: Some("catalog".to_string()),
            selected_row_id: Some(app.model.clone()),
        });
        let session_route = (app.api_provider, app.model.clone());
        let mut picker = ModelPickerView::new_for_fleet_route(
            &app,
            &config,
            FleetRouteTarget::Member(0),
            uuid::Uuid::nil(),
            FleetRouteSelection {
                provider: Some("command_code".to_string()),
                model: Some("deepseek/deepseek-v4-flash".to_string()),
                reasoning: Some(ReasoningEffort::High),
                allow_inherit: true,
            },
        );
        assert_eq!(
            picker.resolved_provider_identity().as_deref(),
            Some("command_code")
        );
        assert_eq!(picker.resolved_model(), "deepseek/deepseek-v4-flash");
        assert_eq!(picker.selected_effort_request, ReasoningEffort::High);
        assert!(picker.can_edit_effort());
        assert_eq!(
            picker
                .visible_model_rows()
                .iter()
                .filter(|row| row.id == "deepseek/deepseek-v4-flash"
                    && row_provider_identity(row) == Some("command_code"))
                .count(),
            1
        );
        picker.selected_effort_request = ReasoningEffort::Low;
        picker.re_resolve_from_app(&app, &config);
        assert!(picker.can_edit_effort());
        assert!(matches!(
            picker.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            ViewAction::EmitAndClose(ViewEvent::FleetRoutePicked {
                provider: ApiProvider::Custom, provider_id: Some(identity), model,
                reasoning: Some(ReasoningEffort::Low), ..
            }) if identity == "command_code" && model == "deepseek/deepseek-v4-flash"
        ));
        assert_eq!((app.api_provider, app.model.clone()), session_route);
        assert!(matches!(
            picker.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            ViewAction::EmitAndClose(ViewEvent::FleetAssignmentPickerDismissed { .. })
        ));
        for query in [
            "openrouter:new-fixture-model",
            "openrouter new-fixture-model",
        ] {
            picker.update_query(query.to_string());
            assert_eq!(picker.resolved_provider(), Some(ApiProvider::Openrouter));
            assert_eq!(picker.resolved_model(), "new-fixture-model");
        }
        // A slash is part of a model id, not a provider switch: compatible
        // routes routinely serve names such as openai/gpt-5.
        picker.update_query("openrouter/new-fixture-model".to_string());
        assert_eq!(picker.resolved_provider(), Some(ApiProvider::Custom));
        assert_eq!(picker.resolved_model(), "openrouter/new-fixture-model");
    }

    #[test]
    fn fleet_shortlist_picker_has_no_inherit_or_reasoning_edits() {
        let _env = crate::test_support::lock_test_env();
        let _live = crate::provider_lake::lock_live_snapshot();
        let (config, app, home, _workspace) = resumed_openrouter_session_with_named_custom_routes();
        let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", home.path());
        let mut picker = ModelPickerView::new_for_fleet_route(
            &app,
            &config,
            FleetRouteTarget::Member(0),
            uuid::Uuid::nil(),
            FleetRouteSelection {
                provider: Some("command_code".to_string()),
                model: Some("deepseek/deepseek-v4-flash".to_string()),
                reasoning: None,
                allow_inherit: false,
            },
        );
        for _ in 0..2 {
            assert!(!picker.can_edit_effort());
            assert!(
                picker
                    .visible_model_rows()
                    .iter()
                    .all(|row| row.id != "auto")
            );
            picker.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
            assert_eq!(picker.focus, Pane::Model);
            render_text(&picker, 150, 42);
            assert!(
                picker
                    .pane_hitboxes
                    .borrow()
                    .iter()
                    .all(|(_, pane)| *pane == Pane::Model)
            );
            assert!(matches!(
                picker.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
                ViewAction::EmitAndClose(ViewEvent::FleetRoutePicked {
                    reasoning: None,
                    ..
                })
            ));
            picker.re_resolve_from_app(&app, &config);
        }
        picker.update_query("auto".to_string());
        assert!(
            !matches!(picker.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)), ViewAction::EmitAndClose(ViewEvent::FleetRoutePicked { model, .. }) if model == "auto")
        );
    }

    #[test]
    fn fleet_inherited_route_is_selectable_and_named_without_session_autorouting_claims() {
        let _env = crate::test_support::lock_test_env();
        let _live = crate::provider_lake::lock_live_snapshot();
        let (config, app, home, _workspace) = resumed_openrouter_session_with_named_custom_routes();
        let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", home.path());
        let mut picker = ModelPickerView::new_for_fleet_route(
            &app,
            &config,
            FleetRouteTarget::Member(0),
            uuid::Uuid::nil(),
            FleetRouteSelection {
                provider: None,
                model: None,
                reasoning: None,
                allow_inherit: true,
            },
        );
        assert_eq!(picker.resolved_model(), "auto");
        assert!(picker.selected_model_is_selectable());
        assert!(
            render_text(&picker, 150, 42)
                .contains(tr(app.ui_locale, MessageId::FleetRouteInherited).as_ref())
        );
        assert_eq!(picker.current_efforts(), vec![ReasoningEffort::Auto]);
        assert!(
            matches!(picker.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)), ViewAction::EmitAndClose(ViewEvent::FleetRoutePicked { model, reasoning: None, .. }) if model == "auto")
        );
    }

    fn render_text(picker: &ModelPickerView, width: u16, height: u16) -> String {
        let area = Rect::new(0, 0, width, height);
        let mut buffer = Buffer::empty(area);
        picker.render(area, &mut buffer);
        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn deepseek_picker_heading_hides_legacy_family_metadata() {
        assert_eq!(
            catalog_family_for_identity(ApiProvider::Deepseek, None, "deepseek-v4-pro").as_deref(),
            Some("DeepSeek")
        );
    }

    #[test]
    fn default_view_shows_only_sectioned_rows_and_auto() {
        let row = model_row(ApiProvider::Deepseek, false);
        assert!(!model_row_visible_by_default(&row));
        assert!(model_row_visible_by_default(&model_row(
            ApiProvider::Deepseek,
            true
        )));
        let mut auto = model_row(ApiProvider::Deepseek, false);
        auto.provider = None;
        assert!(model_row_visible_by_default(&auto));
    }

    #[test]
    fn lowercase_picker_action_letters_begin_a_model_search() {
        for ch in ['a', 'p', 'r'] {
            let mut picker = test_picker();
            assert!(matches!(
                picker.handle_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)),
                ViewAction::None
            ));
            assert_eq!(picker.query, ch.to_string(), "{ch} must begin a search");
            assert_eq!(picker.view, ModelListView::Configured);
        }
    }

    #[test]
    fn shifted_picker_actions_cycle_views_pin_and_refresh_explicitly() {
        let mut picker = test_picker();

        assert!(matches!(
            picker.handle_key(KeyEvent::new(KeyCode::Char('A'), KeyModifiers::SHIFT)),
            ViewAction::None
        ));
        assert_eq!(picker.view, ModelListView::Catalog);
        assert!(picker.query.is_empty());

        assert!(matches!(
            picker.handle_key(KeyEvent::new(KeyCode::Char('P'), KeyModifiers::SHIFT)),
            ViewAction::Emit(ViewEvent::ModelPickerTogglePin {
                provider: ApiProvider::Openai,
                provider_id: None,
                model,
            }) if model == "model"
        ));
        assert!(picker.query.is_empty());

        assert!(matches!(
            picker.handle_key(KeyEvent::new(KeyCode::Char('F'), KeyModifiers::SHIFT)),
            ViewAction::Emit(ViewEvent::ModelPickerToggleFleet {
                provider: ApiProvider::Openai,
                provider_id: None,
                model,
            }) if model == "model"
        ));
        assert!(picker.query.is_empty());

        assert!(matches!(
            picker.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL)),
            ViewAction::Emit(ViewEvent::ModelPickerRefresh)
        ));
    }

    #[test]
    fn fleet_models_lead_the_pins_the_picker_sorts_by() {
        let _lock = crate::test_support::lock_test_env();
        let temp = tempfile::tempdir().expect("tempdir");
        let home = temp.path().join("home");
        let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", home.as_os_str());
        let workspace = temp.path().join("repo");
        std::fs::create_dir_all(&workspace).expect("workspace");
        crate::fleet::members::add_fleet_model(
            &workspace,
            "openrouter",
            "z-ai/glm-5.3-flash",
            &["scout".to_string()],
        )
        .expect("fleet add");

        let options = crate::tui::app::TuiOptions {
            ..crate::test_support::test_tui_options(workspace.clone())
        };
        let mut app = crate::tui::app::App::new(options, &Config::default());
        app.workspace = workspace;
        app.pinned_models = vec![PinnedModel {
            provider: "anthropic".to_string(),
            model: "claude-haiku-4-5".to_string(),
            label: None,
        }];

        let pins = picker_pins_for_app(&app);
        assert_eq!(pins.len(), 2, "fleet model then the person's pin: {pins:?}");
        assert_eq!(pins[0].provider, "openrouter");
        assert_eq!(pins[0].model, "z-ai/glm-5.3-flash");
        assert_eq!(pins[0].label.as_deref(), Some("fleet · explore"));
        assert_eq!(pins[1].model, "claude-haiku-4-5");
    }

    #[test]
    fn fleet_case_distinct_pins_keep_labels_order_and_refresh_selection() {
        std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn(|| {
                let _lock = crate::test_support::lock_test_env();
                let root = tempfile::tempdir().unwrap();
                let _home = crate::test_support::EnvVarGuard::set(
                    "CODEWHALE_HOME",
                    root.path().join("home"),
                );
                let workspace = root.path().join("workspace");
                std::fs::create_dir_all(&workspace).unwrap();
                let lower = "preview-fixture";
                let upper = "Preview-fixture";
                let mut config: Config = toml::from_str(include_str!(
                    "../../../config/tests/fixtures/custom_models.toml"
                ))
                .unwrap();
                config.default_text_model = Some(lower.into());
                config.set_provider_model_override(ApiProvider::Deepseek, Some(lower.into()));
                config.set_provider_api_key_override(
                    ApiProvider::Deepseek,
                    Some("fixture-key".into()),
                );
                config.custom_models.as_mut().unwrap()[0].id = lower.into();
                let mut second = config.custom_models.as_ref().unwrap()[0].clone();
                second.id = upper.into();
                config.custom_models.as_mut().unwrap().push(second);
                crate::fleet::members::add_fleet_model(
                    &workspace,
                    "deepseek",
                    lower,
                    &["scout".into()],
                )
                .unwrap();
                crate::fleet::members::add_fleet_model(
                    &workspace,
                    "deepseek",
                    upper,
                    &["reviewer".into()],
                )
                .unwrap();
                let options = crate::test_support::test_tui_options(workspace.clone());
                let mut app = App::new(options, &config);
                app.workspace = workspace.clone();
                let pins = picker_pins_for_app(&app);
                let rows = picker_model_rows_for_app(&app, &config);
                for pin in &pins {
                    let row = rows
                        .iter()
                        .find(|row| {
                            row.provider == Some(ApiProvider::Deepseek) && row.id == pin.model
                        })
                        .unwrap();
                    assert!(row.hint.starts_with(pin.label.as_deref().unwrap()));
                    assert!(
                        row.hint
                            .contains(&format!("exact deepseek / {}", pin.model))
                    );
                }
                let mut picker = ModelPickerView::new(&app, &config);
                let visible = picker.visible_model_rows();
                // The current route leads (#6533); the pins follow it.
                assert_eq!(visible[0].id, app.model);
                assert_eq!(
                    visible[1].id, lower,
                    "saved pin order precedes lexical order"
                );
                assert_eq!(visible[2].id, upper);
                let upper_index = visible.iter().position(|row| row.id == upper).unwrap();
                drop(visible);
                picker.selected_model_idx = upper_index;
                picker.re_resolve_from_app(&app, &config);
                assert_eq!(
                    picker.resolved_model(),
                    upper,
                    "refresh cannot select its case sibling"
                );

                // The still-saved upper route remains a distinct stale row after its
                // declaration disappears; a live lower row cannot hide it.
                config
                    .custom_models
                    .as_mut()
                    .unwrap()
                    .retain(|row| row.id == lower);
                let options = crate::test_support::test_tui_options(workspace.clone());
                let mut reloaded = App::new(options, &config);
                reloaded.workspace = workspace;
                let rows = picker_model_rows_for_app(&reloaded, &config);
                let stale = rows
                    .iter()
                    .filter(|row| row.provider == Some(ApiProvider::Deepseek) && row.id == upper)
                    .collect::<Vec<_>>();
                assert_eq!(stale.len(), 1);
                assert_eq!(stale[0].blocked_reason.as_deref(), Some("stale pin"));
                assert!(
                    rows.iter()
                        .any(|row| row.provider == Some(ApiProvider::Deepseek)
                            && row.id == lower
                            && row.blocked_reason.as_deref() != Some("stale pin"))
                );
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn wide_picker_footer_advertises_shifted_view_and_pin_actions() {
        let picker = test_picker();
        let text = render_text(&picker, 100, 30);

        assert!(text.contains("⇧A"), "missing shifted view hint: {text}");
        assert!(text.contains("⇧P"), "missing shifted pin hint: {text}");
        assert!(text.contains("⇧F"), "missing shifted fleet hint: {text}");
    }

    #[test]
    fn full_catalog_navigation_sort_refresh_and_mouse_stay_coherent() {
        const PROBE: &str = "CODEWHALE_PICKER_CATALOG_PROBE";
        if std::env::var_os(PROBE).is_none() {
            let fixture = tempfile::tempdir().expect("picker fixture");
            let home = fixture.path().join("home");
            let workspace = fixture.path().join("workspace");
            std::fs::create_dir_all(&home).unwrap();
            std::fs::create_dir_all(&workspace).unwrap();
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "tui::model_picker::tests::full_catalog_navigation_sort_refresh_and_mouse_stay_coherent", "--nocapture", "--test-threads=1"])
                .env_clear()
                .env(PROBE, "1")
                .env("HOME", &home)
                .env("USERPROFILE", &home)
                .env("XDG_CONFIG_HOME", home.join("config"))
                .env("XDG_CACHE_HOME", home.join("cache"))
                .env("XDG_DATA_HOME", home.join("data"))
                .env("CODEWHALE_HOME", home.join(".codewhale"))
                .env("CODEWHALE_DISABLE_MODELS_DEV_FETCH", "1")
                .env("CODEWHALE_NO_UPDATE_CHECK", "1")
                .env("CODEWHALE_TELEMETRY", "0")
                .current_dir(&workspace)
                .output().expect("isolated picker test");
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
            return;
        }
        let config = Config::default();
        let mut app = App::new(
            crate::test_support::test_tui_options(std::env::current_dir().unwrap()),
            &config,
        );
        let mut picker = ModelPickerView::new(&app, &config);
        picker.handle_key(KeyEvent::new(KeyCode::Char('A'), KeyModifiers::SHIFT));
        let count = picker.model_row_count();
        assert!(count >= 200, "use the actual full catalog: {count} rows");
        let projection_storage = || {
            let projection = picker.projection.borrow();
            let projection = projection.as_ref().unwrap();
            (projection.indices.as_ptr(), projection.rows.as_ptr())
        };
        let initial_storage = projection_storage();
        let started = std::time::Instant::now();
        for _ in 0..30 {
            picker.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
            render_text(&picker, 150, 42);
            let projection = picker.projection.borrow();
            let projection = projection.as_ref().unwrap();
            assert_eq!(
                (projection.indices.as_ptr(), projection.rows.as_ptr()),
                initial_storage,
                "navigation must not rebuild the full catalog or formatted rows"
            );
        }
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "30 navigation+render iterations over {count} catalog rows must stay fast"
        );

        // Every catalog entry must remain visible when highlighted, even when
        // adjacent providers each require a separate family heading.
        for (width, height) in [(80, 24), (100, 30), (150, 42)] {
            for selected in 0..count {
                picker.selected_model_idx = selected;
                render_text(&picker, width, height);
                let panes = picker.pane_hitboxes.borrow();
                let area = panes
                    .iter()
                    .find(|(_, pane)| *pane == Pane::Model)
                    .unwrap()
                    .0;
                let hitboxes = picker.row_hitboxes.borrow();
                assert!(
                    hitboxes
                        .iter()
                        .any(|(_, pane, index)| *pane == Pane::Model && *index == selected),
                    "selected row {selected} disappeared at {width}x{height}"
                );
                for (rect, pane, _) in hitboxes.iter().filter(|(_, pane, _)| *pane == Pane::Model) {
                    assert_eq!(*pane, Pane::Model);
                    assert!(rect.y >= area.y && rect.bottom() <= area.bottom());
                }
            }
        }

        picker.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
        render_text(&picker, 150, 42);
        let (effort_rect, _, _) = *picker
            .row_hitboxes
            .borrow()
            .iter()
            .find(|(_, pane, index)| *pane == Pane::Effort && *index == 1)
            .unwrap();
        picker.handle_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: effort_rect.x + 1,
            row: effort_rect.y,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(picker.focus, Pane::Effort);
        let before = picker.selected_model_idx;
        let requested_effort = picker.selected_effort_request;
        let model_area = picker
            .pane_hitboxes
            .borrow()
            .iter()
            .find(|(_, pane)| *pane == Pane::Model)
            .unwrap()
            .0;
        picker.handle_mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: model_area.x + 1,
            row: model_area.y + 3,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(picker.focus, Pane::Model);
        assert_eq!(picker.selected_model_idx, wrapping_next(before, count));
        assert_eq!(picker.selected_effort_request, requested_effort);

        // Actual header hitboxes sort in both directions without changing the
        // selected provider/model; missing context is never treated as zero.
        let selected = (picker.resolved_provider(), picker.resolved_model());
        for column in [
            ModelSortColumn::Model,
            ModelSortColumn::Provider,
            ModelSortColumn::Context,
        ] {
            for descending in [false, true] {
                render_text(&picker, 150, 42);
                let rect = picker
                    .column_hitboxes
                    .borrow()
                    .iter()
                    .find(|(_, target)| *target == column)
                    .unwrap()
                    .0;
                picker.handle_mouse(MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    column: rect.x,
                    row: rect.y,
                    modifiers: KeyModifiers::NONE,
                });
                assert_eq!(picker.sort, Some(ModelSort { column, descending }));
                assert_eq!(
                    picker.focus,
                    Pane::Model,
                    "header click must focus the Model pane"
                );
                assert_eq!(
                    (picker.resolved_provider(), picker.resolved_model()),
                    selected
                );
                let visible = picker.visible_model_rows();
                let rows: Vec<_> = visible
                    .iter()
                    .filter(|row| row.provider.is_some())
                    .collect();
                if column == ModelSortColumn::Context {
                    let mut unknown = false;
                    let mut previous = None;
                    for row in rows {
                        match row.metadata.context_window {
                            None => unknown = true,
                            Some(context) => {
                                assert!(!unknown, "unknown context must stay last");
                                if let Some(previous) = previous {
                                    assert!(if descending {
                                        previous >= context
                                    } else {
                                        previous <= context
                                    });
                                }
                                previous = Some(context);
                            }
                        }
                    }
                }
            }
        }
        picker.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
        assert_eq!(picker.sort, None, "cycle returns to default view/pin order");
        picker.update_query("openrouter".to_string());
        assert!(
            picker
                .visible_model_rows()
                .iter()
                .all(|row| row.provider == Some(ApiProvider::Openrouter))
        );
        assert_eq!(
            picker.projection.borrow().as_ref().unwrap().query,
            "openrouter"
        );
        picker.update_query(String::new());
        assert_eq!(picker.model_row_count(), count);
        let selected = (picker.resolved_provider(), picker.resolved_model());
        app.pinned_models.push(PinnedModel {
            provider: "openrouter".into(),
            model: "z-ai/glm-5.3-flash".into(),
            label: None,
        });
        picker.re_resolve_from_app(&app, &config);
        assert_eq!(picker.visible_model_rows()[0].id, "z-ai/glm-5.3-flash");
        assert_eq!(
            (picker.resolved_provider(), picker.resolved_model()),
            selected
        );

        // A real catalog/readiness refresh replaces cached presentation facts.
        let mut offering =
            catalog_offering_for_model(ApiProvider::Deepseek, "deepseek-v4-pro").unwrap();
        offering.limit.as_mut().unwrap().context = Some(777_000);
        crate::provider_lake::set_live_snapshot(
            codewhale_config::catalog::CatalogSnapshot {
                offerings: vec![offering],
            },
            crate::provider_lake::LiveSource::ModelsDev,
        );
        picker.re_resolve_from_app(&app, &config);
        let visible = picker.visible_model_rows();
        let row = visible
            .iter()
            .find(|row| row.provider == Some(ApiProvider::Deepseek) && row.id == "deepseek-v4-pro")
            .unwrap();
        assert_eq!(row.metadata.context_window, Some(777_000));
        let index = visible
            .iter()
            .position(|row| {
                row.provider == Some(ApiProvider::Deepseek) && row.id == "deepseek-v4-pro"
            })
            .unwrap();
        assert!(
            picker.projection.borrow().as_ref().unwrap().rows[index]
                .meta
                .contains(&format_context_window(777_000))
        );
    }

    #[test]
    fn family_heading_viewport_reserves_the_selected_row_before_hitboxes() {
        let rows: Vec<_> = (0..32)
            .map(|index| PaneRow {
                primary: format!("model-{index}"),
                route: format!("provider-{index}"),
                family: Some(format!("family-{index}")),
                ..PaneRow::default()
            })
            .collect();
        for height in 1..12 {
            for selected in 0..rows.len() {
                let (start, end) = pane_row_window(selected, &rows, height);
                assert!(
                    start <= selected && selected < end,
                    "{start}..{end} omits {selected} at height {height}"
                );
                let lines: usize = (start..end)
                    .map(|index| 1 + usize::from(height > 1 && family_header_before(&rows, index)))
                    .sum();
                assert!(lines <= height);
            }
        }
    }

    #[test]
    fn baseten_picker_models_use_exact_identity_and_direct_provider_label() {
        let _env = crate::test_support::lock_test_env();
        let _live = crate::provider_lake::lock_live_snapshot();
        let home = tempfile::tempdir().expect("test home");
        let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", home.path());
        crate::provider_catalog_live::reset_cache_for_test();
        crate::provider_lake::clear_live_snapshot();

        // No compiled seeds: a custom route offers nothing until a live
        // listing lands for its exact endpoint (#6289).
        assert!(
            provider_catalog_model_ids(
                ApiProvider::Custom,
                codewhale_config::catalog::BASETEN_PROVIDER_ID,
                codewhale_config::catalog::BASETEN_BASE_URL,
            )
            .is_empty()
        );

        let fingerprint = codewhale_config::catalog::base_url_fingerprint(
            codewhale_config::catalog::BASETEN_BASE_URL,
        );
        crate::provider_catalog_live::record_success(
            codewhale_config::catalog::ProviderCatalogDelta {
                provider: codewhale_config::catalog::BASETEN_PROVIDER_ID.to_string(),
                base_url_fingerprint: fingerprint.clone(),
                fetched_at: 1,
                offerings: vec![codewhale_config::catalog::CatalogOffering {
                    provider: codewhale_config::catalog::BASETEN_PROVIDER_ID.to_string(),
                    wire_model_id: codewhale_config::catalog::BASETEN_DEFAULT_MODEL.to_string(),
                    endpoint_key: "chat".to_string(),
                    source: CatalogSource::Live {
                        base_url_fingerprint: fingerprint,
                        fetched_at: 1,
                    },
                    ..Default::default()
                }],
            },
        );

        let models = provider_catalog_model_ids(
            ApiProvider::Custom,
            codewhale_config::catalog::BASETEN_PROVIDER_ID,
            codewhale_config::catalog::BASETEN_BASE_URL,
        );
        assert_eq!(
            models,
            vec![codewhale_config::catalog::BASETEN_DEFAULT_MODEL.to_string()]
        );

        let row = ModelPickerRow {
            id: codewhale_config::catalog::BASETEN_DEFAULT_MODEL.to_string(),
            provider: Some(ApiProvider::Custom),
            provider_identity: Some(codewhale_config::catalog::BASETEN_PROVIDER_ID.to_string()),
            hint: String::new(),
            metadata: EffectivePickerMetadata::default(),
            selectable: true,
            blocked_reason: None,
            default_rank: None,
            credentialed: false,
            not_listed: None,
        };
        let labels = route_labels_for_rows(&[&row]);
        // No compiled display names: the route label is the table key itself.
        assert_eq!(labels.get("baseten").map(String::as_str), Some("baseten"));
    }

    #[test]
    fn provider_catalog_hint_never_calls_failed_or_mismatched_rows_live() {
        assert_eq!(
            provider_catalog_source_label(Some(&(CatalogStatus::Fresh, true))),
            "live"
        );
        assert_eq!(
            provider_catalog_source_label(Some(&(
                CatalogStatus::Failed {
                    reason: CatalogRefreshError::Unauthorized,
                },
                true,
            ))),
            "refresh failed (unauthorized)"
        );
        assert_eq!(
            provider_catalog_source_label(Some(&(CatalogStatus::Fresh, false))),
            "catalog from different endpoint"
        );
        assert_eq!(
            provider_catalog_source_label(None),
            "catalog freshness unknown"
        );
    }

    #[test]
    fn first_provider_catalog_failure_is_visible_on_bundled_fallback_rows() {
        let _env = crate::test_support::lock_test_env();
        let _live = crate::provider_lake::lock_live_snapshot();
        let home = tempfile::tempdir().expect("test home");
        let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", home.path());
        crate::provider_catalog_live::reset_cache_for_test();
        crate::provider_lake::clear_live_snapshot();

        let config = Config {
            provider: Some("openrouter".to_string()),
            ..Config::default()
        };
        let base_url = config.base_url_for_route_identity(ApiProvider::Openrouter, "openrouter");
        let fingerprint = codewhale_config::catalog::base_url_fingerprint(&base_url);
        crate::provider_catalog_live::record_failure(
            "openrouter",
            &fingerprint,
            CatalogRefreshError::Unauthorized,
        );

        let model = provider_catalog_model_ids(
            ApiProvider::Openrouter,
            ApiProvider::Openrouter.as_str(),
            crate::config::DEFAULT_OPENROUTER_BASE_URL,
        )
        .into_iter()
        .next()
        .expect("bundled OpenRouter fallback");
        let mut rows = Vec::new();
        let codex_roster = CodexModelRoster {
            models: Vec::new(),
            freshness: CodexModelCacheFreshness::Missing,
            fetched_at: None,
            observed_at: None,
            observation_persisted: false,
            source: "codex_cli_cache",
        };
        push_provider_model_rows(
            &mut rows,
            ApiProvider::Openrouter,
            None,
            vec![model],
            ApiProvider::Openrouter,
            &config,
            &codex_roster,
            &crate::provider_readiness::ProviderReadinessSnapshot::default(),
        );
        assert_eq!(rows.len(), 1);
        assert!(
            rows[0].hint.contains("refresh failed (unauthorized)"),
            "{}",
            rows[0].hint
        );

        crate::provider_catalog_live::reset_cache_for_test();
        crate::provider_lake::clear_live_snapshot();
    }

    #[test]
    fn same_model_on_distinct_custom_routes_keeps_readiness_and_applied_identity() {
        let _env = crate::test_support::lock_test_env();
        let _live = crate::provider_lake::lock_live_snapshot();
        let (config, app, home, _workspace) = resumed_openrouter_session_with_named_custom_routes();
        let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", home.path());
        crate::provider_catalog_live::reset_cache_for_test();
        crate::provider_lake::clear_live_snapshot();

        const MODEL: &str = "deepseek/deepseek-v4-flash";
        let mut picker = ModelPickerView::new(&app, &config);
        // The uncredentialed route is outside the default view (#6533); the
        // full catalog lists both.
        picker.view = ModelListView::Catalog;
        *picker.projection.get_mut() = None;
        let shared: Vec<(usize, Option<String>, bool)> = picker
            .visible_model_rows()
            .iter()
            .enumerate()
            .filter(|(_, row)| row.provider == Some(ApiProvider::Custom) && row.id == MODEL)
            .map(|(index, row)| (index, row.provider_identity.clone(), row.selectable))
            .collect();
        assert_eq!(
            shared.len(),
            2,
            "one shared model id on two routes must not collapse: {shared:?}"
        );

        for (index, identity, selectable) in shared {
            let identity = identity.expect("custom rows carry their exact route identity");
            // Each route is judged by its own table: only the credentialed one
            // can be attempted, even though neither is the selected provider.
            assert_eq!(
                selectable,
                identity == "command_code",
                "{identity} readiness must come from its own route"
            );
            picker.selected_model_idx = index;
            match picker.build_apply_event(false) {
                ViewEvent::ModelPickerApplied {
                    model,
                    provider,
                    provider_id,
                    ..
                } => {
                    assert_eq!(model, MODEL);
                    assert_eq!(provider, Some(ApiProvider::Custom));
                    assert_eq!(
                        provider_id.as_deref(),
                        Some(identity.as_str()),
                        "applying a row must switch to the route that row describes"
                    );
                }
                other => panic!("unexpected picker event: {other:?}"),
            }
        }

        crate::provider_catalog_live::reset_cache_for_test();
        crate::provider_lake::clear_live_snapshot();
    }

    #[test]
    fn model_rows_keep_case_distinct_custom_identities() {
        let mut rows = Vec::new();
        for identity in ["CustomA", "customa"] {
            push_model_row(
                &mut rows,
                "shared-model".to_string(),
                Some(ApiProvider::Custom),
                Some(identity.to_string()),
                String::new(),
                EffectivePickerMetadata::default(),
                true,
                None,
            );
        }
        assert_eq!(rows.len(), 2);
    }

    /// #6016 fixture: a session resumed on OpenRouter while two named custom
    /// routes are configured, neither of them the config's selected provider.
    fn resumed_openrouter_session_with_named_custom_routes()
    -> (Config, App, tempfile::TempDir, tempfile::TempDir) {
        let config: Config = toml::from_str(
            r#"
provider = "openrouter"

[providers.openrouter]
api_key = "fixture-openrouter-key"

[providers.command_code]
kind = "openai-compatible"
base_url = "https://command.example.test/v1"
model = "deepseek/deepseek-v4-flash"
api_key = "fixture-command-key"

# Same model id on a second route, deliberately without credentials: the two
# routes must stay separate rows with separate readiness.
[providers.other_code]
kind = "openai-compatible"
base_url = "https://other.example.test/v1"
model = "deepseek/deepseek-v4-flash"
"#,
        )
        .expect("named custom fixture");

        let home = tempfile::tempdir().expect("test home");
        let workspace = tempfile::tempdir().expect("test workspace");
        let options = crate::test_support::test_tui_options(workspace.path());
        let mut app = App::new(options, &config);
        // The resumed session stays on the provider it was created with.
        app.api_provider = ApiProvider::Openrouter;
        app.provider_identity = ApiProvider::Openrouter.as_str().to_string();
        app.provider_exact_id = None;
        app.model = "z-ai/glm-5.3".to_string();
        app.active_route_base_url = crate::config::DEFAULT_OPENROUTER_BASE_URL.to_string();
        (config, app, home, workspace)
    }

    #[test]
    fn resumed_session_lists_every_configured_custom_route_by_identity() {
        let _env = crate::test_support::lock_test_env();
        let _live = crate::provider_lake::lock_live_snapshot();
        let (config, app, home, _workspace) = resumed_openrouter_session_with_named_custom_routes();
        let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", home.path());
        crate::provider_catalog_live::reset_cache_for_test();
        crate::provider_lake::clear_live_snapshot();

        let rows = picker_model_rows_for_app(&app, &config);
        let custom: Vec<_> = rows
            .iter()
            .filter(|row| row.provider == Some(ApiProvider::Custom))
            .collect();
        let identities: Vec<_> = custom
            .iter()
            .filter_map(|row| row.provider_identity.as_deref())
            .collect();
        assert!(
            identities.contains(&"command_code"),
            "configured custom route B must stay visible in a resumed session: {custom:#?}"
        );
        assert!(
            identities.contains(&"other_code"),
            "every configured custom route keeps its own rows: {custom:#?}"
        );
        // A credentialed custom route offers its configured model by
        // default; one without credentials stays behind search (#6533).
        for (identity, visible) in [("command_code", true), ("other_code", false)] {
            let row = custom
                .iter()
                .find(|row| {
                    row.provider_identity.as_deref() == Some(identity)
                        && row.id == "deepseek/deepseek-v4-flash"
                })
                .unwrap_or_else(|| panic!("{identity} row missing: {custom:#?}"));
            assert_eq!(
                model_row_visible_by_default(row),
                visible,
                "{identity} default-view visibility: {row:#?}"
            );
        }

        crate::provider_catalog_live::reset_cache_for_test();
        crate::provider_lake::clear_live_snapshot();
    }

    #[test]
    fn shared_custom_model_opens_on_exact_active_route_despite_other_route_pin() {
        let _env = crate::test_support::lock_test_env();
        let _live = crate::provider_lake::lock_live_snapshot();
        let (mut config, mut app, home, _workspace) =
            resumed_openrouter_session_with_named_custom_routes();
        let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", home.path());
        crate::provider_catalog_live::reset_cache_for_test();
        crate::provider_lake::clear_live_snapshot();

        const MODEL: &str = "deepseek/deepseek-v4-flash";
        config.provider = Some("other_code".to_string());
        config
            .providers
            .as_mut()
            .unwrap()
            .custom
            .get_mut("other_code")
            .unwrap()
            .api_key = Some("fixture-other-key".to_string());
        app.set_provider_identity(ApiProvider::Custom, "other_code");
        app.set_model_selection(MODEL.to_string());
        app.active_route_base_url = "https://other.example.test/v1".to_string();
        app.pinned_models = vec![PinnedModel {
            provider: "command_code".to_string(),
            model: MODEL.to_string(),
            label: None,
        }];

        // Opening a picker must keep the current route even if a different
        // credentialed route exposing the same wire model sorts first.
        let mut picker = ModelPickerView::new(&app, &config);
        assert_eq!(
            picker.resolved_provider_identity().as_deref(),
            Some("other_code")
        );
        picker.ensure_projection();
        {
            let projection = picker.projection.borrow();
            let rows = &projection.as_ref().unwrap().rows;
            assert!(rows.iter().any(|row| row.route == "command_code"));
            assert!(rows.iter().any(|row| row.route == "other_code"));
            let active: Vec<_> = rows.iter().filter(|row| row.active).collect();
            assert_eq!(active.len(), 1);
            assert_eq!(active[0].route, "other_code");
        }
        let rendered = render_text(&picker, 140, 40);
        assert!(rendered.contains("command_code"), "{rendered}");
        assert!(rendered.contains("other_code"), "{rendered}");

        // Legacy memory has no route identity; ambiguity must preserve the
        // active route instead of resurrecting the first same-named model.
        picker.restore_memory(Some(&crate::tui::app::ModelPickerMemory {
            catalog_view: true,
            view: Some("catalog".to_string()),
            selected_row_id: Some(MODEL.to_string()),
        }));
        assert!(matches!(
            picker.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            ViewAction::EmitAndClose(ViewEvent::ModelPickerApplied {
                provider: None,
                provider_id: Some(identity),
                model,
                save_as_startup_default: false,
                ..
            }) if identity == "other_code" && model == MODEL
        ));
        assert!(matches!(
            picker.build_event_with_startup_default(true),
            ViewEvent::ModelPickerApplied {
                provider: None,
                provider_id: Some(identity),
                save_as_startup_default: true,
                ..
            } if identity == "other_code"
        ));
        crate::provider_catalog_live::reset_cache_for_test();
        crate::provider_lake::clear_live_snapshot();
    }

    #[test]
    fn locked_custom_model_names_exact_auth_route_without_opening_another_key_editor() {
        let mut picker = test_picker();
        let mut row = model_row(ApiProvider::Custom, true);
        row.provider_identity = Some("other_code".to_string());
        row.selectable = false;
        picker.model_rows = vec![row];
        assert!(matches!(
            picker.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            ViewAction::Emit(ViewEvent::StatusMessage { message })
                if message.contains("other_code/model")
                    && message.contains("Open /provider and select other_code")
        ));

        // Built-in providers retain their existing guided-auth handoff.
        let mut picker = test_picker();
        picker.model_rows[0].selectable = false;
        assert!(matches!(
            picker.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            ViewAction::Emit(ViewEvent::ModelPickerNeedsAuth {
                provider: ApiProvider::Openai,
                ..
            })
        ));
    }

    /// #6533 fixture shaped like the founder's state: a legacy
    /// `[enabled_models]` GLM-5.2, a leftover OpenRouter test model, recent
    /// use of models no config names, and a Fleet pin on a provider with no
    /// configured model.
    fn founder_shaped_app(home: &std::path::Path, workspace: &std::path::Path) -> (Config, App) {
        let settings_path = crate::settings::Settings::path().expect("settings path");
        std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        std::fs::write(
            &settings_path,
            "[enabled_models]\nzai = [\"GLM-5.2\", \"GLM-5.3\"]\n",
        )
        .unwrap();
        assert!(settings_path.starts_with(home));
        crate::fleet::members::add_fleet_model(
            workspace,
            "xai",
            "grok-4.6",
            &["reviewer".to_string()],
        )
        .expect("fleet add");
        let config: Config = toml::from_str(
            r#"
provider = "deepseek"

[providers.deepseek]
api_key = "fixture-deepseek-key"
model = "deepseek-flash"

[providers.openrouter]
api_key = "fixture-openrouter-key"
model = "stealth/ox-alpha"

[providers.zai]
api_key = "fixture-zai-key"
model = "GLM-5.3"

[providers.xai]
api_key = "fixture-xai-key"
"#,
        )
        .expect("founder fixture");
        let mut app = App::new(crate::test_support::test_tui_options(workspace), &config);
        app.workspace = workspace.to_path_buf();
        app.api_provider = ApiProvider::Deepseek;
        app.provider_identity = ApiProvider::Deepseek.as_str().to_string();
        app.provider_exact_id = None;
        app.auto_model = false;
        app.model = "deepseek-flash".to_string();
        app.provider_models
            .insert("deepseek".to_string(), "deepseek-flash".to_string());
        let now = chrono::Utc::now();
        let mut usage = app.route_usage.write().unwrap();
        usage.record("xai", "grok-4.7", now);
        usage.record("zai", "GLM-5.3", now - chrono::Duration::days(8));
        usage.record("deepseek", "deepseek-flash", now);
        drop(usage);
        (config, app)
    }

    fn default_view(picker: &ModelPickerView) -> Vec<(String, String, Option<DefaultSection>)> {
        picker
            .visible_model_rows()
            .iter()
            .filter(|row| row.provider.is_some())
            .map(|row| {
                (
                    row_provider_identity(row).unwrap_or_default().to_string(),
                    row.id.clone(),
                    row.default_rank.map(|(section, _)| section),
                )
            })
            .collect()
    }

    #[test]
    fn default_view_ranks_by_use_and_keeps_the_long_tail_behind_search() {
        std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn(|| {
                let _env = crate::test_support::lock_test_env();
                let _live = crate::provider_lake::lock_live_snapshot();
                let root = tempfile::tempdir().unwrap();
                let home = root.path().join("home");
                let workspace = root.path().join("repo");
                std::fs::create_dir_all(&workspace).unwrap();
                let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", &home);
                crate::provider_catalog_live::reset_cache_for_test();
                crate::provider_lake::clear_live_snapshot();
                let (config, app) = founder_shaped_app(&home, &workspace);

                let mut picker = ModelPickerView::new(&app, &config);
                assert_eq!(picker.view, ModelListView::Configured);
                let rows = default_view(&picker);
                let sections: Vec<_> = rows.iter().map(|(_, _, section)| *section).collect();
                let mut sorted = sections.clone();
                sorted.sort();
                assert_eq!(sections, sorted, "sections render in order: {rows:#?}");
                assert_eq!(
                    rows[0],
                    (
                        "deepseek".to_string(),
                        "deepseek-flash".to_string(),
                        Some(DefaultSection::Current)
                    ),
                    "{rows:#?}"
                );
                assert_eq!(
                    rows[1],
                    (
                        "xai".to_string(),
                        "grok-4.6".to_string(),
                        Some(DefaultSection::Pinned)
                    ),
                    "a Fleet pin shows although xai names no model: {rows:#?}"
                );
                let grok_46 = picker
                    .visible_model_rows()
                    .iter()
                    .find(|row| row.id == "grok-4.6")
                    .unwrap()
                    .hint
                    .clone();
                assert!(grok_46.starts_with("fleet · "), "{grok_46}");
                assert_eq!(
                    rows[2],
                    (
                        "xai".to_string(),
                        "grok-4.7".to_string(),
                        Some(DefaultSection::Recent)
                    ),
                    "{rows:#?}"
                );
                assert_eq!(
                    (rows[3].1.as_str(), rows[3].2),
                    ("GLM-5.3", Some(DefaultSection::Recent)),
                    "{rows:#?}"
                );
                for hidden in ["glm-5.2", "stealth/ox-alpha"] {
                    assert!(
                        !rows
                            .iter()
                            .any(|(_, id, _)| id.eq_ignore_ascii_case(hidden)),
                        "{hidden} has no use and must stay out of the default view: {rows:#?}"
                    );
                }
                for (identity, _, _) in &rows {
                    assert!(
                        rows.iter()
                            .filter(|(other, _, section)| other == identity
                                && *section == Some(DefaultSection::ProviderDefault))
                            .count()
                            <= 1,
                        "at most one default per provider: {rows:#?}"
                    );
                }
                // The active provider's roster stays one search away.
                assert!(
                    !rows.iter().any(|(_, id, _)| id == "deepseek-v4-pro"),
                    "{rows:#?}"
                );

                picker.update_query("GLM-5.2".to_string());
                assert!(
                    picker
                        .visible_model_rows()
                        .iter()
                        .any(|row| row.provider == Some(ApiProvider::Zai)
                            && row.id.eq_ignore_ascii_case("GLM-5.2")),
                    "search still finds an unused model"
                );
                picker.update_query("ox-alpha".to_string());
                let ox = picker
                    .visible_model_rows()
                    .iter()
                    .find(|row| row.id == "stealth/ox-alpha")
                    .cloned()
                    .expect("leftover config model is searchable");
                assert_eq!(ox.not_listed, None, "no roster, no verdict");

                // The Fleet picker shares the builder: the edited row leads,
                // then pins, recent use and defaults in the same order.
                let fleet = ModelPickerView::new_for_fleet_route(
                    &app,
                    &config,
                    FleetRouteTarget::Member(0),
                    uuid::Uuid::nil(),
                    FleetRouteSelection {
                        provider: Some("xai".to_string()),
                        model: Some("grok-4.6".to_string()),
                        reasoning: None,
                        allow_inherit: true,
                    },
                );
                let fleet_rows = default_view(&fleet);
                let sections: Vec<_> = fleet_rows.iter().map(|(_, _, section)| *section).collect();
                let mut sorted = sections.clone();
                sorted.sort();
                assert_eq!(sections, sorted, "{fleet_rows:#?}");
                assert_eq!(
                    fleet_rows[0],
                    (
                        "xai".to_string(),
                        "grok-4.6".to_string(),
                        Some(DefaultSection::Current)
                    ),
                    "{fleet_rows:#?}"
                );
                let recent = |rows: &[(String, String, Option<DefaultSection>)]| {
                    rows.iter()
                        .filter(|(_, _, section)| *section == Some(DefaultSection::Recent))
                        .map(|(identity, id, _)| (identity.clone(), id.clone()))
                        .collect::<Vec<_>>()
                };
                assert_eq!(
                    recent(&fleet_rows),
                    [
                        ("deepseek".to_string(), "deepseek-flash".to_string()),
                        ("xai".to_string(), "grok-4.7".to_string()),
                        ("zai".to_string(), "GLM-5.3".to_string()),
                    ],
                    "the session's own route is ordinary recent use here: {fleet_rows:#?}"
                );

                crate::provider_catalog_live::reset_cache_for_test();
                crate::provider_lake::clear_live_snapshot();
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn not_listed_needs_a_fresh_roster_and_never_hides_the_row() {
        std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn(|| {
                let _env = crate::test_support::lock_test_env();
                let _live = crate::provider_lake::lock_live_snapshot();
                let root = tempfile::tempdir().unwrap();
                let home = root.path().join("home");
                let workspace = root.path().join("repo");
                std::fs::create_dir_all(&workspace).unwrap();
                let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", &home);
                crate::provider_catalog_live::reset_cache_for_test();
                crate::provider_lake::clear_live_snapshot();
                let (config, mut app) = founder_shaped_app(&home, &workspace);
                // OpenRouter was used recently, so its leftover model is its
                // default row and reaches the default view.
                app.note_route_used("openrouter", "stealth/ox-alpha");

                let roster = |provider: ApiProvider, ids: &[&str], fetched_at: u64| {
                    let base_url = config.base_url_for_route_identity(provider, provider.as_str());
                    let fingerprint = codewhale_config::catalog::base_url_fingerprint(&base_url);
                    let _ = crate::provider_catalog_live::record_success(
                        codewhale_config::catalog::ProviderCatalogDelta {
                            provider: provider.as_str().to_string(),
                            base_url_fingerprint: fingerprint.clone(),
                            fetched_at,
                            offerings: ids
                                .iter()
                                .map(|id| codewhale_config::catalog::CatalogOffering {
                                    provider: provider.as_str().to_string(),
                                    wire_model_id: (*id).to_string(),
                                    endpoint_key: "chat".to_string(),
                                    source: CatalogSource::Live {
                                        base_url_fingerprint: fingerprint.clone(),
                                        fetched_at,
                                    },
                                    ..Default::default()
                                })
                                .collect(),
                        },
                    );
                    crate::provider_catalog_live::status_for_route(
                        provider,
                        provider.as_str(),
                        &base_url,
                    )
                };
                let now = u64::try_from(chrono::Utc::now().timestamp()).unwrap();
                // DeepSeek's roster is 15 days old and lacks the current model.
                assert!(matches!(
                    roster(
                        ApiProvider::Deepseek,
                        &["deepseek-v4-flash", "deepseek-v4-pro"],
                        now - 15 * 86_400
                    ),
                    CatalogStatus::Stale { .. }
                ));
                let picker = ModelPickerView::new(&app, &config);
                let rows = picker.visible_model_rows();
                let flash = rows.iter().find(|row| row.id == "deepseek-flash").unwrap();
                assert_eq!(flash.not_listed, None, "a stale roster gives no verdict");
                let ox = rows
                    .iter()
                    .find(|row| row.id == "stealth/ox-alpha")
                    .expect("recently used route's model is in the default view");
                assert_eq!(ox.not_listed, None, "no OpenRouter roster yet");
                drop(rows);

                assert_eq!(
                    roster(ApiProvider::Openrouter, &["openai/gpt-5"], now),
                    CatalogStatus::Fresh
                );
                let picker = ModelPickerView::new(&app, &config);
                let rows = picker.visible_model_rows();
                let ox = rows
                    .iter()
                    .find(|row| row.id == "stealth/ox-alpha")
                    .expect("a not-listed row is dimmed, never hidden");
                let checked = chrono::Utc::now().format("%Y-%m-%d").to_string();
                assert_eq!(
                    ox.not_listed,
                    Some(NotListed {
                        provider: ApiProvider::Openrouter.display_name().to_string(),
                        checked: checked.clone(),
                    })
                );
                assert!(ox.selectable, "still selectable");
                drop(rows);
                let text = render_text(&picker, 160, 40);
                assert!(
                    text.contains(&format!("(checked {checked})")),
                    "the reason is rendered: {text}"
                );
                assert_eq!(
                    config
                        .provider_config_for(ApiProvider::Openrouter)
                        .and_then(|entry| entry.model.as_deref()),
                    Some("stealth/ox-alpha"),
                    "config is never rewritten"
                );

                crate::provider_catalog_live::reset_cache_for_test();
                crate::provider_lake::clear_live_snapshot();
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
