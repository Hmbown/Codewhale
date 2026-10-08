// Registry tests: registration validation, switching, removal, persistence.
import { test, beforeEach } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "cu-reg-test-"));
process.env.CODEWHALE_CU_STATE_DIR = tmp;

const registry = await import("../src/registry.mjs");

beforeEach(() => {
  fs.rmSync(tmp, { recursive: true, force: true });
  fs.mkdirSync(tmp, { recursive: true });
});

test("local computer is always registered and active by default", () => {
  const reg = registry.list();
  assert.equal(reg.active, "local");
  assert.equal(reg.computers.local.transport, "local");
  assert.equal(reg.computers.local.platform, process.platform);
});

test("register accepts valid ssh and hdc computers and persists them", () => {
  registry.register({ id: "winbox", transport: "ssh", host: "winbox.lan", user: "me", port: 2222 });
  registry.register({ id: "pad", transport: "hdc", target: "ABC123" });
  const reg = registry.list();
  assert.equal(reg.computers.winbox.host, "winbox.lan");
  assert.equal(reg.computers.pad.platform, "harmonyos");
  // persisted across a fresh load
  const again = registry.load();
  assert.ok(again.computers.winbox && again.computers.pad);
});

test("register rejects invalid ids, hosts, ports, transports", () => {
  assert.throws(() => registry.register({ id: "bad id!", transport: "ssh", host: "h" }), (e) => e.code === "invalid_id");
  assert.throws(() => registry.register({ id: "x", transport: "carrier-pigeon" }), (e) => e.code === "invalid_transport");
  assert.throws(() => registry.register({ id: "x", transport: "ssh", host: "bad host;rm -rf" }), (e) => e.code === "invalid_host");
  assert.throws(() => registry.register({ id: "x", transport: "ssh", host: "h", port: 99_999 }), (e) => e.code === "invalid_port");
  assert.throws(() => registry.register({ id: "local", transport: "ssh", host: "h" }), (e) => e.code === "reserved_id");
});

test("switchTo validates and persists the active computer", () => {
  registry.register({ id: "pad", transport: "hdc" });
  registry.switchTo("pad");
  assert.equal(registry.active().id, "pad");
  assert.throws(() => registry.switchTo("nope"), (e) => e.code === "unknown_computer");
});

test("remove falls back to local when removing the active computer", () => {
  registry.register({ id: "pad", transport: "hdc" });
  registry.switchTo("pad");
  const res = registry.remove("pad");
  assert.equal(res.active, "local");
  assert.throws(() => registry.remove("local"), (e) => e.code === "reserved_id");
  assert.throws(() => registry.remove("pad"), (e) => e.code === "unknown_computer");
});

test("ssh hosts or users that read as options are refused at registration and ignored on load", async () => {
  const fsMod = await import("node:fs");
  const os = await import("node:os");
  const pathMod = await import("node:path");
  const dir = fsMod.mkdtempSync(pathMod.join(os.tmpdir(), "cu-registry-"));
  const previous = process.env.CODEWHALE_CU_STATE_DIR;
  process.env.CODEWHALE_CU_STATE_DIR = dir;
  try {
    const registry = await import("../src/registry.mjs");
    assert.throws(() => registry.register({ id: "bad", transport: "ssh", host: "-Fx" }), { code: "invalid_host" });
    assert.throws(() => registry.register({ id: "bad2", transport: "ssh", host: "ok.example", user: "-oProxyCommand=x" }), { code: "invalid_user" });
    fsMod.writeFileSync(pathMod.join(dir, "computers.json"), JSON.stringify({
      version: 1,
      active: "planted",
      computers: {
        planted: { id: "planted", transport: "ssh", host: "-Fx" },
        fine: { id: "fine", transport: "ssh", host: "ok.example" },
      },
    }));
    const listed = registry.list();
    assert.equal(listed.computers.planted, undefined);
    assert.ok(listed.computers.fine);
    assert.equal(listed.active, "local");
  } finally {
    if (previous === undefined) delete process.env.CODEWHALE_CU_STATE_DIR;
    else process.env.CODEWHALE_CU_STATE_DIR = previous;
  }
});
