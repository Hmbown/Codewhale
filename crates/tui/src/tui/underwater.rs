//! Coherent shell grammar for the underwater TUI.
//!
//! This module owns phase, responsive density, the empty-state composition,
//! and the compact header/footer fact budget. Product data still belongs to
//! [`App`]; this is only its terminal projection. Keeping these decisions in
//! one place prevents the default UI from drifting back into a header +
//! sidebar + dashboard + footer composition with four owners for one fact.

use crate::tui::mark::MarkSize;
use std::borrow::Cow;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Widget},
};
use unicode_width::UnicodeWidthStr;

use crate::config::HeaderItem;
use crate::tui::ui_text::{semantic_truncate, text_display_width};
use crate::tui::{
    app::{App, HeaderActionTarget, HeaderHitbox, OnboardingState},
    footer_ui::format_token_count_compact,
    ocean::COMPLETION_BREATH_MS,
    views::ModalKind,
};
use codewhale_config::AppMode;
use codewhale_execpolicy::ApprovalMode;
use codewhale_localization::{Locale, MessageId, tr};
use codewhale_palette::{ChromeInk, chrome_style};

/// Responsive density tier. It changes how much truth is shown, never the
/// underlying state grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellTier {
    Compact,
    Normal,
    Wide,
}

/// What one launch key produces. The composer holds focus and takes every
/// ordinary key, so the only launch-owned input is F1 help; the card's
/// rows are driven by Up/Down + Enter (and the mouse) through
/// [`run_launch_card_row`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchAction {
    None,
    /// The prominent new-session entry: begin a fresh session in the
    /// current workspace.
    NewSession,
    /// Resume one recent-work row by session id.
    ResumeSession(String),
    /// The see-all overflow: open the full session picker.
    BrowseSessions,
    Help,
}

/// Translate a launch key into one product action. Reached only through
/// [`LaunchComposerKey::MenuChord`]; every other key belongs to the
/// composer authority.
pub fn handle_launch_key(
    _launch: &mut crate::tui::app::LaunchState,
    key: KeyEvent,
    _locale: Locale,
) -> LaunchAction {
    match key.code {
        KeyCode::F(1) => LaunchAction::Help,
        _ => LaunchAction::None,
    }
}

/// One interactive row on the startup card: the prominent new-session
/// entry, one recent-work row, or the see-all overflow. Labels are
/// localized; `detail` is right-aligned metadata (a recent row's age).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchCardRow {
    pub id: crate::tui::app::LaunchRowId,
    pub label: String,
    pub detail: String,
    /// The new-session entry paints prominent (bold accent) when it is
    /// neither keyboard-selected nor hovered.
    pub prominent: bool,
}

/// A recent session projected for the card: the display title plus its
/// right-aligned detail line. Preformatted by the caller so the renderer
/// stays deterministic for golden buffers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchRecentEntry {
    pub id: String,
    pub title: String,
    pub detail: String,
}

/// The card's rows in paint/click/keyboard order: the prominent
/// new-session entry first, then recent work, then the see-all overflow
/// when more sessions sit behind the inline list. The single ordering
/// keyboard, mouse, and paint share.
#[must_use]
pub fn launch_card_rows(
    locale: Locale,
    recent: &[LaunchRecentEntry],
    has_more: bool,
) -> Vec<LaunchCardRow> {
    let mut rows = Vec::with_capacity(recent.len() + 2);
    rows.push(LaunchCardRow {
        id: crate::tui::app::LaunchRowId::NewSession,
        label: tr(locale, MessageId::LaunchNewSession).into_owned(),
        detail: String::new(),
        prominent: true,
    });
    rows.extend(recent.iter().map(|entry| LaunchCardRow {
        id: crate::tui::app::LaunchRowId::Recent(entry.id.clone()),
        label: entry.title.clone(),
        detail: entry.detail.clone(),
        prominent: false,
    }));
    if has_more {
        rows.push(LaunchCardRow {
            id: crate::tui::app::LaunchRowId::SeeAll,
            label: tr(locale, MessageId::LaunchSeeAllSessions).into_owned(),
            detail: String::new(),
            prominent: false,
        });
    }
    rows
}

/// Project the launch state's loaded recent-work list into card entries:
/// display titles with right-aligned relative ages, like the resume
/// picker. Pure projection of loaded state — no disk reads.
fn launch_recent_entries(app: &App) -> (Vec<LaunchRecentEntry>, bool) {
    let recent = app
        .launch
        .recent
        .iter()
        .map(|session| {
            let raw = crate::session_manager::extract_title(&session.title);
            let title = if raw == "Session" || raw.trim().is_empty() {
                crate::session_manager::truncate_id(&session.id).to_string()
            } else {
                raw.to_string()
            };
            let age = crate::tui::session_picker::format_relative_time(
                &session.updated_at,
                app.ui_locale,
            );
            let count = tr(app.ui_locale, MessageId::SessionsMessageCountCompact)
                .replace("{count}", &session.message_count.to_string());
            LaunchRecentEntry {
                id: session.id.clone(),
                title,
                detail: format!("{age} · {count}"),
            }
        })
        .collect::<Vec<_>>();
    // More sessions than the inline cap, or sessions the card's filter
    // dropped that `/resume` still lists (empty auto-created shells):
    // either way the see-all row is how the truth stays reachable.
    let has_more = app.launch.total_workspace_sessions > recent.len()
        || (recent.is_empty() && app.launch.has_scoped_sessions);
    (recent, has_more)
}

/// The card's rows for live `App` state, for keyboard navigation and Enter.
///
/// The painted rows are the authority. Paint sheds the tail of the recent
/// list to fit a short pane and turns the overflow row on when it does, so a
/// list built here from scratch would let Up/Down land on — and Enter resume
/// — a session the screen is not showing. `row_hitboxes` is what the last
/// frame actually drew, in paint order, and `mouse_ui` indexes that same
/// list: one ordering for paint, mouse, and keyboard, with no second state.
#[must_use]
pub fn launch_rows_for_app(app: &App) -> Vec<LaunchCardRow> {
    let (recent, has_more) = launch_recent_entries(app);
    if app.launch.row_hitboxes.is_empty() {
        // Nothing painted yet (first frame): nothing is selected either.
        return launch_card_rows(app.ui_locale, &recent, has_more);
    }
    let superset = launch_card_rows(app.ui_locale, &recent, true);
    app.launch
        .row_hitboxes
        .iter()
        .filter_map(|(id, _)| superset.iter().find(|row| &row.id == id).cloned())
        .collect()
}

/// Re-anchor the card's clickable rows on what `area` just painted.
///
/// The frame renderer calls this instead of rebuilding hitboxes inline, so
/// the row list keyboard and mouse read back cannot describe a row the
/// transcript did not draw.
pub fn refresh_launch_row_hitboxes(app: &mut App, area: Rect) {
    app.launch.row_hitboxes = launch_empty_state(app, area)
        .rows
        .into_iter()
        .filter_map(|(id, row)| {
            let y = area.y.checked_add(u16::try_from(row).ok()?)?;
            (y < area.y.saturating_add(area.height))
                .then_some((id, Rect::new(area.x, y, area.width, 1)))
        })
        .collect();
    // A pane that shrank can leave the highlight past the last painted row.
    // Clear it rather than clamping: clamping would silently move the
    // selection onto a different session.
    let painted = app.launch.row_hitboxes.len();
    if app
        .launch
        .menu_selected
        .is_some_and(|index| index >= painted)
    {
        app.launch.menu_selected = None;
    }
    if app.launch.hovered_row.is_some_and(|index| index >= painted) {
        app.launch.hovered_row = None;
    }
}

/// The click twin of [`run_launch_card_row`]: one card row id runs the
/// same action the keyboard's Enter runs, so mouse and keyboard share one
/// contract.
#[must_use]
pub fn launch_row_click_action(id: &crate::tui::app::LaunchRowId) -> LaunchAction {
    match id {
        crate::tui::app::LaunchRowId::NewSession => LaunchAction::NewSession,
        crate::tui::app::LaunchRowId::Recent(session_id) => {
            LaunchAction::ResumeSession(session_id.clone())
        }
        crate::tui::app::LaunchRowId::SeeAll => LaunchAction::BrowseSessions,
    }
}

/// Ask before resuming: open the confirmation popup for `session_id`.
///
/// Both the card's Enter and a click on a recent row route here. Resuming
/// replaces the whole session context, and the popup is where that is said —
/// an arming line over the composer read as chrome rather than as a question.
pub fn open_launch_resume_confirm(app: &mut App, session_id: &str) {
    if app.view_stack.top_kind() == Some(crate::tui::views::ModalKind::LaunchResumeConfirm) {
        return;
    }
    let entry = app
        .launch
        .recent
        .iter()
        .find(|entry| entry.id == session_id);
    let title = entry
        .map(|entry| entry.title.clone())
        .unwrap_or_else(|| session_id.to_string());
    let detail = entry
        .map(|entry| {
            let when =
                crate::tui::session_picker::format_relative_time(&entry.updated_at, app.ui_locale);
            format!("{when} · {} msgs", entry.message_count)
        })
        .unwrap_or_default();
    app.view_stack.push(
        crate::tui::launch_resume_confirm::LaunchResumeConfirmView::new(
            session_id.to_string(),
            title,
            detail,
            app.ui_locale,
        ),
    );
    app.needs_redraw = true;
}
/// Run the card's highlighted row. Enter on the card is the list's runner;
/// an untouched list runs nothing.
pub fn run_launch_card_row(rows: &[LaunchCardRow], menu_selected: Option<usize>) -> LaunchAction {
    let Some(selected) = menu_selected else {
        return LaunchAction::None;
    };
    match rows.get(selected) {
        None => LaunchAction::None,
        Some(row) => match &row.id {
            crate::tui::app::LaunchRowId::NewSession => LaunchAction::NewSession,
            crate::tui::app::LaunchRowId::Recent(id) => LaunchAction::ResumeSession(id.clone()),
            crate::tui::app::LaunchRowId::SeeAll => LaunchAction::BrowseSessions,
        },
    }
}

/// What the pre-session composer layer decided about one key.
///
/// This is only an admission guard, never an input implementation: the
/// startup composer is the session's own [`crate::tui::app::ComposerState`],
/// and every editing key is answered by the conversation composer match in
/// the event loop — the single composer input authority — exactly as it
/// would be in a live session. Word motion, selection, completion menus,
/// attachments, history, paste bursts, and vim behaviour therefore cannot
/// drift from the shell. Only three things are launch-specific here: an
/// empty Enter, F1 help, and submitting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchComposerKey {
    /// The key is fully consumed and does nothing more (Enter on an empty
    /// composer with no menu entry highlighted: there is no row to run and
    /// nothing to send; Esc clearing the menu highlight or bringing the
    /// card back).
    Consumed,
    /// Submit the composed message through the normal dispatch path.
    Submit,
    /// A completion-menu selection was applied (a slash or mention popup was
    /// open and Enter picked the highlighted entry); the key is consumed
    /// without submitting — the completed text stays in the composer.
    MenuSelect,
    /// The launch chord (F1 help): the same key is then handed to
    /// [`handle_launch_key`]. It deliberately wins over its composer
    /// meaning while the launch screen is up.
    MenuChord,
    /// Not launch-specific: the conversation composer match below owns the
    /// key. The event loop must not run [`handle_launch_key`] for it.
    ComposerAuthority,
    /// Move the launch card's row selection (Up/Down while the card is up).
    MenuNavigate(i32),
    /// Run the card's highlighted row (Enter while the card is up, the
    /// composer is empty, and the user has arrowed onto a row).
    MenuRun,
}

/// Admit one key for the pre-session composer.
///
/// Editing keys are never handled here — they fall through to the
/// conversation composer match so there is exactly one composer input
/// system. Only F1 help stays launch-owned via
/// [`LaunchComposerKey::MenuChord`].
pub fn handle_launch_composer_key(app: &mut App, key: KeyEvent) -> LaunchComposerKey {
    let multiline = app.composer_multiline_mode;
    let card_up = app.launch.dissolve_started_ms.is_none();
    match key.code {
        KeyCode::Enter
            if crate::tui::composer_ui::composer_submit_chord(key, multiline).is_some() =>
        {
            // #573 parity with the session composer's Enter arm: when a
            // completion popup is matching (e.g. `/mo` → `/model`), Enter
            // applies the highlighted entry instead of sending the literal
            // prefix. A mention completion amends the composed text and is
            // consumed; a slash completion completes the command and falls
            // through to Submit so the launch dispatch path executes it.
            let mention_entries = crate::tui::file_mention::visible_mention_menu_entries(app, 1);
            if !mention_entries.is_empty()
                && crate::tui::file_mention::apply_mention_menu_selection(app, &mention_entries)
            {
                return LaunchComposerKey::MenuSelect;
            }
            let slash_entries = crate::tui::slash_menu::visible_slash_menu_entries(app, 1);
            if !slash_entries.is_empty() {
                crate::tui::slash_menu::apply_slash_menu_selection(app, &slash_entries, false);
                app.close_slash_menu();
            }
            if app.input.trim().is_empty() {
                if card_up && app.launch.menu_selected.is_some() {
                    // The card owns Enter only once the user has arrowed
                    // onto a row; an untouched list runs nothing.
                    return LaunchComposerKey::MenuRun;
                }
                LaunchComposerKey::Consumed
            } else {
                app.launch.dissolve_card(app.ambient_clock_ms);
                LaunchComposerKey::Submit
            }
        }
        KeyCode::Up if card_up => LaunchComposerKey::MenuNavigate(-1),
        KeyCode::Down if card_up => LaunchComposerKey::MenuNavigate(1),
        // Esc walks back one step: a highlighted row is unhighlighted;
        // an empty composer with the card gone brings the card back. A draft
        // in the composer keeps Esc's composer meaning.
        KeyCode::Esc if card_up && app.launch.menu_selected.is_some() => {
            app.launch.menu_selected = None;
            LaunchComposerKey::Consumed
        }
        KeyCode::Esc if !card_up && app.input.is_empty() => {
            app.launch.restore_card();
            LaunchComposerKey::Consumed
        }
        KeyCode::F(1) => LaunchComposerKey::MenuChord,
        // Every other key — text, caret motion, word motion, selection,
        // newline chords, Home/End, kill/chord editing, vim motions, Esc,
        // Tab, history — is answered by the conversation composer authority.
        _ => {
            // Typing goes straight to the composer, and the first keystroke
            // dissolves the card (founder decision, 2026-09-02).
            if card_up
                && matches!(key.code, KeyCode::Char(_))
                && !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
            {
                app.launch.dissolve_card(app.ambient_clock_ms);
            }
            LaunchComposerKey::ComposerAuthority
        }
    }
}

