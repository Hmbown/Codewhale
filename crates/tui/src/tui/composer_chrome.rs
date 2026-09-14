//! Ocean composer chrome policy.
//!
//! The composer auto-fits its content: one input row when empty or
//! single-line, growing with typed content up to the density cap. Comfortable
//! and spacious densities reserve quiet rows around short input when room is
//! available. Compact panes always give that space back to the transcript.

use crate::tui::app::ComposerDensity;

/// Top/bottom chrome rows for the quiet rule (TOP border only) or the
/// enclosed panel (TOP + BOTTOM), plus the total-row growth cap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComposerChrome {
    pub border_rows: u16,
    pub max_total_rows: u16,
}

impl ComposerChrome {
    /// Baseline for the given density. Panel shape gets both borders;
    /// quiet shape keeps a single top rule so the prompt still has a
    /// clear ledge without reading as a card. Density picks the growth
    /// cap; desired_height adds the density's bounded input padding.
    #[must_use]
    pub fn for_density(density: ComposerDensity, enclosed_panel: bool) -> Self {
        let border_rows = if enclosed_panel { 2 } else { 1 };
        let max_total_rows = match density {
            ComposerDensity::Compact => 7,
            ComposerDensity::Comfortable => 9,
            ComposerDensity::Spacious => 12,
        };
        Self {
            border_rows,
            max_total_rows,
        }
    }
}

/// Decide how many rows the composer should occupy.
///
/// The height follows the content: one input row when the composer is
/// empty or holds a single line, growing one row per content line up to
/// the density cap (`max_total_rows`) or the available height, whichever
/// is smaller. Comfortable/spacious density keeps a stable two/three-row
/// input floor when space permits. Menu rows and border chrome add on top. Compact
/// terminals shed the border before they shed typed content.
#[must_use]
pub fn desired_height(
    content_lines: usize,
    extra_menu_lines: usize,
    available_height: u16,
    density: ComposerDensity,
    enclosed_panel: bool,
) -> u16 {
    let chrome = ComposerChrome::for_density(density, enclosed_panel);
    let available = available_height.max(1);
    let input_floor = match density {
        ComposerDensity::Compact => 1,
        ComposerDensity::Comfortable => 2,
        ComposerDensity::Spacious => 3,
    };
    let content = content_lines.max(input_floor);
    let wants_panel = enclosed_panel && available >= 3;

    let border = if wants_panel {
        usize::from(chrome.border_rows)
    } else if available >= 2 {
        1
    } else {
        0
    };

    let total = content
        .saturating_add(extra_menu_lines)
        .saturating_add(border);
    let max_height = usize::from(available.min(chrome.max_total_rows).max(1));
    total.clamp(1, max_height).try_into().unwrap_or(1)
}

/// Top padding inside the content budget. Keep at least one quiet row below a
/// short prompt when the budget has room, instead of bottom-pinning
/// the caret directly against the phase footer. Compact heights naturally
/// report zero padding once the budget collapses. A single spare row stays
/// below the caret; do not spend it all above the input against the footer.
#[must_use]
pub fn top_padding(content_lines: usize, rows_budget: usize) -> usize {
    let content = content_lines.max(1).min(rows_budget.max(1));
    let spare = rows_budget.saturating_sub(content);
    spare / 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_composer_respects_density_and_keeps_padding_below_the_caret() {
        for (density, height) in [
            (ComposerDensity::Compact, 2),
            (ComposerDensity::Comfortable, 3),
            (ComposerDensity::Spacious, 4),
        ] {
            assert_eq!(desired_height(1, 0, 8, density, false), height);
        }
        assert_eq!(
            top_padding(1, 2),
            0,
            "one spare row belongs below the input"
        );
        assert_eq!(top_padding(1, 3), 1);
    }

    #[test]
    fn compact_height_sheds_border_before_content() {
        // Only two rows available: keep a border + one content row.
        let height = desired_height(1, 0, 2, ComposerDensity::Comfortable, false);
        assert_eq!(height, 2);
    }

    #[test]
    fn content_growth_expands_up_to_the_density_cap() {
        // Six content rows + border fits under the Comfortable cap of 9.
        let height = desired_height(6, 0, 12, ComposerDensity::Comfortable, false);
        assert_eq!(height, 7, "typed content must grow the composer: {height}");

        // Past the cap the density setting wins, not the content.
        let capped = desired_height(20, 0, 30, ComposerDensity::Comfortable, false);
        assert_eq!(capped, 9, "Comfortable caps total rows at 9");
        let spacious = desired_height(20, 0, 30, ComposerDensity::Spacious, false);
        assert_eq!(spacious, 12, "Spacious caps total rows at 12");
    }

    #[test]
    fn spacious_panel_reserves_input_padding_and_both_borders() {
        let height = desired_height(1, 0, 12, ComposerDensity::Spacious, true);
        assert_eq!(height, 5, "panel = 2 borders + 3 input rows, got {height}");
    }
}

