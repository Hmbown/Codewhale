// Browser control over the Chrome DevTools Protocol (CDP).
//
// A self-owned Chromium-family browser: launch (or reuse) an instance with its
// own --user-data-dir under the state dir and a loopback-only debugging port,
// then speak CDP over the browser WebSocket. The person's own browser profile
// is never attached to, never typed into, and never closed. No screen
// coordinates and no accessibility are involved: page elements are addressed
// by CSS selector through the DOM domain, and coordinate clicks are page
// viewport pixels — a different space from screen points, named differently so
// the two can never be confused.
//
// One tab per computer session; the last session out closes the shared
// browser.
//
// Attach mode (CODEWHALE_CU_BROWSER_ATTACH=/run/cw/cdp.sock, a Codewhale
// Computer): nothing is launched. The plugin connects to the CDP bridge of the
// one Chromium a person also sees on the shared display — NUL-delimited JSON
// over a Unix socket, the --remote-debugging-pipe framing — so the agent's
// navigations appear in that person's window and the tabs they open appear in
// the agent's targets. That browser is never closed and no tab is closed:
// stop only detaches. Node needs a global WebSocket (22+, or 21 with the default-on
// flag); older runtimes refuse with `unsupported_runtime` instead of
// half-working.
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import crypto from "node:crypto";
import net from "node:net";
import { spawn } from "node:child_process";
import { ExecError, currentSignal } from "./exec.mjs";
import { stateDir } from "./registry.mjs";

const APPLICATIONS = ["Google Chrome", "Chromium", "Brave Browser", "Microsoft Edge"];
const LINUX_BINARIES = ["google-chrome", "chromium", "chromium-browser", "brave-browser", "microsoft-edge"];

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const badArgs = (message) => Object.assign(new ExecError(message), { code: "bad_args" });

function defaultRecordingsDir() {
  return process.env.CODEWHALE_CU_RECORDINGS_DIR || path.join(stateDir(), "recordings");
}

/** Only http(s) and about:blank can be navigated to; everything else is refused. */
export function checkBrowserUrl(url) {
  if (typeof url !== "string" || !url.trim()) throw badArgs("browser navigate/start need a url (http:// or https:// or about:blank)");
  const trimmed = url.trim();
  if (/^about:blank$/i.test(trimmed)) return trimmed;
  let parsed;
  try { parsed = new URL(trimmed); } catch { throw badArgs(`"${trimmed}" is not a URL`); }
  if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
    throw badArgs(`only http(s):// and about:blank URLs can be opened (got "${parsed.protocol}//")`);
  }
  return parsed.href;
}

/** Locate a Chromium-family browser app or binary; CODEWHALE_CU_BROWSER_APP overrides. */
export function findBrowserApp(platform = process.platform, env = process.env, exists = fs.existsSync) {
  if (env.CODEWHALE_CU_BROWSER_APP) return env.CODEWHALE_CU_BROWSER_APP;
  if (platform === "darwin") {
    for (const name of APPLICATIONS) {
      for (const root of ["/Applications", path.join(os.homedir(), "Applications")]) {
        const candidate = path.posix.join(root, `${name}.app`);
        if (exists(candidate)) return candidate;
      }
    }
    return null;
  }
  if (platform === "win32") {
    const installs = [
      ["Google", "Chrome", "Application", "chrome.exe"],
      ["Chromium", "Application", "chrome.exe"],
      ["BraveSoftware", "Brave-Browser", "Application", "brave.exe"],
      ["Microsoft", "Edge", "Application", "msedge.exe"],
    ];
    for (const relative of installs) {
      for (const root of [env.ProgramFiles, env["ProgramFiles(x86)"], env.LOCALAPPDATA]) {
        if (!root) continue;
        const candidate = path.win32.join(root, ...relative);
        if (exists(candidate)) return candidate;
      }
    }
    return null;
  }
  for (const bin of LINUX_BINARIES) {
    for (const dir of (env.PATH ?? "").split(":")) {
      if (dir && exists(path.join(dir, bin))) return path.join(dir, bin);
    }
  }
  return null;
}

