// Target-pipeline tests: real MCP server over stdio with an injected fake
// backend (CODEWHALE_CU_TEST_BACKEND) so raster math and element
// revalidation can be asserted against the exact args the backend receives.
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

const stateDir = fs.mkdtempSync(path.join(os.tmpdir(), "cu-tgt-state-"));
const recDir = fs.mkdtempSync(path.join(os.tmpdir(), "cu-tgt-rec-"));
const callsFile = path.join(fs.mkdtempSync(path.join(os.tmpdir(), "cu-tgt-")), "calls.jsonl");
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
    server.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params: attestParams(method, params) }) + "\n");
  });
}

function rpcId(method, params) {
  const id = nextId++;
  const p = new Promise((resolve) => pending.set(id, resolve));
  server.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params: attestParams(method, params) }) + "\n");
  return { id, p };
}

function notify(method, params) {
  server.stdin.write(JSON.stringify({ jsonrpc: "2.0", method, params }) + "\n");
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
      CODEWHALE_CU_TEST_BACKEND: path.join(__dirname, "fixtures", "fake-backend.mjs"),
      FAKE_BACKEND_CALLS: callsFile,
      FAKE_BACKEND_CONTROL: controlFile,
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
  // The local consent ledger gates every app-targeted call; record the user's
  // decisions for the fixture apps up front, as a real session would.
  for (const app of ["FakeApp", "OCR unavailable", "OtherApp"]) {
    const c = await tool("consent", { action: "allow", app });
    assert.equal(c.ok, true, JSON.stringify(c));
  }
});

after(() => {
  server?.kill("SIGTERM");
  for (const d of [stateDir, recDir, path.dirname(callsFile)]) { try { fs.rmSync(d, { recursive: true, force: true }); } catch {} }
});

test("app-state arguments distinguish an omitted reference from an explicit null", async () => {
  await tool("get_app_state", {});
  assert.equal(Object.hasOwn(calls("get_app_state").at(-1).args, "app_ref"), false);
  await tool("get_app_state", { app_ref: null });
  assert.equal(calls("get_app_state").at(-1).args.app_ref, null);
});

test("summary preserves readable UI and original target indices while full retains tree structure", async () => {
  for (const detail of [undefined, "summary"]) {
    const state = await tool("get_app_state", { detail });
    assert.equal(state.detail, "summary");
    assert.deepEqual(state.elements.map(e => e.index), [0, 1, 2, 3, 6, 7, 8]);
    assert.ok(state.elements.every(e => !("path" in e) && !("windowIndex" in e)));
    const field = state.elements.find(e => e.index === 8);
    assert.deepEqual(field, { index: 8, role: "AXTextField", value: "Fixture text", focused: true, enabled: true, actions: ["AXConfirm"], position: { x: 10, y: 60 }, size: { w: 150, h: 25 } });
    const action = await tool("perform_action", { target: { type: "element", state_id: state.state_id, index: 1 }, action: "AXPress" });
    assert.equal(action.ok, true, JSON.stringify(action));
    assert.deepEqual(calls("perform_action").at(-1).args.target.path, [0, 1]);
    assert.equal(calls("perform_action").at(-1).args.target.windowIndex, 0);
  }
  const full = await tool("get_app_state", { detail: "full" });
  assert.equal(full.elements.length, 9);
  assert.deepEqual(full.elements.find(e => e.label === "Save").path, [0, 0, 0]);
  assert.equal(full.elements.find(e => e.label === "Save").windowIndex, -1);
  const summary = await tool("get_app_state", {});
  setControl({ found: true, element: { role: "AXTextField", position: { x: 10, y: 60 }, size: { w: 150, h: 25 } } });
  try {
    const changed = await tool("set_value", { target: { type: "element", state_id: summary.state_id, index: 8 }, value: "Changed" });
    assert.equal(changed.ok, true, JSON.stringify(changed));
    assert.deepEqual(calls("set_value").at(-1).args.target.path, [0, 2], "sparse public index still addresses the original cached text field");
  } finally { setControl(null); }
  assert.equal((await tool("get_app_state", { detail: "guess" })).error.code, "bad_args");
  assert.equal((await tool("get_app_state", { window_id: -1 })).error.code, "bad_args");
  assert.equal((await tool("get_app_state", { window_id: 0.5 })).error.code, "bad_args");
  assert.equal((await tool("get_app_state", { include_ocr: "yes" })).error.code, "bad_args");
});

