// Browser control over CDP, driven by a scripted fake WebSocket: the command
// sequence, receipts, refusals and file output are all verified without a real
// browser. The live path gets its own smoke on a real Chrome.
import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { createBrowser, checkBrowserUrl, findBrowserApp } from "../src/browser-cdp.mjs";

class FakeWs {
  constructor(script) { this.listeners = {}; this.sent = []; this.script = script; this.closed = false; }
  addEventListener(type, fn) { (this.listeners[type] ??= []).push(fn); }
  emit(type, event = {}) { for (const fn of this.listeners[type] ?? []) fn(event); }
  close() { if (!this.closed) { this.closed = true; queueMicrotask(() => this.emit("close", {})); } }
  send(raw) {
    const msg = JSON.parse(raw);
    this.sent.push(msg);
    queueMicrotask(() => {
      const respond = this.script[msg.method];
      const result = typeof respond === "function" ? respond(msg.params) : respond;
      if (result === undefined) return; // fire-and-forget (Input domain)
      this.emit("message", { data: JSON.stringify({ id: msg.id, ...(msg.sessionId ? { sessionId: msg.sessionId } : {}), result }) });
    });
  }
  event(method, params, sessionId) { this.emit("message", { data: JSON.stringify({ method, params, sessionId }) }); }
  sentOf(method) { return this.sent.filter((m) => m.method === method); }
}

const pageState = () => ({ url: "about:blank", tabs: ["T1"] });
function defaultScript(page) {
  return {
    "Target.createTarget": () => { const id = `T${page.tabs.length + 1}`; page.tabs.push(id); return { targetId: id }; },
    "Target.attachToTarget": { sessionId: "S1" },
    "Page.enable": {},
    "Page.navigate": (p) => { page.url = p.url; return { frameId: "F1" }; },
    "Target.getTargetInfo": () => ({ targetInfo: { url: page.url, title: `t:${page.url}` } }),
    "Target.getTargets": () => ({ targetInfos: page.tabs.map((id) => ({ type: "page", targetId: id, url: page.url, title: `t:${page.url}` })) }),
    "Target.closeTarget": (p) => { page.tabs = page.tabs.filter((id) => id !== p.targetId); return {}; },
    "Browser.close": (p, m) => (m === undefined ? {} : {}),
    "Page.getLayoutMetrics": { cssLayoutViewport: { clientWidth: 800, clientHeight: 600 }, cssVisualViewport: { scale: 2 } },
    "Page.captureScreenshot": { data: Buffer.from("abc").toString("base64") },
    "DOM.enable": {},
    "DOM.getDocument": { root: { nodeId: 1 } },
    "DOM.querySelector": { nodeId: 7 },
    "DOM.scrollIntoViewIfNeeded": {},
    "DOM.getBoxModel": { model: { content: [10, 20, 30, 20, 30, 40, 10, 40] } },
    "DOM.focus": {},
    "Input.insertText": {},
    "Input.dispatchKeyEvent": {},
    "Input.dispatchMouseEvent": {},
  };
}

function harness(t, { script, connectFailures = 0, stateDir } = {}) {
  const dir = stateDir ?? fs.mkdtempSync(path.join(os.tmpdir(), "cu-browser-"));
  const recDir = fs.mkdtempSync(path.join(os.tmpdir(), "cu-browser-rec-"));
  t.after(() => { fs.rmSync(dir, { recursive: true, force: true }); fs.rmSync(recDir, { recursive: true, force: true }); });
  const prior = process.env.CODEWHALE_CU_STATE_DIR;
  process.env.CODEWHALE_CU_STATE_DIR = dir;
  t.after(() => { if (prior === undefined) delete process.env.CODEWHALE_CU_STATE_DIR; else process.env.CODEWHALE_CU_STATE_DIR = prior; });
  const page = pageState();
  const sockets = [];
  const launches = [];
  let connects = 0;
  const browser = createBrowser({
    findApp: () => "/Applications/Google Chrome.app",
    launch: async ({ profileDir }) => {
      launches.push(profileDir);
      fs.mkdirSync(profileDir, { recursive: true });
      fs.writeFileSync(path.join(profileDir, "DevToolsActivePort"), "4242\n/devtools/browser/test\n");
    },
    connect: async (url) => {
      connects += 1;
      if (connects <= connectFailures) throw Object.assign(new Error("connection refused"), { code: "browser_unavailable" });
      const ws = new FakeWs(script ?? defaultScript(page));
      sockets.push({ url, ws });
      return ws;
    },
    recordingsDir: () => recDir,
  });
  return { browser, sockets, launches, recDir, dir, page };
}

