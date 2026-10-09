#!/usr/bin/env node
// Fetch only a completed, green official CI receipt for this exact source.
// This does not install Bun, publish assets, or confer runtime authority.
const { execFileSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");
const hosts = require("../../npm/codewhale/scripts/compiled-hosts");

function fetchProof({ repo, runId, sourceSha, target, output }, exec = execFileSync) {
  if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repo) || !/^\d+$/.test(runId) || !/^[a-f0-9]{40}$/.test(sourceSha) || !hosts.TARGETS[target]) throw new Error("invalid exact-source CI proof request");
  const gh = (args) => exec(process.env.GH_BIN || "gh", args, { encoding: "utf8", maxBuffer: 1024 * 1024 });
  const workflow = JSON.parse(gh(["api", `repos/${repo}/actions/workflows/ci.yml`]));
  const run = JSON.parse(gh(["api", `repos/${repo}/actions/runs/${runId}`]));
  if (run.workflow_id !== workflow.id || run.path !== ".github/workflows/ci.yml" || run.head_sha !== sourceSha || run.status !== "completed" || run.conclusion !== "success" || run.repository?.full_name !== repo) throw new Error("compiled-host proof must come from green official ci.yml at this exact source/repository");
  const runner = { linux: "Linux", darwin: "macOS", win32: "Windows" }[hosts.TARGETS[target][0]];
  const architecture = { x64: "X64", arm64: "ARM64" }[hosts.TARGETS[target][1]];
  const name = `native-compiled-host-${runner}-${architecture}`;
  const pages = JSON.parse(gh(["api", `repos/${repo}/actions/runs/${runId}/artifacts?per_page=100`, "--paginate", "--slurp"]));
  const artifacts = pages.flatMap((page) => page.artifacts || []).filter((artifact) => artifact.name === name && !artifact.expired);
  if (artifacts.length !== 1) throw new Error(`CI has no unique current ${name} artifact; delivery is not qualified`);
  fs.mkdirSync(output, { recursive: true });
  if (fs.readdirSync(output).length) throw new Error("compiled proof directory must be empty");
  gh(["run", "download", runId, "--repo", repo, "--name", name, "--dir", path.resolve(output)]);
  const readRegular = (name, limit = Infinity) => {
    if (typeof name !== "string" || path.basename(name) !== name || name === "." || name === "..") throw new Error("proof files must be contained basenames");
    const file = path.join(output, name);
    const metadata = fs.lstatSync(file);
    if (!metadata.isFile() || metadata.isSymbolicLink() || metadata.size > limit) throw new Error(`invalid proof file: ${name}`);
    return fs.readFileSync(file);
  };
  const receipt = JSON.parse(readRegular("native-receipt.json", 64 * 1024));
  if (receipt.scope !== "native-compiled-host" || receipt.source_sha !== sourceSha || receipt.platform !== hosts.TARGETS[target][0] || receipt.arch !== hosts.TARGETS[target][1] || receipt.failed !== 0 || receipt.skipped !== 0 || (!Number.isSafeInteger(receipt.passed) || receipt.passed < hosts.nativeMinimum(target))) throw new Error("downloaded CI receipt does not qualify this exact native target");
  const image = path.join(output, hosts.HOST_NAME + (target.startsWith("windows-") ? ".exe" : ""));
  hosts.verifyBytes(readRegular(path.basename(image)), receipt.host_sha256, "exact contained CI image");
  if (typeof receipt.log !== "string" || path.basename(receipt.log) !== receipt.log) throw new Error("Native proof log must be a contained basename");
  hosts.verifyBytes(readRegular(receipt.log), receipt.log_sha256, "Native qualification log");
  return { image, receipt: path.join(output, "native-receipt.json") };
}

if (require.main === module) {
  const args = process.argv.slice(2);
  const value = (name) => { const index = args.indexOf(name); if (index < 0 || !args[index + 1]) throw new Error(`required ${name}`); return args[index + 1]; };
  try {
    console.log(JSON.stringify(fetchProof({ repo: value("--repo"), runId: value("--run-id"), sourceSha: value("--source-sha"), target: value("--target"), output: value("--output") })));
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
module.exports = { fetchProof };