impl ShellTier {
    // `for_area` (the two-dimensional variant) went with the empty state's
    // tier branch: the idle caption sheds detail continuously now, so nothing
    // was left that wanted a coarse three-way answer about a whole Rect. The
    // row and column floors it encoded still exist, spelled out as
    // `AMBIENT_MIN_CHAT_HEIGHT` / `AMBIENT_MIN_CHAT_WIDTH` where the layout
    // can honour them.
    #[must_use]
    pub fn for_chrome_width(width: u16) -> Self {
        if width < 60 {
            Self::Compact
        } else if width < 110 {
            Self::Normal
        } else {
            Self::Wide
        }
    }
}

/// Perceptual session phase. Every treatment reads from this same enum so a
/// footer cannot say `idle` while the transcript is asking for approval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellPhase {
    Idle,
    Typing,
    Working,
    /// A live verification pass (tests/checks/lints). Same clock family as
    /// `Working` but rendered as the metered braille tick — checking, not
    /// searching (ocean state model).
    Verifying,
    Waiting,
    Approval,
    Done,
    Failed,
}

/// The one truthful verb shown while a turn is live. This deliberately stays
/// smaller than the tool taxonomy: the phase strip only needs to distinguish
/// hidden reasoning, read-shaped exploration, other tool use, verification,
/// and generic model work. It never exposes reasoning text or tool arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LiveActivityKind {
    Working,
    Compacting,
    AutoCompacting,
    Reasoning,
    Reading,
    UsingTool,
    UsingSubagents,
    Verifying,
}

/// Bounded projection of live turn activity. Completed entries are ignored,
/// so an `ActiveCell` retained until `TurnComplete` cannot keep the shell in a
/// false working state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LiveActivity {
    kind: LiveActivityKind,
    running_tools: usize,
}

impl LiveActivity {
    #[must_use]
    pub(crate) fn from_app(app: &App) -> Self {
        let tools = running_tool_facts(app);
        let kind = if app
            .active_compaction
            .as_ref()
            .is_some_and(|compaction| compaction.auto)
        {
            LiveActivityKind::AutoCompacting
        } else if app.active_compaction.is_some() {
            LiveActivityKind::Compacting
        } else if tools.verifying {
            LiveActivityKind::Verifying
        } else if app_has_unfinished_subagents(app) {
            LiveActivityKind::UsingSubagents
        } else if tools.count > 0 && tools.all_reading {
            LiveActivityKind::Reading
        } else if tools.count > 0 {
            LiveActivityKind::UsingTool
        } else if app.streaming_thinking_active_entry.is_some() {
            LiveActivityKind::Reasoning
        } else {
            LiveActivityKind::Working
        };
        Self {
            kind,
            running_tools: tools.count,
        }
    }

    #[must_use]
    pub(crate) fn kind(self) -> LiveActivityKind {
        self.kind
    }

    #[must_use]
    fn is_explicit(self) -> bool {
        !matches!(self.kind, LiveActivityKind::Working)
    }

    #[must_use]
    fn label(self, locale: Locale) -> Cow<'static, str> {
        match self.kind {
            LiveActivityKind::Working => tr(locale, MessageId::PhaseWorking),
            LiveActivityKind::Compacting => tr(locale, MessageId::ContextManualCompacting),
            LiveActivityKind::AutoCompacting => tr(locale, MessageId::ContextAutoCompacting),
            LiveActivityKind::Reasoning => tr(locale, MessageId::PhaseReasoning),
            LiveActivityKind::Reading => tr(locale, MessageId::PhaseReading),
            LiveActivityKind::UsingTool => tr(locale, MessageId::PhaseUsingTool),
            LiveActivityKind::UsingSubagents => tr(locale, MessageId::PhaseSubagents),
            LiveActivityKind::Verifying => tr(locale, MessageId::PhaseVerifying),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct RunningToolFacts {
    count: usize,
    all_reading: bool,
    verifying: bool,
}

/// True when any sub-agent spawned by this session is still running: live
/// progress rows win over the cache, whose Running entries are the persisted
/// view of the same actors.
fn app_has_unfinished_subagents(app: &App) -> bool {
    !app.agent_progress.is_empty()
        || app.subagent_cache.iter().any(|agent| {
            matches!(
                agent.status,
                crate::tools::subagent::SubAgentStatus::Running
            )
        })
}

impl Default for RunningToolFacts {
    fn default() -> Self {
        Self {
            count: 0,
            all_reading: true,
            verifying: false,
        }
    }
}

impl RunningToolFacts {
    fn observe(&mut self, reading: bool, verifying: bool) {
        self.count = self.count.saturating_add(1);
        self.all_reading &= reading;
        self.verifying |= verifying;
    }
}

const WORKING_BUBBLE_FRAMES: [&str; 8] = ["⠀", "⢀", "⣀", "⣄", "⣤", "⣦", "⣶", "⣿"];
const COMPLETION_RELEASE_MS: u128 = 560;
// The idle whale portrait rows (IDLE_WHALE_ROWS / UWU_IDLE_WHALE_ROWS) and
// their caustic shimmer were deleted per the 2026-08-29 founder directive:
// hand-drawn whale art is out; the only sanctioned terminal mark is the one
// generated from the brand master path. The ambient empty-state surface
// (wordmark, context caption, prompt) below is not whale art and stays.

impl ShellPhase {
    #[must_use]
    pub fn from_app(app: &App) -> Self {
        Self::from_app_with_activity(app, LiveActivity::from_app(app))
    }

    #[must_use]
    pub(crate) fn from_app_with_activity(app: &App, activity: LiveActivity) -> Self {
        if matches!(
            app.view_stack.top_kind(),
            Some(ModalKind::Approval | ModalKind::Elevation | ModalKind::UserInput)
        ) {
            return Self::Approval;
        }
        if matches!(
            activity.kind(),
            LiveActivityKind::Compacting | LiveActivityKind::AutoCompacting
        ) {
            // A typed CompactionStarted event is newer and more specific than
            // a prior turn's failed projection. Keep the recovery operation
            // visible until its matching terminal event arrives.
            return Self::Working;
        }
        if app.turn_error_posted
            || matches!(app.runtime_turn_status.as_deref(), Some("failed" | "error"))
        {
            return Self::Failed;
        }
        if app.pending_user_input_prompt.is_some()
            || app
                .task_panel
                .iter()
                .any(|task| matches!(task.status.as_str(), "waiting" | "needs_user"))
        {
            return Self::Waiting;
        }
        if app.is_loading
            || matches!(app.runtime_turn_status.as_deref(), Some("in_progress"))
            || activity.is_explicit()
        {
            if activity.kind() == LiveActivityKind::Verifying {
                return Self::Verifying;
            }
            return Self::Working;
        }
        if !app.input.is_empty() {
            return Self::Typing;
        }
        if matches!(app.runtime_turn_status.as_deref(), Some("completed")) {
            return Self::Done;
        }
        Self::Idle
    }

    #[must_use]
    pub fn label(self, locale: Locale) -> Cow<'static, str> {
        match self {
            Self::Idle => tr(locale, MessageId::PhaseIdle),
            Self::Typing => tr(locale, MessageId::PhaseDraft),
            Self::Working => tr(locale, MessageId::PhaseWorking),
            Self::Verifying => tr(locale, MessageId::PhaseVerifying),
            Self::Waiting | Self::Approval => tr(locale, MessageId::PhaseWaitingOnYou),
            Self::Done => tr(locale, MessageId::PhaseDone),
            Self::Failed => tr(locale, MessageId::PhaseFailed),
        }
    }

    #[must_use]
    #[allow(dead_code)] // classic header/band renderer: superseded by the Tideline shell
    // (topbar + merged footer, spec §3, 2026-08-29); deletion is its own slice.
    pub fn color(self, app: &App) -> Color {
        phase_ink(self).color(&app.ui_theme)
    }
}

/// Status-bar phase ink. Failure red is only `Failed`.
#[must_use]
pub(crate) fn phase_ink(phase: ShellPhase) -> ChromeInk {
    match phase {
        ShellPhase::Idle => ChromeInk::Metadata,
        ShellPhase::Done => ChromeInk::Outcome,
        ShellPhase::Typing => ChromeInk::Identity,
        // Verifying shares the live seafoam hue; the tick-vs-bubble
        // marker carries the checking/searching distinction.
        ShellPhase::Working | ShellPhase::Verifying => ChromeInk::Active,
        ShellPhase::Waiting | ShellPhase::Approval => ChromeInk::Waiting,
        ShellPhase::Failed => ChromeInk::Failure,
    }
}

/// Exhaustive on purpose: a new [`AppMode`] must be handed a Policy ink
/// deliberately rather than inheriting act's by falling through a wildcard.
fn header_mode_ink(mode: AppMode) -> ChromeInk {
    match mode {
        AppMode::Plan => ChromeInk::PolicyPlan,
        AppMode::Operate => ChromeInk::PolicyOperate,
        AppMode::Agent => ChromeInk::PolicyAct,
    }
}

fn header_permission_ink(mode: ApprovalMode) -> ChromeInk {
    match mode {
        ApprovalMode::Suggest | ApprovalMode::Never => ChromeInk::PermissionAsk,
        ApprovalMode::Auto => ChromeInk::PermissionAutoReview,
        ApprovalMode::Bypass => ChromeInk::PermissionFullAccess,
    }
}

#[allow(dead_code)] // classic header/band renderer: superseded by the Tideline shell
// (topbar + merged footer, spec §3, 2026-08-29); deletion is its own slice.
fn header_fg(app: &App, ink: ChromeInk) -> Style {
    chrome_style(&app.ui_theme, ink)
}

/// One posture word with its ink — the unit the classic header's lockup was
/// made of, now carried as merged-footer chips.
pub(crate) type PostureChip = (Cow<'static, str>, ChromeInk);

/// The posture lockup as two standalone chips for the Tideline merged
/// footer (spec §3: the old header's mode/permission chips move into the
/// footer activity segment). Same words, same inks, and the same mapping
/// the classic header used — [`header_mode_ink`] for the mode word,
/// [`header_permission_ink`] for the permission phrase. The filesystem
/// scope notice, when it deviates, folds into the permission chip's text
/// (the header already painted it in the permission ink).
pub(crate) fn posture_chips(app: &App) -> (Option<PostureChip>, Option<PostureChip>) {
    let mode = (
        mode_label(app.ui_locale, app.mode),
        header_mode_ink(app.mode),
    );
    let mut permission = (
        permission_label(app),
        header_permission_ink(app.approval_mode),
    );
    if let Some(scope) = filesystem_scope_notice(app) {
        permission.0 = format!("{} · {scope}", permission.0).into();
    }
    (Some(mode), Some(permission))
}

/// Summarize only tools whose lifecycle is actually `Running`. A read label
/// is earned only when every running entry is read/exploration-shaped; mixed
/// work stays the neutral `using tool`. Verification wins because it is the
/// existing stronger promise made by the phase strip.
fn running_tool_facts(app: &App) -> RunningToolFacts {
    use crate::tui::history::{HistoryCell, ToolCell, ToolStatus};
    use crate::tui::widgets::tool_card::{ToolFamily, tool_family_for_name};

    let mut facts = RunningToolFacts::default();
    let Some(active) = app.active_cell.as_ref() else {
        return facts;
    };
    for cell in active.entries() {
        let HistoryCell::Tool(tool) = cell else {
            continue;
        };
        match tool {
            ToolCell::Exec(exec) if exec.status == ToolStatus::Running => {
                facts.observe(false, exec_is_verification(&exec.command));
            }
            ToolCell::Generic(generic) if generic.status == ToolStatus::Running => {
                let family = tool_family_for_name(&generic.name);
                facts.observe(
                    matches!(family, ToolFamily::Read | ToolFamily::Find),
                    family == ToolFamily::Verify || generic.name == "read_lints",
                );
            }
            ToolCell::Exploring(exploring) => {
                for entry in &exploring.entries {
                    if entry.status == ToolStatus::Running {
                        facts.observe(true, false);
                    }
                }
            }
            ToolCell::WebSearch(search) if search.status == ToolStatus::Running => {
                facts.observe(true, false);
            }
            other if other.status() == Some(ToolStatus::Running) => {
                facts.observe(false, false);
            }
            _ => {}
        }
    }
    facts
}

fn exec_is_verification(command: &str) -> bool {
    let trimmed = command.trim_start();
    let mut tokens = trimmed.split_whitespace();
    let first = tokens.next().unwrap_or("");
    let second = tokens.next().unwrap_or("");
    match first {
        "cargo" => matches!(second, "test" | "check" | "clippy" | "nextest"),
        "go" => matches!(second, "test" | "vet"),
        "npm" | "pnpm" | "yarn" | "bun" => matches!(second, "test" | "lint" | "check"),
        "make" => matches!(second, "test" | "check" | "lint"),
        "python" | "python3" => trimmed.contains("-m pytest") || trimmed.contains("-m unittest"),
        "pytest" | "jest" | "vitest" | "tsc" | "eslint" | "ruff" | "mypy" | "clippy-driver"
        | "golangci-lint" | "shellcheck" => true,
        _ => false,
    }
}

fn completion_elapsed_ms(app: &App) -> Option<u128> {
    if !app.motion_policy().allows_decorative() {
        return None;
    }
    app.ocean_completion_started_at
        .map(|started| started.elapsed().as_millis())
        .filter(|elapsed| *elapsed < COMPLETION_BREATH_MS)
}

/// Truthful window-title activity verb for the OSC-0 whale animation.
///
/// Uses short English fragments (with fixed-width ellipsis) so alt-tabbed
/// sessions stay legible without depending on the full localized phase strip.
#[must_use]
pub(crate) fn title_activity_verb(app: &App) -> &'static str {
    let activity = LiveActivity::from_app(app);
    let phase = ShellPhase::from_app_with_activity(app, activity);
    match phase {
        ShellPhase::Waiting | ShellPhase::Approval => "waiting on you…",
        ShellPhase::Verifying => "verifying…",
        ShellPhase::Done => "done",
        ShellPhase::Failed => "failed",
        ShellPhase::Typing => "drafting…",
        ShellPhase::Idle => "idle",
        ShellPhase::Working => match activity.kind() {
            LiveActivityKind::Compacting | LiveActivityKind::AutoCompacting => {
                "compacting context…"
            }
            LiveActivityKind::Reasoning => "reasoning…",
            LiveActivityKind::Reading => "reading…",
            LiveActivityKind::UsingTool => "using tool…",
            LiveActivityKind::UsingSubagents => "fleet underway…",
            LiveActivityKind::Verifying => "verifying…",
            LiveActivityKind::Working => "in the current…",
        },
    }
}

/// Push the current shell phase into the terminal title whale animation.
pub(crate) fn sync_title_activity(app: &App) {
    crate::tui::notifications::set_title_motion_enabled(
        app.motion_policy().allows_decorative() && app.status_indicator != "off",
    );
    // Keep the `[title] …` window-title prefix in step with the session and
    // config defaults; change detection inside makes this free when nothing
    // moved.
    crate::tui::notifications::set_title_prefix(app.window_title_prefix());
    if app.is_loading
        || matches!(
            ShellPhase::from_app(app),
            ShellPhase::Working
                | ShellPhase::Verifying
                | ShellPhase::Waiting
                | ShellPhase::Approval
                | ShellPhase::Typing
        )
    {
        crate::tui::notifications::set_title_activity_verb(title_activity_verb(app));
    }
}

pub(crate) fn phase_marker_with_activity(
    app: &App,
    phase: ShellPhase,
    activity: LiveActivity,
) -> (&'static str, Cow<'static, str>) {
    let locale = app.ui_locale;
    match phase {
        ShellPhase::Idle => ("·", phase.label(locale)),
        ShellPhase::Typing => ("›", phase.label(locale)),
        ShellPhase::Working => {
            // The footer and the live tool card share one wall-clock cadence,
            // so the two primary liveness marks never look like unrelated
            // spinners. The shared helper also preserves the 400ms
            // "motion is earned" delay and reduced/still fallback.
            let policy = app.motion_policy();
            let animated = crate::tui::spinner::braille_spinner_frame(app.turn_started_at, false);
            let earned = app.turn_started_at.is_none_or(|started| {
                started.elapsed().as_millis()
                    >= u128::from(crate::tui::spinner::LIVE_MARKER_DELAY_MS)
            });
            let frame = policy.spinner_glyph(animated, earned);
            (frame, activity.label(locale))
        }
        ShellPhase::Verifying => {
            // Metered braille tick on the shared live clock — checking, not
            // searching. Reduced motion holds the legible mid frame.
            let policy = app.motion_policy();
            let animated = crate::tui::spinner::verification_tick_frame(app.turn_started_at, false);
            let earned = app.turn_started_at.is_none_or(|started| {
                started.elapsed().as_millis()
                    >= u128::from(crate::tui::spinner::LIVE_MARKER_DELAY_MS)
            });
            let frame = policy.spinner_glyph(animated, earned);
            (frame, phase.label(locale))
        }
        ShellPhase::Waiting | ShellPhase::Approval => ("◆", phase.label(locale)),
        ShellPhase::Done => match completion_elapsed_ms(app) {
            Some(elapsed) if elapsed < COMPLETION_RELEASE_MS => {
                let index = ((elapsed / 140) as usize + 4).min(WORKING_BUBBLE_FRAMES.len() - 1);
                (
                    WORKING_BUBBLE_FRAMES[index],
                    tr(locale, MessageId::PhaseFinishing),
                )
            }
            _ => (crate::tui::glyphs::DONE, phase.label(locale)),
        },
        ShellPhase::Failed => (crate::tui::glyphs::FAILED, phase.label(locale)),
    }
}

fn mode_label(locale: Locale, mode: AppMode) -> Cow<'static, str> {
    match mode {
        AppMode::Agent => tr(locale, MessageId::ChipModeAct),
        AppMode::Plan => tr(locale, MessageId::ChipModePlan),
        AppMode::Operate => tr(locale, MessageId::ChipModeOperate),
    }
}