// ---------------------------------------------------------------------------
// Tideline composer restyle (spec §2 composer decision, §5a "Composer"):
// rounded border + `[↑]` send hitbox. Translation scaffolding in
// the topbar mold — a pure, deterministic widget over injected state; the
// composer authority logic (composer_ui.rs) is untouched, and wiring into
// `ui/frame.rs` is the landing slice after #5698 settles.
//
// Cell rules (spec §2): no bezier strokes — `╭─╮│╰╯` border dim at rest and
// Info on focus; the send `↑` is a 3-cell `[↑]` hitbox right-aligned inside
// the border. The hand-drawn three-cell crown fluke this cap used to carry
// was deleted by the 2026-08-29 founder decree (terminal marks must be
// generated from the brand master path, never hand-drawn); the corner is a
// plain `╮` again. The hull taper silhouette is deliberately dropped
// (sub-cell vector work).

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
};
use unicode_width::UnicodeWidthStr;

use codewhale_palette::{ChromeInk, UiTheme, chrome_style};

/// Fixed width of the painted `[↑]` submit control.
pub const TIDELINE_COMPOSER_SUBMIT_WIDTH: u16 = 3;

/// Blank cell between input content and the painted submit control.
pub const TIDELINE_COMPOSER_SUBMIT_BREATHING_WIDTH: u16 = 1;

/// What the caller owes the composer chrome. Draft, queued-crumb, and
/// approval state are injected so renders stay deterministic for goldens.
#[allow(dead_code)] // translation scaffolding: wired by the landing slice
pub struct TidelineComposer<'a> {
    pub theme: &'a UiTheme,
    pub focused: bool,
    /// Current draft (first line is shown; wrapping stays the caller's).
    pub input: &'a str,
    /// Queued-message crumb rendered as one row above the input line
    /// (spec §3: slot 3 pending-preview merges into the composer).
    pub pending_crumb: Option<&'a str>,
    /// When a permission ask replaces the input line (approval-replaced
    /// state, spec §5a), its one-line summary.
    pub approval_summary: Option<&'a str>,
    pub ascii_safe: bool,
}

#[allow(dead_code)] // translation scaffolding: builder methods feed tests + the landing slice
impl<'a> TidelineComposer<'a> {
    #[allow(dead_code)] // translation scaffolding: wired by the landing slice
    #[must_use]
    pub fn new(theme: &'a UiTheme, input: &'a str) -> Self {
        Self {
            theme,
            focused: false,
            input,
            pending_crumb: None,
            approval_summary: None,
            ascii_safe: false,
        }
    }

    #[must_use]
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    #[must_use]
    pub fn pending_crumb(mut self, crumb: Option<&'a str>) -> Self {
        self.pending_crumb = crumb;
        self
    }

