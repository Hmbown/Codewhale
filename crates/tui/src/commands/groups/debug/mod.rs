//! Portable debug command group. Real host registration, event conversion and
//! I/O adapters live outside this movable source closure.

pub(in crate::commands) use codewhale_command_contract::outcome::{
    DebugAction, DebugCommandResult as CommandResult,
};

pub(in crate::commands) mod balance;
pub(in crate::commands) mod cache;
pub(in crate::commands) mod cache_format;
pub(in crate::commands) mod change;
pub(in crate::commands) mod preview_request;
pub(in crate::commands) mod receipts;
pub(in crate::commands) mod tokens;
pub(in crate::commands) mod tool_inspection;
pub(in crate::commands) mod undo;

#[cfg(test)]
mod operations_tests;
#[cfg(test)]
mod portable_tests;

/// Complete group inventory, shared by extraction proof and host parity tests.
pub fn portable_handlers() -> [(
    &'static codewhale_command_contract::metadata::CommandInfo,
    codewhale_command_contract::handler::CommandHandler<CommandResult>,
); 14] {
    use codewhale_command_contract::metadata::RegisterCommand;
    [
        (tokens::TokensCmd::info(), tokens::TokensCmd::handler()),
        (tokens::CostCmd::info(), tokens::CostCmd::handler()),
        (
            receipts::ReceiptsCmd::info(),
            receipts::ReceiptsCmd::handler(),
        ),
        (balance::BalanceCmd::info(), balance::BalanceCmd::handler()),
        (cache::CacheCmd::info(), cache::CacheCmd::handler()),
        (
            preview_request::PreviewRequestCmd::info(),
            preview_request::PreviewRequestCmd::handler(),
        ),
        (
            tool_inspection::ToolsCmd::info(),
            tool_inspection::ToolsCmd::handler(),
        ),
        (change::ChangeCmd::info(), change::ChangeCmd::handler()),
        (tokens::SystemCmd::info(), tokens::SystemCmd::handler()),
        (tokens::ContextCmd::info(), tokens::ContextCmd::handler()),
        (undo::EditCmd::info(), undo::EditCmd::handler()),
        (undo::DiffCmd::info(), undo::DiffCmd::handler()),
        (undo::UndoCmd::info(), undo::UndoCmd::handler()),
        (undo::RetryCmd::info(), undo::RetryCmd::handler()),
    ]
}
