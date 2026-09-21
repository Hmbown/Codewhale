// Transport: turn a registered computer into an executor.
//  - local: the Codewhale Computer Use app when it is running or registered
//           (it owns the OS permissions), otherwise spawn directly
//  - ssh:   run the codewhale-cu remote agent over ssh (args travel as base64 JSON,
//           so no tool argument can ever become remote shell syntax)
//  - hdc:   HarmonyOS device over `hdc` shell / file push-pull
import { run, runOk, runInputLease, ExecError, currentSignal } from "./exec.mjs";
import { ensureApp, appSessionRequest } from "./app-socket.mjs";
import { spawn } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import os from "node:os";
import url from "node:url";

const __dirname = path.dirname(url.fileURLToPath(import.meta.url));
export const PLUGIN_ROOT = path.resolve(__dirname, "..");
// A new MCP process always starts a fresh app/input/raster binding, even when
// the permission-owning desktop helper remains running across tasks.
export const SESSION_ID = crypto.randomUUID();
let usedApp = false;
let appSessionClosed = false;
export function closeAppSession({ releaseOnly = false } = {}) {
  if (!usedApp || appSessionClosed) return Promise.resolve();
  return appSessionRequest({ tool: releaseOnly ? "release_session_input" : "close_session", sessionId: SESSION_ID }, { timeoutMs: 2_500, signal: null }).then((reply) => {
    if (!reply?.ok) throw Object.assign(new ExecError(reply?.error?.message ?? "Computer input cleanup failed"), { code: reply?.error?.code ?? "input_release_failed" });
    if (!releaseOnly) appSessionClosed = true;
  });
}

export function b64(obj) {
  return Buffer.from(JSON.stringify(obj), "utf8").toString("base64");
}

/**
 * Validate a remote-side filesystem path we construct ourselves.
 * Blocks shell metacharacters and traversal outside the agent dir.
 */
export function safeRemotePath(p) {
  if (typeof p !== "string" || !/^[A-Za-z0-9.][A-Za-z0-9/._-]{0,511}$/.test(p) || p.includes("..")) {
    throw new ExecError(`refusing unsafe remote path: ${JSON.stringify(p)}`);
  }
  return p;
}

/**
 * Local executor bound to a platform backend name.
 * All backends receive this shape.
 */
export function localExec() {
  return {
    kind: "local",
    run,
    runOk,
    runInputLease,
    async readFile(p) { return fs.promises.readFile(p); },
    async writeFile(p, data) { return fs.promises.writeFile(p, data); },
    tmpFile(prefix) {
      return path.join(fs.mkdtempSync(path.join(os.tmpdir(), prefix)), "out");
    },
  };
}

/**
 * App executor: the local computer driven through the desktop app's socket.
 * Same `remote()` contract as ssh, but files the app writes are on this disk.
 */
export function appExec(app, sessionId = SESSION_ID) {
  return {
    ...localExec(),
    kind: "app",
    app,
    filesLocal: true,
    remote(request, opts = {}) {
      if (sessionId === SESSION_ID && appSessionClosed) throw Object.assign(new ExecError("Computer session was closed; start a new MCP session to use the local helper again"), { code: "app_session_closed" });
      usedApp = true;
      return appSessionRequest({ ...request, sessionId }, { timeoutMs: opts.timeoutMs ?? 30_000 });
    },
  };
}

/**
 * A persistent ssh agent channel: one `ssh host node agent.mjs --serve`
 * process carrying base64-JSON request lines in and JSON receipt lines out.
 * Unlike the one-shot agent it keeps its backend alive between calls, so an
 * open_application binding survives into later raw-input calls and held
 * input/recording can be owned by the session. Requests written before the
 * channel dies may already have run remotely — their failures are marked
 * requestDispatched so the server reports outcome_unknown instead of
 * inviting a blind retry.
 */
