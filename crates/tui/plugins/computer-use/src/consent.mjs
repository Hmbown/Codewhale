// Per-app consent ledger: which applications this host may drive on each
// computer, and whether it may take the shared pointer/foreground at all.
//
// The model is the one the on-computer agent products converged on: the app,
// not the tool, is the unit of trust. The first call that targets an app —
// binding input to it, observing it by name, or acting through a bound or
// element target — refuses with consent_required until a decision exists.
// Decisions are "allow" or "deny"; a bare grant lives for this server session
// (the host asks again next task), remember:true persists it to consent.json.
//
// Two scopes:
//   apps        — keyed by resolved identity (bundle id, name, pid)
//   foreground  — darwin activate:true, the shared-desktop escalation
//
// This is a model-level ledger, not an OS sandbox: a determined agent with
// another channel could still reach an app. What it buys is the honest part —
// no accidental touches, every first contact visible, and a deny the host can
// actually enforce on this surface.
import fs from "node:fs";
import path from "node:path";
import { stateDir } from "./registry.mjs";

const ID_RE = /^[a-zA-Z0-9][a-zA-Z0-9._-]{0,63}$/;
const consentPath = () => path.join(stateDir(), "consent.json");

/** computerId -> Map(key -> {decision, name?, at}) — session-scoped, dies with the server. */
const session = new Map();
function sessionMap(computerId) {
  let m = session.get(computerId);
  if (!m) { m = new Map(); session.set(computerId, m); }
  return m;
}

function load() {
  try {
    const raw = JSON.parse(fs.readFileSync(consentPath(), "utf8"));
    if (!raw || typeof raw !== "object" || !raw.computers) throw new Error("bad shape");
    return raw;
  } catch {
    return { version: 1, computers: {} };
  }
}

function save(data) {
  fs.mkdirSync(stateDir(), { recursive: true });
  const tmp = consentPath() + ".tmp";
  fs.writeFileSync(tmp, JSON.stringify(data, null, 2) + "\n");
  fs.renameSync(tmp, consentPath());
}

/**
 * Identity keys for an app reference. name/bundle match the native resolver's
 * case-insensitive compare, so keys normalize lowercase. pid entries are
 * session-only — a pid never outlives its process, so persisting one would
 * grant (or deny) whichever app inherits the number next.
 */
export function appKeys(ref) {
  const keys = [];
  if (!ref || typeof ref !== "object") return keys;
  if (typeof ref.bundle_id === "string" && ref.bundle_id.trim()) keys.push(`bundle:${ref.bundle_id.trim().toLowerCase()}`);
  // A ".app" suffix is a filesystem spelling, not part of the name — the
  // native resolver strips it, and so must the ledger.
  if (typeof ref.name === "string" && ref.name.trim()) keys.push(`name:${ref.name.trim().replace(/\.app$/i, "").toLowerCase()}`);
  if (Number.isInteger(ref.pid) && ref.pid > 0) keys.push(`pid:${ref.pid}`);
  return keys;
}

/** Split a caller-supplied app string into identity keys. */
export function parseAppArg({ app, name, bundle_id, pid } = {}) {
  const ref = {};
  if (typeof bundle_id === "string" && bundle_id.trim()) ref.bundle_id = bundle_id.trim();
  if (typeof name === "string" && name.trim()) ref.name = name.trim();
  if (Number.isInteger(pid) && pid > 0) ref.pid = pid;
  if (typeof app === "string" && app.trim() && !Object.keys(ref).length) {
    const s = app.trim();
    if (/^pid:\d+$/i.test(s)) ref.pid = Number(s.slice(4));
    else if (/^\d+$/.test(s)) ref.pid = Number(s);
    // ".app" is a filename spelling and always means a name — it must be
    // checked before the reverse-DNS shape, which it also satisfies.
    else if (/\.app$/i.test(s)) ref.name = s.replace(/\.app$/i, "");
    // Reverse-DNS shape (com.foo.bar, no spaces) reads as a bundle id.
    else if (/^[A-Za-z0-9-]+(\.[A-Za-z0-9-]+)+$/.test(s) && !s.includes(" ")) ref.bundle_id = s;
    else ref.name = s;
  }
  return appKeys(ref);
}

function persistedEntries(computerId) {
  return load().computers[computerId]?.apps ?? {};
}

/**
 * The newest decision among matching keys wins — session entries are checked
 * alongside persisted ones, so "always deny" followed by "allow once" yields
 * allow, and "always allow" followed by a session deny yields deny. Returns
 * {state: allowed|denied|undecided, via, persisted}.
 */
export function decisionFor(computerId, keys) {
  let best = null;
  const consider = (entry, key, persisted) => {
    if (!entry || (entry.decision !== "allow" && entry.decision !== "deny")) return;
    // A session entry may itself be backed by disk (remember:true writes
    // both layers and marks the session copy) — durability is honest, not
    // just "which map won".
    const durable = persisted || entry.persisted === true;
    if (!best || String(entry.at ?? "") > String(best.at ?? "")) best = { ...entry, via: key, persisted: durable };
  };
  const sm = session.get(computerId);
  for (const key of keys) {
    consider(sm?.get(key), key, false);
    consider(persistedEntries(computerId)[key], key, true);
  }
  if (!best) return { state: "undecided" };
  return { state: best.decision === "deny" ? "denied" : "allowed", via: best.via, persisted: best.persisted, name: best.name ?? null };
}

