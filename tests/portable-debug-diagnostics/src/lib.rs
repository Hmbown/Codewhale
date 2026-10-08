//! Independent compilation of the actual complete debug group and helpers.
//! No transport stand-ins: production uses these same contract-owned outcomes.
pub mod commands;
#[path = "../../../crates/tui/src/diagnostics_reports/mod.rs"]
pub mod diagnostics_reports;
#[path = "../../../crates/runtime/src/elapsed.rs"]
pub mod elapsed;
