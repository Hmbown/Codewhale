// Windows backend input truthfulness tests. No Windows box (or GUI) is
// involved: a fake powershell.exe is placed on PATH, captures the decoded
// -EncodedCommand payload of every spawn, and exits with a code the test
// controls — mirroring how the failure-truthfulness was verified when the
// fix was developed on the codewhale side before this port.
import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import win32 from "../src/backends/win32.mjs";
import { ExecError } from "../src/exec.mjs";

// Fake powershell.exe logic. It lives in a .mjs file because Node's loader
// refuses to execute a script whose name ends in .exe; powershell.exe itself
// is a #!/bin/sh launcher that execs node on this file (extension is
// irrelevant to the kernel, only to Node's module loader).
const FAKE_PS_MJS = `
import fs from "node:fs";
const args = process.argv.slice(2);
const i = args.indexOf("-EncodedCommand");
const script = i >= 0 ? Buffer.from(args[i + 1], "base64").toString("utf16le") : "";
if (process.env.CU_FAKE_PS_CAPTURE) {
  fs.appendFileSync(process.env.CU_FAKE_PS_CAPTURE, JSON.stringify({ args, script }) + "\\n");
}
if (process.env.CU_FAKE_PS_STDOUT) process.stdout.write(process.env.CU_FAKE_PS_STDOUT);
if (process.env.CU_FAKE_PS_STDERR) process.stderr.write(process.env.CU_FAKE_PS_STDERR);
process.exit(Number(process.env.CU_FAKE_PS_EXIT || 0));
`;

/**
 * Install a controllable fake powershell.exe on PATH (or, with onPath:false,
 * guarantee no powershell.exe at all) and restore every mutation in t.after.
 * Returns calls(): the decoded {args, script} of each spawn so far.
 */
function fakePowershell(t, { exit = 0, stdout = "", stderr = "", onPath = true } = {}) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "cu-win32-test-"));
  const capture = path.join(dir, "captured.jsonl");
  if (onPath) {
    const mjs = path.join(dir, "ps-fake.mjs");
    fs.writeFileSync(mjs, FAKE_PS_MJS);
    const bin = path.join(dir, "powershell.exe");
    fs.writeFileSync(bin, `#!/bin/sh\nexec node ${JSON.stringify(mjs)} "$@"\n`);
    fs.chmodSync(bin, 0o755);
  }
  const saved = {
    PATH: process.env.PATH,
    CU_FAKE_PS_EXIT: process.env.CU_FAKE_PS_EXIT,
    CU_FAKE_PS_STDOUT: process.env.CU_FAKE_PS_STDOUT,
    CU_FAKE_PS_STDERR: process.env.CU_FAKE_PS_STDERR,
    CU_FAKE_PS_CAPTURE: process.env.CU_FAKE_PS_CAPTURE,
  };
  process.env.PATH = onPath ? `${dir}${path.delimiter}${process.env.PATH}` : dir;
  process.env.CU_FAKE_PS_EXIT = String(exit);
  process.env.CU_FAKE_PS_STDOUT = stdout;
  process.env.CU_FAKE_PS_STDERR = stderr;
  process.env.CU_FAKE_PS_CAPTURE = capture;
  t.after(() => {
    process.env.PATH = saved.PATH;
    for (const [k, v] of Object.entries({
      CU_FAKE_PS_EXIT: saved.CU_FAKE_PS_EXIT,
      CU_FAKE_PS_STDOUT: saved.CU_FAKE_PS_STDOUT,
      CU_FAKE_PS_STDERR: saved.CU_FAKE_PS_STDERR,
      CU_FAKE_PS_CAPTURE: saved.CU_FAKE_PS_CAPTURE,
    })) {
      if (v === undefined) delete process.env[k];
      else process.env[k] = v;
    }
    fs.rmSync(dir, { recursive: true, force: true });
  });
  const calls = () =>
    fs.existsSync(capture)
      ? fs.readFileSync(capture, "utf8").split("\n").filter(Boolean).map((l) => JSON.parse(l))
      : [];
  return { calls };
}