test("browser start launches the self-owned profile and binds one tab per session", async (t) => {
  const { browser, sockets, launches, page } = harness(t);
  const start = await browser.start({});
  assert.equal(start.running, true);
  assert.equal(start.launched, true);
  assert.equal(start.reused, false);
  assert.equal(start.browser, "/Applications/Google Chrome.app");
  assert.equal(start.tab.id, "T1");
  assert.match(start.note, /self-owned profile/);
  assert.equal(launches.length, 1);
  assert.match(sockets[0].url, /^ws:\/\/127\.0\.0\.1:4242\/devtools\/browser\/test$/);
  const ws = sockets[0].ws;
  assert.equal(ws.sentOf("Target.createTarget").length, 0, "adopts the launch's blank tab");
  assert.equal(ws.sentOf("Target.attachToTarget")[0].params.targetId, "T1");
  assert.equal(ws.sentOf("Target.attachToTarget")[0].params.flatten, true);
  page.url = "about:blank";

  const status = await browser.status();
  assert.equal(status.running, true);
  assert.equal(status.tabs.length, 1);
  assert.equal(status.activeTab.id, "T1");
});

test("browser start gives a busy instance a new tab instead of stealing one", async (t) => {
  const h = harness(t);
  h.page.tabs = ["T1", "T2"];
  const start = await h.browser.start({});
  assert.equal(start.tab.id, "T3");
  assert.deepEqual(h.page.tabs, ["T1", "T2", "T3"]);
  assert.equal(h.sockets[0].ws.sentOf("Target.createTarget").length, 1);
});

test("browser navigate waits for load and reports url+title from the target, not from us", async (t) => {
  const { browser, sockets } = harness(t);
  await browser.start({});
  const ws = sockets[0].ws;
  const nav = browser.navigate({ url: "https://example.com" });
  await new Promise((r) => setTimeout(r, 5));
  ws.event("Page.loadEventFired", {}, "S1");
  const receipt = await nav;
  assert.equal(receipt.verified, true);
  assert.equal(receipt.url, "https://example.com/");
  assert.equal(receipt.title, "t:https://example.com/");
  assert.equal(ws.sentOf("Page.navigate")[0].params.url, "https://example.com/");
  assert.equal(ws.sentOf("Page.navigate")[0].sessionId, "S1");
});

test("browser navigate reports a load timeout honestly instead of claiming success", async (t) => {
  const priorTimeout = process.env.CODEWHALE_CU_BROWSER_LOAD_TIMEOUT_MS;
  process.env.CODEWHALE_CU_BROWSER_LOAD_TIMEOUT_MS = "120";
  t.after(() => { if (priorTimeout === undefined) delete process.env.CODEWHALE_CU_BROWSER_LOAD_TIMEOUT_MS; else process.env.CODEWHALE_CU_BROWSER_LOAD_TIMEOUT_MS = priorTimeout; });
  const { browser } = harness(t);
  await browser.start({});
  const receipt = await browser.navigate({ url: "https://slow.test" });
  assert.equal(receipt.action_sent, true);
  assert.equal(receipt.verified, false);
  assert.match(receipt.note, /did not report load completion/);
});

test("browser click resolves a selector through the DOM domain and clicks its box center", async (t) => {
  const { browser, sockets } = harness(t);
  await browser.start({});
  const ws = sockets[0].ws;
  const receipt = await browser.click({ selector: "#go" });
  assert.deepEqual(receipt.point, { x: 20, y: 30 });
  assert.equal(receipt.selector, "#go");
  assert.equal(receipt.pointer_moved, false);
  assert.ok(ws.sentOf("DOM.querySelector").some((m) => m.params.selector === "#go" && m.params.nodeId === 1));
  const clicks = ws.sentOf("Input.dispatchMouseEvent").map((m) => m.params);
  assert.deepEqual(clicks.map((c) => c.type), ["mouseMoved", "mousePressed", "mouseReleased"]);
  assert.ok(clicks.every((c) => c.x === 20 && c.y === 30));
});

