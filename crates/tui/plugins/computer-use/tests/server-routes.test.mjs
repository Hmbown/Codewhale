// Real MCP + real transports, with HDC/SSH executables or local backends replaced.
// Every catalog, downloaded byte and command log belongs to this fixture.
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { once } from "node:events";
import { createInterface } from "node:readline";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { routeFingerprint } from "../src/transport.mjs";

const ROOT = path.resolve(import.meta.dirname, "..");

// Every control/catalog write the child or server reads must be atomic:
// a plain writeFileSync is observable mid-write by the polling readers and
// surfaces as "Unexpected end of JSON input" instead of the fixture's error.
function writeJsonAtomic(file, value) {
  const tmp = `${file}.${process.pid}.tmp`;
  fs.writeFileSync(tmp, JSON.stringify(value));
  fs.renameSync(tmp, file);
}

function fixture(t, backendSource, sshSource) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "cu-route-"));
  const log = path.join(dir, "calls.jsonl");
  const control = path.join(dir, "control.json");
  const bin = path.join(dir, "bin");
  fs.mkdirSync(bin);
  fs.writeFileSync(path.join(bin, "hdc"), `#!${process.execPath}
const fs = require('node:fs');
const args = process.argv.slice(2);
const target = args[0] === '-t' ? args.splice(0, 2)[1] : 'default';
fs.appendFileSync(process.env.ROUTE_LOG, JSON.stringify({target,args}) + '\\n');
const control = fs.existsSync(process.env.ROUTE_CONTROL) ? JSON.parse(fs.readFileSync(process.env.ROUTE_CONTROL)) : {};
if (control.fail === target) process.exit(7);
if (args[0] === 'list') console.log(target);
if (args[0] === 'file' && args[1] === 'recv') {
  if (control.changeTo) {
    const file = process.env.CODEWHALE_CU_STATE_DIR + '/computers.json';
    const catalog = JSON.parse(fs.readFileSync(file));
    catalog.computers.pad.target = control.changeTo;
    const tmp = file + '.' + process.pid + '.tmp';
    fs.writeFileSync(tmp, JSON.stringify(catalog));
    fs.renameSync(tmp, file);
  }
  const layout = { attributes: { bundleName: target, type: 'Button', text: 'OK', bounds: '[0,0][20,20]' } };
  const jpeg = Buffer.from([255,216,255,192,0,11,8,0,120,0,168,1,1,17,0,255,217]);
  fs.writeFileSync(args[3], args[2].includes('layout') ? JSON.stringify(layout) : jpeg);
}
`, { mode: 0o755 });
  if (sshSource) fs.writeFileSync(path.join(bin, "ssh"), `#!${process.execPath}\n${sshSource}`, { mode: 0o755 });
  const env = { ...process.env, PATH: `${bin}${path.delimiter}${process.env.PATH}`,
    ROUTE_LOG: log, ROUTE_CONTROL: control, CODEWHALE_CU_APP: "off", CODEWHALE_CU_APP_WARM: "off",
    CODEWHALE_CU_STATE_DIR: dir, CODEWHALE_CU_RECORDINGS_DIR: dir };
  delete env.CODEWHALE_CU_TEST_REMOTE;
  delete env.CODEWHALE_CU_TEST_BACKEND;
  if (backendSource) {
    env.CODEWHALE_CU_TEST_BACKEND = path.join(dir, "backend.mjs");
    fs.writeFileSync(env.CODEWHALE_CU_TEST_BACKEND, backendSource);
  }
  const child = spawn(process.execPath, [path.join(ROOT, "mcp/server.mjs")], { env, stdio: ["pipe", "pipe", "pipe"] });
  let nextId = 0;
  const pending = new Map();
  const lines = createInterface({ input: child.stdout });
  lines.on("line", line => {
    const response = JSON.parse(line);
    pending.get(response.id)?.(response);
  });
  let stderr = "";
  child.stderr.on("data", data => { stderr += data; });
  t.after(async () => {
    const exited = once(child, "exit");
    child.stdin.end();
    await exited;
    fs.rmSync(dir, { recursive: true, force: true });
    assert.equal(stderr, "");
  });
  return {
    dir, env,
    control(value) { writeJsonAtomic(control, value); },
    calls() { return fs.existsSync(log) ? fs.readFileSync(log, "utf8").trim().split("\n").filter(Boolean).map(JSON.parse) : []; },
    async tool(name, args = {}) {
      const id = ++nextId;
      let timer;
      try {
        const response = await new Promise((resolve, reject) => {
          timer = setTimeout(() => reject(new Error(`${name} timed out: ${stderr}`)), 8_000);
          pending.set(id, resolve);
          child.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method: "tools/call", params: { name, arguments: args } }) + "\n");
        });
        assert.ok(response.result, JSON.stringify(response));
        return JSON.parse(response.result.content[0].text);
      } finally { clearTimeout(timer); pending.delete(id); }
    },
    async register(target, label = "Fixture") {
      const result = await this.tool("computer_register", { computer: "pad", transport: "hdc", target, label });
      assert.equal(result.ok, true, JSON.stringify(result));
    },
    externalRegister(target) {
      const result = spawnSync(process.execPath, ["--input-type=module", "-e",
        "import {register} from './src/registry.mjs'; register({id:'pad', transport:'hdc', target:process.argv[1]});", target],
        { cwd: ROOT, env, encoding: "utf8" });
      assert.equal(result.status, 0, result.stderr);
    },
  };
}

