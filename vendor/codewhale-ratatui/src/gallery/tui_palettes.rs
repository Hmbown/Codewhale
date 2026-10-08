//! All the palettes available in Codewhale's terminal theme picker.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::Line,
};

use super::Entry;
use crate::{Caps, Paint, Role, Theme, TuiInk, TuiPalette, WorkbarPanel, detect::Appearance, text};

/// Known terminal grounds use the preset's appearance; an unknown ground
/// continues to use the host's conservative capability fallback.
pub(crate) fn theme_for(palette: TuiPalette, input: &Theme) -> Theme {
    let caps = input.caps();
    Theme::new(Caps {
        appearance: if caps.appearance == Appearance::Unknown {
            Appearance::Unknown
        } else if palette.light() {
            Appearance::Light
        } else {
            Appearance::Dark
        },
        ..caps
    })
    .tui_palette(palette)
}

pub(crate) fn palette_for_name(name: &str) -> Option<TuiPalette> {
    let name = name.strip_prefix("tui-theme-")?;
    TuiPalette::ALL
        .into_iter()
        .find(|palette| palette.name() == name)
}

fn band(area: Rect, top: u16, height: u16) -> Rect {
    let top = top.min(area.height);
    Rect::new(
        area.x,
        area.y.saturating_add(top),
        area.width,
        height.min(area.height.saturating_sub(top)),
    )
}

fn row(area: Rect, buf: &mut Buffer, label: &str, style: Style, theme: &Theme) {
    let area = area.intersection(buf.area);
    if area.is_empty() {
        return;
    }
    let label = text::display_safe(label);
    let label = text::truncate_words(&label, usize::from(area.width), theme.ascii());
    buf.set_line(area.x, area.y, &Line::styled(label, style), area.width);
}

fn clear(area: Rect, buf: &mut Buffer, ground: Role, theme: &Theme) {
    for y in area.y..area.bottom() {
        row(
            Rect::new(area.x, y, area.width, 1),
            buf,
            &" ".repeat(usize::from(area.width)),
            theme.fg(Role::Foreground).patch(theme.bg(ground)),
            theme,
        );
    }
}

fn columns(area: Rect, count: usize) -> Vec<Rect> {
    (0..count)
        .map(|index| {
            let left = u32::from(area.width) * index as u32 / count as u32;
            let right = u32::from(area.width) * (index as u32 + 1) / count as u32;
            Rect::new(
                area.x.saturating_add(left as u16),
                area.y,
                (right - left) as u16,
                area.height,
            )
        })
        .collect()
}

fn inks(area: Rect, buf: &mut Buffer, labels: &[(&str, Style)], theme: &Theme) {
    for (column, (label, style)) in columns(area, labels.len()).into_iter().zip(labels) {
        let inset = u16::from(column.width > 2);
        row(
            Rect {
                x: column.x.saturating_add(inset),
                width: column.width.saturating_sub(inset * 2),
                ..column
            },
            buf,
            label,
            *style,
            theme,
        );
    }
}

