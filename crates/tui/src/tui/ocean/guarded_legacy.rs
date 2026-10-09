use super::*;
use crate::tui::ambient_life::{frame_ocean_ramp, occupied_text_bounds};
use crate::tui::ocean;
use ratatui::{style::Modifier, text::Line};

pub(super) struct LegacyColumn(pub(super) OceanColumn);
impl LegacyColumn {
    fn color_at_y(&self, y: u16) -> Color {
        self.0.color_at_y(y)
    }
    pub fn paint_matching(self, area: Rect, buf: &mut Buffer, background: Color) {
        let area = area.intersection(buf.area);
        for y in area.top()..area.bottom() {
            let row_bg = self.color_at_y(y);
            for x in area.left()..area.right() {
                let cell = &mut buf[(x, y)];
                if cell.bg == background && !cell.modifier.contains(Modifier::REVERSED) {
                    cell.set_bg(row_bg);
                }
            }
        }
    }
}

pub(super) struct LegacyWater {
    pub(super) ocean_column: Option<OceanColumn>,
    pub(super) lines: Vec<Line<'static>>,
    pub(super) background: Color,
    pub(super) ocean_elapsed_ms: u128,
}
impl LegacyWater {
    pub(super) fn paint(&self, area: Rect, buf: &mut Buffer) {
        if let Some(column) = self.ocean_column {
            // Cache per-row ocean colors; invalidate only on phase/size/breath.
            let phase_tag = column.phase_tag();
            let fingerprint = column.ramp_fingerprint();
            let ramp = crate::tui::ambient_life::frame_ocean_ramp(
                &column,
                area.height,
                area.y,
                self.ocean_elapsed_ms,
                phase_tag,
                fingerprint,
            );
            for local_y in 0..area.height {
                let protected = self
                    .lines
                    .get(usize::from(local_y))
                    .and_then(occupied_text_bounds);
                let row_bg = ramp
                    .get(usize::from(local_y))
                    .copied()
                    .unwrap_or_else(|| column.color_at_y(area.y.saturating_add(local_y)));
                for local_x in 0..area.width {
                    let is_protected = protected.is_some_and(|(start, end)| {
                        usize::from(local_x) >= start && usize::from(local_x) < end
                    });
                    let cell = &mut buf[(area.x + local_x, area.y + local_y)];
                    // Plain transcript text participates in the water column;
                    // explicit semantic surfaces (selection, code, warnings)
                    // retain their own background.
                    if !is_protected || cell.bg == self.background {
                        cell.set_bg(row_bg);
                    }
                }
            }
        }
    }
}

pub fn apply_caustic_shimmer(
    area: Rect,
    buf: &mut Buffer,
    column: &OceanColumn,
    elapsed_ms: u128,
    animated: bool,
    lines: &[Line<'static>],
) {
    if !animated || area.width < AMBIENT_MIN_WIDTH || area.height < AMBIENT_MIN_HEIGHT {
        return;
    }
    // Sparse sampling: every 3rd column on every other row near the surface.
    //
    // The light stops where the composition starts. Sunlight raking across
    // the rows a wordmark is sitting in is the same failure as a fish
    // swimming through them, just quieter, and it costs nothing to measure:
    // the surface band is clipped to the first row that carries text.
    let ceiling = (0..area.height)
        .find(|row| {
            lines
                .get(usize::from(*row))
                .and_then(occupied_text_bounds)
                .is_some()
        })
        .unwrap_or(area.height);
    let band = (area.height / 3).max(2).min(ceiling);
    for local_y in 0..band {
        let protected = lines
            .get(usize::from(local_y))
            .and_then(occupied_text_bounds);
        let ramp = frame_ocean_ramp(
            column,
            area.height,
            area.y,
            elapsed_ms,
            column.phase_tag(),
            column.ramp_fingerprint(),
        );
        let row_bg = ramp
            .get(usize::from(local_y))
            .copied()
            .unwrap_or_else(|| column.color_at_y(area.y.saturating_add(local_y)));
        for local_x in (0..area.width).step_by(3) {
            if protected.is_some_and(|(start, end)| {
                usize::from(local_x) >= start && usize::from(local_x) < end
            }) {
                continue;
            }
            let cell = &mut buf[(area.x + local_x, area.y + local_y)];
            // Soften toward ambient ink without replacing semantic glyphs.
            if cell.symbol() == " " || cell.symbol().is_empty() {
                // Sunlight dissolves with depth instead of stopping: full
                // amplitude at the surface easing to zero at the band's
                // floor. The former hard cutoff at `band` drew a visible
                // horizontal line across tall windows.
                let depth_fade = 1.0 - f32::from(local_y) / f32::from(band.max(1));
                let shimmer = ocean::scale_color(
                    row_bg,
                    caustic_brightness(elapsed_ms, local_x, local_y, depth_fade * depth_fade),
                );
                cell.set_bg(shimmer);
            }
        }
    }
}

/// Continuous travelling caustic. The former `(elapsed / 80) % 12` mask
/// toggled cells fully on/off at 12.5 Hz; truecolor made that quantization look
/// like dropped frames. A narrow cosine crest preserves the same sparse light
/// band while cross-fading every sampled cell between frames.
fn caustic_brightness(elapsed_ms: u128, local_x: u16, local_y: u16, depth_fade: f32) -> f32 {
    const CYCLE_MS: f64 = 960.0;
    const SPATIAL_SLOTS: f64 = 4.0;
    let time = (elapsed_ms % CYCLE_MS as u128) as f64 / CYCLE_MS;
    // The sampled grid advances by three terminal columns. Four grid phases
    // therefore preserve the old 12-column repeat instead of stretching the
    // caustic topology while changing only its temporal interpolation.
    let slot = (u32::from(local_x / 3) + u32::from(local_y)) % 4;
    let phase = (time + f64::from(slot) / SPATIAL_SLOTS) * std::f64::consts::TAU;
    let crest = ((phase.cos() + 1.0) * 0.5).powi(8);
    1.0 + 0.08 * (crest as f32) * depth_fade.clamp(0.0, 1.0)
}

// End of exact legacy caustic/brightness source fragment.
