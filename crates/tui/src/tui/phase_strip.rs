//! The posture bar and the route-identity shedding it shares with the
//! metrics line.
//!
//! Two one-row bands sit under the composer and never trade places with
//! it: the **posture bar** (this module's widget — permission, mode, live
//! counts, the one hint that applies now, with the remote-control state or
//! a live notice pinned right) and the **metrics line**
//! (`crate::tui::infoline` — model, context, cost, ttft, tok/s, output
//! tokens). Both rows are reserved in every frame, so a turn moving between
//! idle, thinking, tool use, approval, completion, failure, and cancellation
//! changes text inside fixed rows and never displaces the composer.
//!
//! One owner per fact: the context reading and the price are the metrics
//! line's; mode, permission and the working clock are this bar's. The module
//! name is the historical one — the phase word it painted also lives in the
//! transcript's active row, but the bar keeps its own copy beside the clock
//! (#5914): a bare duration cannot say whether the session is producing
//! tokens or parked waiting on a tool, a sub-agent, or you.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
};
use unicode_width::UnicodeWidthStr;

use crate::tui::{
    app::App,
    underwater::{LiveActivity, ShellPhase, ShellTier, phase_marker_with_activity},
};
use codewhale_localization::{MessageId, tr};
use codewhale_palette::ChromeInk;

/// Fixed one-row reservation for the identity band below the composer.
#[must_use]
pub fn height() -> u16 {
    1
}

/// Route identity for a rail or info line segment, shed field by field until it
/// fits `budget`.
///
/// The old version composed the full `provider · model · effort` label and
/// then `truncate_to_width`'d it to a fixed 24/44/64 columns, which happily
/// rendered `deepseek-v4-flash-prev…`. A clipped model name is worse than no
/// model name: routes share prefixes, so the ellipsis is the rail admitting
/// it will not tell you which model is answering. Shed the qualifiers
/// instead — provider first, then effort — and if the bare model name still
/// does not fit, shed the whole group. `/model` and `/status` own the full
/// route either way.
pub(crate) fn route_identity_fields(
    app: &App,
    tier: ShellTier,
    budget: usize,
) -> Option<Vec<RouteIdentityField>> {
    let (provider, model) = app.effective_route_identity_display();
    // A route that cannot prove its effective tier states no effort field
    // rather than `high→effective unavailable` (#5950): a placeholder that
    // can never resolve is noise, not a reading. First-party routes keep
    // their tier, `auto: tier` and `req→eff` labels.
    let effort = app.provable_reasoning_effort_label().unwrap_or_default();
    if model.is_empty() {
        return None;
    }
    let field = |kind, text: String| RouteIdentityField { kind, text };
    let mut candidates: Vec<Vec<RouteIdentityField>> = Vec::new();
    if tier != ShellTier::Compact && !provider.is_empty() {
        // The smallest shell never repeats the provider: model and effort are
        // the two facts that change what comes back.
        let mut fields = vec![
            field(RouteFieldKind::Provider, provider),
            field(RouteFieldKind::Model, model.clone()),
        ];
        if !effort.is_empty() {
            fields.push(field(RouteFieldKind::Effort, effort.clone()));
        }
        candidates.push(fields);
    }
    if !effort.is_empty() {
        candidates.push(vec![
            field(RouteFieldKind::Model, model.clone()),
            field(RouteFieldKind::Effort, effort),
        ]);
    }
    candidates.push(vec![field(RouteFieldKind::Model, model)]);
    candidates.into_iter().find(|fields| {
        let width = fields.iter().map(|field| field.text.width()).sum::<usize>()
            + fields.len().saturating_sub(1) * ITEM_SEPARATOR_WIDTH;
        width <= budget
    })
}

/// Which route fact a rendered field is. The info line needs this to send a
/// click to the surface that owns the fact the user pointed at: the provider
/// name to `/provider`, the model and its effort tier to `/model`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RouteFieldKind {
    Provider,
    Model,
    Effort,
}

/// One rendered route field: what it says, and what it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RouteIdentityField {
    pub kind: RouteFieldKind,
    pub text: String,
}

/// The info line's route budget: the row's width less the brand lockup,
/// meter, and clock floor, never below 24.
///
/// One owner. The rule used to be written out at the call site in
/// `ui/frame.rs` and copied again into two tests with a comment pointing
/// back at the original, which is how a shed rule drifts.
pub(crate) fn info_route_budget(width: u16) -> usize {
    usize::from(width).saturating_sub(60).max(24)
}

/// Split a notice at its joints, coarsest first.
///
/// A rail notice is prose, and prose has joints. Cutting at a joint keeps
/// every word that survives true; cutting mid-phrase and hanging an ellipsis
/// off the end only advertises that the row lost the argument. Sentence stops
/// are the joint we want; the inner marks are the fallback for a one-sentence
/// notice that is still too long for a narrow rail — losing the second half
/// of `Auto-denied exec_shell: denied earlier` beats losing the warning.
fn notice_clauses<'a>(text: &'a str, marks: &[char]) -> Vec<&'a str> {
    let mut clauses = Vec::new();
    let mut start = 0usize;
    let mut chars = text.char_indices().peekable();
    while let Some((idx, ch)) = chars.next() {
        if !marks.contains(&ch) {
            continue;
        }
        // Full-width marks carry no trailing space, so they break on sight.
        // ASCII marks only break before whitespace, which keeps `0.9.11`,
        // `docs/TELEMETRY.md`, and `https://…` in one piece.
        let breaks = !ch.is_ascii() || chars.peek().is_none_or(|(_, next)| next.is_whitespace());
        if !breaks {
            continue;
        }
        // `1.` opening a numbered step is a list ordinal, not a sentence
        // stop — breaking there leaves the toast ending on a bare `1.`
        // (the send-blocked clip: `…not found. 1.`). Only the pure
        // number-and-stop shape is exempt; `Version 1.` still ends a clause.
        if ch == '.'
            && text[start..idx].trim().bytes().all(|b| b.is_ascii_digit())
            && !text[start..idx].trim().is_empty()
        {
            continue;
        }
        let end = idx + ch.len_utf8();
        let clause = text[start..end].trim();
        if !clause.is_empty() {
            clauses.push(clause);
        }
        start = end;
    }
    let rest = text[start..].trim();
    if !rest.is_empty() {
        clauses.push(rest);
    }
    clauses
}

/// Sentence stops — the joint a notice prefers to be cut at.
const SENTENCE_MARKS: [char; 7] = ['.', '!', '?', '…', '。', '！', '？'];
/// Inner joints, used only when one sentence still will not fit the rail.
const CLAUSE_MARKS: [char; 8] = [';', ':', ',', '—', '；', '：', '，', '、'];

