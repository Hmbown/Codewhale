// The desktop helper applies its own policy to every request that reaches its
// socket, and never runs a launch command read from its state files.
import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import url from "node:url";

const ROOT = path.resolve(path.dirname(url.fileURLToPath(import.meta.url)), "..");
const stateDir = fs.mkdtempSync(path.join(os.tmpdir(), "cu-helper-policy-"));
process.env.CODEWHALE_CU_STATE_DIR = stateDir;
process.env.CODEWHALE_CU_TEST_BACKEND = path.join(ROOT, "tests", "fixtures", "fake-backend.mjs");

const { handle } = await import("../src/app-handler.mjs");
const { launchArgv, readRegistration, writeRegistration } = await import("../src/app-socket.mjs");

test("app.json launch without MAC is ignored", () => {
  const planted = { id: "net.codewhale.computer-use", path: "/Applications/Codewhale Computer Use.app", launch: ["/bin/sh", "-c", "touch /tmp/planted"] };
  assert.deepEqual(launchArgv(planted, "darwin"), ["open", "-g", "-a", "/Applications/Codewhale Computer Use.app"]);
  for (const bad of [{ launch: ["/bin/sh"] }, { path: "relative.app" }, { path: "/tmp/not-a-bundle" }]) {
    assert.equal(launchArgv(bad, "darwin"), null, JSON.stringify(bad));
  }
  writeRegistration({ id: "x", launch: ["/bin/sh", "-c", "true"] });
  assert.equal(readRegistration(), null, "a registration without a bundle path is not launchable");
});

test("the helper refuses scripts its policy refuses, whoever connects", async () => {
  const r = await handle({ tool: "app_script", args: { script: 'do shell script "id"' } }, { computerId: "local", sessionId: "raw" });
  assert.equal(r.ok, false);
  assert.equal(r.error.code, "script_refused");
});

test("the helper honors a recorded deny, whoever connects", async () => {
  fs.writeFileSync(path.join(stateDir, "consent.json"), JSON.stringify({ version: 1, computers: { local: { apps: {
    "name:terminal": { decision: "deny", at: new Date().toISOString() },
  } } } }));
  for (const req of [
    { tool: "open_application", args: { name: "Terminal" } },
    { tool: "kill_app", args: { name: "Terminal" } },
    { tool: "app_script", args: { script: 'tell application "Terminal" to activate' } },
  ]) {
    const r = await handle(req, { computerId: "local", sessionId: "raw" });
    assert.equal(r.error?.code, req.tool === "app_script" ? "script_refused" : "app_denied", JSON.stringify(req));
  }
  const other = await handle({ tool: "app_script", args: { script: 'tell application "Notes" to activate' } }, { computerId: "local", sessionId: "raw" });
  assert.notEqual(other.error?.code, "app_denied");
});
