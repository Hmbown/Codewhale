// Optional release payloads. The Core remains the runtime/authority validator;
// this contract only prevents installers from mixing release bytes or silently
// substituting an unqualified runtime. No host is selected by default.
const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");

const HOST_CATALOG = "codewhale-extension-hosts.json";
const HOST_NAME = "codewhale-extension-host";
const TARGETS = Object.freeze({
  "linux-x64": ["linux", "x64"],
  "linux-arm64": ["linux", "arm64"],
  "macos-x64": ["darwin", "x64"],
  "macos-arm64": ["darwin", "arm64"],
  "windows-x64": ["win32", "x64"],
  "windows-arm64": ["win32", "arm64"],
});
const SHA = /^[a-f0-9]{64}$/;
const SOURCE = /^[a-f0-9]{40}$/;
const VERSION = /^\d+\.\d+\.\d+(?:-[a-zA-Z0-9.-]+)?$/;
const fail = (message) => { throw new Error(`compiled host: ${message}`); };
const sha256 = (bytes) => crypto.createHash("sha256").update(bytes).digest("hex");

function names(target) {
  if (!TARGETS[target]) fail(`unsupported target ${target}; Android is not a qualified Codewhale delivery target`);
  const stem = `${HOST_NAME}-${target}`;
  return {
    binary: `${stem}${target.startsWith("windows-") ? ".exe" : ""}`,
    notices: `${stem}-LICENSES.txt`,
    source: `${stem}-relink-source.tar.gz`,
  };
}

// Direct protocol cases and actual Rust kernel authority are separate proofs.
function compiledMinimum(target) { return target.startsWith("macos-") ? 6 : 5; }
function nativeMinimum(target) { return target.startsWith("windows-") ? 9 : 2; }

function parseCatalog(input, expectedVersion) {
  if (Buffer.isBuffer(input)) input = input.toString("utf8");
  if (Buffer.byteLength(typeof input === "string" ? input : JSON.stringify(input)) > 64 * 1024) fail("catalog exceeds 64 KiB");
  const catalog = typeof input === "string" ? JSON.parse(input) : input;
  if (!catalog || catalog.schema !== 1 || !VERSION.test(catalog.version) || !SOURCE.test(catalog.source_sha) || !SHA.test(catalog.bundle_sha256) || !Array.isArray(catalog.hosts) || catalog.hosts.length > 6) fail("malformed catalog identity");
  if (expectedVersion && catalog.version !== expectedVersion) fail(`catalog version ${catalog.version} differs from requested ${expectedVersion}`);
  const seen = new Set();
  for (const host of catalog.hosts) {
    if (!host || !TARGETS[host.target] || seen.has(host.target)) fail("duplicate or unsupported target");
    seen.add(host.target);
    const expected = names(host.target);
    if (host.asset !== expected.binary || host.notices_asset !== expected.notices || host.source_asset !== expected.source) fail("noncanonical asset name");
    for (const field of ["sha256", "notices_sha256", "source_sha256", "test_log_sha256", "runtime_sha256", "native_log_sha256"]) if (!SHA.test(host[field])) fail(`invalid ${field}`);
    if (!VERSION.test(host.runtime_version) || !SOURCE.test(host.runtime_revision)) fail("invalid runtime identity");
    if (!SOURCE.test(host.webkit_revision) || host.bundle_sha256 !== catalog.bundle_sha256 || host.source_commit !== catalog.source_sha) fail("host/source identity disagreement");
    if (!Number.isSafeInteger(host.passed) || host.passed !== compiledMinimum(host.target) || host.failed !== 0 || host.skipped !== 0 || host.native_platform !== TARGETS[host.target][0] || host.native_arch !== TARGETS[host.target][1]) fail("no successful matching-native compiled-image qualification");
    if (!Number.isSafeInteger(host.native_passed) || host.native_passed < nativeMinimum(host.target) || host.native_failed !== 0 || host.native_skipped !== 0) fail("no actual Native compiled-image containment and memory qualification");
    if (host.target.startsWith("linux-") ? !["glibc", "musl"].includes(host.libc) : host.libc !== "none") fail("unknown runtime libc");
    if (host.license_closure !== "complete" || host.relink_source !== "complete") fail("runtime notices and relinkable corresponding source are required");
  }
  return catalog;
}

function assets(catalog) {
  return catalog ? [HOST_CATALOG, ...parseCatalog(catalog).hosts.flatMap((host) => [host.asset, host.notices_asset, host.source_asset])] : [];
}

function selectedHost(catalog, platform = process.platform, arch = process.arch) {
  if (platform === "android") fail("Android is not a qualified Codewhale compiled-host delivery target; Node remains available");
  const entry = parseCatalog(catalog).hosts.find((host) => TARGETS[host.target][0] === platform && TARGETS[host.target][1] === arch);
  if (!entry) fail(`no qualified image for ${platform}/${arch}; use Node or explicitly qualify a matching local Bun`);
  return entry;
}

function requested(env = process.env) {
  return env.CODEWHALE_INSTALL_COMPILED_HOST === "1";
}

function verifyBytes(bytes, expected, name) {
  if (!SHA.test(expected) || sha256(bytes) !== expected) fail(`SHA256 mismatch for ${name}`);
}

function verifyDirectory(directory, catalog) {
  for (const host of parseCatalog(catalog).hosts) {
    for (const [name, digest] of [[host.asset, host.sha256], [host.notices_asset, host.notices_sha256], [host.source_asset, host.source_sha256]]) {
      const file = path.join(directory, name);
      const stat = fs.lstatSync(file);
      if (!stat.isFile() || stat.isSymbolicLink()) fail(`not a regular payload: ${name}`);
      verifyBytes(fs.readFileSync(file), digest, name);
    }
  }
}

module.exports = { HOST_CATALOG, HOST_NAME, TARGETS, assets, names, compiledMinimum, nativeMinimum, parseCatalog, requested, selectedHost, sha256, verifyBytes, verifyDirectory };
