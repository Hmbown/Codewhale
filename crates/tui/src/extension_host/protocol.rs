//! Extension-host protocol v1 — the phase-1 subset, and its source of truth.
//!
//! Frame: 4-byte magic `CWX1`, u32 little-endian payload length, UTF-8 JSON.
//! Envelope: JSON-RPC 2.0. A length prefix (not NDJSON) makes a plugin that
//! writes raw bytes to the channel detectable: bad magic or length ends the
//! host instead of desynchronising it.
//!
//! Host→core types use `deny_unknown_fields` — host output is untrusted
//! input. Core→host types are what this side writes. The TypeScript mirror is
//! hand-written in phase 1 (`crates/tui/extension-host/src/protocol.ts`); both
//! parse the shared corpus in `tests/fixtures/extension_host/protocol`.
//!
//! There is deliberately no method that expresses approval, and nothing a
//! host can send makes the core *do* anything in phase 1: registrations are
//! admitted or refused, and tool calls only flow core→host after the gate.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value, json};
use tokio::io::{AsyncRead, AsyncReadExt};

pub const PROTOCOL_VERSION: u32 = 1;
pub const MAGIC: [u8; 4] = *b"CWX1";
pub const HEADER_LEN: usize = 8;
/// Enough for base64 screenshots later; anything larger is refused, never truncated.
pub const MAX_FRAME: usize = 32 * 1024 * 1024;
/// Requests in flight per direction.
pub const MAX_INFLIGHT: usize = 256;

/// JSON-RPC error codes used on this channel.
pub mod error_code {
    pub const INVALID_PARAMS: i64 = -32602;
    /// Unknown, revoked, or not-yet-active handle.
    pub const NOT_AVAILABLE: i64 = -32001;
    /// Cancelled by `$/cancel`.
    pub const CANCELLED: i64 = -32800;
}

#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    #[error("bad frame magic")]
    BadMagic,
    #[error("frame of {0} bytes exceeds MAX_FRAME")]
    TooLarge(usize),
    #[error("frame payload is not JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("channel read failed: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct ProtocolError(pub String);

fn perr(message: impl Into<String>) -> ProtocolError {
    ProtocolError(message.into())
}

/// Encode one message into a `CWX1` frame.
pub fn encode_frame(message: &Value) -> Result<Vec<u8>, FrameError> {
    let payload = serde_json::to_vec(message)?;
    if payload.len() > MAX_FRAME {
        return Err(FrameError::TooLarge(payload.len()));
    }
    let mut frame = Vec::with_capacity(HEADER_LEN + payload.len());
    frame.extend_from_slice(&MAGIC);
    frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

/// Read one frame. `Ok(None)` is a clean EOF at a frame boundary. The payload
/// buffer is allocated only after the length is checked against `MAX_FRAME`.
pub async fn read_frame<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Option<Value>, FrameError> {
    let mut header = [0u8; HEADER_LEN];
    let mut filled = 0;
    while filled < HEADER_LEN {
        let read = reader.read(&mut header[filled..]).await?;
        if read == 0 {
            if filled == 0 {
                return Ok(None);
            }
            return Err(FrameError::Io(std::io::ErrorKind::UnexpectedEof.into()));
        }
        filled += read;
    }
    if header[..4] != MAGIC {
        return Err(FrameError::BadMagic);
    }
    let length = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
    if length > MAX_FRAME {
        return Err(FrameError::TooLarge(length));
    }
    let mut payload = vec![0u8; length];
    reader.read_exact(&mut payload).await?;
    Ok(Some(serde_json::from_slice(&payload)?))
}

/// `Some(value)` even for an explicit JSON `null`, so `"result": null` is a
/// present result and round-trips.
fn present<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(deserializer).map(Some)
}

// ---------------------------------------------------------------------------
// Shared types
// ---------------------------------------------------------------------------

