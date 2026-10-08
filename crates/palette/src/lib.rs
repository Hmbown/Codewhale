//! Codewhale color palette and semantic roles.
//!
//! This crate defines the color system for the TUI in three layers:
//!
//! 1. **RGB tuples** (`*_RGB` constants) — raw color values used by theme
//!    generation and runtime palette construction.
//! 2. **Semantic `Color` constants** — pre-computed `ratatui::style::Color`
//!    values mapped to UI roles (surface, text, accent, status, mode).
//! 3. **Backward-compatible aliases** (`DEEPSEEK_*`) — legacy names that
//!    delegate to the current Whale palette constants.
//!
//! ## Features
//!
//! `ratatui` (default) enables layers 2 and 3, themes, adaptation, contrast
//! math and terminal background detection. Without it the crate is only the
//! renderer-free core: the RGB tuples, [`ThemeId`] and theme-name / hex
//! colour normalizers, so a headless settings layer can validate
//! `theme = "..."` without linking ratatui.

mod ids;
mod rgb;

#[cfg(feature = "ratatui")]
mod adapt;
#[cfg(feature = "ratatui")]
mod contrast;
#[cfg(feature = "ratatui")]
mod detect;
#[cfg(feature = "ratatui")]
pub mod grammar;
#[cfg(feature = "ratatui")]
pub mod osc11;
#[cfg(feature = "ratatui")]
mod themes;
#[cfg(feature = "ratatui")]
mod tokens;
#[cfg(feature = "ratatui")]
mod user_theme;

#[cfg(all(test, feature = "ratatui"))]
mod tests;

pub use ids::*;
pub use rgb::*;

#[cfg(feature = "ratatui")]
#[allow(unused_imports)]
pub use adapt::*;
#[cfg(feature = "ratatui")]
#[allow(unused_imports)]
pub use contrast::*;
#[cfg(feature = "ratatui")]
#[allow(unused_imports)]
pub use detect::*;
#[cfg(feature = "ratatui")]
#[allow(unused_imports)]
pub use grammar::{ChromeInk, SemanticFamily, chrome_style};
#[cfg(feature = "ratatui")]
#[allow(unused_imports)]
pub use osc11::*;
#[cfg(feature = "ratatui")]
#[allow(unused_imports)]
pub use themes::*;
#[cfg(feature = "ratatui")]
pub use tokens::*;
#[cfg(feature = "ratatui")]
pub use user_theme::*;
