// Spawned computers: registry shape, executor wiring, docker lifecycle, and
// the MCP spawn/remove path. Docker tests are integration tests — they run
// real containers when a daemon is present and skip otherwise.
import { test, after } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawn } from "node:child_process";
import url from "node:url";

const ROOT = path.resolve(path.dirname(url.fileURLToPath(import.meta.url)), "..");
const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "cu-spawn-test-"));
process.env.CODEWHALE_CU_STATE_DIR = tmp;

const registry = await import("../src/registry.mjs");
const spawnMod = await import("../src/spawn.mjs");
const { dockerExec, routeFingerprint, SESSION_ID } = await import("../src/transport.mjs");
const { run } = await import("../src/exec.mjs");

const DOCKER = await spawnMod.dockerAvailable();
const NEED_DOCKER = { skip: !DOCKER && "docker daemon not available" };
const containers = new Set(); // anything a test leaves behind gets reaped

async function rmContainer(name) {
  containers.delete(name);
  await run("docker", ["rm", "-f", name], { timeoutMs: 15_000, signal: null });
}

after(async () => {
  for (const name of [...containers]) await rmContainer(name);
});

// ---------- registry ----------

test("registry accepts docker computers and defaults them to linux", () => {
  const entry = registry.register({ id: "d1", transport: "docker", container: "cu-spawn-d1-ab12cd", owned: true });
  assert.equal(entry.platform, "linux");
  assert.equal(entry.container, "cu-spawn-d1-ab12cd");
  assert.equal(entry.owned, true);
  const again = registry.load();
  assert.equal(again.computers.d1.container, "cu-spawn-d1-ab12cd");
});

test("registry rejects docker computers without a safe container name", () => {
  assert.throws(() => registry.register({ id: "d2", transport: "docker" }), (e) => e.code === "invalid_container");
  assert.throws(() => registry.register({ id: "d3", transport: "docker", container: "bad;rm -rf" }), (e) => e.code === "invalid_container");
  assert.throws(() => registry.register({ id: "d4", transport: "docker", container: "..-escape" }), (e) => e.code === "invalid_container");
});

// ---------- executor ----------

test("dockerExec speaks the agent contract through agent-exec.sh", () => {
  const ex = dockerExec({ id: "d", transport: "docker", container: "cu-spawn-d-ab12cd", platform: "linux" }, null);
  assert.equal(ex.kind, "docker");
  assert.equal(ex.container, "cu-spawn-d-ab12cd");
  assert.equal(ex.remoteAgent, "/app/docker/agent-exec.sh");
  assert.equal(typeof ex.remote, "function");
  assert.equal(ex.persistent, undefined, "no binding means no persistent channel");
  const bound = dockerExec({ id: "d", transport: "docker", container: "cu-spawn-d-ab12cd" }, {});
  assert.equal(typeof bound.persistent, "function");
  assert.equal(typeof bound.closeChannel, "function");
});

test("routeFingerprint distinguishes containers on the same image", () => {
  const a = routeFingerprint({ id: "d", transport: "docker", container: "cu-spawn-a-1", platform: "linux" });
  const b = routeFingerprint({ id: "d", transport: "docker", container: "cu-spawn-a-2", platform: "linux" });
  assert.notEqual(a, b, "a new container is a new route — stale bindings must re-observe");
});

// ---------- docker lifecycle (integration) ----------