test("compact and query return a filterable page instead of the whole tree", async () => {
  const compact = await tool("get_app_state", { detail: "compact" });
  assert.equal(compact.detail, "compact");
  assert.equal(compact.ocr, undefined);
  const field = compact.elements.find(e => e.index === 8);
  assert.equal(field.role, "AXTextField");
  assert.equal(field.value, "Fixture text");
  assert.equal(field.position, undefined);
  const found = await tool("find_elements", { query: "whale", state_id: compact.state_id });
  assert.equal(found.ok, true);
  assert.equal(found.matched, 0);
  const buttons = await tool("find_elements", { role: "AXButton", state_id: compact.state_id });
  assert.equal(buttons.matched, 1);
  assert.equal(buttons.elements[0].label, "OK");
  const paged = await tool("get_app_state", { limit: 2, offset: 0 });
  assert.equal(paged.returned, 2);
  assert.equal(paged.matched, 7);
  assert.equal(paged.truncated, true);
});

test("type treats newlines and press_enter as Return rather than unicode", async () => {
  const typed = await tool("type", { text: "hello\nworld", press_enter: true });
  assert.equal(typed.ok, true, JSON.stringify(typed.error));
  assert.equal(typed.newlines_as_return, true);
  const typeCalls = calls("type");
  const keyCalls = calls("key");
  assert.deepEqual(typeCalls.slice(-2).map(c => c.args.text), ["hello", "world"]);
  assert.ok(keyCalls.filter(c => c.args.text === "return").length >= 2);
});

test("since returns a small diff while preserving fresh targetable full records", async () => {
  const first = await tool("get_app_state", { detail: "compact" });
  const diff = await tool("get_app_state", { detail: "compact", since: first.state_id });
  assert.equal(diff.delta, true, JSON.stringify(diff));
  assert.equal(diff.base_state_id, first.state_id);
  assert.notEqual(diff.state_id, first.state_id);
  assert.equal(diff.returned, 0);
  assert.equal(diff.unchanged, first.returned);
  assert.deepEqual(diff.removed_indices, []);
  assert.ok(JSON.stringify(diff).length < JSON.stringify(first).length);
  assert.equal((await tool("left_click", { target: { type: "element", state_id: diff.state_id, index: 1 } })).ok, true);
  const latest = await tool("get_app_state", { detail: "compact", since: "latest" });
  assert.equal(latest.base_state_id, diff.state_id);
});

test("state diffs report changes/removals and resync on changed scope, expired or incomplete baselines", async () => {
  const field = { index: 0, path: [0], windowIndex: 0, role: "AXTextField", label: "Email", value: "before" };
  try {
    setControl({ observation: { elements: [field, { ...field, index: 1, path: [1], label: "Name" }] } });
    const first = await tool("get_app_state", { detail: "compact" });
    setControl({ observation: { elements: [{ ...field, value: "after" }] } });
    const changed = await tool("get_app_state", { detail: "compact", since: first.state_id });
    assert.equal(changed.delta, true);
    assert.equal(changed.elements[0].value, "after");
    assert.deepEqual(changed.removed_indices, [1]);
    for (const args of [{ since: first.state_id, query: "Email" }, { since: first.state_id, window_id: 1 }, { since: "s-expired" }]) {
      const full = await tool("get_app_state", { detail: "compact", ...args });
      assert.equal(full.delta, false);
      assert.equal(full.resync_required, true);
      assert.equal(full.returned, 1);
    }
    setControl({ observation: { elements: [field], truncated: true } });
    const partial = await tool("get_app_state", { detail: "compact" });
    assert.equal(partial.tree_truncated, true);
    const full = await tool("get_app_state", { detail: "compact", since: partial.state_id });
    assert.equal(full.delta, false, "a partial walk must never imply removals");
    assert.equal((await tool("get_app_state", { since: 3 })).error.code, "bad_args");
  } finally { setControl(null); }
});

