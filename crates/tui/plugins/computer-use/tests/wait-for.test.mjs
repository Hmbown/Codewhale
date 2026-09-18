// wait_for polling, element-targeted type/key, and the persistent agent
// channel: real MCP server over stdio with the injected fake backend, plus
// the remote agent's --serve mode driven as a local child process.
import { test, before, after } from "node:test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import url from "node:url";
import { ensureSshChannel, channelRequest, b64 } from "../src/transport.mjs";

const __dirname = path.dirname(url.fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, "..");
const FAKE = path.join(__dirname, "fixtures", "fake-backend.mjs");

const stateDir = fs.mkdtempSync(path.join(os.tmpdir(), "cu-wait-state-"));
const recDir = fs.mkdtempSync(path.join(os.tmpdir(), "cu-wait-rec-"));
const callsFile = path.join(fs.mkdtempSync(path.join(os.tmpdir(), "cu-wait-")), "calls.jsonl");
const controlFile = callsFile + ".control.json";

let server;
let buf = "";
const pending = new Map();
let nextId = 1;

function rpc(method, params, timeoutMs = 30_000) {
  const id = nextId++;
  return new Promise((resolve, reject) => {
    const t = setTimeout(() => { pending.delete(id); reject(new Error(`timeout: ${method}`)); }, timeoutMs);
    pending.set(id, (msg) => { clearTimeout(t); resolve(msg); });
    server.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n");
  });
}

async function tool(name, args = {}) {
  const res = await rpc("tools/call", { name, arguments: args });
  assert.ok(res.result, `${name}: protocol error ${JSON.stringify(res.error ?? {})}`);
  return JSON.parse(res.result.content[0].text);
}

function calls(method) {
  if (!fs.existsSync(callsFile)) return [];
  return fs.readFileSync(callsFile, "utf8").split("\n").filter(Boolean).map((l) => JSON.parse(l)).filter((c) => c.method === method);
}

function setControl(obj) {
  if (obj == null) fs.rmSync(controlFile, { force: true });
  else fs.writeFileSync(controlFile, JSON.stringify(obj));
}

before(async () => {
  server = spawn("node", [path.join(ROOT, "mcp", "server.mjs")], {
    env: {
      ...process.env,
      CODEWHALE_CU_APP: "off",
      CODEWHALE_CU_STATE_DIR: stateDir,
      CODEWHALE_CU_RECORDINGS_DIR: recDir,
      CODEWHALE_CU_TEST_BACKEND: FAKE,
      FAKE_BACKEND_CALLS: callsFile,
      FAKE_BACKEND_CONTROL: controlFile,
    },
    stdio: ["pipe", "pipe", "pipe"],
  });
  server.stderr.on("data", (d) => process.stderr.write(`[server] ${d}`));
  server.stdout.setEncoding("utf8");
  server.stdout.on("data", (d) => {
    buf += d;
    let i;
    while ((i = buf.indexOf("\n")) !== -1) {
      const line = buf.slice(0, i).trim();
      buf = buf.slice(i + 1);
      if (!line) continue;
      try {
        const msg = JSON.parse(line);
        if (msg.id && pending.has(msg.id)) { pending.get(msg.id)(msg); pending.delete(msg.id); }
      } catch {}
    }
  });
  const init = await rpc("initialize", { protocolVersion: "2025-06-18" });
  assert.equal(init.result.serverInfo.name, "codewhale-cu");
});

after(() => {
  server?.kill("SIGTERM");
  for (const d of [stateDir, recDir, path.dirname(callsFile)]) { try { fs.rmSync(d, { recursive: true, force: true }); } catch {} }
});

test("wait_for returns matched elements bound to a fresh targetable state_id", async () => {
  const r = await tool("wait_for", { query: "OK", role: "AXButton", timeout: 5 });
  assert.equal(r.ok, true, JSON.stringify(r));
  assert.equal(r.matched, true);
  assert.equal(r.matched_count, 1);
  assert.equal(r.elements[0].label, "OK");
  assert.ok(r.state_id, "the satisfying observation is bound");
  // The returned state_id is targetable: the button resolves and presses.
  const press = await tool("perform_action", { target: { type: "element", state_id: r.state_id, index: r.elements[0].index }, action: "AXPress" });
  assert.equal(press.ok, true, JSON.stringify(press.error));
});

