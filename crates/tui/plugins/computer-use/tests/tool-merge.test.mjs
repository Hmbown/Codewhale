// Slice D: the advertised surface is merged; the wire names stay callable as
// aliases. Unit tests pin the expansion rules; a spawned server proves the
// advertised list and that both merged and alias calls route identically.
import { test, before, after } from "node:test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import url from "node:url";
import { TOOLS, TOOL_NAMES, resolveTool, parseGrant } from "../src/tools.mjs";

const __dirname = path.dirname(url.fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, "..");

// ---------- unit: expansion rules ----------

test("click expands by button and clicks; unsupported shapes are bad_args", () => {
  const t = { type: "element", index: 3 };
  assert.deepEqual(resolveTool("click", { target: t }), { name: "left_click", args: { target: t } });
  assert.deepEqual(resolveTool("click", { target: t, clicks: 2 }), { name: "double_click", args: { target: t } });
  assert.deepEqual(resolveTool("click", { target: t, clicks: 3 }), { name: "triple_click", args: { target: t } });
  assert.deepEqual(resolveTool("click", { target: t, button: "right" }), { name: "right_click", args: { target: t } });
  assert.deepEqual(resolveTool("click", { target: t, button: "middle" }), { name: "middle_click", args: { target: t } });
  // strategy is an a11y left-click concept; it must not leak to right/middle.
  assert.deepEqual(resolveTool("click", { target: t, strategy: "a11y" }), { name: "left_click", args: { target: t, strategy: "a11y" } });
  assert.deepEqual(resolveTool("click", { target: t, button: "right", strategy: "a11y" }), { name: "right_click", args: { target: t } });
  assert.throws(() => resolveTool("click", { target: t, clicks: 4 }), /1-3 clicks/);
  assert.throws(() => resolveTool("click", { target: t, button: "right", clicks: 2 }), /right x1/);
});

test("pointer, clipboard, recording and computer expand with their requirements enforced", () => {
  const t = { type: "element", index: 0 };
  assert.deepEqual(resolveTool("pointer", { action: "move", target: t }), { name: "mouse_move", args: { target: t } });
  assert.deepEqual(resolveTool("pointer", { action: "up" }), { name: "left_mouse_up", args: {} });
  assert.throws(() => resolveTool("pointer", { action: "tap", target: t }), /"move", "down" or "up"/);
  assert.throws(() => resolveTool("pointer", {}), /pointer action/);

  assert.deepEqual(resolveTool("clipboard", { action: "read" }), { name: "read_clipboard", args: { computer: undefined } });
  assert.deepEqual(resolveTool("clipboard", { action: "write", text: "x" }), { name: "write_clipboard", args: { text: "x", computer: undefined } });
  assert.throws(() => resolveTool("clipboard", { action: "write" }), /requires text/);
  assert.throws(() => resolveTool("clipboard", { action: "paste" }), /"read" or "write"/);

  assert.deepEqual(resolveTool("recording", { action: "list" }), { name: "recording_list", args: {} });
  assert.deepEqual(resolveTool("recording", { action: "status", id: "r1" }), { name: "recording_status", args: { id: "r1" } });
  assert.throws(() => resolveTool("recording", { action: "stop" }), /requires id/);
  assert.throws(() => resolveTool("recording", { action: "pause" }), /start, stop, status or list/);

  assert.deepEqual(resolveTool("computer", { action: "list" }), { name: "computer_list", args: {} });
  assert.deepEqual(resolveTool("computer", { action: "switch", id: "mac2" }), { name: "computer_switch", args: { computer: "mac2" } });
  assert.deepEqual(resolveTool("computer", { action: "register", id: "box", transport: "ssh", host: "h" }), { name: "computer_register", args: { transport: "ssh", host: "h", computer: "box" } });
  assert.deepEqual(resolveTool("computer", { action: "spawn", id: "task-x", transport: "docker" }), { name: "computer_spawn", args: { transport: "docker", computer: "task-x" } });
  assert.throws(() => resolveTool("computer", { action: "switch" }), /requires id/);
  assert.throws(() => resolveTool("computer", { action: "reset" }), /list, switch, register, spawn or remove/);
});

test("consent expands by action; app identity or foreground scope required", () => {
  assert.deepEqual(resolveTool("consent", { action: "status" }), { name: "consent_status", args: { computer: undefined } });
  assert.deepEqual(resolveTool("consent", { action: "allow", app: "Safari", remember: true }), { name: "consent_allow", args: { app: "Safari", remember: true } });
  assert.deepEqual(resolveTool("consent", { action: "deny", scope: "foreground" }), { name: "consent_deny", args: { scope: "foreground" } });
  assert.deepEqual(resolveTool("consent", { action: "revoke", bundle_id: "com.apple.Safari" }), { name: "consent_revoke", args: { bundle_id: "com.apple.Safari" } });
  assert.throws(() => resolveTool("consent", { action: "allow" }), /needs an app/);
  assert.throws(() => resolveTool("consent", { action: "ponder" }), /status, allow, deny or revoke/);
});

