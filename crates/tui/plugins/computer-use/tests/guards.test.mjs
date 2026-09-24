// Guards that stand between the model and the user's machine, over the real
// MCP server with the fake backend (nothing reaches osascript or the desktop):
//   - app_script policy: shell escapes refused, named apps go through the
//     consent ledger (System Events and its processes included);
//   - irreversible-action confirmation: pay/buy/order/send/transfer/delete
//     controls need a per-call user confirmation that no app grant covers.
import { test, before, after } from "node:test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import url from "node:url";
import { checkAppScript, appScriptMode } from "../src/app-script-policy.mjs";
import { helperStaleness, newerVersion } from "../src/app-socket.mjs";

const __dirname = path.dirname(url.fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, "..");
const stateDir = fs.mkdtempSync(path.join(os.tmpdir(), "cu-guard-state-"));
const recDir = fs.mkdtempSync(path.join(os.tmpdir(), "cu-guard-rec-"));
const work = fs.mkdtempSync(path.join(os.tmpdir(), "cu-guard-"));
const callsFile = path.join(work, "calls.jsonl");
const controlFile = path.join(work, "control.json");

const EXTRA = [
  { index: 9, path: [0, 3], windowIndex: 0, role: "AXButton", label: "Place order", position: { x: 200, y: 20 }, size: { w: 80, h: 30 } },
  { index: 10, path: [0, 4], windowIndex: 0, role: "AXButton", label: "Delete", position: { x: 300, y: 20 }, size: { w: 60, h: 30 } },
  { index: 11, path: [0, 5], windowIndex: 0, role: "AXTextField", label: "Send to", position: { x: 10, y: 100 }, size: { w: 150, h: 25 } },
];

let server;
let buf = "";
const pending = new Map();
let nextId = 1;

function rpc(method, params) {
  const id = nextId++;
  return new Promise((resolve, reject) => {
    const t = setTimeout(() => { pending.delete(id); reject(new Error(`timeout: ${method}`)); }, 30_000);
    pending.set(id, (msg) => { clearTimeout(t); resolve(msg); });
    server.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n");
  });
}
async function tool(name, args = {}) {
  const res = await rpc("tools/call", { name, arguments: args });
  assert.ok(res.result, `${name}: protocol error ${JSON.stringify(res.error ?? {})}`);
  return JSON.parse(res.result.content[0].text);
}
const calls = (method) => fs.existsSync(callsFile)
  ? fs.readFileSync(callsFile, "utf8").split("\n").filter(Boolean).map((l) => JSON.parse(l)).filter((c) => c.method === method)
  : [];
const answerResolve = (element) => fs.writeFileSync(controlFile, JSON.stringify({ found: true, element }));

before(async () => {
  server = spawn(process.execPath, [path.join(ROOT, "mcp", "server.mjs")], {
    env: {
      ...process.env,
      CODEWHALE_CU_APP: "off",
      CODEWHALE_CU_APP_SCRIPT: "",
      CODEWHALE_CU_STATE_DIR: stateDir,
      CODEWHALE_CU_RECORDINGS_DIR: recDir,
      CODEWHALE_CU_TEST_BACKEND: path.join(__dirname, "fixtures", "fake-backend.mjs"),
      FAKE_BACKEND_CALLS: callsFile,
      FAKE_BACKEND_CONTROL: controlFile,
      FAKE_BACKEND_EXTRA_ELEMENTS: JSON.stringify(EXTRA),
    },
    stdio: ["pipe", "pipe", "pipe"],
  });
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
  await rpc("initialize", { protocolVersion: "2025-06-18" });
  assert.equal((await tool("consent", { action: "allow", app: "FakeApp" })).ok, true);
});

after(() => {
  try { server.stdin.end(); } catch {}
  server?.kill("SIGTERM");
  for (const d of [stateDir, recDir, work]) fs.rmSync(d, { recursive: true, force: true });
});

// ---- app_script policy (unit) ----

test("app_script policy refuses shell escapes in AppleScript and JXA", () => {
  for (const [script, language] of [
    ['do shell script "id"', "applescript"],
    ['do shell ¬\n script "id"', "applescript"],
    ['tell application "Terminal" to do script "id"', "applescript"],
    ['«event sysoexec» "id"', "applescript"],
    ['use framework "Foundation"\ncurrent application\'s NSTask\'s new()', "applescript"],
    ['run script "do shell" & " script \\"id\\""', "applescript"],
    ['tell application "System Events" to keystroke "id"', "applescript"],
    ['var a = Application.currentApplication(); a.includeStandardAdditions = true; a.doShellScript("id")', "javascript"],
    ['var a = Application.currentApplication(); a["do" + "ShellScript"]("id")', "javascript"],
    ['var k = "doShell" + "Script"; var o = {[k]: 1}', "javascript"],
    ['ObjC.import("Foundation"); $.NSTask.alloc.init', "javascript"],
    ['[].constructor.constructor("return 1")()', "javascript"],
    ['Reflect.get(Application.currentApplication(), "x")', "javascript"],
    ['Application("iTerm2").createWindowWithDefaultProfile()', "javascript"],
  ]) {
    const r = checkAppScript(script, language);
    assert.ok(r.refused, `must refuse: ${script}`);
  }
});

