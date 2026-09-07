// The desktop app is the local computer's out-of-process runner: a long-lived
// daemon that owns the OS permissions (macOS Accessibility / Screen Recording
// are granted to *it*, not to whichever terminal hosts the MCP server) and
// answers {tool, args} requests over a per-user local socket. This module is
// the client side plus the shared naming; app/daemon.mjs is the server side.
//
// Wire format: one JSON object per line, request then reply, same shape as
// the ssh remote agent. Only ALLOWED tools (src/app-handler.mjs) execute.
import fs from "node:fs";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import url from "node:url";
import crypto from "node:crypto";
import { spawn } from "node:child_process";
import { stateDir } from "./registry.mjs";
import { ExecError } from "./exec.mjs";

export const PLUGIN_ROOT = path.resolve(path.dirname(url.fileURLToPath(import.meta.url)), "..");
export const APP_ID = "net.codewhale.computer-use";
export const APP_NAME = "Codewhale Computer Use";
export const APP_VERSION = JSON.parse(fs.readFileSync(path.join(PLUGIN_ROOT, "plugin.json"), "utf8")).version;

function shortHash(s) {
  return crypto.createHash("sha256").update(s).digest("hex").slice(0, 12);
}

/** Per-user socket endpoint, keyed by the state dir so isolated state dirs get isolated apps. */
export function socketPath() {
  if (process.env.CODEWHALE_CU_APP_SOCKET) return process.env.CODEWHALE_CU_APP_SOCKET;
  const dir = stateDir();
  if (process.platform === "win32") return `\\\\.\\pipe\\codewhale-cu-${shortHash(dir)}`;
  const preferred = path.join(dir, "app.sock");
  // sun_path is 104 bytes on macOS / 108 on Linux; fall back to a short tmp name.
  return Buffer.byteLength(preferred) < 100 ? preferred : path.join(os.tmpdir(), `codewhale-cu-${shortHash(dir)}.sock`);
}

/** Where the app records how to launch itself (written by the app on first launch). */
export function registrationPath() { return path.join(stateDir(), "app.json"); }
/** Where the running daemon records its pid/socket (written on listen, removed on exit). */
export function runInfoPath() { return path.join(stateDir(), "app-run.json"); }

export function readRegistration() {
  try {
    const reg = JSON.parse(fs.readFileSync(registrationPath(), "utf8"));
    if (!Array.isArray(reg?.launch) || reg.launch.length === 0 || typeof reg.launch[0] !== "string") return null;
    return reg;
  } catch { return null; }
}

export function writeRegistration(reg) {
  fs.mkdirSync(stateDir(), { recursive: true });
  fs.writeFileSync(registrationPath(), JSON.stringify({ ...reg, registeredAt: new Date().toISOString() }, null, 2) + "\n");
}

/** Send one request to the app and await its single-line reply. */
export function appRequest(request, { timeoutMs = 30_000 } = {}) {
  return new Promise((resolve, reject) => {
    const sock = net.connect(socketPath());
    let buf = "";
    let settled = false;
    const done = (fn, v) => { if (settled) return; settled = true; clearTimeout(timer); sock.destroy(); fn(v); };
    const timer = setTimeout(() => done(reject, new ExecError(`${APP_NAME}: request timed out after ${timeoutMs}ms`, { code: "app_timeout" })), timeoutMs);
    sock.on("error", (err) => done(reject, Object.assign(new ExecError(`${APP_NAME} is not reachable at ${socketPath()}: ${err.code ?? err.message}`), { code: "app_unavailable" })));
    sock.on("connect", () => sock.write(JSON.stringify(request) + "\n"));
    sock.on("data", (d) => {
      buf += d.toString("utf8");
      const nl = buf.indexOf("\n");
      if (nl === -1) return;
      try { done(resolve, JSON.parse(buf.slice(0, nl))); }
      catch { done(reject, Object.assign(new ExecError(`${APP_NAME}: malformed reply`), { code: "app_bad_reply" })); }
    });
    sock.on("close", () => done(reject, Object.assign(new ExecError(`${APP_NAME}: connection closed before a reply`), { code: "app_unavailable" })));
  });
}

/** App identity if it is running, else null. Cheap: one connect. */
export async function hello({ timeoutMs = 2_000 } = {}) {
  try {
    const r = await appRequest({ tool: "hello" }, { timeoutMs });
    return r?.ok && r.app ? r.app : null;
  } catch { return null; }
}

/** How each OS re-launches an installed bundle so it is its own responsible process. */
export function defaultLaunch(bundlePath, platform = process.platform) {
  if (platform === "darwin") return ["open", "-g", "-a", bundlePath];
  if (platform === "win32") return ["powershell.exe", "-NoProfile", "-WindowStyle", "Hidden", "-ExecutionPolicy", "Bypass", "-File", path.join(bundlePath, "launch.ps1")];
  return [path.join(bundlePath, "bin", "codewhale-computer-use")];
}

/** Start the registered app detached (LaunchServices on macOS so TCC attributes it to the app). */
export function launchApp(reg) {
  const [cmd, ...args] = reg.launch;
  const child = spawn(cmd, args, { detached: true, stdio: "ignore", windowsHide: true });
  child.on("error", () => {});
  child.unref();
  return child.pid ?? null;
}

let lastLaunchAt = 0;

/**
 * Decide how the local computer is driven this call: through the app when it
 * is running (or registered and launchable), otherwise directly from this
 * process. Set CODEWHALE_CU_APP=off to force direct.
 */
export async function ensureApp({ launch = true } = {}) {
  if (process.env.CODEWHALE_CU_APP === "off") return { via: "direct", reason: "CODEWHALE_CU_APP=off" };
  let app = await hello();
  if (app) return { via: "app", app };
  const reg = readRegistration();
  if (!reg) {
    return { via: "direct", reason: `${APP_NAME} is not installed on this computer; run "npm run build:app && npm run install:app" in the plugin so OS permissions belong to the app instead of the host terminal` };
  }
  if (!launch || Date.now() - lastLaunchAt < 15_000) {
    return { via: "direct", reason: `${APP_NAME} is installed at ${reg.path} but not running (last launch attempt did not come up)` };
  }
  lastLaunchAt = Date.now();
  launchApp(reg);
  const deadline = Date.now() + 8_000;
  while (Date.now() < deadline) {
    await new Promise((r) => setTimeout(r, 250));
    app = await hello({ timeoutMs: 1_000 });
    if (app) return { via: "app", app, launched: true };
  }
  return { via: "direct", reason: `${APP_NAME} at ${reg.path} did not answer within 8s of launch; open it manually and check ${runInfoPath()}` };
}
