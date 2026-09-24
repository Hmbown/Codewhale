const fs = require("fs");
const { execFileSync } = require("child_process");

const GLIBC_VERSION_RE = /GLIBC_(\d+)\.(\d+)(?:\.(\d+))?/g;

function isLinux() {
  return process.platform === "linux";
}

function parseVersion(text) {
  const match = String(text || "").match(/(\d+)\.(\d+)(?:\.(\d+))?/);
  if (!match) return null;
  return [Number(match[1]), Number(match[2]), Number(match[3] || 0)];
}

function compareVersion(a, b) {
  for (let i = 0; i < 3; i += 1) {
    if (a[i] !== b[i]) return a[i] - b[i];
  }
  return 0;
}

function formatVersion(version) {
  return version[2] ? `${version[0]}.${version[1]}.${version[2]}` : `${version[0]}.${version[1]}`;
}

function detectHostGlibc() {
  try {
    const out = execFileSync("getconf", ["GNU_LIBC_VERSION"], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    });
    const version = parseVersion(out);
    if (version) return version;
  } catch {
    // fall through to ldd
  }
  try {
    const out = execFileSync("ldd", ["--version"], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    });
    const firstLine = out.split("\n", 1)[0];
    const version = parseVersion(firstLine);
    if (version) return version;
  } catch {
    // glibc not present (e.g. musl / Alpine)
  }
  return null;
}

function detectBinaryRequiredGlibc(filePath) {
  const buf = fs.readFileSync(filePath);
  const text = buf.toString("latin1");
  let highest = null;
  GLIBC_VERSION_RE.lastIndex = 0;
  let match;
  while ((match = GLIBC_VERSION_RE.exec(text)) !== null) {
    const version = [Number(match[1]), Number(match[2]), Number(match[3] || 0)];
    if (!highest || compareVersion(version, highest) > 0) {
      highest = version;
    }
  }
  return highest;
}

function buildFromSourceHint() {
  return [
    "You can still run codewhale by building from source with Cargo:",
    "",
    "  # Requires Rust 1.88+ (https://rustup.rs)",
    "  cargo install codewhale-cli --locked   # provides `codewhale`",
    "  bin=$(dirname \"$(command -v codewhale)\")",
    "  ln -sf \"$bin/codewhale\" \"$bin/codew\"   # optional short alias",
    "",
    "Or build from a checkout:",
    "",
    "  git clone https://github.com/Hmbown/CodeWhale.git",
    "  cd CodeWhale",
    "  cargo install --path crates/cli --locked",
    "  bin=$(dirname \"$(command -v codewhale)\")",
    "  ln -sf \"$bin/codewhale\" \"$bin/codew\"",
    "",
    "See https://github.com/Hmbown/CodeWhale/blob/main/docs/INSTALL.md",
  ].join("\n");
}

function skipGlibcCheck() {
  return (
    process.env.CODEWHALE_SKIP_GLIBC_CHECK === "1" ||
    process.env.DEEPSEEK_TUI_SKIP_GLIBC_CHECK === "1" ||
    process.env.DEEPSEEK_SKIP_GLIBC_CHECK === "1"
  );
}

function glibcCompatibilityMessage(required, host) {
  const hostLine = host
    ? `this system has glibc ${formatVersion(host)}, which is too old for that asset.`
    : "this system does not appear to provide glibc.";
  return [
    `This Codewhale binary requires GLIBC_${formatVersion(required)}, but ${hostLine}`,
    "",
    "Official Codewhale Linux release assets (x64 and arm64) are static musl builds",
    "with no glibc dependency, so this binary is not an official release asset.",
    "Check where it came from, or build from source on this host.",
    "",
    buildFromSourceHint(),
    "",
    "Set CODEWHALE_SKIP_GLIBC_CHECK=1 to bypass this check at your own risk.",
  ].join("\n");
}

function preflightGlibc(filePath) {
  if (!isLinux()) return;
  if (skipGlibcCheck()) {
    return;
  }

  const required = detectBinaryRequiredGlibc(filePath);
  if (!required) {
    // Statically linked / musl binary, or no GLIBC_* version dependencies present.
    return;
  }

  const host = detectHostGlibc();
  if (!host) {
    throw new Error(glibcCompatibilityMessage(required, null));
  }

  if (compareVersion(host, required) < 0) {
    throw new Error(glibcCompatibilityMessage(required, host));
  }
}

module.exports = {
  preflightGlibc,
  detectHostGlibc,
  detectBinaryRequiredGlibc,
  // exported for tests
  _internal: {
    parseVersion,
    compareVersion,
    formatVersion,
    glibcCompatibilityMessage,
    skipGlibcCheck,
  },
};
