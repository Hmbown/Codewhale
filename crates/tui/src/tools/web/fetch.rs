//! Unified guarded fetch pipeline for `fetch_url` and `web.run`.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
#[cfg(not(test))]
use std::time::{SystemTime, UNIX_EPOCH};

use futures_util::StreamExt;
use serde::Serialize;

use super::cache::{self, CachedFetch};
use super::extract::is_js_shell_error;
use super::guard::{
    DnsPin, guarded_reqwest_client_builder, validate_fetch_target, validate_network_policy,
};
use crate::features::Feature;
use crate::tools::spec::{ToolContext, ToolError};
use crate::worker_profile::ShellPolicy;

pub(crate) const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);
pub(crate) const HARD_MAX_TIMEOUT: Duration = Duration::from_secs(60);
pub(crate) const DEFAULT_MAX_BYTES: usize = 1_000_000;
pub(crate) const HARD_MAX_BYTES: usize = 10 * 1024 * 1024;
const MAX_REDIRECTS: usize = 5;
const USER_AGENT: &str = concat!(
    "Mozilla/5.0 (compatible; codewhale/",
    env!("CARGO_PKG_VERSION"),
    "; +https://github.com/Hmbown/CodeWhale)"
);

#[derive(Debug, Clone)]
pub(crate) struct FetchOptions {
    pub(crate) timeout: Duration,
    pub(crate) max_bytes: usize,
    pub(crate) accept: &'static str,
    pub(crate) user_agent: &'static str,
}

impl FetchOptions {
    pub(crate) fn new(timeout: Duration, max_bytes: usize, accept: &'static str) -> Self {
        Self {
            timeout: timeout.min(HARD_MAX_TIMEOUT),
            max_bytes: max_bytes.clamp(1, HARD_MAX_BYTES),
            accept,
            user_agent: USER_AGENT,
        }
    }

    /// Request with the shared browser user-agent instead of the Codewhale
    /// one. Only the `web.run` browse surface uses this, and only as the
    /// one-shot fallback after a site refused the default agent.
    #[must_use]
    pub(crate) fn with_browser_user_agent(mut self) -> Self {
        self.user_agent = super::scrape::BROWSER_USER_AGENT;
        self
    }
}

#[derive(Debug, Clone)]
pub(crate) struct FetchedPayload {
    pub(crate) url: String,
    pub(crate) status: u16,
    pub(crate) headers: BTreeMap<String, String>,
    pub(crate) content_type: String,
    pub(crate) bytes: Arc<Vec<u8>>,
    pub(crate) truncated: bool,
    pub(crate) cache_hit: bool,
    pub(crate) retries: usize,
    pub(crate) redirects: usize,
}

/// Whether one request may be answered from a cache, or must revalidate.
///
/// `Revalidate` bypasses the session fetch cache *and* asks every intermediary
/// to revalidate. An edge cache can hold a prerendered variant while an origin
/// MISS serves the client-side shell, so the same URL alternates between
/// readable and unreadable depending on which variant answered (#5904).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CacheMode {
    Default,
    Revalidate,
}

impl CacheMode {
    const fn is_revalidate(self) -> bool {
        matches!(self, Self::Revalidate)
    }
}

/// Response headers that explain the cache state behind a 200.
///
/// These are the four that distinguish "the edge served a prerendered page"
/// from "the origin served the JavaScript shell", so both the success and the
/// failure receipt carry whichever of them the response actually had.
const CACHE_STATE_HEADERS: [&str; 4] = [
    "age",
    "cf-cache-status",
    "x-nextjs-prerender",
    "x-vercel-cache",
];

/// One request inside a readable-fetch sequence, as it appears on the receipt.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct FetchAttempt {
    /// 1-based position in the sequence.
    pub(crate) attempt: usize,
    pub(crate) status: u16,
    /// Whether the session fetch cache answered this attempt.
    pub(crate) cache_hit: bool,
    /// Whether this attempt sent `Cache-Control: no-cache` / `Pragma: no-cache`
    /// and skipped the session cache.
    pub(crate) cache_busted: bool,
    /// Whether this attempt is the one that yielded a readable document.
    pub(crate) produced_content: bool,
    /// `age`, `cf-cache-status`, `x-nextjs-prerender`, `x-vercel-cache` — only
    /// those the response actually carried.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) cache_headers: BTreeMap<String, String>,
}

