import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import { setTimeout as delay } from "node:timers/promises";
import { Duplex } from "node:stream";
import { appRequest } from "../src/app-socket.mjs";

function controlClient(stream, { timeoutMs = 5_000, signal, diagnostics = () => "" } = {}) {
  let seq = 0, buffer = "", failure;
  const pending = new Map();
  const fail = message => {
    if (failure) return;
    failure = new Error(`FD3 control: ${message}${diagnostics() ? `\n${diagnostics()}` : ""}`);
    for (const { reject, timer } of pending.values()) { clearTimeout(timer); reject(failure); }
    pending.clear();
    stream.destroy();
  };
  stream.setEncoding("utf8");
  stream.on("error", error => fail(error.message));
  stream.on("close", () => fail("channel closed before a reply"));
  stream.on("end", () => fail("channel ended before a reply"));
  const abort = () => fail("test cancelled");
  signal?.addEventListener("abort", abort, { once: true });
  stream.once("close", () => signal?.removeEventListener("abort", abort));
  if (signal?.aborted) abort();
  stream.on("data", chunk => {
    buffer += chunk;
    if (buffer.length > 8192) return fail("reply exceeds 8192 bytes");
    let newline;
    while ((newline = buffer.indexOf("\n")) >= 0) {
      const line = buffer.slice(0, newline); buffer = buffer.slice(newline + 1);
      let state;
      try { state = JSON.parse(line); } catch { return fail("malformed JSON reply"); }
      if (!state || typeof state !== "object") return fail("malformed control state");
      const waiting = pending.get(state.id);
      if (!waiting) continue;
      if (state.error) return fail(`${waiting.command} failed: ${state.error}`);
      clearTimeout(waiting.timer); pending.delete(state.id); waiting.resolve(state);
    }
  });
  return command => {
    if (failure) return Promise.reject(failure);
    const id = ++seq;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => fail(`${command} timed out after ${timeoutMs}ms`), timeoutMs);
      pending.set(id, { command, resolve, reject, timer });
      try { stream.write(JSON.stringify({ id, command }) + "\n", error => { if (error) fail(error.message); }); }
      catch (error) { fail(error.message); }
    });
  };
}

const root = fileURLToPath(new URL("../", import.meta.url));
test("human Pause and Stop cannot be bypassed; a lost owner exits and can reopen stopped", { timeout: 20_000 }, async t => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "cu-control-"));
  const log = path.join(dir, "calls.jsonl");
  const endpoint = process.platform === "win32"
    ? `\\\\.\\pipe\\cu-control-${process.pid}-${path.basename(dir)}`
    : path.join(dir, "app.sock");
  const daemonOptions = { env: { ...process.env,
    CODEWHALE_CU_STATE_DIR: dir, CODEWHALE_CU_APP_SOCKET: endpoint, CODEWHALE_CU_APP_WARM: "off",
    CODEWHALE_CU_TEST_BACKEND: path.join(root, "tests/fixtures/session-backend.mjs"), CU_SESSION_CALLS: log, CODEWHALE_CU_CONTROL_FD: "3" }, stdio: ["ignore", "ignore", "pipe", "pipe"] };
  let errors = "", daemon, exited;
  function launch() {
    daemon = spawn(process.execPath, [path.join(root, "app/daemon.mjs")], daemonOptions);
    daemon.stderr.on("data", chunk => { errors += chunk; });
    exited = new Promise(resolve => { daemon.once("exit", resolve); daemon.once("error", error => { errors += error.message; resolve(); }); });
  }
  launch();
  const sockets = [];
  const previousSocket = process.env.CODEWHALE_CU_APP_SOCKET;
  process.env.CODEWHALE_CU_APP_SOCKET = endpoint;
  t.after(async () => {
    if (previousSocket === undefined) delete process.env.CODEWHALE_CU_APP_SOCKET;
    else process.env.CODEWHALE_CU_APP_SOCKET = previousSocket;
    sockets.forEach(socket => socket.destroy()); daemon.stdio[3].destroy();
    let deadline;
    const forceKill = setTimeout(() => daemon.kill("SIGKILL"), 3_000);
    try {
      if (daemon.exitCode === null && daemon.signalCode === null) daemon.kill("SIGTERM");
      await Promise.race([exited, new Promise((_, reject) => { deadline = setTimeout(() => reject(new Error("Daemon did not exit after SIGKILL")), 4_000); })]);
    } finally { clearTimeout(forceKill); clearTimeout(deadline); fs.rmSync(dir, { recursive: true, force: true }); }
  });
  async function until(predicate) { for(let i=0;i<250;i++) { if(predicate()) return; if (daemon.exitCode !== null || daemon.signalCode !== null) throw new Error(`Daemon exited: ${errors}`); await delay(20, undefined, { signal: t.signal }); } throw new Error(`Timed out: ${errors}`); }
  // Named pipes on Windows have no filesystem entry; both transports publish
  // the same run receipt only after the listener is ready.
  await until(() => fs.existsSync(path.join(dir, "app-run.json")));
  function request(payload, keepOpen = false) {
    return appRequest(payload, { keepOpen, timeoutMs: 5_000, signal: t.signal }).then(result => {
      if (!keepOpen) return result;
      sockets.push(result.socket); return result.reply;
    });
  }
  const control = controlClient(daemon.stdio[3], { signal: t.signal, diagnostics: () => errors });
  assert.equal((await control("status")).mode, "ready", "human-control channel responds before input starts");
  assert.equal((await request({tool:"hello"})).app.controlOwner,true);
  const sessionId="human-controls";
  const { leaseToken }=await request({tool:"open_session",sessionId},true);
  const call=(tool,args={})=>request({tool,args,sessionId,leaseToken});
  assert.equal((await call("get_app_state",{app_ref:{name:"Practice"}})).ok,true);
  const held=call("hold_key",{text:"must be cancelled"});
  held.catch(() => {}); // The result is asserted below; avoid orphan rejections if control fails first.
  await until(()=>fs.existsSync(log)&&fs.readFileSync(log,"utf8").includes("child_started"));
  const queued=call("type",{text:"must not replay"});
  queued.catch(() => {});
  const paused=await control("pause");
  assert.equal(paused.mode,"paused"); assert.equal(paused.cleanupPending,false);
  assert.equal((await held).ok,false); assert.equal((await queued).ok,false);
  assert.equal((await call("type",{text:"while paused"})).error.code,"control_paused");
  for(const tool of ["resume","set_control_mode","control","updates"]) assert.equal((await call(tool)).error.code,"tool_not_allowed");
  assert.equal((await control("resume")).mode,"ready");
  assert.equal((await call("type",{text:"allowed again"})).ok,true);
  assert.equal((await control("stop")).mode,"stopped");
  await control("resume");
  assert.equal((await call("type",{text:"old stopped owner"})).error.code,"control_stopped");
  const records=fs.readFileSync(log,"utf8").trim().split("\n").map(JSON.parse);
  assert.ok(records.some(record=>record.method==="child_released"));
  assert.deepEqual(records.filter(record=>record.method==="type").map(record=>record.text),["allowed again"]);
  // Owner loss must release input and retire the listener, so relaunching
  // restores the menu controls without silently authorizing new input.
  daemon.stdio[3].destroy();
  await until(() => daemon.exitCode !== null);
  assert.equal(daemon.exitCode,0,errors);
  assert.equal(JSON.parse(fs.readFileSync(path.join(dir,"control.json"))).mode,"stopped");
  assert.equal(fs.existsSync(path.join(dir,"app-run.json")),false);
  await assert.rejects(request({tool:"hello"}),error=>error.code==="app_unavailable");
  launch();
  await until(() => fs.existsSync(path.join(dir,"app-run.json")));
  const reopened = controlClient(daemon.stdio[3], { signal: t.signal, diagnostics: () => errors });
  assert.equal((await reopened("status")).mode,"stopped");
  assert.equal((await request({tool:"hello"})).app.controlOwner,true);
  const fresh=await request({tool:"open_session",sessionId:"fresh"},true);
  assert.equal((await request({tool:"get_app_state",sessionId:"fresh",leaseToken:fresh.leaseToken})).error.code,"control_stopped");
  assert.equal((await reopened("resume")).mode,"ready");
  assert.equal((await request({tool:"get_app_state",args:{app_ref:{name:"Reopened"}},sessionId:"fresh",leaseToken:fresh.leaseToken})).ok,true);
  assert.equal((await call("type",{text:"old lease after reopen"})).error.code,"session_owner_required");
});

