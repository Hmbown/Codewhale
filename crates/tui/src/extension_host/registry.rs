//! Owned registrations: the Rust side is the authority (design §3).
//!
//! Every registration belongs to exactly one `(plugin_id, generation)` owner.
//! Revocation is synchronous and never waits for the host: removing an owner
//! removes its tools from every later turn's registry at once, and
//! `HostToolSpec` re-checks liveness before each call. Handles are never
//! reused, so undoing one registration can never touch a newer one.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde_json::Value;

use super::protocol::{OwnerRef, RegisterParams};
use crate::plugins::types::PluginAuthority;

/// Largest accepted tool input schema, serialized.
pub const MAX_SCHEMA_BYTES: usize = 64 * 1024;
/// Largest accepted tool description.
pub const MAX_DESCRIPTION_BYTES: usize = 4 * 1024;
pub const MAX_TOOLS_PER_OWNER: usize = 128;
pub const MAX_TOOLS_PER_HOST: usize = 1024;

/// Name prefixes no extension may use: MCP's namespace, and one kept free
/// for future core-issued extension names.
const RESERVED_PREFIXES: &[&str] = &["mcp_", "ext_"];

/// Core tool names that exist outside the native registry builder (catalog
/// meta-tools) and so never show up in a registry snapshot.
const RESERVED_NAMES: &[&str] = &[
    "tool_search",
    "tool_search_tool_regex",
    "tool_search_tool_bm25",
    "retrieve_tool_result",
    "execute_tools",
    "code_execution",
    "js_execution",
    "request_user_input",
    "multi_tool_use.parallel",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnerState {
    Activating,
    Active,
    Failed(String),
    Faulted(String),
    Revoked,
}

#[derive(Debug, Clone)]
pub struct OwnerEntry {
    pub owner: OwnerRef,
    pub plugin_name: String,
    pub authority: PluginAuthority,
    pub content_hash: String,
    pub state: OwnerState,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolRegistration {
    pub handle: u64,
    pub owner: OwnerRef,
    pub plugin_name: String,
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug, Default)]
pub struct OwnerRegistry {
    next_handle: u64,
    next_generation: u64,
    owners: HashMap<String, OwnerEntry>,
    tools: BTreeMap<u64, ToolRegistration>,
    /// Lower-cased tool name → handle, so `Read` cannot impersonate `read`.
    by_name: HashMap<String, u64>,
    /// Lower-cased names of native tools seen at the last turn build.
    native_names: HashSet<String>,
}

fn mint_token() -> String {
    // Two v4 UUIDs: 244 random bits from the OS generator.
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

fn valid_tool_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(first) if first.is_ascii_alphabetic())
        && name.len() <= 64
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Why the core would treat a tool called `name` as something other than an
/// opaque extension tool, if it would.
///
/// Approval keys, approval-card summaries, and the approval/auto-review
/// category are all derived from the tool *name*. A name any of them
/// special-cases would let an extension borrow a native tool's identity: a
/// session grant for `fetch_url` on a host (`net:<host>`) would approve a
/// plugin tool named `web_fetch`, and a `read_*` name would be classified as
/// a read. Such names need not be registered natives (`web_fetch`,
/// `exec_wait`, and mode-dependent tools such as `task_shell_start` are not),
/// so they are refused by probing the classifiers themselves rather than by a
/// hand-kept list that would drift from them.
fn core_special_case(name: &str) -> Option<&'static str> {
    use crate::core::authority::{ToolCategory, get_tool_category_for_call};
    use crate::tools::approval_cache::{build_approval_grouping_key, build_approval_key};
    let empty = Value::Object(serde_json::Map::new());
    let spellings = [name.to_string(), name.to_ascii_lowercase()];
    for spelling in &spellings {
        let generic = format!("tool:{spelling}:");
        if !build_approval_key(spelling, &empty).0.starts_with(&generic)
            || !build_approval_grouping_key(spelling, &empty)
                .0
                .starts_with(&generic)
        {
            return Some("the approval cache keys it as a built-in tool family");
        }
        if crate::tools::canonical_action::canonical_action_alias(spelling, &empty) != spelling {
            return Some("it is an alias of a built-in tool");
        }
        if crate::tools::approval_summary::approval_summary(spelling, &empty, None)
            != format!("Use the {spelling} tool")
        {
            return Some("approval cards describe it as a built-in tool");
        }
        if get_tool_category_for_call(spelling, &empty) != ToolCategory::Unknown {
            return Some(
                "the approval policy classifies it by name (read, write, shell, network, MCP or agent)",
            );
        }
    }
    None
}

impl OwnerRegistry {
    #[must_use]
    pub fn new() -> Self {
        let mut registry = Self::default();
        registry.set_native_names(std::iter::empty::<&str>());
        registry
    }

