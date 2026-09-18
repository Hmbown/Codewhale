// Exercise the actual helper socket and MCP lifecycle with recording backends.
// Child-process cancellation must prevent delayed input, not just hide replies.
import { test, before, after } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawn } from "node:child_process";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const dir = fs.mkdtempSync(path.join(os.tmpdir(), "cu-session-"));
const log = path.join(dir, "calls.jsonl");
const env = {
  ...process.env,
  CODEWHALE_CU_STATE_DIR: dir,
  CODEWHALE_CU_APP_WARM: "off",
  CODEWHALE_CU_TEST_BACKEND: path.join(ROOT, "tests/fixtures/session-backend.mjs"),
  CU_SESSION_CALLS: log,
};
delete env.CODEWHALE_CU_APP;
delete env.CODEWHALE_CU_APP_SOCKET;
delete env.CODEWHALE_CU_TEST_REMOTE;
process.env.CODEWHALE_CU_STATE_DIR = dir;
delete process.env.CODEWHALE_CU_APP_SOCKET;
const { appRequest, appSessionRequest, openAppSession, hello } = await import("../src/app-socket.mjs");
const { appExec } = await import("../src/transport.mjs");
let daemon;
let daemonErrors = "";
const hosts = new Set();
const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const calls = () => fs.existsSync(log) ? fs.readFileSync(log, "utf8").trim().split("\n").filter(Boolean).map(JSON.parse) : [];
async function until(check, timeoutMs = 3000) {
  const end = Date.now() + timeoutMs;
  do {
    if (await check()) return;
    await pause(20);
  } while (Date.now() < end);
  assert.fail(`Condition did not become true within ${timeoutMs}ms`);
}

function mcp() {
  const child = spawn(process.execPath, [path.join(ROOT, "mcp/server.mjs")], { env, stdio: ["pipe", "pipe", "pipe"] });
  hosts.add(child);
  const replies = new Map();
  let buf = "";
  let id = 0;
  child.stderr.on("data", () => {});
  child.stdout.on("data", (chunk) => {
    buf += chunk.toString();
    let nl;
    while ((nl = buf.indexOf("\n")) !== -1) {
      const line = buf.slice(0, nl); buf = buf.slice(nl + 1);
      const msg = JSON.parse(line);
      replies.set(msg.id, msg);
    }
  });
  const send = (method, params, requestId) => child.stdin.write(JSON.stringify({ jsonrpc: "2.0", ...(requestId === undefined ? {} : { id: requestId }), method, params }) + "\n");
  const start = (name, args = {}) => { const requestId = ++id; send("tools/call", { name, arguments: args }, requestId); return requestId; };
  const response = async (requestId) => {
    await until(() => replies.has(requestId));
    const msg = replies.get(requestId);
    assert.ok(msg.result, JSON.stringify(msg.error));
    return JSON.parse(msg.result.content[0].text);
  };
  return { child, replies, start, response, cancel: (requestId) => send("notifications/cancelled", { requestId }), tool: (name, args) => response(start(name, args)) };
}

async function closeHost(host) {
  const exit = new Promise((resolve) => host.child.once("exit", resolve));
  host.child.stdin.end();
  await exit;
  hosts.delete(host.child);
}

before(async () => {
  daemon = spawn(process.execPath, [path.join(ROOT, "app/daemon.mjs")], { env, stdio: ["ignore", "ignore", "pipe"] });
  daemon.stderr.on("data", (data) => { daemonErrors += data; });
  await until(async () => !!(await hello({ timeoutMs: 100 })), 5000).catch((err) => { throw new Error(`${err.message}\n${daemonErrors}`); });
});
after(async () => {
  for (const child of hosts) child.kill("SIGTERM");
  if (daemon?.exitCode === null) {
    const exit = new Promise((resolve) => daemon.once("exit", resolve));
    daemon.kill("SIGTERM");
    await exit;
  }
  fs.rmSync(dir, { recursive: true, force: true });
});

test("helper requires session identities while compatibility probes remain available", async () => {
  assert.equal((await hello()).sessionProtocol, 2);
  assert.equal((await appRequest({ tool: "platform" })).ok, true);
  for (const sessionId of [undefined, "", "bad:session", "x".repeat(129)]) {
    const reply = await appRequest({ tool: "type", args: { text: "must not type" }, sessionId });
    assert.equal(reply.error.code, "session_required");
  }
});