pub(crate) fn sample(palette: TuiPalette, area: Rect, buf: &mut Buffer, input: &Theme) {
    let area = area.intersection(buf.area);
    if area.is_empty() {
        return;
    }
    let theme = theme_for(palette, input);
    clear(area, buf, Role::Background, &theme);
    clear(band(area, 0, 2), buf, Role::Sidebar, &theme);
    row(
        band(area, 0, 1),
        buf,
        &format!("Codewhale / {}", palette.name()),
        theme.fg(Role::Foreground).add_modifier(Modifier::BOLD),
        &theme,
    );
    row(
        band(area, 1, 1),
        buf,
        if palette.light() {
            "Light theme"
        } else {
            "Dark theme"
        },
        theme.fg(Role::Muted),
        &theme,
    );

    row(
        band(area, 3, 1),
        buf,
        "Build with the colors already in Codewhale.",
        theme.fg(Role::Foreground),
        &theme,
    );
    for (column, (label, ground)) in columns(band(area, 5, 3), 5).into_iter().zip([
        ("Stage", Role::Background),
        ("Panel", Role::Surface),
        ("Raised", Role::Hover),
        ("Chrome", Role::Sidebar),
        ("Selected", Role::Selected),
    ]) {
        clear(column, buf, ground, &theme);
        let inset = u16::from(column.width > 2);
        row(
            Rect {
                x: column.x.saturating_add(inset),
                y: column.y.saturating_add(u16::from(column.height > 1)),
                width: column.width.saturating_sub(inset * 2),
                height: u16::from(column.height > 0),
            },
            buf,
            label,
            theme.fg(Role::Foreground).patch(theme.bg(ground)),
            &theme,
        );
    }
    inks(
        band(area, 10, 1),
        buf,
        &[
            ("Working", theme.tui_ink(TuiInk::Working)),
            ("Completed", theme.tui_ink(TuiInk::Success)),
            ("Needs you", theme.tui_ink(TuiInk::Warning)),
            ("Failed", theme.fg(Role::Danger)),
        ],
        &theme,
    );
    inks(
        band(area, 12, 1),
        buf,
        &[
            ("Work", theme.tui_ink(TuiInk::ModeWork)),
            ("Plan", theme.tui_ink(TuiInk::ModePlan)),
            ("Operate", theme.tui_ink(TuiInk::ModeOperate)),
        ],
        &theme,
    );
    inks(
        band(area, 14, 1),
        buf,
        &[
            ("Ask", theme.tui_ink(TuiInk::PermissionAsk)),
            ("Auto review", theme.tui_ink(TuiInk::PermissionAutoReview)),
            ("Full access", theme.tui_ink(TuiInk::PermissionFullAccess)),
        ],
        &theme,
    );
    row(
        band(area, 16, 1),
        buf,
        "Keep the work visible. Stay in control.",
        theme.tui_ink(TuiInk::Soft),
        &theme,
    );
    if area.height >= 22 {
        super::workbar::sample(WorkbarPanel::Tasks)
            .offset(1)
            .selected("graph:extract")
            .paint(band(area, area.height.saturating_sub(5), 5), buf, &theme);
    }
}

macro_rules! palette_draw {
    ($name:ident, $palette:ident) => {
        fn $name(area: Rect, buf: &mut Buffer, theme: &Theme) {
            sample(TuiPalette::$palette, area, buf, theme);
        }
    };
}
palette_draw!(underwater, Underwater);
palette_draw!(underwater_retro, UnderwaterRetro);
palette_draw!(shoreline, Shoreline);
palette_draw!(shoreline_light, ShorelineLight);
palette_draw!(whale, Whale);
palette_draw!(whale_light, WhaleLight);
palette_draw!(terminal, Terminal);
palette_draw!(grayscale, Grayscale);
palette_draw!(catppuccin_mocha, CatppuccinMocha);
palette_draw!(tokyo_night, TokyoNight);
palette_draw!(dracula, Dracula);
palette_draw!(gruvbox_dark, GruvboxDark);
palette_draw!(claude, Claude);
palette_draw!(matrix, Matrix);
palette_draw!(solarized_light, SolarizedLight);
palette_draw!(uwu, Uwu);

pub(crate) fn entries() -> Vec<Entry> {
    [
        (
            "tui-theme-underwater",
            underwater as fn(Rect, &mut Buffer, &Theme),
        ),
        ("tui-theme-underwater-retro", underwater_retro),
        ("tui-theme-shoreline", shoreline),
        ("tui-theme-shoreline-light", shoreline_light),
        ("tui-theme-whale", whale),
        ("tui-theme-whale-light", whale_light),
        ("tui-theme-terminal", terminal),
        ("tui-theme-grayscale", grayscale),
        ("tui-theme-catppuccin-mocha", catppuccin_mocha),
        ("tui-theme-tokyo-night", tokyo_night),
        ("tui-theme-dracula", dracula),
        ("tui-theme-gruvbox-dark", gruvbox_dark),
        ("tui-theme-claude", claude),
        ("tui-theme-matrix", matrix),
        ("tui-theme-solarized-light", solarized_light),
        ("tui-theme-uwu", uwu),
    ]
    .into_iter()
    .map(|(name, draw)| Entry {
        name,
        width: 104,
        height: 24,
        draw,
    })
    .collect()
}
