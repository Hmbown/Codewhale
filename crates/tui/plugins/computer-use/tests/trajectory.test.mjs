// Trajectory recording and replay over the real server: files land in an
// isolated recordings dir; replay re-enters the normal tool pipeline and the
// recorder never records itself.
import { test, before, after } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import url from "node:url";
import { spawn } from "node:child_process";

const ROOT = path.resolve(path.dirname(url.fileURLToPath(import.meta.url)), "..");
const stateDir = fs.mkdtempSync(path.join(os.tmpdir(), "cu-traj-state-"));
const recDir = fs.mkdtempSync(path.join(os.tmpdir(), "cu-traj-rec-"));
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
    env: { ...process.env, CODEWHALE_CU_STATE_DIR: stateDir, CODEWHALE_CU_RECORDINGS_DIR: recDir, CODEWHALE_CU_APP: "off" },
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
after(() => { try { server.stdin.end(); } catch {} server?.kill("SIGTERM"); fs.rmSync(stateDir, { recursive: true, force: true }); fs.rmSync(recDir, { recursive: true, force: true }); });

test("record → status → stop writes a local JSONL with turns and refusals", async () => {
  const started = await tool("trajectory", { action: "start" });
  assert.equal(started.ok, true);
  assert.equal(started.recording, true);
  assert.ok(fs.existsSync(started.file));
  assert.equal((await tool("trajectory", { action: "status" })).recording, true);
  await tool("wait", { seconds: 0.05 });
  await tool("computer", { action: "list" });
  const refused = await tool("click", {});
  assert.equal(refused.error?.code, "bad_args");
  const stopped = await tool("trajectory", { action: "stop" });
  assert.equal(stopped.recording, false);
  assert.equal(stopped.turns, 3, `turns=${stopped.turns}`);
  const lines = fs.readFileSync(stopped.file, "utf8").trim().split("\n").map(JSON.parse);
  assert.equal(lines[0].type, "start");
  assert.equal(lines.at(-1).type, "stop");
  const calls = lines.filter((l) => l.type === "call");
  assert.deepEqual(calls.map((c) => c.tool), ["wait", "computer", "click"]);
  assert.equal(calls[2].ok, false, "refusals are part of the record");
  assert.equal(calls[2].code, "bad_args");
});

test("replay dry_run lists the plan without executing", async () => {
  const started = await tool("trajectory", { action: "start" });
  assert.equal(started.recording, true);
  await tool("wait", { seconds: 0.01 });
  const stopped = await tool("trajectory", { action: "stop" });
  const dry = await tool("trajectory", { action: "replay", id: path.basename(stopped.file), dry_run: true });
  assert.equal(dry.dry_run, true);
  assert.equal(dry.replayed, 0);
  assert.deepEqual(dry.plan, ["wait"]);
});

test("replay re-enters the pipeline, stops at the first refusal, and never records itself", async () => {
  const started = await tool("trajectory", { action: "start" });
  assert.equal(started.recording, true);
  await tool("wait", { seconds: 0.01 });
  await tool("click", {});
  await tool("wait", { seconds: 0.01 });
  const stopped = await tool("trajectory", { action: "stop" });
  const replay = await tool("trajectory", { action: "replay", id: path.basename(stopped.file) });
  assert.equal(replay.ok, true);
  assert.equal(replay.turns_in_file, 3);
  assert.equal(replay.replayed, 2, "stops at the refusal instead of continuing");
  assert.deepEqual(replay.results.map((r) => r.tool), ["wait", "click"]);
  assert.equal(replay.results[1].ok, false);
  const calls = fs.readFileSync(stopped.file, "utf8").trim().split("\n").map(JSON.parse).filter((l) => l.type === "call");
  assert.equal(calls.length, 3, "replayed calls are not re-recorded");
  assert.equal((await tool("trajectory", { action: "status" })).recording, false);
});

test("E4: entered text is redacted, the file is 0600 in a 0700 dir, and redacted steps never replay", async () => {
  const started = await tool("trajectory", { action: "start" });
  assert.equal(started.recording, true);
  // set_value without a target refuses before any backend is touched, but the
  // attempt — with its secret — is still part of the record.
  const refused = await tool("set_value", { value: "hunter2-secret" });
  assert.equal(refused.error?.code, "bad_args");
  await tool("wait", { seconds: 0.01 });
  const stopped = await tool("trajectory", { action: "stop" });
  const text = fs.readFileSync(stopped.file, "utf8");
  assert.ok(!text.includes("hunter2"), "the secret never reaches disk");
  const call = text.trim().split("\n").map(JSON.parse).find((l) => l.tool === "set_value");
  assert.equal(call.args.value, "[redacted]");
  assert.equal(call.redacted, true);
  assert.equal(call.replayable, false);
  if (process.platform !== "win32") {
    assert.equal(fs.statSync(stopped.file).mode & 0o777, 0o600);
    assert.equal(fs.statSync(path.dirname(stopped.file)).mode & 0o777, 0o700);
  }
  const dry = await tool("trajectory", { action: "replay", id: path.basename(stopped.file), dry_run: true });
  assert.deepEqual(dry.not_replayable, [0]);
  const replay = await tool("trajectory", { action: "replay", id: path.basename(stopped.file) });
  assert.equal(replay.replayed, 1);
  assert.deepEqual(replay.results, [{ tool: "set_value", ok: false, code: "not_replayable" }]);
});

test("replay refuses escaping ids; the kill switch gates replay but not status", async () => {
  const bad = await tool("trajectory", { action: "replay", id: "../escape.jsonl" });
  assert.equal(bad.error?.code, "bad_args");
  const missing = await tool("trajectory", { action: "replay", id: "traj-nope.jsonl" });
  assert.equal(missing.error?.code, "trajectory_not_found");
  await tool("stop_computer_control", { reason: "trajectory test" });
  const afterStop = await tool("trajectory", { action: "replay", dry_run: true });
  assert.equal(afterStop.error?.code, "control_stopped", "replay is an action, not a read");
  const status = await tool("trajectory", { action: "status" });
  assert.equal(status.ok, true, "status stays readable after the stop, like other read-only probes");
});
