// Win32 backend tests that run on ANY host via the injectable runner seam
// (`create({ exec })`): the fake runner captures the PowerShell scripts so the
// generated commands can be asserted directly, and no real powershell.exe is
// ever spawned. These pin the failure-truthful and self-contained-action
// behavior ported from the codewhale-side hardening (injectable runner +
// truthful PowerShell failure reporting) without needing a Windows host or a
// fake-powershell.exe-on-PATH fixture.
import { test } from "node:test";
import assert from "node:assert/strict";

function decodeScript(args) {
  const i = args.indexOf("-EncodedCommand");
  if (i === -1) return null;
  return Buffer.from(args[i + 1], "base64").toString("utf16le");
}

function mockExec({ fail = false } = {}) {
  const calls = [];
  const run = async (_cmd, args) => {
    calls.push({ script: decodeScript(args) });
    if (fail) return { code: 1, stdout: "", stderr: "simulated powershell failure" };
    return { code: 0, stdout: '{"ok": true}\n', stderr: "" };
  };
  return { run, calls };
}

test("win32: actions run through an injected runner (no powershell needed)", async () => {
  const { run, calls } = mockExec();
  const mod = await import("../src/backends/win32.mjs");
  const b = mod.create({ exec: { run } });
  const r = await b.left_click({ target: { x: 5, y: 6 } });
  assert.equal(r.action_sent, true);
  assert.ok(calls.length >= 1, "the injected runner must receive the action command");
});

test("win32: input actions fail truthfully on a nonzero exit", async () => {
  const { run } = mockExec({ fail: true });
  const mod = await import("../src/backends/win32.mjs");
  const b = mod.create({ exec: { run } });
  await assert.rejects(() => b.left_click({ target: { x: 1, y: 2 } }), /exited 1/);
  await assert.rejects(() => b.left_mouse_down({ target: { x: 1, y: 2 } }), /exited 1/);
});

test("win32: coordinate clicks refuse strategy=a11y instead of silently degrading", async () => {
  const { run, calls } = mockExec();
  const mod = await import("../src/backends/win32.mjs");
  const b = mod.create({ exec: { run } });
  // left_click refuses synchronously (it is not async), like every fail-closed
  // guard in this backend — see backends.test.mjs for the same convention.
  assert.throws(() => b.left_click({ target: { x: 1, y: 2 }, strategy: "a11y" }), /macOS-only/);
  assert.equal(calls.length, 0, "no command may be spawned for a refused strategy");
  // auto and event remain accepted on this backend.
  assert.equal((await b.left_click({ target: { x: 1, y: 2 }, strategy: "event" })).action_sent, true);
});

test("win32: targeted left_mouse_down both moves and presses, self-contained", async () => {
  const { run, calls } = mockExec();
  const mod = await import("../src/backends/win32.mjs");
  const b = mod.create({ exec: { run } });
  await b.left_mouse_down({ target: { x: 12, y: 34 } });
  const script = calls.at(-1).script;
  // Self-contained: the User32 P/Invoke type travels with the action.
  assert.ok(script.includes("public static class User32"), "action must define User32 in its own process");
  assert.ok(script.includes("SetCursorPos(12, 34)"), "must move the cursor to the target");
  assert.ok(script.includes("LEFTDOWN"), "must press the left button");
});

test("win32: every User32 action carries the type prelude in-process", async () => {
  const { run, calls } = mockExec();
  const mod = await import("../src/backends/win32.mjs");
  const b = mod.create({ exec: { run } });
  await b.mouse_move({ target: { x: 3, y: 4 } });
  await b.key({ text: "a" });
  assert.ok(calls.length >= 2, "two actions should have run through the injected runner");
  for (const call of calls) {
    assert.ok(call.script.includes("public static class User32"), "each action must redefine User32 in-process");
  }
});
