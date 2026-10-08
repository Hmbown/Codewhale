// The out-of-process route — the desktop app on macOS, the ssh agent on a
// remote computer — is the DEFAULT route for the local computer whenever the
// app is installed, yet it used to prepare arguments differently from the
// in-process route. These tests drive the real server over stdio with that
// route forced (CODEWHALE_CU_TEST_REMOTE) and assert that a coordinate target
// reaches the backend as screen points, with the raster's refusals intact.
import { hostKeysLine, attest, attestParams } from "./fixtures/host-decision.mjs";
import { test, before, after } from "node:test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import url from "node:url";

const __dirname = path.dirname(url.fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, "..");

const stateDir = fs.mkdtempSync(path.join(os.tmpdir(), "cu-wire-state-"));
const recDir = fs.mkdtempSync(path.join(os.tmpdir(), "cu-wire-rec-"));
const callsFile = path.join(fs.mkdtempSync(path.join(os.tmpdir(), "cu-wire-")), "calls.jsonl");

let server;
let buf = "";
const pending = new Map();
let nextId = 1;

function rpc(method, params, timeoutMs = 30_000) {
  const id = nextId++;
  return new Promise((resolve, reject) => {
    const t = setTimeout(() => { pending.delete(id); reject(new Error(`timeout: ${method}`)); }, timeoutMs);
    pending.set(id, (msg) => { clearTimeout(t); resolve(msg); });
    server.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params: attestParams(method, params) }) + "\n");
  });
}

async function tool(name, args = {}) {
  const res = await rpc("tools/call", { name, arguments: args });
  return JSON.parse(res.result.content[0].text);
}

function calls() {
  if (!fs.existsSync(callsFile)) return [];
  return fs.readFileSync(callsFile, "utf8").trim().split("\n").filter(Boolean).map((l) => JSON.parse(l));
}

before(async () => {
  server = spawn("node", [path.join(ROOT, "mcp", "server.mjs")], {
    env: {
      ...process.env,
      CODEWHALE_CU_TEST_REMOTE: process.env.CODEWHALE_CU_WIRE_TEST_ROUTE === "direct" ? "" : "1",
      CODEWHALE_CU_STATE_DIR: stateDir,
      CODEWHALE_CU_RECORDINGS_DIR: recDir,
      CODEWHALE_CU_TEST_BACKEND: path.join(__dirname, "fixtures", "fake-backend.mjs"),
      FAKE_BACKEND_CALLS: callsFile,
      FAKE_BACKEND_CONTROL: callsFile + ".control.json",
    },
    stdio: ["pipe", "pipe", "pipe"],
  });
  server.stdin.write(hostKeysLine());
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
  // The local consent ledger gates app-targeted calls; record the fixture
  // app's decision up front, as a real session would.
  const c = await tool("consent", { action: "allow", app: "FakeApp" });
  assert.equal(c.ok, true, JSON.stringify(c));
});

after(() => {
  server?.kill("SIGTERM");
  for (const d of [stateDir, recDir, path.dirname(callsFile)]) { try { fs.rmSync(d, { recursive: true, force: true }); } catch {} }
});

test("coordinate targets reach an out-of-process backend as screen points, not raster pixels", async () => {
  // The fake backend reports a 2x raster whose origin is (100, 50) in points.
  const shot = await tool("screenshot", { region: [100, 50, 200, 100] });
  assert.equal(shot.ok, true);
  assert.equal(shot.scale, 2);
  assert.deepEqual({ x: shot.points.x, y: shot.points.y }, { x: 100, y: 50 });

  const click = await tool("left_click", { target: { type: "coordinate", x: 80, y: 40 } });
  assert.equal(click.ok, true, JSON.stringify(click));
  const sent = calls().filter((c) => c.method === "left_click").at(-1);
  assert.deepEqual({ x: sent.args.target.x, y: sent.args.target.y }, { x: 140, y: 70 },
    "80/2 + 100 = 140, 40/2 + 50 = 70 — the backend only ever knows the screen");
});

