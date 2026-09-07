// macOS backend. Zero third-party dependencies:
//  - observation and input: native Accessibility and CoreGraphics APIs
//  - stills:      /usr/sbin/screencapture
//  - video:       ScreenCaptureKit in the signed helper (macOS 13+, no overlay)
//  - crop:         sips   - clipboard: pbcopy/pbpaste
// Helper requests travel as one JSON argument without shell interpolation.
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import crypto from "node:crypto";
import { fileURLToPath } from "node:url";
import { spawn } from "node:child_process";
import { run, runOk, ExecError, tryJson, have } from "../exec.mjs";

const KEY_CODES = {
  return: 36, enter: 36, tab: 48, space: 49, escape: 53, esc: 53, delete: 51,
  backspace: 51, forwarddelete: 117, home: 115, end: 119, pageup: 116, pagedown: 121,
  left: 123, right: 124, down: 125, up: 126, clear: 71, capslock: 57, f1: 122,
  f2: 120, f3: 99, f4: 118, f5: 96, f6: 97, f7: 98, f8: 100, f9: 101, f10: 109,
  f11: 103, f12: 111, volumeup: 72, volumedown: 73, mute: 74, help: 114,
  a: 0, s: 1, d: 2, f: 3, h: 4, g: 5, z: 6, x: 7, c: 8, v: 9, b: 11, q: 12,
  w: 13, e: 14, r: 15, y: 16, t: 17, "1": 18, "2": 19, "3": 20, "4": 21,
  "5": 23, "6": 22, "7": 26, "8": 28, "9": 25, "0": 29, "-": 27, "=": 24,
  "[": 33, "]": 30, "\\": 42, ";": 41, "'": 39, ",": 43, ".": 47, "/": 44,
  o: 31, u: 32, i: 34, p: 35, l: 37, j: 38, k: 40, n: 45, m: 46,
};
const MODIFIERS = {
  cmd: 1 << 20, command: 1 << 20, win: 1 << 20, meta: 1 << 20,
  shift: 1 << 17, ctrl: 1 << 18, control: 1 << 18, alt: 1 << 19, opt: 1 << 19, option: 1 << 19,
  fn: 1 << 23, function: 1 << 23,
};
// CGEventType values (CGEventTypes.h). The dragged codes are easy to get
// wrong: 6 is LeftMouseDragged and 7 is RightMouseDragged, so a left drag sent
// as 7 is delivered as a right-button drag and no view ever sees it.
const MOUSE = {
  left: { down: 1, up: 2, dragged: 6 },
  right: { down: 3, up: 4, dragged: 7 },
  middle: { down: 25, up: 26, dragged: 27 },
};
const MOUSE_MOVED = 5;