test("named batches find fresh elements, preserve semantic revalidation and return a final observation", async () => {
  setControl({ found: true, element: { role: "AXTextField", position: { x: 10, y: 60 }, size: { w: 150, h: 25 } } });
  try {
    const result = await tool("run_actions", { steps: [
      { tool: "set_value", find: { role: "AXTextField", query: "Fixture" }, args: { value: "updated" } },
      { tool: "get_value", find: { role: "AXTextField" } },
    ], observe: { query: "Fixture", detail: "compact" } });
    assert.equal(result.ok, true, JSON.stringify(result));
    assert.equal(result.completed_steps, 2);
    assert.equal(result.action_sent, true);
    assert.equal(result.observation.returned, 1);
    const setter = calls("set_value").at(-1).args;
    assert.deepEqual(setter.target.path, [0, 2]);
    assert.equal(setter.target.role, "AXTextField");
    assert.ok(calls("resolve_element").length > 0);
    assert.equal(result.steps[1].receipt.value, "Fixture text");
  } finally { setControl(null); }
});

test("batches prevalidate later steps and refuse ambiguous, missing and stale finds without input", async () => {
  const before = calls("type").length + calls("left_click").length;
  for (const tail of [
    { tool: "not_a_tool" }, { tool: "type", args: [] }, { tool: "type", args: null },
    { tool: "type", arguments: { text: "a" }, args: { text: "b" } },
    { tool: "type", args: { text: "x", computer: "elsewhere" } },
    { tool: "type", args: { text: "x" }, find: { query: " " } },
    { tool: "type", args: { text: "x", target: { type: "element", index: 1 } }, find: { query: "OK" } },
    { tool: "wait_for", args: {} },
    { tool: "get_app_state", args: { detail: "invalid" } },
    { tool: "left_click", find: { query: "OK", window_id: -1 } },
  ]) {
    const result = await tool("run_actions", { steps: [{ tool: "type", args: { text: "must not send" } }, tail] });
    assert.equal(result.ok, false, JSON.stringify(tail));
  }
  for (const [find, code] of [[{ query: "AX" }, "element_ambiguous"], [{ query: "not-present" }, "element_not_found"]]) {
    const result = await tool("run_actions", { steps: [{ tool: "left_click", find }] });
    assert.equal(result.error.code, code);
    assert.equal(result.completed_steps, 0);
    assert.equal(result.action_sent, false);
  }
  setControl({ found: false, reason: "removed after lookup" });
  try {
    const result = await tool("run_actions", { steps: [{ tool: "left_click", find: { query: "OK", role: "AXButton" } }] });
    assert.equal(result.error.code, "element_stale");
    assert.equal(result.action_sent, false);
  } finally { setControl(null); }
  assert.equal(calls("type").length + calls("left_click").length, before);
});

test("merged computer aliases cannot switch or remove another computer inside a batch", async () => {
  await tool("computer_register", { computer: "batch-pad", transport: "hdc" });
  const before = calls("type").length;
  try {
    for (const action of ["switch", "remove"]) {
      const result = await tool("run_actions", { steps: [
        { tool: "type", args: { text: "must not send" } },
        { tool: "computer", args: { action, id: "batch-pad" } },
      ] });
      assert.equal(result.ok, false, JSON.stringify(result));
      assert.equal(result.error.code, "bad_args");
    }
    assert.equal(calls("type").length, before);
    assert.equal((await tool("computer_switch", { computer: "batch-pad" })).ok, true, "refused remove preserved the computer");
  } finally {
    await tool("computer_switch", { computer: "local" });
    await tool("computer_remove", { computer: "batch-pad" });
  }
});