test("wait_for absent is satisfied immediately when nothing matches", async () => {
  const r = await tool("wait_for", { query: "no such element anywhere", state: "absent", timeout: 5 });
  assert.equal(r.ok, true, JSON.stringify(r));
  assert.equal(r.matched, true);
  assert.equal(r.matched_count, 0);
  assert.equal(r.timed_out, undefined);
});

test("wait_for times out honestly when the predicate never holds", async () => {
  const before = calls("get_app_state").length;
  const r = await tool("wait_for", { query: "never-present-label", timeout: 0.6, interval: 150 });
  assert.equal(r.ok, true, JSON.stringify(r));
  assert.equal(r.matched, false);
  assert.equal(r.timed_out, true);
  assert.ok(r.polls >= 2, `expected several polls, got ${r.polls}`);
  assert.equal(r.state_id, undefined, "a timed-out wait binds nothing");
  assert.ok(calls("get_app_state").length - before >= r.polls - 1, "ephemeral polls reached the backend");
});

test("ephemeral wait_for polls do not evict earlier states", async () => {
  const st = await tool("get_app_state", { app_ref: { name: "FakeApp" } });
  assert.ok(st.state_id);
  // ~30 polls: more than the 24-state cache cap. If polls were cached, st
  // would be evicted; ephemeral polling keeps it targetable.
  const w = await tool("wait_for", { query: "never-present-label", timeout: 3, interval: 100 });
  assert.equal(w.timed_out, true);
  assert.ok(w.polls > 24, `expected >24 polls to prove non-caching, got ${w.polls}`);
  const focus = await tool("focus", { target: { type: "element", state_id: st.state_id, index: 1 } });
  assert.equal(focus.ok, true, JSON.stringify(focus.error));
});

test("wait_for validates its arguments", async () => {
  const bad = async (args, match) => {
    const res = await rpc("tools/call", { name: "wait_for", arguments: args });
    assert.match(res.error?.message ?? "", match, JSON.stringify(res));
  };
  await bad({}, /needs a query and\/or role/);
  await bad({ query: "x", state: "bogus" }, /state must be "present" or "absent"/);
  await bad({ query: "x", timeout: 999 }, /timeout/);
  await bad({ query: "x", interval: 5 }, /interval/);
});

test("type with an element target focuses first, then types", async () => {
  const st = await tool("get_app_state", { app_ref: { name: "FakeApp" } });
  const beforeFocus = calls("focus").length;
  const beforeType = calls("type").length;
  setControl({ found: true, element: { role: "AXTextField", position: { x: 10, y: 60 }, size: { w: 150, h: 25 } }, reason: null });
  try {
    const r = await tool("type", { text: "hello", target: { type: "element", state_id: st.state_id, index: 8 } });
    assert.equal(r.ok, true, JSON.stringify(r.error));
    const focusCalls = calls("focus");
    const typeCalls = calls("type");
    assert.equal(focusCalls.length, beforeFocus + 1);
    assert.equal(typeCalls.length, beforeType + 1);
    assert.deepEqual(focusCalls.at(-1).args.target.path, [0, 2], "focus received the semantic element target");
    assert.equal(typeCalls.at(-1).args.text, "hello");
  } finally { setControl(null); }
});

test("key with an element target focuses first; a coordinate target is refused", async () => {
  const st = await tool("get_app_state", { app_ref: { name: "FakeApp" } });
  setControl({ found: true, element: { role: "AXTextField", position: { x: 10, y: 60 }, size: { w: 150, h: 25 } }, reason: null });
  try {
    const r = await tool("key", { text: "return", target: { type: "element", state_id: st.state_id, index: 8 } });
    assert.equal(r.ok, true, JSON.stringify(r.error));
  } finally { setControl(null); }
  const refused = await tool("type", { text: "x", target: { type: "coordinate", x: 1, y: 1 } });
  assert.equal(refused.ok, false);
  assert.equal(refused.error.code, "bad_target");
});

