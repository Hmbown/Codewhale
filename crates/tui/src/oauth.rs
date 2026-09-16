//! The ONE access-route flow: Codex CLI import (read-only, consented) plus
//! the unified OAuth login core every subscription provider runs through.
//! Providers are data rows in the parameter table below, not modules.
//!
//! External Codex CLI credentials are read only after an exact, provider-scoped
//! consent grant. Codewhale never refreshes or rewrites that external file.
//!
//! # Security
//!
//! Token values are never logged or printed. All debug representations
//! redact sensitive fields.

use std::collections::BTreeMap;
use std::io::{Read, Write as _};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use codewhale_config::ExternalCredentialReadGrant;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::config::Config;

/// OAuth token payload stored in `auth.json`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
struct AuthTokens {
    access_token: Option<String>,
    account_id: Option<String>,
}

/// Top-level structure of Codex CLI's `auth.json`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
struct CodexAuthFile {
    tokens: Option<AuthTokens>,
}

/// Resolved OAuth credentials ready for API use.
#[derive(Debug, Clone)]
pub struct CodexCredentials {
    pub access_token: String,
    pub account_id: Option<String>,
}

/// JWT claims subset for expiry extraction.
#[derive(Debug, Deserialize)]
struct JwtClaims {
    exp: Option<u64>,
}

/// Resolve the path to the Codex auth file.
///
/// Priority:
/// 1. `OPENAI_CODEX_AUTH_FILE` env var
/// 2. `$CODEX_HOME/auth.json`
/// 3. `~/.codex/auth.json`
pub fn auth_file_path() -> PathBuf {
    if let Ok(path) = std::env::var("OPENAI_CODEX_AUTH_FILE") {
        let p = PathBuf::from(&path);
        if !p.as_os_str().is_empty() {
            return codewhale_config::resolve_external_credential_path(&p).unwrap_or(p);
        }
    }
    let codex_home = std::env::var("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            crate::config::effective_home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".codex")
        });
    let path = codex_home.join("auth.json");
    codewhale_config::resolve_external_credential_path(&path).unwrap_or(path)
}

/// Try to extract `exp` (epoch seconds) from a JWT without verifying
/// the signature. Returns `None` on any parse failure.
fn jwt_expiry_seconds(token: &str) -> Option<u64> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() < 2 {
        return None;
    }
    let payload = parts[1];
    let decoded = URL_SAFE_NO_PAD.decode(payload).ok()?;
    let claims: JwtClaims = serde_json::from_slice(&decoded).ok()?;
    claims.exp
}

/// Check whether an access token is expired, with a 60-second safety margin.
fn token_is_expired(access_token: &str) -> bool {
    match jwt_expiry_seconds(access_token) {
        Some(exp) => {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_secs();
            // 60-second safety margin
            now + 60 >= exp
        }
        // If we can't prove freshness, fail closed. External credentials are
        // never refreshed by Codewhale.
        None => true,
    }
}

/// Load Codex credentials from the auth file.
///
/// Returns `Ok(None)` if the file doesn't exist or has no usable tokens.
/// Returns `Err` only on parse/IO errors that aren't "file not found".
fn load_credentials(grant: &ExternalCredentialReadGrant) -> Result<Option<CodexCredentials>> {
    let Some(contents) = crate::external_credentials::read_to_string(grant)? else {
        return Ok(None);
    };
    let auth: CodexAuthFile = serde_json::from_str(&contents).map_err(|_| {
        anyhow::anyhow!(
            "Codex credential file {} is not valid credential JSON",
            codewhale_config::quote_os_path(grant.path())
        )
    })?;
    let tokens = match auth.tokens {
        Some(t) => t,
        None => return Ok(None),
    };
    let access_token = match tokens.access_token {
        Some(t) if !t.trim().is_empty() => t,
        _ => return Ok(None),
    };
    Ok(Some(CodexCredentials {
        access_token,
        account_id: tokens.account_id,
    }))
}

/// Prompt-free, non-refreshing readiness check for picker/onboarding surfaces.
/// It reads process-level token variables only; no file or network access occurs.
#[must_use]
pub fn credentials_from_env() -> Option<CodexCredentials> {
    ["OPENAI_CODEX_ACCESS_TOKEN", "CODEX_ACCESS_TOKEN"]
        .iter()
        .find_map(|name| {
            std::env::var(name)
                .ok()
                .filter(|token| !token.trim().is_empty())
        })
        .map(|access_token| CodexCredentials {
            access_token,
            account_id: codex_account_id_env(),
        })
}

/// Validate only the stored OAuth file, excluding token environment
/// overrides so config-vs-env provenance remains truthful.
///
/// This consumes a grant, so it can only run for a path the user explicitly
/// consented to. Consent is not a credential (#5772): status surfaces call
/// this to find out whether the consented file *still* holds a usable token,
/// because a record that outlives its token would otherwise read as stored.
#[must_use]
pub fn stored_credentials_present(grant: &ExternalCredentialReadGrant) -> bool {
    load_credentials(grant)
        .ok()
        .flatten()
        .is_some_and(|credentials| !token_is_expired(&credentials.access_token))
}

/// Load read-only credentials from the exact external path authorized by
/// `grant`. Expired tokens fail with guidance; they are never refreshed.
pub fn get_credentials(grant: &ExternalCredentialReadGrant) -> Result<CodexCredentials> {
    let creds =
        load_credentials(grant)?.with_context(|| missing_auth_message(OAuthProvider::Chatgpt))?;

    // Check if the access token is still valid.
    if !token_is_expired(&creds.access_token) {
        return Ok(creds);
    }

    bail!(
        "Codex access token in {} is expired. Read-only consent never refreshes or rewrites another CLI's credentials. Sign in with ChatGPT via `codewhale auth chatgpt`, run `codex login` again, or provide OPENAI_CODEX_ACCESS_TOKEN for this process.",
        codewhale_config::quote_os_path(grant.path())
    )
}

/// Read a ChatGPT account id from env overrides only.
fn codex_account_id_env() -> Option<String> {
    for var in ["OPENAI_CODEX_ACCOUNT_ID", "CODEX_ACCOUNT_ID"] {
        if let Ok(value) = std::env::var(var) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

// ── ONE access-route flow ─────────────────────────────────────────────
// Providers are DATA, not files. Every subscription login — xAI device
// code today, ChatGPT PKCE next — runs through the parameter table below
// and the shared device-code core; per-provider OAuth modules are deleted,
// not repaired.

/// How this run reaches a provider: an OAuth login Codewhale owns, a
/// read-only import from another CLI, a pasted key, or (reserved) an ACP
/// subscription bridge. The bridge arm lands with the ACP work; until then
/// it resolves to a clear error, never a silent fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(
    dead_code,
    reason = "3b-ii wires the access-route dispatch that consumes this"
)]
pub enum AccessMethod {
    /// Browser/device OAuth login whose tokens Codewhale stores and refreshes.
    OwnedOAuth(OAuthProvider),
    /// Read-only credentials owned by another CLI, behind a consent grant.
    ExternalImport(ExternalImportSource),
    /// A pasted API key. No login, no refresh, no storage beyond config.
    ApiKey,
    /// Subscription access through an ACP bridge (Antigravity, Copilot).
    /// Reserved: no producer yet.
    AcpBridge,
}

/// Providers with an OAuth login Codewhale can own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OAuthProvider {
    Xai,
    /// No device-code producer yet; the PKCE path (3b-i(b)) constructs this.
    #[allow(
        dead_code,
        reason = "3b-i(b) wires the PKCE login that constructs this"
    )]
    Chatgpt,
}

impl AccessMethod {
    /// Short user-facing name for picker and status surfaces.
    #[must_use]
    #[allow(
        dead_code,
        reason = "3b-ii wires the access-route dispatch that consumes this"
    )]
    pub fn label(&self) -> &'static str {
        match self {
            AccessMethod::OwnedOAuth(OAuthProvider::Xai) => "xAI subscription",
            AccessMethod::OwnedOAuth(OAuthProvider::Chatgpt) => "ChatGPT subscription",
            AccessMethod::ExternalImport(ExternalImportSource::GrokCli) => "Grok CLI import",
            AccessMethod::ExternalImport(ExternalImportSource::CodexCli) => "Codex CLI import",
            AccessMethod::ExternalImport(ExternalImportSource::Antigravity) => "Antigravity import",
            AccessMethod::ExternalImport(ExternalImportSource::Dsh) => "DSH import",
            AccessMethod::ApiKey => "API key",
            AccessMethod::AcpBridge => "ACP bridge",
        }
    }
}

/// Another CLI whose credentials can be imported read-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code, reason = "3b-ii wires the import table that consumes this")]
pub enum ExternalImportSource {
    /// `~/.grok/auth.json` / `XAI_AUTH_PATH`, keyed by issuer::client-id.
    GrokCli,
    /// `~/.codex/auth.json`, `{tokens:{access_token, account_id}}`.
    CodexCli,
    /// Antigravity `state.vscdb` SQLite row, opened read-only.
    Antigravity,
    /// `$DSH_HOME/.credentials.yaml` flat mapping, `DEEPSEEK_API_KEY`.
    Dsh,
}

/// Test/dev knobs a provider reads from the environment. Data, so a new
/// provider adds rows here instead of a new module.
pub struct OAuthEnvOverrides {
    pub issuer_vars: &'static [&'static str],
    pub client_id_vars: &'static [&'static str],
    pub scope_vars: &'static [&'static str],
    pub no_browser_var: &'static str,
}

/// Everything about one provider's OAuth login that is not logic.
pub struct OAuthProviderParams {
    /// Human name for prompts and errors: "xAI", "ChatGPT".
    pub display_name: &'static str,
    pub default_issuer: &'static str,
    pub default_client_id: &'static str,
    pub default_scopes: &'static str,
    pub env: OAuthEnvOverrides,
    /// `Some` device-authorization path under the issuer (xAI); `None`
    /// means the issuer offers no device flow and device login must fail
    /// loudly instead of guessing (ChatGPT).
    pub device_code_path: Option<&'static str>,
    /// `Some` browser authorization path under the issuer (ChatGPT PKCE);
    /// `None` means the issuer offers no browser flow and browser login
    /// fails the same loud way (xAI is device-code only).
    pub authorize_path: Option<&'static str>,
    /// Token path under the issuer.
    pub token_path: &'static str,
    /// Whether the issuer was discovered (xAI) or pinned (ChatGPT paths).
    pub discover_endpoints: bool,
    /// Seconds the device-code poll runs past the server's `expires_in`.
    pub device_poll_floor_secs: u64,
    /// Extra authorize-endpoint parameters beyond the standard OAuth set,
    /// sent verbatim so the issuer sees exactly who is calling.
    pub authorize_extras: &'static [(&'static str, &'static str)],
    /// Honest client identity for issuers that require one (ChatGPT's
    /// `originator`). Never impersonate another CLI.
    pub originator: Option<&'static str>,
    /// Remote revoke path under the issuer, pinned rather than discovered:
    /// revoke must still clear local credentials when the issuer is
    /// unreachable, so a discovery fetch would only add a failure mode to a
    /// path whose contract is to clean up regardless. `None` when
    /// revocation is purely local (xAI).
    pub revoke_path: Option<&'static str>,
    /// Registered loopback redirect for browser flows.
    pub callback_path: &'static str,
    /// Loopback ports the public client registered, in preference order.
    pub loopback_ports: &'static [u16],
    /// The command that re-runs this provider's login, for error guidance.
    pub relogin_hint: &'static str,
    /// What to tell the user when every callback port is taken.
    pub callback_conflict_hint: &'static str,
}

pub const XAI_OAUTH_PARAMS: OAuthProviderParams = OAuthProviderParams {
    display_name: "xAI",
    // Single source: the legacy module still owns these strings until its
    // activation path unifies and they move here in 3b-iii.
    default_issuer: XAI_OIDC_ISSUER,
    default_client_id: GROK_OIDC_CLIENT_ID,
    default_scopes: DEFAULT_SCOPES,
    env: OAuthEnvOverrides {
        issuer_vars: &["GROK_OIDC_ISSUER", "XAI_OIDC_ISSUER"],
        client_id_vars: &["GROK_OIDC_CLIENT_ID", "XAI_OIDC_CLIENT_ID"],
        scope_vars: &["GROK_OIDC_SCOPES", "XAI_OIDC_SCOPES"],
        no_browser_var: "CODEWHALE_XAI_OAUTH_NO_BROWSER",
    },
    device_code_path: Some("oauth2/device/code"),
    authorize_path: None,
    token_path: "oauth2/token",
    discover_endpoints: true,
    device_poll_floor_secs: 30,
    authorize_extras: &[],
    originator: None,
    revoke_path: None,
    callback_path: "",
    loopback_ports: &[],
    relogin_hint: "codewhale auth xai-device",
    callback_conflict_hint: "",
};

pub const CHATGPT_OAUTH_PARAMS: OAuthProviderParams = OAuthProviderParams {
    display_name: "ChatGPT",
    // Single source: same arrangement as the xAI row above.
    default_issuer: CHATGPT_OAUTH_ISSUER,
    default_client_id: CHATGPT_OAUTH_CLIENT_ID,
    default_scopes: CHATGPT_OAUTH_SCOPE,
    originator: Some(CHATGPT_OAUTH_ORIGINATOR),
    env: OAuthEnvOverrides {
        issuer_vars: &["CODEWHALE_CHATGPT_OAUTH_ISSUER"],
        client_id_vars: &["CODEWHALE_CHATGPT_OAUTH_CLIENT_ID"],
        scope_vars: &[],
        no_browser_var: "CODEWHALE_CHATGPT_OAUTH_NO_BROWSER",
    },
    device_code_path: None,
    authorize_path: Some("oauth/authorize"),
    token_path: "oauth/token",
    discover_endpoints: false,
    device_poll_floor_secs: 30,
    authorize_extras: &[("id_token_add_organizations", "true")],
    revoke_path: Some("api/accounts/oauth/revoke"),
    callback_path: "/auth/callback",
    loopback_ports: &[1455, 1457],
    relogin_hint: "codewhale auth chatgpt",
    callback_conflict_hint: "Stop the process holding that port, or import Codex CLI credentials with `codewhale auth external-consent`.",
};

/// The parameter table. A provider login looks its row up here; adding a
/// provider means adding a row, never a module.
#[must_use]
pub fn oauth_provider_params(provider: OAuthProvider) -> &'static OAuthProviderParams {
    match provider {
        OAuthProvider::Xai => &XAI_OAUTH_PARAMS,
        OAuthProvider::Chatgpt => &CHATGPT_OAUTH_PARAMS,
    }
}

/// Resolved login inputs: schema defaults, environment-tested in order.
pub struct ResolvedOAuthInputs {
    pub issuer: String,
    pub client_id: String,
    pub scopes: String,
    pub open_browser: bool,
}

impl OAuthProviderParams {
    /// Resolve issuer/client/scopes from the environment, first var wins.
    #[must_use]
    pub fn resolve_inputs(&self) -> ResolvedOAuthInputs {
        let first_set = |vars: &[&str], fallback: &str| {
            vars.iter()
                .filter_map(|var| std::env::var(var).ok())
                .find(|value| !value.trim().is_empty())
                .unwrap_or_else(|| fallback.to_string())
        };
        ResolvedOAuthInputs {
            issuer: first_set(self.env.issuer_vars, self.default_issuer),
            client_id: first_set(self.env.client_id_vars, self.default_client_id),
            scopes: first_set(self.env.scope_vars, self.default_scopes),
            open_browser: std::env::var_os(self.env.no_browser_var).is_none(),
        }
    }
}

/// Token material from a completed grant. No Debug: the tokens never print.
/// `interval` rides along because a `slow_down` error response may carry
/// the server's new minimum, which the loop prefers over its own tracked
/// value (RFC 8628 §3.5; WSL/VM clock drift).
#[derive(Clone, Deserialize)]
pub struct OAuthTokenMaterial {
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub expires_in: Option<u64>,
    /// OpenID Connect id token; carries the account claim when issued.
    #[serde(default)]
    pub id_token: Option<String>,
    #[serde(default)]
    pub interval: Option<u64>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub error_description: Option<String>,
}

/// A completed login awaiting activation (owned-generation commit).
/// The provider is the table row it resolved through; unified activation
/// (3b-ii) matches on it.
pub struct PendingOAuthLogin {
    #[allow(dead_code, reason = "3b-ii unified activation matches on this")]
    pub provider: OAuthProvider,
    pub issuer: String,
    pub client_id: String,
    pub token: OAuthTokenMaterial,
}

/// Device-authorization response. Error fields ride along so a refused
/// grant classifies instead of failing to parse.
#[derive(Clone, Deserialize)]
struct DeviceGrantResponse {
    device_code: Option<String>,
    user_code: Option<String>,
    verification_uri: Option<String>,
    verification_uri_complete: Option<String>,
    expires_in: Option<u64>,
    interval: Option<u64>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    error_description: Option<String>,
}

/// Bounds copied with the behavior: 20 s requests, 64 KiB bodies, 256 B of
/// error detail. A server that will not fit in the budget is an error, and
/// error text is whitespace-collapsed so a hostile endpoint cannot smuggle
/// terminal controls into diagnostics.
const OAUTH_REQUEST_TIMEOUT_SECS: u64 = 20;
const OAUTH_RESPONSE_BODY_LIMIT: u64 = 64 * 1024;
const OAUTH_ERROR_DETAIL_LIMIT: usize = 256;

/// Apply the existing OAuth browser URI policy to the URL that reqwest will
/// actually use. Parsing first keeps transport and loopback interpretation
/// identical; an issuer override does not authorize remote plaintext forms.
fn oauth_endpoint_url(raw: &str) -> Result<reqwest::Url> {
    let url = reqwest::Url::parse(raw).context("OAuth endpoint is not a valid URL")?;
    codewhale_config::device_code::validate_browser_verification_uri(
        url.as_str(),
        "OAuth endpoint",
    )
    .context("OAuth endpoints require HTTPS, except for local loopback HTTP")?;
    Ok(url)
}

fn oauth_http_client(purpose: &str) -> Result<reqwest::blocking::Client> {
    crate::tls::reqwest_blocking_client_builder()
        // An issuer-approved endpoint cannot delegate credential-bearing forms
        // to a redirect destination, including HTTPS-to-HTTP downgrades.
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(OAUTH_REQUEST_TIMEOUT_SECS))
        .build()
        .with_context(|| format!("Failed to build OAuth {purpose} client"))
}

fn parse_oauth_json<T: serde::de::DeserializeOwned>(
    response: reqwest::blocking::Response,
    operation: &str,
) -> Result<(reqwest::StatusCode, T)> {
    let status = response.status();
    // Join every content-type value: some test doubles stack a second one
    // next to the body's implicit type, and the diagnostic must name what
    // the server actually sent, not whichever header won the map lookup.
    let content_type = {
        let joined = response
            .headers()
            .get_all(reqwest::header::CONTENT_TYPE)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .collect::<Vec<_>>()
            .join(", ");
        if joined.is_empty() {
            "missing".to_string()
        } else {
            joined
        }
    };
    let mut reader = response.take(OAUTH_RESPONSE_BODY_LIMIT + 1);
    let mut body = Vec::new();
    reader
        .read_to_end(&mut body)
        .with_context(|| format!("reading {operation} response"))?;
    let truncated = body.len() as u64 > OAUTH_RESPONSE_BODY_LIMIT;
    if truncated {
        body.truncate(OAUTH_RESPONSE_BODY_LIMIT as usize);
    }
    let parsed = serde_json::from_slice(&body).map_err(|_| {
        let limit = if truncated {
            " (body exceeded the 64 KiB diagnostic limit)"
        } else {
            ""
        };
        anyhow::anyhow!(
            "{operation} returned HTTP {status} with content type {content_type}; expected JSON{limit}"
        )
    })?;
    Ok((status, parsed))
}

fn bounded_oauth_error_text(raw: &str) -> String {
    let mut output = String::with_capacity(raw.len().min(OAUTH_ERROR_DETAIL_LIMIT));
    let mut previous_was_space = false;
    let mut written = 0;
    for character in raw.chars() {
        let character = if character.is_whitespace() {
            ' '
        } else if character.is_control() {
            continue;
        } else {
            character
        };
        if character == ' ' && previous_was_space {
            continue;
        }
        if written == OAUTH_ERROR_DETAIL_LIMIT {
            break;
        }
        output.push(character);
        previous_was_space = character == ' ';
        written += 1;
    }
    output.trim().to_string()
}

fn oauth_failure_detail(
    error: Option<&str>,
    description: Option<&str>,
    status: reqwest::StatusCode,
) -> String {
    let mut code = bounded_oauth_error_text(error.unwrap_or("request_failed"));
    if code.is_empty() {
        code = "request_failed".to_string();
    }
    let description = description
        .map(bounded_oauth_error_text)
        .filter(|description| !description.is_empty() && description != &code);
    match description {
        Some(description) => format!("{code}: {description}; HTTP {status}"),
        None => format!("{code}; HTTP {status}"),
    }
}

/// Resolved token + device endpoints for one login.
#[derive(Clone, Debug, PartialEq, Eq)]
struct OAuthEndpoints {
    device_authorization_endpoint: Option<String>,
    token_endpoint: String,
}

/// OIDC discovery document. Only the fields a login needs.
#[derive(Clone, Deserialize)]
struct OidcDiscoveryDocument {
    issuer: Option<String>,
    device_authorization_endpoint: Option<String>,
    token_endpoint: Option<String>,
}

/// Resolve endpoints: OIDC discovery when the provider row asks for it,
/// documented-path fallback otherwise — and fallback on ANY discovery
/// failure, loudly logged. A hostile or broken discovery document must
/// never brick login; it only loses the custom endpoints.
fn resolve_oauth_endpoints(params: &OAuthProviderParams, issuer: &str) -> OAuthEndpoints {
    let fallback = || fallback_oauth_endpoints(params, issuer);
    if !params.discover_endpoints {
        return fallback();
    }
    match discover_oauth_endpoints(params, issuer) {
        Ok(endpoints) => endpoints,
        Err(err) => {
            tracing::warn!(
                target: "codewhale::oauth",
                error = %err,
                "{} OIDC discovery failed; using documented endpoint fallback",
                params.display_name
            );
            fallback()
        }
    }
}

fn discover_oauth_endpoints(params: &OAuthProviderParams, issuer: &str) -> Result<OAuthEndpoints> {
    let name = params.display_name;
    let discovery_url = oauth_endpoint_url(&format!(
        "{}/.well-known/openid-configuration",
        issuer.trim_end_matches('/')
    ))?;
    let client = oauth_http_client("OIDC discovery")?;
    #[cfg(test)]
    crate::external_credentials::record_oauth_network();
    let response = client
        .get(discovery_url)
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .with_context(|| format!("{name} OIDC discovery request failed"))?;
    let (status, discovery): (_, OidcDiscoveryDocument) =
        parse_oauth_json(response, &format!("{name} OIDC discovery"))?;
    if !status.is_success() {
        bail!("{name} OIDC discovery failed with HTTP {status}");
    }
    validate_discovered_issuer(discovery.issuer, issuer)
        .with_context(|| format!("{name} OIDC discovery"))?;
    Ok(OAuthEndpoints {
        device_authorization_endpoint: params
            .device_code_path
            .map(|_| {
                validate_discovered_oauth_endpoint(
                    discovery.device_authorization_endpoint,
                    "device_authorization_endpoint",
                    issuer,
                )
            })
            .transpose()?,
        token_endpoint: validate_discovered_oauth_endpoint(
            discovery.token_endpoint,
            "token_endpoint",
            issuer,
        )?,
    })
}

