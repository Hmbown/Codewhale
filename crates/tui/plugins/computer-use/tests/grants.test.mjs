// Capability grants: CODEWHALE_CU_GRANT narrows the advertised and callable
// surface for the whole server process, fixed at launch; the daemon enforces
// the same set independently (covered in session-lifecycle.test.mjs).
import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import url from "node:url";
import { spawn } from "node:child_process";

const ROOT = path.resolve(path.dirname(url.fileURLToPath(import.meta.url)), "..");

async function boot(t, grant) {
  const stateDir = fs.mkdtempSync(path.join(os.tmpdir(), "cu-grant-"));
  const recDir = fs.mkdtempSync(path.join(os.tmpdir(), "cu-grant-rec-"));
  const child = spawn("node", [path.join(ROOT, "mcp", "server.mjs")], {
    env: { ...process.env, CODEWHALE_CU_STATE_DIR: stateDir, CODEWHALE_CU_RECORDINGS_DIR: recDir, CODEWHALE_CU_APP: "off", CODEWHALE_CU_GRANT: grant },
    stdio: ["pipe", "pipe", "pipe"],
  });
  t.after(() => { try { child.stdin.end(); } catch {} child.kill("SIGTERM"); fs.rmSync(stateDir, { recursive: true, force: true }); fs.rmSync(recDir, { recursive: true, force: true }); });
  let buf = "";
  const pending = new Map();
  let nextId = 1;
  child.stdout.on("data", (c) => {
    buf += c.toString();
    let i;
    while ((i = buf.indexOf("\n")) !== -1) {
      const line = buf.slice(0, i).trim();
      buf = buf.slice(i + 1);
      if (!line) continue;
      const msg = JSON.parse(line);
      if (msg.id != null && pending.has(msg.id)) { pending.get(msg.id)(msg); pending.delete(msg.id); }
    }
  });
  const rpc = (method, params) => {
    const id = nextId++;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => { pending.delete(id); reject(new Error(`timeout: ${method}`)); }, 20_000);
      pending.set(id, (msg) => { clearTimeout(timer); resolve(msg); });
      child.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n");
    });
  };
  const tool = async (name, args = {}) => JSON.parse((await rpc("tools/call", { name, arguments: args })).result.content[0].text);
  return { rpc, tool };
}

test("read-only grant: advertised and callable surface is the read-only set (plus parents with a read-only action)", async (t) => {
  const s = await boot(t, "read-only");
  const names = (await s.rpc("tools/list", {})).result.tools.map((x) => x.name);
  for (const kept of ["wait", "list_apps", "get_app_state", "request_access", "stop_computer_control", "computer", "browser", "trajectory"]) {
    assert.ok(names.includes(kept), `${kept} must stay advertised under read-only`);
  }
  for (const gone of ["click", "pointer", "type", "key", "set_value", "kill_app", "set_window_frame", "open_application", "run_actions", "invoke_menu"]) {
    assert.ok(!names.includes(gone), `${gone} must not be advertised under read-only`);
  }
  assert.equal((await s.tool("wait", { seconds: 0.01 })).ok, true);
  assert.equal((await s.tool("computer", { action: "list" })).ok, true);
  assert.equal((await s.tool("trajectory", { action: "status" })).ok, true);
  assert.equal((await s.tool("browser", { action: "status" })).running, false);
  assert.equal((await s.tool("click", { target: { type: "coordinate", x: 5, y: 5 } })).error?.code, "not_granted");
  assert.equal((await s.tool("left_click", { target: { type: "coordinate", x: 5, y: 5 } })).error?.code, "not_granted", "the alias takes the same gate");
  assert.equal((await s.tool("browser", { action: "start" })).error?.code, "not_granted", "an ungranted action on a granted parent is refused");
  const probe = await s.tool("request_access", {});
  assert.equal(probe.grant?.mode, "narrowed");
  assert.ok(probe.grant.tools.includes("wait"));
});

test("named grant admits exactly the named wires and their parents", async (t) => {
  const s = await boot(t, "wait,computer_list");
  const names = (await s.rpc("tools/list", {})).result.tools.map((x) => x.name);
  assert.ok(names.includes("wait") && names.includes("computer") && names.includes("stop_computer_control"));
  assert.ok(!names.includes("list_apps") && !names.includes("click") && !names.includes("kill_app") && !names.includes("trajectory") && !names.includes("browser") && !names.includes("request_access"));
  assert.equal((await s.tool("computer", { action: "list" })).ok, true);
  assert.equal((await s.tool("computer_list", {})).ok, true, "the wire name is callable directly");
  assert.equal((await s.tool("list_apps", {})).error?.code, "not_granted");
  assert.equal((await s.tool("computer", { action: "switch", id: "local" })).error?.code, "not_granted", "computer_switch is not granted");
  assert.equal((await s.tool("stop_computer_control", {})).ok, true, "the safety valve always works");
});
