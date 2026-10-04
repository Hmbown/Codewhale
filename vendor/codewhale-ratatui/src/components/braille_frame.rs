//! Transparent packed Braille supplied by the host's existing simulation.
//! No clock, activity classification, state transition or color inference.

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::Style,
    widgets::{Paragraph, Widget, Wrap},
};
use unicode_width::UnicodeWidthStr;

/// Raw-caption companion raster. Tiny viewports retain the complete wrapped
/// non-color cue; zero cells remain transparent. Ink includes host modifiers.
#[derive(Clone, Copy, Debug)]
pub struct BrailleFrame<'a> {
    pub cells: &'a [u8],
    pub caption: &'a str,
    pub style: Style,
}

/// Shared with Whale's semantic-caption/ombre renderer. Coordinates are the
/// original grid coordinates even when the buffer clips its edges.
pub(crate) fn paint_braille_cells(
    area: Rect,
    buf: &mut Buffer,
    cells: &[u8],
    mut ink: impl FnMut(u16) -> Style,
) {
    let visible = area.intersection(buf.area);
    for y in visible.y..visible.bottom() {
        let row = y - area.y;
        let style = ink(row);
        for x in visible.x..visible.right() {
            let index = usize::from(row) * usize::from(area.width) + usize::from(x - area.x);
            let bits = cells.get(index).copied().unwrap_or(0);
            if bits != 0
                && let Some(cell) = buf.cell_mut((x, y))
            {
                let glyph = char::from_u32(0x2800 + u32::from(bits)).expect("braille");
                let mut encoded = [0; 4];
                cell.set_symbol(glyph.encode_utf8(&mut encoded))
                    .set_style(style);
            }
        }
    }
}

impl Widget for BrailleFrame<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        if self.caption.width() > usize::from(area.width) || area.height < 4 {
            Paragraph::new(self.caption)
                .style(self.style)
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: false })
                .render(area, buf);
            return;
        }
        paint_braille_cells(
            Rect {
                height: area.height - 1,
                ..area
            },
            buf,
            self.cells,
            |_| self.style,
        );
        let x = area
            .x
            .saturating_add((area.width - self.caption.width() as u16) / 2);
        let y = area.bottom().saturating_sub(1);
        if buf.area.contains((x, y).into()) {
            buf.set_stringn(x, y, self.caption, usize::from(area.width), self.style);
        } else if y >= buf.area.y && y < buf.area.bottom() && x < buf.area.right() {
            let mut caption = Buffer::empty(Rect::new(0, 0, area.width, 1));
            for offset in 0..area.width {
                if let Some(cell) = buf.cell((x.saturating_add(offset), y)) {
                    caption[(offset, 0)] = cell.clone();
                }
            }
            let (end, _) =
                caption.set_stringn(0, 0, self.caption, usize::from(area.width), self.style);
            for offset in 0..end {
                if let Some(cell) = buf.cell_mut((x.saturating_add(offset), y)) {
                    *cell = caption[(offset, 0)].clone();
                }
            }
        }
    }
}