    #[must_use]
    pub fn ascii_safe(mut self, ascii_safe: bool) -> Self {
        self.ascii_safe = ascii_safe;
        self
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
}

fn chrome(theme: &UiTheme, ink: ChromeInk) -> Style {
    chrome_style(theme, ink)
}

fn put(buf: &mut Buffer, x: u16, y: u16, text: &str, style: Style) {
    let width = text.width();
    buf.set_stringn(x, y, text, width, style);
}

fn symbol(glyph: &str, ascii_safe: bool) -> String {
    if !ascii_safe {
        return glyph.to_string();
    }
    if let Some(fallback) = crate::tui::glyphs::ascii_fallback(glyph) {
        return fallback.to_string();
    }
    glyph
        .chars()
        .map(|ch| {
            crate::tui::glyphs::ascii_fallback(&ch.to_string())
                .map(str::to_string)
                .unwrap_or_else(|| ch.to_string())
        })
        .collect()
}

/// Shared geometry for the rounded Tideline composer shell.
///
/// Rendering, launch hit-testing, and the live composer must derive their
/// interior and submit rect from this one cell map. Otherwise a visible
/// `[↑]` can drift away from the mouse target at a terminal width boundary.
#[derive(Debug, Clone, Copy)]
pub struct TidelineComposerGeometry {
    /// Interior input rows, excluding the one-cell rails, the submit control,
    /// and its one-cell breathing space.
    pub content: Rect,
    /// The visible three-cell `[↑]` submit affordance.
    pub submit: Rect,
}

/// Derive the fixed shell geometry. The caller must only paint the rounded
/// shell when the area is at least three rows tall.
#[must_use]
pub fn tideline_composer_geometry(area: Rect) -> TidelineComposerGeometry {
    let rail_width = 1;
    let interior_breathing_width = 1;
    let submit = Rect {
        x: area.x.saturating_add(area.width.saturating_sub(
            rail_width + interior_breathing_width + TIDELINE_COMPOSER_SUBMIT_WIDTH,
        )),
        y: area.y.saturating_add(area.height.saturating_sub(2)),
        width: TIDELINE_COMPOSER_SUBMIT_WIDTH.min(area.width),
        height: 1.min(area.height),
    };
    let content_x = area.x.saturating_add(rail_width + interior_breathing_width);
    let content_right = submit
        .x
        .saturating_sub(TIDELINE_COMPOSER_SUBMIT_BREATHING_WIDTH);
    let content = Rect {
        x: content_x,
        y: area.y.saturating_add(1),
        width: content_right.saturating_sub(content_x),
        height: area.height.saturating_sub(2),
    };
    TidelineComposerGeometry { content, submit }
}

/// Paint only the shared rounded shell and its visible `[↑]` submit target.
///
/// Content remains caller-owned: the launch surface supplies its localized
/// placeholder/caret/hint projection, while the live composer supplies its
/// multiline editor. Sharing this shell keeps the visual component and exact
/// submit geometry coherent without creating a second input authority.
pub fn render_tideline_composer_shell(
    area: Rect,
    buf: &mut Buffer,
    theme: &UiTheme,
    focused: bool,
    ascii_safe: bool,
) {
    if area.width < 6 || area.height < 3 {
        return;
    }
    let border_ink = if focused {
        ChromeInk::Info
    } else {
        ChromeInk::MetadataDim
    };
    let border = chrome(theme, border_ink);
    let top_fill = usize::from(area.width.saturating_sub(2).max(1));
    let top: String = std::iter::once('╭')
        .chain(std::iter::repeat_n('─', top_fill))
        .chain(std::iter::once('╮'))
        .collect();
    put(buf, area.x, area.y, &symbol(&top, ascii_safe), border);

    let bottom_fill = usize::from(area.width.saturating_sub(2));
    let bottom: String = std::iter::once('╰')
        .chain(std::iter::repeat_n('─', bottom_fill))
        .chain(std::iter::once('╯'))
        .collect();
    put(
        buf,
        area.x,
        area.y + area.height - 1,
        &symbol(&bottom, ascii_safe),
        border,
    );

    let rail = symbol("│", ascii_safe);
    let rail_width = rail.width() as u16;
    for y in (area.y + 1)..(area.y + area.height - 1) {
        put(buf, area.x, y, &rail, border);
        put(buf, area.x + area.width - rail_width, y, &rail, border);
    }

    render_tideline_composer_submit(area, buf, theme, focused, ascii_safe);
}

/// Paint or restore the visible `[↑]` affordance above caller-owned content.
///
/// The standalone shell paints it immediately. The multiline work composer
/// calls this again after it has painted a long input or queued crumb, so that
/// content can never overwrite the one cell target the user is meant to click.
pub fn render_tideline_composer_submit(
    area: Rect,
    buf: &mut Buffer,
    theme: &UiTheme,
    focused: bool,
    ascii_safe: bool,
) {
    if area.width < 6 || area.height < 3 {
        return;
    }
    let geometry = tideline_composer_geometry(area);
    let send = symbol("[↑]", ascii_safe);
    let send_ink = if focused {
        ChromeInk::Active
    } else {
        ChromeInk::MetadataDim
    };
    put(
        buf,
        geometry.submit.x,
        geometry.submit.y,
        &send,
        chrome(theme, send_ink),
    );
}

/// Paint the composer chrome. Deterministic: the caller owns the caret clock
/// (a `low_motion` caller passes the still `_`); this render shows the draft
/// and a terminal caret block.
pub fn render_tideline_composer(area: Rect, buf: &mut Buffer, composer: &TidelineComposer<'_>) {
    if area.width < 6 || area.height < 3 {
        return;
    }
    let theme = composer.theme;
    render_tideline_composer_shell(area, buf, theme, composer.focused, composer.ascii_safe);

    let geometry = tideline_composer_geometry(area);
    let inner_x = geometry.content.x;
    let inner_w = geometry.content.width.max(1);
    let content_top = geometry.content.y;
    // Last row *inside* the border (the bottom border owns the final row).
    let content_bottom = geometry.content.bottom().saturating_sub(1);

    // Content rows: the crumb (if any) sits one row above the input line
    // (spec §3 slot-3 merge); without a crumb the input takes the first
    // content row and the quiet row under it carries only the send hitbox.
    let input_y = if composer.pending_crumb.is_some() && content_bottom > content_top {
        content_bottom
    } else {
        content_top
    };
    if let Some(crumb) = composer.pending_crumb {
        let text = composer.sym(&format!("… queued: {crumb}"));
        put(
            buf,
            inner_x,
            content_top,
            &truncate_cells(&text, inner_w as usize),
            chrome(theme, ChromeInk::MetadataHint),
        );
    }

    // Input line (or the approval ask that replaced it).
    if let Some(approval) = composer.approval_summary {
        let text = composer.sym(&format!("◆ approve: {approval}"));
        put(
            buf,
            inner_x,
            input_y,
            &truncate_cells(&text, inner_w as usize),
            chrome(theme, ChromeInk::PermissionAsk).add_modifier(Modifier::BOLD),
        );
    } else {
        let draft = if composer.input.is_empty() {
            String::new()
        } else {
            composer.sym(composer.input)
        };
        let caret = if composer.ascii_safe { "_" } else { "▌" };
        let line = format!("{draft}{caret}");
        let line = truncate_cells(&line, inner_w as usize);
        let ink = if composer.focused {
            ChromeInk::MetadataValue
        } else {
            ChromeInk::Metadata
        };
        put(buf, inner_x, input_y, &line, chrome(theme, ink));
    }

    // `render_tideline_composer_shell` paints the action for standalone
    // callers; restore it after content so a long draft cannot erase it.
    render_tideline_composer_submit(area, buf, theme, composer.focused, composer.ascii_safe);
}

/// Truncate a rendered string to `width` cells on a char boundary (never
/// wrap — the composer is one line per row).
fn truncate_cells(text: &str, width: usize) -> String {
    let mut out = String::new();
    let mut used = 0;
    for ch in text.chars() {
        let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + w > width {
            break;
        }
        out.push(ch);
        used += w;
    }
    out
}

#[cfg(test)]
mod tideline_tests;
