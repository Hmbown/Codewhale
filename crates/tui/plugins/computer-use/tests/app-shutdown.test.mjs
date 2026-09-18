import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawn } from "node:child_process";
import { setTimeout as delay } from "node:timers/promises";
import { appRequest, hello } from "../src/app-socket.mjs";

test("retiring daemon cleanup preserves a replacement listener and its run receipt", { timeout: 15_000 }, async t => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "cu-retire-"));
  const endpoint = process.platform === "win32" ? `\\\\.\\pipe\\cu-retire-${process.pid}` : path.join(directory, "app.sock");
  const previous = process.env.CODEWHALE_CU_APP_SOCKET;
  process.env.CODEWHALE_CU_APP_SOCKET = endpoint;
  const children = [], sockets = [];
  t.after(async () => {
    sockets.forEach(socket => socket.destroy());
    for (const { child, exited } of children) {
      if (child.exitCode === null && child.signalCode === null) child.kill("SIGKILL");
      await exited;
    }
    if (previous === undefined) delete process.env.CODEWHALE_CU_APP_SOCKET;
    else process.env.CODEWHALE_CU_APP_SOCKET = previous;
    fs.rmSync(directory, { recursive: true, force: true });
  });
  const started = path.join(directory, "cleanup-started"), release = path.join(directory, "allow-cleanup");
  const fixture = path.join(directory, "backend.mjs");
  fs.writeFileSync(fixture, `
    import fs from 'node:fs';
    import {setTimeout as delay} from 'node:timers/promises';
    export function create() { return {
      async get_app_state() { return {found:true,elements:[]}; },
      async releaseInput() {
        if (process.env.CU_BLOCK_CLEANUP !== '1') return;
        fs.writeFileSync(process.env.CU_CLEANUP_STARTED, 'started');
        while (!fs.existsSync(process.env.CU_ALLOW_CLEANUP)) await delay(10);
      }
    }; }
  `);
  function launch(block) {
    const child = spawn(process.execPath, [fileURLToPath(new URL("../app/daemon.mjs", import.meta.url))], {
      env: { ...process.env, CODEWHALE_CU_STATE_DIR: directory, CODEWHALE_CU_APP_SOCKET: endpoint,
        CODEWHALE_CU_TEST_BACKEND: fixture, CU_BLOCK_CLEANUP: block ? "1" : "0",
        CU_CLEANUP_STARTED: started, CU_ALLOW_CLEANUP: release },
      stdio: ["ignore", "ignore", "pipe"],
    });
    let errors = ""; child.stderr.on("data", chunk => { errors += chunk; });
    const exited = new Promise(resolve => { child.once("exit", resolve); child.once("error", resolve); });
    const processState = { child, exited, errors: () => errors };
    children.push(processState); return processState;
  }
  async function until(check, label) {
    const deadline = Date.now() + 5000;
    while (!(await check())) {
      assert.ok(Date.now() < deadline, `${label}: ${children.map(p => p.errors()).join("\n")}`);
      await delay(10, undefined, { signal: t.signal });
    }
  }
  const old = launch(true);
  await until(async () => (await hello({ timeoutMs: 100 }))?.pid === old.child.pid, "first daemon ready");
  const owner = await appRequest({ tool: "open_session", sessionId: "retiring" }, { keepOpen: true });
  sockets.push(owner.socket);
  assert.equal((await appRequest({ tool: "get_app_state", sessionId: "retiring", leaseToken: owner.reply.leaseToken })).ok, true);
  old.child.kill("SIGTERM");
  await until(() => fs.existsSync(started), "cleanup started");
  const replacement = launch(false);
  await until(async () => (await hello({ timeoutMs: 100 }))?.pid === replacement.child.pid, "replacement ready during old cleanup");
  fs.writeFileSync(release, "release");
  await until(() => old.child.exitCode !== null, "old daemon exit");
  assert.equal(old.child.exitCode, 0, old.errors());
  assert.equal((await hello()).pid, replacement.child.pid, "cleanup must not remove the new listener");
  assert.equal(JSON.parse(fs.readFileSync(path.join(directory, "app-run.json"))).pid, replacement.child.pid);
});
