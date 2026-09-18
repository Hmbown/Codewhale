#!/usr/bin/env node
// codewhale-cu MCP server — zero-dependency JSON-RPC 2.0 over stdio.
// One tool surface, four platforms (darwin, win32, linux, harmonyos), with
// computer switching as a default: every tool accepts `computer`, and using a
// computer id switches the sticky active computer.
import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";
import * as registry from "../src/registry.mjs";
import { backendFor, installRemoteAgent, executorFor, closeAppSession, routeFingerprint, closeSshChannel } from "../src/transport.mjs";
import { TOOLS, TOOL_NAMES, REQUIRED_ARGS, ELEMENT_ONLY_TARGET, READ_ONLY_TOOLS, REMOTE_TOOLS, BACKEND_METHOD, resolveTool, parseGrant, MERGED_EXPANSION } from "../src/tools.mjs";
import { tryJson, withSignal, throwIfAborted, wait } from "../src/exec.mjs";
import { APP_VERSION } from "../src/app-socket.mjs";
import { createRecorder, readTrajectory, listTrajectories, resolveTrajectory, isTrajectoryTool } from "../src/trajectory.mjs";

const SERVER_NAME = "codewhale-cu";

// ---------- per-session runtime state ----------
let controlStopped = false;
// Registered computers are shared; the selected destination belongs to this
// MCP host. Another task must never redirect an implicit input action.
let activeComputerId = "local";
let stateCounter = 0;
let inFlight = 0; // actions currently dispatching to a backend/executor
/** request ids cancelled via notifications/cancelled */
const cancelled = new Set();
const requests = new Map();
let dispatch = Promise.resolve();
const recorder = createRecorder();
let replaying = false;
// Fixed at process start; nothing can widen it. See parseGrant for the form.
const GRANT = parseGrant(process.env.CODEWHALE_CU_GRANT);
/** The active capability grant, as `request_access` reports it on success or refusal. */
const grantReport = () => (GRANT ? { mode: "narrowed", tools: [...GRANT].sort(), count: GRANT.size, note: "This session's tools were narrowed at launch (CODEWHALE_CU_GRANT); do not work around it." } : null);
/** state_id -> { computerId, app_ref, windowIndex, elements } */
const appStates = new Map();
/** computerId -> state_id of its most recent observation */
const latestStateByComputer = new Map();
/** computerId -> last raster metadata {file, scale, origin} */
const lastRasters = new Map();
/** computerId -> route-bound session resources; the registry owns configuration. */
const backendCache = new Map();
const ROUTE_INSPECTION_TOOLS = new Set([
  "request_access", "list_displays", "list_apps", "list_windows", "get_app_state", "screenshot",
  "cursor_position", "read_clipboard", "recording_list", "recording_status",
  "find_elements", "get_value", "wait_for",
]);
const STATE_CHAR_BUDGET = Number(process.env.CODEWHALE_CU_MAX_STATE_CHARS) > 0
  ? Number(process.env.CODEWHALE_CU_MAX_STATE_CHARS)
  : 16_000;
/**
 * Largest base64 image payload we will put in one JSON-RPC message. Hosts cap
 * how much a stdio server may write between message boundaries (Claude Code
 * disconnects at 16MB) and model APIs cap image bytes well below that, so a
 * full-screen 5K PNG must degrade rather than take the transport down.
 */
const INLINE_IMAGE_MAX_BYTES = Number(process.env.CODEWHALE_CU_MAX_IMAGE_BYTES) > 0
  ? Number(process.env.CODEWHALE_CU_MAX_IMAGE_BYTES)
  : 5_000_000;

/** Base64 expands 3 bytes to 4, padded to a multiple of 4. */
const encodedSize = (bytes) => Math.ceil(bytes / 3) * 4;

function receipt(computer, extra) {
  return {
    computer: computer ? { id: computer.id, transport: computer.transport, platform: computer.platform ?? computer.platformHint ?? null } : null,
    ts: new Date().toISOString(),
    ...extra,
  };
}

function fail(computer, code, message, extra = {}) {
  return receipt(computer, { ok: false, error: { code, message }, ...extra });
}

function invalidateObservations(id) {
  lastRasters.delete(id);
  latestStateByComputer.delete(id);
  for (const [stateId, state] of appStates) {
    if (state.computerId === id) appStates.delete(stateId);
  }
}

async function retireBinding(id) {
  const binding = backendCache.get(id);
  invalidateObservations(id);
  if (!binding) return;
  closeSshChannel(binding);
  // Mark unusable before awaiting cleanup. A failure, or a catalog rollback,
  // must never resurrect this backend or its observations.
  binding.retired = true;
  binding.needsObservation = true;
  await withSignal(null, async () => {
    const outcomes = await Promise.allSettled([
      binding.usedApp ? closeAppSession() : Promise.resolve(),
      (async () => {
        try { await binding.backend?.releaseInput?.(); }
        finally { await binding.backend?.closeSession?.(); }
      })(),
    ]);
    const failed = outcomes.find(result => result.status === "rejected");
    if (failed) throw failed.reason;
  });
  binding.backend = null;
}

async function bindComputer(computer) {
  const route = routeFingerprint(computer);
  let binding = backendCache.get(computer.id);
  if (binding && (binding.route !== route || binding.retired)) {
    await retireBinding(computer.id);
    binding = { route, needsObservation: true };
    backendCache.set(computer.id, binding);
  } else if (!binding) {
    binding = { route, needsObservation: false };
    backendCache.set(computer.id, binding);
  }
  return binding;
}

async function assertCurrentRoute(computer, binding, dispatched = false) {
  try {
    let current;
    try { current = registry.get(computer.id); }
    catch (err) { await retireBinding(computer.id); throw err; }
    if (binding.retired || routeFingerprint(current) !== binding.route) {
      await bindComputer(current);
      throw new ServerError("computer_route_changed", "Computer route changed during this request — observe the registered target again before acting");
    }
  } catch (err) {
    if (dispatched) err.requestDispatched = true;
    throw err;
  }
}

async function getBackend(computer, binding) {
  if (!binding.backend) binding.backend = (await backendFor(computer)).backend;
  return binding.backend;
}

/**
 * Element target -> enriched target with cached app identity and AX path.
 * An explicit state_id pins a specific observation; a bare index addresses
 * the latest observation on this computer — the flat addressing a caller
 * uses when it acts on what it just saw.
 */
function resolveElement(target, computer) {
  const stateId = target.state_id ?? latestStateByComputer.get(computer.id);
  const st = stateId ? appStates.get(stateId) : null;
  if (!st) throw new ServerError("unknown_state", target.state_id
    ? `state_id "${target.state_id}" is unknown or expired — call get_app_state again`
    : "no observation on this computer yet — call get_app_state first");
  const el = st.elements[target.index];
  if (!el) throw new ServerError("unknown_element", `element index ${target.index} is outside state ${stateId} (0..${st.elements.length - 1})`);
  return { state: st, element: el, stateId };
}

class ServerError extends Error {
  constructor(code, message) { super(message); this.code = code; }
}