test("a timed-out batch wait stops later input and retains the already sent step receipt", async () => {
  const before = calls("type").length;
  const result = await tool("run_actions", { steps: [
    { tool: "type", args: { text: "first" } },
    { tool: "wait_for", args: { query: "never-present", timeout: 0.5 } },
    { tool: "type", args: { text: "must not send" } },
  ] });
  assert.equal(result.ok, false);
  assert.equal(result.error.code, "condition_not_met");
  assert.equal(result.stopped_at, 1);
  assert.equal(result.completed_steps, 1);
  assert.equal(result.attempted_steps, 2);
  assert.equal(result.action_sent, true);
  assert.equal(calls("type").length, before + 1);
});

test("named lookup refuses a single match from a truncated native walk before input", async () => {
  const before = calls("set_value").length;
  setControl({ observation: { truncated: true, elements: [
    { index: 0, path: [0], windowIndex: 0, role: "AXTextField", value: "only visited match" },
  ] } });
  try {
    const result = await tool("run_actions", { steps: [
      { tool: "set_value", find: { role: "AXTextField" }, args: { value: "must not send" } },
    ] });
    assert.equal(result.error?.code, "observation_incomplete", JSON.stringify(result));
    assert.equal(result.action_sent, false);
    assert.equal(calls("set_value").length, before);
  } finally { setControl(null); }
});

test("screen-space coordinates skip raster conversion", async () => {
  await tool("screenshot");
  const r = await tool("left_click", { target: { type: "coordinate", x: 400, y: 300, space: "screen" } });
  assert.equal(r.ok, true, JSON.stringify(r.error));
  const last = calls("left_click").at(-1);
  assert.deepEqual({ x: last.args.target.x, y: last.args.target.y }, { x: 400, y: 300 });
  assert.equal(last.args.target.coordinate_space, "screen");
});

test("run_actions sequences click, type and key then stops on failure", async () => {
  const shot = await tool("screenshot");
  assert.equal(shot.ok, true);
  const batch = await tool("run_actions", { steps: [
    { tool: "type", arguments: { text: "hi" } },
    { tool: "key", arguments: { text: "return" } },
    { tool: "wait", arguments: { seconds: 0 } },
  ] });
  assert.equal(batch.ok, true, JSON.stringify(batch.error));
  assert.equal(batch.steps.length, 3);
  const nested = await tool("run_actions", { steps: [{ tool: "run_actions", arguments: { steps: [] } }] });
  assert.equal(nested.ok, false);
});

test("optional OCR binds its exact raster for coordinate actions while preserving AX state", async () => {
  const state = await tool("get_app_state", { include_ocr: true });
  assert.equal(state.ok, true);
  assert.equal(state.ocr.status, "ok");
  assert.ok(state.elements.some(e => e.index === 8 && e.value === "Fixture text"));
  assert.equal(state.ocr.blocks[0].role, undefined, "recognized text is not a fabricated semantic element");
  const clicked = await tool("left_click", { target: state.ocr.blocks[0].target });
  assert.equal(clicked.ok, true);
  assert.deepEqual(calls("left_click").at(-1).args.target, { type: "coordinate", x: 140, y: 70, strategy: "event", coordinate_space: "raster" });
  assert.equal((await tool("left_click", { target: { type: "coordinate", x: 400, y: 0 } })).error.code, "target_outside_raster");
  const unavailable = await tool("get_app_state", { include_ocr: true, app_ref: { name: "OCR unavailable" } });
  assert.equal(unavailable.ok, true);
  assert.equal(unavailable.ocr.status, "unavailable");
  assert.ok(unavailable.elements.length > 0);
  assert.equal((await tool("get_app_state", {})).ocr, undefined, "normal observations do not request OCR");
});

test("coordinate targets map raster pixels through the bound scale", async () => {
  const shot = await tool("screenshot");
  assert.equal(shot.ok, true);
  assert.deepEqual(shot.pixels, { w: 1600, h: 1200 });
  const r = await tool("left_click", { target: { type: "coordinate", x: 400, y: 300 } });
  assert.equal(r.ok, true, JSON.stringify(r.error));
  const last = calls("left_click").at(-1);
  assert.deepEqual({ x: last.args.target.x, y: last.args.target.y }, { x: 200, y: 150 });
});

