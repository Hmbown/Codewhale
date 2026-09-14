#!/usr/bin/env node
// codewhale-cu MCP server — zero-dependency JSON-RPC 2.0 over stdio.
// One tool surface, four platforms (darwin, win32, linux, harmonyos), with
// computer switching as a default: every tool accepts `computer`, and using a
// computer id switches the sticky active computer.
import fs from "node:fs";
import * as registry from "../src/registry.mjs";
import { backendFor, installRemoteAgent, executorFor, closeAppSession, routeFingerprint } from "../src/transport.mjs";
import { TOOLS, TOOL_NAMES, READ_ONLY_TOOLS, REMOTE_TOOLS, BACKEND_METHOD } from "../src/tools.mjs";
import { tryJson, withSignal, throwIfAborted, wait } from "../src/exec.mjs";
import { APP_VERSION } from "../src/app-socket.mjs";

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
/** state_id -> { computerId, app_ref, windowIndex, elements } */
const appStates = new Map();
/** computerId -> last raster metadata {file, scale, origin} */
const lastRasters = new Map();
/** computerId -> route-bound session resources; the registry owns configuration. */
const backendCache = new Map();
const ROUTE_INSPECTION_TOOLS = new Set([
  "request_access", "list_displays", "list_apps", "list_windows", "get_app_state", "screenshot",
  "cursor_position", "read_clipboard", "recording_list", "recording_status",
]);
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
  for (const [stateId, state] of appStates) {
    if (state.computerId === id) appStates.delete(stateId);
  }
}

