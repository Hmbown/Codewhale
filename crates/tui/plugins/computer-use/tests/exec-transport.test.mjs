// exec + transport safety tests.
import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import net from "node:net";
import { run, runOk, runInputLease, ExecError, have, trim, withSignal } from "../src/exec.mjs";
import { safeRemotePath, b64, localExec, hdcExec, executorFor } from "../src/transport.mjs";

test("macOS refuses a helper that lacks the background-control contract before any input", {skip:process.platform!=="darwin"}, async t => {
  const dir=fs.mkdtempSync(path.join(os.tmpdir(),"cu-old-helper-"));
  const savedSocket=process.env.CODEWHALE_CU_APP_SOCKET, savedApp=process.env.CODEWHALE_CU_APP;
  process.env.CODEWHALE_CU_APP_SOCKET=path.join(dir,"app.sock");
  delete process.env.CODEWHALE_CU_APP;
  const requests=[];
  const server=net.createServer(socket=>socket.once("data",data=>{
    requests.push(JSON.parse(data));
    socket.end(JSON.stringify({ok:true,app:{id:"old-fixture",sessionProtocol:2}})+"\n");
  }));
  t.after(async()=>{
    await new Promise(resolve=>server.close(resolve));
    if(savedSocket===undefined) delete process.env.CODEWHALE_CU_APP_SOCKET; else process.env.CODEWHALE_CU_APP_SOCKET=savedSocket;
    if(savedApp===undefined) delete process.env.CODEWHALE_CU_APP; else process.env.CODEWHALE_CU_APP=savedApp;
    fs.rmSync(dir,{recursive:true,force:true});
  });
  await new Promise(resolve=>server.listen(process.env.CODEWHALE_CU_APP_SOCKET,resolve));
  await assert.rejects(executorFor({id:"local",transport:"local"}),error=>error.code==="app_upgrade_required" && /background/.test(error.message));
  assert.deepEqual(requests.map(request=>request.tool),["hello"]);
});

test("run captures stdout/stderr and exit codes without a shell", async () => {
  const r = await run("node", ["-e", "console.log('hello'); console.error('boo')"]);
  assert.equal(r.code, 0);
  assert.equal(r.stdout.trim(), "hello");
  assert.match(r.stderr, /boo/);
});

test("run reports missing executables as code -1 with ENOENT, never throws", async () => {
  const r = await run("definitely-not-a-real-tool-xyz", ["--version"]);
  assert.equal(r.code, -1);
  assert.match(r.stderr, /ENOENT/);
});

test("runOk throws ExecError on non-zero exit and includes stderr", async () => {
  await assert.rejects(() => runOk("node", ["-e", "console.error('reason-here'); process.exit(3)"]), (e) => {
    assert.ok(e instanceof ExecError);
    assert.match(e.message, /exited 3/);
    assert.match(e.message, /reason-here/);
    return true;
  });
});

test("run enforces timeouts", async () => {
  const r = await run("node", ["-e", "setInterval(()=>{},1000)"], { timeoutMs: 300 });
  assert.equal(r.timedOut, true);
});

test("run distinguishes an early cancellation from a child that was dispatched", async () => {
  const early = await withSignal(AbortSignal.abort(), () => run(process.execPath, ["-e", "process.exit(9)"]));
  assert.equal(early.aborted, true);
  assert.equal(early.spawned, false);
  const dispatched = await run(process.execPath, ["-e", "setInterval(()=>{}, 1000)"], { timeoutMs: 50 });
  assert.equal(dispatched.timedOut, true);
  assert.equal(dispatched.spawned, true);
});

test("cancelling a later pointer command closes its original input owner promptly", async t => {
  const dir=fs.mkdtempSync(path.join(os.tmpdir(),"cu-lease-cancel-"));
  t.after(()=>fs.rmSync(dir,{recursive:true,force:true}));
  const released=path.join(dir,"released");
  const lease=await runInputLease(process.execPath,["-e",`
    const fs=require('node:fs');
    console.log(JSON.stringify({action_sent:true,input_lease:true}));
    process.stdin.resume();
    process.stdin.on('data',()=>{});
    process.stdin.on('end',()=>{fs.writeFileSync(process.argv[1],'released');process.exit(0);});
  `,released]);
  const controller=new AbortController();
  const started=Date.now();
  const motion=withSignal(controller.signal,()=>lease.send({point:{x:12,y:34}}));
  setTimeout(()=>controller.abort(),50);
  await assert.rejects(motion,error=>error.code==='cancelled');
  assert.equal(fs.readFileSync(released,'utf8'),'released');
  assert.ok(Date.now()-started<1500,'later request cancellation must not wait for the 20-second motion timeout');
  await lease.release();
});

test("an exited input owner rejects later movement immediately", async () => {
  const lease=await runInputLease(process.execPath,["-e",`
    console.log(JSON.stringify({action_sent:true,input_lease:true}));
    setTimeout(()=>process.kill(process.pid,'SIGTERM'),20);
  `]);
  await new Promise(resolve=>setTimeout(resolve,100));
  const started=Date.now();
  await assert.rejects(lease.send({point:{x:1,y:2}}),/owner is closed/);
  assert.ok(Date.now()-started<500);
});