test("app_script policy names every target app and refuses targets it cannot read", () => {
  assert.deepEqual(checkAppScript('return "whole computer"').targets, []);
  assert.deepEqual(checkAppScript('tell application "Finder" to get name of every window').targets, [{ name: "Finder" }]);
  assert.deepEqual(checkAppScript('tell application id "com.apple.Safari" to get URL of front document').targets, [{ bundle_id: "com.apple.Safari" }]);
  const se = checkAppScript('tell application "System Events" to tell process "Safari" to click button 1 of window 1');
  assert.equal(se.refused, null);
  assert.deepEqual(se.targets, [{ name: "System Events" }, { name: "Safari" }]);
  const jxa = checkAppScript('Application("System Events").processes.byName("Safari").windows[0].name()', "javascript");
  assert.equal(jxa.refused, null);
  assert.deepEqual(jxa.targets, [{ name: "System Events" }, { name: "Safari" }]);
  assert.equal(checkAppScript("set p to path to application support folder from user domain").refused, null);
  assert.equal(checkAppScript('Application("Finder").windows.at(0).name()', "javascript").refused, null);
  for (const [script, language] of [
    ['tell application ("Term" & "inal") to activate', "applescript"],
    ['tell application "System Events" to tell (first process whose frontmost is true) to click button 1', "applescript"],
    ['tell application "System Events" to click button 1 of window 1 of process 1', "applescript"],
    ['var n = "Fin" + "der"; Application(n).activate()', "javascript"],
    ['Application("System Events").processes.whose({frontmost: true})[0].name()', "javascript"],
  ]) assert.ok(checkAppScript(script, language).refused, `must refuse: ${script}`);
});

test("app_script policy modes: off refuses everything, unknown fails closed, unrestricted keeps targets", () => {
  assert.equal(appScriptMode({}), "apps");
  assert.equal(appScriptMode({ CODEWHALE_CU_APP_SCRIPT: "nonsense" }), "off");
  assert.ok(checkAppScript("return 1", "applescript", { CODEWHALE_CU_APP_SCRIPT: "off" }).refused);
  const open = checkAppScript('tell application "Mail" to do shell script "id"', "applescript", { CODEWHALE_CU_APP_SCRIPT: "unrestricted" });
  assert.equal(open.refused, null);
  assert.deepEqual(open.targets, [{ name: "Mail" }]);
});

// ---- app_script policy (server) ----

test("E7: shell escapes are refused by the server before any dispatch", async () => {
  const before = calls("app_script").length;
  for (const [script, language] of [['do shell script "id"', undefined], ['Application.currentApplication().doShellScript("id")', "javascript"]]) {
    const r = await tool("app_script", { script, ...(language ? { language } : {}) });
    assert.equal(r.ok, false);
    assert.equal(r.error.code, "script_refused", JSON.stringify(r));
  }
  assert.equal(calls("app_script").length, before, "nothing reached the backend");
});

test("E6: an app reached through System Events goes through the ledger, and a denied one is refused", async () => {
  const script = 'tell application "System Events" to tell process "Vault" to get name of window 1';
  const first = await tool("app_script", { script });
  assert.equal(first.error?.code, "consent_required", JSON.stringify(first));
  assert.match(first.error.message, /System Events/);
  assert.equal((await tool("consent", { action: "allow", app: "System Events" })).ok, true);
  assert.equal((await tool("consent", { action: "deny", app: "Vault" })).ok, true);
  const denied = await tool("app_script", { script });
  assert.equal(denied.error?.code, "app_denied", JSON.stringify(denied));
  assert.equal(calls("app_script").length, 0, "no refused script was dispatched");
  const ok = await tool("app_script", { script: 'tell application "System Events" to get name of every process' });
  assert.equal(ok.ok, true, JSON.stringify(ok));
  assert.equal(calls("app_script").length, 1);
});

// ---- irreversible-action confirmation ----