test("browser click reports selector_not_found and bad_target for an out-of-viewport point", async (t) => {
  const script = { ...defaultScript(pageState()), "DOM.querySelector": { nodeId: 0 } };
  const { browser } = harness(t, { script });
  await browser.start({});
  await assert.rejects(browser.click({ selector: "#missing" }), (e) => e.code === "selector_not_found" && /#missing/.test(e.message));
  await assert.rejects(browser.click({ point: { x: 900, y: 5 } }), (e) => e.code === "bad_target" && /800x600/.test(e.message));
  await assert.rejects(browser.click({}), (e) => e.code === "bad_args");
});

test("browser type focuses an optional selector, inserts text, and can press Enter", async (t) => {
  const { browser, sockets } = harness(t);
  await browser.start({});
  const ws = sockets[0].ws;
  const receipt = await browser.type({ text: "hi", selector: "#q", enter: true });
  assert.equal(receipt.chars, 2);
  assert.equal(receipt.selector, "#q");
  assert.equal(receipt.entered, true);
  assert.equal(ws.sentOf("DOM.focus")[0].params.nodeId, 7);
  assert.equal(ws.sentOf("Input.insertText")[0].params.text, "hi");
  const keys = ws.sentOf("Input.dispatchKeyEvent").map((m) => m.params);
  assert.deepEqual(keys.map((k) => k.type), ["keyDown", "keyUp"]);
  assert.ok(keys.every((k) => k.key === "Enter" && k.windowsVirtualKeyCode === 13));
  await assert.rejects(browser.type({}), (e) => e.code === "bad_args");
});

test("browser screenshot writes a real file and names the viewport space", async (t) => {
  const { browser, recDir } = harness(t);
  await browser.start({});
  const receipt = await browser.screenshot({});
  assert.ok(fs.existsSync(receipt.file), receipt.file);
  assert.equal(fs.readFileSync(receipt.file, "utf8"), "abc");
  assert.equal(receipt.bytes, 3);
  assert.deepEqual(receipt.viewport, { w: 800, h: 600 });
  assert.equal(receipt.scale, 2);
  assert.equal(receipt.space, "page-viewport");
  assert.match(receipt.file, new RegExp(recDir.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")));
});

test("browser stop closes this session's tab and the browser only when no tabs remain", async (t) => {
  const { browser, sockets } = harness(t);
  await browser.start({});
  const ws = sockets[0].ws;
  const stop = await browser.stop();
  assert.equal(stop.closed, true);
  assert.equal(stop.browser_closed, true);
  assert.equal(ws.sentOf("Target.closeTarget")[0].params.targetId, "T1");
  assert.equal(ws.sentOf("Browser.close").length, 1);
  assert.equal((await browser.status()).running, false);
});

test("browser stop leaves the shared browser up while other sessions' tabs remain", async (t) => {
  const script = { ...defaultScript(pageState()), "Target.getTargets": { targetInfos: [{ type: "page", targetId: "T2", url: "https://other.test/", title: "other" }] } };
  const { browser, sockets } = harness(t, { script });
  await browser.start({});
  const stop = await browser.stop();
  assert.equal(stop.browser_closed, false);
  assert.match(stop.note, /other sessions remain open/);
  assert.equal(sockets[0].ws.sentOf("Browser.close").length, 0);
});

test("browser reuses a live instance and replaces a stale port file", async (t) => {
  const shared = fs.mkdtempSync(path.join(os.tmpdir(), "cu-browser-shared-"));
  t.after(() => fs.rmSync(shared, { recursive: true, force: true }));
  // A stale file must not shadow a fresh launch: the first connect attempt fails.
  const first = harness(t, { stateDir: shared, connectFailures: 1 });
  const profile = path.join(shared, "browser", "profile");
  fs.mkdirSync(profile, { recursive: true });
  fs.writeFileSync(path.join(profile, "DevToolsActivePort"), "9999\n/devtools/browser/stale\n");
  const start = await first.browser.start({});
  assert.equal(start.launched, true);
  assert.equal(first.launches.length, 1);
  // A second session (fresh bridge) sees the live file and reuses the instance.
  const second = harness(t, { stateDir: shared });
  const reused = await second.browser.start({});
  assert.equal(reused.reused, true);
  assert.equal(reused.launched, false);
  assert.equal(second.launches.length, 0);
});

test("browser refuses before start, on bad urls, and on old runtimes", async (t) => {
  const { browser } = harness(t);
  await assert.rejects(browser.navigate({ url: "https://x.test" }), (e) => e.code === "browser_not_running");
  assert.equal((await browser.status()).running, false);
  await assert.rejects(browser.start({ url: "ftp://x.test" }), (e) => e.code === "bad_args");
  await assert.rejects(browser.start({ url: "javascript:alert(1)" }), (e) => e.code === "bad_args");
  const prior = globalThis.WebSocket;
  globalThis.WebSocket = undefined;
  t.after(() => { globalThis.WebSocket = prior; });
  await assert.rejects(browser.start({}), (e) => e.code === "unsupported_runtime");
});

test("checkBrowserUrl allows http(s) and about:blank only; findBrowserApp honors overrides", () => {
  assert.equal(checkBrowserUrl("https://a.test/x"), "https://a.test/x");
  assert.equal(checkBrowserUrl("http://127.0.0.1:8080/"), "http://127.0.0.1:8080/");
  assert.equal(checkBrowserUrl("about:blank"), "about:blank");
  assert.throws(() => checkBrowserUrl("file:///etc/passwd"), /only http/);
  assert.throws(() => checkBrowserUrl("example.com"), /not a URL/);
  assert.throws(() => checkBrowserUrl(""), /need a url/);
  assert.equal(findBrowserApp("darwin", {}, (p) => p === "/Applications/Google Chrome.app"), "/Applications/Google Chrome.app");
  assert.equal(findBrowserApp("darwin", {}, () => false), null);
  assert.equal(findBrowserApp("darwin", { CODEWHALE_CU_BROWSER_APP: "/custom/Chromium.app" }, () => false), "/custom/Chromium.app");
});
