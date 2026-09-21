// Session-state and child-process fixture. No GUI or real input is touched.
import fs from "node:fs";
import crypto from "node:crypto";
import { runOk, withSignal } from "../../src/exec.mjs";

export function create() {
  const instance = crypto.randomUUID();
  const log = process.env.CU_SESSION_CALLS;
  let appName = null;
  let pointerDown = false;
  const record = (method, extra = {}) => fs.appendFileSync(log, JSON.stringify({ instance, method, appName, at: Date.now(), ...extra }) + "\n");
  return {
    async probe() { return { ready: true }; },
    async get_app_state({ app_ref }) {
      appName = app_ref?.name;
      record("get_app_state");
      return { found: true, name: appName, elements: [] };
    },
    async type({ text }) {
      if (!appName) throw Object.assign(new Error("Observe an app in this session first"), { code: "target_app_required" });
      record("type", { text });
      return { action_sent: true, appName };
    },
    async hold_key({ text }) {
      record("hold_key", { text });
      try {
        await runOk(process.execPath, ["-e", `
          const fs = require('node:fs');
          const [file, instance, text] = process.argv.slice(1);
          const record = method => fs.appendFileSync(file, JSON.stringify({ instance, method, text, at: Date.now() }) + '\\n');
          record('child_started');
          setTimeout(() => record('late_input'), 5000);
        `, log, instance, text]);
      } finally {
        await withSignal(null, () => runOk(process.execPath, ["-e", `
          const fs = require('node:fs');
          fs.appendFileSync(process.argv[1], JSON.stringify({ instance: process.argv[2], method: 'child_released', at: Date.now() }) + '\\n');
        `, log, instance]));
      }
      return { action_sent: true };
    },
    async left_mouse_down() { pointerDown = true; record("pointer_down"); return { action_sent: true }; },
    async releaseInput() {
      record("release_input", { pointerDown });
      pointerDown = false;
    },
    async closeSession() { record("session_closed"); },
  };
}