    /// Record the native tool names of the current build (the registry before
    /// scripts, plugins or extensions are added), on top of the static set.
    pub fn set_native_names<'a>(&mut self, names: impl IntoIterator<Item = &'a str>) {
        let mut set: HashSet<String> = RESERVED_NAMES
            .iter()
            .chain(crate::core::engine::tool_catalog::DEFAULT_ACTIVE_NATIVE_TOOLS)
            .map(|name| name.to_ascii_lowercase())
            .collect();
        for (family, _, alias) in crate::tools::canonical_action::CANONICAL_ACTION_ALIASES {
            set.insert(family.to_ascii_lowercase());
            set.insert(alias.to_ascii_lowercase());
        }
        set.extend(names.into_iter().map(str::to_ascii_lowercase));
        self.native_names = set;
    }

    /// Start a new activation for `plugin_id`, superseding any previous one.
    pub fn begin_owner(
        &mut self,
        plugin_id: &str,
        plugin_name: &str,
        authority: PluginAuthority,
        content_hash: &str,
    ) -> OwnerRef {
        self.revoke_owner(plugin_id);
        self.next_generation += 1;
        let owner = OwnerRef {
            plugin_id: plugin_id.to_string(),
            generation: self.next_generation,
            owner_token: mint_token(),
        };
        self.owners.insert(
            plugin_id.to_string(),
            OwnerEntry {
                owner: owner.clone(),
                plugin_name: plugin_name.to_string(),
                authority,
                content_hash: content_hash.to_string(),
                state: OwnerState::Activating,
            },
        );
        owner
    }

    #[must_use]
    pub fn owner(&self, plugin_id: &str) -> Option<&OwnerEntry> {
        self.owners.get(plugin_id)
    }

    pub fn owners(&self) -> impl Iterator<Item = &OwnerEntry> {
        self.owners.values()
    }

    /// The exact current, not-yet-revoked owner (token and generation match).
    fn current(&self, owner: &OwnerRef) -> Option<&OwnerEntry> {
        self.owners.get(&owner.plugin_id).filter(|entry| {
            entry.owner == *owner
                && matches!(entry.state, OwnerState::Activating | OwnerState::Active)
        })
    }

    pub fn mark_active(&mut self, owner: &OwnerRef) -> bool {
        match self.owners.get_mut(&owner.plugin_id) {
            Some(entry) if entry.owner == *owner && entry.state == OwnerState::Activating => {
                entry.state = OwnerState::Active;
                true
            }
            _ => false,
        }
    }

    /// Mark an owner failed or faulted and drop everything it registered.
    pub fn mark_failed(&mut self, owner: &OwnerRef, state: OwnerState) {
        let matches = self
            .owners
            .get(&owner.plugin_id)
            .is_some_and(|entry| entry.owner == *owner);
        if matches {
            self.remove_tools_of(&owner.plugin_id);
            if let Some(entry) = self.owners.get_mut(&owner.plugin_id) {
                entry.state = state;
            }
        }
    }

    /// Admit or refuse one `registry/register`.
    pub fn register_tool(&mut self, params: &RegisterParams) -> Result<u64, String> {
        let entry = self
            .current(&params.owner)
            .ok_or_else(|| "stale or unknown owner".to_string())?;
        let plugin_name = entry.plugin_name.clone();
        let spec = &params.spec;
        let name = spec.name.as_str();
        if !valid_tool_name(name) {
            return Err(format!(
                "tool name `{}` must match ^[A-Za-z][A-Za-z0-9_-]{{0,63}}$",
                crate::safe_label::SafeLabel::identifier(name)
            ));
        }
        let key = name.to_ascii_lowercase();
        if RESERVED_PREFIXES
            .iter()
            .any(|prefix| key.starts_with(prefix))
        {
            return Err(format!("tool name `{name}` uses a reserved prefix"));
        }
        if self.native_names.contains(&key) {
            return Err(format!(
                "tool name `{name}` collides with a built-in tool; extensions never shadow core tools"
            ));
        }
        if let Some(reason) = core_special_case(name) {
            return Err(format!(
                "tool name `{name}` is reserved: {reason}; extension tools never borrow a built-in's approval identity"
            ));
        }
        if spec.description.len() > MAX_DESCRIPTION_BYTES {
            return Err(format!(
                "tool `{name}` description exceeds {MAX_DESCRIPTION_BYTES} bytes"
            ));
        }
        let schema = Value::Object(spec.input_schema.clone());
        let schema_bytes = serde_json::to_vec(&schema)
            .map(|bytes| bytes.len())
            .unwrap_or(usize::MAX);
        if schema_bytes > MAX_SCHEMA_BYTES {
            return Err(format!(
                "tool `{name}` input schema exceeds {MAX_SCHEMA_BYTES} bytes"
            ));
        }
        if schema.get("type").and_then(Value::as_str) != Some("object") {
            return Err(format!(
                "tool `{name}` input schema must be a JSON object schema (`\"type\": \"object\"`)"
            ));
        }
        let mut replaced = None;
        if let Some(existing) = self
            .by_name
            .get(&key)
            .and_then(|handle| self.tools.get(handle))
        {
            if existing.owner.plugin_id != params.owner.plugin_id {
                return Err(format!(
                    "tool name `{name}` is already registered by extension `{}`",
                    existing.plugin_name
                ));
            }
            // Same owner re-registering a name: the new handle retires the old.
            replaced = Some(existing.handle);
        }
        let owned = self
            .tools
            .values()
            .filter(|tool| tool.owner.plugin_id == params.owner.plugin_id)
            .count()
            - usize::from(replaced.is_some());
        if owned >= MAX_TOOLS_PER_OWNER {
            return Err(format!(
                "an extension may register at most {MAX_TOOLS_PER_OWNER} tools"
            ));
        }
        if self.tools.len() - usize::from(replaced.is_some()) >= MAX_TOOLS_PER_HOST {
            return Err(format!(
                "the extension host holds at most {MAX_TOOLS_PER_HOST} tools"
            ));
        }
        if let Some(old) = replaced {
            self.tools.remove(&old);
        }
        self.next_handle += 1;
        let handle = self.next_handle;
        self.tools.insert(
            handle,
            ToolRegistration {
                handle,
                owner: params.owner.clone(),
                plugin_name,
                name: name.to_string(),
                description: spec.description.clone(),
                input_schema: schema,
            },
        );
        self.by_name.insert(key, handle);
        Ok(handle)
    }

    /// Undo exactly one registration. Idempotent; a stale or foreign handle is a no-op.
    pub fn unregister(&mut self, owner: &OwnerRef, handle: u64) {
        let owned = self
            .tools
            .get(&handle)
            .is_some_and(|tool| tool.owner == *owner);
        if !owned {
            return;
        }
        if let Some(tool) = self.tools.remove(&handle) {
            let key = tool.name.to_ascii_lowercase();
            if self.by_name.get(&key) == Some(&handle) {
                self.by_name.remove(&key);
            }
        }
    }

    fn remove_tools_of(&mut self, plugin_id: &str) -> Vec<u64> {
        let handles: Vec<u64> = self
            .tools
            .values()
            .filter(|tool| tool.owner.plugin_id == plugin_id)
            .map(|tool| tool.handle)
            .collect();
        for handle in &handles {
            if let Some(tool) = self.tools.remove(handle) {
                let key = tool.name.to_ascii_lowercase();
                if self.by_name.get(&key) == Some(handle) {
                    self.by_name.remove(&key);
                }
            }
        }
        handles
    }

    /// Revoke an owner synchronously. Returns the owner that was live, if any.
    pub fn revoke_owner(&mut self, plugin_id: &str) -> Option<OwnerRef> {
        self.remove_tools_of(plugin_id);
        let entry = self.owners.get_mut(plugin_id)?;
        let was_live = matches!(entry.state, OwnerState::Activating | OwnerState::Active);
        entry.state = OwnerState::Revoked;
        was_live.then(|| entry.owner.clone())
    }

    /// Forget an owner entirely (after revocation, when its plugin is gone).
    pub fn forget_owner(&mut self, plugin_id: &str) {
        self.remove_tools_of(plugin_id);
        self.owners.remove(plugin_id);
    }

    /// Forget owners that are not live (failed, faulted, revoked) so a new
    /// session retries them.
    pub fn forget_inactive(&mut self) {
        self.owners
            .retain(|_, entry| matches!(entry.state, OwnerState::Activating | OwnerState::Active));
    }

    /// The host exited: every owner is revoked and every tool is gone.
    pub fn revoke_all(&mut self, reason: &str) {
        self.tools.clear();
        self.by_name.clear();
        for entry in self.owners.values_mut() {
            if matches!(entry.state, OwnerState::Activating | OwnerState::Active) {
                entry.state = OwnerState::Failed(reason.to_string());
            }
        }
    }

    /// Tools of active owners, in handle order.
    #[must_use]
    pub fn live_tools(&self) -> Vec<ToolRegistration> {
        self.tools
            .values()
            .filter(|tool| {
                self.owners.get(&tool.owner.plugin_id).is_some_and(|entry| {
                    entry.owner == tool.owner && entry.state == OwnerState::Active
                })
            })
            .cloned()
            .collect()
    }

    /// Whether `handle` is still admitted for exactly this owner generation.
    #[must_use]
    pub fn is_live(&self, handle: u64, owner: &OwnerRef) -> bool {
        self.tools
            .get(&handle)
            .is_some_and(|tool| tool.owner == *owner)
            && self
                .owners
                .get(&owner.plugin_id)
                .is_some_and(|entry| entry.owner == *owner && entry.state == OwnerState::Active)
    }

    /// Active owners other than `plugin_id` sharing the one host process.
    #[must_use]
    pub fn other_active_owners(&self, plugin_id: &str) -> usize {
        self.owners
            .values()
            .filter(|entry| entry.owner.plugin_id != plugin_id && entry.state == OwnerState::Active)
            .count()
    }

    #[must_use]
    pub fn authority_for(&self, owner: &OwnerRef) -> Option<PluginAuthority> {
        self.current(owner).map(|entry| entry.authority.clone())
    }
}