test("region screenshots bind the region origin for later coordinates", async () => {
  const shot = await tool("screenshot", { region: [50, 40, 400, 200] });
  assert.equal(shot.ok, true);
  const r = await tool("left_click", { target: { type: "coordinate", x: 100, y: 60 } });
  assert.equal(r.ok, true, JSON.stringify(r.error));
  const last = calls("left_click").at(-1);
  // origin (50,40) + pixel (100,60) / scale 2 -> (100, 70)
  assert.deepEqual({ x: last.args.target.x, y: last.args.target.y }, { x: 100, y: 70 });
});

test("zoom binds a child raster that keeps parent scale and shifted origin", async () => {
  await tool("screenshot"); // rebind the full 1600x1200 @ scale 2 raster
  const z = await tool("zoom", { region: [100, 100, 200, 200] });
  assert.equal(z.ok, true, JSON.stringify(z.error));
  const r = await tool("left_click", { target: { type: "coordinate", x: 10, y: 10 } });
  assert.equal(r.ok, true, JSON.stringify(r.error));
  const last = calls("left_click").at(-1);
  // origin 0 + (100 + 10) / 2 = 55
  assert.deepEqual({ x: last.args.target.x, y: last.args.target.y }, { x: 55, y: 55 });
});

test("screenshot and zoom never forward a caller-named source file", async () => {
  const shot = await tool("screenshot", { source: "/etc/hosts" });
  assert.equal(Object.hasOwn(calls("screenshot").at(-1).args, "source"), false);
  const z = await tool("zoom", { region: [0, 0, 10, 10], source: "/etc/hosts" });
  assert.equal(z.ok, true, JSON.stringify(z.error));
  assert.equal(calls("zoom").at(-1).args.source, shot.file, "only the server-bound capture is forwarded");
  assert.notEqual(calls("zoom").at(-1).args.source, "/etc/hosts");
});

test("zoom without a bound raster fails with no_raster", async () => {
  // Fresh computer id has no raster — hdc "pad" registered with no state.
  const reg = await tool("computer_register", { computer: "pad", transport: "hdc" });
  assert.equal(reg.ok, true);
  const z = await tool("zoom", { region: [0, 0, 10, 10], computer: "pad" });
  assert.equal(z.ok, false);
  assert.equal(z.error.code, "no_raster");
  await tool("computer_remove", { computer: "pad" });
});

test("coordinate outside the bound raster fails with target_outside_raster", async () => {
  await tool("screenshot"); // 1600x1200 bound
  const r = await tool("left_click", { target: { type: "coordinate", x: 2000, y: 10 } });
  assert.equal(r.ok, false);
  assert.equal(r.error.code, "target_outside_raster");
  assert.match(r.error.message, /1600x1200/);
  assert.equal(calls("left_click").filter((c) => c.args.target.x === 2000).length, 0, "backend must not be called");
});

async function freshState() {
  const st = await tool("get_app_state", { app_ref: { name: "FakeApp" } });
  assert.equal(st.ok, true, JSON.stringify(st.error));
  assert.ok(st.state_id);
  return st;
}

test("element targets are revalidated; moved geometry re-aims and marks the receipt", async () => {
  const st = await freshState();
  setControl({ found: true, element: { role: "AXButton", label: "OK", position: { x: 100, y: 200 }, size: { w: 60, h: 30 } }, reason: null });
  try {
    const r = await tool("left_click", { target: { type: "element", state_id: st.state_id, index: 1 } });
    assert.equal(r.ok, true, JSON.stringify(r.error));
    assert.equal(r.target_reacquired, true);
    const last = calls("left_click").at(-1);
    assert.deepEqual({ x: last.args.target.x, y: last.args.target.y }, { x: 130, y: 215 }); // fresh center
    assert.deepEqual(last.args.target.path, [0, 1], "pointer dispatch retains the original element path");
    assert.equal(last.args.target.windowIndex, 0);
    assert.equal(last.args.target.label, "OK");
  } finally {
    setControl(null);
  }
});