const point = { type: "coordinate", x: 10, y: 10 };

test("a warmed HDC backend cannot send input to A after registration reports B", async t => {
  const f = fixture(t);
  await f.register("A");
  assert.equal((await f.tool("request_access", { computer: "pad" })).connected, true);
  await f.register("B");
  const result = await f.tool("key", { text: "ENTER" });
  const input = f.calls().filter(call => call.args.includes("uiInput"));
  assert.equal(result.error?.code, "computer_observation_required", JSON.stringify({ result, input }));
  assert.deepEqual(input, []);
});

test("same-ID HDC replacement requires fresh observation and dispatches only to B", async t => {
  const f = fixture(t);
  await f.register("A");
  const initial = await f.tool("screenshot", { computer: "pad" });
  assert.equal(initial.ok, true, JSON.stringify(initial));
  const state = await f.tool("get_app_state");
  assert.equal((await f.tool("left_click", { target: point })).ok, true);
  await f.register("B");
  const before = f.calls().length;
  assert.equal((await f.tool("key", { text: "ENTER" })).error.code, "computer_observation_required");
  assert.equal(f.calls().length, before);
  assert.equal((await f.tool("get_app_state")).bundle_id, "B");
  assert.equal((await f.tool("left_click", { target: point })).error.code, "no_raster");
  assert.equal((await f.tool("perform_action", { target: { type: "element", state_id: state.state_id, index: 0 }, action: "click" })).error.code, "unknown_state");
  assert.equal((await f.tool("screenshot")).ok, true);
  assert.equal((await f.tool("left_click", { target: point })).ok, true);
  const input = f.calls().filter(call => call.args.includes("uiInput"));
  assert.deepEqual(input.map(call => call.target), ["A", "B"]);
  assert.ok(f.calls().slice(before).every(call => call.target === "B"));
});

test("a different catalog writer invalidates the active host's HDC route on use", async t => {
  const f = fixture(t);
  await f.register("A");
  await f.tool("screenshot", { computer: "pad" });
  f.externalRegister("B");
  assert.equal((await f.tool("type", { text: "fixture" })).error.code, "computer_observation_required");
  assert.equal((await f.tool("computer_list")).active, "pad");
  await f.tool("screenshot");
  assert.equal((await f.tool("key", { text: "ENTER" })).ok, true);
  assert.deepEqual(f.calls().filter(call => call.args.includes("uiInput")).map(call => call.target), ["B"]);
});

test("label-only catalog changes reuse cached backend geometry and observation", async t => {
  const f = fixture(t);
  await f.register("A");
  await f.tool("screenshot", { computer: "pad" });
  await f.register("A", "Renamed");
  const before = f.calls().length;
  assert.equal((await f.tool("list_displays")).ok, true);
  assert.equal(f.calls().length, before, "cached display geometry proves backend reuse");
  assert.equal((await f.tool("left_click", { target: point })).ok, true);
});