impl FetchAttempt {
    fn record(payload: &FetchedPayload, attempt: usize, mode: CacheMode) -> Self {
        Self {
            attempt,
            status: payload.status,
            cache_hit: payload.cache_hit,
            cache_busted: mode.is_revalidate(),
            produced_content: false,
            cache_headers: cache_state_headers(&payload.headers),
        }
    }

    fn summarize(&self) -> String {
        let mut facts = vec![format!("HTTP {}", self.status)];
        if self.cache_hit {
            facts.push("session cache hit".to_string());
        }
        for (name, value) in &self.cache_headers {
            facts.push(format!("{name}={value}"));
        }
        let label = if self.cache_busted {
            format!("attempt {} (Cache-Control: no-cache)", self.attempt)
        } else {
            format!("attempt {}", self.attempt)
        };
        format!("{label}: {}", facts.join(", "))
    }
}

fn cache_state_headers(headers: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    CACHE_STATE_HEADERS
        .iter()
        .filter_map(|name| {
            headers
                .get(*name)
                .map(|value| ((*name).to_string(), value.clone()))
        })
        .collect()
}

/// The extraction step [`fetch_readable`] may run against either attempt.
///
/// Spelled as an explicit boxed future over an *owned* payload rather than as
/// an `AsyncFn` over a borrowed one: the callers live inside
/// `async fn execute(&self, .., &ToolContext)` futures that must stay `Send`,
/// and a higher-ranked borrow of the payload would force the returned future
/// to outlive the tool context it reads.
pub(crate) type ExtractFuture<'a, T> =
    std::pin::Pin<Box<dyn Future<Output = Result<T, ToolError>> + Send + 'a>>;

/// A fetch that produced a readable document, plus the attempts it took.
#[derive(Debug)]
pub(crate) struct ReadableFetch<T> {
    pub(crate) payload: FetchedPayload,
    pub(crate) document: T,
    pub(crate) attempts: Vec<FetchAttempt>,
}

/// Fetch `url` and extract it, re-fetching once past every cache when a 2xx
/// response yields no readable content.
///
/// This is the single place that turns the JS-shell case into either a second
/// chance or an error the model can act on. `extract` runs against the fetched
/// payload; only [`is_js_shell_error`] failures earn the second request, so
/// transport failures keep the existing single-retry behavior of one request.
pub(crate) async fn fetch_readable<'e, T, F>(
    url: &str,
    options: &FetchOptions,
    context: &ToolContext,
    tool_label: &str,
    extract: F,
) -> Result<ReadableFetch<T>, ToolError>
where
    F: Fn(FetchedPayload) -> ExtractFuture<'e, T>,
{
    fetch_readable_inner(url, options, context, tool_label, None, extract).await
}

#[cfg(test)]
pub(crate) async fn fetch_readable_with_initial_pin<'e, T, F>(
    url: &str,
    options: &FetchOptions,
    context: &ToolContext,
    tool_label: &str,
    initial_pin: DnsPin,
    extract: F,
) -> Result<ReadableFetch<T>, ToolError>
where
    F: Fn(FetchedPayload) -> ExtractFuture<'e, T>,
{
    fetch_readable_inner(
        url,
        options,
        context,
        tool_label,
        Some(initial_pin),
        extract,
    )
    .await
}

async fn fetch_readable_inner<'e, T, F>(
    url: &str,
    options: &FetchOptions,
    context: &ToolContext,
    tool_label: &str,
    test_initial_pin: Option<DnsPin>,
    extract: F,
) -> Result<ReadableFetch<T>, ToolError>
where
    F: Fn(FetchedPayload) -> ExtractFuture<'e, T>,
{
    let mut attempts: Vec<FetchAttempt> = Vec::with_capacity(2);
    for mode in [CacheMode::Default, CacheMode::Revalidate] {
        let payload = fetch_inner(
            url,
            options,
            context,
            tool_label,
            test_initial_pin.clone(),
            mode,
        )
        .await?;
        let mut record = FetchAttempt::record(&payload, attempts.len() + 1, mode);
        let final_url = payload.url.clone();
        match extract(payload.clone()).await {
            Ok(document) => {
                record.produced_content = true;
                attempts.push(record);
                return Ok(ReadableFetch {
                    payload,
                    document,
                    attempts,
                });
            }
            // A 2xx whose body held no readable content is the one failure a
            // second request can fix: the first response may have been a
            // cached client-side shell.
            Err(error)
                if is_js_shell_error(&error)
                    && (200..300).contains(&payload.status)
                    && mode == CacheMode::Default =>
            {
                attempts.push(record);
            }
            Err(error) => {
                attempts.push(record);
                return Err(if is_js_shell_error(&error) {
                    js_shell_failure(&final_url, &attempts, context, tool_label)
                } else {
                    error
                });
            }
        }
    }
    unreachable!("the revalidate pass either returns a document or an error");
}

