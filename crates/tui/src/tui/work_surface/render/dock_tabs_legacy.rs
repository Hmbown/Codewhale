//! Frozen current owning renderer, used only for adoption acceptance.
use super::*;

#[derive(Debug, Clone)]
struct DockTab {
    target: DockTabTarget,
    label: std::borrow::Cow<'static, str>,
    count: usize,
}

pub(super) fn render_dock_tabs(frame: &mut Frame, area: Rect, app: &mut App) {
    let width = usize::from(area.width);
    let mut entries = Vec::new();
    for panel in RailPanel::ORDER {
        let count = dock_tab_count(app, panel);
        let useful = count.is_some_and(|count| count > 0)
            || super::super::views::view_always_has_content(panel);
        if useful || panel == app.work_surface.panel {
            entries.push(DockTab {
                target: DockTabTarget::Panel(panel),
                label: match panel {
                    RailPanel::Tasks => "Tasks",
                    RailPanel::Agents => "Fleet",
                    RailPanel::Background => "Jobs",
                    RailPanel::Files => "Files",
                    RailPanel::Notepad => "Notes",
                    RailPanel::Context => "Context",
                    RailPanel::Git => "Git",
                    RailPanel::Price => "Cost",
                }
                .into(),
                count: count.unwrap_or(0),
            });
        }
    }

    let close_mark = if crate::tui::color_compat::ascii_safe_enabled() {
        "x"
    } else {
        "×"
    };
    // #6502: name Esc beside the close control only while Esc really closes
    // the dock — the dock owns keyboard focus and has something to close
    // (`input::handle_key`). Otherwise Esc belongs to the composer and stops
    // the running turn, which the posture bar already says with the turn
    // status; a bare `×` here keeps the two from reading as one shortcut.
    let esc_closes = app.work_surface.focused
        && !super::super::interaction::opened_detail_on_screen(app)
        && (app.work_surface.explicit_view || !visible_rows_for_panel(app).is_empty());
    let close = if esc_closes && area.width >= 60 {
        format!(" Esc {close_mark} ")
    } else {
        format!(" {close_mark} ")
    };
    let close_width = close.width().min(width);
    let mut show_counts = true;
    let fits = |tabs: &[DockTab], counts: bool| {
        tabs.iter()
            .map(|tab| {
                UnicodeWidthStr::width(tab.label.as_ref())
                    + if counts && tab.count > 0 {
                        1 + tab.count.to_string().len()
                    } else {
                        0
                    }
                    + 2
            })
            .sum::<usize>()
            .saturating_add(tabs.len().saturating_sub(1).saturating_mul(2))
            .saturating_add(close_width + 2)
            <= width
    };
    if !fits(&entries, true) {
        show_counts = false;
    }
    // Shed from the right (price, git, context, notepad, files… in reverse
    // cycle order), never the active tab: a narrow dock keeps the work views.
    while !fits(&entries, show_counts) && entries.len() > 1 {
        let remove = entries
            .iter()
            .rposition(|tab| tab.target != DockTabTarget::Panel(app.work_surface.panel));
        let Some(index) = remove else { break };
        entries.remove(index);
    }

    let tab_y = if app.work_surface.effective_placement == WorkSurfacePlacement::Bottom {
        area.y
            .saturating_add(1)
            .min(area.bottom().saturating_sub(1))
    } else {
        area.y
    };
    let tab_area = Rect {
        x: area.x,
        y: tab_y,
        width: area.width,
        height: 1,
    };
    let close_area = Rect {
        x: tab_area.right().saturating_sub(close_width as u16),
        y: tab_y,
        width: close_width as u16,
        height: 1,
    };
    app.work_surface.dock_tabs.clear();
    for tab in &entries {
        let label = if show_counts && tab.count > 0 {
            format!("{} {}", tab.label, tab.count)
        } else {
            tab.label.to_string()
        };
        let tab_width = u16::try_from(UnicodeWidthStr::width(label.as_str()).saturating_add(2))
            .unwrap_or(u16::MAX)
            .min(tab_area.width);
        let x = tab_area.x.saturating_add(
            app.work_surface
                .dock_tabs
                .last()
                .map(|hitbox| hitbox.area.right().saturating_sub(tab_area.x) + 2)
                .unwrap_or(1),
        );
        if x.saturating_add(tab_width) > close_area.x {
            break;
        }
        let hitbox = Rect {
            x,
            y: tab_y,
            width: tab_width,
            height: 1,
        };
        let active = tab.target == DockTabTarget::Panel(app.work_surface.panel);
        let pressed = app.work_surface.pressed_tab == Some(tab.target);
        let hovered = app.work_surface.hovered_tab == Some(tab.target);
        let style = if active || pressed {
            Style::default()
                .fg(app.ui_theme.text_body)
                .bg(app.ui_theme.selection_bg)
                .add_modifier(Modifier::BOLD)
        } else if hovered {
            Style::default()
                .fg(app.ui_theme.text_body)
                .bg(app.ui_theme.elevated_bg)
                .add_modifier(Modifier::UNDERLINED)
        } else {
            chrome_style(&app.ui_theme, ChromeInk::Metadata)
        };
        Paragraph::new(Line::from(Span::styled(format!(" {label} "), style)))
            .render(hitbox, frame.buffer_mut());
        app.work_surface.dock_tabs.push(DockTabHitbox {
            target: tab.target,
            area: hitbox,
        });
    }
    let close_style = if app.work_surface.hovered_tab == Some(DockTabTarget::Close) {
        chrome_style(&app.ui_theme, ChromeInk::Info)
            .bg(app.ui_theme.elevated_bg)
            .add_modifier(Modifier::UNDERLINED)
    } else {
        chrome_style(&app.ui_theme, ChromeInk::MetadataHint)
    };
    Paragraph::new(Line::from(Span::styled(close, close_style)))
        .render(close_area, frame.buffer_mut());
    app.work_surface.dock_tabs.push(DockTabHitbox {
        target: DockTabTarget::Close,
        area: close_area,
    });
}
