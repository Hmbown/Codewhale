#!/usr/bin/env node
/**
 * derive-changelog.mjs — emit `web/lib/changelog.generated.ts` from the
 * repository CHANGELOG.md so the /changelog route and the docs release-truth
 * band render the real release record, never a hand-typed copy.
 *
 * Runs at `npm run prebuild`, `npm run dev` and in the vitest global setup.
 * The output is NOT tracked (web/.gitignore): a committed copy raced between
 * merges — two PRs that each added a CHANGELOG line each regenerated it
 * against their own base, git merged both to a stale item count, and main
 * went red (twice on 2026-09-26). Deriving at build time removes the race.
 */
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { parseChangelog, renderChangelogModule } from "./changelog-lib.mjs";

const __dirname = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = resolve(__dirname, "..", "..");
const SOURCE = resolve(REPO_ROOT, "CHANGELOG.md");
const TARGET = resolve(__dirname, "..", "lib", "changelog.generated.ts");

if (!existsSync(SOURCE)) {
  console.error(`[derive-changelog] CHANGELOG.md not found at ${SOURCE}`);
  process.exit(1);
}

const parsed = parseChangelog(readFileSync(SOURCE, "utf-8"));
const next = renderChangelogModule(parsed);
const current = existsSync(TARGET) ? readFileSync(TARGET, "utf-8") : null;
if (current !== next) {
  writeFileSync(TARGET, next);
  console.log(`[derive-changelog] wrote ${parsed.releases.length} releases → lib/changelog.generated.ts`);
} else {
  console.log(`[derive-changelog] lib/changelog.generated.ts is current (${parsed.releases.length} releases)`);
}
