//! Codewhale runtime: the headless half of the runtime/TUI split.
//!
//! Modules move here from `crates/tui/src` with `git mv`, in the order
//! `docs/design/TUI_DECONSTRUCTION.md` records. The TUI reaches them through
//! one path-alias block in its `lib.rs`, so moved code keeps its
//! `crate::<module>` paths on both sides until the split deletes the alias.
//!
//! This crate never depends on the terminal UI: no `codewhale-tui`, ratatui
//! or crossterm (`scripts/check-command-crate-boundaries.py`).

// Copied verbatim from the TUI crate root: moved code relies on it.
#![allow(clippy::uninlined_format_args)]

pub mod context_budget;
pub mod continual_harness;
pub mod elapsed;
pub mod fast_hash;
pub mod goal_loop;
pub mod hashing;
pub mod llm_response_cache;
pub mod media_originals;
pub mod model_context;
pub mod native_memory;
pub mod prompt_zones;
pub mod regex_cache;
pub mod retry_status;
pub mod safe_label;
pub mod session_tree;
pub mod skill_state;
pub mod sleep_guard;
pub mod tool_history_repair;
pub mod workspace_discovery;
