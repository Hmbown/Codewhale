// Process execution helper: spawn, timeout, text capture. Zero dependencies.
import { spawn } from "node:child_process";
import { AsyncLocalStorage } from "node:async_hooks";
import { setTimeout as delay } from "node:timers/promises";

const requests = new AsyncLocalStorage();
export const currentSignal = () => requests.getStore()?.signal;
// A null signal is reserved for bounded cleanup, such as releasing a held key.
export const withSignal = (signal, fn) => requests.run({ signal }, fn);
export function throwIfAborted(signal = currentSignal()) {
  if (signal?.aborted) throw Object.assign(new Error("computer request cancelled"), { code: "cancelled" });
}
export async function wait(ms) {
  try { await delay(ms, undefined, { signal: currentSignal() ?? undefined }); }
  catch (err) { throwIfAborted(); throw err; }
}

/**
 * Run a command. Never uses a shell: cmd + args array only, so tool arguments
 * can never become command injection.
 * @returns {Promise<{code:number|null, stdout:string, stderr:string, timedOut:boolean, signal:string|null}>}
 */
export function run(cmd, args = [], opts = {}) {
  const signal = opts.signal === undefined ? currentSignal() : opts.signal;
  if (signal?.aborted) return Promise.resolve({ code: -1, stdout: "", stderr: "computer request cancelled", timedOut: false, signal: null, aborted: true, spawned: false });
  const timeoutMs = opts.timeoutMs ?? 20_000;
  const maxBuffer = opts.maxBuffer ?? 32 * 1024 * 1024;
  return new Promise((resolve) => {
    let child;
    try {
      child = spawn(cmd, args, {
        env: opts.env ? { ...process.env, ...opts.env } : process.env,
        cwd: opts.cwd,
        stdio: [opts.ownerPipe ? "pipe" : "ignore", "pipe", "pipe"],
        // Windows: node handles .cmd/.exe resolution for known tools via shell:false + full name
        windowsHide: true,
      });
    } catch (err) {
      resolve({ code: -1, stdout: "", stderr: String(err?.message ?? err), timedOut: false, signal: null, spawned: false });
      return;
    }
    if (opts.ownerPipe) child.stdin.on("error", () => {});
    opts.onSpawn?.(child);
    let stdout = "";
    let stderr = "";
    let timedOut = false;
    let settled = false;
    let hardKill;
    const terminate = () => {
      try { child.kill("SIGTERM"); } catch {}
      hardKill ??= setTimeout(() => { try { child.kill("SIGKILL"); } catch {} }, 1500);
    };
    const abort = () => terminate();
    signal?.addEventListener("abort", abort, { once: true });
    const timer = timeoutMs === 0 ? null : setTimeout(() => {
      timedOut = true;
      terminate();
    }, timeoutMs);
    child.stdout.on("data", (d) => {
      if (stdout.length < maxBuffer) stdout += d.toString();
      opts.onStdout?.(d.toString());
    });
    child.stderr.on("data", (d) => {
      if (stderr.length < maxBuffer) stderr += d.toString();
    });
    const finish = (code, exitSignal) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      clearTimeout(hardKill);
      signal?.removeEventListener("abort", abort);
      resolve({ code, stdout, stderr, timedOut, signal: exitSignal, spawned: !!child.pid, ...(signal?.aborted ? { aborted: true } : {}) });
    };
    child.on("error", (err) => {
      stderr += String(err?.message ?? err);
      finish(-1, null);
    });
    child.on("close", (code, signal) => finish(code, signal));
  });
}

