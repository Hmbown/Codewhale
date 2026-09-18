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

use ratatui::{buffer::Buffer, layout::Rect, style::Style};
use unicode_width::UnicodeWidthStr;

use codewhale_palette::{ChromeInk, UiTheme, chrome_style};

/// Fixed width of the painted `[↑]` submit control.
pub const TIDELINE_COMPOSER_SUBMIT_WIDTH: u16 = 3;

/// Blank cell between input content and the painted submit control.
pub const TIDELINE_COMPOSER_SUBMIT_BREATHING_WIDTH: u16 = 1;

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