export function create({ exec }) {
  const runL = (cmd, args, opts) => exec.run(cmd, args, opts);
  const state = { activeDisplay: 1, lastRaster: null, inputApp: null, previewEnabled: false, pointer: null };

  async function nativeHelper() {
    let helper = process.env.CODEWHALE_CU_APP_BUNDLE
      ? path.join(process.env.CODEWHALE_CU_APP_BUNDLE, "Contents", "MacOS", "accessibility") : null;
    if (!helper || !fs.existsSync(helper)) {
      const source = fileURLToPath(new URL("./darwin-accessibility.m", import.meta.url));
      const hash = crypto.createHash("sha256").update(fs.readFileSync(source)).update(fs.readFileSync(new URL("./darwin-recording.h", import.meta.url))).digest("hex").slice(0, 16);
      const dir = path.join(os.homedir(), ".codewhale-cu", "bin");
      fs.mkdirSync(dir, { recursive: true, mode: 0o700 });
      helper = path.join(dir, `accessibility-${hash}`);
      if (!fs.existsSync(helper)) {
        const tmp = `${helper}-${process.pid}`;
        const r = await runL("clang", ["-fobjc-arc", "-Os", "-framework", "Cocoa", "-framework", "ApplicationServices", "-framework", "ScreenCaptureKit", "-framework", "AVFoundation", "-framework", "CoreMedia", source, "-o", tmp], { timeoutMs: 60_000 });
        if (r.code !== 0) throw new ExecError(`native accessibility helper needs a built app or Xcode Command Line Tools: ${r.stderr}`, r);
        fs.renameSync(tmp, helper);
      }
    }
    return helper;
  }

  async function native(tool, args = {}) {
    const helper = await nativeHelper();
    const r = await runL(helper, [JSON.stringify({ tool, args: { ...args, input_app_ref: state.inputApp } })], { timeoutMs: 20_000 });
    if (r.code !== 0) throw new ExecError(r.stderr.trim() || "native accessibility helper failed", r);
    const result = tryJson(r.stdout, null);
    if (state.previewEnabled && ["type", "key_event", "pointer_sequence", "set_value", "select_text", "perform_action", "hit_test"].includes(tool)) {
      try { await updatePreview(); } catch (error) { result.preview_error = error.message; }
    }
    return result;
  }

  async function updatePreview(show = false) {
    const win = await native("window_info", { app_ref: state.inputApp });
    const dir = path.join(os.homedir(), ".codewhale-cu", "preview");
    fs.mkdirSync(dir, { recursive: true, mode: 0o700 });
    const temp = path.join(dir, "next.png"), file = path.join(dir, "latest.png");
    const r = await runL("screencapture", ["-x", "-o", "-l", String(win.window_id), "-t", "png", temp], { timeoutMs: 8000 });
    if (r.code !== 0) throw new ExecError(`background preview capture failed: ${r.stderr}`);
    fs.renameSync(temp, file);
    const p = state.pointer;
    await native("preview_notify", { enabled: true, show, title: `Codewhale · ${win.name}`, x: p ? (p.x-win.points.x)/win.points.w : -1, y: p ? (p.y-win.points.y)/win.points.h : -1 });
    return { enabled: true, file, app: state.inputApp, pointer: p };
  }

  // ---------- pointer input ----------
  // macOS delivers keyboard events to a chosen process, but not pointer or
  // scroll events: those are dropped unless they go through the shared event
  // tap, which moves the user's real cursor. So the pointer path is:
  //   1. accessibility action on the element under the point (quiet, exact),
  //   2. otherwise a global gesture that is refused unless the bound
  //      application owns the window under the point, and that puts the
  //      cursor back where it was.
  // Every receipt says which of the two happened.
  function mouseName(button) { return { left: "left", right: "right", middle: "middle" }[button] ?? "left"; }

  function assertInScreen(x, y) {
    if (!Number.isFinite(x) || !Number.isFinite(y)) throw new ExecError("coordinates must be finite numbers");
  }

  function buttonCode(button) { return button === "middle" ? 2 : button === "right" ? 1 : 0; }

  /** Refuse a global gesture whose landing point belongs to another application. */
  async function assertOwnsPoint(x, y) {
    if (!state.inputApp) throw new ExecError("open_application first to choose which application receives input");
    const w = await native("window_at_point", { x, y });
    if (!w?.found) throw new ExecError(`no window at (${x}, ${y}) — take a fresh screenshot and choose a point inside the target window`);
    if (w.owner_pid !== state.inputApp.pid) {
      throw new ExecError(`(${x}, ${y}) is covered by a window owned by ${w.owner_name || "another application"} (pid ${w.owner_pid}), not the application input is bound to — raise the window you meant with open_application(activate:true), observe again, or use an element target`);
    }
    return w;
  }

  /** What a global gesture cost the user: their cursor, and briefly their foreground. */
  function pointerCost(r) {
    return {
      pointer_moved: true,
      pointer_restored: !!r?.restored,
      foreground_taken: !!r?.foreground_taken,
      ...(r?.foreground_before ? { foreground_before: r.foreground_before } : {}),
      ...(r?.foreground_after ? { foreground_after: r.foreground_after } : {}),
    };
  }

  async function gesture(steps, { restore = true, guard = null } = {}) {
    if (guard) await assertOwnsPoint(guard.x, guard.y);
    const r = await native("pointer_sequence", { steps, restore });
    const last = [...steps].reverse().find((s) => s.x != null);
    if (last) state.pointer = { x: last.x, y: last.y };
    return r;
  }

  function clickSteps(button, x, y, clicks) {
    const m = MOUSE[button] ?? MOUSE.left;
    const b = buttonCode(button);
    const steps = [{ type: MOUSE_MOVED, x, y, button: b, clickState: 0 }];
    for (let i = 1; i <= clicks; i++) {
      steps.push({ type: m.down, x, y, button: b, clickState: i });
      steps.push({ type: m.up, x, y, button: b, clickState: i });
    }
    return steps;
  }

  /**
   * Coordinate pointer click. A left single click is first hit-tested against
   * the bound application's accessibility tree: when the point names a
   * pressable element we perform its semantic action, which needs no pointer
   * and no foreground. strategy="a11y" requires that and fails closed;
   * strategy="event" goes straight to the guarded global gesture.
   */
  async function pointerClick(button, x, y, clicks, strategy = "auto") {
    assertInScreen(x, y);
    if (!["auto", "a11y", "event"].includes(strategy)) throw new ExecError(`strategy must be auto, a11y or event (got ${JSON.stringify(strategy)})`);
    let a11yReason = null;
    if (strategy !== "event" && button === "left" && clicks === 1) {
      const hit = await native("hit_test", { x, y, perform: true });
      if (hit?.action_sent) {
        return { action_sent: true, strategy: "a11y", action: hit.action, pointer_moved: false, at: { x, y }, button, clicks,
                 element: { role: hit.element?.role ?? null, label: hit.element?.label ?? null } };
      }
      a11yReason = hit?.reason ?? "not_found";
      if (strategy === "a11y") {
        throw new ExecError(`no pressable accessibility element at (${x}, ${y}) in the bound application (${a11yReason}) — observe again or use strategy "event"`);
      }
    } else if (strategy === "a11y") {
      throw new ExecError(`strategy "a11y" is only available for a left single click on this backend; ${mouseName(button)} x${clicks} has no accessibility equivalent`);
    }
    const r = await gesture(clickSteps(button, x, y, clicks), { restore: true, guard: { x, y } });
    return { action_sent: true, strategy: "event", at: { x, y }, button, clicks, ...pointerCost(r),
             ...(a11yReason ? { a11y_reason: a11yReason } : {}) };
  }

  async function keyEvent(code, flags, down) { return native("key_event", { code, flags, down }); }

  function parseChord(text) {
    const parts = String(text).split("+").map((s) => s.trim().toLowerCase()).filter(Boolean);
    if (!parts.length) throw new ExecError("empty key text");
    let flags = 0;
    let key = null;
    for (const p of parts) {
      if (MODIFIERS[p] != null) flags |= MODIFIERS[p];
      else if (KEY_CODES[p] != null) { if (key) throw new ExecError(`multiple non-modifier keys in "${text}"`); key = p; }
      else throw new ExecError(`unknown key "${p}" (supported: ${Object.keys(KEY_CODES).join(", ")} + modifiers cmd/ctrl/alt/shift/fn)`);
    }
    if (key == null) throw new ExecError(`no non-modifier key in "${text}" — use hold_key for modifier-only holds`);
    return { flags, code: KEY_CODES[key], key };
  }

  // ---------- displays ----------
  async function displayInfo() { return native("displays"); }

  // ---------- screenshots ----------
  function recordingsDir() {
    return process.env.CODEWHALE_CU_RECORDINGS_DIR || path.join(os.homedir(), ".codewhale-cu", "recordings");
  }

  async function screenshot({ display, region, app_ref, path: outPath } = {}) {
    const dir = recordingsDir();
    fs.mkdirSync(dir, { recursive: true });
    const file = outPath || path.join(dir, `shot-${new Date().toISOString().replace(/[:.]/g, "-")}-${crypto.randomBytes(3).toString("hex")}.png`);
    if (!/\.png$/.test(file)) throw new ExecError("screenshot path must end in .png");
    const args = ["-x", "-t", "png"];
    const disp = display ?? state.activeDisplay;
    const window = app_ref ? await native("window_info", { app_ref }) : null;
    if (window && region) throw new ExecError("choose app_ref or region, not both");
    if (window) args.push("-o", "-l", String(window.window_id));
    else if (disp && disp !== "all") args.push("-D", String(disp));
    if (region) {
      if (!region.every((n) => Number.isFinite(n) && n >= 0) || region.length !== 4) {
        throw new ExecError("region must be [x, y, w, h] in screen points");
      }
      args.push("-R", region.join(","));
    }
    args.push(file);
    const r = await runL("screencapture", args, { timeoutMs: 20_000 });
    if (r.code !== 0) throw new ExecError(`screencapture exited ${r.code}: ${r.stderr.trim().slice(0, 300)}`, r);
    const stat = fs.statSync(file);
    const displays = await displayInfo();
    const d = displays.find((x) => x.index === (disp === "all" ? 1 : disp)) ?? displays[0];
    const scale = d?.scale ?? 1;
    state.lastRaster = {
      file,
      bytes: stat.size,
      display: disp ?? 1,
      // Region and window rasters describe that rect, not the whole display.
      // The PNG header is the pixel ground truth; scale is derived from
      // pixels/points below so Retina and mixed-DPI stay exact.
      points: window?.points ?? (region ? { x: region[0], y: region[1], w: region[2], h: region[3] } : d?.points ?? null),
      pixels: (() => { const header = fs.readFileSync(file); return { w: header.readUInt32BE(16), h: header.readUInt32BE(20) }; })(),
      scale: d?.scale ?? 1,
      capturedAt: new Date().toISOString(),
    };
    if (state.lastRaster.points?.w) state.lastRaster.scale = state.lastRaster.pixels.w / state.lastRaster.points.w;
    return { ...state.lastRaster, path: file };
  }

  async function zoom({ source, region, path: outPath }) {
    if (!source && !state.lastRaster) throw new ExecError("no screenshot taken yet on this computer — call screenshot first");
    const [x, y, w, h] = region;
    if (![x, y, w, h].every((n) => Number.isInteger(n) && n >= 0) || !w || !h || x + w > state.lastRaster.pixels.w || y + h > state.lastRaster.pixels.h) throw new ExecError("region must be [x, y, w, h] in last-raster pixels");
    const src = source ?? state.lastRaster.file;
    const dir = recordingsDir();
    fs.mkdirSync(dir, { recursive: true });
    const out = outPath || path.join(dir, `zoom-${crypto.randomBytes(4).toString("hex")}.png`);
    await runOk("sips", ["-s", "format", "png", "-c", String(Math.round(h)), String(Math.round(w)), "--cropOffset", String(Math.round(y)), String(Math.round(x)), src, "--out", out], { timeoutMs: 15_000 });
    const parent = state.lastRaster;
    state.lastRaster = { file: out, bytes: fs.statSync(out).size, source: src, region,
      points: { x: (parent.points?.x ?? 0) + x / parent.scale, y: (parent.points?.y ?? 0) + y / parent.scale, w: w / parent.scale, h: h / parent.scale },
      pixels: { w, h }, scale: parent.scale, capturedAt: new Date().toISOString() };
    return state.lastRaster;
  }

  // ---------- recording ----------
  const rec = new Map(); // id -> {pid, file, startedAt, mode}

  async function recordingStart({ display, durationSec, region } = {}) {
    const dir = recordingsDir();
    fs.mkdirSync(dir, { recursive: true });
    const id = crypto.randomBytes(4).toString("hex");
    const file = path.join(dir, `rec-${id}.mov`);
    const displays=await displayInfo();
    const disp=display ?? state.activeDisplay;
    const selected=displays.find(d=>d.index===disp);
    if(!selected) throw new ExecError("choose one available display for recording");
    if(durationSec!=null && (!Number.isFinite(durationSec) || durationSec<=0)) throw new ExecError("durationSec must be positive");
    const helper=await nativeHelper();
    const child=spawn(helper,[JSON.stringify({tool:"record",args:{file,displayID:selected.id,region,durationSec}})],{stdio:["ignore","pipe","pipe"]});
    const startedAt=new Date().toISOString();
    let stderr="", output="", ready=false;
    const completion=new Promise(resolve=>{
      child.once("error",error=>resolve({code:-1,error:error.message}));
      child.once("close",code=>resolve({code,error:stderr.trim()}));
    });
    child.stderr.on("data",chunk=>{stderr=(stderr+chunk).slice(-4000);});
    const readyPromise=new Promise((resolve,reject)=>{
      const timer=setTimeout(()=>{child.kill("SIGTERM");reject(new ExecError("screen recorder startup timed out"));},20000);
      child.stdout.on("data",chunk=>{
        output+=chunk;
        let i;
        while((i=output.indexOf("\n"))>=0){
          const line=output.slice(0,i);output=output.slice(i+1);
          try { if(JSON.parse(line).ready){ready=true;clearTimeout(timer);resolve();} } catch {}
        }
      });
      completion.then(result=>{clearTimeout(timer);if(!ready)reject(new ExecError(result.error || "screen recorder exited before capture started"));});
    });
    await readyPromise;
    rec.set(id,{child,completion,pid:child.pid,file,startedAt,mode:"ScreenCaptureKit",display:disp});
    return {id,pid:child.pid,file,display:disp,durationSec:durationSec??null,region:region??null,fps:30,mode:"ScreenCaptureKit",startedAt};
  }

  async function recordingStop({ id }) {
    const r=rec.get(id);
    if(!r) throw new ExecError(`unknown or already-finished recording "${id}"`);
    if(r.child.exitCode==null && r.child.signalCode==null) r.child.kill("SIGINT");
    let timer;
    const result=await Promise.race([r.completion,new Promise(resolve=>{timer=setTimeout(()=>resolve({code:-1,error:"screen recorder finalization timed out; recording retained for retry"}),20000);})]);
    clearTimeout(timer);
    if(result.code!==0) throw new ExecError(result.error || "screen recorder failed; partial file retained");
    const size=fs.existsSync(r.file)?fs.statSync(r.file).size:0;
    if(!size) throw new ExecError("screen recorder produced no video");
    rec.delete(id);
    return {id,file:r.file,mp4:null,bytes:size,mode:r.mode,startedAt:r.startedAt,stoppedAt:new Date().toISOString()};
  }

  async function recordingStatus({ id }) {
    const r = rec.get(id);
    if (!r) return { id, running: false };
    const alive = r.child.exitCode == null && r.child.signalCode == null;
    return { id, running: alive, pid: r.pid, file: r.file, bytes: fs.existsSync(r.file) ? fs.statSync(r.file).size : 0, startedAt: r.startedAt };
  }

  async function recordingList() {
    const dir = recordingsDir();
    const out = [];
    for (const f of fs.existsSync(dir) ? fs.readdirSync(dir) : []) {
      const full = path.join(dir, f);
      const st = fs.statSync(full);
      if (st.isFile() && /\.(mov|mp4|png)$/i.test(f)) out.push({ file: full, bytes: st.size, modifiedAt: st.mtime.toISOString() });
    }
    out.sort((a, b) => b.modifiedAt.localeCompare(a.modifiedAt));
    return { dir, recordings: out.slice(0, 50), running: [...rec.keys()] };
  }

  // ---------- apps / windows ----------
  async function listApps() { return native("list_apps"); }

  async function listWindows(appRef) { return native("list_windows", { app_ref: appRef }); }

  async function openApplication({ name, bundle_id: bid, pid, url: urlArg, activate = false } = {}) {
    if (!name && !bid && !pid) throw new ExecError("open_application needs name, bundle_id or pid");
    // pid is the most specific identity and the only one that separates two
    // processes of the same bundle (e.g. a second Chrome on its own profile),
    // so it wins when given.
    const find = {};
    if (pid) find.pid = pid; else if (bid) find.bundle_id = bid; else find.name = String(name).replace(/\.app$/, "");
    let p;
    // Binding an already-running app must not ask LaunchServices to reopen
    // it: reopen can raise windows even with open -g on some applications.
    if (!urlArg) {
      try { p = await native("app_info", { app_ref: find, activate }); }
      catch (error) {
        if (!error.message.includes("application not found")) throw error;
      }
    }
    if (!p?.found) {
      if (!name && !bid) throw new ExecError(`no running application with pid ${pid}; call list_apps for the current processes`);
      const args = [];
      if (urlArg) args.push(urlArg);
      if (bid) args.unshift("-b", bid); else args.unshift("-a", name);
      if (!activate) args.unshift("-g");
      const r = await runL("open", args, { timeoutMs: 25_000 });
      if (r.code !== 0) throw new ExecError(`open failed: ${r.stderr.trim().slice(0, 200)}`, r);
      await new Promise((res) => setTimeout(res, 600));
      p = await native("app_info", { app_ref: find, activate });
    }
    // A bare executable has no bundle id; carrying an empty one would make the
    // identity unmatchable.
    state.inputApp = { pid: p.pid, ...(p.bundle_id ? { bundle_id: p.bundle_id } : {}) };
    return { launched: true, activate, url: urlArg ?? null, resolved: p?.found ? { name: p.name, pid: p.pid, bundle_id: p.bundle_id, frontmost: p.frontmost } : null };
  }

  // ---------- clipboard / cursor / waits ----------
  async function readClipboard() {
    const r = await runL("pbpaste", [], { timeoutMs: 5_000, maxBuffer: 4 * 1024 * 1024 });
    return { text: r.stdout, encoding: "utf8" };
  }
  async function writeClipboard({ text }) {
    const child = spawn("pbcopy", [], { stdio: ["pipe", "ignore", "ignore"] });
    child.stdin.end(String(text ?? ""));
    await new Promise((res, rej) => { child.on("close", res); child.on("error", rej); });
    return { written: String(text ?? "").length };
  }
  async function cursorPosition() { return native("cursor_position"); }

  // ---------- probe ----------
  async function probe() {
    const caps = { screenshot: true, recording: true, accessibility_tree: true, clipboard: true, displays: true };
    const perms = {};
    try {
      const ax = await native("permissions");
      perms.accessibility = ax.trusted ? "granted" : "denied";
    } catch { perms.accessibility = "denied_or_unavailable"; }
    caps.accessibility_tree = perms.accessibility === "granted";
    caps.raw_input = caps.accessibility_tree;
    try {
      const t = os.tmpdir() + `/cu-probe-${crypto.randomBytes(3).toString("hex")}.png`;
      const r = await runL("screencapture", ["-x", "-R0,0,2,2", "-t", "png", t], { timeoutMs: 8_000 });
      perms.screen_capture = r.code === 0 ? "ok" : "failed";
      try { fs.rmSync(t, { force: true }); } catch {}
    } catch { perms.screen_capture = "failed"; }
    caps.screenshot = perms.screen_capture === "ok";
    caps.recording = caps.screenshot;
    return { platform: "darwin", capabilities: caps, permissions: perms, note: "macOS does not expose Screen-Recording TCC state to CLI; a black/empty screenshot means Screen Recording permission is missing. Raw input is bound to the process selected by open_application (activate:false by default). It does not require bringing that app forward. App-specific focus behavior still requires verification." };
  }

  return {
    platform: "darwin",
    probe,
    list_displays: displayInfo,
    async switch_display({ index }) {
      const ds = await displayInfo();
      if (!ds.some((d) => d.index === index)) throw new ExecError(`no display ${index}; have [${ds.map((d) => d.index).join(", ")}]`);
      state.activeDisplay = index;
      return { activeDisplay: index };
    },
    list_apps: listApps,
    list_windows: listWindows,
    open_application: openApplication,
    get_app_state: async ({ app_ref, detail, depth, window_id }) => {
      const t = await native("get_app_state", { app_ref, detail, window_id });
      if (!t.found) throw new ExecError("application not found — call list_apps for exact names/pids");
      return t;
    },
    resolve_element: async ({ app_ref, windowIndex, path: pathArr } = {}) => {
      const r = await native("resolve_element", { app_ref: app_ref ?? {}, windowIndex: windowIndex ?? 0, path: pathArr ?? [] });
      return { found: !!r?.found, element: r?.element ?? null, reason: r?.reason ?? null };
    },
    preview: async ({ enabled = true } = {}) => {
      state.previewEnabled = enabled;
      if (!enabled) { await native("preview_notify", { enabled: false }); return { enabled: false }; }
      if (!state.inputApp) throw new ExecError("open_application first to choose the preview app");
      return updatePreview(true);
    },
    screenshot,
    zoom,
    left_click: ({ target, strategy }) => pointerClick("left", target.x, target.y, 1, strategy ?? "auto"),
    double_click: ({ target }) => pointerClick("left", target.x, target.y, 2),
    triple_click: ({ target }) => pointerClick("left", target.x, target.y, 3),
    right_click: ({ target }) => pointerClick("right", target.x, target.y, 1),
    middle_click: ({ target }) => pointerClick("middle", target.x, target.y, 1),
    mouse_move: async ({ target }) => {
      assertInScreen(target.x, target.y);
      // A hover has to leave the pointer where it was asked to go.
      const r = await gesture([{ type: MOUSE_MOVED, x: target.x, y: target.y, button: 0, clickState: 0 }], { restore: false, guard: target });
      return { action_sent: true, strategy: "event", at: { x: target.x, y: target.y }, ...pointerCost(r) };
    },
    left_mouse_down: async ({ target }) => {
      assertInScreen(target.x, target.y);
      const r = await gesture([
        { type: MOUSE_MOVED, x: target.x, y: target.y, button: 0, clickState: 0 },
        { type: MOUSE.left.down, x: target.x, y: target.y, button: 0, clickState: 1 },
      ], { restore: false, guard: target });
      return { action_sent: true, strategy: "event", at: { x: target.x, y: target.y }, ...pointerCost(r) };
    },
    left_mouse_up: async ({ target }) => {
      const loc = target ?? state.pointer;
      if (!loc) throw new ExecError("no agent pointer position — mouse_move or left_mouse_down first");
      assertInScreen(loc.x, loc.y);
      // No ownership guard: the button is already held, and the drag may have
      // legitimately left the originating window.
      const r = await gesture([{ type: MOUSE.left.up, x: loc.x, y: loc.y, button: 0, clickState: 1 }], { restore: false });
      return { action_sent: true, strategy: "event", at: { x: loc.x, y: loc.y }, ...pointerCost(r) };
    },
    left_click_drag: async ({ from_target: from, to }) => {
      assertInScreen(from.x, from.y); assertInScreen(to.x, to.y);
      const steps = [
        { type: MOUSE_MOVED, x: from.x, y: from.y, button: 0, clickState: 0 },
        { type: MOUSE.left.down, x: from.x, y: from.y, button: 0, clickState: 1, delayMs: 60 },
      ];
      const n = 12;
      for (let i = 1; i <= n; i++) {
        steps.push({ type: MOUSE.left.dragged, x: from.x + ((to.x - from.x) * i) / n, y: from.y + ((to.y - from.y) * i) / n, button: 0, clickState: 1, delayMs: 45 });
      }
      steps.push({ type: MOUSE.left.up, x: to.x, y: to.y, button: 0, clickState: 1, delayMs: 80 });
      const r = await gesture(steps, { restore: true, guard: from });
      return { action_sent: true, strategy: "event", from, to, ...pointerCost(r) };
    },
    scroll: async ({ target, direction = "down", amount = 5 }) => {
      assertInScreen(target.x, target.y);
      const dx = direction === "left" ? -amount : direction === "right" ? amount : 0;
      const dy = direction === "up" ? amount : direction === "down" ? -amount : 0;
      // A wheel sends one notch at a time. One event carrying the whole amount
      // is clamped by the scroll view's momentum handling and moves a fraction
      // of the distance, so emit the notches.
      const notches = Math.max(1, Math.min(100, Math.round(amount)));
      const steps = [{ type: MOUSE_MOVED, x: target.x, y: target.y, button: 0, clickState: 0, delayMs: 40 }];
      for (let i = 0; i < notches; i++) {
        steps.push({ scroll: [Math.sign(dx), Math.sign(dy)], delayMs: 15 });
      }
      const r = await gesture(steps, { restore: true, guard: target });
      return { action_sent: true, strategy: "event", direction, amount, ...pointerCost(r) };
    },
    type: (args) => native("type", args),
    key: async ({ text, repeat = 1 }) => {
      const { flags, code, key } = parseChord(text);
      for (let i = 0; i < Math.max(1, Math.min(100, repeat)); i++) {
        await keyEvent(code, flags, true);
        await keyEvent(code, flags, false);
        if (i < repeat - 1) await new Promise((r) => setTimeout(r, 30));
      }
      return { action_sent: true, key, code, repeat: Math.max(1, Math.min(100, repeat)) };
    },
    hold_key: async ({ text, duration }) => {
      const { flags, code, key } = parseChord(text);
      const d = Math.max(0.05, Math.min(30, Number(duration) || 1));
      await keyEvent(code, flags, true);
      await new Promise((r) => setTimeout(r, d * 1000));
      await keyEvent(code, flags, false);
      return { action_sent: true, key, heldSec: d };
    },
    set_value: (args) => native("set_value", args),
    select_text: (args) => native("select_text", args),
    perform_action: (args) => native("perform_action", args),
    read_clipboard: readClipboard,
    write_clipboard: writeClipboard,
    cursor_position: cursorPosition,
    recordingStart,
    recordingStop,
    recordingStatus,
    recordingList,
  };
}

export default { create };