export function ensureSshChannel(binding, argv) {
  let ch = binding.sshChannel;
  if (ch?.alive) return ch;
  const next = { alive: false, restarted: !!ch, everReplied: false, pending: new Map(), seq: 1, buf: "", proc: null };
  binding.sshChannel = next;
  const failAll = (err) => {
    for (const [, p] of next.pending) { clearTimeout(p.timer); p.reject(err); }
    next.pending.clear();
  };
  let proc;
  try {
    proc = spawn(argv[0], argv.slice(1), { stdio: ["pipe", "pipe", "pipe"], windowsHide: true });
  } catch (err) {
    next.spawnError = err;
    return next;
  }
  next.proc = proc;
  next.alive = true;
  proc.stdin.on("error", () => {});
  proc.stderr.on("data", () => {}); // drain; stderr is never parsed
  proc.stdout.setEncoding("utf8");
  proc.stdout.on("data", (d) => {
    next.buf += d;
    let i;
    while ((i = next.buf.indexOf("\n")) !== -1) {
      const line = next.buf.slice(0, i).trim();
      next.buf = next.buf.slice(i + 1);
      if (!line.startsWith("{")) continue; // MOTD/banner noise
      let msg;
      try { msg = JSON.parse(line); } catch { continue; }
      next.everReplied = true;
      const p = next.pending.get(msg.id);
      if (!p) continue; // timed-out or unknown request: drop the late reply
      next.pending.delete(msg.id);
      clearTimeout(p.timer);
      p.resolve(msg);
    }
  });
  const dead = (why) => {
    if (!next.alive) return;
    next.alive = false;
    failAll(Object.assign(new ExecError(`ssh agent channel closed${why ? `: ${why}` : ""}`), { code: "remote_session_lost", requestDispatched: true }));
  };
  proc.on("error", (err) => dead(String(err?.message ?? err)));
  proc.on("close", (code, sig) => dead(code != null ? `exited ${code}` : `signal ${sig}`));
  return next;
}

export function closeSshChannel(binding) {
  const ch = binding.sshChannel;
  if (!ch) return;
  binding.sshChannel = null;
  ch.alive = false;
  try { ch.proc?.stdin.end(); } catch {}
  try { ch.proc?.kill("SIGTERM"); } catch {}
  for (const [, p] of ch.pending ?? []) {
    clearTimeout(p.timer);
    p.reject(Object.assign(new ExecError("ssh agent channel closed"), { code: "remote_session_lost", requestDispatched: true }));
  }
  ch.pending?.clear();
}

export function channelRequest(ch, request, timeoutMs) {
  return new Promise((resolve, reject) => {
    if (!ch.alive) {
      reject(Object.assign(new ExecError("ssh agent channel is closed"), { code: "remote_session_lost" }));
      return;
    }
    const id = ch.seq++;
    const timer = setTimeout(() => {
      ch.pending.delete(id);
      // The request was written; the remote may still be executing it.
      reject(Object.assign(new ExecError(`ssh agent timed out after ${timeoutMs}ms`), { code: "remote_timeout", requestDispatched: true }));
    }, timeoutMs);
    ch.pending.set(id, { resolve, reject, timer });
    ch.proc.stdin.write(b64({ id, tool: request.tool, args: request.args ?? {} }) + "\n");
  });
}

/**
 * Shared persistent-channel front for executors whose requests ride one
 * long-lived `<argv> --serve` process (ssh agent, docker exec). Read-only and
 * identity requests may run on a restarted channel; input tools may not —
 * the fresh remote agent no longer holds this session's open_application
 * binding.
 */
const SAFE_AFTER_RESTART = new Set([
  "platform", "probe", "list_displays", "switch_display", "list_apps", "list_windows",
  "get_app_state", "resolve_element", "screenshot", "zoom", "cursor_position",
  "read_clipboard", "recordingList", "recordingStatus", "open_application", "preview",
]);
function attachPersistentChannel(ex, binding, serveArgv) {
  if (!binding) return;
  ex.persistent = (request, opts = {}) => {
    const ch = ensureSshChannel(binding, serveArgv);
    if (ch.spawnError) {
      return Promise.reject(Object.assign(new ExecError(`remote agent channel failed to start: ${ch.spawnError.message}`), { code: "remote_session_lost" }));
    }
    if (ch.restarted) {
      ch.restarted = false;
      binding.needsObservation = true;
      if (!SAFE_AFTER_RESTART.has(request.tool)) {
        return Promise.reject(Object.assign(new ExecError("the remote agent session restarted — rebind with open_application and observe before acting"), { code: "remote_session_restarted" }));
      }
    }
    return channelRequest(ch, request, opts.timeoutMs ?? 25_000);
  };
  ex.closeChannel = () => closeSshChannel(binding);
}