test("unmoved element geometry does not mark the receipt reacquired", async () => {
  const st = await freshState();
  setControl(null); // fake returns the cached geometry for element 1
  const r = await tool("left_click", { target: { type: "element", state_id: st.state_id, index: 1 } });
  assert.equal(r.ok, true, JSON.stringify(r.error));
  assert.notEqual(r.target_reacquired, true);
  const last = calls("left_click").at(-1);
  assert.deepEqual({ x: last.args.target.x, y: last.args.target.y }, { x: 40, y: 35 });
});

test("an element target without state_id binds the computer's latest observation", async () => {
  const st = await freshState();
  setControl(null);
  const r = await tool("left_click", { target: { type: "element", index: 1 } });
  assert.equal(r.ok, true, JSON.stringify(r.error));
  const last = calls("left_click").at(-1);
  assert.deepEqual({ x: last.args.target.x, y: last.args.target.y }, { x: 40, y: 35 });
  assert.notEqual(st.state_id, undefined);
});

test("a bare element index follows the newest observation; an explicit state_id pins the older one", async () => {
  const first = await freshState();
  const second = await freshState();
  assert.notEqual(first.state_id, second.state_id);
  setControl(null);
  // index 1 in the fresh state is the same fixture button in both states.
  const latest = await tool("left_click", { target: { type: "element", index: 1 } });
  assert.equal(latest.ok, true, JSON.stringify(latest.error));
  const pinned = await tool("left_click", { target: { type: "element", state_id: first.state_id, index: 1 } });
  assert.equal(pinned.ok, true, JSON.stringify(pinned.error));
});

test("stale element fails element_stale without touching the pointer", async () => {
  const st = await freshState();
  setControl({ found: false, element: null, reason: "element_gone" });
  const before = calls("left_click").length;
  try {
    const r = await tool("left_click", { target: { type: "element", state_id: st.state_id, index: 1 } });
    assert.equal(r.ok, false);
    assert.equal(r.error.code, "element_stale");
    assert.match(r.error.message, /element 1/);
    assert.equal(calls("left_click").length, before, "backend pointer must not be called");
  } finally {
    setControl(null);
  }
});

test("in-place replacement (same geometry, different label) fails element_stale", async () => {
  const st = await freshState();
  setControl({ found: true, element: { role: "AXButton", label: "Confirm", position: { x: 10, y: 20 }, size: { w: 60, h: 30 } }, reason: null });
  const before = calls("left_click").length;
  try {
    const r = await tool("left_click", { target: { type: "element", state_id: st.state_id, index: 1 } });
    assert.equal(r.ok, false);
    assert.equal(r.error.code, "element_stale");
    assert.match(r.error.message, /changed label \(OK → Confirm\)/);
    assert.equal(calls("left_click").length, before, "backend pointer must not be called");
  } finally {
    setControl(null);
  }
});

test("an element losing its label or role is stale even if the geometry matches", async () => {
  const st = await freshState();
  const before = calls("left_click").length;
  try {
    for (const identity of [{ role: "AXButton", label: "" }, { role: "AXButton" }, { label: "OK" }]) {
      setControl({ found: true, element: { ...identity, position: { x: 10, y: 20 }, size: { w: 60, h: 30 } } });
      const r = await tool("left_click", { target: { type: "element", state_id: st.state_id, index: 1 } });
      assert.equal(r.ok, false);
      assert.equal(r.error.code, "element_stale");
    }
    assert.equal(calls("left_click").length, before);
  } finally { setControl(null); }
});

test("a state_id issued on another computer fails state_wrong_computer", async () => {
  const st = await freshState(); // bound to "local"
  const reg = await tool("computer_register", { computer: "other-pad", transport: "hdc" });
  assert.equal(reg.ok, true);
  try {
    const r = await tool("left_click", { computer: "other-pad", target: { type: "element", state_id: st.state_id, index: 1 } });
    assert.equal(r.ok, false);
    assert.equal(r.error.code, "state_wrong_computer");
    const diff = await tool("get_app_state", { computer: "other-pad", since: st.state_id });
    assert.equal(diff.error.code, "state_wrong_computer");
  } finally {
    await tool("computer_remove", { computer: "other-pad" });
    await tool("computer_switch", { computer: "local" });
  }
});

