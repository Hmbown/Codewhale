// Skill pack over MCP (resources + skills methods), tool annotations, and the
// pure helpers behind list_apps filtering, menu targeting and native error
// codes. The protocol part spawns the real server; state is isolated.
import { test, before, after } from "node:test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import url from "node:url";
import { TOOLS } from "../src/tools.mjs";
import { pickMenuElement, selectApps, nativeErrorCode } from "../src/backends/darwin.mjs";

const __dirname = path.dirname(url.fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, "..");

// ---------- pure helpers ----------

test("every tool carries MCP annotations, and the observation/action split holds", () => {
  assert.equal(TOOLS.length, TOOLS.filter((t) => t.annotations).length, "a tool without annotations would let a host guess");
  for (const t of TOOLS) {
    for (const key of ["readOnlyHint", "destructiveHint", "idempotentHint", "openWorldHint"]) {
      assert.equal(typeof t.annotations[key], "boolean", `${t.name}.annotations.${key} must be a boolean`);
    }
  }
  const byName = Object.fromEntries(TOOLS.map((t) => [t.name, t.annotations]));
  assert.equal(byName.get_app_state.readOnlyHint, true);
  assert.equal(byName.screenshot.readOnlyHint, true);
  assert.equal(byName.request_access.readOnlyHint, true);
  assert.equal(byName.left_click.readOnlyHint, false);
  assert.equal(byName.left_click.destructiveHint, true);
  assert.equal(byName.stop_computer_control.readOnlyHint, false);
  assert.equal(byName.invoke_menu.readOnlyHint, false);
  assert.equal(byName.list_apps.readOnlyHint, true);
});

test("invoke_menu and list_apps advertise their new surfaces", () => {
  const menu = TOOLS.find((t) => t.name === "invoke_menu");
  assert.ok(menu, "invoke_menu is part of the surface");
  assert.deepEqual(menu.inputSchema.required, ["path"]);
  assert.equal(menu.inputSchema.properties.path.maxItems, 3);
  const apps = TOOLS.find((t) => t.name === "list_apps");
  assert.equal(apps.inputSchema.properties.all.type, "boolean");
});

test("selectApps keeps regular apps by default, passes everything with all:true, and tolerates an old helper", () => {
  const apps = [
    { name: "Finder", activation_policy: "regular" },
    { name: "Terminal", activation_policy: "regular", frontmost: true },
    { name: "chmod", activation_policy: "prohibited" },
    { name: "SwiftBar", activation_policy: "accessory" },
  ];
  assert.deepEqual(selectApps(apps, false).map((a) => a.name), ["Finder", "Terminal"]);
  assert.equal(selectApps(apps, true).length, 4);
  // A pre-0.6.2 helper does not report the field: return the list whole
  // rather than hiding every app.
  const legacy = [{ name: "Finder" }, { name: "chmod" }];
  assert.equal(selectApps(legacy, false).length, 2);
});

test("pickMenuElement matches exact titles and roles only", () => {
  const els = [
    { role: "AXMenuBarItem", label: "File", path: [3] },
    { role: "AXMenuItem", label: "New Window", path: [3, 0, 1] },
    { role: "AXMenuItem", label: "New", path: [3, 0, 0] },
  ];
  assert.equal(pickMenuElement(els, "File", true)?.label, "File");
  assert.equal(pickMenuElement(els, "New", false)?.path[2], 0, "exact match must not take \u201cNew Window\u201d");
  assert.equal(pickMenuElement(els, "Open\u2026", true), null, "no fuzzy matches");
  assert.equal(pickMenuElement(els, "File", false), null, "role must match the level");
});

test("nativeErrorCode maps refusal reasons to stable codes", () => {
  assert.equal(nativeErrorCode("the selected window is ambiguous; observe the app windows again"), "window_ambiguous");
  assert.equal(nativeErrorCode("the selected app window is not capturable; observe the app windows again"), "window_not_capturable");
  assert.equal(nativeErrorCode("application not found"), "app_not_found");
  assert.equal(nativeErrorCode("no running application with pid 42"), "app_not_found");
  assert.equal(nativeErrorCode("accessibility action failed: -25205"), null);
});

// ---------- protocol: the bundled skill pack ----------

const stateDir = fs.mkdtempSync(path.join(os.tmpdir(), "cu-skill-state-"));
let server;
let buf = "";
const pending = new Map();
let nextId = 1;

