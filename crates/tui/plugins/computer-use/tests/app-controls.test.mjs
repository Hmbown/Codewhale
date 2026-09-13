import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import net from "node:net";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import { setTimeout as delay } from "node:timers/promises";

const root = fileURLToPath(new URL("../", import.meta.url));
test("human Pause cancels queued input; Stop invalidates owners; MCP cannot resume", async t => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "cu-control-"));
  const log = path.join(dir, "calls.jsonl");
  const endpoint = process.platform === "win32"
    ? `\\\\.\\pipe\\cu-control-${process.pid}-${path.basename(dir)}`
    : path.join(dir, "app.sock");
  const daemon = spawn(process.execPath, [path.join(root, "app/daemon.mjs")], { env: { ...process.env,
    CODEWHALE_CU_STATE_DIR: dir, CODEWHALE_CU_APP_SOCKET: endpoint, CODEWHALE_CU_APP_WARM: "off",
    CODEWHALE_CU_TEST_BACKEND: path.join(root, "tests/fixtures/session-backend.mjs"), CU_SESSION_CALLS: log, CODEWHALE_CU_CONTROL_FD: "3" }, stdio: ["ignore", "ignore", "pipe", "pipe"] });
  let errors = ""; daemon.stderr.on("data", chunk => { errors += chunk; });
  const sockets = [];
  t.after(async () => { sockets.forEach(socket => socket.destroy()); if(daemon.exitCode === null && daemon.signalCode === null) { daemon.kill("SIGTERM"); await new Promise(resolve => daemon.once("exit", resolve)); } fs.rmSync(dir, { recursive: true, force: true }); });
  async function until(predicate) { for(let i=0;i<250;i++) { if(predicate()) return; await delay(20); } throw new Error(`Timed out: ${errors}`); }
  // Named pipes on Windows have no filesystem entry; both transports publish
  // the same run receipt only after the listener is ready.
  await until(() => fs.existsSync(path.join(dir, "app-run.json")));
  function request(payload, keepOpen = false) {
    const socket = net.connect(endpoint); sockets.push(socket);
    return new Promise((resolve, reject) => {
      socket.on("error", reject); socket.on("connect", () => socket.write(JSON.stringify(payload)+"\n"));
      let buffer=""; socket.on("data", chunk => { buffer += chunk; if(!buffer.includes("\n")) return; if(!keepOpen) socket.destroy(); resolve(JSON.parse(buffer.split("\n")[0])); });
    });
  }
  let seq=0, buffer=""; const pending=new Map();
  daemon.stdio[3].on("data", chunk => { buffer+=chunk; let n; while((n=buffer.indexOf("\n"))>=0) { const state=JSON.parse(buffer.slice(0,n)); buffer=buffer.slice(n+1); pending.get(state.id)?.(state); pending.delete(state.id); } });
  function control(command) { const id=++seq; return new Promise(resolve => { pending.set(id,resolve); daemon.stdio[3].write(JSON.stringify({id,command})+"\n"); }); }
  const sessionId="human-controls";
  const { leaseToken }=await request({tool:"open_session",sessionId},true);
  const call=(tool,args={})=>request({tool,args,sessionId,leaseToken});
  assert.equal((await call("get_app_state",{app_ref:{name:"Practice"}})).ok,true);
  const held=call("hold_key",{text:"must be cancelled"});
  await until(()=>fs.existsSync(log)&&fs.readFileSync(log,"utf8").includes("child_started"));
  const queued=call("type",{text:"must not replay"});
  const paused=await control("pause");
  assert.equal(paused.mode,"paused"); assert.equal(paused.cleanupPending,false);
  assert.equal((await held).ok,false); assert.equal((await queued).ok,false);
  assert.equal((await call("type",{text:"while paused"})).error.code,"control_paused");
  for(const tool of ["resume","set_control_mode","control","updates"]) assert.equal((await call(tool)).error.code,"tool_not_allowed");
  assert.equal((await control("resume")).mode,"ready");
  assert.equal((await call("type",{text:"allowed again"})).ok,true);
  assert.equal((await control("stop")).mode,"stopped");
  await control("resume");
  assert.equal((await call("type",{text:"old stopped owner"})).error.code,"control_stopped");
  const records=fs.readFileSync(log,"utf8").trim().split("\n").map(JSON.parse);
  assert.ok(records.some(record=>record.method==="child_released"));
  assert.deepEqual(records.filter(record=>record.method==="type").map(record=>record.text),["allowed again"]);
  // Losing the only human-control owner stops a new session too.
  daemon.stdio[3].destroy(); await delay(100);
  const fresh=await request({tool:"open_session",sessionId:"fresh"},true);
  assert.equal((await request({tool:"get_app_state",sessionId:"fresh",leaseToken:fresh.leaseToken})).error.code,"control_stopped");
});
