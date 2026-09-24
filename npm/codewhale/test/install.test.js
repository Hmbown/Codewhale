const assert = require("node:assert/strict");
const crypto = require("node:crypto");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const test = require("node:test");

const installScript = fs.readFileSync(
  path.join(__dirname, "..", "scripts", "install.js"),
  "utf8",
);
const { installFailureHint, _internal } = require("../scripts/install");
const { _internal: glibcInternal } = require("../scripts/preflight-glibc");

function sha256(content) {
  return crypto.createHash("sha256").update(content).digest("hex");
}

async function makeTempDir(t) {
  const dir = await fs.promises.mkdtemp(path.join(os.tmpdir(), "codewhale-install-test-"));
  t.after(() => fs.promises.rm(dir, { force: true, recursive: true }));
  return dir;
}

async function exists(file) {
  return fs.promises.access(file).then(
    () => true,
    () => false,
  );
}

async function withoutForcedDownload(callback) {
  const previousCodewhale = process.env.CODEWHALE_FORCE_DOWNLOAD;
  const previousTui = process.env.DEEPSEEK_TUI_FORCE_DOWNLOAD;
  const previousLegacy = process.env.DEEPSEEK_FORCE_DOWNLOAD;
  delete process.env.CODEWHALE_FORCE_DOWNLOAD;
  delete process.env.DEEPSEEK_TUI_FORCE_DOWNLOAD;
  delete process.env.DEEPSEEK_FORCE_DOWNLOAD;
  try {
    return await callback();
  } finally {
    if (previousCodewhale === undefined) {
      delete process.env.CODEWHALE_FORCE_DOWNLOAD;
    } else {
      process.env.CODEWHALE_FORCE_DOWNLOAD = previousCodewhale;
    }
    if (previousTui === undefined) {
      delete process.env.DEEPSEEK_TUI_FORCE_DOWNLOAD;
    } else {
      process.env.DEEPSEEK_TUI_FORCE_DOWNLOAD = previousTui;
    }
    if (previousLegacy === undefined) {
      delete process.env.DEEPSEEK_FORCE_DOWNLOAD;
    } else {
      process.env.DEEPSEEK_FORCE_DOWNLOAD = previousLegacy;
    }
  }
}

test("install script checks Node support before loading helpers", () => {
  const guardIndex = installScript.indexOf("assertSupportedNode();");
  const firstRequireIndex = installScript.indexOf("require(");

  assert.notEqual(guardIndex, -1);
  assert.notEqual(firstRequireIndex, -1);
  assert.ok(guardIndex < firstRequireIndex);
});

test("install script remains parseable before the Node support guard runs", () => {
  assert.equal(installScript.includes("??"), false);
  assert.equal(installScript.includes("?."), false);
});

test("native shortcut has its own platform asset and installed path", () => {
  const paths = _internal.binaryPaths();
  assert.match(paths.codew.asset, /^codew-/);
  assert.equal(path.basename(paths.codew.target), process.platform === "win32" ? "codew.exe" : "codew");
  assert.notEqual(paths.codew.target, paths.codewhale.target);
});