test("failed observation of a replacement never falls back to the old backend", async t => {
  const f = fixture(t);
  await f.register("A");
  await f.tool("screenshot", { computer: "pad" });
  f.externalRegister("B");
  f.control({ fail: "B" });
  const before = f.calls().length;
  assert.equal((await f.tool("screenshot")).ok, false);
  assert.equal((await f.tool("key", { text: "ENTER" })).error.code, "computer_observation_required");
  assert.ok(f.calls().slice(before).every(call => call.target === "B"));
  assert.equal(f.calls().filter(call => call.args.includes("uiInput")).length, 0);
});

test("an observation completed after an external route change cannot authorize input", async t => {
  const f = fixture(t);
  await f.register("A");
  f.control({ changeTo: "B" });
  const stale = await f.tool("get_app_state", { computer: "pad" });
  assert.equal(stale.error.code, "computer_route_changed");
  assert.equal(stale.request_dispatched, true);
  assert.equal(stale.outcome_unknown, true);
  f.control({});
  assert.equal((await f.tool("key", { text: "ENTER" })).error.code, "computer_observation_required");
  assert.equal((await f.tool("get_app_state")).bundle_id, "B");
  assert.equal((await f.tool("key", { text: "ENTER" })).ok, true);
  assert.deepEqual(f.calls().filter(call => call.args.includes("uiInput")).map(call => call.target), ["B"]);
});

test("remove and re-register cannot reuse observations even for the same route", async t => {
  const f = fixture(t);
  await f.register("A");
  await f.tool("screenshot", { computer: "pad" });
  await f.tool("computer_remove", { computer: "pad" });
  await f.register("A");
  assert.equal((await f.tool("left_click", { computer: "pad", target: point })).error.code, "computer_observation_required");
});

test("route fingerprints include transport fields and effective defaults only", () => {
  const base = { transport: "ssh", host: "a" };
  assert.equal(routeFingerprint(base), routeFingerprint({ ...base, label: "renamed", registeredAt: "later", platformHint: "linux", agentPath: ".codewhale-cu/agent/agent.mjs" }));
  for (const changed of [{ host: "b" }, { port: 2222 }, { user: "other" }, { agentPath: "other/agent.mjs" }, { platformHint: "darwin" }, { transport: "hdc" }]) {
    assert.notEqual(routeFingerprint(base), routeFingerprint({ ...base, ...changed }));
  }
  assert.equal(routeFingerprint({ transport: "hdc" }), routeFingerprint({ transport: "hdc", target: "", platform: "harmonyos" }));
  assert.notEqual(routeFingerprint({ transport: "hdc", target: "A" }), routeFingerprint({ transport: "hdc", target: "B" }));
});

test("failed cleanup and catalog rollback cannot resurrect the retired backend", async t => {
  const f = fixture(t, `
    import fs from 'node:fs';
    let instance = 0;
    export function create() {
      const id = ++instance;
      const record = method => fs.appendFileSync(process.env.ROUTE_LOG, JSON.stringify({method,id}) + '\\n');
      return {
        get_app_state: async () => ({found:true, elements:[], instance:id}),
        key: async () => { record('key'); return {action_sent:true}; },
        releaseInput: async () => {
          record('releaseInput');
          if (JSON.parse(fs.readFileSync(process.env.ROUTE_CONTROL)).failRelease)
            throw Object.assign(new Error('fixture release failed'), {code:'release_failed'});
        },
        closeSession: async () => {
          record('closeSession');
          if (JSON.parse(fs.readFileSync(process.env.ROUTE_CONTROL)).failCleanup)
            throw Object.assign(new Error('fixture cleanup failed'), {code:'cleanup_failed'});
        }
      };
    }
  `);
  f.control({ failCleanup: false });
  assert.equal((await f.tool("computer_register", { computer: "pad", transport: "local" })).ok, true);
  assert.equal((await f.tool("get_app_state", { computer: "pad" })).instance, 1);
  f.control({ failRelease: true });
  assert.equal((await f.tool("computer_register", { computer: "pad", transport: "hdc", target: "B" })).error.code, "release_failed");
  assert.deepEqual(f.calls().slice(-2).map(call => call.method), ["releaseInput", "closeSession"], "recorder cleanup is attempted even when input release fails");
  f.control({ failCleanup: true });
  assert.equal((await f.tool("computer_register", { computer: "pad", transport: "hdc", target: "B" })).error.code, "cleanup_failed");
  assert.equal((await f.tool("key", { text: "ENTER" })).error.code, "cleanup_failed");
  // Roll back the catalog exactly, bypassing the host which still owns A.
  const file = path.join(f.dir, "computers.json");
  const catalog = JSON.parse(fs.readFileSync(file));
  catalog.computers.pad = { id: "pad", transport: "local" };
  writeJsonAtomic(file, catalog);
  assert.equal((await f.tool("key", { text: "ENTER" })).error.code, "cleanup_failed");
  assert.equal(f.calls().filter(call => call.method === "key").length, 0);
  f.control({ failCleanup: false });
  assert.equal((await f.tool("key", { text: "ENTER" })).error.code, "computer_observation_required");
  assert.equal((await f.tool("get_app_state")).instance, 2);
  assert.equal((await f.tool("key", { text: "ENTER" })).ok, true);
  assert.deepEqual(f.calls().filter(call => call.method === "key").map(call => call.id), [2]);
  assert.deepEqual(f.calls().slice(0, 2).map(call => call.method), ["releaseInput", "closeSession"]);
});