/** Launch detached so the browser is its own process, never a child we must reap. */
function defaultLaunch({ app, profileDir, url, platform = process.platform }) {
  const flags = ["--remote-debugging-port=0", `--user-data-dir=${profileDir}`, "--no-first-run", "--no-default-browser-check"];
  const target = url || "about:blank";
  let cmd, args;
  // -n (new instance) matters: without it, `open --args` is ignored whenever
  // the person already has Chrome running — the args never reach a new process.
  if (platform === "darwin") { cmd = "open"; args = ["-g", "-n", "-a", app, "--args", ...flags, target]; }
  else { cmd = app; args = [...flags, target]; }
  const child = spawn(cmd, args, { detached: true, stdio: "ignore" });
  child.on("error", () => {});
  child.unref();
}

/** The CDP bridge socket to attach to, or null for launch mode. */
export function attachSocket(env = process.env) {
  const value = env.CODEWHALE_CU_BROWSER_ATTACH;
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

/**
 * Connect to a NUL-framed CDP Unix socket and expose the small WebSocket-like
 * surface makeChannel uses (addEventListener message/close/error, send, close).
 * The bridge admits one client; a second gets `{"error":"cdp_busy"}` and EOF,
 * surfaced as `closeReason`.
 */
export function connectPipeSocket(socketPath, { timeoutMs = 8_000, createConnection = net.createConnection } = {}) {
  return new Promise((resolve, reject) => {
    const listeners = { message: [], close: [], error: [] };
    const emit = (type, event) => { for (const entry of [...listeners[type]]) { if (entry.once) listeners[type] = listeners[type].filter((e) => e !== entry); entry.fn(event); } };
    let inbuf = Buffer.alloc(0);
    let opened = false;
    let closed = false;
    const ws = {
      closeReason: null,
      addEventListener(type, fn, opts) { listeners[type]?.push({ fn, once: !!opts?.once }); },
      send(text) { if (!closed) sock.write(`${text}\0`); },
      close() { if (closed) return; closed = true; sock.destroy(); emit("close", {}); },
    };
    const sock = createConnection(socketPath);
    const timer = setTimeout(() => {
      sock.destroy();
      reject(Object.assign(new ExecError(`the CDP bridge at ${socketPath} did not accept within ${timeoutMs}ms`), { code: "browser_unavailable" }));
    }, timeoutMs);
    sock.on("connect", () => { opened = true; clearTimeout(timer); resolve(ws); });
    sock.on("data", (chunk) => {
      inbuf = Buffer.concat([inbuf, chunk]);
      let i;
      while ((i = inbuf.indexOf(0)) >= 0) {
        const text = inbuf.subarray(0, i).toString("utf8");
        inbuf = inbuf.subarray(i + 1);
        if (text.startsWith("{\"error\"")) {
          try { ws.closeReason = JSON.parse(text).error ?? ws.closeReason; } catch {}
          continue;
        }
        emit("message", { data: text });
      }
    });
    sock.on("error", (error) => {
      if (!opened) {
        clearTimeout(timer);
        const denied = error?.code === "EACCES";
        reject(Object.assign(new ExecError(denied
          ? `permission denied on the CDP bridge ${socketPath} — only the Engine's user may attach to the shared browser`
          : `cannot reach the CDP bridge at ${socketPath} (${error?.code ?? error?.message}) — is the chrome service running?`), { code: "browser_unavailable" }));
        return;
      }
      emit("error", error);
    });
    sock.on("close", () => { if (!closed) { closed = true; emit("close", {}); } });
  });
}

/**
 * Open the CDP WebSocket. Injectable: tests supply a fake ws-like object so
 * the command sequence is verifiable without a browser.
 */
function defaultConnect(url, { timeoutMs = 8_000 } = {}) {
  return new Promise((resolve, reject) => {
    if (typeof WebSocket === "undefined") {
      reject(Object.assign(new ExecError("browser actions need a Node runtime with a global WebSocket (22+); this runtime does not have one"), { code: "unsupported_runtime" }));
      return;
    }
    let ws;
    try { ws = new WebSocket(url); } catch (error) {
      reject(Object.assign(new ExecError(`cannot open a CDP socket at ${url}: ${error.message}`), { code: "browser_unavailable" }));
      return;
    }
    const timer = setTimeout(() => {
      try { ws.close(); } catch {}
      reject(Object.assign(new ExecError(`the CDP socket at ${url} did not open within ${timeoutMs}ms`), { code: "browser_unavailable" }));
    }, timeoutMs);
    ws.addEventListener("open", () => { clearTimeout(timer); resolve(ws); }, { once: true });
    ws.addEventListener("error", () => {
      clearTimeout(timer);
      reject(Object.assign(new ExecError(`the CDP socket at ${url} refused the connection`), { code: "browser_unavailable" }));
    }, { once: true });
  });
}

/** id-matched JSON-RPC over the WebSocket, plus CDP event fan-out. */
function makeChannel(ws) {
  let nextId = 0;
  const pending = new Map();
  const listeners = new Map();
  let closed = false;
  const failAll = (reason) => {
    closed = true;
    for (const [, entry] of pending) entry.reject(Object.assign(new ExecError(reason), { code: "browser_not_running" }));
    pending.clear();
  };
  ws.addEventListener("message", (event) => {
    let msg;
    try { msg = JSON.parse(typeof event.data === "string" ? event.data : String(event.data)); } catch { return; }
    if (msg.id != null && pending.has(msg.id)) {
      const entry = pending.get(msg.id);
      pending.delete(msg.id);
      if (msg.error) entry.reject(Object.assign(new ExecError(`CDP ${msg.method ?? ""} failed: ${msg.error.message}`), { code: "cdp_error", cdp: msg.error }));
      else entry.resolve(msg.result ?? {});
      return;
    }
    if (msg.method) for (const fn of listeners.get(msg.method) ?? []) fn(msg);
  });
  ws.addEventListener("close", () => failAll("the browser closed the CDP connection"));
  ws.addEventListener("error", () => {});
  return {
    get alive() { return !closed; },
    send(method, params = {}, sessionId) {
      if (closed) return Promise.reject(Object.assign(new ExecError("the browser is no longer reachable (CDP connection closed)"), { code: "browser_not_running" }));
      return new Promise((resolve, reject) => {
        const id = ++nextId;
        pending.set(id, { resolve, reject });
        ws.send(JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) }));
      });
    },
    waitFor(method, { sessionId, timeoutMs = 15_000 } = {}) {
      return new Promise((resolve, reject) => {
        const signal = currentSignal();
        const done = () => {
          clearTimeout(timer);
          signal?.removeEventListener("abort", onAbort);
          const list = listeners.get(method) ?? [];
          listeners.set(method, list.filter((fn) => fn !== handler));
        };
        const onAbort = () => { done(); reject(Object.assign(new ExecError("computer request cancelled"), { code: "cancelled" })); };
        const handler = (msg) => {
          if (sessionId && msg.sessionId !== sessionId) return;
          done();
          resolve(msg.params ?? {});
        };
        const timer = setTimeout(() => {
          done();
          reject(Object.assign(new ExecError(`timed out waiting for ${method}`), { code: "timeout" }));
        }, timeoutMs);
        if (signal?.aborted) { onAbort(); return; }
        signal?.addEventListener("abort", onAbort, { once: true });
        listeners.set(method, [...(listeners.get(method) ?? []), handler]);
      });
    },
    close() { closed = true; try { ws.close(); } catch {} },
  };
}

