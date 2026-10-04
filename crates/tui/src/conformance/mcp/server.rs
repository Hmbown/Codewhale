//! The loopback Streamable HTTP transcript server for the `mcp` family.
//!
//! One server per case, scripted by the case's `server` object. It speaks
//! just enough HTTP for any MCP client to connect to it by URL, so a child
//! Node process can be pointed at it exactly like at a user's server.
//!
//! Per-method answers (`initialize`, `tools/list`, `resources/list`,
//! `resources/templates/list`, `prompts/list`, `tools/call`,
//! `resources/read`, `prompts/get`): either one entry, or an array of
//! candidate entries. For an array the first candidate whose `match` fields
//! all equal the request params wins (no `match` matches anything), and a
//! candidate marked `"once": true` is skipped after it has answered once —
//! that is how a case scripts "the first `tools/list` differs from the
//! second". Unknown methods get JSON-RPC `-32601`.
//!
//! An entry answers with exactly one of:
//!
//! - `result` / `error`: a JSON-RPC reply. `progress` (progress
//!   notifications) and `notifications` (arbitrary notification objects) are
//!   sent as SSE events ahead of the reply, which then becomes SSE too.
//! - `hold`: never answer until the client goes away (or [`HOLD_LIMIT`]).
//! - `http`: a raw HTTP response, `{status, headers, body}`, instead of a
//!   JSON-RPC one. `{{authorization}}` in `body` is replaced by the request's
//!   `Authorization` header value (the "server echoes your credential" case),
//!   and `{{sink_url}}` anywhere in the spec by the case's sink server URL.
//! - `expire_session`: the server forgets the current session and answers
//!   `404`, without running the request.
//! - `generate_tools` `{count, prefix}` / `generate_pages` `{count, prefix}`:
//!   a `tools/list` result built at request time, so a cap-sized catalog does
//!   not have to live in a fixture. `generate_pages` serves one tool per page
//!   and links pages with `nextCursor` `page-N`.
//!
//! [`ServerOptions`] adds server-wide behaviour: numbered sessions (the
//! `initialize` reply issues `session-1`, `session-2`, … and every later
//! request must carry the latest one, else `404`) and a required bearer token
//! (anything else is `401`).
//!
//! What the server records, for the golden:
//!
//! - `received`: side-effecting requests it actually dispatched
//!   (`tools/call`, `resources/read`, `prompts/get`). A request it refused at
//!   the HTTP layer (stale session) is not here: the server never ran it.
//! - `requests`: every POST, as `{rpc, session, protocol_version,
//!   authorization, status}`. The `Authorization` value is recorded only as a
//!   shape (`<bearer>` / `<other>`), never as bytes, so a recorded frame
//!   cannot carry a credential.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Notify;

/// A held `tools/call` is released after this long even if nobody cancels,
/// so a dispatch that ignores cancellation fails the case instead of hanging.
pub(super) const HOLD_LIMIT: Duration = Duration::from_secs(20);

/// Server-wide behaviour a case opts into (`server_options`).
#[derive(Clone, Default)]
pub(super) struct ServerOptions {
    /// `initialize` issues `session-N`; every later request must carry the
    /// latest one or is refused with `404`.
    pub(super) numbered_sessions: bool,
    /// Every request must carry `Authorization: Bearer <token>`, else `401`.
    pub(super) require_bearer: Option<String>,
}

pub(super) struct State {
    spec: Value,
    options: ServerOptions,
    received: Mutex<Vec<Value>>,
    requests: Mutex<Vec<Value>>,
    /// Signalled when a `hold` entry starts holding a request.
    pub(super) held: Notify,
    sessions_issued: AtomicUsize,
    current_session: Mutex<Option<String>>,
    used: Mutex<HashSet<(String, usize)>>,
}

pub(super) struct TranscriptServer {
    pub(super) url: String,
    pub(super) addr: String,
    pub(super) state: Arc<State>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for TranscriptServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl TranscriptServer {
    pub(super) async fn start(spec: Value, options: ServerOptions) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind transcript server");
        let addr = listener.local_addr().expect("addr").to_string();
        let url = format!("http://{addr}/mcp");
        let state = Arc::new(State {
            spec,
            options,
            received: Mutex::new(Vec::new()),
            requests: Mutex::new(Vec::new()),
            held: Notify::new(),
            sessions_issued: AtomicUsize::new(0),
            current_session: Mutex::new(None),
            used: Mutex::new(HashSet::new()),
        });
        let served = Arc::clone(&state);
        let task = tokio::spawn(async move {
            let mut connections = tokio::task::JoinSet::new();
            loop {
                let accepted = tokio::select! {
                    accepted = listener.accept() => accepted,
                    _ = connections.join_next(), if !connections.is_empty() => continue,
                };
                let Ok((socket, _)) = accepted else {
                    break;
                };
                let state = Arc::clone(&served);
                connections.spawn(async move {
                    answer(socket, &state).await;
                });
            }
        });
        Self {
            url,
            addr,
            state,
            task,
        }
    }