/** Map raster-pixel coordinates to screen points using the bound raster. */
function rasterToPoints(computerId, x, y) {
  const r = lastRasters.get(computerId);
  if (!r) throw new ServerError("no_raster", "no screenshot bound on this computer yet — call screenshot first so pixel targets have a frame");
  if (r.pixels?.w != null && r.pixels?.h != null && (x < 0 || y < 0 || x >= r.pixels.w || y >= r.pixels.h)) {
    throw new ServerError("target_outside_raster", `target (${x},${y}) is outside the bound raster (${r.pixels.w}x${r.pixels.h} pixels) — take a fresh screenshot`);
  }
  const scale = r.scale && r.scale > 0 ? r.scale : 1;
  return { x: (r.origin?.x ?? 0) + x / scale, y: (r.origin?.y ?? 0) + y / scale };
}

/**
 * Normalize a target into backend form: points for coordinates, resolved
 * element for elements. Element targets are revalidated against the live
 * backend when a resolver is available: stale elements throw `element_stale`,
 * moved-but-identical elements are re-aimed at their fresh center
 * (sink.reacquired = true so the receipt can say target_reacquired).
 */
async function normalizeTarget(computer, target, kind, resolve, sink) {
  if (target?.type === "coordinate") {
    if (target.space === "screen") {
      if (!Number.isFinite(target.x) || !Number.isFinite(target.y)) {
        throw new ServerError("bad_target", "screen coordinates must be finite numbers");
      }
      return { x: Math.round(target.x), y: Math.round(target.y), strategy: "event", coordinate_space: "screen" };
    }
    if (target.x < 0 || target.y < 0) throw new ServerError("bad_target", "raster coordinates must be non-negative");
    const pt = rasterToPoints(computer.id, target.x, target.y);
    return { x: Math.round(pt.x), y: Math.round(pt.y), strategy: "event", coordinate_space: "raster" };
  }
  if (target?.type === "element") {
    const { state, element, stateId } = resolveElement(target, computer);
    if (state.computerId && state.computerId !== computer.id) {
      throw new ServerError("state_wrong_computer", `state_id "${stateId}" belongs to computer "${state.computerId}", not "${computer.id}" — call get_app_state on that computer again`);
    }
    // The receipt must name the observation actually resolved — a bare index
    // binds the computer's latest state, so reporting `target.state_id` would
    // say "undefined" for the common case.
    const where = `state ${stateId} (${state.app_ref?.name ?? state.app_ref?.bundle_id ?? `pid ${state.app_ref?.pid}`})`;
    let fresh = null;
    if (resolve) {
      const res = await resolve({ app_ref: state.app_ref, windowIndex: element.windowIndex ?? 0, path: element.path });
      if (!res?.found || !res.element) {
        throw new ServerError("element_stale", `element ${target.index} of ${where} no longer resolves (${res?.reason ?? "not_found"}) — the user or the app may have changed it; call get_app_state again`);
      }
      fresh = res.element;
      if (fresh.role !== element.role) {
        throw new ServerError("element_stale", `element ${target.index} of ${where} changed role (${element.role} → ${fresh.role}) — call get_app_state again`);
      }
      // In-place replacement: same role and geometry but a different label is
      // still a different element (e.g. "Load" → "Confirm").
      if (fresh.label !== element.label) {
        throw new ServerError("element_stale", `element ${target.index} of ${where} changed label (${element.label} → ${fresh.label}) — call get_app_state again`);
      }
    }
    if (kind === "semantic") {
      return {
        app_ref: state.app_ref, windowIndex: element.windowIndex ?? 0, path: element.path,
        strategy: "a11y", role: element.role, label: element.label, reacquired: false,
      };
    }
    const moved = !!fresh && (
      fresh.position?.x !== element.position?.x || fresh.position?.y !== element.position?.y ||
      fresh.size?.w !== element.size?.w || fresh.size?.h !== element.size?.h);
    const pos = fresh?.position ?? element.position;
    const sz = fresh?.size ?? element.size;
    if (!pos || !sz) throw new ServerError("element_no_geometry", `element ${target.index} of ${where} has no cached geometry — use a coordinate target`);
    if (moved && sink) sink.reacquired = true;
    // Keep the element identity as well as geometry: semantic clicks must not
    // substitute whichever element happens to occupy an oversized AX center.
    const c = { x: Math.round(pos.x + sz.w / 2), y: Math.round(pos.y + sz.h / 2) };
    return { ...c, strategy: "a11y-center", role: element.role, label: element.label, app_ref: state.app_ref,
      windowIndex: element.windowIndex ?? 0, path: element.path, reacquired: moved };
  }
  throw new ServerError("bad_target", "target must be {type:'coordinate',x,y} or {type:'element',index} (state_id optional to pin a specific observation)");
}

function bindRaster(computer, shot) {
  lastRasters.set(computer.id, {
    file: shot.file ?? shot.path,
    scale: shot.scale ?? 1,
    origin: shot.points ?? { x: 0, y: 0 },
    pixels: shot.pixels ?? null,
    capturedAt: shot.capturedAt ?? new Date().toISOString(),
  });
}

/** A zoom produces a child raster: origin shifted by the crop, parent scale. */
function bindZoomRaster(computer, parent, region, file) {
  const scale = parent.scale && parent.scale > 0 ? parent.scale : 1;
  lastRasters.set(computer.id, {
    file,
    scale,
    origin: {
      x: (parent.origin?.x ?? 0) + region[0] / scale,
      y: (parent.origin?.y ?? 0) + region[1] / scale,
    },
    pixels: { w: region[2], h: region[3] },
    parent: parent.file,
    capturedAt: new Date().toISOString(),
  });
}

function rememberState(computer, app_ref, result) {
  const id = `s-${++stateCounter}`;
  // The observed identity wins over the caller's hint: "chrome" may have
  // resolved to "Google Chrome", and later re-resolution has to name the same
  // process, not re-run a loose match that could pick a different one.
  const resolved = { ...app_ref };
  for (const key of ["pid", "bundle_id", "name"]) if (result[key] != null && result[key] !== "") resolved[key] = result[key];
  appStates.set(id, { computerId: computer.id, app_ref: resolved, elements: result.elements ?? [], ts: Date.now() });
  latestStateByComputer.set(computer.id, id);
  if (appStates.size > 24) {
    for (const k of appStates.keys()) { appStates.delete(k); break; }
  }
  return id;
}

function filterElements(elements, { detail, query, role, limit, offset, compact }) {
  const full = detail === "full";
  let rows = (elements ?? []).map((el, i) => ({ ...el, index: el.index ?? i }));
  if (!full) {
    rows = rows.filter((el) => el.windowIndex !== -1 || !Array.isArray(el.path) || el.path.length <= 1);
  }
  if (role) rows = rows.filter((el) => el.role === role);
  if (query) {
    const q = String(query).toLowerCase();
    rows = rows.filter((el) => [el.label, el.value, el.role, el.subrole].some((v) => String(v ?? "").toLowerCase().includes(q)));
  }
  const matched = rows.length;
  const start = Math.max(0, Number(offset) || 0);
  const cap = limit != null ? Math.max(1, Math.min(200, Number(limit))) : null;
  const sliced = cap != null ? rows.slice(start, start + cap) : rows.slice(start);
  const view = sliced.map((el) => {
    if (full) return el;
    const { path, windowIndex, ...rest } = el;
    if (!compact) return rest;
    const label = rest.label != null ? String(rest.label).slice(0, 80) : rest.label;
    const value = rest.value != null && String(rest.value).length > 200 ? String(rest.value).slice(0, 200) : rest.value;
    return { index: rest.index, role: rest.role, label, value, focused: rest.focused, enabled: rest.enabled, actions: rest.actions };
  });
  return { elements: view, matched, offset: start, returned: view.length, truncated: start + view.length < matched };
}

