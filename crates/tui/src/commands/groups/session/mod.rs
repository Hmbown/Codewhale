//! Session command area: saving, forking, resuming, exporting, and the
//! `/relay` session-handoff artifact.

mod branch;
mod compact;
mod export;
mod fork;
mod load;
mod new;
mod purge;
mod relay;
mod remote_control;
mod remote_env;
mod rename;
mod resume;
mod save;
mod sessions;
mod structcopy;
mod title;
mod tree;
// This group dir intentionally has a `session.rs` child module with the same
// name. The module_inception allow is a permanent structure rationale, not
// migration scaffolding; see docs/architecture/command-dispatch.md.
#[allow(clippy::module_inception)]
mod session;

use crate::commands::CommandResult;

/// Shared user-facing length policy for `/rename` and `/title`.
pub(in crate::commands) const MAX_TITLE_LEN: usize = 100;
use crate::commands::traits::{
    Command, CommandGroup, ContextualCommand, FunctionCommand, RegisterCommand,
};

pub struct SessionCommands;

impl CommandGroup for SessionCommands {
    fn commands(&self) -> &'static [Box<dyn Command>] {
        cached_command_list!(vec![
            Box::new(
                ContextualCommand::from_contract::<rename::RenameCmd>()
                    .expect("rename registration")
            ),
            Box::new(
                ContextualCommand::from_contract::<title::TitleCmd>().expect("title registration")
            ),
            Box::new(
                ContextualCommand::from_contract::<save::SaveCmd>().expect("save registration")
            ),
            Box::new(
                ContextualCommand::from_contract::<fork::ForkCmd>().expect("fork registration")
            ),
            Box::new(ContextualCommand::from_contract::<new::NewCmd>().expect("new registration")),
            Box::new(
                ContextualCommand::from_contract::<sessions::SessionsCmd>()
                    .expect("sessions registration")
            ),
            Box::new(
                ContextualCommand::from_contract::<load::LoadCmd>().expect("load registration")
            ),
            Box::new(
                ContextualCommand::from_contract::<resume::ResumeCmd>()
                    .expect("resume registration")
            ),
            Box::new(
                ContextualCommand::from_contract::<tree::TreeCmd>().expect("tree registration")
            ),
            Box::new(
                ContextualCommand::from_contract::<branch::BranchCmd>()
                    .expect("branch registration")
            ),
            Box::new(
                ContextualCommand::from_contract::<compact::CompactCmd>()
                    .expect("compact registration")
            ),
            Box::new(
                ContextualCommand::from_contract::<purge::PurgeCmd>().expect("purge registration")
            ),
            Box::new(
                ContextualCommand::from_contract::<relay::RelayCmd>().expect("relay registration")
            ),
            Box::new(
                ContextualCommand::from_contract::<remote_control::RemoteControlCmd>()
                    .expect("remote_control registration")
            ),
            Box::new(
                ContextualCommand::from_contract::<remote_env::RemoteEnvCmd>()
                    .expect("remote_env registration")
            ),
            Box::new(
                ContextualCommand::from_contract::<export::ExportCmd>()
                    .expect("export registration")
            ),
            Box::new(FunctionCommand::new(
                structcopy::StructcopyCmd::info(),
                structcopy::StructcopyCmd::execute,
            )),
        ])
    }
}

// ---------------------------------------------------------------------------
// FEAT-023 Phase 4 (D6): map a portable lifecycle sync payload into the
// temporary `SyncSession` action. FEAT-037 owns the eventual shared outcome
// types; until then the mapping lives here so every portable handler composes
// the same action from the same receipt.
// ---------------------------------------------------------------------------
pub(in crate::commands) fn sync_session_action(
    sync: codewhale_command_contract::facets::SessionSyncPayload,
) -> crate::tui::app::AppAction {
    crate::tui::app::AppAction::SyncSession {
        session_id: sync.session_id,
        messages: sync.messages,
        system_prompt: sync.system_prompt,
        model: sync.model,
        workspace: sync.workspace,
        mode: crate::commands::contract::from_command_mode(sync.mode),
    }
}

#[cfg(test)]
mod control_test_support;
#[cfg(test)]
mod lifecycle_portable_tests;
#[cfg(test)]
mod lifecycle_test_support;
