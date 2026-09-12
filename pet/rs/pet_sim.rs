//! Compatibility entry point: the product owns the single Rust implementation.
#[path = "../../crates/tui/src/tui/ambient_life/pet_sim.rs"]
mod shared;
pub use self::shared::*;