test("win32: targeted left_mouse_down both moves and presses in one self-contained command", async (t) => {
  const fake = fakePowershell(t);
  const backend = win32.create();
  const r = await backend.left_mouse_down({ target: { x: 12, y: 34 } });
  assert.deepEqual(r, { action_sent: true });
  const calls = fake.calls();
  assert.equal(calls.length, 1, "exactly one PowerShell process — no bootstrap process is relied on");
  assert.ok(calls[0].args.includes("-EncodedCommand"), "script travels as an encoded command");
  const script = calls[0].script;
  assert.match(script, /Add-Type -TypeDefinition/, "the action command carries the type definition itself");
  assert.match(script, /public static class User32/);
  assert.match(script, /\[User32\]::SetCursorPos\(12, 34\)/, "targeted press moves the cursor");
  assert.match(script, /\[User32\]::mouse_event\(\[User32\]::LEFTDOWN/, "targeted press also presses");
});

test("win32: PowerShell exit != 0 becomes an error, never action_sent:true", async (t) => {
  fakePowershell(t, { exit: 1, stderr: "unable to find type [User32]" });
  const backend = win32.create();
  const cases = [
    ["left_mouse_down", () => backend.left_mouse_down({ target: { x: 1, y: 2 } })],
    ["mouse_move", () => backend.mouse_move({ target: { x: 1, y: 2 } })],
    ["key", () => backend.key({ text: "enter" })],
  ];
  for (const [name, fn] of cases) {
    await assert.rejects(fn(), (e) => {
      assert.ok(e instanceof ExecError, `${name} rejects with ExecError`);
      assert.match(e.message, /exited 1/, `${name} reports the nonzero exit code`);
      assert.match(e.message, /unable to find type/, `${name} surfaces PowerShell stderr`);
      return true;
    }, `${name} must not report success when PowerShell exits nonzero`);
  }
});

test("win32: spawn failure (powershell.exe missing) becomes an error, never action_sent:true", async (t) => {
  fakePowershell(t, { onPath: false });
  const backend = win32.create();
  await assert.rejects(backend.left_mouse_down({ target: { x: 1, y: 2 } }), (e) => {
    assert.ok(e instanceof ExecError);
    assert.match(e.message, /exited -1|ENOENT/);
    return true;
  });
});

test("win32: successful input still reports success", async (t) => {
  const fake = fakePowershell(t);
  const backend = win32.create();
  assert.deepEqual(await backend.mouse_move({ target: { x: 5, y: 6 } }), { action_sent: true, at: { x: 5, y: 6 } });
  const click = await backend.left_click({ target: { x: 9, y: 8 } });
  assert.equal(click.action_sent, true);
  const clickScript = fake.calls()[1].script;
  assert.match(clickScript, /\[User32\]::SetCursorPos\(9, 8\)/);
  assert.match(clickScript, /\[User32\]::LEFTDOWN/);
  assert.match(clickScript, /\[User32\]::LEFTUP/);
});

test("win32: every User32-backed action carries the Add-Type definition in its own process", async (t) => {
  const fake = fakePowershell(t);
  const backend = win32.create();
  const at = { x: 3, y: 4 };
  const actions = [
    ["mouse_move", () => backend.mouse_move({ target: at })],
    ["left_mouse_up", () => backend.left_mouse_up()],
    ["left_click", () => backend.left_click({ target: at })],
    ["double_click", () => backend.double_click({ target: at })],
    ["right_click", () => backend.right_click({ target: at })],
    ["middle_click", () => backend.middle_click({ target: at })],
    ["left_click_drag", () => backend.left_click_drag({ from_target: at, to: { x: 10, y: 11 } })],
    ["scroll", () => backend.scroll({ target: at, direction: "down", amount: 2 })],
    ["key", () => backend.key({ text: "ctrl+a" })],
    ["hold_key", () => backend.hold_key({ text: "a", duration: 0.05 })],
  ];
  for (const [, fn] of actions) await fn();
  const calls = fake.calls();
  assert.equal(calls.length, actions.length, "exactly one self-contained PowerShell process per action");
  for (const [i, [name]] of actions.entries()) {
    assert.match(calls[i].script, /Add-Type -TypeDefinition/, `${name} is self-contained`);
    assert.match(calls[i].script, /public static class User32/, `${name} defines User32 inline`);
  }
});

test("win32: cursor_position is self-contained and parses JSON output", async (t) => {
  const fake = fakePowershell(t, { stdout: '{"x": 11, "y": 22}' });
  const backend = win32.create();
  assert.deepEqual(await backend.cursor_position(), { x: 11, y: 22 });
  const script = fake.calls()[0].script;
  assert.match(script, /Add-Type -TypeDefinition/, "cursor_position carries the type definition");
  assert.match(script, /GetCursorPos/);
});
