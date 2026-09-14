//! Shared stream entry seam for Chat Completions / Anthropic Messages / Responses.
//!
//! Scoped consolidation for v0.9.1: wire-protocol adapters stay at the edge
//! (`chat.rs`, `anthropic.rs`, `responses.rs`); this module owns the common
//! open path, HTTP/1.1 fallback policy, and idle-timeout envelope so providers
//! do not re-implement transport differently.
//!
//! Full piagent-style provider collapse is deferred — see
//! `docs/notes/post-0.9.1-thin-tui-and-stream.md`.

use std::future::Future;
use std::time::Duration;

use anyhow::Result;
use reqwest::Client;

use crate::llm_client::LlmError;

/// Default bounded wait for SSE response headers. Intentionally shorter than
/// the per-chunk idle timeout: it covers connection setup and upstream header
/// return only, never model thinking time after streaming has started.
pub(crate) const DEFAULT_STREAM_OPEN_TIMEOUT: Duration = Duration::from_secs(45);

/// Env override (`CODEWHALE_STREAM_OPEN_TIMEOUT_SECS`, legacy
/// `DEEPSEEK_STREAM_OPEN_TIMEOUT_SECS`) for the response-header wait,
/// shared by every streaming adapter.
pub(crate) fn stream_open_timeout() -> Duration {
    stream_open_timeout_from_env(
        std::env::var("CODEWHALE_STREAM_OPEN_TIMEOUT_SECS")
            .or_else(|_| std::env::var("DEEPSEEK_STREAM_OPEN_TIMEOUT_SECS"))
            .ok()
            .as_deref(),
    )
}

pub(crate) fn stream_open_timeout_from_env(value: Option<&str>) -> Duration {
    let secs = value
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(DEFAULT_STREAM_OPEN_TIMEOUT.as_secs())
        .clamp(5, 300);
    Duration::from_secs(secs)
}

/// How the shared stream open path should pin HTTP version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamHttpPolicy {
    /// Prefer the dual client (H2 primary, H1 twin for fallback).
    DualWithH1Fallback,
    /// Force HTTP/1.1 only (env pin or prior H2 stall).
    Http1Only,
}

/// Inputs shared by every streaming provider adapter at open time.
#[derive(Debug, Clone)]
pub struct StreamOpenRequest {
    pub policy: StreamHttpPolicy,
    pub open_timeout: Duration,
    pub idle_timeout: Duration,
}

impl StreamOpenRequest {
    #[must_use]
    pub fn new(open_timeout: Duration, idle_timeout: Duration) -> Self {
        Self {
            policy: if super::force_http1_from_env() {
                StreamHttpPolicy::Http1Only
            } else {
                StreamHttpPolicy::DualWithH1Fallback
            },
            open_timeout,
            idle_timeout,
        }
    }

    /// After an H2 stall, retry on the HTTP/1.1 twin.
    #[must_use]
    pub fn with_h1_only(mut self) -> Self {
        self.policy = StreamHttpPolicy::Http1Only;
        self
    }
}

/// Select the HTTP client for a stream open attempt.
#[must_use]
pub fn client_for_policy<'a>(
    primary: &'a Client,
    http1_fallback: &'a Client,
    policy: StreamHttpPolicy,
) -> &'a Client {
    match policy {
        StreamHttpPolicy::DualWithH1Fallback => primary,
        StreamHttpPolicy::Http1Only => http1_fallback,
    }
}

/// Whether a transport error should trigger H1 fallback retry.
#[must_use]
pub fn should_retry_with_h1(policy: StreamHttpPolicy, err_text: &str) -> bool {
    if policy != StreamHttpPolicy::DualWithH1Fallback {
        return false;
    }
    let lower = err_text.to_ascii_lowercase();
    lower.contains("http2")
        || lower.contains("h2 ")
        || lower.contains("stream closed")
        || lower.contains("connection reset")
        || lower.contains("protocol error")
        || lower.contains("frame size")
}

/// Whether an error raised before response headers should retry through the
/// HTTP/1.1 twin. Prefer typed transport errors, then retain the narrow text
/// classifier for lower-level H2 errors that reqwest exposes only as prose.
#[must_use]
fn should_retry_error_with_h1(policy: StreamHttpPolicy, err: &anyhow::Error) -> bool {
    if policy != StreamHttpPolicy::DualWithH1Fallback {
        return false;
    }

    if let Some(llm_error) = err.downcast_ref::<LlmError>() {
        return matches!(llm_error, LlmError::NetworkError(_) | LlmError::Timeout(_));
    }

    if let Some(reqwest_error) = err.downcast_ref::<reqwest::Error>() {
        return reqwest_error.is_connect()
            || reqwest_error.is_timeout()
            || reqwest_error.is_request();
    }

    should_retry_with_h1(policy, &format!("{err:#}"))
}