/// The terminal JS-shell error, carrying the failure receipt and the recovery
/// the *calling role* actually owns.
fn js_shell_failure(
    url: &str,
    attempts: &[FetchAttempt],
    context: &ToolContext,
    tool_label: &str,
) -> ToolError {
    let receipt = attempts
        .iter()
        .map(FetchAttempt::summarize)
        .collect::<Vec<_>>()
        .join("; ");
    ToolError::execution_failed(format!(
        "{marker} {url} after {count} attempts, the second past every cache ({receipt}). The response parsed but held no readable body, which usually means the page renders its content with JavaScript. Recovery: {recovery}",
        marker = super::extract::JS_SHELL_MARKER,
        count = attempts.len(),
        recovery = js_shell_recovery(context, tool_label),
    ))
}

/// Whether the `web.run` browse surface is reachable from this context.
///
/// Both facts already exist: the web family is feature-gated, and a
/// network-denied Fleet worker carries `network_access: Some(false)` on the
/// authority envelope that also removes `web.run` from its registry
/// (`fleet::role::NETWORK_TOOL_DENYLIST`). Nothing new is registered here.
fn browser_surface_available(context: &ToolContext) -> bool {
    context.features.enabled(Feature::WebSearch) && network_authorized(context)
}

/// Whether this role could shell out to `curl` as a last resort. Read-only and
/// shell-less roles cannot: the read-only grammar rejects a network fetch.
fn shell_fallback_available(context: &ToolContext) -> bool {
    context.shell_policy == ShellPolicy::Full && network_authorized(context)
}

fn network_authorized(context: &ToolContext) -> bool {
    context
        .tool_authority
        .as_deref()
        .is_none_or(|authority| authority.network_access != Some(false))
}

fn js_shell_recovery(context: &ToolContext, tool_label: &str) -> String {
    // `web.run` is itself the escalation, so it never names itself.
    if tool_label != "web_run" && browser_surface_available(context) {
        return "open this URL with the `web.run` browse surface (`web.run {\"open\": {\"url\": ...}}`), which requests it with a browser user-agent and a ten-megabyte budget and usually receives the prerendered variant.".to_string();
    }
    let unavailable = if tool_label == "web_run" {
        "this is already the `web.run` browse surface, so there is no further web escalation."
    } else {
        "the `web.run` browse surface is not available to this role."
    };
    if shell_fallback_available(context) {
        format!("{unavailable} Fall back to a shell fetch (`curl -sSL`) or a rendering tool.")
    } else {
        format!(
            "{unavailable} This role is read-only and cannot fall back to a shell fetch, so report this URL as unreadable rather than substituting another source."
        )
    }
}

#[cfg(test)]
pub(crate) async fn fetch_with_initial_pin(
    url: &str,
    options: &FetchOptions,
    context: &ToolContext,
    tool_label: &str,
    initial_pin: DnsPin,
) -> Result<FetchedPayload, ToolError> {
    fetch_inner(
        url,
        options,
        context,
        tool_label,
        Some(initial_pin),
        CacheMode::Default,
    )
    .await
}

