#!/usr/bin/env node
// Codewhale Computer Use — the desktop app process.
//
// A long-lived daemon that runs the platform backend on this machine and
// answers one-line JSON requests over a per-user local socket (see
// src/app-socket.mjs). The app bundles built by scripts/build-app.mjs launch
// exactly this file, so the OS attributes every osascript / screencapture /
// UI-automation call to the app: grant Accessibility and Screen Recording to
// "Codewhale Computer Use" once and every host that speaks to the plugin
// inherits it.
//
// Env (set by the launchers inside the bundles):
//   CODEWHALE_CU_APP_BUNDLE   absolute path of the installed bundle
//   CODEWHALE_CU_APP_LAUNCH   JSON argv that re-launches the bundle detached
//   CODEWHALE_CU_STATE_DIR    state dir (defaults to ~/.codewhale-cu)
import fs from "node:fs";
import net from "node:net";
import crypto from "node:crypto";
import { handle, closeSession, closeAllSessions, releaseSessionInput, reopenSession, ALLOWED } from "../src/app-handler.mjs";
import { APP_ID, APP_NAME, APP_VERSION, socketPath, runInfoPath, writeRegistration, defaultLaunch, hello } from "../src/app-socket.mjs";
import { stateDir } from "../src/registry.mjs";

const startedAt = new Date().toISOString();
const bundle = process.env.CODEWHALE_CU_APP_BUNDLE || null;
const log = (msg) => process.stderr.write(`${new Date().toISOString()} ${APP_NAME}: ${msg}\n`);

function appInfo() {
  return { id: APP_ID, name: APP_NAME, version: APP_VERSION, sessionProtocol: 2, backgroundProtocol: 1, pid: process.pid, platform: process.platform, node: process.version, bundle, startedAt, socket: socketPath() };
}

if (await hello({ timeoutMs: 1_500 })) {
  log(`already running on ${socketPath()}; exiting`);
  process.exit(0);
}

const sock = socketPath();
fs.mkdirSync(stateDir(), { recursive: true });
if (process.platform !== "win32") {
  try { fs.unlinkSync(sock); } catch {} // stale file from an unclean exit; nobody answered hello above
}

const leases = new Map();
let shuttingDown = false;
async function serve(conn) {
  let buf = "";
  let chain = Promise.resolve();
  const controller = new AbortController();
  let ownedSession = null;
  conn.on("close", () => {
    controller.abort();
    if (ownedSession && leases.get(ownedSession)?.socket === conn) {
      leases.delete(ownedSession);
      closeSession(ownedSession).catch((err) => log(`disconnected session input cleanup failed: ${err.message}`));
    }
  });
  conn.setEncoding("utf8");
  conn.on("error", () => {});
  conn.on("data", (chunk) => {
    buf += chunk;
    let nl;
    while ((nl = buf.indexOf("\n")) !== -1) {
      const line = buf.slice(0, nl).trim();
      buf = buf.slice(nl + 1);
      if (!line) continue;
      chain = chain.then(async () => {
        if (controller.signal.aborted) return;
        let req;
        try { req = JSON.parse(line); } catch { return conn.write(JSON.stringify({ ok: false, error: { code: "bad_payload", message: "request is not JSON" } }) + "\n"); }
        let reply;
        if (shuttingDown) reply = { ok: false, error: { code: "app_shutting_down", message: "Computer Use helper is shutting down" } };
        else if (req?.tool === "hello") reply = { ok: true, app: appInfo() };
        else if (req?.tool === "platform") reply = await handle(req);
        else if (!ALLOWED.has(req?.tool) && !["open_session", "close_session", "release_session_input"].includes(req?.tool)) reply = await handle(req);
        else if (typeof req?.sessionId !== "string" || !/^[A-Za-z0-9_-]{1,128}$/.test(req.sessionId)) {
          reply = { ok: false, error: { code: "session_required", message: "Update the MCP server: every computer request must carry its session identity." } };
        } else if (req.tool === "open_session") {
          if (ownedSession || leases.has(req.sessionId)) reply = { ok: false, error: { code: "session_owned", message: "Computer session already has an owner" } };
          else if (leases.size >= 256) reply = { ok: false, error: { code: "session_limit", message: "Too many active computer sessions" } };
          else {
            ownedSession = req.sessionId;
            const leaseToken = crypto.randomUUID();
            leases.set(ownedSession, { socket: conn, token: leaseToken });
            reopenSession(ownedSession);
            reply = { ok: true, leaseToken };
          }
        } else if (!leases.has(req.sessionId) || leases.get(req.sessionId).token !== req.leaseToken) {
          reply = { ok: false, error: { code: "session_owner_required", message: "Computer request needs its live session owner lease; update or restart the MCP server" } };
        } else if (["close_session", "release_session_input"].includes(req.tool)) {
          try {
            if (req.tool === "close_session") await closeSession(req.sessionId);
            else await releaseSessionInput(req.sessionId);
            reply = { ok: true, closed: req.tool === "close_session", inputReleased: true };
          } catch (err) {
            reply = { ok: false, error: { code: "input_release_failed", message: String(err?.message ?? err) } };
          }
        } else reply = await handle(req, { computerId: "local", sessionId: req.sessionId, signal: controller.signal, persistentInputOwner: true });
        if (!conn.destroyed) conn.write(JSON.stringify(reply) + "\n");
      });
    }
  });
}