function fitStatePayload(data, budget) {
  let payload = data;
  let json = JSON.stringify(payload);
  if (json.length <= budget) return payload;
  if (payload.ocr) {
    payload = { ...payload, ocr: { status: payload.ocr.status ?? "omitted", omitted: true, reason: "ocr_too_large", note: "OCR omitted so this observation stays readable. Retry include_ocr with ocr_region, query, or a smaller window." } };
    json = JSON.stringify(payload);
    if (json.length <= budget) return { ...payload, truncated: true };
  }
  let elements = payload.elements ?? [];
  const matched = payload.matched ?? elements.length;
  while (elements.length > 4 && json.length > budget) {
    elements = elements.slice(0, Math.max(4, Math.floor(elements.length / 2)));
    payload = {
      ...payload,
      elements,
      truncated: true,
      matched,
      returned: elements.length,
      next_offset: (payload.offset ?? 0) + elements.length,
      note: "Observation truncated to keep the transport intact. Pass query, role, limit and offset; do not retry an unfiltered dump.",
    };
    json = JSON.stringify(payload);
  }
  return payload;
}

async function invokeType(invoke, prepared) {
  const text = String(prepared.text ?? "");
  const pressEnter = prepared.press_enter === true;
  const parts = text.split(/\r\n|\n|\r/);
  const rest = { ...prepared };
  delete rest.press_enter;
  if (parts.length === 1 && !pressEnter) return invoke("type", rest);
  const steps = [];
  for (let i = 0; i < parts.length; i++) {
    if (parts[i]) steps.push(await invoke("type", { ...rest, text: parts[i] }));
    if (i < parts.length - 1 || (pressEnter && i === parts.length - 1)) {
      steps.push(await invoke("key", { text: "return" }));
    }
  }
  const last = steps.at(-1) ?? { action_sent: true };
  return { ...last, newlines_as_return: true, typed_parts: steps.length };
}

function observeState(computer, app_ref, result, args = {}) {
  // Cache the complete backend records before making the model-facing view.
  // Public indices still address those records, including their private AX
  // paths; a compact response must never weaken live target revalidation.
  // Ephemeral polls (wait_for) share the filter math without churning the
  // state cache: only the observation a caller can act on earns a state_id.
  const ephemeral = args.ephemeral === true;
  const state_id = ephemeral ? null : rememberState(computer, app_ref, result);
  const compact = args.detail === "compact" || args.compact === true;
  const detail = args.detail === "full" ? "full" : compact ? "compact" : "summary";
  const filtered = filterElements(result.elements, {
    detail: args.detail === "full" ? "full" : "summary",
    query: args.query,
    role: args.role,
    limit: args.limit,
    offset: args.offset,
    compact,
  });
  const data = {
    ...result,
    state_id,
    elements: filtered.elements,
    detail,
    matched: filtered.matched,
    offset: filtered.offset,
    returned: filtered.returned,
    truncated: filtered.truncated,
    note: ephemeral
      ? "Ephemeral poll: elements are not bound to a state_id."
      : "Indices target this observation's cached tree (including rows not shown); pin it with state_id, or re-observe after the app changes.",
  };
  if (compact && data.ocr && args.include_ocr !== true) delete data.ocr;
  return fitStatePayload(data, STATE_CHAR_BUDGET);
}

/**
 * Poll get_app_state until the query/role predicate holds or the deadline
 * passes. Intermediate polls are ephemeral — they share the filter math but
 * never churn the state cache; the observation that satisfies the predicate
 * is read once more, bound, and its state_id is what the caller targets.
 * Errors that can resolve themselves (app not launched yet) count as "no
 * match yet"; errors that cannot (stopped, route changed) abort the wait.
 */
async function waitFor(computer, args, switched) {
  const { query, role } = args;
  if (query == null && role == null) throw new ServerError("bad_args", "wait_for needs a query and/or role to watch for");
  if (query != null && typeof query !== "string") throw new ServerError("bad_args", "query must be a string");
  if (role != null && typeof role !== "string") throw new ServerError("bad_args", "role must be a string");
  const state = args.state ?? "present";
  if (state !== "present" && state !== "absent") throw new ServerError("bad_args", 'state must be "present" or "absent"');
  const timeoutSec = Number(args.timeout ?? 10);
  if (!Number.isFinite(timeoutSec) || timeoutSec < 0.5 || timeoutSec > 60) throw new ServerError("bad_args", "timeout must be 0.5..60 seconds");
  const intervalMs = Number(args.interval ?? 400);
  if (!Number.isInteger(intervalMs) || intervalMs < 100 || intervalMs > 5000) throw new ServerError("bad_args", "interval must be an integer 100..5000 ms");
  const limit = args.limit ?? 20;
  if (!Number.isSafeInteger(limit) || limit < 1 || limit > 100) throw new ServerError("bad_args", "limit must be an integer 1..100");

  const FATAL = new Set(["cancelled", "control_stopped", "computer_route_changed", "app_upgrade_required"]);
  const observe = (ephemeral) => callTool({ name: "get_app_state", arguments: {
    app_ref: args.app_ref, window_id: args.window_id, query, role,
    limit, detail: "compact", ephemeral, computer: computer.id,
  }});
  const started = Date.now();
  const deadline = started + timeoutSec * 1000;
  let polls = 0, lastError = null, everObserved = false;
  while (true) {
    const res = await observe(true);
    polls++;
    const body = JSON.parse(res.content[0].text);
    let usable = false, matchedCount = 0;
    if (!res.isError && body.ok !== false) { usable = true; matchedCount = body.matched ?? 0; }
    else if (FATAL.has(body?.error?.code)) {
      return { content: [{ type: "text", text: JSON.stringify(fail(computer, body.error.code, body.error.message, { tool: "wait_for", switched, polls })) }], isError: true };
    } else if (body?.found === false || /application not found/.test(body?.error?.message ?? "")) {
      usable = true; // not running yet, or gone: zero matches either way
    } else {
      lastError = body?.error ?? { code: "observe_failed", message: "observation failed" };
    }
    if (usable) { everObserved = true; lastError = null; }
    if (usable && (state === "absent" ? matchedCount === 0 : matchedCount > 0)) {
      const bound = await observe(false);
      polls++;
      const b = JSON.parse(bound.content[0].text);
      if (bound.isError || b.ok === false) {
        return { content: [{ type: "text", text: JSON.stringify(receipt(computer, { ok: true, tool: "wait_for", switched, matched: true, state, polls, elapsed_ms: Date.now() - started, note: "Condition held but the follow-up observation failed — call get_app_state before targeting." })) }] };
      }
      return { content: [{ type: "text", text: JSON.stringify(receipt(computer, { ok: true, tool: "wait_for", switched, matched: true, state, polls, elapsed_ms: Date.now() - started, state_id: b.state_id, matched_count: b.matched ?? 0, elements: b.elements, app: { name: b.name ?? null, pid: b.pid ?? null, bundle_id: b.bundle_id ?? null }, note: "Elements are bound to this observation — target them with {type:'element', index}; add state_id only to pin this snapshot after later observes. Re-observe if the UI changes again." })) }] };
    }
    if (Date.now() >= deadline) break;
    await wait(Math.min(intervalMs, Math.max(1, deadline - Date.now())));
    throwIfAborted();
  }
  if (!everObserved && lastError) {
    return { content: [{ type: "text", text: JSON.stringify(fail(computer, lastError.code ?? "observe_failed", lastError.message ?? "observation failed", { tool: "wait_for", switched, polls, elapsed_ms: Date.now() - started })) }], isError: true };
  }
  return { content: [{ type: "text", text: JSON.stringify(receipt(computer, { ok: true, tool: "wait_for", switched, matched: false, timed_out: true, state, polls, elapsed_ms: Date.now() - started, ...(lastError ? { last_error: lastError } : {}), note: state === "absent" ? "Matches remained until the deadline." : "No match appeared before the deadline. Observe the app or widen the query." })) }] };
}

