// Per-app consent: the app, not the tool, is the unit of trust on the local
// computer. Unit tests pin the ledger; server tests prove the gate refuses
// before backend dispatch, cannot be sidestepped by re-spelling the app, and
// that foreground control is a separate consent from app access.
import { hostKeysLine, attest, attestParams, TEST_LEDGER_KEY, ledgerMac } from "./fixtures/host-decision.mjs";
import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import url from "node:url";
import { spawn } from "node:child_process";
import * as consent from "../src/consent.mjs";
import { dockerAvailable } from "../src/spawn.mjs";

const ROOT = path.resolve(path.dirname(url.fileURLToPath(import.meta.url)), "..");
const DOCKER = await dockerAvailable();
const NEED_DOCKER = { skip: !DOCKER && "docker daemon not available" };

let tmpSeq = 0;
function freshDir() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), `cu-consent-${tmpSeq++}-`));
  process.env.CODEWHALE_CU_STATE_DIR = dir;
  return dir;
}
// The unit tests run as the host would configure the plugin: remembered
// allows are signed with the ledger key.
consent.setLedgerKey(Buffer.from(TEST_LEDGER_KEY, "hex"));
// Every test gets a clean store and a clean session map key space.
let cidSeq = 0;
const cid = () => `c${cidSeq++}`;

// ---------- unit: the ledger ----------

test("appKeys normalize identity; parseAppArg reads every spelling", () => {
  assert.deepEqual(consent.appKeys({ name: "Safari", bundle_id: "Com.Apple.Safari", pid: 42 }),
    ["bundle:com.apple.safari", "name:safari", "pid:42"]);
  assert.deepEqual(consent.appKeys({}), []);
  assert.deepEqual(consent.appKeys(null), []);
  assert.deepEqual(consent.parseAppArg({ app: "pid:77" }), ["pid:77"]);
  assert.deepEqual(consent.parseAppArg({ app: "77" }), ["pid:77"]);
  assert.deepEqual(consent.parseAppArg({ app: "com.apple.Safari" }), ["bundle:com.apple.safari"]);
  assert.deepEqual(consent.parseAppArg({ app: "Safari.app" }), ["name:safari"]);
  assert.deepEqual(consent.parseAppArg({ app: "My App" }), ["name:my app"]);
  // Explicit fields win over the app string entirely.
  assert.deepEqual(consent.parseAppArg({ app: "Other", name: "Chosen" }), ["name:chosen"]);
});

test("record + decisionFor: allow/deny per computer, newest decision wins", () => {
  freshDir();
  const id = cid();
  assert.equal(consent.decisionFor(id, ["name:calc"]).state, "undecided");
  consent.record(id, ["name:calc"], "allow");
  assert.equal(consent.decisionFor(id, ["name:calc"]).state, "allowed");
  // A different computer sees nothing.
  assert.equal(consent.decisionFor(cid(), ["name:calc"]).state, "undecided");
  consent.record(id, ["name:calc"], "deny");
  assert.equal(consent.decisionFor(id, ["name:calc"]).state, "denied");
});

test("session and persisted layers overlay: a later session decision wins over 'always'", () => {
  freshDir();
  const id = cid();
  consent.record(id, ["name:mail"], "deny", { remember: true });
  assert.equal(consent.decisionFor(id, ["name:mail"]).state, "denied");
  // A session allow recorded later outranks the persisted deny.
  consent.record(id, ["name:mail"], "allow");
  const d = consent.decisionFor(id, ["name:mail"]);
  assert.equal(d.state, "allowed");
  assert.equal(d.persisted, false);
});

test("pid keys are session-only — a pid never persists to consent.json", () => {
  const dir = freshDir();
  const id = cid();
  consent.record(id, ["name:thing", "pid:4242"], "allow", { remember: true });
  const file = JSON.parse(fs.readFileSync(path.join(dir, "consent.json"), "utf8"));
  assert.ok(file.computers[id].apps["name:thing"]);
  assert.equal(file.computers[id].apps["pid:4242"], undefined, "pid must not persist");
  assert.ok(consent.decisionFor(id, ["pid:4242"]).state === "allowed", "session still sees the pid key");
});