const server = net.createServer(serve);
server.on("error", (err) => { log(`socket error: ${err.message}`); process.exit(1); });
server.listen(sock, () => {
  if (process.platform !== "win32") { try { fs.chmodSync(sock, 0o600); } catch {} }
  fs.writeFileSync(runInfoPath(), JSON.stringify(appInfo(), null, 2) + "\n");
  if (bundle) {
    // Launching the bundle once is what registers it: the MCP server reads this
    // record to bring the app up on demand.
    try {
      const launch = process.env.CODEWHALE_CU_APP_LAUNCH ? JSON.parse(process.env.CODEWHALE_CU_APP_LAUNCH) : defaultLaunch(bundle);
      writeRegistration({ id: APP_ID, path: bundle, launch });
    } catch (err) { log(`could not record launch command: ${err.message}`); }
  }
  log(`v${APP_VERSION} listening on ${sock}${bundle ? ` (bundle ${bundle})` : " (bare, no bundle identity)"}`);
  if (bundle && process.env.CODEWHALE_CU_APP_WARM !== "off") warmPermissions();
});

/**
 * Touch every permission-gated capability once so the OS asks for the grants
 * under the app's own name and icon (macOS: Automation → System Events,
 * Accessibility, Screen Recording). Failures are logged, never fatal: the
 * probe fails closed and names the missing grant, which is the point.
 */
async function warmPermissions() {
  for (const tool of ["probe", "list_apps"]) {
    const r = await handle({ tool }, { computerId: "local" });
    log(`warm-up ${tool}: ${r.ok ? JSON.stringify(r.data?.permissions ?? "ok") : `${r.error?.code}: ${r.error?.message}`}`);
  }
}

async function shutdown(signal) {
  if (shuttingDown) return;
  shuttingDown = true;
  log(`${signal}; shutting down`);
  server.close();
  const timer = setTimeout(() => process.exit(1), 3_000);
  const results = await closeAllSessions();
  clearTimeout(timer);
  for (const result of results) {
    if (result.status === "rejected") log(`input cleanup failed: ${result.reason?.message ?? result.reason}`);
  }
  try { fs.unlinkSync(runInfoPath()); } catch {}
  if (process.platform !== "win32") { try { fs.unlinkSync(sock); } catch {} }
  process.exit(0);
}
for (const s of ["SIGINT", "SIGTERM", "SIGHUP"]) process.on(s, () => shutdown(s));
