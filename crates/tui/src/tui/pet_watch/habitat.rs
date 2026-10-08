//! Full viewport ownership uses the existing modal stack. Hidden composer,
//! history, selection and active Engine state are untouched.
use super::Control;
use crate::tui::{
    shell_key_routing::{self, Focus, ShellBindingId as Id},
    views::{ModalKind, ModalView, ViewAction, ViewEvent},
};
use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::{buffer::Buffer, layout::Rect};
use std::sync::{Arc, Mutex};
pub struct Habitat {
    controls: Arc<Mutex<Vec<Control>>>,
    selection: Arc<Mutex<Option<String>>>,
}
impl Habitat {
    pub fn new(controls: Arc<Mutex<Vec<Control>>>, selection: Arc<Mutex<Option<String>>>) -> Self {
        Self {
            controls,
            selection,
        }
    }
}
impl ModalView for Habitat {
    fn kind(&self) -> ModalKind {
        ModalKind::PetHabitat
    }
    fn handle_key(&mut self, key: KeyEvent) -> ViewAction {
        let action = match shell_key_routing::route(Focus::Modal(self.kind()), &key) {
            Some(Id::PetResultUp) => Some(Control::Scroll(-1)),
            Some(Id::PetResultDown) => Some(Control::Scroll(1)),
            Some(Id::PetResultPageUp) => Some(Control::Scroll(-10)),
            Some(Id::PetResultPageDown) => Some(Control::Scroll(10)),
            Some(Id::PetBack) => return ViewAction::Close,
            Some(Id::PetFocusAgents) => Some(Control::FocusAgents),
            Some(Id::PetResultStart) => Some(Control::ScrollEnd(false)),
            Some(Id::PetResultEnd) => Some(Control::ScrollEnd(true)),
            Some(Id::PetOpenAgent) => {
                if let Some(agent_id) = self.selection.lock().ok().and_then(|s| s.clone()) {
                    return ViewAction::EmitAndClose(ViewEvent::OpenAgentTranscript { agent_id });
                }
                None
            }
            Some(Id::PetSound) => Some(Control::Sound),
            Some(Id::PetBrowser) => Some(Control::Browser),
            Some(Id::PetWindow) => Some(Control::Window),
            _ => None,
        };
        if let Some(action) = action
            && let Ok(mut queue) = self.controls.lock()
            && queue.len() < 16
        {
            queue.push(action);
        }
        ViewAction::None
    }
    fn handle_mouse(&mut self, mouse: MouseEvent) -> ViewAction {
        if let Ok(mut queue) = self.controls.lock()
            && queue.len() < 16
        {
            queue.push(Control::Mouse(mouse));
        }
        ViewAction::None
    }
    fn render(&self, _area: Rect, _buf: &mut Buffer) {}
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
pub fn hints(locale: codewhale_localization::Locale) -> String {
    use codewhale_localization::{MessageId, tr};
    let mut text = tr(locale, MessageId::PetHabitatHints).into_owned();
    for (name, id) in [
        ("back", Id::PetBack),
        ("sound", Id::PetSound),
        ("browser", Id::PetBrowser),
        ("window", Id::PetWindow),
    ] {
        text = text.replace(
            &format!("{{{name}}}"),
            shell_key_routing::binding(id).footer_chord,
        );
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyModifiers};
    #[test]
    fn enter_opens_only_the_painted_worker_through_the_existing_event_and_closes_pet_focus() {
        let controls = Arc::new(Mutex::new(Vec::new()));
        let selection = Arc::new(Mutex::new(None));
        let mut view = Habitat::new(controls, selection.clone());
        let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert!(matches!(view.handle_key(enter), ViewAction::None));
        *selection.lock().unwrap() = Some("worker-id".into());
        assert!(
            matches!(view.handle_key(enter), ViewAction::EmitAndClose(ViewEvent::OpenAgentTranscript { agent_id }) if agent_id == "worker-id")
        );
    }
    #[test]
    fn pet_habitat_full_view_preserves_composer_history_and_session_on_escape() {
        let mut app =
            crate::test_support::test_app_with_options(crate::test_support::test_tui_options("."));
        app.input = "unfinished composer".into();
        app.add_message(crate::tui::history::HistoryCell::System {
            content: "retained transcript".into(),
        });
        let history = app.history.len();
        let session = app.current_session_id.clone();
        // Push the actual focus owner without starting a network client.
        let controls = Arc::new(Mutex::new(Vec::new()));
        app.view_stack
            .push(Habitat::new(controls.clone(), Arc::new(Mutex::new(None))));
        assert_eq!(app.focus(), Focus::Modal(ModalKind::PetHabitat));
        assert_eq!(
            shell_key_routing::route(
                Focus::Composer,
                &KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE)
            ),
            None
        );
        app.view_stack
            .handle_key(KeyEvent::new(KeyCode::F(6), KeyModifiers::NONE));
        assert!(matches!(
            controls.lock().unwrap().as_slice(),
            [Control::Sound]
        ));
        app.view_stack
            .handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
        app.view_stack
            .handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(app.view_stack.is_empty());
        assert_eq!(app.input, "unfinished composer");
        assert_eq!(app.history.len(), history);
        assert_eq!(app.current_session_id, session);
    }
}
