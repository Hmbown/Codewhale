//! Pure redaction and output sanitization, moved from codewhale-secrets.
//! No credential storage, environment observation or host I/O belongs here.
#![deny(missing_docs)]

/// Secret-redaction primitives.
pub mod redact;
/// Text, URL and ANSI sanitization.
pub mod sanitize;
