/**
 * Chromewhale's trust boundary: which page a browser tool may touch, and which
 * field it may never type into.
 *
 * Deliberately pure — no `chrome.*`, no DOM — so `node --test` exercises the
 * guards directly instead of through a browser harness. Everything that decides
 * "may this act on that page" lives here; `browser.js` only routes.
 *
 * Known limitations, stated where the behaviour is owned:
 *
 * - Runtime dynamic tools are registered with `ApprovalRequirement::Auto`
 *   (`crates/tui/src/tools/dynamic.rs`), so Codewhale's own approval gate never
 *   sees a browser tool call. This module plus Chrome's optional host
 *   permissions are the only gate. Do not weaken one assuming the other.
 * - A decision is per **origin**, matching Chrome's own permission granularity.
 *   There is no per-path or per-action grant, and none is planned: a grant that
 *   is finer than the platform's would be a claim the platform cannot keep.
 * - The sensitive-field list below is a floor, not a promise that every secret
 *   input is recognised. It refuses the fields whose markup says what they are.
 */

/** Schemes a browser tool must never touch, whatever the user granted. */
const BLOCKED_SCHEMES = new Set([
  "chrome:",
  "chrome-extension:",
  "chrome-untrusted:",
  "chrome-search:",
  "devtools:",
  "edge:",
  "about:",
  "view-source:",
  "file:",
  "data:",
  "blob:",
  "javascript:",
  "filesystem:",
  "ws:",
  "wss:",
]);

/**
 * Hosts Chrome itself refuses to let extensions script. Listed so the model
 * gets a sentence it can act on instead of an opaque injection failure.
 */
const BLOCKED_HOSTS = new Set([
  "chromewebstore.google.com",
  "chrome.google.com",
  "microsoftedge.microsoft.com",
]);

/**
 * `autocomplete` tokens and input types that name a secret. Typing into these
 * is refused outright: the model has no business filling a password, a
 * one-time code, or a card number, and a user who wants that typed can type it.
 */
const SENSITIVE_INPUT_TYPES = new Set(["password"]);
const SENSITIVE_AUTOCOMPLETE = [
  "current-password",
  "new-password",
  "one-time-code",
  "cc-number",
  "cc-csc",
  "cc-exp",
  "cc-exp-month",
  "cc-exp-year",
  "cc-name",
];

/**
 * Classify a tab URL (or a navigation target) for tool use.
 *
 * @param {unknown} rawUrl
 * @returns {{ok: true, origin: string, pattern: string, host: string}
 *          | {ok: false, reason: string}}
 */
export function classifyTarget(rawUrl) {
  if (typeof rawUrl !== "string" || rawUrl.trim() === "") {
    return { ok: false, reason: "No page is open in the active tab." };
  }
  let url;
  try {
    url = new URL(rawUrl);
  } catch {
    return { ok: false, reason: `Not a usable page address: ${truncate(rawUrl, 120)}` };
  }
  if (BLOCKED_SCHEMES.has(url.protocol)) {
    return {
      ok: false,
      reason: `Chromewhale never acts on ${url.protocol} pages. Open an http(s) page first.`,
    };
  }
  if (url.protocol !== "http:" && url.protocol !== "https:") {
    return {
      ok: false,
      reason: `Chromewhale only acts on http and https pages, not ${url.protocol}.`,
    };
  }
  if (BLOCKED_HOSTS.has(url.hostname)) {
    return {
      ok: false,
      reason: `Chrome blocks extensions on ${url.hostname}, so Chromewhale cannot act there.`,
    };
  }
  return {
    ok: true,
    origin: url.origin,
    host: url.hostname,
    pattern: `${url.origin}/*`,
  };
}

/**
 * Read the stored decision for an origin.
 *
 * @param {Record<string, unknown> | undefined | null} store
 * @param {string} origin
 * @returns {"allow" | "block" | "ask"}
 */
export function decisionFor(store, origin) {
  const value = store && typeof store === "object" ? store[origin] : undefined;
  return value === "allow" || value === "block" ? value : "ask";
}

/**
 * Apply a decision to the stored map, returning a new map.
 *
 * `"ask"` removes the entry, which is how the panel's "Forget" control works:
 * an origin with no entry is asked about again on the next call.
 *
 * @param {Record<string, unknown> | undefined | null} store
 * @param {string} origin
 * @param {"allow" | "block" | "ask"} decision
 * @returns {Record<string, "allow" | "block">}
 */
export function withDecision(store, origin, decision) {
  /** @type {Record<string, "allow" | "block">} */
  const next = {};
  for (const [key, value] of Object.entries(store && typeof store === "object" ? store : {})) {
    if (key !== origin && (value === "allow" || value === "block")) {
      next[key] = value;
    }
  }
  if (decision === "allow" || decision === "block") {
    next[origin] = decision;
  }
  return next;
}

/**
 * Is this form field one Chromewhale must refuse to type into?
 *
 * @param {{tag?: unknown, type?: unknown, autocomplete?: unknown, name?: unknown,
 *          id?: unknown, ariaLabel?: unknown}} field
 * @returns {{sensitive: true, reason: string} | {sensitive: false}}
 */
export function sensitiveField(field) {
  const descriptor = field && typeof field === "object" ? field : {};
  const type = lower(descriptor.type);
  if (SENSITIVE_INPUT_TYPES.has(type)) {
    return {
      sensitive: true,
      reason: "this is a password field; Chromewhale never types into one",
    };
  }
  const autocomplete = lower(descriptor.autocomplete);
  for (const token of autocomplete.split(/\s+/)) {
    if (SENSITIVE_AUTOCOMPLETE.includes(token)) {
      return {
        sensitive: true,
        reason: `the field is marked autocomplete="${token}"; Chromewhale never types credentials or card details`,
      };
    }
  }
  // Not anchored on word boundaries: real markup names these fields
  // `user_password`, `login[passwd]`, `card-number`. Erring toward refusal on a
  // field merely *named* like a secret is the safe direction — the user can
  // always type it themselves.
  const named = `${lower(descriptor.name)} ${lower(descriptor.id)} ${lower(descriptor.ariaLabel)}`;
  if (/password|passwd|passcode|\botp\b|\bcvv\b|\bcvc\b|card[-_ ]?number/.test(named)) {
    return {
      sensitive: true,
      reason: "the field names itself a password, one-time code, or card number",
    };
  }
  return { sensitive: false };
}

/** @param {unknown} value */
function lower(value) {
  return typeof value === "string" ? value.trim().toLowerCase() : "";
}

/**
 * @param {string} value
 * @param {number} max
 */
function truncate(value, max) {
  return value.length <= max ? value : `${value.slice(0, max)}…`;
}