test("spawnDockerComputer provisions a usable desktop and destroy removes it", NEED_DOCKER, async () => {
  const spawned = await spawnMod.spawnDockerComputer({ id: "it1" });
  containers.add(spawned.container);
  assert.match(spawned.container, /^cu-spawn-it1-[0-9a-f]{6}$/);

  // Labels mark it ours, this session's, and name the computer.
  const labels = await run("docker", ["inspect", "--format",
    '{{index .Config.Labels "codewhale.cu.spawned"}}|{{index .Config.Labels "codewhale.cu.session"}}|{{index .Config.Labels "codewhale.cu.computer"}}',
    spawned.container], { timeoutMs: 10_000 });
  assert.equal(labels.stdout.trim(), `1|${SESSION_ID}|it1`);

  // The desktop stack is genuinely up — the readiness probe waits for the WM.
  const ex = dockerExec({ id: "it1", transport: "docker", container: spawned.container, platform: "linux" }, {});
  const wins = await ex.persistent({ tool: "list_windows", args: {} });
  assert.equal(wins.ok, true);
  const cur = await ex.persistent({ tool: "cursor_position", args: {} });
  assert.equal(cur.ok, true);
  assert.ok(Number.isFinite(cur.data.x));

  const res = await spawnMod.destroyDockerComputer({ container: spawned.container });
  assert.equal(res.destroyed, true);
  containers.delete(spawned.container);
  const gone = await run("docker", ["inspect", spawned.container], { timeoutMs: 10_000 });
  assert.notEqual(gone.code, 0, "container is gone after destroy");
});

test("destroyDockerComputer refuses containers it did not spawn", NEED_DOCKER, async () => {
  const r = await run("docker", ["run", "-d", "--name", "cu-not-ours", "codewhale-cu-linux", "sleep", "infinity"], { timeoutMs: 30_000 });
  assert.equal(r.code, 0, r.stderr);
  containers.add("cu-not-ours");
  const res = await spawnMod.destroyDockerComputer({ container: "cu-not-ours" });
  assert.deepEqual(res, { destroyed: false, reason: "not_spawned" });
  const alive = await run("docker", ["inspect", "--format", "{{.State.Running}}", "cu-not-ours"], { timeoutMs: 10_000 });
  assert.equal(alive.stdout.trim(), "true", "unlabeled containers are never destroyed");
  await rmContainer("cu-not-ours");
});

test("destroyDockerComputer reports a missing container without destroying anything", NEED_DOCKER, async () => {
  const res = await spawnMod.destroyDockerComputer({ container: "cu-spawn-ghost-000000" });
  assert.deepEqual(res, { destroyed: false, reason: "container_gone" });
});

test("spawn refuses an image that is not present instead of guessing a build", NEED_DOCKER, async () => {
  await assert.rejects(
    () => spawnMod.spawnDockerComputer({ id: "it2", image: "cu-image-that-does-not-exist" }),
    (e) => e.code === "spawn_image_missing");
});

// ---------- MCP end to end ----------

const pending = new Map();
let server, buf = "", nextId = 1;
function rpc(method, params) {
  const id = nextId++;
  return new Promise((resolve, reject) => {
    const t = setTimeout(() => { pending.delete(id); reject(new Error(`timeout: ${method}`)); }, 90_000);
    pending.set(id, (msg) => { clearTimeout(t); resolve(msg); });
    server.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n");
  });
}
const call = async (name, args = {}) => JSON.parse((await rpc("tools/call", { name, arguments: args })).result.content[0].text);

test("computer spawn registers an owned docker computer, acts on it, and remove destroys it", NEED_DOCKER, async () => {
  server = spawn("node", [path.join(ROOT, "mcp", "server.mjs")], {
    env: { ...process.env, CODEWHALE_CU_STATE_DIR: tmp },
    stdio: ["pipe", "pipe", "pipe"],
  });
  server.stdout.on("data", (c) => {
    buf += c.toString();
    let i;
    while ((i = buf.indexOf("\n")) !== -1) {
      const line = buf.slice(0, i).trim(); buf = buf.slice(i + 1);
      if (!line) continue;
      const msg = JSON.parse(line);
      if (msg.id != null && pending.has(msg.id)) { pending.get(msg.id)(msg); pending.delete(msg.id); }
    }
  });
  await rpc("initialize", { protocolVersion: "2024-11-05", capabilities: {}, clientInfo: { name: "t", version: "0" } });
  server.stdin.write(JSON.stringify({ jsonrpc: "2.0", method: "notifications/initialized" }) + "\n");

  const s = await call("computer", { action: "spawn", id: "mcp-e2e", transport: "docker" });
  assert.equal(s.ok, true, JSON.stringify(s));
  assert.equal(s.active, "mcp-e2e", "spawn selects the disposable computer");
  assert.equal(s.spawned.owned, true);
  containers.add(s.spawned.container);

  const listed = await call("computer", { action: "list" });
  const entry = listed.computers.find((c) => c.id === "mcp-e2e");
  assert.equal(entry.transport, "docker");
  assert.equal(entry.owned, true);
  assert.equal(entry.platform, "linux");

  // A real tool call against the spawned desktop — same path as ssh.
  const wins = await call("list_windows", {});
  assert.equal(wins.ok, true, JSON.stringify(wins));
  assert.equal(wins.computer.id, "mcp-e2e");

  // app_script must be refused — a spawned channel is not a shell either.
  const script = await call("app_script", { language: "applescript", script: "return 1" });
  assert.equal(script.ok, false);
  assert.equal(script.error.code, "unsupported_on_transport");

  const removed = await call("computer", { action: "remove", id: "mcp-e2e" });
  assert.equal(removed.ok, true);
  assert.equal(removed.destroyed, true);
  assert.equal(removed.active, "local");
  containers.delete(s.spawned.container);
  const gone = await run("docker", ["inspect", s.spawned.container], { timeoutMs: 10_000 });
  assert.notEqual(gone.code, 0);
});

