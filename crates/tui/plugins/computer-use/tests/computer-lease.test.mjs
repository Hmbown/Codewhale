// The human/agent control lease on a shared Codewhale Computer: input tools
// refuse with computer_busy_human_driving while a person drives, observation
// keeps working, and hand-back restores input without a restart.
import { test, after } from "node:test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { readLease, inputRefusal, watchLease, HUMAN_DRIVING, LEASE_UNREADABLE } from "../src/lease.mjs";
import { LEASE_GATED_TOOLS, OBSERVATION_TOOLS } from "../src/tools.mjs";

const ROOT = path.resolve(import.meta.dirname, "..");
const dir = fs.mkdtempSync(path.join(os.tmpdir(), "cu-lease-"));
const LEASE = path.join(dir, "lease.json");
const writeLease = (value) => {
  const tmp = `${LEASE}.tmp`;
  fs.writeFileSync(tmp, typeof value === "string" ? value : JSON.stringify(value));
  fs.renameSync(tmp, LEASE);
};
after(() => fs.rmSync(dir, { recursive: true, force: true }));

test("readLease: unconfigured, absent, human, expired, agent and malformed", () => {
  assert.equal(readLease({ file: null }).state, "none");
  assert.equal(readLease({ file: path.join(dir, "missing.json") }).state, "none");
  const now = Date.parse("2026-09-22T20:00:00Z");
  const read = (text) => () => text;
  assert.equal(readLease({ file: "x", now, read: read('{"holder":"human","since":"2026-09-22T19:59:00Z"}') }).state, "human");
  assert.equal(readLease({ file: "x", now, read: read('{"holder":"human","expires_at":"2026-09-22T20:05:00Z"}') }).state, "human");
  const expired = readLease({ file: "x", now, read: read('{"holder":"human","expires_at":"2026-09-22T19:00:00Z"}') });
  assert.equal(expired.state, "none");
  assert.equal(expired.expired, true);
  assert.equal(readLease({ file: "x", now, read: read('{"holder":"agent"}') }).state, "agent");
  assert.equal(readLease({ file: "x", now, read: read('{"holder":null}') }).state, "none");
  for (const bad of ["{", "[]", '{"holder":"robot"}', '{"holder":"human","expires_at":"soon"}']) {
    assert.equal(readLease({ file: "x", now, read: read(bad) }).state, "unreadable", bad);
  }
  const eacces = () => { throw Object.assign(new Error("denied"), { code: "EACCES" }); };
  assert.equal(readLease({ file: "x", read: eacces }).state, "unreadable");
});

test("inputRefusal names the code, is retryable, and is null when the agent may act", () => {
  const human = inputRefusal("left_click", { state: "human", since: "t0", expires_at: null, generation: 3 });
  assert.equal(human.code, HUMAN_DRIVING);
  assert.equal(human.extra.retryable, true);
  assert.equal(human.extra.lease.generation, 3);
  assert.equal(inputRefusal("left_click", { state: "unreadable", reason: "not JSON" }).code, LEASE_UNREADABLE);
  assert.equal(inputRefusal("left_click", { state: "agent" }), null);
  assert.equal(inputRefusal("left_click", { state: "none" }), null);
});

test("every input tool is gated and no observation tool is", () => {
  for (const name of ["left_click", "type", "key", "scroll", "left_click_drag", "left_mouse_down", "mouse_move",
    "browser_start", "browser_navigate", "browser_click", "browser_type", "open_application", "kill_app",
    "write_clipboard", "set_value", "perform_action", "app_script"]) {
    assert.ok(LEASE_GATED_TOOLS.has(name), `${name} must be lease-gated`);
  }
  for (const name of OBSERVATION_TOOLS) assert.ok(!LEASE_GATED_TOOLS.has(name), `${name} is observation`);
  for (const name of ["stop_computer_control", "browser_stop", "computer_switch", "run_actions"]) {
    assert.ok(!LEASE_GATED_TOOLS.has(name), `${name} is not itself gated`);
  }
});

test("watchLease fires when a person takes the lease", async () => {
  let state = '{"holder":"agent"}';
  const seen = [];
  const stop = watchLease((s) => seen.push(s), { file: "x", intervalMs: 10, read: () => state });
  await new Promise((r) => setTimeout(r, 40));
  state = '{"holder":"human"}';
  await new Promise((r) => setTimeout(r, 40));
  stop();
  assert.deepEqual(seen, ["human"]);
});

// ---- the real MCP server, with a lease file ----
function startServer() {
  const env = { ...process.env, CODEWHALE_CU_LEASE_FILE: LEASE, CODEWHALE_CU_APP: "off", CODEWHALE_CU_APP_WARM: "off",
    CODEWHALE_CU_STATE_DIR: fs.mkdtempSync(path.join(dir, "state-")), CODEWHALE_CU_RECORDINGS_DIR: dir };
  const child = spawn(process.execPath, [path.join(ROOT, "mcp/server.mjs")], { env, stdio: ["pipe", "pipe", "ignore"] });
  let buf = "";
  const pending = new Map();
  let nextId = 1;
  child.stdout.on("data", (chunk) => {
    buf += chunk;
    let i;
    while ((i = buf.indexOf("\n")) >= 0) {
      const line = buf.slice(0, i); buf = buf.slice(i + 1);
      let msg; try { msg = JSON.parse(line); } catch { continue; }
      pending.get(msg.id)?.(msg); pending.delete(msg.id);
    }
  });
  const rpc = (method, params) => new Promise((resolve) => {
    const id = nextId++;
    pending.set(id, resolve);
    child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", id, method, params })}\n`);
  });
  const tool = async (name, args = {}) => JSON.parse((await rpc("tools/call", { name, arguments: args })).result.content[0].text);
  return { child, rpc, tool };
}

test("MCP server: input refused under a human lease, observation not, input back after hand-back", async (t) => {
  const { child, rpc, tool } = startServer();
  t.after(() => child.kill());
  await rpc("initialize", { protocolVersion: "2025-06-18" });

  writeLease({ holder: "human", since: new Date().toISOString(), expires_at: null, generation: 1 });
  let r = await tool("left_click", { target: { type: "coordinate", x: 10, y: 10 } });
  assert.equal(r.ok, false);
  assert.equal(r.error.code, HUMAN_DRIVING);
  assert.equal(r.retryable, true);
  r = await tool("click", { target: { type: "coordinate", x: 10, y: 10 } });
  assert.equal(r.error.code, HUMAN_DRIVING, "merged names resolve before the gate");
  r = await tool("browser", { action: "navigate", url: "https://example.com" });
  assert.equal(r.error.code, HUMAN_DRIVING);
  r = await tool("run_actions", { steps: [{ tool: "type", arguments: { text: "x" } }] });
  assert.equal(r.error.code, HUMAN_DRIVING, "run_actions steps are gated");
  // Observation is not refused by the lease (it may fail for host reasons).
  r = await tool("browser", { action: "status" });
  assert.equal(r.ok, true);
  r = await tool("screenshot");
  assert.notEqual(r.error?.code, HUMAN_DRIVING);

  writeLease("{ not json");
  r = await tool("type", { text: "x" });
  assert.equal(r.error.code, LEASE_UNREADABLE, "a lease that cannot be read fails closed");

  writeLease({ holder: "agent", since: new Date().toISOString(), generation: 2 });
  r = await tool("left_click", { target: { type: "coordinate", x: 10, y: 10 } });
  assert.notEqual(r.error?.code, HUMAN_DRIVING, "hand-back restores input with no restart");
  assert.notEqual(r.error?.code, "control_stopped");
});
