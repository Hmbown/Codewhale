// Backend tests that can run on any host: harmony logic via a mocked hdc
// exec, linux fail-closed probing, and module-shape checks for win32.
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { parseBounds, flatten } from "../src/backends/harmonyos.mjs";

function fakeJpeg(w, h) {
  // Minimal JPEG with an SOF0 marker carrying the dimensions.
  return Buffer.from([
    0xff, 0xd8, 0xff, 0xc0, 0x00, 0x0b, 0x08, (h >> 8) & 0xff, h & 0xff,
    (w >> 8) & 0xff, w & 0xff, 0x01, 0x01, 0x11, 0x00, 0xff, 0xd9,
  ]);
}

function harmonyFixtureExec(t) {
  const layout = {
    attributes: { bundleName: "com.example.app", type: "FrameNode" },
    children: [
      {
        attributes: { type: "Button", text: "OK", id: "ok_btn", bounds: "[100,200][300,260]" },
        children: [],
      },
      {
        attributes: { type: "Text", text: "Hello", bounds: "[0,0][100,50]" },
        children: [{ attributes: { type: "Text", text: "nested", bounds: "[10,10][90,40]" }, children: [] }],
      },
    ],
  };
  const calls = [];
  const exec = {
    targetArgs: [],
    run: async () => ({ code: 0, stdout: "", stderr: "" }),
    runOk: async () => ({ code: 0, stdout: "", stderr: "" }),
    shell: async (args) => { calls.push({ kind: "shell", args }); return { code: 0, stdout: "", stderr: "" }; },
    async pullFile(remote, local) {
      calls.push({ kind: "pull", remote, local });
      if (remote.includes("layout")) fs.writeFileSync(local, Buffer.from(JSON.stringify(layout)));
      else fs.writeFileSync(local, fakeJpeg(168, 120));
      return local;
    },
    async readFile(remote) {
      if (remote.includes("layout")) return Buffer.from(JSON.stringify(layout));
      return fakeJpeg(168, 120);
    },
  };
  return { exec, calls, layout };
}

test("harmony: parseBounds handles uitest bounds strings", () => {
  assert.deepEqual(parseBounds("[100,200][300,260]"), { x: 100, y: 200, w: 200, h: 60, cx: 200, cy: 230 });
  assert.equal(parseBounds("garbage"), null);
});

test("harmony: flatten produces indexed elements with paths and geometry", () => {
  const tree = { attributes: { type: "root" }, children: [{ attributes: { type: "Button", text: "OK", bounds: "[0,0][10,10]" } }] };
  const els = flatten(tree);
  assert.equal(els[0].role, "root");
  assert.equal(els[0].path.length, 0);
  assert.equal(els[1].label, "OK");
  assert.deepEqual(els[1].path, [0]);
  assert.equal(els[1].bounds.cx, 5);
});

test("harmony: get_app_state flattens dumpLayout with indices and actions", async () => {
  const { exec } = harmonyFixtureExec();
  const mod = await import("../src/backends/harmonyos.mjs");
  const b = mod.create({ exec });
  const st = await b.get_app_state({});
  assert.equal(st.bundle_id, "com.example.app");
  assert.ok(st.elements.length >= 4);
  const ok = st.elements.find((e) => e.label === "OK");
  assert.ok(ok, "OK button flattened");
  assert.deepEqual(ok.bounds, { x: 100, y: 200, w: 200, h: 60, cx: 200, cy: 230 });
  assert.ok(ok.actions.includes("click"));
});

test("harmony: screenshot pulls the file and reports panel dimensions", async (t) => {
  const { exec } = harmonyFixtureExec();
  const mod = await import("../src/backends/harmonyos.mjs");
  const b = mod.create({ exec });
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "cu-hm-test-"));
  const oldRec = process.env.CODEWHALE_CU_RECORDINGS_DIR; process.env.CODEWHALE_CU_RECORDINGS_DIR = dir;
  t.after(() => { if (oldRec === undefined) delete process.env.CODEWHALE_CU_RECORDINGS_DIR; else process.env.CODEWHALE_CU_RECORDINGS_DIR = oldRec; });
  const shot = await b.screenshot({ path: path.join(dir, "shot.jpeg") });
  assert.equal(shot.pixels.w, 168);
  assert.equal(shot.pixels.h, 120);
  assert.ok(fs.existsSync(shot.file));
  fs.rmSync(dir, { recursive: true, force: true });
});

test("harmony: click routes through uitest uiInput with validated args", async () => {
  const { exec, calls } = harmonyFixtureExec();
  const mod = await import("../src/backends/harmonyos.mjs");
  const b = mod.create({ exec });
  const r = await b.left_click({ target: { x: 123.6, y: 45.2 } });
  assert.equal(r.action_sent, true);
  const ui = calls.find((c) => c.args[0] === "uitest");
  assert.deepEqual(ui.args, ["uitest", "uiInput", "click", "124", "45"]);
});