/// Permission chip words. This maps from the typed [`ApprovalMode`] state —
/// never from the English `permission_chip_label()` strings — so localizing
/// (or rewording) the upstream chip labels can never silently break the chip.
///
/// Tool-approval posture only. Filesystem scope is a separate fact and only
/// earns header columns when it is worth reading — see
/// [`filesystem_scope_notice`].
fn permission_label(app: &App) -> Cow<'static, str> {
    let locale = app.ui_locale;
    if app.mode == AppMode::Plan {
        return tr(locale, MessageId::ChipPermissionReadOnly);
    }
    match app.approval_mode {
        ApprovalMode::Suggest => tr(locale, MessageId::ChipPermissionAsk),
        ApprovalMode::Auto => tr(locale, MessageId::ChipPermissionAuto),
        // Keep the effective permission explicit. `bypass` is an
        // implementation detail and, more importantly, can imply that
        // repository law no longer applies. Full Access never bypasses
        // constitution rules. This is **tool-approval posture**, not
        // filesystem scope — see filesystem_scope_notice.
        ApprovalMode::Bypass => tr(locale, MessageId::ChipPermissionFullAccess),
        ApprovalMode::Never => tr(locale, MessageId::ChipPermissionNever),
    }
}

/// The effective filesystem scope — but only when it says something the
/// permission word beside it does not already say.
///
/// This chip exists because "Full Access" (tool approval) was being read as
/// unrestricted disk writes (user report, 2026-07-23), and because a policy
/// with no enforcement backend used to name a boundary nobody applied
/// (2026-08-04 audit). Both of those are deviations. The default — an
/// enforced workspace-write boundary — is what every ordinary session already
/// has, and printing `files: workspace` on every frame of every session spent
/// seventeen columns of the primary chrome saying so. A notice that is always
/// on cannot signal anything; folding the expected case away is what lets
/// `files: workspace (unenforced)` and the Full-Access-but-confined case land
/// as warnings when they do appear.
///
/// `read-only` under Plan is dropped for the same reason from the other side:
/// the permission word there is already the literal phrase "read only".
#[must_use]
fn filesystem_scope_notice(app: &App) -> Option<Cow<'static, str>> {
    // Spelled out because the old `fs:` prefix read as an unexplained
    // acronym (user report, 2026-07-23): this chip states which files the
    // session may write.
    let policy = crate::core::authority::sandbox_policy_for_turn(
        app.mode,
        app.approval_mode,
        app.configured_sandbox_mode.as_deref(),
        &app.workspace,
        crate::core::authority::SandboxNetworkAccess::from_config(app.configured_sandbox_network),
    );
    // A policy is an intent; enforcement needs a backend. On default Linux
    // (bubblewrap is opt-in) and on all Windows there is none. Say
    // "unenforced" rather than name a boundary that is not applied.
    // `DangerFullAccess` is already honest, and `ExternalSandbox` is enforced
    // by the external runner, not by us.
    let unenforced = app.sandbox_backend.is_none()
        && !matches!(
            policy,
            crate::sandbox::SandboxPolicy::DangerFullAccess
                | crate::sandbox::SandboxPolicy::ExternalSandbox { .. }
        );
    match policy {
        crate::sandbox::SandboxPolicy::ReadOnly if unenforced => {
            Some(Cow::Borrowed("files: read-only (unenforced)"))
        }
        crate::sandbox::SandboxPolicy::ReadOnly => {
            (app.mode != AppMode::Plan).then_some(Cow::Borrowed("files: read-only"))
        }
        // `DangerFullAccess` only ever arises from the Bypass posture
        // (`sandbox_policy_for_turn`), whose permission chip already reads
        // "Full Access" two words to the left. The name is the disclosure;
        // restating it as `files: full disk` spent columns saying it twice.
        // The scope chip speaks in this posture only when the scope is
        // *narrower* than the name implies (the WorkspaceWrite arm below).
        crate::sandbox::SandboxPolicy::DangerFullAccess => None,
        crate::sandbox::SandboxPolicy::ExternalSandbox { .. } => {
            Some(Cow::Borrowed("files: external sandbox"))
        }
        crate::sandbox::SandboxPolicy::WorkspaceWrite { .. } if unenforced => {
            Some(Cow::Borrowed("files: workspace (unenforced)"))
        }
        // The unremarkable case: writes are confined to the workspace and the
        // OS is actually enforcing it. Saying so on every frame of every
        // session spends the header on a fact nobody is asking about — with
        // one exception. When the permission chip reads "Full Access", the
        // scope chip is the only thing on screen that says the writes are
        // still confined. Suppressing it there recreates precisely the
        // misreading the chip was added for (tool-approval "Full Access" taken
        // to mean unrestricted disk writes), and that pairing is reachable:
        // Bypass with a configured `workspace-write` is clamped to this policy
        // by `sandbox_policy_for_turn`.
        crate::sandbox::SandboxPolicy::WorkspaceWrite { .. } => {
            (app.approval_mode == ApprovalMode::Bypass).then_some(Cow::Borrowed("files: workspace"))
        }
    }
}

fn span_width(spans: &[Span<'_>]) -> usize {
    spans.iter().map(|span| span.content.width()).sum()
}

fn truncate_to_width(text: &str, width: usize) -> String {
    if text.width() <= width {
        return text.to_string();
    }
    if width == 0 {
        return String::new();
    }
    if width <= 3 {
        return ".".repeat(width);
    }
    let mut result = String::new();
    let mut used = 0;
    for ch in text.chars() {
        let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + ch_width + 1 > width {
            break;
        }
        result.push(ch);
        used += ch_width;
    }
    result.push('…');
    result
}

#[allow(dead_code)] // classic header/band renderer: superseded by the Tideline shell
// (topbar + merged footer, spec §3, 2026-08-29); deletion is its own slice.
fn compact_tokens(tokens: i64) -> String {
    if tokens >= 1_000_000 {
        format!("{:.1}M", tokens as f64 / 1_000_000.0)
    } else if tokens >= 1_000 {
        format!("{:.0}K", tokens as f64 / 1_000.0)
    } else {
        tokens.to_string()
    }
}

#[allow(dead_code)]
// classic header/band renderer: superseded by the Tideline shell
// (topbar + merged footer, spec §3, 2026-08-29); deletion is its own slice.
/// The context meter is one measured fact: an exact percentage for scanning,
/// a token fraction for auditability when room permits, and a short bar for
/// peripheral vision. It is deliberately the final header fact so its rect
/// stays stable and can point at the inspector without parsing rendered text.
fn header_context_meter(app: &App, tier: ShellTier) -> Option<Span<'static>> {
    crate::tui::ui::context_usage_snapshot(app).map(|(used, max, percent)| {
        let filled = ((percent / 100.0) * 5.0).ceil().clamp(0.0, 5.0) as usize;
        let percentage = format!("{percent:.0}%");
        let text = match tier {
            ShellTier::Compact => format!("ctx {percentage}"),
            ShellTier::Normal | ShellTier::Wide => format!(
                "context {percentage} {}/{} {}{}",
                compact_tokens(used),
                compact_tokens(i64::from(max)),
                "▰".repeat(filled),
                "▱".repeat(5usize.saturating_sub(filled)),
            ),
        };
        Span::styled(text, header_fg(app, ChromeInk::Info))
    })
}

/// Return concrete, typed header targets for the latest frame.
///
/// The context meter is right-aligned and always the final header span, so
/// its visible geometry does not depend on optional git/token facts. The
/// keyboard route remains `Alt+C`; this gives that same inspectable fact a
/// mouse route without inventing another context screen or state owner.
#[allow(dead_code)]
// classic header/band renderer: superseded by the Tideline shell
// (topbar + merged footer, spec §3, 2026-08-29); deletion is its own slice.
// Its posture-floor guard (a hitbox never claims overlapped cells) is the
// discipline `topbar::context_meter_hitbox` carries forward.
#[must_use]
pub(crate) fn header_hitboxes(area: Rect, app: &App) -> Vec<HeaderHitbox> {
    if area.width == 0 || area.height == 0 {
        return Vec::new();
    }
    let tier = ShellTier::for_chrome_width(area.width);
    let Some(meter) = header_context_meter(app, tier) else {
        return Vec::new();
    };
    let width = u16::try_from(span_width(&[meter]))
        .unwrap_or(area.width)
        .min(area.width);
    if width == 0 {
        return Vec::new();
    }
    // The posture lockup is the header's guaranteed floor and is never
    // truncated to make room for the right cluster (see
    // render_header_with_git_status). At compact widths that floor can run
    // into the meter's columns, so a hitbox anchored blindly at the right
    // edge would claim cells the posture actually paints (review finding 5).
    // Recompute the floor's width with the same spans the renderer composes
    // and refuse the hitbox when the two would overlap.
    let mut posture_width = 0usize;
    if let Some(indicator) = crate::tui::widgets::header_status_indicator_frame(
        (!app.low_motion && app.fancy_animations)
            .then_some(app.turn_started_at)
            .flatten(),
        &app.status_indicator,
    ) {
        posture_width += indicator.width() + GROUP_GAP.len();
    }
    posture_width += mode_label(app.ui_locale, app.mode).width();
    posture_width += FIELD_JOIN.len() + permission_label(app).width();
    if let Some(scope) = filesystem_scope_notice(app) {
        posture_width += FIELD_JOIN.len() + scope.width();
    }
    let meter_start = usize::from(area.width.saturating_sub(width));
    if meter_start <= posture_width.saturating_add(usize::from(width > 0)) {
        return Vec::new();
    }
    vec![HeaderHitbox {
        area: Rect {
            x: area.x.saturating_add(area.width.saturating_sub(width)),
            y: area.y,
            width,
            height: 1,
        },
        target: HeaderActionTarget::InspectContext,
    }]
}

fn session_token_breakdown(app: &App) -> Option<Span<'static>> {
    app.header_items.contains(&HeaderItem::Tokens).then(|| {
        Span::styled(
            format!(
                "{} in · {} cch · {} out",
                format_token_count_compact(u64::from(app.session.displayed_total_input_tokens())),
                format_token_count_compact(u64::from(
                    app.session.displayed_total_cache_hit_tokens(),
                )),
                format_token_count_compact(u64::from(app.session.displayed_total_output_tokens())),
            ),
            header_fg(app, ChromeInk::Info),
        )
    })
}

