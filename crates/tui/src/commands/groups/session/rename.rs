//! `/rename` command — portable handler over the session-control facet.
//!
//! The handler owns sanitization, blank and 100-character validation, and
//! exact message composition. The atomic `rename_session` delegate owns only
//! first-snapshot recovery, persistence, and publication so the policy moves
//! with the handler while host mutation ordering cannot drift.

use super::CommandResult;
use codewhale_command_contract::facets::CommandSessionControlContext;
use codewhale_command_contract::handler::{CommandContexts, CommandHandler};
use codewhale_command_contract::metadata::{
    CommandInfo as ContractInfo, RegisterCommand as ContractRegisterCommand,
};

pub(in crate::commands) struct RenameCmd;

pub(in crate::commands) const CONTRACT_INFO: ContractInfo = ContractInfo {
    name: "rename",
    aliases: &["gaiming", "chongmingming"],
    usage: "/rename <new title>",
    description_key: "cmd_rename_description",
};

impl ContractRegisterCommand<CommandResult> for RenameCmd {
    fn info() -> &'static ContractInfo {
        &CONTRACT_INFO
    }
    fn handler() -> CommandHandler<CommandResult> {
        CommandHandler::Contextual {
            capabilities: codewhale_command_contract::handler::CommandCapabilities::SESSION_CONTROL,
            handler: rename_contextual,
        }
    }
}

pub(in crate::commands) fn rename_contextual(
    contexts: CommandContexts<'_>,
    arg: Option<&str>,
) -> CommandResult {
    let mut parts = contexts.into_parts();
    let Some(control) = parts.control.as_deref_mut() else {
        return CommandResult::error("Command capability unavailable: session_control".to_string());
    };
    rename_portable(control, arg)
}

pub(in crate::commands) fn rename_portable(
    control: &mut dyn CommandSessionControlContext,
    arg: Option<&str>,
) -> CommandResult {
    let Some(raw) = arg else {
        return CommandResult::error("Usage: /rename <new title>");
    };
    let sanitized = control.sanitize_session_title(raw);
    let title = sanitized.trim();
    if title.is_empty() {
        return CommandResult::error("Usage: /rename <new title>");
    }
    if title.chars().count() > super::MAX_TITLE_LEN {
        return CommandResult::error(format!(
            "Title too long (max {} characters)",
            super::MAX_TITLE_LEN
        ));
    }
    match control.rename_session(title) {
        Ok(receipt) => CommandResult::message(format!("Session renamed to \"{}\"", receipt.title)),
        Err(error) => CommandResult::error(error),
    }
}

#[cfg(test)]
mod tests {
    use super::super::control_test_support::message;
    use super::*;
    use codewhale_command_contract::facets::SessionTitleReceipt;

    fn control_fake() -> super::super::control_test_support::FakeControl {
        super::super::control_test_support::FakeControl::default()
    }

    #[test]
    fn rename_usage_boundaries_and_success_messages_are_exact() {
        let mut no_arg = control_fake();
        let result = rename_portable(&mut no_arg, None);
        assert!(result.is_error);
        assert_eq!(message(&result), "Usage: /rename <new title>");
        assert!(
            no_arg.calls.borrow().is_empty(),
            "no delegate call for blank input"
        );

        let mut blank = control_fake();
        let result = rename_portable(&mut blank, Some("   "));
        assert!(result.is_error);
        assert_eq!(message(&result), "Usage: /rename <new title>");
        assert_eq!(
            blank.calls.borrow().as_slice(),
            ["sanitize_session_title(   )"],
            "sanitized blank input never reaches host mutation"
        );

        let mut oversized = control_fake();
        oversized.sanitized_title = Some("x".repeat(super::super::MAX_TITLE_LEN + 1));
        let result = rename_portable(&mut oversized, Some("raw"));
        assert!(result.is_error);
        assert_eq!(message(&result), "Title too long (max 100 characters)");
        assert_eq!(
            oversized.calls.borrow().as_slice(),
            ["sanitize_session_title(raw)"],
            "length policy runs before host mutation"
        );

        let mut ok = control_fake();
        ok.rename = Some(Ok(SessionTitleReceipt {
            title: "New Name".to_string(),
        }));
        let result = rename_portable(&mut ok, Some("New Name"));
        assert!(!result.is_error);
        assert_eq!(message(&result), "Session renamed to \"New Name\"");
        assert_eq!(
            ok.calls.borrow().as_slice(),
            [
                "sanitize_session_title(New Name)",
                "rename_session(New Name)"
            ]
        );
        assert!(result.action.is_none(), "/rename emits no action");
    }

    #[test]
    fn rename_host_errors_pass_through_exactly() {
        let mut fake = control_fake();
        fake.rename = Some(Err("Could not save session: boom".to_string()));
        let result = rename_portable(&mut fake, Some("Whatever"));
        assert!(result.is_error);
        assert_eq!(message(&result), "Could not save session: boom");
    }

    #[test]
    fn rename_missing_control_authority_fails_safely() {
        let contexts = codewhale_command_contract::handler::CommandContexts::empty();
        let result = rename_contextual(contexts, None);
        assert!(result.is_error);
        assert_eq!(
            message(&result),
            "Command capability unavailable: session_control"
        );
    }
}