test("out-of-process coordinate actions keep the raster refusals", async () => {
  const outside = await tool("left_click", { target: { type: "coordinate", x: 5000, y: 5000 } });
  assert.equal(outside.ok, false);
  assert.equal(outside.error.code, "target_outside_raster");

  const stale = await tool("left_click", { target: { type: "element", state_id: "s-999", index: 0 } });
  assert.equal(stale.ok, false);
  assert.equal(stale.error.code, "unknown_state");

  const before = calls().filter((c) => c.method === "left_click").length;
  await tool("left_click", { target: { type: "coordinate", x: 1, y: 1 } });
  assert.equal(calls().filter((c) => c.method === "left_click").length, before + 1,
    "refusals never reach the backend; the valid click does");
});

test("element targets retain their identity and AX path on the out-of-process pointer route", async () => {
  const state = await tool("get_app_state", {});
  assert.equal(state.ok, true);
  assert.equal(state.detail, "summary");
  assert.ok(state.elements.every(e => !("path" in e) && !("windowIndex" in e)));
  assert.ok(!state.elements.some(e => e.label === "Save"));
  assert.equal(state.elements.find(e => e.index === 8).value, "Fixture text");
  const target = { type: "element", state_id: state.state_id, index: 1 };

  const click = await tool("left_click", { target });
  assert.equal(click.ok, true, JSON.stringify(click));
  const clicked = calls().filter((c) => c.method === "left_click").at(-1);
  assert.deepEqual({ x: clicked.args.target.x, y: clicked.args.target.y }, { x: 40, y: 35 }, "element center in points");
  assert.deepEqual(clicked.args.target.path, [0, 1]);
  assert.equal(clicked.args.target.windowIndex, 0);
  assert.equal(clicked.args.target.role, "AXButton");
  assert.equal(clicked.args.target.label, "OK");

  const pressed = await tool("perform_action", { target, action: "AXPress" });
  assert.equal(pressed.ok, true, JSON.stringify(pressed));
  const semantic = calls().filter((c) => c.method === "perform_action").at(-1);
  assert.deepEqual(semantic.args.target.path, [0, 1], "semantic actions address the element, not a point");
  assert.equal(semantic.args.target.windowIndex, 0);

  const full = await tool("get_app_state", { detail: "full" });
  assert.equal(full.elements.length, 9);
  assert.deepEqual(full.elements.find(e => e.label === "Save").path, [0, 0, 0]);
});

test("out-of-process OCR observation binds the raster that its text targets use", async () => {
  const state = await tool("get_app_state", { include_ocr: true });
  assert.equal(state.ocr.status, "ok");
  assert.equal(typeof state.ocr.raster.raster_id, "string");
  assert.equal(state.ocr.blocks[0].target.raster_id, state.ocr.raster.raster_id);
  const clicked = await tool("left_click", { target: state.ocr.blocks[0].target });
  assert.equal(clicked.ok, true);
  assert.equal(clicked.target_raster_id, state.ocr.raster.raster_id);
  const target = calls().filter(c => c.method === "left_click").at(-1).args.target;
  assert.deepEqual({ x: target.x, y: target.y }, { x: 140, y: 70 });
  assert.equal((await tool("left_click", { target: { type: "coordinate", x: 400, y: 0 } })).error.code, "target_outside_raster");
});

test("capture pins refuse superseded coordinates before input, while legacy latest-raster calls still work", async () => {
  const first = await tool("screenshot", { region: [100, 50, 200, 100] });
  const latest = await tool("screenshot", { region: [700, 500, 200, 100] });
  assert.notEqual(first.raster_id, latest.raster_id);
  const before = calls().filter(c => c.method === "left_click").length;
  const stale = await tool("left_click", { target: { type: "coordinate", x: 80, y: 40, raster_id: first.raster_id } });
  assert.equal(stale.ok, false);
  assert.equal(stale.error.code, "raster_stale");
  assert.equal(calls().filter(c => c.method === "left_click").length, before);
  const pinned = await tool("left_click", { target: { type: "coordinate", x: 80, y: 40, raster_id: latest.raster_id } });
  assert.equal(pinned.ok, true);
  assert.equal(pinned.target_raster_id, latest.raster_id);
  const sent = calls().filter(c => c.method === "left_click").at(-1).args.target;
  assert.deepEqual({ x: sent.x, y: sent.y }, { x: 740, y: 520 });
  assert.equal("raster_id" in sent, false, "old helpers receive only the resolved target");
  assert.equal((await tool("left_click", { target: { type: "coordinate", x: 1, y: 1 } })).ok, true);
});