async fn fetch_inner(
    url: &str,
    options: &FetchOptions,
    context: &ToolContext,
    tool_label: &str,
    test_initial_pin: Option<DnsPin>,
    cache_mode: CacheMode,
) -> Result<FetchedPayload, ToolError> {
    let initial_url = reqwest::Url::parse(url)
        .map_err(|err| ToolError::invalid_input(format!("invalid URL: {err}")))?;
    if !matches!(initial_url.scheme(), "http" | "https") {
        return Err(ToolError::invalid_input(
            "only http:// and https:// URLs are supported",
        ));
    }

    // Validation precedes cache lookup so a policy tightened during the
    // session cannot be bypassed by a previously cached response.
    let validated_initial_pin = match test_initial_pin {
        Some(pin) => pin,
        None => validate_fetch_target(&initial_url, context, tool_label).await?,
    };

    if let Some(cached) = (!cache_mode.is_revalidate())
        .then(|| {
            cache::get(
                &context.state_namespace,
                &initial_url,
                options.accept,
                options.max_bytes,
            )
        })
        .flatten()
    {
        let cached_url = reqwest::Url::parse(&cached.url).map_err(|err| {
            ToolError::execution_failed(format!("cached response URL was invalid: {err}"))
        })?;
        let cached_host = cached_url.host_str().ok_or_else(|| {
            ToolError::execution_failed("cached response URL did not include a host")
        })?;
        // No network request occurs on a cache hit, so DNS/SSRF validation is
        // unnecessary. The final redirect destination still needs a policy
        // check in case the session policy was tightened after insertion.
        validate_network_policy(cached_host, context, tool_label)?;
        return Ok(from_cached(cached, true, 0));
    }

    let deadline = Instant::now() + options.timeout;
    let mut last_transient = None;
    for attempt in 0..=1 {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match fetch_attempt(
            initial_url.clone(),
            options,
            context,
            tool_label,
            remaining,
            validated_initial_pin.clone(),
            cache_mode,
        )
        .await
        {
            Ok(payload) if is_transient_status(payload.status) && attempt == 0 => {
                last_transient = Some(format!("HTTP {}", payload.status));
            }
            Ok(payload) => {
                let fetched = from_cached(payload.clone(), false, attempt);
                if (200..300).contains(&payload.status) {
                    cache::insert(
                        &context.state_namespace,
                        &initial_url,
                        options.accept,
                        payload,
                    );
                }
                return Ok(fetched);
            }
            Err(AttemptError::Fatal(error)) => return Err(error),
            Err(AttemptError::Transient(message)) if attempt == 0 => {
                last_transient = Some(message);
            }
            Err(AttemptError::Transient(message)) => {
                return Err(ToolError::execution_failed(format!(
                    "request failed after one retry: {message}"
                )));
            }
        }

        let delay = retry_delay();
        if deadline.saturating_duration_since(Instant::now()) <= delay {
            break;
        }
        tokio::time::sleep(delay).await;
    }

    Err(ToolError::execution_failed(format!(
        "request timed out before retry completed{}",
        last_transient
            .map(|message| format!(" (last failure: {message})"))
            .unwrap_or_default()
    )))
}

#[derive(Debug)]
enum AttemptError {
    Fatal(ToolError),
    Transient(String),
}