    pub(super) fn received(&self) -> Vec<Value> {
        self.state.received.lock().expect("log").clone()
    }

    pub(super) fn requests(&self) -> Vec<Value> {
        self.state.requests.lock().expect("log").clone()
    }
}

struct Request {
    method: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

async fn read_request(socket: tokio::net::TcpStream) -> Option<(Request, tokio::net::TcpStream)> {
    let mut reader = BufReader::new(socket);
    let mut line = String::new();
    if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
        return None;
    }
    let method = line
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_string();
    let mut headers = HashMap::new();
    loop {
        line.clear();
        if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
            return None;
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }
    let content_length = headers
        .get("content-length")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0usize);
    let mut body = vec![0u8; content_length];
    reader.read_exact(&mut body).await.ok()?;
    Some((
        Request {
            method,
            headers,
            body,
        },
        reader.into_inner(),
    ))
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        202 => "Accepted",
        307 => "Temporary Redirect",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        405 => "Method Not Allowed",
        500 => "Internal Server Error",
        _ => "Status",
    }
}

struct Reply {
    status: u16,
    headers: Vec<(String, String)>,
    body: String,
}

impl Reply {
    fn new(status: u16, body: impl Into<String>) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: body.into(),
        }
    }

    fn header(mut self, name: &str, value: impl Into<String>) -> Self {
        self.headers.push((name.to_string(), value.into()));
        self
    }

    fn bytes(&self) -> Vec<u8> {
        let mut head = format!("HTTP/1.1 {} {}\r\n", self.status, reason(self.status));
        for (name, value) in &self.headers {
            head.push_str(&format!("{name}: {value}\r\n"));
        }
        head.push_str(&format!(
            "Content-Length: {}\r\nConnection: close\r\n\r\n{}",
            self.body.len(),
            self.body
        ));
        head.into_bytes()
    }
}

/// The shape of an `Authorization` header, never its bytes.
fn authorization_shape(value: Option<&String>) -> Value {
    match value {
        None => Value::Null,
        Some(value) if value.to_ascii_lowercase().starts_with("bearer ") => json!("<bearer>"),
        Some(_) => json!("<other>"),
    }
}

