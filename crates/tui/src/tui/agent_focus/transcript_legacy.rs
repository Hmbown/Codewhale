//! Frozen focused-child transcript painter; exact base function bytes.
use super::*;
use ratatui::widgets::{Paragraph, Widget};

pub(crate) fn render_focus(app: &mut App, area: Rect, buf: &mut Buffer) {
    let Some(focus) = app.agent_focus.as_ref() else {
        return;
    };
    let theme = app.ui_theme;
    let background = Style::default().bg(theme.surface_bg);
    buf.set_style(area, background);
    if area.height == 0 || area.width == 0 {
        return;
    }
    let (status_glyph, status_word) = focused_status(app).unwrap_or(('○', "unknown".to_string()));
    let banner = app
        .tr(MessageId::AgentFocusBanner)
        .replace("{agent}", &focus.label)
        .replace("{status}", &status_word);
    let mut banner_spans = vec![
        Span::styled(
            format!("{status_glyph} "),
            Style::default().fg(theme.accent_action),
        ),
        Span::styled(
            banner,
            Style::default()
                .fg(theme.accent_action)
                .add_modifier(Modifier::BOLD),
        ),
    ];
    // The worker's effective posture, in the same dot chain: what it may do
    // is stated where its conversation is read, not hidden in a role name.
    if let Some(posture) = focused_posture(app) {
        banner_spans.push(Span::styled(
            format!(" · {posture}"),
            Style::default().fg(theme.text_muted),
        ));
    }
    let banner_line = Line::from(banner_spans);
    let width = area.width.max(1);
    let mut lines: Vec<Line<'static>> = Vec::new();
    if focus.omitted_messages > 0 {
        lines.push(Line::from(Span::styled(
            app.tr(MessageId::AgentFocusOmitted)
                .replace("{count}", &focus.omitted_messages.to_string()),
            Style::default().fg(theme.text_muted),
        )));
    }
    let result_cells = result_cells(app, focus.result.as_ref());
    if focus.cells.is_empty() && result_cells.is_empty() && focus.local_cells.is_empty() {
        lines.push(Line::from(Span::styled(
            app.tr(MessageId::AgentFocusNoTranscript)
                .replace("{agent}", &focus.label),
            Style::default().fg(theme.text_muted),
        )));
    }
    for cell in focus
        .cells
        .iter()
        .chain(result_cells.iter())
        .chain(focus.local_cells.iter())
    {
        lines.extend(cell.transcript_lines(width));
        lines.push(Line::default());
    }
    let visible = usize::from(area.height.saturating_sub(1)).max(1);
    let total = lines.len();
    let max_top = total.saturating_sub(visible);
    let delta = app.viewport.pending_scroll_delta;
    app.viewport.pending_scroll_delta = 0;
    let Some(focus) = app.agent_focus.as_mut() else {
        return;
    };
    let current = focus.scroll_top.unwrap_or(max_top);
    let next = if delta < 0 {
        current.saturating_sub(delta.unsigned_abs() as usize)
    } else {
        current.saturating_add(delta as usize)
    }
    .min(max_top);
    focus.scroll_top = if next >= max_top { None } else { Some(next) };
    focus.last_visible = visible;
    focus.last_total = total;
    let top = focus.scroll_top.unwrap_or(max_top);

    let banner_area = Rect::new(area.x, area.y, area.width, 1);
    Paragraph::new(banner_line)
        .style(background)
        .render(banner_area, buf);
    let body_area = Rect::new(
        area.x,
        area.y.saturating_add(1),
        area.width,
        area.height.saturating_sub(1),
    );
    let shown: Vec<Line<'static>> = lines.into_iter().skip(top).take(visible).collect();
    Paragraph::new(shown)
        .style(background)
        .render(body_area, buf);
    // The focused view owns the transcript geometry for paging keys.
    app.viewport.last_transcript_area = Some(body_area);
    app.viewport.last_transcript_visible = visible;
    app.viewport.last_transcript_total = total;
    app.viewport.last_transcript_top = top;
}

// Frozen source boundary.