async fn fetch_attempt(
    initial_url: reqwest::Url,
    options: &FetchOptions,
    context: &ToolContext,
    tool_label: &str,
    timeout: Duration,
    initial_pin: DnsPin,
    cache_mode: CacheMode,
) -> Result<CachedFetch, AttemptError> {
    let mut current_url = initial_url;
    let mut redirects = 0usize;
    let mut initial_pin = initial_pin;
    let deadline = Instant::now() + timeout;

    let response = loop {
        let dns_pin = if redirects == 0 {
            match initial_pin.take() {
                Some(pin) => Some(pin),
                None => validate_fetch_target(&current_url, context, tool_label)
                    .await
                    .map_err(AttemptError::Fatal)?,
            }
        } else {
            validate_fetch_target(&current_url, context, tool_label)
                .await
                .map_err(AttemptError::Fatal)?
        };

        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(AttemptError::Transient(
                "request timed out while following redirects".to_string(),
            ));
        }
        let mut builder = guarded_reqwest_client_builder()
            .timeout(remaining)
            .user_agent(options.user_agent)
            .redirect(reqwest::redirect::Policy::none());
        if let Some((hostname, validated_ip)) = dns_pin {
            builder = builder.resolve(&hostname, std::net::SocketAddr::new(validated_ip, 0));
        }
        let client = builder.build().map_err(|err| {
            AttemptError::Fatal(ToolError::execution_failed(format!(
                "failed to build HTTP client: {err}"
            )))
        })?;
        let mut request = client
            .get(current_url.clone())
            .header("Accept", options.accept)
            .header("Accept-Language", "en-US,en;q=0.5");
        if cache_mode.is_revalidate() {
            // `no-cache` (revalidate), not `no-store`: the shared caches still
            // get to serve a validated copy, which is what recovers a page
            // whose prerendered variant exists but was not the one served.
            // `Pragma` is the HTTP/1.0 spelling some CDNs still honor.
            request = request
                .header("Cache-Control", "no-cache")
                .header("Pragma", "no-cache");
        }
        let response = request
            .send()
            .await
            .map_err(|err| AttemptError::Transient(err.to_string()))?;

        if !response.status().is_redirection() {
            break response;
        }
        if redirects >= MAX_REDIRECTS {
            return Err(AttemptError::Fatal(ToolError::execution_failed(
                "request exceeded the five-redirect limit",
            )));
        }
        let Some(location) = response
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|value| value.to_str().ok())
        else {
            break response;
        };
        current_url = response.url().join(location).map_err(|err| {
            AttemptError::Fatal(ToolError::execution_failed(format!(
                "invalid redirect location: {err}"
            )))
        })?;
        redirects += 1;
    };

    let final_url = response.url().to_string();
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();
    let headers = response_headers(response.headers());
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::with_capacity(options.max_bytes.min(64 * 1024));
    let mut truncated = false;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|err| AttemptError::Transient(err.to_string()))?;
        let remaining = options.max_bytes.saturating_sub(bytes.len());
        if chunk.len() > remaining {
            bytes.extend_from_slice(&chunk[..remaining]);
            truncated = true;
            break;
        }
        bytes.extend_from_slice(&chunk);
        if bytes.len() == options.max_bytes {
            // A response exactly at the cap may be complete. Ask for one more
            // chunk to distinguish exact length from actual truncation.
            if let Some(next) = stream.next().await {
                let next = next.map_err(|err| AttemptError::Transient(err.to_string()))?;
                truncated = !next.is_empty();
            }
            break;
        }
    }

    Ok(CachedFetch {
        url: final_url,
        status,
        headers,
        content_type,
        bytes: Arc::new(bytes),
        truncated,
        redirects,
    })
}

fn from_cached(payload: CachedFetch, cache_hit: bool, retries: usize) -> FetchedPayload {
    FetchedPayload {
        url: payload.url,
        status: payload.status,
        headers: payload.headers,
        content_type: payload.content_type,
        bytes: payload.bytes,
        truncated: payload.truncated,
        cache_hit,
        retries,
        redirects: payload.redirects,
    }
}

fn response_headers(headers: &reqwest::header::HeaderMap) -> BTreeMap<String, String> {
    headers
        .iter()
        .filter(|(name, _)| {
            !matches!(
                name.as_str(),
                "authorization"
                    | "proxy-authorization"
                    | "cookie"
                    | "set-cookie"
                    | "set-cookie2"
                    | "x-api-key"
                    | "api-key"
            )
        })
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.as_str().to_ascii_lowercase(), value.to_string()))
        })
        .collect()
}

fn is_transient_status(status: u16) -> bool {
    (500..600).contains(&status)
}