test("a route change during element revalidation refuses dispatch to the old backend", async t => {
  const f = fixture(t, `
    import fs from 'node:fs';
    export function create() {
      const element = {index:0, path:[0], role:'Button', label:'OK', position:{x:0,y:0}, size:{w:20,h:20}};
      return {
        get_app_state: async () => ({found:true, elements:[element]}),
        resolve_element: async () => {
          const file = process.env.CODEWHALE_CU_STATE_DIR + '/computers.json';
          const catalog = JSON.parse(fs.readFileSync(file));
          catalog.computers.pad = {id:'pad',transport:'hdc',target:'B'};
          const tmp = file + '.' + process.pid + '.tmp';
          fs.writeFileSync(tmp, JSON.stringify(catalog));
          fs.renameSync(tmp, file);
          return {found:true,element};
        },
        perform_action: async () => { throw new Error('must never dispatch stale action'); }
      };
    }
  `);
  await f.tool("computer_register", { computer: "pad", transport: "local" });
  const state = await f.tool("get_app_state", { computer: "pad" });
  const action = await f.tool("perform_action", { target: { type: "element", state_id: state.state_id, index: 0 }, action: "click" });
  assert.equal(action.error.code, "computer_route_changed");
  assert.equal(action.request_dispatched, undefined);
  assert.equal((await f.tool("key", { text: "ENTER" })).error.code, "computer_observation_required");
  assert.deepEqual(f.calls(), []);
});

const dispatchFailureBackend = `
  import fs from 'node:fs';
  import {setTimeout as delay} from 'node:timers/promises';
  export function create() {
    const record = method => fs.appendFileSync(process.env.ROUTE_LOG, JSON.stringify({method}) + '\\n');
    return {
      get_app_state: async () => ({found:true,elements:[]}),
      key: async () => {
        record('dispatched');
        while (!JSON.parse(fs.readFileSync(process.env.ROUTE_CONTROL)).release) await delay(5);
        throw Object.freeze(Object.assign(new Error('fixture failed after dispatch'), {code:'fixture_dispatch_failed'}));
      },
      releaseInput: async () => { record('releaseInput'); },
      closeSession: async () => {
        record('closeSession');
        if (JSON.parse(fs.readFileSync(process.env.ROUTE_CONTROL)).failCleanup)
          throw Object.assign(new Error('fixture cleanup failed'), {code:'fixture_cleanup_failed'});
      }
    };
  }
`;

