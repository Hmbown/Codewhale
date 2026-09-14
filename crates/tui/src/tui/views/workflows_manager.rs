//! `/workflows` — the live workflow **run** dashboard.
//!
//! Lists every run this workspace's journal knows for the current session —
//! active and retained, not saved definitions — with the run's status, label,
//! elapsed time, child count, and latest progress. The detail pane adds the
//! phase order, the child-agent roster, the retained progress tail, and the
//! run id `/workflow cancel` accepts. `x` cancels the selected running run
//! through the same host path as `/workflow cancel` (no model turn, no
//! confirmation friction — matching the workflow panel's one-press cancel);
//! `r` re-reads the journal.
//!
//! This view never asks the model anything: it reads
//! [`crate::tools::workflow::host_workflow_run_details`] and writes only
//! through [`crate::tools::workflow::host_cancel_workflow`].

use std::cell::Cell;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph, Widget, Wrap},
};

use super::{ActionHint, ModalKind, ModalView, ViewAction, render_modal_footer};
use crate::tools::workflow::{
    HostWorkflowChildRow, HostWorkflowRunDetail, host_cancel_workflow, host_workflow_run_details,
};
use crate::tui::app::App;
use crate::tui::list_nav::wrap_index;
use codewhale_palette as palette;

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}

/// Status ink follows the workflow panel's grammar: running is working ink,
/// completion is outcome ink, only failure (including budget and replay
/// failures) spends red.
fn status_style(status: &str) -> Style {
    match status {
        "queued" | "running" | "waiting" | "pending" => {
            Style::default().fg(palette::STATUS_WARNING)
        }
        "completed" | "succeeded" => Style::default().fg(palette::STATUS_SUCCESS),
        "degraded" => Style::default().fg(palette::STATUS_WARNING),
        "failed" | "budget_exceeded" | "replay_diverged" => {
            Style::default().fg(palette::STATUS_ERROR)
        }
        _ => Style::default().fg(palette::TEXT_MUTED),
    }
}

fn child_state_glyph(state: &str) -> &'static str {
    match state {
        "running" | "pending" => "•",
        "succeeded" => "✓",
        _ => "✗",
    }
}

pub struct WorkflowsManagerView {
    /// Newest first, so the live run is the first thing read.
    runs: Vec<HostWorkflowRunDetail>,
    row: usize,
    detail_open: bool,
    detail_scroll: usize,
    /// Receipt line for the last host action (cancel), shown under the header.
    status: Option<String>,
    workspace: PathBuf,
    owner_session_id: Option<String>,
    /// The manager follows owner-state changes while open without polling the
    /// journal on every terminal frame.
    last_refresh_at: Instant,
    /// Screen rect of the run list body, recorded at render for mouse parity.
    list_body: Cell<Rect>,
}

impl WorkflowsManagerView {
    #[must_use]
    pub fn new(app: &App) -> Self {
        let mut view = Self {
            runs: Vec::new(),
            row: 0,
            detail_open: false,
            detail_scroll: 0,
            status: None,
            workspace: app.workspace.clone(),
            owner_session_id: app.current_session_id.clone(),
            last_refresh_at: Instant::now(),
            list_body: Cell::new(Rect::ZERO),
        };
        view.refresh();
        view
    }

    /// Re-read the journal (newest first), preserving the selected run id when
    /// a newer run arrives rather than silently moving focus to a different
    /// row.
    fn refresh(&mut self) {
        let selected_id = self.selected().map(|detail| detail.line.run_id.clone());
        self.runs = host_workflow_run_details(&self.workspace, self.owner_session_id.as_deref())
            .into_iter()
            .rev()
            .collect();
        self.row = selected_id
            .and_then(|run_id| {
                self.runs
                    .iter()
                    .position(|detail| detail.line.run_id == run_id)
            })
            .unwrap_or_else(|| self.row.min(self.runs.len().saturating_sub(1)));
        self.last_refresh_at = Instant::now();
    }

    fn selected(&self) -> Option<&HostWorkflowRunDetail> {
        self.runs.get(self.row)
    }