/// The header speaks with exactly two separators, and each one means one
/// thing.
///
/// [`FIELD_JOIN`] binds words that qualify one another into a single phrase:
/// `work · ask` is one statement of posture, not two facts. [`GROUP_GAP`]
/// stands between whole facts — posture, then the goal chip, then the update
/// notice; workspace, then the context meter.
///
/// Before this, every one of those boundaries was the same dotted separator at
/// the same dim ink, so the header read as an undifferentiated list and there
/// was nothing for the eye to group on. The gap is deliberately wider than the
/// visual whitespace inside `" · "` — four blank columns against one — because
/// that ratio is the only thing carrying the grouping.
#[allow(dead_code)] // classic header/band renderer: superseded by the Tideline shell
// (topbar + merged footer, spec §3, 2026-08-29); deletion is its own slice.
const FIELD_JOIN: &str = " · ";
#[allow(dead_code)] // classic header/band renderer: superseded by the Tideline shell
// (topbar + merged footer, spec §3, 2026-08-29); deletion is its own slice.
const GROUP_GAP: &str = "    ";

/// Append one chrome element, inserting the group separator only between
/// elements so an absent element never leaves trailing padding.
#[allow(dead_code)] // classic header/band renderer: superseded by the Tideline shell
// (topbar + merged footer, spec §3, 2026-08-29); deletion is its own slice.
fn push_chrome(spans: &mut Vec<Span<'static>>, span: Span<'static>) {
    if !spans.is_empty() {
        spans.push(Span::raw(GROUP_GAP));
    }
    spans.push(span);
}

/// Render the one-line shell header. Immediate operating posture and workspace
/// truth live here; quieter route identity lives beside the phase footer.
#[allow(dead_code)] // classic header/band renderer: superseded by the Tideline shell
// (topbar + merged footer, spec §3, 2026-08-29); deletion is its own slice.
pub fn render_header(area: Rect, buf: &mut Buffer, app: &App) {
    let git_status = crate::tui::git_status::cached_status();
    render_header_with_git_status(area, buf, app, &git_status);
}

#[allow(dead_code)] // classic header/band renderer: superseded by the Tideline shell
// (topbar + merged footer, spec §3, 2026-08-29); deletion is its own slice.
fn render_header_with_git_status(
    area: Rect,
    buf: &mut Buffer,
    app: &App,
    git_status: &crate::tui::git_status::GitStatusSnapshot,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let tier = ShellTier::for_chrome_width(area.width);
    Block::default()
        .style(Style::default().bg(app.ui_theme.header_bg))
        .render(area, buf);

    let mode_color = header_mode_ink(app.mode).color(&app.ui_theme);
    // Match the composer's warm top edge exactly: Ask amber, Auto-Review
    // Signal Gold, and Full Access coral.
    let permission_color = header_permission_ink(app.approval_mode).color(&app.ui_theme);
    let dim = header_fg(app, ChromeInk::MetadataDim);
    // `status_indicator` owns the single header mark. It used to be filtered
    // against the literal "cw" because the header also hardcoded a leading
    // "cw" span, and `header_status_indicator_frame` collapses `cw`, the
    // legacy `whale` opt-in, and unknown values onto that same mark — so the
    // filter silently discarded three of the setting's four documented values
    // and left `off` with nothing to turn off (#5512). There is one mark now,
    // and this setting decides what occupies it.
    let status_indicator = crate::tui::widgets::header_status_indicator_frame(
        (!app.low_motion && app.fancy_animations)
            .then_some(app.turn_started_at)
            .flatten(),
        &app.status_indicator,
    );
    // The posture lockup: mark, then mode and permission (and the filesystem
    // scope when it deviates) joined into one phrase. This is the guaranteed
    // floor of the header — everything after it is sheddable — so it is built
    // once and reused by the cramped rebuild below rather than spelled twice.
    let mut left = Vec::new();
    if let Some(indicator) = status_indicator {
        left.push(Span::styled(
            indicator,
            header_fg(app, ChromeInk::Identity).add_modifier(Modifier::BOLD),
        ));
        left.push(Span::raw(GROUP_GAP));
    }
    left.push(Span::styled(
        mode_label(app.ui_locale, app.mode),
        Style::default().fg(mode_color),
    ));
    // Permission is safety state, not optional chrome. Compact terminals shed
    // auxiliary detail, but keep mode and the effective posture.
    left.push(Span::styled(FIELD_JOIN, dim));
    left.push(Span::styled(
        permission_label(app),
        Style::default().fg(permission_color),
    ));
    let scope_notice = filesystem_scope_notice(app);
    if let Some(scope) = scope_notice.clone() {
        left.push(Span::styled(FIELD_JOIN, dim));
        left.push(Span::styled(scope, Style::default().fg(permission_color)));
    }
    let posture = left.clone();
    // Active-goal chip (#39): the ocean shell has no sidebar, so the topbar
    // is the only always-on surface where a goal set via `create_goal` can
    // live. Objective truncated to a fixed budget; terminal goals render
    // nothing. The cramped-layout rebuild below keeps the chip in `suffix`.
    let goal_chip =
        crate::tui::footer_ui::active_goal_chip_state(app).map(|(objective, paused)| {
            let budget = if paused { 22 } else { 26 };
            let flat = objective.trim().replace(['\n', '\r'], " ");
            let text = if paused {
                format!("goal paused {}", truncate_to_width(&flat, budget))
            } else {
                format!("goal {}", truncate_to_width(&flat, budget))
            };
            let color = if paused {
                ChromeInk::Attention.color(&app.ui_theme)
            } else {
                ChromeInk::Active.color(&app.ui_theme)
            };
            (text, color)
        });
    if let Some((text, color)) = &goal_chip {
        left.push(Span::raw(GROUP_GAP));
        left.push(Span::styled(
            text.clone(),
            Style::default().fg(*color).add_modifier(Modifier::BOLD),
        ));
    }
    // Workflow-run chip (#5040): the same `WorkflowPanel::top_bar_chip` the
    // classic header shows, so a collapsed run stays visible on the ocean
    // shell too. No workflow panel means no chip. The cramped-layout rebuild
    // below keeps the chip in `suffix` alongside the goal chip.
    let workflow_chip = app.workflow_panel.as_ref().map(|panel| {
        let ink = if matches!(
            panel.lifecycle,
            crate::tui::widgets::workflow_panel::WorkflowPanelLifecycle::Degraded
        ) {
            ChromeInk::Attention
        } else {
            ChromeInk::Info
        };
        (panel.top_bar_chip(), ink.color(&app.ui_theme))
    });
    if let Some((text, color)) = &workflow_chip {
        left.push(Span::raw(GROUP_GAP));
        left.push(Span::styled(
            text.clone(),
            Style::default().fg(*color).add_modifier(Modifier::BOLD),
        ));
    }
    // Update-available chip (#14): a quiet, persistent affordance set once by
    // the startup version check. Gets the workflow chip's treatment: last in
    // the left cluster, the route label yields its budget first, and the chip
    // drops cleanly when even a minimal chip cannot fit — never a modal,
    // never mid-chip clipping.
    let update_chip = app
        .update_available
        .as_ref()
        .map(|label| (label.clone(), ChromeInk::Attention.color(&app.ui_theme)));
    if let Some((text, color)) = &update_chip {
        left.push(Span::raw(GROUP_GAP));
        left.push(Span::styled(
            text.clone(),
            Style::default().fg(*color).add_modifier(Modifier::BOLD),
        ));
    }

    let context_meter = header_context_meter(app, tier);
    let token_breakdown = (tier != ShellTier::Compact)
        .then(|| session_token_breakdown(app))
        .flatten();
    // Cached repository/worktree status only — never probe from the render path.
    // Background refresh is scheduled from the event loop / idle ticks.
    let git_label = crate::tui::git_status::chrome_label(git_status).map(|label| {
        let max_width = match tier {
            ShellTier::Compact => 24,
            ShellTier::Normal => 36,
            ShellTier::Wide => 52,
        };
        Span::styled(
            truncate_to_width(&label, max_width),
            header_fg(app, crate::tui::git_status::chrome_ink()),
        )
    });

    // Baseline right-hand chrome: git, then the context meter.
    //
    // The build version used to close this cluster. It was already the first
    // thing the header sacrificed — present only on `Wide`, gone below 110
    // columns — which is the layout admitting it was never load-bearing. It is
    // a fact you check deliberately (`codewhale --version`, `codewhale
    // doctor`, the launch screen) exactly once, and the half of it that *is*
    // worth reading mid-session — "your build is stale" — already has its own
    // chip on the left. Fifteen columns of the primary chrome on every screen
    // forever bought a numeral nobody was reading.
    let mut right = Vec::new();
    if let Some(git_label) = git_label.clone() {
        push_chrome(&mut right, git_label);
    }
    if let Some(context_meter) = context_meter.clone() {
        push_chrome(&mut right, context_meter);
    }

    // The posture lockup is the header's floor: mark, mode, permission, and a
    // deviating filesystem scope never yield their columns to anything on the
    // right. It is measured, not re-derived, so the floor cannot drift away
    // from what actually gets drawn.
    let minimum_left_width = span_width(&posture);
    let available = usize::from(area.width);
    // The optional token breakdown is the only elidable element: it is added
    // between the git label and the context meter when the terminal is wide
    // enough to keep the whole baseline plus the guaranteed-left minimum.
    if let Some(token_breakdown) = token_breakdown {
        let mut enhanced_right = Vec::new();
        if let Some(git_label) = git_label.clone() {
            push_chrome(&mut enhanced_right, git_label);
        }
        push_chrome(&mut enhanced_right, token_breakdown);
        if let Some(context_meter) = context_meter.clone() {
            push_chrome(&mut enhanced_right, context_meter);
        }
        let enhanced_width = span_width(&enhanced_right);
        let gap = usize::from(enhanced_width > 0);
        if minimum_left_width
            .saturating_add(gap)
            .saturating_add(enhanced_width)
            <= available
        {
            right = enhanced_right;
        }
    }

    let right_width = span_width(&right);
    let left_budget = available.saturating_sub(right_width + usize::from(right_width > 0));
    if span_width(&left) > left_budget {
        // Cramped: keep the posture lockup exactly as composed and re-hang the
        // chips behind it. Rebuilding the lockup by hand here is how the two
        // passes used to disagree about what the header guarantees.
        let mut compact_left = posture.clone();
        // The goal chip survives cramped layouts too — it is operator state,
        // not decoration. The route label yields its budget first (down to
        // nothing, as it always has); below that the goal itself truncates,
        // and when even a minimal chip cannot fit it drops rather than
        // clipping mid-word (#39).
        let base_fixed = span_width(&compact_left);
        if let Some((text, color)) = &goal_chip {
            let goal_room = left_budget
                .saturating_sub(base_fixed)
                .saturating_sub(GROUP_GAP.len());
            if goal_room >= 8 {
                compact_left.push(Span::raw(GROUP_GAP));
                compact_left.push(Span::styled(
                    truncate_to_width(text, goal_room),
                    Style::default().fg(*color).add_modifier(Modifier::BOLD),
                ));
            }
        }
        // The workflow chip (#5040) is operator state too, so it gets the
        // goal chip's treatment: whatever room remains after the chips ahead
        // of it, clean truncation, and a clean drop when even a minimal chip
        // cannot fit. The route label still yields its budget first.
        if let Some((text, color)) = &workflow_chip {
            let workflow_room = left_budget
                .saturating_sub(span_width(&compact_left))
                .saturating_sub(GROUP_GAP.len());
            if workflow_room >= 8 {
                compact_left.push(Span::raw(GROUP_GAP));
                compact_left.push(Span::styled(
                    truncate_to_width(text, workflow_room),
                    Style::default().fg(*color).add_modifier(Modifier::BOLD),
                ));
            }
        }
        // The update chip (#14) gets the same treatment, last in line: it is
        // useful, but it yields to every piece of operator state ahead of it.
        if let Some((text, color)) = &update_chip {
            let update_room = left_budget
                .saturating_sub(span_width(&compact_left))
                .saturating_sub(GROUP_GAP.len());
            if update_room >= 8 {
                compact_left.push(Span::raw(GROUP_GAP));
                compact_left.push(Span::styled(
                    truncate_to_width(text, update_room),
                    Style::default().fg(*color).add_modifier(Modifier::BOLD),
                ));
            }
        }
        left = compact_left;
    }
    let left_width = span_width(&left);
    let gap = available.saturating_sub(left_width + right_width);
    left.push(Span::raw(" ".repeat(gap)));
    left.extend(right);
    let title_area = Rect { height: 1, ..area };
    Paragraph::new(Line::from(left)).render(title_area, buf);
    if area.height > 1 {
        let rule_area = Rect {
            y: area.y.saturating_add(1),
            height: 1,
            ..area
        };
        Paragraph::new(Line::from(Span::styled(
            "─".repeat(usize::from(area.width)),
            Style::default().fg(app.ui_theme.border),
        )))
        .render(rule_area, buf);
    }
}

/// The transcript rows the idle brand mark needs before it will draw at all.
///
/// Named so the *layout* can honour it before the frame is split. Anything that reserves rows above
/// the transcript must subtract against this constant rather than guess, or
/// the reservation and the render gate drift and the mark is evicted by
/// chrome that was sized without knowing the mark existed.
pub(crate) const AMBIENT_MIN_CHAT_HEIGHT: u16 = 16;
/// Companion column floor, same reasoning as [`AMBIENT_MIN_CHAT_HEIGHT`].
pub(crate) const AMBIENT_MIN_CHAT_WIDTH: u16 = 60;

/// Build the post-launch idle composition: brand, workspace context, and one
/// direct invitation. Commands stay in the command surface instead of reading
/// like onboarding homework.
///
/// Expressed in terms of the ambient floor constants so the layout rule that
/// reserves the rows and the gate that spends them cannot disagree. (The old
/// spelling also tested `height >= 14 && width >= 28`, which was dead: the
/// tier check already demands 16 rows and 60 columns.)
#[must_use]
pub(crate) fn empty_state_mark_visible(area: Rect) -> bool {
    area.height >= AMBIENT_MIN_CHAT_HEIGHT && area.width >= AMBIENT_MIN_CHAT_WIDTH
}

#[must_use]
pub(crate) fn decorative_shell_motion_enabled(app: &App) -> bool {
    app.motion_policy().allows_decorative()
        && !app.attention_hold_active()
        && app.onboarding == OnboardingState::None
        && !app.launch.visible
        && app.view_stack.is_empty()
}

/// Shorten a workspace path to its trailing components, marked with a leading
/// ellipsis so it reads as "somewhere above here" rather than as a real path.
fn shorten_workspace(workspace: &str, keep: usize) -> String {
    let sep = if workspace.contains('/') { '/' } else { '\\' };
    let parts: Vec<&str> = workspace.split(sep).filter(|p| !p.is_empty()).collect();
    if parts.len() <= keep {
        return workspace.to_string();
    }
    let tail = parts[parts.len() - keep..].join(&sep.to_string());
    let shortened = format!("…{sep}{tail}");
    // Only elide when it actually buys width. `~/code/app` -> `…/code/app` is
    // the same length and throws away the `~`, which carries more meaning than
    // the ellipsis does.
    if shortened.width() >= workspace.width() {
        return workspace.to_string();
    }
    shortened
}

