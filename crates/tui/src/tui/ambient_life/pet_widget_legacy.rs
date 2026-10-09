//! Frozen current packed-raster painter, acceptance only.
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::Style,
    widgets::{Paragraph, Widget, Wrap},
};
use unicode_width::UnicodeWidthStr;

pub(super) fn render_grid(area: Rect, buf: &mut Buffer, grid: &[u8], label: &str, style: Style) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    if label.width() > usize::from(area.width) || area.height < 4 {
        Paragraph::new(label)
            .style(style)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false })
            .render(area, buf);
        return;
    }
    for y in 0..area.height - 1 {
        for x in 0..area.width {
            let bits = grid
                .get(usize::from(y) * usize::from(area.width) + usize::from(x))
                .copied()
                .unwrap_or(0);
            if bits != 0
                && let Some(cell) = buf.cell_mut((area.x + x, area.y + y))
            {
                let glyph = char::from_u32(0x2800 + u32::from(bits)).expect("braille");
                cell.set_symbol(&glyph.to_string()).set_style(style);
            }
        }
    }
    let x = area.x + (area.width - label.width() as u16) / 2;
    buf.set_stringn(x, area.bottom() - 1, label, usize::from(area.width), style);
}