/// Validate that an OIDC discovery document's issuer matches the requested issuer.
fn validate_discovered_issuer(discovered: Option<String>, expected: &str) -> Result<()> {
    let discovered = discovered
        .as_deref()
        .map(str::trim)
        .filter(|issuer| !issuer.is_empty())
        .context("OIDC discovery missing issuer")?;
    if discovered.trim_end_matches('/') != expected.trim_end_matches('/') {
        bail!("OIDC discovery issuer does not match the requested issuer");
    }
    let _ = oauth_endpoint_url(expected).context("OIDC issuer is not a trusted URL")?;
    Ok(())
}

/// Validate one discovered endpoint against the issuer: https-or-http scheme,
/// no plaintext downgrade, no embedded credentials, same origin.
fn validate_discovered_oauth_endpoint(
    endpoint: Option<String>,
    field: &str,
    issuer: &str,
) -> Result<String> {
    let endpoint = endpoint
        .as_deref()
        .map(str::trim)
        .filter(|endpoint| !endpoint.is_empty())
        .with_context(|| format!("OIDC discovery missing {field}"))?;
    let parsed = reqwest::Url::parse(endpoint)
        .with_context(|| format!("OIDC discovery returned an invalid {field}"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        bail!("OIDC discovery returned unsupported {field} scheme");
    }
    let issuer = oauth_endpoint_url(issuer).context("OIDC issuer is not a trusted URL")?;
    if issuer.scheme() == "https" && parsed.scheme() != "https" {
        bail!("OIDC discovery attempted to downgrade {field} from HTTPS");
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        bail!("OIDC discovery returned credentials in {field}");
    }
    if parsed.origin() != issuer.origin() {
        bail!("OIDC discovery returned {field} on a different origin than the issuer");
    }
    let _ = oauth_endpoint_url(parsed.as_str())?;
    Ok(endpoint.to_string())
}

/// Documented-path endpoints for a provider row, no discovery.
fn fallback_oauth_endpoints(params: &OAuthProviderParams, issuer: &str) -> OAuthEndpoints {
    OAuthEndpoints {
        device_authorization_endpoint: params
            .device_code_path
            .map(|path| format!("{}/{}", issuer.trim_end_matches('/'), path)),
        token_endpoint: format!("{}/{}", issuer.trim_end_matches('/'), params.token_path),
    }
}

/// POST a device-authorization request. Pure transport over an explicit
/// endpoint: discovery (or its absence) is the caller's decision.
fn request_device_grant(
    device_authorization_endpoint: &str,
    client_id: &str,
    scopes: &str,
) -> Result<DeviceGrantResponse> {
    let device_authorization_endpoint = oauth_endpoint_url(device_authorization_endpoint)?;
    let client = oauth_http_client("device-code")?;
    let params = [("client_id", client_id), ("scope", scopes)];
    #[cfg(test)]
    crate::external_credentials::record_oauth_network();
    let response = client
        .post(device_authorization_endpoint)
        .form(&params)
        .send()
        .context("OAuth device-code request failed")?;
    let (status, body): (_, DeviceGrantResponse) =
        parse_oauth_json(response, "OAuth device-code request")?;
    if !status.is_success() || body.error.is_some() {
        let detail = oauth_failure_detail(
            body.error.as_deref(),
            body.error_description.as_deref(),
            status,
        );
        bail!("OAuth device-code request failed ({detail})");
    }
    if body
        .device_code
        .as_deref()
        .is_some_and(|code| !code.trim().is_empty())
        && body
            .user_code
            .as_deref()
            .is_some_and(|code| !code.trim().is_empty())
    {
        return Ok(body);
    }
    bail!("OAuth device-code request returned success without a device and user code");
}

/// Poll the token endpoint once, classifying the RFC 8628 outcome. Matches
/// the legacy per-provider poll so the ported tests pin identical behavior.
fn poll_device_grant(
    token_endpoint: &str,
    client_id: &str,
    device_code: &str,
) -> Result<codewhale_config::device_code::DevicePollOutcome<OAuthTokenMaterial>> {
    use codewhale_config::device_code::DevicePollOutcome;
    let token_endpoint = oauth_endpoint_url(token_endpoint)?;
    let client = oauth_http_client("device-code poll")?;
    let params = [
        ("client_id", client_id),
        ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ("device_code", device_code),
    ];
    #[cfg(test)]
    crate::external_credentials::record_oauth_network();
    let response = client
        .post(token_endpoint)
        .form(&params)
        .send()
        .context("OAuth device-code poll failed")?;
    let (status, body): (_, OAuthTokenMaterial) =
        parse_oauth_json(response, "OAuth device-code poll")?;
    if status.is_success() && body.error.is_none() {
        return Ok(DevicePollOutcome::Complete(body));
    }
    match body.error.as_deref().unwrap_or("") {
        "authorization_pending" => Ok(DevicePollOutcome::Pending),
        "slow_down" => Ok(DevicePollOutcome::SlowDown {
            interval_seconds: body.interval,
        }),
        _ => {
            let detail = oauth_failure_detail(
                body.error.as_deref(),
                body.error_description.as_deref(),
                status,
            );
            bail!("OAuth device-code poll failed ({detail})");
        }
    }
}

/// Interactive device-code login for any provider whose row offers it.
/// Prints the verification URL + user code to stderr and polls until
/// approved. A provider with no device flow (ChatGPT) fails here with the
/// reason, instead of deep in transport code.
pub async fn device_code_login(provider: OAuthProvider) -> Result<PendingOAuthLogin> {
    // Endpoint resolution does blocking HTTP (discovery): it must run on the
    // blocking worker, never on the async executor. Providers with no device
    // flow fail here, before any thread spawns and before any network.
    let params = oauth_provider_params(provider);
    if params.device_code_path.is_none() {
        bail!(
            "{} offers no device-code flow; sign in through the browser login instead",
            params.display_name
        );
    }
    let inputs = params.resolve_inputs();
    let display_name = params.display_name;
    tokio::task::spawn_blocking(move || device_code_login_with(provider, &inputs))
        .await
        .with_context(|| format!("{display_name} device-code login worker failed"))?
}

/// Blocking worker body for [`device_code_login`]. `pub(crate)` so the
/// legacy activation tests can drive the unified login end to end until
/// activation unifies in 3b-ii.
pub(crate) fn device_code_login_with(
    provider: OAuthProvider,
    inputs: &ResolvedOAuthInputs,
) -> Result<PendingOAuthLogin> {
    let params = oauth_provider_params(provider);
    let display_name = params.display_name;
    let endpoints = resolve_oauth_endpoints(params, &inputs.issuer);
    let Some(device_endpoint) = endpoints.device_authorization_endpoint else {
        bail!(
            "{display_name} offers no device-code flow; sign in through the browser login instead"
        );
    };
    let token_endpoint = endpoints.token_endpoint;
    let poll_floor_secs = params.device_poll_floor_secs;
    let grant = request_device_grant(&device_endpoint, &inputs.client_id, &inputs.scopes)?;
    let verify = grant
        .verification_uri_complete
        .clone()
        .or(grant.verification_uri.clone())
        .unwrap_or_else(|| format!("{}/device", inputs.issuer.trim_end_matches('/')));
    // Off the wire, headed for `webbrowser::open`: must be a bare
    // navigation, never a scheme or credential smuggle.
    let verify = codewhale_config::device_code::validate_browser_verification_uri(
        &verify,
        &format!("{display_name} device-code request"),
    )?;
    let user_code = grant.user_code.unwrap_or_default();

    eprintln!("{display_name} device-code login");
    eprintln!("  Open:  {verify}");
    eprintln!("  Code:  {user_code}");
    eprintln!("Waiting for approval in the browser… (Ctrl+C to abort)");
    if inputs.open_browser
        && let Err(err) = webbrowser::open(&verify)
    {
        eprintln!("Could not open the browser automatically: {err}");
    }

    let lifetime = Duration::from_secs(
        grant
            .expires_in
            .unwrap_or(DEVICE_POLL_MAX_SECS)
            .max(poll_floor_secs),
    );
    let token = codewhale_config::device_code::DeviceCodePoll::new(
        lifetime,
        format!(
            "{display_name} device-code authorization timed out. Re-run device login \
             and approve the code before it expires."
        ),
    )
    .interval_seconds(grant.interval)
    .wait_before_first_poll(true)
    .slow_down_timeout_message(format!(
        "{display_name} device-code authorization timed out after one or more slow_down \
         responses. That is usually clock drift in a WSL or VM environment; \
         sync the clock, then re-run device login and approve the code before \
         it expires."
    ))
    .run(std::thread::sleep, || {
        poll_device_grant(
            &token_endpoint,
            &inputs.client_id,
            grant.device_code.as_deref().unwrap_or(""),
        )
    })?;

    Ok(PendingOAuthLogin {
        provider,
        issuer: inputs.issuer.clone(),
        client_id: inputs.client_id.clone(),
        token,
    })
}

/// One login entry point for every provider: the params row decides whether
/// the grant is device-code or browser PKCE. A provider with neither fails
/// here with the reason.
pub async fn login(provider: OAuthProvider) -> Result<PendingOAuthLogin> {
    let params = oauth_provider_params(provider);
    if params.device_code_path.is_some() {
        device_code_login(provider).await
    } else if params.authorize_path.is_some() {
        pkce_login(provider).await
    } else {
        bail!("{} offers no sign-in flow", params.display_name);
    }
}

// ── form-post transport seam ──────────────────────────────────────────
//
// One seam for every OAuth form post (PKCE exchange, refresh, revoke): the
// production client is reqwest with the shared bounds; tests substitute a
// mock issuer. The seam records network/refresh in test builds so the
// side-effect trap can still prove "zero external I/O" assertions.

pub(crate) trait OAuthFormClient {
    fn post_form(&self, url: &str, form: &[(&str, &str)]) -> Result<(u16, String)>;
}

pub(crate) struct ReqwestOAuthFormClient;

impl OAuthFormClient for ReqwestOAuthFormClient {
    fn post_form(&self, url: &str, form: &[(&str, &str)]) -> Result<(u16, String)> {
        let url = oauth_endpoint_url(url)?;
        #[cfg(test)]
        crate::external_credentials::record_oauth_network();
        let client = oauth_http_client("form")?;
        let response = client
            .post(url)
            .form(form)
            .send()
            .context("OAuth form request failed")?;
        let status = response.status().as_u16();
        let mut reader = response.take(OAUTH_RESPONSE_BODY_LIMIT + 1);
        let mut body = Vec::new();
        reader
            .read_to_end(&mut body)
            .context("reading OAuth form response")?;
        if body.len() as u64 > OAUTH_RESPONSE_BODY_LIMIT {
            body.truncate(OAUTH_RESPONSE_BODY_LIMIT as usize);
        }
        Ok((status, String::from_utf8(body).unwrap_or_default()))
    }
}

/// Token/authorize endpoint URLs for one provider row.
pub(crate) fn form_token_url(params: &OAuthProviderParams, issuer: &str) -> String {
    format!("{}/{}", issuer.trim_end_matches('/'), params.token_path)
}

/// Pinned remote revoke URL; `None` when the provider revokes locally only.
pub(crate) fn remote_revoke_url(params: &OAuthProviderParams, issuer: &str) -> Option<String> {
    params
        .revoke_path
        .map(|path| format!("{}/{}", issuer.trim_end_matches('/'), path))
}

/// Parse a form-post token response. Error bodies are never echoed: the
/// detail names the error code only, so a hostile issuer cannot smuggle
/// secret-bearing text back through diagnostics.
pub(crate) fn parse_oauth_form_response(
    status: u16,
    body: &str,
    operation: &str,
    params: &OAuthProviderParams,
) -> Result<OAuthTokenMaterial> {
    let name = params.display_name;
    let parsed: OAuthTokenMaterial = serde_json::from_str(body).map_err(|_| {
        anyhow::anyhow!("{name} OAuth {operation} returned HTTP {status} that was not token JSON")
    })?;
    if !(200..300).contains(&status) || parsed.error.is_some() {
        let err = parsed.error.as_deref().unwrap_or("token_error");
        if matches!(
            err,
            "invalid_grant"
                | "refresh_token_reused"
                | "refresh_token_expired"
                | "refresh_token_invalidated"
        ) || status == 401
        {
            bail!(
                "{name} OAuth {operation} failed permanently ({err}). Sign in again with `{}`.",
                params.relogin_hint
            );
        }
        bail!("{name} OAuth {operation} failed ({err})");
    }
    anyhow::ensure!(
        parsed
            .access_token
            .as_deref()
            .is_some_and(|token| !token.trim().is_empty()),
        "{name} OAuth {operation} returned an empty access token"
    );
    Ok(parsed)
}

fn compact_form_error(body: &str) -> String {
    body.chars().filter(|c| !c.is_control()).take(80).collect()
}

/// Refresh an owned token through the seam at an explicit token URL —
/// discovered when the provider row demands it, pinned otherwise. Refresh is
/// a Codewhale-owned credential operation only: external imports never
/// refresh.
pub(crate) fn refresh_access_token_via(
    client: &dyn OAuthFormClient,
    params: &OAuthProviderParams,
    token_url: &str,
    client_id: &str,
    refresh_token: &str,
) -> Result<OAuthTokenMaterial> {
    #[cfg(test)]
    crate::external_credentials::record_oauth_refresh();
    let (status, body) = client.post_form(
        token_url,
        &[
            ("grant_type", "refresh_token"),
            ("client_id", client_id),
            ("refresh_token", refresh_token),
        ],
    )?;
    parse_oauth_form_response(status, &body, "refresh", params)
}

/// Best-effort remote revoke through the seam. Callers clear local
/// credentials regardless of this outcome.
pub(crate) fn revoke_remote_token_via(
    client: &dyn OAuthFormClient,
    params: &OAuthProviderParams,
    issuer: &str,
    client_id: &str,
    token: &str,
) -> Result<()> {
    let Some(revoke_url) = remote_revoke_url(params, issuer) else {
        bail!("{} has no remote revoke endpoint", params.display_name);
    };
    let (status, body) =
        client.post_form(&revoke_url, &[("token", token), ("client_id", client_id)])?;
    if !(200..300).contains(&status) {
        bail!(
            "{} OAuth revoke failed with HTTP {status}: {}",
            params.display_name,
            compact_form_error(&body)
        );
    }
    Ok(())
}

// ── PKCE browser login ────────────────────────────────────────────────

/// RFC 7636 S256 PKCE pair. Custom Debug: the verifier is exchanged for
/// bearer material and never prints.
#[derive(Clone)]
pub struct PkceChallenge {
    pub verifier: String,
    pub challenge: String,
}

impl std::fmt::Debug for PkceChallenge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PkceChallenge")
            .field("verifier", &"<redacted>")
            .field("challenge", &self.challenge)
            .finish()
    }
}

/// A browser authorization request in flight: state, PKCE pair, and the
/// registered redirect the callback server answers on.
#[derive(Clone)]
pub struct BrowserAuthRequest {
    pub state: String,
    pub pkce: PkceChallenge,
    pub redirect_uri: String,
    pub authorize_url: String,
}

impl std::fmt::Debug for BrowserAuthRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrowserAuthRequest")
            .field("state", &self.state)
            .field("pkce", &self.pkce)
            .field("redirect_uri", &self.redirect_uri)
            .field("authorize_url", &self.authorize_url)
            .finish()
    }
}

/// Parsed callback query: a code+state pair, or the issuer's refusal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallbackOutcome {
    Success {
        code: String,
        state: String,
    },
    Error {
        error: String,
        description: Option<String>,
        state: Option<String>,
    },
}

/// RFC 7636 S256 PKCE pair.
#[must_use]
pub fn generate_pkce() -> PkceChallenge {
    let verifier = random_url_token(32);
    let digest = Sha256::digest(verifier.as_bytes());
    PkceChallenge {
        verifier,
        challenge: URL_SAFE_NO_PAD.encode(digest),
    }
}

#[must_use]
pub fn generate_state() -> String {
    random_url_token(16)
}

fn random_url_token(nbytes: usize) -> String {
    let mut bytes = vec![0u8; nbytes.max(16)];
    let mut offset = 0;
    while offset < bytes.len() {
        let chunk = uuid::Uuid::new_v4();
        let take = (bytes.len() - offset).min(16);
        bytes[offset..offset + take].copy_from_slice(&chunk.as_bytes()[..take]);
        offset += take;
    }
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn build_authorize_url(
    params: &OAuthProviderParams,
    issuer: &str,
    client_id: &str,
    scopes: &str,
    redirect_uri: &str,
    state: &str,
    pkce: &PkceChallenge,
) -> Result<String> {
    let Some(authorize_path) = params.authorize_path else {
        bail!("{} offers no browser sign-in flow", params.display_name);
    };
    // A malformed configured issuer must fail loudly. Silently redirecting
    // the browser to the production authorize endpoint would hand the
    // issuer a sign-in the user aimed somewhere else.
    let issuer_var = params
        .env
        .issuer_vars
        .first()
        .copied()
        .unwrap_or("the issuer environment variable");
    let mut url = oauth_endpoint_url(&format!(
        "{}/{}",
        issuer.trim_end_matches('/'),
        authorize_path
    ))
    .with_context(|| {
        format!(
            "{} OAuth issuer is not a valid URL or uses an insecure endpoint — check {issuer_var}",
            params.display_name
        )
    })?;
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("scope", scopes)
        .append_pair("code_challenge", &pkce.challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", state);
    if let Some(originator) = params.originator {
        url.query_pairs_mut().append_pair("originator", originator);
    }
    for (key, value) in params.authorize_extras {
        url.query_pairs_mut().append_pair(key, value);
    }
    Ok(url.to_string())
}

pub fn parse_callback_query(params: &OAuthProviderParams, query: &str) -> Result<CallbackOutcome> {
    let parsed = reqwest::Url::parse(&format!("http://127.0.0.1{}?{query}", params.callback_path))
        .context("OAuth callback query is not valid")?;
    let mut code = None;
    let mut state = None;
    let mut error = None;
    let mut description = None;
    for (key, value) in parsed.query_pairs() {
        match key.as_ref() {
            "code" => code = Some(value.into_owned()),
            "state" => state = Some(value.into_owned()),
            "error" => error = Some(value.into_owned()),
            "error_description" => description = Some(value.into_owned()),
            _ => {}
        }
    }
    if let Some(error) = error {
        return Ok(CallbackOutcome::Error {
            error,
            description,
            state,
        });
    }
    let code = code
        .filter(|c| !c.trim().is_empty())
        .context("OAuth callback missing authorization code")?;
    let state = state
        .filter(|s| !s.trim().is_empty())
        .context("OAuth callback missing state")?;
    Ok(CallbackOutcome::Success { code, state })
}

pub fn accept_callback(expected_state: &str, outcome: CallbackOutcome) -> Result<String> {
    match outcome {
        CallbackOutcome::Success { code, state } => {
            anyhow::ensure!(
                state == expected_state,
                "OAuth callback state did not match the pending login"
            );
            Ok(code)
        }
        CallbackOutcome::Error {
            error,
            description,
            state,
        } => {
            if let Some(state) = state {
                anyhow::ensure!(
                    state == expected_state,
                    "OAuth error callback state did not match the pending login"
                );
            }
            let detail = description
                .filter(|text| !text.trim().is_empty())
                .unwrap_or(error);
            bail!("sign-in was not completed: {detail}")
        }
    }
}

fn parse_http_request_target(request_line: &str) -> Result<String> {
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    anyhow::ensure!(
        method.eq_ignore_ascii_case("GET"),
        "OAuth callback must be GET"
    );
    let target = parts
        .next()
        .context("OAuth callback missing request target")?;
    Ok(target.to_string())
}

fn query_from_target<'a>(params: &OAuthProviderParams, target: &'a str) -> Result<&'a str> {
    let path = target.split('?').next().unwrap_or(target);
    anyhow::ensure!(
        path == params.callback_path,
        "OAuth callback path was not {}",
        params.callback_path
    );
    Ok(target.split_once('?').map(|(_, q)| q).unwrap_or(""))
}

/// Bind the loopback callback on both IP stacks for the first free port.
///
/// The redirect URI has to say `localhost` — that is what is registered with
/// the authorization server, and redirect matching is exact — but `localhost`
/// resolves to `::1` before `127.0.0.1` on IPv6-first hosts. Binding only
/// IPv4 left the browser connecting to a closed port, which browsers paper
/// over with Happy Eyeballs fallback: a working sign-in becomes a slow one,
/// and a broken one wherever that fallback is disabled. Binding both is the
/// fix that keeps the registered redirect URI intact.
///
/// A host with only one stack available binds only that one and still works.
pub fn bind_loopback_callback(params: &OAuthProviderParams) -> Result<Vec<TcpListener>> {
    let name = params.display_name;
    let mut last_error = None;
    for port in params.loopback_ports {
        let mut bound = Vec::new();
        for addr in [
            SocketAddr::from((Ipv4Addr::LOCALHOST, *port)),
            SocketAddr::from((Ipv6Addr::LOCALHOST, *port)),
        ] {
            match TcpListener::bind(addr) {
                Ok(listener) => {
                    listener.set_nonblocking(true).with_context(|| {
                        format!("{name} OAuth callback listener could not be set non-blocking")
                    })?;
                    bound.push(listener);
                }
                Err(error) => last_error = Some(error),
            }
        }
        if !bound.is_empty() {
            return Ok(bound);
        }
    }
    let ports = params
        .loopback_ports
        .iter()
        .map(u16::to_string)
        .collect::<Vec<_>>()
        .join(" or ");
    let hint = if params.callback_conflict_hint.is_empty() {
        String::new()
    } else {
        format!(" {}", params.callback_conflict_hint)
    };
    Err(last_error
        .map(anyhow::Error::from)
        .unwrap_or_else(|| anyhow::anyhow!("unable to bind {name} OAuth callback ports")))
    .with_context(|| format!("{name} sign-in needs loopback port {ports}.{hint}"))
}

pub(crate) fn start_auth_request_on(
    listeners: &[TcpListener],
    params: &OAuthProviderParams,
    inputs: &ResolvedOAuthInputs,
) -> Result<BrowserAuthRequest> {
    let port = listeners
        .first()
        .with_context(|| {
            format!(
                "{} OAuth callback has no bound listener",
                params.display_name
            )
        })?
        .local_addr()
        .with_context(|| {
            format!(
                "{} OAuth callback listener has no local address",
                params.display_name
            )
        })?
        .port();
    let redirect_uri = format!("http://localhost:{port}{}", params.callback_path);
    let pkce = generate_pkce();
    let state = generate_state();
    let authorize_url = build_authorize_url(
        params,
        &inputs.issuer,
        &inputs.client_id,
        &inputs.scopes,
        &redirect_uri,
        &state,
        &pkce,
    )?;
    Ok(BrowserAuthRequest {
        state,
        pkce,
        redirect_uri,
        authorize_url,
    })
}