test("alias folds a resolved identity's other spellings into the same decision", () => {
  freshDir();
  const id = cid();
  consent.record(id, ["name:safari"], "allow");
  // open_application resolved com.apple.Safari — the same allow now covers it.
  consent.alias(id, ["bundle:com.apple.safari", "pid:501"], { persisted: false, name: "Safari" });
  assert.equal(consent.decisionFor(id, ["bundle:com.apple.safari"]).state, "allowed");
  assert.equal(consent.decisionFor(id, ["pid:501"]).state, "allowed");
});

test("revoke removes decisions at both layers; dropSession keeps persisted", () => {
  const dir = freshDir();
  const id = cid();
  consent.record(id, ["name:a"], "allow", { remember: true });
  consent.record(id, ["name:b"], "allow");
  consent.revoke(id, ["name:a", "name:b"]);
  assert.equal(consent.decisionFor(id, ["name:a"]).state, "undecided");
  assert.equal(consent.decisionFor(id, ["name:b"]).state, "undecided");
  consent.record(id, ["name:c"], "allow", { remember: true });
  consent.record(id, ["name:d"], "allow");
  consent.dropSession(id);
  assert.equal(consent.decisionFor(id, ["name:c"]).state, "allowed", "persisted survives a route teardown");
  assert.equal(consent.decisionFor(id, ["name:d"]).state, "undecided", "session decision dies with the route");
  assert.ok(fs.existsSync(path.join(dir, "consent.json")));
});

test("foreground is its own scope: record, deny, revoke, status", () => {
  freshDir();
  const id = cid();
  assert.equal(consent.foregroundDecision(id).state, "undecided");
  consent.recordForeground(id, "allow");
  assert.equal(consent.foregroundDecision(id).state, "allowed");
  consent.recordForeground(id, "deny", { remember: true });
  assert.equal(consent.foregroundDecision(id).state, "denied");
  consent.revokeForeground(id);
  assert.equal(consent.foregroundDecision(id).state, "undecided");
  const st = consent.status(id);
  assert.equal(st.foreground, null);
  assert.deepEqual(Object.keys(st.apps), []);
});

test("status merges persisted and session entries and labels their source", () => {
  freshDir();
  const id = cid();
  consent.record(id, ["name:persisted-app"], "allow", { remember: true, name: "Persisted App" });
  consent.record(id, ["name:session-app"], "deny", { name: "Session App" });
  consent.recordForeground(id, "allow");
  const st = consent.status(id);
  assert.equal(st.apps["name:persisted-app"].source, "persisted");
  assert.equal(st.apps["name:session-app"].source, "session");
  assert.equal(st.apps["name:session-app"].decision, "deny");
  assert.equal(st.foreground.state, "allowed");
});

// ---------- wire: the gate, over the real server ----------

async function boot(t, env = {}, { hostKeys = true, attested = true, elicitation = null } = {}) {
  const stateDir = fs.mkdtempSync(path.join(os.tmpdir(), "cu-consent-srv-"));
  const recDir = fs.mkdtempSync(path.join(os.tmpdir(), "cu-consent-rec-"));
  const child = spawn("node", [path.join(ROOT, "mcp", "server.mjs")], {
    env: { ...process.env, CODEWHALE_CU_STATE_DIR: stateDir, CODEWHALE_CU_RECORDINGS_DIR: recDir, CODEWHALE_CU_APP: "off", CODEWHALE_CU_TEST_BACKEND: path.join(ROOT, "tests", "fixtures", "fake-backend.mjs"), ...env },
    stdio: ["pipe", "pipe", "pipe"],
  });
  if (hostKeys) child.stdin.write(hostKeysLine());
  t.after(() => { try { child.stdin.end(); } catch {} child.kill("SIGTERM"); fs.rmSync(stateDir, { recursive: true, force: true }); fs.rmSync(recDir, { recursive: true, force: true }); });
  let buf = "";
  const pending = new Map();
  let nextId = 1;
  child.stdout.on("data", (c) => {
    buf += c.toString();
    let i;
    while ((i = buf.indexOf("\n")) !== -1) {
      const line = buf.slice(0, i).trim();
      buf = buf.slice(i + 1);
      if (!line) continue;
      const msg = JSON.parse(line);
      if (msg.method === "elicitation/create" && elicitation) {
        if (typeof elicitation === "function") elicitation(msg);
        else child.stdin.write(JSON.stringify({ jsonrpc: "2.0", id: msg.id, result: { action: elicitation } }) + "\n");
        continue;
      }
      if (msg.id != null && pending.has(msg.id)) { pending.get(msg.id)(msg); pending.delete(msg.id); }
    }
  });
  const rpc = (method, params, timeoutMs = 20_000) => {
    const id = nextId++;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => { pending.delete(id); reject(new Error(`timeout: ${method}`)); }, timeoutMs);
      pending.set(id, (msg) => { clearTimeout(timer); resolve(msg); });
      child.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params: attested ? attestParams(method, params) : params }) + "\n");
    });
  };
  const tool = async (name, args = {}, timeoutMs) => JSON.parse((await rpc("tools/call", { name, arguments: args }, timeoutMs)).result.content[0].text);
  const send = (params) => rpc("tools/call", params).then((r) => JSON.parse(r.result.content[0].text));
  const sendWire = message => child.stdin.write(JSON.stringify(message) + "\n");
  return { rpc, tool, send, sendWire, stateDir };
}

