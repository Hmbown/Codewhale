const { spawn: spawnChild } = require("child_process");
const os = require("os");
const { getBinaryPath, installFailureHint } = require("./install");

const pkg = require("../package.json");

function isVersionFlag(args = process.argv.slice(2)) {
  return args.includes("--version") || args.includes("-V");
}

// Print the install hint (mirror / release-base guidance) for a failure that
// looks like a download problem. Prints nothing for other failures.
function printInstallFailureHint(error, log = console.error) {
  const hint = installFailureHint(error);
  if (hint) {
    log(hint);
  }
}

// Shared by the `codewhale` and `codew` bin shims: the error and, when the
// binary could not be downloaded, the hint that says what to do about it.
function reportStartFailure(binaryName, error, log = console.error) {
  log(`Failed to start ${binaryName}:`, error && error.message ? error.message : String(error));
  printInstallFailureHint(error, log);
}

// `--version` must still answer when the native binary is missing, but it
// must not report a binary version that is not actually installed: the
// expected version goes to stdout labelled as such, the failure to stderr.
function printVersionFallback(binaryName, error) {
  const binVersion =
    process.env.CODEWHALE_VERSION ||
    process.env.DEEPSEEK_TUI_VERSION ||
    process.env.DEEPSEEK_VERSION ||
    pkg.codewhaleBinaryVersion || pkg.deepseekBinaryVersion || pkg.version;
  console.log(`${binaryName} (npm wrapper) v${pkg.version}`);
  console.log(`binary: not installed (expected v${binVersion})`);
  console.log(`repo: ${pkg.repository?.url || "N/A"}`);
  if (error) {
    console.error(`${binaryName}: native binary unavailable: ${error.message || String(error)}`);
    printInstallFailureHint(error);
  }
}

// Signals that end the wrapper must end the native binary too. With a
// blocking spawn the wrapper died on SIGTERM and left the child running (still
// executing tools, still holding its port); as PID 1 in a container it ignored
// SIGTERM entirely. So SIGTERM, which a terminal never sends, is forwarded.
//
// SIGINT (Ctrl-C) and SIGHUP (terminal hangup) come from the terminal, which
// delivers them to the whole foreground process group, native child included.
// Forwarding them would deliver each one twice, and the native binary treats a
// second signal as "skip the graceful drain". On Windows `child.kill()` is a
// hard TerminateProcess, which would race the child's own Ctrl-C cleanup. The
// wrapper only has to outlive them, then mirror how the child ended.
const FORWARDED_SIGNALS = process.platform === "win32" ? [] : ["SIGTERM"];
const OUTLIVED_SIGNALS = ["SIGINT", "SIGHUP"];

function ignoreSignal() {}

// Run the native binary and settle with how it ended. While it runs, the
// forwarded signals go to the child and the outlived ones do not kill the
// wrapper.
function runChild(spawn, binaryPath, args, proc) {
  return new Promise((resolve) => {
    let child;
    try {
      child = spawn(binaryPath, args, { stdio: "inherit" });
    } catch (error) {
      resolve({ error });
      return;
    }
    const listeners = [
      ...FORWARDED_SIGNALS.map((signal) => [
        signal,
        () => {
          try {
            child.kill(signal);
          } catch {
            // The child already exited; its exit event settles the run.
          }
        },
      ]),
      ...OUTLIVED_SIGNALS.map((signal) => [signal, ignoreSignal]),
    ];
    for (const [signal, listener] of listeners) {
      proc.on(signal, listener);
    }
    let settled = false;
    const settle = (result) => {
      if (settled) return;
      settled = true;
      for (const [signal, listener] of listeners) {
        proc.removeListener(signal, listener);
      }
      resolve(result);
    };
    child.once("error", (error) => settle({ error }));
    child.once("exit", (status, signal) => settle({ status, signal }));
  });
}

// Mirror a signal death to the parent shell: re-raise it on the wrapper once
// our listeners are gone, and fall back to the conventional 128 + n.
function exitLikeChild(result, exit, proc) {
  if (result.signal) {
    try {
      proc.kill(proc.pid, result.signal);
    } catch {
      // fall through to the numeric status
    }
    const number = os.constants.signals[result.signal];
    return exit(number ? 128 + number : 1);
  }
  return exit(result.status ?? 1);
}

async function run(binaryName, options = {}) {
  const args = options.args || process.argv.slice(2);
  const resolveBinaryPath = options.getBinaryPath || getBinaryPath;
  const spawn = options.spawn || spawnChild;
  const exit = options.exit || process.exit;
  const proc = options.process || process;
  const versionFlag = isVersionFlag(args);

  let binaryPath;
  try {
    binaryPath = await resolveBinaryPath(binaryName);
  } catch (error) {
    if (versionFlag) {
      printVersionFallback(binaryName, error);
      return exit(0);
    }
    throw error;
  }

  const result = await runChild(spawn, binaryPath, args, proc);
  if (result.error) {
    if (versionFlag) {
      printVersionFallback(binaryName, result.error);
      return exit(0);
    }
    throw result.error;
  }
  return exitLikeChild(result, exit, proc);
}

async function runCodeWhale() {
  await run("codewhale");
}

async function runCodeWhaleTui() {
  // v0.9.5 single-binary: tui is now an alias to codewhale (kept for backwards compat, will warn)
  if (!process.env.CODEWHALE_SUPPRESS_TUI_DEPRECATION) {
    process.stderr.write("codewhale-tui: deprecated alias to `codewhale` (single binary since v0.9.5). Use `codewhale` instead.\n");
  }
  await run("codewhale");
}

module.exports = {
  run,
  runCodeWhale,
  runCodeWhaleTui,
  reportStartFailure,
  _internal: { isVersionFlag, printVersionFallback, FORWARDED_SIGNALS, OUTLIVED_SIGNALS },
};

if (require.main === module) {
  const command = process.argv[1] || "";
  if (command.includes("tui")) {
    runCodeWhaleTui().catch((error) => {
      reportStartFailure("codewhale", error);
      process.exit(1);
    });
  } else {
    runCodeWhale().catch((error) => {
      reportStartFailure("codewhale", error);
      process.exit(1);
    });
  }
}