/** Record a decision under every given key. Session-only unless remember. */
export function record(computerId, keys, decision, { remember = false, name = null } = {}) {
  if (!ID_RE.test(computerId)) throw Object.assign(new Error(`bad computer id "${computerId}"`), { code: "bad_args" });
  if (decision !== "allow" && decision !== "deny") throw Object.assign(new Error(`decision must be "allow" or "deny"`), { code: "bad_args" });
  const at = new Date().toISOString();
  const entry = { decision, at, ...(name ? { name } : {}) };
  // pid keys never persist — see appKeys.
  const persistable = keys.filter((k) => !k.startsWith("pid:"));
  if (remember && persistable.length) {
    const data = load();
    const apps = (data.computers[computerId] ??= { apps: {} }).apps ??= {};
    for (const key of persistable) apps[key] = { ...entry };
    save(data);
  }
  const sm = sessionMap(computerId);
  const backed = new Set(remember ? persistable : []);
  for (const key of keys) sm.set(key, { ...entry, ...(backed.has(key) ? { persisted: true } : {}) });
  return { keys, decision, persisted: remember && persistable.length > 0 };
}

/**
 * Copy an already-made decision onto a resolved identity's other keys — an
 * allow for "Safari" also covers bundle:com.apple.safari once open_application
 * resolves it, so the next request under a different spelling does not
 * re-prompt. Aliases land at the same layer (session or persisted) as the
 * decision they extend.
 */
export function alias(computerId, keys, { persisted = false, name = null } = {}) {
  const at = new Date().toISOString();
  const sm = sessionMap(computerId);
  const entry = { decision: "allow", at, ...(name ? { name } : {}) };
  let durableKeys = new Set();
  if (persisted) {
    const data = load();
    const apps = (data.computers[computerId] ??= { apps: {} }).apps ??= {};
    for (const key of keys.filter((k) => !k.startsWith("pid:"))) apps[key] ??= { ...entry };
    save(data);
    // A key already persisted as deny stays deny on disk — the marker only
    // goes on keys whose disk entry is actually this allow.
    durableKeys = new Set(keys.filter((k) => apps[k]?.decision === "allow"));
  }
  for (const key of keys) sm.set(key, { ...entry, ...(durableKeys.has(key) ? { persisted: true } : {}) });
}

/** Remove every trace of the given keys (and foreground when asked) at both layers. */
export function revoke(computerId, keys) {
  const sm = session.get(computerId);
  let removed = 0;
  for (const key of keys) if (sm?.delete(key)) removed++;
  const data = load();
  const apps = data.computers[computerId]?.apps;
  if (apps) {
    for (const key of keys) if (delete apps[key]) removed++;
    save(data);
  }
  return { removed };
}

export function foregroundDecision(computerId) {
  const s = session.get(computerId)?.get("scope:foreground");
  const p = load().computers[computerId]?.foreground;
  const pick = [s && { ...s, persisted: s.persisted === true }, p && { ...p, persisted: true }]
    .filter(Boolean)
    .sort((a, b) => String(b.at).localeCompare(String(a.at)))[0];
  if (!pick || (pick.decision !== "allow" && pick.decision !== "deny")) return { state: "undecided" };
  return { state: pick.decision === "deny" ? "denied" : "allowed", persisted: pick.persisted };
}

export function recordForeground(computerId, decision, { remember = false } = {}) {
  const at = new Date().toISOString();
  sessionMap(computerId).set("scope:foreground", { decision, at, ...(remember ? { persisted: true } : {}) });
  if (remember) {
    const data = load();
    (data.computers[computerId] ??= {}).foreground = { decision, at };
    save(data);
  }
  return { scope: "foreground", decision, persisted: remember };
}

export function revokeForeground(computerId) {
  const sm = session.get(computerId);
  let removed = sm?.delete("scope:foreground") ? 1 : 0;
  const data = load();
  if (data.computers[computerId]?.foreground) { delete data.computers[computerId].foreground; removed++; save(data); }
  return { removed };
}

/** Merged view for consent status: persisted entries overlaid with this session's. */
export function status(computerId) {
  const apps = {};
  for (const [key, entry] of Object.entries(persistedEntries(computerId))) apps[key] = { ...entry, source: "persisted" };
  for (const [key, entry] of session.get(computerId) ?? []) {
    if (key === "scope:foreground") continue;
    apps[key] = { ...entry, source: entry.persisted === true ? "persisted" : "session" };
  }
  const fg = foregroundDecision(computerId);
  return { computer: computerId, apps, foreground: fg.state === "undecided" ? null : fg };
}

/** Drop this session's grants for a computer whose route went away. */
export function dropSession(computerId) {
  session.delete(computerId);
}