/// Compose the empty-state caption so the caller's centering can survive.
///
/// This line sits between the wordmark and "What do you want to accomplish?",
/// and every other element of that block is centered. It used to be built at
/// full length and then handed to `truncate_to_width(.., width)`, which made it
/// exactly `width` wide — so the caller's `(width - context.width()) / 2` inset
/// evaluated to zero and the caption rendered flush-left, full-bleed, cutting
/// the composition in half. The clipping also destroyed the information: an
/// absolute path truncated mid-directory ("…/34267917-11f4-4d15-911a-…") tells
/// the reader nothing about where they are.
///
/// So the caption sheds detail rather than getting cut. In order of what goes
/// first: the MCP count, then the branch, then the leading path components. The
/// folder you are in is the last thing to go, because it is the only part a
/// person actually reads here.
///
/// One rule was added after watching it at 120 columns: the margin is
/// proportional, not a flat four. A flat four let a 114-column path "fit" a
/// 119-column lane, which put the centring inset back at two and reproduced
/// the full-bleed banner this function exists to prevent — the same failure,
/// arrived at from the other direction. A sixth of the lane, split either
/// side, means the caption is always visibly a caption.
fn empty_state_caption(
    workspace: &str,
    branch: &str,
    mcp_label: &str,
    mcp_count: usize,
    width: usize,
) -> String {
    // Leave a margin so the line is visibly inset rather than merely fitting,
    // and scale it, because "four columns" is only a margin at 60 columns.
    let budget = width.saturating_sub((width / 6).max(4)).max(8);
    let candidates = [
        format!("{workspace} · {branch} · {mcp_label} {mcp_count}"),
        format!("{workspace} · {branch}"),
        workspace.to_string(),
        format!("{} · {branch}", shorten_workspace(workspace, 2)),
        shorten_workspace(workspace, 2),
        shorten_workspace(workspace, 1),
    ];
    for candidate in &candidates {
        if candidate.width() <= budget {
            return candidate.clone();
        }
    }
    // Nothing fit: the last resort is the folder name alone, and the caller
    // still clamps. Better a bare name than a path clipped mid-component.
    shorten_workspace(workspace, 1)
}

/// The launch card as the idle transcript's own content, plus where its
/// clickable rows landed.
///
/// The opening screen used to be a second surface: its own layout, its own
/// composer widget, its own input authority. Founder ruling: "we don't have
/// to have a different look for the opening screen ... we can make it an
/// asset that exists there instead". So it is drawn as the empty state of the
/// ordinary transcript — the ocean, the water and the chrome underneath it are
/// the ones every other screen already uses, and the composer below it is the
/// real one.
pub struct LaunchEmptyState {
    pub lines: Vec<Line<'static>>,
    /// Clickable rows as `(id, row index within `lines`)`. The caller turns
    /// these into rects against the painted area, so hitboxes and glyphs
    /// cannot drift apart.
    pub rows: Vec<(crate::tui::app::LaunchRowId, usize)>,
}

/// Left indent for the whole block. Small: this is a top-left anchor, not a
/// centred hero.
const LAUNCH_BLOCK_INDENT: usize = 2;
/// Column gap between the mark and the text beside it.
const LAUNCH_MARK_GAP: usize = 3;
/// The card's reading measure: a row is a title with its detail set against
/// it, and without a ceiling the detail right-aligns against the terminal's
/// far edge. Titles persist at 50 characters, the detail reads ~20.
const LAUNCH_CARD_MEASURE: usize = 72;
/// Gap between a row's title and its right-aligned detail.
const LAUNCH_ROW_GAP: usize = 3;
/// Below this the row spends its whole lane on the title and sheds the detail.
const LAUNCH_ROW_MIN_TITLE: usize = 24;
/// The recent list and its overflow row sit under the `Recent` heading.
const LAUNCH_LIST_INDENT: usize = 2;
/// Blank rows the card spends on rhythm when the pane is tall enough.
const LAUNCH_SEPARATORS: usize = 3;
/// Blank rows per separator when the pane can afford them.
const LAUNCH_GAP_ROOMY: usize = 2;
/// Below this width the block gives up its left indent.
const LAUNCH_INDENT_MIN_WIDTH: usize = 12;

pub fn empty_state_lines(app: &App, area: Rect) -> Vec<Line<'static>> {
    if area.width == 0 || area.height == 0 {
        return Vec::new();
    }
    // The opening screen is this screen: the launch card is the idle
    // transcript's own content, not a second surface painted over it.
    if app.launch.visible {
        // The first keystroke starts the dissolve clock; the card sinks by
        // ink, not by position — every span eases toward the water behind it
        // over `LAUNCH_CARD_DISSOLVE_MS`, and at the end the ambient surface
        // (wordmark, caption, prompt) is what remains. Reduced motion takes
        // the endpoint at once.
        let motion_allowed = app.motion_policy().allows_decorative() && !app.low_motion;
        let dissolve = app
            .launch
            .card_dissolve_progress(app.ambient_clock_ms, motion_allowed);
        if dissolve < 1.0 {
            let mut state = launch_empty_state(app, area);
            if dissolve > 0.0 {
                fade_lines(&mut state.lines, dissolve, app.ui_theme.surface_bg);
            }
            return state.lines;
        }
    }
    let width = usize::from(area.width);
    let mut lines = vec![Line::from(""); usize::from(area.height / 4)];
    // The idle whale portrait that used to open this block was deleted per
    // the 2026-08-29 founder directive; the ambient empty-state surface
    // (wordmark, context caption, prompt) is not whale art and stays.

    let identity = crate::tui::workspace_context::identity_from_context(
        &app.workspace,
        app.workspace_context.as_deref(),
    );
    let workspace = crate::utils::display_path(&app.workspace);
    let branch = identity.branch.as_deref().map_or_else(
        || tr(app.ui_locale, MessageId::EmptyStateNoGit),
        |branch| Cow::Owned(branch.to_string()),
    );
    // Compact used to bypass the caption entirely and print the bare branch,
    // which in a plain folder rendered as the single centred word "no git" —
    // a whole row of the hero spent naming something that is not there. The
    // shedding ladder already degrades gracefully at any width, so every tier
    // now goes through it.
    let context = empty_state_caption(
        &workspace,
        &branch,
        tr(app.ui_locale, MessageId::EmptyStateMcpLabel).as_ref(),
        app.mcp_configured_count,
        width,
    );
    let brand = "codewhale";
    let brand_inset = " ".repeat(width.saturating_sub(brand.width()) / 2);
    lines.push(Line::from(Span::styled(
        format!("{brand_inset}{brand}"),
        Style::default()
            .fg(app.ui_theme.text_body)
            .add_modifier(Modifier::BOLD),
    )));
    let context = truncate_to_width(&context, width);
    let inset = " ".repeat(width.saturating_sub(context.width()) / 2);
    lines.push(Line::from(Span::styled(
        format!("{inset}{context}"),
        Style::default().fg(app.ui_theme.text_soft),
    )));
    if area.height >= 4 {
        lines.push(Line::from(""));
        let prompt = tr(app.ui_locale, MessageId::EmptyStatePrompt);
        let prompt = truncate_to_width(prompt.as_ref(), width);
        let inset = " ".repeat(width.saturating_sub(prompt.width()) / 2);
        lines.push(Line::from(Span::styled(
            format!("{inset}{prompt}"),
            Style::default().fg(app.ui_theme.text_body),
        )));
    }
    lines
}

/// The launch screen's MCP block: what actually became of the configured
/// servers, painted under the recent-work list.
///
/// The Tideline footer has one clause for this whole fact, so a 23-server
/// workspace rendered as `MCP · 1 connecting · alibaba-cloud-ops` — one
/// arbitrary name, every failure hidden (founder, 2026-09-09). The launch
/// screen has the rows the footer does not, so the two states that carry a
/// remedy get a row each and *name* their servers; the healthy majority stays
/// a count, because a list of things that worked is not information. A server
/// that needs a login and a server that could not connect are different
/// problems with different fixes, so they never share a row.
///
/// State comes from [`crate::tui::session_boot::SessionBootSurface`], the one
/// MCP status owner; this is only its launch projection, and it computes
/// nothing about a server itself.
///
/// The rows are informational, not selectable. A fourth interactive row would
/// have to join `LaunchRowId` and the paint/click/keyboard ordering the card
/// shares, and it would buy nothing the block cannot already say: the composer
/// below has focus from the first frame, so the block simply prints the exact
/// command to type.
fn mcp_launch_lines(app: &App, text_width: usize) -> Vec<Line<'static>> {
    use crate::tui::session_boot::{
        ITEM_SEPARATOR, McpServerBootState, PluginBootSummary, SessionBootPhase, SessionBootSurface,
    };

    // MCP only: the plugin half of the boot surface has its own footer chip
    // and its own screen, and walking the plugin registry every frame to
    // discard it would be waste.
    let boot = SessionBootSurface::from_parts(
        app.mcp_snapshot.as_ref(),
        app.mcp_initializing,
        &app.mcp_connecting,
        app.mcp_configured_count,
        PluginBootSummary::default(),
    );
    if boot.phase == SessionBootPhase::Hidden || text_width == 0 {
        return Vec::new();
    }
    let theme = &app.ui_theme;
    let locale = app.ui_locale;
    let names_in = |state: McpServerBootState| -> Vec<&str> {
        boot.servers
            .iter()
            .filter(|row| row.state == state)
            .map(|row| row.name.as_str())
            .collect()
    };
    let failed = names_in(McpServerBootState::Failed);
    let needs_login = names_in(McpServerBootState::NeedsLogin);
    let connected = names_in(McpServerBootState::Connected).len();
    // Before the first boot event the names have not arrived, but the count
    // has; without it a 23-server workspace would paint nothing at all.
    let connecting = names_in(McpServerBootState::Connecting)
        .len()
        .max(boot.connecting_without_names());

    let indent = LAUNCH_LIST_INDENT.min(text_width.saturating_sub(1));
    let lane = text_width.saturating_sub(indent);
    let mut lines: Vec<Line<'static>> = Vec::new();

    // The summary carries every non-zero state, because it is also the floor:
    // when the pane can afford one row of this block, that row still has to
    // say two servers failed. Order is by what the reader must act on, so the
    // tail shed below gives up `connected` first — and while anything is in
    // flight that clause leads, since a boot that has connected nothing yet
    // must never read as a boot that finished with nothing connected.
    // `label (n)` rather than `n label`: number agreement is a grammar this
    // renderer cannot get right in fifteen languages.
    let mut parts: Vec<String> = Vec::new();
    let mut count_part = |id: MessageId, count: usize| {
        if count > 0 {
            parts.push(format!("{} ({count})", tr(locale, id)));
        }
    };
    count_part(MessageId::McpStateConnecting, connecting);
    count_part(MessageId::McpStateFailed, failed.len());
    count_part(MessageId::McpStateAuthorizationRequired, needs_login.len());
    count_part(MessageId::ExtensionsStateConnected, connected);
    if parts.is_empty() {
        parts.push(format!(
            "{} (0)",
            tr(locale, MessageId::ExtensionsStateConnected)
        ));
    }
    let mut summary = format!("MCP{ITEM_SEPARATOR}{}", parts.join(ITEM_SEPARATOR));
    while parts.len() > 1 && text_display_width(&summary) > text_width {
        parts.pop();
        summary = format!("MCP{ITEM_SEPARATOR}{}", parts.join(ITEM_SEPARATOR));
    }
    lines.push(Line::from(Span::styled(
        semantic_truncate(&summary, text_width),
        Style::default().fg(theme.text_muted),
    )));

    // One problems row answers *which* and *what to type*: `✕` groups the
    // failed names, `⚠` switches the group to names that want a login, and
    // the remedy rides at the tail. Narrow panes shed the hint, then names
    // from the tail into `+N`, and finally the row itself — never the
    // summary. Glyph *and* state grouping carry the difference, so it
    // survives a monochrome terminal and a colour-blind reader.
    if !failed.is_empty() || !needs_login.is_empty() {
        let hint = match (needs_login.first(), failed.is_empty()) {
            (Some(name), true) => format!("/mcp login {name}"),
            (Some(name), false) => format!("/mcp{ITEM_SEPARATOR}/mcp login {name}"),
            (None, false) => "/mcp".to_string(),
            (None, true) => String::new(),
        };
        let problems = mcp_problems_row(&failed, &needs_login, &hint, lane);
        if let Some(text) = problems {
            let mut spans = Vec::with_capacity(2);
            if indent > 0 {
                spans.push(Span::raw(" ".repeat(indent)));
            }
            spans.push(Span::styled(
                text,
                Style::default().fg(if failed.is_empty() {
                    theme.warning
                } else {
                    theme.error_fg
                }),
            ));
            lines.push(Line::from(spans));
        }
    }
    lines
}

/// One problems row: `✕ alibaba-cloud-ops · aws-mcp · ⚠ slack +4 · /mcp`.
///
/// `✕` opens the failed group, `⚠` opens the needs-login group, and the
/// remedy sits at the tail. Shedding folds names into `+N` from the tail
/// while the remedy holds — at each width the named command is tried first,
/// then bare `/mcp`, then none — and when even `✕ +2 · ⚠ +5` cannot fit
/// the row is dropped whole: the summary above still says the count.
fn mcp_problems_row(
    failed: &[&str],
    needs_login: &[&str],
    hint: &str,
    lane: usize,
) -> Option<String> {
    use crate::tui::glyphs::{ATTENTION, FAILED};
    use crate::tui::session_boot::ITEM_SEPARATOR;

    let group = |glyph: &str, names: &[&str], shown: usize, row: &mut String| {
        if names.is_empty() {
            return;
        }
        if !row.is_empty() {
            row.push_str(ITEM_SEPARATOR);
        }
        row.push_str(glyph);
        row.push(' ');
        if shown > 0 {
            row.push_str(&names[..shown].join(ITEM_SEPARATOR));
            let extra = names.len() - shown;
            if extra > 0 {
                row.push_str(ITEM_SEPARATOR);
                row.push('+');
                row.push_str(&extra.to_string());
            }
        } else {
            row.push('+');
            row.push_str(&names.len().to_string());
        }
    };
    let total = failed.len() + needs_login.len();
    for shown in (0..=total).rev() {
        let failed_shown = shown.min(failed.len());
        let login_shown = shown.saturating_sub(failed_shown);
        let mut body = String::new();
        group(FAILED, failed, failed_shown, &mut body);
        group(ATTENTION, needs_login, login_shown, &mut body);
        for tail in [hint, "/mcp", ""] {
            if tail.is_empty() && !hint.is_empty() && shown > 0 {
                continue;
            }
            let mut row = body.clone();
            if !tail.is_empty() {
                row.push_str(ITEM_SEPARATOR);
                row.push_str(tail);
            }
            if text_display_width(&row) <= lane {
                return Some(row);
            }
        }
    }
    None
}

