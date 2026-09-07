#!/usr/bin/env node
// codewhale-cu MCP server — zero-dependency JSON-RPC 2.0 over stdio.
// One tool surface, four platforms (darwin, win32, linux, harmonyos), with
// computer switching as a default: every tool accepts `computer`, and using a
// computer id switches the sticky active computer.
import fs from "node:fs";
import * as registry from "../src/registry.mjs";
import { backendFor, installRemoteAgent, executorFor } from "../src/transport.mjs";
import { TOOLS, TOOL_NAMES, READ_ONLY_TOOLS, REMOTE_TOOLS, BACKEND_METHOD } from "../src/tools.mjs";
import { tryJson } from "../src/exec.mjs";

const VERSION = "0.2.0";
const SERVER_NAME = "codewhale-cu";

// ---------- per-session runtime state ----------
let controlStopped = false;
let stateCounter = 0;
let inFlight = 0; // actions currently dispatching to a backend/executor
/** request ids cancelled via notifications/cancelled */
const cancelled = new Set();
/** state_id -> { computerId, app_ref, windowIndex, elements } */
const appStates = new Map();
/** computerId -> last raster metadata {file, scale, origin} */
const lastRasters = new Map();
/** computerId -> cached backend (local/hdc only) */
const backendCache = new Map();

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