/** ssh executor: speaks to the remote agent installed by installRemoteAgent(). */
export function sshExec(computer, binding) {
  const userHost = computer.user ? `${computer.user}@${computer.host}` : computer.host;
  const portArgs = computer.port ? ["-p", String(computer.port)] : [];
  const remoteAgent = safeRemotePath(computer.agentPath ?? ".codewhale-cu/agent/agent.mjs");
  const base = ["-o", "BatchMode=yes", "-o", "ConnectTimeout=8", "-o", "StrictHostKeyChecking=accept-new", ...portArgs, userHost];
  const ex = {
    kind: "ssh",
    base,
    userHost,
    remoteAgent,
    run(cmd, args = [], opts = {}) {
      // Local side commands (e.g. ssh itself) run directly.
      return run(cmd, args, opts);
    },
    async remote(request, opts = {}) {
      const r = await run("ssh", [...base, "node", remoteAgent, b64({ args: request.args ?? {}, tool: request.tool, nonce: crypto.randomBytes(6).toString("hex") })], {
        timeoutMs: opts.timeoutMs ?? 25_000,
      });
      if (r.aborted) throw Object.assign(new ExecError("computer request cancelled", r), { code: "cancelled" });
      if (r.timedOut) throw new ExecError(`ssh ${userHost}: timed out`, r);
      if (r.code !== 0) throw new ExecError(`ssh ${userHost} exited ${r.code}: ${r.stderr.trim().slice(0, 400)}`, r);
      // The agent prints exactly one JSON line; anything before it is MOTD noise.
      const line = r.stdout.trim().split("\n").filter((l) => l.startsWith("{")).pop();
      const reply = line ? JSON.parse(line) : null;
      if (!reply) throw new ExecError(`ssh ${userHost}: agent returned no JSON receipt`, r);
      return reply;
    },
  };
  attachPersistentChannel(ex, binding, ["ssh", ...base, "node", remoteAgent, "--serve"]);
  return ex;
}

/**
 * docker executor: a spawned task-owned desktop container. Same agent contract
 * as ssh, but the channel is `docker exec` — no sshd, no keys, the container
 * boundary itself is the isolation. Every call goes through
 * docker/agent-exec.sh, which joins the desktop session env (display + bus)
 * the container entrypoint recorded before serving requests.
 */
export function dockerExec(computer, binding) {
  const container = safeRemotePath(computer.container);
  const remoteAgent = "/app/docker/agent-exec.sh";
  const ex = {
    kind: "docker",
    container,
    remoteAgent,
    run(cmd, args = [], opts = {}) {
      // Local side commands (docker itself) run directly.
      return run(cmd, args, opts);
    },
    async remote(request, opts = {}) {
      const r = await run("docker", ["exec", container, "/bin/sh", remoteAgent, b64({ args: request.args ?? {}, tool: request.tool, nonce: crypto.randomBytes(6).toString("hex") })], {
        timeoutMs: opts.timeoutMs ?? 25_000,
      });
      if (r.aborted) throw Object.assign(new ExecError("computer request cancelled", r), { code: "cancelled" });
      if (r.timedOut) throw new ExecError(`docker exec ${container}: timed out`, r);
      if (r.code !== 0) throw new ExecError(`docker exec ${container} exited ${r.code}: ${r.stderr.trim().slice(0, 400)}`, r);
      const line = r.stdout.trim().split("\n").filter((l) => l.startsWith("{")).pop();
      const reply = line ? JSON.parse(line) : null;
      if (!reply) throw new ExecError(`docker exec ${container}: agent returned no JSON receipt`, r);
      return reply;
    },
  };
  attachPersistentChannel(ex, binding, ["docker", "exec", "-i", container, "/bin/sh", remoteAgent, "--serve"]);
  return ex;
}

/** Push the self-contained remote agent + src tree to an ssh computer. */
export async function installRemoteAgent(computer) {
  const ex = sshExec(computer);
  const srcDir = path.join(PLUGIN_ROOT, "src");
  const rels = ["agent.mjs"];
  for (const dir of ["", "backends"]) {
    const full = path.join(srcDir, dir);
    for (const f of fs.readdirSync(full)) {
      if (f.endsWith(".mjs") || f.endsWith(".m") || f.endsWith(".h")) rels.push(`src/${dir ? dir + "/" : ""}${f}`);
    }
  }
  const marker = ".codewhale-cu/agent";
  let r = await run("ssh", [...ex.base, "mkdir", "-p", `${marker}/src/backends`], { timeoutMs: 15_000 });
  if (r.code !== 0) throw new ExecError(`ssh ${ex.userHost}: mkdir failed: ${r.stderr.trim().slice(0, 300)}`, r);
  for (const rel of rels) {
    const localPath = rel === "agent.mjs" ? path.join(PLUGIN_ROOT, "agent.mjs") : path.join(srcDir, rel.slice(4));
    const dest = safeRemotePath(`${marker}/${rel}`);
    r = await run("scp", [...(computer.port ? ["-P", String(computer.port)] : []), localPath, `${ex.userHost}:${dest}`], { timeoutMs: 30_000 });
    if (r.code !== 0) throw new ExecError(`scp ${rel} failed: ${r.stderr.trim().slice(0, 300)}`, r);
  }
  // Probe remote platform via the agent itself.
  const reply = await ex.remote({ tool: "platform" });
  return { installed: rels.length, remotePlatform: reply.platform, agentPath: `${marker}/agent.mjs` };
}

