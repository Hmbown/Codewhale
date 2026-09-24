import type { DocsConfigurationDict } from "../types";

export const docsConfiguration: DocsConfigurationDict = {
  metaTitle: "Configuration · Codewhale Docs",
  metaDescription:
    "Where config.toml is read from, the per-project overlay, credential precedence, and legacy path migration.",
  bodyClassName: "text-ink-soft leading-relaxed",
  overviewTitle: "Configuration",
  overviewLead:
    "Codewhale reads its configuration from ~/.codewhale/config.toml (the legacy ~/.deepseek/config.toml is still read as a fallback). The --config flag and the CODEWHALE_CONFIG_PATH environment variable can point elsewhere; --config wins when both are set, and environment variable overrides are applied after the file is loaded.",
  auditLead:
    "Inside the TUI, {auditCommand} shows which documented keys can change in the current session, which can also be persisted, and which stay file-only or restart-only — treat its “Command / reason” column as the source of truth before editing by hand.",
  overlayTitle: "Per-project overlay",
  overlayLead:
    "When a workspace contains a regular-file <workspace>/.codewhale/config.toml, the safe values it declares are merged on top of the global config (legacy <workspace>/.deepseek/config.toml files are still read when the Codewhale path is absent; symlinked project configs are rejected). This lets a repository suggest a model or tighten the local safety posture without touching the user's global config. Pass --no-project-config to skip the overlay for one launch.",
  overlayLimits:
    "The overlay is intentionally narrow: it supports model, reasoning_effort, approval_policy and sandbox_mode (tightening values only), notes_path, max_subagents (clamped to 1..=128), and allow_shell (false applies, true is ignored). Credentials, endpoints, provider selection, MCP config, hooks, skills, and instructions = [...] stay user-global — a repo-local config.toml that declares api_key, base_url, or provider is ignored, so a cloned repository cannot pick arbitrary local files into the prompt.",
  credentialsTitle: "Credential lookup",
  credentialsLead:
    "For the active provider, the API key resolves in this order, first match wins: the route's own auth contract (OAuth routes use their consented token; auth_mode = \"none\" sends no key), then an explicit {apiKey}, then the config file api_key, then an api_key_env binding, then the secret store written by codewhale auth set (a file under ~/.codewhale/secrets/ by default; the OS keyring only when CODEWHALE_SECRET_BACKEND=system), then the provider's own environment variable, which is only sent to that provider's official endpoint. {authStatus} inspects the active provider's config file, secret-store backend, environment variable, winning source, and last-four label without printing the key itself. Hosted, generic OpenAI-compatible, self-hosted, or native Anthropic routes are selected with {providerConfig} or {providerFlag}; the full registry lives on the Models & providers page and in docs/PROVIDERS.md.",
  legacyTitle: "Legacy .deepseek/ paths",
  legacyLead:
    "Codewhale was renamed from DeepSeek-TUI. To avoid breaking existing installs, the runtime reads state from the new ~/.codewhale/ location but falls back to ~/.deepseek/ when only the legacy directory exists, and always writes to ~/.codewhale/ — read-with-fallback, write-to-new. State-dir resolution is consolidated in resolve_state_dir / ensure_state_dir in crates/config/src/lib.rs, and every legacy path reference carries an audited keep decision.",
  sourceNote:
    "Source documents: docs/CONFIGURATION.md, docs/LEGACY_PATHS.md · Update docs-map.ts when changing.",
};