/// Preserve provider-semantic failures returned by the H1 attempt. Only a
/// transport failure should be normalized into the shared retryable network
/// error; otherwise an auth or invalid-request failure could be retried as if
/// switching protocols had failed.
fn h1_fallback_error(err: anyhow::Error) -> anyhow::Error {
    if let Some(llm_error) = err.downcast_ref::<LlmError>()
        && !matches!(llm_error, LlmError::NetworkError(_) | LlmError::Timeout(_))
    {
        return err;
    }

    anyhow::Error::new(LlmError::NetworkError(format!(
        "SSE stream request failed after HTTP/1.1 fallback: {err}. \
         `codewhale doctor` can still pass when non-streaming requests work; \
         on Windows or proxy networks, try `CODEWHALE_FORCE_HTTP1=1` and rerun `codewhale`."
    )))
}

/// Open an SSE response through the shared transport policy.
///
/// `attempt` builds and sends one wire-specific request on the client
/// selected for the given policy (via [`client_for_policy`]); everything
/// transport-shared lives here:
///
/// - the response-header wait is bounded by `open_req.open_timeout`;
/// - a classified transport failure or header stall on the dual client
///   retries exactly once on the HTTP/1.1 twin;
/// - a failure on an already H1-pinned request never retries;
/// - once response headers have been received the seam never retries —
///   body/stream errors belong to the adapter's decode loop.
pub(crate) async fn open_sse_response<F, Fut>(
    open_req: &StreamOpenRequest,
    attempt: F,
) -> Result<reqwest::Response>
where
    F: Fn(StreamHttpPolicy) -> Fut,
    Fut: Future<Output = Result<reqwest::Response>>,
{
    let fallback_reason = match tokio::time::timeout(
        open_req.open_timeout,
        attempt(open_req.policy),
    )
    .await
    {
        Ok(Ok(response)) => return Ok(response),
        Ok(Err(err)) => {
            if !should_retry_error_with_h1(open_req.policy, &err) {
                return Err(err);
            }
            "transport error before response headers"
        }
        Err(_elapsed) => {
            if open_req.policy == StreamHttpPolicy::Http1Only {
                return Err(anyhow::Error::new(LlmError::NetworkError(format!(
                    "SSE stream request did not receive response headers after {}s. \
                         `codewhale doctor` can still pass when non-streaming requests work; \
                         on Windows or proxy networks, try `CODEWHALE_FORCE_HTTP1=1` and rerun `codewhale`.",
                    open_req.open_timeout.as_secs()
                ))));
            }
            "response-header timeout"
        }
    };

    // No response body exists yet, so switching protocols and replaying the
    // request is safe. The policy guard above keeps this to exactly one retry.
    let h1_req = open_req.clone().with_h1_only();
    crate::logging::warn(format!(
        "SSE stream {fallback_reason}; retrying once with HTTP/1.1"
    ));
    match tokio::time::timeout(h1_req.open_timeout, attempt(h1_req.policy)).await {
        Ok(Ok(response)) => Ok(response),
        Ok(Err(err)) => Err(h1_fallback_error(err)),
        // Typed, not a bare string: a header stall is a transport
        // failure, and `LlmError::NetworkError` is what the shared
        // retry layer recognizes as retryable. As an untyped anyhow
        // error this killed the whole turn outright.
        Err(_elapsed) => Err(anyhow::Error::new(LlmError::NetworkError(format!(
            "SSE stream request did not receive response headers after {}s \
             (HTTP/2 and HTTP/1.1). `codewhale doctor` can still pass when \
             non-streaming requests work; try `CODEWHALE_FORCE_HTTP1=1` and \
             rerun `codewhale`.",
            open_req.open_timeout.as_secs()
        )))),
    }
}