async function retireBinding(id) {
  const binding = backendCache.get(id);
  invalidateObservations(id);
  if (!binding) return;
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

/** Element target -> enriched target with cached app identity and AX path. */
function resolveElement(target) {
  const st = appStates.get(target.state_id);
  if (!st) throw new ServerError("unknown_state", `state_id "${target.state_id}" is unknown or expired — call get_app_state again`);
  const el = st.elements[target.index];
  if (!el) throw new ServerError("unknown_element", `element index ${target.index} is outside state ${target.state_id} (0..${st.elements.length - 1})`);
  return { state: st, element: el };
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
    const pt = rasterToPoints(computer.id, target.x, target.y);
    return { x: Math.round(pt.x), y: Math.round(pt.y), strategy: "event" };
  }
  if (target?.type === "element") {
    const { state, element } = resolveElement(target);
    if (state.computerId && state.computerId !== computer.id) {
      throw new ServerError("state_wrong_computer", `state_id "${target.state_id}" belongs to computer "${state.computerId}", not "${computer.id}" — call get_app_state on that computer again`);
    }
    let fresh = null;
    if (resolve) {
      const res = await resolve({ app_ref: state.app_ref, windowIndex: element.windowIndex ?? 0, path: element.path });
      if (!res?.found || !res.element) {
        throw new ServerError("element_stale", `element ${target.index} of ${target.state_id} no longer resolves (${res?.reason ?? "not_found"}) — call get_app_state again`);
      }
      fresh = res.element;
      if (fresh.role !== element.role) {
        throw new ServerError("element_stale", `element ${target.index} of ${target.state_id} changed role (${element.role} → ${fresh.role}) — call get_app_state again`);
      }
      // In-place replacement: same role and geometry but a different label is
      // still a different element (e.g. "Load" → "Confirm").
      if (fresh.label !== element.label) {
        throw new ServerError("element_stale", `element ${target.index} of ${target.state_id} changed label (${element.label} → ${fresh.label}) — call get_app_state again`);
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
    if (!pos || !sz) throw new ServerError("element_no_geometry", `element ${target.index} has no cached geometry — use a coordinate target`);
    if (moved && sink) sink.reacquired = true;
    // Keep the element identity as well as geometry: semantic clicks must not
    // substitute whichever element happens to occupy an oversized AX center.
    const c = { x: Math.round(pos.x + sz.w / 2), y: Math.round(pos.y + sz.h / 2) };
    return { ...c, strategy: "a11y-center", role: element.role, label: element.label, app_ref: state.app_ref,
      windowIndex: element.windowIndex ?? 0, path: element.path, reacquired: moved };
  }
  throw new ServerError("bad_target", "target must be {type:'coordinate',x,y} or {type:'element',state_id,index}");
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
  if (appStates.size > 24) {
    for (const k of appStates.keys()) { appStates.delete(k); break; }
  }
  return id;
}

function observeState(computer, app_ref, result, detail) {
  // Cache the complete backend records before making the model-facing view.
  // Public indices still address those records, including their private AX
  // paths; a compact response must never weaken live target revalidation.
  const state_id = rememberState(computer, app_ref, result);
  const full = detail === "full";
  const elements = full ? result.elements : (result.elements ?? [])
    .filter((el) => el.windowIndex !== -1 || !Array.isArray(el.path) || el.path.length <= 1)
    .map(({ path, windowIndex, ...el }) => el);
  return {
    ...result, state_id, elements, detail: full ? "full" : "summary",
    note: "Target observed elements with {type:'element', state_id, index}; observe again after UI changes. " +
      (full ? "" : "Summary keeps app content and top-level menus; use detail:'full' for nested menus and tree structure. ") +
      "Missing labels or values are unknown; do not guess their contents.",
  };
}

// ---------- tool dispatch ----------
async function callTool(params) {
  const name = params.name;
  if (!TOOL_NAMES.has(name)) {
    return { content: [{ type: "text", text: JSON.stringify({ ok: false, error: { code: "unknown_tool", message: `unknown tool "${name}"` } }) }], isError: true };
  }
  const args = params.arguments ?? {};

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
    // Out-of-process runners (the desktop app for the local computer, the
    // remote agent for ssh computers) get the request over the wire.
    const backendMethod = BACKEND_METHOD[name] === "request_access" ? "probe" : BACKEND_METHOD[name];
    let data;
    const ex = computer.transport === "local" || computer.transport === "ssh" ? await executorFor(computer) : null;
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
      const resolve = async (req) => {
        const rep = await ex.remote({ tool: "resolve_element", args: req }, { timeoutMs: 30_000 });
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
      let reply;
      try {
        dispatched = true;
        reply = await ex.remote({ tool: backendMethod, args: wireArgs }, { timeoutMs: backendMethod.startsWith("recording") || backendMethod === "get_app_state" ? 60_000 : 30_000 });
      } finally {
        inFlight--;
      }
      if (!reply.ok) throw new ServerError(reply.error?.code ?? "remote_error", reply.error?.message ?? "remote agent failed");
      await assertCurrentRoute(computer, binding, true);
      data = reply.data;
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
        data = observeState(computer, wireArgs.app_ref, data, args.detail);
      }
      if (backendMethod === "probe") Object.assign(data, { via: ex.kind, app: ex.app ?? null });
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
        data = await backend[backendMethod](prepared);
      } finally {
        inFlight--;
      }
      await assertCurrentRoute(computer, binding, true);
      if (Array.isArray(data)) data = { items: data }; // keep receipts objects
      if (name === "screenshot") bindRaster(computer, data);
      if (backendMethod === "zoom") bindZoomRaster(computer, zoomParent, args.region, data?.file ?? data?.path);
      if (name === "get_app_state") {
        data = observeState(computer, prepared.app_ref, data, args.detail);
      }
      if (backendMethod === "probe" && computer.transport === "local") {
        // Direct mode: permissions belong to whatever hosts this server. Say so.
        Object.assign(data, { via: "direct", app: null, appHint: ex?.appReason ?? null });
      }
    }

    if (name === "get_app_state" && args.include_ocr) {
      data.ocr ??= { status: "unavailable", reason: "Text recognition is not available on this backend", blocks: [] };
      if (data.ocr.raster) {
        const localFile = typeof ex?.remote !== "function" || ex.filesLocal;
        bindRaster(computer, localFile ? data.ocr.raster : { ...data.ocr.raster, file: null, path: null });
      }
      data.ocr.note = "Recognized text may be imperfect. These coordinate targets belong to this captured image, not to accessibility elements; observe again after the UI changes.";
    }

    // Inline the raster only when it fits the budget. One oversized JSON-RPC
    // message drops the whole stdio transport and every other tool with it, so
    // an over-budget capture degrades to its text receipt: the file is still on
    // disk and still bound, so zoom or a narrower capture returns a viewable
    // image. Never trade the session for one screenshot.
    let imageBlock = null;
    if ((name === "screenshot" || name === "zoom") && computer.transport === "local" && (data.file || data.path)) {
      const file = data.file || data.path;
      const size = fs.statSync(file).size;
      if (encodedSize(size) > INLINE_IMAGE_MAX_BYTES) {
        data.image_omitted = {
          reason: "raster_too_large",
          bytes: size,
          encoded_bytes: encodedSize(size),
          limit_bytes: INLINE_IMAGE_MAX_BYTES,
          note: "The capture is on disk at the returned path, but inlining it would exceed this host's single-message budget and drop the connection. Capture one display, a region, or an app window, or zoom into part of this raster to get a viewable image.",
        };
      } else {
        const bytes = fs.readFileSync(file);
        imageBlock = { type: "image", mimeType: bytes[0] === 0xff ? "image/jpeg" : "image/png", data: bytes.toString("base64") };
      }
    }
    const content = [{ type: "text", text: JSON.stringify(receipt(computer, { ok: true, tool: name, switched, ...(sink.reacquired ? { target_reacquired: true } : {}), ...data })) }];
    if (imageBlock) content.push(imageBlock);
    if ((name === "screenshot" && (data?.file || data?.path) && data?.pixels?.w > 0 && data?.pixels?.h > 0) ||
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
    return { content: [{ type: "text", text: JSON.stringify(fail(computer, err.code ?? "tool_error", err.message ?? String(err), {
      tool: name, switched,
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
  const semantic = new Set(["set_value", "select_text", "perform_action"]);
  for (const key of ["target", "from_target", "to"]) {
    const given = out[key];
    if (!given?.type) continue;
    const kind = key === "target" && semantic.has(name) ? "semantic" : "pointer";
    out[key] = { ...given, ...(await normalizeTarget(computer, given, kind, resolve, sink)) };
  }
  if (name === "get_app_state") {
    if (out.detail != null && !["summary", "compact", "full"].includes(out.detail)) throw new ServerError("bad_args", "detail must be summary or full (compact is an alias for summary)");
    out.detail = out.detail === "full" ? "full" : "summary";
    if (out.include_ocr != null && typeof out.include_ocr !== "boolean") throw new ServerError("bad_args", "include_ocr must be true or false");
    if (out.window_id != null && (!Number.isSafeInteger(out.window_id) || out.window_id < 0)) throw new ServerError("bad_args", "window_id must be a non-negative window index from list_windows");
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

const HANDLERS = {
  initialize(params) {
    return {
      protocolVersion: params?.protocolVersion ?? "2025-06-18",
      capabilities: { tools: { listChanged: false } },
      serverInfo: { name: SERVER_NAME, version: APP_VERSION, platforms: ["darwin", "win32", "linux", "harmonyos"], transports: ["local", "ssh", "hdc"] },
    };
  },
  "tools/list"() {
    return { tools: TOOLS };
  },
  async "tools/call"(params) {
    if (params?.name === "stop_computer_control") return callTool(params);
    const previous = dispatch;
    let release;
    dispatch = new Promise((resolve) => { release = resolve; });
    try {
      await previous;
      throwIfAborted();
      return await callTool(params ?? {});
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
    if (id != null && !cancelled.delete(id)) respondError(id, -32603, err?.message ?? String(err));
  } finally {
    if (id != null) requests.delete(id);
  }
}

// Notifications we must tolerate
["notifications/initialized", "initialized"].forEach((m) => { if (!HANDLERS[m]) HANDLERS[m] = () => ({}); });
