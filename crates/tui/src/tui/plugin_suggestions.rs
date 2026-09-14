//! In-context plugin reminders: prompt matching, live composer CTA, and idle
//! catalog polling.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Widget};
use unicode_width::UnicodeWidthStr;

use crate::plugins::recommend::{
    PluginNextStep, load_marketplace_candidates, match_plugin_for_draft,
};
use crate::tui::app::{App, StatusToast, StatusToastKind, StatusToastLevel};
use codewhale_localization::{MessageId, tr};

const CATALOG_POLL_INTERVAL: Duration = Duration::from_secs(2);
const CTA_DEBOUNCE: Duration = Duration::from_millis(200);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginCtaPhase {
    Hidden,
    Matched { name: String, command: String },
}

impl PluginCtaPhase {
    #[must_use]
    pub fn is_visible(&self) -> bool {
        matches!(self, Self::Matched { .. })
    }

    #[must_use]
    pub fn matched_name(&self) -> Option<&str> {
        match self {
            Self::Hidden => None,
            Self::Matched { name, .. } => Some(name.as_str()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PluginCtaState {
    pub phase: PluginCtaPhase,
    pub dismissed: BTreeSet<String>,
    matched_term: Option<String>,
    debounce_at: Option<Instant>,
    last_draft: String,
}

impl Default for PluginCtaState {
    fn default() -> Self {
        Self {
            phase: PluginCtaPhase::Hidden,
            dismissed: BTreeSet::new(),
            matched_term: None,
            debounce_at: None,
            last_draft: String::new(),
        }
    }
}

impl PluginCtaState {
    pub(crate) fn from_settings(settings: &crate::settings::Settings) -> Self {
        Self {
            dismissed: settings
                .dismissed_plugin_suggestions
                .iter()
                .map(|name| name.to_ascii_lowercase())
                .collect(),
            ..Self::default()
        }
    }
}

impl App {
    /// When the user sends a task that matches an installed-but-idle plugin
    /// or a locally added marketplace candidate, toast the next review step
    /// once. Never installs, trusts, or enables anything.
    pub fn maybe_nudge_plugin_for_prompt(&mut self, input: &str) -> bool {
        if !self.behavioral_tips.guidance_available() {
            return false;
        }
        let marketplace = load_marketplace_candidates(self.plugin_registry.state_path());
        let Some(recommendation) = match_plugin_for_draft(
            input,
            self.plugin_registry.as_ref(),
            &marketplace,
            &self.plugin_cta.dismissed,
        ) else {
            return false;
        };
        let message_id = match recommendation.next_step {
            PluginNextStep::Trust => MessageId::PluginPromptSuggestTrust,
            PluginNextStep::Enable => MessageId::PluginPromptSuggestEnable,
            PluginNextStep::MarketplaceInstall { .. } => MessageId::PluginPromptSuggestMarketplace,
            PluginNextStep::AlreadyActive
            | PluginNextStep::Inspect
            | PluginNextStep::SourceInstall { .. } => return false,
        };
        let mut message = tr(self.ui_locale, message_id).replace("{name}", &recommendation.name);
        if let PluginNextStep::MarketplaceInstall { catalog_id } = &recommendation.next_step {
            message = message.replace("{catalog}", catalog_id);
        }
        if let Some(term) = recommendation.matched_term {
            message.push_str(" · ");
            message.push_str(
                &tr(self.ui_locale, MessageId::PluginSuggestionReason).replace("{trigger}", &term),
            );
        }
        self.behavioral_tips.record_guidance_impression();
        let mut toast = StatusToast::new(message, StatusToastLevel::Info, Some(8_000));
        toast.kind = StatusToastKind::PluginSuggestion;
        self.push_status_toast_record(toast);
        true
    }

    /// Cheap idle poll so on-disk plugin changes can surface between turns,
    /// not only on send. Fingerprints directories; never auto-reloads.
    pub fn maybe_poll_plugin_catalog_idle(&mut self) {
        let now = Instant::now();
        if self
            .last_plugin_catalog_poll
            .is_some_and(|seen| now.duration_since(seen) < CATALOG_POLL_INTERVAL)
        {
            return;
        }
        self.last_plugin_catalog_poll = Some(now);
        if let Some(message) = crate::plugins::plugin_reload_nudge(
            self.plugin_registry.as_ref(),
            &mut self.plugin_reload_nudge_stamp,
        ) {
            self.push_status_toast(message, StatusToastLevel::Warning, Some(8_000));
            self.needs_redraw = true;
        }
    }

    /// Arm a short debounce whenever the composer draft changes.
    pub fn notify_plugin_cta_text_changed(&mut self) {
        if self.input == self.plugin_cta.last_draft {
            return;
        }
        self.plugin_cta.last_draft = self.input.clone();
        self.plugin_cta.debounce_at = Some(Instant::now() + CTA_DEBOUNCE);
    }

    /// Recompute the live CTA after the debounce window. One match at a
    /// time; already-active plugins stay hidden; a dismissed name stays
    /// dismissed across sessions. Never auto-installs.
    pub fn handle_plugin_cta_debounce_expired(&mut self) {
        self.plugin_cta.debounce_at = None;
        self.plugin_cta.last_draft = self.input.clone();
        let marketplace = load_marketplace_candidates(self.plugin_registry.state_path());
        let Some(matched) = match_plugin_for_draft(
            &self.input,
            self.plugin_registry.as_ref(),
            &marketplace,
            &self.plugin_cta.dismissed,
        ) else {
            if self.plugin_cta.phase.is_visible() {
                self.plugin_cta.phase = PluginCtaPhase::Hidden;
                self.needs_redraw = true;
            }
            return;
        };
        let command = matched.command();
        let new_phase = PluginCtaPhase::Matched {
            name: matched.name,
            command,
        };
        if self.plugin_cta.phase != new_phase
            || self.plugin_cta.matched_term != matched.matched_term
        {
            self.plugin_cta.matched_term = matched.matched_term;
            self.plugin_cta.phase = new_phase;
            self.needs_redraw = true;
        }
    }

    /// Poll draft changes and fire the CTA debounce without a dedicated timer
    /// task. The event loop already ticks this often.
    pub fn maybe_poll_plugin_cta(&mut self) {
        self.notify_plugin_cta_text_changed();
        let Some(at) = self.plugin_cta.debounce_at else {
            return;
        };
        if Instant::now() < at {
            return;
        }
        self.handle_plugin_cta_debounce_expired();
    }

    #[must_use]
    pub fn plugin_cta_row_height(&self) -> u16 {
        u16::from(self.plugin_cta.phase.is_visible())
    }

    /// Persist an explicit dismissal while hiding it immediately this session.
    pub fn dismiss_plugin_cta(&mut self) -> bool {
        let Some(name) = self.plugin_cta.phase.matched_name().map(str::to_string) else {
            return false;
        };
        let name = name.to_ascii_lowercase();
        self.plugin_cta.dismissed.insert(name.clone());
        self.plugin_cta.phase = PluginCtaPhase::Hidden;
        self.needs_redraw = true;
        if let Err(error) = crate::settings::Settings::transact_opt(|settings| {
            Ok(settings
                .dismissed_plugin_suggestions
                .insert(name)
                .then_some(()))
        }) {
            tracing::warn!(%error, "could not persist plugin suggestion dismissal");
            self.push_status_toast(
                tr(self.ui_locale, MessageId::PluginCtaDismissSaveFailed).into_owned(),
                StatusToastLevel::Warning,
                Some(8_000),
            );
        }
        true
    }

    /// Human-initiated review: return the slash command so the TUI can run
    /// the existing `/plugin trust` / marketplace-install / `/plugin install`
    /// path. Never runs it here.
    #[must_use]
    pub fn accept_plugin_cta_command(&mut self) -> Option<String> {
        let (command, name) = match &self.plugin_cta.phase {
            PluginCtaPhase::Matched { command, name } => (command.clone(), name.clone()),
            PluginCtaPhase::Hidden => return None,
        };
        self.plugin_cta.dismissed.insert(name.to_ascii_lowercase());
        self.plugin_cta.phase = PluginCtaPhase::Hidden;
        self.needs_redraw = true;
        Some(command)
    }

    /// Model-requested review: show the live CTA and a toast. Does not run
    /// the command, so nothing is installed, trusted, or enabled.
    pub fn surface_plugin_review_request(&mut self, name: &str, command: &str) {
        if name.trim().is_empty()
            || command.trim().is_empty()
            || self
                .plugin_cta
                .dismissed
                .contains(&name.to_ascii_lowercase())
        {
            return;
        }
        self.plugin_cta.matched_term = None;
        self.plugin_cta.phase = PluginCtaPhase::Matched {
            name: name.to_string(),
            command: command.to_string(),
        };
        self.push_status_toast(command.to_string(), StatusToastLevel::Info, Some(8_000));
        self.needs_redraw = true;
    }
}

/// Draw the one-line live CTA above the composer. No-op when hidden.
pub fn draw_plugin_cta(app: &mut App, area: Rect, buf: &mut Buffer) {
    app.viewport.last_plugin_cta_area = None;
    app.viewport.last_plugin_cta_review_area = None;
    app.viewport.last_plugin_cta_dismiss_area = None;
    let PluginCtaPhase::Matched { name, .. } = &app.plugin_cta.phase else {
        return;
    };
    let name = name.clone();
    if area.height == 0 || area.width == 0 {
        return;
    }
    let mut prompt = tr(app.ui_locale, MessageId::PluginCtaInstallPrompt).replace("{name}", &name);
    if let Some(term) = &app.plugin_cta.matched_term {
        prompt.push_str(" · ");
        prompt.push_str(
            &tr(app.ui_locale, MessageId::PluginSuggestionReason).replace("{trigger}", term),
        );
    }
    let review = tr(app.ui_locale, MessageId::PluginCtaReview);
    let dismiss = tr(app.ui_locale, MessageId::PluginCtaDismiss);
    let review_label = format!("[{review}]");
    let dismiss_label = format!("[{dismiss}]");
    let review_w = review_label.width() as u16;
    let dismiss_w = dismiss_label.width() as u16;
    let gap = 1u16;
    let right_w = review_w.saturating_add(gap).saturating_add(dismiss_w);
    let bg = Style::default().bg(app.ui_theme.composer_bg);
    Block::default().style(bg).render(area, buf);
    let left_budget = if area.width > right_w.saturating_add(1) {
        area.width - right_w - 1
    } else {
        area.width
    };
    let left = Line::from(vec![Span::styled(
        prompt,
        Style::default().fg(app.ui_theme.text_hint),
    )]);
    buf.set_line(area.x, area.y, &left, left_budget);
    if area.width <= right_w {
        app.viewport.last_plugin_cta_area = Some(area);
        return;
    }
    let review_x = area.x + area.width - right_w;
    let dismiss_x = review_x + review_w + gap;
    buf.set_stringn(
        review_x,
        area.y,
        &review_label,
        usize::from(review_w),
        Style::default().fg(app.ui_theme.accent_action),
    );
    buf.set_stringn(
        dismiss_x,
        area.y,
        &dismiss_label,
        usize::from(dismiss_w),
        Style::default().fg(app.ui_theme.text_hint),
    );
    app.viewport.last_plugin_cta_area = Some(area);
    app.viewport.last_plugin_cta_review_area = Some(Rect::new(review_x, area.y, review_w, 1));
    app.viewport.last_plugin_cta_dismiss_area = Some(Rect::new(dismiss_x, area.y, dismiss_w, 1));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::tui::app::TuiOptions;
    use codewhale_localization::Locale;
    use std::fs;
    use tempfile::TempDir;

    fn app_with_supabase_plugin() -> (App, TempDir, crate::test_support::EnvVarGuard) {
        let root = TempDir::new().unwrap();
        let home =
            crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", root.path().join("home"));
        let bundle = root.path().join(".codewhale/plugins/supabase");
        fs::create_dir_all(&bundle).unwrap();
        fs::write(
            bundle.join("plugin.toml"),
            "schema_version = 1\n[plugin]\nname = \"supabase\"\nversion = \"1.0.0\"\ndescription = \"Hosted Postgres and auth\"\nkeywords = [\"supabase\"]\n",
        )
        .unwrap();
        let temp = TempDir::new().unwrap();
        let options = TuiOptions {
            config_path: Some(temp.path().join("config.toml")),
            skills_dir: temp.path().join("skills"),
            memory_path: temp.path().join("memory.md"),
            notes_path: temp.path().join("notes.txt"),
            mcp_config_path: temp.path().join("mcp.json"),
            ..crate::test_support::test_tui_options(root.path())
        };
        let discovery = crate::plugins::PluginDiscoveryContext::capture_pre_dotenv();
        let registry = discovery.registry_for_workspace(root.path());
        let mut app = App::new_with_plugin_registry(options, &Config::default(), registry);
        app.ui_locale = Locale::En;
        (app, root, home)
    }

    #[test]
    fn sending_a_supabase_prompt_toasts_trust_for_an_installed_idle_plugin() {
        let _lock = crate::test_support::lock_test_env();
        let (mut app, _root, _home) = app_with_supabase_plugin();

        assert!(app.maybe_nudge_plugin_for_prompt("add supabase auth to login"));
        assert_eq!(app.status_toasts.len(), 1);
        assert!(
            app.status_toasts[0].text.contains("/plugin trust supabase"),
            "{}",
            app.status_toasts[0].text
        );
        assert!(!app.maybe_nudge_plugin_for_prompt("add supabase auth to login"));
    }

    #[test]
    fn optional_plugin_and_behavioral_guidance_share_one_session_budget() {
        use crate::tui::behavioral_tips::BehavioralTip;
        let _lock = crate::test_support::lock_test_env();
        for plugin_first in [true, false] {
            let (mut app, _root, _home) = app_with_supabase_plugin();
            if plugin_first {
                assert!(app.maybe_nudge_plugin_for_prompt("add supabase auth"));
                assert!(!app.maybe_show_behavioral_tip(BehavioralTip::PlanningMode));
            } else {
                assert!(app.maybe_show_behavioral_tip(BehavioralTip::PlanningMode));
                assert!(!app.maybe_nudge_plugin_for_prompt("add supabase auth"));
            }
            assert_eq!(app.status_toasts.len(), 1);
        }
    }

    #[test]
    fn tips_off_removes_plugin_guidance_but_preserves_required_notices_and_explicit_review() {
        let _lock = crate::test_support::lock_test_env();
        let (mut app, _root, _home) = app_with_supabase_plugin();
        app.set_contextual_tips_enabled(false);
        assert!(!app.maybe_nudge_plugin_for_prompt("add supabase auth"));
        app.set_contextual_tips_enabled(true);
        assert!(app.maybe_nudge_plugin_for_prompt("add supabase auth"));
        app.push_status_toast_record(
            StatusToast::new("Review required", StatusToastLevel::Warning, None).for_action("a"),
        );
        app.push_status_toast("Keep this error", StatusToastLevel::Error, None);
        app.set_contextual_tips_enabled(false);
        assert_eq!(app.status_toasts.len(), 2);
        assert!(
            app.status_toasts
                .iter()
                .all(|toast| toast.kind != StatusToastKind::PluginSuggestion)
        );
        app.surface_plugin_review_request("supabase", "/plugin trust supabase");
        assert!(app.plugin_cta.phase.is_visible());
        assert_eq!(
            app.status_toasts.len(),
            3,
            "explicit review is not unsolicited guidance"
        );
        app.set_contextual_tips_enabled(true);
        assert!(
            !app.maybe_nudge_plugin_for_prompt("add supabase auth"),
            "reenabling must not reset the shared cap"
        );
    }

    #[test]
    fn live_cta_shows_for_a_matching_idle_plugin() {
        let _lock = crate::test_support::lock_test_env();
        let (mut app, _root, _home) = app_with_supabase_plugin();
        app.input = "add supabase auth to login".to_string();
        app.handle_plugin_cta_debounce_expired();
        assert_eq!(
            app.plugin_cta.phase.matched_name(),
            Some("supabase"),
            "{:?}",
            app.plugin_cta.phase
        );
        assert_eq!(app.plugin_cta_row_height(), 1);
        assert_eq!(app.plugin_cta.matched_term.as_deref(), Some("supabase"));
        let area = Rect::new(0, 0, 140, 1);
        let mut buffer = Buffer::empty(area);
        draw_plugin_cta(&mut app, area, &mut buffer);
        let row = buffer
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(row.contains("Matched “supabase”"), "{row}");
    }

    #[test]
    fn live_cta_hides_when_the_plugin_is_already_active() {
        let _lock = crate::test_support::lock_test_env();
        let (mut app, _root, _home) = app_with_supabase_plugin();
        let registry = std::sync::Arc::make_mut(&mut app.plugin_registry);
        registry.trust("supabase").unwrap();
        registry.enable("supabase").unwrap();
        app.input = "add supabase auth to login".to_string();
        app.handle_plugin_cta_debounce_expired();
        assert!(
            !app.plugin_cta.phase.is_visible(),
            "{:?}",
            app.plugin_cta.phase
        );
    }

    #[test]
    fn live_cta_dismiss_stays_dismissed_for_that_name_this_session() {
        let _lock = crate::test_support::lock_test_env();
        let (mut app, _root, _home) = app_with_supabase_plugin();
        app.input = "add supabase auth to login".to_string();
        app.handle_plugin_cta_debounce_expired();
        assert!(app.dismiss_plugin_cta());
        assert!(!app.plugin_cta.phase.is_visible());
        app.handle_plugin_cta_debounce_expired();
        assert!(
            !app.plugin_cta.phase.is_visible(),
            "dismissed names must not reappear this session: {:?}",
            app.plugin_cta.phase
        );
    }

    #[test]
    fn dismissal_survives_restart_and_all_proactive_paths_preserving_settings() {
        use crate::settings::Settings;
        let _lock = crate::test_support::lock_test_env();
        let (mut app, root, _home) = app_with_supabase_plugin();
        Settings::transact(|settings| settings.set("max_history", "321")).unwrap();
        app.input = "add supabase auth to login".into();
        app.handle_plugin_cta_debounce_expired();
        assert!(app.dismiss_plugin_cta());
        let saved = Settings::load_read_only().unwrap();
        assert_eq!(saved.max_input_history, 321);
        assert!(saved.dismissed_plugin_suggestions.contains("supabase"));
        // A freshly initialized App must hydrate the persisted preference.
        let mut restarted = App::new_with_plugin_registry(
            crate::test_support::test_tui_options(root.path()),
            &Config::default(),
            app.plugin_registry.clone(),
        );
        restarted.input = app.input.clone();
        restarted.handle_plugin_cta_debounce_expired();
        assert!(!restarted.plugin_cta.phase.is_visible());
        assert!(!restarted.maybe_nudge_plugin_for_prompt(&app.input));
        restarted.surface_plugin_review_request("supabase", "/plugin trust supabase");
        assert!(!restarted.plugin_cta.phase.is_visible());
        assert!(
            crate::plugins::recommend::recommended_plugins_user_fragment(
                &app.input,
                restarted.plugin_registry.as_ref(),
                &[],
            )
            .is_none()
        );
        assert!(
            crate::plugins::recommend::lookup_reviewable_plugin(
                "supabase",
                restarted.plugin_registry.as_ref(),
                &[],
            )
            .is_some(),
            "manual plugin commands remain available"
        );
    }

    #[test]
    fn failed_dismissal_save_preserves_malformed_preferences_and_hides_this_session() {
        let _lock = crate::test_support::lock_test_env();
        let (mut app, root, _home) = app_with_supabase_plugin();
        app.input = "add supabase auth".into();
        app.handle_plugin_cta_debounce_expired();
        let path = crate::settings::Settings::path().unwrap();
        assert!(path.starts_with(root.path()));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let malformed = "theme = [private_fixture_payload\n";
        fs::write(&path, malformed).unwrap();
        assert!(app.dismiss_plugin_cta());
        assert_eq!(fs::read_to_string(&path).unwrap(), malformed);
        app.handle_plugin_cta_debounce_expired();
        assert!(!app.plugin_cta.phase.is_visible());
        assert!(!app.maybe_nudge_plugin_for_prompt("add supabase auth"));
        let toast = app.status_toasts.back().expect("save failure receipt");
        assert!(toast.text.contains("could not save"));
        assert!(!toast.text.contains("private_fixture_payload"));
    }
}
