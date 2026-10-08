//! `/rc` command — account-owned web remote control (portable handler).

use super::CommandResult;
use codewhale_command_contract::facets::{CommandSessionControlContext, RemoteOpenOutcome};
use codewhale_command_contract::handler::{CommandContexts, CommandHandler};
use codewhale_command_contract::metadata::{
    CommandInfo as ContractInfo, RegisterCommand as ContractRegisterCommand,
};

/// Shown by `/rc link` and `/rc open` before the control plane has advertised
/// a session link (not connected yet, or an older control plane).
const NO_LINK_MESSAGE: &str =
    "Remote control has no live session link yet; run /rc to hand this session to the web first.";

pub(in crate::commands) struct RemoteControlCmd;

// ---------------------------------------------------------------------------
// FEAT-024 Phase 4 (D6/D7): portable contextual registration and handler.
// Start/status/link/open/stop routing, active-turn wording, no-link guidance,
// stop-refusal safety, and the bounded RemoteControl action payload stay
// handler-owned; all remote-service state stays behind the control facet.
// `/rc open` remains synchronous with no deferred external-URL action.
// ---------------------------------------------------------------------------

pub(in crate::commands) const CONTRACT_INFO: ContractInfo = ContractInfo {
    name: "rc",
    aliases: &["remote-control"],
    usage: "/rc [status|link|open|stop]",
    description_key: "cmd_remote_control_description",
};

impl ContractRegisterCommand<CommandResult> for RemoteControlCmd {
    fn info() -> &'static ContractInfo {
        &CONTRACT_INFO
    }
    fn handler() -> CommandHandler<CommandResult> {
        CommandHandler::Contextual {
            capabilities: codewhale_command_contract::handler::CommandCapabilities::SESSION_CONTROL,
            handler: remote_control_contextual,
        }
    }
}

pub(in crate::commands) fn remote_control_contextual(
    contexts: CommandContexts<'_>,
    arg: Option<&str>,
) -> CommandResult {
    let mut parts = contexts.into_parts();
    let Some(control) = parts.control.as_deref_mut() else {
        return CommandResult::error("Command capability unavailable: session_control".to_string());
    };
    remote_control_portable(control, arg)
}

pub(in crate::commands) fn remote_control_portable(
    control: &mut dyn CommandSessionControlContext,
    arg: Option<&str>,
) -> CommandResult {
    match arg.map(str::trim).filter(|value| !value.is_empty()) {
        None | Some("start") => {
            let connecting = control.remote_start_info().connecting;
            CommandResult::with_message_and_action(
                if connecting {
                    "Connecting web remote control to the active turn…"
                } else {
                    "Starting account-owned web remote control…"
                },
                codewhale_command_contract::outcome::SessionAction::RemoteControl(
                    codewhale_command_contract::outcome::SessionRemoteControlAction::Start,
                ),
            )
        }
        Some("status") => CommandResult::message(control.remote_status()),
        Some("link") => match control.remote_link() {
            Some(link) => {
                let mut message = format!("Remote control session: {}", link.url);
                if let Some(computer_url) = link.computer_url {
                    message.push_str(&format!("\nManage this computer: {computer_url}"));
                }
                CommandResult::message(message)
            }
            None => CommandResult::error(NO_LINK_MESSAGE),
        },
        Some("open") => match control.remote_browser_open() {
            RemoteOpenOutcome::Opened { url } => {
                CommandResult::message(format!("Opening {url} in your browser…"))
            }
            RemoteOpenOutcome::LaunchFailed { url } => {
                CommandResult::error(format!("Could not launch a browser; open {url} manually."))
            }
            RemoteOpenOutcome::NoLink => CommandResult::error(NO_LINK_MESSAGE),
        },
        Some("stop") => {
            // Stop is refused while a remote turn is active or while any
            // terminal/approval/integrity envelope is still awaiting the
            // server-confirmed cursor; releasing the session earlier could
            // strand account-side truth or create a second owner.
            if let Some(reason) = control.remote_stop_refusal() {
                return CommandResult::error(reason);
            }
            CommandResult::with_message_and_action(
                "Stopping web remote control…",
                codewhale_command_contract::outcome::SessionAction::RemoteControl(
                    codewhale_command_contract::outcome::SessionRemoteControlAction::Stop,
                ),
            )
        }
        Some(_) => CommandResult::error("Usage: /rc [status|link|open|stop]"),
    }
}

#[cfg(test)]
mod tests {
    use super::super::control_test_support::message;
    use super::*;
    use codewhale_command_contract::facets::{RemoteLink, RemoteOpenOutcome, RemoteStartInfo};

    fn fake_with_defaults() -> super::super::control_test_support::FakeControl {
        super::super::control_test_support::FakeControl {
            remote_status: Some("Remote control: off".to_string()),
            remote_link: Some(None),
            browser_open: Some(RemoteOpenOutcome::NoLink),
            start_info: Some(RemoteStartInfo { connecting: false }),
            stop_refusal: Some(None),
            ..super::super::control_test_support::FakeControl::default()
        }
    }

