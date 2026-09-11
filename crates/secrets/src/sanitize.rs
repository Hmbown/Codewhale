//! Pure output-sanitization primitives shared by portable command helpers
//! (FEAT-025 D4).
//!
//! These helpers previously lived in the TUI command, client, and OSC8
//! modules. Relocating the pure algorithms here gives `/export` and
//! `/structcopy` exactly one implementation with no TUI, client, or
//! configuration dependency. TUI callers delegate back to these functions so
//! behavior cannot drift.

use std::sync::OnceLock;

use regex::Regex;
use serde_json::Value;

use crate::redact::redact_secrets;

/// Strip ANSI/OSC/control sequences from `s` into `out`.
///
/// Handles CSI (`ESC [ … final`), OSC (`ESC ] … BEL` or `ESC \`), DCS, SOS,
/// PM, APC, and standalone two-byte ESC sequences. OSC 8 hyperlink wrappers
/// (`ESC ] 8 ; … BEL` / `ESC \`) are stripped along with the rest.
pub fn strip_ansi_into(s: &str, out: &mut String) {
    strip_ansi_impl(s, out, false);
}

/// Like [`strip_ansi_into`], but SGR sequences (`ESC [ … m`: colour, bold,
/// underline, reset) pass through untouched so a renderer that understands
/// them can paint the output as the tool emitted it. Everything else — OSC
/// (including OSC 8 hyperlink wrappers), cursor movement, DCS, lone control
/// bytes — is still removed; only the styling survives.
pub fn strip_ansi_keep_sgr_into(s: &str, out: &mut String) {
    strip_ansi_impl(s, out, true);
}

/// Length in bytes of the UTF-8 sequence that starts with `lead`. Falls back
/// to `1` for continuation bytes / invalid leads so callers always make
/// forward progress.
pub fn utf8_seq_len(lead: u8) -> usize {
    if lead < 0xc0 {
        1
    } else if lead < 0xe0 {
        2
    } else if lead < 0xf0 {
        3
    } else {
        4
    }
}

fn strip_ansi_impl(s: &str, out: &mut String, keep_sgr: bool) {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b && i + 1 < bytes.len() {
            let next = bytes[i + 1];
            match next {
                // CSI: ESC [ ... <final byte 0x40..=0x7E>
                b'[' => {
                    let mut j = i + 2;
                    let mut final_byte = 0u8;
                    while j < bytes.len() {
                        let b = bytes[j];
                        if (0x40..=0x7e).contains(&b) {
                            final_byte = b;
                            j += 1;
                            break;
                        }
                        j += 1;
                    }
                    if keep_sgr
                        && final_byte == b'm'
                        && let Ok(seq) = std::str::from_utf8(&bytes[i..j])
                    {
                        out.push_str(seq);
                    }
                    i = j;
                    continue;
                }
                // OSC / DCS / SOS / PM / APC: ESC ] | P | X | ^ | _ ... ST(ESC \) or BEL
                b']' | b'P' | b'X' | b'^' | b'_' => {
                    let mut j = i + 2;
                    while j < bytes.len() {
                        if bytes[j] == 0x07 {
                            j += 1;
                            break;
                        }
                        if bytes[j] == 0x1b && j + 1 < bytes.len() && bytes[j + 1] == b'\\' {
                            j += 2;
                            break;
                        }
                        j += 1;
                    }
                    i = j;
                    continue;
                }
                // Standalone two-byte ESC sequence (RIS, charset selection, etc.)
                _ => {
                    i += 2;
                    continue;
                }
            }
        }
        // Strip lone control bytes that ratatui would otherwise drop (and which
        // mean nothing in transcript output) but keep \n, \r, \t as legitimate
        // formatting.
        let b = bytes[i];
        if b < 0x80 {
            if b < 0x20 && b != b'\n' && b != b'\r' && b != b'\t' {
                i += 1;
                continue;
            }
            out.push(b as char);
            i += 1;
        } else {
            // UTF-8 multi-byte sequence: copy the whole code point intact.
            // Pushing `b as char` would mis-decode it as Latin-1 and mangle
            // non-ASCII text (CJK, accented Latin, emoji, …).
            let len = utf8_seq_len(b);
            let end = (i + len).min(bytes.len());
            if let Ok(chunk) = std::str::from_utf8(&bytes[i..end]) {
                out.push_str(chunk);
            }
            i = end;
        }
    }
}