test("helper accepts actions only while their original socket owner is alive", async () => {
  const sessionId = "lease-owner";
  const lease = await openAppSession(sessionId);
  assert.equal((await appRequest({ tool: "open_session", sessionId })).error.code, "session_owned");
  for (const leaseToken of [undefined, "another-owner"]) {
    assert.equal((await appRequest({ tool: "get_app_state", sessionId, leaseToken, args: { app_ref: { name: "Spoofed owner" } } })).error.code, "session_owner_required");
  }
  assert.equal((await appSessionRequest({ tool: "get_app_state", sessionId, args: { app_ref: { name: "Lease owner" } } })).ok, true);
  lease.socket.destroy();
  await until(() => calls().some((item) => item.method === "release_input" && item.appName === "Lease owner"));
  assert.equal((await appRequest({ tool: "type", sessionId, leaseToken: lease.token, args: { text: "stale lease" } })).error.code, "session_owner_required");
  assert.equal((await appSessionRequest({ tool: "probe", sessionId })).ok, true, "a dropped owner socket re-leases transparently instead of bricking the session");
  assert.equal((await appSessionRequest({ tool: "get_app_state", sessionId, args: { app_ref: { name: "Re-leased owner" } } })).data.name, "Re-leased owner");
  assert.ok(!calls().some((item) => item.appName === "Spoofed owner" || item.text === "stale lease"));
});

test("separate sessions keep their own bound apps and closed sessions cannot revive", async () => {
  const a = appExec({}, "binding-a");
  const b = appExec({}, "binding-b");
  await a.remote({ tool: "get_app_state", args: { app_ref: { name: "Editor A" } } });
  assert.equal((await b.remote({ tool: "type", args: { text: "unbound" } })).error.code, "target_app_required");
  await b.remote({ tool: "get_app_state", args: { app_ref: { name: "Editor B" } } });
  assert.equal((await a.remote({ tool: "type", args: { text: "first" } })).data.appName, "Editor A");
  assert.equal((await b.remote({ tool: "type", args: { text: "second" } })).data.appName, "Editor B");
  assert.equal((await appSessionRequest({ tool: "close_session", sessionId: "binding-a" })).ok, true);
  await appExec({}, "unrelated-new-session").remote({ tool: "probe" });
  await assert.rejects(a.remote({ tool: "get_app_state", args: { app_ref: { name: "Revived" } } }), (err) => err.code === "app_session_closed");
  assert.equal((await b.remote({ tool: "type", args: { text: "still alive" } })).ok, true);
});

test("list_sessions names live sessions content-free and drops closed ones", async () => {
  const a = appExec({}, "ls-a");
  const b = appExec({}, "ls-b");
  await a.remote({ tool: "probe" });
  await b.remote({ tool: "probe" });
  const both = await a.remote({ tool: "list_sessions" });
  assert.equal(both.ok, true);
  assert.equal(both.data.control, "ready");
  assert.ok(both.data.count >= 2, `both live sessions are listed (${both.data.count})`);
  for (const s of both.data.sessions) {
    assert.ok(s.target === null || (typeof s.target === "object" && Number.isInteger(s.target.pid)), "targets are app identity or null, never task text");
    assert.ok(typeof s.ageSec === "number" && typeof s.inputHeld === "boolean" && typeof s.action !== "undefined");
  }
  const forged = await appRequest({ tool: "list_sessions", sessionId: "ls-a", leaseToken: "forged" });
  assert.equal(forged.ok, false);
  assert.equal(forged.error.code, "session_owner_required", "the registry needs the live owner lease, not a session id alone");
  assert.equal((await appSessionRequest({ tool: "close_session", sessionId: "ls-b" })).ok, true);
  const one = await a.remote({ tool: "list_sessions" });
  assert.equal(one.data.count, both.data.count - 1, "a closed session leaves the registry");
});

test("a capability grant narrows the daemon lease, and cleanup is never blocked", async () => {
  const prior = process.env.CODEWHALE_CU_GRANT;
  process.env.CODEWHALE_CU_GRANT = "probe";
  try {
    await openAppSession("grant-a");
    assert.equal((await appSessionRequest({ tool: "probe", sessionId: "grant-a" })).ok, true);
    const refused = await appSessionRequest({ tool: "get_app_state", sessionId: "grant-a", args: { app_ref: { name: "Nope" } } });
    assert.equal(refused.ok, false);
    assert.equal(refused.error.code, "not_granted", "the daemon refuses ungranted tools even if the server asked");
    assert.equal((await appSessionRequest({ tool: "close_session", sessionId: "grant-a" })).ok, true, "cleanup must never be blocked by a grant");
    process.env.CODEWHALE_CU_GRANT = "";
    await openAppSession("grant-b");
    assert.equal((await appSessionRequest({ tool: "get_app_state", sessionId: "grant-b", args: { app_ref: { name: "Open" } } })).ok, true, "a new session without a grant is unrestricted");
    await appSessionRequest({ tool: "close_session", sessionId: "grant-b" });
  } finally {
    if (prior === undefined) delete process.env.CODEWHALE_CU_GRANT; else process.env.CODEWHALE_CU_GRANT = prior;
  }
});

