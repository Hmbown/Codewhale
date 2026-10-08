const assert = require("node:assert/strict");
const { spawn } = require("node:child_process");
const { EventEmitter } = require("node:events");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const test = require("node:test");

const { run, reportStartFailure, _internal } = require("../scripts/run");

// A stand-in for a spawned child that exits on the next tick.
function exitingChild(status, signal = null) {
  const child = new EventEmitter();
  child.kill = () => true;
  process.nextTick(() => child.emit("exit", status, signal));
  return child;
}

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
    spawn: (binary, args, options) => {
      spawned = true;
      assert.equal(binary, "/tmp/codewhale-test-binary");
      assert.deepEqual(args, ["--version"]);
      assert.deepEqual(options, { stdio: "inherit" });
      return exitingChild(0);
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
    spawn: (binary, args) => {
      spawned.push({ binary, args });
      return exitingChild(0);
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
      spawn: () => {
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
    /https:\/\/github\.com\/codewhale-hq\/CodeWhale\/blob\/main\/docs\/INSTALL\.md#npm-binary-download-times-out/,
  );

  logged.length = 0;
  reportStartFailure("codewhale", new Error("permission denied"), log);
  assert.deepEqual(logged, ["Failed to start codewhale: permission denied"]);
});

test("termination signals reach the native child and its status is kept", async () => {
  const proc = new EventEmitter();
  const child = new EventEmitter();
  const killed = [];
  child.kill = (signal) => {
    killed.push(signal);
    process.nextTick(() => child.emit("exit", 143, null));
    return true;
  };
  const exits = [];
  const running = run("codewhale", {
    args: ["exec", "--auto", "task"],
    getBinaryPath: async () => "/tmp/codewhale-test-binary",
    spawn: () => child,
    exit: (status) => exits.push(status),
    process: proc,
  });
  await new Promise((resolve) => setImmediate(resolve));
  // Terminal signals are outlived, never forwarded: the terminal already
  // delivered them to the child through the process group.
  for (const signal of ["SIGINT", "SIGHUP"]) {
    assert.equal(proc.listenerCount(signal), 1, signal);
    proc.emit(signal);
  }
  assert.deepEqual(killed, []);
  const forwarded = _internal.FORWARDED_SIGNALS;
  assert.deepEqual(forwarded, process.platform === "win32" ? [] : ["SIGTERM"]);
  if (forwarded.length === 0) {
    child.emit("exit", 0, null);
    await running;
    assert.deepEqual(exits, [0]);
    return;
  }
  assert.equal(proc.listenerCount("SIGTERM"), 1);
  proc.emit("SIGTERM");
  await running;

  assert.deepEqual(killed, ["SIGTERM"]);
  assert.deepEqual(exits, [143]);
  for (const signal of ["SIGINT", "SIGTERM", "SIGHUP"]) {
    assert.equal(proc.listenerCount(signal), 0, signal);
  }
});

test("a child killed by a signal is reported as that signal", async () => {
  const proc = new EventEmitter();
  const raised = [];
  proc.pid = 4242;
  proc.kill = (pid, signal) => raised.push({ pid, signal });
  const exits = [];
  await run("codewhale", {
    args: [],
    getBinaryPath: async () => "/tmp/codewhale-test-binary",
    spawn: () => exitingChild(null, "SIGTERM"),
    exit: (status) => exits.push(status),
    process: proc,
  });
  assert.deepEqual(raised, [{ pid: 4242, signal: "SIGTERM" }]);
  assert.deepEqual(exits, [128 + os.constants.signals.SIGTERM]);
});