/// Mask credentials in a URL so it can appear in output or a report.
///
/// Userinfo is replaced with `***` and query values under sensitive keys are
/// masked. A URL that does not parse is returned unchanged.
pub fn redact_url_for_display(url: &str) -> String {
    let Ok(mut parsed) = url::Url::parse(url) else {
        return url.to_string();
    };
    if !parsed.username().is_empty() || parsed.password().is_some() {
        let _ = parsed.set_username("***");
        let _ = parsed.set_password(Some("***"));
    }
    if parsed.query().is_none() {
        return parsed.to_string();
    }
    let pairs: Vec<(String, String)> = parsed
        .query_pairs()
        .map(|(key, value)| {
            let value = if is_sensitive_url_query_key(&key) {
                "***".to_string()
            } else {
                value.into_owned()
            };
            (key.into_owned(), value)
        })
        .collect();
    parsed.set_query(None);
    let mut query = parsed.query_pairs_mut();
    for (key, value) in pairs {
        query.append_pair(&key, &value);
    }
    drop(query);
    parsed.to_string()
}

fn is_sensitive_url_query_key(key: &str) -> bool {
    let normalized = key.trim().replace(['-', '.'], "_").to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "api_key"
            | "apikey"
            | "access_token"
            | "auth_token"
            | "authorization"
            | "bearer"
            | "client_secret"
            | "credential"
            | "id_token"
            | "password"
            | "refresh_token"
            | "secret"
            | "token"
    ) || normalized.ends_with("_api_key")
        || normalized.ends_with("_authorization")
        || normalized.ends_with("_password")
        || normalized.ends_with("_secret")
        || normalized.ends_with("_token")
}

/// True when `role` names an internal, non-user-visible message role.
pub fn is_internal_role(role: &str) -> bool {
    matches!(
        role.trim().to_ascii_lowercase().as_str(),
        "system" | "developer" | "internal"
    )
}

/// True when a JSON/assignment key names a credential-bearing value.
///
/// Classification normalizes separators and quotes so obfuscated variants are
/// still caught; the vocabulary is shared with `/structcopy`.
pub fn is_sensitive_key(key: &str) -> bool {
    let normalized = key
        .trim()
        .trim_matches(['\'', '"'])
        .replace(['-', '.', ' '], "_")
        .to_ascii_lowercase();
    [
        "api_key",
        "apikey",
        "secret",
        "token",
        "password",
        "passwd",
        "authorization",
        "access_key",
        "client_secret",
        "private_key",
        "cookie",
        "session_key",
    ]
    .iter()
    .any(|hint| normalized.contains(hint))
}

/// Sanitize arbitrary text for safe export output.
///
/// Strips ANSI/control bytes, normalizes newlines, then applies private-key,
/// bearer-token, JWT, URL-credential, and keyed-secret redaction in the exact
/// established order.
pub fn sanitize_text(input: &str) -> String {
    let mut visible = String::with_capacity(input.len());
    strip_ansi_into(input, &mut visible);
    let visible = visible.replace("\r\n", "\n").replace('\r', "\n");
    let visible: String = visible
        .chars()
        .filter(|ch| *ch == '\n' || *ch == '\t' || !ch.is_control())
        .collect();
    let private_keys = private_key_regex().replace_all(&visible, "[redacted private key]");
    let bearer = bearer_regex().replace_all(&private_keys, "Bearer [redacted]");
    let jwt = jwt_regex().replace_all(&bearer, "[redacted token]");
    let urls = url_regex().replace_all(&jwt, |captures: &regex::Captures<'_>| {
        redact_url_match(captures.get(0).map_or("", |value| value.as_str()))
    });
    redact_secrets(&urls)
}

