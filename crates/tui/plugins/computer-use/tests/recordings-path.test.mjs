// Caller-chosen screenshot/zoom output paths stay inside the recordings
// directory.
import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { recordingsOutputPath } from "../src/recordings.mjs";

function withRecordingsDir(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "cu-rec-path-"));
  const dir = path.join(root, "recordings");
  const old = process.env.CODEWHALE_CU_RECORDINGS_DIR;
  process.env.CODEWHALE_CU_RECORDINGS_DIR = dir;
  t.after(() => {
    if (old === undefined) delete process.env.CODEWHALE_CU_RECORDINGS_DIR; else process.env.CODEWHALE_CU_RECORDINGS_DIR = old;
    fs.rmSync(root, { recursive: true, force: true });
  });
  return { root, dir };
}

test("recordingsOutputPath keeps an omitted path on the default name", (t) => {
  withRecordingsDir(t);
  assert.equal(recordingsOutputPath(undefined), null);
  assert.equal(recordingsOutputPath(null), null);
});

test("recordingsOutputPath accepts image files inside the recordings directory", (t) => {
  const { dir } = withRecordingsDir(t);
  const shot = path.join(dir, "a.png");
  assert.equal(recordingsOutputPath(shot), shot);
  const nested = path.join(dir, "run-1", "b.jpeg");
  assert.equal(recordingsOutputPath(nested), nested);
  assert.ok(fs.statSync(path.dirname(nested)).isDirectory());
});

test("recordingsOutputPath refuses paths outside the recordings directory", (t) => {
  const { root, dir } = withRecordingsDir(t);
  const bad = [
    path.join(root, "outside.png"),
    path.join(dir, "..", "escape.png"),
    path.join(os.homedir(), ".zshrc.png"),
    "relative.png",
    "",
    123,
    path.join(dir, "a\0b.png"),
    dir,
  ];
  for (const file of bad) {
    assert.throws(() => recordingsOutputPath(file), (err) => err.code === "bad_args", String(file));
  }
  assert.equal(fs.existsSync(path.join(root, "outside.png")), false);
});

test("recordingsOutputPath refuses non-image extensions", (t) => {
  const { dir } = withRecordingsDir(t);
  for (const name of ["x.txt", "x.sh", "x.png.txt", "authorized_keys"]) {
    assert.throws(() => recordingsOutputPath(path.join(dir, name)), /must end in \.png, \.jpg or \.jpeg/);
  }
});

test("recordingsOutputPath refuses symlinks that lead out of the recordings directory", { skip: process.platform === "win32" }, (t) => {
  const { root, dir } = withRecordingsDir(t);
  const outside = path.join(root, "outside");
  fs.mkdirSync(outside);
  fs.mkdirSync(dir, { recursive: true });
  fs.symlinkSync(outside, path.join(dir, "linkdir"));
  assert.throws(() => recordingsOutputPath(path.join(dir, "linkdir", "a.png")), /symlink/);
  assert.throws(() => recordingsOutputPath(path.join(dir, "linkdir", "deeper", "a.png")), /symlink/);
  assert.equal(fs.existsSync(path.join(outside, "deeper")), false, "no directory is created through the link");
  fs.symlinkSync(path.join(outside, "target.png"), path.join(dir, "link.png"));
  assert.throws(() => recordingsOutputPath(path.join(dir, "link.png")), /symlink/);
});

// Every backend's screenshot and zoom must route a caller path through
// recordingsOutputPath before running anything. The exec records calls, so a
// backend that fell back to the raw path would reach it (or write a file).
const BACKENDS = ["darwin", "linux", "win32", "harmonyos"];

for (const name of BACKENDS) {
  test(`${name} screenshot and zoom refuse output paths outside the recordings directory`, async (t) => {
    const { root, dir } = withRecordingsDir(t);
    const calls = [];
    const fail = async (...args) => { calls.push(args); return { code: 1, stdout: "", stderr: "unexpected call" }; };
    const exec = { targetArgs: [], run: fail, runOk: fail, shell: fail, pullFile: fail, readFile: fail };
    const { create } = await import(`../src/backends/${name}.mjs`);
    const backend = create({ exec });
    const outside = path.join(root, "outside.png");
    const notImage = path.join(dir, "inside.txt");
    for (const file of [outside, notImage]) {
      await assert.rejects(backend.screenshot({ path: file }), (err) => err.code === "bad_args", `${name} screenshot ${file}`);
      if (name !== "harmonyos") {
        await assert.rejects(backend.zoom({ region: [0, 0, 1, 1], path: file }), (err) => err.code === "bad_args", `${name} zoom ${file}`);
      }
      assert.equal(fs.existsSync(file), false, `${name} wrote ${file}`);
    }
    assert.deepEqual(calls, [], `${name} ran a command before refusing the path`);
  });
}