    fn move_row(&mut self, delta: isize) {
        let rows = self.runs.len();
        if rows == 0 {
            return;
        }
        self.row = wrap_index(self.row, rows, delta);
    }

    /// Cancel the selected run through the host path — the same one
    /// `/workflow cancel <run_id>` takes, so receipts and journal state are
    /// identical. No model turn.
    fn cancel_selected(&mut self) {
        let Some(detail) = self.selected() else {
            return;
        };
        if !detail.line.active {
            self.status = Some(format!(
                "Run {} already {} — nothing to cancel.",
                detail.line.run_id, detail.line.status
            ));
            return;
        }
        let run_id = detail.line.run_id.clone();
        match host_cancel_workflow(&self.workspace, &run_id, self.owner_session_id.as_deref()) {
            Ok(line) => {
                self.status = Some(format!(
                    "Workflow {} {} · {}",
                    line.run_id, line.status, line.label
                ));
            }
            Err(reason) => {
                self.status = Some(format!("Cancel failed: {reason}"));
            }
        }
        self.refresh();
    }

    fn footer_hints(&self) -> Vec<ActionHint> {
        let mut hints = vec![ActionHint::new("↑/↓", "move")];
        if self.detail_open {
            hints.push(ActionHint::new("←", "runs"));
        } else {
            hints.push(ActionHint::new("Enter", "detail"));
        }
        if self.selected().is_some_and(|d| d.line.active) {
            hints.push(ActionHint::new("x", "cancel"));
        }
        hints.push(ActionHint::new("r", "refresh"));
        hints.push(ActionHint::new("Esc", "close"));
        hints
    }