test("a pin cannot be malformed or change its coordinate space", async () => {
  const shot = await tool("screenshot");
  const before = calls().filter(c => c.method === "left_click").length;
  for (const raster_id of [null, 7, "", "x".repeat(129)]) {
    const res = await tool("left_click", { target: { type: "coordinate", x: 1, y: 1, raster_id } });
    assert.equal(res.error.code, "bad_target");
  }
  const screen = await tool("left_click", { target: { type: "coordinate", x: 1, y: 1, raster_id: shot.raster_id, space: "screen" } });
  assert.equal(screen.error.code, "bad_target");
  assert.equal(calls().filter(c => c.method === "left_click").length, before);
});

test("nested zooms crop the bound parent file and issue a child identity and geometry", async () => {
  const shot = await tool("screenshot", { region: [100, 50, 200, 100] });
  const first = await tool("zoom", { region: [40, 20, 100, 80], raster_id: shot.raster_id, source: "/untrusted/caller.png" });
  assert.equal(first.ok, true);
  assert.equal(first.parent_raster_id, shot.raster_id);
  assert.notEqual(first.raster_id, shot.raster_id);
  assert.equal(calls().filter(c => c.method === "zoom").at(-1).args.source, shot.file);
  const before = calls().filter(c => c.method === "zoom").length;
  const stale = await tool("zoom", { region: [0, 0, 10, 10], raster_id: shot.raster_id });
  assert.equal(stale.error.code, "raster_stale");
  assert.equal(calls().filter(c => c.method === "zoom").length, before);
  const child = await tool("zoom", { region: [20, 10, 30, 20], raster_id: first.raster_id });
  assert.equal(child.ok, true);
  assert.equal(child.parent_raster_id, first.raster_id);
  assert.equal(calls().filter(c => c.method === "zoom").at(-1).args.source, first.file);
  const click = await tool("left_click", { target: { type: "coordinate", x: 10, y: 8, raster_id: child.raster_id } });
  assert.equal(click.ok, true);
  const target = calls().filter(c => c.method === "left_click").at(-1).args.target;
  assert.deepEqual({ x: target.x, y: target.y }, { x: 135, y: 69 });
  assert.equal((await tool("left_click", { target: { type: "coordinate", x: 30, y: 0, raster_id: child.raster_id } })).error.code, "target_outside_raster");
});

test("binding an app retires earlier captures before another coordinate action", async () => {
  const shot = await tool("screenshot");
  assert.equal((await tool("open_application", { name: "FakeApp", activate: false })).ok, true);
  const before = calls().filter(c => c.method === "left_click").length;
  const stale = await tool("left_click", { target: { type: "coordinate", x: 1, y: 1, raster_id: shot.raster_id } });
  assert.equal(stale.error.code, "no_raster");
  assert.equal(calls().filter(c => c.method === "left_click").length, before);
});

test("a successful launch without resolved app identity still retires capture pins", async () => {
  const shot = await tool("screenshot");
  fs.writeFileSync(callsFile + ".control.json", JSON.stringify({ open_without_resolved: true }));
  try {
    const opened = await tool("open_application", { name: "FakeApp", activate: false });
    assert.equal(opened.ok, true, JSON.stringify(opened));
    assert.equal(opened.launched, true);
    assert.equal(opened.resolved, undefined, "Windows/Linux-style launch receipt has no resolved identity");
    const before = calls().filter(c => c.method === "left_click").length;
    const stale = await tool("left_click", { target: { type: "coordinate", x: 1, y: 1, raster_id: shot.raster_id } });
    assert.equal(stale.error.code, "no_raster");
    assert.equal(calls().filter(c => c.method === "left_click").length, before, "retired pin dispatches no input");
    const fresh = await tool("screenshot");
    const clicked = await tool("left_click", { target: { type: "coordinate", x: 1, y: 1, raster_id: fresh.raster_id } });
    assert.equal(clicked.ok, true, JSON.stringify(clicked));
    assert.equal(clicked.target_raster_id, fresh.raster_id);
    assert.equal(calls().filter(c => c.method === "left_click").length, before + 1);
  } finally {
    fs.unlinkSync(callsFile + ".control.json");
  }
});