test("an unresponsive input helper is force-terminated within the MCP cleanup budget", async () => {
  const lease=await runInputLease(process.execPath,["-e",`
    process.on('SIGTERM',()=>{}); process.stdin.resume();
    process.stdin.on('data',()=>{}); process.stdin.on('end',()=>{});
    console.log(JSON.stringify({action_sent:true,input_lease:true}));
    setInterval(()=>{},1000);
  `]);
  const started=Date.now();
  await assert.rejects(lease.release(),error=>error.code==='input_release_failed' && error.result.signal==='SIGKILL');
  assert.ok(Date.now()-started<2500);
});

test("have() detects real and missing tools", async () => {
  assert.equal(await have("node"), true);
  assert.equal(await have("definitely-not-a-real-tool-xyz"), false);
});

test("safeRemotePath blocks traversal, metacharacters, and absolute escapes", () => {
  assert.equal(safeRemotePath(".codewhale-cu/agent/agent.mjs"), ".codewhale-cu/agent/agent.mjs");
  for (const bad of ["../../etc/passwd", "/etc/passwd", "a;rm -rf /", "a b", "$(id)", "a\nb", "a'b", ".codewhale-cu/../escape"]) {
    assert.throws(() => safeRemotePath(bad), ExecError, `should reject: ${bad}`);
  }
});

test("one-shot SSH agent refuses operations that outlive its request", async () => {
  for(const tool of ['left_mouse_down','recordingStart']) {
    const result=await run(process.execPath,['agent.mjs',b64({tool,args:{target:{x:1,y:2}}})]);
    assert.equal(result.code,0);
    assert.equal(JSON.parse(result.stdout).error.code,'persistent_session_required');
  }
});

test("b64 round-trips JSON payloads", () => {
  const obj = { tool: "screenshot", args: { region: [0, 0, 10, 10] } };
  assert.deepEqual(JSON.parse(Buffer.from(b64(obj), "base64").toString("utf8")), obj);
});

test("localExec provides run/runOk/tmpFile", async () => {
  const ex = localExec();
  const r = await ex.run("echo", ["hi"]);
  assert.equal(r.code, 0);
  const f = ex.tmpFile("cu-test-");
  assert.ok(typeof f === "string");
});

test("hdc readFile pulls into a private temp dir and cleans up only that dir", async (t) => {
  // Regression guard for the temp-dir deletion bug: readFile used to place the
  // pull directly in os.tmpdir() and then rm(dirname(tmp), {recursive}) —
  // deleting the ENTIRE user temp directory on every HDC read. The sentinel
  // proves sibling temp content now survives, and the pull must land inside a
  // private cu-hdc-* mkdtemp dir that is removed afterwards.
  const sentinel = path.join(os.tmpdir(), `cu-hdc-sentinel-${process.pid}-${Date.now()}.txt`);
  fs.writeFileSync(sentinel, "keep");
  t.after(() => fs.rmSync(sentinel, { force: true }));

  const ex = hdcExec({});
  let seenLocal = null;
  ex.pullFile = async (remote, local) => {
    seenLocal = local;
    fs.writeFileSync(local, Buffer.from("pulled-bytes"));
    return local;
  };
  const data = await ex.readFile("data/local/tmp/layout.json");
  assert.equal(data.toString(), "pulled-bytes");
  const pullDir = path.dirname(seenLocal);
  assert.equal(path.dirname(pullDir), os.tmpdir(), "pull must land in a direct child of tmpdir, never in tmpdir itself");
  assert.match(path.basename(pullDir), /^cu-hdc-/, "pull dir must be a private cu-hdc- mkdtemp dir");
  assert.ok(!fs.existsSync(pullDir), "private temp dir is removed after the read");
  assert.ok(fs.existsSync(sentinel), "sibling files in the user temp dir must survive an hdc read");
});

test("hdc readFile cleans up its private temp dir even when the pull fails", async () => {
  const ex = hdcExec({});
  let seenLocal = null;
  ex.pullFile = async (remote, local) => {
    seenLocal = local;
    throw new Error("hdc file recv failed");
  };
  await assert.rejects(() => ex.readFile("data/local/tmp/layout.json"), /hdc file recv failed/);
  assert.ok(seenLocal, "pull was attempted");
  assert.ok(!fs.existsSync(path.dirname(seenLocal)), "failed pull still cleans up its private temp dir");
});

test("hdc pullFile rejects traversal and shell metacharacters before execution", async () => {
  const ex = hdcExec({});
  for (const remote of ["/data/../secret", "/data/file;touch", "/data/$(touch)", "//data/file", null]) {
    await assert.rejects(ex.pullFile(remote, "/unused-fixture-output"), /refusing unsafe remote path/);
  }
});
