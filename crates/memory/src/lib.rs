//! CodeWhale memory: a local library, not a second agent loop.
//!
//! Hosts construct [`Access`] from their trusted runtime, never model arguments.
//! Captures are candidates. Only a trusted reviewer can promote them. Search
//! filters scope, status, expiry, and repository freshness before ranking.
//! The synchronous SQLite owner belongs on a blocking worker, not the UI thread.
#![forbid(unsafe_code)]

pub mod auth;
pub mod context;
pub mod hooks;
pub mod import;
pub mod lens;
pub mod model;
pub mod policy;
pub mod protocol;
pub mod runtime;
pub mod store;
pub mod workspace;

pub use auth::{Access, Capability};
pub use context::{ByteCounter, ContextBudget, ContextPacket, TokenCounter, compile_context};
pub use model::*;
pub use store::{MemoryBackend, Store};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("memory access denied")]
    Denied,
    #[error("memory not found in the authorized scope")]
    NotFound,
    #[error("revision conflict; reload before writing")]
    RevisionConflict,
    #[error("an active memory already owns this key")]
    KeyConflict,
    #[error("request key was already used for different content")]
    IdempotencyConflict,
    #[error("this exact memory was forgotten; automatic reimport is blocked")]
    Forgotten,
    #[error("memory is not in a valid state for this operation")]
    InvalidState,
    #[error("a trusted, content-bound passing validation receipt is required")]
    ValidationRequired,
    #[error("a parent is not active or has expired")]
    InvalidParent,
    #[error("possible secret detected; content was not persisted")]
    SecretDetected,
    #[error("invalid input: {0}")]
    Invalid(String),
    #[error("unsupported or non-memory database; no migration was attempted")]
    DatabaseMismatch,
    #[error("memory is disabled")]
    Disabled,
    #[error("storage error")]
    Sql(#[from] rusqlite::Error),
    #[error("JSON encoding or decoding error")]
    Json(#[from] serde_json::Error),
    #[error("filesystem or stream error")]
    Io(#[from] std::io::Error),
}

impl Error {
    /// Safe public code: never echoes a SQL statement, a path, or secret input.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Denied => "denied",
            Self::NotFound => "not_found",
            Self::RevisionConflict => "revision_conflict",
            Self::KeyConflict => "key_conflict",
            Self::IdempotencyConflict => "idempotency_conflict",
            Self::Forgotten => "forgotten",
            Self::InvalidState => "invalid_state",
            Self::ValidationRequired => "validation_required",
            Self::InvalidParent => "invalid_parent",
            Self::SecretDetected => "secret_detected",
            Self::Invalid(_) => "invalid_input",
            Self::DatabaseMismatch => "database_mismatch",
            Self::Disabled => "disabled",
            Self::Sql(_) => "storage_error",
            Self::Json(_) => "json_error",
            Self::Io(_) => "io_error",
        }
    }
}
pub type Result<T> = std::result::Result<T, Error>;
