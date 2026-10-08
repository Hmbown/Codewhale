// Sprite Task hold for one turn: 5 min expiry refreshed every 60 s, released
// at turn end, capped so a crashed holder leaves only a short tail (S0 Q7).
import { test, after } from "node:test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { once } from "node:events";
import { createTaskHold, expireSeconds, spriteApi } from "../src/sprite-task.mjs";
// These transports are Unix sockets inside the Linux Sprite; Windows cannot bind the path.
const UNIX_SOCKETS = { skip: process.platform === "win32" && "Unix-socket transport (Sprite/Linux only)" };

const ROOT = path.resolve(import.meta.dirname, "..");
const dir = fs.mkdtempSync(path.join(os.tmpdir(), "cu-task-"));
after(() => fs.rmSync(dir, { recursive: true, force: true }));

/** A fake /.sprite/api.sock: POST/PUT register, DELETE removes, GET lists. */
function fakeApi(sock, { putStatus = null } = {}) {
  const tasks = new Map();
  const log = [];
  const server = http.createServer((req, res) => {
    let body = "";
    req.on("data", (c) => { body += c; });
    req.on("end", () => {
      log.push(`${req.method} ${req.url} host=${req.headers.host}`);
      const send = (status, obj) => { res.writeHead(status, { "Content-Type": "application/json" }); res.end(obj ? JSON.stringify(obj) : ""); };
      const name = decodeURIComponent(req.url.split("/")[3] ?? "");
      if (req.method === "GET") return send(200, { tasks: [...tasks.values()] });
      if (req.method === "POST" || req.method === "PUT") {
        if (req.method === "PUT" && putStatus) return send(putStatus, null);
        const p = JSON.parse(body);
        const task = { name: p.name, expire: p.expire, expires_at: "2026-09-22T20:05:00Z" };
        tasks.set(p.name, task);
        return send(200, task);
      }
      if (req.method === "DELETE") { tasks.delete(name); return send(204, null); }
      send(405, null);
    });
  });
  return new Promise((resolve) => server.listen(sock, () => resolve({ server, tasks, log })));
}

test("expiry is capped at 5 minutes", () => {
  assert.equal(expireSeconds("5m"), 300);
  assert.equal(expireSeconds("90s"), 90);
  assert.throws(() => expireSeconds("1h"));
  assert.throws(() => expireSeconds("6m"));
  assert.throws(() => expireSeconds("10s"));
  assert.throws(() => createTaskHold({ name: "Bad Name" }));
  assert.throws(() => createTaskHold({ name: "turn-1", expire: "60s", refreshMs: 60_000 }), /shorter/);
});

test("acquire registers, refresh re-registers, release deletes", UNIX_SOCKETS, async (t) => {
  const sock = path.join(dir, "api1.sock");
  const api = await fakeApi(sock);
  t.after(() => api.server.close());
  const events = [];
  const hold = createTaskHold({ name: "turn-abc", refreshMs: 30, socket: sock, onEvent: (e) => events.push(e.event) });
  await hold.acquire();
  assert.equal(api.tasks.get("turn-abc").expire, "300s");
  await new Promise((r) => setTimeout(r, 80));
  await hold.release();
  assert.equal(api.tasks.size, 0);
  assert.ok(events.includes("refreshed"));
  assert.equal(events[0], "acquired");
  assert.equal(events.at(-1), "released");
  assert.ok(api.log.every((l) => l.endsWith("host=sprite")));
  assert.ok(api.log.some((l) => l.startsWith("PUT /v1/tasks/turn-abc")));
});

test("refresh falls back to POST when PUT is not offered", UNIX_SOCKETS, async (t) => {
  const sock = path.join(dir, "api2.sock");
  const api = await fakeApi(sock, { putStatus: 405 });
  t.after(() => api.server.close());
  const events = [];
  const hold = createTaskHold({ name: "turn-x", refreshMs: 30, socket: sock, onEvent: (e) => events.push(e.event) });
  await hold.acquire();
  await new Promise((r) => setTimeout(r, 70));
  await hold.release();
  assert.ok(events.includes("refreshed"));
  assert.ok(!events.includes("refresh_failed"));
});

test("turn-hold CLI holds for the turn and releases on stdin EOF (parent gone)", UNIX_SOCKETS, async (t) => {
  const sock = path.join(dir, "api3.sock");
  const api = await fakeApi(sock);
  t.after(() => api.server.close());
  const child = spawn(process.execPath, [path.join(ROOT, "mcp/turn-hold.mjs"), "--name", "turn-cli", "--socket", sock], { stdio: ["pipe", "pipe", "inherit"] });
  let out = "";
  child.stdout.on("data", (c) => { out += c; });
  for (let i = 0; i < 50 && !out.includes("acquired"); i++) await new Promise((r) => setTimeout(r, 20));
  assert.ok(api.tasks.has("turn-cli"), "held while the turn runs");
  child.stdin.end();
  const [code] = await once(child, "exit");
  assert.equal(code, 0);
  assert.equal(api.tasks.size, 0, "released at turn end");
  assert.match(out, /"event":"released"/);
  const listed = await spriteApi("GET", "/v1/tasks", null, { socket: sock });
  assert.deepEqual(listed.body.tasks, []);
});

test("turn-hold CLI refuses a long expiry and reports an unreachable socket", async () => {
  const run = (args) => new Promise((resolve) => {
    const c = spawn(process.execPath, [path.join(ROOT, "mcp/turn-hold.mjs"), ...args], { stdio: ["ignore", "pipe", "ignore"] });
    let out = ""; c.stdout.on("data", (d) => { out += d; });
    c.on("exit", (code) => resolve({ code, out }));
  });
  let r = await run(["--name", "turn-1", "--expire", "1h", "--socket", path.join(dir, "none.sock")]);
  assert.equal(r.code, 2);
  assert.match(r.out, /"event":"refused"/);
  r = await run(["--name", "turn-1", "--socket", path.join(dir, "none.sock")]);
  assert.equal(r.code, 1);
  assert.match(r.out, /"event":"acquire_failed"/);
});