    fn header_lines(&self) -> Vec<Line<'static>> {
        let active = self.runs.iter().filter(|d| d.line.active).count();
        let finished = self.runs.len() - active;
        let mut header = vec![
            Line::from(vec![
                Span::styled(
                    "─ Workflow runs ",
                    Style::default().fg(palette::WHALE_ACTION).bold(),
                ),
                Span::styled(
                    format!("· {active} active · {finished} finished"),
                    Style::default().fg(palette::TEXT_MUTED),
                ),
            ]),
            Line::from(""),
        ];
        if let Some(status) = &self.status {
            header.push(Line::from(Span::styled(
                format!("  {status}"),
                Style::default().fg(palette::WHALE_HUMAN),
            )));
        }
        header
    }

    fn render_list(&self, area: Rect, buf: &mut Buffer) {
        self.list_body.set(area);
        if self.runs.is_empty() {
            Paragraph::new(Line::from(vec![
                Span::styled(
                    "  No workflow runs in this workspace yet.",
                    Style::default().fg(palette::TEXT_MUTED),
                ),
                Span::styled(
                    "  /workflow <objective> starts one.",
                    Style::default().fg(palette::TEXT_DIM),
                ),
            ]))
            .render(area, buf);
            return;
        }

        let now = now_ms();
        let rows_visible = usize::from(area.height).max(1);
        let scroll = self.row.saturating_sub(rows_visible.saturating_sub(1));
        for (idx, detail) in self.runs.iter().enumerate() {
            if idx < scroll || idx >= scroll + rows_visible {
                continue;
            }
            let y = area.y + u16::try_from(idx - scroll).unwrap_or(u16::MAX);
            if y >= area.y + area.height {
                break;
            }
            let selected = idx == self.row;
            let row_rect = Rect {
                x: area.x,
                y,
                width: area.width,
                height: 1,
            };
            let elapsed = detail
                .line
                .completed_at_ms
                .unwrap_or(now)
                .saturating_sub(detail.line.started_at_ms)
                / 1000;
            let marker = if selected { "▸ " } else { "  " };
            let base = if selected {
                Style::default().fg(palette::WHALE_ACTION).bold()
            } else {
                Style::default().fg(palette::TEXT_SECONDARY)
            };
            let mut spans = vec![
                Span::styled(marker, base),
                Span::styled(
                    format!("{:<9}", detail.line.status),
                    status_style(detail.line.status).add_modifier(if selected {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
                ),
                Span::styled(detail.line.label.clone(), base),
                Span::styled(
                    format!(
                        "  ·  {}  ·  {} children",
                        crate::elapsed::format_elapsed_secs(elapsed),
                        detail.line.child_count
                    ),
                    Style::default().fg(palette::TEXT_DIM),
                ),
            ];
            if let Some(progress) = detail.line.last_progress.as_deref() {
                spans.push(Span::styled(
                    format!("  ·  {progress}"),
                    Style::default().fg(palette::TEXT_DIM),
                ));
            }
            Line::from(spans).render(row_rect, buf);
        }
    }

    fn detail_lines(&self, detail: &HostWorkflowRunDetail) -> Vec<Line<'static>> {
        let now = now_ms();
        let elapsed = detail
            .line
            .completed_at_ms
            .unwrap_or(now)
            .saturating_sub(detail.line.started_at_ms)
            / 1000;
        let mut lines = vec![
            Line::from(vec![
                Span::styled("─ ", Style::default().fg(palette::WHALE_ACTION).bold()),
                Span::styled(
                    detail.line.label.clone(),
                    Style::default().fg(palette::TEXT_PRIMARY).bold(),
                ),
                Span::styled(
                    format!("  ·  {}", detail.line.status),
                    status_style(detail.line.status),
                ),
            ]),
            Line::from(Span::styled(
                format!(
                    "  run {}  ·  {}  ·  {} children",
                    detail.line.run_id,
                    crate::elapsed::format_elapsed_secs(elapsed),
                    detail.line.child_count
                ),
                Style::default().fg(palette::TEXT_DIM),
            )),
            Line::from(""),
        ];
        if !detail.phases.is_empty() {
            lines.push(Line::from(Span::styled(
                "  Phases",
                Style::default().fg(palette::TEXT_PRIMARY).bold(),
            )));
            lines.push(Line::from(Span::styled(
                format!("  {}", detail.phases.join(" → ")),
                Style::default().fg(palette::TEXT_SECONDARY),
            )));
            lines.push(Line::from(""));
        }
        if !detail.children.is_empty() {
            lines.push(Line::from(Span::styled(
                format!("  Children ({})", detail.children.len()),
                Style::default().fg(palette::TEXT_PRIMARY).bold(),
            )));
            for child in &detail.children {
                lines.push(child_row_line(child));
            }
            lines.push(Line::from(""));
        }
        if !detail.progress_tail.is_empty() {
            lines.push(Line::from(Span::styled(
                "  Recent progress",
                Style::default().fg(palette::TEXT_PRIMARY).bold(),
            )));
            for progress in &detail.progress_tail {
                lines.push(Line::from(Span::styled(
                    format!("  {progress}"),
                    Style::default().fg(palette::TEXT_SECONDARY),
                )));
            }
            lines.push(Line::from(""));
        }
        if let Some(error) = detail.line.error.as_deref() {
            lines.push(Line::from(vec![
                Span::styled("  Error  ", Style::default().fg(palette::STATUS_ERROR)),
                Span::styled(
                    error.to_string(),
                    Style::default().fg(palette::TEXT_SECONDARY),
                ),
            ]));
        }
        lines.push(Line::from(Span::styled(
            if detail.has_result {
                "  Result retained in the run journal (.codewhale/workflow-runs.jsonl)."
            } else {
                "  No result recorded yet."
            },
            Style::default().fg(palette::TEXT_DIM),
        )));
        lines
    }

    fn render_detail(&self, area: Rect, buf: &mut Buffer) {
        let Some(detail) = self.selected() else {
            self.render_list(area, buf);
            return;
        };
        let lines = self.detail_lines(detail);
        let visible = usize::from(area.height).max(1);
        let max_scroll = lines.len().saturating_sub(visible);
        let scroll = self.detail_scroll.min(max_scroll);
        Paragraph::new(lines.iter().skip(scroll).cloned().collect::<Vec<_>>())
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }
}