/**
 * The browser bridge. One instance per backend (per computer session).
 * Exposes browser_start / browser_status / browser_navigate / browser_click /
 * browser_type / browser_screenshot / browser_stop plus close() for the
 * session teardown hook.
 */
export function createBrowser({
  connect = defaultConnect,
  launch = defaultLaunch,
  findApp = findBrowserApp,
  recordingsDir = defaultRecordingsDir,
  platform = process.platform,
  attach = attachSocket(),
  connectAttach = connectPipeSocket,
} = {}) {
  const state = { channel: null, port: null, profileDir: null, app: null, targetId: null, sessionId: null, pageEnabled: false, domEnabled: false, attached: false, product: null, closeReason: null };

  const profileDir = () => path.join(stateDir(), "browser", "profile");
  const loadTimeout = () => Number(process.env.CODEWHALE_CU_BROWSER_LOAD_TIMEOUT_MS) || 15_000;
  const requireRunning = () => {
    if (!state.channel) throw Object.assign(new ExecError("no browser for this session yet — run browser {action:\"start\"} first"), { code: "browser_not_running" });
  };
  async function targetInfo(targetId) {
    const { targetInfo: info } = await state.channel.send("Target.getTargetInfo", { targetId });
    return { url: info?.url ?? "", title: info?.title ?? "" };
  }
  async function listTabs() {
    const { targetInfos } = await state.channel.send("Target.getTargets", {});
    return (targetInfos ?? []).filter((t) => t.type === "page");
  }
  async function ensureDom() {
    if (!state.domEnabled) { await state.channel.send("DOM.enable", {}, state.sessionId); state.domEnabled = true; }
  }
  async function resolveNode(selector) {
    if (typeof selector !== "string" || !selector.trim()) throw badArgs("selector must be a non-empty CSS selector");
    await ensureDom();
    const { root } = await state.channel.send("DOM.getDocument", { depth: 0 }, state.sessionId);
    const { nodeId } = await state.channel.send("DOM.querySelector", { nodeId: root.nodeId, selector }, state.sessionId);
    if (!nodeId) throw Object.assign(new ExecError(`no element on this page matches ${JSON.stringify(selector)}`), { code: "selector_not_found" });
    return nodeId;
  }
  async function viewport() {
    const metrics = await state.channel.send("Page.getLayoutMetrics", {}, state.sessionId);
    const vp = metrics.cssLayoutViewport ?? {};
    return { w: vp.clientWidth ?? 0, h: vp.clientHeight ?? 0, scale: metrics.cssVisualViewport?.scale ?? 1, metrics };
  }
  async function mousePoint(x, y) {
    for (const [type, extra] of [["mouseMoved", {}], ["mousePressed", { button: "left", clickCount: 1, buttons: 1 }], ["mouseReleased", { button: "left", clickCount: 1, buttons: 0 }]]) {
      await state.channel.send("Input.dispatchMouseEvent", { type, x, y, ...extra }, state.sessionId);
    }
  }
  async function waitLoad(timeoutMs) {
    await state.channel.send("Page.enable", {}, state.sessionId).catch(() => {});
    state.pageEnabled = true;
    return state.channel.waitFor("Page.loadEventFired", { sessionId: state.sessionId, timeoutMs });
  }
  async function bindTab(url, reused) {
    // A fresh launch already came with one about:blank tab — adopt it instead
    // of stacking a second tab that nothing will ever close. A single
    // leftover blank tab (a dead session's) is adopted too; visible tabs are
    // never stolen (an instance with real tabs gets a new tab of this
    // session's own).
    const tabs = await listTabs();
    let targetId = null;
    if (tabs.length === 1 && (!reused || tabs[0].url === "about:blank")) targetId = tabs[0].targetId;
    if (!targetId) ({ targetId } = await state.channel.send("Target.createTarget", { url: "about:blank" }));
    const { sessionId } = await state.channel.send("Target.attachToTarget", { targetId, flatten: true });
    state.targetId = targetId;
    state.sessionId = sessionId;
    state.pageEnabled = false;
    state.domEnabled = false;
    let verified = true;
    if (url && url !== "about:blank") {
      const load = waitLoad(loadTimeout());
      await state.channel.send("Page.navigate", { url }, sessionId);
      verified = await load.then(() => true).catch(() => false);
    }
    return { verified };
  }

  /**
   * Attach mode: bind to a requested tab, or adopt a lone blank tab, or open
   * a new foreground tab in the person's window — and bring it to the front so
   * what the agent does is visible. A person's open tab is only taken when
   * named explicitly with `tab`.
   */
  async function bindSharedTab(url, tab) {
    const tabs = await listTabs();
    let targetId = null;
    if (tab != null) {
      if (!tabs.some((t) => t.targetId === tab)) throw Object.assign(new ExecError(`no tab ${JSON.stringify(tab)} in the shared browser — browser {action:"status"} lists them`), { code: "bad_target" });
      targetId = tab;
    } else if (tabs.length === 1 && /^(about:blank|chrome:\/\/newtab\/?|chrome:\/\/new-tab-page\/?)$/.test(tabs[0].url ?? "")) {
      targetId = tabs[0].targetId;
    } else {
      ({ targetId } = await state.channel.send("Target.createTarget", { url: "about:blank", background: false }));
    }
    const { sessionId } = await state.channel.send("Target.attachToTarget", { targetId, flatten: true });
    await state.channel.send("Target.activateTarget", { targetId }).catch(() => {});
    state.targetId = targetId;
    state.sessionId = sessionId;
    state.pageEnabled = false;
    state.domEnabled = false;
    let verified = true;
    if (url && url !== "about:blank") {
      const load = waitLoad(loadTimeout());
      await state.channel.send("Page.navigate", { url }, sessionId);
      verified = await load.then(() => true).catch(() => false);
    }
    return { verified, adopted: tab != null || targetId !== null && tabs.some((t) => t.targetId === targetId) };
  }

  async function startAttached(target, tab) {
    const ws = await connectAttach(attach);
    state.channel = makeChannel(ws);
    state.attached = true;
    state.app = `attached:${attach}`;
    try {
      const version = await state.channel.send("Browser.getVersion", {});
      state.product = version.product ?? null;
      const { verified, adopted } = await bindSharedTab(target, tab);
      const info = await targetInfo(state.targetId);
      return {
        running: true, attached: true, launched: false, shared: true, browser: state.product, socket: attach,
        tab: { id: state.targetId, url: info.url, title: info.title }, adopted_tab: adopted, verified,
        note: "attached to the computer's shared browser: the person watching sees this tab, and tabs they open appear in browser status. Stop only detaches.",
      };
    } catch (error) {
      const reason = ws.closeReason;
      state.channel?.close();
      state.channel = null; state.attached = false; state.targetId = null; state.sessionId = null;
      if (reason === "cdp_busy") throw Object.assign(new ExecError(`the shared browser's CDP bridge (${attach}) already has a client — only one controller may attach at a time`), { code: "browser_busy" });
      throw error;
    }
  }

  const api = {
    async start({ url, tab } = {}) {
      const target = url ? checkBrowserUrl(url) : "about:blank";
      if (state.channel) {
        if (state.attached && tab != null && tab !== state.targetId) {
          await state.channel.send("Target.detachFromTarget", { sessionId: state.sessionId }).catch(() => {});
          const { verified } = await bindSharedTab(target, tab);
          return { ...(await this.status()), switched_tab: true, verified };
        }
        if (url) await this.navigate({ url: target });
        return { ...(await this.status()), already_running: true };
      }
      if (tab != null && !attach) throw badArgs("tab selects a tab of the shared browser and needs attach mode (CODEWHALE_CU_BROWSER_ATTACH)");
      if (attach) return startAttached(target, tab);
      if (typeof WebSocket === "undefined") throw Object.assign(new ExecError("browser actions need a Node runtime with a global WebSocket (22+); this runtime does not have one"), { code: "unsupported_runtime" });
      const app = findApp();
      if (!app) throw Object.assign(new ExecError(`no Chromium-family browser found (looked for ${APPLICATIONS.join(", ")}); set CODEWHALE_CU_BROWSER_APP to the app path`), { code: "browser_not_installed" });
      const dir = profileDir();
      fs.mkdirSync(dir, { recursive: true, mode: 0o700 });

      // Reuse a live instance for this profile (a previous session may not have
      // stopped it); a stale port file must never shadow a fresh launch. The
      // port file carries the browser WebSocket path, so no HTTP probe is
      // needed — a socket that opens is an instance that is alive.
      const portFile = path.join(dir, "DevToolsActivePort");
      const readPortFile = () => {
        try {
          const [port, wsPath] = fs.readFileSync(portFile, "utf8").split("\n");
          return /^\d+$/.test((port ?? "").trim()) && (wsPath ?? "").trim()
            ? { port: Number(port.trim()), wsPath: wsPath.trim() }
            : null;
        } catch { return null; }
      };
      const attempt = async (file) => {
        if (!file) return null;
        try { return await connect(`ws://127.0.0.1:${file.port}${file.wsPath}`); } catch { return null; }
      };
      let ws = null, reused = false;
      const existing = readPortFile();
      if (existing) {
        ws = await attempt(existing);
        if (ws) { reused = true; state.port = existing.port; }
        else { try { fs.rmSync(portFile, { force: true }); } catch {} }
      }
      if (!ws) {
        await launch({ app, profileDir: dir, url: "about:blank", platform });
        const deadline = Date.now() + 20_000;
        for (;;) {
          if (currentSignal()?.aborted) throw Object.assign(new ExecError("computer request cancelled"), { code: "cancelled" });
          const file = readPortFile();
          if (file) {
            ws = await attempt(file);
            if (ws) { state.port = file.port; break; }
          }
          if (Date.now() > deadline) throw Object.assign(new ExecError(`the browser started but its debugging endpoint never came up (profile ${dir}); is it running with a usable profile?`), { code: "browser_unavailable" });
          await sleep(300);
        }
      }
      state.channel = makeChannel(ws);
      state.profileDir = dir;
      state.app = app;
      const { verified } = await bindTab(target, reused);
      const info = await targetInfo(state.targetId);
      return {
        running: true, launched: !reused, reused, browser: app, profile: dir,
        tab: { id: state.targetId, url: info.url, title: info.title }, verified,
        note: "self-owned profile under the state dir; the user's own browser was not touched",
      };
    },

    async status() {
      if (!state.channel) {
        if (attach) return { running: false, attached: false, socket: attach, note: "not attached yet — browser {action:\"start\"} attaches to the computer's shared browser" };
        return { running: false, browser: state.app, profile: state.profileDir ?? profileDir(), note: "no browser session for this computer session yet — browser {action:\"start\"} launches a self-owned instance" };
      }
      try {
        const tabs = await listTabs();
        if (state.attached) {
          return {
            running: true, attached: true, shared: true, browser: state.product, socket: attach,
            tabs: tabs.map((t) => ({ id: t.targetId, title: t.title, url: t.url, agent: t.targetId === state.targetId })),
            activeTab: tabs.some((t) => t.targetId === state.targetId) ? (({ url, title }) => ({ id: state.targetId, url, title }))(await targetInfo(state.targetId)) : null,
            note: "every page tab in the shared browser, including the person's; start {tab} moves the agent to one of them",
          };
        }
        return {
          running: true, browser: state.app, port: state.port, profile: state.profileDir,
          tabs: tabs.map((t) => ({ id: t.targetId, title: t.title, url: t.url })),
          activeTab: tabs.some((t) => t.targetId === state.targetId) ? (({ url, title }) => ({ id: state.targetId, url, title }))(await targetInfo(state.targetId)) : null,
        };
      } catch (error) {
        state.channel?.close();
        state.channel = null; state.targetId = null; state.sessionId = null;
        return { running: false, note: `the browser went away: ${error.message}` };
      }
    },

    async navigate({ url } = {}) {
      requireRunning();
      const target = checkBrowserUrl(url);
      const loadPromise = target === "about:blank" ? null : waitLoad(loadTimeout());
      await state.channel.send("Page.navigate", { url: target }, state.sessionId);
      let verified = true;
      if (loadPromise) {
        try { await loadPromise; } catch (error) { verified = false; void error; }
      }
      const info = await targetInfo(state.targetId).catch(() => ({ url: target, title: "" }));
      return {
        action_sent: true, url: info.url || target, title: info.title, verified,
        ...(verified ? {} : { note: "the page did not report load completion (slow page or same-document navigation) — observe before relying on it" }),
      };
    },

    async click({ selector, point } = {}) {
      requireRunning();
      let where;
      if (typeof selector === "string" && selector.trim()) {
        const nodeId = await resolveNode(selector);
        await state.channel.send("DOM.scrollIntoViewIfNeeded", { nodeId }, state.sessionId);
        const { model } = await state.channel.send("DOM.getBoxModel", { nodeId }, state.sessionId);
        const quad = model.content;
        where = { x: Math.round((quad[0] + quad[2] + quad[4] + quad[6]) / 4), y: Math.round((quad[1] + quad[3] + quad[5] + quad[7]) / 4), selector };
      } else if (point && Number.isFinite(point.x) && Number.isFinite(point.y)) {
        const vp = await viewport();
        if (point.x < 0 || point.y < 0 || (vp.w && point.x >= vp.w) || (vp.h && point.y >= vp.h)) {
          throw Object.assign(new ExecError(`(${point.x}, ${point.y}) is outside the page viewport (${vp.w}x${vp.h} CSS px) — browser {action:\"screenshot\"} shows this space`), { code: "bad_target" });
        }
        where = { x: point.x, y: point.y };
      } else {
        throw badArgs("browser click needs selector (CSS) or point {x,y} (page viewport pixels)");
      }
      await mousePoint(where.x, where.y);
      return {
        action_sent: true, ...(where.selector ? { selector: where.selector } : {}), point: { x: where.x, y: where.y },
        pointer_moved: false, verified: false, verification_required: "screenshot or status",
        note: "clicked in the page viewport; the user's pointer never moved",
      };
    },

    async type({ text, selector, enter } = {}) {
      requireRunning();
      if (typeof text !== "string" || !text.length) throw badArgs("browser type needs text");
      let focused = null;
      if (selector != null) {
        const nodeId = await resolveNode(selector);
        await state.channel.send("DOM.focus", { nodeId }, state.sessionId);
        focused = selector;
      }
      await state.channel.send("Input.insertText", { text }, state.sessionId);
      if (enter) {
        for (const type of ["keyDown", "keyUp"]) {
          await state.channel.send("Input.dispatchKeyEvent", { type, key: "Enter", code: "Enter", windowsVirtualKeyCode: 13, nativeVirtualKeyCode: 13, ...(type === "keyDown" ? { text: "\r" } : {}) }, state.sessionId);
        }
      }
      return {
        action_sent: true, chars: text.length, ...(focused ? { selector: focused } : {}), ...(enter ? { entered: true } : {}),
        verified: false, verification_required: "screenshot or status",
        note: focused ? "text was inserted into the selector's element" : "text was inserted at the page's current focus",
      };
    },

    async screenshot({ full } = {}) {
      requireRunning();
      const vp = await viewport();
      const shot = await state.channel.send("Page.captureScreenshot", { format: "png", ...(full ? { captureBeyondViewport: true } : {}) }, state.sessionId);
      const dir = path.join(recordingsDir(), "captures");
      fs.mkdirSync(dir, { recursive: true });
      const file = path.join(dir, `browser-${crypto.randomBytes(4).toString("hex")}.png`);
      fs.writeFileSync(file, Buffer.from(shot.data, "base64"));
      return {
        file, bytes: fs.statSync(file).size, format: "png", space: "page-viewport",
        viewport: { w: vp.w, h: vp.h }, scale: vp.scale,
        note: "page pixels, not screen pixels — the same space browser click point targets use",
      };
    },

    async stop() {
      if (!state.channel) return { running: false, note: "no browser session for this computer session" };
      if (state.attached) {
        // The person's browser: never close a tab or the browser, only detach.
        if (state.sessionId) await state.channel.send("Target.detachFromTarget", { sessionId: state.sessionId }).catch(() => {});
        state.channel.close();
        state.channel = null; state.targetId = null; state.sessionId = null; state.pageEnabled = false; state.domEnabled = false; state.attached = false;
        return { running: false, detached: true, browser_closed: false, note: "detached from the shared browser; its window and tabs stay as they are" };
      }
      try { await state.channel.send("Target.closeTarget", { targetId: state.targetId }); } catch { /* the tab may already be gone */ }
      let remaining = null;
      try { remaining = (await listTabs()).length; } catch { remaining = null; }
      let browserClosed = false;
      if (remaining === 0) {
        try { await state.channel.send("Browser.close"); browserClosed = true; } catch {}
        await sleep(200);
      }
      state.channel.close();
      state.channel = null; state.targetId = null; state.sessionId = null; state.pageEnabled = false; state.domEnabled = false;
      return { running: false, closed: true, browser_closed: browserClosed,
        ...(remaining ? { note: `${remaining} tab(s) from other sessions remain open; the shared browser stays up` } : {}) };
    },

    /** Session teardown: close this session's tab; last one out closes the browser. */
    async close() {
      if (!state.channel) return;
      try { await this.stop(); } catch { state.channel?.close(); state.channel = null; }
    },
  };
  // Bound once so backends can hand the methods out individually without
  // losing `this` (start/close re-enter the api by name).
  for (const key of Object.keys(api)) api[key] = api[key].bind(api);
  return api;
}
