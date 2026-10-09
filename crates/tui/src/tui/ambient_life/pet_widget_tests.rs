use super::*;
use ratatui::style::Modifier;

fn seeded(area: Rect) -> Buffer {
    let mut buffer = Buffer::empty(area);
    for cell in &mut buffer.content {
        cell.set_symbol("~").set_style(
            Style::default()
                .fg(Color::Yellow)
                .bg(Color::Rgb(9, 23, 41))
                .add_modifier(Modifier::ITALIC | Modifier::REVERSED),
        );
    }
    buffer
}

#[test]
fn packed_native_cameo_and_world_buffers_match_frozen_painter() {
    let styles = [
        Style::default(),
        Style::default().fg(Color::Rgb(21, 71, 129)),
        Style::default()
            .fg(Color::Cyan)
            .bg(Color::Rgb(17, 19, 31))
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
    ];
    for width in 0..=32 {
        for height in 0..=12 {
            let area = Rect::new(4, 3, width, height);
            for label in [
                "m · 6",
                "resting",
                "鲸 · cafe\u{0301}",
                "a cue wider than this viewport",
            ] {
                for &style in &styles {
                    // Includes the actual 18×5 cameo, transparent cells, and
                    // short/extra grids the shared embedded-world facade accepts.
                    let grid = (0..90)
                        .map(|i| if i % 4 == 0 { 0 } else { (i % 255 + 1) as u8 })
                        .collect::<Vec<_>>();
                    for cells in [&grid[..], &grid[..7], &[][..]] {
                        let mut actual = seeded(Rect::new(1, 1, 40, 18));
                        let mut expected = actual.clone();
                        super::pet_widget_legacy::render_grid(
                            area,
                            &mut expected,
                            cells,
                            label,
                            style,
                        );
                        render_grid(area, &mut actual, cells, label, style);
                        assert_eq!(
                            actual,
                            expected,
                            "area={area:?} label={label:?} style={style:?} cells={}",
                            cells.len()
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn packed_native_grid_clips_without_reflow_or_changing_transparent_cells() {
    let full_area = Rect::new(3, 2, 18, 6);
    let clipped = Rect::new(13, 2, 6, 6);
    let grid = (0..90)
        .map(|i| if i % 3 == 0 { 0 } else { (i + 1) as u8 })
        .collect::<Vec<_>>();
    let style = Style::default()
        .fg(Color::Rgb(33, 87, 142))
        .add_modifier(Modifier::BOLD);
    let mut full = seeded(Rect::new(0, 0, 25, 10));
    render_grid(full_area, &mut full, &grid, "m · 6", style);
    let mut actual = seeded(clipped);
    render_grid(full_area, &mut actual, &grid, "m · 6", style);
    for y in clipped.y..clipped.bottom() {
        for x in clipped.x..clipped.right() {
            assert_eq!(actual[(x, y)], full[(x, y)], "cell=({x},{y})");
        }
    }
}
