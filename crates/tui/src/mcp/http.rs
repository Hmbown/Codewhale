//! HTTP MCP transport.
//!
//! Speaks Streamable HTTP first and falls back to the legacy SSE endpoint
//! when the server rejects the newer protocol, plus the header/token/OAuth
//! resolution shared by both HTTP-flavoured transports.

use std::collections::HashMap;
use std::time::Duration;

use anyhow::Result;

use super::headers::{apply_safe_custom_headers, with_default_mcp_http_headers};
use super::http_client::McpHttpClient;
use super::sse::SseTransport;
use super::streamable_http::{StreamableHttpTransport, StreamableSendError};
use super::{McpServerConfig, McpTransport, ReviewedPluginMcpSource, oauth};
pub(super) struct HttpTransport {
    mode: HttpTransportMode,
    client: McpHttpClient,
    base_url: String,
    auth: McpHttpAuth,
    cancel_token: tokio_util::sync::CancellationToken,
    endpoint_timeout: Duration,
}

enum HttpTransportMode {
    Streamable(StreamableHttpTransport),
    Sse(SseTransport),
}

#[derive(Clone, Default)]
pub(super) struct McpHttpAuth {
    pub(super) server_name: String,
    pub(super) headers: HashMap<String, String>,
    pub(super) env_headers: HashMap<String, String>,
    pub(super) bearer_token_env_var: Option<String>,
    pub(super) oauth: Option<oauth::McpOAuthRuntime>,
    /// Whether the server's *configuration* routes authentication through
    /// OAuth, independent of whether a credential is cached yet: a URL-based
    /// server that is neither plugin-contributed nor supplied a manual
    /// bearer/Authorization credential (#6030).
    ///
    /// This is [`oauth::server_supports_oauth_login`] — the same predicate the
    /// login flow itself is gated on — so the recovery copy it selects
    /// (`/mcp login <name>`) names a command that will actually run. A live
    /// [`Self::oauth`] runtime always implies it: the runtime is only built
    /// for a server that passes this predicate. A first-run OAuth server has
    /// no runtime yet, which is exactly the case that used to fall through to
    /// the bearer-token copy.
    pub(super) oauth_configured: bool,
    pub(super) suppress_server_error_details: bool,
    pub(super) reviewed_plugin: Option<ReviewedPluginMcpSource>,
}

impl McpHttpAuth {
    pub(super) fn from_config(
        server_name: &str,
        config: &McpServerConfig,
        oauth: Option<oauth::McpOAuthRuntime>,
    ) -> Self {
        Self {
            server_name: server_name.to_string(),
            headers: config.headers.clone(),
            env_headers: config.env_headers.clone(),
            bearer_token_env_var: config.bearer_token_env_var.clone(),
            oauth,
            oauth_configured: oauth::server_supports_oauth_login(config),
            suppress_server_error_details: config.reviewed_plugin.is_some(),
            reviewed_plugin: config.reviewed_plugin.clone(),
        }
    }

    pub(super) fn server_error_preview(&self, preview: &str) -> String {
        if self.suppress_server_error_details {
            "<server details suppressed for reviewed plugin>".to_string()
        } else {
            preview.to_string()
        }
    }

    pub(super) async fn resolved_headers(&self) -> Result<HashMap<String, String>> {
        if let Some(source) = self.reviewed_plugin.as_ref() {
            source.validate_before_use(&self.server_name, "authenticate request to")?;
        }
        let mut headers = self.headers.clone();
        for (name, env_var) in &self.env_headers {
            let value = self.reviewed_plugin.as_ref().map_or_else(
                || std::env::var(env_var),
                |source| source.host_environment.var(env_var),
            );
            if let Ok(value) = value
                && !value.trim().is_empty()
            {
                headers.insert(name.clone(), value);
            }
        }
        if !mcp_headers_have_authorization(&headers)
            && let Some(env_var) = self.bearer_token_env_var.as_deref()
            && let Ok(token) = self.reviewed_plugin.as_ref().map_or_else(
                || std::env::var(env_var),
                |source| source.host_environment.var(env_var),
            )
        {
            let token = token.trim();
            if !token.is_empty() {
                headers.insert("Authorization".to_string(), format!("Bearer {token}"));
            }
        }
        if !mcp_headers_have_authorization(&headers)
            && let Some(oauth) = &self.oauth
        {
            let authorization = match oauth.authorization_header().await {
                Ok(authorization) => authorization,
                Err(_) if self.suppress_server_error_details => {
                    anyhow::bail!(
                        "Reviewed plugin MCP authentication failed (provider details suppressed)"
                    )
                }
                Err(error) => return Err(error),
            };
            if let Some(value) = authorization {
                headers.insert("Authorization".to_string(), value);
            }
        }
        Ok(headers)
    }
}

pub(super) fn mcp_headers_have_authorization(headers: &HashMap<String, String>) -> bool {
    headers
        .keys()
        .any(|key| key.trim().eq_ignore_ascii_case("authorization"))
}

impl HttpTransport {
    pub(super) fn new(
        client: McpHttpClient,
        url: String,
        auth: McpHttpAuth,
        cancel_token: tokio_util::sync::CancellationToken,
        endpoint_timeout: Duration,
    ) -> Self {
        Self {
            mode: HttpTransportMode::Streamable(StreamableHttpTransport::new(
                client.clone(),
                url.clone(),
                auth.clone(),
            )),
            client,
            base_url: url,
            auth,
            cancel_token,
            endpoint_timeout,
        }
    }