test("read-only grants survive the wire-name translation (request_access travels as probe)", async () => {
  const prior = process.env.CODEWHALE_CU_GRANT;
  process.env.CODEWHALE_CU_GRANT = "read-only";
  try {
    await openAppSession("grant-ro");
    assert.equal((await appSessionRequest({ tool: "probe", sessionId: "grant-ro" })).ok, true, "the grant must cover the transport name the daemon actually sees");
    assert.equal((await appSessionRequest({ tool: "get_app_state", sessionId: "grant-ro", args: { app_ref: { name: "Visible" } } })).ok, true, "observation tools stay granted");
    const refused = await appSessionRequest({ tool: "left_click", sessionId: "grant-ro", args: { target: { type: "coordinate", x: 1, y: 1 } } });
    assert.equal(refused.ok, false);
    assert.equal(refused.error.code, "not_granted");
    await appSessionRequest({ tool: "close_session", sessionId: "grant-ro" });
  } finally {
    if (prior === undefined) delete process.env.CODEWHALE_CU_GRANT; else process.env.CODEWHALE_CU_GRANT = prior;
  }
});

test("disconnect cancels the child process and a queued request never posts input", async () => {
  const active = new AbortController();
  const queued = new AbortController();
  const held = appSessionRequest({ tool: "hold_key", sessionId: "socket-active", args: { text: "socket-cancel" } }, { signal: active.signal });
  const heldRejection = assert.rejects(held, (err) => err.code === "cancelled");
  await until(() => calls().some((item) => item.method === "child_started" && item.text === "socket-cancel"));
  const waiting = appSessionRequest({ tool: "get_app_state", sessionId: "socket-queued", args: { app_ref: { name: "Cancelled queue" } } }, { signal: queued.signal });
  const queuedRejection = assert.rejects(waiting, (err) => err.code === "cancelled");
  queued.abort();
  active.abort();
  await Promise.all([heldRejection, queuedRejection]);
  await until(() => calls().some((item) => item.method === "child_released"));
  assert.equal((await appExec({}, "socket-check").remote({ tool: "probe" })).ok, true);
  assert.ok(!calls().some((item) => item.appName === "Cancelled queue"));
  assert.ok(!calls().some((item) => item.method === "late_input"));
});

test("MCP cancellation drains input, keeps the host alive, and isolates a second host", async () => {
  const a = mcp();
  const b = mcp();
  await a.tool("get_app_state", { app_ref: { name: "Host A" } });
  assert.equal((await b.tool("type", { text: "unbound host B" })).error.code, "target_app_required");
  await b.tool("get_app_state", { app_ref: { name: "Host B" } });
  const id = a.start("hold_key", { text: "mcp-cancel", duration: 10 });
  await until(() => calls().some((item) => item.method === "child_started" && item.text === "mcp-cancel"));
  a.cancel(id);
  assert.equal((await a.tool("type", { text: "after cancellation" })).ok, true);
  assert.ok(!a.replies.has(id), "cancelled MCP request must not reply");
  assert.equal((await b.tool("type", { text: "other host" })).appName, "Host B");
  await closeHost(a);
  assert.equal((await b.tool("type", { text: "after other host exits" })).ok, true);
  await closeHost(b);
});

test("stop cancels active and queued actions, releases held input, and leaves probes usable", async () => {
  const host = mcp();
  await host.tool("get_app_state", { app_ref: { name: "Stopped host" } });
  await host.tool("left_mouse_down", { target: { type: "coordinate", space: "screen", x: 10, y: 10 } });
  const hold = host.start("hold_key", { text: "mcp-stop", duration: 10 });
  await until(() => calls().some((item) => item.method === "child_started" && item.text === "mcp-stop"));
  const queued = host.start("type", { text: "must never arrive after stop" });
  const stopped = await host.tool("stop_computer_control");
  assert.equal(stopped.ok, true, JSON.stringify(stopped));
  assert.equal(stopped.inputReleased, true);
  assert.equal((await host.response(queued)).error.code, "control_stopped");
  assert.equal((await host.response(hold)).ok, false);
  assert.ok(calls().some((item) => item.method === "release_input" && item.appName === "Stopped host" && item.pointerDown));
  assert.ok(!calls().some((item) => item.text === "must never arrive after stop"));
  assert.equal((await host.tool("request_access")).ok, true);
  assert.equal((await host.tool("type", { text: "after stop" })).error.code, "control_stopped");
  assert.ok(!calls().some((item)=>item.method==="session_closed"&&item.appName==="Stopped host"),"stop releases input but does not close the session's other owned resources");
  await closeHost(host);
  assert.ok(calls().some((item)=>item.method==="session_closed"&&item.appName==="Stopped host"));
});