// ---------- tool dispatch ----------
async function callTool(params) {
  const requested = params.name;
  if (!TOOL_NAMES.has(requested)) {
    return { content: [{ type: "text", text: JSON.stringify({ ok: false, error: { code: "unknown_tool", message: `unknown tool "${requested}"` } }) }], isError: true };
  }
  // Merged tools (click, pointer, clipboard, recording, computer, key+duration)
  // resolve to the wire tool they dispatch to before any gate below, so they
  // cannot bypass required args, the kill switch or routing. Wire names stay
  // callable as aliases.
  let name = requested;
  let args = params.arguments ?? {};
  try {
    ({ name, args } = resolveTool(requested, args));
  } catch (err) {
    return { content: [{ type: "text", text: JSON.stringify(fail(null, err.code ?? "bad_args", err.message)) }], isError: true };
  }
  // A narrowed session (CODEWHALE_CU_GRANT) refuses anything outside its grant
  // before required-arg or routing behavior can leak. stop_computer_control
  // stays reachable as the safety valve; the daemon enforces the same set.
  if (GRANT && requested !== "stop_computer_control" && !GRANT.has(requested) && !GRANT.has(name)) {
    return { content: [{ type: "text", text: JSON.stringify(fail(null, "not_granted", `"${requested}" is outside this session's capability grant (${GRANT.size} tools). The host narrowed this session deliberately; do not look for a workaround.`)) }], isError: true };
  }
  // Hosts are not required to enforce inputSchema. Check declared `required`
  // fields here so a missing argument becomes bad_args instead of a backend
  // crash or an opaque native error. The message names the tool the caller
  // asked for, not the wire name it resolved to.
  for (const field of REQUIRED_ARGS.get(name) ?? []) {
    if (args[field] === undefined || args[field] === null) {
      return { content: [{ type: "text", text: JSON.stringify(fail(null, "bad_args", `${requested} requires "${field}"`)) }], isError: true };
    }
  }

  if (name === "stop_computer_control") {
    controlStopped = true;
    for (const request of requests.values()) {
      if (request.name && request.name !== "stop_computer_control") request.controller.abort();
    }
    try {
      await releaseControl({ releaseOnly: true });
      return { content: [{ type: "text", text: JSON.stringify(receipt(null, { ok: true, stopped: true, inFlight, inputReleased: true, note: "Queued input was refused and ongoing requests were cancelled. Input already delivered cannot be undone. Restart this MCP session to resume." })) }] };
    } catch (err) {
      return { content: [{ type: "text", text: JSON.stringify(fail(null, "input_release_failed", String(err?.message ?? err), { stopped: true, inFlight })) }], isError: true };
    }
  }
  if (controlStopped && !READ_ONLY_TOOLS.has(name)) {
    return { content: [{ type: "text", text: JSON.stringify(fail(null, "control_stopped", "stop_computer_control is active; no further actions are permitted this session")) }], isError: true };
  }

  if (name === "wait") {
    const s = Math.max(0, Math.min(30, Number(args.seconds) || 1));
    await wait(s * 1000);
    return { content: [{ type: "text", text: JSON.stringify(receipt(null, { ok: true, waitedSec: s })) }] };
  }

  if (name === "trajectory_start") {
    const r = recorder.start();
    return { content: [{ type: "text", text: JSON.stringify(receipt(null, { ok: true, tool: "trajectory_start", ...r, note: "Every tool call this session makes is appended to a local JSONL. Arguments are stored verbatim so replay is faithful — start it only when the person knows it runs." })) }] };
  }
  if (name === "trajectory_stop") {
    return { content: [{ type: "text", text: JSON.stringify(receipt(null, { ok: true, tool: "trajectory_stop", ...recorder.stop() })) }] };
  }
  if (name === "trajectory_status") {
    return { content: [{ type: "text", text: JSON.stringify(receipt(null, { ok: true, tool: "trajectory_status", ...recorder.status(), recent: listTrajectories(5) })) }] };
  }
  if (name === "trajectory_replay") {
    let file;
    try { file = resolveTrajectory(args.id); } catch (err) {
      return { content: [{ type: "text", text: JSON.stringify(fail(null, err.code ?? "bad_args", err.message)) }], isError: true };
    }
    const calls = readTrajectory(file).filter((entry) => entry.type === "call" && typeof entry.tool === "string" && !isTrajectoryTool(entry.tool));
    if (calls.length > 200) {
      return { content: [{ type: "text", text: JSON.stringify(fail(null, "replay_too_large", `this trajectory has ${calls.length} calls; replay is limited to 200 at a time`)) }], isError: true };
    }
    const dryRun = args.dry_run === true;
    const results = [];
    if (!dryRun) {
      replaying = true;
      try {
        for (const call of calls) {
          if (controlStopped && !READ_ONLY_TOOLS.has(call.tool)) { results.push({ tool: call.tool, ok: false, code: "control_stopped" }); break; }
          let body = null;
          try {
            const r = await callTool({ name: call.tool, arguments: call.args ?? {} });
            body = JSON.parse(r?.content?.[0]?.text ?? "null");
          } catch (err) {
            results.push({ tool: call.tool, ok: false, code: err?.code ?? "replay_failed", message: String(err?.message ?? err).slice(0, 200) });
            break;
          }
          const ok = body?.ok !== false;
          results.push({ tool: call.tool, ok, ...(ok ? {} : { code: body?.error?.code ?? "refused" }) });
          if (!ok) break; // a trajectory is a sequence — replay stops where it broke
        }
      } finally { replaying = false; }
    }
    const failed = results.filter((r) => r.ok === false).length;
    return { content: [{ type: "text", text: JSON.stringify(receipt(null, { ok: true, tool: "trajectory_replay", trajectory: path.basename(file), dry_run: dryRun, turns_in_file: calls.length, replayed: results.length, failed, ...(dryRun ? { plan: calls.map((c) => c.tool) } : { results }), note: dryRun ? "Nothing was executed. Run again without dry_run:true to replay through the normal gates." : "Replay re-entered the normal pipeline; grants, permissions and the kill switch still apply." })) }] };
  }

  if (name === "computer_list") {
    const reg = registry.list();
    return { content: [{ type: "text", text: JSON.stringify(receipt(null, {
      ok: true,
      active: activeComputerId,
      computers: Object.values(reg.computers).map((c) => ({ id: c.id, transport: c.transport, platform: c.platform ?? c.platformHint ?? null, label: c.label ?? null, host: c.host ?? null })),
      note: "Pass `computer` on any tool to switch (sticky), or computer_switch to switch explicitly.",
    })) }] };
  }

  if (name === "computer_register") {
    try {
      const entry = registry.register({ id: args.computer, transport: args.transport, label: args.label, host: args.host, port: args.port, user: args.user, target: args.target });
      await bindComputer(entry);
      let installed = null;
      if (entry.transport === "ssh" && args.installAgent !== false) {
        installed = await installRemoteAgent(entry);
        registry.register({ id: entry.id, transport: "ssh", host: entry.host, port: entry.port, user: entry.user, platformHint: installed.remotePlatform, agentPath: installed.agentPath });
      }
      if (entry.transport === "ssh" && args.installAgent === false && !entry.platformHint) {
        // Probe cheaply through the agent; if it is missing, registration still succeeds.
        try {
          const ex = await executorFor(entry);
          const reply = await ex.remote({ tool: "platform" });
          registry.register({ id: entry.id, transport: "ssh", host: entry.host, port: entry.port, user: entry.user, platformHint: reply.platform });
        } catch {}
      }
      const fresh = registry.get(entry.id);
      await bindComputer(fresh);
      return { content: [{ type: "text", text: JSON.stringify(receipt(null, { ok: true, registered: { ...fresh, platform: fresh.platform ?? fresh.platformHint ?? null }, agentInstall: installed })) }] };
    } catch (err) {
      // Registration problems (unreachable host, agent push failed) are
      // receipts, not protocol errors.
      return { content: [{ type: "text", text: JSON.stringify(fail(null, err.code ?? "register_failed", err.message ?? String(err))) }], isError: true };
    }
  }

  if (name === "computer_remove") {
    const res = registry.remove(args.computer);
    if (activeComputerId === args.computer) activeComputerId = "local";
    res.active = activeComputerId;
    await retireBinding(args.computer);
    return { content: [{ type: "text", text: JSON.stringify(receipt(null, { ok: true, ...res })) }] };
  }

  if (name === "computer_switch") {
    const c = registry.get(args.computer);
    activeComputerId = c.id;
    return { content: [{ type: "text", text: JSON.stringify(receipt(c, { ok: true, active: c.id })) }] };
  }

  // Everything below acts on a computer.
  let computer;
  let switched = false;
  try {
    if (args.computer && args.computer !== activeComputerId) {
      computer = registry.get(args.computer);
      activeComputerId = computer.id;
      switched = true;
    } else {
      computer = registry.get(activeComputerId);
    }
  } catch (err) {
    await retireBinding(args.computer || activeComputerId);
    return { content: [{ type: "text", text: JSON.stringify(fail(null, err.code ?? "registry_error", err.message)) }], isError: true };
  }

  let binding;
  let dispatched = false;
  try {
    binding = await bindComputer(computer);
    if (binding.needsObservation && !ROUTE_INSPECTION_TOOLS.has(name)) {
      throw new ServerError("computer_observation_required", "Computer route changed — call screenshot or get_app_state on the registered target before acting");
    }
    if (name === "run_actions") {
      const steps = args.steps;
      if (!Array.isArray(steps) || steps.length < 1 || steps.length > 8) throw new ServerError("bad_args", "run_actions needs 1..8 steps");
      const results = [];
      for (const [i, step] of steps.entries()) {
        if (!step || typeof step.tool !== "string") throw new ServerError("bad_args", `step ${i} needs a tool name`);
        if (step.tool === "run_actions") throw new ServerError("bad_args", "run_actions cannot nest");
        if (!TOOL_NAMES.has(step.tool)) throw new ServerError("unknown_tool", `unknown tool "${step.tool}"`);
        const result = await callTool({ name: step.tool, arguments: { ...(step.arguments ?? {}), computer: computer.id } });
        const body = JSON.parse(result.content[0].text);
        results.push({ tool: step.tool, ok: body.ok !== false, receipt: body });
        if (body.ok === false || result.isError) {
          return { content: [{ type: "text", text: JSON.stringify(fail(computer, body.error?.code ?? "step_failed", body.error?.message ?? "step failed", { tool: "run_actions", switched, stopped_at: i, steps: results })) }], isError: true };
        }
      }
      return { content: [{ type: "text", text: JSON.stringify(receipt(computer, { ok: true, tool: "run_actions", switched, steps: results })) }] };
    }
    if (name === "find_elements") {
      const st = args.state_id ? appStates.get(args.state_id) : null;
      if (args.state_id && !st) throw new ServerError("unknown_state", `state_id "${args.state_id}" is unknown or expired — call get_app_state again`);
      if (st) {
        if (st.computerId && st.computerId !== computer.id) {
          throw new ServerError("state_wrong_computer", `state_id "${args.state_id}" belongs to computer "${st.computerId}", not "${computer.id}"`);
        }
        const filtered = filterElements(st.elements, {
          detail: "summary", query: args.query, role: args.role,
          limit: args.limit ?? 20, offset: args.offset ?? 0, compact: true,
        });
        return { content: [{ type: "text", text: JSON.stringify(receipt(computer, { ok: true, tool: "find_elements", switched, state_id: args.state_id, ...filtered, note: "Indices address the cached tree from this state_id." })) }] };
      }
      return callTool({ name: "get_app_state", arguments: { ...args, detail: "compact", limit: args.limit ?? 20, computer: computer.id } });
    }
    if (name === "wait_for") {
      return waitFor(computer, args, switched);
    }
    // type/key with an element target run the documented focus-then-act idiom
    // in one call: the element is revalidated and accessibility-focused first,
    // through the same routed path a separate focus call would take.
    if ((name === "type" || name === "key") && args.target != null) {
      if (args.target.type !== "element") {
        throw new ServerError("bad_target", `${name} accepts element targets only — use left_click for a coordinate, then ${name}`);
      }
      const focused = await callTool({ name: "focus", arguments: { target: args.target, computer: computer.id } });
      const focusBody = JSON.parse(focused.content[0].text);
      if (focused.isError || focusBody.ok === false) {
        return { content: [{ type: "text", text: JSON.stringify(fail(computer, focusBody.error?.code ?? "focus_failed", focusBody.error?.message ?? "element could not be focused", { tool: name, stage: "focus" })) }], isError: true };
      }
      args = { ...args };
      delete args.target;
    }
    // Out-of-process runners (the desktop app for the local computer, the
    // remote agent for ssh computers) get the request over the wire.
    const backendMethod = BACKEND_METHOD[name];
    let data;
    const ex = computer.transport === "local" || computer.transport === "ssh" ? await executorFor(computer, binding) : null;
    if (ex?.kind === "app") binding.usedApp = true;
    // Zoom needs the bound parent raster up front (server-side check too, not
    // only the backend) so it can bind the child raster after success.
    let zoomParent = null;
    if (name === "zoom") {
      zoomParent = lastRasters.get(computer.id);
      if (!zoomParent) throw new ServerError("no_raster", "no screenshot bound on this computer yet — call screenshot first so zoom has a source raster");
      if (!Array.isArray(args.region) || args.region.length !== 4) throw new ServerError("bad_args", "zoom needs region [x, y, w, h] in last-raster pixels");
    }
    const sink = { reacquired: false };

    if (typeof ex?.remote === "function" && REMOTE_TOOLS.has(backendMethod)) {
      // ssh rides the persistent agent channel when the remote supports
      // --serve; a channel that never produced a reply means an old agent,
      // so fall back to one-shot for that binding rather than failing.
      const remoteCall = async (request, opts = {}) => {
        if (typeof ex.persistent === "function" && binding.sshServe !== false) {
          try {
            return await ex.persistent(request, opts);
          } catch (err) {
            const ch = binding.sshChannel;
            if (err?.code === "remote_session_lost" && ch && !ch.everReplied) {
              binding.sshServe = false;
              ex.closeChannel?.();
              return ex.remote(request, opts);
            }
            throw err;
          }
        }
        return ex.remote(request, opts);
      };
      const resolve = async (req) => {
        const rep = await remoteCall({ tool: "resolve_element", args: req }, { timeoutMs: 30_000 });
        if (!rep?.ok) return { found: false, element: null, reason: rep?.error?.code ?? "remote_error" };
        return rep.data;
      };
      const wireArgs = await prepareArgs(computer, name, args, resolve, sink);
      throwIfAborted();
      await assertCurrentRoute(computer, binding);
      // Re-check the kill switch: a stop that arrived while the executor was
      // being resolved still blocks this dispatch.
      if (controlStopped && !READ_ONLY_TOOLS.has(name)) throw new ServerError("control_stopped", "stop_computer_control is active; no further actions are permitted this session");
      inFlight++;
      try {
        dispatched = true;
        const timeoutMs = backendMethod.startsWith("recording") || backendMethod === "get_app_state" ? 60_000 : 30_000;
        const invoke = async (tool, a) => {
          const r = await remoteCall({ tool, args: a }, { timeoutMs });
          if (!r.ok) throw new ServerError(r.error?.code ?? "remote_error", r.error?.message ?? "remote agent failed");
          return r.data;
        };
        data = name === "type" ? await invokeType(invoke, wireArgs) : await invoke(backendMethod, wireArgs);
      } finally {
        inFlight--;
      }
      await assertCurrentRoute(computer, binding, true);
      if (Array.isArray(data)) data = { items: data };
      if ((backendMethod === "screenshot" || backendMethod === "zoom") && data?.file) {
        if (ex.filesLocal) bindRaster(computer, data);
        else {
          // Raster lives on the remote machine; bind geometry for coordinate mapping.
          bindRaster(computer, { ...data, file: null });
          data.note = "file lives on the remote computer; pull it with scp if you need the bytes locally";
        }
      }
      if (backendMethod === "zoom") bindZoomRaster(computer, zoomParent, args.region, ex.filesLocal ? data?.file ?? data?.path : null);
      if (name === "get_app_state") {
        data = observeState(computer, wireArgs.app_ref, data, args);
      }
      if (backendMethod === "probe") Object.assign(data, { via: ex.kind, app: ex.app ?? null });
      if (backendMethod === "probe" && data?.app?.version && data.app.version !== APP_VERSION) {
        // The helper owns the modules it loaded at start, so a plugin update
        // without a helper restart serves the previous build's behavior. Say
        // so instead of letting the agent debug a build that is not running.
        data.app.bundled_version = APP_VERSION;
        data.app.stale = true;
        data.note = [data.note, `The running helper reports ${data.app.version} but this plugin is ${APP_VERSION} — restart the Codewhale Computer Use app to load the current build.`].filter(Boolean).join(" ");
      }
    } else {
      const backend = await getBackend(computer, binding);
      if (typeof backend[backendMethod] !== "function") {
        throw new ServerError("unsupported_on_backend", `"${name}" is not implemented on the ${computer.platform ?? computer.transport} backend`);
      }
      const resolve = typeof backend.resolve_element === "function" ? (req) => backend.resolve_element(req) : null;
      const prepared = await prepareArgs(computer, name, args, resolve, sink);
      throwIfAborted();
      await assertCurrentRoute(computer, binding);
      if (controlStopped && !READ_ONLY_TOOLS.has(name)) throw new ServerError("control_stopped", "stop_computer_control is active; no further actions are permitted this session");
      inFlight++;
      try {
        dispatched = true;
        data = name === "type"
          ? await invokeType((tool, a) => backend[BACKEND_METHOD[tool] ?? tool](a), prepared)
          : await backend[backendMethod](prepared);
      } finally {
        inFlight--;
      }
      await assertCurrentRoute(computer, binding, true);
      if (Array.isArray(data)) data = { items: data }; // keep receipts objects
      if (name === "screenshot") bindRaster(computer, data);
      if (backendMethod === "zoom") bindZoomRaster(computer, zoomParent, args.region, data?.file ?? data?.path);
      if (name === "get_app_state") {
        data = observeState(computer, prepared.app_ref, data, args);
      }
      if (backendMethod === "probe" && computer.transport === "local") {
        // Direct mode: permissions belong to whatever hosts this server. Say so.
        Object.assign(data, { via: "direct", app: null, appHint: ex?.appReason ?? null });
      }
    }

    // Binding a different app retires this computer's element cache: a bare
    // index must never silently address the previous app's observation —
    // under a concurrent user that mistake clicks the wrong window.
    if (name === "open_application" && data?.resolved) {
      const latestId = latestStateByComputer.get(computer.id);
      const latest = latestId ? appStates.get(latestId) : null;
      if (latest) {
        const a = latest.app_ref ?? {};
        const b = data.resolved;
        const sameApp = a.pid != null && b.pid != null
          ? a.pid === b.pid
          : (a.bundle_id && b.bundle_id ? a.bundle_id === b.bundle_id : a.name === b.name);
        if (!sameApp) {
          latestStateByComputer.delete(computer.id);
          data.note = [data.note, "Element indices from earlier observations belonged to a different app — call get_app_state before targeting."].filter(Boolean).join(" ");
        }
      }
    }

    if (name === "get_app_state" && args.include_ocr) {
      data.ocr ??= { status: "unavailable", reason: "Text recognition is not available on this backend", blocks: [] };
      if (data.ocr.raster) {
        const localFile = typeof ex?.remote !== "function" || ex.filesLocal;
        bindRaster(computer, localFile ? data.ocr.raster : { ...data.ocr.raster, file: null, path: null });
      }
      data.ocr.note = "Recognized text may be imperfect. These coordinate targets belong to this captured image, not to accessibility elements; observe again after the UI changes. Prefer ocr_region or query over a second full-window OCR.";
    }

    // Inline the raster only when it fits the budget. One oversized JSON-RPC
    // message drops the whole stdio transport and every other tool with it, so
    // an over-budget capture degrades to its text receipt: the file is still on
    // disk and still bound, so zoom or a narrower capture returns a viewable
    // image. Never trade the session for one screenshot.
    let imageBlock = null;
    if ((name === "screenshot" || name === "zoom" || name === "browser_screenshot") && computer.transport === "local" && (data.file || data.path)) {
      const file = data.file || data.path;
      const size = fs.statSync(file).size;
      if (encodedSize(size) > INLINE_IMAGE_MAX_BYTES) {
        data.image_omitted = {
          reason: "raster_too_large",
          bytes: size,
          encoded_bytes: encodedSize(size),
          limit_bytes: INLINE_IMAGE_MAX_BYTES,
          note: "The capture is on disk at the returned path, but inlining it would exceed this host's single-message budget and drop the connection. Capture one display, a region, or an app window, or call zoom on this raster to get a viewable image.",
        };
      } else {
        const bytes = fs.readFileSync(file);
        imageBlock = { type: "image", mimeType: bytes[0] === 0xff ? "image/jpeg" : "image/png", data: bytes.toString("base64") };
      }
    }
    if (name === "request_access") {
      const grant = grantReport();
      if (grant) data.grant = grant;
    }
    const content = [{ type: "text", text: JSON.stringify(receipt(computer, { ok: true, tool: name, switched, ...(sink.reacquired ? { target_reacquired: true } : {}), ...data })) }];
    if (imageBlock) content.push(imageBlock);
    if ((name === "screenshot" && (data?.file || data?.path) && data?.pixels?.w > 0 && data?.pixels?.h > 0) ||
        (name === "browser_screenshot" && !!data?.file) ||
        (name === "get_app_state" && data?.found !== false && Array.isArray(data?.elements))) {
      binding.needsObservation = false;
    }
    return { content };
  } catch (err) {
    let outcomeUnknown = !!err.requestDispatched;
    if (dispatched && !outcomeUnknown) {
      // A transport/backend can fail after delivering input. Reconcile its
      // captured route on failure too, without replacing the original error
      // with a route/cleanup error or claiming an unchanged-route failure sent input.
      try { await assertCurrentRoute(computer, binding, true); }
      catch { outcomeUnknown = true; }
    }
    // The grant is a launch-time server fact: report it on refusal receipts too,
    // so a narrowed session knows its bounds even when the probe itself failed
    // (for example a headless Linux host with no DISPLAY to inspect).
    const grant = name === "request_access" ? grantReport() : null;
    return { content: [{ type: "text", text: JSON.stringify(fail(computer, err.code ?? "tool_error", err.message ?? String(err), {
      tool: name, switched,
      ...(grant ? { grant } : {}),
      ...(outcomeUnknown ? { request_dispatched: true, outcome_unknown: true,
        note: "Dispatch to the previous route was attempted; its effect is unconfirmed. Observe the current target; do not automatically retry the action." } : {}),
    })) }], isError: true };
  }
}

