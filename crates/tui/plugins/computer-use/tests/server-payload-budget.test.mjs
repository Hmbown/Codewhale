// A single JSON-RPC message larger than the host's stdout budget kills the
// stdio transport and takes every tool with it (Claude Code disconnects at
// 16MB; a full-screen 5K PNG base64s to ~29MB). These tests drive the real
// server over stdio and assert that an over-budget raster degrades to its
// text receipt while the connection keeps serving.
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import url from "node:url";

const __dirname = path.dirname(url.fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, "..");

async function session(t, extraEnv = {}) {
  const stateDir = fs.mkdtempSync(path.join(os.tmpdir(), "cu-budget-state-"));
  const recDir = fs.mkdtempSync(path.join(os.tmpdir(), "cu-budget-rec-"));
  const server = spawn(process.execPath, [path.join(ROOT, "mcp", "server.mjs")], {
    env: {
      ...process.env,
      CODEWHALE_CU_APP: "off",
      CODEWHALE_CU_APP_WARM: "off",
      CODEWHALE_CU_STATE_DIR: stateDir,
      CODEWHALE_CU_RECORDINGS_DIR: recDir,
      CODEWHALE_CU_TEST_BACKEND: path.join(__dirname, "fixtures", "fake-backend.mjs"),
      FAKE_BACKEND_CALLS: path.join(stateDir, "calls.jsonl"),
      FAKE_BACKEND_CONTROL: path.join(stateDir, "control.json"),
      ...extraEnv,
    },
    stdio: ["pipe", "pipe", "pipe"],
  });
  t.after(() => {
    server.kill("SIGTERM");
    for (const dir of [stateDir, recDir]) { try { fs.rmSync(dir, { recursive: true, force: true }); } catch {} }
  });

  const pending = new Map();
  let nextId = 1;
  let buf = "";
  server.stdout.setEncoding("utf8");
  server.stdout.on("data", (chunk) => {
    buf += chunk;
    let i;
    while ((i = buf.indexOf("\n")) !== -1) {
      const line = buf.slice(0, i).trim();
      buf = buf.slice(i + 1);
      if (!line) continue;
      try {
        const msg = JSON.parse(line);
        if (msg.id && pending.has(msg.id)) { pending.get(msg.id)(msg); pending.delete(msg.id); }
      } catch {}
    }
  });

  const rpc = (method, params) => new Promise((resolve, reject) => {
    const id = nextId++;
    const timer = setTimeout(() => { pending.delete(id); reject(new Error(`timeout: ${method}`)); }, 30_000);
    pending.set(id, (msg) => { clearTimeout(timer); resolve(msg); });
    server.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n");
  });

  const init = await rpc("initialize", { protocolVersion: "2025-06-18" });
  assert.equal(init.result.serverInfo.name, "codewhale-cu");

  return {
    server,
    async call(name, args = {}) {
      const res = await rpc("tools/call", { name, arguments: args });
      assert.ok(res.result, `${name}: protocol error ${JSON.stringify(res.error ?? {})}`);
      return { content: res.result.content, receipt: JSON.parse(res.result.content[0].text) };
    },
  };
}

test("an over-budget raster degrades to its receipt and keeps the transport alive", async (t) => {
  // The fixture's 1x1 PNG base64s to ~92 bytes, so a 64-byte budget puts any
  // capture over the line without writing a huge file.
  const cu = await session(t, { CODEWHALE_CU_MAX_IMAGE_BYTES: "64" });

  const shot = await cu.call("screenshot");
  assert.equal(shot.receipt.ok, true, "an over-budget capture still succeeds");
  assert.equal(shot.content.length, 1, "the image block is dropped, not truncated");
  assert.equal(shot.content.every((c) => c.type !== "image"), true);

  const omitted = shot.receipt.image_omitted;
  assert.ok(omitted, "the receipt must say why no image came back");
  assert.equal(omitted.reason, "raster_too_large");
  assert.equal(omitted.limit_bytes, 64);
  assert.ok(omitted.encoded_bytes > omitted.limit_bytes);
  assert.match(omitted.note, /zoom|region/i);
  // The path and geometry survive, so the caller can still act on the capture.
  assert.ok(shot.receipt.file);
  assert.deepEqual(shot.receipt.pixels, { w: 1600, h: 1200 });

  // The whole point: the connection is still usable afterwards.
  assert.equal(server_alive(cu.server), true);
  const after = await cu.call("get_app_state", {});
  assert.equal(after.receipt.ok, true, "every other tool survives an over-budget capture");

  // Geometry stays bound, so coordinate targets still resolve off the receipt.
  const click = await cu.call("left_click", { target: { type: "coordinate", x: 400, y: 300 } });
  assert.equal(click.receipt.ok, true, JSON.stringify(click.receipt.error));
});

test("a raster within budget is still inlined as an image block", async (t) => {
  const cu = await session(t);
  const shot = await cu.call("screenshot");
  assert.equal(shot.receipt.ok, true);
  assert.equal(shot.receipt.image_omitted, undefined);
  const image = shot.content.find((c) => c.type === "image");
  assert.ok(image, "normal captures must keep inlining the raster");
  assert.equal(image.mimeType, "image/png");
  assert.ok(Buffer.from(image.data, "base64").length > 0);
});

function server_alive(child) {
  return child.exitCode === null && child.signalCode === null;
}