/** A native input owner acknowledges its press, then releases on stdin EOF. */
export async function runInputLease(cmd, args = [], opts = {}) {
  throwIfAborted();
  let child, buffer = "", stopped = false, closed = false, killTimer;
  const pending = [];
  const reply = () => new Promise((resolve, reject) => pending.push({ resolve, reject }));
  const ready = reply();
  const completion = run(cmd, args, {
    ...opts, ownerPipe: true, timeoutMs: 0,
    onSpawn: (process) => { child = process; },
    onStdout: (chunk) => {
      buffer += chunk;
      let nl;
      while ((nl = buffer.indexOf("\n")) !== -1) {
        const line = buffer.slice(0, nl); buffer = buffer.slice(nl + 1);
        const waiter = pending.shift();
        if (!waiter) continue;
        try { waiter.resolve(JSON.parse(line)); }
        catch { waiter.reject(new ExecError("Native input owner returned an invalid receipt")); }
      }
    },
  });
  completion.then((result) => {
    closed = true;
    clearTimeout(killTimer);
    const error = Object.assign(new ExecError(result.stderr || "Native input owner closed", result), { code: result.aborted ? "cancelled" : "input_owner_closed" });
    for (const waiter of pending.splice(0)) waiter.reject(error);
  });
  const release = async (message = {}) => {
    if (!stopped) {
      stopped = true;
      if (!closed) {
        child?.stdin.end(JSON.stringify({ ...message, release: true }) + "\n");
        killTimer = setTimeout(() => {
          child?.kill("SIGTERM");
          killTimer = setTimeout(() => child?.kill("SIGKILL"), 750);
        }, 750);
      }
    }
    const result = await completion;
    clearTimeout(killTimer);
    if (result.code !== 0) throw Object.assign(new ExecError(result.stderr || "Native input cleanup failed", result), { code: result.aborted ? "cancelled" : "input_release_failed" });
  };
  let timer;
  try {
    const receipt = await Promise.race([ready, new Promise((_, reject) => { timer = setTimeout(() => reject(new ExecError("Native input owner did not acknowledge input")), opts.timeoutMs ?? 20_000); })]);
    if (receipt?.action_sent !== true || receipt?.input_lease !== true) throw new ExecError("Native input owner did not confirm a live input lease");
    return { receipt, release, async send(message) {
      const signal = currentSignal();
      let commandTimer, abort;
      try {
        throwIfAborted(signal);
        if (closed || stopped || child?.exitCode !== null || child?.signalCode) throw Object.assign(new ExecError("Native input owner is closed"), { code: "input_owner_closed" });
        const next = reply();
        const cancelled = new Promise((_, reject) => {
          abort = () => reject(Object.assign(new ExecError("computer request cancelled"), { code: "cancelled" }));
          signal?.addEventListener("abort", abort, { once: true });
        });
        child.stdin.write(JSON.stringify(message) + "\n");
        return await Promise.race([next, cancelled, new Promise((_, reject) => { commandTimer = setTimeout(() => reject(new ExecError("Native input owner did not acknowledge pointer motion")), opts.timeoutMs ?? 20_000); })]);
      }
      catch (error) { await release().catch(() => {}); throw error; }
      finally { clearTimeout(commandTimer); signal?.removeEventListener("abort", abort); }
    } };
  } catch (error) {
    await release().catch(() => {});
    throw error;
  } finally { clearTimeout(timer); }
}

/** run() and throw a typed error on non-zero exit / timeout. */
export async function runOk(cmd, args = [], opts = {}) {
  const r = await run(cmd, args, opts);
  if (r.aborted) throw Object.assign(new ExecError("computer request cancelled", r), { code: "cancelled" });
  if (r.timedOut) throw new ExecError(`timeout after ${opts.timeoutMs ?? 20_000}ms: ${cmd}`, r);
  if (r.code !== 0) throw new ExecError(`${cmd} exited ${r.code}: ${trim(r.stderr || r.stdout)}`, r);
  return r;
}

export class ExecError extends Error {
  constructor(message, result) {
    super(message);
    this.name = "ExecError";
    this.result = result;
  }
}

/** True when the executable exists on PATH (or opts.fullPath exists). */
export async function have(cmd) {
  const probe = process.platform === "win32" ? "where" : "which";
  const r = await run(probe, [cmd], { timeoutMs: 5000 });
  return r.code === 0 && r.stdout.trim().length > 0;
}

export function trim(s, n = 400) {
  s = String(s ?? "").trim();
  return s.length > n ? s.slice(0, n) + "…" : s;
}

/** Parse JSON safely, returning fallback on failure. */
export function tryJson(s, fallback = null) {
  try { return JSON.parse(s); } catch { return fallback; }
}