test("install failure hint explains release base override for blocked GitHub downloads", () => {
  const previous = process.env.DEEPSEEK_TUI_RELEASE_BASE_URL;
  delete process.env.DEEPSEEK_TUI_RELEASE_BASE_URL;
  try {
    const error = Object.assign(
      new Error(
        "fetch https://github.com/Hmbown/CodeWhale/releases/download/v0.8.19/codewhale-artifacts-sha256.txt failed after 5 attempts:\ngetaddrinfo ENOTFOUND github.com",
      ),
      { code: "ENOTFOUND" },
    );

    const hint = installFailureHint(error);

    assert.match(hint, /CODEWHALE_RELEASE_BASE_URL/);
    assert.match(hint, /codewhale-artifacts-sha256\.txt/);
    assert.match(hint, /platform binaries/);
    assert.match(hint, /#npm-binary-download-times-out/);
  } finally {
    if (previous === undefined) {
      delete process.env.DEEPSEEK_TUI_RELEASE_BASE_URL;
    } else {
      process.env.DEEPSEEK_TUI_RELEASE_BASE_URL = previous;
    }
  }
});

test("install failure hint checks configured release base when override is already set", () => {
  const previous = process.env.DEEPSEEK_TUI_RELEASE_BASE_URL;
  process.env.DEEPSEEK_TUI_RELEASE_BASE_URL = "https://mirror.example/deepseek/";
  try {
    const error = Object.assign(new Error("download stalled"), {
      code: "EDOWNLOADTIMEOUT",
    });

    const hint = installFailureHint(error);

    assert.match(hint, /resolves to https:\/\/mirror\.example\/deepseek\//);
    assert.match(hint, /codewhale-artifacts-sha256\.txt/);
    assert.doesNotMatch(hint, /If GitHub is unavailable/);
  } finally {
    if (previous === undefined) {
      delete process.env.DEEPSEEK_TUI_RELEASE_BASE_URL;
    } else {
      process.env.DEEPSEEK_TUI_RELEASE_BASE_URL = previous;
    }
  }
});

test("glibc preflight message is Codewhale-branded and actionable", () => {
  const message = glibcInternal.glibcCompatibilityMessage([2, 39, 0], [2, 35, 0]);

  assert.match(message, /This Codewhale binary requires GLIBC_2\.39/);
  assert.match(message, /this system has glibc 2\.35/);
  assert.match(message, /cargo install codewhale-cli --locked/);
  assert.match(message, /ln -sf .*codewhale.*codew/);
  assert.doesNotMatch(message, /cargo install codewhale-tui/);
  assert.match(message, /Linux release assets \(x64 and arm64\) are static musl builds/);
  assert.doesNotMatch(message, /GNU libc/);
  assert.match(message, /CODEWHALE_SKIP_GLIBC_CHECK=1/);
});

test("glibc preflight accepts canonical and legacy skip env vars", () => {
  const previousCodewhale = process.env.CODEWHALE_SKIP_GLIBC_CHECK;
  const previousTui = process.env.DEEPSEEK_TUI_SKIP_GLIBC_CHECK;
  const previousLegacy = process.env.DEEPSEEK_SKIP_GLIBC_CHECK;
  delete process.env.CODEWHALE_SKIP_GLIBC_CHECK;
  delete process.env.DEEPSEEK_TUI_SKIP_GLIBC_CHECK;
  delete process.env.DEEPSEEK_SKIP_GLIBC_CHECK;
  try {
    assert.equal(glibcInternal.skipGlibcCheck(), false);
    process.env.CODEWHALE_SKIP_GLIBC_CHECK = "1";
    assert.equal(glibcInternal.skipGlibcCheck(), true);
    delete process.env.CODEWHALE_SKIP_GLIBC_CHECK;
    process.env.DEEPSEEK_TUI_SKIP_GLIBC_CHECK = "1";
    assert.equal(glibcInternal.skipGlibcCheck(), true);
  } finally {
    if (previousCodewhale === undefined) {
      delete process.env.CODEWHALE_SKIP_GLIBC_CHECK;
    } else {
      process.env.CODEWHALE_SKIP_GLIBC_CHECK = previousCodewhale;
    }
    if (previousTui === undefined) {
      delete process.env.DEEPSEEK_TUI_SKIP_GLIBC_CHECK;
    } else {
      process.env.DEEPSEEK_TUI_SKIP_GLIBC_CHECK = previousTui;
    }
    if (previousLegacy === undefined) {
      delete process.env.DEEPSEEK_SKIP_GLIBC_CHECK;
    } else {
      process.env.DEEPSEEK_SKIP_GLIBC_CHECK = previousLegacy;
    }
  }
});

test("ensureBinary adopts a manually placed target binary after checksum validation", async (t) => {
  const dir = await makeTempDir(t);
  const target = path.join(dir, process.platform === "win32" ? "codewhale.exe" : "codewhale");
  const assetName = process.platform === "win32" ? "codewhale-windows-x64.exe" : "codewhale-linux-x64";
  const version = "0.8.25";
  const content = Buffer.from("manual codewhale binary");
  let checksumLoads = 0;

  await fs.promises.writeFile(target, content, { mode: 0o600 });
  await fs.promises.writeFile(`${target}.version`, "0.8.24", "utf8");

  const result = await withoutForcedDownload(() =>
    _internal.ensureBinary(target, assetName, version, "Hmbown/CodeWhale", async () => {
      checksumLoads += 1;
      return new Map([[assetName, sha256(content)]]);
    }),
  );

  assert.equal(result, target);
  assert.equal(checksumLoads, 1);
  assert.equal(await fs.promises.readFile(`${target}.version`, "utf8"), version);
  if (process.platform !== "win32") {
    assert.notEqual((await fs.promises.stat(target)).mode & 0o111, 0);
  }
});

test("ensureBinary adopts an official release-named binary placed in downloads", async (t) => {
  const dir = await makeTempDir(t);
  const target = path.join(dir, process.platform === "win32" ? "codewhale.exe" : "codewhale");
  const assetName = process.platform === "win32" ? "codewhale-windows-x64.exe" : "codewhale-linux-x64";
  const assetPath = path.join(dir, assetName);
  const version = "0.8.25";
  const content = Buffer.from("official release binary");

  await fs.promises.writeFile(assetPath, content);

  const result = await withoutForcedDownload(() =>
    _internal.ensureBinary(target, assetName, version, "Hmbown/CodeWhale", async () =>
      new Map([[assetName, sha256(content)]]),
    ),
  );

  assert.equal(result, target);
  assert.equal(await exists(target), true);
  assert.equal(await exists(assetPath), false);
  assert.equal(await fs.promises.readFile(`${target}.version`, "utf8"), version);
});

test("manual binaries with mismatched checksums are not adopted", async (t) => {
  const dir = await makeTempDir(t);
  const target = path.join(dir, process.platform === "win32" ? "codewhale.exe" : "codewhale");
  const assetName = process.platform === "win32" ? "codewhale-windows-x64.exe" : "codewhale-linux-x64";
  const content = Buffer.from("wrong binary bytes");

  await fs.promises.writeFile(target, content);

  const adopted = await _internal.adoptExistingBinaryIfValid(
    target,
    assetName,
    "0.8.25",
    async () => new Map([[assetName, sha256(Buffer.from("different bytes"))]]),
    `${target}.version`,
  );

  assert.equal(adopted, false);
  assert.equal(await exists(`${target}.version`), false);
});

test("resolvePackageVersion honors codewhaleBinaryVersion precedence (#3769)", () => {
  const { resolvePackageVersion } = _internal;

  // codewhaleBinaryVersion wins over deepseekBinaryVersion and pkg.version.
  assert.equal(
    resolvePackageVersion(
      {
        codewhaleBinaryVersion: "1.2.3",
        deepseekBinaryVersion: "0.0.1",
        version: "9.9.9",
      },
      {},
    ),
    "1.2.3",
  );

  // Falls back to deepseekBinaryVersion, then pkg.version.
  assert.equal(
    resolvePackageVersion({ deepseekBinaryVersion: "0.0.1", version: "9.9.9" }, {}),
    "0.0.1",
  );
  assert.equal(resolvePackageVersion({ version: "9.9.9" }, {}), "9.9.9");

  assert.equal(
    resolvePackageVersion(
      { codewhaleBinaryVersion: "1.2.3" },
      {
        CODEWHALE_VERSION: "6.6.6",
        DEEPSEEK_TUI_VERSION: "7.7.7",
        DEEPSEEK_VERSION: "8.8.8",
      },
    ),
    "6.6.6",
  );

  // Legacy env vars still take precedence over package fields, unchanged.
  assert.equal(
    resolvePackageVersion(
      { codewhaleBinaryVersion: "1.2.3", version: "9.9.9" },
      { DEEPSEEK_TUI_VERSION: "7.7.7" },
    ),
    "7.7.7",
  );
  assert.equal(
    resolvePackageVersion(
      { codewhaleBinaryVersion: "1.2.3" },
      { DEEPSEEK_VERSION: "8.8.8" },
    ),
    "8.8.8",
  );
});

test("canonical Codewhale installer variables outrank legacy aliases", () => {
  assert.equal(
    _internal.resolveRepo({
      CODEWHALE_GITHUB_REPO: "example/codewhale",
      DEEPSEEK_TUI_GITHUB_REPO: "legacy/tui",
      DEEPSEEK_GITHUB_REPO: "legacy/root",
    }),
    "example/codewhale",
  );
  assert.equal(
    _internal.isOptionalInstall([], {
      CODEWHALE_OPTIONAL_INSTALL: "1",
    }),
    true,
  );
  assert.equal(
    _internal.isQuietInstall({ CODEWHALE_QUIET_INSTALL: "1" }),
    true,
  );
  assert.equal(
    _internal.shouldForceDownload({ CODEWHALE_FORCE_DOWNLOAD: "1" }),
    true,
  );
  assert.equal(
    _internal.shouldDisableInstall({ CODEWHALE_DISABLE_INSTALL: "1" }),
    true,
  );
  assert.equal(
    _internal.downloadTimeoutMs("runtime", {
      CODEWHALE_DOWNLOAD_TIMEOUT_MS: "1234",
      DEEPSEEK_TUI_DOWNLOAD_TIMEOUT_MS: "9999",
    }),
    1234,
  );
  assert.equal(
    _internal.downloadStallMs("runtime", {
      CODEWHALE_DOWNLOAD_STALL_MS: "5678",
      DEEPSEEK_TUI_DOWNLOAD_STALL_MS: "9999",
    }),
    5678,
  );
});

test("httpRequest handles invalid URL parsing errors", async () => {
  const { httpRequest } = _internal;
  const invalidUrl = "not-a-valid-url";
  try {
    await httpRequest(invalidUrl);
    assert.fail("httpRequest should throw for an invalid URL");
  } catch (err) {
    assert.equal(err.name, "NonRetryableError");
    assert.match(err.message, /Invalid URL: not-a-valid-url/);
  }
});
