//! `/title` command — portable handler over the session-control facet.
//!
//! Distinct from `/rename`: it sets the session *window/tab* title. The
//! handler owns the bare-report branch, the `off|clear|none` synonyms, and
//! the 100-character limit on the raw argument, sanitization, and exact
//! messages; the atomic set/clear delegates own persistence, publication,
//! and the redraw flag.

use super::CommandResult;
use codewhale_command_contract::facets::{CommandSessionControlContext, TitleSource};
use codewhale_command_contract::handler::{CommandContexts, CommandHandler};
use codewhale_command_contract::metadata::{
    CommandInfo as ContractInfo, RegisterCommand as ContractRegisterCommand,
};

pub(in crate::commands) struct TitleCmd;

pub(in crate::commands) const CONTRACT_INFO: ContractInfo = ContractInfo {
    name: "title",
    aliases: &["tabtitle", "window-title"],
    usage: "/title [new title|off]",
    description_key: "cmd_title_description",
};

impl ContractRegisterCommand<CommandResult> for TitleCmd {
    fn info() -> &'static ContractInfo {
        &CONTRACT_INFO
    }
    fn handler() -> CommandHandler<CommandResult> {
        CommandHandler::Contextual {
            capabilities: codewhale_command_contract::handler::CommandCapabilities::SESSION_CONTROL,
            handler: title_contextual,
        }
    }
}

pub(in crate::commands) fn title_contextual(
    contexts: CommandContexts<'_>,
    arg: Option<&str>,
) -> CommandResult {
    let mut parts = contexts.into_parts();
    let Some(control) = parts.control.as_deref_mut() else {
        return CommandResult::error("Command capability unavailable: session_control".to_string());
    };
    title_portable(control, arg)
}

pub(in crate::commands) fn title_portable(
    control: &mut dyn CommandSessionControlContext,
    arg: Option<&str>,
) -> CommandResult {
    let trimmed = arg.map(str::trim).filter(|s| !s.is_empty());
    let Some(arg) = trimmed else {
        let report = control.title_report();
        let source = match report.source {
            TitleSource::Session => " (session)",
            TitleSource::ConfigDefault => " (config default)",
            TitleSource::None => "",
        };
        return CommandResult::message(format!("Window title: [{}]{source}", report.effective));
    };

    if arg == "off" || arg == "clear" || arg == "none" {
        return match control.clear_window_title() {
            Ok(()) => CommandResult::message(
                "Window title cleared (the config default still applies if set)",
            ),
            Err(error) => CommandResult::error(error),
        };
    }

    if arg.chars().count() > super::MAX_TITLE_LEN {
        return CommandResult::error(format!(
            "Title too long (max {} characters)",
            super::MAX_TITLE_LEN
        ));
    }

    let sanitized = control.sanitize_session_title(arg);
    let title = sanitized.trim();
    if title.is_empty() {
        return CommandResult::error(
            "Title cannot be empty; use /title off to clear a session title",
        );
    }

    match control.set_window_title(title.to_string()) {
        Ok(()) => CommandResult::message(format!(
            "Window title set to \"{title}\" — the terminal tab now reads [\"{title}\"] …"
        )),
        Err(error) => CommandResult::error(error),
    }
}

#[cfg(test)]
mod tests {
    use super::super::control_test_support::message;
    use super::*;
    use codewhale_command_contract::facets::TitleReport;

    fn control_fake() -> super::super::control_test_support::FakeControl {
        super::super::control_test_support::FakeControl::default()
    }

    #[test]
    fn title_bare_reports_effective_title_and_source() {
        let mut fake = control_fake();
        fake.title_report = Some(TitleReport {
            effective: "task-7".to_string(),
            source: TitleSource::Session,
        });
        let result = title_portable(&mut fake, None);
        assert_eq!(message(&result), "Window title: [task-7] (session)");

        fake.title_report = Some(TitleReport {
            effective: "workspace-x".to_string(),
            source: TitleSource::ConfigDefault,
        });
        let result = title_portable(&mut fake, None);
        assert_eq!(
            message(&result),
            "Window title: [workspace-x] (config default)"
        );

        fake.title_report = Some(TitleReport {
            effective: "unset".to_string(),
            source: TitleSource::None,
        });
        let result = title_portable(&mut fake, None);
        assert_eq!(message(&result), "Window title: [unset]");
    }

    #[test]
    fn title_synonyms_clear_and_set_messages_are_exact() {
        let mut fake = control_fake();
        fake.clear_title = Some(Ok(()));
        for synonym in ["off", "clear", "none"] {
            let result = title_portable(&mut fake, Some(synonym));
            assert!(!result.is_error, "{synonym}");
            assert_eq!(
                message(&result),
                "Window title cleared (the config default still applies if set)"
            );
        }
        assert_eq!(fake.calls.borrow().len(), 3);
        assert!(
            fake.calls
                .borrow()
                .iter()
                .all(|call| call == "clear_window_title")
        );

        let mut fake = control_fake();
        fake.set_title = Some(Ok(()));
        let result = title_portable(&mut fake, Some("task-7"));
        assert!(!result.is_error);
        assert_eq!(
            message(&result),
            "Window title set to \"task-7\" — the terminal tab now reads [\"task-7\"] …"
        );
        assert_eq!(
            fake.calls.borrow().as_slice(),
            ["sanitize_session_title(task-7)", "set_window_title(task-7)"]
        );
        assert!(result.action.is_none(), "/title emits no action");
    }

    #[test]
    fn title_oversized_and_host_errors_are_exact() {
        let mut fake = control_fake();
        let result = title_portable(
            &mut fake,
            Some(&"x".repeat(super::super::MAX_TITLE_LEN + 1)),
        );
        assert!(result.is_error);
        assert!(
            result
                .message
                .as_deref()
                .unwrap()
                .contains("Title too long (max 100 characters)")
        );
        assert!(
            fake.calls.borrow().is_empty(),
            "length check precedes the delegate"
        );

        let mut fake = control_fake();
        fake.sanitized_title = Some(String::new());
        let result = title_portable(&mut fake, Some("\u{1b}\u{7}\u{200b}"));
        assert!(result.is_error);
        assert_eq!(
            message(&result),
            "Title cannot be empty; use /title off to clear a session title"
        );
        assert_eq!(
            fake.calls.borrow().as_slice(),
            ["sanitize_session_title(\u{1b}\u{7}\u{200b})"],
            "sanitized-empty validation precedes host mutation"
        );

        let mut fake = control_fake();
        fake.set_title = Some(Err("Could not save session: set failed".to_string()));
        let result = title_portable(&mut fake, Some("task-7"));
        assert!(result.is_error);
        assert_eq!(message(&result), "Could not save session: set failed");

        let mut fake = control_fake();
        fake.clear_title = Some(Err("Could not save session: clear failed".to_string()));
        let result = title_portable(&mut fake, Some("off"));
        assert!(result.is_error);
        assert_eq!(message(&result), "Could not save session: clear failed");
    }

    #[test]
    fn title_missing_control_authority_fails_safely() {
        let contexts = codewhale_command_contract::handler::CommandContexts::empty();
        let result = title_contextual(contexts, None);
        assert!(result.is_error);
        assert_eq!(
            message(&result),
            "Command capability unavailable: session_control"
        );
    }
}
