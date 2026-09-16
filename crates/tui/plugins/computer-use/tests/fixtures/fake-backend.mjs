// Test-only backend, injected via CODEWHALE_CU_TEST_BACKEND (see
// src/transport.mjs). Records every call as JSONL in FAKE_BACKEND_CALLS and
// answers with canned raster/element data. resolve_element reads its reply
// from the JSON file at FAKE_BACKEND_CONTROL, falling back to element 0's
// cached geometry.
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import crypto from "node:crypto";

// Smallest valid 1x1 PNG.
const PNG_1X1 = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==",
  "base64",
);

const ELEMENTS = [
  { index: 0, path: [0], windowIndex: 0, role: "AXWindow", label: "Main", position: { x: 0, y: 0 }, size: { w: 400, h: 300 } },
  { index: 1, path: [0, 1], windowIndex: 0, role: "AXButton", label: "OK", position: { x: 10, y: 20 }, size: { w: 60, h: 30 } },
  { index: 2, path: [], windowIndex: -1, role: "AXMenuBar", actions: [] },
  { index: 3, path: [0], windowIndex: -1, role: "AXMenuBarItem", label: "File", actions: ["AXPress"] },
  { index: 4, path: [0, 0], windowIndex: -1, role: "AXMenu", actions: [] },
  { index: 5, path: [0, 0, 0], windowIndex: -1, role: "AXMenuItem", label: "Save", actions: ["AXPress"] },
  { index: 6, path: [0], windowIndex: -2, role: "AXMenu", actions: [] },
  { index: 7, path: [0, 0], windowIndex: -2, role: "AXMenuItem", label: "Choose", actions: ["AXPress"] },
  { index: 8, path: [0, 2], windowIndex: 0, role: "AXTextField", value: "Fixture text", focused: true, enabled: true, actions: ["AXConfirm"], position: { x: 10, y: 60 }, size: { w: 150, h: 25 } },
];

function tmpPng(prefix) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), prefix));
  const file = path.join(dir, `${crypto.randomBytes(3).toString("hex")}.png`);
  fs.writeFileSync(file, PNG_1X1);
  return file;
}

export function create() {
  const callsFile = process.env.FAKE_BACKEND_CALLS;
  const controlFile = process.env.FAKE_BACKEND_CONTROL;
  const record = (method, args) => {
    try { fs.appendFileSync(callsFile, JSON.stringify({ method, args }) + "\n"); } catch {}
  };
  // One backend instance owns this binding; serve-mode tests read it back to
  // prove state survives between requests on the same agent process.
  let boundApp = null;

  return {
    platform: "fake",
    async open_application(args = {}) {
      record("open_application", args);
      boundApp = { name: args.name ?? "FakeApp", pid: args.pid ?? 4242, bundle_id: args.bundle_id ?? "com.fake.app" };
      return { launched: true, activate: !!args.activate, resolved: boundApp, keyboard_delivery: args.activate ? "foreground-guarded" : "process", input_scope: args.activate ? "shared-desktop" : "application", shared_pointer: !!args.activate };
    },
    async list_apps() {
      record("list_apps", {});
      return { items: [{ name: boundApp?.name ?? "FakeApp", pid: boundApp?.pid ?? 4242, bundle_id: "com.fake.app" }] };
    },
    async left_mouse_down(args = {}) { record("left_mouse_down", args); return { action_sent: true, at: { x: args.target?.x, y: args.target?.y } }; },
    async left_mouse_up(args = {}) { record("left_mouse_up", args); return { action_sent: true }; },
    async screenshot({ region } = {}) {
      record("screenshot", { region });
      const file = tmpPng("cu-fake-shot-");
      return region
        ? { file, points: { x: region[0], y: region[1], w: region[2], h: region[3] }, pixels: { w: region[2] * 2, h: region[3] * 2 }, scale: 2 }
        : { file, points: { x: 0, y: 0, w: 800, h: 600 }, pixels: { w: 1600, h: 1200 }, scale: 2 };
    },
    async zoom({ region } = {}) {
      record("zoom", { region });
      return { file: tmpPng("cu-fake-zoom-"), region };
    },
    async get_app_state({ app_ref, detail, include_ocr } = {}) {
      record("get_app_state", { app_ref, detail, include_ocr });
      return { found: true, name: app_ref?.name ?? "FakeApp", elements: ELEMENTS, ...(include_ocr ? { ocr: {
        status: app_ref?.name === "OCR unavailable" ? "unavailable" : "ok",
        raster: { file: "/fixture/ocr.png", points: { x: 100, y: 50, w: 200, h: 100 }, pixels: { w: 400, h: 200 }, scale: 2 },
        blocks: app_ref?.name === "OCR unavailable" ? [] : [{ text: "Raster text", confidence: 0.9, bounds: { x: 60, y: 20, w: 40, h: 40 }, target: { type: "coordinate", x: 80, y: 40 } }],
      } } : {}) };
    },
    async resolve_element(args) {
      record("resolve_element", args);
      let ctrl = null;
      try { ctrl = JSON.parse(fs.readFileSync(controlFile, "utf8")); } catch {}
      if (ctrl) return ctrl;
      const el = ELEMENTS[1];
      return { found: true, element: { role: el.role, label: el.label, position: el.position, size: el.size }, reason: null };
    },
    async left_click({ target } = {}) { record("left_click", { target }); return { action_sent: true, at: { x: target?.x, y: target?.y } }; },
    async double_click({ target } = {}) { record("double_click", { target }); return { action_sent: true, at: { x: target?.x, y: target?.y } }; },
    async mouse_move({ target } = {}) { record("mouse_move", { target }); return { action_sent: true, at: { x: target?.x, y: target?.y } }; },
    async perform_action(args) { record("perform_action", args); return { action_sent: true, strategy: "a11y" }; },
    async set_value(args) { record("set_value", args); return { action_sent: true, strategy: "a11y", verified: true, after: args.value }; },
    async type(args) { record("type", args); return { action_sent: true, text: args.text, verified: true, bound_app: boundApp?.name ?? null }; },
    async key(args) { record("key", args); return { action_sent: true, key: args.text ?? "return" }; },
    async focus(args) { record("focus", args); return { action_sent: true, focused: true, strategy: "a11y" }; },
    async get_value(args) { record("get_value", args); return { value: "Fixture text", strategy: "a11y" }; },
  };
}

export default { create };