/**
 * Convert public tool args into backend args, identically for every route.
 * Element targets carry their revalidated AX path and fresh center; coordinate
 * targets are mapped from raster pixels to screen points here, once.
 *
 * The desktop app and the ssh agent are backends like any other: sending them
 * raw raster pixels would put every click at the wrong place on a scaled
 * display and skip the raster's own fail-closed checks (no_raster,
 * target_outside_raster), which is what happened while this ran per-route.
 */
async function prepareArgs(computer, name, args, resolve, sink) {
  const out = { ...args };
  delete out.computer;
  delete out.ephemeral; // server-internal: never reaches a backend
  const semantic = new Set(["set_value", "select_text", "perform_action", "focus", "get_value"]);
  for (const key of ["target", "from_target", "to"]) {
    const given = out[key];
    if (given == null) continue;
    // Hosts that don't enforce inputSchema can hand us any shape. Refuse
    // before it reaches a backend as an opaque native error or a TypeError.
    if (typeof given !== "object" || Array.isArray(given) || (given.type !== "coordinate" && given.type !== "element")) {
      throw new ServerError("bad_target", `${key} must be {type:'coordinate',x,y[,space]} or {type:'element',index[,state_id]} — got ${JSON.stringify(given)?.slice(0, 120)}`);
    }
    if (key === "target" && ELEMENT_ONLY_TARGET.has(name) && given.type !== "element") {
      throw new ServerError("bad_target", `${name} accepts element targets only — observe the control with get_app_state and pass {type:'element',index}`);
    }
    const kind = key === "target" && semantic.has(name) ? "semantic" : "pointer";
    out[key] = { ...given, ...(await normalizeTarget(computer, given, kind, resolve, sink)) };
  }
  if (name === "get_app_state" || name === "find_elements") {
    if (out.detail != null && !["summary", "compact", "full"].includes(out.detail)) throw new ServerError("bad_args", "detail must be summary, compact or full");
    if (name === "get_app_state") {
      out.compact = out.detail === "compact";
      out.detail = out.detail === "full" ? "full" : "summary";
    }
    if (out.include_ocr != null && typeof out.include_ocr !== "boolean") throw new ServerError("bad_args", "include_ocr must be true or false");
    if (out.window_id != null && (!Number.isSafeInteger(out.window_id) || out.window_id < 0)) throw new ServerError("bad_args", "window_id must be a non-negative window index from list_windows");
    if (out.limit != null && (!Number.isSafeInteger(out.limit) || out.limit < 1 || out.limit > 200)) throw new ServerError("bad_args", "limit must be an integer 1..200");
    if (out.offset != null && (!Number.isSafeInteger(out.offset) || out.offset < 0)) throw new ServerError("bad_args", "offset must be a non-negative integer");
    if (out.query != null && typeof out.query !== "string") throw new ServerError("bad_args", "query must be a string");
    if (out.role != null && typeof out.role !== "string") throw new ServerError("bad_args", "role must be a string");
    if (out.ocr_region != null && (!Array.isArray(out.ocr_region) || out.ocr_region.length !== 4)) throw new ServerError("bad_args", "ocr_region must be [x, y, w, h] in screen points");
  }
  return out;
}

