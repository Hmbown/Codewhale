// app_script: the programmatic interface into apps with a scripting
// dictionary. Local-computer only — a remote channel must never become a
// shell, so ssh/hdc computers refuse before dispatch and the remote agent
// refuses again at its own handler boundary.
import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import url from "node:url";
import { spawn } from "node:child_process";
import { handle } from "../src/app-handler.mjs";

const ROOT = path.resolve(path.dirname(url.fileURLToPath(import.meta.url)), "..");

async function boot(t, env = {}) {
  const stateDir = fs.mkdtempSync(path.join(os.tmpdir(), "cu-script-"));
  const recDir = fs.mkdtempSync(path.join(os.tmpdir(), "cu-script-rec-"));
  const child = spawn("node", [path.join(ROOT, "mcp", "server.mjs")], {
    env: { ...process.env, CODEWHALE_CU_STATE_DIR: stateDir, CODEWHALE_CU_RECORDINGS_DIR: recDir, CODEWHALE_CU_APP: "off", ...env },
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

test("tools/list advertises app_script with a required script", async (t) => {
  const s = await boot(t);
  const tools = (await s.rpc("tools/list", {})).result.tools;
  const def = tools.find((x) => x.name === "app_script");
  assert.ok(def, "app_script must be advertised");
  assert.deepEqual(def.inputSchema.required, ["script"]);
  assert.equal(def.annotations?.readOnlyHint, false, "scripting mutates apps; hosts must not treat it as read-only");
});

test("app_script runs AppleScript and JXA on the local computer", { skip: process.platform !== "darwin" }, async (t) => {
  const s = await boot(t);
  const as = await s.tool("app_script", { script: 'return "whole computer"' });
  assert.equal(as.ok, true);
  assert.equal(as.result, "whole computer");
  const jxa = await s.tool("app_script", { script: '"ok".toUpperCase()', language: "javascript" });
  assert.equal(jxa.ok, true);
  assert.equal(jxa.result, "OK");
  assert.equal(jxa.language, "javascript");
});

test("app_script failures are typed, never opaque", { skip: process.platform !== "darwin" }, async (t) => {
  const s = await boot(t);
  const bad = await s.tool("app_script", { script: "this is not applescript at all" });
  assert.equal(bad.ok, false);
  assert.equal(bad.error.code, "script_error");
  assert.match(bad.error.message, /syntax error|expected/i);
  for (const [args, match] of [
    [{ script: "  " }, /non-empty script/],
    [{ script: "return 1", language: "perl" }, /language/],
    [{ script: "return 1", timeout: 0 }, /timeout/],
    [{ script: "return 1", timeout: 500 }, /timeout/],
  ]) {
    const r = await s.tool("app_script", args);
    assert.equal(r.error?.code, "bad_args", JSON.stringify(args));
    assert.match(r.error.message, match);
  }
});

test("app_script is refused on remote computers before any dispatch", async (t) => {
  const s = await boot(t);
  const reg = await s.tool("computer", { action: "register", id: "faraway", transport: "ssh", host: "192.0.2.1", installAgent: false });
  assert.equal(reg.ok, true);
  const r = await s.tool("app_script", { script: "return 1", computer: "faraway" });
  assert.equal(r.ok, false);
  assert.equal(r.error.code, "unsupported_on_transport");
});

test("the remote agent refuses app_script at its own boundary", async () => {
  const r = await handle({ tool: "app_script", args: { script: "return 1" } }, { computerId: "remote", sessionId: "ssh-serve" });
  assert.equal(r.ok, false);
  assert.equal(r.error.code, "unsupported_on_transport");
});

test("the local handler runs app_script through the normal session machinery", { skip: process.platform !== "darwin" }, async () => {
  const r = await handle({ tool: "app_script", args: { script: 'return "via handler"' } }, { computerId: "local", sessionId: "test-script" });
  assert.equal(r.ok, true);
  assert.equal(r.tool, "app_script");
  assert.equal(r.data.result, "via handler");
});

test("a read-only grant never advertises or calls app_script", async (t) => {
  const s = await boot(t, { CODEWHALE_CU_GRANT: "read-only" });
  const names = (await s.rpc("tools/list", {})).result.tools.map((x) => x.name);
  assert.ok(!names.includes("app_script"));
  assert.equal((await s.tool("app_script", { script: "return 1" })).error?.code, "not_granted");
});

test("a named grant admits app_script exactly", { skip: process.platform !== "darwin" }, async (t) => {
  const s = await boot(t, { CODEWHALE_CU_GRANT: "app_script" });
  const names = (await s.rpc("tools/list", {})).result.tools.map((x) => x.name);
  assert.ok(names.includes("app_script"));
  assert.equal((await s.tool("app_script", { script: "return 42" })).result, "42");
});

test("the kill switch stops scripting too", async (t) => {
  const s = await boot(t);
  assert.equal((await s.tool("stop_computer_control", {})).ok, true);
  const r = await s.tool("app_script", { script: "return 1" });
  assert.equal(r.ok, false);
  assert.equal(r.error.code, "control_stopped");
});
