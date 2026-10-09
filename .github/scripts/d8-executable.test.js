const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const { test } = require("node:test");

const sourceRoot = path.resolve(__dirname, "../..");

function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "cw-d8-launch-"));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const repo = path.join(root, "source");
  fs.mkdirSync(path.join(repo, "scripts"), { recursive: true });
  for (const name of ["dev-test.sh", "dev-cache.sh", "dev-cargo.sh", "with-hermetic-test-home.sh", "build-lock.py"]) {
    fs.copyFileSync(path.join(sourceRoot, "scripts", name), path.join(repo, "scripts", name));
    fs.chmodSync(path.join(repo, "scripts", name), 0o755);
  }
  fs.writeFileSync(path.join(repo, "Cargo.toml"), '[workspace]\nmembers = []\n');
  const bin = path.join(root, "bin");
  const home = path.join(root, "ambient home");
  fs.mkdirSync(bin); fs.mkdirSync(home);
  const log = path.join(root, "cargo-invocations.jsonl");
  fs.writeFileSync(path.join(bin, "cargo"), `#!${process.execPath}

const fs = require("node:fs");
const args = process.argv.slice(2);
if (args.includes("--version")) { console.log("cargo 1.97.0 (source fixture)"); process.exit(0); }
fs.appendFileSync(process.env.D8_INVOCATIONS, JSON.stringify({args, home: process.env.HOME, cache: process.env.CODEWHALE_CACHE_ROOT}) + "\\n");
if (args.includes("build") && process.env.D8_BUILD_FAILURE) process.exit(Number(process.env.D8_BUILD_FAILURE));
console.log("test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s");
`);
  fs.writeFileSync(path.join(bin, "rustc"), `#!/bin/sh\nprintf '%s\n' 'rustc 1.97.0 (source fixture)' 'commit-hash: source-fixture'\n`);
  fs.writeFileSync(path.join(bin, "rustup"), `#!/bin/sh\nif [ "$1" = which ] && [ "$2" = rustc ]; then printf '%s\n' "$D8_TOOLCHAIN/rustc"; else exit 99; fi\n`);
  for (const name of ["cargo", "rustc", "rustup"]) fs.chmodSync(path.join(bin, name), 0o755);
  return {
    home, log,
    run(area, failure) {
      const result = spawnSync("sh", [path.join(repo, "scripts/dev-test.sh"), area, "filter with spaces"], {
        cwd: repo, encoding: "utf8",
        env: { ...process.env, PATH: `${bin}${path.delimiter}${process.env.PATH}`, HOME: home,
          USERPROFILE: home, CARGO_HOME: path.join(root, "cargo-home"), RUSTUP_HOME: path.join(root, "rustup-home"),
          CODEWHALE_DEV_NEXTEST: "0", CODEWHALE_BUILD_LOCK: "0", CODEWHALE_BUILD_LOCK_HELD: "",
          CODEWHALE_CACHE_ROOT: path.join(root, "cache"), D8_INVOCATIONS: log, D8_TOOLCHAIN: bin,
          D8_BUILD_FAILURE: failure ? String(failure) : "" },
      });
      const calls = fs.existsSync(log) ? fs.readFileSync(log, "utf8").trim().split("\n").map(JSON.parse) : [];
      return { result, calls };
    },
  };
}

for (const area of ["tui-integration", "tui-cucumber"]) {
  test(`${area} builds the canonical executable before isolated acceptance`, (t) => {
    const f = fixture(t); const { result, calls } = f.run(area);
    assert.equal(result.status, 0, result.stderr + result.stdout);
    assert.equal(calls.length, 2);
    const build = calls[0].args.slice(calls[0].args.indexOf("build"));
    assert.deepEqual(build, ["build", "-p", "codewhale-cli", "--bin", "codewhale", "--locked"]);
    const check = calls[1].args.slice(calls[1].args.indexOf("test"));
    assert.deepEqual(check, ["test", "-p", "codewhale-tui", "--test", area === "tui-integration" ? "integration" : "cucumber", "--locked", "filter with spaces"]);
    for (const call of calls) {
      assert.notEqual(call.home, f.home);
      assert.equal(fs.existsSync(call.home), false, "temporary test HOME is removed");
      assert.ok(call.cache.endsWith("cache"));
    }
  });
}

test("canonical build failure prevents acceptance and preserves its exit status", (t) => {
  const f = fixture(t); const { result, calls } = f.run("tui-integration", 39);
  assert.equal(result.status, 39, result.stderr + result.stdout);
  assert.equal(calls.length, 1);
  assert.ok(calls[0].args.includes("build"));
});

test("library-only verification does not build an executable", (t) => {
  const f = fixture(t); const { result, calls } = f.run("config");
  assert.equal(result.status, 0, result.stderr + result.stdout);
  assert.equal(calls.length, 1);
  assert.ok(calls[0].args.includes("test"));
  assert.ok(calls[0].args.includes("--lib"));
});