fn join_while_fitting(clauses: &[&str], budget: usize) -> Option<String> {
    let mut fitted = String::new();
    for clause in clauses {
        // A full-width stop already carries its own breathing room; putting
        // a Latin space after `。` is a typographic accent in the wrong
        // language.
        let space = usize::from(!fitted.is_empty() && fitted.ends_with(|ch: char| ch.is_ascii()));
        let candidate = fitted.width() + space + clause.width();
        if candidate > budget {
            break;
        }
        if space == 1 {
            fitted.push(' ');
        }
        fitted.push_str(clause);
    }
    // A phrase that ends on `:` or `;` is still telling you more is coming —
    // the same lie an ellipsis tells. Cut the mark and let the phrase stand.
    let fitted = fitted
        .trim_end_matches(|ch| CLAUSE_MARKS.contains(&ch) || ch == ' ')
        .to_string();
    (!fitted.is_empty()).then_some(fitted)
}

/// Fit a notice into `budget` by dropping whole trailing clauses.
///
/// Returns `None` only when not even the first inner phrase fits, and the
/// rail then says nothing rather than dangling a stump. Notices get first
/// call on the row: identity and the ledger chips have already stood down by
/// the time this is asked, and the key hints stand down after it if that is
/// what the notice needs.
fn fit_notice(text: &str, budget: usize) -> Option<String> {
    let text = text.trim();
    if text.is_empty() || budget == 0 {
        return None;
    }
    if text.width() <= budget {
        return Some(text.to_string());
    }
    let sentences = notice_clauses(text, &SENTENCE_MARKS);
    if let Some(fitted) = join_while_fitting(&sentences, budget) {
        return Some(fitted);
    }
    let first = sentences.first().copied().unwrap_or(text);
    join_while_fitting(&notice_clauses(first, &CLAUSE_MARKS), budget)
}

/// Map the boot surface's typed severity through the same semantic palette as
/// every other footer fact. Keeping this conversion closed makes the plugin
/// warning/failure distinction testable without guessing from its text.
fn boot_activity_ink(level: crate::tui::session_boot::SessionBootActivityLevel) -> ChromeInk {
    match level {
        crate::tui::session_boot::SessionBootActivityLevel::Active => ChromeInk::Active,
        crate::tui::session_boot::SessionBootActivityLevel::Attention => ChromeInk::Attention,
        crate::tui::session_boot::SessionBootActivityLevel::Failure => ChromeInk::Failure,
    }
}

/// Pick the notice a band owes its row to right now, if any. Shared by the
/// classic activity band and the Tideline merged footer so the two can never
/// disagree about which toast is live. Completion may land in the same event
/// drain as an approval denial: unresolved Warning/Error receipts stay
/// visible after `done`, only routine informational copy yields.
fn selected_notice(
    status_toast: Option<crate::tui::app::StatusToast>,
    phase_label: &str,
) -> Option<(String, ChromeInk, bool)> {
    status_toast
        .filter(|toast| !toast.text.trim().is_empty() && toast.text.trim() != phase_label)
        .map(|toast| {
            let urgent = matches!(
                toast.level,
                crate::tui::app::StatusToastLevel::Warning
                    | crate::tui::app::StatusToastLevel::Error
            );
            (toast.text.clone(), toast.level.ink(), urgent)
        })
}

