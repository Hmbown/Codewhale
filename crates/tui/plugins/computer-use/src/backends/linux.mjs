// Linux backend — X11 first (xdotool/wmctrl/scrot/xclip), Wayland where the
// right tools exist (grim/wtype/ydotool/wf-recorder/wl-clipboard). The
// accessibility tree comes from AT-SPI via python3+pyatspi when installed.
// Everything probes at call time and fails closed with the missing tool named.
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import crypto from "node:crypto";
import { spawn } from "node:child_process";
import { run as nativeRun, runOk, ExecError, tryJson, have as nativeHave, withSignal, throwIfAborted, wait } from "../exec.mjs";
import { pngSize } from "../png-size.mjs";
import { createBrowser } from "../browser-cdp.mjs";
import { recordingsDir, recordingsOutputPath } from "../recordings.mjs";

const XKEYS = {
  return: "Return", enter: "Return", tab: "Tab", escape: "Escape", esc: "Escape",
  space: "space", backspace: "BackSpace", delete: "Delete", home: "Home", end: "End",
  pageup: "Page_Up", pagedown: "Page_Down", left: "Left", right: "Right", up: "Up",
  down: "Down", capslock: "Caps_Lock", menu: "Menu", print: "Print",
};

function spawnDetached(cmd, args, stdinText = "", quiet = true) {
  const child = spawn(cmd, args, {
    stdio: stdinText ? ["pipe", "ignore", quiet ? "ignore" : "pipe"] : ["ignore", "ignore", quiet ? "ignore" : "pipe"],
    detached: true,
  });
  if (stdinText) child.stdin.end(stdinText);
  child.unref();
  return child;
}

/** Coordinate clicks on this backend are always raw pointer events; strategy="a11y" must fail closed rather than silently degrade. */
function assertEventStrategy(strategy) {
  if (strategy != null && strategy !== "auto" && strategy !== "event") {
    throw new ExecError(`strategy "${strategy}" is macOS-only; this backend dispatches coordinate clicks as raw pointer events — use an element target for a semantic action`);
  }
}

function appName(ref) {
  if (ref === undefined) return "";
  if (!ref || typeof ref !== "object" || Array.isArray(ref) || Object.keys(ref).length !== 1 || typeof ref.name !== "string" || !ref.name.trim()) {
    throw Object.assign(new ExecError("Linux accessibility targeting supports only a nonblank app_ref.name; PID and bundle selectors are unavailable"), { code: "unsupported_selector" });
  }
  return ref.name;
}

/** Linux selects windows by exact app name (app_ref.name); numeric window ids are not exposed. */
function rejectWindowSelectors(args) {
  if (Object.hasOwn(args, "window_id")) throw Object.assign(new ExecError("Linux window selection is by app_ref.name only; window_id is unavailable"), { code: "unsupported_selector" });
}

/** Exact, case-insensitive app-name match used everywhere a name is compared. */
function sameName(a, b) {
  return typeof a === "string" && typeof b === "string" && a.trim().length > 0 && a.trim().replace(/\.app$/i, "").toLowerCase() === b.trim().replace(/\.app$/i, "").toLowerCase();
}

function assertAppRootWindow(index, windowId) {
  if (windowId !== undefined || (index !== undefined && index !== 0)) throw Object.assign(new ExecError("Linux accessibility paths start at the app root; window selectors are unavailable"), { code: "unsupported_selector" });
}

function outputPath(file) {
  if (typeof file !== "string" || !path.isAbsolute(file) || file.includes("\0")) throw new ExecError("output path must be an absolute filename");
  return file;
}

/**
 * ffmpeg arguments for an x11grab H.264 recording of one rectangle. H.264 with
 * yuv420p needs even dimensions, so the rectangle is trimmed to even sizes.
 * stdin stays open: 'q' on it is how the recorder finishes the file.
 */
export function x11GrabArgs({ display, rect, fps, file, durationSec = null }) {
  const w = rect[2] - (rect[2] % 2);
  const h = rect[3] - (rect[3] % 2);
  if (w < 2 || h < 2) throw Object.assign(new ExecError("the recording area must be at least 2x2 pixels"), { code: "bad_args" });
  return [
    "-y", "-loglevel", "error",
    "-f", "x11grab", "-framerate", String(fps), "-video_size", `${w}x${h}`,
    "-i", `${display}+${rect[0]},${rect[1]}`,
    ...(durationSec ? ["-t", String(durationSec)] : []),
    "-c:v", "libx264", "-preset", "veryfast", "-crf", "23", "-pix_fmt", "yuv420p", "-movflags", "+faststart",
    file,
  ];
}

