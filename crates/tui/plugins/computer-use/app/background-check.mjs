import fs from "node:fs";
import path from "node:path";
import os from "node:os";
import crypto from "node:crypto";
import { spawn } from "node:child_process";
import { setTimeout as delay } from "node:timers/promises";
import { handle, closeSession } from "../src/app-handler.mjs";

/** Uses the same daemon backend and cancellation path as connected hosts. */
export async function runBackgroundCheck({ bundle, demoDirectory } = {}) {
  if (process.platform !== "darwin" || !bundle) throw new Error("The background check requires the installed macOS app.");
  const executable = path.join(bundle, "Contents", "Resources", "Practice.app", "Contents", "MacOS", "practice");
  if (!fs.existsSync(executable)) throw new Error("Update the Computer Use app to run the background check.");
  const sessionId = `setup-${crypto.randomUUID()}`;
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), 15_000);
  const child = spawn(executable, [], { stdio: ["ignore", "pipe", "pipe"] });
  const scratch = fs.mkdtempSync(path.join(os.tmpdir(), "cu-setup-capture-"));
  let ready = false, applied = null, latest = null, buffer = "", spawnError = null;
  child.on("error", error => { spawnError = error; });
  child.stderr.on("data", () => {});
  child.stdout.setEncoding("utf8");
  child.stdout.on("data", chunk => {
    buffer += chunk;
    if (buffer.length > 16_384) { controller.abort(); return; }
    let newline;
    while ((newline = buffer.indexOf("\n")) >= 0) {
      const line = buffer.slice(0, newline); buffer = buffer.slice(newline + 1);
      try { const result = JSON.parse(line); latest = result; if (result.event === "ready") ready = true; if (result.event === "applied") applied = result; } catch { /* Cocoa diagnostics are not receipts. */ }
    }
  });
  async function until(predicate) {
    while (!predicate()) {
      if (spawnError) throw spawnError;
      if (controller.signal.aborted || child.exitCode !== null) throw new Error("The check was interrupted. You can run it again when ready.");
      await delay(40);
    }
  }
  async function call(tool, args = {}) {
    const reply = await handle({ tool, args }, { sessionId, signal: controller.signal, persistentInputOwner: true });
    if (!reply.ok) throw new Error(reply.error.message);
    return reply.data;
  }
  try {
    await until(() => ready);
    await call("open_application", { pid: child.pid, activate: false });
    const state = await call("get_app_state");
    if (demoDirectory) {
      fs.mkdirSync(demoDirectory, { recursive: true });
      // AX can register before WindowServer makes a new window capturable.
      // Retry only this read, before sending any input, within the check limit.
      while (true) {
        try { await call("screenshot", { app_ref: { pid: child.pid }, path: path.join(demoDirectory, "01-ready.png") }); break; }
        catch (error) {
          if (controller.signal.aborted || !error.message.includes("not capturable")) throw error;
          await delay(100, undefined, { signal: controller.signal });
        }
      }
    }
    const entry = state.elements.find(element => element.role === "AXTextField" && element.label === "Practice text");
    const apply = state.elements.find(element => element.role === "AXButton" && element.label === "Apply");
    if (!entry || !apply) throw new Error("The practice controls could not be read. Check Accessibility permission and retry.");
    // Backend targets are resolved AX records, using the exact state index.
    const focus = await call("resolve_element", { app_ref: { pid: child.pid }, windowIndex: entry.windowIndex, path: entry.path });
    if (!focus.found) throw new Error("The practice text field changed. Run the check again.");
    await call("left_click", { target: { ...focus.element, type: "element", app_ref: { pid: child.pid } } });
    const phrase = "Background check complete 🐋";
    await call("type", { text: phrase });
    if (demoDirectory) await call("screenshot", { app_ref: { pid: child.pid }, path: path.join(demoDirectory, "02-entered.png") });
    const button = await call("resolve_element", { app_ref: { pid: child.pid }, windowIndex: apply.windowIndex, path: apply.path });
    if (!button.found) throw new Error("The practice Apply button changed. Run the check again.");
    await call("left_click", { target: { ...button.element, type: "element", app_ref: { pid: child.pid } } });
    await until(() => applied);
    if (applied.value !== phrase) throw new Error("The text received by the practice window did not match. Check the app log and retry.");
    const capture = await call("screenshot", { app_ref: { pid: child.pid }, path: path.join(scratch, "practice.png") });
    if (demoDirectory) fs.copyFileSync(capture.file, path.join(demoDirectory, "03-verified.png"));
    if (capture.app_ref?.pid !== child.pid || !capture.pixels?.w || !capture.pixels?.h) throw new Error("The practice window screenshot could not be verified. Check Screen Recording permission.");
    const count = latest.samples;
    await until(() => latest.samples > count);
    const isolated = latest.samples > 0 && latest.pointerChanges === 0 && latest.foregroundChanges === 0;
    return { ok: isolated, edited: true, screenshot: true, samples: latest.samples, pointerChanges: latest.pointerChanges, foregroundChanges: latest.foregroundChanges,
      message: isolated ? "Text entered, Apply verified and the practice window captured. Your foreground app and pointer stayed unchanged."
        : "Edit and screenshot verified. Your app or pointer moved during the check, so background isolation is inconclusive." };
  } finally {
    clearTimeout(timer);
    try { await closeSession(sessionId); }
    finally {
      if (child.exitCode === null && child.signalCode === null) child.kill("SIGTERM");
      fs.rmSync(scratch, { recursive: true, force: true });
    }
  }
}
