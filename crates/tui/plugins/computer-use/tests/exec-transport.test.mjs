// exec + transport safety tests.
import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { run, runOk, ExecError, have, trim } from "../src/exec.mjs";
import { safeRemotePath, b64, localExec, hdcExec } from "../src/transport.mjs";

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