fn retry_delay() -> Duration {
    #[cfg(test)]
    return Duration::ZERO;

    #[cfg(not(test))]
    {
        let jitter_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| u64::from(duration.subsec_nanos()) % 41)
            .unwrap_or(0);
        Duration::from_millis(30 + jitter_ms)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::net::{IpAddr, Ipv4Addr};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

    use super::*;

    fn context(namespace: &str) -> ToolContext {
        ToolContext::new(".").with_state_namespace(namespace)
    }

    fn pin() -> DnsPin {
        Some((
            "public.example".to_string(),
            IpAddr::V4(Ipv4Addr::LOCALHOST),
        ))
    }

    #[derive(Clone)]
    struct FailOnce {
        calls: Arc<AtomicUsize>,
    }

    impl Respond for FailOnce {
        fn respond(&self, _request: &Request) -> ResponseTemplate {
            if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
                ResponseTemplate::new(503).set_body_json(json!({"error": "retry"}))
            } else {
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/plain")
                    .set_body_string("recovered response")
            }
        }
    }

    #[tokio::test]
    async fn transient_server_error_retries_once_then_caches() {
        let server = MockServer::start().await;
        let calls = Arc::new(AtomicUsize::new(0));
        Mock::given(method("GET"))
            .and(path("/retry"))
            .respond_with(FailOnce {
                calls: Arc::clone(&calls),
            })
            .mount(&server)
            .await;
        let url = format!("http://public.example:{}/retry", server.address().port());
        let options = FetchOptions::new(Duration::from_secs(5), 1_024, "text/plain");
        let context = context("fetch-retry-cache");

        let first = fetch_with_initial_pin(&url, &options, &context, "test", pin())
            .await
            .expect("retry succeeds");
        assert_eq!(first.status, 200);
        assert_eq!(first.retries, 1);
        assert!(!first.cache_hit);
        assert_eq!(&*first.bytes, b"recovered response");

        let second = fetch_with_initial_pin(&url, &options, &context, "test", pin())
            .await
            .expect("cache hit");
        assert!(second.cache_hit);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn truncated_cache_refetches_when_larger_body_is_requested() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/large"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/plain")
                    .set_body_string("0123456789"),
            )
            .mount(&server)
            .await;
        let url = format!("http://public.example:{}/large", server.address().port());
        let context = context("fetch-truncated-refetch");

        let small = fetch_with_initial_pin(
            &url,
            &FetchOptions::new(Duration::from_secs(5), 4, "text/plain"),
            &context,
            "test",
            pin(),
        )
        .await
        .expect("small fetch");
        assert_eq!(&*small.bytes, b"0123");
        assert!(small.truncated);

        let large = fetch_with_initial_pin(
            &url,
            &FetchOptions::new(Duration::from_secs(5), 16, "text/plain"),
            &context,
            "test",
            pin(),
        )
        .await
        .expect("larger refetch");
        assert_eq!(&*large.bytes, b"0123456789");
        assert!(!large.truncated);
        assert!(!large.cache_hit);
    }

    /// A Vercel-style edge that serves the client-side shell to an ordinary
    /// request and the prerendered page to a revalidating one (#5904).
    #[derive(Clone)]
    struct ShellUntilRevalidated {
        calls: Arc<AtomicUsize>,
        always_shell: bool,
    }

    const JS_SHELL_BODY: &str = "<html><head><title>Pricing</title></head><body><div id='root'></div><script>boot()</script></body></html>";
    const PRERENDERED_BODY: &str = "<html><head><title>Pricing</title></head><body><main><h1>Pricing</h1><p>The prerendered variant carries the full pricing table for every plan.</p></main></body></html>";

    impl Respond for ShellUntilRevalidated {
        fn respond(&self, request: &Request) -> ResponseTemplate {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let revalidating = request
                .headers
                .get("cache-control")
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| value.contains("no-cache"));
            if revalidating && !self.always_shell {
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/html")
                    .insert_header("x-vercel-cache", "HIT")
                    .insert_header("x-nextjs-prerender", "1")
                    .insert_header("age", "12")
                    .set_body_string(PRERENDERED_BODY)
            } else {
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/html")
                    .insert_header("x-vercel-cache", "MISS")
                    .set_body_string(JS_SHELL_BODY)
            }
        }
    }

    async fn js_shell_server(always_shell: bool) -> (MockServer, Arc<AtomicUsize>) {
        let server = MockServer::start().await;
        let calls = Arc::new(AtomicUsize::new(0));
        Mock::given(method("GET"))
            .and(path("/pricing"))
            .respond_with(ShellUntilRevalidated {
                calls: Arc::clone(&calls),
                always_shell,
            })
            .mount(&server)
            .await;
        (server, calls)
    }

    fn extract_html_document(
        payload: FetchedPayload,
    ) -> ExtractFuture<'static, super::super::extract::ExtractedDocument> {
        Box::pin(async move {
            super::super::extract::extract_document(
                &payload.url,
                Some(&payload.content_type),
                &payload.bytes,
                None,
            )
            .await
        })
    }

    #[tokio::test]
    async fn js_shell_is_refetched_past_every_cache_before_it_becomes_an_error() {
        let (server, calls) = js_shell_server(false).await;
        let url = format!("http://public.example:{}/pricing", server.address().port());
        let context = context("js-shell-recovers");

        let readable = fetch_readable_with_initial_pin(
            &url,
            &FetchOptions::new(Duration::from_secs(5), 65_536, "text/html"),
            &context,
            "fetch_url",
            pin(),
            extract_html_document,
        )
        .await
        .expect("the revalidated response carries the prerendered page");

        assert!(
            readable.document.markdown.contains("full pricing table"),
            "the second attempt's content must be what the caller receives: {}",
            readable.document.markdown
        );
        assert_eq!(calls.load(Ordering::SeqCst), 2, "exactly one extra request");
        assert_eq!(readable.attempts.len(), 2);
        assert!(!readable.attempts[0].cache_busted);
        assert!(!readable.attempts[0].produced_content);
        assert_eq!(
            readable.attempts[0].cache_headers.get("x-vercel-cache"),
            Some(&"MISS".to_string()),
            "the failing attempt keeps the header that explains its cache state"
        );
        assert!(readable.attempts[1].cache_busted);
        assert!(readable.attempts[1].produced_content);
        assert_eq!(
            readable.attempts[1].cache_headers.get("x-vercel-cache"),
            Some(&"HIT".to_string())
        );
        assert_eq!(
            readable.attempts[1].cache_headers.get("x-nextjs-prerender"),
            Some(&"1".to_string())
        );
        assert_eq!(
            readable.attempts[1].cache_headers.get("age"),
            Some(&"12".to_string())
        );
    }

    #[tokio::test]
    async fn two_shells_fail_with_the_escalation_the_calling_role_owns() {
        let (server, calls) = js_shell_server(true).await;
        let url = format!("http://public.example:{}/pricing", server.address().port());
        let options = FetchOptions::new(Duration::from_secs(5), 65_536, "text/html");

        let error = fetch_readable_with_initial_pin(
            &url,
            &options,
            &context("js-shell-browser-role"),
            "fetch_url",
            pin(),
            extract_html_document,
        )
        .await
        .expect_err("two shells must fail");
        let message = error.to_string();
        assert_eq!(calls.load(Ordering::SeqCst), 2, "no third request");
        assert!(
            message.contains("web.run"),
            "a role that has the browse surface must be told to use it: {message}"
        );
        assert!(
            message.contains("attempt 1")
                && message.contains("attempt 2 (Cache-Control: no-cache)"),
            "the failure receipt names both attempts: {message}"
        );
        assert!(
            message.contains("x-vercel-cache=MISS"),
            "the failure receipt carries the cache-state headers: {message}"
        );

        // A read-only worker whose envelope denies network keeps `Web{fetch}`
        // but loses `web.run` and any shell fallback, so the error must say so
        // instead of naming a surface the role cannot call.
        let mut denied = context("js-shell-read-only-role");
        denied.shell_policy = ShellPolicy::ReadOnly;
        denied.execution.tool_authority =
            Some(Arc::new(crate::tools::spec::ToolAuthorityEnvelope {
                schema_version: 1,
                owner: "scout".to_string(),
                authority: crate::tools::spec::ToolMutationAuthority::ReadOnly,
                network_access: Some(false),
                shell: crate::tools::spec::ToolShellAuthority::ReadOnly,
                verification: crate::tools::spec::ToolVerificationAuthority::None,
                writable_roots: Vec::new(),
                writable_files: Vec::new(),
                coordination_contracts: Vec::new(),
            }));
        let error = fetch_readable_with_initial_pin(
            &url,
            &options,
            &denied,
            "fetch_url",
            pin(),
            extract_html_document,
        )
        .await
        .expect_err("two shells must fail");
        let message = error.to_string();
        assert!(
            message.contains("not available to this role"),
            "a role without the browse surface must be told plainly: {message}"
        );
        assert!(
            message.contains("cannot fall back to a shell fetch"),
            "read-only roles must not be sent to curl: {message}"
        );
    }

    #[tokio::test]
    async fn transport_failures_do_not_earn_a_cache_busting_refetch() {
        let server = MockServer::start().await;
        let calls = Arc::new(AtomicUsize::new(0));
        Mock::given(method("GET"))
            .and(path("/flaky"))
            .respond_with(FailOnce {
                calls: Arc::clone(&calls),
            })
            .mount(&server)
            .await;
        let url = format!("http://public.example:{}/flaky", server.address().port());
        let context = context("js-shell-transport-retry");

        let readable = fetch_readable_with_initial_pin(
            &url,
            &FetchOptions::new(Duration::from_secs(5), 1_024, "text/plain"),
            &context,
            "fetch_url",
            pin(),
            |payload: FetchedPayload| {
                Box::pin(async move { Ok(String::from_utf8_lossy(&payload.bytes).into_owned()) })
            },
        )
        .await
        .expect("the existing transport retry still recovers");

        assert_eq!(readable.document, "recovered response");
        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "the 503 costs the existing single transport retry and nothing more"
        );
        assert_eq!(
            readable.attempts.len(),
            1,
            "a transport retry is not a readable-fetch attempt"
        );
        assert_eq!(readable.payload.retries, 1);
        assert!(!readable.attempts[0].cache_busted);
        assert!(readable.attempts[0].produced_content);
    }

    #[test]
    fn fetch_user_agent_tracks_the_crate_version() {
        assert!(
            USER_AGENT.contains(concat!("codewhale/", env!("CARGO_PKG_VERSION"))),
            "guarded-fetch UA must never pin a stale release: {USER_AGENT}"
        );
    }

    #[test]
    fn response_headers_drop_set_cookie_values() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("content-type", "text/plain".parse().unwrap());
        headers.insert("set-cookie", "session=secret".parse().unwrap());
        headers.insert("x-api-key", "secret".parse().unwrap());

        let filtered = response_headers(&headers);

        assert_eq!(
            filtered.get("content-type").map(String::as_str),
            Some("text/plain")
        );
        assert!(!filtered.contains_key("set-cookie"));
        assert!(!filtered.contains_key("x-api-key"));
    }

    #[tokio::test]
    async fn tightened_network_policy_blocks_an_existing_cache_entry() {
        use crate::network_policy::{Decision, NetworkPolicy, NetworkPolicyDecider};

        let url = reqwest::Url::parse("https://example.com/cached").unwrap();
        cache::insert(
            "policy-cache",
            &url,
            "text/plain",
            CachedFetch {
                url: url.to_string(),
                status: 200,
                headers: BTreeMap::new(),
                content_type: "text/plain".to_string(),
                bytes: Arc::new(b"cached".to_vec()),
                truncated: false,
                redirects: 0,
            },
        );
        let policy = NetworkPolicy {
            default: Decision::Deny.into(),
            allow: Vec::new(),
            deny: Vec::new(),
            proxy: Vec::new(),
            proxy_fake_ip_cidrs: Vec::new(),
            audit: false,
        };
        let context =
            context("policy-cache").with_network_policy(NetworkPolicyDecider::new(policy, None));

        let error = fetch_inner(
            url.as_str(),
            &FetchOptions::new(Duration::from_secs(1), 100, "text/plain"),
            &context,
            "fetch_url",
            None,
            CacheMode::Default,
        )
        .await
        .expect_err("policy must win over cache");
        assert!(error.to_string().contains("blocked by network policy"));
    }

    #[tokio::test]
    async fn tightened_network_policy_checks_cached_redirect_destination() {
        use crate::network_policy::{Decision, NetworkPolicy, NetworkPolicyDecider};

        let initial_url = reqwest::Url::parse("https://8.8.8.8/cached").unwrap();
        cache::insert(
            "redirect-policy-cache",
            &initial_url,
            "text/plain",
            CachedFetch {
                url: "https://1.1.1.1/redirected".to_string(),
                status: 200,
                headers: BTreeMap::new(),
                content_type: "text/plain".to_string(),
                bytes: Arc::new(b"cached".to_vec()),
                truncated: false,
                redirects: 1,
            },
        );
        let policy = NetworkPolicy {
            default: Decision::Allow.into(),
            allow: Vec::new(),
            deny: vec!["1.1.1.1".to_string()],
            proxy: Vec::new(),
            proxy_fake_ip_cidrs: Vec::new(),
            audit: false,
        };
        let context = context("redirect-policy-cache")
            .with_network_policy(NetworkPolicyDecider::new(policy, None));

        let error = fetch_inner(
            initial_url.as_str(),
            &FetchOptions::new(Duration::from_secs(1), 100, "text/plain"),
            &context,
            "fetch_url",
            None,
            CacheMode::Default,
        )
        .await
        .expect_err("final redirect policy must win over cache");
        assert!(error.to_string().contains("1.1.1.1"));
        assert!(error.to_string().contains("blocked by network policy"));
    }
}