/// Identity of one activation. `generation` bumps on every (re)activation;
/// `owner_token` is 128+ random bits minted by the core and handed over only
/// in `ext/activate`. It catches bugs and stale fibers; it is not a boundary
/// against a malicious plugin in the same process (see the design, §4.4).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerRef {
    pub plugin_id: String,
    pub generation: u64,
    pub owner_token: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RpcErrorWire {
    pub code: i64,
    pub message: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub data: Option<Value>,
}

// ---------------------------------------------------------------------------
// Host → core (strict)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProtocolRange {
    pub min: u32,
    pub max: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HelloParams {
    pub protocol: ProtocolRange,
    pub host_version: String,
    pub bundle_sha256: String,
    pub node_version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmptyParams {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegisterKind {
    /// The only kind in phase 1.
    Tool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolSpecWire {
    pub name: String,
    pub description: String,
    pub input_schema: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegisterParams {
    pub owner: OwnerRef,
    pub kind: RegisterKind,
    pub spec: ToolSpecWire,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnregisterParams {
    pub owner: OwnerRef,
    pub handle: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FaultedParams {
    pub owner: OwnerRef,
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogParams {
    pub level: String,
    pub msg: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CancelParams {
    pub id: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HostRequest {
    Register(RegisterParams),
    Unregister(UnregisterParams),
}

#[derive(Debug, Clone, PartialEq)]
pub enum HostNotification {
    Hello(HelloParams),
    Ready,
    Faulted(FaultedParams),
    Log(LogParams),
    Cancel(CancelParams),
}

/// One decoded, validated host→core message.
#[derive(Debug, Clone, PartialEq)]
pub enum HostMessage {
    Request {
        id: u64,
        request: HostRequest,
    },
    Notification(HostNotification),
    Response {
        id: u64,
        outcome: Result<Value, RpcErrorWire>,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    jsonrpc: String,
    #[serde(default)]
    id: Option<u64>,
    #[serde(default)]
    method: Option<String>,
    #[serde(default, deserialize_with = "present")]
    params: Option<Value>,
    #[serde(default, deserialize_with = "present")]
    result: Option<Value>,
    #[serde(default)]
    error: Option<RpcErrorWire>,
}

fn decode_envelope(value: Value) -> Result<Envelope, ProtocolError> {
    let envelope: Envelope =
        serde_json::from_value(value).map_err(|e| perr(format!("envelope: {e}")))?;
    if envelope.jsonrpc != "2.0" {
        return Err(perr("envelope: jsonrpc must be \"2.0\""));
    }
    Ok(envelope)
}

fn params<T: DeserializeOwned>(method: &str, params: Option<Value>) -> Result<T, ProtocolError> {
    serde_json::from_value(params.unwrap_or_else(|| json!({})))
        .map_err(|e| perr(format!("{method}: {e}")))
}

fn expect_request(method: &str, id: Option<u64>) -> Result<u64, ProtocolError> {
    id.ok_or_else(|| perr(format!("`{method}` must be a request (with id)")))
}

fn expect_notification(method: &str, id: Option<u64>) -> Result<(), ProtocolError> {
    match id {
        None => Ok(()),
        Some(_) => Err(perr(format!("`{method}` must be a notification (no id)"))),
    }
}

fn decode_response(
    envelope: Envelope,
) -> Result<(u64, Result<Value, RpcErrorWire>), ProtocolError> {
    let id = envelope.id.ok_or_else(|| perr("response: missing id"))?;
    if envelope.params.is_some() {
        return Err(perr("response: unexpected params"));
    }
    match (envelope.result, envelope.error) {
        (Some(result), None) => Ok((id, Ok(result))),
        (None, Some(error)) => Ok((id, Err(error))),
        _ => Err(perr("response: needs exactly one of result or error")),
    }
}

/// Parse and strictly validate one host→core message.
pub fn parse_host_message(value: Value) -> Result<HostMessage, ProtocolError> {
    let envelope = decode_envelope(value)?;
    let Some(method) = envelope.method.clone() else {
        let (id, outcome) = decode_response(envelope)?;
        return Ok(HostMessage::Response { id, outcome });
    };
    if envelope.result.is_some() || envelope.error.is_some() {
        return Err(perr(format!(
            "`{method}`: a request carries no result or error"
        )));
    }
    let id = envelope.id;
    let p = envelope.params;
    let message = match method.as_str() {
        "registry/register" => HostMessage::Request {
            id: expect_request(&method, id)?,
            request: HostRequest::Register(params(&method, p)?),
        },
        "registry/unregister" => HostMessage::Request {
            id: expect_request(&method, id)?,
            request: HostRequest::Unregister(params(&method, p)?),
        },
        "host/hello" => {
            expect_notification(&method, id)?;
            HostMessage::Notification(HostNotification::Hello(params(&method, p)?))
        }
        "host/ready" => {
            expect_notification(&method, id)?;
            let _: EmptyParams = params(&method, p)?;
            HostMessage::Notification(HostNotification::Ready)
        }
        "ext/faulted" => {
            expect_notification(&method, id)?;
            HostMessage::Notification(HostNotification::Faulted(params(&method, p)?))
        }
        "log" => {
            expect_notification(&method, id)?;
            HostMessage::Notification(HostNotification::Log(params(&method, p)?))
        }
        "$/cancel" => {
            expect_notification(&method, id)?;
            HostMessage::Notification(HostNotification::Cancel(params(&method, p)?))
        }
        other => return Err(perr(format!("unknown host_to_core method `{other}`"))),
    };
    Ok(message)
}

fn request_value(id: u64, method: &str, params: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
}

fn notification_value(method: &str, params: Value) -> Value {
    json!({"jsonrpc": "2.0", "method": method, "params": params})
}

fn response_value(id: u64, outcome: &Result<Value, RpcErrorWire>) -> Value {
    match outcome {
        Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
        Err(error) => json!({"jsonrpc": "2.0", "id": id, "error": error}),
    }
}

fn to_value<T: Serialize>(value: &T) -> Value {
    serde_json::to_value(value).expect("protocol types serialize")
}

#[cfg(test)]
impl HostMessage {
    /// Re-encode (used by the conformance corpus round-trip).
    #[must_use]
    pub fn to_value(&self) -> Value {
        match self {
            Self::Request { id, request } => match request {
                HostRequest::Register(p) => request_value(*id, "registry/register", to_value(p)),
                HostRequest::Unregister(p) => {
                    request_value(*id, "registry/unregister", to_value(p))
                }
            },
            Self::Notification(notification) => match notification {
                HostNotification::Hello(p) => notification_value("host/hello", to_value(p)),
                HostNotification::Ready => notification_value("host/ready", json!({})),
                HostNotification::Faulted(p) => notification_value("ext/faulted", to_value(p)),
                HostNotification::Log(p) => notification_value("log", to_value(p)),
                HostNotification::Cancel(p) => notification_value("$/cancel", to_value(p)),
            },
            Self::Response { id, outcome } => response_value(*id, outcome),
        }
    }
}

// ---------------------------------------------------------------------------
// Core → host (written by this side)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostLimits {
    pub max_frame: u64,
    pub max_inflight: u64,
    pub dispose_deadline_ms: u64,
    pub activate_deadline_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InitializeParams {
    pub protocol: u32,
    pub limits: HostLimits,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntryRef {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActivateParams {
    pub owner: OwnerRef,
    pub plugin_name: String,
    pub entry: EntryRef,
    #[serde(default = "empty_object")]
    pub config: Value,
}

fn empty_object() -> Value {
    json!({})
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeactivateParams {
    pub owner: OwnerRef,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCallParams {
    pub handle: u64,
    pub call_id: String,
    pub input: Value,
    pub deadline_ms: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CoreRequest {
    Initialize(InitializeParams),
    /// Production relies on stdin EOF at process exit (the host is shared by
    /// every engine in the process); the bounded shutdown is test-driven.
    #[cfg(test)]
    Shutdown,
    Activate(ActivateParams),
    Deactivate(DeactivateParams),
    ToolCall(ToolCallParams),
}

impl CoreRequest {
    #[must_use]
    pub fn method(&self) -> &'static str {
        match self {
            Self::Initialize(_) => "host/initialize",
            #[cfg(test)]
            Self::Shutdown => "host/shutdown",
            Self::Activate(_) => "ext/activate",
            Self::Deactivate(_) => "ext/deactivate",
            Self::ToolCall(_) => "tool/call",
        }
    }

    #[must_use]
    pub fn params(&self) -> Value {
        match self {
            Self::Initialize(p) => to_value(p),
            #[cfg(test)]
            Self::Shutdown => json!({}),
            Self::Activate(p) => to_value(p),
            Self::Deactivate(p) => to_value(p),
            Self::ToolCall(p) => to_value(p),
        }
    }

    #[must_use]
    pub fn to_value(&self, id: u64) -> Value {
        request_value(id, self.method(), self.params())
    }
}

/// `registry/register` answer: a handle, or a refusal the host reports as a
/// failed activation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RegisterResult {
    Admitted { handle: u64 },
    Refused { refused: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ActivateResult {
    Ok { tools: Vec<String> },
    Failed { diagnostic: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeactivateResult {
    pub disposed: bool,
    pub leaked: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlockWire {
    Text { text: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolResultWire {
    pub content: Vec<ContentBlockWire>,
    pub is_error: bool,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub structured: Option<Value>,
}

/// One core→host message, parsed back (tests and the corpus only).
#[cfg(test)]
#[derive(Debug, Clone, PartialEq)]
pub enum CoreMessage {
    Request {
        id: u64,
        request: CoreRequest,
    },
    Cancel(CancelParams),
    Response {
        id: u64,
        outcome: Result<Value, RpcErrorWire>,
    },
}

/// Parse a core→host message (the shape this side writes).
#[cfg(test)]
pub fn parse_core_message(value: Value) -> Result<CoreMessage, ProtocolError> {
    let envelope = decode_envelope(value)?;
    let Some(method) = envelope.method.clone() else {
        let (id, outcome) = decode_response(envelope)?;
        return Ok(CoreMessage::Response { id, outcome });
    };
    let id = envelope.id;
    let p = envelope.params;
    if method == "$/cancel" {
        expect_notification(&method, id)?;
        return Ok(CoreMessage::Cancel(params(&method, p)?));
    }
    let id = expect_request(&method, id)?;
    let request = match method.as_str() {
        "host/initialize" => CoreRequest::Initialize(params(&method, p)?),
        "host/shutdown" => CoreRequest::Shutdown,
        "ext/activate" => CoreRequest::Activate(params(&method, p)?),
        "ext/deactivate" => CoreRequest::Deactivate(params(&method, p)?),
        "tool/call" => CoreRequest::ToolCall(params(&method, p)?),
        other => return Err(perr(format!("unknown core_to_host method `{other}`"))),
    };
    Ok(CoreMessage::Request { id, request })
}

#[cfg(test)]
impl CoreMessage {
    #[must_use]
    pub fn to_value(&self) -> Value {
        match self {
            Self::Request { id, request } => request.to_value(*id),
            Self::Cancel(p) => notification_value("$/cancel", to_value(p)),
            Self::Response { id, outcome } => response_value(*id, outcome),
        }
    }
}

#[must_use]
pub fn cancel_value(id: u64) -> Value {
    notification_value("$/cancel", json!({ "id": id }))
}

#[must_use]
pub fn response_ok(id: u64, result: Value) -> Value {
    response_value(id, &Ok(result))
}
