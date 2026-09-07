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

  return {
    platform: "fake",
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
    async get_app_state({ app_ref } = {}) {
      record("get_app_state", { app_ref });
      return { found: true, name: app_ref?.name ?? "FakeApp", elements: ELEMENTS };
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
  };
}

export default { create };