test("first app contact refuses consent_required before any backend work", async (t) => {
  const s = await boot(t);
  const r = await s.tool("open_application", { name: "FakeApp" });
  assert.equal(r.ok, false);
  assert.equal(r.error.code, "consent_required");
  assert.match(r.error.message, /consent \{action:"allow"\|"deny"/);
  const st = await s.tool("consent", { action: "status" });
  assert.equal(st.ok, true);
  assert.deepEqual(st.apps, {});
});

test("a deny cannot be sidestepped by re-spelling the same app", async (t) => {
  const s = await boot(t);
  // The recording backend resolves the alias without opening a real app.
  await s.tool("consent", { action: "allow", app: "FakeApp" });
  const opened = await s.tool("open_application", { name: "FakeApp" });
  assert.equal(opened.ok, true);
  const denied = await s.tool("consent", { action: "deny", app: "FakeApp" });
  assert.equal(denied.ok, true);
  assert.equal(denied.decision, "deny");
  for (const args of [{ name: "FakeApp" }, { bundle_id: "com.fake.app" }, { name: "FakeApp.app" }]) {
    const r = await s.tool("open_application", args);
    assert.equal(r.error?.code, "app_denied", JSON.stringify(args));
  }
  // A destructive tool honors the same deny — it cannot terminate the app.
  const kill = await s.tool("kill_app", { name: "FakeApp" });
  assert.equal(kill.error?.code, "app_denied");
});

test("allow opens; activate:true is a separate foreground consent", async (t) => {
  const s = await boot(t);
  await s.tool("consent", { action: "allow", app: "FakeApp" });
  const fg = await s.tool("open_application", { name: "FakeApp", activate: true });
  assert.equal(fg.error?.code, "foreground_consent_required");
  const deniedFg = await s.tool("consent", { action: "deny", scope: "foreground" });
  assert.equal(deniedFg.scope, "foreground");
  const again = await s.tool("open_application", { name: "FakeApp", activate: true });
  assert.equal(again.error?.code, "foreground_denied");
  await s.tool("consent", { action: "allow", scope: "foreground" });
  const opened = await s.tool("open_application", { name: "FakeApp", activate: true });
  assert.equal(opened.ok, true);
  assert.equal(opened.shared_pointer, true);
  // Background re-open needs no foreground consent — the bound app carries it.
  const bg = await s.tool("open_application", { name: "FakeApp", activate: false });
  assert.equal(bg.ok, true);
});

test("foreground consent gates activate:true on every local platform, not just macOS", async (t) => {
  const s = await boot(t, { CODEWHALE_CU_TEST_BACKEND: path.join(ROOT, "tests", "fixtures", "fake-backend.mjs") });
  await s.tool("consent", { action: "allow", app: "FakeApp" });
  const fg = await s.tool("open_application", { name: "FakeApp", activate: true });
  assert.equal(fg.error?.code, "foreground_consent_required", "the shared-surface escalation asks on every platform");
  await s.tool("consent", { action: "allow", scope: "foreground" });
  const opened = await s.tool("open_application", { name: "FakeApp", activate: true });
  assert.equal(opened.ok, true, JSON.stringify(opened));
  assert.equal(opened.shared_pointer, true);
});

test("remember:true persists; consent status shows the ledger", async (t) => {
  const s = await boot(t);
  const r = await s.tool("consent", { action: "allow", app: "Finder", remember: true });
  assert.equal(r.persisted, true);
  const file = JSON.parse(fs.readFileSync(path.join(s.stateDir, "consent.json"), "utf8"));
  assert.equal(file.computers.local.apps["name:finder"].decision, "allow");
  const st = await s.tool("consent", { action: "status" });
  assert.equal(st.apps["name:finder"].source, "persisted");
  const revoked = await s.tool("consent", { action: "revoke", app: "Finder" });
  assert.equal(revoked.ok, true);
  assert.equal(consent.decisionFor("local", ["name:finder"]).state, "undecided");
});

test("remote computers are covered by the transport, not the app ledger", async (t) => {
  const s = await boot(t);
  const reg = await s.tool("computer", { action: "register", id: "faraway", transport: "ssh", host: "192.0.2.1", installAgent: false });
  assert.equal(reg.ok, true);
  const st = await s.tool("consent", { action: "status", computer: "faraway" });
  assert.equal(st.ok, true);
  // open_application may fail at transport level — never at consent.
  const r = await s.tool("open_application", { name: "x", computer: "faraway" }, 40_000);
  assert.notEqual(r.error?.code, "consent_required");
  assert.notEqual(r.error?.code, "app_denied");
});

test("spawned computers are task-owned — the app ledger never gates them", { skip: NEED_DOCKER.skip }, async (t) => {
  const s = await boot(t);
  const id = `consent-${Date.now()}`;
  const spawned = await s.tool("computer", { action: "spawn", id, transport: "docker" }, 60_000);
  assert.equal(spawned.ok, true, JSON.stringify(spawned));
  const r = await s.tool("open_application", { name: "xterm" });
  assert.notEqual(r.error?.code, "consent_required", "an owned computer must never consult the user's app ledger");
  const st = await s.tool("consent", { action: "status", computer: id });
  assert.equal(st.ok, true);
});

// ---------- the user's own decision ----------

test("consent allow without a host decision is refused", async (t) => {
  const s = await boot(t, {}, { attested: false });
  const r = await s.tool("consent", { action: "allow", app: "FakeApp" });
  assert.equal(r.ok, false);
  assert.equal(r.error.code, "consent_needs_user");
  for (const [name, args] of [["consent_allow", { app: "FakeApp" }], ["consent", { action: "revoke", app: "FakeApp" }], ["app_script", { script: "return 1" }], ["computer_register", { computer: "box", transport: "local" }]]) {
    const refused = await s.tool(name, args);
    assert.equal(refused.error?.code, "consent_needs_user", name);
  }
  // Narrowing needs no decision.
  assert.equal((await s.tool("consent", { action: "deny", app: "OtherApp" })).ok, true);
  const opened = await s.tool("open_application", { name: "FakeApp" });
  assert.equal(opened.error?.code, "consent_required");
});

test("consent allow with a valid decision MAC succeeds; replayed nonce is refused", async (t) => {
  const s = await boot(t, {}, { attested: false });
  const params = attest({ name: "consent", arguments: { action: "allow", app: "FakeApp" } });
  assert.equal((await s.send(params)).ok, true);
  const replayed = await s.send(params);
  assert.equal(replayed.error?.code, "consent_needs_user");
  // A MAC for other arguments does not cover these.
  const forged = attest({ name: "consent", arguments: { action: "allow", app: "OtherApp" } });
  forged.arguments = { action: "allow", app: "Terminal" };
  assert.equal((await s.send(forged)).error?.code, "consent_needs_user");
  // A MAC under another key does not verify.
  const wrongKey = attest({ name: "consent", arguments: { action: "allow", app: "Terminal" } }, "33".repeat(32));
  assert.equal((await s.send(wrongKey)).error?.code, "consent_needs_user");
});

test("a decision is refused when the host sent no key, and keys are read only from the first message", async (t) => {
  const s = await boot(t, {}, { hostKeys: false });
  // Keys arriving after the first message are ignored.
  await s.rpc("ping", {});
  const late = await s.rpc("codewhale/host_keys", { decision_key: "11".repeat(32) }, 300).catch(() => null);
  void late;
  const r = await s.tool("consent", { action: "allow", app: "FakeApp" });
  assert.equal(r.error?.code, "consent_needs_user");
});

test("a client with elicitation decides for other hosts", async (t) => {
  for (const [answer, ok] of [["accept", true], ["decline", false]]) {
    const s = await boot(t, {}, { hostKeys: false, attested: false, elicitation: answer });
    await s.rpc("initialize", { protocolVersion: "2025-06-18", capabilities: { elicitation: {} } });
    const r = await s.tool("consent", { action: "allow", app: "FakeApp" });
    assert.equal(r.ok, ok, `${answer}: ${JSON.stringify(r)}`);
    if (!ok) assert.equal(r.error.code, "consent_declined");
  }
});

test("planted consent.json allow entry without MAC is ignored; deny is honored", () => {
  const dir = freshDir();
  const id = cid();
  const at = new Date().toISOString();
  const good = { decision: "allow", at };
  fs.writeFileSync(path.join(dir, "consent.json"), JSON.stringify({ version: 1, computers: { [id]: {
    apps: {
      "name:planted": { decision: "allow", at },
      "name:forged": { decision: "allow", at, mac: "00".repeat(32) },
      "name:signed": { ...good, mac: ledgerMac(id, "name:signed", good) },
      "name:blocked": { decision: "deny", at },
    },
    foreground: { decision: "allow", at },
  } } }));
  assert.equal(consent.decisionFor(id, ["name:planted"]).state, "undecided");
  assert.equal(consent.decisionFor(id, ["name:forged"]).state, "undecided");
  assert.equal(consent.decisionFor(id, ["name:signed"]).state, "allowed");
  assert.equal(consent.decisionFor(id, ["name:blocked"]).state, "denied");
  assert.equal(consent.foregroundDecision(id).state, "undecided");
});

test("confirm token requires a host decision", async (t) => {
  const s = await boot(t, {}, { attested: false });
  const r = await s.tool("consent", { action: "allow", confirm: "confirm-000000000000000000" });
  assert.equal(r.error?.code, "consent_needs_user");
});

test("run_actions app_script step needs a host decision", async (t) => {
  const s = await boot(t);
  const r = await s.tool("run_actions", { steps: [{ tool: "app_script", arguments: { script: "return 1" } }] });
  assert.equal(r.ok, false);
  assert.equal(r.error.code, "consent_needs_user");
});


test("cancelled user elicitation releases dispatch and cannot grant consent later", async (t) => {
  let sawPrompt;
  const prompt = new Promise(resolve => { sawPrompt = resolve; });
  const s = await boot(t, {}, { hostKeys: false, attested: false, elicitation: sawPrompt });
  await s.rpc("initialize", { protocolVersion: "2025-06-18", capabilities: { elicitation: {} } });
  s.sendWire({ jsonrpc: "2.0", id: "cancelled-consent", method: "tools/call", params: { name: "consent", arguments: { action: "allow", app: "FakeApp" } } });
  const request = await Promise.race([prompt, new Promise((_, reject) => setTimeout(() => reject(new Error("elicitation was not shown")), 2_000))]);
  s.sendWire({ jsonrpc: "2.0", method: "notifications/cancelled", params: { requestId: "cancelled-consent" } });
  // The unanswered card no longer holds the serialized tool queue.
  const before = await s.tool("consent", { action: "status" }, 1_000);
  assert.deepEqual(before.apps, {});
  s.sendWire({ jsonrpc: "2.0", id: request.id, result: { action: "accept" } });
  const after = await s.tool("consent", { action: "status" }, 1_000);
  assert.deepEqual(after.apps, {}, "a late accept cannot grant the cancelled request");
});