    #[test]
    fn rc_start_uses_active_turn_copy_when_connecting() {
        let mut fake = fake_with_defaults();
        fake.start_info = Some(RemoteStartInfo { connecting: true });
        for arg in [None, Some("start")] {
            let result = remote_control_portable(&mut fake, arg);
            assert!(!result.is_error);
            assert!(
                result
                    .message
                    .as_deref()
                    .is_some_and(|m| m == "Connecting web remote control to the active turn…")
            );
            assert!(matches!(
                result.action,
                Some(
                    codewhale_command_contract::outcome::SessionAction::RemoteControl(
                        codewhale_command_contract::outcome::SessionRemoteControlAction::Start
                    )
                )
            ));
        }
        fake.start_info = Some(RemoteStartInfo { connecting: false });
        let result = remote_control_portable(&mut fake, None);
        assert!(
            result
                .message
                .as_deref()
                .is_some_and(|m| m == "Starting account-owned web remote control…")
        );
        assert_eq!(
            fake.calls.borrow().as_slice(),
            [
                "remote_start_info",
                "remote_start_info",
                "remote_start_info"
            ]
        );
    }

    #[test]
    fn rc_status_link_and_no_link_messages_are_exact() {
        let mut fake = fake_with_defaults();
        let status = remote_control_portable(&mut fake, Some("status"));
        assert_eq!(message(&status), "Remote control: off");

        let no_link = remote_control_portable(&mut fake, Some("link"));
        assert!(no_link.is_error);
        assert!(
            no_link
                .message
                .as_deref()
                .unwrap()
                .contains("no live session link")
        );

        fake.remote_link = Some(Some(RemoteLink {
            url: "https://remote.example/s".to_string(),
            computer_url: Some("https://remote.example/c".to_string()),
        }));
        let link = remote_control_portable(&mut fake, Some("link"));
        assert!(!link.is_error);
        assert_eq!(
            message(&link),
            "Remote control session: https://remote.example/s\nManage this computer: https://remote.example/c"
        );
        assert_eq!(
            fake.calls.borrow().as_slice(),
            ["remote_status", "remote_link", "remote_link"]
        );
    }

    #[test]
    fn rc_open_is_synchronous_and_never_emits_external_url_action() {
        let mut fake = fake_with_defaults();
        let no_link = remote_control_portable(&mut fake, Some("open"));
        assert!(no_link.is_error);
        assert!(no_link.action.is_none());
        assert!(
            no_link
                .message
                .as_deref()
                .unwrap()
                .contains("no live session link")
        );

        fake.browser_open = Some(RemoteOpenOutcome::Opened {
            url: "https://remote.example/s".to_string(),
        });
        let opened = remote_control_portable(&mut fake, Some("open"));
        assert!(!opened.is_error);
        assert_eq!(
            message(&opened),
            "Opening https://remote.example/s in your browser…"
        );
        assert!(opened.action.is_none());

        fake.browser_open = Some(RemoteOpenOutcome::LaunchFailed {
            url: "https://remote.example/s".to_string(),
        });
        let failed = remote_control_portable(&mut fake, Some("open"));
        assert!(failed.is_error);
        assert_eq!(
            message(&failed),
            "Could not launch a browser; open https://remote.example/s manually."
        );
        assert!(failed.action.is_none());
        assert_eq!(
            fake.calls.borrow().as_slice(),
            [
                "remote_browser_open",
                "remote_browser_open",
                "remote_browser_open"
            ]
        );
    }

    #[test]
    fn rc_stop_refuses_active_turns_and_unknown_ops_show_usage() {
        let mut fake = fake_with_defaults();
        fake.stop_refusal = Some(Some(
            "stop refused while a remote turn is active".to_string(),
        ));
        let refused = remote_control_portable(&mut fake, Some("stop"));
        assert!(refused.is_error);
        assert_eq!(
            message(&refused),
            "stop refused while a remote turn is active"
        );
        assert!(refused.action.is_none());

        fake.stop_refusal = Some(None);
        let stopped = remote_control_portable(&mut fake, Some("stop"));
        assert!(!stopped.is_error);
        assert!(matches!(
            stopped.action,
            Some(
                codewhale_command_contract::outcome::SessionAction::RemoteControl(
                    codewhale_command_contract::outcome::SessionRemoteControlAction::Stop
                )
            )
        ));

        let unknown = remote_control_portable(&mut fake, Some("frobnicate"));
        assert!(unknown.is_error);
        assert_eq!(message(&unknown), "Usage: /rc [status|link|open|stop]");
        assert_eq!(
            fake.calls.borrow().as_slice(),
            ["remote_stop_refusal", "remote_stop_refusal"]
        );
    }

    #[test]
    fn rc_missing_control_authority_fails_safely() {
        let contexts = codewhale_command_contract::handler::CommandContexts::empty();
        let result = remote_control_contextual(contexts, None);
        assert!(result.is_error);
        assert_eq!(
            message(&result),
            "Command capability unavailable: session_control"
        );
    }
}
