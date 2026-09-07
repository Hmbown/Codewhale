//! Deterministic JSON argument repair for malformed tool-call inputs.
//!
//! DeepSeek streams `tool_calls.function.arguments` as deltas. Two failure
//! shapes are common: (a) SSE chunk boundary cuts inside a JSON string and
//! reassembly leaves a trailing comma or unclosed brace; (b) some local
//! backends emit literal control characters inside JSON string values.
//!
//! The repair ladder runs five stages before reporting unrecoverable input:
//!
//!  1. Strict parse — done if it parses.
//!  2. Strip literal control chars inside string values.
//!  3. Strip trailing commas before `}` or `]`.
//!  4. Balance braces/brackets (append closers).
//!  5. Strip excess closers if delta is negative.
//!
//! Stages 1-3 never change structure: they parse as-is, or normalize text
//! that was already structurally complete. Stages 4-5 do — they synthesize
//! or discard closers to force a parse. A value that only parsed because of
//! stage 4 or 5 came from argument text that was *incomplete*, and the usual
//! cause is a provider cutting the stream at its output limit mid-argument.
//! `Repaired::structure_synthesized` reports that, because such a value must
//! never be dispatched as if the model had finished writing it.

use serde_json::Value;

/// Maximum raw argument length we'll attempt to repair (1 MiB).
const MAX_ARG_LEN: usize = 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum ArgRepairError {
    #[error("argument exceeded {0} chars; refusing to repair")]
    TooLarge(usize),
    #[error("argument could not be repaired into valid JSON")]
    Unrepairable,
}

/// Repair a raw JSON argument string into a valid `serde_json::Value`.
///
/// Runs the deterministic ladder; on success returns the parsed value.
/// A repaired value plus whether the repair had to invent structure.
#[derive(Debug, Clone)]
pub struct Repaired {
    pub value: Value,
    /// True when the text only parsed after closers were appended (stage 4)
    /// or discarded (stage 5) — i.e. the argument text was structurally
    /// incomplete. Callers making the *final* dispatch decision must treat
    /// this as malformed input rather than executing it.
    pub structure_synthesized: bool,
}

impl Repaired {
    fn intact(value: Value) -> Self {
        Self {
            value,
            structure_synthesized: false,
        }
    }
    fn synthesized(value: Value) -> Self {
        Self {
            value,
            structure_synthesized: true,
        }
    }
}

pub fn repair(raw: &str) -> Result<Repaired, ArgRepairError> {
    if raw.len() > MAX_ARG_LEN {
        return Err(ArgRepairError::TooLarge(raw.len()));
    }
    // Stage 1: strict parse
    if let Ok(v) = serde_json::from_str(raw) {
        return Ok(Repaired::intact(v));
    }
    // Stage 2: strip control chars inside strings
    let mut s = strip_control_chars_in_strings(raw);
    if let Ok(v) = serde_json::from_str(&s) {
        return Ok(Repaired::intact(v));
    }
    // Stage 3: strip trailing commas
    s = strip_trailing_commas(&s);
    if let Ok(v) = serde_json::from_str(&s) {
        return Ok(Repaired::intact(v));
    }
    // Stage 4: balance braces
    // Stages 4 and 5 change structure. Anything they rescue is reported as
    // synthesized: `balance_braces` counts braces without tracking string
    // literals, so a stream cut at the end of a complete string value yields
    // JSON that parses cleanly and is still missing whatever the model had
    // not written yet.
    s = balance_braces(&s, 50);
    if let Ok(v) = serde_json::from_str(&s) {
        return Ok(Repaired::synthesized(v));
    }
    // Stage 5: strip excess closers
    s = strip_excess_closers(&s);
    if let Ok(v) = serde_json::from_str(&s) {
        return Ok(Repaired::synthesized(v));
    }
    Err(ArgRepairError::Unrepairable)
}

/// Strip ASCII control characters (0x00–0x1F except \t, \n, \r) that appear
/// inside JSON string values. We walk character-by-character tracking whether
/// we're inside a string (between unescaped double-quotes).
fn strip_control_chars_in_strings(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_string = false;
    let mut escape = false;
    for ch in s.chars() {
        if escape {
            out.push(ch);
            escape = false;
            continue;
        }
        if ch == '\\' {
            escape = true;
            out.push(ch);
            continue;
        }
        if ch == '"' {
            in_string = !in_string;
            out.push(ch);
            continue;
        }
        if in_string && (ch as u32) < 0x20 && ch != '\t' && ch != '\n' && ch != '\r' {
            // Drop control characters inside strings
            continue;
        }
        out.push(ch);
    }
    out
}

/// Strip trailing commas before `}` or `]`.
fn strip_trailing_commas(s: &str) -> String {
    // Repeatedly replace ",}" and ",]" until stable (handles nested cases).
    let mut out = s.to_string();
    loop {
        let prev = out.clone();
        out = out.replace(",}", "}").replace(",]", "]");
        // Handle trailing comma at end of string
        out = out.trim_end_matches(',').to_string();
        if out == prev {
            break;
        }
    }
    out
}

/// Balance braces and brackets: count `{`/`}` and `[`/`]`, append closers if
/// positive delta (more opens than closes). Caps iterations so a
/// catastrophically broken input doesn't loop forever.
fn balance_braces(s: &str, max_iter: usize) -> String {
    let mut out = s.to_string();
    for _ in 0..max_iter {
        let brace_delta: i32 = out
            .chars()
            .map(|ch| match ch {
                '{' => 1,
                '}' => -1,
                _ => 0,
            })
            .sum();
        let bracket_delta: i32 = out
            .chars()
            .map(|ch| match ch {
                '[' => 1,
                ']' => -1,
                _ => 0,
            })
            .sum();
        if brace_delta <= 0 && bracket_delta <= 0 {
            break;
        }
        // Append needed closers in reverse order (brackets before braces
        // for correct nesting when both are unbalanced).
        for _ in 0..bracket_delta.max(0) {
            out.push(']');
        }
        for _ in 0..brace_delta.max(0) {
            out.push('}');
        }
    }
    out
}

