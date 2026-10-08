//! Shared session lifecycle implementation (FEAT-023).
//!
//! The nine lifecycle commands' concrete host work moved into the
//! `SessionLifecycleAdapter` in the host command contract module (FEAT-023 Phase 3)
//! and the portable handlers own all parsing/message/action composition
//! (Phase 4). Dispatch switched to the contract registrations in Phase 6, so
//! no lifecycle body remains here. Shared host helpers used by the still
//! legacy session leaves (FEAT-024/025/026 ownership) live beside those leaves;
//! the migration-topology `session::lifecycle` scope keeps this file as the
//! tenth predeclared source file until the root `session` entry is removed by
//! FEAT-026.