const CALLBACK_TIMEOUT: Duration = Duration::from_secs(300);
const CALLBACK_HTML_OK: &str = "<!doctype html><html><body><p>Signed in to Codewhale. You can close this tab.</p></body></html>";
const CALLBACK_HTML_ERR: &str = "<!doctype html><html><body><p>Sign-in did not complete. You can close this tab and retry in Codewhale.</p></body></html>";

fn wait_for_callback(
    listeners: &[TcpListener],
    params: &OAuthProviderParams,
    expected_state: &str,
) -> Result<String> {
    let deadline = Instant::now() + CALLBACK_TIMEOUT;
    loop {
        if Instant::now() >= deadline {
            bail!(
                "{} sign-in timed out waiting for the browser callback",
                params.display_name
            );
        }
        // Whichever stack `localhost` resolved to for the browser is the one
        // that gets the connection; poll them all.
        for listener in listeners {
            match listener.accept() {
                Ok((stream, _)) => {
                    return handle_callback_stream(stream, params, expected_state);
                }
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        || error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(error) => {
                    return Err(error).context(format!(
                        "{} OAuth callback accept failed",
                        params.display_name
                    ));
                }
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn handle_callback_stream(
    mut stream: TcpStream,
    params: &OAuthProviderParams,
    expected_state: &str,
) -> Result<String> {
    // BSD sockets (macOS) hand the accepted stream the listener's O_NONBLOCK;
    // the bounded read below needs a blocking socket with a timeout.
    stream.set_nonblocking(false).with_context(|| {
        format!(
            "{} OAuth callback stream could not be set blocking",
            params.display_name
        )
    })?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    // One read is not one request: TCP may deliver the callback in
    // fragments, and a truncated query parses as a missing parameter.
    // Read until the blank line that ends the HTTP headers.
    let mut buf = [0u8; 4096];
    let mut len = 0usize;
    loop {
        if len == buf.len() {
            break;
        }
        let n = stream
            .read(&mut buf[len..])
            .with_context(|| format!("reading {} OAuth callback request", params.display_name))?;
        if n == 0 {
            break;
        }
        len += n;
        if buf[..len].windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    let request = String::from_utf8_lossy(&buf[..len]);
    let request_line = request.lines().next().unwrap_or_default();
    let result = (|| {
        let target = parse_http_request_target(request_line)?;
        let query = query_from_target(params, &target)?;
        let outcome = parse_callback_query(params, query)?;
        accept_callback(expected_state, outcome)
    })();
    let (status, body) = match &result {
        Ok(_) => ("200 OK", CALLBACK_HTML_OK),
        Err(_) => ("400 Bad Request", CALLBACK_HTML_ERR),
    };
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\ncontent-type: text/html; charset=utf-8\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    result
}

pub(crate) fn exchange_authorization_code(
    client: &dyn OAuthFormClient,
    params: &OAuthProviderParams,
    token_endpoint: &str,
    client_id: &str,
    redirect_uri: &str,
    code: &str,
    verifier: &str,
) -> Result<OAuthTokenMaterial> {
    let (status, body) = client.post_form(
        token_endpoint,
        &[
            ("grant_type", "authorization_code"),
            ("client_id", client_id),
            ("redirect_uri", redirect_uri),
            ("code", code),
            ("code_verifier", verifier),
        ],
    )?;
    parse_oauth_form_response(status, &body, "authorization code exchange", params)
}

/// Interactive PKCE browser login for any provider whose row offers it.
/// Prints the authorize URL, opens a browser, and waits for the loopback
/// callback. A provider with no browser flow (xAI) fails here with the
/// reason, before any listener binds.
pub async fn pkce_login(provider: OAuthProvider) -> Result<PendingOAuthLogin> {
    let params = oauth_provider_params(provider);
    if params.authorize_path.is_none() {
        bail!(
            "{} offers no browser sign-in flow; sign in through the device-code login instead",
            params.display_name
        );
    }
    let inputs = params.resolve_inputs();
    let display_name = params.display_name;
    tokio::task::spawn_blocking(move || pkce_login_with(provider, &inputs))
        .await
        .with_context(|| format!("{display_name} PKCE login worker failed"))?
}

/// Blocking worker body for [`pkce_login`]. `pub(crate)` so the activation
/// tests can drive the unified login end to end until activation unifies.
pub(crate) fn pkce_login_with(
    provider: OAuthProvider,
    inputs: &ResolvedOAuthInputs,
) -> Result<PendingOAuthLogin> {
    let params = oauth_provider_params(provider);
    let display_name = params.display_name;
    let listeners = bind_loopback_callback(params)?;
    let request = start_auth_request_on(&listeners, params, inputs)?;
    eprintln!("{display_name} sign-in (PKCE)");
    eprintln!("  Open:  {}", request.authorize_url);
    eprintln!("Waiting for the browser callback… (Ctrl+C to abort)");
    if inputs.open_browser
        && let Err(err) = webbrowser::open(&request.authorize_url)
    {
        eprintln!("Could not open the browser automatically: {err}");
    }
    let code = wait_for_callback(&listeners, params, &request.state)?;
    let token = exchange_authorization_code(
        &ReqwestOAuthFormClient,
        params,
        &form_token_url(params, &inputs.issuer),
        &inputs.client_id,
        &request.redirect_uri,
        &code,
        &request.pkce.verifier,
    )?;
    Ok(PendingOAuthLogin {
        provider,
        issuer: inputs.issuer.clone(),
        client_id: inputs.client_id.clone(),
        token,
    })
}

// ── owned credential storage (one store, two providers) ───────────────
//
// Codewhale-owned OAuth generations live in the config crate's credential
// store under per-provider generation prefixes. xAI historically stored the
// access token as `key` (Grok CLI shape); ChatGPT as `access_token`. One
// entry type reads both; new generations write the unified shape.

/// xAI issuer and public client (constants the params rows and the route
/// surfaces share).
pub const XAI_OIDC_ISSUER: &str = "https://auth.x.ai";
pub const GROK_OIDC_CLIENT_ID: &str = "b1a00492-073a-47ea-816f-4c329264a828";
pub const DEFAULT_SCOPES: &str = "openid profile email offline_access api:access grok-cli:access";
/// Hard ceiling past a device grant's own `expires_in`.
pub(crate) const DEVICE_POLL_MAX_SECS: u64 = 900;

/// ChatGPT issuer and public client.
pub const CHATGPT_OAUTH_ISSUER: &str = "https://auth.openai.com";
pub const CHATGPT_OAUTH_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
/// Honest originator; never impersonate `codex_cli_rs`.
pub const CHATGPT_OAUTH_ORIGINATOR: &str = "codewhale";
pub const CHATGPT_OAUTH_SCOPE: &str = "openid profile email offline_access";

impl OAuthProvider {
    /// The engine provider this OAuth row belongs to.
    #[must_use]
    pub fn api(self) -> crate::config::ApiProvider {
        match self {
            OAuthProvider::Xai => crate::config::ApiProvider::Xai,
            OAuthProvider::Chatgpt => crate::config::ApiProvider::OpenaiCodex,
        }
    }

    /// `[providers.<key>]` table this provider's auth state lives under.
    fn config_key(self) -> &'static str {
        crate::config::provider_config_key(self.api()).unwrap_or(match self {
            OAuthProvider::Xai => "xai",
            OAuthProvider::Chatgpt => "openai_codex",
        })
    }

    fn legacy_file_name(self) -> &'static str {
        match self {
            OAuthProvider::Xai => codewhale_config::LEGACY_XAI_OAUTH_FILE_NAME,
            OAuthProvider::Chatgpt => codewhale_config::LEGACY_CHATGPT_OAUTH_FILE_NAME,
        }
    }

    #[must_use]
    pub fn is_valid_generation(self, name: &str) -> bool {
        match self {
            OAuthProvider::Xai => codewhale_config::is_valid_xai_oauth_generation(name),
            OAuthProvider::Chatgpt => codewhale_config::is_valid_chatgpt_oauth_generation(name),
        }
    }

    fn validate_generation(self, name: &str) -> Result<()> {
        match self {
            OAuthProvider::Xai => codewhale_config::validate_xai_oauth_generation(name),
            OAuthProvider::Chatgpt => codewhale_config::validate_chatgpt_oauth_generation(name),
        }
        .map(|_| ())
    }

    fn generation_path(self, name: &str) -> Result<PathBuf> {
        match self {
            OAuthProvider::Xai => codewhale_config::xai_oauth_generation_path(name),
            OAuthProvider::Chatgpt => codewhale_config::chatgpt_oauth_generation_path(name),
        }
    }

    /// A fresh, uniquely named generation file name for this provider.
    fn new_generation(self) -> String {
        let (prefix, suffix) = match self {
            OAuthProvider::Xai => (
                codewhale_config::XAI_OAUTH_GENERATION_PREFIX,
                codewhale_config::XAI_OAUTH_GENERATION_SUFFIX,
            ),
            OAuthProvider::Chatgpt => (
                codewhale_config::CHATGPT_OAUTH_GENERATION_PREFIX,
                codewhale_config::CHATGPT_OAUTH_GENERATION_SUFFIX,
            ),
        };
        format!("{prefix}{}{suffix}", uuid::Uuid::new_v4().simple())
    }
}

/// One entry in a Codewhale-owned auth generation. Reads both historical
/// on-disk shapes; `key` was the xAI/Grok field name for the access token.
#[derive(Clone, Serialize, Deserialize)]
pub struct OwnedAuthEntry {
    #[serde(default, alias = "key", skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oidc_issuer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oidc_client_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub originator: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_mode: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl std::fmt::Debug for OwnedAuthEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OwnedAuthEntry")
            .field("access_token", &redacted(self.access_token.is_some()))
            .field("refresh_token", &redacted(self.refresh_token.is_some()))
            .field("expires_at", &self.expires_at)
            .field("id_token", &redacted(self.id_token.is_some()))
            .field("account_id", &self.account_id)
            .field("oidc_issuer", &self.oidc_issuer)
            .field("oidc_client_id", &self.oidc_client_id)
            .field("originator", &self.originator)
            .field("auth_mode", &self.auth_mode)
            .field("extra_keys", &self.extra.keys().collect::<Vec<_>>())
            .finish()
    }
}

fn redacted(present: bool) -> &'static str {
    if present { "<redacted>" } else { "<none>" }
}

/// Resolved owned credentials ready for API use. No Debug: bearer material
/// never prints; consumers redact explicitly.
#[derive(Clone)]
pub struct OwnedOAuthCredentials {
    pub access_token: String,
    pub account_id: Option<String>,
    #[allow(dead_code, reason = "read by provider routes and tests as needed")]
    pub refresh_token: Option<String>,
    #[allow(dead_code, reason = "diagnostic surface only")]
    pub expires_at: Option<String>,
    #[allow(dead_code, reason = "route provenance only")]
    pub issuer: String,
    #[allow(dead_code, reason = "route provenance only")]
    pub client_id: String,
}

/// Receipt for a committed Codewhale-owned OAuth generation.
pub struct OAuthActivation {
    #[expect(dead_code)]
    pub credentials: OwnedOAuthCredentials,
    pub config_path: PathBuf,
    pub auth_path: PathBuf,
}

impl std::fmt::Debug for OAuthActivation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OAuthActivation")
            .field("credentials", &redacted(true))
            .field("config_path", &self.config_path)
            .field("auth_path", &self.auth_path)
            .finish()
    }
}

type AuthFile = BTreeMap<String, OwnedAuthEntry>;

fn load_owned_auth_file(path: &Path) -> Result<Option<AuthFile>> {
    let Some(raw) = crate::external_credentials::read_codewhale_owned_to_string(path)? else {
        return Ok(None);
    };
    parse_auth_file(&raw, path).map(Some)
}

fn load_owned_auth_file_from_store(
    store: &codewhale_config::XaiOAuthCredentialStore,
    name: &str,
) -> Result<Option<AuthFile>> {
    let Some(raw) = store.read_to_string(name)? else {
        return Ok(None);
    };
    parse_auth_file(&raw, &store.path_for(name)?).map(Some)
}

fn parse_auth_file(raw: &str, path: &Path) -> Result<AuthFile> {
    let value: Value = serde_json::from_str(raw).map_err(|_| {
        anyhow::anyhow!(
            "credential file {} is not valid credential JSON",
            codewhale_config::quote_os_path(path)
        )
    })?;
    let obj = value.as_object().ok_or_else(|| {
        anyhow::anyhow!(
            "credential file {} must be a JSON object of entries",
            codewhale_config::quote_os_path(path)
        )
    })?;
    let mut out = BTreeMap::new();
    for (k, v) in obj {
        match serde_json::from_value::<OwnedAuthEntry>(v.clone()) {
            Ok(entry) => {
                out.insert(k.clone(), entry);
            }
            Err(_) => {
                tracing::warn!(
                    target: "codewhale::oauth",
                    "skipping unreadable owned auth entry"
                );
            }
        }
    }
    Ok(out)
}

fn write_auth_file_to_store(
    store: &codewhale_config::XaiOAuthCredentialStore,
    name: &str,
    file: &AuthFile,
    allow_replace: bool,
) -> Result<()> {
    let serialized =
        serde_json::to_vec_pretty(file).context("serializing owned OAuth credentials")?;
    store
        .write(name, &serialized, allow_replace)
        .with_context(|| {
            format!(
                "writing owned OAuth credentials to {}",
                codewhale_config::quote_os_path(&store.directory().join(name))
            )
        })?;
    #[cfg(test)]
    crate::external_credentials::record_owned_credential_write();
    Ok(())
}

/// Read-only parse of another CLI's granted credential file.
fn load_external_auth_file(
    grant: &codewhale_config::ExternalCredentialReadGrant,
) -> Result<AuthFile> {
    let Some(raw) = crate::external_credentials::read_to_string(grant)? else {
        bail!(
            "external credential file not found at {}",
            codewhale_config::quote_os_path(grant.path())
        );
    };
    parse_auth_file(&raw, grant.path())
}

fn select_entry(provider: OAuthProvider, file: &mut AuthFile) -> Option<(String, OwnedAuthEntry)> {
    // Prefer this provider's registered client-id scope when present.
    let preferred_suffix = format!("::{}", oauth_provider_params(provider).default_client_id);
    if let Some((k, v)) = file
        .iter()
        .find(|(k, e)| k.ends_with(&preferred_suffix) && entry_has_usable_secret(e))
    {
        return Some((k.clone(), v.clone()));
    }
    file.iter()
        .find(|(_, e)| entry_has_usable_secret(e))
        .map(|(k, v)| (k.clone(), v.clone()))
}

fn entry_has_usable_secret(entry: &OwnedAuthEntry) -> bool {
    entry
        .access_token
        .as_deref()
        .is_some_and(|t| !t.trim().is_empty())
        || entry
            .refresh_token
            .as_deref()
            .is_some_and(|t| !t.trim().is_empty())
}

const REFRESH_SKEW_SECS: i64 = 60;

fn entry_access_token_is_fresh(entry: &OwnedAuthEntry) -> bool {
    let Some(token) = entry
        .access_token
        .as_deref()
        .filter(|t| !t.trim().is_empty())
    else {
        return false;
    };
    let stored_expiry = entry.expires_at.as_deref().and_then(parse_rfc3339_secs);
    let token_expiry = jwt_expiry_seconds(token).and_then(|exp| i64::try_from(exp).ok());
    // A later stored expiry must not hide an already-expired access token.
    // Opaque tokens still use stored expiry; no known expiry remains stale.
    stored_expiry
        .into_iter()
        .chain(token_expiry)
        .min()
        .is_some_and(|exp| exp.saturating_sub(now_unix_secs().unwrap_or(0)) > REFRESH_SKEW_SECS)
}

fn credentials_from_entry(
    provider: OAuthProvider,
    scope: &str,
    entry: &OwnedAuthEntry,
    access_token: String,
) -> OwnedOAuthCredentials {
    OwnedOAuthCredentials {
        access_token,
        account_id: entry.account_id.clone(),
        refresh_token: entry.refresh_token.clone(),
        expires_at: entry.expires_at.clone(),
        issuer: entry
            .oidc_issuer
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| issuer_from_scope(provider, scope)),
        client_id: entry
            .oidc_client_id
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| client_id_from_scope(provider, scope)),
    }
}

fn issuer_from_scope(provider: OAuthProvider, scope: &str) -> String {
    scope
        .split_once("::")
        .map(|(issuer, _)| issuer.to_string())
        .unwrap_or_else(|| oauth_provider_params(provider).default_issuer.to_string())
}

fn client_id_from_scope(provider: OAuthProvider, scope: &str) -> String {
    scope
        .split_once("::")
        .map(|(_, id)| id.to_string())
        .unwrap_or_else(|| {
            oauth_provider_params(provider)
                .default_client_id
                .to_string()
        })
}

fn apply_token_response(
    provider: OAuthProvider,
    entry: &mut OwnedAuthEntry,
    issuer: &str,
    client_id: &str,
    token: &OAuthTokenMaterial,
) -> Result<()> {
    let access = token
        .access_token
        .as_deref()
        .filter(|t| !t.trim().is_empty())
        .context("token response missing access_token")?;
    entry.access_token = Some(access.to_string());
    if let Some(rt) = token
        .refresh_token
        .as_deref()
        .filter(|t| !t.trim().is_empty())
    {
        entry.refresh_token = Some(rt.to_string());
    }
    entry.oidc_issuer = Some(issuer.to_string());
    entry.oidc_client_id = Some(client_id.to_string());
    entry.auth_mode = Some("oidc".to_string());
    entry.originator = oauth_provider_params(provider)
        .originator
        .map(ToOwned::to_owned);
    if let Some(id_token) = token.id_token.clone() {
        if let Some(account_id) = account_id_from_id_token(&id_token) {
            entry.account_id = Some(account_id);
        }
        entry.id_token = Some(id_token);
    }
    if let Some(expires_in) = token.expires_in {
        entry.expires_at = Some(rfc3339_from_now(expires_in));
    } else if let Some(exp) = jwt_expiry_seconds(access) {
        entry.expires_at = Some(rfc3339_from_unix(exp as i64));
    }
    Ok(())
}

