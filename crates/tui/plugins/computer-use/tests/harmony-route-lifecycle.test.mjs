import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { create } from "../src/backends/harmonyos.mjs";
import { withSignal } from "../src/exec.mjs";

const ok = { code: 0, stdout: "", stderr: "" };

async function until(predicate, message) {
  const deadline = Date.now() + 2_000;
  while (!predicate() && Date.now() < deadline) await delay(10);
  assert.ok(predicate(), message);
}

function fixture(t, { blockAt = 2, blockStage = "capture", ignoreAbort = false } = {}) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "cu-harmony-lifecycle-"));
  const originalPath = process.env.PATH;
  const originalRecordingsDir = process.env.CODEWHALE_CU_RECORDINGS_DIR;
  const muxMarker = path.join(root, "unexpected-mux");
  // Only have("ffmpeg") should inspect this executable. No encoder or device
  // program runs; invoking it makes the fixture fail during cleanup.
  const ffmpeg = path.join(root, "ffmpeg");
  fs.writeFileSync(ffmpeg, `#!${process.execPath}\nrequire('node:fs').writeFileSync(${JSON.stringify(muxMarker)}, 'invoked'); process.exit(97);\n`, { mode: 0o700 });
  process.env.PATH = `${root}${path.delimiter}${originalPath ?? ""}`;
  process.env.CODEWHALE_CU_RECORDINGS_DIR = path.join(root, "recordings");

  const frameDirs = new Set();
  const blocked = new Set();
  const captures = [];
  const pulls = [];
  let aborts = 0;
  function pause(signal) {
    return new Promise((resolve, reject) => {
      const finish = (error) => {
        signal.removeEventListener("abort", abort);
        blocked.delete(release);
        if (error) reject(error); else resolve();
      };
      const release = () => finish();
      const abort = () => {
        aborts++;
        if (!ignoreAbort) finish(Object.assign(new Error("fixture hdc aborted"), { code: "cancelled" }));
      };
      blocked.add(release);
      signal.addEventListener("abort", abort, { once: true });
      if (signal.aborted) abort();
    });
  }
  const backend = create({ exec: {
    async shell(args, { signal } = {}) {
      assert.ok(signal instanceof AbortSignal, "capture and cleanup use the recording owner's signal");
      if (args[0] === "rm") return signal.aborted ? { ...ok, code: -1 } : ok;
      assert.equal(args[0], "snapshot_display", "the fake never launches a real device command");
      assert.equal(signal.aborted, false, "no new capture starts after its owner stops");
      captures.push({ remote: args[2], signal });
      if (blockStage === "capture" && captures.length >= blockAt) await pause(signal);
      return ok;
    },
    async pullFile(remote, local, { signal } = {}) {
      pulls.push({ remote, local, signal });
      frameDirs.add(path.dirname(local));
      fs.writeFileSync(local, "partial fixture frame");
      if (blockStage === "pull" && pulls.length >= blockAt) await pause(signal);
      fs.writeFileSync(local, `completed fixture frame: ${remote}`);
      return local;
    },
  } });
  t.after(async () => {
    for (const release of [...blocked]) release();
    const closed = await backend.closeSession();
    if (closed?.framesDir) frameDirs.add(closed.framesDir);
    if (originalPath === undefined) delete process.env.PATH; else process.env.PATH = originalPath;
    if (originalRecordingsDir === undefined) delete process.env.CODEWHALE_CU_RECORDINGS_DIR;
    else process.env.CODEWHALE_CU_RECORDINGS_DIR = originalRecordingsDir;
    const muxed = fs.existsSync(muxMarker);
    for (const dir of frameDirs) fs.rmSync(dir, { recursive: true, force: true });
    fs.rmSync(root, { recursive: true, force: true });
    assert.equal(muxed, false, "route/session close must not invoke ffmpeg");
  });
  return {
    backend, captures, pulls, frameDirs,
    get blocked() { return blocked.size; },
    get aborts() { return aborts; },
    release() { for (const resume of [...blocked]) resume(); },
  };
}