/// Ease every painted glyph toward the water behind it. `dissolve` is
/// `card_dissolve_progress` — 0 is full ink, 1 is gone. Spans without a
/// foreground (padding, blanks) carry nothing to fade.
fn fade_lines(lines: &mut [Line<'static>], dissolve: f32, water: Color) {
    for line in lines.iter_mut() {
        for span in line.spans.iter_mut() {
            if let Some(fg) = span.style.fg {
                span.style.fg = Some(crate::tui::mark::lerp_color(fg, water, dissolve));
            }
        }
    }
}

/// How much of the card fits the pane. `New session` is never shed: it is the
/// screen's one actionable choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LaunchFit {
    brand: bool,
    help: bool,
    notice: bool,
    heading: bool,
    blanks: usize,
    shown: usize,
    see_all: bool,
    /// Rows of the MCP block, tail-first as the pane shrinks. The block
    /// always costs one separator row on top of these, so it never reads as
    /// another session in the list above it.
    mcp: usize,
    /// Blank rows per separator. A pane with height to spare spends it on
    /// breathing room before it spends it on more content: one blank line
    /// between blocks packs the card into the top-left corner of a tall
    /// terminal and reads as clutter even when every row is earning its place.
    /// This is the first thing shed, so a short pane is unaffected.
    gap: usize,
}

impl LaunchFit {
    const fn rows(self) -> usize {
        (self.brand as usize)
            + (self.help as usize)
            + (self.notice as usize)
            + self.blanks * self.gap
            + 1
            + (self.heading as usize)
            + self.shown
            + (self.see_all as usize)
            + self.mcp
            + (self.mcp > 0) as usize * self.gap
    }
}

/// Shed the card down to `height`, in a fixed order: rhythm, the migration
/// notice, the MCP block's detail, the tail of the recent list — which turns
/// the overflow row on, so nothing shed becomes unreachable — then chrome.
///
/// The MCP block gives up its rows before the recent list does (recent work
/// is what the screen is *for*) but keeps its summary line until almost
/// everything else has gone, because "2 failed" in one row still tells the
/// truth that the footer chip could not.
fn launch_fit(height: usize, recent: usize, has_more: bool, notice: bool, mcp: usize) -> LaunchFit {
    let mut fit = LaunchFit {
        brand: true,
        help: true,
        notice,
        heading: true,
        blanks: LAUNCH_SEPARATORS,
        shown: recent,
        see_all: has_more,
        mcp,
        gap: LAUNCH_GAP_ROOMY,
    };
    let mut step = 0u8;
    while fit.rows() > height {
        match step {
            // Breathing room is the first luxury to go, before any content.
            0 => fit.gap = 1,
            1 => fit.blanks = 1,
            2 => fit.blanks = 0,
            3 => fit.notice = false,
            4 => {
                while fit.mcp > 1 && fit.rows() > height {
                    fit.mcp -= 1;
                }
            }
            5 => {
                while fit.shown > 0 && fit.rows() > height {
                    fit.shown -= 1;
                    fit.see_all = true;
                }
            }
            6 => fit.help = false,
            7 => fit.heading = false,
            8 => fit.brand = false,
            9 => fit.mcp = 0,
            10 => fit.see_all = false,
            _ => break,
        }
        step += 1;
    }
    fit
}

pub fn launch_empty_state(app: &App, area: Rect) -> LaunchEmptyState {
    if area.width == 0 || area.height == 0 {
        return LaunchEmptyState {
            lines: Vec::new(),
            rows: Vec::new(),
        };
    }
    let width = usize::from(area.width);
    let height = usize::from(area.height);
    let theme = &app.ui_theme;
    let locale = app.ui_locale;
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut rows: Vec<(crate::tui::app::LaunchRowId, usize)> = Vec::new();

    // The mark rides beside the text rather than above it, so the block reads
    // as one top-left unit. It sheds a rung at a time, then out entirely,
    // before any row of text is given up. ASCII-safe terminals get no mark.
    let mark_rung = if crate::tui::color_compat::ascii_safe_enabled() {
        None
    } else if width >= 64 && area.height >= 10 {
        Some(MarkSize::Large)
    } else if width >= 44 && area.height >= 7 {
        Some(MarkSize::Small)
    } else if width >= 32 && area.height >= 5 {
        Some(MarkSize::Tiny)
    } else {
        None
    };
    let (mark_rows, mark_width): (Vec<&'static str>, usize) = match mark_rung {
        None => (Vec::new(), 0),
        Some(rung) => (rung.rows().to_vec(), usize::from(rung.cells().0)),
    };
    let block_indent = if width >= LAUNCH_INDENT_MIN_WIDTH {
        LAUNCH_BLOCK_INDENT
    } else {
        0
    };
    let text_indent = block_indent
        + if mark_width == 0 {
            0
        } else {
            mark_width + LAUNCH_MARK_GAP
        };
    // The lane is what is left beside the mark; the measure is what the card
    // uses of it, so a row's detail stays beside its title.
    let text_width = width.saturating_sub(text_indent).min(LAUNCH_CARD_MEASURE);
    if text_width == 0 {
        return LaunchEmptyState {
            lines: Vec::new(),
            rows: Vec::new(),
        };
    }

    let (entries, has_more) = launch_recent_entries(app);
    // Whether this workspace has recent work at all, before any is shed for
    // height: the heading must not say "no recent sessions" about a list that
    // only ran out of rows.
    let had_recent = !entries.is_empty();
    // Built before the fit ladder runs: how many rows the block wants is a
    // fact about this workspace's servers, not about the pane.
    let mcp_lines = mcp_launch_lines(app, text_width);
    let fit = launch_fit(
        height,
        entries.len(),
        has_more,
        app.launch.claude_code_detected,
        mcp_lines.len(),
    );
    let spacious = fit.blanks == LAUNCH_SEPARATORS;
    let visible: Vec<LaunchRecentEntry> = entries.into_iter().take(fit.shown).collect();
    let card_rows = launch_card_rows(locale, &visible, fit.see_all);

    // The text column, in order. `None` is a blank row.
    let mut text: Vec<Option<Line<'static>>> = Vec::new();
    if fit.brand {
        let brand = "codewhale";
        let version = format!("v{}", env!("CODEWHALE_BUILD_VERSION"));
        let mut spans = vec![Span::styled(
            semantic_truncate(brand, text_width),
            Style::default().fg(theme.accent_primary).bold(),
        )];
        if text_width >= text_display_width(brand) + 1 + text_display_width(&version) {
            spans.push(Span::styled(
                format!(" {version}"),
                Style::default().fg(theme.text_muted),
            ));
        }
        text.push(Some(Line::from(spans)));
    }
    // What to press, on the one screen where it has not been learned yet.
    if fit.help {
        text.push(Some(Line::from(Span::styled(
            semantic_truncate(
                &tr(locale, MessageId::LaunchHelpLine).replace(
                    "{dock}",
                    crate::tui::shell_key_routing::binding(
                        crate::tui::shell_key_routing::ShellBindingId::ViewCycle,
                    )
                    .footer_chord,
                ),
                text_width,
            ),
            Style::default().fg(theme.text_hint),
        ))));
    }
    // The migration notice, while there is still a question to answer. It
    // retires for good once `/import-claude` has been run.
    if fit.notice {
        text.push(Some(Line::from(Span::styled(
            semantic_truncate(&tr(locale, MessageId::LaunchNoticeClaude), text_width),
            Style::default().fg(theme.text_muted),
        ))));
    }
    if fit.blanks >= 1 {
        for _ in 0..fit.gap {
            text.push(None);
        }
    }

    for row in &card_rows {
        let style = if row.prominent {
            Style::default().fg(theme.accent_action).bold()
        } else {
            Style::default().fg(theme.text_soft)
        };
        // The recent list and its overflow hang under the heading.
        let indent = match row.id {
            crate::tui::app::LaunchRowId::NewSession => 0,
            _ => LAUNCH_LIST_INDENT.min(text_width.saturating_sub(1)),
        };
        if matches!(row.id, crate::tui::app::LaunchRowId::SeeAll) && spacious {
            for _ in 0..fit.gap {
                text.push(None);
            }
        }
        let lane = text_width.saturating_sub(indent);
        let detail_width = text_display_width(&row.detail);
        // Age and message count are context, not the row: when the lane
        // cannot hold a readable title beside them they are dropped whole
        // rather than ellipsing the title to a stub. This is also the
        // fallback for a locale that spends more cells on the same fact.
        let detail = if row.detail.is_empty()
            || lane < LAUNCH_ROW_MIN_TITLE + LAUNCH_ROW_GAP + detail_width
        {
            ""
        } else {
            row.detail.as_str()
        };
        let label_budget = if detail.is_empty() {
            lane
        } else {
            lane.saturating_sub(LAUNCH_ROW_GAP + detail_width)
        };
        let label = semantic_truncate(&row.label, label_budget);
        let label_width = text_display_width(&label);
        let mut spans = Vec::with_capacity(4);
        if indent > 0 {
            spans.push(Span::raw(" ".repeat(indent)));
        }
        spans.push(Span::styled(label, style));
        if !detail.is_empty() {
            let pad = lane
                .saturating_sub(label_width)
                .saturating_sub(detail_width);
            spans.push(Span::raw(" ".repeat(pad)));
            spans.push(Span::styled(
                detail.to_string(),
                Style::default().fg(theme.text_dim),
            ));
        }
        rows.push((row.id.clone(), text.len()));
        text.push(Some(Line::from(spans)));
        if matches!(row.id, crate::tui::app::LaunchRowId::NewSession) && fit.heading {
            // With no resumable work the heading says so — but only when the
            // workspace genuinely has none. Sessions the card's filter drops
            // (empty auto-created shells) still exist in `/resume`, so their
            // presence earns the honest "Recent" + a see-all row, not a
            // "no recent sessions" the picker would immediately disprove.
            let heading = if had_recent || app.launch.has_scoped_sessions {
                MessageId::LaunchRecentHeading
            } else {
                MessageId::LaunchNoRecentSessions
            };
            if spacious {
                for _ in 0..fit.gap {
                    text.push(None);
                }
            }
            text.push(Some(Line::from(Span::styled(
                semantic_truncate(&tr(locale, heading), text_width),
                Style::default().fg(theme.text_muted),
            ))));
        }
    }

    // MCP status, under the recent-work list, where the founder asked for it
    // (2026-09-09): the footer chip could name one server out of 23 and hid
    // every failure behind a count.
    if fit.mcp > 0 {
        for _ in 0..fit.gap {
            text.push(None);
        }
        text.extend(mcp_lines.into_iter().take(fit.mcp).map(Some));
    }

    // The whale still surfaces. It rises by ink rather than by position: at 0
    // it is exactly the water behind it and eases to full over
    // `MARK_SURFACE_MS`. Reduced motion gets the endpoint.
    let rise = if app.motion_policy().allows_decorative() && !app.low_motion {
        crate::tui::mark::surface_progress(app.ambient_clock_ms, MARK_SURFACE_MS)
    } else {
        1.0
    };
    let ink_color = crate::tui::mark::lerp_color(theme.surface_bg, theme.accent_primary, rise);

    // Compose: the mark column on the left, the text column beside it. The
    // block is as tall as whichever column is taller, and never taller than
    // the pane it is drawn into.
    let block_rows = mark_rows.len().max(text.len()).min(height);
    let mut row_offsets: Vec<usize> = Vec::with_capacity(block_rows);
    for row in 0..block_rows {
        let mut spans = vec![Span::raw(" ".repeat(block_indent))];
        if mark_width > 0 {
            let ink = mark_rows.get(row).copied().unwrap_or("");
            let pad = mark_width.saturating_sub(ink.width());
            spans.push(Span::styled(
                ink.to_string(),
                Style::default().fg(ink_color),
            ));
            spans.push(Span::raw(" ".repeat(pad + LAUNCH_MARK_GAP)));
        }
        if let Some(Some(line)) = text.get(row) {
            spans.extend(line.spans.iter().cloned());
        }
        row_offsets.push(lines.len());
        lines.push(Line::from(spans));
    }

    // Re-point the hitboxes at the composed rows. A row the block could not
    // fit has no offset, so it has no hitbox either.
    let rows = rows
        .into_iter()
        .filter_map(|(id, text_row)| row_offsets.get(text_row).map(|y| (id, *y)))
        .collect();

    LaunchEmptyState { lines, rows }
}

#[cfg(test)]
mod launch_card_tests {
    use super::{
        LAUNCH_CARD_MEASURE, LaunchAction, launch_empty_state, launch_fit, launch_rows_for_app,
        refresh_launch_row_hitboxes, run_launch_card_row, text_display_width,
    };
    use crate::tui::app::{App, LaunchRecentSession, LaunchRowId};
    use ratatui::layout::Rect;
    use ratatui::text::Line;
    use unicode_segmentation::UnicodeSegmentation;

    fn app_with_recent(titles: &[&str], total: usize) -> App {
        let mut app = crate::test_support::test_app_with_options(
            crate::test_support::test_tui_options(std::env::temp_dir()),
        );
        app.launch.visible = true;
        app.launch.claude_code_detected = false;
        app.launch.recent = titles
            .iter()
            .enumerate()
            .map(|(index, title)| LaunchRecentSession {
                id: format!("{index}0abcdef-session"),
                title: (*title).to_string(),
                updated_at: chrono::Utc::now()
                    - chrono::Duration::hours(i64::try_from(index).unwrap_or(0) + 1),
                message_count: 40 + index,
            })
            .collect();
        app.launch.total_workspace_sessions = total;
        app
    }

