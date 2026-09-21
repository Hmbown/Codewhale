//! `/home` command.

use crate::commands::traits::{CommandInfo, RegisterCommand};
use crate::tui::app::App;
use codewhale_localization::MessageId;

use super::CommandResult;

pub(in crate::commands) const COMMAND_INFO: CommandInfo = CommandInfo {
    name: "home",
    aliases: &["zhuye", "shouye"],
    usage: "/home",
    description_id: MessageId::CmdHomeDescription,
};

pub(in crate::commands) struct HomeCmd;

impl RegisterCommand for HomeCmd {
    fn info() -> &'static CommandInfo {
        &COMMAND_INFO
    }

    fn execute(app: &mut App, _arg: Option<&str>) -> CommandResult {
        if app.session_transition_blocked() {
            return CommandResult::error(app.tr(MessageId::HomeNavigationBusy).into_owned());
        }
        app.launch.workspace = app.workspace.clone();
        app.launch.restore_card();
        app.launch.visible = true;
        app.launch.return_to_session = true;
        app.needs_redraw = true;
        CommandResult::ok()
    }
}

/// Preserve the old read-only statistics view under its descriptive aliases.
pub(in crate::commands) struct OverviewCmd;

impl RegisterCommand for OverviewCmd {
    fn info() -> &'static CommandInfo {
        &CommandInfo {
            name: "overview",
            aliases: &["stats"],
            usage: "/overview",
            description_id: MessageId::CmdOverviewDescription,
        }
    }

    fn execute(app: &mut App, _arg: Option<&str>) -> CommandResult {
        super::core::home_dashboard(app)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn home_preserves_session_draft_and_reveal_and_escape_returns() {
        let mut app =
            crate::test_support::test_app_with_options(crate::test_support::test_tui_options("."));
        app.launch.dismiss();
        app.current_session_id = Some("retained-session".into());
        app.api_messages_mut().push(codewhale_models::Message {
            role: codewhale_models::Role::User,
            content: vec![codewhale_models::ContentBlock::Text {
                text: "Retain this conversation".into(),
                cache_control: None,
            }],
        });
        app.add_message(crate::tui::history::HistoryCell::System {
            content: "Retain this transcript".into(),
        });
        app.input = "unsent draft".into();
        app.cursor_position = app.input.chars().count();
        app.launch.mark_reveal_started_at = Some(std::time::Instant::now());
        let reveal = app.launch.mark_reveal_started_at;
        let messages = app.api_messages.clone();
        let history_len = app.history.len();
        let result = crate::commands::execute("/home", &mut app);
        assert!(!result.is_error, "{:?}", result.message);
        assert!(
            result.action.is_none(),
            "navigation must not sync/reset the Engine"
        );
        assert!(crate::tui::widgets::should_render_empty_state(&app));
        assert!(app.launch.return_to_session);
        assert!(!crate::tui::underwater::launch_motion_active(
            &app, false, true
        ));
        crate::tui::underwater::refresh_launch_row_hitboxes(
            &mut app,
            ratatui::layout::Rect::new(0, 0, 80, 20),
        );
        let rows = crate::tui::underwater::launch_rows_for_app(&app);
        assert_eq!(
            crate::tui::underwater::launch_row_click_action(&rows[0].id),
            crate::tui::underwater::LaunchAction::ReturnToSession,
        );
        app.launch.menu_selected = Some(0);
        assert_eq!(
            crate::tui::underwater::handle_launch_composer_key(
                &mut app,
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            ),
            crate::tui::underwater::LaunchComposerKey::MenuRun,
        );
        assert_eq!(app.input, "unsent draft");
        crate::tui::underwater::handle_launch_composer_key(
            &mut app,
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
        );
        assert!(!app.launch.visible);
        assert!(!app.launch.return_to_session);
        assert_eq!(app.current_session_id.as_deref(), Some("retained-session"));
        assert_eq!(app.api_messages, messages);
        assert_eq!(app.history.len(), history_len);
        assert_eq!(app.input, "unsent draft");
        assert_eq!(app.launch.mark_reveal_started_at, reveal);
        assert!(!crate::tui::widgets::should_render_empty_state(&app));

        for command in ["/overview", "/stats"] {
            let result = crate::commands::execute(command, &mut app);
            assert!(!result.is_error);
            assert!(result.message.unwrap().contains("Quick Actions"));
            assert!(!app.launch.visible);
        }
    }

    #[test]
    fn home_refuses_to_cover_active_work() {
        let mut app =
            crate::test_support::test_app_with_options(crate::test_support::test_tui_options("."));
        app.launch.dismiss();
        app.is_loading = true;
        let result = HomeCmd::execute(&mut app, None);
        assert!(result.is_error);
        assert!(!app.launch.visible);
    }
}
