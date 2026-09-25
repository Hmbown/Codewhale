//! `/receipts`: what this session did, from the same builder as
//! `codewhale receipts` and `GET /v1/threads/{id}/receipt`.
//!
//! It reads the session's transcript as it stands (the messages the next save
//! writes) and the session's approval log. It does not read display cells, so
//! the terminal and the saved record cannot tell two stories.

use crate::commands::CommandResult;
use crate::receipts::{ReceiptSource, SourceKind, render_json, render_markdown, session_receipt};
use crate::tui::app::App;

pub fn receipts(app: &mut App, arg: Option<&str>) -> CommandResult {
    let mut json = false;
    let mut turn: Option<&str> = None;
    for word in arg.unwrap_or_default().split_whitespace() {
        match word {
            "json" => json = true,
            word if word.parse::<usize>().is_ok() => turn = Some(word),
            other => {
                return CommandResult::error(format!(
                    "Unknown argument '{other}'. Use /receipts [json] [<turn>]."
                ));
            }
        }
    }
    let approvals = match app.current_session_id.as_deref() {
        Some(id) => match crate::approval_log::ApprovalReceiptStore::default_location()
            .and_then(|store| store.load(id))
        {
            Ok(receipts) => receipts,
            Err(error) => {
                return CommandResult::error(format!(
                    "Could not read this session's approval log: {error}"
                ));
            }
        },
        None => Vec::new(),
    };
    let source = ReceiptSource {
        kind: SourceKind::Session,
        id: app
            .current_session_id
            .clone()
            .unwrap_or_else(|| "unsaved".to_string()),
        title: app.session_title.clone(),
        workspace: Some(app.workspace.display().to_string()),
        model: Some(app.model.clone()),
        started_at: Some(app.session_started_at),
        updated_at: None,
    };
    match session_receipt(source, &app.api_messages, &approvals, turn) {
        Ok(receipt) if json => CommandResult::message(render_json(&receipt)),
        Ok(receipt) => CommandResult::message(render_markdown(&receipt).trim_end().to_string()),
        Err(error) => CommandResult::error(error.to_string()),
    }
}