test("server shutdown destroys session-owned spawned computers", NEED_DOCKER, async () => {
  // Fresh server: spawn, then end stdin — the session teardown must reap.
  const s2 = await call("computer", { action: "spawn", id: "mcp-reap", transport: "docker" });
  assert.equal(s2.ok, true, JSON.stringify(s2));
  containers.add(s2.spawned.container);
  server.stdin.end();
  await new Promise((resolve) => server.on("close", resolve));
  await new Promise((r) => setTimeout(r, 500));
  const gone = await run("docker", ["inspect", s2.spawned.container], { timeoutMs: 10_000 });
  assert.notEqual(gone.code, 0, "session end reaps its spawned containers");
  containers.delete(s2.spawned.container);
});

test('disposable desktops require a live Linux Docker engine, including on Windows hosts', async () => {
  for (const [response, expected] of [[{code:0,stdout:'linux\n'},true],[{code:0,stdout:'windows\n'},false],[{code:1,stdout:'linux'},false],[{code:0,stdout:'linux',timedOut:true},false],[{code:0,stdout:'linux',aborted:true},false]]) {
    assert.equal(await spawnMod.dockerAvailable(async args => { assert.deepEqual(args,['info','--format','{{.OSType}}']); return response; }),expected);
  }
});


test("Docker desktop entrypoint survives repeated orderly restarts", { ...NEED_DOCKER, timeout: 90_000 }, async t => {
  const name = `cu-restart-${process.pid}-${Date.now()}`;
  containers.add(name);
  t.after(() => rmContainer(name));
  const started = await run("docker", [
    "run", "-d", "--name", name, "--init", "--network", "none",
    // Exercise the current entrypoint even if this developer has an older
    // cached desktop image. No host display or input device is mounted.
    "--mount", `type=bind,src=${path.join(ROOT, "docker", "entrypoint.sh")},dst=/app/docker/entrypoint.sh,readonly`,
    spawnMod.DEFAULT_IMAGE, "sleep", "infinity",
  ], { timeoutMs: 30_000 });
  assert.equal(started.code, 0, started.stderr);
  const request = Buffer.from(JSON.stringify({ tool: "list_windows", args: {} })).toString("base64");
  for (let cycle = 0; cycle < 3; cycle++) {
    if (cycle) {
      const restarted = await run("docker", ["restart", name], { timeoutMs: 15_000 });
      assert.equal(restarted.code, 0, restarted.stderr);
    }
    const deadline = Date.now() + 20_000;
    let observed = false, last = "";
    while (Date.now() < deadline) {
      const probe = await run("docker", ["exec", name, "/bin/sh", "/app/docker/agent-exec.sh", request], { timeoutMs: 5_000 });
      last = probe.stdout || probe.stderr;
      try { observed = probe.code === 0 && JSON.parse(probe.stdout).ok === true; } catch {}
      if (observed) break;
      await new Promise(resolve => setTimeout(resolve, 250));
    }
    assert.equal(observed, true, `desktop unavailable after restart ${cycle}: ${last}`);
  }
});
