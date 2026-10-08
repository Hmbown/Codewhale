//! Normal compilation of the actual complete session group without a host.
pub mod commands;

#[cfg(test)]
mod tests {
    use super::commands::groups::session::portable_handlers;
    use codewhale_command_contract::handler::{CommandContexts, CommandHandler};

    #[test]
    fn whole_inventory_keeps_order_and_rejects_missing_authority() {
        let handlers = portable_handlers();
        assert_eq!(
            handlers.map(|(info, _)| info.name),
            [
                "rename",
                "title",
                "save",
                "fork",
                "new",
                "sessions",
                "load",
                "resume",
                "tree",
                "branch",
                "compact",
                "purge",
                "relay",
                "rc",
                "remote-env",
                "export",
                "structcopy",
            ]
        );
        for (info, handler) in portable_handlers() {
            if let CommandHandler::Contextual { handler, .. } = handler {
                let result = handler(CommandContexts::empty(), None);
                assert!(result.is_error, "{} accepted missing authority", info.name);
                assert!(
                    result.action.is_none(),
                    "{} emitted action without authority",
                    info.name
                );
            }
        }
    }
}
