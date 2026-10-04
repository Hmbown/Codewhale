/**
 * facts-lib.mjs — shared derivation logic for website fact generation and
 * drift checking. Imported by both derive-facts.mjs (prebuild) and
 * check-facts.mjs (CI gate).
 *
 * Sources of truth:
 *   - <repo>/Cargo.toml                         → version, workspace crates
 *   - <repo>/crates/tui/src/sandbox/mod.rs      → enforced sandbox markers
 * *   - <repo>/crates/config/assets/provider_descriptors.json → provider labels and default model
 *   - <repo>/npm/codewhale/package.json         → node engines
 *   - <repo>/crates/tui/src/tools/*.rs          → tool count (ToolSpec impls)
 *   - <repo>/LICENSE                            → license
 *   - <repo>/web/data/latest-published-release.json → latest published release
 */
import { parseModelCatalog } from "../lib/model-catalog.mjs";
import { parseProviderDescriptors } from "../lib/provider-descriptors.mjs";
import { readFileSync, readdirSync, existsSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { join, dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
// __dirname is web/scripts; REPO_ROOT is the workspace root (two levels up).
export const REPO_ROOT = resolve(__dirname, "..", "..");

function read(rel) {
  const p = join(REPO_ROOT, rel);
  if (!existsSync(p)) return null;
  return readFileSync(p, "utf-8");
}

export function deriveVersion() {
  const cargo = read("Cargo.toml");
  if (!cargo) return null;
  const m = cargo.match(/^version\s*=\s*"([^"]+)"/m);
  return m ? m[1] : null;
}

export function deriveCrates() {
  const cargo = read("Cargo.toml");
  if (!cargo) return [];
  const block = cargo.match(/members\s*=\s*\[([\s\S]*?)\]/);
  if (!block) return [];
  return [...block[1].matchAll(/"crates\/([^"]+)"/g)].map((m) => m[1]).sort();
}

export function deriveSandboxBackends() {
  const source = read("crates/tui/src/sandbox/mod.rs");
  return source ? deriveSandboxBackendsFromSource(source) : [];
}

export function deriveSandboxBackendsFromSource(source) {
  const marker = source.match(
    /pub const PUBLIC_SANDBOX_BACKENDS\s*:\s*&\[&str\]\s*=\s*&\[([\s\S]*?)\];/,
  );
  if (!marker) return [];
  return [...marker[1].matchAll(/"([^"]+)"/g)].map((match) => match[1]);
}

// Descriptor data owns public identities, order, labels and defaults.
const descriptorData = parseProviderDescriptors(read("crates/config/assets/provider_descriptors.json"));
export function deriveProviders() {
  if (!descriptorData) throw new Error("missing or malformed provider descriptors");
  return descriptorData.providers;
}

export function deriveDefaultModel() {
  return descriptorData?.defaultModel ?? null;
}

export function deriveNodeEngines() {
  const pkg = read("npm/codewhale/package.json");
  if (!pkg) return null;
  try {
    return JSON.parse(pkg).engines?.node ?? null;
  } catch {
    return null;
  }
}

// Public model identities, labels and source-support dates share the reviewed
// Engine catalog owner. A malformed source cannot become a partial/empty list.
export function deriveModels() {
  const models = parseModelCatalog(read("crates/config/assets/models_dev.bundled.json"));
  if (!models) throw new Error("missing or malformed reviewed model catalog");
  return models;
}

export function deriveToolCount() {
  const dir = join(REPO_ROOT, "crates/tui/src/tools");
  if (!existsSync(dir)) return null;
  let count = 0;
  for (const f of readdirSync(dir)) {
    if (!f.endsWith(".rs")) continue;
    const body = readFileSync(join(dir, f), "utf-8");
    count += (body.match(/^impl ToolSpec for /gm) ?? []).length;
  }
  return count > 0 ? count : null;
}

export function deriveLicense() {
  const lic = read("LICENSE");
  if (!lic) return null;
  const first = lic.split(/\r?\n/).find((l) => l.trim().length > 0);
  if (!first) return null;
  if (/^MIT License/i.test(first)) return "MIT";
  if (/Apache.*2\.0/i.test(first)) return "Apache-2.0";
  return first.trim();
}

export function deriveLatestPublishedRelease() {
  const raw = read("web/data/latest-published-release.json");
  if (!raw) return null;
  try {
    const release = JSON.parse(raw);
    if (
      typeof release.tag !== "string" ||
      typeof release.version !== "string" ||
      release.tag !== `v${release.version}` ||
      typeof release.publishedAt !== "string" ||
      !Number.isFinite(Date.parse(release.publishedAt)) ||
      typeof release.url !== "string" ||
      release.url !== `https://github.com/Hmbown/CodeWhale/releases/tag/${release.tag}`
    ) {
      return null;
    }
    return release;
  } catch {
    return null;
  }
}

/**
 * Re-derive all mechanical facts from the current workspace. The returned
 * object is the same shape as web/lib/facts.generated.ts → RepoFacts.
 */
export function buildFacts() {
  const providers = deriveProviders();

  const facts = {
    generatedAt: new Date().toISOString(),
    // next.config.ts injects these from the exact checkout into the built
    // artifact. They stay null in the tracked snapshot to avoid a
    // self-referential generated-file diff after every commit.
    sourceRevision: null,
    sourceCommittedAt: null,
    version: deriveVersion(),
    crates: deriveCrates(),
    sandboxBackends: deriveSandboxBackends(),
    providers,
    models: deriveModels(),
    defaultModel: deriveDefaultModel(),
    nodeEngines: deriveNodeEngines(),
    toolCount: deriveToolCount(),
    license: deriveLicense(),
    latestPublishedRelease: deriveLatestPublishedRelease(),
  };

  return facts;
}
