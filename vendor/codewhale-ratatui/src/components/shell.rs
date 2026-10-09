//! Portable slot geometry from Codewhale's `tui/ui/frame.rs`.
//! The conversation has no permanent header or side pane. The caller paints
//! pending input, composer, posture, workflow progress, metrics and work dock.
use crate::{Paint, Role, Theme};
use ratatui::{buffer::Buffer, layout::Rect};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ShellAreas {
    pub conversation: Rect,
    pub pending: Rect,
    pub composer: Rect,
    pub posture: Rect,
    pub workflows: Rect,
    pub metrics: Rect,
    pub workbar: Rect,
}

#[derive(Clone, Copy, Debug)]
pub struct TerminalShell {
    pub composer_rows: u16,
    pub pending_rows: u16,
    pub workflow_rows: u16,
    pub workbar_rows: u16,
    pub minimum_chat_rows: u16,
}
impl TerminalShell {
    pub const fn new(composer_rows: u16) -> Self {
        Self {
            composer_rows,
            pending_rows: 0,
            workflow_rows: 0,
            workbar_rows: 0,
            minimum_chat_rows: 3,
        }
    }
    pub const fn pending_rows(mut self, rows: u16) -> Self {
        self.pending_rows = rows;
        self
    }
    pub const fn workflow_rows(mut self, rows: u16) -> Self {
        self.workflow_rows = rows;
        self
    }
    pub const fn workbar_rows(mut self, rows: u16) -> Self {
        self.workbar_rows = rows;
        self
    }
    pub fn areas(&self, area: Rect) -> ShellAreas {
        let composer = self.composer_rows.min(area.height);
        let mut remaining = area.height.saturating_sub(composer);
        let posture = remaining.min(1);
        remaining = remaining.saturating_sub(posture);
        let metrics = remaining.min(1);
        remaining = remaining.saturating_sub(metrics);
        let chat = remaining.min(self.minimum_chat_rows);
        let mut auxiliary = remaining.saturating_sub(chat);
        let dock = self.workbar_rows.min(auxiliary);
        auxiliary = auxiliary.saturating_sub(dock);
        let pending = self.pending_rows.min(4).min(auxiliary);
        auxiliary = auxiliary.saturating_sub(pending);
        let workflows = self.workflow_rows.min(auxiliary);
        let conversation = remaining
            .saturating_sub(dock)
            .saturating_sub(pending)
            .saturating_sub(workflows);
        let mut y = area.y;
        let mut slot = |height: u16| {
            let rect = Rect::new(area.x, y, area.width, height);
            y = y.saturating_add(height);
            rect
        };
        ShellAreas {
            conversation: slot(conversation),
            pending: slot(pending),
            composer: slot(composer),
            posture: slot(posture),
            workflows: slot(workflows),
            metrics: slot(metrics),
            workbar: slot(dock),
        }
    }
}
impl Paint for TerminalShell {
    fn paint(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        buf.set_style(
            area.intersection(buf.area),
            theme.bg(Role::Background).patch(theme.fg(Role::Foreground)),
        );
    }
}
