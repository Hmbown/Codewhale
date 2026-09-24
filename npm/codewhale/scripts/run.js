const { spawnSync } = require("child_process");
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

async function run(binaryName, options = {}) {
  const args = options.args || process.argv.slice(2);
  const resolveBinaryPath = options.getBinaryPath || getBinaryPath;
  const spawn = options.spawnSync || spawnSync;
  const exit = options.exit || process.exit;
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

  const result = spawn(binaryPath, args, {
    stdio: "inherit",
  });
  if (result.error) {
    if (versionFlag) {
      printVersionFallback(binaryName, result.error);
      return exit(0);
    }
    throw result.error;
  }
  return exit(result.status ?? 1);
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
  _internal: { isVersionFlag, printVersionFallback },
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