test("missing required arguments fail bad_args before any backend call", async () => {
  const counted = ["left_mouse_down", "select_text", "key", "set_value"];
  const before = counted.reduce((n, m) => n + calls(m).length, 0);
  for (const [name, args, field] of [
    ["left_mouse_down", {}, "target"],
    ["left_click", {}, "target"],
    ["select_text", {}, "target"],
    ["key", {}, "text"],
    ["set_value", { value: "x" }, "target"],
    ["hold_key", { text: "a" }, "duration"],
  ]) {
    const r = await tool(name, args);
    assert.equal(r.ok, false, `${name} must refuse missing args`);
    assert.equal(r.error.code, "bad_args", `${name}: ${JSON.stringify(r.error)}`);
    assert.match(r.error.message, new RegExp(field));
  }
  const after = counted.reduce((n, m) => n + calls(m).length, 0);
  assert.equal(after, before, "no request may reach the backend");
});

test("element-only tools refuse coordinate or malformed targets with bad_target", async () => {
  for (const [name, args] of [
    ["set_value", { target: { x: 10, y: 10 }, value: "x" }],
    ["set_value", { target: { type: "coordinate", space: "screen", x: 1, y: 2 }, value: "x" }],
    ["select_text", { target: { type: "coordinate", space: "screen", x: 1, y: 2 } }],
    ["perform_action", { target: { type: "coordinate", space: "screen", x: 1, y: 2 }, action: "AXPress" }],
    ["left_click", { target: "5,5" }],
    ["left_click", { target: { type: "nonsense" } }],
    ["left_click", { target: 42 }],
  ]) {
    const r = await tool(name, args);
    assert.equal(r.ok, false, `${name}: ${JSON.stringify(r)}`);
    assert.equal(r.error.code, "bad_target", `${name}: ${JSON.stringify(r.error)}`);
  }
});

test("a stale element error names the resolved state and app, not 'undefined'", async () => {
  const st = await freshState();
  setControl({ found: false, element: null, reason: "window_not_found" });
  try {
    // Bare index binds the latest observation — the message must say which.
    const r = await tool("left_click", { target: { type: "element", index: 1 } });
    assert.equal(r.ok, false);
    assert.equal(r.error.code, "element_stale");
    assert.match(r.error.message, new RegExp(`state ${st.state_id}`));
    assert.match(r.error.message, /FakeApp/);
    assert.doesNotMatch(r.error.message, /undefined/);
  } finally {
    setControl(null);
  }
});

test("binding a different app retires bare element indices but keeps pinned states", async () => {
  const st = await freshState();
  const opened = await tool("open_application", { name: "OtherApp" });
  assert.equal(opened.ok, true, JSON.stringify(opened.error));
  assert.match(opened.note ?? "", /different app/);
  const bare = await tool("left_click", { target: { type: "element", index: 1 } });
  assert.equal(bare.ok, false);
  assert.equal(bare.error.code, "unknown_state");
  // An explicit state_id still resolves through its own pinned observation.
  const pinned = await tool("left_click", { target: { type: "element", state_id: st.state_id, index: 1 } });
  assert.equal(pinned.ok, true, JSON.stringify(pinned.error));
});

test("notifications/cancelled drops the in-flight response but not the server", async () => {
  const { id, p } = rpcId("tools/call", { name: "wait", arguments: { seconds: 3 } });
  await new Promise((r) => setTimeout(r, 200));
  notify("notifications/cancelled", { requestId: id });
  const winner = await Promise.race([
    p.then((m) => ({ got: true, m })),
    new Promise((r) => setTimeout(() => r({ got: false }), 4_000)),
  ]);
  assert.equal(winner.got, false, "cancelled request must not produce a response");
  const ping = await rpc("ping", {});
  assert.deepEqual(ping.result, {});
});