test("harmony: element actions re-find the observed element and refuse a reordered tree", async () => {
  const { exec, calls, layout } = harmonyFixtureExec();
  const mod = await import("../src/backends/harmonyos.mjs");
  const b = mod.create({ exec });
  const ok = (await b.get_app_state({})).elements.find((e) => e.label === "OK");
  const target = { index: ok.index, path: ok.path, role: ok.role, label: ok.label };
  await b.perform_action({ target, action: "click" });
  assert.deepEqual(calls.filter((c) => c.args?.[1] === "uiInput").at(-1).args, ["uitest", "uiInput", "click", "200", "230"]);
  // A new sibling ahead of OK shifts every uitest index: the stored index now
  // names "Delete". Nothing may be clicked.
  layout.children.unshift({ attributes: { type: "Button", text: "Delete", bounds: "[0,300][100,360]" }, children: [] });
  const before = calls.filter((c) => c.args?.[1] === "uiInput").length;
  await assert.rejects(b.perform_action({ target, action: "click" }), (error) => error.code === "element_stale" && /label changed/.test(error.message));
  await assert.rejects(b.set_value({ target, value: "x" }), (error) => error.code === "element_stale");
  await assert.rejects(b.perform_action({ target: { index: ok.index }, action: "click" }), (error) => error.code === "element_stale", "a bare index has no identity to verify");
  assert.equal(calls.filter((c) => c.args?.[1] === "uiInput").length, before, "no input after the tree changed");
});

test("harmony: clipboard and select_text fail closed with named reasons", async () => {
  const { exec } = harmonyFixtureExec();
  const mod = await import("../src/backends/harmonyos.mjs");
  const b = mod.create({ exec });
  await assert.rejects(() => b.read_clipboard(), /not exposed by hdc/);
  await assert.rejects(() => b.select_text({}), /not exposed by uitest/);
  assert.throws(() => b.key({ text: "cmd+c" }), /unsupported key/);
});

test("linux: probe reports the session and names missing tools (fail-closed)", async () => {
  const mod = await import("../src/backends/linux.mjs");
  const b = mod.create({ exec: (await import("../src/remote-runtime.mjs")).exec });
  let p = null;
  try {
    p = await b.probe();
  } catch (e) {
    // No display session on this host: probe must fail closed with no_session.
    assert.equal(e.code, "no_session");
    return;
  }
  assert.equal(p.platform, "linux");
  assert.ok(["x11", "wayland"].includes(p.session));
  assert.equal(typeof p.missing.length, "number");
});

test("linux: probe rejects with no_session when no display session is visible", async () => {
  const saved = {};
  for (const k of ["DISPLAY", "WAYLAND_DISPLAY", "XDG_SESSION_TYPE"]) { saved[k] = process.env[k]; delete process.env[k]; }
  try {
    const mod = await import("../src/backends/linux.mjs");
    const b = mod.create({ exec: (await import("../src/remote-runtime.mjs")).exec });
    await assert.rejects(() => b.probe(), (e) => e.code === "no_session" && /DISPLAY/.test(e.message));
  } finally {
    for (const k of ["DISPLAY", "WAYLAND_DISPLAY", "XDG_SESSION_TYPE"]) { if (saved[k] !== undefined) process.env[k] = saved[k]; }
  }
});

test("linux: open_application hands focus back unless activate:true", async () => {
  const saved = {};
  for (const k of ["DISPLAY", "WAYLAND_DISPLAY", "XDG_SESSION_TYPE"]) { saved[k] = process.env[k]; }
  process.env.DISPLAY = "fixture";
  delete process.env.WAYLAND_DISPLAY;
  process.env.XDG_SESSION_TYPE = "x11";
  try {
    const mod = await import("../src/backends/linux.mjs");
    const cmds = [];
    const b = mod.create({ exec: {
      have: async () => true,
      run: async (cmd, args) => {
        cmds.push([cmd, ...args].join(" "));
        if (cmd === "xdotool" && args[0] === "getactivewindow") return { code: 0, stdout: "4242\n", stderr: "" };
        return { code: 0, stdout: "", stderr: "" };
      },
    } });
    // "true" is a real binary — the launch itself is harmless; the assertion is
    // on the focus bookkeeping around it.
    const bg = await b.open_application({ name: "true" });
    assert.equal(bg.activate, false);
    assert.equal(bg.focus_restored, true);
    assert.ok(cmds.includes("xdotool getactivewindow"));
    assert.ok(cmds.includes("xdotool windowactivate 4242"));
    cmds.length = 0;
    const fg = await b.open_application({ name: "true", activate: true });
    assert.equal(fg.activate, true);
    assert.ok(!cmds.some((c) => c.includes("windowactivate")), "activate:true must not touch focus bookkeeping");
  } finally {
    for (const k of ["DISPLAY", "WAYLAND_DISPLAY", "XDG_SESSION_TYPE"]) { if (saved[k] !== undefined) process.env[k] = saved[k]; else delete process.env[k]; }
  }
});