    async fn switch_to_sse_and_send(&mut self, msg: Vec<u8>) -> Result<()> {
        let mut sse = SseTransport::connect(
            self.client.clone(),
            self.base_url.clone(),
            self.auth.clone(),
            self.cancel_token.clone(),
            self.endpoint_timeout,
        )
        .await?;
        sse.send(msg).await?;
        self.mode = HttpTransportMode::Sse(sse);
        Ok(())
    }

    /// Best-effort session-establishment GET preflight.
    ///
    /// Per the Streamable HTTP spec, the server may return an
    /// `Mcp-Session-Id` header on the `initialize` response (the normal
    /// path handled inside [`StreamableHttpTransport::send`] above).
    /// However some servers (e.g. Hindsight, #1629) **require** a session
    /// ID on every POST including `initialize`, creating a chicken-and-egg
    /// problem. For those servers we send a short-lived GET before the
    /// first POST: if the server returns a session ID in the GET response
    /// it will be captured by the header-reading code in
    /// [`StreamableHttpTransport::send`] just as if it came from a POST
    /// response.
    ///
    /// This is intentionally best-effort:
    /// * The GET uses a tight per-request inner timeout so it never
    ///   blocks connection startup for long.
    /// * If the server doesn't support GET (405, 404, …) we log a debug
    ///   line and move on — the `initialize` POST will proceed without a
    ///   session ID.
    /// * If the server opens an SSE stream in response (the GET from old
    ///   SSE transport), we read only the headers, then discard the body
    ///   so the SSE stream is torn down. The actual SSE path uses a
    ///   dedicated `SseTransport` and is triggered by the incompatible-
    ///   status fallback in [`HttpTransport::send`].
    pub(super) async fn try_establish_session(&mut self) -> Result<()> {
        let cancel = self.cancel_token.clone();
        let transport = match &mut self.mode {
            HttpTransportMode::Streamable(t) => t,
            // Already on SSE — session is implicit via the long-lived GET.
            HttpTransportMode::Sse(_) => return Ok(()),
        };

        let headers = tokio::select! {
            biased;
            _ = cancel.cancelled() => {
                anyhow::bail!("MCP session preflight cancelled after plugin authority changed")
            }
            headers = transport.auth.resolved_headers() => headers?,
        };
        let request = apply_safe_custom_headers(
            with_default_mcp_http_headers(transport.client.get(&transport.url), false),
            &headers,
        );
        let response = tokio::select! {
            biased;
            _ = cancel.cancelled() => {
                anyhow::bail!("MCP session preflight cancelled after plugin authority changed")
            }
            response = tokio::time::timeout(Duration::from_secs(5), transport.client.send(request)) => {
                response
                    .map_err(|_| anyhow::anyhow!("GET timeout"))?
                    .map_err(|e| anyhow::anyhow!("GET error: {e}"))?
            }
        };

        // Capture session ID from the GET response so subsequent POSTs
        // (including `initialize`) can include it. This is the same
        // header-reading logic that would be hit inside
        // `StreamableHttpTransport::send` for POST responses, but since
        // the GET is sent before any POST we do it here directly.
        if let Some(sid) = response
            .headers()
            .get("Mcp-Session-Id")
            .and_then(|v| v.to_str().ok())
            && transport.session_id.as_deref() != Some(sid)
        {
            let session_ref = crate::utils::redacted_identifier_for_log(sid);
            tracing::debug!(target: "mcp", session = %session_ref, "captured MCP session ID via GET preflight");
            transport.session_id = Some(sid.to_string());
        }

        // We only care about the response headers — discard the body.
        // If the server opened an SSE stream in response (some servers
        // do this on GET), it will be torn down when response is dropped.
        drop(response);

        Ok(())
    }
}

#[async_trait::async_trait]
impl McpTransport for HttpTransport {
    fn set_protocol_version(&mut self, version: &str) {
        // Only Streamable HTTP carries the MCP-Protocol-Version header; the
        // legacy SSE transport predates it and ignores the negotiation result.
        if let HttpTransportMode::Streamable(transport) = &mut self.mode {
            transport.set_protocol_version(version);
        }
    }

    async fn send(&mut self, msg: Vec<u8>) -> Result<()> {
        match &mut self.mode {
            HttpTransportMode::Streamable(transport) => match transport.send(msg.clone()).await {
                Ok(()) => Ok(()),
                Err(StreamableSendError::Incompatible(detail)) => {
                    tracing::debug!(
                        "MCP Streamable HTTP unavailable; falling back to SSE endpoint discovery: {}",
                        detail
                    );
                    self.switch_to_sse_and_send(msg).await
                }
                Err(StreamableSendError::StaleSession(detail)) => {
                    if let HttpTransportMode::Streamable(transport) = &mut self.mode {
                        tracing::debug!(
                            target: "mcp",
                            error = %detail,
                            "MCP Streamable HTTP session expired; clearing cached session ID"
                        );
                        transport.session_id = None;
                    }
                    Err(anyhow::anyhow!(
                        "MCP Streamable HTTP session expired; retry with a new session required ({detail})"
                    ))
                }
                Err(StreamableSendError::Other(err)) => Err(err),
            },
            HttpTransportMode::Sse(transport) => transport.send(msg).await,
        }
    }

    async fn recv(&mut self) -> Result<Vec<u8>> {
        match &mut self.mode {
            HttpTransportMode::Streamable(transport) => transport.recv().await,
            HttpTransportMode::Sse(transport) => transport.recv().await,
        }
    }

    async fn shutdown(&mut self) {
        if let HttpTransportMode::Sse(transport) = &mut self.mode {
            transport.shutdown().await;
        }
    }
}