test("another MCP host cannot redirect the selected computer", async () => {
  const a = mcp();
  const b = mcp();
  await a.tool("get_app_state", { app_ref: { name: "Local host A" } });
  await b.tool("computer_register", { computer: "session-test-pad", transport: "hdc" });
  await b.tool("computer_switch", { computer: "session-test-pad" });
  assert.equal((await a.tool("computer_list")).active, "local");
  assert.equal((await b.tool("computer_list")).active, "session-test-pad");
  const input = await a.tool("type", { text: "stay on local host A" });
  assert.equal(input.ok, true, JSON.stringify(input));
  assert.equal(input.computer.id, "local");
  await b.tool("computer_remove", { computer: "session-test-pad" });
  await closeHost(a);
  await closeHost(b);
});

test("retiring a helper-backed local alias closes only that MCP host's session", async () => {
  const retiring = mcp();
  const survivor = mcp();
  let retiringErrors = "";
  let survivorErrors = "";
  retiring.child.stderr.on("data", chunk => { retiringErrors += chunk; });
  survivor.child.stderr.on("data", chunk => { survivorErrors += chunk; });
  const alias = "retiring-helper-alias";
  assert.equal((await retiring.tool("computer_register", { computer: alias, transport: "local" })).ok, true);
  assert.equal((await retiring.tool("get_app_state", { computer: alias, app_ref: { name: "Retiring alias owner" } })).ok, true);
  assert.equal((await survivor.tool("get_app_state", { app_ref: { name: "Alias retirement survivor" } })).ok, true);
  const owner = calls().find(item => item.method === "get_app_state" && item.appName === "Retiring alias owner").instance;
  const other = calls().find(item => item.method === "get_app_state" && item.appName === "Alias retirement survivor").instance;
  assert.notEqual(owner, other);
  assert.equal((await retiring.tool("left_mouse_down", { target: { type: "coordinate", space: "screen", x: 10, y: 10 } })).ok, true);
  assert.equal((await survivor.tool("type", { text: "blocked by retiring owner" })).error.code, "input_busy");

  // Registration only changes the private fixture catalog. No HDC observation
  // or backend operation is requested, so no device command can run here.
  const registered = await retiring.tool("computer_register", { computer: alias, transport: "hdc", target: "unobserved-fixture-device" });
  assert.equal(registered.ok, true, JSON.stringify(registered));
  assert.equal(registered.registered.transport, "hdc");
  assert.ok(calls().some(item => item.instance === owner && item.method === "release_input" && item.pointerDown), "retiring the route releases the old helper's held pointer");
  assert.ok(calls().some(item => item.instance === owner && item.method === "session_closed"), "retiring the route closes its helper backend");
  assert.ok(!calls().some(item => item.instance === other && item.method === "session_closed"), "the second MCP host keeps its helper session");

  for (const [name, args] of [
    ["request_access", {}],
    ["type", { text: "must not revive retired helper" }],
  ]) {
    const reply = await retiring.tool(name, { computer: "local", ...args });
    assert.equal(reply.ok, false);
    assert.equal(reply.error.code, "app_session_closed");
    assert.match(reply.error.message, /new MCP session/);
  }
  assert.ok(!calls().some(item => item.text === "must not revive retired helper"));
  assert.equal((await survivor.tool("type", { text: "survives alias retirement" })).appName, "Alias retirement survivor");
  assert.equal((await retiring.tool("computer_remove", { computer: alias })).ok, true);

  const retiringClosed = new Promise(resolve => retiring.child.once("close", resolve));
  await closeHost(retiring);
  await retiringClosed;
  assert.equal(retiringErrors, "", "shutdown must not retry an already closed helper as a cleanup failure");
  assert.equal((await survivor.tool("type", { text: "survives retiring host shutdown" })).appName, "Alias retirement survivor");
  const survivorClosed = new Promise(resolve => survivor.child.once("close", resolve));
  await closeHost(survivor);
  await survivorClosed;
  assert.equal(survivorErrors, "");
});