test("Harmony route close aborts a pending capture, retains frames, and stops its timer", { skip: process.platform === "win32" }, async t => {
  const f = fixture(t);
  const recording = await f.backend.recordingStart({ intervalMs: 150 });
  await until(() => f.blocked === 1, "the second capture must be in flight");
  await delay(350);
  assert.equal(f.captures.length, 2, "several timer periods cannot overlap a pending frame");
  const saved = fs.readFileSync(f.pulls[0].local);
  const closed = await f.backend.closeSession();
  assert.equal(f.aborts, 1);
  assert.equal(closed.id, recording.id);
  assert.equal(closed.frames, 1);
  assert.equal(closed.framesDir, path.dirname(f.pulls[0].local));
  assert.deepEqual(fs.readFileSync(f.pulls[0].local), saved, "closing must preserve the completed frame bytes");
  assert.equal((await f.backend.recordingStatus({ id: recording.id })).running, false);
  await delay(350);
  assert.equal(f.captures.length, 2, "no timer capture occurs after close resolves");
  assert.equal(await f.backend.closeSession(), null, "closing an already quiesced owner is harmless");
});

test("Harmony close during its initial frame preserves partial output and prevents a late timer", { skip: process.platform === "win32" }, async t => {
  const f = fixture(t, { blockAt: 1, blockStage: "pull" });
  const startup = f.backend.recordingStart({ intervalMs: 150 });
  const rejected = assert.rejects(startup, /closed during startup/);
  await until(() => f.blocked === 1, "startup must reach the pending pull");
  const partial = f.pulls[0].local;
  await f.backend.closeSession();
  await rejected;
  assert.equal(f.aborts, 1);
  assert.equal(fs.readFileSync(partial, "utf8"), "partial fixture frame");
  await delay(350);
  assert.equal(f.captures.length, 1, "startup must not install a timer after its close");
  assert.deepEqual((await f.backend.recordingList()).running, []);
});

test("Harmony recording startup cancellation propagates to its owned frame", { skip: process.platform === "win32" }, async t => {
  const f = fixture(t, { blockAt: 1, blockStage: "pull" });
  const controller = new AbortController();
  const startup = withSignal(controller.signal, () => f.backend.recordingStart({ intervalMs: 150 }));
  const rejected = assert.rejects(startup, error => error.code === "cancelled");
  await until(() => f.blocked === 1, "startup must own a frame before cancellation");
  controller.abort();
  await rejected;
  assert.equal(f.aborts, 1);
  assert.equal(fs.readFileSync(f.pulls[0].local, "utf8"), "partial fixture frame");
  await delay(350);
  assert.equal(f.captures.length, 1);
  assert.deepEqual((await f.backend.recordingList()).running, []);
});

test("Harmony failed quiesce retains recorder ownership until a later cleanup succeeds", { skip: process.platform === "win32" }, async t => {
  const f = fixture(t, { blockStage: "pull", ignoreAbort: true });
  const recording = await f.backend.recordingStart({ intervalMs: 150 });
  await until(() => f.blocked === 1, "a stubborn frame must be in flight");
  const began = Date.now();
  await assert.rejects(f.backend.closeSession(), /did not stop within 2 seconds/);
  assert.ok(Date.now() - began < 3_000, "failed cleanup must remain inside the MCP shutdown budget");
  assert.equal(f.aborts, 1);
  await assert.rejects(f.backend.recordingStart(), /already running/, "a new recorder cannot replace an unquiesced owner");
  assert.equal(fs.readFileSync(f.pulls[1].local, "utf8"), "partial fixture frame");
  assert.equal(f.captures.length, 2, "timers stay stopped even after cleanup times out");
  f.release();
  const closed = await f.backend.closeSession();
  assert.equal(closed.id, recording.id, "retry still owns the original recorder");
  assert.equal(closed.framesDir, path.dirname(f.pulls[0].local));
  assert.equal(closed.frames, 2);
  assert.equal(fs.readdirSync(closed.framesDir).length, 2);
  await delay(350);
  assert.equal(f.captures.length, 2);
  assert.equal((await f.backend.recordingStatus({ id: recording.id })).running, false);
});