async function getBackend(computer) {
  const key = computer.id;
  if (computer.transport === "local" || computer.transport === "hdc") {
    if (!backendCache.has(key)) {
      const { backend } = await backendFor(computer);
      backendCache.set(key, backend);
    }
    return backendCache.get(key);
  }
  const { backend } = await backendFor(computer);
  return backend;
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
      if (fresh.role && element.role && fresh.role !== element.role) {
        throw new ServerError("element_stale", `element ${target.index} of ${target.state_id} changed role (${element.role} → ${fresh.role}) — call get_app_state again`);
      }
      // In-place replacement: same role and geometry but a different label is
      // still a different element (e.g. "Load" → "Confirm").
      if (typeof fresh.label === "string" && fresh.label.length > 0 &&
          typeof element.label === "string" && element.label.length > 0 &&
          fresh.label !== element.label) {
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
    // Pointer tools on element targets: aim at the (fresh) element center.
    const c = { x: Math.round(pos.x + sz.w / 2), y: Math.round(pos.y + sz.h / 2) };
    return { ...c, strategy: "a11y-center", role: element.role, label: element.label, app_ref: state.app_ref, reacquired: moved };
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

// ---------- tool dispatch ----------
async function callTool(params) {
  const name = params.name;
  if (!TOOL_NAMES.has(name)) {
    return { content: [{ type: "text", text: JSON.stringify({ ok: false, error: { code: "unknown_tool", message: `unknown tool "${name}"` } }) }], isError: true };
  }
  const args = params.arguments ?? {};

  if (name === "stop_computer_control") {
    controlStopped = true;
    return { content: [{ type: "text", text: JSON.stringify(receipt(null, { ok: true, stopped: true, inFlight, note: "Computer control refused for the rest of this session. Restart the session or the codewhale-cu server to continue. Actions already dispatching may still land." })) }] };
  }
  if (controlStopped && !READ_ONLY_TOOLS.has(name)) {
    return { content: [{ type: "text", text: JSON.stringify(fail(null, "control_stopped", "stop_computer_control is active; no further actions are permitted this session")) }], isError: true };
  }

  if (name === "wait") {
    const s = Math.max(0, Math.min(30, Number(args.seconds) || 1));
    await new Promise((r) => setTimeout(r, s * 1000));
    return { content: [{ type: "text", text: JSON.stringify(receipt(null, { ok: true, waitedSec: s })) }] };
  }

  if (name === "computer_list") {
    const reg = registry.list();
    return { content: [{ type: "text", text: JSON.stringify(receipt(null, {
      ok: true,
      active: reg.active,
      computers: Object.values(reg.computers).map((c) => ({ id: c.id, transport: c.transport, platform: c.platform ?? c.platformHint ?? null, label: c.label ?? null, host: c.host ?? null })),
      note: "Pass `computer` on any tool to switch (sticky), or computer_switch to switch explicitly.",
    })) }] };
  }

  if (name === "computer_register") {
    try {
      const entry = registry.register({ id: args.computer, transport: args.transport, label: args.label, host: args.host, port: args.port, user: args.user, target: args.target });
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
      return { content: [{ type: "text", text: JSON.stringify(receipt(null, { ok: true, registered: { ...fresh, platform: fresh.platform ?? fresh.platformHint ?? null }, agentInstall: installed })) }] };
    } catch (err) {
      // Registration problems (unreachable host, agent push failed) are
      // receipts, not protocol errors.
      return { content: [{ type: "text", text: JSON.stringify(fail(null, err.code ?? "register_failed", err.message ?? String(err))) }], isError: true };
    }
  }

  if (name === "computer_remove") {
    const res = registry.remove(args.computer);
    backendCache.delete(args.computer);
    lastRasters.delete(args.computer);
    return { content: [{ type: "text", text: JSON.stringify(receipt(null, { ok: true, ...res })) }] };
  }

  if (name === "computer_switch") {
    const c = registry.switchTo(args.computer);
    return { content: [{ type: "text", text: JSON.stringify(receipt(c, { ok: true, active: c.id })) }] };
  }

  // Everything below acts on a computer.
  let computer;
  let switched = false;
  try {
    if (args.computer && args.computer !== registry.list().active) {
      computer = registry.switchTo(args.computer);
      switched = true;
    } else {
      computer = registry.active();
    }
  } catch (err) {
    return { content: [{ type: "text", text: JSON.stringify(fail(null, err.code ?? "registry_error", err.message)) }], isError: true };
  }

  try {
    // Out-of-process runners (the desktop app for the local computer, the
    // remote agent for ssh computers) get the request over the wire.
    const backendMethod = BACKEND_METHOD[name] === "request_access" ? "probe" : BACKEND_METHOD[name];
    let data;
    const ex = computer.transport === "local" || computer.transport === "ssh" ? await executorFor(computer) : null;
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
      // Re-check the kill switch: a stop that arrived while the executor was
      // being resolved still blocks this dispatch.
      if (controlStopped && !READ_ONLY_TOOLS.has(name)) throw new ServerError("control_stopped", "stop_computer_control is active; no further actions are permitted this session");
      inFlight++;
      let reply;
      try {
        reply = await ex.remote({ tool: backendMethod, args: wireArgs }, { timeoutMs: backendMethod.startsWith("recording") || backendMethod === "get_app_state" ? 60_000 : 30_000 });
      } finally {
        inFlight--;
      }
      if (!reply.ok) throw new ServerError(reply.error?.code ?? "remote_error", reply.error?.message ?? "remote agent failed");
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
        data.state_id = rememberState(computer, wireArgs.app_ref, data);
        data.note = "Element targets are {type:'element', state_id, index}. State goes stale when the UI changes; observe again.";
      }
      if (backendMethod === "probe") Object.assign(data, { via: ex.kind, app: ex.app ?? null });
    } else {
      const backend = await getBackend(computer);
      if (typeof backend[backendMethod] !== "function") {
        throw new ServerError("unsupported_on_backend", `"${name}" is not implemented on the ${computer.platform ?? computer.transport} backend`);
      }
      const resolve = typeof backend.resolve_element === "function" ? (req) => backend.resolve_element(req) : null;
      const prepared = await prepareArgs(computer, name, args, resolve, sink);
      if (controlStopped && !READ_ONLY_TOOLS.has(name)) throw new ServerError("control_stopped", "stop_computer_control is active; no further actions are permitted this session");
      inFlight++;
      try {
        data = await backend[backendMethod](prepared);
      } finally {
        inFlight--;
      }
      if (Array.isArray(data)) data = { items: data }; // keep receipts objects
      if (name === "screenshot") bindRaster(computer, data);
      if (backendMethod === "zoom") bindZoomRaster(computer, zoomParent, args.region, data?.file ?? data?.path);
      if (name === "get_app_state") {
        const stateId = rememberState(computer, prepared.app_ref, data);
        data.state_id = stateId;
        data.note = "Element targets are {type:'element', state_id, index}. State goes stale when the UI changes; observe again.";
      }
      if (backendMethod === "probe" && computer.transport === "local") {
        // Direct mode: permissions belong to whatever hosts this server. Say so.
        Object.assign(data, { via: "direct", app: null, appHint: ex?.appReason ?? null });
      }
    }

    const content = [{ type: "text", text: JSON.stringify(receipt(computer, { ok: true, tool: name, switched, ...(sink.reacquired ? { target_reacquired: true } : {}), ...data })) }];
    if ((name === "screenshot" || name === "zoom") && computer.transport === "local" && (data.file || data.path)) {
      const bytes = fs.readFileSync(data.file || data.path);
      content.push({ type: "image", mimeType: bytes[0] === 0xff ? "image/jpeg" : "image/png", data: bytes.toString("base64") });
    }
    return { content };
  } catch (err) {
    return { content: [{ type: "text", text: JSON.stringify(fail(computer, err.code ?? "tool_error", err.message ?? String(err), { tool: name, switched })) }], isError: true };
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
    out.app_ref = out.app_ref ?? null;
    if (out.window_id != null) out.window_id = Number(out.window_id);
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
      serverInfo: { name: SERVER_NAME, version: VERSION, platforms: ["darwin", "win32", "linux", "harmonyos"], transports: ["local", "ssh", "hdc"] },
    };
  },
  "tools/list"() {
    return { tools: TOOLS };
  },
  async "tools/call"(params) {
    return await callTool(params ?? {});
  },
  "notifications/cancelled"(params) {
    if (params?.requestId != null) cancelled.add(params.requestId);
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
process.stdin.on("end", () => process.exit(0));

async function handleLine(line) {
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
  try {
    const result = await handler(params);
    // Cancelled mid-flight: drop the completed response.
    if (id != null) {
      if (cancelled.has(id)) { cancelled.delete(id); return; }
      respond(id, result);
    }
  } catch (err) {
    if (id != null && !cancelled.delete(id)) respondError(id, -32603, err?.message ?? String(err));
  }
}

// Notifications we must tolerate
["notifications/initialized", "initialized"].forEach((m) => { if (!HANDLERS[m]) HANDLERS[m] = () => ({}); });
