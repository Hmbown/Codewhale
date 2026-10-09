// Input ownership at the MCP boundary: keystrokes and pointer events are judged
// against the app that owns the window receiving them, not the app the agent
// last observed. Runs the real server with the fake backend, which reports the
// owner it is told about (FAKE_BACKEND_OWNER) and records every backend call.
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import url from "node:url";

const ROOT = path.resolve(path.dirname(url.fileURLToPath(import.meta.url)), "..");
const XTERM = { name: "xterm", comm: "xterm", pid: 4242, wm_class: "xterm.XTerm", title: "root", window: "0x00e0000c" };

function startServer(t, owner) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "cu-owner-"));
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const calls = path.join(dir, "calls.jsonl");
  const server = spawn("node", [path.join(ROOT, "mcp/server.mjs")], {
    env: {
      ...process.env,
      CODEWHALE_CU_STATE_DIR: path.join(dir, "state"),
      CODEWHALE_CU_RECORDINGS_DIR: path.join(dir, "rec"),
      CODEWHALE_CU_TEST_BACKEND: path.join(ROOT, "tests/fixtures/fake-backend.mjs"),
      FAKE_BACKEND_CALLS: calls,
      FAKE_BACKEND_OWNER: owner === null ? "null" : JSON.stringify(owner),
    },
    stdio: ["pipe", "pipe", "pipe"],
  });
  t.after(() => { try { server.kill("SIGTERM"); } catch {} });
  const pending = new Map();
  const prompts = [];
  let buf = "";
  let nextId = 1;
  server.stdout.setEncoding("utf8");
  server.stdout.on("data", (chunk) => {
    buf += chunk;
    let i;
    while ((i = buf.indexOf("\n")) !== -1) {
      const line = buf.slice(0, i).trim();
      buf = buf.slice(i + 1);
      if (!line) continue;
      const msg = JSON.parse(line);
      if (msg.method === "elicitation/create" && msg.id !== undefined) {
        // The user approves exactly one call here: consent for xterm, nothing else.
        prompts.push(msg.params.message);
        const approve = msg.params.message === 'Computer Use asks for your decision: consent_allow {"app":"xterm"}';
        server.stdin.write(JSON.stringify({ jsonrpc: "2.0", id: msg.id, result: { action: approve ? "accept" : "decline" } }) + "\n");
        continue;
      }
      if (msg.id !== undefined && pending.has(msg.id)) { pending.get(msg.id)(msg); pending.delete(msg.id); }
    }
  });
  const rpc = (method, params) => new Promise((resolve) => {
    const id = nextId++;
    pending.set(id, resolve);
    server.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n");
  });
  const tool = async (name, args = {}) => {
    const res = await rpc("tools/call", { name, arguments: args });
    const parsed = JSON.parse(res.result.content[0].text);
    return { ...parsed, isError: res.result.isError === true };
  };
  const backendCalls = () => (fs.existsSync(calls) ? fs.readFileSync(calls, "utf8").trim().split("\n").filter(Boolean).map((l) => JSON.parse(l)) : []);
  return {
    init: () => rpc("initialize", { protocolVersion: "2025-06-18", capabilities: { elicitation: {} }, clientInfo: { name: "owner-test", version: "0" } }),
    tool,
    prompts,
    backendCalls,
    methods: () => backendCalls().map((c) => c.method),
  };
}

test("keystrokes are refused when the app that owns the active window has no decision, before the backend sees them", async (t) => {
  const s = startServer(t, XTERM);
  await s.init();
  const r = await s.tool("type", { text: "hello" });
  assert.equal(r.ok, false);
  assert.equal(r.error.code, "consent_required");
  assert.match(r.error.message, /xterm/);
  assert.ok(!s.methods().includes("type"), "the backend must not receive the keystrokes");
  assert.ok(s.methods().includes("input_owner"), "the owner is resolved before input");
});

test("pointer input is judged against the window under the point, after a screenshot pins the frame", async (t) => {
  const s = startServer(t, XTERM);
  await s.init();
  assert.equal((await s.tool("screenshot", {})).ok, true);
  const r = await s.tool("left_click", { target: { type: "coordinate", x: 10, y: 20 } });
  assert.equal(r.error.code, "consent_required");
  assert.match(r.error.message, /xterm/);
  assert.ok(!s.methods().includes("left_click"));
  const owner = s.backendCalls().find((c) => c.method === "input_owner");
  assert.equal(owner.args.kind, "pointer");
  // The raster pixel (10,20) is converted to screen points before ownership
  // is judged; the fake screenshot is at scale 2, so the point is (5,10).
  assert.deepEqual(owner.args.point, { x: 5, y: 10 });
});

test("an app the user approved receives input; the approval is recorded through the host, not by the model", async (t) => {
  const s = startServer(t, XTERM);
  await s.init();
  const refused = await s.tool("consent", { action: "allow", app: "xterm" });
  assert.equal(refused.ok, true, "the user approves through elicitation, which this test answers");
  assert.deepEqual(s.prompts, ['Computer Use asks for your decision: consent_allow {"app":"xterm"}']);
  const typed = await s.tool("type", { text: "hi" });
  assert.equal(typed.ok, true);
  assert.ok(s.methods().includes("type"));
});

test("a denied app is refused with app_denied even when it is the only window", async (t) => {
  const s = startServer(t, XTERM);
  await s.init();
  assert.equal((await s.tool("consent", { action: "deny", app: "xterm" })).ok, true);
  const r = await s.tool("key", { text: "Return" });
  assert.equal(r.error.code, "app_denied");
  assert.ok(!s.methods().includes("key"));
});

test("with the desktop as owner there is no application to consent to", async (t) => {
  const s = startServer(t, null);
  await s.init();
  const r = await s.tool("type", { text: "desktop" });
  assert.equal(r.ok, true);
  assert.ok(s.methods().includes("type"));
});