fn account_id_from_id_token(token: &str) -> Option<String> {
    let payload = jwt_payload(token)?;
    if let Some(id) = payload.get("chatgpt_account_id").and_then(Value::as_str) {
        let trimmed = id.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    payload
        .get("https://api.openai.com/auth")
        .and_then(|auth| auth.get("chatgpt_account_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(ToOwned::to_owned)
}

fn jwt_payload(token: &str) -> Option<Value> {
    let mut parts = token.split('.');
    let _header = parts.next()?;
    let payload = parts.next()?;
    let decoded = URL_SAFE_NO_PAD.decode(payload).ok()?;
    serde_json::from_slice(&decoded).ok()
}

fn now_unix_secs() -> Option<i64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| i64::try_from(d.as_secs()).ok())
}

fn parse_rfc3339_secs(raw: &str) -> Option<i64> {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(raw) {
        return Some(dt.timestamp());
    }
    // Simple UTC forms chrono's strict parser rejects, e.g. a missing
    // fractional second on an offset-less timestamp.
    let trimmed = raw.trim().trim_end_matches('Z');
    let (date, time) = trimmed.split_once('T')?;
    let mut d = date.split('-');
    let y: i32 = d.next()?.parse().ok()?;
    let m: u32 = d.next()?.parse().ok()?;
    let day: u32 = d.next()?.parse().ok()?;
    let time = time.split('+').next()?.split('-').next()?;
    let mut t = time.split(':');
    let hh: u32 = t.next()?.parse().ok()?;
    let mm: u32 = t.next()?.parse().ok()?;
    let ss: u32 = t
        .next()
        .and_then(|s| s.split('.').next())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let ndt = chrono::NaiveDate::from_ymd_opt(y, m, day)?.and_hms_opt(hh, mm, ss)?;
    Some(ndt.and_utc().timestamp())
}

fn rfc3339_from_now(expires_in: u64) -> String {
    let ts = now_unix_secs().unwrap_or(0) + expires_in as i64;
    rfc3339_from_unix(ts)
}

fn rfc3339_from_unix(ts: i64) -> String {
    chrono::DateTime::from_timestamp(ts, 0)
        .map(|dt| dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
        .unwrap_or_else(|| format!("{ts}"))
}

/// Refresh through the seam, resolving the token endpoint the provider's
/// row demands (discovered for xAI, pinned for ChatGPT).
fn refresh_for_provider(
    provider: OAuthProvider,
    client: &dyn OAuthFormClient,
    issuer: &str,
    client_id: &str,
    refresh_token: &str,
) -> Result<OAuthTokenMaterial> {
    let params = oauth_provider_params(provider);
    let token_url = if params.discover_endpoints {
        resolve_oauth_endpoints(params, issuer).token_endpoint
    } else {
        form_token_url(params, issuer)
    };
    refresh_access_token_via(client, params, &token_url, client_id, refresh_token)
}

fn configured_owned_auth_file_path(
    provider: OAuthProvider,
    config: &Config,
) -> Result<Option<PathBuf>> {
    let generation = config
        .provider_config_for(provider.api())
        .and_then(|entry| entry.oauth_credential_generation.as_deref());
    match generation {
        Some(generation) => provider.generation_path(generation).map(Some),
        None => Ok(None),
    }
}

/// Prompt-free structural check for owned OAuth material. Never refreshes,
/// writes, or makes network requests. External storage is not inspected
/// until exact read-only consent has been persisted, and then only at the
/// exact consented path — no ambient candidate is ever resolved or opened
/// (#5772).
#[must_use]
pub fn credentials_valid(provider: OAuthProvider, config: &Config) -> bool {
    // Codewhale-owned OAuth bytes are inert until the provider route
    // explicitly selects OAuth. A failed post-login config finalization can
    // therefore never make a newly written token silently ready on the next
    // launch.
    if provider == OAuthProvider::Xai
        && !config
            .provider_config_for(provider.api())
            .and_then(|entry| entry.auth_mode.as_deref())
            .is_some_and(auth_mode_uses_xai_oauth)
    {
        return false;
    }
    if let Ok(Some(path)) = configured_owned_auth_file_path(provider, config)
        && let Ok(Some(mut file)) = load_owned_auth_file(&path)
        && let Some((_, entry)) = select_entry(provider, &mut file)
        && (entry_access_token_is_fresh(&entry)
            || entry
                .refresh_token
                .as_deref()
                .is_some_and(|token| !token.trim().is_empty()))
    {
        return true;
    }
    if config
        .provider_config_for(provider.api())
        .and_then(|entry| entry.oauth_credential_generation.as_deref())
        .is_some()
    {
        // A configured generation is authoritative. Invalid, missing, unsafe,
        // or malformed owned storage must not fall through to an external CLI.
        return false;
    }
    if provider == OAuthProvider::Xai {
        // The pre-generation legacy file is the last owned location.
        if let Ok(path) = codewhale_config::legacy_xai_oauth_path()
            && let Ok(Some(mut file)) = load_owned_auth_file(&path)
            && let Some((_, entry)) = select_entry(provider, &mut file)
            && (entry_access_token_is_fresh(&entry)
                || entry
                    .refresh_token
                    .as_deref()
                    .is_some_and(|token| !token.trim().is_empty()))
        {
            return true;
        }
        // #5772: with no persisted consent record there is no external path
        // to resolve and nothing to open.
        if let Some(consent_path) = config
            .provider_config_for(provider.api())
            .and_then(|entry| entry.external_credentials.as_ref())
            .map(|consent| consent.path.clone())
            && let Ok(grant) = config.external_credential_read_grant(
                provider.api(),
                codewhale_config::ExternalCredentialSource::GrokCli,
                &consent_path,
            )
            && let Ok(mut file) = load_external_auth_file(&grant)
        {
            return select_entry(provider, &mut file)
                .is_some_and(|(_, entry)| entry_access_token_is_fresh(&entry));
        }
    }
    false
}

#[must_use]
pub fn credentials_present(provider: OAuthProvider, config: &Config) -> bool {
    credentials_valid(provider, config)
}

/// Grant-time validation for an external Grok CLI credential file (#5772).
///
/// Reads exactly the granted path through the secure adapter and requires a
/// usable, unexpired entry. Never refreshes, rewrites, or makes a network
/// request. Consent is persisted only after this succeeds, so a consent
/// record can never be written for a file that holds nothing usable.
pub fn validate_grok_external_credentials(
    grant: &codewhale_config::ExternalCredentialReadGrant,
) -> Result<()> {
    let mut file = load_external_auth_file(grant)?;
    let (_, entry) = select_entry(OAuthProvider::Xai, &mut file).ok_or_else(|| {
        anyhow::anyhow!(
            "xAI OAuth credentials at {} have no usable entry. Run `grok login` again or use `codewhale auth xai-device` for Codewhale-owned storage.",
            codewhale_config::quote_os_path(grant.path())
        )
    })?;
    if !entry_access_token_is_fresh(&entry) {
        bail!(
            "xAI OAuth access token in {} is expired. Read-only consent never refreshes or rewrites another CLI's credentials. Run `grok login` again or use `codewhale auth xai-device`.",
            codewhale_config::quote_os_path(grant.path())
        );
    }
    Ok(())
}

/// Load xAI OAuth credentials with full precedence: configured generation,
/// legacy owned file, then the consented Grok CLI import. Codewhale-owned
/// credentials may refresh and rewrite Codewhale-owned storage; external
/// credentials are read-only.
pub fn get_xai_credentials(config: &Config) -> Result<OwnedOAuthCredentials> {
    anyhow::ensure!(
        config.api_provider() == crate::config::ApiProvider::Xai
            && config
                .provider_config_for(crate::config::ApiProvider::Xai)
                .and_then(|entry| entry.auth_mode.as_deref())
                .is_some_and(auth_mode_uses_xai_oauth),
        "Codewhale-owned xAI OAuth credentials are inactive until the xAI route explicitly selects OAuth"
    );
    if let Some(owned_path) = configured_owned_auth_file_path(OAuthProvider::Xai, config)? {
        return get_owned_credentials_at(OAuthProvider::Xai, &owned_path);
    }
    let owned_path = codewhale_config::legacy_xai_oauth_path()?;
    if load_owned_auth_file(&owned_path)?.is_some() {
        return get_owned_credentials_at(OAuthProvider::Xai, &owned_path);
    }

    let external_path = grok_auth_file_path();
    let grant = config.external_credential_read_grant(
        crate::config::ApiProvider::Xai,
        codewhale_config::ExternalCredentialSource::GrokCli,
        &external_path,
    )?;
    let mut file = load_external_auth_file(&grant)?;
    let (scope, entry) = select_entry(OAuthProvider::Xai, &mut file).ok_or_else(|| {
        anyhow::anyhow!(
            "xAI OAuth credentials at {} have no usable entry. Run `grok login` again or use `codewhale auth xai-device` for Codewhale-owned storage.",
            codewhale_config::quote_os_path(grant.path())
        )
    })?;
    if !entry_access_token_is_fresh(&entry) {
        bail!(
            "xAI OAuth access token in {} is expired. Read-only consent never refreshes or rewrites another CLI's credentials. Run `grok login` again or use `codewhale auth xai-device`.",
            codewhale_config::quote_os_path(grant.path())
        );
    }
    let token = entry
        .access_token
        .clone()
        .filter(|token| !token.trim().is_empty())
        .context("xAI OAuth access token is empty")?;
    Ok(credentials_from_entry(
        OAuthProvider::Xai,
        &scope,
        &entry,
        token,
    ))
}

pub fn get_xai_access_token(config: &Config) -> Result<String> {
    Ok(get_xai_credentials(config)?.access_token)
}

/// Load ChatGPT owned credentials from the configured generation,
/// refreshing through the seam when stale.
pub fn get_owned_credentials(
    provider: OAuthProvider,
    config: &Config,
) -> Result<OwnedOAuthCredentials> {
    get_owned_credentials_with(provider, config, &ReqwestOAuthFormClient)
}

fn get_owned_credentials_with(
    provider: OAuthProvider,
    config: &Config,
    client: &dyn OAuthFormClient,
) -> Result<OwnedOAuthCredentials> {
    let Some(path) = configured_owned_auth_file_path(provider, config)? else {
        bail!(
            "Codewhale-owned {} OAuth credentials are not configured",
            oauth_provider_params(provider).display_name
        );
    };
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .context("Codewhale-owned OAuth path must have a UTF-8 basename")?;
    codewhale_config::with_xai_oauth_lifecycle_lock(|store| {
        get_owned_credentials_locked(provider, store, name, |issuer, client_id, refresh| {
            refresh_for_provider(provider, client, issuer, client_id, refresh)
        })
    })
}

fn get_owned_credentials_at(provider: OAuthProvider, path: &Path) -> Result<OwnedOAuthCredentials> {
    let directory = codewhale_config::xai_oauth_credentials_dir()?;
    anyhow::ensure!(
        path.parent() == Some(directory.as_path()),
        "Codewhale-owned OAuth path escaped the credentials directory"
    );
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .context("Codewhale-owned OAuth path must have a UTF-8 basename")?;
    anyhow::ensure!(
        name == provider.legacy_file_name() || provider.is_valid_generation(name),
        "Codewhale-owned OAuth path has an invalid basename"
    );
    codewhale_config::with_xai_oauth_lifecycle_lock(|store| {
        get_owned_credentials_locked(provider, store, name, |issuer, client_id, refresh| {
            refresh_for_provider(
                provider,
                &ReqwestOAuthFormClient,
                issuer,
                client_id,
                refresh,
            )
        })
    })
}

fn get_owned_credentials_locked<F>(
    provider: OAuthProvider,
    store: &codewhale_config::XaiOAuthCredentialStore,
    name: &str,
    refresh_access: F,
) -> Result<OwnedOAuthCredentials>
where
    F: FnOnce(&str, &str, &str) -> Result<OAuthTokenMaterial>,
{
    let hint = oauth_provider_params(provider).relogin_hint;
    let path = store.path_for(name)?;
    let mut file = load_owned_auth_file_from_store(store, name)?.ok_or_else(|| {
        anyhow::anyhow!(
            "Codewhale-owned OAuth credentials were not found at {}. Run `{hint}` again.",
            codewhale_config::quote_os_path(&path)
        )
    })?;
    let (scope, mut entry) = select_entry(provider, &mut file).ok_or_else(|| {
        anyhow::anyhow!(
            "Codewhale-owned OAuth credentials at {} have no usable entry. Run `{hint}` again.",
            codewhale_config::quote_os_path(&path)
        )
    })?;

    if entry_access_token_is_fresh(&entry) {
        let token = entry
            .access_token
            .clone()
            .filter(|t| !t.trim().is_empty())
            .context("OAuth access token is empty")?;
        return Ok(credentials_from_entry(provider, &scope, &entry, token));
    }

    let refresh = entry
        .refresh_token
        .as_deref()
        .filter(|t| !t.trim().is_empty())
        .context(format!(
            "OAuth access token expired and no refresh_token is stored. Run `{hint}` again."
        ))?;
    let issuer = entry
        .oidc_issuer
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| issuer_from_scope(provider, &scope));
    let client_id = entry
        .oidc_client_id
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| client_id_from_scope(provider, &scope));

    let refreshed = refresh_access(&issuer, &client_id, refresh)?;
    apply_token_response(provider, &mut entry, &issuer, &client_id, &refreshed)?;
    file.insert(scope.clone(), entry.clone());
    write_auth_file_to_store(store, name, &file, true)?;

    let token = entry
        .access_token
        .clone()
        .filter(|t| !t.trim().is_empty())
        .context("OAuth refresh returned an empty access token")?;
    Ok(credentials_from_entry(provider, &scope, &entry, token))
}

/// Commit a pending login as a uniquely named owned generation and
/// atomically point the provider's config table at it under the shared
/// config lock.
///
/// The credential file is staged while the config lock is held. If config
/// persistence fails, the unreferenced stage is removed. Only after the new
/// pointer commits is the previously selected generation removed best-effort.
pub fn activate_login(
    pending: PendingOAuthLogin,
    config_path: Option<&Path>,
    live_config: Option<&mut Config>,
) -> Result<OAuthActivation> {
    codewhale_config::with_xai_oauth_lifecycle_lock(move |store| {
        activate_login_locked(pending, config_path, live_config, store)
    })
}

fn activate_login_locked(
    pending: PendingOAuthLogin,
    config_path: Option<&Path>,
    live_config: Option<&mut Config>,
    store: &codewhale_config::XaiOAuthCredentialStore,
) -> Result<OAuthActivation> {
    let provider = pending.provider;
    let display_name = oauth_provider_params(provider).display_name;
    let config_path = crate::config_persistence::config_toml_path(config_path)?;
    let generation = provider.new_generation();
    provider.validate_generation(&generation)?;
    let auth_path = store.path_for(&generation)?;
    let key_inside = provider.config_key();
    let mut stage_written = false;

    let activation = codewhale_config::mutate_config_document(&config_path, |document| {
        let previous_generation_item = document
            .get("providers")
            .and_then(toml_edit::Item::as_table_like)
            .and_then(|providers| providers.get(key_inside))
            .and_then(toml_edit::Item::as_table_like)
            .and_then(|provider| provider.get("oauth_credential_generation"));
        let previous_generation = previous_generation_item
            .map(|item| {
                item.as_str()
                    .context(format!(
                        "refusing {display_name} login because the existing credential generation pointer is not a string"
                    ))
                    .map(ToOwned::to_owned)
            })
            .transpose()?;
        if let Some(previous) = previous_generation.as_deref() {
            provider.validate_generation(previous).with_context(|| {
                format!(
                    "refusing {display_name} login because the existing credential generation pointer is invalid"
                )
            })?;
        }

        let previous_owned_name = match previous_generation.as_deref() {
            Some(previous) => Some(previous.to_string()),
            None if store.read_to_string(provider.legacy_file_name())?.is_some() => {
                Some(provider.legacy_file_name().to_string())
            }
            None => None,
        };
        // Carry the previous generation's other scopes forward. A valid
        // pointer whose file is gone (interrupted revocation, external
        // cleanup) must not brick login: only a successful activation can
        // ever rewrite the pointer, so treat the missing generation like a
        // fresh start instead of failing (#5032).
        let mut file = match previous_owned_name.as_deref() {
            Some(name) => load_owned_auth_file_from_store(store, name)?.unwrap_or_else(|| {
                tracing::warn!(
                    target: "codewhale::oauth",
                    generation = name,
                    "config pointed at a missing owned OAuth generation; starting a fresh credential file"
                );
                BTreeMap::new()
            }),
            None => BTreeMap::new(),
        };
        let scope = format!("{}::{}", pending.issuer, pending.client_id);
        let mut entry = file.remove(&scope).unwrap_or_else(|| OwnedAuthEntry {
            access_token: None,
            refresh_token: None,
            expires_at: None,
            id_token: None,
            account_id: None,
            oidc_issuer: Some(pending.issuer.clone()),
            oidc_client_id: Some(pending.client_id.clone()),
            originator: None,
            auth_mode: Some("oidc".to_string()),
            extra: BTreeMap::new(),
        });
        apply_token_response(
            provider,
            &mut entry,
            &pending.issuer,
            &pending.client_id,
            &pending.token,
        )?;
        let access = entry
            .access_token
            .clone()
            .filter(|token| !token.trim().is_empty())
            .context(format!(
                "{display_name} login returned an empty access token"
            ))?;
        file.insert(scope.clone(), entry.clone());
        write_auth_file_to_store(store, &generation, &file, false)?;
        stage_written = true;

        codewhale_config::set_config_document_value(
            document,
            &["providers", key_inside, "auth_mode"],
            "oauth",
        )?;
        codewhale_config::set_config_document_value(
            document,
            &["providers", key_inside, "oauth_credential_generation"],
            generation.clone(),
        )?;
        codewhale_config::unset_config_document_value(
            document,
            &["providers", key_inside, "external_credentials"],
        )?;
        Ok((
            previous_owned_name,
            credentials_from_entry(provider, &scope, &entry, access),
        ))
    });

    let (previous_owned_name, credentials) = match activation {
        Ok(activation) => activation,
        Err(error) => {
            if stage_written && let Err(cleanup_error) = store.remove(&generation) {
                return Err(error).context(format!(
                    "{display_name} login was not activated; also failed to remove unreferenced staged credentials at {}: {cleanup_error}",
                    codewhale_config::quote_os_path(&auth_path)
                ));
            }
            return Err(error).context(format!(
                "{display_name} login was not activated; provider configuration is unchanged"
            ));
        }
    };

    if let Some(config) = live_config {
        match provider {
            OAuthProvider::Xai => config.mark_codewhale_owned_xai_oauth(generation.clone()),
            OAuthProvider::Chatgpt => {
                config.mark_codewhale_owned_chatgpt_oauth(generation.clone());
            }
        }
    }
    if let Some(previous) = previous_owned_name
        && previous != generation
        && let Err(error) = store.remove(&previous)
    {
        tracing::warn!(
            target: "codewhale::oauth",
            error = %error,
            "new OAuth generation committed but superseded generation cleanup failed"
        );
    }
    eprintln!(
        "Signed in with {display_name}. Codewhale-owned credentials activated at {}.",
        codewhale_config::quote_os_path(&auth_path)
    );
    Ok(OAuthActivation {
        credentials,
        config_path,
        auth_path,
    })
}

/// Remove Codewhale-owned tokens and the config pointer for one provider.
///
/// ChatGPT's remote revoke is best-effort against the row's pinned revoke
/// path (see [`OAuthProviderParams::revoke_path`]): a failed or unreachable
/// revoke must never stop the local credentials from being removed. xAI
/// revokes locally only. External CLI consent is left untouched.
pub fn revoke_owned_login(
    provider: OAuthProvider,
    config_path: Option<&Path>,
    live_config: Option<&mut Config>,
) -> Result<()> {
    codewhale_config::with_xai_oauth_lifecycle_lock(|store| {
        revoke_owned_login_locked(provider, config_path, live_config, store)
    })
}

fn revoke_owned_login_locked(
    provider: OAuthProvider,
    config_path: Option<&Path>,
    live_config: Option<&mut Config>,
    store: &codewhale_config::XaiOAuthCredentialStore,
) -> Result<()> {
    revoke_owned_login_locked_with(
        provider,
        config_path,
        live_config,
        store,
        &ReqwestOAuthFormClient,
    )
}

fn revoke_owned_login_locked_with(
    provider: OAuthProvider,
    config_path: Option<&Path>,
    live_config: Option<&mut Config>,
    store: &codewhale_config::XaiOAuthCredentialStore,
    client: &dyn OAuthFormClient,
) -> Result<()> {
    let config_path = crate::config_persistence::config_toml_path(config_path)?;
    let key_inside = provider.config_key();
    let previous = codewhale_config::mutate_config_document(&config_path, |document| {
        let previous = document
            .get("providers")
            .and_then(toml_edit::Item::as_table_like)
            .and_then(|providers| providers.get(key_inside))
            .and_then(toml_edit::Item::as_table_like)
            .and_then(|provider| provider.get("oauth_credential_generation"))
            .and_then(toml_edit::Item::as_str)
            .map(ToOwned::to_owned);
        codewhale_config::unset_config_document_value(
            document,
            &["providers", key_inside, "oauth_credential_generation"],
        )?;
        let auth_mode_is_oauth = document
            .get("providers")
            .and_then(toml_edit::Item::as_table_like)
            .and_then(|providers| providers.get(key_inside))
            .and_then(toml_edit::Item::as_table_like)
            .and_then(|provider| provider.get("auth_mode"))
            .and_then(toml_edit::Item::as_str)
            == Some("oauth");
        if auth_mode_is_oauth {
            codewhale_config::unset_config_document_value(
                document,
                &["providers", key_inside, "auth_mode"],
            )?;
        }
        Ok(previous)
    })?;
    if let Some(config) = live_config
        && provider == OAuthProvider::Chatgpt
    {
        config.clear_codewhale_owned_chatgpt_oauth();
    }
    let names = match previous.as_deref() {
        Some(generation) if provider.is_valid_generation(generation) => {
            vec![generation.to_string()]
        }
        _ => vec![provider.legacy_file_name().to_string()],
    };
    for name in names {
        if let Ok(Some(raw)) = store.read_to_string(&name)
            && let Ok(file) = parse_auth_file(&raw, &store.path_for(&name)?)
        {
            for entry in file.values() {
                if let Some(token) = entry
                    .refresh_token
                    .as_deref()
                    .or(entry.access_token.as_deref())
                    .filter(|token| !token.trim().is_empty())
                {
                    let issuer = entry
                        .oidc_issuer
                        .as_deref()
                        .unwrap_or_else(|| oauth_provider_params(provider).default_issuer);
                    let client_id = entry
                        .oidc_client_id
                        .as_deref()
                        .unwrap_or_else(|| oauth_provider_params(provider).default_client_id);
                    if let Err(error) = revoke_remote_token_via(
                        client,
                        oauth_provider_params(provider),
                        issuer,
                        client_id,
                        token,
                    ) {
                        tracing::warn!(
                            target: "codewhale::oauth",
                            error = %error,
                            "remote OAuth revoke failed; local credentials will still be removed"
                        );
                    }
                }
            }
        }
        let _ = store.remove(&name);
    }
    Ok(())
}

/// Detect the [#5032] bricked-launch state: the provider's config selects
/// OAuth and points `oauth_credential_generation` at a Codewhale-owned
/// credential file that no longer exists. This is a distinct, more specific
/// failure than "unconfigured" — the pointer is present and authoritative,
/// so [`credentials_valid`] returns false and cannot fall through to a
/// legacy or external credential, which is exactly what bricked the
/// dogfood machine.
///
/// Returns false for any other state: OAuth not selected, no generation
/// configured, a malformed generation pointer (a different, already
/// fail-closed failure), or a generation whose owned file is present.
///
/// [#5032]: https://github.com/Hmbown/CodeWhale/issues/5032
#[must_use]
pub fn owned_generation_is_dangling(provider: OAuthProvider, config: &Config) -> bool {
    if provider == OAuthProvider::Xai
        && !config
            .provider_config_for(provider.api())
            .and_then(|entry| entry.auth_mode.as_deref())
            .is_some_and(auth_mode_uses_xai_oauth)
    {
        return false;
    }
    match configured_owned_auth_file_path(provider, config) {
        Ok(Some(path)) => !path.exists(),
        // `None` => no generation configured (not a dangling pointer). `Err` =>
        // the generation is malformed/invalid; that is a different, already
        // fail-closed failure, not the missing-file state this detects.
        _ => false,
    }
}

/// Best-effort repair for the [#5032] bricked-launch state: remove the stale
/// `oauth_credential_generation` pointer from the PERSISTED config file so
/// the next launch is no longer bricked. Mirrors the document edits in
/// [`activate_login_locked`] (which replaces the pointer under the config
/// lock) and [`crate::config::clear_api_key`]'s unlocked scrub.
///
/// Leaves `auth_mode = "oauth"` intact: the user still wants OAuth, they
/// simply need to re-authenticate. The launch-path caller must treat any
/// error as non-fatal — log a warning and continue. Returns `Ok(())` when
/// the stale pointer was removed (or was already absent).
///
/// [#5032]: https://github.com/Hmbown/CodeWhale/issues/5032
pub fn clear_dangling_generation(
    provider: OAuthProvider,
    config_path: Option<&Path>,
) -> Result<()> {
    let config_path = crate::config_persistence::config_toml_path(config_path)?;
    let key_inside = provider.config_key();
    codewhale_config::mutate_config_document(&config_path, |document| {
        codewhale_config::unset_config_document_value(
            document,
            &["providers", key_inside, "oauth_credential_generation"],
        )?;
        Ok(())
    })
}

/// Whether `[providers.xai] auth_mode` selects the OAuth path.
#[must_use]
pub fn auth_mode_uses_xai_oauth(mode: &str) -> bool {
    matches!(
        normalize_auth_mode(mode).as_str(),
        "oauth"
            | "xai_oauth"
            | "xai"
            | "grok"
            | "grok_oauth"
            | "grok_cli"
            | "device"
            | "device_code"
            | "device_auth"
    )
}

fn normalize_auth_mode(mode: &str) -> String {
    mode.trim().to_ascii_lowercase().replace(['-', ' '], "_")
}

/// Resolve the Grok CLI auth file path.
///
/// Priority:
/// 1. `GROK_AUTH_PATH` / `XAI_AUTH_PATH`
/// 2. `$GROK_HOME/auth.json`
/// 3. `~/.grok/auth.json`
#[must_use]
pub fn grok_auth_file_path() -> PathBuf {
    for key in ["GROK_AUTH_PATH", "XAI_AUTH_PATH"] {
        if let Ok(path) = std::env::var(key) {
            let p = PathBuf::from(path.trim());
            if !p.as_os_str().is_empty() {
                return codewhale_config::resolve_external_credential_path(&p).unwrap_or(p);
            }
        }
    }
    if let Ok(home) = std::env::var("GROK_HOME") {
        let p = PathBuf::from(home.trim());
        if !p.as_os_str().is_empty() {
            let path = p.join("auth.json");
            return codewhale_config::resolve_external_credential_path(&path).unwrap_or(path);
        }
    }
    let path = crate::config::effective_home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".grok")
        .join("auth.json");
    codewhale_config::resolve_external_credential_path(&path).unwrap_or(path)
}

#[must_use]
pub fn missing_auth_message(provider: OAuthProvider) -> String {
    match provider {
        OAuthProvider::Xai => format!(
            "xAI OAuth credentials not found.\n\
             Options:\n\
             1. Run `codewhale auth xai-device` for Codewhale-owned OAuth storage\n\
             2. To read an existing Grok CLI login without changing it, run \
             `codewhale auth external-consent --provider xai --mode read-only --path {}`\n\
             3. Or use API-key auth: export XAI_API_KEY=... / \
             codewhale auth set --provider xai",
            codewhale_config::quote_os_path(&grok_auth_file_path())
        ),
        OAuthProvider::Chatgpt => format!(
            "OpenAI Codex OAuth credentials are unavailable.\n\
             \n\
             Sign in with ChatGPT (subscription billing, Codewhale-owned tokens):\n\
             `codewhale auth chatgpt` or /provider setup openai-codex.\n\
             The openai API-key route is a different billing owner.\n\
             \n\
             Alternatives:\n\
             - Process token: OPENAI_CODEX_ACCESS_TOKEN / CODEX_ACCESS_TOKEN\n\
             - Explicit Codex CLI import (not a prerequisite): after `codex login`, run \
             `codewhale auth external-consent --provider openai-codex --mode read-only --path {}`\n\
             Read-only access never refreshes or rewrites the Codex CLI file.\n\
             Revoke Codewhale-owned tokens with `codewhale auth chatgpt-revoke`.",
            codewhale_config::quote_os_path(&auth_file_path())
        ),
    }
}

