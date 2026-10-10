use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Padding, Paragraph, Widget, Wrap};

use crate::tui::views::{
    ModalKind, ModalView, ViewAction, ViewEvent, centered_modal_area, render_modal_surface,
};
use codewhale_localization::{Locale, MessageId, tr};
use codewhale_palette as palette;

pub(crate) use crate::core::authority::FullAccessScope;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FullAccessOrigin {
    Cycle,
    RootCycle,
    Yolo,
    Config,
    RootConfig,
    Setup,
    Migration,
}

pub struct FullAccessConfirmView {
    origin: FullAccessOrigin,
    locale: Locale,
}

impl FullAccessConfirmView {
    #[must_use]
    pub fn new(origin: FullAccessOrigin, locale: Locale) -> Self {
        Self { origin, locale }
    }

    fn migration(&self) -> bool {
        self.origin == FullAccessOrigin::Migration
    }

    fn confirm(&self, scope: FullAccessScope) -> ViewAction {
        ViewAction::EmitAndClose(ViewEvent::FullAccessConfirmed {
            scope,
            origin: self.origin,
        })
    }
}

impl ModalView for FullAccessConfirmView {
    fn kind(&self) -> ModalKind {
        ModalKind::FullAccessConfirm
    }

    fn handle_key(&mut self, key: KeyEvent) -> ViewAction {
        if key.kind != KeyEventKind::Press {
            return ViewAction::None;
        }
        if key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
        {
            return ViewAction::None;
        }
        match key.code {
            KeyCode::Char('r') | KeyCode::Char('R') => self.confirm(FullAccessScope::Repo),
            KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
                self.confirm(FullAccessScope::Session)
            }
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') if self.migration() => {
                self.confirm(FullAccessScope::Session)
            }
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => ViewAction::Close,
            _ => ViewAction::None,
        }
    }

    fn occupied_region(&self, area: Rect) -> Rect {
        centered_modal_area(area, 72, 13, 36, 10)
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        let popup = self.occupied_region(area);
        render_modal_surface(area, popup, buf);

        let (title_id, body_id, keys_id) = if self.migration() {
            (
                MessageId::FullAccessMigrationTitle,
                MessageId::FullAccessMigrationBody,
                MessageId::FullAccessMigrationKeys,
            )
        } else {
            (
                MessageId::FullAccessConfirmTitle,
                MessageId::FullAccessConfirmRisk,
                MessageId::FullAccessConfirmKeys,
            )
        };
        let block = Block::default()
            .title(Line::from(Span::styled(
                tr(self.locale, title_id).to_string(),
                Style::default().fg(palette::STATUS_WARNING).bold(),
            )))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette::STATUS_WARNING))
            .style(Style::default().bg(palette::WHALE_BG))
            .padding(Padding::uniform(1));
        let inner = block.inner(popup);
        block.render(popup, buf);

        let controls_height = if self.migration() { 1 } else { 4 };
        let body_height = inner.height.saturating_sub(controls_height + 1);
        Paragraph::new(tr(self.locale, body_id).to_string())
            .style(Style::default().fg(palette::STATUS_WARNING))
            .wrap(Wrap { trim: true })
            .render(Rect::new(inner.x, inner.y, inner.width, body_height), buf);

        let mut y = inner.bottom().saturating_sub(controls_height);
        if !self.migration() {
            for (id, key) in [
                (MessageId::FullAccessConfirmSession, "Enter"),
                (MessageId::FullAccessConfirmRepo, "R"),
                (MessageId::FullAccessConfirmCancel, "Esc"),
            ] {
                Paragraph::new(format!("{key}  {}", tr(self.locale, id)))
                    .style(Style::default().fg(palette::TEXT_PRIMARY))
                    .render(Rect::new(inner.x, y, inner.width, 1), buf);
                y += 1;
            }
        }
        Paragraph::new(tr(self.locale, keys_id).to_string())
            .style(Style::default().fg(palette::TEXT_MUTED))
            .render(Rect::new(inner.x, y, inner.width, 1), buf);
    }
}
