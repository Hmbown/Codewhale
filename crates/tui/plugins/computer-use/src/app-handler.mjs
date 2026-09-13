// One request handler for every out-of-process runner of the backends: the
// ssh remote agent (one request per process) and the desktop app daemon (one
// long-lived process). Only tools in ALLOWED execute, so neither the ssh
// transport nor the app socket can ever become a generic shell.
import url from "node:url";
import { exec } from "./remote-runtime.mjs";
import { withSignal, throwIfAborted } from "./exec.mjs";

export const ALLOWED = new Set([
  "preview", "platform", "probe", "list_displays", "switch_display", "list_apps", "list_windows",
  "open_application", "get_app_state", "resolve_element", "screenshot", "zoom",
  "left_click", "double_click", "triple_click", "right_click", "middle_click",
  "mouse_move", "left_click_drag", "left_mouse_down", "left_mouse_up", "scroll",
  "type", "key", "hold_key", "set_value", "select_text", "perform_action",
  "read_clipboard", "write_clipboard", "cursor_position",
  "recordingStart", "recordingStop", "recordingStatus", "recordingList",
]);

const backends = new Map();
const heldPointers = new Map();
const INPUT_MUTATIONS = new Set([
  "open_application", "left_click", "double_click", "triple_click", "right_click", "middle_click", "mouse_move",
  "left_click_drag", "left_mouse_down", "left_mouse_up", "scroll", "type", "key", "hold_key", "set_value", "select_text", "perform_action",
]);
let queue = Promise.resolve();

async function backend(computerId, sessionId, persistentInputOwner) {
  const key = `${computerId}:${sessionId}`;
  if (!backends.has(key)) {
    // Same test hook as src/transport.mjs, so the out-of-process route can be
    // driven end to end against a recording backend (never set in production).
    const test = process.env.CODEWHALE_CU_TEST_BACKEND;
    const mod = await import(test ? url.pathToFileURL(test).href : `./backends/${process.platform}.mjs`);
    backends.set(key, mod.create({ exec: { ...exec, persistentInputOwner }, computer: { id: computerId, transport: "local", platform: process.platform } }));
  }
  return backends.get(key);
}

const sessions = new Map();
let controlMode = "ready";
let controlGeneration = 0;
let cleanupPending = false;

/** Human-facing state contains app identity and action names, never task text. */
export function controlStatus() {
  return { mode: controlMode, cleanupPending, sessions: [...sessions.values()]
    .filter((s) => !s.closed && s.target)
    .map((s) => ({ target: s.target, mode: s.mode, action: s.action ?? null })) };
}

// Called only by the launcher's inherited control channel, never an MCP tool.
// Abort before queuing cleanup so even a held gesture yields to the person.
export async function setControlMode(mode) {
  if (!["ready", "paused", "stopped"].includes(mode)) throw new Error("Unknown control mode");
  if (mode === "ready") {
    if (cleanupPending) throw new Error("Input is still being released; try again in a moment.");
    controlMode = mode;
    return controlStatus();
  }
  controlMode = mode;
  const generation = ++controlGeneration;
  cleanupPending = true;
  const results = await Promise.allSettled([...sessions.keys()].map((key) => {
    const colon = key.indexOf(":");
    return releaseSessionInput(key.slice(colon + 1), key.slice(0, colon), { close: mode === "stopped" });
  }));
  const failure = results.find((r) => r.status === "rejected");
  // A cleanup failure stays blocked. Resume cannot hide owned input.
  if (failure) throw failure.reason;
  if (generation === controlGeneration) cleanupPending = false;
  return controlStatus();
}

function enqueue(fn) {
  const next = queue.then(fn);
  queue = next.catch(() => {});
  return next;
}

/** Cancel this host's work, then release only input held by its backend. */
export function releaseSessionInput(sessionId, computerId = "local", { close = false } = {}) {
  const key = `${computerId}:${sessionId}`;
  let session = sessions.get(key);
  if (!session) {
    if (!close) return Promise.resolve();
    session = { requests: new Set(), closed: true, touched: Date.now() };
    sessions.set(key, session);
  }
  if (close) session.closed = true;
  for (const controller of session.requests) controller.abort();
  return enqueue(async () => {
    try {
      await withSignal(null, () => backends.get(key)?.releaseInput?.());
      if (heldPointers.get(computerId) === key) heldPointers.delete(computerId);
      if (close) await withSignal(null, () => backends.get(key)?.closeSession?.());
      if (close) backends.delete(key);
    } finally { session.touched = Date.now(); }
  });
}

export function closeSession(sessionId, computerId = "local") {
  return releaseSessionInput(sessionId, computerId, { close: true });
}

/**
 * A freshly granted session owner supersedes a closed-session tombstone:
 * without this, a daemon that re-leases a session id (the old owner socket
 * died with the previous daemon) would keep aborting every request on it.
 * In-flight requests from the dead owner stay aborted; the tombstone is the
 * only thing removed.
 */