/// Pending-login test constructor shared by the activation tests.
#[cfg(test)]
pub(crate) fn pending_login_for_test(
    provider: OAuthProvider,
    access_token: &str,
    refresh_token: &str,
) -> PendingOAuthLogin {
    pending_login_with_id_token_for_test(provider, access_token, refresh_token, None)
}

#[cfg(test)]
pub(crate) fn pending_login_with_id_token_for_test(
    provider: OAuthProvider,
    access_token: &str,
    refresh_token: &str,
    id_token: Option<&str>,
) -> PendingOAuthLogin {
    PendingOAuthLogin {
        provider,
        issuer: oauth_provider_params(provider).default_issuer.to_string(),
        client_id: oauth_provider_params(provider)
            .default_client_id
            .to_string(),
        token: OAuthTokenMaterial {
            access_token: Some(access_token.to_string()),
            refresh_token: Some(refresh_token.to_string()),
            expires_in: Some(3600),
            id_token: id_token.map(ToOwned::to_owned),
            interval: None,
            error: None,
            error_description: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ApiProvider;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn oauth_endpoint_requires_https_or_parsed_loopback_http() {
        for raw in [
            "https://issuer.example/token",
            "http://localhost:8123/token",
            "http://127.0.0.1:8123/token",
            "http://127.0.0.2/token",
            "http://[::1]:8123/token",
        ] {
            assert!(oauth_endpoint_url(raw).is_ok());
        }
        for raw in [
            "http://issuer.example/token",
            "http://localhost.example/token",
            "http://127.0.0.1.example/token",
            "http://192.0.2.1/token",
            "https://user:example@issuer.example/token",
            "file:///tmp/token",
            "not a URL",
        ] {
            assert!(oauth_endpoint_url(raw).is_err());
        }
    }

    #[test]
    fn oauth_discovery_rejects_an_initial_plaintext_remote_issuer() {
        let issuer = "http://issuer.example";
        assert!(validate_discovered_issuer(Some(issuer.into()), issuer).is_err());
        assert!(
            validate_discovered_oauth_endpoint(
                Some(format!("{issuer}/token")),
                "token_endpoint",
                issuer,
            )
            .is_err()
        );
        let fallback = fallback_oauth_endpoints(&XAI_OAUTH_PARAMS, issuer);
        assert!(oauth_endpoint_url(&fallback.token_endpoint).is_err());
        assert!(
            oauth_endpoint_url(fallback.device_authorization_endpoint.as_deref().unwrap()).is_err()
        );
    }

    #[test]
    fn oauth_authorize_url_refuses_plaintext_remote_issuer_before_browser_use() {
        let pkce = PkceChallenge {
            verifier: "test-verifier".into(),
            challenge: "test-challenge".into(),
        };
        for issuer in [
            "http://issuer.example",
            "https://user:example@issuer.example",
        ] {
            assert!(
                build_authorize_url(
                    &CHATGPT_OAUTH_PARAMS,
                    issuer,
                    "test-client",
                    "openid",
                    "http://localhost:1455/auth/callback",
                    "test-state",
                    &pkce,
                )
                .is_err()
            );
        }
    }

    fn grant(path: &std::path::Path) -> ExternalCredentialReadGrant {
        codewhale_config::ExternalCredentialConsentToml::read_only(
            codewhale_config::ProviderKind::OpenaiCodex,
            codewhale_config::ExternalCredentialSource::CodexCli,
            path.to_path_buf(),
        )
        .read_grant(
            codewhale_config::ProviderKind::OpenaiCodex,
            codewhale_config::ExternalCredentialSource::CodexCli,
            path,
        )
        .expect("test read grant")
    }

    #[test]
    fn jwt_expiry_parses_valid_token() {
        // A minimal JWT with {"exp": 9999999999} as payload.
        let payload = URL_SAFE_NO_PAD.encode(b"{\"exp\":9999999999}");
        let token = format!("header.{payload}.signature");
        assert_eq!(jwt_expiry_seconds(&token), Some(9999999999));
    }

    #[test]
    fn jwt_expiry_returns_none_for_malformed() {
        assert_eq!(jwt_expiry_seconds("not.a.jwt"), None);
        assert_eq!(jwt_expiry_seconds(""), None);
        assert_eq!(jwt_expiry_seconds("x"), None);
    }

    #[test]
    fn token_is_expired_detects_future() {
        // Far future — should not be expired.
        let payload = URL_SAFE_NO_PAD.encode(b"{\"exp\":9999999999}");
        let token = format!("header.{payload}.sig");
        assert!(!token_is_expired(&token));
    }

    #[test]
    fn token_is_expired_detects_past() {
        // Way in the past.
        let payload = URL_SAFE_NO_PAD.encode(b"{\"exp\":1000000000}");
        let token = format!("header.{payload}.sig");
        assert!(token_is_expired(&token));
    }

    #[test]
    fn owned_token_freshness_honors_both_expiries_and_preserves_fallbacks() {
        let now = now_unix_secs().expect("clock");
        let future = rfc3339_from_unix(now + 3600);
        let past = rfc3339_from_unix(now - 3600);
        let near = rfc3339_from_unix(now + 30);
        let fresh_token = jwt_with_exp((now + 3600) as u64);
        let expired_token = jwt_with_exp((now - 3600) as u64);
        let near_token = jwt_with_exp((now + 30) as u64);

        for (stored, token, expected) in [
            (Some(future.as_str()), expired_token.as_str(), false),
            (Some(past.as_str()), fresh_token.as_str(), false),
            (Some(future.as_str()), fresh_token.as_str(), true),
            (Some(future.as_str()), near_token.as_str(), false),
            (Some(near.as_str()), fresh_token.as_str(), false),
            (None, fresh_token.as_str(), true),
            (None, expired_token.as_str(), false),
            (Some("invalid-date"), fresh_token.as_str(), true),
            (Some(future.as_str()), "opaque-token", true),
            (Some(past.as_str()), "opaque-token", false),
            (None, "opaque-token", false),
            (Some("invalid-date"), "opaque-token", false),
            (Some(future.as_str()), "", false),
        ] {
            let entry: OwnedAuthEntry = serde_json::from_value(serde_json::json!({
                "access_token": token,
                "expires_at": stored,
            }))
            .expect("synthetic owned entry");
            assert_eq!(
                entry_access_token_is_fresh(&entry),
                expected,
                "stored={stored:?}, JWT expiry={:?}",
                jwt_expiry_seconds(token),
            );
        }
    }

    #[test]
    fn credential_presence_rejects_empty_and_malformed_files_without_refresh() {
        let _lock = crate::test_support::lock_test_env();
        let home = tempfile::tempdir().expect("temp Codex home");
        let auth_path = home
            .path()
            .canonicalize()
            .expect("canonical temp root")
            .join("auth.json");
        let _auth = crate::test_support::EnvVarGuard::set("OPENAI_CODEX_AUTH_FILE", &auth_path);
        let _access = crate::test_support::EnvVarGuard::remove("OPENAI_CODEX_ACCESS_TOKEN");
        let _legacy_access = crate::test_support::EnvVarGuard::remove("CODEX_ACCESS_TOKEN");
        let grant = grant(&auth_path);

        std::fs::write(&auth_path, "{}").expect("empty auth");
        crate::external_credentials::reset_side_effect_trap();
        assert!(!stored_credentials_present(&grant));
        assert_eq!(
            crate::external_credentials::side_effect_trap_counts(),
            (1, 1)
        );
        assert_eq!(
            crate::external_credentials::complete_side_effect_trap_counts(),
            (1, 1, 0, 0, 0)
        );
        std::fs::write(&auth_path, "{not-json").expect("malformed auth");
        crate::external_credentials::reset_side_effect_trap();
        assert!(!stored_credentials_present(&grant));
        assert_eq!(
            crate::external_credentials::side_effect_trap_counts(),
            (1, 1)
        );

        let payload = URL_SAFE_NO_PAD.encode(b"{\"exp\":9999999999}");
        let access_token = format!("header.{payload}.signature");
        std::fs::write(
            &auth_path,
            serde_json::to_vec(&serde_json::json!({
                "tokens": {"access_token": access_token}
            }))
            .expect("valid auth json"),
        )
        .expect("valid auth");
        crate::external_credentials::reset_side_effect_trap();
        assert!(stored_credentials_present(&grant));
        assert_eq!(
            crate::external_credentials::side_effect_trap_counts(),
            (1, 1)
        );
    }

    #[test]
    fn expired_external_token_fails_without_refresh_or_rewrite() {
        let _lock = crate::test_support::lock_test_env();
        let home = tempfile::tempdir().expect("temp Codex home");
        let auth_path = home
            .path()
            .canonicalize()
            .expect("canonical temp root")
            .join("auth.json");
        let payload = URL_SAFE_NO_PAD.encode(b"{\"exp\":1000000000}");
        let access_token = format!("header.{payload}.signature");
        let raw = serde_json::to_string_pretty(&serde_json::json!({
            "tokens": {
                "access_token": access_token,
                "refresh_token": "must-never-be-used",
                "account_id": "acct-test",
                "future_field": {"preserve": true}
            },
            "future_top_level": [1, 2, 3]
        }))
        .expect("auth fixture");
        std::fs::write(&auth_path, &raw).expect("expired auth fixture");

        crate::external_credentials::reset_side_effect_trap();
        let error = get_credentials(&grant(&auth_path))
            .expect_err("read-only external tokens must not refresh");
        assert!(error.to_string().contains("never refreshes or rewrites"));
        assert_eq!(
            crate::external_credentials::side_effect_trap_counts(),
            (1, 1)
        );
        assert_eq!(
            std::fs::read_to_string(&auth_path).expect("unchanged auth file"),
            raw
        );
    }

    #[test]
    fn auth_file_path_respects_env() {
        // Just verify it returns a path without panicking.
        let path = auth_file_path();
        assert!(path.to_string_lossy().contains("auth.json"));
    }

    #[test]
    fn missing_auth_message_explains_disabled_default_and_explicit_consent() {
        let _lock = crate::test_support::lock_test_env();
        let message = missing_auth_message(OAuthProvider::Chatgpt);

        assert!(message.contains("OpenAI Codex OAuth credentials are unavailable"));
        assert!(message.contains("OPENAI_CODEX_ACCESS_TOKEN"));
        assert!(message.contains("CODEX_ACCESS_TOKEN"));
        assert!(message.contains(&codewhale_config::quote_os_path(&auth_file_path())));
        assert!(message.contains("codewhale auth chatgpt"));
        assert!(message.contains("subscription billing"));
        assert!(message.contains("openai API-key"));
        assert!(message.contains("codex login"));
        assert!(message.contains("external-consent"));
        assert!(message.contains("chatgpt-revoke"));
    }

    #[test]
    fn provider_table_separates_device_flow_from_browser_flow() {
        let xai = oauth_provider_params(OAuthProvider::Xai);
        assert_eq!(xai.device_code_path, Some("oauth2/device/code"));
        assert!(xai.discover_endpoints);
        let chatgpt = oauth_provider_params(OAuthProvider::Chatgpt);
        assert_eq!(chatgpt.device_code_path, None);
        assert!(!chatgpt.discover_endpoints);
        assert_ne!(xai.default_client_id, chatgpt.default_client_id);
    }

    #[test]
    fn provider_inputs_resolve_from_defaults_without_env() {
        let _lock = crate::test_support::lock_test_env();
        let _guards: Vec<_> = [
            "GROK_OIDC_ISSUER",
            "XAI_OIDC_ISSUER",
            "GROK_OIDC_CLIENT_ID",
            "XAI_OIDC_CLIENT_ID",
            "GROK_OIDC_SCOPES",
            "XAI_OIDC_SCOPES",
            "CODEWHALE_XAI_OAUTH_NO_BROWSER",
        ]
        .into_iter()
        .map(crate::test_support::EnvVarGuard::remove)
        .collect();
        let inputs = XAI_OAUTH_PARAMS.resolve_inputs();
        assert_eq!(inputs.issuer, "https://auth.x.ai");
        assert!(inputs.scopes.contains("grok-cli:access"));
        assert!(inputs.open_browser);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn request_device_grant_round_trips_a_mock_grant() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/oauth2/device/code"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "device_code": "device-123",
                "user_code": "USER-456",
                "verification_uri": "https://example.com/device",
                "expires_in": 900
            })))
            .expect(1)
            .mount(&server)
            .await;
        let grant = tokio::task::block_in_place(|| {
            request_device_grant(
                &format!("{}/oauth2/device/code", server.uri()),
                "test-client",
                "openid",
            )
        })
        .expect("mock grant");
        assert_eq!(grant.device_code.as_deref(), Some("device-123"));
        assert_eq!(grant.user_code.as_deref(), Some("USER-456"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn request_device_grant_rejects_success_without_codes() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/oauth2/device/code"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .expect(1)
            .mount(&server)
            .await;
        let result = tokio::task::block_in_place(|| {
            request_device_grant(
                &format!("{}/oauth2/device/code", server.uri()),
                "test-client",
                "openid",
            )
        });
        let Err(error) = result else {
            panic!("a grant without codes must fail");
        };
        assert!(error.to_string().contains("without a device and user code"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn poll_device_grant_classifies_rfc8628_states() {
        use codewhale_config::device_code::DevicePollOutcome;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        async fn outcome(
            body: serde_json::Value,
            status: u16,
        ) -> Result<DevicePollOutcome<OAuthTokenMaterial>> {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/oauth2/token"))
                .respond_with(ResponseTemplate::new(status).set_body_json(body))
                .expect(1)
                .mount(&server)
                .await;
            tokio::task::block_in_place(|| {
                poll_device_grant(
                    &format!("{}/oauth2/token", server.uri()),
                    "test-client",
                    "device-token",
                )
            })
        }
        assert!(matches!(
            outcome(serde_json::json!({ "error": "authorization_pending" }), 400).await,
            Ok(DevicePollOutcome::Pending)
        ));
        assert!(matches!(
            outcome(
                serde_json::json!({ "error": "slow_down", "interval": 12 }),
                400
            )
            .await,
            Ok(DevicePollOutcome::SlowDown {
                interval_seconds: Some(12)
            })
        ));
        for error in ["access_denied", "expired_token"] {
            let result = outcome(serde_json::json!({ "error": error }), 400).await;
            let Err(failure) = result else {
                panic!("{error} must stop polling");
            };
            assert!(failure.to_string().contains(error), "{failure}");
        }
        let result = outcome(
            serde_json::json!({ "access_token": "at", "expires_in": 3600 }),
            200,
        )
        .await;
        let Ok(DevicePollOutcome::Complete(material)) = result else {
            panic!("success must complete");
        };
        assert_eq!(material.access_token.as_deref(), Some("at"));
    }

    #[tokio::test]
    async fn device_login_without_a_device_flow_fails_before_network() {
        let result = device_code_login(OAuthProvider::Chatgpt).await;
        let Err(error) = result else {
            panic!("ChatGPT has no device flow");
        };
        assert!(
            error.to_string().contains("no device-code flow"),
            "{error:#}"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn oauth_transports_never_forward_forms_to_redirect_destinations() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let issuer = MockServer::start().await;
        let destination = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "synthetic-access",
            })))
            .expect(0)
            .mount(&destination)
            .await;
        for status in [301, 302, 303, 307, 308] {
            let endpoint = format!("{}/redirect-{status}", issuer.uri());
            Mock::given(path(format!("/redirect-{status}")))
                .respond_with(
                    ResponseTemplate::new(status)
                        .insert_header("Location", format!("{}/token", destination.uri()))
                        .set_body_json(serde_json::json!({})),
                )
                .expect(3)
                .mount(&issuer)
                .await;
            tokio::task::block_in_place(|| {
                assert!(request_device_grant(&endpoint, "synthetic-client", "scope").is_err());
                assert!(
                    poll_device_grant(&endpoint, "synthetic-client", "synthetic-device").is_err()
                );
                let (actual_status, _) = ReqwestOAuthFormClient
                    .post_form(&endpoint, &[("refresh_token", "synthetic-refresh")])
                    .unwrap();
                assert_eq!(actual_status, status);
            });
        }
        // An explicitly selected issuer remains usable without a redirect.
        Mock::given(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "device_code": "synthetic-device",
                "user_code": "synthetic-user",
                "access_token": "synthetic-access",
            })))
            .expect(3)
            .mount(&issuer)
            .await;
        let endpoint = format!("{}/token", issuer.uri());
        tokio::task::block_in_place(|| {
            assert!(request_device_grant(&endpoint, "synthetic-client", "scope").is_ok());
            assert!(poll_device_grant(&endpoint, "synthetic-client", "synthetic-device").is_ok());
            assert_eq!(
                ReqwestOAuthFormClient
                    .post_form(&endpoint, &[("refresh_token", "synthetic-refresh")])
                    .unwrap()
                    .0,
                200
            );
        });
        assert!(destination.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn discovery_honors_advertised_endpoints() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/.well-known/openid-configuration"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "issuer": server.uri(),
                "device_authorization_endpoint": format!("{}/custom/device", server.uri()),
                "token_endpoint": format!("{}/custom/token", server.uri()),
            })))
            .expect(1)
            .mount(&server)
            .await;
        let endpoints = tokio::task::block_in_place(|| {
            resolve_oauth_endpoints(&XAI_OAUTH_PARAMS, &server.uri())
        });
        assert_eq!(
            endpoints.device_authorization_endpoint.as_deref(),
            Some(format!("{}/custom/device", server.uri()).as_str())
        );
        assert_eq!(
            endpoints.token_endpoint,
            format!("{}/custom/token", server.uri())
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn discovery_issuer_mismatch_falls_back_to_documented_paths() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/.well-known/openid-configuration"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "issuer": "https://someone-else.example",
                "device_authorization_endpoint": "https://someone-else.example/device",
                "token_endpoint": "https://someone-else.example/token",
            })))
            .expect(1)
            .mount(&server)
            .await;
        let endpoints = tokio::task::block_in_place(|| {
            resolve_oauth_endpoints(&XAI_OAUTH_PARAMS, &server.uri())
        });
        assert_eq!(
            endpoints.device_authorization_endpoint.as_deref(),
            Some(format!("{}/oauth2/device/code", server.uri()).as_str())
        );
        assert_eq!(
            endpoints.token_endpoint,
            format!("{}/oauth2/token", server.uri())
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn device_login_aborts_on_untrusted_verification_uri() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/.well-known/openid-configuration"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "issuer": server.uri(),
                "device_authorization_endpoint": format!("{}/oauth2/device-advertised", server.uri()),
                "token_endpoint": format!("{}/oauth2/token", server.uri()),
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/oauth2/device-advertised"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "device_code": "device-token",
                "user_code": "CW-TEST",
                "verification_uri": "https://auth.x.ai/device",
                "verification_uri_complete": "vscode://attacker/run?code=CW-TEST",
                "expires_in": 60,
                "interval": 1
            })))
            .expect(1)
            .mount(&server)
            .await;
        // No token-endpoint mock: the flow must fail before it ever polls.
        let inputs = ResolvedOAuthInputs {
            issuer: server.uri(),
            client_id: "test-client".to_string(),
            scopes: "openid".to_string(),
            open_browser: false,
        };
        let result =
            tokio::task::block_in_place(|| device_code_login_with(OAuthProvider::Xai, &inputs));
        let Err(error) = result else {
            panic!("a non-web verification URI must abort login");
        };
        assert!(
            format!("{error:#}").contains("untrusted verification URI"),
            "{error:#}"
        );
    }

    /// Discovery + device grant run on the blocking worker: this fails with
    /// the grant refusal, never with a runtime-drop panic.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn device_login_runs_blocking_http_off_the_executor() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let _lock = crate::test_support::lock_test_env();
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/.well-known/openid-configuration"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "issuer": server.uri(),
                "device_authorization_endpoint": format!("{}/oauth2/device-advertised", server.uri()),
                "token_endpoint": format!("{}/oauth2/token-advertised", server.uri())
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/oauth2/device-advertised"))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "error": "invalid_scope",
                "error_description": "mock refusal before browser or polling"
            })))
            .expect(1)
            .mount(&server)
            .await;
        let _issuer =
            crate::test_support::EnvVarGuard::set("GROK_OIDC_ISSUER", server.uri().as_str());
        let _no_browser =
            crate::test_support::EnvVarGuard::set("CODEWHALE_XAI_OAUTH_NO_BROWSER", "1");

        let result = device_code_login(OAuthProvider::Xai).await;
        let Err(error) = result else {
            panic!("mock device request must fail without a runtime-drop panic");
        };
        let message = format!("{error:#}");
        assert!(message.contains("invalid_scope"), "{message}");
        assert!(message.contains("HTTP 400"), "{message}");
    }

    /// Full orchestration against mocks: discovery, grant, one poll, done.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn device_login_exchanges_and_returns_token_material() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/.well-known/openid-configuration"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "issuer": server.uri(),
                "device_authorization_endpoint": format!("{}/oauth2/device-advertised", server.uri()),
                "token_endpoint": format!("{}/oauth2/token-advertised", server.uri())
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/oauth2/device-advertised"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "device_code": "device-token",
                "user_code": "CW-TEST",
                "verification_uri": format!("{}/verify", server.uri()),
                "expires_in": 60,
                "interval": 1
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/oauth2/token-advertised"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "unified-access",
                "refresh_token": "unified-refresh",
                "expires_in": 3600
            })))
            .expect(1)
            .mount(&server)
            .await;
        let inputs = ResolvedOAuthInputs {
            issuer: server.uri(),
            client_id: "test-client".to_string(),
            scopes: "openid".to_string(),
            open_browser: false,
        };
        let pending =
            tokio::task::block_in_place(|| device_code_login_with(OAuthProvider::Xai, &inputs))
                .expect("mock login exchanges");
        assert_eq!(pending.issuer, server.uri());
        assert_eq!(
            pending.token.access_token.as_deref(),
            Some("unified-access")
        );
        assert_eq!(
            pending.token.refresh_token.as_deref(),
            Some("unified-refresh")
        );
    }

    /// The shared poll loop walks pending and slow_down to completion.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn device_login_polls_through_pending_and_slow_down() {
        use codewhale_config::device_code::DeviceCodePoll;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        // wiremock matches mocks in mount order, so mount the one-shot
        // transient-error responses before the terminal success response:
        // poll 1 -> authorization_pending, poll 2 -> slow_down, poll 3 -> ok.
        for (body, status) in [
            (serde_json::json!({ "error": "authorization_pending" }), 400),
            (serde_json::json!({ "error": "slow_down" }), 400),
        ] {
            Mock::given(method("POST"))
                .and(path("/oauth2/token"))
                .respond_with(ResponseTemplate::new(status).set_body_json(body))
                .up_to_n_times(1)
                .expect(1)
                .mount(&server)
                .await;
        }
        Mock::given(method("POST"))
            .and(path("/oauth2/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({ "access_token": "loop-access", "expires_in": 3600 }),
            ))
            .expect(1)
            .mount(&server)
            .await;
        let endpoint = format!("{}/oauth2/token", server.uri());
        let material = tokio::task::block_in_place(|| {
            DeviceCodePoll::new(
                std::time::Duration::from_secs(60),
                "mock poll must complete",
            )
            .run(
                |_| {},
                || poll_device_grant(&endpoint, "test-client", "device-token"),
            )
        })
        .expect("poll loop completes");
        assert!(matches!(
            material.access_token.as_deref(),
            Some("loop-access")
        ));
    }

    /// A denied grant stops the full login with the server's reason.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn device_login_surfaces_user_denial() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/.well-known/openid-configuration"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "issuer": server.uri(),
                "device_authorization_endpoint": format!("{}/oauth2/device-advertised", server.uri()),
                "token_endpoint": format!("{}/oauth2/token-advertised", server.uri())
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/oauth2/device-advertised"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "device_code": "device-token",
                "user_code": "CW-TEST",
                "verification_uri": format!("{}/verify", server.uri()),
                "expires_in": 60,
                "interval": 1
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/oauth2/token-advertised"))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "error": "access_denied",
                "error_description": "The user denied the authorization request"
            })))
            .expect(1)
            .mount(&server)
            .await;
        let inputs = ResolvedOAuthInputs {
            issuer: server.uri(),
            client_id: "test-client".to_string(),
            scopes: "openid".to_string(),
            open_browser: false,
        };
        let result =
            tokio::task::block_in_place(|| device_code_login_with(OAuthProvider::Xai, &inputs));
        let Err(error) = result else {
            panic!("user denial must stop the login");
        };
        let message = format!("{error:#}");
        assert!(message.contains("access_denied"), "{message}");
        assert!(message.contains("HTTP 400"), "{message}");
    }

    /// Non-JSON answers name the content type, never the body.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn device_transport_reports_non_json_without_echoing_body() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        // set_body_bytes carries no implicit content type, so the inserted
        // text/html is the only one on the wire (set_body_string would
        // stack text/plain next to it and the diagnostic would name both).
        Mock::given(method("POST"))
            .and(path("/oauth2/device-code"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes("<html>sentinel-body-bytes</html>".as_bytes())
                    .insert_header("content-type", "text/html"),
            )
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/oauth2/token"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes("<html>sentinel-body-bytes</html>".as_bytes())
                    .insert_header("content-type", "text/html"),
            )
            .expect(1)
            .mount(&server)
            .await;
        let grant = tokio::task::block_in_place(|| {
            request_device_grant(
                &format!("{}/oauth2/device-code", server.uri()),
                "test-client",
                "openid",
            )
        });
        let Err(grant_error) = grant else {
            panic!("non-JSON grant must fail");
        };
        let poll = tokio::task::block_in_place(|| {
            poll_device_grant(
                &format!("{}/oauth2/token", server.uri()),
                "test-client",
                "device-token",
            )
        });
        let Err(poll_error) = poll else {
            panic!("non-JSON poll must fail");
        };
        for message in [format!("{grant_error:#}"), format!("{poll_error:#}")] {
            assert!(message.contains("text/html"), "{message}");
            assert!(!message.contains("sentinel-body-bytes"), "{message}");
        }
    }

    /// The wire format is a contract: form-encoded posts carrying the exact
    /// client, scope, and grant-type parameters the issuers expect.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn device_transport_posts_exact_oauth_form_parameters() {
        use wiremock::matchers::{body_string_contains, header, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/.well-known/openid-configuration"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "issuer": server.uri(),
                "device_authorization_endpoint": format!("{}/oauth2/device-advertised", server.uri()),
                "token_endpoint": format!("{}/oauth2/token-advertised", server.uri())
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/oauth2/device-advertised"))
            .and(header("content-type", "application/x-www-form-urlencoded"))
            .and(body_string_contains("client_id=test-client"))
            .and(body_string_contains("scope=openid"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "device_code": "device-token",
                "user_code": "CW-TEST",
                "verification_uri": format!("{}/verify", server.uri()),
                "expires_in": 60,
                "interval": 1
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/oauth2/token-advertised"))
            .and(header("content-type", "application/x-www-form-urlencoded"))
            .and(body_string_contains(
                "grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code",
            ))
            .and(body_string_contains("client_id=test-client"))
            .and(body_string_contains("device_code=device-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "form-access",
                "expires_in": 3600
            })))
            .expect(1)
            .mount(&server)
            .await;
        let inputs = ResolvedOAuthInputs {
            issuer: server.uri(),
            client_id: "test-client".to_string(),
            scopes: "openid".to_string(),
            open_browser: false,
        };
        let pending =
            tokio::task::block_in_place(|| device_code_login_with(OAuthProvider::Xai, &inputs))
                .expect("mock login exchanges");
        assert_eq!(pending.token.access_token.as_deref(), Some("form-access"));
    }

    #[test]
    fn access_method_labels_name_every_route() {
        assert_eq!(
            AccessMethod::OwnedOAuth(OAuthProvider::Xai).label(),
            "xAI subscription"
        );
        assert_eq!(
            AccessMethod::ExternalImport(ExternalImportSource::CodexCli).label(),
            "Codex CLI import"
        );
        assert_eq!(AccessMethod::ApiKey.label(), "API key");
        assert_eq!(AccessMethod::AcpBridge.label(), "ACP bridge");
    }

    #[test]
    fn malformed_codex_credential_errors_never_echo_file_contents() {
        let _lock = crate::test_support::lock_test_env();
        let home = tempfile::tempdir().expect("temp Codex home");
        let path = home.path().canonicalize().unwrap().join("auth.json");
        let sentinel = "must-not-appear-in-diagnostics";
        std::fs::write(
            &path,
            format!(r#"{{"tokens":{{"access_token":{{"secret":"{sentinel}"}}}}}}"#),
        )
        .unwrap();

        let error = load_credentials(&grant(&path)).expect_err("malformed schema");
        let message = format!("{error:#}");
        assert!(message.contains("not valid credential JSON"), "{message}");
        assert!(!message.contains(sentinel), "{message}");
    }

    // ── unified PKCE core (ported from the deleted chatgpt_oauth flow) ──

    use std::sync::Mutex;

    type MockForm = Vec<(String, String)>;
    type MockPost = (String, MockForm);

    struct MockFormClient {
        responses: Mutex<Vec<(u16, String)>>,
        posts: Mutex<Vec<MockPost>>,
    }

    impl MockFormClient {
        fn new(responses: Vec<(u16, String)>) -> Self {
            Self {
                responses: Mutex::new(responses),
                posts: Mutex::new(Vec::new()),
            }
        }
    }

    impl OAuthFormClient for MockFormClient {
        fn post_form(&self, url: &str, form: &[(&str, &str)]) -> Result<(u16, String)> {
            self.posts.lock().expect("posts").push((
                url.to_string(),
                form.iter()
                    .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                    .collect(),
            ));
            let mut responses = self.responses.lock().expect("responses");
            anyhow::ensure!(
                !responses.is_empty(),
                "mock issuer has no remaining responses"
            );
            Ok(responses.remove(0))
        }
    }

    fn jwt_with_account(account: &str) -> String {
        let payload = URL_SAFE_NO_PAD.encode(format!(
            r#"{{"https://api.openai.com/auth":{{"chatgpt_account_id":"{account}"}}}}"#
        ));
        format!("header.{payload}.sig")
    }

    fn chatgpt() -> &'static OAuthProviderParams {
        oauth_provider_params(OAuthProvider::Chatgpt)
    }

    #[test]
    fn pkce_verifier_and_challenge_are_s256() {
        let pkce = generate_pkce();
        assert!(pkce.verifier.len() >= 43);
        assert_eq!(
            pkce.challenge,
            URL_SAFE_NO_PAD.encode(Sha256::digest(pkce.verifier.as_bytes()))
        );
        let other = generate_pkce();
        assert_ne!(pkce.verifier, other.verifier);
        assert_ne!(generate_state(), generate_state());
    }

    #[test]
    fn malformed_issuer_fails_loudly_not_to_production() {
        let pkce = PkceChallenge {
            verifier: "verifier".into(),
            challenge: "challenge".into(),
        };
        let err = build_authorize_url(
            chatgpt(),
            "not a url \\ ",
            "client",
            "openid",
            "http://localhost:1455/auth/callback",
            "state-1",
            &pkce,
        )
        .expect_err("malformed issuer must not produce an authorize URL");
        assert!(
            format!("{err:#}").contains("CODEWHALE_CHATGPT_OAUTH_ISSUER"),
            "{err:#}"
        );
    }

    #[test]
    fn authorize_url_is_honest_originator_and_pkce() {
        let pkce = PkceChallenge {
            verifier: "verifier".into(),
            challenge: "challenge".into(),
        };
        let url = build_authorize_url(
            chatgpt(),
            CHATGPT_OAUTH_ISSUER,
            CHATGPT_OAUTH_CLIENT_ID,
            CHATGPT_OAUTH_SCOPE,
            "http://localhost:1455/auth/callback",
            "state-1",
            &pkce,
        )
        .expect("static issuer parses");
        assert!(url.starts_with("https://auth.openai.com/oauth/authorize?"));
        assert!(url.contains("code_challenge=challenge"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("originator=codewhale"));
        assert!(!url.contains("codex_cli_rs"));
        assert!(url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A1455%2Fauth%2Fcallback"));
        assert!(url.contains("id_token_add_organizations=true"));
    }

    #[test]
    fn authorize_url_rejects_providers_without_a_browser_flow() {
        let pkce = PkceChallenge {
            verifier: "verifier".into(),
            challenge: "challenge".into(),
        };
        let err = build_authorize_url(
            oauth_provider_params(OAuthProvider::Xai),
            "https://auth.x.ai",
            "client",
            "openid",
            "http://localhost:1455/auth/callback",
            "state-1",
            &pkce,
        )
        .expect_err("xAI has no browser flow");
        assert!(
            format!("{err:#}").contains("no browser sign-in flow"),
            "{err:#}"
        );
    }

    #[test]
    fn callback_success_requires_matching_state() {
        let ok = parse_callback_query(chatgpt(), "code=abc&state=s1").unwrap();
        assert_eq!(accept_callback("s1", ok).unwrap(), "abc");
        let mismatch = parse_callback_query(chatgpt(), "code=abc&state=other").unwrap();
        let err = accept_callback("s1", mismatch).unwrap_err().to_string();
        assert!(err.contains("state did not match"), "{err}");
    }

    #[test]
    fn callback_error_is_user_visible_without_code() {
        let outcome = parse_callback_query(
            chatgpt(),
            "error=access_denied&error_description=nope&state=s1",
        )
        .unwrap();
        let err = accept_callback("s1", outcome).unwrap_err().to_string();
        assert!(err.contains("nope"), "{err}");
        assert!(!err.contains("access_token"));
    }

    #[test]
    fn callback_missing_code_fails() {
        let err = parse_callback_query(chatgpt(), "state=s1")
            .unwrap_err()
            .to_string();
        assert!(err.contains("missing authorization code"), "{err}");
    }

    #[test]
    fn token_exchange_uses_pkce_verifier_against_mock_issuer() {
        let client = MockFormClient::new(vec![(
            200,
            serde_json::json!({
                "access_token": "at-1",
                "refresh_token": "rt-1",
                "expires_in": 3600,
                "id_token": jwt_with_account("acct-9")
            })
            .to_string(),
        )]);
        let token = exchange_authorization_code(
            &client,
            chatgpt(),
            &form_token_url(chatgpt(), CHATGPT_OAUTH_ISSUER),
            CHATGPT_OAUTH_CLIENT_ID,
            "http://localhost:1455/auth/callback",
            "auth-code",
            "verifier",
        )
        .unwrap();
        assert_eq!(token.access_token.as_deref(), Some("at-1"));
        let posts = client.posts.lock().unwrap();
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].0, "https://auth.openai.com/oauth/token");
        let form: std::collections::BTreeMap<_, _> = posts[0].1.iter().cloned().collect();
        assert_eq!(form["grant_type"], "authorization_code");
        assert_eq!(form["code_verifier"], "verifier");
        assert_eq!(form["code"], "auth-code");
    }

    #[test]
    fn token_exchange_error_does_not_echo_body_secrets() {
        let client = MockFormClient::new(vec![(
            400,
            serde_json::json!({
                "error": "invalid_grant",
                "error_description": "secret-must-not-leak"
            })
            .to_string(),
        )]);
        // OAuthTokenMaterial is deliberately Debug-free; extract the error
        // without demanding a Debug bound on the success type.
        let err = match exchange_authorization_code(
            &client,
            chatgpt(),
            &form_token_url(chatgpt(), CHATGPT_OAUTH_ISSUER),
            CHATGPT_OAUTH_CLIENT_ID,
            "http://localhost:1455/auth/callback",
            "bad",
            "verifier",
        ) {
            Ok(_) => panic!("invalid_grant must fail"),
            Err(err) => err.to_string(),
        };
        assert!(err.contains("permanently"), "{err}");
        assert!(!err.contains("secret-must-not-leak"), "{err}");
    }

    #[test]
    fn callback_server_handles_success_and_error_requests() {
        use std::io::Write as _;
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind test port");
        listener.set_nonblocking(false).unwrap();
        let addr = listener.local_addr().unwrap();
        let state = "state-xyz".to_string();
        let expected = state.clone();
        let params = chatgpt();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept");
            handle_callback_stream(stream, params, &expected)
        });
        let mut client = std::net::TcpStream::connect(addr).expect("connect");
        write!(
            client,
            "GET /auth/callback?code=tok&state={state} HTTP/1.1\r\nHost: localhost\r\n\r\n"
        )
        .unwrap();
        let code = server.join().expect("server").expect("callback ok");
        assert_eq!(code, "tok");

        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind error port");
        listener.set_nonblocking(false).unwrap();
        let addr = listener.local_addr().unwrap();
        let params = chatgpt();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept");
            handle_callback_stream(stream, params, "state-xyz")
        });
        let mut client = std::net::TcpStream::connect(addr).expect("connect");
        write!(
            client,
            "GET /auth/callback?error=access_denied&state=state-xyz HTTP/1.1\r\nHost: localhost\r\n\r\n"
        )
        .unwrap();
        let err = server.join().expect("server").unwrap_err().to_string();
        assert!(err.contains("not completed"), "{err}");
    }

    /// The registered redirect URI says `localhost`, which resolves to `::1`
    /// as readily as `127.0.0.1`. A callback arriving on the IPv6 listener has
    /// to be accepted, or an IPv6-first browser hangs until the timeout.
    #[test]
    fn callback_is_accepted_on_either_loopback_family() {
        use std::io::Write as _;
        for addr in [
            SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
            SocketAddr::from((Ipv6Addr::LOCALHOST, 0)),
        ] {
            let Ok(target) = TcpListener::bind(addr) else {
                // A host without this stack cannot exercise it; the other arm
                // still covers the polling loop.
                continue;
            };
            target.set_nonblocking(true).unwrap();
            let target_addr = target.local_addr().unwrap();

            // A second, permanently idle listener stands in for the family the
            // browser did not pick: `wait_for_callback` must poll past it.
            let idle = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
                .expect("bind idle listener");
            idle.set_nonblocking(true).unwrap();

            let listeners = vec![idle, target];
            let params = chatgpt();
            let server =
                std::thread::spawn(move || wait_for_callback(&listeners, params, "state-xyz"));
            let mut client = std::net::TcpStream::connect(target_addr).expect("connect");
            write!(
                client,
                "GET /auth/callback?code=tok&state=state-xyz HTTP/1.1\r\nHost: localhost\r\n\r\n"
            )
            .unwrap();
            let code = server
                .join()
                .expect("server")
                .unwrap_or_else(|error| panic!("callback on {target_addr} rejected: {error}"));
            assert_eq!(code, "tok", "callback on {target_addr}");
        }
    }

    #[test]
    fn form_refresh_and_revoke_target_the_row_endpoints() {
        let client = MockFormClient::new(vec![
            (
                200,
                serde_json::json!({"access_token": "fresh", "expires_in": 3600}).to_string(),
            ),
            (200, String::new()),
        ]);
        let refreshed = refresh_access_token_via(
            &client,
            chatgpt(),
            &form_token_url(chatgpt(), CHATGPT_OAUTH_ISSUER),
            CHATGPT_OAUTH_CLIENT_ID,
            "rt-1",
        )
        .expect("refresh");
        assert_eq!(refreshed.access_token.as_deref(), Some("fresh"));
        revoke_remote_token_via(
            &client,
            chatgpt(),
            CHATGPT_OAUTH_ISSUER,
            CHATGPT_OAUTH_CLIENT_ID,
            "rt-1",
        )
        .expect("revoke");
        let posts = client.posts.lock().unwrap();
        assert_eq!(posts.len(), 2, "{posts:?}");
        assert!(posts[0].0.ends_with("/oauth/token"), "{posts:?}");
        assert!(
            posts[0]
                .1
                .iter()
                .any(|(k, v)| k == "grant_type" && v == "refresh_token")
        );
        assert!(
            posts[1].0.ends_with("/api/accounts/oauth/revoke"),
            "{posts:?}"
        );
    }

    // ────────────────────────────────────────────────────────────────────
    // Ported from the deleted per-provider modules (§D security pins).
    // ────────────────────────────────────────────────────────────────────

    use tempfile::TempDir;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn auth_mode_accepts_oauth_aliases() {
        for mode in [
            "oauth",
            "xai_oauth",
            "XAI-OAuth",
            "grok",
            "grok_cli",
            "device_code",
            "device-auth",
        ] {
            assert!(
                auth_mode_uses_xai_oauth(mode),
                "expected oauth mode: {mode}"
            );
        }
        assert!(!auth_mode_uses_xai_oauth("api_key"));
        assert!(!auth_mode_uses_xai_oauth("keyring"));
    }

    #[test]
    fn loads_fresh_token_from_grok_auth_json() {
        let _guard = crate::test_support::lock_test_env();
        let dir = TempDir::new().unwrap();
        let root = dir.path().canonicalize().expect("canonical temp root");
        let path = root.join("auth.json");
        let future = rfc3339_from_now(3600);
        let scope = format!("{XAI_OIDC_ISSUER}::{GROK_OIDC_CLIENT_ID}");
        let file = serde_json::json!({
            scope: {
                "key": "test-access-token",
                "refresh_token": "test-refresh",
                "expires_at": future,
                "oidc_issuer": XAI_OIDC_ISSUER,
                "oidc_client_id": GROK_OIDC_CLIENT_ID,
                "auth_mode": "oidc"
            }
        });
        fs::write(&path, serde_json::to_vec_pretty(&file).unwrap()).unwrap();
        let _home_guard = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", &root);
        let _path_guard = crate::test_support::EnvVarGuard::set("GROK_AUTH_PATH", &path);
        let config = Config {
            provider: Some(ApiProvider::Xai.as_str().to_string()),
            providers: Some(crate::config::ProvidersConfig {
                xai: crate::config::ProviderConfig {
                    auth_mode: Some("oauth".to_string()),
                    external_credentials: Some(
                        codewhale_config::ExternalCredentialConsentToml::read_only(
                            codewhale_config::ProviderKind::Xai,
                            codewhale_config::ExternalCredentialSource::GrokCli,
                            path.clone(),
                        ),
                    ),
                    ..Default::default()
                },
                ..Default::default()
            }),
            ..Default::default()
        };
        crate::external_credentials::reset_side_effect_trap();
        let result = get_xai_credentials(&config);
        let creds = result.expect("load");
        assert_eq!(creds.access_token, "test-access-token");
        assert_eq!(creds.client_id, GROK_OIDC_CLIENT_ID);
        assert_eq!(
            crate::external_credentials::side_effect_trap_counts(),
            (1, 1)
        );
    }

    #[test]
    fn disabled_external_grok_credentials_cause_zero_external_io() {
        let _guard = crate::test_support::lock_test_env();
        let dir = TempDir::new().unwrap();
        let root = dir.path().canonicalize().expect("canonical temp root");
        let path = root.join("external-grok-auth.json");
        let raw = serde_json::json!({
            format!("{XAI_OIDC_ISSUER}::{GROK_OIDC_CLIENT_ID}"): {
                "key": "must-never-be-read",
                "refresh_token": "must-never-be-used",
                "expires_at": rfc3339_from_now(3600),
                "future_field": {"preserve": true}
            }
        })
        .to_string();
        fs::write(&path, &raw).unwrap();
        let owned_home = root.join("codewhale-owned");
        let _home_guard = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", &owned_home);
        let _path_guard = crate::test_support::EnvVarGuard::set("GROK_AUTH_PATH", &path);
        let config = Config {
            provider: Some(ApiProvider::Xai.as_str().to_string()),
            providers: Some(crate::config::ProvidersConfig {
                xai: crate::config::ProviderConfig {
                    auth_mode: Some("oauth".to_string()),
                    ..Default::default()
                },
                ..Default::default()
            }),
            ..Default::default()
        };

        crate::external_credentials::reset_side_effect_trap();
        assert!(!credentials_valid(OAuthProvider::Xai, &config));
        let error = match get_xai_credentials(&config) {
            Ok(_) => panic!("external access is disabled"),
            Err(e) => e,
        };
        assert!(error.to_string().contains("are disabled"));
        assert_eq!(
            crate::external_credentials::side_effect_trap_counts(),
            (0, 0)
        );
        assert_eq!(
            crate::external_credentials::complete_side_effect_trap_counts(),
            (0, 0, 0, 0, 0),
            "disabled external authority must reach no credential or OAuth sink"
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), raw);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn expired_read_only_external_credentials_never_refresh_rewrite_or_network() {
        let _guard = crate::test_support::lock_test_env();
        let server = MockServer::start().await;
        let dir = TempDir::new().unwrap();
        let root = dir.path().canonicalize().expect("canonical temp root");
        let path = root.join("external-grok-auth.json");
        let scope = format!("{}::{GROK_OIDC_CLIENT_ID}", server.uri());
        let raw = serde_json::json!({
            scope: {
                "key": "expired-external-access",
                "refresh_token": "must-never-be-submitted",
                "expires_at": rfc3339_from_unix(now_unix_secs().unwrap_or(0) - 3600),
                "oidc_issuer": server.uri(),
                "oidc_client_id": GROK_OIDC_CLIENT_ID,
                "future_field": {"preserve": true}
            }
        })
        .to_string();
        fs::write(&path, &raw).unwrap();
        let owned_home = root.join("codewhale-owned");
        let _home_guard = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", &owned_home);
        let _path_guard = crate::test_support::EnvVarGuard::set("GROK_AUTH_PATH", &path);
        let config = Config {
            provider: Some(ApiProvider::Xai.as_str().to_string()),
            providers: Some(crate::config::ProvidersConfig {
                xai: crate::config::ProviderConfig {
                    auth_mode: Some("oauth".to_string()),
                    external_credentials: Some(
                        codewhale_config::ExternalCredentialConsentToml::read_only(
                            codewhale_config::ProviderKind::Xai,
                            codewhale_config::ExternalCredentialSource::GrokCli,
                            path.clone(),
                        ),
                    ),
                    ..Default::default()
                },
                ..Default::default()
            }),
            ..Default::default()
        };

        crate::external_credentials::reset_side_effect_trap();
        let error = match tokio::task::block_in_place(|| get_xai_credentials(&config)) {
            Ok(_) => panic!("read-only external credentials must fail instead of refreshing"),
            Err(e) => e,
        };
        assert!(
            error
                .to_string()
                .contains("Read-only consent never refreshes")
        );
        assert_eq!(
            crate::external_credentials::side_effect_trap_counts(),
            (1, 1)
        );
        assert_eq!(
            crate::external_credentials::complete_side_effect_trap_counts(),
            (1, 1, 0, 0, 0),
            "read-only external expiry must not reach write, refresh, or network sinks"
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), raw);
        assert!(!owned_home.join("credentials/xai-auth.json").exists());
        assert!(
            server
                .received_requests()
                .await
                .expect("recorded requests")
                .is_empty(),
            "external refresh tokens must never be sent over the network"
        );
    }

    /// #4763 root trigger, re-pinned for #5772. A returning xAI-OAuth user
    /// whose only material is an external Grok CLI grant loses readiness the
    /// moment that CLI's short-lived access token expires, even though a
    /// refresh token sits right beside it — read-only consent deliberately
    /// never refreshes or rewrites another CLI's file, so there is nothing to
    /// renew it with. `needs_api_key` therefore flips to true and onboarding
    /// reopens. That is the intended invariant, not a leak, and it is what
    /// stops a surviving consent record from reading as a stored credential;
    /// this test pins it so the onboarding entry point stays explainable.
    #[test]
    fn expired_external_grok_grant_reads_as_missing_key_despite_refresh_token() {
        let _guard = crate::test_support::lock_test_env();
        let dir = TempDir::new().unwrap();
        let root = dir.path().canonicalize().expect("canonical temp root");
        let path = root.join("external-grok-auth.json");
        let scope = format!("https://auth.x.ai::{GROK_OIDC_CLIENT_ID}");
        fs::write(
            &path,
            serde_json::json!({
                scope.clone(): {
                    "key": "expired-external-access",
                    "refresh_token": "present-but-unusable-under-read-only-consent",
                    "expires_at": rfc3339_from_unix(now_unix_secs().unwrap_or(0) - 3600),
                    "oidc_client_id": GROK_OIDC_CLIENT_ID,
                }
            })
            .to_string(),
        )
        .unwrap();
        let _home_guard =
            crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", root.join("codewhale-owned"));
        let _path_guard = crate::test_support::EnvVarGuard::set("GROK_AUTH_PATH", &path);
        let _key_guard = crate::test_support::EnvVarGuard::remove("XAI_API_KEY");
        let config = Config {
            provider: Some(ApiProvider::Xai.as_str().to_string()),
            providers: Some(crate::config::ProvidersConfig {
                xai: crate::config::ProviderConfig {
                    auth_mode: Some("oauth".to_string()),
                    external_credentials: Some(
                        codewhale_config::ExternalCredentialConsentToml::read_only(
                            codewhale_config::ProviderKind::Xai,
                            codewhale_config::ExternalCredentialSource::GrokCli,
                            path.clone(),
                        ),
                    ),
                    ..Default::default()
                },
                ..Default::default()
            }),
            ..Default::default()
        };

        crate::external_credentials::reset_side_effect_trap();
        assert!(
            !credentials_present(OAuthProvider::Xai, &config),
            "an expired external access token is not usable material"
        );
        assert!(
            !crate::config::has_api_key_for(&config, ApiProvider::Xai),
            "expired external xAI OAuth must fall through to the missing-key path"
        );
        assert_eq!(
            crate::external_credentials::complete_side_effect_trap_counts().2,
            0,
            "a consented read never rewrites Codewhale-owned storage"
        );
        assert_eq!(
            crate::external_credentials::complete_side_effect_trap_counts().3,
            0,
            "a consented read never refreshes another CLI's token"
        );

        // The same file with a live access token is ready, so the check is
        // expiry-driven rather than a blanket rejection of external grants.
        fs::write(
            &path,
            serde_json::json!({
                scope: {
                    "key": "fresh-external-access",
                    "refresh_token": "unused",
                    "expires_at": rfc3339_from_now(3600),
                    "oidc_client_id": GROK_OIDC_CLIENT_ID,
                }
            })
            .to_string(),
        )
        .unwrap();
        assert!(credentials_present(OAuthProvider::Xai, &config));
        assert!(crate::config::has_api_key_for(&config, ApiProvider::Xai));
    }

    #[test]
    fn native_login_storage_is_codewhale_owned() {
        let _guard = crate::test_support::lock_test_env();
        let dir = TempDir::new().unwrap();
        let grok_path = dir.path().join("external-grok-auth.json");
        let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", dir.path());
        let _grok = crate::test_support::EnvVarGuard::set("GROK_AUTH_PATH", &grok_path);

        let owned = codewhale_config::legacy_xai_oauth_path().expect("Codewhale-owned auth path");
        assert_eq!(owned, dir.path().join("credentials/xai-auth.json"));
        assert_ne!(owned, grok_auth_file_path());
    }

    /// #4257 storage contract: the on-disk credential file is a JSON object
    /// keyed `{issuer}::{client_id}` whose entries use the Grok CLI's field
    /// names. Consolidating the device-code poller must not touch it, so pin
    /// the format with literal bytes rather than a round-trip through the
    /// writer — a round-trip would follow the code if the code drifted.
    #[test]
    fn a_token_stored_in_the_current_on_disk_format_still_loads() {
        let _guard = crate::test_support::lock_test_env();
        let dir = TempDir::new().unwrap();
        let home = dir
            .path()
            .canonicalize()
            .expect("canonical temp root")
            .join("owned-home");
        fs::create_dir_all(&home).unwrap();
        let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", &home);

        let generation = "xai-auth-fedcba9876543210fedcba9876543210.json";
        let expires_at = rfc3339_from_now(3600);
        let stored = format!(
            r#"{{
  "https://auth.x.ai::b1a00492-073a-47ea-816f-4c329264a828": {{
    "key": "stored-access-token",
    "refresh_token": "stored-refresh-token",
    "expires_at": "{expires_at}",
    "oidc_issuer": "https://auth.x.ai",
    "oidc_client_id": "b1a00492-073a-47ea-816f-4c329264a828",
    "auth_mode": "oidc",
    "unknown_cli_field": "preserved"
  }}
}}"#
        );

        let credentials = codewhale_config::with_xai_oauth_lifecycle_lock(|store| {
            store.write(generation, stored.as_bytes(), false)?;
            // A fresh stored token must be used as-is: refreshing here would
            // mean an existing login stopped working offline.
            get_owned_credentials_locked(OAuthProvider::Xai, store, generation, |_, _, _| {
                panic!("a fresh stored token must not be refreshed")
            })
        })
        .expect("read back a credential stored in the current format");

        assert_eq!(credentials.access_token, "stored-access-token");
        assert_eq!(
            credentials.refresh_token.as_deref(),
            Some("stored-refresh-token")
        );
        assert_eq!(credentials.issuer, XAI_OIDC_ISSUER);
        assert_eq!(credentials.client_id, GROK_OIDC_CLIENT_ID);
    }

    /// `Debug` reaches production through tracing's `?` sigil, anyhow context,
    /// and panic messages. Nothing that holds bearer material may print it.
    #[test]
    fn debug_output_never_contains_bearer_material() {
        let entry = OwnedAuthEntry {
            access_token: Some("secret-access-token".to_string()),
            refresh_token: Some("secret-refresh-token".to_string()),
            expires_at: Some("2030-01-01T00:00:00.000Z".to_string()),
            id_token: None,
            account_id: None,
            oidc_issuer: Some(XAI_OIDC_ISSUER.to_string()),
            oidc_client_id: Some(GROK_OIDC_CLIENT_ID.to_string()),
            originator: None,
            auth_mode: Some("oidc".to_string()),
            extra: BTreeMap::new(),
        };
        let credentials = credentials_from_entry(
            OAuthProvider::Xai,
            &format!("{XAI_OIDC_ISSUER}::{GROK_OIDC_CLIENT_ID}"),
            &entry,
            "secret-access-token".to_string(),
        );
        let activation = OAuthActivation {
            credentials: credentials.clone(),
            config_path: PathBuf::from("/tmp/config.toml"),
            auth_path: PathBuf::from("/tmp/auth.json"),
        };

        let rendered = format!("{entry:?} {activation:?}");
        for secret in ["secret-access-token", "secret-refresh-token"] {
            assert!(!rendered.contains(secret), "{secret} leaked: {rendered}");
        }
        // The shape stays useful for diagnosis.
        assert!(rendered.contains("<redacted>"), "{rendered}");
        assert!(rendered.contains(GROK_OIDC_CLIENT_ID), "{rendered}");
    }

    fn pending_login(access: &str, refresh: &str) -> PendingOAuthLogin {
        pending_login_for_test(OAuthProvider::Xai, access, refresh)
    }

    fn seed_expired_owned_generation() -> String {
        let generation = "xai-auth-0123456789abcdef0123456789abcdef.json".to_string();
        codewhale_config::with_xai_oauth_lifecycle_lock(|store| {
            let scope = format!("{}::{}", XAI_OIDC_ISSUER, GROK_OIDC_CLIENT_ID);
            let mut file = AuthFile::new();
            file.insert(
                scope,
                OwnedAuthEntry {
                    access_token: Some("expired-access".to_string()),
                    refresh_token: Some("initial-refresh".to_string()),
                    expires_at: Some("1970-01-01T00:00:00.000Z".to_string()),
                    id_token: None,
                    account_id: None,
                    oidc_issuer: Some(XAI_OIDC_ISSUER.to_string()),
                    oidc_client_id: Some(GROK_OIDC_CLIENT_ID.to_string()),
                    originator: None,
                    auth_mode: Some("oidc".to_string()),
                    extra: BTreeMap::new(),
                },
            );
            write_auth_file_to_store(store, &generation, &file, false)
        })
        .expect("seed expired owned generation");
        generation
    }

    fn seed_legacy_owned_credentials() -> PathBuf {
        codewhale_config::with_xai_oauth_lifecycle_lock(|store| {
            let scope = format!("{}::{}", XAI_OIDC_ISSUER, GROK_OIDC_CLIENT_ID);
            let mut legacy = AuthFile::new();
            legacy.insert(
                scope,
                OwnedAuthEntry {
                    access_token: Some("legacy-access".to_string()),
                    refresh_token: Some("legacy-refresh".to_string()),
                    expires_at: Some(rfc3339_from_now(3600)),
                    id_token: None,
                    account_id: None,
                    oidc_issuer: Some(XAI_OIDC_ISSUER.to_string()),
                    oidc_client_id: Some(GROK_OIDC_CLIENT_ID.to_string()),
                    originator: None,
                    auth_mode: Some("oidc".to_string()),
                    extra: BTreeMap::new(),
                },
            );
            write_auth_file_to_store(
                store,
                codewhale_config::LEGACY_XAI_OAUTH_FILE_NAME,
                &legacy,
                false,
            )?;
            store.path_for(codewhale_config::LEGACY_XAI_OAUTH_FILE_NAME)
        })
        .expect("seed legacy credentials")
    }

    #[test]
    fn concurrent_refreshes_share_one_rotated_epoch() {
        let _guard = crate::test_support::lock_test_env();
        let dir = TempDir::new().unwrap();
        let home = dir
            .path()
            .canonicalize()
            .expect("canonical temp root")
            .join("owned-home");
        let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", &home);
        let generation = seed_expired_owned_generation();
        let refreshes = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();

        let first_generation = generation.clone();
        let first_refreshes = refreshes.clone();
        let first = std::thread::spawn(move || {
            codewhale_config::with_xai_oauth_lifecycle_lock(|store| {
                get_owned_credentials_locked(
                    OAuthProvider::Xai,
                    store,
                    &first_generation,
                    |_, _, refresh| {
                        assert_eq!(refresh, "initial-refresh");
                        first_refreshes.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        entered_tx.send(()).unwrap();
                        release_rx.recv().unwrap();
                        Ok(OAuthTokenMaterial {
                            id_token: None,
                            access_token: Some("rotated-access".to_string()),
                            refresh_token: Some("rotated-refresh".to_string()),
                            expires_in: Some(3600),
                            error: None,
                            error_description: None,
                            interval: None,
                        })
                    },
                )
            })
        });
        entered_rx.recv().expect("first refresh reached barrier");

        let second_generation = generation.clone();
        let second_refreshes = refreshes.clone();
        let (attempt_tx, attempt_rx) = std::sync::mpsc::channel();
        let second = std::thread::spawn(move || {
            attempt_tx.send(()).unwrap();
            codewhale_config::with_xai_oauth_lifecycle_lock(|store| {
                get_owned_credentials_locked(
                    OAuthProvider::Xai,
                    store,
                    &second_generation,
                    |_, _, _| {
                        second_refreshes.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        bail!("second refresh must observe the first thread's committed token")
                    },
                )
            })
        });
        attempt_rx.recv().expect("second refresh attempted lock");
        release_tx.send(()).expect("release first refresh");

        let first = first.join().unwrap().expect("first refresh");
        let second = second.join().unwrap().expect("second refresh");
        assert_eq!(first.access_token, "rotated-access");
        assert_eq!(second.access_token, "rotated-access");
        assert_eq!(refreshes.load(std::sync::atomic::Ordering::SeqCst), 1);
        codewhale_config::with_xai_oauth_lifecycle_lock(|store| {
            let mut file = load_owned_auth_file_from_store(store, &generation)?
                .context("generation must remain active")?;
            let (_, entry) = select_entry(OAuthProvider::Xai, &mut file).context("stored entry")?;
            assert_eq!(entry.refresh_token.as_deref(), Some("rotated-refresh"));
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn logout_waits_for_refresh_then_revokes_the_committed_epoch() {
        let _guard = crate::test_support::lock_test_env();
        let dir = TempDir::new().unwrap();
        let home = dir
            .path()
            .canonicalize()
            .expect("canonical temp root")
            .join("owned-home");
        fs::create_dir_all(&home).unwrap();
        let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", &home);
        let generation = seed_expired_owned_generation();
        fs::write(
            home.join("config.toml"),
            format!(
                "[providers.xai]\nauth_mode = \"oauth\"\noauth_credential_generation = \"{generation}\"\n"
            ),
        )
        .unwrap();
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();

        let refresh_generation = generation.clone();
        let refresh = std::thread::spawn(move || {
            codewhale_config::with_xai_oauth_lifecycle_lock(|store| {
                get_owned_credentials_locked(
                    OAuthProvider::Xai,
                    store,
                    &refresh_generation,
                    |_, _, _| {
                        entered_tx.send(()).unwrap();
                        release_rx.recv().unwrap();
                        Ok(OAuthTokenMaterial {
                            id_token: None,
                            access_token: Some("last-refresh-access".to_string()),
                            refresh_token: Some("last-refresh-rotation".to_string()),
                            expires_in: Some(3600),
                            error: None,
                            error_description: None,
                            interval: None,
                        })
                    },
                )
            })
        });
        entered_rx.recv().expect("refresh reached barrier");

        let (attempt_tx, attempt_rx) = std::sync::mpsc::channel();
        let config_path = home.join("config.toml");
        let logout = std::thread::spawn(move || {
            attempt_tx.send(()).unwrap();
            codewhale_config::with_xai_oauth_revocation_transaction(|| {
                codewhale_config::mutate_config_document(&config_path, |document| {
                    codewhale_config::unset_config_document_value(
                        document,
                        &["providers", "xai", "oauth_credential_generation"],
                    )?;
                    codewhale_config::unset_config_document_value(
                        document,
                        &["providers", "xai", "auth_mode"],
                    )?;
                    Ok(())
                })
            })
        });
        attempt_rx.recv().expect("logout attempted lifecycle lock");
        release_tx.send(()).expect("release refresh");

        assert_eq!(
            refresh.join().unwrap().expect("refresh").access_token,
            "last-refresh-access"
        );
        logout.join().unwrap().expect("logout");
        let auth_path = home.join("credentials").join(&generation);
        assert!(
            !auth_path.exists(),
            "logout must retire the generation written by the preceding refresh"
        );
        let config = fs::read_to_string(home.join("config.toml")).unwrap();
        assert!(!config.contains("oauth_credential_generation"));
        assert!(!config.contains("auth_mode"));
    }

    #[test]
    fn activation_commits_unique_generation_pointer_and_revokes_external_consent() {
        let _guard = crate::test_support::lock_test_env();
        let dir = TempDir::new().unwrap();
        let home = dir
            .path()
            .canonicalize()
            .expect("canonical temp root")
            .join("owned-home");
        let config_path = dir.path().join("config.toml");
        let external_path = dir.path().join("grok-external.json");
        fs::write(&external_path, "external owner bytes").unwrap();
        fs::write(
            &config_path,
            format!(
                r#"# operator note
[providers.xai]
model = "grok-code-fast-1" # model note
future_setting = "preserve"

[providers.xai.external_credentials]
access = "read_only"
provider = "xai"
source = "grok_cli"
path = {}
consent_version = 1
"#,
                toml::Value::String(external_path.display().to_string())
            ),
        )
        .unwrap();
        let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", &home);
        let consent = codewhale_config::ExternalCredentialConsentToml::read_only(
            codewhale_config::ProviderKind::Xai,
            codewhale_config::ExternalCredentialSource::GrokCli,
            external_path.clone(),
        );
        let mut live = Config {
            providers: Some(crate::config::ProvidersConfig {
                xai: crate::config::ProviderConfig {
                    model: Some("grok-code-fast-1".to_string()),
                    external_credentials: Some(consent),
                    ..Default::default()
                },
                ..Default::default()
            }),
            ..Default::default()
        };

        crate::external_credentials::reset_side_effect_trap();
        let activation = activate_login(
            pending_login("activation-access", "activation-refresh"),
            Some(&config_path),
            Some(&mut live),
        )
        .expect("activate login");

        assert_eq!(activation.config_path, config_path);
        let generation = activation
            .auth_path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("generation basename");
        assert!(codewhale_config::is_valid_xai_oauth_generation(generation));
        let persisted = fs::read_to_string(&config_path).unwrap();
        assert!(persisted.contains("# operator note"));
        assert!(persisted.contains("model = \"grok-code-fast-1\" # model note"));
        assert!(persisted.contains("future_setting = \"preserve\""));
        assert!(persisted.contains("auth_mode = \"oauth\""));
        assert!(persisted.contains(&format!("oauth_credential_generation = \"{generation}\"")));
        assert!(!persisted.contains("external_credentials"));
        assert_eq!(
            fs::read_to_string(&external_path).unwrap(),
            "external owner bytes"
        );
        let owned = fs::read_to_string(&activation.auth_path).unwrap();
        assert!(owned.contains("activation-access"));
        assert!(owned.contains("activation-refresh"));
        #[cfg(unix)]
        assert_eq!(
            fs::metadata(&activation.auth_path)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        let live_xai = live.provider_config_for(ApiProvider::Xai).unwrap();
        assert_eq!(live_xai.auth_mode.as_deref(), Some("oauth"));
        assert_eq!(
            live_xai.oauth_credential_generation.as_deref(),
            Some(generation)
        );
        assert!(live_xai.external_credentials.is_none());
        assert_eq!(
            crate::external_credentials::complete_side_effect_trap_counts(),
            (0, 0, 1, 0, 0),
            "activation must reach exactly the owned write sink"
        );
    }

    #[test]
    fn activation_retires_legacy_owned_file_only_after_config_commit() {
        let _guard = crate::test_support::lock_test_env();
        let dir = TempDir::new().unwrap();
        let home = dir
            .path()
            .canonicalize()
            .expect("canonical temp root")
            .join("owned-home");
        let config_path = dir.path().join("config.toml");
        fs::write(&config_path, "[providers.xai]\nmodel = \"grok-4.5\"\n").unwrap();
        let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", &home);
        let legacy_path = seed_legacy_owned_credentials();
        assert!(legacy_path.exists());

        let activation = activate_login(
            pending_login("new-access", "new-refresh"),
            Some(&config_path),
            None,
        )
        .expect("activate replacement generation");

        assert!(activation.auth_path.exists());
        assert!(
            !legacy_path.exists(),
            "legacy duplicate must be removed after the generation pointer commits"
        );
        let persisted = fs::read_to_string(config_path).unwrap();
        assert!(persisted.contains(activation.auth_path.file_name().unwrap().to_str().unwrap()));
    }

    #[test]
    fn activation_rotation_cleans_only_the_superseded_generation_after_commit() {
        let _guard = crate::test_support::lock_test_env();
        let dir = TempDir::new().unwrap();
        let home = dir
            .path()
            .canonicalize()
            .expect("canonical temp root")
            .join("owned-home");
        let config_path = dir.path().join("config.toml");
        fs::write(&config_path, "[providers.xai]\nmodel = \"grok-4.5\"\n").unwrap();
        let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", &home);
        let mut live = Config::default();

        let first = activate_login(
            pending_login("first-access", "first-refresh"),
            Some(&config_path),
            Some(&mut live),
        )
        .expect("first activation");
        assert!(first.auth_path.exists());
        let first_name = first
            .auth_path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();

        let second = activate_login(
            pending_login("second-access", "second-refresh"),
            Some(&config_path),
            Some(&mut live),
        )
        .expect("second activation");
        assert_ne!(first.auth_path, second.auth_path);
        assert!(second.auth_path.exists());
        assert!(
            !first.auth_path.exists(),
            "superseded generation must be removed only after the new pointer commits"
        );
        let persisted = fs::read_to_string(&config_path).unwrap();
        assert!(!persisted.contains(&first_name));
        assert!(persisted.contains(second.auth_path.file_name().unwrap().to_str().unwrap()));
        assert!(
            fs::read_to_string(second.auth_path)
                .unwrap()
                .contains("second-access")
        );
    }

    #[test]
    fn activation_recovers_from_a_dangling_generation_pointer() {
        let _guard = crate::test_support::lock_test_env();
        let dir = TempDir::new().unwrap();
        let home = dir
            .path()
            .canonicalize()
            .expect("canonical temp root")
            .join("owned-home");
        let config_path = dir.path().join("config.toml");
        // A valid-looking generation pointer whose credential file does not
        // exist: the state Hunter's dogfood machine was bricked in (#5032).
        let stale = "xai-auth-0123456789abcdef0123456789abcdef.json";
        fs::write(
            &config_path,
            format!(
                "[providers.xai]\nauth_mode = \"oauth\"\noauth_credential_generation = \"{stale}\"\n"
            ),
        )
        .unwrap();
        let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", &home);
        let mut live = Config::default();

        let activation = activate_login(
            pending_login("recovered-access", "recovered-refresh"),
            Some(&config_path),
            Some(&mut live),
        )
        .expect("a dangling generation pointer must not brick login");
        assert!(activation.auth_path.exists());
        assert!(
            fs::read_to_string(&activation.auth_path)
                .unwrap()
                .contains("recovered-access")
        );
        let persisted = fs::read_to_string(&config_path).unwrap();
        assert!(
            !persisted.contains(stale),
            "stale pointer must be replaced: {persisted}"
        );
        assert!(persisted.contains(activation.auth_path.file_name().unwrap().to_str().unwrap()));
        assert!(persisted.contains("auth_mode = \"oauth\""));
    }

    #[test]
    fn dangling_generation_pointer_is_detected_and_repaired() {
        let _guard = crate::test_support::lock_test_env();
        let dir = TempDir::new().unwrap();
        let home = dir
            .path()
            .canonicalize()
            .expect("canonical temp root")
            .join("owned-home");
        let config_path = dir.path().join("config.toml");
        // A valid-looking generation pointer whose credential file does not
        // exist: the state Hunter's dogfood machine was bricked in (#5032).
        let stale = "xai-auth-0123456789abcdef0123456789abcdef.json";
        fs::write(
            &config_path,
            format!(
                "[providers.xai]\nauth_mode = \"oauth\"\noauth_credential_generation = \"{stale}\"\n"
            ),
        )
        .unwrap();
        let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", &home);

        let config = Config {
            provider: Some(ApiProvider::Xai.as_str().to_string()),
            providers: Some(crate::config::ProvidersConfig {
                xai: crate::config::ProviderConfig {
                    auth_mode: Some("oauth".to_string()),
                    oauth_credential_generation: Some(stale.to_string()),
                    ..Default::default()
                },
                ..Default::default()
            }),
            ..Default::default()
        };

        assert!(
            owned_generation_is_dangling(OAuthProvider::Xai, &config),
            "OAuth mode pointing at a missing owned file is the #5032 bricked state"
        );
        // Specificity: OAuth selected but no generation configured is the normal
        // "needs auth" state, not a dangling pointer.
        let unconfigured = Config {
            provider: Some(ApiProvider::Xai.as_str().to_string()),
            providers: Some(crate::config::ProvidersConfig {
                xai: crate::config::ProviderConfig {
                    auth_mode: Some("oauth".to_string()),
                    ..Default::default()
                },
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(
            !owned_generation_is_dangling(OAuthProvider::Xai, &unconfigured),
            "an unconfigured OAuth mode must not be reported as dangling"
        );

        clear_dangling_generation(OAuthProvider::Xai, Some(&config_path))
            .expect("best-effort repair must clear the stale pointer");

        let persisted = fs::read_to_string(&config_path).unwrap();
        assert!(
            !persisted.contains(stale),
            "stale generation pointer must be cleared: {persisted}"
        );
        assert!(
            persisted.contains("auth_mode = \"oauth\""),
            "the user's OAuth mode selection must be preserved: {persisted}"
        );
    }

    #[test]
    fn activation_rejects_a_non_string_generation_pointer_without_staging_credentials() {
        let _guard = crate::test_support::lock_test_env();
        let dir = TempDir::new().unwrap();
        let home = dir
            .path()
            .canonicalize()
            .expect("canonical temp root")
            .join("owned-home");
        let config_path = dir.path().join("config.toml");
        let original = "[providers.xai]\noauth_credential_generation = { path = \"attacker\" }\n";
        fs::write(&config_path, original).unwrap();
        let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", &home);

        let error = activate_login(
            pending_login("must-not-stage", "must-not-persist"),
            Some(&config_path),
            None,
        )
        .expect_err("non-string generation pointers must fail closed");
        assert!(error.to_string().contains("not activated"), "{error:#}");
        assert_eq!(fs::read_to_string(&config_path).unwrap(), original);
        let credentials = home.join("credentials");
        assert!(credentials.exists(), "lifecycle lock directory is durable");
        assert!(fs::read_dir(credentials).unwrap().all(|entry| {
            let name = entry.unwrap().file_name();
            let name = name.to_string_lossy();
            name != codewhale_config::LEGACY_XAI_OAUTH_FILE_NAME
                && !codewhale_config::is_valid_xai_oauth_generation(&name)
        }));
    }

    #[cfg(unix)]
    #[test]
    fn activation_failure_cleans_unreferenced_stage_and_keeps_live_config_inert() {
        let _guard = crate::test_support::lock_test_env();
        let dir = TempDir::new().unwrap();
        let home = dir
            .path()
            .canonicalize()
            .expect("canonical temp root")
            .join("owned-home");
        let config_dir = dir.path().join("config-parent");
        fs::create_dir(&config_dir).unwrap();
        let config_path = config_dir.join("config.toml");
        fs::write(&config_path, "[providers.xai]\nauth_mode = \"api_key\"\n").unwrap();
        fs::create_dir(config_dir.join("config.toml.bak")).unwrap();
        let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", &home);
        let legacy_path = seed_legacy_owned_credentials();
        let legacy_before = fs::read(&legacy_path).unwrap();
        let mut live = Config {
            providers: Some(crate::config::ProvidersConfig {
                xai: crate::config::ProviderConfig {
                    auth_mode: Some("api_key".to_string()),
                    api_key: Some("still-selected".to_string()),
                    ..Default::default()
                },
                ..Default::default()
            }),
            ..Default::default()
        };

        let result = activate_login(
            pending_login("must-be-cleaned", "must-not-persist"),
            Some(&config_path),
            Some(&mut live),
        );
        let error = result.expect_err("invalid backup path must fail activation");
        assert!(error.to_string().contains("not activated"), "{error:#}");
        let live_xai = live.provider_config_for(ApiProvider::Xai).unwrap();
        assert_eq!(live_xai.auth_mode.as_deref(), Some("api_key"));
        assert!(live_xai.oauth_credential_generation.is_none());
        assert_eq!(
            fs::read(&legacy_path).unwrap(),
            legacy_before,
            "legacy owned credentials must remain byte-identical until activation commits"
        );
        let credentials = home.join("credentials");
        if credentials.exists() {
            assert!(
                fs::read_dir(credentials).unwrap().all(|entry| {
                    let name = entry.unwrap().file_name();
                    let name = name.to_string_lossy();
                    name == codewhale_config::LEGACY_XAI_OAUTH_FILE_NAME
                        || !codewhale_config::is_valid_xai_oauth_generation(&name)
                }),
                "failed activation must remove every unreferenced generation but retain legacy"
            );
        }
        assert!(
            !fs::read_to_string(config_path)
                .unwrap()
                .contains("must-be-cleaned")
        );
    }

    #[test]
    fn missing_file_message_mentions_oauth_paths() {
        let _guard = crate::test_support::lock_test_env();
        let msg = missing_auth_message(OAuthProvider::Xai);
        assert!(msg.contains("xAI OAuth credentials not found"), "{msg}");
        assert!(msg.contains("external-consent"), "{msg}");
        assert!(msg.contains("Codewhale-owned OAuth storage"), "{msg}");
        assert!(msg.contains("XAI_API_KEY"), "{msg}");
    }

    #[test]
    fn parse_rfc3339_accepts_zulu() {
        let ts = parse_rfc3339_secs("2026-07-09T12:00:00.000Z").expect("parse");
        assert!(ts > 0);
    }

    #[test]
    fn device_code_constants_match_discovery_shape() {
        assert_eq!(
            DEFAULT_SCOPES.split_whitespace().collect::<Vec<_>>(),
            [
                "openid",
                "profile",
                "email",
                "offline_access",
                "api:access",
                "grok-cli:access",
            ]
        );
        assert_eq!(XAI_OIDC_ISSUER, "https://auth.x.ai");
        assert_eq!(GROK_OIDC_CLIENT_ID.len(), 36);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn discovery_binds_to_advertised_endpoints() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/.well-known/openid-configuration"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "issuer": server.uri(),
                "device_authorization_endpoint": format!("{}/oauth2/device-advertised", server.uri()),
                "token_endpoint": format!("{}/oauth2/token-advertised", server.uri())
            })))
            .expect(1)
            .mount(&server)
            .await;

        let endpoints = tokio::task::block_in_place(|| {
            discover_oauth_endpoints(&XAI_OAUTH_PARAMS, &server.uri()).expect("discover endpoints")
        });

        assert_eq!(
            endpoints,
            OAuthEndpoints {
                device_authorization_endpoint: Some(format!(
                    "{}/oauth2/device-advertised",
                    server.uri()
                )),
                token_endpoint: format!("{}/oauth2/token-advertised", server.uri()),
            }
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn refresh_uses_discovered_token_endpoint() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/.well-known/openid-configuration"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "issuer": server.uri(),
                "device_authorization_endpoint": format!("{}/oauth2/device-advertised", server.uri()),
                "token_endpoint": format!("{}/oauth2/token-advertised", server.uri())
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/oauth2/token-advertised"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "refreshed-access",
                "refresh_token": "rotated-refresh",
                "expires_in": 3600
            })))
            .expect(1)
            .mount(&server)
            .await;

        let token = tokio::task::block_in_place(|| {
            refresh_for_provider(
                OAuthProvider::Xai,
                &ReqwestOAuthFormClient,
                &server.uri(),
                GROK_OIDC_CLIENT_ID,
                "refresh-secret",
            )
            .expect("refresh token")
        });

        assert_eq!(token.access_token.as_deref(), Some("refreshed-access"));
        assert_eq!(token.refresh_token.as_deref(), Some("rotated-refresh"));
    }

    #[test]
    fn https_discovery_rejects_plaintext_endpoint_downgrade() {
        let error = validate_discovered_oauth_endpoint(
            Some("http://auth.x.ai/oauth2/device/code".to_string()),
            "device_authorization_endpoint",
            XAI_OIDC_ISSUER,
        )
        .expect_err("HTTPS issuer must reject an HTTP endpoint");

        assert!(error.to_string().contains("downgrade"), "{error}");
    }

    #[test]
    fn https_discovery_accepts_same_origin_with_explicit_default_port() {
        let endpoint = "https://auth.x.ai:443/oauth2/token";
        let validated = validate_discovered_oauth_endpoint(
            Some(endpoint.to_string()),
            "token_endpoint",
            XAI_OIDC_ISSUER,
        )
        .expect("URL origins normalize the explicit default HTTPS port");

        assert_eq!(validated, endpoint);
    }

    #[test]
    fn https_discovery_rejects_cross_origin_endpoint() {
        let error = validate_discovered_oauth_endpoint(
            Some("https://oauth.attacker.example/oauth2/token".to_string()),
            "token_endpoint",
            XAI_OIDC_ISSUER,
        )
        .expect_err("discovered OAuth endpoints must stay on the issuer origin");

        assert!(error.to_string().contains("different origin"), "{error}");
    }

    #[test]
    fn discovery_rejects_mismatched_issuer() {
        let error = validate_discovered_issuer(
            Some("https://attacker.example".to_string()),
            XAI_OIDC_ISSUER,
        )
        .expect_err("discovery issuer must bind to the request issuer");

        assert!(error.to_string().contains("does not match"), "{error}");
    }

    #[test]
    fn oauth_error_details_collapse_control_whitespace() {
        let detail = oauth_failure_detail(
            Some("invalid_scope\nforged"),
            Some("bad\t scope\r\nnext line"),
            reqwest::StatusCode::BAD_REQUEST,
        );

        assert!(
            !detail
                .chars()
                .any(|character| matches!(character, '\n' | '\r' | '\t')),
            "{detail}"
        );
        assert!(detail.contains("invalid_scope forged"), "{detail}");
        assert!(detail.contains("bad scope next line"), "{detail}");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn discovery_failure_uses_documented_endpoint_fallback() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/.well-known/openid-configuration"))
            .respond_with(
                ResponseTemplate::new(503)
                    .set_body_raw("<html>temporarily unavailable</html>", "text/html"),
            )
            .expect(1)
            .mount(&server)
            .await;

        let endpoints = tokio::task::block_in_place(|| {
            resolve_oauth_endpoints(&XAI_OAUTH_PARAMS, &server.uri())
        });

        assert_eq!(
            endpoints,
            OAuthEndpoints {
                device_authorization_endpoint: Some(format!("{}/oauth2/device/code", server.uri())),
                token_endpoint: format!("{}/oauth2/token", server.uri()),
            }
        );
    }

    /// End-to-end regression for the v0.9.4 dogfood failure (#5032): starting
    /// from the exact state the dogfood machine was bricked in — a
    /// `providers.xai.oauth_credential_generation` pointer whose credential
    /// file no longer exists — the full device flow (discovery, device-code
    /// request, token poll, activation) must succeed and replace the stale
    /// pointer instead of dying with "xAI login was not activated; provider
    /// configuration is unchanged".
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn device_login_end_to_end_recovers_from_dangling_generation_pointer() {
        let _guard = crate::test_support::lock_test_env();
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/.well-known/openid-configuration"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "issuer": server.uri(),
                "device_authorization_endpoint": format!("{}/oauth2/device-advertised", server.uri()),
                "token_endpoint": format!("{}/oauth2/token-advertised", server.uri())
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/oauth2/device-advertised"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "device_code": "device-token",
                "user_code": "CW-TEST",
                "verification_uri": format!("{}/verify", server.uri()),
                "expires_in": 60,
                "interval": 1
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/oauth2/token-advertised"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "e2e-xai-access",
                "refresh_token": "e2e-xai-refresh",
                "expires_in": 3600,
                "token_type": "Bearer"
            })))
            .expect(1)
            .mount(&server)
            .await;

        let dir = TempDir::new().unwrap();
        let home = dir
            .path()
            .canonicalize()
            .expect("canonical temp root")
            .join("owned-home");
        let config_path = dir.path().join("config.toml");
        // The exact dogfood-machine state: valid-looking generation pointer,
        // missing credential file.
        let stale = "xai-auth-39a2f3e766ab47f89490002cd04fe187.json";
        fs::write(
            &config_path,
            format!(
                "[providers.xai]\nauth_mode = \"oauth\"\noauth_credential_generation = \"{stale}\"\n"
            ),
        )
        .unwrap();
        let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", &home);

        // The login half runs through the unified flow; only activation is
        // still legacy here (3b-ii unifies it).
        let inputs = crate::oauth::ResolvedOAuthInputs {
            issuer: server.uri(),
            client_id: GROK_OIDC_CLIENT_ID.to_string(),
            scopes: DEFAULT_SCOPES.to_string(),
            open_browser: false,
        };
        let unified = tokio::task::block_in_place(|| {
            crate::oauth::device_code_login_with(crate::oauth::OAuthProvider::Xai, &inputs)
        })
        .expect("device login against mock xAI");
        let pending = unified;
        let mut live = Config::default();
        let activation = activate_login(pending, Some(&config_path), Some(&mut live))
            .expect("dangling pointer must not brick activation");

        assert!(activation.auth_path.exists());
        let owned = fs::read_to_string(&activation.auth_path).unwrap();
        assert!(owned.contains("e2e-xai-access"));
        assert!(owned.contains("e2e-xai-refresh"));
        let persisted = fs::read_to_string(&config_path).unwrap();
        assert!(
            !persisted.contains(stale),
            "stale pointer must be replaced: {persisted}"
        );
        assert!(persisted.contains(activation.auth_path.file_name().unwrap().to_str().unwrap()));
        assert!(persisted.contains("auth_mode = \"oauth\""));
        assert!(
            credentials_valid(OAuthProvider::Xai, &live),
            "activated login must be usable"
        );
    }

    #[test]
    fn apply_token_response_sets_expiry_from_expires_in() {
        let mut entry = OwnedAuthEntry {
            access_token: None,
            refresh_token: None,
            expires_at: None,
            id_token: None,
            account_id: None,
            oidc_issuer: None,
            oidc_client_id: None,
            originator: None,
            auth_mode: None,
            extra: BTreeMap::new(),
        };
        let token = OAuthTokenMaterial {
            id_token: None,
            access_token: Some("fresh-access".to_string()),
            refresh_token: Some("fresh-refresh".to_string()),
            expires_in: Some(3600),
            error: None,
            error_description: None,
            interval: None,
        };
        let before = now_unix_secs().expect("clock");

        apply_token_response(
            OAuthProvider::Xai,
            &mut entry,
            XAI_OIDC_ISSUER,
            GROK_OIDC_CLIENT_ID,
            &token,
        )
        .expect("apply token");

        assert_eq!(entry.access_token.as_deref(), Some("fresh-access"));
        assert_eq!(entry.refresh_token.as_deref(), Some("fresh-refresh"));
        let expires_at = entry
            .expires_at
            .as_deref()
            .and_then(parse_rfc3339_secs)
            .expect("expires_at set from expires_in");
        let after = now_unix_secs().expect("clock");
        assert!(
            expires_at >= before + 3600,
            "{expires_at} < {before} + 3600"
        );
        assert!(expires_at <= after + 3600, "{expires_at} > {after} + 3600");
    }

    #[test]
    fn apply_token_response_rejects_missing_access_token() {
        let mut entry = OwnedAuthEntry {
            access_token: None,
            refresh_token: None,
            expires_at: None,
            id_token: None,
            account_id: None,
            oidc_issuer: None,
            oidc_client_id: None,
            originator: None,
            auth_mode: None,
            extra: BTreeMap::new(),
        };
        let token = OAuthTokenMaterial {
            id_token: None,
            access_token: None,
            refresh_token: None,
            expires_in: None,
            error: None,
            error_description: None,
            interval: None,
        };

        let error = apply_token_response(
            OAuthProvider::Xai,
            &mut entry,
            XAI_OIDC_ISSUER,
            GROK_OIDC_CLIENT_ID,
            &token,
        )
        .expect_err("missing access_token must fail");

        assert!(
            error.to_string().contains("missing access_token"),
            "{error}"
        );
    }

    struct MockTokenClient {
        responses: Mutex<Vec<(u16, String)>>,
        posts: Mutex<Vec<MockPost>>,
    }

    impl MockTokenClient {
        fn new(responses: Vec<(u16, String)>) -> Self {
            Self {
                responses: Mutex::new(responses),
                posts: Mutex::new(Vec::new()),
            }
        }
    }

    impl crate::oauth::OAuthFormClient for MockTokenClient {
        fn post_form(&self, url: &str, form: &[(&str, &str)]) -> Result<(u16, String)> {
            self.posts.lock().expect("posts").push((
                url.to_string(),
                form.iter()
                    .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                    .collect(),
            ));
            let mut responses = self.responses.lock().expect("responses");
            anyhow::ensure!(
                !responses.is_empty(),
                "mock issuer has no remaining responses"
            );
            Ok(responses.remove(0))
        }
    }

    fn jwt_with_exp(exp: u64) -> String {
        let payload = URL_SAFE_NO_PAD.encode(format!(r#"{{"exp":{exp}}}"#));
        format!("header.{payload}.sig")
    }

    #[test]
    fn store_persist_refresh_and_revoke_use_mock_issuer() {
        let _lock = crate::test_support::lock_test_env();
        let home = tempfile::tempdir().expect("temp home");
        let root = home.path().canonicalize().expect("canonical home");
        let _home = crate::test_support::EnvVarGuard::set("CODEWHALE_HOME", &root);
        let config_path = root.join("config.toml");
        std::fs::write(&config_path, "").expect("empty config");

        let pending = pending_login_with_id_token_for_test(
            OAuthProvider::Chatgpt,
            "access-1",
            "refresh-1",
            Some(&jwt_with_account("acct-7")),
        );
        let activation = activate_login(pending, Some(&config_path), None).expect("activate");
        assert!(activation.auth_path.exists());
        let persisted = std::fs::read_to_string(root.join("config.toml")).expect("config");
        assert!(persisted.contains("chatgpt-auth-"));
        assert!(persisted.contains("auth_mode = \"oauth\""));
        assert!(!persisted.contains("access-1"), "{persisted}");

        let generation = toml::from_str::<toml::Value>(&persisted)
            .unwrap()["providers"]["openai_codex"]["oauth_credential_generation"]
            .as_str()
            .unwrap()
            .to_string();
        let mut config = Config {
            provider: Some(ApiProvider::OpenaiCodex.as_str().to_string()),
            ..Config::default()
        };
        config.mark_codewhale_owned_chatgpt_oauth(generation.clone());
        assert!(credentials_valid(OAuthProvider::Chatgpt, &config));

        let stale = jwt_with_exp(1_000_000_000);
        let scope = format!("{CHATGPT_OAUTH_ISSUER}::{CHATGPT_OAUTH_CLIENT_ID}");
        let raw = serde_json::json!({
            &scope: {
                "access_token": stale,
                "refresh_token": "refresh-old",
                // Conflicting metadata must not suppress the existing refresh.
                "expires_at": rfc3339_from_now(3600),
                "oidc_issuer": CHATGPT_OAUTH_ISSUER,
                "oidc_client_id": CHATGPT_OAUTH_CLIENT_ID,
                "originator": CHATGPT_OAUTH_ORIGINATOR
            }
        });
        codewhale_config::with_xai_oauth_lifecycle_lock(|store| {
            store.write(
                &generation,
                serde_json::to_vec_pretty(&raw).unwrap().as_slice(),
                true,
            )
        })
        .unwrap();

        let mock = MockTokenClient::new(vec![(
            200,
            serde_json::json!({
                "access_token": "access-2",
                "refresh_token": "refresh-2",
                "expires_in": 3600
            })
            .to_string(),
        )]);
        let refreshed =
            get_owned_credentials_with(OAuthProvider::Chatgpt, &config, &mock).expect("refresh");
        assert_eq!(refreshed.access_token, "access-2");
        let stored = codewhale_config::with_xai_oauth_lifecycle_lock(|store| {
            store.read_to_string(&generation)
        })
        .unwrap()
        .unwrap();
        assert!(stored.contains("refresh-2"), "{stored}");
        assert!(!stored.contains("refresh-old"), "{stored}");

        let revoke_mock = MockTokenClient::new(vec![(200, String::new())]);
        codewhale_config::with_xai_oauth_lifecycle_lock(|store| {
            revoke_owned_login_locked_with(
                OAuthProvider::Chatgpt,
                Some(&config_path),
                None,
                store,
                &revoke_mock,
            )
        })
        .expect("revoke");
        let after = std::fs::read_to_string(&config_path).expect("config after revoke");
        assert!(!after.contains("chatgpt-auth-"), "{after}");
        let posts = revoke_mock.posts.lock().unwrap();
        assert!(
            posts.iter().any(|(url, _)| url.contains("/oauth/revoke")),
            "{posts:?}"
        );
    }

    #[test]
    fn debug_impls_redact_secrets() {
        let entry = OwnedAuthEntry {
            access_token: Some("secret-access".into()),
            refresh_token: Some("secret-refresh".into()),
            expires_at: None,
            id_token: Some("secret-id".into()),
            account_id: None,
            oidc_issuer: None,
            oidc_client_id: None,
            originator: None,
            auth_mode: None,
            extra: BTreeMap::new(),
        };
        let rendered = format!("{entry:?}");
        assert!(rendered.contains("<redacted>"));
        assert!(!rendered.contains("secret-access"));
        assert!(!rendered.contains("secret-refresh"));
        assert!(!rendered.contains("secret-id"));

        let activation = OAuthActivation {
            credentials: OwnedOAuthCredentials {
                access_token: "secret-access".into(),
                account_id: None,
                refresh_token: Some("secret-refresh".into()),
                expires_at: None,
                issuer: "issuer".into(),
                client_id: "client".into(),
            },
            config_path: PathBuf::from("/tmp/config.toml"),
            auth_path: PathBuf::from("/tmp/auth.json"),
        };
        let rendered = format!("{activation:?}");
        assert!(rendered.contains("<redacted>"));
        assert!(!rendered.contains("secret-access"));
    }
}
