#!/usr/bin/env node
/**
 * check-docs.mjs — drift / parity gate for website documentation.
 *
 * Verifies that:
 *   1. Every doc topic in docs-map.ts points to a real repo source file.
 *   2. Version, command snippets, and tool names referenced on the website
 *      match the current workspace state.
 *
 * Usage:
 *   cd web && npm run check:docs
 *
 * Relies on facts-lib.mjs for version / provider / tool derivation.
 */
import { readFileSync, existsSync, readdirSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { buildInstallGuide, installAnchorErrors } from "./install-guide-lib.mjs";

const __dirname = dirname(fileURLToPath(import.meta.url));
const WEB_DIR = resolve(__dirname, "..");
const REPO_ROOT = resolve(WEB_DIR, "..");

/* ------------------------------------------------------------------ */
/*  Parse docs-map.ts (regex — avoids ts-node dependency)              */
/* ------------------------------------------------------------------ */

function parseDocsMap() {
  const path = resolve(WEB_DIR, "lib", "docs-map.ts");
  if (!existsSync(path)) {
    console.error(`[check-docs] ERROR: docs-map.ts not found at ${path}`);
    process.exit(1);
  }
  const src = readFileSync(path, "utf-8");

  const topics = [];
  const re =
    /\{\s*id:\s*"(\w[^"]*)",\s*slug:\s*"(\w[^"]*)",[\s\S]*?repoSource:\s*(\[[^\]]+\]|"[^"]+")/g;
  let m;
  while ((m = re.exec(src)) !== null) {
    const id = m[1];
    const slug = m[2];
    let rawSource = m[3];
    const sources = rawSource.startsWith("[")
      ? rawSource.match(/"([^"]+)"/g)?.map((s) => s.slice(1, -1)) ?? []
      : [rawSource.slice(1, -1)];
    topics.push({ id, slug, repoSource: sources });
  }
  return topics;
}

/* ------------------------------------------------------------------ */
/*  Check 1: every repo source file exists                             */
/* ------------------------------------------------------------------ */

function checkSourcesExist(topics) {
  const missing = [];
  for (const t of topics) {
    for (const src of t.repoSource) {
      const p = resolve(REPO_ROOT, src);
      if (!existsSync(p)) {
        missing.push({ topic: t.id, source: src, expected: p });
      }
    }
  }
  return missing;
}

/* ------------------------------------------------------------------ */
/*  Check 2: version matches Cargo.toml                                 */
/* ------------------------------------------------------------------ */

function deriveVersion() {
  const cargoPath = resolve(REPO_ROOT, "Cargo.toml");
  if (!existsSync(cargoPath)) return null;
  const cargo = readFileSync(cargoPath, "utf-8");
  const m = cargo.match(/^version\s*=\s*"([^"]+)"/m);
  return m ? m[1] : null;
}

function checkVersion() {
  const version = deriveVersion();
  return { version, ok: version != null };
}

/* ------------------------------------------------------------------ */
/*  Check 3: command snippet freshness (install commands)               */
/* ------------------------------------------------------------------ */

function checkInstallSnippets() {
  const version = deriveVersion();
  if (!version) return { ok: false, note: "could not derive version" };

  // The page renders the verified document, including historical release
  // examples; those versions must not be rewritten to the workspace candidate.
  const src = readFileSync(resolve(REPO_ROOT, "docs/INSTALL.md"), "utf-8");
  const guide = buildInstallGuide(src);
  const stale = [];
  function checkLinks(dir) {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const path = resolve(dir, entry.name);
      if (entry.isDirectory()) checkLinks(path);
      else if (/\.(tsx?|md|json)$/.test(entry.name) && !entry.name.endsWith(".test.ts")) {
        for (const anchor of installAnchorErrors(readFileSync(path, "utf8"), guide.anchors)) {
          stale.push({ found: anchor, expected: "an existing INSTALL.md anchor", context: path });
        }
      }
    }
  }
  for (const dir of ["app", "components", "lib"]) checkLinks(resolve(WEB_DIR, dir));

  // A clone without an explicit destination creates a directory whose name
  // matches the repository slug exactly. Keep the following `cd` command
  // case-correct so source installation works on case-sensitive filesystems.
  const sourceCheckout = src.match(
    /git clone[^\n]*https:\/\/github\.com\/Hmbown\/([^\s`]+)\s*\ncd\s+([^\s`]+)/,
  );
  const checkout = sourceCheckout
    ? {
        cloned: sourceCheckout[1].replace(/\.git$/, ""),
        entered: sourceCheckout[2],
      }
    : null;
  const checkoutOk = checkout !== null && checkout.cloned === checkout.entered;

  return { ok: stale.length === 0 && checkoutOk, stale, checkout };
}

/* ------------------------------------------------------------------ */
/*  Main                                                                */
/* ------------------------------------------------------------------ */

function main() {
  const topics = parseDocsMap();
  if (topics.length === 0) {
    console.error("[check-docs] ERROR: no topics parsed from docs-map.ts");
    process.exit(1);
  }
  console.log(`[check-docs] parsed ${topics.length} doc topics`);

  // Check 1: sources exist
  const missingSources = checkSourcesExist(topics);
  if (missingSources.length > 0) {
    console.error("[check-docs] FAIL — missing repo source files:");
    for (const m of missingSources) {
      console.error(`  ${m.topic}: ${m.source} → ${m.expected} (not found)`);
    }
    process.exit(1);
  }
  console.log("[check-docs] OK — all repo source files exist");

  // Check 2: version
  const ver = checkVersion();
  if (!ver.ok) {
    console.error("[check-docs] FAIL — could not derive version from workspace");
    process.exit(1);
  }
  console.log(`[check-docs] OK — version=${ver.version}`);

  // Check 3: install snippets
  const install = checkInstallSnippets();
  if (!install.ok && !install.note) {
    if (install.stale.length > 0) {
      console.error("[check-docs] FAIL — missing install-guide anchor:");
      for (const s of install.stale) {
        console.error(`  found "${s.found}", expected "${s.expected}" in: ${s.context}`);
      }
    }
    if (install.checkout === null) {
      console.error("[check-docs] FAIL — source checkout clone/cd commands not found");
    } else if (install.checkout.cloned !== install.checkout.entered) {
      console.error(
        `[check-docs] FAIL — source checkout clones "${install.checkout.cloned}" but enters "${install.checkout.entered}"`,
      );
    }
    // #3770: a stale install snippet must fail the gate, not fall through to
    // the final PASS. The same applies to source checkout copy drift.
    process.exit(1);
  }
  console.log(`[check-docs] OK — install snippets${install.note ? ` (${install.note})` : ""}`);

  console.log("[check-docs] PASS");
}

try {
  main();
} catch (e) {
  console.error("[check-docs] ERROR:", e.message);
  process.exit(1);
}
