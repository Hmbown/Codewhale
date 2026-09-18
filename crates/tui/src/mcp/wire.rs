//! MCP wire-format helpers shared by the HTTP, SSE, streamable-HTTP, and
//! stdio transports: frame/response size ceilings, SSE event framing and
//! field parsing, and the error-text classifiers that decide whether a
//! failure is a stale session or a closed connection.
/// Hard ceiling on the SSE frame-assembly buffer. A server that never emits a
/// frame separator would otherwise grow it without bound (OOM DoS).
pub(super) const MAX_SSE_FRAME_BYTES: usize = 8 * 1024 * 1024;

/// Hard ceiling on a single MCP HTTP response body / stdio line. A misbehaving
/// or malicious server could otherwise stream an unbounded body (or a
/// newline-free multi-GB "line") and OOM the process at transport-read time,
/// before any transcript-level spillover applies.
pub(super) const MAX_MCP_RESPONSE_BYTES: usize = 16 * 1024 * 1024;

pub(super) fn is_mcp_stale_session_body(body: &str) -> bool {
    let body = body.to_ascii_lowercase();
    body.contains("session") && (body.contains("expired") || body.contains("invalid"))
}

/// A tool call worth replaying after drop→reconnect: either the server
/// rejected the session id, or the transport itself is gone (dead
/// pipe/socket) rather than merely idle.
pub(super) fn is_retriable_mcp_call_error(err: &anyhow::Error) -> bool {
    if is_mcp_stale_session_error(err) {
        return true;
    }
    let lower = format!("{err:#}").to_ascii_lowercase();
    is_connection_closed_error_text(&lower)
}

pub(super) fn is_mcp_stale_session_error(err: &anyhow::Error) -> bool {
    let err = format!("{err:#}");
    let lower_err = err.to_ascii_lowercase();
    err.contains("MCP Streamable HTTP session expired")
        || err.contains("MCP session expired")
        || err.contains("SSE transport closed")
        // The exact bail text of a stdio transport whose child died (the
        // EOF arm of `StdioTransport::recv`); without this arm a dead-child
        // error missed the drop→reconnect→retry path that SSE closes get.
        || err.contains("Stdio transport closed")
        || (err.contains("MCP SSE POST send failed") && is_connection_closed_error_text(&lower_err))
        || is_mcp_stale_session_body(&err)
}

pub(super) fn is_connection_closed_error_text(err: &str) -> bool {
    err.contains("connection closed")
        || err.contains("connection reset")
        || err.contains("broken pipe")
        || err.contains("unexpected eof")
        || err.contains("forcibly closed")
}

pub(super) fn parse_sse_message_data(body: &str) -> Vec<Vec<u8>> {
    let normalized = body.replace("\r\n", "\n");
    let mut messages = Vec::new();

    for block in normalized.split("\n\n") {
        let mut event_type = "message";
        let mut data = String::new();

        for line in block.lines() {
            if let Some(value) = sse_field_value(line, "event:") {
                event_type = value;
            } else if let Some(value) = sse_field_value(line, "data:") {
                if !data.is_empty() {
                    data.push('\n');
                }
                data.push_str(value);
            }
        }

        if event_type != "message" || data.trim().is_empty() {
            continue;
        }

        messages.push(data.trim().as_bytes().to_vec());
    }

    messages
}

// Retained for tests; the SSE transport now uses the byte-oriented twin.
#[cfg(test)]
pub(super) fn find_sse_event_separator(buffer: &str) -> Option<(usize, usize)> {
    match (buffer.find("\n\n"), buffer.find("\r\n\r\n")) {
        (Some(lf), Some(crlf)) if crlf < lf => Some((crlf, 4)),
        (Some(lf), _) => Some((lf, 2)),
        (_, Some(crlf)) => Some((crlf, 4)),
        _ => None,
    }
}

/// Byte-oriented twin of `find_sse_event_separator`. Used by the SSE
/// transport so it can accumulate RAW bytes and decode only complete event
/// blocks — a multi-byte UTF-8 char split across two network reads is never
/// corrupted to U+FFFD (the `\n`/`\r` separators are ASCII and can never fall
/// inside a multi-byte sequence).
pub(super) fn find_sse_event_separator_bytes(buffer: &[u8]) -> Option<(usize, usize)> {
    let lf = buffer.windows(2).position(|w| w == b"\n\n");
    let crlf = buffer.windows(4).position(|w| w == b"\r\n\r\n");
    match (lf, crlf) {
        (Some(lf), Some(crlf)) if crlf < lf => Some((crlf, 4)),
        (Some(lf), _) => Some((lf, 2)),
        (_, Some(crlf)) => Some((crlf, 4)),
        _ => None,
    }
}

pub(super) fn sse_field_value<'a>(line: &'a str, field: &str) -> Option<&'a str> {
    let value = line.strip_prefix(field)?;
    Some(value.strip_prefix(' ').unwrap_or(value))
}