    /// The founder's own shape: many servers, a couple genuinely broken, a
    /// pile sitting unauthenticated, the rest fine.
    fn with_mcp(mut app: App) -> App {
        use crate::mcp::{McpManagerSnapshot, McpServerCapabilityMetadata, McpServerSnapshot};
        let mut servers = Vec::new();
        let mut push = |name: &str, connected: bool, error: Option<&str>, auth: bool| {
            servers.push(McpServerSnapshot {
                name: name.to_string(),
                enabled: true,
                required: false,
                transport: "stdio".to_string(),
                command_or_url: format!("cmd-{name}"),
                connect_timeout: 5,
                execute_timeout: 5,
                read_timeout: 5,
                connected,
                error: error.map(str::to_string),
                auth_required: auth,
                capability_metadata: McpServerCapabilityMetadata::NotObserved,
                tools: Vec::new(),
                resources: Vec::new(),
                prompts: Vec::new(),
            });
        };
        for name in ["github", "linear", "supabase", "posthog", "vercel"] {
            push(name, true, None, false);
        }
        push(
            "alibaba-cloud-ops",
            false,
            Some("Invalid request parameters"),
            false,
        );
        push("aws-mcp", false, Some("Stdio transport closed"), false);
        for name in ["slack", "notion", "stripe", "figma", "excalidraw"] {
            push(name, false, Some("401 Unauthorized"), true);
        }
        app.mcp_configured_count = servers.len();
        app.mcp_snapshot = Some(McpManagerSnapshot {
            config_path: std::path::PathBuf::from("mcp.json"),
            config_exists: true,
            reload_required: false,
            servers,
        });
        app.mcp_initializing = false;
        app.mcp_connecting = Vec::new();
        app
    }