/// The identity band's right-aligned key legend, and — since the Tideline
/// footer merge — the merged footer's `keys_legend` source. Live phases keep
/// the row quiet; idle and drafting advertise the chords the shell owns.
/// `← for agents · ↓ to manage` joins the chorus whenever the empty composer
/// still owns those keys.
/// Peers inside one group — provider and model, a count and its verb — keep
/// the middle dot.
const ITEM_SEPARATOR: &str = " · ";
const ITEM_SEPARATOR_WIDTH: usize = 3;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::Config, tui::app::TuiOptions};
    use std::path::PathBuf;

    fn test_app() -> App {
        App::new(
            TuiOptions {
                model: "deepseek-v4-flash".to_string(),
                ..crate::test_support::test_tui_options(PathBuf::from("."))
            },
            &Config::default(),
        )
    }

    #[test]
    fn working_marker_uses_the_live_work_status_role() {
        let mut app = test_app();
        // Match Terminal intentionally aliases both roles to ANSI Cyan. Use
        // the branded palette here to prove the renderer selects the working
        // slot rather than merely observing an equal terminal color.
        app.ui_theme = codewhale_palette::UI_THEME;
        assert_eq!(ShellPhase::Working.color(&app), app.ui_theme.status_working);
        assert_ne!(ShellPhase::Working.color(&app), app.ui_theme.info);
        assert_eq!(
            crate::tui::underwater::phase_ink(ShellPhase::Working),
            ChromeInk::Active
        );
        assert_eq!(
            crate::tui::underwater::phase_ink(ShellPhase::Failed),
            ChromeInk::Failure
        );
        assert_ne!(
            crate::tui::underwater::phase_ink(ShellPhase::Working).family(),
            codewhale_palette::SemanticFamily::Failure
        );
    }

    #[test]
    fn done_footer_preserves_unresolved_notice_behind_later_routine_info() {
        use crate::tui::app::StatusToastLevel;
        for (level, ink) in [
            (StatusToastLevel::Warning, ChromeInk::Attention),
            (StatusToastLevel::Error, ChromeInk::Failure),
        ] {
            let mut app = test_app();
            app.runtime_turn_status = Some("completed".into());
            app.push_status_toast("Unresolved issue", level, Some(12_000));
            app.push_status_toast("Routine update", StatusToastLevel::Info, Some(5_000));
            assert_eq!(ShellPhase::from_app(&app), ShellPhase::Done);
            assert!(
                app.history.is_empty(),
                "the transcript must not satisfy this fixture"
            );
            let facts = tideline_footer_from_app(&mut app, 140);
            assert_eq!(facts.right, Some(("Unresolved issue".into(), ink)));
            let mut buf = Buffer::empty(Rect::new(0, 0, 140, 1));
            render_tideline_footer(
                Rect::new(0, 0, 140, 1),
                &mut buf,
                &facts.widget(&app.ui_theme, false),
            );
            let text: String = buf.content.iter().map(|cell| cell.symbol()).collect();
            assert!(text.contains("Unresolved issue"), "{text}");
            assert!(!text.contains("Routine update"));
        }
    }

    #[test]
    fn boot_activity_levels_keep_plugin_attention_and_failure_distinct() {
        use crate::tui::session_boot::SessionBootActivityLevel;

        assert_eq!(
            boot_activity_ink(SessionBootActivityLevel::Active),
            ChromeInk::Active
        );
        assert_eq!(
            boot_activity_ink(SessionBootActivityLevel::Attention),
            ChromeInk::Attention
        );
        assert_eq!(
            boot_activity_ink(SessionBootActivityLevel::Failure),
            ChromeInk::Failure
        );
    }

    #[test]
    fn notice_clauses_split_on_sentences_and_keep_versions_and_paths_whole() {
        assert_eq!(
            notice_clauses(
                "Counts are on. Code is never collected. See docs/T.md",
                &SENTENCE_MARKS
            ),
            vec![
                "Counts are on.",
                "Code is never collected.",
                "See docs/T.md"
            ]
        );
        assert_eq!(
            notice_clauses("Updated to 0.9.11 from 0.9.10", &SENTENCE_MARKS),
            vec!["Updated to 0.9.11 from 0.9.10"]
        );
        // Full-width stops carry no trailing space, so they break on sight.
        assert_eq!(
            notice_clauses(
                "匿名の利用回数は有効です。会話とコードは収集されません。",
                &SENTENCE_MARKS
            ),
            vec![
                "匿名の利用回数は有効です。",
                "会話とコードは収集されません。"
            ]
        );
        // A colon inside a URL is not a joint.
        assert_eq!(
            notice_clauses("Docs: https://example.test/x", &CLAUSE_MARKS),
            vec!["Docs:", "https://example.test/x"]
        );
    }

    #[test]
    fn shed_clauses_rejoin_without_a_latin_space_after_a_full_width_stop() {
        const JA: &str = "匿名の利用状況集計はオンです。会話やコードは一切収集しません。/settings で変更できます。スキーマ: docs/TELEMETRY.md";
        let clauses = notice_clauses(JA, &SENTENCE_MARKS);
        let joined = join_while_fitting(&clauses, 200).expect("fits");
        assert!(!joined.contains("。 "), "{joined:?}");
        assert!(joined.ends_with("docs/TELEMETRY.md"), "{joined:?}");
    }

    #[test]
    fn a_notice_sheds_whole_clauses_and_never_dangles() {
        const NOTICE: &str = "Anonymous usage counts are on. Conversations and code are never collected. Change this in /settings; schema: docs/TELEMETRY.md";
        assert_eq!(fit_notice(NOTICE, 200).as_deref(), Some(NOTICE));
        assert_eq!(
            fit_notice(NOTICE, 80).as_deref(),
            Some("Anonymous usage counts are on. Conversations and code are never collected.")
        );
        assert_eq!(
            fit_notice(NOTICE, 40).as_deref(),
            Some("Anonymous usage counts are on.")
        );
        assert_eq!(fit_notice("   ", 40), None);
    }

    /// The failure this caught: a one-sentence warning longer than the row
    /// used to have no sentence joint to shed at, so the rail dropped the
    /// whole warning. Inner joints are the fallback, and the phrase that
    /// survives never ends on a `:` or `;` — that mark says "more is coming"
    /// as loudly as an ellipsis does.
    #[test]
    fn a_clause_less_warning_sheds_at_inner_joints_rather_than_vanishing() {
        const WARNING: &str =
            "Auto-denied exec_shell: denied earlier; restart Codewhale to re-enable it.";
        assert_eq!(fit_notice(WARNING, 120).as_deref(), Some(WARNING));
        assert_eq!(
            fit_notice(WARNING, 60).as_deref(),
            Some("Auto-denied exec_shell: denied earlier")
        );
        assert_eq!(
            fit_notice(WARNING, 30).as_deref(),
            Some("Auto-denied exec_shell")
        );
    }

    #[test]
    fn session_metrics_strip_is_on_by_default() {
        assert!(
            crate::config::StatusItem::default_footer()
                .contains(&crate::config::StatusItem::SessionMetrics)
        );
        assert_eq!(
            crate::config::StatusItem::from_key("session_metrics"),
            Some(crate::config::StatusItem::SessionMetrics)
        );
    }

    /// The route identity (the info line Model segment's value) sheds whole
    /// fields — provider first, then the effort label — and stands down
    /// entirely rather than clip a model name. Ported from the identity
    /// band to `route_identity_fields`, the live shedding authority the
    /// info line calls with the same budget rule.
    #[test]
    fn route_identity_sheds_qualifiers_before_it_would_clip_a_model_name() {
        let model = "deepseek-v4-flash-preview-2026-05-01";
        let mut app = App::new(
            TuiOptions {
                model: model.to_string(),
                ..crate::test_support::test_tui_options(PathBuf::from("."))
            },
            &Config::default(),
        );
        app.ui_locale = codewhale_localization::Locale::En;

        // The info line's own budget rule (ui/frame.rs): width minus the brand
        // lockup, meter, and clock floor, never below 24.
        let fields = |width: u16| {
            route_identity_fields(
                &app,
                ShellTier::for_chrome_width(width),
                info_route_budget(width),
            )
        };

        let wide = fields(140).expect("wide budget keeps the route");
        assert!(
            wide.iter().any(|f| f.text.contains("DeepSeek"))
                && wide.iter().any(|f| f.text == model),
            "{wide:?}"
        );

        // Below the group's width the provider sheds first; the model stays
        // whole or the whole group stands down — never a clipped name.
        for width in [30u16, 34, 40, 46, 50, 60] {
            let shed = fields(width).unwrap_or_default();
            for field in &shed {
                assert!(
                    !field.text.contains('…'),
                    "{width} dangled a clipped field: {shed:?}"
                );
            }
            if shed.iter().any(|f| f.text.contains("deepseek-v4-flash-p")) {
                assert!(
                    shed.iter().any(|f| f.text == model),
                    "{width} clipped the model name: {shed:?}"
                );
            }
        }
    }

    /// A named custom route can carry a long provider identity next to a
    /// long model id; whole fields shed (provider first, effort label next)
    /// and neither name is ever clipped. Ported from the identity band to
    /// `route_identity_fields`.
    #[test]
    fn unproven_effort_keeps_named_provider_when_the_route_fits() {
        let mut app = test_app();
        app.set_provider_identity(crate::config::ApiProvider::Custom, "lab-gateway");
        app.model = "unlisted-model".to_string();
        assert!(app.provable_reasoning_effort_label().is_none());
        let fields = route_identity_fields(&app, ShellTier::for_chrome_width(160), 100).unwrap();
        assert_eq!(
            fields.iter().map(|field| field.kind).collect::<Vec<_>>(),
            vec![RouteFieldKind::Provider, RouteFieldKind::Model]
        );
        assert_eq!(fields[0].text, "lab-gateway");
        assert_eq!(fields[1].text, "unlisted-model");
        let compact = route_identity_fields(&app, ShellTier::Compact, 100).unwrap();
        assert_eq!(compact.len(), 1);
        assert_eq!(compact[0].kind, RouteFieldKind::Model);
        let narrow = route_identity_fields(&app, ShellTier::for_chrome_width(160), 14).unwrap();
        assert_eq!(narrow.len(), 1);
        assert_eq!(narrow[0].text, "unlisted-model");
    }

    #[test]
    fn long_custom_route_names_shed_whole_fields_across_width_tiers() {
        let model = "deepseek-v4-flash-vision-preview-2026-08-01";
        let mut app = test_app();
        app.ui_locale = codewhale_localization::Locale::En;
        app.set_provider_identity(
            crate::config::ApiProvider::Custom,
            "acme-research-gateway-eu-central",
        );
        app.model = model.to_string();

        for width in [30u16, 40, 50, 60, 70, 80, 160] {
            let shed = route_identity_fields(
                &app,
                ShellTier::for_chrome_width(width),
                info_route_budget(width),
            )
            .unwrap_or_default();
            for field in &shed {
                assert!(
                    !field.text.contains('…'),
                    "{width} dangled a clipped field: {shed:?}"
                );
            }
            if shed
                .iter()
                .any(|f| f.text.contains("deepseek-v4-flash-vision"))
            {
                assert!(
                    shed.iter().any(|f| f.text == model),
                    "{width} clipped the model name: {shed:?}"
                );
            }
            assert!(
                !shed
                    .iter()
                    .any(|f| f.text.contains("acme-research-gateway-eu-c")
                        && f.text != "acme-research-gateway-eu-central"),
                "{width} clipped the provider name: {shed:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Tideline merged footer (spec §3 slots 6+8 merged, §5a "Footer"): one
// band — phase·cost on the left, the notice/keys slot on the right.
// Wired into `ui/frame.rs` as the shell's single footer row: the classic
// activity band (slot 6) and identity band (slot 8) collapsed into it, with
// the old header's mode/permission chips carried in the left half per §3.
// ---------------------------------------------------------------------------
// The posture bar (SHELL-DESIGN-20260901 §2.0 item 3, §2.3b; founder
// direction 2026-09-02): the first row under the composer, in Claude Code's
// grammar —
//
//   ▶▶ full access (Shift+Tab) · work (Tab) · 2 agents, 1 task · Esc to interrupt      rc connected
//
// permission chip first (never sheds, #5796), the mode, the turn clock, the
// live counts, the session clock, then the one hint that applies right now;
// the remote-control state or a live notice pinned right. No cost: the
// roster owns per-agent elapsed and the metrics line owns the price. The
// context reading is the metrics line's; this row only says what to do about
// it at the cap.
//
// The clock came back in #5914. It was the `worked Nm Ss` chip on the
// classic footer until `146ab7f756` deleted that path, and the phase band's
// `working_detail` until `329960fcbf` (the 0.9.12 mega shell) merged the
// bands and left the elapsed reading to the transcript's active row. A
// multi-hour operate session scrolls that row out of sight, so the founder
// looking straight at the screen had no way to tell how long the session had
// been working or whether the current turn was stuck. The fixed row is where
// a glancing user looks; the clock lives here.
// ---------------------------------------------------------------------------

/// The context cap warning at ≥80% (spec §5a/§5e). The reading itself lives
/// in the metrics line; this bar still says what to do about it.
const DEPTH_WARN: &str = "surface soon — /compact";

/// The bar's leading glyph — Claude Code's double chevron, in the
/// permission chip's ink.
const POSTURE_MARK: &str = "▶▶";
/// Inside the counts group (`2 agents, 1 task`).
const COUNT_SEPARATOR: &str = ", ";

/// What the caller owes the posture bar. All injected, deterministic.
pub struct TidelineFooter<'a> {
    pub theme: &'a codewhale_palette::UiTheme,
    /// Permission chip (`ask` / `auto` / `full access`, plus the filesystem
    /// scope notice when it deviates) in its Permission ink. Never sheds.
    pub permission_chip: (&'a str, codewhale_palette::ChromeInk),
    /// The chord that cycles the permission posture, when the binding is
    /// live for the current focus (`Shift+Tab`).
    pub permission_key: Option<&'a str>,
    /// Mode chip (`work` / `plan` / `operate`) in its Policy ink.
    pub mode_chip: Option<(&'a str, codewhale_palette::ChromeInk)>,
    /// The chord that cycles the mode, when the binding is live (`Tab`).
    pub mode_key: Option<&'a str>,
    /// The turn half of the working clock (`working 1m 15s`): what the
    /// session is doing right now and for how long. `None` between turns.
    pub turn_clock: Option<(&'a str, codewhale_palette::ChromeInk)>,
    /// Live counts (`2 agents`, `1 task`) in their own inks, joined with
    /// `, `.
    pub counts: &'a [(String, ChromeInk)],
    /// The session half of the working clock (`worked 41m 12s`): how long
    /// this session has actually worked — the reading the founder went
    /// looking for and could not find (#5914). Outlives the turn half, and
    /// sheds before the hint and the counts. `None` until the session has
    /// worked a minute, and while it would repeat the turn reading (#6041).
    pub session_clock: Option<(&'a str, codewhale_palette::ChromeInk)>,
    /// The one hint that applies right now (`Esc to interrupt`).
    pub hint: Option<(&'a str, codewhale_palette::ChromeInk)>,
    /// Context window percentage 0–100. The metrics line paints the reading;
    /// this bar only uses it to decide whether the ≥80% cap warning outranks
    /// `hint`.
    pub context_percent: u8,
    /// Pinned right: a live notice (status toast / boot activity chip) or
    /// the remote-control state.
    pub right: Option<(&'a str, codewhale_palette::ChromeInk)>,
    pub ascii_safe: bool,
    /// `tui.posture_bar = "compact"` (#5950): start the shed ladder at
    /// [`COMPACT_SHED`] instead of rung 0, so the row states its posture —
    /// the permission and mode chips, and the cap warning when it is owed —
    /// and nothing live. Width sheds the rest exactly as it always did.
    pub compact: bool,
}

impl<'a> TidelineFooter<'a> {
    #[must_use]
    pub fn new(
        theme: &'a codewhale_palette::UiTheme,
        permission_chip: (&'a str, codewhale_palette::ChromeInk),
    ) -> Self {
        Self {
            theme,
            permission_chip,
            permission_key: None,
            mode_chip: None,
            mode_key: None,
            turn_clock: None,
            counts: &[],
            session_clock: None,
            hint: None,
            context_percent: 0,
            right: None,
            ascii_safe: false,
            compact: false,
        }
    }

    #[must_use]
    pub fn permission_key(mut self, key: Option<&'a str>) -> Self {
        self.permission_key = key;
        self
    }

    #[must_use]
    pub fn mode_chip(mut self, chip: Option<(&'a str, codewhale_palette::ChromeInk)>) -> Self {
        self.mode_chip = chip;
        self
    }

    #[must_use]
    pub fn mode_key(mut self, key: Option<&'a str>) -> Self {
        self.mode_key = key;
        self
    }

    #[must_use]
    pub fn turn_clock(mut self, clock: Option<(&'a str, codewhale_palette::ChromeInk)>) -> Self {
        self.turn_clock = clock;
        self
    }

    #[must_use]
    pub fn session_clock(mut self, clock: Option<(&'a str, codewhale_palette::ChromeInk)>) -> Self {
        self.session_clock = clock;
        self
    }

    #[must_use]
    pub fn counts(mut self, counts: &'a [(String, ChromeInk)]) -> Self {
        self.counts = counts;
        self
    }

    #[must_use]
    pub fn hint(mut self, hint: Option<(&'a str, codewhale_palette::ChromeInk)>) -> Self {
        self.hint = hint;
        self
    }

    #[must_use]
    pub fn context_percent(mut self, percent: u8) -> Self {
        self.context_percent = percent;
        self
    }

    #[must_use]
    pub fn right(mut self, right: Option<(&'a str, codewhale_palette::ChromeInk)>) -> Self {
        self.right = right;
        self
    }

    #[must_use]
    pub fn ascii_safe(mut self, ascii_safe: bool) -> Self {
        self.ascii_safe = ascii_safe;
        self
    }

    #[must_use]
    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }

    /// The rung the shed ladder starts from: 0 for a full row, past the
    /// clocks, hint and counts for a compact one.
    fn first_shed_rung(&self) -> u8 {
        if self.compact { COMPACT_SHED } else { 0 }
    }

    fn sym(&self, glyph: &str) -> String {
        if !self.ascii_safe {
            return glyph.to_string();
        }
        if let Some(fb) = crate::tui::glyphs::ascii_fallback(glyph) {
            return fb.to_string();
        }
        glyph
            .chars()
            .map(|c| {
                crate::tui::glyphs::ascii_fallback(&c.to_string())
                    .map(str::to_string)
                    .unwrap_or_else(|| c.to_string())
            })
            .collect()
    }

    /// Whether the context window is full enough that the bar owes the cap
    /// warning. It replaces the hint and, unlike a hint, outranks the clock
    /// and the counts on the shed ladder.
    fn at_context_cap(&self) -> bool {
        self.context_percent.clamp(0, 100) >= 80
    }

    /// The hint the left run ends on: the cap warning outranks whatever the
    /// caller passed, because a full context is the one thing that stops the
    /// next turn.
    fn effective_hint(&self) -> Option<(String, ChromeInk)> {
        if self.at_context_cap() {
            return Some((
                format!("{} {}", self.sym("▲"), self.sym(DEPTH_WARN)),
                ChromeInk::Attention,
            ));
        }
        self.hint.map(|(text, ink)| (self.sym(text), ink))
    }
}

fn tchrome(theme: &codewhale_palette::UiTheme, ink: codewhale_palette::ChromeInk) -> Style {
    codewhale_palette::grammar::chrome_style(theme, ink)
}

fn tput(buf: &mut Buffer, x: u16, y: u16, text: &str, style: Style) {
    buf.set_stringn(x, y, text, text.width(), style);
}

/// One painted item of the left run: text, ink, and whether it is a chip
/// (bold) or a hint.
struct PostureItem {
    text: String,
    ink: ChromeInk,
    bold: bool,
    /// Painted after `, ` rather than ` · `: the counts are one group.
    joined: bool,
    /// Which of `footer.counts` this item is, when it is one.
    count_index: Option<usize>,
}

/// Shed rungs for the left run, most expendable first. The permission chip
/// itself has no rung: it never sheds (#5796), because a silently missing
/// `full access` is the bar under-reporting the authority the session
/// actually holds.
///
/// The two halves of the working clock shed apart, and both go first
/// (#5914). The clock is what a *glance* wants; at the narrowest widths the
/// row's other facts are what a *keystroke* wants — the counts name work you
/// can open, the hint names a chord you can press right now (`Esc to
/// interrupt`, `Enter again to send now`). An 80-column row carrying the
/// filesystem-scope notice cannot hold all of it, and losing the affordance
/// to keep the stopwatch is the wrong trade. When both halves would paint,
/// the turn half goes before the session half: the transcript's active row
/// and the spinner also show the turn is alive, while the session total is
/// stated nowhere else. When the session half is suppressed (#6041) or
/// otherwise absent, the turn half sheds at the session-clock rung instead
/// so the ladder does not abandon the only clock at the turn-only rung
/// while a both-clocks row would still be stating a stopwatch (#6084).
/// The hint and counts still outrank it (#5914). Above them the
/// context-cap warning, which is not a hint but the reason the next turn
/// will not start at all.
const SHED_TURN_CLOCK: u8 = 1;
const SHED_SESSION_CLOCK: u8 = 2;
const SHED_HINT: u8 = 3;
const SHED_COUNTS: u8 = 4;
const SHED_CAP_WARNING: u8 = 5;
const SHED_MODE_KEY: u8 = 6;
const SHED_MODE: u8 = 7;
const SHED_PERMISSION_KEY: u8 = 8;
/// The most-shed rung: everything gone but the permission chip.
const MAX_SHED: u8 = SHED_PERMISSION_KEY;
/// Where a compact posture bar (`tui.posture_bar = "compact"`, #5950)
/// starts on the ladder: the clocks, the hint and the counts are gone
/// before width is consulted; the cap warning, the mode chip and the
/// permission chip — the row's posture — stay and shed only by width.
const COMPACT_SHED: u8 = SHED_COUNTS;

fn posture_items(footer: &TidelineFooter<'_>, shed: u8) -> Vec<PostureItem> {
    let chip = |text: &str, key: Option<&str>| -> String {
        match key {
            Some(key) => format!("{text} ({key})"),
            None => text.to_string(),
        }
    };
    let mut items = vec![PostureItem {
        text: chip(
            &footer.sym(footer.permission_chip.0),
            footer.permission_key.filter(|_| shed < SHED_PERMISSION_KEY),
        ),
        ink: footer.permission_chip.1,
        bold: true,
        joined: false,
        count_index: None,
    }];
    if let Some((mode, ink)) = footer.mode_chip.filter(|_| shed < SHED_MODE) {
        items.push(PostureItem {
            text: chip(
                &footer.sym(mode),
                footer.mode_key.filter(|_| shed < SHED_MODE_KEY),
            ),
            ink,
            bold: true,
            joined: false,
            count_index: None,
        });
    }
    // When no session half will paint, shed the turn clock at the session
    // rung so a width that would keep the session half (and drop the turn)
    // still keeps the only informative clock (#6084). Hint and counts still
    // outrank it (#5914). With both halves present the turn half still goes
    // first.
    let turn_shed = if footer.session_clock.is_none() {
        SHED_SESSION_CLOCK
    } else {
        SHED_TURN_CLOCK
    };
    if let Some((clock, ink)) = footer.turn_clock.filter(|_| shed < turn_shed) {
        items.push(PostureItem {
            text: footer.sym(clock),
            ink,
            bold: false,
            joined: false,
            count_index: None,
        });
    }
    if shed < SHED_COUNTS {
        for (index, (count, ink)) in footer.counts.iter().enumerate() {
            items.push(PostureItem {
                text: footer.sym(count),
                ink: *ink,
                bold: false,
                joined: index > 0,
                count_index: Some(index),
            });
        }
    }
    if let Some((clock, ink)) = footer.session_clock.filter(|_| shed < SHED_SESSION_CLOCK) {
        items.push(PostureItem {
            text: footer.sym(clock),
            ink,
            bold: false,
            joined: false,
            count_index: None,
        });
    }
    // The cap warning is not a hint: it sheds after the counts and both
    // clock halves, because a full context is the one thing that stops the
    // next turn from starting at all.
    let hint_rung = if footer.at_context_cap() {
        SHED_CAP_WARNING
    } else {
        SHED_HINT
    };
    if shed < hint_rung
        && let Some((text, ink)) = footer.effective_hint()
    {
        items.push(PostureItem {
            text,
            ink,
            bold: false,
            joined: false,
            count_index: None,
        });
    }
    items
}

/// The separator painted before an item: `, ` inside the counts group,
/// ` · ` between groups.
fn separator_before(item: &PostureItem) -> &'static str {
    if item.joined {
        COUNT_SEPARATOR
    } else {
        ITEM_SEPARATOR
    }
}

fn left_run_width(mark: &str, items: &[PostureItem]) -> usize {
    mark.width()
        + 1
        + items.iter().map(|item| item.text.width()).sum::<usize>()
        + items
            .iter()
            .skip(1)
            .map(|item| separator_before(item).width())
            .sum::<usize>()
}

/// Paint the posture bar (spec §5b: `Constraint::Length(1)`).
///
/// Left: the mark, the permission chip, the mode, the counts, the hint —
/// shed from the right until the run fits beside the pinned right slot.
/// Right: the notice or remote-control state, clause-shed by the caller and
/// truncated here as the last resort; it never covers the permission chip.
/// Paint the posture bar. Returns the painted rect of each live count, by
/// its index into `footer.counts`, so the caller can make the counts the
/// bottom-of-screen affordance that opens the matching dock view.
pub fn render_tideline_footer(
    area: Rect,
    buf: &mut Buffer,
    footer: &TidelineFooter<'_>,
) -> Vec<(usize, Rect)> {
    let mut count_rects = Vec::new();
    if area.width < 8 || area.height < 1 {
        return count_rects;
    }
    let theme = footer.theme;
    let width = usize::from(area.width);
    let mark = footer.sym(POSTURE_MARK);

    // The permission chip alone is the floor; the right slot takes what is
    // left after it, and the rest of the left run sheds against the slot.
    let floor = left_run_width(&mark, &posture_items(footer, MAX_SHED));
    let right = footer.right.map(|(text, ink)| {
        let budget = width.saturating_sub(floor + 1);
        (
            crate::tui::ui_text::truncate_line_to_width(&footer.sym(text), budget),
            ink,
        )
    });
    let right_width = right
        .as_ref()
        .map(|(text, _)| text.width() + 1)
        .unwrap_or(0);
    let left_budget = width.saturating_sub(right_width);
    let items = (footer.first_shed_rung()..=MAX_SHED)
        .map(|shed| posture_items(footer, shed))
        .find(|items| left_run_width(&mark, items) <= left_budget)
        .unwrap_or_else(|| posture_items(footer, MAX_SHED));

    let permission_ink = footer.permission_chip.1;
    let mut x = usize::from(area.x);
    let clip = |x: usize, text: &str| -> String {
        crate::tui::ui_text::truncate_line_to_width(
            text,
            (usize::from(area.x) + left_budget).saturating_sub(x),
        )
    };
    tput(
        buf,
        x as u16,
        area.y,
        &clip(x, &mark),
        tchrome(theme, permission_ink).add_modifier(Modifier::BOLD),
    );
    x += mark.width() + 1;
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            // Projected like every other glyph on the row: an ascii-safe
            // terminal that cannot draw `·` must not get one here either,
            // least of all next to a clock reading whose own separator was
            // projected.
            let separator = footer.sym(separator_before(item));
            tput(
                buf,
                x as u16,
                area.y,
                &clip(x, &separator),
                tchrome(theme, ChromeInk::MetadataDim),
            );
            x += separator.width();
        }
        let mut style = tchrome(theme, item.ink);
        if item.bold {
            style = style.add_modifier(Modifier::BOLD);
        }
        let text = clip(x, &item.text);
        tput(buf, x as u16, area.y, &text, style);
        if let Some(count_index) = item.count_index
            && !text.is_empty()
        {
            count_rects.push((
                count_index,
                Rect::new(x as u16, area.y, text.width() as u16, 1),
            ));
        }
        x += item.text.width();
    }

    if let Some((text, ink)) = right
        && !text.is_empty()
    {
        let sx = (usize::from(area.x) + width).saturating_sub(text.width());
        tput(buf, sx as u16, area.y, &text, tchrome(theme, ink));
    }
    count_rects
}

/// Owned posture facts, built from real `App` state at render time and lent
/// to [`TidelineFooter`] for painting.
pub(crate) struct TidelineFooterFacts {
    pub permission_chip: (String, codewhale_palette::ChromeInk),
    pub permission_key: Option<&'static str>,
    pub mode_chip: Option<(String, codewhale_palette::ChromeInk)>,
    pub mode_key: Option<&'static str>,
    pub turn_clock: ClockReading,
    pub counts: Vec<(String, ChromeInk)>,
    pub session_clock: ClockReading,
    /// The action each entry of `counts` runs when clicked — same
    /// length, same order.
    pub count_actions: Vec<crate::tui::tideline::InteractionAction>,
    pub hint: Option<(String, codewhale_palette::ChromeInk)>,
    pub context_percent: u8,
    pub right: Option<(String, codewhale_palette::ChromeInk)>,
}

impl TidelineFooterFacts {
    /// Borrow the facts as the deterministic widget's input.
    pub(crate) fn widget<'a>(
        &'a self,
        theme: &'a codewhale_palette::UiTheme,
        ascii_safe: bool,
    ) -> TidelineFooter<'a> {
        let borrow = |chip: &'a Option<(String, ChromeInk)>| {
            chip.as_ref().map(|(text, ink)| (text.as_str(), *ink))
        };
        TidelineFooter::new(
            theme,
            (self.permission_chip.0.as_str(), self.permission_chip.1),
        )
        .permission_key(self.permission_key)
        .mode_chip(borrow(&self.mode_chip))
        .mode_key(self.mode_key)
        .turn_clock(borrow(&self.turn_clock))
        .counts(&self.counts)
        .session_clock(borrow(&self.session_clock))
        .hint(borrow(&self.hint))
        .context_percent(self.context_percent)
        .right(borrow(&self.right))
        .ascii_safe(ascii_safe)
    }
}

/// The session has to have worked this long before the bar states a total.
/// A fresh launch that says `worked 4s` is furniture, not information; the
/// classic footer's `worked` chip used the same floor (#448).
const CLOCK_SESSION_FLOOR_SECS: u64 = 60;

/// One half of the working clock: its text and the ink that says whether the
/// clock is running. `None` when that half has nothing true to say.
pub(crate) type ClockReading = Option<(String, ChromeInk)>;

/// The working clock (#5914), as its two halves — `(turn, session)`.
///
/// * the **turn** reading is `{phase_label} {elapsed}` — the phase word is
///   the transcript's own, so `working 1m 15s` and `waiting on you 1m 15s`
///   and `sub-agents underway 1m 15s` all say what the clock is counting.
///   A duration alone cannot distinguish a session producing tokens from one
///   parked on a tool, a sub-agent, or an unanswered prompt. `None` between
///   turns: there is no turn to time.
/// * the **session** reading is the classic `worked {elapsed}` chip (#448):
///   `App::cumulative_turn_duration` (the sum of finished turns) plus the
///   live turn, so it ticks continuously and never jumps at `TurnComplete`.
///   It is model work, not wall clock since launch — an idle TUI does not
///   claim to have been working. Quiet ink while no turn is running, because
///   the clock is stopped. Suppressed while it would repeat the turn
///   reading — on a session's first turn the two are the same duration
///   (#6041); it returns as soon as a finished turn makes the totals
///   different.
pub(crate) fn working_clock(
    app: &App,
    phase: ShellPhase,
    phase_label: &str,
) -> (ClockReading, ClockReading) {
    let turn = app.turn_started_at.map(|started| started.elapsed());
    let ink = match phase {
        ShellPhase::Waiting | ShellPhase::Approval => ChromeInk::Waiting,
        ShellPhase::Failed => ChromeInk::Attention,
        _ => ChromeInk::Active,
    };
    let turn_clock = turn.map(|turn| {
        (
            format!(
                "{phase_label} {}",
                crate::elapsed::format_elapsed_secs(turn.as_secs())
            ),
            ink,
        )
    });
    let worked = app
        .cumulative_turn_duration
        .saturating_add(turn.unwrap_or_default());
    // #6041: on a session's first turn there is no finished-turn total, so
    // the session reading would print the same duration the turn reading
    // already carries. The turn half names what is happening; the worked
    // chip earns its place only once a finished turn makes it a different
    // number.
    let repeats_turn = turn.is_some_and(|turn| turn.as_secs() == worked.as_secs());
    let session_clock =
        (worked.as_secs() >= CLOCK_SESSION_FLOOR_SECS && !repeats_turn).then(|| {
            (
                tr(app.ui_locale, MessageId::FooterWorkedChip).replace(
                    "{duration}",
                    &crate::elapsed::format_elapsed_secs(worked.as_secs()),
                ),
                if turn.is_some() {
                    ink
                } else {
                    ChromeInk::MetadataValue
                },
            )
        });
    (turn_clock, session_clock)
}

/// Context window percentage — the snapshot the metrics line's reading
/// paints, and the posture bar's ≥80% cap-warning trigger.
pub(crate) fn context_percent_from_app(app: &App) -> u8 {
    crate::tui::ui::context_usage_snapshot(app)
        .map(|(_, _, percent)| percent.round().clamp(0.0, 100.0) as u8)
        .unwrap_or(0)
}

/// The live counts: running sub-agents, live shells, background tasks, and
/// scheduled automation. Each count is zero-suppressed — the bar never
/// grows furniture for work that is not happening.
fn live_counts(
    app: &App,
    tier: ShellTier,
) -> (
    Vec<(String, ChromeInk)>,
    Vec<crate::tui::tideline::InteractionAction>,
) {
    use crate::tui::background_indicator::{PendingItemKind, pending_work_from_app};
    use crate::tui::tideline::InteractionAction;
    use crate::tui::work_surface::RailPanel;
    let mut counts = Vec::new();
    let mut panels = Vec::new();
    let agents = crate::tui::subagent_routing::running_agent_count(app);
    match agents {
        0 => {}
        1 => {
            counts.push((
                tr(app.ui_locale, MessageId::FooterAgentSingular).into_owned(),
                ChromeInk::Active,
            ));
            panels.push(InteractionAction::ShowDockPanel(RailPanel::Agents));
        }
        n => {
            counts.push((
                tr(app.ui_locale, MessageId::FooterAgentsPlural).replace("{count}", &n.to_string()),
                ChromeInk::Active,
            ));
            panels.push(InteractionAction::ShowDockPanel(RailPanel::Agents));
        }
    }
    let shells = app
        .task_panel
        .iter()
        .filter(|entry| crate::tui::background_indicator::is_live_shell_entry(entry))
        .count();
    if shells > 0 {
        counts.push((
            format!("{shells} {}", PendingItemKind::Shell.plural_noun(shells)),
            ChromeInk::Active,
        ));
        panels.push(InteractionAction::ShowDockPanel(RailPanel::Background));
    }
    let tasks = pending_work_from_app(app).count(PendingItemKind::Task);
    if tasks > 0 {
        counts.push((
            format!("{tasks} {}", PendingItemKind::Task.plural_noun(tasks)),
            ChromeInk::Active,
        ));
        panels.push(InteractionAction::ShowDockPanel(RailPanel::Background));
    }
    // Scheduled automation: the `AutomationPanelState` projection stays the
    // single owner; Compact keeps the abbreviated count (chrome sheds
    // before content) and the ink says whether a run failed unacknowledged.
    let automation = if tier == ShellTier::Compact {
        app.automation_panel.activity_slot_compact()
    } else {
        app.automation_panel.activity_slot(app.ui_locale)
    };
    if let Some(automation) = automation {
        counts.push((automation, app.automation_panel.activity_ink()));
        panels.push(InteractionAction::OpenAutomations);
    }
    // With nothing live there is still one bottom affordance that opens the
    // dock (founder, 2026-09-03: the bar opens when used, or when you click
    // something at the bottom to ask for it). The word is the dock's own
    // TODO view title; it disappears while the dock is up.
    //
    // It reads at `MetadataValue`, not `MetadataDim`: `TEXT_DIM` is aliased to
    // `TEXT_HINT` in every theme but Solarized, so a dim affordance paints the
    // exact grey as the separators around it and the one clickable word in an
    // idle footer disappears into punctuation. Live counts already carry
    // `Active`; this is the idle case earning the same "you can click this".
    if counts.is_empty() && app.work_surface.last_area.is_none() {
        // The word alone did not say it was an affordance, let alone which
        // key opened it — founder live-test: "what do we press at the bottom
        // to get the workbar to show up?". It carries its chord until the
        // binding has been used, exactly like the permission and mode chips.
        let label = RailPanel::Tasks.title().to_ascii_lowercase();
        let chord = crate::tui::shell_key_routing::binding(
            crate::tui::shell_key_routing::ShellBindingId::ViewCycle,
        )
        .footer_chord;
        let label = if crate::tui::footer_hints::retired(
            &app.footer_hint_uses,
            crate::tui::footer_hints::DOCK_OPEN,
        ) {
            label
        } else {
            format!("{label} ({chord})")
        };
        counts.push((label, ChromeInk::MetadataValue));
        panels.push(InteractionAction::ShowDockPanel(RailPanel::Tasks));
    }
    (counts, panels)
}

/// Build the posture bar's facts from live `App` state. `width` is the
/// row's width — notices clause-shed against it, never dangle.
pub(crate) fn tideline_footer_from_app(app: &mut App, width: u16) -> TidelineFooterFacts {
    use crate::tui::shell_key_routing::{ShellBindingId, binding};
    let activity = LiveActivity::from_app(app);
    let phase = ShellPhase::from_app_with_activity(app, activity);
    let (_, phase_label) = phase_marker_with_activity(app, phase, activity);
    let tier = ShellTier::for_chrome_width(width);
    let focus = app.focus();

    let (mode_chip, permission_chip) = crate::tui::underwater::posture_chips(app);
    let permission_chip = permission_chip
        .map(|(text, ink)| (text.into_owned(), ink))
        .unwrap_or_else(|| (String::new(), ChromeInk::PermissionAsk));
    // The mode chip is the one posture fact `/statusline` composes (#5950):
    // its `StatusItem::Mode` toggle used to be inert. The permission chip,
    // the working clocks (#5914) and the live counts are the bar's own
    // posture statement and stay unconditional.
    let mode_chip = mode_chip
        .filter(|_| app.status_items.contains(&crate::config::StatusItem::Mode))
        .map(|(text, ink)| (text.into_owned(), ink));
    // Cycle keys come from the binding table and only when that binding is
    // live for the current focus.
    let live_chord = |id: ShellBindingId| -> Option<&'static str> {
        let binding = binding(id);
        binding.focus.admits(focus).then_some(binding.footer_chord)
    };

    // The one hint that applies now: the double-tap send-now window while a
    // turn is running, else the interrupt affordance, else the arrow keys
    // the empty composer lends to the agent roster. Each hint retires once
    // its binding has been used enough times (`footer_hints::retired`): a
    // taught binding renders the bare state, never more chrome.
    let hint = if app.double_tap_window_open() {
        Some((
            tr(app.ui_locale, MessageId::PostureHintEnterAgain)
                .replace("{enter}", "Enter")
                .replace("{steer}", "Ctrl+Enter"),
            ChromeInk::MetadataHint,
            crate::tui::footer_hints::ENTER_AGAIN,
        ))
    } else if matches!(phase, ShellPhase::Working | ShellPhase::Verifying) {
        Some((
            tr(app.ui_locale, MessageId::FooterHintEscInterrupt).into_owned(),
            ChromeInk::MetadataHint,
            crate::tui::footer_hints::ESC_INTERRUPT,
        ))
    } else if crate::tui::agent_focus::shell_shortcuts_available(app, false) {
        Some((
            crate::tui::agent_focus::footer_agent_hints(app),
            ChromeInk::MetadataHint,
            crate::tui::footer_hints::AGENT_ARROWS,
        ))
    } else {
        None
    };
    let hint = hint
        .filter(|(_, _, key)| !crate::tui::footer_hints::retired(&app.footer_hint_uses, key))
        .map(|(text, ink, _)| (text, ink));

    // The right slot: the live status toast if one is owed, else the compact
    // MCP or plugin boot chip, else the remote-control state when it is on.
    // Clause-shed against half the row — the posture facts own the other
    // half.
    let notice_budget = (usize::from(width) / 2).max(8);
    let right = selected_notice(app.active_status_toast(phase), &phase_label)
        .map(|(text, ink, _urgent)| (text, ink))
        .or_else(|| {
            // The launch screen carries the full MCP block — every state, with
            // the failing and unauthorized servers named. Repeating a squeezed
            // one-server chip down here would give the same fact two homes and
            // show strictly less of it. Suppress the chip, not the surface:
            // `SessionBootSurface::from_app` stays untouched so every other
            // consumer of boot state, including the launch block itself, is
            // unaffected.
            if app.launch.visible {
                return None;
            }
            let boot = crate::tui::session_boot::SessionBootSurface::from_app(app);
            boot.activity_notice(app.ui_locale, notice_budget)
                .map(|chip| (chip.text, boot_activity_ink(chip.level)))
        })
        .and_then(|(text, ink)| fit_notice(&text, notice_budget).map(|fitted| (fitted, ink)))
        .or_else(|| {
            app.remote_control
                .status_word()
                .map(|word| (format!("/rc {word}"), ChromeInk::Info))
        });

    let (turn_clock, session_clock) = working_clock(app, phase, &phase_label);
    let (counts, count_actions) = live_counts(app, tier);
    TidelineFooterFacts {
        permission_chip,
        permission_key: live_chord(ShellBindingId::PermissionCycle).filter(|_| {
            !crate::tui::footer_hints::retired(
                &app.footer_hint_uses,
                crate::tui::footer_hints::PERMISSION_CYCLE,
            )
        }),
        mode_chip,
        mode_key: live_chord(ShellBindingId::ModeCycle).filter(|_| {
            !crate::tui::footer_hints::retired(
                &app.footer_hint_uses,
                crate::tui::footer_hints::MODE_CYCLE,
            )
        }),
        turn_clock,
        counts,
        session_clock,
        count_actions,
        hint,
        context_percent: context_percent_from_app(app),
        right,
    }
}

#[cfg(test)]
mod tideline_tests;

#[cfg(test)]
mod neutrality_tests {
    #[test]
    fn session_metrics_strip_is_on_by_default() {
        assert!(
            crate::config::StatusItem::default_footer()
                .contains(&crate::config::StatusItem::SessionMetrics)
        );
        assert_eq!(
            crate::config::StatusItem::from_key("session_metrics"),
            Some(crate::config::StatusItem::SessionMetrics)
        );
    }
}