/// Recursively redact a JSON value.
///
/// A value under a sensitive key is replaced wholesale; other strings pass
/// through [`sanitize_text`], and arrays/objects are traversed in place.
pub fn redact_json(value: &mut Value, key: Option<&str>) {
    if key.is_some_and(is_sensitive_key) {
        *value = Value::String("[redacted]".to_string());
        return;
    }
    match value {
        Value::String(text) => *text = sanitize_text(text),
        Value::Array(items) => {
            for item in items {
                redact_json(item, None);
            }
        }
        Value::Object(map) => {
            for (key, value) in map {
                redact_json(value, Some(key));
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

/// Collapse whitespace and neutralize inline backticks for a single-line
/// export field.
pub fn inline_text(input: &str) -> String {
    sanitize_text(input)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('`', "'")
}

fn redact_url_match(raw: &str) -> String {
    let trimmed = raw.trim_end_matches(['.', ',', ';', '!']);
    let suffix = &raw[trimmed.len()..];
    format!("{}{}", redact_url_for_display(trimmed), suffix)
}

fn private_key_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?is)-----BEGIN [^-\r\n]*PRIVATE KEY-----.*?-----END [^-\r\n]*PRIVATE KEY-----",
        )
        .expect("private-key redaction regex")
    })
}

fn bearer_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\bbearer\s+[a-z0-9._~+/=-]{6,}").expect("bearer redaction regex")
    })
}

fn jwt_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\beyJ[a-zA-Z0-9_-]{5,}\.[a-zA-Z0-9_-]{5,}(?:\.[a-zA-Z0-9_-]{5,})?\b")
            .expect("JWT redaction regex")
    })
}

fn url_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"https?://[^\s<>\"'`\]\[\)\(\}\{]+"#).expect("URL redaction regex")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_ansi_removes_control_sequences_but_keeps_text() {
        let mut out = String::new();
        strip_ansi_into("a\u{1b}[31mred\u{1b}[0m b", &mut out);
        assert_eq!(out, "ared b");
    }

    #[test]
    fn url_redaction_masks_userinfo_and_sensitive_query_values() {
        assert_eq!(
            redact_url_for_display(
                "https://alice:password@example.com/path?token=very-secret&ok=1"
            ),
            "https://***:***@example.com/path?token=***&ok=1"
        );
    }

    #[test]
    fn sensitive_keys_normalize_separators_and_quotes() {
        assert!(is_sensitive_key("API-KEY"));
        assert!(is_sensitive_key("\"client secret\""));
        assert!(!is_sensitive_key("monkey"));
    }

    #[test]
    fn sanitize_text_redacts_private_keys_bearer_and_jwt() {
        // Assemble the PEM markers and the provider-token prefix at runtime so
        // this source file never contains a literal private-key header (or a
        // literal token) for a secret scanner to match. The runtime strings are
        // identical to the real shapes, and this mirrors the convention the
        // pre-move test used in `config::persistence` - moving that test into
        // this crate silently dropped it, which is what GitGuardian caught.
        let begin = ["-----BEGIN RSA", " PRIVATE KEY-----"].concat();
        let end = ["-----END RSA", " PRIVATE KEY-----"].concat();
        let bearer = format!("{} abcdefghijklmnop", "Bearer");
        let opaque = ["sk-", "abcdef1234567890"].concat();
        let text = format!("{begin}\nMII\n{end}\nAuthorization: {bearer}\n{opaque}");

        let out = sanitize_text(&text);
        assert!(!out.contains("MII"), "{out}");
        assert!(!out.contains("abcdefghijklmnop"), "{out}");
        assert!(out.contains("[redacted]"), "{out}");
    }
}