export function reopenSession(sessionId, computerId = "local") {
  sessions.delete(`${computerId}:${sessionId}`);
}

export function closeAllSessions() {
  return Promise.allSettled([...sessions.keys()].map((key) => {
    const colon = key.indexOf(":");
    return closeSession(key.slice(colon + 1), key.slice(0, colon));
  }));
}

/**
 * Execute one {tool, args} request on this machine's backend. Never throws:
 * every outcome is a receipt object with `ok`.
 */
export async function handle(req, { computerId = "local", sessionId = "direct", signal, persistentInputOwner = false } = {}) {
  const tool = req?.tool;
  if (!ALLOWED.has(tool)) {
    return { ok: false, error: { code: "tool_not_allowed", message: `tool "${tool}" is not in the remote allow-list` } };
  }
  if (tool === "platform") return { ok: true, platform: process.platform };
  if (controlMode !== "ready") return { ok: false, error: { code: `control_${controlMode}`, message: `Computer Use is ${controlMode} by the user. Wait for them to resume it in the menu bar.` } };
  const generation = controlGeneration;
  const key = `${computerId}:${sessionId}`;
  let session = sessions.get(key);
  if (!session) {
    // Retain closed-session tombstones briefly, and bound abandoned sessions
    // when a host is killed without a graceful MCP disconnect.
    for (const [id, old] of sessions) {
      if (old.requests.size || Date.now() - old.touched <= (old.closed ? 300_000 : 3_600_000)) continue;
      if (backends.has(id)) {
        const colon = id.indexOf(":");
        try { await closeSession(id.slice(colon + 1), id.slice(0, colon)); }
        catch (err) { return { ok: false, error: { code: "input_release_failed", message: String(err?.message ?? err) } }; }
      } else sessions.delete(id);
    }
    if ([...sessions.values()].filter((entry) => !entry.closed).length >= 256) return { ok: false, error: { code: "session_limit", message: "Too many active computer sessions; close unused hosts or restart the helper." } };
    session = { requests: new Set(), closed: false, touched: Date.now() };
    sessions.set(key, session);
  }
  const controller = new AbortController();
  const abort = () => controller.abort();
  signal?.addEventListener("abort", abort, { once: true });
  if (signal?.aborted || session.closed) controller.abort();
  session.requests.add(controller);
  // One desktop can execute only one input gesture at a time. The queue spans
  // sockets and sessions; a disconnected/cancelled request is checked again
  // when its slot arrives, before it can post any input.
  try {
    return await enqueue(() => withSignal(controller.signal, async () => {
      throwIfAborted();
      if (generation !== controlGeneration || controlMode !== "ready") throw Object.assign(new Error("Computer control was interrupted by the user."), { code: "cancelled" });
      const instance = await backend(computerId, sessionId, persistentInputOwner);
      throwIfAborted();
      session.action = tool;
      if (tool === "open_application") { session.target = null; session.mode = null; }
      if (INPUT_MUTATIONS.has(tool) && heldPointers.has(computerId) && heldPointers.get(computerId) !== key) {
        return { ok: false, error: { code: "input_busy", message: "Another computer session owns a held pointer; release it or close that session before sending input." } };
      }
      const fn = instance[tool];
      if (typeof fn !== "function") {
        return { ok: false, error: { code: "unsupported_on_platform", message: `"${tool}" is not implemented on ${process.platform}` } };
      }
      // Preserve ownership across calls, not just during the serialized
      // request. Another host's click/up must not release this host's press.
      if (tool === "left_mouse_down") heldPointers.set(computerId, key);
      let data;
      try { data = await fn(req.args ?? {}); throwIfAborted(); }
      catch (error) {
        if (["left_mouse_down", "left_mouse_up", "mouse_move"].includes(tool) && heldPointers.get(computerId) === key) {
          await withSignal(null, () => instance.releaseInput?.());
          if (heldPointers.get(computerId) === key) heldPointers.delete(computerId);
        }
        throw error;
      }
      if (tool === "left_mouse_up" && heldPointers.get(computerId) === key) heldPointers.delete(computerId);
      if (tool === "open_application" && data?.resolved) {
        session.target = { name: String(data.resolved.name ?? "Application").slice(0, 128), pid: data.resolved.pid };
        session.mode = data.shared_pointer || data.activate ? "foreground" : "background";
      }
      return { ok: true, platform: process.platform, tool, data };
    }));
  } catch (err) {
    return { ok: false, platform: process.platform, tool, error: { code: err?.code ?? "tool_error", message: String(err?.message ?? err) } };
  } finally {
    signal?.removeEventListener("abort", abort);
    session.requests.delete(controller);
    session.action = null;
    session.touched = Date.now();
  }
}