fn child_row_line(child: &HostWorkflowChildRow) -> Line<'static> {
    let name = child.label.clone().unwrap_or_else(|| child.task_id.clone());
    let mut spans = vec![
        Span::styled(
            format!("    {} ", child_state_glyph(child.state)),
            status_style(child.state),
        ),
        Span::styled(name, Style::default().fg(palette::TEXT_SECONDARY)),
        Span::styled(format!(" · {}", child.state), status_style(child.state)),
    ];
    let mut meta = Vec::new();
    if let Some(role) = child.role.as_deref() {
        meta.push(role.to_string());
    }
    if let Some(model) = child.model.as_deref() {
        meta.push(model.to_string());
    }
    if let Some(phase) = child.phase.as_deref() {
        meta.push(phase.to_string());
    }
    if !meta.is_empty() {
        spans.push(Span::styled(
            format!("  ·  {}", meta.join(" · ")),
            Style::default().fg(palette::TEXT_DIM),
        ));
    }
    Line::from(spans)
}

impl ModalView for WorkflowsManagerView {
    fn kind(&self) -> ModalKind {
        ModalKind::WorkflowsManager
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn handle_key(&mut self, key: KeyEvent) -> ViewAction {
        match key.code {
            KeyCode::Esc => {
                if self.detail_open {
                    self.detail_open = false;
                    self.detail_scroll = 0;
                    ViewAction::None
                } else {
                    ViewAction::Close
                }
            }
            KeyCode::Char('q') => ViewAction::Close,
            KeyCode::Up | KeyCode::Char('k') => {
                if self.detail_open {
                    self.detail_scroll = self.detail_scroll.saturating_sub(1);
                } else {
                    self.move_row(-1);
                }
                ViewAction::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.detail_open {
                    // The render path clamps to the last scrollable line.
                    self.detail_scroll = self.detail_scroll.saturating_add(1);
                } else {
                    self.move_row(1);
                }
                ViewAction::None
            }
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
                if !self.runs.is_empty() {
                    self.detail_open = true;
                }
                ViewAction::None
            }
            KeyCode::Left | KeyCode::Tab => {
                self.detail_open = false;
                self.detail_scroll = 0;
                ViewAction::None
            }
            KeyCode::Char('x') | KeyCode::Char('c') => {
                self.cancel_selected();
                ViewAction::None
            }
            KeyCode::Char('r') => {
                self.refresh();
                ViewAction::None
            }
            KeyCode::Char('g') => ViewAction::Close,
            _ => ViewAction::None,
        }
    }

    fn handle_mouse(&mut self, mouse: MouseEvent) -> ViewAction {
        if self.detail_open {
            return ViewAction::None;
        }
        // The wheel moves this list, not the transcript behind it.
        match mouse.kind {
            MouseEventKind::ScrollUp => {
                self.move_row(-1);
                return ViewAction::None;
            }
            MouseEventKind::ScrollDown => {
                self.move_row(1);
                return ViewAction::None;
            }
            _ => {}
        }
        if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
            let body = self.list_body.get();
            if body.width > 0
                && mouse.row >= body.y
                && mouse.row < body.y + body.height
                && mouse.column >= body.x
                && mouse.column < body.x + body.width
            {
                let offset = usize::from(mouse.row - body.y);
                let scroll = self
                    .row
                    .saturating_sub(usize::from(body.height).saturating_sub(1));
                let idx = scroll + offset;
                if idx < self.runs.len() {
                    self.row = idx;
                }
            }
        }
        ViewAction::None
    }

    fn tick(&mut self) -> ViewAction {
        let interval = if self.runs.iter().any(|detail| detail.line.active) {
            Duration::from_millis(250)
        } else {
            Duration::from_secs(2)
        };
        if self.last_refresh_at.elapsed() >= interval {
            self.refresh();
        }
        ViewAction::None
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        Block::default()
            .style(Style::default().bg(palette::WHALE_BG))
            .render(area, buf);

        let hints = self.footer_hints();
        let content = render_modal_footer(area, buf, &hints);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(4), Constraint::Min(1)])
            .split(content);

        Paragraph::new(self.header_lines())
            .wrap(Wrap { trim: false })
            .render(chunks[0], buf);

        if self.detail_open {
            self.render_detail(chunks[1], buf);
        } else {
            self.render_list(chunks[1], buf);
        }
    }
}