test("type target fails closed when the element went stale before any text is sent", async () => {
  const st = await tool("get_app_state", { app_ref: { name: "FakeApp" } });
  setControl({ found: false, element: null, reason: "element_gone" });
  const before = calls("type").length;
  try {
    const r = await tool("type", { text: "should never land", target: { type: "element", state_id: st.state_id, index: 1 } });
    assert.equal(r.ok, false);
    assert.equal(r.error.code, "element_stale");
    assert.equal(r.stage, "focus");
    assert.equal(calls("type").length, before, "no keystrokes after the failed focus");
  } finally {
    setControl(null);
  }
});

// ---------- persistent remote agent ----------

test("agent --serve keeps its backend binding across requests", async t => {
  const child = spawn("node", [path.join(ROOT, "agent.mjs"), "--serve"], {
    env: { ...process.env, CODEWHALE_CU_TEST_BACKEND: FAKE, FAKE_BACKEND_CALLS: callsFile },
    stdio: ["pipe", "pipe", "pipe"],
  });
  t.after(() => child.kill("SIGKILL"));
  const replies = new Map();
  let rbuf = "";
  child.stdout.setEncoding("utf8").on("data", (d) => {
    rbuf += d;
    let i;
    while ((i = rbuf.indexOf("\n")) !== -1) {
      const msg = JSON.parse(rbuf.slice(0, i));
      rbuf = rbuf.slice(i + 1);
      replies.set(msg.id, msg);
    }
  });
  const send = async (id, toolName, args = {}) => {
    child.stdin.write(b64({ id, tool: toolName, args }) + "\n");
    const deadline = Date.now() + 10_000;
    while (!replies.has(id) && Date.now() < deadline) await new Promise((r) => setTimeout(r, 10));
    assert.ok(replies.has(id), `no reply for ${toolName}`);
    return replies.get(id);
  };
  assert.equal((await send(1, "platform")).ok, true);
  const open = await send(2, "open_application", { name: "FakeApp" });
  assert.equal(open.ok, true, JSON.stringify(open));
  const typed = await send(3, "type", { text: "hi" });
  assert.equal(typed.ok, true);
  assert.equal(typed.data.bound_app, "FakeApp", "the second request reused the first request's binding");
  // Held input is no longer blanket-refused in persistent mode: it reaches
  // the backend, which the fixture answers.
  const held = await send(4, "left_mouse_down", { target: { type: "coordinate", x: 5, y: 5 } });
  assert.equal(held.ok, true, JSON.stringify(held));
  const refused = await send(5, "shell_escape");
  assert.equal(refused.ok, false);
  assert.equal(refused.error.code, "tool_not_allowed");
});

test("one-shot agent still refuses operations that need a persistent session", async () => {
  const out = spawn("node", [path.join(ROOT, "agent.mjs"), b64({ tool: "left_mouse_down", args: {} })], {
    env: { ...process.env, CODEWHALE_CU_TEST_BACKEND: FAKE },
    stdio: ["ignore", "pipe", "pipe"],
  });
  let stdout = "";
  out.stdout.setEncoding("utf8").on("data", (d) => { stdout += d; });
  await new Promise((resolve) => out.once("close", resolve));
  const reply = JSON.parse(stdout.trim().split("\n").pop());
  assert.equal(reply.ok, false);
  assert.equal(reply.error.code, "persistent_session_required");
});

test("channelRequest correlates replies and survives out-of-order completion", async t => {
  const binding = {};
  const ch = ensureSshChannel(binding, ["node", path.join(ROOT, "agent.mjs"), "--serve"]);
  t.after(() => { try { ch.proc?.kill("SIGKILL"); } catch {} });
  // Override spawn is not needed here: the channel factory takes argv, so a
  // local agent process stands in for `ssh host node agent --serve`.
  assert.equal(ch.alive, true);
  const p1 = channelRequest(ch, { tool: "platform" }, 10_000);
  const p2 = channelRequest(ch, { tool: "not_a_tool" }, 10_000);
  const [r1, r2] = await Promise.all([p1, p2]);
  assert.equal(r1.ok, true);
  assert.equal(r2.ok, false);
  assert.equal(r2.error.code, "tool_not_allowed");
  ch.proc.kill("SIGKILL");
  await assert.rejects(channelRequest(ch, { tool: "platform" }, 500), (e) => e.code === "remote_session_lost");
});