async fn answer(socket: tokio::net::TcpStream, state: &State) {
    let Some((request, mut socket)) = read_request(socket).await else {
        return;
    };
    if request.method != "POST" {
        // No server-initiated stream: the spec's answer for a GET.
        let _ = socket
            .write_all(b"HTTP/1.1 405 Method Not Allowed\r\nAllow: POST\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .await;
        return;
    }
    let Ok(message) = serde_json::from_slice::<Value>(&request.body) else {
        let _ = socket
            .write_all(
                b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .await;
        return;
    };
    let rpc_method = message["method"].as_str().unwrap_or_default().to_string();
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    let session = request.headers.get("mcp-session-id").cloned();
    let authorization = request.headers.get("authorization").cloned();
    let record = |status: u16| {
        state.requests.lock().expect("log").push(json!({
            "rpc": rpc_method,
            "session": session,
            "protocol_version": request.headers.get("mcp-protocol-version"),
            "authorization": authorization_shape(authorization.as_ref()),
            "status": status,
        }));
    };
    let finish = async |socket: &mut tokio::net::TcpStream, reply: Reply| {
        record(reply.status);
        let _ = socket.write_all(&reply.bytes()).await;
        let _ = socket.shutdown().await;
    };

    if let Some(token) = &state.options.require_bearer
        && authorization.as_deref() != Some(format!("Bearer {token}").as_str())
    {
        let reply = Reply::new(401, r#"{"error":"unauthorized"}"#)
            .header("WWW-Authenticate", "Bearer realm=\"conformance\"")
            .header("Content-Type", "application/json");
        finish(&mut socket, reply).await;
        return;
    }
    let is_initialize = rpc_method == "initialize";
    if state.options.numbered_sessions && !is_initialize {
        let current = state.current_session.lock().expect("session").clone();
        if current.is_none() || current != session {
            finish(&mut socket, Reply::new(404, "session not found")).await;
            return;
        }
    }
    let Some(id) = message.get("id").cloned() else {
        // Notifications (initialized, cancelled, progress) are accepted.
        finish(&mut socket, Reply::new(202, "")).await;
        return;
    };

    let entry = scripted_entry(state, &rpc_method, &params);
    if entry.get("expire_session").and_then(Value::as_bool) == Some(true) {
        *state.current_session.lock().expect("session") = None;
        finish(&mut socket, Reply::new(404, "session not found")).await;
        return;
    }
    match rpc_method.as_str() {
        "tools/call" => state.received.lock().expect("log").push(json!({
            "method": "tools/call",
            "name": params["name"],
            "arguments": params.get("arguments").cloned().unwrap_or(Value::Null),
        })),
        "resources/read" => state
            .received
            .lock()
            .expect("log")
            .push(json!({ "method": "resources/read", "uri": params["uri"] })),
        "prompts/get" => state
            .received
            .lock()
            .expect("log")
            .push(json!({ "method": "prompts/get", "name": params["name"] })),
        _ => {}
    }
    if entry.get("hold").and_then(Value::as_bool) == Some(true) {
        state.held.notify_one();
        // Answer nothing until the client goes away (or the hold limit).
        let mut byte = [0u8; 1];
        let _ = tokio::time::timeout(HOLD_LIMIT, socket.read(&mut byte)).await;
        return;
    }
    if let Some(http) = entry.get("http") {
        let status = http["status"].as_u64().unwrap_or(500) as u16;
        let body = http["body"].as_str().unwrap_or_default().replace(
            "{{authorization}}",
            authorization.as_deref().unwrap_or("<none>"),
        );
        let mut reply = Reply::new(status, body);
        if let Some(headers) = http["headers"].as_object() {
            for (name, value) in headers {
                reply = reply.header(name, value.as_str().unwrap_or_default());
            }
        }
        finish(&mut socket, reply).await;
        return;
    }
    let body = match entry.get("error") {
        Some(error) => json!({ "jsonrpc": "2.0", "id": id, "error": error }),
        None => json!({ "jsonrpc": "2.0", "id": id, "result": entry["result"] }),
    };
    let mut reply = if entry.get("progress").is_some() || entry.get("notifications").is_some() {
        let token = params
            .pointer("/_meta/progressToken")
            .cloned()
            .unwrap_or_else(|| json!("conformance"));
        let mut sse = String::new();
        for step in entry
            .get("progress")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let notification = json!({
                "jsonrpc": "2.0",
                "method": "notifications/progress",
                "params": { "progressToken": token, "progress": step["progress"], "total": step["total"] },
            });
            sse.push_str(&format!("event: message\ndata: {notification}\n\n"));
        }
        for notification in entry
            .get("notifications")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            sse.push_str(&format!("event: message\ndata: {notification}\n\n"));
        }
        sse.push_str(&format!("event: message\ndata: {body}\n\n"));
        Reply::new(200, sse).header("Content-Type", "text/event-stream")
    } else {
        Reply::new(200, body.to_string()).header("Content-Type", "application/json")
    };
    if is_initialize {
        let session = if state.options.numbered_sessions {
            let number = state.sessions_issued.fetch_add(1, Ordering::SeqCst) + 1;
            let issued = format!("session-{number}");
            *state.current_session.lock().expect("session") = Some(issued.clone());
            issued
        } else {
            "conformance-session".to_string()
        };
        reply = reply.header("Mcp-Session-Id", session);
    }
    finish(&mut socket, reply).await;
}

/// The transcript's answer for one request: one entry, or the first usable
/// candidate of an array (see the module doc). Unknown methods get -32601.
fn scripted_entry(state: &State, method: &str, params: &Value) -> Value {
    let not_found =
        || json!({ "error": { "code": -32601, "message": format!("method not found: {method}") } });
    let Some(entry) = state.spec.get(method) else {
        return not_found();
    };
    let Some(candidates) = entry.as_array() else {
        return expand_generated(entry.clone(), params);
    };
    for (index, candidate) in candidates.iter().enumerate() {
        let matches = candidate["match"].as_object().is_none_or(|fields| {
            fields
                .iter()
                .all(|(key, value)| params.get(key) == Some(value))
        });
        if !matches {
            continue;
        }
        if candidate.get("once").and_then(Value::as_bool) == Some(true)
            && !state
                .used
                .lock()
                .expect("used")
                .insert((method.to_string(), index))
        {
            continue;
        }
        return expand_generated(candidate.clone(), params);
    }
    json!({ "error": { "code": -32602, "message": format!("no scripted {method} answer for {params}") } })
}

fn generated_tool(prefix: &str, index: usize) -> Value {
    json!({ "name": format!("{prefix}-{index:05}"), "inputSchema": { "type": "object" } })
}

fn expand_generated(mut entry: Value, params: &Value) -> Value {
    if let Some(spec) = entry.get("generate_tools") {
        let count = spec["count"].as_u64().unwrap_or(0) as usize;
        let prefix = spec["prefix"].as_str().unwrap_or("tool");
        let tools: Vec<Value> = (0..count)
            .map(|index| generated_tool(prefix, index))
            .collect();
        entry["result"] = json!({ "tools": tools });
    } else if let Some(spec) = entry.get("generate_pages") {
        let count = spec["count"].as_u64().unwrap_or(1) as usize;
        let prefix = spec["prefix"].as_str().unwrap_or("tool");
        let page = params["cursor"]
            .as_str()
            .and_then(|cursor| cursor.strip_prefix("page-"))
            .and_then(|page| page.parse::<usize>().ok())
            .unwrap_or(1);
        let mut result = json!({ "tools": [generated_tool(prefix, page)] });
        if page < count {
            result["nextCursor"] = json!(format!("page-{}", page + 1));
        }
        entry["result"] = result;
    }
    entry
}
