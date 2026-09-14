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
import path from "node:path";
import { handle, closeSession, closeAllSessions, releaseSessionInput, reopenSession, ALLOWED, controlStatus, setControlMode } from "../src/app-handler.mjs";
import { runBackgroundCheck } from "./background-check.mjs";
import { checkForUpdate, prepareUpdate, restartWithUpdate, readUpdateResult } from "./updates.mjs";
import { APP_ID, APP_NAME, APP_VERSION, socketPath, runInfoPath, writeRegistration, defaultLaunch, hello } from "../src/app-socket.mjs";
import { stateDir } from "../src/registry.mjs";

const startedAt = new Date().toISOString();
const bundle = process.env.CODEWHALE_CU_APP_BUNDLE || null;
let controlOwner = false;
const log = (msg) => process.stderr.write(`${new Date().toISOString()} ${APP_NAME}: ${msg}\n`);

function appInfo() {
  return { id: APP_ID, name: APP_NAME, version: APP_VERSION, sessionProtocol: 2, backgroundProtocol: 1, controlProtocol: 1, controlOwner, pid: process.pid, platform: process.platform, node: process.version, bundle, startedAt, socket: socketPath() };
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
let backgroundCheck = null;
let checking = false;
let update = readUpdateResult();
let updating = false;
let controlError = null;
const controlFile = path.join(stateDir(), "control.json");
async function userControl(mode) {
  const work = setControlMode(mode);
  if (mode === "ready") await work;
  const temporary = `${controlFile}.${process.pid}.tmp`;
  let storageError;
  try {
    fs.writeFileSync(temporary, JSON.stringify({ mode }), { mode: 0o600 });
    fs.renameSync(temporary, controlFile);
  } catch (error) { storageError = error; }
  const result = await work;
  if (storageError) throw new Error("Control changed, but its restart preference could not be saved. Check disk space before reopening the app.");
  return result;
}
try {
  const saved = JSON.parse(fs.readFileSync(controlFile, "utf8"));
  if (saved.mode !== "ready") await setControlMode(["paused", "stopped"].includes(saved.mode) ? saved.mode : "stopped");
} catch (error) { if (error.code !== "ENOENT") await setControlMode("stopped"); }

// An inherited socketpair joins the menu-bar owner and its child. This has
// no filesystem endpoint, no reusable credential and no MCP equivalent.
// Losing the human control process fails closed before accepting more work.
if (process.env.CODEWHALE_CU_CONTROL_FD === "3") {
  const control = new net.Socket({ fd: 3, readable: true, writable: true });
  controlOwner = true;
  delete process.env.CODEWHALE_CU_CONTROL_FD;
  let input = "";
  control.setEncoding("utf8");
  const status = () => ({ ...controlStatus(), version: APP_VERSION, checking, backgroundCheck, error: controlError,
    update: update ? { available: update.available, version: update.version, message: update.message, busy: updating } : null });
  const send = (id, error) => {
    if (error) controlError = String(error.message ?? error).slice(0, 400);
    if (!control.destroyed) control.write(JSON.stringify({ id, ...status() }) + "\n");
  };
  control.on("data", chunk => {
    input += chunk;
    if (input.length > 8192) { control.destroy(); return; }
    let newline;
    while ((newline = input.indexOf("\n")) >= 0) {
      const line = input.slice(0, newline); input = input.slice(newline + 1);
      let request;
      try { request = JSON.parse(line); } catch { continue; }
      const { id, command } = request;
      if (command === "status") send(id);
      else if (["pause", "resume", "stop"].includes(command)) {
        const mode = { pause: "paused", resume: "ready", stop: "stopped" }[command];
        userControl(mode).then(() => {
          controlError = null;
          if (mode === "stopped") {
            // Keep old owner sockets alive but invalidate their leases. An
            // already queued request can never silently obtain a fresh one.
            for (const lease of leases.values()) lease.stopped = true;
          }
          send(id);
        }).catch(error => send(id, error));
      } else if (command === "check") {
        if (checking || controlStatus().sessions.some(s => s.action)) { send(id, new Error("Wait for the current action to finish before running the check.")); continue; }
        checking = true; backgroundCheck = null; send(id);
        runBackgroundCheck({ bundle }).then(result => { backgroundCheck = result; }).catch(error => {
          backgroundCheck = { ok: false, message: error.message };
        }).finally(() => { checking = false; send(id); });
      } else if (command === "updates" && !updating) {
        updating = true; update = { available: false, message: "Checking for updates…" }; send(id);
        checkForUpdate().then(result => { update = result; }).catch(error => { update = { available: false, message: error.message }; }).finally(() => { updating = false; send(id); });
      } else if (command === "install_update" && update?.available && !updating && bundle) {
        updating = true; update.message = "Downloading and verifying the update…"; send(id);
        prepareUpdate(update).then(async prepared => {
          await userControl("stopped");
          await restartWithUpdate(prepared, bundle);
          update.message = "Restarting Computer Use…"; send(id);
        }).catch(error => { updating = false; update.message = error.message; send(id); });
      } else send(id, new Error("Unknown or unavailable control command"));
    }
  });
  control.on("error", () => {});
  control.on("close", () => {
    controlOwner = false;
    // Persist the stop and abort active input synchronously, then retire the
    // listener. Reopening the menu app must be able to start a new owner.
    userControl("stopped").catch(error => log(`control owner cleanup: ${error.message}`));
    shutdown("control owner disconnected");
  });
}
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
        } else if (leases.get(req.sessionId).stopped && !["close_session", "release_session_input"].includes(req.tool)) {
          reply = { ok: false, error: { code: "control_stopped", message: "The user stopped this computer session. Start a new task after they allow control in the menu bar." } };
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
});

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
  try {
    if (JSON.parse(fs.readFileSync(runInfoPath(), "utf8")).pid === process.pid) fs.unlinkSync(runInfoPath());
  } catch {}
  // net.Server owns its Unix socket and removes it on close. Cleanup may
  // finish after a replacement has bound the path; never unlink its socket.
  process.exit(0);
}
for (const s of ["SIGINT", "SIGTERM", "SIGHUP"]) process.on(s, () => shutdown(s));