function rpc(method, params) {
  const id = nextId++;
  return new Promise((resolve, reject) => {
    const t = setTimeout(() => { pending.delete(id); reject(new Error(`timeout: ${method}`)); }, 15_000);
    pending.set(id, (msg) => { clearTimeout(t); resolve(msg); });
    server.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n");
  });
}

before(() => {
  server = spawn("node", [path.join(ROOT, "mcp", "server.mjs")], {
    env: { ...process.env, CODEWHALE_CU_STATE_DIR: stateDir, CODEWHALE_CU_RECORDINGS_DIR: path.join(stateDir, "rec") },
    stdio: ["pipe", "pipe", "pipe"],
  });
  server.stdout.on("data", (c) => {
    buf += c.toString();
    let i;
    while ((i = buf.indexOf("\n")) !== -1) {
      const line = buf.slice(0, i).trim();
      buf = buf.slice(i + 1);
      if (!line) continue;
      const msg = JSON.parse(line);
      if (msg.id != null && pending.has(msg.id)) { pending.get(msg.id)(msg); pending.delete(msg.id); }
    }
  });
});

after(() => { try { server.stdin.end(); } catch {} server?.kill("SIGTERM"); });

test("initialize advertises resources and the skills extension", async () => {
  const init = await rpc("initialize", { protocolVersion: "2025-06-18", capabilities: {} });
  assert.equal(init.result.capabilities.resources.listChanged, false);
  assert.ok(init.result.capabilities.experimental["io.modelcontextprotocol/skills"], "the skills extension is advertised");
});

test("resources/templates/list answers with an empty template list, never method-not-found", async () => {
  const res = await rpc("resources/templates/list", {});
  assert.equal(res.error, undefined, "a method implied by the advertised resources capability must not 404");
  assert.deepEqual(res.result.resourceTemplates, []);
});

test("resources/list names the pack; resources/read returns exact bytes with hashes", async () => {
  const list = await rpc("resources/list", {});
  const uris = list.result.resources.map((r) => r.uri);
  assert.ok(uris.includes("skill://codewhale-cu/SKILL.md"));
  assert.ok(uris.includes("skill://codewhale-cu/references/quick-reference.md"));
  assert.ok(uris.includes("skill://codewhale-cu/references/refusal-codes.md"));

  for (const uri of uris) {
    const read = await rpc("resources/read", { uri });
    const text = read.result.contents[0].text;
    const rel = uri.replace("skill://codewhale-cu/", "");
    const onDisk = fs.readFileSync(path.join(ROOT, "skills", "computer-use", rel), "utf8");
    assert.equal(text, onDisk, `${uri} must serve exactly the file on disk`);
  }
});

test("resources/read refuses unknown URIs with invalid-params, never a traversal", async () => {
  const bad = await rpc("resources/read", { uri: "skill://codewhale-cu/../../../etc/passwd" });
  assert.equal(bad.error.code, -32602);
  const alsoBad = await rpc("resources/read", { uri: "file:///etc/passwd" });
  assert.equal(alsoBad.error.code, -32602);
});

test("skills/list and skills/get carry the manifest with matching sha256 digests", async () => {
  const skills = await rpc("skills/list", {});
  const entry = skills.result.skills[0];
  assert.equal(entry.name, "computer-use");
  assert.ok(entry.description.length > 40, "the description comes from SKILL.md frontmatter");
  assert.equal(entry.files.length, 3);

  const got = await rpc("skills/get", { uri: "skill://codewhale-cu/SKILL.md" });
  assert.equal(got.result.skill.frontmatter.name, "computer-use");
  for (const file of got.result.manifest) {
    const rel = file.uri.replace("skill://codewhale-cu/", "");
    const bytes = fs.readFileSync(path.join(ROOT, "skills", "computer-use", rel));
    const digest = crypto.createHash("sha256").update(bytes).digest("hex");
    assert.equal(file.sha256, digest, `${rel} sha256 must match the bytes`);
    assert.equal(file.bytes, bytes.length);
  }
});

test("tools/list still answers after the resource methods (no dispatch regressions)", async () => {
  const tools = await rpc("tools/list", {});
  const names = tools.result.tools.map((t) => t.name);
  assert.ok(names.includes("invoke_menu"));
  assert.equal(new Set(names).size, names.length, "tool names stay unique");
});