test("key with duration routes to hold semantics; conflicts are bad_args", () => {
  assert.deepEqual(resolveTool("key", { text: "a" }), { name: "key", args: { text: "a" } });
  assert.deepEqual(resolveTool("key", { text: "shift", duration: 1.5 }), { name: "hold_key", args: { text: "shift", duration: 1.5 } });
  assert.throws(() => resolveTool("key", { text: "a", duration: 1, repeat: 2 }), /cannot be combined/);
  assert.throws(() => resolveTool("key", { text: "a", duration: 99 }), /0.05..30/);
});

test("trajectory actions expand to their wire tools; misuse fails as bad_args", () => {
  assert.deepEqual(resolveTool("trajectory", { action: "start" }), { name: "trajectory_start", args: {} });
  assert.deepEqual(resolveTool("trajectory", { action: "replay", id: "traj-x.jsonl", dry_run: true }), { name: "trajectory_replay", args: { id: "traj-x.jsonl", dry_run: true } });
  assert.throws(() => resolveTool("trajectory", { action: "wat" }), (e) => e.code === "bad_args" && /start, stop, status or replay/.test(e.message));
});

test("parseGrant expands read-only and merged names into wire sets", () => {
  assert.equal(parseGrant(undefined), null);
  assert.equal(parseGrant("   "), null);
  const ro = parseGrant("read-only");
  assert.ok(ro.has("wait") && ro.has("list_apps") && ro.has("get_value") && ro.has("computer_list"));
  assert.ok(!ro.has("left_click") && !ro.has("kill_app") && !ro.has("trajectory_start") && !ro.has("type"));
  const mixed = parseGrant("click, list_apps ,browser_status");
  for (const wire of ["left_click", "double_click", "middle_click", "list_apps", "browser_status"]) assert.ok(mixed.has(wire), wire);
  assert.ok(mixed.has("right_click"), "click expansion admits every button"); 
});

test("browser actions expand to their wire tools; misuse fails as bad_args naming browser", () => {
  assert.deepEqual(resolveTool("browser", { action: "status" }), { name: "browser_status", args: {} });
  assert.deepEqual(resolveTool("browser", { action: "navigate", url: "https://a.test" }), { name: "browser_navigate", args: { url: "https://a.test" } });
  assert.deepEqual(resolveTool("browser", { action: "click", selector: "#x" }), { name: "browser_click", args: { selector: "#x" } });
  assert.throws(() => resolveTool("browser", { action: "click", selector: "#x", point: { x: 1, y: 2 } }), (e) => e.code === "bad_args" && /not both/.test(e.message));
  assert.deepEqual(resolveTool("browser", { action: "type", text: "hi", enter: true }), { name: "browser_type", args: { text: "hi", enter: true } });
  assert.throws(() => resolveTool("browser", { action: "navigate" }), (e) => e.code === "bad_args" && /requires url/.test(e.message));
  assert.throws(() => resolveTool("browser", { action: "click" }), (e) => e.code === "bad_args" && /needs selector/.test(e.message));
  assert.throws(() => resolveTool("browser", { action: "type" }), (e) => e.code === "bad_args" && /requires text/.test(e.message));
  assert.throws(() => resolveTool("browser", { action: "fly" }), (e) => e.code === "bad_args" && /browser action must be start, status, navigate/.test(e.message));
});

test("the advertised list is the merged surface; aliases are not listed", () => {
  const advertised = TOOLS.filter((t) => t.hidden !== true).map((t) => t.name);
  const hidden = TOOLS.filter((t) => t.hidden === true).map((t) => t.name);
  assert.equal(advertised.length, 38, `advertised surface is ${advertised.length}`);
  assert.equal(hidden.length, 35, `hidden aliases are ${hidden.length}`);
  for (const merged of ["click", "pointer", "clipboard", "recording", "computer", "browser", "trajectory", "consent"]) assert.ok(advertised.includes(merged), merged);
  for (const straight of ["list_sessions", "kill_app", "set_window_frame"]) assert.ok(advertised.includes(straight), straight);
  for (const gone of ["left_click", "double_click", "triple_click", "right_click", "middle_click", "mouse_move",
    "left_mouse_down", "left_mouse_up", "read_clipboard", "write_clipboard",
    "recording_start", "recording_stop", "recording_status", "recording_list",
    "computer_list", "computer_switch", "computer_register", "computer_spawn", "computer_remove", "hold_key",
    "consent_status", "consent_allow", "consent_deny", "consent_revoke",
    "browser_start", "browser_status", "browser_navigate", "browser_click", "browser_type", "browser_screenshot", "browser_stop",
    "trajectory_start", "trajectory_stop", "trajectory_status", "trajectory_replay"]) {
    assert.ok(!advertised.includes(gone), `${gone} must not be advertised`);
    assert.ok(TOOL_NAMES.has(gone), `${gone} must stay callable as an alias`);
  }
  assert.equal(advertised.length + hidden.length, TOOLS.length);
});

