//! Host-side opt-in gate. No store, index, or model tools are opened when off.
//! Construct this from trusted application configuration, not tool arguments.
use crate::{Access, Result, Scope, Store, protocol::ToolServer};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct RuntimeOptions {
    pub enabled: bool,
    pub database: PathBuf,
    pub workspace_root: Option<PathBuf>,
}

/// Own this synchronous adapter on a blocking worker. Expose its catalog only
/// when Some is returned. Review and destructive controls stay with the host.
pub fn open_tools(
    options: RuntimeOptions,
    access: Access,
    write_scope: Scope,
    checkpoint_scope: Option<Scope>,
) -> Result<Option<ToolServer>> {
    if !options.enabled {
        return Ok(None);
    }
    let store = Store::open(&options.database)?;
    Ok(Some(ToolServer::new(
        store,
        access,
        write_scope,
        checkpoint_scope,
        options.workspace_root,
    )?))
}