/// Format a stable idle-timeout message shared across adapters.
#[must_use]
pub fn idle_timeout_message(
    idle: Duration,
    bytes_received: usize,
    stream_age: Duration,
    since_last_chunk: Duration,
) -> String {
    format!(
        "SSE stream idle timeout after {}s — no data received \
         (bytes_received={}, stream_age_ms={}, ms_since_last_chunk={})",
        idle.as_secs(),
        bytes_received,
        stream_age.as_millis(),
        since_last_chunk.as_millis(),
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    fn open_req(policy: StreamHttpPolicy, open_timeout: Duration) -> StreamOpenRequest {
        StreamOpenRequest {
            policy,
            open_timeout,
            idle_timeout: Duration::from_secs(30),
        }
    }

    async fn ok_server() -> MockServer {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        server
    }

    #[tokio::test]
    async fn open_returns_first_attempt_response_on_dual_policy() {
        let server = ok_server().await;
        let _ = rustls::crypto::ring::default_provider().install_default();
        let client = reqwest::Client::new();
        let attempts = Arc::new(AtomicUsize::new(0));
        let response = open_sse_response(
            &open_req(StreamHttpPolicy::DualWithH1Fallback, Duration::from_secs(5)),
            |policy| {
                assert_eq!(policy, StreamHttpPolicy::DualWithH1Fallback);
                let attempts = Arc::clone(&attempts);
                let client = client.clone();
                let url = server.uri();
                async move {
                    attempts.fetch_add(1, Ordering::SeqCst);
                    Ok(client.post(url).send().await?)
                }
            },
        )
        .await
        .expect("first attempt succeeds");
        assert_eq!(response.status(), 200);
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn header_stall_on_dual_policy_retries_exactly_once_on_h1() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let response = open_sse_response(
            &open_req(
                StreamHttpPolicy::DualWithH1Fallback,
                Duration::from_millis(150),
            ),
            |policy| {
                let attempts = Arc::clone(&attempts);
                async move {
                    let attempt = attempts.fetch_add(1, Ordering::SeqCst);
                    if attempt == 0 {
                        // First attempt stalls before response headers.
                        assert_eq!(policy, StreamHttpPolicy::DualWithH1Fallback);
                        std::future::pending::<()>().await;
                    }
                    assert_eq!(policy, StreamHttpPolicy::Http1Only);
                    // This test exercises retry policy, not loopback scheduling.
                    // A ready response keeps the 150 ms first-attempt timeout
                    // from also imposing a network deadline under suite load.
                    Ok(reqwest::Response::from(
                        axum::http::Response::builder().status(200).body("")?,
                    ))
                }
            },
        )
        .await
        .expect("H1 fallback retry succeeds");
        assert_eq!(response.status(), 200);
        assert_eq!(
            attempts.load(Ordering::SeqCst),
            2,
            "exactly one fallback retry"
        );
    }

    #[tokio::test]
    async fn transport_error_before_headers_retries_exactly_once_on_h1() {
        let server = ok_server().await;
        let _ = rustls::crypto::ring::default_provider().install_default();
        let client = reqwest::Client::new();
        let attempts = Arc::new(AtomicUsize::new(0));
        let response = open_sse_response(
            &open_req(StreamHttpPolicy::DualWithH1Fallback, Duration::from_secs(5)),
            |policy| {
                let attempts = Arc::clone(&attempts);
                let client = client.clone();
                let url = server.uri();
                async move {
                    let attempt = attempts.fetch_add(1, Ordering::SeqCst);
                    if attempt == 0 {
                        assert_eq!(policy, StreamHttpPolicy::DualWithH1Fallback);
                        return Err(anyhow::Error::new(LlmError::NetworkError(
                            "connection reset before response headers".to_string(),
                        ))
                        .context("Chat API request failed"));
                    }
                    assert_eq!(policy, StreamHttpPolicy::Http1Only);
                    Ok(client.post(url).send().await?)
                }
            },
        )
        .await
        .expect("H1 fallback retry succeeds after a transport error");
        assert_eq!(response.status(), 200);
        assert_eq!(
            attempts.load(Ordering::SeqCst),
            2,
            "exactly one fallback retry"
        );
    }

    #[tokio::test]
    async fn header_stall_when_h1_pinned_never_retries_and_reports_timeout_text() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let err = open_sse_response(
            &open_req(StreamHttpPolicy::Http1Only, Duration::from_millis(100)),
            |_| {
                let attempts = Arc::clone(&attempts);
                async move {
                    attempts.fetch_add(1, Ordering::SeqCst);
                    std::future::pending::<()>().await;
                    unreachable!("stalled attempt never resolves")
                }
            },
        )
        .await
        .expect_err("H1-pinned stall fails without retry");
        assert_eq!(attempts.load(Ordering::SeqCst), 1, "no retry when pinned");
        let text = err.to_string();
        assert!(text.contains("did not receive response headers"), "{text}");
        // The whole point of the typed error: a header stall must reach the
        // shared retry layer as retryable. As an untyped anyhow error it
        // killed the turn outright, so a long root run lost all its work while
        // a sub-agent — which text-matches the same message in its own
        // classifier — would have retried and continued.
        let classified = err
            .downcast_ref::<crate::llm_client::LlmError>()
            .expect("header stall must be a typed LlmError");
        assert!(
            classified.is_retryable(),
            "header stall must be retryable: {classified:?}"
        );
        assert!(text.contains("CODEWHALE_FORCE_HTTP1=1"), "{text}");
        assert!(
            !text.contains("HTTP/2 and HTTP/1.1"),
            "single-protocol stall must not claim a dual-protocol attempt: {text}"
        );
    }

    #[tokio::test]
    async fn transport_error_when_h1_pinned_never_retries() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let err = open_sse_response(
            &open_req(StreamHttpPolicy::Http1Only, Duration::from_secs(5)),
            |_| {
                let attempts = Arc::clone(&attempts);
                async move {
                    attempts.fetch_add(1, Ordering::SeqCst);
                    Err(anyhow::Error::new(LlmError::NetworkError(
                        "connection reset before response headers".to_string(),
                    )))
                }
            },
        )
        .await
        .expect_err("an H1-pinned transport error must not retry");
        assert_eq!(attempts.load(Ordering::SeqCst), 1, "no retry when pinned");
        assert!(err.to_string().contains("connection reset"), "{err}");
    }

    #[tokio::test]
    async fn provider_error_from_h1_fallback_keeps_its_semantic_type() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let err = open_sse_response(
            &open_req(StreamHttpPolicy::DualWithH1Fallback, Duration::from_secs(5)),
            |policy| {
                let attempts = Arc::clone(&attempts);
                async move {
                    let attempt = attempts.fetch_add(1, Ordering::SeqCst);
                    if attempt == 0 {
                        assert_eq!(policy, StreamHttpPolicy::DualWithH1Fallback);
                        return Err(anyhow::Error::new(LlmError::NetworkError(
                            "connection reset before response headers".to_string(),
                        )));
                    }
                    assert_eq!(policy, StreamHttpPolicy::Http1Only);
                    Err(anyhow::Error::new(LlmError::InvalidRequest {
                        status: 400,
                        message: "invalid request".to_string(),
                    }))
                }
            },
        )
        .await
        .expect_err("the H1 provider error must be returned");
        assert_eq!(attempts.load(Ordering::SeqCst), 2, "one fallback attempt");
        assert!(
            matches!(
                err.downcast_ref::<LlmError>(),
                Some(LlmError::InvalidRequest { status: 400, .. })
            ),
            "provider error was reclassified: {err:#}"
        );
    }

    #[tokio::test]
    async fn attempt_error_before_headers_is_not_h1_retried() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let err = open_sse_response(
            &open_req(StreamHttpPolicy::DualWithH1Fallback, Duration::from_secs(5)),
            |_| {
                let attempts = Arc::clone(&attempts);
                async move {
                    attempts.fetch_add(1, Ordering::SeqCst);
                    Err(anyhow::anyhow!("HTTP 401: invalid api key"))
                }
            },
        )
        .await
        .expect_err("provider error propagates");
        assert_eq!(
            attempts.load(Ordering::SeqCst),
            1,
            "non-stall errors are never H1-retried"
        );
        assert!(err.to_string().contains("HTTP 401"), "{err}");
    }

    #[tokio::test]
    async fn double_stall_reports_both_protocols_in_timeout_text() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let err = open_sse_response(
            &open_req(
                StreamHttpPolicy::DualWithH1Fallback,
                Duration::from_millis(100),
            ),
            |_| {
                let attempts = Arc::clone(&attempts);
                async move {
                    attempts.fetch_add(1, Ordering::SeqCst);
                    std::future::pending::<()>().await;
                    unreachable!("stalled attempt never resolves")
                }
            },
        )
        .await
        .expect_err("double stall fails");
        assert_eq!(attempts.load(Ordering::SeqCst), 2, "one fallback, no more");
        let text = err.to_string();
        assert!(text.contains("HTTP/2 and HTTP/1.1"), "{text}");
    }

    #[test]
    fn h1_retry_only_on_dual_policy() {
        assert!(should_retry_with_h1(
            StreamHttpPolicy::DualWithH1Fallback,
            "http2 protocol error"
        ));
        assert!(!should_retry_with_h1(
            StreamHttpPolicy::Http1Only,
            "http2 protocol error"
        ));
    }

    #[test]
    fn stream_open_timeout_defaults_and_clamps_env_values() {
        assert_eq!(stream_open_timeout_from_env(None), Duration::from_secs(45));
        assert_eq!(
            stream_open_timeout_from_env(Some("not-a-number")),
            Duration::from_secs(45)
        );
        assert_eq!(
            stream_open_timeout_from_env(Some("1")),
            Duration::from_secs(5)
        );
        assert_eq!(
            stream_open_timeout_from_env(Some("120")),
            Duration::from_secs(120)
        );
        assert_eq!(
            stream_open_timeout_from_env(Some("999")),
            Duration::from_secs(300)
        );
    }

    #[test]
    fn idle_message_is_stable() {
        let msg = idle_timeout_message(
            Duration::from_secs(30),
            0,
            Duration::from_secs(30),
            Duration::from_secs(30),
        );
        assert!(msg.contains("idle timeout"));
        assert!(msg.contains("bytes_received=0"));
    }
}