test("win32: module loads with the full backend surface", async () => {
  const mod = await import("../src/backends/win32.mjs");
  assert.equal(typeof mod.create, "function");
  if (process.platform !== "win32") {
    const b = mod.create();
    for (const m of ["probe", "screenshot", "left_click", "type", "key", "recordingStart", "get_app_state", "set_value"]) {
      assert.equal(typeof b[m], "function", `win32 backend missing ${m}`);
    }
  }
});

test("remote agent refuses tools outside the allow-list", async () => {
  const { run } = await import("../src/exec.mjs");
  const sentinel = path.join(fs.mkdtempSync(path.join(os.tmpdir(), "cu-agent-deny-")), "x");
  const payload = Buffer.from(JSON.stringify({ tool: "write_file", args: { path: sentinel } })).toString("base64");
  const r = await run("node", [fileURLToPath(new URL("../agent.mjs", import.meta.url)), payload]);
  const reply = JSON.parse(r.stdout.trim());
  assert.equal(reply.ok, false);
  assert.equal(reply.error.code, "tool_not_allowed");
  assert.ok(!fs.existsSync(sentinel));
});

test("remote agent answers the platform probe", async () => {
  const { run } = await import("../src/exec.mjs");
  const payload = Buffer.from(JSON.stringify({ tool: "platform" })).toString("base64");
  const r = await run("node", [fileURLToPath(new URL("../agent.mjs", import.meta.url)), payload]);
  const reply = JSON.parse(r.stdout.trim());
  assert.equal(reply.ok, true);
  assert.equal(reply.platform, process.platform);
});


test("linux: nested zooms crop only the latest backend-owned raster", async (t) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "cu-linux-crop-"));
  const saved = Object.fromEntries(["CODEWHALE_CU_RECORDINGS_DIR", "DISPLAY", "WAYLAND_DISPLAY", "XDG_SESSION_TYPE"].map(k => [k, process.env[k]]));
  process.env.CODEWHALE_CU_RECORDINGS_DIR = dir;
  process.env.DISPLAY = "fixture";
  delete process.env.WAYLAND_DISPLAY;
  process.env.XDG_SESSION_TYPE = "x11";
  t.after(() => {
    for (const [key, value] of Object.entries(saved)) value === undefined ? delete process.env[key] : process.env[key] = value;
    fs.rmSync(dir, { recursive: true, force: true });
  });
  const png = Buffer.alloc(24);
  Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]).copy(png);
  png.write("IHDR", 12); png.writeUInt32BE(100, 16); png.writeUInt32BE(80, 20);
  const commands = [];
  let cropFailure = null;
  const run = async (cmd, args) => {
    commands.push({ cmd, args });
    if (cmd === "ffmpeg" && cropFailure) return { code: 0, stdout: "", stderr: "", ...cropFailure };
    const output = args.at(-1);
    if (typeof output === "string" && output.startsWith(dir) && output.endsWith(".png")) fs.writeFileSync(output, png);
    return { code: 0, stdout: "", stderr: "" };
  };
  const { create } = await import("../src/backends/linux.mjs");
  const backend = create({ exec: { run, have: async () => true } });
  const shot = await backend.screenshot();
  for (const [failure, reason] of [
    [{ aborted: true }, /cancelled/],
    [{ timedOut: true }, /timeout/],
    [{ code: 1, stderr: "fixture crop failure" }, /fixture crop failure/],
  ]) {
    cropFailure = failure;
    await assert.rejects(backend.zoom({ region: [0, 0, 10, 10] }), reason);
  }
  cropFailure = null;
  const first = await backend.zoom({ region: [10, 20, 40, 30], source: "/untrusted/caller.png" });
  const child = await backend.zoom({ region: [2, 3, 10, 8], source: "/untrusted/caller.png" });
  const crops = commands.filter(call => call.cmd === "ffmpeg").slice(-2);
  assert.equal(crops[0].args[crops[0].args.indexOf("-i") + 1], shot.file);
  assert.equal(crops[1].args[crops[1].args.indexOf("-i") + 1], first.file);
  assert.deepEqual(child.points, { x: 12, y: 23, w: 10, h: 8 });
  assert.deepEqual(child.pixels, { w: 10, h: 8 });
});