// ---------- JSON-RPC loop ----------
function respond(id, result) {
  process.stdout.write(JSON.stringify({ jsonrpc: "2.0", id, result }) + "\n");
}
function respondError(id, code, message) {
  process.stdout.write(JSON.stringify({ jsonrpc: "2.0", id, error: { code, message } }) + "\n");
}

/** JSON-RPC invalid-params error that survives the dispatch catch below. */
function paramError(message) {
  return Object.assign(new Error(message), { rpcCode: -32602 });
}

// ---------- bundled skill pack ----------
// The operating guide travels with the server and is served as MCP resources
// (skill://codewhale-cu/…) so any host can read the loop, the failure codes and
// the safety rules without paying for them in every receipt. The pack is loaded
// once at startup; a trimmed install without skills/ simply serves none.
const SKILL_NAME = "computer-use";
const SKILL_ROOT_URI = `skill://codewhale-cu/SKILL.md`;

function parseFrontmatter(text) {
  if (!text.startsWith("---\n")) return null;
  const end = text.indexOf("\n---", 4);
  if (end === -1) return null;
  const out = {};
  let key = null;
  for (const line of text.slice(4, end).split("\n")) {
    const m = /^([A-Za-z0-9_-]+):\s*(.*)$/.exec(line);
    if (m) { key = m[1]; out[key] = [">-", ">"].includes(m[2]) ? "" : m[2].replace(/^["']|["']$/g, ""); continue; }
    if (key && /^\s+\S/.test(line)) out[key] = `${out[key] ? `${out[key]} ` : ""}${line.trim()}`;
  }
  return out;
}

const skillPack = (() => {
  const root = new URL("../skills/computer-use/", import.meta.url);
  const files = [
    ["SKILL.md", "text/markdown"],
    ["references/quick-reference.md", "text/markdown"],
    ["references/refusal-codes.md", "text/markdown"],
  ];
  const pack = [];
  for (const [rel, mime] of files) {
    try {
      const bytes = fs.readFileSync(new URL(rel, root));
      const text = bytes.toString("utf8");
      pack.push({
        rel, uri: `skill://codewhale-cu/${rel}`, mime, size: bytes.length, text,
        frontmatter: rel === "SKILL.md" ? parseFrontmatter(text) : null,
        sha256: crypto.createHash("sha256").update(bytes).digest("hex"),
      });
    } catch { /* no pack on disk — serve nothing */ }
  }
  return pack;
})();
const SKILL_DESCRIPTION = skillPack.find((f) => f.rel === "SKILL.md")?.frontmatter?.description ?? "Computer-use operating guide";

/**
 * callTool plus optional trajectory recording. Recording wraps every call the
 * session makes (refusals included — they are part of what happened); the
 * recorder's own tools and replayed calls are never re-recorded.
 */
async function callToolRecorded(params) {
  const result = await callTool(params);
  if (recorder.active && !replaying && !isTrajectoryTool(params?.name)) {
    let body = null;
    try { body = JSON.parse(result?.content?.[0]?.text ?? "null"); } catch { /* non-JSON receipts record without an outcome */ }
    recorder.append({ tool: params.name, args: params.arguments ?? {}, ok: body?.ok !== false, code: body?.error?.code ?? null });
  }
  return result;
}

const HANDLERS = {
  initialize(params) {
    return {
      protocolVersion: params?.protocolVersion ?? "2025-06-18",
      capabilities: {
        tools: { listChanged: false },
        resources: { listChanged: false, subscribe: false },
        experimental: { "io.modelcontextprotocol/skills": {} },
      },
      serverInfo: { name: SERVER_NAME, version: APP_VERSION, platforms: ["darwin", "win32", "linux", "harmonyos"], transports: ["local", "ssh", "hdc"] },
    };
  },
  "tools/list"() {
    // The advertised surface is what every session pays for; merged-away wire
    // names stay callable as aliases but are never listed. A capability grant
    // narrows the listing further, never widens it.
    const advertised = TOOLS.filter((t) => t.hidden !== true);
    if (!GRANT) return { tools: advertised };
    return { tools: advertised.filter((t) => t.name === "stop_computer_control" || GRANT.has(t.name) || (MERGED_EXPANSION[t.name] ?? []).some((wire) => GRANT.has(wire))) };
  },
  "resources/list"() {
    return { resources: skillPack.map(({ uri, rel, mime, size }) => ({ uri, name: rel, mimeType: mime, size })) };
  },
  "resources/read"(params) {
    const file = skillPack.find((f) => f.uri === params?.uri);
    if (!file) throw paramError(`resource "${params?.uri ?? ""}" is not part of the bundled skill pack — resources/list names the readable URIs`);
    return { contents: [{ uri: file.uri, mimeType: file.mime, text: file.text }] };
  },
  "skills/list"() {
    return {
      skills: [{
        uri: SKILL_ROOT_URI, name: SKILL_NAME, description: SKILL_DESCRIPTION,
        files: skillPack.map(({ uri, sha256, size }) => ({ uri, sha256, bytes: size })),
      }],
    };
  },
  "skills/get"(params) {
    const entry = skillPack.find((f) => f.uri === (params?.uri ?? SKILL_ROOT_URI));
    if (!entry) throw paramError(`skill "${params?.uri ?? ""}" is unknown — skills/list names the catalog`);
    return {
      skill: { uri: entry.uri, name: SKILL_NAME, description: SKILL_DESCRIPTION, frontmatter: entry.frontmatter, content: entry.text },
      manifest: skillPack.map(({ uri, sha256, size }) => ({ uri, sha256, bytes: size })),
    };
  },
  async "tools/call"(params) {
    if (params?.name === "stop_computer_control") return callTool(params);
    const previous = dispatch;
    let release;
    dispatch = new Promise((resolve) => { release = resolve; });
    try {
      await previous;
      throwIfAborted();
      return await callToolRecorded(params ?? {});
    } catch (err) {
      if (err?.code !== "cancelled") throw err;
      return { content: [{ type: "text", text: JSON.stringify(fail(null, controlStopped ? "control_stopped" : "cancelled", err.message)) }], isError: true };
    } finally { release(); }
  },
  "notifications/cancelled"(params) {
    const request = requests.get(params?.requestId);
    if (request) {
      cancelled.add(params.requestId);
      request.controller.abort();
    }
    return {};
  },
  ping() {
    return {};
  },
};

let buffer = "";
process.stdin.setEncoding("utf8");
process.stdin.on("data", (chunk) => {
  buffer += chunk;
  let idx;
  while ((idx = buffer.indexOf("\n")) !== -1) {
    const line = buffer.slice(0, idx).trim();
    buffer = buffer.slice(idx + 1);
    if (!line) continue;
    handleLine(line);
  }
});
async function releaseControl({ releaseOnly = false } = {}) {
  let timer;
  try {
    await Promise.race([
      (async () => {
        await dispatch;
        await withSignal(null, () => Promise.all([
          closeAppSession({ releaseOnly }),
          ...[...backendCache.values()].map(async ({ backend }) => {
            await backend?.releaseInput?.();
            if (!releaseOnly) await backend?.closeSession?.();
          }),
        ]));
      })(),
      new Promise((_, reject) => { timer = setTimeout(() => reject(new Error("Computer input cleanup did not finish within 3 seconds")), 3_000); }),
    ]);
  } finally { clearTimeout(timer); }
}

let shuttingDown = false;
async function shutdown() {
  if (shuttingDown) return;
  shuttingDown = true;
  for (const request of requests.values()) request.controller.abort();
  try { await releaseControl(); }
  catch (err) { process.stderr.write(`Computer input cleanup failed: ${err?.message ?? err}\n`); }
  process.exit(0);
}
process.stdin.on("end", shutdown);
for (const signal of ["SIGTERM", "SIGINT", "SIGHUP"]) process.on(signal, shutdown);

async function handleLine(line) {
  if (shuttingDown) return;
  const msg = tryJson(line, null);
  if (!msg || typeof msg !== "object") return;
  const { id, method, params } = msg;
  if (!method) return; // response to a server request — we never issue any
  const handler = HANDLERS[method];
  if (!handler) {
    if (id != null) respondError(id, -32601, `method not found: ${method}`);
    return;
  }
  // Cancelled before dispatch: per MCP, respond nothing.
  if (id != null && cancelled.has(id)) { cancelled.delete(id); return; }
  const controller = new AbortController();
  if (id != null) requests.set(id, { controller, name: method === "tools/call" ? params?.name : null });
  try {
    const result = await withSignal(controller.signal, () => handler(params));
    // Cancelled mid-flight: drop the completed response.
    if (id != null) {
      if (cancelled.has(id)) { cancelled.delete(id); return; }
      respond(id, result);
    }
  } catch (err) {
    if (id != null && !cancelled.delete(id)) respondError(id, Number.isInteger(err?.rpcCode) ? err.rpcCode : -32603, err?.message ?? String(err));
  } finally {
    if (id != null) requests.delete(id);
  }
}

// Notifications we must tolerate
["notifications/initialized", "initialized"].forEach((m) => { if (!HANDLERS[m]) HANDLERS[m] = () => ({}); });