test("silent FD3 peer rejects every pending command and closes the channel", { timeout: 2_000 }, async () => {
  const stream = new Duplex({ read() {}, write(_chunk, _encoding, done) { done(); } });
  const control = controlClient(stream, { timeoutMs: 30, diagnostics: () => "fixture daemon stderr" });
  const results = await Promise.allSettled([control("pause"), control("status")]);
  for (const result of results) {
    assert.equal(result.status, "rejected");
    assert.match(result.reason.message, /pause timed out after 30ms\nfixture daemon stderr/);
  }
  assert.equal(stream.destroyed, true);
  await assert.rejects(control("resume"), /timed out/);
});

for (const [name, respond, expected] of [
  ["EOF", stream => stream.push(null), /channel ended/],
  ["close", stream => stream.destroy(), /channel closed/],
  ["error", stream => stream.destroy(new Error("broken pipe")), /broken pipe/],
  ["invalid JSON", stream => stream.push("not json\n"), /malformed JSON/],
  ["invalid state", stream => stream.push("null\n"), /malformed control state/],
  ["oversized reply", stream => stream.push("x".repeat(8193)), /exceeds 8192/],
  ["command failure", stream => stream.push('{"id":1,"error":"disk full"}\n'), /pause failed: disk full/],
]) {
  test(`FD3 ${name} rejects a waiting control command`, { timeout: 2_000 }, async () => {
    const stream = new Duplex({ read() {}, write(_chunk, _encoding, done) { done(); queueMicrotask(() => respond(this)); } });
    const control = controlClient(stream);
    await assert.rejects(control("pause"), expected);
    assert.equal(stream.destroyed, true);
  });
}

test("FD3 test cancellation rejects pending commands", { timeout: 2_000 }, async () => {
  const stream = new Duplex({ read() {}, write(_chunk, _encoding, done) { done(); } });
  const abort = new AbortController();
  const control = controlClient(stream, { signal: abort.signal });
  const rejected = assert.rejects(control("pause"), /test cancelled/);
  abort.abort(); await rejected;
  assert.equal(stream.destroyed, true);
});

test("FD3 matches fragmented replies to their command IDs", { timeout: 2_000 }, async () => {
  const requests = [];
  const stream = new Duplex({ read() {}, write(chunk, _encoding, done) { requests.push(JSON.parse(chunk)); done(); } });
  const control = controlClient(stream);
  const replies = Promise.all([control("status"), control("pause")]);
  stream.push('{"id":2,"mode":"pau');
  stream.push('sed"}\n{"id":1,"mode":"ready"}\n');
  assert.deepEqual(await replies, [{ id: 1, mode: "ready" }, { id: 2, mode: "paused" }]);
  assert.deepEqual(requests.map(request => request.command), ["status", "pause"]);
  stream.destroy();
});
