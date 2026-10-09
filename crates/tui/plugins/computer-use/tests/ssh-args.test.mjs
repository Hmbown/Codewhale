import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import url from "node:url";
import { sshArgv, sshOptions, validateSshTarget } from "../src/ssh-args.mjs";

const here = path.dirname(url.fileURLToPath(import.meta.url));
const vectors = JSON.parse(fs.readFileSync(path.join(here, "fixtures", "ssh-destinations.json"), "utf8"));

test("transport ssh argv pins known hosts and ends options", () => {
  const argv = sshArgv({ host: "builder.example.test", user: "fleet", port: 2222, knownHosts: "/etc/cu_known_hosts" });
  for (const option of ["StrictHostKeyChecking=yes", "UpdateHostKeys=no", "UserKnownHostsFile=/etc/cu_known_hosts", `GlobalKnownHostsFile=${process.platform === "win32" ? "NUL" : "/dev/null"}`, "BatchMode=yes"]) {
    const at = argv.indexOf(option);
    assert.ok(at > 0, `missing ${option}: ${argv.join(" ")}`);
    assert.equal(argv[at - 1], "-o");
  }
  assert.ok(!argv.some((arg) => arg.includes("accept-new")));
  const end = argv.indexOf("--");
  assert.equal(argv[end + 1], "fleet@builder.example.test");
  assert.equal(end + 2, argv.length, "the destination is last; the remote command follows it");
  assert.deepEqual(sshOptions({ host: "h", port: 2200 }, { portFlag: "-P" }).slice(-2), ["-P", "2200"]);
});

test("ssh destinations follow the shared vectors", () => {
  for (const entry of vectors.valid) assert.doesNotThrow(() => validateSshTarget(entry), JSON.stringify(entry));
  for (const entry of vectors.invalid) assert.throws(() => validateSshTarget(entry), JSON.stringify(entry));
});

test("known-hosts paths use the native filesystem and cannot expand into config tokens", () => {
  const file = path.resolve("trusted-hosts");
  const argv = sshArgv({ host: "fixture.test", knownHosts: file });
  const normalized = process.platform === "win32" ? file.replaceAll("\\", "/") : file;
  assert.ok(argv.includes(`UserKnownHostsFile=${normalized}`));
  for (const knownHosts of ["relative-hosts", path.resolve("two files"), path.resolve('quoted"hosts')]) {
    assert.throws(() => sshArgv({ host: "fixture.test", knownHosts }), /invalid_known_hosts|absolute path/);
  }
});