// ---------- protocol: both surfaces over the wire ----------

const stateDir = fs.mkdtempSync(path.join(os.tmpdir(), "cu-merge-state-"));
let server;
let buf = "";
const pending = new Map();
let nextId = 1;

function rpc(method, params) {
  const id = nextId++;
  return new Promise((resolve, reject) => {
    const t = setTimeout(() => { pending.delete(id); reject(new Error(`timeout: ${method}`)); }, 20_000);
    pending.set(id, (msg) => { clearTimeout(t); resolve(msg); });
    server.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n");
  });
}

async function tool(name, args = {}) {
  const res = await rpc("tools/call", { name, arguments: args });
  return JSON.parse(res.result.content[0].text);
}

before(() => {
  server = spawn("node", [path.join(ROOT, "mcp", "server.mjs")], {
    env: { ...process.env, CODEWHALE_CU_STATE_DIR: stateDir, CODEWHALE_CU_RECORDINGS_DIR: path.join(stateDir, "rec") },
    stdio: ["pipe", "pipe", "pipe"],
  });
  server.stdout.on("data", (c) => {
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
});

after(() => { try { server.stdin.end(); } catch {} server?.kill("SIGTERM"); });

test("tools/list serves exactly the advertised union, validated shapes included", async () => {
  const res = await rpc("tools/list", {});
  const names = res.result.tools.map((t) => t.name);
  assert.equal(names.length, 38);
  assert.ok(names.includes("click") && names.includes("pointer") && names.includes("clipboard") && names.includes("recording") && names.includes("computer") && names.includes("consent"));
  assert.ok(names.includes("list_sessions") && names.includes("kill_app") && names.includes("browser") && names.includes("set_window_frame") && names.includes("trajectory"));
  assert.ok(!names.includes("left_click") && !names.includes("hold_key") && !names.includes("read_clipboard") && !names.includes("consent_allow"));
});

test("a merged call and its wire alias route to the same place (no dispatch without a raster)", async () => {
  const target = { type: "coordinate", x: 5, y: 5 };
  const merged = await tool("click", { target });
  const alias = await tool("left_click", { target });
  assert.equal(merged.ok, false);
  assert.equal(merged.error.code, "no_raster", "click must reach the coordinate path, which fails closed without a screenshot");
  assert.equal(alias.error.code, merged.error.code);
  assert.equal(alias.error.message, merged.error.message);
});

test("merged-call validation failures name the requested tool and never dispatch", async () => {
  const badClicks = await tool("click", { target: { type: "coordinate", x: 5, y: 5 }, clicks: 4 });
  assert.equal(badClicks.error.code, "bad_args");
  assert.match(badClicks.error.message, /^click supports/);

  const missingTarget = await tool("click", {});
  assert.equal(missingTarget.error.code, "bad_args");
  assert.match(missingTarget.error.message, /^click requires "target"/);

  const pointer = await tool("pointer", { action: "tap", target: { type: "coordinate", x: 5, y: 5 } });
  assert.equal(pointer.error.code, "bad_args");
  assert.match(pointer.error.message, /pointer action/);

  const clip = await tool("clipboard", { action: "write" });
  assert.equal(clip.error.code, "bad_args");
  assert.match(clip.error.message, /^clipboard action "write" requires text/);

  const rec = await tool("recording", { action: "stop" });
  assert.equal(rec.error.code, "bad_args");
  assert.match(rec.error.message, /^recording action "stop" requires id/);

  const comp = await tool("computer", { action: "switch" });
  assert.equal(comp.error.code, "bad_args");
  assert.match(comp.error.message, /^computer action "switch" requires id/);

  const hold = await tool("key", { text: "a", duration: 1, repeat: 2 });
  assert.equal(hold.error.code, "bad_args");
  assert.match(hold.error.message, /cannot be combined/);
});

test("the merged computer tool drives the registry exactly like its wire names", async () => {
  const listed = await tool("computer", { action: "list" });
  assert.equal(listed.ok, true);
  assert.equal(listed.active, "local");
  assert.ok(Array.isArray(listed.computers));

  const switched = await tool("computer", { action: "switch", id: "local" });
  assert.equal(switched.ok, true);
  assert.equal(switched.active, "local");
});