test("MCP forced exit releases idle held input without waiting for another client", async () => {
  const dead = mcp();
  const survivor = mcp();
  await dead.tool("get_app_state", { app_ref: { name: "Killed idle host" } });
  await survivor.tool("get_app_state", { app_ref: { name: "Surviving host" } });
  await dead.tool("left_mouse_down", { target: { type: "coordinate", space: "screen", x: 10, y: 10 } });
  assert.equal((await survivor.tool("type", {text:"must wait for held pointer"})).error.code,"input_busy");
  assert.equal((await survivor.tool("request_access")).ok,true,"observation stays available while another session holds input");
  const exit = new Promise((resolve) => dead.child.once("exit", resolve));
  dead.child.kill("SIGKILL");
  await exit;
  hosts.delete(dead.child);
  await until(() => calls().some((item) => item.method === "release_input" && item.appName === "Killed idle host" && item.pointerDown));
  assert.equal((await survivor.tool("type", { text: "surviving binding" })).appName, "Surviving host");
  await closeHost(survivor);
});

test("MCP forced exit cancels its active child before delayed input can post", async () => {
  const host = mcp();
  await host.tool("get_app_state", { app_ref: { name: "Killed active host" } });
  host.start("hold_key", { text: "killed-active", duration: 10 });
  await until(() => calls().some((item) => item.method === "child_started" && item.text === "killed-active"));
  const instance = calls().find((item) => item.method === "child_started" && item.text === "killed-active").instance;
  const exit = new Promise((resolve) => host.child.once("exit", resolve));
  host.child.kill("SIGKILL");
  await exit;
  hosts.delete(host.child);
  await until(() => calls().some((item) => item.method === "child_released" && item.instance === instance));
  await until(() => calls().some((item) => item.method === "release_input" && item.appName === "Killed active host"));
  assert.ok(!calls().some((item) => item.method === "late_input"));
});

test("an app update re-leases live sessions transparently; an absent app still fails without bricking", async () => {
  const sessionId = "upgrade-survivor";
  assert.equal((await appSessionRequest({ tool: "get_app_state", sessionId, args: { app_ref: { name: "Survivor" } } })).ok, true);
  const exit = new Promise((resolve) => daemon.once("exit", resolve));
  daemon.kill("SIGTERM");
  await exit;
  // Mid-update the app is genuinely absent: the request fails, and that
  // failure is not cached against the session.
  await assert.rejects(appSessionRequest({ tool: "probe", sessionId }), (err) => err.code === "app_unavailable");
  daemon = spawn(process.execPath, [path.join(ROOT, "app/daemon.mjs")], { env, stdio: ["ignore", "ignore", "pipe"] });
  daemon.stderr.on("data", (data) => { daemonErrors += data; });
  await until(async () => !!(await hello({ timeoutMs: 100 })), 5000).catch((err) => { throw new Error(`${err.message}\n${daemonErrors}`); });
  const reply = await appSessionRequest({ tool: "get_app_state", sessionId, args: { app_ref: { name: "Survivor again" } } });
  assert.equal(reply.ok, true, JSON.stringify(reply));
  assert.equal(reply.data.name, "Survivor again", "the same session id works on the replacement daemon without a host reload");
});

test("MCP EOF releases a completed mouse-down and helper shutdown aborts active children", async () => {
  const host = mcp();
  await host.tool("get_app_state", { app_ref: { name: "Disconnected host" } });
  await host.tool("left_mouse_down", { target: { type: "coordinate", space: "screen", x: 10, y: 10 } });
  await closeHost(host);
  assert.ok(calls().some((item) => item.method === "release_input" && item.appName === "Disconnected host" && item.pointerDown));
  const held = appSessionRequest({ tool: "hold_key", sessionId: "helper-exit", args: { text: "helper-exit" } });
  const result = held.catch((err) => ({ error: err.code }));
  await until(() => calls().some((item) => item.method === "child_started" && item.text === "helper-exit"));
  const instance = calls().find((item) => item.method === "child_started" && item.text === "helper-exit").instance;
  const exit = new Promise((resolve) => daemon.once("exit", resolve));
  daemon.kill("SIGTERM");
  assert.equal(await exit, 0, daemonErrors);
  await result;
  assert.ok(calls().some((item) => item.method === "child_released" && item.instance === instance));
  assert.ok(!calls().some((item) => item.method === "late_input"));
});