    fn flatten(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.to_string())
            .collect::<String>()
    }

    fn painted(app: &App, width: u16, height: u16) -> Vec<String> {
        launch_empty_state(app, Rect::new(0, 0, width, height))
            .lines
            .iter()
            .map(|line| flatten(line).trim_end().to_string())
            .collect()
    }

    fn row_ids(app: &App) -> Vec<LaunchRowId> {
        app.launch
            .row_hitboxes
            .iter()
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// The recent row's own text, with the block indent stripped. Only used at
    /// widths narrow enough that the detail has shed, so what remains is the
    /// title alone.
    fn recent_row_title(app: &App, width: u16, height: u16) -> String {
        let state = launch_empty_state(app, Rect::new(0, 0, width, height));
        let (_, row) = state
            .rows
            .iter()
            .find(|(id, _)| matches!(id, LaunchRowId::Recent(_)))
            .expect("a recent row painted");
        flatten(&state.lines[*row]).trim().to_string()
    }

    fn is_grapheme_prefix(candidate: &str, full: &str) -> bool {
        let mut source = full.graphemes(true);
        candidate.graphemes(true).all(|g| source.next() == Some(g))
    }

    // --- one ordering for paint, mouse, and keyboard -------------------

    #[test]
    fn keyboard_rows_are_exactly_the_rows_the_pane_painted() {
        let mut app = app_with_recent(&["one", "two", "three", "four", "five"], 9);

        refresh_launch_row_hitboxes(&mut app, Rect::new(0, 0, 120, 30));
        let tall = launch_rows_for_app(&app);
        assert_eq!(
            tall.iter().map(|row| row.id.clone()).collect::<Vec<_>>(),
            row_ids(&app),
            "keyboard list must be the painted list",
        );
        assert!(tall.len() >= 7, "{:?}", row_ids(&app));

        // Highlight the last row, then shrink the pane under it.
        app.launch.menu_selected = Some(tall.len() - 1);
        refresh_launch_row_hitboxes(&mut app, Rect::new(0, 0, 120, 9));
        let short = launch_rows_for_app(&app);
        assert_eq!(
            short.iter().map(|row| row.id.clone()).collect::<Vec<_>>(),
            row_ids(&app),
        );
        assert!(short.len() < tall.len(), "the short pane shed nothing");

        // The stale highlight is gone, so Enter cannot resume a row that is
        // no longer on screen.
        assert_eq!(app.launch.menu_selected, None);
        assert_eq!(
            run_launch_card_row(&short, app.launch.menu_selected),
            LaunchAction::None,
        );
    }

    #[test]
    fn no_keyboard_row_names_a_session_the_pane_is_not_showing() {
        let mut app = app_with_recent(&["one", "two", "three", "four", "five"], 5);
        for height in 1u16..=14 {
            refresh_launch_row_hitboxes(&mut app, Rect::new(0, 0, 120, height));
            let painted: Vec<LaunchRowId> = row_ids(&app);
            for row in launch_rows_for_app(&app) {
                assert!(
                    painted.contains(&row.id),
                    "height {height}: {:?} is runnable but was not painted",
                    row.id,
                );
            }
            // Every arrow position runs a painted row or nothing at all.
            for index in 0..painted.len() + 3 {
                let rows = launch_rows_for_app(&app);
                match run_launch_card_row(&rows, Some(index)) {
                    LaunchAction::None => {}
                    LaunchAction::ResumeSession(id) => assert!(
                        painted.contains(&LaunchRowId::Recent(id.clone())),
                        "height {height}: Enter at {index} would resume unpainted {id}",
                    ),
                    _ => {}
                }
            }
        }
    }

    #[test]
    fn shed_sessions_stay_reachable_through_the_overflow_row() {
        let mut app = app_with_recent(&["one", "two", "three", "four", "five"], 5);
        // Five sessions, none behind the inline list: a tall pane needs no
        // overflow row, a short one sheds and therefore must offer it.
        refresh_launch_row_hitboxes(&mut app, Rect::new(0, 0, 120, 30));
        assert!(!row_ids(&app).contains(&LaunchRowId::SeeAll));
        refresh_launch_row_hitboxes(&mut app, Rect::new(0, 0, 120, 7));
        let ids = row_ids(&app);
        assert!(ids.contains(&LaunchRowId::SeeAll), "{ids:?}");
        assert!(
            launch_rows_for_app(&app)
                .iter()
                .any(|row| row.id == LaunchRowId::SeeAll)
        );
    }

    // --- the card fits the pane it is drawn into -----------------------

    #[test]
    fn the_card_fits_every_pane_it_is_drawn_into() {
        // Both shapes: no MCP servers at all, and the twelve-server workspace
        // whose status block is the widest thing the card paints.
        for app in [
            app_with_recent(&["one", "two", "three", "four", "five"], 9),
            with_mcp(app_with_recent(&["one", "two", "three", "four", "five"], 9)),
        ] {
            for width in [0u16, 1, 2, 3, 8, 12, 20, 31, 32, 40, 44, 64, 80, 120, 200] {
                for height in 0u16..=10 {
                    let state = launch_empty_state(&app, Rect::new(0, 0, width, height));
                    assert!(
                        state.lines.len() <= usize::from(height),
                        "{width}x{height}: {} lines",
                        state.lines.len(),
                    );
                    for line in &state.lines {
                        let painted = text_display_width(&flatten(line));
                        assert!(
                            painted <= usize::from(width),
                            "{width}x{height}: row of {painted} cells",
                        );
                    }
                    for (id, row) in &state.rows {
                        assert!(
                            *row < state.lines.len(),
                            "{width}x{height}: hitbox {id:?} has no row",
                        );
                    }
                    if width > 0 && height > 0 {
                        assert!(
                            state
                                .rows
                                .iter()
                                .any(|(id, _)| matches!(id, LaunchRowId::NewSession)),
                            "{width}x{height}: nothing actionable painted",
                        );
                    }
                }
            }
        }
    }

    // --- MCP status, under the recent list ------------------------------

    /// The defect this block was built for: the footer chip named one server
    /// out of twenty-three and reported every other state as a bare count, so
    /// ten servers sitting unauthenticated were invisible. Failed and
    /// needs-login are different problems with different fixes and must never
    /// collapse into one clause — and the block is two lines at most now:
    /// the summary owns the counts, one problems row names what broke.
    #[test]
    fn the_mcp_block_names_what_broke_and_separates_it_from_what_needs_a_login() {
        let app = with_mcp(app_with_recent(&["one", "two"], 2));
        let lines = painted(&app, 120, 30);
        // The summary owns the totals, and every non-zero state is on it.
        let summary = lines
            .iter()
            .find(|line| line.contains("MCP"))
            .expect("the MCP summary");
        assert!(summary.contains("connection failed (2)"), "{summary:?}");
        assert!(
            summary.contains("authorization required (5)"),
            "ten servers at not-logged-in were invisible before this: {summary:?}",
        );
        // One problems row answers *which*: ✕ groups the failures, ⚠ groups
        // the logins, and the remedy rides at the tail.
        let problems: Vec<_> = lines
            .iter()
            .filter(|line| {
                line.contains(crate::tui::glyphs::FAILED)
                    || line.contains(crate::tui::glyphs::ATTENTION)
            })
            .collect();
        assert_eq!(problems.len(), 1, "one problems row: {lines:#?}");
        let row = problems[0];
        assert!(row.contains("alibaba-cloud-ops"), "{row:?}");
        assert!(row.contains("aws-mcp"), "{row:?}");
        assert!(row.contains("slack"), "{row:?}");
        // The remedy is the command to type, not advice about typing one —
        // the named form sheds to bare `/mcp` before any name does.
        assert!(row.contains("/mcp"), "{row:?}");
    }

    /// While the boot is in flight the block must not read as a finished one.
    #[test]
    fn a_boot_in_flight_says_connecting_rather_than_connected() {
        let mut app = app_with_recent(&["one"], 1);
        app.mcp_initializing = true;
        app.mcp_configured_count = 23;
        app.mcp_connecting = Vec::new();
        let lines = painted(&app, 120, 30);
        let summary = lines
            .iter()
            .find(|line| line.contains("MCP"))
            .expect("the MCP summary");
        assert!(summary.contains("connecting (23)"), "{summary:?}");
        assert!(
            !summary.contains("connected"),
            "a boot with nothing connected yet claimed a count: {summary:?}",
        );
    }

    #[test]
    fn the_fit_ladder_never_sheds_the_one_actionable_row() {
        for height in 1usize..=16 {
            for recent in 0usize..=5 {
                for has_more in [false, true] {
                    for notice in [false, true] {
                        for mcp in 0usize..=4 {
                            let fit = launch_fit(height, recent, has_more, notice, mcp);
                            assert!(fit.rows() <= height.max(1), "{height} {recent}: {fit:?}");
                            assert!(fit.shown <= recent);
                            assert!(fit.mcp <= mcp);
                            if fit.shown < recent {
                                assert!(
                                    fit.see_all || fit.rows() >= height,
                                    "shed rows became unreachable: {fit:?}",
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    // --- the row reads as one object -----------------------------------

    #[test]
    fn a_row_keeps_its_detail_beside_its_title() {
        let app = app_with_recent(&["Ship the launch card"], 1);
        let lines = painted(&app, 200, 24);
        let row = lines
            .iter()
            .find(|line| line.contains("Ship the launch card"))
            .expect("recent row painted");
        assert!(
            text_display_width(row) <= LAUNCH_CARD_MEASURE + 32,
            "row runs to the terminal edge: {row:?}",
        );
        assert!(row.contains("msgs"), "row lost its detail: {row:?}");
    }

    #[test]
    fn a_narrow_pane_spends_its_lane_on_the_title() {
        let app = app_with_recent(&["Ship the launch card"], 1);
        let row = recent_row_title(&app, 40, 24);
        assert!(!row.contains("msgs"), "detail should have shed: {row:?}");
        assert!(row.starts_with("Ship the launch"), "{row:?}");
    }

    // --- scripts --------------------------------------------------------

    #[test]
    fn titles_truncate_on_grapheme_boundaries_in_several_scripts() {
        // Not a claim about every script: these are the four shapes that break
        // char-indexed truncation — wide cells, RTL runs, ZWJ/skin-tone emoji,
        // and combining marks.
        let samples = [
            (
                "latin",
                "Ship the launch card and verify truncation behaviour",
            ),
            ("cjk", "部署新的会话启动卡片并验证宽字符截断行为"),
            ("arabic", "تهيئة بطاقة إطلاق الجلسة والتحقق من سلوك الاقتطاع"),
            ("hebrew", "הגדרת כרטיס פתיחת הפעלה ואימות התנהגות הקיצוץ"),
            ("emoji", "👩🏽‍🚀 crew 👨‍👩‍👧‍👦 families and flags 🇯🇵 shipping today"),
            (
                "combining",
                "cafe\u{301} de\u{301}ja\u{300} vu\u{308} with combining marks throughout",
            ),
        ];
        for (name, title) in samples {
            let app = app_with_recent(&[title], 1);
            for width in [8u16, 12, 20, 32, 40, 60, 80, 120, 200] {
                for line in painted(&app, width, 24) {
                    assert!(
                        text_display_width(&line) <= usize::from(width),
                        "{name} at {width}: {line:?} overruns the pane",
                    );
                }
            }
            // At these widths the detail has shed, so the row is the title
            // alone: what is painted must be whole graphemes off its front.
            for width in [12u16, 20, 32, 40] {
                let row = recent_row_title(&app, width, 24);
                let body = row.strip_suffix('…').unwrap_or(&row);
                assert!(
                    is_grapheme_prefix(body, title),
                    "{name} at {width}: {body:?} splits a grapheme of {title:?}",
                );
            }
        }
    }

    // --- rhythm ---------------------------------------------------------

    #[test]
    fn a_tall_pane_breathes_and_a_short_one_gives_the_rhythm_up_first() {
        // Counting blank *painted* rows would be wrong: the mark column sits
        // behind the first six of them. The rhythm is the gap between the
        // new-session entry and the heading below it.
        let app = app_with_recent(&["one", "two", "three", "four", "five"], 9);

        let tall = painted(&app, 120, 30);
        let entry = tall
            .iter()
            .position(|line| line.contains("New session"))
            .expect("new-session row painted");
        assert!(
            !tall[entry + 1].contains("Recent"),
            "a tall pane should breathe: {tall:#?}",
        );

        let tight = painted(&app, 120, 12);
        let entry = tight
            .iter()
            .position(|line| line.contains("New session"))
            .expect("new-session row painted");
        assert!(
            tight[entry + 1].contains("Recent"),
            "a tight pane kept rhythm it cannot afford: {tight:#?}",
        );
    }

    /// Print the card as the renderer actually paints it, for the evidence
    /// fixture. Ignored by default; run with
    /// `cargo test -p codewhale-tui --lib render_launch_card_fixture -- --ignored --nocapture`.
    #[test]
    #[ignore = "fixture generator, not an assertion"]
    #[allow(clippy::print_stdout)] // the fixture's whole job is its stdout
    fn render_launch_card_fixture() {
        let app = with_mcp(app_with_recent(
            &[
                "Fix the diagnostic display",
                "Hunter has explicitly authorized setting up Codewhale",
                "部署新的会话启动卡片并验证宽字符截断行为",
                "👩🏽\u{200d}🚀 crew 👨\u{200d}👩\u{200d}👧\u{200d}👦 families and flags 🇯🇵",
                "תהיה כרטיס פתיחת הפעלה",
            ],
            9,
        ));
        for (width, height) in [(170u16, 24u16), (120, 24), (80, 24), (40, 12), (20, 8)] {
            println!("\n{width}x{height}");
            println!("+{}+", "-".repeat(usize::from(width)));
            for line in painted(&app, width, height) {
                let pad = usize::from(width).saturating_sub(text_display_width(&line));
                println!("|{line}{}|", " ".repeat(pad));
            }
            println!("+{}+", "-".repeat(usize::from(width)));
        }
    }

    #[test]
    fn every_hitbox_points_at_the_row_that_painted() {
        let app = app_with_recent(&["one", "two", "three"], 9);
        for height in 1u16..=24 {
            let state = launch_empty_state(&app, Rect::new(0, 0, 120, height));
            for (id, row) in &state.rows {
                let text = flatten(&state.lines[*row]);
                assert!(
                    !text.trim().is_empty(),
                    "height {height}: hitbox for {id:?} points at a blank row",
                );
            }
        }
    }
}

#[cfg(test)]
mod empty_state_caption_tests {
    use super::{empty_state_caption, shorten_workspace};
    use unicode_width::UnicodeWidthStr;

    const DEEP: &str = "/private/tmp/claude-501/-Volumes-VIXinSSD-CW-codewhale/34267917-11f4-4d15-911a-2a8acd5c49e1/scratchpad/surface/ws2";

    #[test]
    fn caption_stays_narrow_enough_to_actually_centre() {
        // The caller centres this line with `(width - caption.width()) / 2`.
        // Building it at full length and truncating to `width` made that inset
        // zero, so the caption rendered flush-left and full-bleed straight
        // through the centred whale/wordmark/prompt composition.
        for width in [60usize, 80, 100, 120] {
            let caption = empty_state_caption(DEEP, "no git", "MCP", 0, width);
            assert!(
                caption.width() <= width,
                "width {width}: caption {caption:?} overflows the lane",
            );
            assert!(
                width.saturating_sub(caption.width()) / 2 > 0,
                "width {width}: caption {caption:?} would render flush-left",
            );
        }
    }

    #[test]
    fn caption_keeps_the_folder_you_are_standing_in() {
        let long = "/a/very/deeply/nested/checkout/somewhere/far/away/myproject";
        for width in [40usize, 60, 80, 120] {
            let caption = empty_state_caption(long, "main", "MCP", 2, width);
            assert!(
                caption.contains("myproject"),
                "width {width}: {caption:?} dropped the current folder",
            );
        }
    }

    #[test]
    fn caption_sheds_the_least_important_detail_first() {
        let ws = "~/code/app";
        let wide = empty_state_caption(ws, "main", "MCP", 3, 120);
        assert!(wide.contains("MCP 3") && wide.contains("main") && wide.contains(ws));

        let mid = empty_state_caption(ws, "main", "MCP", 3, 24);
        assert!(
            !mid.contains("MCP"),
            "{mid:?} should shed the MCP count first"
        );
        assert!(mid.contains("main"), "{mid:?} should still name the branch");

        let tight = empty_state_caption(ws, "main", "MCP", 3, 16);
        assert!(
            tight.contains("app"),
            "{tight:?} should still name the folder"
        );
    }

    #[test]
    fn elision_lands_on_a_separator_not_mid_component() {
        // The old line ended in an ellipsis mid-directory
        // ("…/34267917-11f4-4d15-911a-"), which told the reader nothing.
        let caption = empty_state_caption(DEEP, "no git", "MCP", 0, 60);
        assert!(
            !caption.contains("2a8acd5c49e1"),
            "{caption:?} clipped mid-component"
        );
        if caption.starts_with('…') {
            assert!(
                caption.starts_with("…/"),
                "elision must land on a separator: {caption:?}",
            );
        }
    }

    #[test]
    fn caption_margin_scales_so_it_is_always_visibly_a_caption() {
        // The flat four-column margin only looked like a margin at 60 columns.
        // At 119 it let a 114-column path through with an inset of two — a
        // full-bleed banner cutting the centred composition in half, which is
        // the exact failure the shedding ladder exists to prevent.
        for width in [40usize, 60, 80, 100, 119, 120, 200] {
            for workspace in [DEEP, "/a/b/c/d/e/f/g/h/i/j/k/l/m/n/o/p/q/r/s/project"] {
                let caption = empty_state_caption(workspace, "main", "MCP", 2, width);
                let inset = width.saturating_sub(caption.width()) / 2;
                assert!(
                    inset * 12 >= width,
                    "width {width}: caption {caption:?} insets by only {inset}",
                );
            }
        }
    }

    #[test]
    fn shorten_workspace_is_a_no_op_when_it_already_fits() {
        assert_eq!(shorten_workspace("~/code/app", 2), "~/code/app".to_string());
        assert_eq!(shorten_workspace("app", 2), "app".to_string());
    }
}

#[cfg(test)]
mod header_tests {
    use super::{
        FIELD_JOIN, GROUP_GAP, filesystem_scope_notice, header_hitboxes,
        render_header_with_git_status,
    };
    use crate::tui::app::App;
    use crate::tui::widgets::workflow_panel::{WorkflowPanel, WorkflowPanelLifecycle};
    use codewhale_config::AppMode;
    use codewhale_execpolicy::ApprovalMode;
    use codewhale_palette::ChromeInk;
    use ratatui::{buffer::Buffer, layout::Rect};

    fn app() -> App {
        let mut app = crate::test_support::test_app_with_options(
            crate::test_support::test_tui_options(std::env::temp_dir()),
        );
        // Enforcement present, so the scope chip reflects the policy rather
        // than the host's missing backend.
        app.sandbox_backend = Some(crate::sandbox::SandboxType::None);
        app.mode = AppMode::Agent;
        app.approval_mode = ApprovalMode::Suggest;
        app
    }

    fn header_line(app: &App, width: u16) -> String {
        let area = Rect::new(0, 0, width, 1);
        let mut buf = Buffer::empty(area);
        render_header_with_git_status(
            area,
            &mut buf,
            app,
            &crate::tui::git_status::GitStatusSnapshot::default(),
        );
        (0..width)
            .map(|x| buf[(x, 0)].symbol())
            .collect::<String>()
            .trim_end()
            .to_string()
    }

    #[test]
    fn default_posture_spends_no_columns_on_the_expected_scope() {
        // `files: workspace` used to be printed on every frame of every
        // session: seventeen columns of the primary chrome restating the
        // default. A notice that never turns off cannot warn.
        let app = app();
        assert!(filesystem_scope_notice(&app).is_none());
        let line = header_line(&app, 120);
        assert!(!line.contains("files:"), "{line:?}");
        assert!(line.starts_with("codewhale"), "{line:?}");
        assert!(line.contains("work"), "{line:?}");
        assert!(line.contains("ask"), "{line:?}");
    }

    #[test]
    fn full_access_is_the_disclosure_and_is_not_restated() {
        // Full disk access is stated once, by the permission chip's own
        // name. A second `files: full disk` chip beside it said the same
        // thing twice; the mode name stays prominent and does the work.
        let mut app = app();
        app.approval_mode = ApprovalMode::Bypass;
        app.configured_sandbox_mode = Some("danger-full-access".to_string());
        assert!(filesystem_scope_notice(&app).is_none());
        let line = header_line(&app, 120);
        assert!(!line.contains("files:"), "{line:?}");
        assert!(
            line.contains(&*super::tr(
                app.ui_locale,
                super::MessageId::ChipPermissionFullAccess
            )),
            "{line:?}"
        );
    }

    #[test]
    fn full_access_never_stands_alone_without_its_scope() {
        // Bypass clamped to workspace-write: the permission chip says
        // "Full Access" while writes are in fact confined. That pairing is the
        // exact misreading the scope chip exists to prevent, so the chip must
        // speak even though workspace-write is otherwise the quiet default.
        let mut full = app();
        full.approval_mode = ApprovalMode::Bypass;
        full.configured_sandbox_mode = Some("workspace-write".to_string());
        let notice = filesystem_scope_notice(&full)
            .expect("Full Access must never appear without a scope beside it");
        assert_eq!(notice, "files: workspace");
        let line = header_line(&full, 120);
        assert!(line.contains("files: workspace"), "{line:?}");

        // And the default posture still stays quiet.
        let mut quiet = app();
        quiet.approval_mode = ApprovalMode::Suggest;
        quiet.configured_sandbox_mode = Some("workspace-write".to_string());
        assert!(filesystem_scope_notice(&quiet).is_none());
    }

    #[test]
    fn plan_mode_does_not_say_read_only_twice() {
        let mut app = app();
        app.mode = AppMode::Plan;
        assert!(filesystem_scope_notice(&app).is_none());
        let line = header_line(&app, 120);
        assert!(line.contains("read only"), "{line:?}");
        assert!(!line.contains("files: read-only"), "{line:?}");
    }

    #[test]
    fn the_build_version_is_not_permanent_chrome() {
        // It was already `Wide`-only, which is the layout admitting it was
        // never load-bearing; `codewhale --version`, `codewhale doctor` and
        // the launch screen are where a version is actually looked up, and
        // the half worth reading mid-session is the update chip.
        let app = app();
        for width in [60u16, 80, 120, 200] {
            let line = header_line(&app, width);
            assert!(
                !line.contains(concat!("v", env!("CODEWHALE_BUILD_VERSION"))),
                "width {width}: {line:?}",
            );
        }
    }

    #[test]
    fn chips_are_separated_from_posture_by_a_wider_gap_than_the_posture_join() {
        // One weight per meaning: `" · "` binds words into one phrase, the
        // group gap stands between whole facts. If a goal chip hangs off the
        // same dotted separator that joins mode to permission, the header is
        // an undifferentiated list again.
        let mut app = app();
        app.update_available = Some("update 0.9.11".to_string());
        let line = header_line(&app, 120);
        assert!(
            line.contains(&format!("ask{GROUP_GAP}update 0.9.11")),
            "{line:?}",
        );
        assert!(line.contains(&format!("work{FIELD_JOIN}ask")), "{line:?}");
        assert!(
            unicode_width::UnicodeWidthStr::width(GROUP_GAP)
                > unicode_width::UnicodeWidthStr::width(FIELD_JOIN),
            "the group gap must out-space the phrase join or nothing groups",
        );
    }

    #[test]
    fn collapsed_degraded_workflow_chip_uses_attention_ink() {
        let mut app = app();
        let mut panel = WorkflowPanel::new("workflow-partial", "review release", 1_000);
        panel.lifecycle = WorkflowPanelLifecycle::Degraded;
        panel.expanded = false;
        panel.completed_at_ms = Some(2_000);
        app.workflow_panel = Some(panel);

        let width = 200;
        let area = Rect::new(0, 0, width, 1);
        let mut buf = Buffer::empty(area);
        render_header_with_git_status(
            area,
            &mut buf,
            &app,
            &crate::tui::git_status::GitStatusSnapshot::default(),
        );
        let text = (0..width).map(|x| buf[(x, 0)].symbol()).collect::<String>();
        let start = text.find("wf degraded").expect("degraded workflow chip");
        let expected = ChromeInk::Attention.color(&app.ui_theme);
        for x in start..start + "wf degraded".len() {
            assert_eq!(
                buf[(x as u16, 0)].fg,
                expected,
                "collapsed degraded chip must stay amber at column {x}: {text:?}"
            );
        }
    }

    #[test]
    fn the_context_meter_states_its_percentage_and_registers_an_inspector_target() {
        // The percentage is the direct operator question ("how full am I?").
        // Fraction remains the auditable fact and the bar is the glance.
        let mut app = app();
        app.session.total_input_tokens = 3_000;
        let line = header_line(&app, 120);
        if line.contains('▱') || line.contains('▰') {
            assert!(!line.contains('['), "{line:?}");
            assert!(line.contains("context"), "{line:?}");
            assert!(line.contains('%'), "{line:?}");
            let hitboxes = header_hitboxes(Rect::new(0, 0, 120, 1), &app);
            assert_eq!(hitboxes.len(), 1);
            assert_eq!(hitboxes[0].area.right(), 120);
            assert_eq!(
                hitboxes[0].target,
                crate::tui::app::HeaderActionTarget::InspectContext
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Tideline startup stage — the launch header (shell design §2.0 item 2,
// founder direction 2026-09-02: "Claude Code's structure, not a centred hero
// with quick actions"). Top-left of the stage:
//
//   <mark>  Codewhale v0.9.12
//   <mark>  openrouter · deepseek-v4        (or `not connected`, gate ink)
//   <mark>  owner/repo · branch             (or the workspace path)
//
//   ⚠ no model connected · run /provider    (only while it is true)
//   ● 2 MCP servers connected · 1 needs sign-in · run /mcp   (only if true)
//
// then room, then the docked pre-session composer. Nothing else: no heading,
// no quick actions, no option strip, no wave rules. The stage is a pure,
// deterministic widget fed injected facts (`tideline_startup_from_app`
// projects `App`), proven against golden buffers `startup_{w}x{h}`.
// ---------------------------------------------------------------------------

/// How long the hero mark takes to surface, then it holds still forever.
const MARK_SURFACE_MS: u128 = 640;

/// Whether the launch screen wants animation frames right now: the mark is
/// still surfacing, the card is dissolving, or the underwater field is
/// alive and not yet settled. Nothing here paints; the event loop reads it
/// to schedule redraws, and each redraw advances the ambient clock.
#[must_use]
pub fn launch_motion_active(app: &App, obscured: bool, ambient_settled: bool) -> bool {
    if !app.launch.visible
        || obscured
        || app.onboarding != OnboardingState::None
        || !app.view_stack.is_empty()
        || !app.motion_policy().allows_decorative()
    {
        return false;
    }
    let now = app.ambient_clock_ms;
    let surfacing = crate::tui::mark::surface_progress(now, MARK_SURFACE_MS) < 1.0;
    let dissolve = app.launch.card_dissolve_progress(now, true);
    let dissolving = dissolve > 0.0 && dissolve < 1.0;
    let water_alive = app.theme_id == codewhale_palette::ThemeId::Underwater && !ambient_settled;
    surfacing || dissolving || water_alive
}