/** hdc (HarmonyOS) executor. Commands run on-device; files pull to local tmp. */
export function hdcExec(computer) {
  const targetArgs = computer.target ? ["-t", computer.target] : [];
  const shell = (args, opts = {}) => run("hdc", [...targetArgs, "shell", ...args], opts);
  return {
    kind: "hdc",
    targetArgs,
    run,
    runOk,
    shell,
    async pullFile(remotePath, localPath, opts = {}) {
      // HDC device captures use absolute paths; SSH agent paths are relative.
      // Validate the remaining path with the same traversal/metacharacter guard.
      safeRemotePath(typeof remotePath === "string" ? remotePath.replace(/^\//, "") : remotePath);
      const r = await run("hdc", [...targetArgs, "file", "recv", remotePath, localPath], opts);
      if (r.code !== 0) throw new ExecError(`hdc file recv failed: ${r.stderr.trim().slice(0, 300)}`, r);
      return localPath;
    },
    async readFile(remotePath, opts = {}) {
      // Containment: pull into a private mkdtemp dir and remove exactly that
      // dir. Never rm() the parent of a file placed directly in os.tmpdir() —
      // that recursively deletes the entire user temp directory.
      const dir = await fs.promises.mkdtemp(path.join(os.tmpdir(), "cu-hdc-"));
      try {
        const tmp = path.join(dir, "out");
        await this.pullFile(remotePath, tmp, opts);
        return await fs.promises.readFile(tmp);
      } finally {
        // Cleanup must not replace downloaded bytes or the original I/O error.
        await fs.promises.rm(dir, { recursive: true, force: true }).catch(() => {});
      }
    },
  };
}

export async function executorFor(computer, binding) {
  if (computer.transport === "local") {
    // Test hook: exercise the out-of-process wire path (desktop app / ssh
    // agent) in-process, so wire argument preparation is covered by tests.
    if (process.env.CODEWHALE_CU_TEST_REMOTE === "1") {
      const { handle } = await import("./app-handler.mjs");
      return { ...appExec({ id: "test", name: "test app" }), remote: (request) => handle(request, { sessionId: SESSION_ID, signal: currentSignal() }) };
    }
    const status = await ensureApp();
    if (status.via === "app") {
      if (status.app.sessionProtocol !== 2) throw Object.assign(new ExecError("The installed Computer Use helper needs an update for isolated sessions and disconnect cleanup. Rebuild/reinstall it, then retry."), { code: "app_upgrade_required" });
      if (process.platform === "darwin" && status.app.backgroundProtocol !== 1) throw Object.assign(new ExecError("The installed Computer Use app predates background scrolling, scoped observations and foreground preemption. Update and restart the helper before using it."), { code: "app_upgrade_required" });
      return appExec(status.app);
    }
    return { ...localExec(), appReason: status.reason };
  }
  if (computer.transport === "ssh") return sshExec(computer, binding);
  if (computer.transport === "docker") return dockerExec(computer, binding);
  if (computer.transport === "hdc") return hdcExec(computer);
  throw new ExecError(`unknown transport ${computer.transport}`);
}

/**
 * Map a computer to its backend module. Local platform is fixed; ssh
 * computers may carry platformHint (probed at registration).
 */
function effectivePlatform(computer) {
  let platform = computer.platform ?? computer.platformHint;
  if (!platform) {
    if (computer.transport === "local") platform = process.platform;
    else if (computer.transport === "hdc") platform = "harmonyos";
    else if (computer.transport === "docker") platform = "linux"; // spawned containers are always the Linux desktop image
    else platform = "linux"; // conservative default for ssh; registration probes it
  }
  return platform;
}

/** Identity of the effective route, excluding catalog presentation metadata. */
export function routeFingerprint(computer) {
  const route = [computer.transport, effectivePlatform(computer)];
  if (computer.transport === "docker") route.push(computer.container || null);
  if (computer.transport === "hdc") route.push(computer.target || null);
  if (computer.transport === "ssh") route.push(computer.host, computer.user || null,
    computer.port || null, computer.agentPath ?? ".codewhale-cu/agent/agent.mjs");
  return JSON.stringify(route);
}

export async function backendFor(computer) {
  const platform = effectivePlatform(computer);
  // Test hook: inject a fake local backend by absolute path to an .mjs
  // module exporting `create` (used by tests/, never set in production).
  const testBackend = computer.transport === "local" && process.env.CODEWHALE_CU_TEST_BACKEND;
  const mod = await import(testBackend ? url.pathToFileURL(testBackend).href : `./backends/${platform}.mjs`);
  // Backends always get a direct executor; routing through the app happens
  // one level up (the server dispatches to `executor.remote` when present).
  const exec = computer.transport === "local" ? localExec() : await executorFor(computer);
  return { backend: mod.create({ exec, computer, platform }), platform };
}
