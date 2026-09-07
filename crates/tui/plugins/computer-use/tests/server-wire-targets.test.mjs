// The out-of-process route — the desktop app on macOS, the ssh agent on a
// remote computer — is the DEFAULT route for the local computer whenever the
// app is installed, yet it used to prepare arguments differently from the
// in-process route. These tests drive the real server over stdio with that
// route forced (CODEWHALE_CU_TEST_REMOTE) and assert that a coordinate target
// reaches the backend as screen points, with the raster's refusals intact.
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
    server.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n");
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
      CODEWHALE_CU_TEST_REMOTE: "1",
      CODEWHALE_CU_STATE_DIR: stateDir,
      CODEWHALE_CU_RECORDINGS_DIR: recDir,
      CODEWHALE_CU_TEST_BACKEND: path.join(__dirname, "fixtures", "fake-backend.mjs"),
      FAKE_BACKEND_CALLS: callsFile,
      FAKE_BACKEND_CONTROL: callsFile + ".control.json",
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

test("element targets travel as a center for pointer tools and an AX path for semantic ones", async () => {
  const state = await tool("get_app_state", {});
  assert.equal(state.ok, true);
  const target = { type: "element", state_id: state.state_id, index: 1 };

  const click = await tool("left_click", { target });
  assert.equal(click.ok, true, JSON.stringify(click));
  const clicked = calls().filter((c) => c.method === "left_click").at(-1);
  assert.deepEqual({ x: clicked.args.target.x, y: clicked.args.target.y }, { x: 40, y: 35 }, "element center in points");

  const pressed = await tool("perform_action", { target, action: "AXPress" });
  assert.equal(pressed.ok, true, JSON.stringify(pressed));
  const semantic = calls().filter((c) => c.method === "perform_action").at(-1);
  assert.deepEqual(semantic.args.target.path, [0, 1], "semantic actions address the element, not a point");
  assert.equal(semantic.args.target.windowIndex, 0);
});
