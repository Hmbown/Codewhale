// Platform command tests use injected runners, never a real Windows/Linux GUI.
import { test } from "node:test";
import assert from "node:assert/strict";
import windows from "../src/backends/win32.mjs";
import linux from "../src/backends/linux.mjs";
import { currentSignal, withSignal } from "../src/exec.mjs";

const success = { code: 0, stdout: '{"ok":true}', stderr: "", timedOut: false };
const decode = (args) => Buffer.from(args[args.indexOf("-EncodedCommand") + 1], "base64").toString("utf16le");

for (const [name, invoke, release] of [
  ["key hold", (backend) => backend.hold_key({ text: "ctrl+a", duration: 30 }), /SendKey\(65, 2\).*SendKey\(17, 2\)/s],
  ["drag", (backend) => backend.left_click_drag({ from_target: { x: 1, y: 2 }, to: { x: 30, y: 40 } }), /mouse_event\(\[User32\]::LEFTUP/],
  ["right click", (backend) => backend.right_click({ target: { x: 1, y: 2 } }), /mouse_event\(\[User32\]::RIGHTUP/],
]) {
  test(`Windows ${name} cancellation releases the owned input in an uncancelled process`, async () => {
    const controller = new AbortController();
    const calls = [];
    const backend = windows.create({ exec: { persistentInputOwner: true, run: async (_cmd, args) => {
      calls.push({ script: decode(args), signal: currentSignal() });
      if (calls.length === 1) {
        controller.abort();
        return { ...success, code: null, aborted: true };
      }
      return success;
    } } });
    await assert.rejects(withSignal(controller.signal, () => invoke(backend)), (err) => err.code === "cancelled");
    assert.equal(calls.length, 2);
    assert.equal(calls[1].signal, null, "cleanup must survive the cancelled request");
    assert.match(calls[1].script, release);
    await backend.releaseInput();
    assert.equal(calls.length, 2, "released input is no longer owned");
  });
}

test("Windows session cleanup releases a completed mouse-down without releasing another session", async () => {
  const calls = [];
  const create = () => windows.create({ exec: { persistentInputOwner: true, run: async (_cmd, args) => { calls.push(decode(args)); return success; } } });
  const owner = create();
  const other = create();
  await owner.left_mouse_down({ target: { x: 1, y: 2 } });
  await owner.key({ text: "a" });
  await other.releaseInput();
  assert.equal(calls.length, 2, "a normal key and another session's cleanup retain the owned mouse-down");
  await owner.releaseInput();
  assert.equal(calls.length, 3);
  assert.match(calls.at(-1), /mouse_event\(\[User32\]::LEFTUP/);
});

function fakeLinux(t, kind, onRun) {
  const saved = Object.fromEntries(["DISPLAY", "WAYLAND_DISPLAY", "XDG_SESSION_TYPE"].map((key) => [key, process.env[key]]));
  delete process.env.DISPLAY;
  delete process.env.WAYLAND_DISPLAY;
  process.env.XDG_SESSION_TYPE = kind;
  process.env[kind === "x11" ? "DISPLAY" : "WAYLAND_DISPLAY"] = "test-only";
  t.after(() => { for (const [key, value] of Object.entries(saved)) { if (value === undefined) delete process.env[key]; else process.env[key] = value; } });
  const calls = [];
  const backend = linux.create({ exec: {
    persistentInputOwner: true,
    have: async () => true,
    run: async (cmd, args) => {
      const call = { cmd, args, signal: currentSignal() };
      calls.push(call);
      return await onRun?.(call) ?? success;
    },
  } });
  return { backend, calls };
}

test("X11 cancelled hold and drag release their keys/buttons with cancellation disabled", async (t) => {
  let controller = new AbortController();
  const { backend, calls } = fakeLinux(t, "x11", ({ args }) => {
    if (["keydown", "mousedown"].includes(args[0])) controller.abort();
  });
  await assert.rejects(withSignal(controller.signal, () => backend.hold_key({ text: "ctrl+a", duration: 30 })), (err) => err.code === "cancelled");
  assert.deepEqual(calls.at(-1).args, ["keyup", "ctrl+a"]);
  assert.equal(calls.at(-1).signal, null);
  controller = new AbortController();
  await assert.rejects(withSignal(controller.signal, () => backend.left_click_drag({ from_target: { x: 1, y: 2 }, to: { x: 20, y: 30 } })), (err) => err.code === "cancelled");
  assert.deepEqual(calls.at(-1).args, ["mouseup", "1"]);
  assert.equal(calls.at(-1).signal, null);
});

test("X11 targeted mouse-down initializes input, aims, and retains release ownership", async (t) => {
  const { backend, calls } = fakeLinux(t, "x11");
  await backend.left_mouse_down({ target: { x: 12, y: 34 } });
  assert.deepEqual(calls.slice(-2).map((call) => call.args), [["mousemove", "--sync", "12", "34"], ["mousedown", "1"]]);
  await backend.releaseInput();
  assert.deepEqual(calls.at(-1).args, ["mouseup", "1"]);
  const count = calls.length;
  await backend.releaseInput();
  assert.equal(calls.length, count);
});

test("Wayland hold and repeated shortcuts use one complete temporary keyboard gesture", async (t) => {
  const { backend, calls } = fakeLinux(t, "wayland");
  await backend.hold_key({ text: "ctrl+left", duration: 2 });
  assert.deepEqual(calls.at(-1).args, ["-M", "ctrl", "-P", "Left", "-s", "2000", "-p", "Left", "-m", "ctrl"]);
  await backend.key({ text: "alt+tab", repeat: 2 });
  assert.deepEqual(calls.at(-1).args, ["-M", "alt", "-P", "Tab", "-p", "Tab", "-P", "Tab", "-p", "Tab", "-m", "alt"]);
  assert.equal(calls.filter((call) => call.cmd === "wtype").length, 2);
  assert.ok(!calls.some((call) => call.cmd === "ydotool"));
});

test("Wayland child cancellation is reported as cancelled instead of success", async (t) => {
  const controller = new AbortController();
  const { backend } = fakeLinux(t, "wayland", ({ cmd }) => {
    if (cmd === "wtype") { controller.abort(); return { ...success, code: null, aborted: true }; }
  });
  await assert.rejects(withSignal(controller.signal, () => backend.hold_key({ text: "shift", duration: 30 })), (err) => err.code === "cancelled");
});

for (const [name, platform] of [["Windows", windows], ["Linux", linux]]) {
  test(`${name} refuses recording before spawning an unowned recorder`, async () => {
    const calls = [];
    const backend = platform.create({ exec: { persistentInputOwner: true, run: async (...args) => { calls.push(args); return success; } } });
    await assert.rejects(backend.recordingStart({ fps: 15 }), (err) => err.code === "owned_recording_unavailable");
    assert.deepEqual(calls, []);
    assert.deepEqual(await backend.recordingStatus({ id: "not-started" }), { id: "not-started", running: false });
  });

  test(`${name} direct mode refuses held gestures before moving or pressing`, async () => {
    const calls = [];
    const backend = platform.create({ exec: { run: async (...args) => { calls.push(args); return success; } } });
    for (const [tool, args] of [
      ["left_mouse_down", { target: { x: 1, y: 2 } }],
      ["left_click_drag", { from_target: { x: 1, y: 2 }, to: { x: 3, y: 4 } }],
      ["hold_key", { text: "shift", duration: 1 }],
    ]) await assert.rejects(backend[tool](args), (err) => err.code === "input_owner_required");
    await assert.rejects(backend.left_mouse_up(), (err) => err.code === "input_not_held");
    await backend.releaseInput();
    assert.deepEqual(calls, [], "no command or unrelated release is sent without an input owner");
  });
}