test("E3: a Place order click needs a per-call confirmation that no app grant covers", async () => {
  const state = await tool("get_app_state", {});
  const target = { type: "element", state_id: state.state_id, index: 9 };
  answerResolve({ role: "AXButton", label: "Place order", position: { x: 200, y: 20 }, size: { w: 80, h: 30 } });
  const clicksBefore = calls("left_click").length;
  const refused = await tool("click", { target });
  assert.equal(refused.error?.code, "confirmation_required", JSON.stringify(refused));
  assert.equal(refused.confirm.label, "Place order");
  assert.match(refused.confirm.token, /^confirm-[0-9a-f]+$/);
  assert.equal(calls("left_click").length, clicksBefore, "the refused click was never dispatched");
  // Repeating without confirmation hands back the same pending token.
  assert.equal((await tool("click", { target })).confirm.token, refused.confirm.token);
  // An app-level allow is not a confirmation.
  assert.equal((await tool("consent", { action: "allow", app: "FakeApp" })).ok, true);
  assert.equal((await tool("click", { target })).error?.code, "confirmation_required");
  assert.equal((await tool("consent", { action: "allow", confirm: "confirm-000" })).error?.code, "confirmation_unknown");
  const confirmed = await tool("consent", { action: "allow", confirm: refused.confirm.token });
  assert.equal(confirmed.ok, true, JSON.stringify(confirmed));
  assert.equal(confirmed.confirmed.label, "Place order");
  // A different call is not admitted by that confirmation.
  const other = await tool("click", { target, clicks: 2 });
  assert.equal(other.error?.code, "confirmation_required");
  const ok = await tool("click", { target });
  assert.equal(ok.ok, true, JSON.stringify(ok));
  assert.equal(calls("left_click").length, clicksBefore + 1);
  // Single use: the identical call asks again.
  assert.equal((await tool("click", { target })).error?.code, "confirmation_required");
  assert.equal((await tool("consent", { action: "allow", confirm: refused.confirm.token })).error?.code, "confirmation_unknown");
});

test("E5: delete through perform_action, a coordinate click or a menu is gated; a Send-to text field is not", async () => {
  const state = await tool("get_app_state", {});
  answerResolve({ role: "AXButton", label: "Delete", position: { x: 300, y: 20 }, size: { w: 60, h: 30 } });
  const pressed = await tool("perform_action", { target: { type: "element", state_id: state.state_id, index: 10 }, action: "AXPress" });
  assert.equal(pressed.error?.code, "confirmation_required", JSON.stringify(pressed));
  const keyed = await tool("key", { text: "space", target: { type: "element", state_id: state.state_id, index: 10 } });
  assert.equal(keyed.error?.code, "confirmation_required", JSON.stringify(keyed));
  const coord = await tool("click", { target: { type: "coordinate", space: "screen", x: 320, y: 30 } });
  assert.equal(coord.error?.code, "confirmation_required", JSON.stringify(coord));
  assert.equal(coord.confirm.label, "Delete");
  await tool("open_application", { name: "FakeApp" });
  const menu = await tool("invoke_menu", { path: ["Edit", "Delete"] });
  assert.equal(menu.error?.code, "confirmation_required", JSON.stringify(menu));
  const save = await tool("invoke_menu", { path: ["File", "Save"] });
  assert.notEqual(save.error?.code, "confirmation_required");
  answerResolve({ role: "AXTextField", label: "Send to", position: { x: 10, y: 100 }, size: { w: 150, h: 25 } });
  const field = await tool("click", { target: { type: "element", state_id: state.state_id, index: 11 } });
  assert.notEqual(field.error?.code, "confirmation_required", JSON.stringify(field));
  fs.rmSync(controlFile, { force: true });
});

// ---- helper staleness (K6) ----

test("a consent decision is never a run_actions step or a replayed trajectory step", async () => {
  // Batched: refused before any step runs, so the grant is not recorded.
  const batched = await tool("run_actions", { steps: [
    { tool: "consent", arguments: { action: "allow", app: "BatchedApp" } },
    { tool: "screenshot", arguments: {} },
  ] });
  assert.equal(batched.error?.code, "bad_args", JSON.stringify(batched));
  assert.match(batched.error.message, /consent decisions cannot be a run_actions step/);
  const status = await tool("consent", { action: "status" });
  assert.ok(!JSON.stringify(status).includes("BatchedApp"), "the batched allow was not recorded");
  // Replayed: a recorded allow/revoke stops the replay instead of re-deciding.
  await tool("trajectory", { action: "start" });
  assert.equal((await tool("consent", { action: "allow", app: "ReplayApp" })).ok, true);
  assert.equal((await tool("consent", { action: "revoke", app: "ReplayApp" })).ok, true);
  const stopped = await tool("trajectory", { action: "stop" });
  const dry = await tool("trajectory", { action: "replay", id: path.basename(stopped.file), dry_run: true });
  assert.deepEqual(dry.not_replayable, [0, 1], JSON.stringify(dry));
  const replay = await tool("trajectory", { action: "replay", id: path.basename(stopped.file) });
  assert.equal(replay.results[0].code, "not_replayable", JSON.stringify(replay));
  assert.ok(!JSON.stringify(await tool("consent", { action: "status" })).includes("ReplayApp"), "the replay did not re-grant ReplayApp");
});

test("D2: a helper newer than the bundled plugin is not stale; an older one is", () => {
  assert.equal(newerVersion("0.11.3", "0.11.2"), true);
  assert.equal(newerVersion("0.11.10", "0.11.9"), true);
  assert.equal(helperStaleness("0.11.3", "0.11.2").stale, false, "notarized 0.11.3 beside the 0.11.2 built-in");
  assert.equal(helperStaleness("0.11.2", "0.11.2").stale, false);
  const old = helperStaleness("0.11.2", "0.11.3");
  assert.equal(old.stale, true);
  assert.match(old.note, /restart/);
  assert.equal(helperStaleness("garbage", "0.11.3").stale, false, "an unreadable version is not reported as stale");
});
