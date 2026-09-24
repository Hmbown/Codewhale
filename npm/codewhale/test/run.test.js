const assert = require("node:assert/strict");
const test = require("node:test");

const { run, reportStartFailure, _internal } = require("../scripts/run");

test("version fallback handles only version flags", () => {
  assert.equal(_internal.isVersionFlag(["--version"]), true);
  assert.equal(_internal.isVersionFlag(["-V"]), true);
  assert.equal(_internal.isVersionFlag(["-v"]), false);
  assert.equal(_internal.isVersionFlag(["--verbose"]), false);
});

test("version flags prefer the installed binary over package metadata", async () => {
  let spawned = false;
  const exits = [];

  await run("codewhale", {
    args: ["--version"],
    getBinaryPath: async () => "/tmp/codewhale-test-binary",
    spawnSync: (binary, args, options) => {
      spawned = true;
      assert.equal(binary, "/tmp/codewhale-test-binary");
      assert.deepEqual(args, ["--version"]);
      assert.deepEqual(options, { stdio: "inherit" });
      return { status: 0 };
    },
    exit: (status) => {
      exits.push(status);
    },
  });

  assert.equal(spawned, true);
  assert.deepEqual(exits, [0]);
});

test("codew wrapper dispatches the native shortcut binary", async () => {
  const resolvedNames = [];
  const spawned = [];

  await run("codew", {
    args: ["--version"],
    getBinaryPath: async (name) => {
      resolvedNames.push(name);
      return "/tmp/codew-test-binary";
    },
    spawnSync: (binary, args) => {
      spawned.push({ binary, args });
      return { status: 0 };
    },
    exit: () => {},
  });

  assert.deepEqual(resolvedNames, ["codew"]);
  assert.deepEqual(spawned, [
    { binary: "/tmp/codew-test-binary", args: ["--version"] },
  ]);
});

test("version flags fall back to package metadata when the binary is unavailable", async () => {
  const originalLog = console.log;
  const originalError = console.error;
  const lines = [];
  const errors = [];
  const exits = [];
  console.log = (line) => lines.push(line);
  console.error = (...parts) => errors.push(parts.join(" "));
  try {
    await run("codewhale", {
      args: ["--version"],
      getBinaryPath: async () => {
        throw Object.assign(new Error("getaddrinfo ENOTFOUND github.com"), {
          code: "ENOTFOUND",
        });
      },
      spawnSync: () => {
        throw new Error("spawn should not run without a binary");
      },
      exit: (status) => {
        exits.push(status);
      },
    });
  } finally {
    console.log = originalLog;
    console.error = originalError;
  }

  assert.deepEqual(exits, [0]);
  assert.match(lines.join("\n"), /codewhale \(npm wrapper\) v/);
  // The fallback must not claim a binary version that is not installed.
  assert.doesNotMatch(lines.join("\n"), /binary version: v/);
  assert.match(lines.join("\n"), /binary: not installed \(expected v[^)]+\)/);
  const stderr = errors.join("\n");
  assert.match(stderr, /ENOTFOUND github\.com/);
  assert.match(stderr, /codewhale install hint:/);
});

test("start failures print the install hint for download errors", () => {
  const logged = [];
  const log = (...parts) => logged.push(parts.join(" "));

  reportStartFailure(
    "codew",
    Object.assign(new Error("download stalled"), { code: "EDOWNLOADTIMEOUT" }),
    log,
  );
  const output = logged.join("\n");
  assert.match(output, /^Failed to start codew: download stalled/);
  assert.match(output, /codewhale install hint:/);
  assert.match(
    output,
    /https:\/\/github\.com\/Hmbown\/CodeWhale\/blob\/main\/docs\/INSTALL\.md#npm-binary-download-times-out/,
  );

  logged.length = 0;
  reportStartFailure("codewhale", new Error("permission denied"), log);
  assert.deepEqual(logged, ["Failed to start codewhale: permission denied"]);
});
