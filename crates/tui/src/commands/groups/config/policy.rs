//! Complete portable config-policy slice and its production inventory.
//! Remaining config commands are host-owned until their separate adoption.
use codewhale_command_contract::handler::CommandHandler;
use codewhale_command_contract::metadata::{CommandInfo, RegisterCommand};
use codewhale_command_contract::money;
use codewhale_command_contract::outcome::ConfigPolicyCommandResult as CommandResult;
#[path = "permissions.rs"]
pub mod permissions;
#[path = "policy_messages.rs"]
pub(crate) mod policy_messages;
#[path = "status.rs"]
pub mod status;

pub fn portable_handlers() -> [(&'static CommandInfo, CommandHandler<CommandResult>); 2] {
    [
        (
            permissions::PermissionsCmd::info(),
            permissions::PermissionsCmd::handler(),
        ),
        (
            status::StatusCmd::info(),
            CommandHandler::Contextual {
                capabilities: status::CAPABILITIES,
                handler: |contexts, args| {
                    let result = status::execute(contexts, args);
                    CommandResult {
                        message: result.message,
                        action: result.action.map(|impossible| match impossible {}),
                        is_error: result.is_error,
                    }
                },
            },
        ),
    ]
}

use codewhale_protocol::display::interpolate;

#[cfg(test)]
#[path = "policy_tests.rs"]
mod tests;