/// Strip excess closers when the delta is negative (more closes than opens).
fn strip_excess_closers(s: &str) -> String {
    let mut brace_depth: i32 = 0;
    let mut bracket_depth: i32 = 0;
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '}' => {
                if brace_depth > 0 {
                    brace_depth -= 1;
                    out.push(ch);
                }
                // else drop excess closer
            }
            ']' => {
                if bracket_depth > 0 {
                    bracket_depth -= 1;
                    out.push(ch);
                }
            }
            '{' => {
                brace_depth += 1;
                out.push(ch);
            }
            '[' => {
                bracket_depth += 1;
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn strict_parse_passes_through() {
        let r = repair(r#"{"path": "hello.txt"}"#).unwrap();
        assert_eq!(r.value, json!({"path": "hello.txt"}));
        assert!(!r.structure_synthesized);
    }

    #[test]
    fn repairs_trailing_comma() {
        let r = repair(r#"{"path": "hello.txt",}"#).unwrap();
        assert_eq!(r.value, json!({"path": "hello.txt"}));
        // Structurally complete, just sloppy — must stay dispatchable.
        assert!(!r.structure_synthesized);
    }

    #[test]
    fn repairs_trailing_comma_in_array() {
        let r = repair(r#"["a", "b",]"#).unwrap();
        assert_eq!(r.value, json!(["a", "b"]));
        assert!(!r.structure_synthesized);
    }

    #[test]
    fn repairs_missing_close_brace() {
        let r = repair(r#"{"path": "hello.txt""#).unwrap();
        assert_eq!(r.value, json!({"path": "hello.txt"}));
        assert!(r.structure_synthesized);
    }

    #[test]
    fn repairs_missing_close_bracket() {
        let r = repair(r#"["a", "b""#).unwrap();
        assert_eq!(r.value, json!(["a", "b"]));
        assert!(r.structure_synthesized);
    }

    #[test]
    fn strips_embedded_control_chars() {
        // Raw \x0B (vertical tab) inside a string value
        let raw = "{\"key\": \"val\x0Bue\"}";
        let v = repair(raw).unwrap();
        assert_eq!(v.value, json!({"key": "value"}));
        assert!(!v.structure_synthesized);
    }

    #[test]
    fn rejects_empty_string() {
        assert!(matches!(repair(""), Err(ArgRepairError::Unrepairable)));
    }

    #[test]
    fn rejects_gibberish() {
        assert!(matches!(
            repair("not json at all"),
            Err(ArgRepairError::Unrepairable)
        ));
    }

    #[test]
    fn balances_nested_braces() {
        let r = repair(r#"{"outer": {"inner": "val""#).unwrap();
        assert_eq!(r.value, json!({"outer": {"inner": "val"}}));
        // Closers were appended, so this is a truncated argument, not a
        // complete one that merely needed tidying.
        assert!(r.structure_synthesized);
    }

    #[test]
    fn strips_excess_closers() {
        let r = repair(r#"{"key": "val"}}"#).unwrap();
        assert_eq!(r.value, json!({"key": "val"}));
        assert!(r.structure_synthesized);
    }

    #[test]
    fn handles_double_encoded_json() {
        // This is a valid JSON string containing a JSON object literal.
        // repair parses it as a string; the engine's existing fallback
        // (parse_tool_input) will unwrap the string and re-parse.
        let r = repair(r#""{\"path\": \"hello.txt\"}""#).unwrap();
        assert_eq!(
            r.value,
            Value::String(r#"{"path": "hello.txt"}"#.to_string())
        );
        assert!(!r.structure_synthesized);
    }

    #[test]
    fn oversize_input_rejected() {
        let big = "x".repeat(MAX_ARG_LEN + 1);
        assert!(repair(&big).is_err());
    }

    #[test]
    fn a_write_cut_at_a_string_boundary_is_reported_as_synthesized() {
        // The defect this flag exists for: `balance_braces` counts braces
        // without tracking string literals, so a provider that cuts the
        // stream at its output limit right after a complete string value
        // yields text that parses cleanly once one `}` is appended. Nothing
        // downstream could previously tell this from a finished argument, so
        // the truncated `content` was written to the user's file.
        let cut = r#"{"path": "notes.md", "content": "first line""#;
        let r = repair(cut).unwrap();
        assert_eq!(
            r.value,
            json!({"path": "notes.md", "content": "first line"}),
            "the ladder still parses it — that is exactly why the flag is needed"
        );
        assert!(
            r.structure_synthesized,
            "a truncated write must be reported as synthesized so dispatch refuses it"
        );
    }

    #[test]
    fn a_complete_argument_needing_only_control_char_stripping_stays_intact() {
        // Stage 2 normalizes text that was already structurally complete, so
        // it must NOT be flagged — otherwise every DeepSeek chunk-boundary
        // repair would start failing tool calls that are perfectly fine.
        let r = repair("{\"a\": \"line\u{0008}break\"}").unwrap();
        assert_eq!(r.value, json!({"a": "linebreak"}));
        assert!(!r.structure_synthesized);
    }

    #[test]
    fn repairs_brace_balance_with_trailing_comma() {
        let r = repair(r#"{"a": 1,"#).unwrap();
        assert_eq!(r.value, json!({"a": 1}));
        assert!(r.structure_synthesized);
    }
}