export function create({ exec } = {}) {
  const run = exec?.run ?? nativeRun;
  const have = exec?.have ?? nativeHave;
  const browser = createBrowser({ platform: "linux" });
  function requireInputOwner() {
    if (exec?.persistentInputOwner !== true) throw Object.assign(new ExecError(
      "This held-input gesture requires a connected Codewhale Computer Use desktop helper so a disconnected client cannot leave keys or buttons pressed. Start the helper and reconnect before retrying."
    ), { code: "input_owner_required" });
  }

  const tools = {};
  let session = null; // "x11" | "wayland"
  let probed = false;
  let lastRaster = null;
  let mouseHeld = false;
  const heldKeys = new Set();
  let lastTypedAt = 0;
  const recordings = new Map(); // id -> { child, file, rect, fps, startedAt, stderr }

  async function settleAfterTyping() {
    const since = Date.now() - lastTypedAt;
    if (lastTypedAt && since < 300) await wait(300 - since);
  }

  async function releaseMouse() {
    if (!mouseHeld) return;
    await withSignal(null, () => session === "x11" ? xdotool(["mouseup", "1"], { timeoutMs: 2_000 }) : ydotool(["click", "0x80"], { timeoutMs: 2_000 }));
    mouseHeld = false;
  }

  async function releaseKey(key) {
    if (!heldKeys.has(key)) return;
    await withSignal(null, () => xdotool(["keyup", key], { timeoutMs: 2_000 }));
    heldKeys.delete(key);
  }

  async function releaseInput() {
    await releaseMouse();
    for (const key of heldKeys) await releaseKey(key);
  }

  async function probeSession() {
    if (probed) return session;
    throwIfAborted();
    const wayland = !!(process.env.WAYLAND_DISPLAY || process.env.XDG_SESSION_TYPE === "wayland");
    const x11 = !!(process.env.DISPLAY || process.env.XDG_SESSION_TYPE === "x11");
    session = wayland && !x11 ? "wayland" : x11 ? "x11" : null;
    if (session === null) {
      const e = new ExecError("no X11 ($DISPLAY) or Wayland ($WAYLAND_DISPLAY) session visible to this process — set DISPLAY or run inside the desktop session");
      e.code = "no_session";
      throw e;
    }
    for (const t of ["xdotool", "wmctrl", "scrot", "import", "grim", "slurp", "wtype", "ydotool", "wf-recorder", "ffmpeg", "xclip", "xsel", "wl-copy", "wl-paste", "python3", "xrandr", "swaymsg", "hyprctl"]) {
      tools[t] = await have(t);
    }
    tools.pyatspi = tools.python3 && (await run("python3", ["-c", "import pyatspi"], { timeoutMs: 10_000 })).code === 0;
    throwIfAborted();
    probed = true;
    return session;
  }

  function need(tool, purpose) {
    if (!tools[tool]) throw new ExecError(`linux backend needs "${tool}" for ${purpose} — install it and retry`);
  }

  // ---------- X11 windows: stacking, frames and owner identity ----------
  // Input lands on the active window (keys) or the topmost window under the
  // point (pointer). Consent must be checked against that window's app, not
  // against whichever app the agent last observed. The window manager
  // publishes both facts through EWMH; without it we cannot attribute input,
  // so callers fail closed.
  async function softRun(cmd, args) {
    const r = await run(cmd, args, { timeoutMs: 5_000 });
    if (r.aborted) throwIfAborted();
    return r.code === 0 ? r.stdout : "";
  }

  function normWindowId(value) {
    const n = Number(String(value ?? "").trim());
    return Number.isFinite(n) && n > 0 ? `0x${n.toString(16).padStart(8, "0")}` : null;
  }

  function procComm(pid) {
    if (!Number.isInteger(pid) || pid <= 0) return null;
    try { return fs.readFileSync(`/proc/${pid}/comm`, "utf8").trim() || null; } catch { return null; }
  }

  function parseFrameExtents(text) {
    const m = /_NET_FRAME_EXTENTS\(CARDINAL\) = ([\d,\s-]+)/.exec(text);
    const v = m ? m[1].split(",").map((s) => Number(s.trim()) || 0) : [0, 0, 0, 0];
    return { left: v[0], right: v[1], top: v[2], bottom: v[3] };
  }

  /** Client rectangle and mapping state from `xwininfo -id`; null when the window vanished. */
  function parseXwininfo(text) {
    const x = /Absolute upper-left X:\s+(-?\d+)/.exec(text);
    const y = /Absolute upper-left Y:\s+(-?\d+)/.exec(text);
    const w = /Width:\s+(\d+)/.exec(text);
    const h = /Height:\s+(\d+)/.exec(text);
    if (!x || !y || !w || !h) return null;
    const map = /Map State:\s+(\S+)/.exec(text);
    return { x: Number(x[1]), y: Number(y[1]), w: Number(w[1]), h: Number(h[1]), viewable: map ? map[1] === "IsViewable" : true };
  }

  /** Every managed window with its owner (wmctrl -lpx), in no particular order. */
  async function wmctrlWindows() {
    const out = [];
    for (const line of (await softRun("wmctrl", ["-lpx"])).split("\n")) {
      const m = /^(0x[0-9a-f]+)\s+(-?\d+)\s+(\d+)\s+(\S+)\s+\S+\s+(.*)$/i.exec(line.trim());
      if (!m) continue;
      const wm_class = m[4];
      out.push({ id: normWindowId(m[1]), pid: Number(m[3]), wm_class, name: wm_class.split(".")[0] || wm_class, title: m[5] });
    }
    return out;
  }

  /** Managed windows bottom to top: id, owner (wm class, process, pid), client and frame rectangles. Null without a window manager. */
  async function x11Windows() {
    const stack = await softRun("xprop", ["-root", "_NET_CLIENT_LIST_STACKING"]);
    if (!/_NET_CLIENT_LIST_STACKING\(WINDOW\)/.test(stack)) return null;
    const order = [...stack.matchAll(/0x[0-9a-f]+/gi)].map((m) => normWindowId(m[0])).filter(Boolean);
    const listed = new Map((await wmctrlWindows()).map((w) => [w.id, w]));
    const windows = [];
    for (const id of order) {
      const meta = listed.get(id);
      if (!meta) continue;
      const client = parseXwininfo(await softRun("xwininfo", ["-id", id]));
      const ext = parseFrameExtents(await softRun("xprop", ["-id", id, "_NET_FRAME_EXTENTS"]));
      const frame = client ? {
        x: client.x - ext.left, y: client.y - ext.top,
        w: client.w + ext.left + ext.right, h: client.h + ext.top + ext.bottom,
      } : null;
      windows.push({
        id, ...meta, name: meta.wm_class.split(".")[0] || meta.wm_class,
        comm: procComm(meta.pid), viewable: client?.viewable ?? false, frame,
      });
    }
    return windows;
  }

  function identityOf(win) {
    return { name: win.name, comm: win.comm, pid: win.pid, wm_class: win.wm_class, title: win.title, window: win.id };
  }

  const ownerUnknown = (message) => Object.assign(new ExecError(message), { code: "input_owner_unknown" });

  /** The app whose window has keyboard focus. Fails closed: never guesses. */
  async function keyboardOwner() {
    await probeSession();
    if (session !== "x11") throw ownerUnknown("Linux Wayland does not expose the focused window to Codewhale, so keyboard input is refused rather than sent to an unknown window; use an X11 session");
    const windows = await x11Windows();
    if (!windows) throw ownerUnknown("the window manager does not publish its stacking order, so Codewhale cannot tell which app would receive keys");
    const active = normWindowId((await softRun("xdotool", ["getactivewindow"])).trim());
    const win = active ? windows.find((w) => w.id === active) : null;
    if (!win) throw ownerUnknown("no managed application window has keyboard focus; click the target app's window first, then retry");
    return identityOf(win);
  }

  /** The app whose window is topmost under a screen point, or null when the point is on the desktop. */
  async function pointOwner(x, y) {
    await probeSession();
    if (session !== "x11") throw ownerUnknown("Linux Wayland does not expose pointer ownership to Codewhale, so pointer input is refused; use an X11 session");
    const windows = await x11Windows();
    if (!windows) throw ownerUnknown("the window manager does not publish its stacking order, so Codewhale cannot tell which app a pointer target belongs to");
    for (let i = windows.length - 1; i >= 0; i--) {
      const win = windows[i];
      if (!win.viewable) continue;
      if (!win.frame) throw ownerUnknown(`the geometry of window "${win.title}" could not be read, so this pointer target cannot be attributed`);
      const f = win.frame;
      if (x >= f.x && y >= f.y && x < f.x + f.w && y < f.y + f.h) return identityOf(win);
    }
    return null;
  }

  async function cursorPoint() {
    await probeSession();
    const out = await xdotool(["getmouselocation"]);
    const m = /x:(-?\d+)\s+y:(-?\d+)/.exec(out);
    if (!m) throw ownerUnknown(`could not read the pointer location (${out.slice(0, 80)})`);
    return { x: Number(m[1]), y: Number(m[2]) };
  }

  /** Geometry of an app's topmost viewable window, plus the windows stacked above it that overlap it. */
  async function appFrame(name) {
    const windows = await x11Windows();
    if (!windows) throw Object.assign(new ExecError("the window manager does not publish window geometry; start one, or capture the whole display"), { code: "no_window_manager" });
    const owned = windows.filter((w) => sameName(w.name, name) || sameName(w.comm, name));
    if (!owned.length) throw Object.assign(new ExecError(`no window belongs to "${name}" — call list_apps and check the name`), { code: "app_not_found" });
    const win = [...owned].reverse().find((w) => w.viewable && w.frame);
    if (!win) throw Object.assign(new ExecError(`"${name}" has no visible window to capture (it may be minimized)`), { code: "no_window" });
    const top = windows.indexOf(win);
    const f = win.frame;
    const occludedBy = windows.slice(top + 1)
      .filter((w) => w.viewable && w.frame && w.frame.x < f.x + f.w && f.x < w.frame.x + w.frame.w && w.frame.y < f.y + f.h && f.y < w.frame.y + w.frame.h)
      .map((w) => ({ name: w.name, title: w.title }));
    return { win, rect: [f.x, f.y, f.w, f.h], occludedBy };
  }

  /** Refuse keystrokes when an open menu would take them, or when the element's window is not the active one. */
  async function assertKeyboardReady({ target, allowEscape = false } = {}) {
    if (session !== "x11") return;
    if (target?.app_ref?.name) {
      const owner = await keyboardOwner();
      if (!sameName(owner.name, target.app_ref.name) && !sameName(owner.comm, target.app_ref.name)) {
        throw Object.assign(new ExecError(`the keystrokes would go to "${owner.name}", not "${target.app_ref.name}" — the element is focused but its window is not the active one. Bring the app forward (open_application activate:true needs foreground consent) or ask the user to focus it, then retry.`), { code: "window_not_active" });
      }
    }
    if (allowEscape || !tools.pyatspi) return;
    // Without an owner the server already refused this keystroke, so there
    // is no menu to probe for.
    const owner = await keyboardOwner().catch(() => null);
    if (!owner) return;
    const r = await run("python3", ["-c", PYATSPI_POPUP, owner.name], { timeoutMs: 10_000 });
    throwIfAborted();
    const out = tryJson(r.stdout.trim().split("\n").pop() ?? "", null);
    if (out?.popup) {
      throw Object.assign(new ExecError(`a ${out.popup.role}${out.popup.label ? ` "${out.popup.label}"` : ""} is open in ${owner.name}, and keystrokes would go into it. Send key Escape (allowed while a menu is open), observe the app, then retry.`), { code: "popup_open" });
    }
  }

  async function shotTool() {
    if (session === "wayland") { need("grim", "screenshots on Wayland"); return { cmd: "grim", base: [] }; }
    if (session === "x11") {
      if (tools.scrot) return { cmd: "scrot", base: ["-z"] };
      need("import", "screenshots on X11 (imagemagick)");
      return { cmd: "import", base: ["-window", "root"] };
    }
    throw new ExecError("no X11 ($DISPLAY) or Wayland ($WAYLAND_DISPLAY) session visible to this process");
  }

  /** Capture a PNG to `file`, optionally cropped to region [x,y,w,h] points. */
  async function takeShot(file, region) {
    outputPath(file);
    const { cmd, base } = await shotTool();
    let args = [...base];
    if (cmd === "grim") {
      if (region) args.push("-g", `${Math.round(region[0])},${Math.round(region[1])} ${Math.round(region[2])}x${Math.round(region[3])}`);
      args.push(file);
    } else if (cmd === "scrot") {
      if (region) args.push("-a", `${Math.round(region[0])},${Math.round(region[1])},${Math.round(region[2])},${Math.round(region[3])}`);
      args.push(file);
    } else {
      if (region) args.push("-crop", `${Math.round(region[2])}x${Math.round(region[3])}+${Math.round(region[0])}+${Math.round(region[1])}`);
      args.push(file);
    }
    const r = await run(cmd, args, { timeoutMs: 10_000 });
    if (r.code !== 0) throw new ExecError(`${cmd} exited ${r.code}: ${r.stderr.trim().slice(0, 300)}`, r);
  }

  async function xdotool(args, opts = {}) {
    need("xdotool", "input on X11");
    throwIfAborted();
    const r = await run("xdotool", args, opts);
    throwIfAborted();
    if (r.code !== 0) throw new ExecError(`xdotool ${args[0]} exited ${r.code}: ${r.stderr.trim().slice(0, 200)}`, r);
    return r.stdout.trim();
  }

  async function ydotool(args, opts = {}) {
    need("ydotool", "input on Wayland (ydotool needs its daemon running: sudo ydotoold)");
    throwIfAborted();
    const r = await run("ydotool", args, opts);
    throwIfAborted();
    if (r.code !== 0) throw new ExecError(`ydotool exited ${r.code}: ${r.stderr.trim().slice(0, 200)}`, r);
    return r.stdout.trim();
  }

  function xdotoolKey(text) {
    return String(text).split("+").map((p) => {
      const k = p.trim().toLowerCase();
      if (XKEYS[k]) return XKEYS[k];
      if (/^f\d{1,2}$/.test(k)) return k.toUpperCase();
      return p.trim(); // pass through names already in xdotool form
    }).join("+");
  }

  async function waylandKey(text, { repeat = 1, holdMs = 0 } = {}) {
    need("wtype", "key presses on Wayland");
    const parts = String(text).split("+").map((part) => part.trim().toLowerCase());
    const aliases = { control: "ctrl", meta: "logo", cmd: "logo", super: "logo" };
    const modifiers = new Set(["ctrl", "alt", "shift", "logo", "win", "altgr", "capslock"]);
    const rawKey = parts.pop();
    const key = xdotoolKey(rawKey);
    const mods = parts.map((part) => aliases[part] ?? part);
    if (!key || mods.some((mod) => !modifiers.has(mod))) throw new ExecError(`unknown key combination "${text}"`);
    const modKey = aliases[rawKey] ?? rawKey;
    const onlyModifier = modifiers.has(modKey);
    const args = mods.flatMap((mod) => ["-M", mod]);
    for (let i = 0; i < repeat; i++) {
      args.push(onlyModifier ? "-M" : "-P", onlyModifier ? modKey : key);
      if (holdMs) args.push("-s", String(holdMs));
      args.push(onlyModifier ? "-m" : "-p", onlyModifier ? modKey : key);
    }
    args.push(...mods.reverse().flatMap((mod) => ["-m", mod]));
    // wtype owns a temporary Wayland keyboard; the compositor releases its
    // keys on process exit, including cancellation. Keep the complete gesture
    // in one process (https://github.com/atx/wtype#usage).
    throwIfAborted();
    const result = await run("wtype", args, { timeoutMs: Math.max(10_000, holdMs + 8_000) });
    throwIfAborted();
    if (result.code !== 0) throw new ExecError(`wtype exited ${result.code}: ${result.stderr.trim().slice(0, 200)}`, result);
  }

  function assertNum(v, name) {
    const n = Number(v);
    if (!Number.isFinite(n)) throw new ExecError(`${name} must be a finite number`);
    return n;
  }

  // ---------- AT-SPI tree ----------
  const PYATSPI_APP = `def resolve_app(desktop, app_name):
    apps = [desktop.getChildAtIndex(i) for i in range(desktop.childCount)]
    if app_name:
        matches = [app for app in apps if app and app_name.casefold() == (app.name or "").casefold()]
        return matches[0] if len(matches) == 1 else None
    return next((app for app in apps if app and app.childCount), None)
`;
  // The popup probe looks for menu items and popup menus that are showing.
  // Menu-bar entries stay "showing" while closed, so only items count.
  const PYATSPI_POPUP = `import json, sys, pyatspi
${PYATSPI_APP}
desktop = pyatspi.Registry.getDesktop(0)
root = resolve_app(desktop, sys.argv[1] if len(sys.argv) > 1 else None)
if root is None:
    print(json.dumps({"found": False, "popup": None}))
    sys.exit(0)
popup = None
seen = 0
queue = [(root, 0)]
while queue and popup is None and seen < 1000:
    node, depth = queue.pop(0)
    if depth > 8:
        continue
    try:
        count = node.childCount
    except Exception:
        continue
    for i in range(count):
        try:
            child = node.getChildAtIndex(i)
        except Exception:
            continue
        if child is None:
            continue
        seen += 1
        role = child.getRoleName() or ""
        if role in ("menu item", "popup menu"):
            try:
                showing = child.getState().contains(pyatspi.STATE_SHOWING)
            except Exception:
                showing = False
            if showing:
                popup = {"role": role, "label": (child.name or "").strip() or None}
                break
        queue.append((child, depth + 1))
print(json.dumps({"found": True, "popup": popup, "visited": seen}))`;

  const PYATSPI_WALK = `import json, sys, pyatspi
${PYATSPI_APP}
app_name = sys.argv[1] if len(sys.argv) > 1 else None
depth_max = int(sys.argv[2]) if len(sys.argv) > 2 else 8
max_el = int(sys.argv[3]) if len(sys.argv) > 3 else 400
desktop = pyatspi.Registry.getDesktop(0)
root = resolve_app(desktop, app_name)
if root is None:
    print(json.dumps({"found": False}))
    sys.exit(0)
els = []
truncated = False
def info(e, path):
    ext = None
    try: ext = e.queryComponent().getExtents(pyatspi.DESKTOP_COORDS)
    except Exception: pass
    txt = None
    try:
        q = e.queryText()
        n = min(120, q.characterCount)
        if n > 0: txt = q.getText(0, n)
    except Exception: pass
    acts = []
    try:
        a = e.queryAction()
        acts = [a.getName(i) for i in range(a.nActions)]
    except Exception: pass
    els.append({"index": len(els), "path": path, "role": e.getRoleName() if e.getRoleName() else None,
                "label": e.name or None, "value": txt,
                "position": {"x": ext.x, "y": ext.y} if ext else None,
                "size": {"w": ext.width, "h": ext.height} if ext else None,
                "actions": acts})
def walk(e, path, d):
    global truncated
    if len(els) >= max_el or d > depth_max:
        truncated = True
        return
    try: info(e, path)
    except Exception: return
    for i in range(e.childCount):
        try: c = e.getChildAtIndex(i)
        except Exception: continue
        if c: walk(c, path + [i], d + 1)
walk(root, [], 0)
print(json.dumps({"found": True, "name": root.name, "elements": els, "truncated": truncated}))`;

  async function atspiResolve(target, pythonBody, extraArg = null) {
    const name = appName(target.app_ref);
    assertAppRootWindow(target.windowIndex, target.window_id);
    await probeSession();
    need("python3", "semantic element actions (AT-SPI)");
    const script = `import json, sys, pyatspi
${PYATSPI_APP}
desktop = pyatspi.Registry.getDesktop(0)
app_name = sys.argv[1] if len(sys.argv) > 1 and sys.argv[1] else None
target_path = json.loads(sys.argv[2])
extra = sys.argv[3] if len(sys.argv) > 3 else None
node = resolve_app(desktop, app_name)
if node is None:
    print(json.dumps({"ok": False, "code": "app_not_found"}))
    sys.exit(0)
found = None
stack = [(node, [])]
while stack:
    n, p = stack.pop(0)
    if p == target_path:
        found = n
        break
    if len(p) > 12: continue
    try:
        for i in range(n.childCount):
            c = n.getChildAtIndex(i)
            if c: stack.append((c, p + [i]))
    except Exception: pass
if found is None:
    print(json.dumps({"ok": False, "code": "element_stale"}))
    sys.exit(0)
try:
${pythonBody}
except Exception as e:
    print(json.dumps({"ok": False, "code": str(e)}))`;
    const argv = ["-c", script, name, JSON.stringify(target.path ?? [])];
    if (extraArg != null) argv.push(String(extraArg));
    const r = await run("python3", argv, { timeoutMs: 30_000 });
    const out = tryJson((r.stdout.trim().split("\n").pop() ?? ""), null);
    if (!out) throw new ExecError(`AT-SPI action failed: ${(r.stderr || r.stdout).slice(0, 250)}`, r);
    return out;
  }

  // ---------- input helpers ----------
  function clickButton(button, clicks) {
    if (session === "x11") {
      const args = ["click"];
      if (clicks > 1) args.push("--repeat", String(clicks), "--delay", "80");
      args.push(String(button));
      return xdotool(args);
    }
    // ydotool click mask: down|up|count nibble (0xC0 = left click once, +1 per extra click;
    // 0x04 bit selects right button, 0x02 middle).
    const count = Math.max(1, Math.min(3, clicks));
    const code = button === 3 ? 0xc0 + count + 0x04 : button === 2 ? 0xc0 + count + 0x02 : 0xc0 + count - 1;
    return ydotool(["click", "0x" + code.toString(16)]);
  }

  return {
    platform: "linux",
    releaseInput,
    browser_start: browser.start,
    browser_status: browser.status,
    browser_navigate: browser.navigate,
    browser_click: browser.click,
    browser_type: browser.type,
    browser_screenshot: browser.screenshot,
    browser_stop: browser.stop,
    closeSession: async () => {
      await Promise.all([...recordings.values()].map((entry) => stopRecording(entry, 3_000)));
      recordings.clear();
      await browser.close().catch(() => {});
    },
    probe: async () => {
      const s = await probeSession();
      const caps = {
        screenshot: !!((session === "wayland" && tools.grim) || (session === "x11" && (tools.scrot || tools.import))),
        clipboard: !!(tools.xclip || tools.xsel || (tools["wl-copy"] && tools["wl-paste"])),
        recording: session === "x11" && !!tools.ffmpeg,
        accessibility_tree: tools.pyatspi,
        held_input: exec?.persistentInputOwner === true && !!(session === "x11" ? tools.xdotool : tools.wtype && tools.ydotool),
      };
      const missing = [];
      if (exec?.persistentInputOwner !== true) missing.push("connected Computer Use desktop helper (held keys, held buttons and drag)");
      if (session === "x11" && !tools.xdotool) missing.push("xdotool (input)");
      if (session === "wayland" && !tools.ydotool) missing.push("ydotool+ydotoold (mouse input)");
      if (session === "wayland" && !tools.grim) missing.push("grim (screenshots)");
      if (session === "x11" && !tools.scrot && !tools.import) missing.push("scrot or imagemagick (screenshots)");
      if (!(session === "x11" && tools.ffmpeg)) missing.push("ffmpeg with x11grab (screen recording; X11 only)");
      if (!tools.pyatspi) missing.push("python3-pyatspi (accessibility tree)");
      // Real permission probes, not just `have()`: each check is bounded to 10s.
      const permissions = { input: "failed", screen_capture: "failed", accessibility: "unavailable" };
      if (session === "x11" && tools.xdotool) {
        const r = await run("xdotool", ["getdisplaygeometry"], { timeoutMs: 10_000 });
        permissions.input = r.code === 0 ? "ok" : "failed";
      } else if (session === "wayland" && tools.ydotool) {
        permissions.input = "unavailable"; // ydotool can't be probed without moving the pointer
      }
      try {
        const dir = fs.mkdtempSync(path.join(os.tmpdir(), "cu-probe-"));
        try {
          await takeShot(path.join(dir, "probe.png"), [0, 0, 2, 2]);
          permissions.screen_capture = "ok";
        } finally {
          try { fs.rmSync(dir, { recursive: true, force: true }); } catch {}
        }
      } catch {}
      if (tools.pyatspi) {
        const r = await run("python3", ["-c", "import pyatspi; pyatspi.Registry.getDesktop(0).childCount"], { timeoutMs: 10_000 });
        permissions.accessibility = r.code === 0 ? "ok" : "failed";
      }
      if (permissions.screen_capture === "failed" || permissions.input === "failed") {
        const bad = [];
        if (permissions.input === "failed") bad.push(`input (${session === "wayland" ? "ydotool" : "xdotool getdisplaygeometry"})`);
        if (permissions.screen_capture === "failed") bad.push(`screen_capture (${session === "wayland" ? "grim" : "scrot/import"} probe shot)`);
        const e = new ExecError(`permission checks failed: ${bad.join("; ")} — permissions ${JSON.stringify(permissions)}`);
        e.code = "permissions_denied";
        throw e;
      }
      return { platform: "linux", session: s, capabilities: caps, permissions, missing, note: "Every capability probes at call time and fails closed naming the missing tool." };
    },
    list_displays: async () => {
      await probeSession();
      if (session === "x11" && tools.xrandr) {
        const r = await runOk("xrandr", ["--query"], { timeoutMs: 15_000 });
        const displays = [];
        let i = 1;
        for (const m of r.stdout.matchAll(/^(\S+) connected (?:primary )?(\d+)x(\d+)\+(\d+)\+(\d+)/gm)) {
          displays.push({ index: i++, name: m[1], points: { x: Number(m[4]), y: Number(m[5]), w: Number(m[2]), h: Number(m[3]) }, pixels: { w: Number(m[2]), h: Number(m[3]) }, scale: 1, main: /primary/.test(m[0]) || i === 1 });
        }
        if (displays.length) return displays;
      }
      if (session === "wayland" && tools.swaymsg) {
        const r = await run("swaymsg", ["-t", "get_outputs", "-r"], { timeoutMs: 15_000 });
        const outs = tryJson(r.stdout, []);
        if (Array.isArray(outs) && outs.length) {
          return outs.map((o, i) => ({ index: i + 1, name: o.name, points: { x: o.rect?.x, y: o.rect?.y, w: o.rect?.width, h: o.rect?.height }, pixels: { w: o.current_mode?.width, h: o.current_mode?.height }, scale: o.scale ?? 1, main: i === 0 }));
        }
      }
      if (session === "wayland" && tools.hyprctl) {
        const r = await run("hyprctl", ["-j", "monitors"], { timeoutMs: 15_000 });
        const ms = tryJson(r.stdout, []);
        if (Array.isArray(ms) && ms.length) {
          return ms.map((o, i) => ({ index: i + 1, name: o.name, points: { x: o.x, y: o.y, w: o.width, h: o.height }, pixels: { w: o.width, h: o.height }, scale: o.scale ?? 1, main: !!o.main || i === 0 }));
        }
      }
      throw new ExecError("display enumeration needs xrandr (X11) or swaymsg/hyprctl (Wayland) — install one and retry");
    },
    switch_display: async ({ index }) => ({ activeDisplay: index ?? 1, note: "linux screenshots grab the compositor's virtual screen; per-display selection applies only where the shot tool supports it" }),
    list_apps: async () => {
      await probeSession();
      if (session === "x11" && tools.wmctrl) {
        // One entry per process: the pid is what lets a consent decision or
        // kill_app name exactly one of several instances of the same program.
        const seen = new Map();
        for (const w of await wmctrlWindows()) {
          const key = `${w.pid}|${w.wm_class}`;
          const entry = seen.get(key) ?? { name: w.name, wm_class: w.wm_class, pid: w.pid, process: procComm(w.pid), windows: 0 };
          entry.windows++;
          seen.set(key, entry);
        }
        return { apps: [...seen.values()] };
      }
      if (session === "wayland" && (tools.swaymsg || tools.hyprctl)) {
        const w = await this.list_windows();
        const seen = new Map();
        for (const win of w.windows) {
          const cls = win.wm_class || win.app_id;
          if (cls) seen.set(cls, { name: cls, wm_class: cls });
        }
        return { apps: [...seen.values()] };
      }
      throw new ExecError("list_apps needs wmctrl (X11) or swaymsg/hyprctl (Wayland)");
    },
    list_windows: async (args = {}) => {
      rejectWindowSelectors(args);
      const app = Object.hasOwn(args, "app_ref") ? appName(args.app_ref) : null;
      await probeSession();
      if (session === "x11" && tools.wmctrl) {
        // Top-most first. Frames include decorations, the same rectangles the
        // screenshot and recording paths crop to.
        const all = (await x11Windows()) ?? (await wmctrlWindows()).map((w) => ({ ...w, frame: null, viewable: null, comm: procComm(w.pid) }));
        const windows = all.filter((w) => !app || sameName(w.name, app) || sameName(w.comm, app)).reverse().map((w) => ({
          id: w.id, title: w.title, wm_class: w.wm_class, pid: w.pid, process: w.comm ?? null,
          position: w.frame ? { x: w.frame.x, y: w.frame.y } : null,
          size: w.frame ? { w: w.frame.w, h: w.frame.h } : null,
          viewable: w.viewable,
        }));
        return { windows };
      }
      if (session === "wayland" && tools.swaymsg) {
        const r = await run("swaymsg", ["-t", "get_tree", "-r"], { timeoutMs: 15_000 });
        const windows = [];
        const walk = (n) => {
          if (n.type === "con" && n.name) windows.push({ id: String(n.id), title: n.name, wm_class: n.app_id ?? null, position: { x: n.rect?.x, y: n.rect?.y }, size: { w: n.rect?.width, h: n.rect?.height }, focused: !!n.focused });
          (n.nodes ?? []).forEach(walk);
          (n.floating_nodes ?? []).forEach(walk);
        };
        walk(tryJson(r.stdout, {}));
        return { windows };
      }
      if (session === "wayland" && tools.hyprctl) {
        const r = await run("hyprctl", ["-j", "clients"], { timeoutMs: 15_000 });
        const clients = tryJson(r.stdout, []);
        return { windows: clients.map((c) => ({ id: String(c.address), title: c.title, wm_class: c.class, position: { x: c.at?.[0], y: c.at?.[1] }, size: { w: c.size?.[0], h: c.size?.[1] }, focused: !!c.focused })) };
      }
      throw new ExecError("list_windows needs wmctrl (X11), swaymsg (sway) or hyprctl (hyprland)");
    },
    open_application: async ({ name, bundle_id: bid, url: urlArg, activate } = {}) => {
      const target = name ?? bid;
      if (!target || !/^[A-Za-z0-9][A-Za-z0-9 ._-]*$/.test(target)) throw new ExecError("open_application needs a plain executable/desktop name");
      // activate defaults to background: on X11 a new window grabs focus, so
      // remember the active window and hand focus back after the launch.
      let prevWindow = null;
      if (activate !== true) {
        try {
          await probeSession();
          if (session === "x11" && tools.xdotool) {
            const active = await run("xdotool", ["getactivewindow"], { timeoutMs: 3_000 });
            if (active.code === 0 && /^\d+$/.test(active.stdout.trim())) prevWindow = active.stdout.trim();
          }
        } catch { /* no session/tools — the launch itself is still fine */ }
      }
      spawnDetached(target, urlArg ? [urlArg] : [], "", true);
      await new Promise((r) => setTimeout(r, 500));
      let focusRestored = false;
      if (prevWindow) {
        try {
          focusRestored = (await run("xdotool", ["windowactivate", prevWindow], { timeoutMs: 3_000 })).code === 0;
        } catch { /* best-effort */ }
      }
      return { launched: true, name: target, url: urlArg ?? null, activate: activate === true, ...(activate === true ? {} : { focus_restored: focusRestored }) };
    },
    get_app_state: async ({ app_ref, window_id } = {}) => {
      const name = appName(app_ref);
      if (window_id !== undefined) throw Object.assign(new ExecError("Linux app-state window_id selection is unavailable"), { code: "unsupported_selector" });
      const t = await run("python3", ["-c", PYATSPI_WALK, name, "10", "500"], { timeoutMs: 45_000 }).then((r) =>
        tryJson((r.stdout.trim().split("\n").pop() ?? ""), null));
      if (!t) throw new ExecError("AT-SPI walk failed — is python3-pyatspi installed and the desktop running an accessibility bus (AT_SPI_BUS)?");
      if (!t.found) throw new ExecError("application not found or name is ambiguous in the AT-SPI tree — use a unique exact app_ref.name");
      return t;
    },
    screenshot: async (args = {}) => {
      rejectWindowSelectors(args);
      const appRef = Object.hasOwn(args, "app_ref") ? appName(args.app_ref) : null;
      const { display, region } = args;
      const outPath = recordingsOutputPath(args.path);
      await probeSession();
      const dir = recordingsDir();
      fs.mkdirSync(dir, { recursive: true });
      const file = outPath ?? path.join(dir, `shot-${new Date().toISOString().replace(/[:.]/g, "-")}-${crypto.randomBytes(3).toString("hex")}.png`);
      // app_ref crops to the app's window frame. The crop is the screen
      // there, so anything stacked above the window appears in it; the
      // receipt names those windows rather than pretending they are absent.
      let shotRegion = region;
      let windowInfo = null;
      let occludedBy = null;
      if (appRef !== null) {
        if (region) throw Object.assign(new ExecError("choose app_ref or region, not both"), { code: "bad_args" });
        const frame = await appFrame(appRef);
        shotRegion = frame.rect;
        windowInfo = { id: frame.win.id, title: frame.win.title, wm_class: frame.win.wm_class, pid: frame.win.pid };
        occludedBy = frame.occludedBy;
      }
      await takeShot(file, shotRegion);
      const dims = pngSize(file);
      lastRaster = {
        file,
        bytes: fs.statSync(file).size,
        // Region rasters describe the region; full shots get geometry from the
        // PNG itself (Linux shots are always scale 1: points == pixels).
        points: shotRegion ? { x: shotRegion[0], y: shotRegion[1], w: shotRegion[2], h: shotRegion[3] } : dims ? { x: 0, y: 0, w: dims.w, h: dims.h } : null,
        pixels: dims ?? (shotRegion ? { w: Math.round(shotRegion[2]), h: Math.round(shotRegion[3]) } : null),
        scale: 1,
        capturedAt: new Date().toISOString(),
      };
      return { ...lastRaster, ...(windowInfo ? { window: windowInfo, occluded_by: occludedBy, occlusion_note: occludedBy.length ? "windows listed in occluded_by are stacked above this window and appear in the crop" : null } : {}) };
    },
    resolve_element: async ({ app_ref, windowIndex, window_id, path: pathArr } = {}) => {
      const name = appName(app_ref);
      assertAppRootWindow(windowIndex, window_id);
      await probeSession();
      need("python3", "element resolution (AT-SPI)");
      const script = `import json, sys, pyatspi
${PYATSPI_APP}
desktop = pyatspi.Registry.getDesktop(0)
app_name = sys.argv[1] if len(sys.argv) > 1 and sys.argv[1] else None
target_path = json.loads(sys.argv[2])
root = resolve_app(desktop, app_name)
if root is None:
    print(json.dumps({"found": False, "element": None, "reason": "app_not_found"}))
    sys.exit(0)
node = root
ok = True
for k in target_path:
    found = None
    try:
        if k < node.childCount:
            found = node.getChildAtIndex(k)
    except Exception:
        found = None
    if found is None:
        ok = False
        break
    node = found
if not ok:
    print(json.dumps({"found": True, "element": None, "reason": "element_stale"}))
    sys.exit(0)
ext = None
try: ext = node.queryComponent().getExtents(pyatspi.DESKTOP_COORDS)
except Exception: pass
print(json.dumps({"found": True, "reason": None, "element": {
    "role": node.getRoleName() or None, "label": node.name or None,
    "position": {"x": ext.x, "y": ext.y} if ext else None,
    "size": {"w": ext.width, "h": ext.height} if ext else None}}))`;
      const r = await run("python3", ["-c", script, name, JSON.stringify(pathArr ?? [])], { timeoutMs: 30_000 });
      const out = tryJson(r.stdout.trim().split("\n").pop() ?? "", null);
      if (!out) throw new ExecError(`AT-SPI resolve failed: ${(r.stderr || r.stdout).slice(0, 250)}`, r);
      return out;
    },
    // Always crops the last raster this backend captured; a caller-named
    // source file is not accepted.
    zoom: async ({ region, path: outPath }) => {
      // Validate the caller's output path before anything else runs.
      const explicitOut = recordingsOutputPath(outPath);
      need("ffmpeg", "zoom/crop");
      const src = lastRaster?.file;
      if (!src) throw new ExecError("no screenshot taken yet on this computer — call screenshot first");
      const out = outputPath(explicitOut ?? path.join(recordingsDir(), `zoom-${crypto.randomBytes(4).toString("hex")}.png`));
      const cropped = await run("ffmpeg", ["-y", "-loglevel", "error", "-i", src, "-vf", `crop=${Math.round(region[2])}:${Math.round(region[3])}:${Math.round(region[0])}:${Math.round(region[1])}`, out], { timeoutMs: 20_000 });
      if (cropped.aborted) throw Object.assign(new ExecError("computer request cancelled", cropped), { code: "cancelled" });
      if (cropped.timedOut) throw new ExecError("timeout after 20000ms: ffmpeg", cropped);
      if (cropped.code !== 0) throw new ExecError(`ffmpeg exited ${cropped.code}: ${(cropped.stderr || cropped.stdout || "").trim().slice(0, 300)}`, cropped);
      const parent = lastRaster;
      const [x, y, w, h] = region.map(Math.round);
      lastRaster = { file: out, bytes: fs.statSync(out).size, region, source: src,
        points: { x: (parent.points?.x ?? 0) + x / parent.scale, y: (parent.points?.y ?? 0) + y / parent.scale, w: w / parent.scale, h: h / parent.scale },
        pixels: { w, h }, scale: parent.scale, capturedAt: new Date().toISOString() };
      return { ...lastRaster };
    },
    left_click: ({ target, strategy }) => { assertNum(target.x, "x"); assertNum(target.y, "y"); assertEventStrategy(strategy); return inputChain(target.x, target.y, () => clickButton(1, 1)); },
    double_click: ({ target }) => inputChain(target.x, target.y, () => clickButton(1, 2)),
    triple_click: ({ target }) => inputChain(target.x, target.y, () => clickButton(1, 3)),
    right_click: ({ target }) => inputChain(target.x, target.y, () => clickButton(3, 1)),
    middle_click: ({ target }) => inputChain(target.x, target.y, () => clickButton(2, 1)),
    mouse_move: ({ target }) => inputMove(target.x, target.y),
    left_click_drag: async ({ from_target: from, to }) => {
      requireInputOwner();
      await inputMove(from.x, from.y);
      throwIfAborted();
      mouseHeld = true;
      try {
        if (session === "x11") await xdotool(["mousedown", "1"]);
        else await ydotool(["click", "0x40"]);
        for (let i = 1; i <= 10; i++) {
          await wait(20);
          await inputMove(from.x + ((to.x - from.x) * i) / 10, from.y + ((to.y - from.y) * i) / 10);
        }
      } finally { await releaseMouse(); }
      return { action_sent: true, from, to };
    },
    left_mouse_down: async ({ target } = {}) => {
      requireInputOwner();
      await probeSession();
      if (target) await inputMove(target.x, target.y);
      throwIfAborted();
      mouseHeld = true;
      try {
        if (session === "x11") await xdotool(["mousedown", "1"]);
        else await ydotool(["click", "0x40"]);
      } catch (err) { await releaseMouse(); throw err; }
      return { action_sent: true };
    },
    left_mouse_up: async () => {
      if (!mouseHeld) throw Object.assign(new ExecError("no agent pointer press to release"), { code: "input_not_held" });
      await releaseMouse();
      return { action_sent: true };
    },
    scroll: async ({ target, direction = "down", amount = 3 }) => {
      await inputMove(target.x, target.y);
      if (session === "x11") {
        const buttons = { down: 5, up: 4, right: 7, left: 6 };
        await xdotool(["click", "--repeat", String(Math.max(1, Math.min(30, amount))), "--delay", "60", String(buttons[direction] ?? 5)]);
        return { action_sent: true, direction, amount };
      }
      // Wayland: synthesize wheel via ydotool is not wired in this build — honest refusal.
      throw new ExecError('scroll on Wayland is not available in this build; use swipe-style drags or run an X11/XWayland window. (Roadmap: ydotool wheel events.)');
    },
    type: async ({ text, target }) => {
      if (!text) return { action_sent: false, note: "empty text" };
      await probeSession();
      await assertKeyboardReady({ target });
      if (session === "x11") {
        // xdotool `type` remaps a spare keycode for characters absent from the
        // current keymap. Two failure modes follow: a cased letter produces a
        // single-symbol key whose XKB level 0 is the lowercase form (Ü → ü),
        // and consecutive remaps inside one `type` call race the X server's
        // keymap-change propagation, so non-ASCII chars intermittently drop or
        // arrive mangled (héllo → hllo, 日本 → 本). Route every non-ASCII char
        // through `key U<hex>` — one synchronous remap+press+restore per char —
        // adding Shift only when the char is cased-uppercase, and batch ASCII
        // runs through `type` as before.
        let runText = "";
        const chunks = [];
        for (const ch of String(text)) {
          if (ch.codePointAt(0) > 127) {
            if (runText) { chunks.push(runText); runText = ""; }
            chunks.push(ch);
          } else runText += ch;
        }
        if (runText) chunks.push(runText);
        for (const chunk of chunks) {
          // Supplementary-plane chars are one code point but length 2; test
          // the code point, not the string length.
          if (chunk.codePointAt(0) > 127) {
            const hex = chunk.codePointAt(0).toString(16).toUpperCase().padStart(4, "0");
            const shift = chunk !== chunk.toLowerCase() ? "shift+" : "";
            await xdotool(["key", `${shift}U${hex}`]);
            // Each temp remap restores the keymap as soon as the event is
            // queued; a lagging app can then read the press against the
            // restored map and drop it. A short settle narrows that window.
            // Under heavy host saturation XTEST drops remain possible — that
            // residual is documented in the suite's known_limitations.
            await new Promise((r) => setTimeout(r, 30));
          } else {
            await xdotool(["type", "--delay", "12", "--", chunk]);
          }
        }
        lastTypedAt = Date.now();
        return { action_sent: true, chars: text.length };
      }
      need("wtype", "typing on Wayland");
      const r = await run("wtype", ["--", String(text)], { timeoutMs: 15_000 });
      if (r.code !== 0) throw new ExecError(`wtype failed: ${r.stderr.slice(0, 200)}`, r);
      return { action_sent: true, chars: text.length };
    },
    key: async ({ text, repeat = 1, target }) => {
      await probeSession();
      const k = xdotoolKey(text);
      const n = Math.max(1, Math.min(100, Number(repeat) || 1));
      await assertKeyboardReady({ target, allowEscape: k === "Escape" });
      if (session === "x11") {
        // Return right after typing races GTK's async completion popups (the
        // file-chooser location bar swallows the first Return to accept a
        // completion), so give the app a moment to settle first.
        if (/^(Return|KP_Enter)$/.test(k)) await settleAfterTyping();
        throwIfAborted();
        heldKeys.add(k);
        try {
          await xdotool(["key", "--repeat", String(n), "--delay", "60", k]);
          heldKeys.delete(k);
        } finally { await releaseKey(k); }
      } else await waylandKey(text, { repeat: n });
      return { action_sent: true, key: k };
    },
    hold_key: async ({ text, duration }) => {
      requireInputOwner();
      await probeSession();
      await assertKeyboardReady({});
      const k = xdotoolKey(text);
      const d = Math.max(0.05, Math.min(30, Number(duration) || 1));
      if (session === "x11") {
        throwIfAborted();
        heldKeys.add(k);
        try {
          await xdotool(["keydown", k]);
          await wait(d * 1000);
        } finally { await releaseKey(k); }
      } else await waylandKey(text, { holdMs: Math.round(d * 1000) });
      return { action_sent: true, key: k, heldSec: d };
    },
    set_value: async ({ target, value }) => {
      // Select a supported interface before sending input. A refused write or
      // failed readback must never trigger a second, ambiguously applied edit.
      const out = await atspiResolve(target, `    state = found.getState()
    if not state.contains(pyatspi.STATE_ENABLED):
        raise RuntimeError("element_disabled")
    try:
        editor = found.queryEditableText()
    except NotImplementedError:
        editor = None
    if editor is not None:
        if not state.contains(pyatspi.STATE_EDITABLE):
            raise RuntimeError("element_read_only")
        if not editor.setTextContents(extra):
            raise RuntimeError("value_rejected")
        text = found.queryText()
        after = text.getText(0, text.characterCount)
        if after != extra:
            raise RuntimeError("value_verification_failed")
    else:
        import math
        desired = float(extra)
        if not math.isfinite(desired):
            raise RuntimeError("invalid_value")
        numeric = found.queryValue()
        numeric.currentValue = desired
        after = numeric.currentValue
        if after != desired:
            raise RuntimeError("value_verification_failed")
    print(json.dumps({"ok": True, "after": after}))`, String(value));
      if (!out.ok) throw new ExecError(`set_value failed: ${out.code}`);
      return { action_sent: true, strategy: "a11y", verified: true, after: out.after };
    },
    focus: async ({ target }) => {
      const out = await atspiResolve(target, `    if not found.getState().contains(pyatspi.STATE_FOCUSABLE):
        raise RuntimeError("element_not_focusable")
    if not found.queryComponent().grabFocus():
        raise RuntimeError("focus_refused")
    print(json.dumps({"ok": True}))`);
      if (!out.ok) throw Object.assign(new ExecError(`focus failed: ${out.code}`), { code: "focus_failed", reason: out.code });
      // Focusing an element inside a background window does not make that
      // window active. Report it honestly: keystrokes would still go to the
      // active window, so type and key refuse with this target until it is.
      const owner = await keyboardOwner().catch(() => null);
      const appName = target.app_ref?.name ?? "";
      const windowActive = !!owner && (sameName(owner.name, appName) || sameName(owner.comm, appName));
      return { action_sent: true, strategy: "a11y", focused: true, window_active: windowActive,
        ...(windowActive ? {} : { note: `the element is focused inside ${appName || "its app"}, but that window is not the active one, so keystrokes would go elsewhere` }) };
    },
    get_value: async ({ target }) => {
      const out = await atspiResolve(target, `    result = {"ok": False, "code": "no_value_interface"}
    try:
        text = found.queryText()
        total = text.characterCount
        result = {"ok": True, "kind": "text", "value": text.getText(0, min(total, 4000)), "characters": total, "truncated": total > 4000}
    except Exception:
        try:
            numeric = found.queryValue()
            result = {"ok": True, "kind": "numeric", "value": numeric.currentValue}
        except Exception:
            pass
    print(json.dumps(result))`);
      if (!out.ok) throw Object.assign(new ExecError(`get_value failed: ${out.code}`), { code: out.code === "no_value_interface" ? "no_value" : "get_value_failed" });
      return { strategy: "a11y", kind: out.kind, value: out.value, characters: out.characters ?? null, truncated: out.truncated ?? false };
    },
    select_text: async ({ target, text_range }) => {
      const range = text_range == null ? null : text_range.map((n) => Math.trunc(Number(n)));
      if (range && (range.length !== 2 || range.some((n) => !Number.isFinite(n) || n < 0))) {
        throw Object.assign(new ExecError("text_range must be [start, length] with non-negative integers"), { code: "bad_args" });
      }
      const out = await atspiResolve(target, `    text = found.queryText()
    total = text.characterCount
    rng = None if extra in (None, "null") else json.loads(extra)
    if rng is None:
        if not text.setCaretOffset(total):
            raise RuntimeError("caret_rejected")
        print(json.dumps({"ok": True, "caret": total, "characters": total}))
    else:
        start, length = rng
        if start + length > total:
            raise RuntimeError("range_out_of_bounds")
        # AT-SPI replaces an existing selection with setSelection, but a widget
        # with none (GTK's usual state) only accepts addSelection.
        if text.getNSelections() == 0:
            selected = text.addSelection(start, start + length)
        else:
            selected = text.setSelection(0, start, start + length)
        if not selected:
            raise RuntimeError("selection_rejected")
        begin, end = text.getSelection(0)
        print(json.dumps({"ok": True, "selected": {"start": begin, "end": end}, "text": text.getText(begin, end)[:500]}))`, range ? JSON.stringify(range) : "null");
      if (!out.ok) throw Object.assign(new ExecError(`select_text failed: ${out.code}`), { code: "select_text_failed", reason: out.code });
      return { action_sent: true, strategy: "a11y", ...(range ? { selected: out.selected, text: out.text } : { caret: out.caret, characters: out.characters }) };
    },
    input_owner: async ({ kind, point } = {}) => {
      if (kind === "keyboard") return keyboardOwner();
      if (kind !== "pointer") throw new ExecError('input_owner kind must be "keyboard" or "pointer"');
      await probeSession();
      const at = point ?? (await cursorPoint());
      return pointOwner(assertNum(at.x, "x"), assertNum(at.y, "y"));
    },
    list_sessions: async () => {
      const owner = await keyboardOwner().catch(() => null);
      return {
        via: "direct", count: 1,
        sessions: [{
          target: owner ? { name: owner.name, pid: owner.pid } : null,
          mode: "background", action: null, ageSec: 0,
          inputHeld: mouseHeld || heldKeys.size > 0,
        }],
      };
    },
    kill_app: async ({ name, bundle_id, pid, force = false } = {}) => {
      if (bundle_id) throw Object.assign(new ExecError("bundle_id is unavailable on Linux; kill by name or pid"), { code: "unsupported_selector" });
      if (!name && pid == null) throw Object.assign(new ExecError("kill_app needs name or pid"), { code: "bad_args" });
      await probeSession();
      const guarded = protectedPids();
      let target;
      if (pid != null) {
        target = Math.trunc(Number(pid));
        if (guarded.has(target)) throw Object.assign(new ExecError("refusing to terminate the Computer Use server or one of its parents"), { code: "bad_args" });
        if (!alive(target)) throw Object.assign(new ExecError(`no running process has pid ${target}`), { code: "app_not_found" });
      } else {
        const want = String(name).replace(/\.app$/i, "");
        const pids = new Set(processesNamed(want));
        for (const w of await wmctrlWindows()) if (sameName(w.name, want) && w.pid) pids.add(w.pid);
        const found = [...pids].filter((p) => !guarded.has(p) && alive(p));
        if (!found.length) throw Object.assign(new ExecError(`no running application matches "${want}"`), { code: "app_not_found" });
        if (found.length > 1) throw Object.assign(new ExecError(`${found.length} running processes match "${want}" (pids ${found.join(", ")}); pass pid to choose one`), { code: "ambiguous_application", pids: found });
        target = found[0];
      }
      if (force === true) {
        try { process.kill(target, "SIGKILL"); } catch { /* already gone */ }
        return { killed: await waitGone(target, 3_000), pid: target, method: "sigkill", forced: true };
      }
      const windows = (await wmctrlWindows()).filter((w) => w.pid === target);
      let method;
      if (windows.length) {
        // A window close is the app's own quit: it can ask to save, which is
        // what keeps unsaved work safe.
        for (const w of windows) await softRun("wmctrl", ["-i", "-c", w.id]);
        method = "window_close";
      } else {
        try { process.kill(target, "SIGTERM"); } catch { /* already gone */ }
        method = "sigterm";
      }
      if (await waitGone(target, 5_000)) return { killed: true, pid: target, method, forced: false };
      return {
        killed: false, pid: target, method, forced: false, still_running: true,
        note: method === "window_close"
          ? "the app received a close request and is still running; it may be asking whether to save. Ask the user; retry with force:true only if unsaved work can be discarded."
          : "the process ignored SIGTERM; retry with force:true to terminate it",
      };
    },
    perform_action: async ({ target, action }) => {
      const body = `    a = found.queryAction()
    names = [a.getName(i) for i in range(a.nActions)]
    want = (extra or "click").lower()
    match = next((n for n in names if n.lower() == want), None)
    if match is None and want == "click":
        match = next((n for n in names if n.lower() in ("click", "press", "activate")), None)
    if match is None:
        print(json.dumps({"ok": False, "code": "action_not_found: " + ",".join(names)}))
    else:
        a.doAction(names.index(match))
        print(json.dumps({"ok": True, "sent": True}))`;
      const out = await atspiResolve(target, body, String(action));
      if (!out.ok) throw new ExecError(`perform_action failed: ${out.code}`);
      return { action_sent: true, strategy: "a11y", action };
    },
    read_clipboard: async () => {
      await probeSession();
      const cmd = session === "x11"
        ? (tools.xclip ? ["xclip", "-selection", "clipboard", "-o"] : ["xsel", "--clipboard", "--output"])
        : ["wl-paste"];
      need(cmd[0], "clipboard read");
      const r = await run(cmd[0], cmd.slice(1), { timeoutMs: 10_000 });
      if (r.code !== 0) throw new ExecError("clipboard read failed", r);
      return { text: r.stdout, encoding: "utf8" };
    },
    write_clipboard: async ({ text }) => {
      await probeSession();
      const cmd = session === "x11"
        ? (tools.xclip ? ["xclip", "-selection", "clipboard"] : ["xsel", "--clipboard", "--input"])
        : ["wl-copy"];
      need(cmd[0], "clipboard write");
      spawnDetached(cmd[0], cmd.slice(1), String(text ?? ""), true);
      return { written: String(text ?? "").length };
    },
    cursor_position: async () => {
      await probeSession();
      if (session === "x11") {
        const out = await xdotool(["getmouselocation"]);
        const m = /x:(-?\d+)\s+y:(-?\d+)/.exec(out);
        if (!m) throw new ExecError(`could not parse xdotool getmouselocation output: ${out}`);
        return { x: Number(m[1]), y: Number(m[2]) };
      }
      throw new ExecError("cursor position needs an X11 session in this build");
    },
    // Session-owned recording: the ffmpeg child belongs to this server. It is
    // stopped by recording_stop, by session close, and by process exit.
    recordingStart: async ({ fps = 15, durationSec, region, app_ref } = {}) => {
      if (app_ref !== undefined) appName(app_ref);
      await probeSession();
      if (session !== "x11") throw Object.assign(new ExecError("Linux screen recording needs an X11 session in this build"), { code: "owned_recording_unavailable" });
      need("ffmpeg", "screen recording");
      if (region && app_ref) throw Object.assign(new ExecError("choose app_ref or region, not both"), { code: "bad_args" });
      if (durationSec != null && !(Number(durationSec) > 0)) throw Object.assign(new ExecError("durationSec must be positive"), { code: "bad_args" });
      const rate = Math.max(1, Math.min(60, Math.round(Number(fps) || 15)));
      let rect;
      let windowInfo = null;
      let occludedBy = null;
      if (app_ref !== undefined) {
        const frame = await appFrame(appName(app_ref));
        rect = frame.rect;
        windowInfo = { id: frame.win.id, title: frame.win.title, wm_class: frame.win.wm_class, pid: frame.win.pid };
        occludedBy = frame.occludedBy;
      } else if (region) {
        if (!Array.isArray(region) || region.length !== 4 || region.some((n) => !Number.isFinite(Number(n)))) throw Object.assign(new ExecError("region must be [x, y, w, h]"), { code: "bad_args" });
        rect = region.map((n) => Math.round(Number(n)));
      } else {
        rect = await displayRect();
      }
      const id = crypto.randomBytes(4).toString("hex");
      const dir = recordingsDir();
      fs.mkdirSync(dir, { recursive: true });
      const file = path.join(dir, `rec-${id}.mp4`);
      const display = process.env.DISPLAY;
      const args = x11GrabArgs({ display, rect, fps: rate, file, durationSec: durationSec == null ? null : Math.ceil(Number(durationSec)) });
      const child = spawn("ffmpeg", args, { stdio: ["pipe", "ignore", "pipe"] });
      const entry = { id, child, file, rect, fps: rate, startedAt: new Date().toISOString(), stderr: "" };
      child.stdin.on("error", () => {});
      child.stderr.on("data", (chunk) => { entry.stderr = (entry.stderr + chunk).slice(-4000); });
      const killOnExit = () => { try { if (child.exitCode == null && child.signalCode == null) child.kill("SIGKILL"); } catch { /* gone */ } };
      process.once("exit", killOnExit);
      child.once("close", () => process.off("exit", killOnExit));
      recordings.set(id, entry);
      try {
        await wait(500);
      } catch (err) {
        await stopRecording(entry, 2_000);
        recordings.delete(id);
        throw err;
      }
      if (child.exitCode != null || child.signalCode != null) {
        recordings.delete(id);
        throw Object.assign(new ExecError(`ffmpeg could not start the recording: ${entry.stderr.trim().slice(-300) || `exit ${child.exitCode}`}`), { code: "recording_failed" });
      }
      return {
        id, pid: child.pid, file, display, region: rect, durationSec: durationSec ?? null, fps: rate, mode: "x11grab", startedAt: entry.startedAt,
        ...(windowInfo ? { window: windowInfo, occluded_by: occludedBy, note: "Recording the app's window frame as it was at start; it does not follow moves or resizes." } : {}),
      };
    },
    recordingStop: async ({ id }) => {
      const entry = recordings.get(id);
      if (!entry) throw new ExecError(`unknown or already-finished recording "${id}"`);
      const result = await stopRecording(entry, 20_000);
      recordings.delete(id);
      if (result.code !== 0) throw Object.assign(new ExecError(`ffmpeg exited ${result.code}: ${result.error.slice(-300) || "partial file retained"}`), { code: "recording_failed", file: entry.file });
      const bytes = fs.existsSync(entry.file) ? fs.statSync(entry.file).size : 0;
      if (!bytes) throw new ExecError("the recorder produced no video");
      return { id, file: entry.file, bytes, mode: "x11grab", startedAt: entry.startedAt, stoppedAt: new Date().toISOString() };
    },
    recordingStatus: ({ id }) => {
      const entry = recordings.get(id);
      if (!entry) return { id, running: false };
      const running = entry.child.exitCode == null && entry.child.signalCode == null;
      return { id, running, pid: entry.child.pid, file: entry.file, bytes: fs.existsSync(entry.file) ? fs.statSync(entry.file).size : 0, startedAt: entry.startedAt };
    },
    recordingList: async () => {
      const dir = recordingsDir();
      const out = fs.existsSync(dir)
        ? fs.readdirSync(dir).filter((f) => /\.(mp4|mkv|png|jpe?g)$/i.test(f)).map((f) => {
            const st = fs.statSync(path.join(dir, f));
            return { file: path.join(dir, f), bytes: st.size, modifiedAt: st.mtime.toISOString() };
          }).sort((a, b) => b.modifiedAt.localeCompare(a.modifiedAt)).slice(0, 50)
        : [];
      return { dir, recordings: out, running: [...recordings.keys()] };
    },
  };

  /** Process state from /proc: state letter and parent pid, or null when the process is gone. */
  function procStat(pid) {
    try {
      const raw = fs.readFileSync(`/proc/${pid}/stat`, "utf8");
      const fields = raw.slice(raw.lastIndexOf(")") + 2).split(" ");
      return { state: fields[0], ppid: Number(fields[1]) };
    } catch { return null; }
  }

  /** Running, not a zombie: a process that exited but was not yet reaped is gone. */
  function alive(pid) {
    const st = procStat(pid);
    return !!st && st.state !== "Z" && st.state !== "X";
  }

  /** The server and its ancestors: kill_app must never end the process that is serving the request. */
  function protectedPids() {
    const set = new Set([0, 1, process.pid]);
    let pid = process.pid;
    for (let i = 0; i < 64; i++) {
      const st = procStat(pid);
      if (!st || st.ppid <= 0) break;
      set.add(st.ppid);
      pid = st.ppid;
    }
    return set;
  }

  /** Processes whose kernel name (comm, 15 bytes at most) equals the requested app name. */
  function processesNamed(name) {
    const want = String(name).toLowerCase().slice(0, 15);
    let entries = [];
    try { entries = fs.readdirSync("/proc"); } catch { return []; }
    return entries.filter((e) => /^\d+$/.test(e)).map(Number).filter((pid) => (procComm(pid) ?? "").toLowerCase() === want);
  }

  async function waitGone(pid, ms) {
    const until = Date.now() + ms;
    while (alive(pid)) {
      if (Date.now() >= until) return false;
      await wait(100);
    }
    return true;
  }

  async function displayRect() {
    const out = (await xdotool(["getdisplaygeometry"])).trim().split(/\s+/).map(Number);
    if (out.length !== 2 || out.some((n) => !Number.isFinite(n) || n <= 0)) throw new ExecError("could not read the display size for recording");
    return [0, 0, out[0], out[1]];
  }

  /** Stop a recorder: ask ffmpeg to finish the file ('q'), kill it if it does not. */
  function stopRecording(entry, timeoutMs) {
    return new Promise((resolve) => {
      const { child } = entry;
      if (child.exitCode != null || child.signalCode != null) {
        resolve({ code: child.exitCode, error: entry.stderr.trim() });
        return;
      }
      const killer = setTimeout(() => { try { child.kill("SIGKILL"); } catch { /* gone */ } }, timeoutMs);
      child.once("close", (code) => { clearTimeout(killer); resolve({ code, error: entry.stderr.trim() }); });
      try { child.stdin.write("q"); child.stdin.end(); } catch { /* already exiting */ }
    });
  }

  async function inputChain(x, y, act) {
    await inputMove(x, y);
    await act();
    return { action_sent: true, at: { x: Number(x), y: Number(y) } };
  }

  async function inputMove(x, y) {
    await probeSession();
    const nx = Math.round(assertNum(x, "x"));
    const ny = Math.round(assertNum(y, "y"));
    if (session === "x11") await xdotool(["mousemove", "--sync", String(nx), String(ny)]);
    else await ydotool(["moveto", String(nx), String(ny)]);
  }
}

export default { create };