test(
  "SIGTERM to the wrapper process does not leave the native child running",
  { skip: process.platform === "win32" && "POSIX signals only", timeout: 20000 },
  async () => {
    const dir = await fs.promises.mkdtemp(path.join(os.tmpdir(), "codewhale-run-signal-"));
    const pidFile = path.join(dir, "child.pid");
    const childScript =
      `require("fs").writeFileSync(${JSON.stringify(pidFile)}, String(process.pid));` +
      "setInterval(() => {}, 1000);";
    const wrapperScript =
      `require(${JSON.stringify(path.join(__dirname, "..", "scripts", "run.js"))})` +
      `.run("codewhale", { getBinaryPath: async () => process.execPath, ` +
      `args: ["-e", ${JSON.stringify(childScript)}] });`;
    const wrapper = spawn(process.execPath, ["-e", wrapperScript], { stdio: "ignore" });
    let childPid = null;
    const alive = (pid) => {
      try {
        process.kill(pid, 0);
        return true;
      } catch {
        return false;
      }
    };
    try {
      const deadline = Date.now() + 10000;
      while (childPid === null && Date.now() < deadline) {
        try {
          childPid = Number.parseInt(fs.readFileSync(pidFile, "utf8"), 10) || null;
        } catch {
          await new Promise((resolve) => setTimeout(resolve, 25));
        }
      }
      assert.ok(childPid, "native child should have started");
      const wrapperExit = new Promise((resolve) => wrapper.once("exit", resolve));
      wrapper.kill("SIGTERM");
      await wrapperExit;
      const settle = Date.now() + 5000;
      while (alive(childPid) && Date.now() < settle) {
        await new Promise((resolve) => setTimeout(resolve, 25));
      }
      assert.equal(alive(childPid), false, "native child outlived the wrapper");
    } finally {
      if (childPid && alive(childPid)) process.kill(childPid, "SIGKILL");
      if (wrapper.exitCode === null && wrapper.signalCode === null) wrapper.kill("SIGKILL");
      await fs.promises.rm(dir, { recursive: true, force: true });
    }
  },
);

test(
  "Ctrl-C to the terminal's process group reaches the native child once",
  { skip: process.platform === "win32" && "POSIX process groups only", timeout: 20000 },
  async () => {
    const dir = await fs.promises.mkdtemp(path.join(os.tmpdir(), "codewhale-run-sigint-"));
    const pidFile = path.join(dir, "child.pid");
    const logFile = path.join(dir, "signals.log");
    // Count every SIGINT, then exit the way a Ctrl-C'd CLI does.
    const childScript =
      `const fs = require("fs");` +
      `process.on("SIGINT", () => { fs.appendFileSync(${JSON.stringify(logFile)}, "INT\\n");` +
      ` setTimeout(() => process.exit(130), 300); });` +
      `fs.writeFileSync(${JSON.stringify(pidFile)}, String(process.pid));` +
      "setInterval(() => {}, 1000);";
    const wrapperScript =
      `require(${JSON.stringify(path.join(__dirname, "..", "scripts", "run.js"))})` +
      `.run("codewhale", { getBinaryPath: async () => process.execPath, ` +
      `args: ["-e", ${JSON.stringify(childScript)}] });`;
    // `detached` makes the wrapper a process-group leader, standing in for the
    // terminal's foreground job; the native child joins that group.
    const wrapper = spawn(process.execPath, ["-e", wrapperScript], {
      stdio: "ignore",
      detached: true,
    });
    let childPid = null;
    try {
      const deadline = Date.now() + 10000;
      while (childPid === null && Date.now() < deadline) {
        try {
          childPid = Number.parseInt(fs.readFileSync(pidFile, "utf8"), 10) || null;
        } catch {
          await new Promise((resolve) => setTimeout(resolve, 25));
        }
      }
      assert.ok(childPid, "native child should have started");
      const wrapperExit = new Promise((resolve) =>
        wrapper.once("exit", (code, signal) => resolve({ code, signal })),
      );
      process.kill(-wrapper.pid, "SIGINT");
      const ended = await wrapperExit;
      const received = fs.readFileSync(logFile, "utf8").trim().split("\n");
      assert.deepEqual(received, ["INT"], "the child must see one SIGINT, not a forwarded copy");
      assert.deepEqual(ended, { code: 130, signal: null });
    } finally {
      try {
        process.kill(-wrapper.pid, "SIGKILL");
      } catch {
        // already gone
      }
      await fs.promises.rm(dir, { recursive: true, force: true });
    }
  },
);
