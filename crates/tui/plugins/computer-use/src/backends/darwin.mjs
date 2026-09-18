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
import { run, runOk, ExecError, tryJson, have, withSignal, wait, throwIfAborted, currentSignal } from "../exec.mjs";
import { stateDir } from "../registry.mjs";
import { createBrowser } from "../browser-cdp.mjs";

/** Base64 expands 3 bytes to 4, padded to a multiple of 4. */
const encodedSize = (bytes) => Math.ceil(bytes / 3) * 4;

/**
 * Largest base64 payload a single JSON-RPC message may carry. Stdio hosts cap
 * what a server may write between message boundaries (Claude Code disconnects
 * at 16MB) and model image APIs cap well below that. Mirrors
 * CODEWHALE_CU_MAX_IMAGE_BYTES in mcp/server.mjs, which keeps the hard guard.
 */
const rasterByteBudget = () => (Number(process.env.CODEWHALE_CU_MAX_IMAGE_BYTES) > 0
  ? Number(process.env.CODEWHALE_CU_MAX_IMAGE_BYTES)
  : 5_000_000);

/**
 * Pixel dimensions from a PNG IHDR or a JPEG frame header, reading only the
 * bytes that carry them rather than the whole raster.
 */
function imagePixels(file) {
  const fd = fs.openSync(file, "r");
  try {
    const head = Buffer.alloc(24);
    fs.readSync(fd, head, 0, 24, 0);
    if (head[0] === 0x89 && head.toString("ascii", 1, 4) === "PNG") {
      return { w: head.readUInt32BE(16), h: head.readUInt32BE(20) };
    }
    if (head[0] !== 0xff || head[1] !== 0xd8) throw new ExecError(`unrecognized raster format: ${file}`);
    // Walk JPEG segments to the frame header. SOF0/1/2/3/5..7/9..11/13..15
    // carry the dimensions; DHT/DQT and the rest are skipped by their length.
    const size = fs.fstatSync(fd).size;
    const seg = Buffer.alloc(9);
    for (let at = 2; at + 9 <= size; ) {
      fs.readSync(fd, seg, 0, 9, at);
      if (seg[0] !== 0xff) throw new ExecError(`malformed JPEG at byte ${at}: ${file}`);
      const marker = seg[1];
      if (marker >= 0xc0 && marker <= 0xcf && marker !== 0xc4 && marker !== 0xc8 && marker !== 0xcc) {
        return { w: seg.readUInt16BE(7), h: seg.readUInt16BE(5) };
      }
      if (marker === 0xd8 || (marker >= 0xd0 && marker <= 0xd9)) { at += 2; continue; }
      at += 2 + seg.readUInt16BE(2);
    }
    throw new ExecError(`JPEG carries no frame header: ${file}`);
  } finally {
    fs.closeSync(fd);
  }
}

/**
 * Shrink a raster until it fits the single-message budget.
 *
 * A 5K display captures to ~22MB of PNG, which is ~29MB of base64 — past every
 * host limit, so the alternative is handing back a receipt with no picture and
 * a screenshot tool that never shows anything. Downscaling here, before the
 * caller reads the PNG header, keeps coordinates exact by construction:
 * `pixels` comes from the header, `points` stays in screen points, and `scale`
 * is derived from the two, so raster-to-point conversion follows automatically.
 *
 * PNG bytes track pixel count, so the long edge shrinks by the square root of
 * the overshoot. The estimate is verified rather than trusted — screen content
 * compresses unevenly — and gives up rather than shrinking past legibility.
 */
async function fitRasterToBudget(file) {
  for (let attempt = 0; attempt < 4; attempt += 1) {
    const budget = rasterByteBudget();
    const size = fs.statSync(file).size;
    if (encodedSize(size) <= budget) return;
    const { w, h } = imagePixels(file);
    const longest = Math.max(w, h);
    if (longest <= 640) return;
    const overshoot = encodedSize(size) / budget;
    const target = Math.max(640, Math.floor((longest / Math.sqrt(overshoot)) * 0.9));
    if (target >= longest) return;
    await runOk("sips", ["-Z", String(target), file], { timeoutMs: 20_000 });
  }
}

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

/**
 * Native refusals carry exception reasons; these map to stable codes so
 * receipts and callers can branch without parsing prose. Unknown reasons stay
 * uncoded (the message is the contract there).
 */
export function nativeErrorCode(message) {
  const m = String(message ?? "");
  if (/ambiguous/i.test(m)) return "window_ambiguous";
  if (/not capturable/i.test(m)) return "window_not_capturable";
  if (/several running applications match/i.test(m)) return "ambiguous_application";
  if (/cannot be terminated by this plugin/i.test(m)) return "protected_application";
  if (/refused the window frame change/i.test(m)) return "frame_refused";
  if (/no accessibility geometry/i.test(m)) return "window_target_not_found";
  if (/application not found|no running application/i.test(m)) return "app_not_found";
  return null;
}

/**
 * Choose the menu element for an exact title: a menu bar item at level 0, an
 * open menu's item below it. Exact match only — a fuzzy match would activate
 * the wrong command, and menu titles are stable enough to state precisely.
 * Exported for tests; the walk itself is native.
 */
export function pickMenuElement(elements, label, menuBar) {
  const role = menuBar ? "AXMenuBarItem" : "AXMenuItem";
  return elements.find((el) => el?.label === label && el?.role === role) ?? null;
}

/**
 * Regular apps are what "open an app" means; accessories and daemons answer
 * menu-bar and background questions. Keep the signal, drop the XPC soup.
 * A helper that predates the activation_policy field returns the list whole.
 */
export function selectApps(apps, all) {
  if (all || !apps.some((a) => a.activation_policy)) return apps;
  return apps.filter((a) => a.activation_policy === "regular" || a.frontmost === true);
}

/**
 * Front-lease interference verdict (SHA-6643 slice 1). The native helper
 * reports the borrow window (lease_ms) and the HID idle clock around it
 * (idle_before_s/idle_after_s); synthesized events do not tick that clock,
 * so a clock that fails to advance across the window means hardware input
 * arrived mid-lease. Null when the reply carries no accounting (no borrow,
 * or a helper that predates it) — receipts stay quiet then.
 */
const LEASE_IDLE_EPSILON_S = 0.25;
export function leaseVerdict(r) {
  if (r?.front_lease !== true) return null;
  const leaseMs = r.lease_ms, before = r.idle_before_s, after = r.idle_after_s;
  if (![leaseMs, before, after].every(Number.isFinite)) return null;
  return after < before + leaseMs / 1000 - LEASE_IDLE_EPSILON_S;
}

/**
 * Threads the interference accounting from a native lease reply into a
 * model-facing receipt. Call sites that build receipts field-by-field
 * spread this; verbatim flows (type) already carry it via native().
 */
export function leaseAccounting(r) {
  if (r?.front_lease !== true) return {};
  const out = {};
  for (const k of ["lease_ms", "idle_before_s", "idle_after_s"]) {
    if (Number.isFinite(r[k])) out[k] = r[k];
  }
  if (typeof r.user_input_during_lease === "boolean") out.user_input_during_lease = r.user_input_during_lease;
  return out;
}