const dispatchFailureSSH = `
  const fs = require('node:fs');
  const {setTimeout:delay} = require('node:timers/promises');
  const request = JSON.parse(Buffer.from(process.argv.at(-1), 'base64'));
  const reply = value => process.stdout.write(JSON.stringify(value) + '\\n');
  (async () => {
    if (request.tool === 'platform') return reply({ok:true,platform:'linux'});
    if (request.tool === 'get_app_state') return reply({ok:true,data:{found:true,elements:[]}});
    if (request.tool !== 'key') throw new Error('unexpected fixture tool');
    fs.appendFileSync(process.env.ROUTE_LOG, JSON.stringify({method:'dispatched',host:process.argv.find(arg => arg.startsWith('fixture-'))}) + '\\n');
    let control;
    while (!(control=JSON.parse(fs.readFileSync(process.env.ROUTE_CONTROL))).release) await delay(5);
    if (control.failure === 'reply') return reply({ok:false,error:{code:'fixture_dispatch_failed',message:'fixture failed after dispatch'}});
    process.stderr.write('fixture connection lost after dispatch');
    process.exitCode = 7;
  })().catch(error => { process.stderr.write(error.message); process.exitCode = 9; });
`;

for (const mode of ["backend", "reply", "connection"]) {
  for (const change of ["changed", "removed", "unchanged", ...(mode === "backend" ? ["cleanup-failed"] : [])]) {
    test(`${mode} dispatch failure preserves the original error when the route is ${change}`, async t => {
      const local = mode === "backend";
      const f = fixture(t, local ? dispatchFailureBackend : null, local ? null : dispatchFailureSSH);
      f.control({ release: false, failure: mode });
      const registration = await f.tool("computer_register", local
        ? { computer: "pad", transport: "local" }
        : { computer: "pad", transport: "ssh", host: "fixture-a.test", installAgent: false });
      assert.equal(registration.ok, true, JSON.stringify(registration));
      assert.equal((await f.tool("get_app_state", { computer: "pad" })).ok, true);
      const pending = f.tool("key", { text: "ENTER" });
      let failed;
      try {
        // The child has entered dispatch before the independent catalog writer
        // changes anything. Release only after that change is visible in-fixture.
        const deadline = Date.now() + 2_000;
        while (!f.calls().some(call => call.method === "dispatched") && Date.now() < deadline) await delay(5);
        assert.equal(f.calls().filter(call => call.method === "dispatched").length, 1);
        const file = path.join(f.dir, "computers.json");
        const catalog = JSON.parse(fs.readFileSync(file));
        if (change === "removed") delete catalog.computers.pad;
        else if (change !== "unchanged") {
          if (local) catalog.computers.pad = { id: "pad", transport: "hdc", target: "B" };
          else catalog.computers.pad.host = "fixture-b.test";
        }
        writeJsonAtomic(file, catalog);
        f.control({ release: true, failure: mode, failCleanup: change === "cleanup-failed" });
        failed = await pending;
        const expected = mode === "connection"
          ? { code: "tool_error", message: "ssh fixture-a.test exited 7: fixture connection lost after dispatch" }
          : { code: "fixture_dispatch_failed", message: "fixture failed after dispatch" };
        assert.equal(failed.ok, false);
        assert.deepEqual(failed.error, expected, "route reconciliation must not mask the dispatch error");
        if (change === "unchanged") {
          assert.equal(Object.hasOwn(failed, "request_dispatched"), false);
          assert.equal(Object.hasOwn(failed, "outcome_unknown"), false);
          assert.equal(Object.hasOwn(failed, "note"), false);
          assert.deepEqual(f.calls().map(call => call.method), ["dispatched"], "unchanged routes keep their resources");
        } else {
          assert.equal(failed.request_dispatched, true, JSON.stringify({ receipt: failed, calls: f.calls() }));
          assert.equal(failed.outcome_unknown, true);
          assert.match(failed.note, /effect is unconfirmed/);
          assert.match(failed.note, /do not automatically retry/);
          if (local) assert.deepEqual(f.calls().map(call => call.method), ["dispatched", "releaseInput", "closeSession"]);
          const next = await f.tool("key", { text: "must remain blocked" });
          assert.equal(next.error.code, change === "removed" ? "unknown_computer"
            : change === "cleanup-failed" ? "fixture_cleanup_failed" : "computer_observation_required");
        }
        assert.equal(f.calls().filter(call => call.method === "dispatched").length, 1, "no automatic replay or new-target input");
      } finally {
        // Also unblock the owned child when an assertion fails; no live helper,
        // device, or network endpoint participates in this fixture.
        f.control({ release: true, failure: mode });
        await pending;
      }
    });
  }
}