export function create({ exec }) {
  const runL = (cmd, args, opts) => exec.run(cmd, args, opts);
  // The preview panel is on by default: while a session is bound to an app,
  // every action updates the floating capture and its drawn cursor so the
  // person can watch without the real pointer moving. `preview(enabled:false)`
  // mutes it for the session.
  // The preview panel is live while a session is bound: after the first
  // successful capture a timer keeps refreshing it, so the person watches the
  // app instead of a frozen still. CODEWHALE_CU_PREVIEW_REFRESH_MS=0 disables
  // the loop (tests, headless); the floor keeps a hostile value tolerable.
  const state = { activeDisplay: 1, lastRaster: null, inputApp: null, foregroundInput: false, previewEnabled: true, pointer: null, pointerLease: null };
  let previewLoop = null;
  let previewBusy = false;
  function stopPreviewLoop() { if (previewLoop) { clearInterval(previewLoop); previewLoop = null; } }
  // A hide must not race an in-flight capture: its late preview_notify would
  // re-show a panel that was just dismissed.
  async function quiescePreview() { for (let i = 0; i < 20 && previewBusy; i++) await wait(25); }
  const browser = createBrowser();
  function startPreviewLoop() {
    if (previewLoop) return;
    const ms = Number(process.env.CODEWHALE_CU_PREVIEW_REFRESH_MS ?? 1000);
    if (!Number.isFinite(ms) || ms <= 0) return;
    previewLoop = setInterval(() => {
      if (previewBusy || !state.previewEnabled || !state.inputApp) return;
      previewBusy = true;
      updatePreview(false).catch(() => {}).finally(() => { previewBusy = false; });
    }, Math.max(50, ms));
    previewLoop.unref?.();
  }

  async function nativeHelper() {
    let helper = process.env.CODEWHALE_CU_APP_BUNDLE
      ? path.join(process.env.CODEWHALE_CU_APP_BUNDLE, "Contents", "MacOS", "accessibility") : null;
    if (!helper || !fs.existsSync(helper)) {
      const packaged = fileURLToPath(new URL("../../bin/darwin/accessibility", import.meta.url));
      if (fs.existsSync(packaged)) helper = packaged;
    }
    // A source checkout (plugin installs in other hosts) self-compiles an
    // unsigned helper, which has no TCC grant. Prefer the installed app's
    // signed helper so accessibility and screen-recording grants carry over.
    if (!helper || !fs.existsSync(helper)) {
      const installed = path.join(os.homedir(), "Applications", "Codewhale Computer Use.app", "Contents", "MacOS", "accessibility");
      if (fs.existsSync(installed)) helper = installed;
    }
    if (!helper || !fs.existsSync(helper)) {
      const source = fileURLToPath(new URL("./darwin-accessibility.m", import.meta.url));
      const hash = crypto.createHash("sha256").update(fs.readFileSync(source)).update(fs.readFileSync(new URL("./darwin-recording.h", import.meta.url))).update(fs.readFileSync(new URL("./darwin-ocr.h", import.meta.url))).digest("hex").slice(0, 16);
      const dir = path.join(os.homedir(), ".codewhale-cu", "bin");
      fs.mkdirSync(dir, { recursive: true, mode: 0o700 });
      helper = path.join(dir, `accessibility-${hash}`);
      if (!fs.existsSync(helper)) {
        const tmp = `${helper}-${process.pid}`;
        const r = await runL("clang", ["-fobjc-arc", "-Os", "-framework", "Cocoa", "-framework", "ApplicationServices", "-framework", "ScreenCaptureKit", "-framework", "AVFoundation", "-framework", "CoreMedia", "-framework", "Vision", source, "-o", tmp], { timeoutMs: 60_000 });
        if (r.code !== 0) throw new ExecError(`native accessibility helper needs a built app or Xcode Command Line Tools: ${r.stderr}`, r);
        fs.renameSync(tmp, helper);
      }
    }
    return helper;
  }

  async function native(tool, args = {}) {
    // Every resolved target (element center or screen point) is where the
    // action lands; tracking it here means the preview cursor follows element
    // actions, not just raw pointer events.
    const t = args?.target;
    if (t && Number.isFinite(t.x) && Number.isFinite(t.y)) state.pointer = { x: t.x, y: t.y };
    if (tool === "bg_pointer") {
      const last = [...(args.steps ?? [])].reverse().find((s) => Number.isFinite(s?.x) && Number.isFinite(s?.y));
      if (last) state.pointer = { x: last.x, y: last.y };
    }
    if (tool === "pointer_sequence" && !args.app_scoped) requireSharedPointer();
    const helper = await nativeHelper();
    const r = await runL(helper, [JSON.stringify({ tool, args: { ...args, input_app_ref: state.inputApp, foreground_input: state.foregroundInput, owner_pipe: true } })], { timeoutMs: 20_000, ownerPipe: true });
    if (r.aborted || r.timedOut || r.code !== 0) {
      const error = new ExecError(r.aborted ? "computer request cancelled" : r.timedOut ? "native accessibility helper timed out" : r.stderr.trim() || "native accessibility helper failed", r);
      if (r.aborted) error.code = "cancelled";
      else error.code = nativeErrorCode(error.message) ?? undefined;
      // A deterministic native refusal sent no input. A killed/timed-out
      // helper may have posted the press before losing its response.
      const postsPress = (tool === "key_event" && args.down) || ["type", "perform_action", "click_element", "scroll_element", "set_value", "focus_element", "select_text", "bg_pointer", "bg_key"].includes(tool) || (tool === "hit_test" && args.perform) || (tool === "pointer_sequence" && args.steps?.some((step) => [1, 3, 25].includes(step.type)));
      error.inputMayHaveBeenSent = postsPress && r.spawned === true && (r.aborted || r.timedOut);
      if (error.inputMayHaveBeenSent) error.message += "; input may already have been sent — observe the target before doing anything else";
      throw error;
    }
    const result = tryJson(r.stdout, null);
    const interference = leaseVerdict(result);
    if (interference !== null) result.user_input_during_lease = interference;
    if (state.previewEnabled && state.inputApp && ["type", "key_event", "pointer_sequence", "bg_pointer", "bg_key", "set_value", "select_text", "perform_action", "hit_test", "click_element", "scroll_element", "focus_element"].includes(tool)) {
      try { await updatePreview(); } catch (error) { result.preview_error = error.message; }
    }
    return result;
  }

  async function requireBackgroundActions() {
    if ((await native("input_capabilities"))?.background_actions !== 1) {
      throw Object.assign(new ExecError("Update the Computer Use helper to use background focus, selection, context menus and scrolling."), { code: "app_upgrade_required" });
    }
  }

  function assertBoundElement(target) {
    if (!state.inputApp || target.app_ref?.pid !== state.inputApp.pid) throw new ExecError("element does not belong to the bound application — open_application and observe again");
    if (!Array.isArray(target.path) || !Number.isInteger(target.windowIndex) || !target.role) throw new ExecError("element has no resolved accessibility identity");
  }

  async function nativeLease(tool, args) {
    if (tool === "pointer_sequence") requireSharedPointer();
    if (!exec.runInputLease) throw new ExecError("This executor cannot safely own held input; update Computer Use");
    if ((await native("input_capabilities"))?.input_lease !== 1) throw new ExecError("The native helper needs an update for disconnect-safe held input");
    const helper = await nativeHelper();
    return exec.runInputLease(helper, [JSON.stringify({ tool, args: { ...args, input_app_ref: state.inputApp, foreground_input: state.foregroundInput, owner_pipe: true, input_lease: true } })]);
  }

  async function updatePreview(show = false) {
    const win = await native("window_info", { app_ref: state.inputApp });
    const dir = path.join(stateDir(), "preview");
    fs.mkdirSync(dir, { recursive: true, mode: 0o700 });
    const temp = path.join(dir, "next.png"), file = path.join(dir, "latest.png");
    const r = await runL("screencapture", ["-x", "-o", "-l", String(win.window_id), "-t", "png", temp], { timeoutMs: 8000 });
    if (r.code !== 0) throw new ExecError(`background preview capture failed: ${r.stderr}`);
    fs.renameSync(temp, file);
    const p = state.pointer;
    // The user's own hardware cursor goes on the preview too, so the panel
    // shows both pointers in the same window-relative space.
    let userCursor = null;
    try { userCursor = await native("cursor_position"); } catch {}
    await native("preview_notify", { enabled: true, show, title: `Codewhale · ${win.name} · ${state.foregroundInput ? "Shared desktop control" : "Background app control"}`, x: p ? (p.x-win.points.x)/win.points.w : -1, y: p ? (p.y-win.points.y)/win.points.h : -1,
      user_x: userCursor && Number.isFinite(userCursor.x) ? (userCursor.x-win.points.x)/win.points.w : -1,
      user_y: userCursor && Number.isFinite(userCursor.y) ? (userCursor.y-win.points.y)/win.points.h : -1 });
    // Any successful capture (bind, explicit preview, action refresh) starts
    // the live refresh; the tick itself re-enters this function as a no-op.
    if (state.previewEnabled && state.inputApp) startPreviewLoop();
    return { enabled: true, file, app: state.inputApp, pointer: p };
  }

  // ---------- pointer input ----------
  // Our qualified raw pointer path uses the shared event tap, which moves
  // the user's real cursor. Process/window-directed mouse delivery has not
  // passed the independent fixture. So the pointer path is:
  //   1. accessibility action on the element under the point (quiet, exact),
  //   2. otherwise refuse in background mode. Explicit foreground control
  //      permits a global gesture only when the bound application owns the
  //      window under the point. Restoring the cursor is not isolation.
  // Every receipt says which of the two happened.
  function mouseName(button) { return { left: "left", right: "right", middle: "middle" }[button] ?? "left"; }

  function assertInScreen(x, y) {
    if (!Number.isFinite(x) || !Number.isFinite(y)) throw new ExecError("coordinates must be finite numbers");
  }

  function buttonCode(button) { return button === "middle" ? 2 : button === "right" ? 1 : 0; }

  function requireSharedPointer() {
    if (!state.foregroundInput) throw Object.assign(new ExecError("This action needs the shared macOS pointer and was not sent in background mode. Use an accessibility action or a separate computer; foreground control requires exclusive desktop use authorized by the user."), { code: "shared_pointer_required" });
  }

  /** Refuse a global gesture whose landing point belongs to another application. */
  async function assertOwnsPoint(x, y) {
    if (!state.inputApp) throw new ExecError("open_application first to choose which application receives input");
    const w = await native("window_at_point", { x, y });
    if (!w?.found) throw new ExecError(`no window at (${x}, ${y}) — take a fresh screenshot and choose a point inside the target window`);
    if (w.owner_pid !== state.inputApp.pid) {
      throw new ExecError(`(${x}, ${y}) is covered by a window owned by ${w.owner_name || "another application"} (pid ${w.owner_pid}) — use an accessibility element target or a separate computer; no pointer input was sent`);
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
    requireSharedPointer();
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
    if (!["auto", "a11y", "event", "app"].includes(strategy)) throw new ExecError(`strategy must be auto, a11y, app or event (got ${JSON.stringify(strategy)})`);
    let a11yReason = null;
    if (strategy !== "event" && ["left", "right"].includes(button) && clicks === 1) {
      if (button === "right") await requireBackgroundActions();
      const hit = await native("hit_test", { x, y, perform: true, ...(button === "right" ? { operation: "context" } : {}) });
      if (hit?.action_sent) {
        return { action_sent: true, strategy: "a11y", action: hit.action, pointer_moved: false, at: { x, y }, button, clicks,
                 element: { role: hit.element?.role ?? null, label: hit.element?.label ?? null } };
      }
      a11yReason = hit?.reason ?? "not_found";
      if (strategy === "a11y") {
        throw new ExecError(`no supported accessibility click at (${x}, ${y}) in the bound application (${a11yReason}) — observe the available actions, use strategy "app" for a window-scoped pointer click, or a separate computer`);
      }
    } else if (strategy === "a11y") {
      throw new ExecError(`strategy "a11y" is only available for a left single click on this backend; ${mouseName(button)} x${clicks} has no accessibility equivalent`);
    }
    if (strategy === "app" || (strategy === "auto" && !state.foregroundInput)) {
      // Window-routed record delivery: AppKit accepts the events as genuine
      // input, the cursor never moves. A momentary no-raise front lease is
      // taken and restored inside the helper; it is reported, not hidden.
      if ((await native("input_capabilities"))?.window_record === 1) {
        // Ownership is enforced by window containment inside the helper: the
        // events are addressed to a window id of the bound app, so a covered
        // background window is still safe — they cannot land on the coverer.
        const r = await native("bg_pointer", { steps: clickSteps(button, x, y, clicks),
          ...(a11yReason === "web_popup_requires_real_click" ? { menu_poll_ms: 6000 } : {}) });
        return { action_sent: true, strategy: "window-record", input_scope: "application-window",
                 at: { x, y }, button, clicks, pointer_moved: false, front_lease: r.front_lease ?? true,
                 ...leaseAccounting(r),
                 ...(r.menu_lease_held ? { menu_lease_held: true } : {}),
                 window: r.window ?? null,
                 ...(a11yReason ? { a11y_reason: a11yReason } : {}) };
      }
      if (strategy !== "app") {
        // auto in background still fails closed for raw pointer; app is the
        // explicit missing middle.
        requireSharedPointer();
      }
      const owner = await assertOwnsPoint(x, y);
      const r = await native("pointer_sequence", { steps: clickSteps(button, x, y, clicks), restore: true, app_scoped: true });
      const last = { x, y };
      state.pointer = last;
      return { action_sent: true, strategy: "app-pointer", input_scope: "application-window",
               at: last, button, clicks, window: { id: owner.window_id, owner_pid: owner.owner_pid },
               ...pointerCost(r), ...(a11yReason ? { a11y_reason: a11yReason } : {}) };
    }
    const r = await gesture(clickSteps(button, x, y, clicks), { restore: true, guard: { x, y } });
    return { action_sent: true, strategy: "event", at: { x, y }, button, clicks, ...pointerCost(r),
             ...(a11yReason ? { a11y_reason: a11yReason } : {}) };
  }

  async function withPressedKey(code, flags, action) {
    const lease = await nativeLease("key_event", { code, flags, down: true });
    try {
      return await action();
    } finally {
      await withSignal(null, () => lease.release());
    }
  }

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
    if (key == null) throw new ExecError(`no non-modifier key in "${text}"`);
    return { flags, code: KEY_CODES[key], key };
  }

  // ---------- displays ----------
  async function displayInfo() { return native("displays"); }

  // ---------- screenshots ----------
  function recordingsDir() {
    return process.env.CODEWHALE_CU_RECORDINGS_DIR || path.join(os.homedir(), ".codewhale-cu", "recordings");
  }

  async function screenshot({ display, region, app_ref, window_id, path: outPath } = {}) {
    // Once an app is selected, ordinary observations follow it behind the
    // user's work. An explicit display/region remains a deliberate desktop capture.
    if (app_ref === undefined && display === undefined && region === undefined) app_ref = state.inputApp ?? undefined;
    const dir = recordingsDir();
    fs.mkdirSync(dir, { recursive: true });
    // JPEG, not PNG. A screen is photographic content — gradients, wallpaper,
    // antialiased text — and lossless compression of it is enormous: the same
    // 5760x3240 frame is 21.8MB as PNG and 2.1MB as JPEG, at full resolution
    // and with terminal text still crisp. PNG stays available by asking for a
    // `.png` path, which is what a pixel-exact comparison wants.
    const file = outPath || path.join(dir, `shot-${new Date().toISOString().replace(/[:.]/g, "-")}-${crypto.randomBytes(3).toString("hex")}.jpg`);
    if (!/\.(png|jpe?g)$/i.test(file)) throw new ExecError("screenshot path must end in .png, .jpg or .jpeg");
    const args = ["-x", "-t", /\.png$/i.test(file) ? "png" : "jpg"];
    const disp = display ?? state.activeDisplay;
    // An explicit app reference resolves first and alone: nothing may run
    // before it and redirect the capture to another target.
    const window = app_ref !== undefined ? await native("window_info", { app_ref, window_id }) : null;
    if (window && region) throw new ExecError("choose app_ref or region, not both");
    // On the display path, resolve displays before capturing so an unknown
    // index is a clean error instead of a raster silently labelled with another
    // display's geometry — list_displays reports `index` and `id` separately,
    // and a caller passing the id would otherwise get points and scale that
    // mis-target every later coordinate. A window capture ignores `display`.
    let displays = null;
    if (!window) {
      displays = await displayInfo();
      if (disp != null && disp !== "all" && !displays.some((x) => x.index === disp)) {
        throw new ExecError(`no display ${disp}; have [${displays.map((x) => x.index).join(", ")}] — screenshot takes the display index from list_displays, not its id`);
      }
    }
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
    await fitRasterToBudget(file);
    const stat = fs.statSync(file);
    displays ??= await displayInfo();
    const d = displays.find((x) => x.index === (disp === "all" ? 1 : disp)) ?? displays[0];
    const scale = d?.scale ?? 1;
    state.lastRaster = {
      file,
      ...(window ? { app_ref, window_index: window_id ?? 0 } : {}),
      bytes: stat.size,
      display: disp ?? 1,
      // Region and window rasters describe that rect, not the whole display.
      // The PNG header is the pixel ground truth; scale is derived from
      // pixels/points below so Retina and mixed-DPI stay exact.
      points: window?.points ?? (region ? { x: region[0], y: region[1], w: region[2], h: region[3] } : d?.points ?? null),
      pixels: imagePixels(file),
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
  const rec = new Map(); // Includes starting children so session close owns them too.

  function requestRecordingStop(r) {
    if (r.child.exitCode == null && r.child.signalCode == null) {
      r.child.stdin.end();
      r.child.kill("SIGINT");
    }
  }

  async function waitForRecordingStop(r, timeoutMs) {
    let timer;
    try {
      return await Promise.race([r.completion, new Promise(resolve => {
        timer = setTimeout(async () => {
          r.child.kill("SIGKILL");
          let reapTimer;
          const terminated = await Promise.race([r.completion.then(() => true), new Promise(done => { reapTimer = setTimeout(() => done(false), 500); })]);
          clearTimeout(reapTimer);
          resolve({ code: -1, terminated, error: terminated ? "screen recorder finalization timed out; partial file retained" : "screen recorder could not be terminated; recording ownership retained for retry" });
        }, timeoutMs);
      })]);
    } finally { clearTimeout(timer); }
  }

  async function recordingStart({ display, durationSec, region, app_ref, window_id } = {}) {
    const dir = recordingsDir();
    fs.mkdirSync(dir, { recursive: true });
    const id = crypto.randomBytes(4).toString("hex");
    const file = path.join(dir, `rec-${id}.mov`);
    const displays = await displayInfo();
    // app_ref scopes the recording to the app's window rect: resolved once at
    // start through the same window_info the AX path uses, so a background
    // window records behind the user's work. The rect is fixed at start —
    // it does not track later moves or resizes.
    let window = null;
    if (app_ref !== undefined || window_id != null) {
      window = await native("window_info", { app_ref: app_ref === undefined ? state.inputApp ?? undefined : app_ref, window_id });
      if (!window?.points || !(window.points.w > 0) || !(window.points.h > 0)) throw new ExecError("the selected application has no capturable window — call list_windows");
      if (region) throw new ExecError("choose app_ref or region, not both");
      region = [window.points.x, window.points.y, window.points.w, window.points.h];
    }
    let disp = display ?? state.activeDisplay;
    if (window && display == null) {
      const cx = region[0] + region[2] / 2, cy = region[1] + region[3] / 2;
      const host = displays.find(d => d.points && cx >= d.points.x && cx < d.points.x + d.points.w && cy >= d.points.y && cy < d.points.y + d.points.h);
      if (host) disp = host.index;
    }
    const selected = displays.find(d => d.index === disp);
    if (!selected) throw new ExecError("choose one available display for recording");
    if (durationSec != null && (!Number.isFinite(durationSec) || durationSec <= 0)) throw new ExecError("durationSec must be positive");
    const capabilities = await native("input_capabilities");
    if (capabilities?.record_owner_pipe !== 1) throw new ExecError("native screen recorder cannot own its client lifetime; update Computer Use before recording");
    const helper = await nativeHelper();
    throwIfAborted();
    const child = spawn(helper, [JSON.stringify({ tool: "record", args: { file, displayID: selected.id, region, durationSec, owner_pipe: true } })], { stdio: ["pipe", "pipe", "pipe"] });
    child.stdin.on("error", () => {});
    const startedAt = new Date().toISOString();
    let stderr = "", output = "", ready = false;
    const completion = new Promise(resolve => {
      child.once("error", error => resolve({ code: -1, error: error.message }));
      child.once("close", code => resolve({ code, error: stderr.trim() }));
    });
    child.stderr.on("data", chunk => { stderr = (stderr + chunk).slice(-4000); });
    const recording = { child, completion, pid: child.pid, file, startedAt, mode: "ScreenCaptureKit", display: disp };
    rec.set(id, recording);
    const signal = currentSignal();
    let timer, abort;
    try {
      await new Promise((resolve, reject) => {
        abort = () => { requestRecordingStop(recording); reject(Object.assign(new ExecError("computer request cancelled"), { code: "cancelled" })); };
        signal?.addEventListener("abort", abort, { once: true });
        if (signal?.aborted) { abort(); return; }
        timer = setTimeout(() => reject(new ExecError("screen recorder startup timed out")), 20_000);
        child.stdout.on("data", chunk => {
          output += chunk;
          let i;
          while ((i = output.indexOf("\n")) >= 0) {
            const line = output.slice(0, i); output = output.slice(i + 1);
            try { if (JSON.parse(line).ready) { ready = true; resolve(); } } catch {}
          }
        });
        completion.then(result => { if (!ready) reject(new ExecError(result.error || "screen recorder exited before capture started")); });
      });
      throwIfAborted();
      return { id, pid: child.pid, file, display: disp, durationSec: durationSec ?? null, region: region ?? null, fps: 30, mode: "ScreenCaptureKit", startedAt,
        ...(window ? { window: { id: window.window_id ?? null, name: window.name ?? null }, note: "Recording the window's rect as it was at start; it does not track moves or resizes." } : {}) };
    } catch (error) {
      requestRecordingStop(recording);
      const result = await waitForRecordingStop(recording, 2_000);
      if (result.terminated !== false) rec.delete(id);
      throw error;
    } finally {
      clearTimeout(timer);
      signal?.removeEventListener("abort", abort);
    }
  }

  async function recordingStop({ id }) {
    const r = rec.get(id);
    if (!r) throw new ExecError(`unknown or already-finished recording "${id}"`);
    requestRecordingStop(r);
    const result = await waitForRecordingStop(r, 20_000);
    if (result.code !== 0) throw new ExecError(result.error || "screen recorder failed; partial file retained");
    const size = fs.existsSync(r.file) ? fs.statSync(r.file).size : 0;
    if (!size) throw new ExecError("screen recorder produced no video");
    rec.delete(id);
    return { id, file: r.file, mp4: null, bytes: size, mode: r.mode, startedAt: r.startedAt, stoppedAt: new Date().toISOString() };
  }

  async function closeSession() {
    // The preview this session showed must not outlive the session; a panel
    // from a dead session has no owner to refresh or hide it.
    stopPreviewLoop();
    await quiescePreview();
    if (state.previewEnabled && state.inputApp) {
      try { await native("preview_notify", { enabled: false }); } catch { /* hiding is best-effort */ }
    }
    state.previewEnabled = false;
    await browser.close().catch(() => {});
    const owned = [...rec.entries()];
    for (const [, recording] of owned) requestRecordingStop(recording);
    const results = await Promise.all(owned.map(async ([id, recording]) => {
      const result = await waitForRecordingStop(recording, 2_000);
      if (result.terminated !== false) rec.delete(id);
      return result;
    }));
    const failed = results.find(result => result.code !== 0);
    if (failed) throw new ExecError(failed.error || "screen recorder failed; partial file retained");
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
      if (st.isFile() && /\.(mov|mp4|png|jpe?g)$/i.test(f)) out.push({ file: full, bytes: st.size, modifiedAt: st.mtime.toISOString() });
    }
    out.sort((a, b) => b.modifiedAt.localeCompare(a.modifiedAt));
    return { dir, recordings: out.slice(0, 50), running: [...rec.keys()] };
  }

  // ---------- apps / windows ----------
  async function listApps(args = {}) {
    if (args?.installed === true) {
      const r = await native("installed_apps", {});
      const apps = Array.isArray(r?.apps) ? r.apps : [];
      return {
        apps,
        total: apps.length,
        installed: true,
        note: "Installed catalog from /Applications, /System/Applications and ~/Applications; running flags reflect this moment. This scan takes a moment.",
      };
    }
    const r = await native("list_apps");
    const apps = Array.isArray(r?.apps) ? r.apps : [];
    const shown = selectApps(apps, args?.all === true);
    return {
      apps: shown,
      total: apps.length,
      filtered: args?.all === true ? "all" : "regular",
      ...(shown.length !== apps.length ? { note: "Regular (user-facing) apps only — pass all:true to include menu-bar helpers and background processes." } : {}),
    };
  }

  async function listWindows({ app_ref } = {}) { return native("list_windows", { app_ref: app_ref === undefined ? state.inputApp ?? undefined : app_ref }); }

  async function openApplication({ name, bundle_id: bid, pid, url: urlArg, activate = false } = {}) {
    if (!name && !bid && !pid) throw new ExecError("open_application needs name, bundle_id or pid");
    // Failed selection must not leave an earlier app armed for shared input.
    state.foregroundInput = false;
    state.inputApp = null;
    // pid is the most specific identity and the only one that separates two
    // processes of the same bundle (e.g. a second Chrome on its own profile),
    // so it wins when given.
    const find = {};
    if (pid) find.pid = pid; else if (bid) find.bundle_id = bid; else find.name = String(name).replace(/\.app$/, "");
    let p;
    let launched = false;
    // Binding an already-running app must not ask LaunchServices to reopen
    // it: reopen can raise windows even with open -g on some applications.
    if (!urlArg) {
      try { p = await native("app_info", { app_ref: find, activate }); }
      catch (error) {
        if (!error.message.includes("application not found")) throw error;
      }
    }
    if (!p?.found) {
      if (!name && !bid) throw Object.assign(new ExecError(`no running application with pid ${pid}; call list_apps for the current processes`), { code: "app_not_found" });
      const args = [];
      if (urlArg) args.push(urlArg);
      if (bid) args.unshift("-b", bid); else args.unshift("-a", name);
      if (!activate) args.unshift("-g");
      const r = await runL("open", args, { timeoutMs: 25_000 });
      if (r.code !== 0) {
        const stderr = (r.stderr ?? "").trim();
        // A name or bundle id that resolves nowhere is a stable refusal code,
        // not a generic opener failure — agents branch on the code.
        const code = /Unable to find application|failed while trying to determine the application/i.test(stderr)
          ? "app_not_found" : undefined;
        throw Object.assign(new ExecError(`open failed: ${stderr.slice(0, 200)}`), { code });
      }
      launched = true;
      await new Promise((res) => setTimeout(res, 600));
      p = await native("app_info", { app_ref: find, activate });
    }
    if (activate && p?.frontmost === false) throw Object.assign(new ExecError("The selected application did not become frontmost; no input mode was enabled. Continue with background control or wait for the user."), { code: "activation_not_confirmed" });
    if (p?.bundle_id === "net.codewhale.computer-use") throw Object.assign(new ExecError("The Computer Use setup and safety controls belong to the user and cannot be operated by this plugin."), { code: "protected_application" });
    // A bare executable has no bundle id; carrying an empty one would make the
    // identity unmatchable.
    state.inputApp = { pid: p.pid, ...(p.bundle_id ? { bundle_id: p.bundle_id } : {}), ...(p.name ? { name: p.name } : {}) };
    state.foregroundInput = !!activate;
    // Surface the watch panel on bind; a capture failure (e.g. missing Screen
    // Recording) must never block the bind itself. The first successful
    // capture also starts the refresh loop so the panel stays live while bound.
    if (state.previewEnabled) {
      previewBusy = true;
      updatePreview(true).catch(() => {}).finally(() => { previewBusy = false; });
    }
    return { launched, activate, keyboard_delivery: activate ? "foreground-guarded" : "process", input_scope: activate ? "shared-desktop" : "application", shared_pointer: !!activate, isolated_desktop: false, url: urlArg ?? null, resolved: p?.found ? { name: p.name, pid: p.pid, bundle_id: p.bundle_id, frontmost: p.frontmost } : null };
  }

  /**
   * Menu items by title path, through accessibility only: no key events, no
   * focus lease. Menus expose items only while open, so each level is pressed
   * and the next is polled for. Exact titles; an ellipsis is part of the title.
   */
  async function invokeMenu(menuPath) {
    if (!state.inputApp) throw new ExecError("open_application first — invoke_menu acts on the bound application");
    if (!Array.isArray(menuPath) || menuPath.length < 1 || menuPath.length > 3 || menuPath.some((s) => typeof s !== "string" || !s.trim())) {
      throw new ExecError('invoke_menu needs path: 1..3 non-empty menu titles, e.g. ["File","New"]');
    }
    const titles = menuPath.map((s) => s.trim());
    const app_ref = state.inputApp;
    const pressed = [];
    for (let level = 0; level < titles.length; level++) {
      const found = await findMenuItem(app_ref, titles[level], level === 0);
      if (!found) {
        throw Object.assign(new ExecError(`menu item "${titles[level]}" not found ${pressed.length ? `under ${pressed.join(" ▸ ")}` : "on the menu bar"} — menus expose items only while open; check the exact title with get_app_state (an ellipsis is part of the title)`), { code: "menu_item_not_found" });
      }
      if (found.enabled === false) {
        throw Object.assign(new ExecError(`menu item "${titles[level]}" is present but disabled right now — the app validates it against its current state (in background mode that is often a missing key window for window-targeted commands like Close). Use an element action on the window's own control instead of pressing a disabled item.`), { code: "menu_item_disabled" });
      }
      const target = { app_ref, windowIndex: found.windowIndex ?? 0, path: found.path, role: found.role, label: found.label };
      assertBoundElement(target);
      const action = found.role === "AXMenuItem" && (found.actions ?? []).includes("AXPick") ? "AXPick" : "AXPress";
      await native("perform_action", { target, action });
      pressed.push(titles[level]);
      if (level < titles.length - 1) await wait(140);
    }
    return { action_sent: true, strategy: "a11y", route: "accessibility", delivery: "background", menu: pressed, front_lease: false,
             note: "Menu activation used accessibility only — no key events or focus lease. Verify the app effect (list_windows / get_app_state) before reporting success." };
  }

  /** Poll for the exact menu element; opens and submenu population are async. */
  async function findMenuItem(app_ref, label, menuBar) {
    const deadline = Date.now() + 4_000;
    for (;;) {
      const obs = await native("get_app_state", { app_ref, detail: "full" });
      const hit = pickMenuElement(obs?.elements ?? [], label, menuBar);
      if (hit) return hit;
      if (Date.now() >= deadline) return null;
      await wait(120);
    }
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
    return { platform: "darwin", capabilities: caps, permissions: perms, note: "macOS does not expose Screen-Recording TCC state to CLI; a black/empty screenshot means Screen Recording permission is missing. Background mode (open_application activate:false) uses process-bound keyboard events and accessibility actions; shared pointer gestures are refused. Foreground control (activate:true) uses the shared desktop and requires exclusive use. Neither mode is an isolated desktop. App-specific behavior still requires verification." };
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
    set_window_frame: async ({ app_ref, window_id, frame } = {}) => {
      if (!frame || !Number.isFinite(frame.x) || !Number.isFinite(frame.y) || !Number.isFinite(frame.w) || !Number.isFinite(frame.h) || frame.w <= 0 || frame.h <= 0) {
        throw Object.assign(new ExecError("set_window_frame needs frame {x,y,w,h} with positive w/h"), { code: "bad_args" });
      }
      if (!Number.isSafeInteger(window_id) || window_id < 0) {
        throw Object.assign(new ExecError("set_window_frame needs window_id (a non-negative window index from list_windows)"), { code: "bad_args" });
      }
      const r = await native("set_window_frame", { app_ref, window_id, frame });
      return { ...r, verified: r?.verified === true, note: r?.note ?? "the after frame is the app's own readback; cross-check with list_windows before relying on it" };
    },
    list_windows: listWindows,
    open_application: openApplication,
    get_app_state: async ({ app_ref, detail, depth, window_id, include_ocr = false, ocr_region } = {}) => {
      const t0 = Date.now();
      const t = await native("get_app_state", { app_ref: app_ref === undefined ? state.inputApp ?? undefined : app_ref, detail, window_id });
      if (process.env.CODEWHALE_CU_DEBUG_OBSERVE) console.error(`observe ${Date.now() - t0}ms elements=${t.elements?.length} truncated=${t.truncated}`);
      if (!t.found) throw new ExecError("application not found — call list_apps for exact names/pids");
      if (include_ocr) {
        // Resolve once through AX, then capture only that exact application's
        // selected window. A changing foreground cannot redirect this image.
        let raster;
        try {
          if (!Number.isSafeInteger(t.pid) || t.pid <= 0) throw new ExecError("The observed application did not provide an exact process identity for OCR");
          if ((await native("input_capabilities"))?.window_ocr !== 1) throw new ExecError("The native helper needs an update for selected-window text recognition");
          // PNG here, against the JPEG default: this raster is fed to text
          // recognition, not to a viewer, and lossless glyph edges are what
          // Vision reads. A single window is small enough that the size the
          // JPEG default exists to solve does not arise.
          const ocrDir = path.join(recordingsDir(), "captures");
          fs.mkdirSync(ocrDir, { recursive: true });
          raster = await screenshot({
            ...(ocr_region
              ? { region: ocr_region }
              : { app_ref: { pid: t.pid, ...(t.bundle_id ? { bundle_id: t.bundle_id } : {}) }, window_id }),
            path: path.join(ocrDir, `ocr-${crypto.randomBytes(4).toString("hex")}.png`),
          });
          const ocr = await native("recognize_text", { file: raster.file });
          if (ocr?.status === "ok" && ocr.pixels?.w === raster.pixels.w && ocr.pixels?.h === raster.pixels.h && Array.isArray(ocr.blocks)) {
            t.ocr = { ...ocr, raster, blocks: ocr.blocks.map(block => ({ ...block, target: {
              type: "coordinate", x: Math.floor(block.bounds.x + block.bounds.w / 2), y: Math.floor(block.bounds.y + block.bounds.h / 2),
            } })) };
          } else {
            t.ocr = { status: "unavailable", engine: "apple_vision", reason: ocr?.reason ?? "The native OCR helper needs an update or returned mismatched image dimensions", blocks: [], raster };
          }
        } catch (error) {
          throwIfAborted();
          if (error.code === "cancelled") throw error;
          t.ocr = { status: "unavailable", engine: "apple_vision", reason: error.message, blocks: [], ...(raster ? { raster } : {}) };
        }
      }
      return t;
    },
    resolve_element: async ({ app_ref, windowIndex, path: pathArr } = {}) => {
      const r = await native("resolve_element", { app_ref, windowIndex: windowIndex ?? 0, path: pathArr ?? [] });
      return { found: !!r?.found, element: r?.element ?? null, reason: r?.reason ?? null };
    },
    preview: async ({ enabled = true } = {}) => {
      state.previewEnabled = enabled;
      if (!enabled) { stopPreviewLoop(); await quiescePreview(); await native("preview_notify", { enabled: false }); return { enabled: false }; }
      if (!state.inputApp) throw new ExecError("open_application first to choose the preview app");
      return updatePreview(true);
    },
    screenshot,
    zoom,
    left_click: async ({ target, strategy = "auto" } = {}) => {
      if (target?.type !== "element" || strategy === "event" || strategy === "app") return pointerClick("left", target?.x, target?.y, 1, strategy);
      if (!["auto", "a11y"].includes(strategy)) throw new ExecError(`strategy must be auto, a11y, app or event (got ${JSON.stringify(strategy)})`);
      try {
        assertBoundElement(target);
        if ((await native("input_capabilities"))?.element_identity !== 1) throw new ExecError("native helper needs an update for element identity validation");
        const semantic = ["AXTextField", "AXTextArea", "AXComboBox", "AXRow", "AXCell", "AXMenuItem"].includes(target.role);
        if (semantic) await requireBackgroundActions();
        const receipt = await native(semantic ? "click_element" : "perform_action", { target, action: "AXPress" });
        if (!receipt?.action_sent) throw new ExecError("element press was not acknowledged");
        return { ...receipt, action: receipt.action ?? "AXPress", strategy: "a11y", pointer_moved: false,
          element: { role: target.role, label: target.label ?? null }, verified: receipt.verified ?? false, verification_required: "observation" };
      } catch (error) {
        // An AX frame can cover other controls. Never turn a refused or
        // ambiguous element press into another element's press or a raw click.
        error.message += ' — no coordinate fallback was sent; take a fresh screenshot or OCR observation and choose an advertised action or a separate computer';
        throw error;
      }
    },
    double_click: ({ target } = {}) => pointerClick("left", target?.x, target?.y, 2),
    triple_click: ({ target } = {}) => pointerClick("left", target?.x, target?.y, 3),
    right_click: async ({ target } = {}) => {
      if (target?.type !== "element") return pointerClick("right", target?.x, target?.y, 1);
      assertBoundElement(target);
      await requireBackgroundActions();
      return native("click_element", { target, context: true });
    },
    middle_click: ({ target } = {}) => pointerClick("middle", target?.x, target?.y, 1),
    mouse_move: async ({ target } = {}) => {
      assertInScreen(target?.x, target?.y);
      requireSharedPointer();
      if (state.pointerLease) {
        try {
          const r = await state.pointerLease.send({ point: target });
          state.pointer = { x: target.x, y: target.y };
          return { action_sent: true, strategy: "event", at: state.pointer, ...pointerCost(r) };
        } catch (error) { state.pointerLease = null; throw error; }
      }
      // A hover has to leave the pointer where it was asked to go.
      const r = await gesture([{ type: MOUSE_MOVED, x: target.x, y: target.y, button: 0, clickState: 0 }], { restore: false, guard: target });
      return { action_sent: true, strategy: "event", at: { x: target.x, y: target.y }, ...pointerCost(r) };
    },
    left_mouse_down: async ({ target } = {}) => {
      assertInScreen(target?.x, target?.y);
      requireSharedPointer();
      if (state.pointerLease) throw new ExecError("this session already holds the left pointer button; release it first");
      await assertOwnsPoint(target.x, target.y);
      state.pointerLease = await nativeLease("pointer_sequence", { steps: [
          { type: MOUSE_MOVED, x: target.x, y: target.y, button: 0, clickState: 0 },
          { type: MOUSE.left.down, x: target.x, y: target.y, button: 0, clickState: 1 },
        ], restore: false });
      state.pointer = { x: target.x, y: target.y };
      return { action_sent: true, strategy: "event", at: state.pointer, ...pointerCost(state.pointerLease.receipt) };
    },
    left_mouse_up: async ({ target } = {}) => {
      if (!state.pointerLease) throw new ExecError("no agent pointer button is held by this session");
      const loc = target ?? state.pointer;
      if (!loc) throw new ExecError("no agent pointer position — mouse_move or left_mouse_down first");
      assertInScreen(loc.x, loc.y);
      // No ownership guard: the button is already held, and the drag may have
      // legitimately left the originating window.
      try { await withSignal(null, () => state.pointerLease.release({ point: loc })); }
      finally { state.pointerLease = null; }
      state.pointer = { x: loc.x, y: loc.y };
      return { action_sent: true, strategy: "event", at: state.pointer, pointer_moved: true, pointer_restored: false };
    },
    left_click_drag: async ({ from_target: from, to } = {}) => {
      assertInScreen(from?.x, from?.y); assertInScreen(to?.x, to?.y);
      const steps = [
        { type: MOUSE_MOVED, x: from.x, y: from.y, button: 0, clickState: 0 },
        { type: MOUSE.left.down, x: from.x, y: from.y, button: 0, clickState: 1, delayMs: 60 },
      ];
      const n = 12;
      for (let i = 1; i <= n; i++) {
        steps.push({ type: MOUSE.left.dragged, x: from.x + ((to.x - from.x) * i) / n, y: from.y + ((to.y - from.y) * i) / n, button: 0, clickState: 1, delayMs: 45 });
      }
      steps.push({ type: MOUSE.left.up, x: to.x, y: to.y, button: 0, clickState: 1, delayMs: 80 });
      if (!state.foregroundInput && (await native("input_capabilities"))?.window_record === 1) {
        const r = await native("bg_pointer", { steps });
        return { action_sent: true, strategy: "window-record", input_scope: "application-window",
                 from, to, pointer_moved: false, front_lease: r.front_lease === true, window: r.window ?? null,
                 ...leaseAccounting(r),
                 ...(typeof r.front_restored === "boolean" ? { front_restored: r.front_restored } : {}) };
      }
      const r = await gesture(steps, { restore: true, guard: from });
      return { action_sent: true, strategy: "event", from, to, ...pointerCost(r) };
    },
    scroll: async ({ target, direction = "down", amount = 5 } = {}) => {
      assertInScreen(target?.x, target?.y);
      if (!state.foregroundInput) {
        await requireBackgroundActions();
        if (target.type === "element") {
          assertBoundElement(target);
          return native("scroll_element", { target, direction, amount });
        }
        const receipt = await native("hit_test", { x: target.x, y: target.y, perform: true, direction, amount,
          operation: ["left", "right"].includes(direction) ? "scroll-horizontal" : "scroll-vertical" });
        if (receipt?.action_sent) return receipt;
        // No AX scrollbar here (overlay scrollers, web pages): wheel events
        // still reach the view through the window-record route.
        if ((await native("input_capabilities"))?.window_record === 1) {
          const dx = direction === "left" ? amount : direction === "right" ? -amount : 0;
          const dy = direction === "up" ? amount : direction === "down" ? -amount : 0;
          const notches = Math.max(1, Math.min(100, Math.round(amount)));
          const steps = [];
          for (let i = 0; i < notches; i++) steps.push({ scroll: [Math.sign(dx), Math.sign(dy)], x: target.x, y: target.y, delayMs: 15 });
          const r = await native("bg_pointer", { steps });
          return { action_sent: true, strategy: "window-record", input_scope: "application-window",
                   direction, amount, pointer_moved: false, front_lease: r.front_lease === true, window: r.window ?? null,
                   verified: false, verification_required: "observation", ...leaseAccounting(r),
                   ...(typeof r.front_restored === "boolean" ? { front_restored: r.front_restored } : {}) };
        }
        throw Object.assign(new ExecError(`No background scrollbar at this point (${receipt?.reason ?? "not_found"}); choose an observed scroll area or a separate computer.`), { code: "background_scroll_unavailable" });
      }
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
    type: (args = {}) => native("type", args),
    key: async ({ text, repeat = 1 } = {}) => {
      const { flags, code, key } = parseChord(text);
      const n = Math.max(1, Math.min(100, repeat));
      // A chorded press is usually aimed at the menu system (cmd+w,
      // cmd+shift+g, …), and key equivalents only validate against a key
      // window. A process-bound event without one is discarded silently —
      // the receipt would still say action_sent. In background mode the
      // window-record route supplies a momentary key window, so flagged
      // chords go through it when the helper supports it.
      if (flags !== 0 && !state.foregroundInput && (await native("input_capabilities"))?.window_record === 1) {
        try {
          let last;
          for (let i = 0; i < n; i++) {
            last = await native("bg_key", { code, flags });
            if (i < n - 1) await wait(30);
          }
          return { action_sent: true, key, code, keyboard_delivery: "window-record", input_scope: "application-window",
                   front_lease: last?.front_lease === true, repeat: n, ...leaseAccounting(last),
                   ...(typeof last?.front_restored === "boolean" ? { front_restored: last.front_restored } : {}),
                   ...(last?.front_restored === false ? { note: "the momentary window-record lease did not hand the user's foreground back; their next keystrokes may land in this app. Tell the user." } : {}) };
        } catch (error) {
          // No focused window or a refused lease: the key cannot reach the
          // menu system this way either. Fall through to process delivery
          // and say plainly in the receipt what was actually sent.
          if (!/no focused window|window-routed background keys|bg_dispatch/.test(error.message)) throw error;
        }
      }
      for (let i = 0; i < n; i++) {
        await withPressedKey(code, flags, () => {});
        if (i < n - 1) await wait(30);
      }
      return { action_sent: true, key, code, keyboard_delivery: state.foregroundInput ? "foreground-guarded" : "process", repeat: n,
        ...(flags !== 0 && !state.foregroundInput ? { note: "process delivery (no focus lease was taken); menu key equivalents can be dropped without a key window. Verify the effect before retrying, or use invoke_menu for app menu commands." } : {}) };
    },
    hold_key: async ({ text, duration } = {}) => {
      const { flags, code, key } = parseChord(text);
      const d = Math.max(0.05, Math.min(30, Number(duration) || 1));
      await withPressedKey(code, flags, () => wait(d * 1000));
      return { action_sent: true, key, keyboard_delivery: state.foregroundInput ? "foreground-guarded" : "process", heldSec: d };
    },
    set_value: async (args = {}) => {
      if (args.target?.type !== "element") throw new ExecError("set_value needs an element target — {type:'element',index} from get_app_state");
      try {
        return await native("set_value", args);
      } catch (error) {
        // Web text controls ignore AXValue writes, so the native side refuses
        // before dispatch. The replacement path is focus + select-all + type
        // with a read-back verify — the same shape kimi-cu uses, with the
        // value proven rather than asserted.
        if (!/web area/i.test(error.message)) throw error;
        if (args.target?.type !== "element") throw error;
        const value = String(args.value ?? "");
        await native("focus_element", { target: args.target });
        // cmd+a through the record channel: menu key equivalents only
        // validate against a key window, which the lease provides. bg_key
        // posts a complete press (down and up); the `down` field is unused.
        await native("bg_key", { code: 0, flags: 1 << 20 });
        await new Promise((r) => setTimeout(r, 60));
        await native("type", { text: value });
        const back = await native("get_value", { target: args.target });
        const verified = back?.value === value;
        return { action_sent: true, strategy: "focus-type-replace", role: back?.role ?? null,
                 after: back?.value ?? null, verified,
                 ...(verified ? {} : { note: "replacement did not verify against the control's own value; observe before relying on it" }) };
      }
    },
    focus: (args = {}) => native("focus_element", args),
    get_value: (args = {}) => native("get_value", args),
    select_text: async (args = {}) => {
      if (args.target?.type !== "element") throw new ExecError("select_text needs an element target — {type:'element',index} from get_app_state");
      return native("select_text", args);
    },
    perform_action: async (args = {}) => {
      if (args.target?.type !== "element") throw new ExecError("perform_action needs an element target — {type:'element',index} from get_app_state");
      return native("perform_action", args);
    },
    invoke_menu: async ({ path: menuPath } = {}) => invokeMenu(menuPath),
    read_clipboard: readClipboard,
    write_clipboard: writeClipboard,
    cursor_position: cursorPosition,
    recordingStart,
    recordingStop,
    recordingStatus,
    recordingList,
    closeSession,
    list_sessions: async () => ({
      via: "direct",
      count: 1,
      sessions: [{
        target: state.inputApp ? { pid: state.inputApp.pid, ...(state.inputApp.bundle_id ? { bundle_id: state.inputApp.bundle_id } : {}), ...(state.inputApp.name ? { name: state.inputApp.name } : {}) } : null,
        mode: state.foregroundInput ? "foreground" : "background",
        action: null,
        ageSec: 0,
        inputHeld: !!state.pointerLease,
      }],
    }),
    kill_app: async (args = {}) => {
      const { name, bundle_id, pid, force } = args;
      if (!name && !bundle_id && pid == null) throw Object.assign(new ExecError("kill_app needs name, bundle_id or pid"), { code: "bad_args" });
      return native("kill_app", { name, bundle_id, pid, force: force === true });
    },
    browser_start: browser.start,
    browser_status: browser.status,
    browser_navigate: browser.navigate,
    browser_click: browser.click,
    browser_type: browser.type,
    browser_screenshot: browser.screenshot,
    browser_stop: browser.stop,
    releaseInput: async () => {
      if (!state.pointerLease) return;
      try { await withSignal(null, () => state.pointerLease.release({ point: state.pointer })); }
      finally { state.pointerLease = null; }
    },
  };
}

export default { create };
