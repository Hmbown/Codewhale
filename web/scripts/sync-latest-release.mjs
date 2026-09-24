#!/usr/bin/env node
// Refresh the checked-in "latest published release" fact from the real GitHub
// release. The fact is mirrored in three places and ALL must move together:
//
//   web/data/latest-published-release.json   (read by derive-facts.mjs)
//   docs/public-surface-facts.json           (latestPublishedRelease, which
//                                             names the file above as its
//                                             `sources`)
//   docs/cloud-facts/stable.json             (release.latest / release_url,
//                                             compared by check-cloud-facts)
//
// web/lib/public-surface-contract.test.ts asserts the first two agree and
// check-cloud-facts.mjs asserts the third, so updating only one turns a stale
// marketing fact into a red Lint & Type Check. web/lib/facts.generated.ts is
// derived from the first file; regenerate it with derive-facts.mjs afterwards.
//
// release.yml's `sync-release-record` job runs this after every publish and
// proposes the result to main as a bot PR, so nobody hand-commits the record.
//
// Facts must be derivable from the repo with no network (derive-facts.mjs reads
// this file, it does not call GitHub), so the file is checked in. Nothing wrote
// it, which is why it drifted: the marketing deploy's post-deploy comparison
// failed on latestPublishedRelease.tag because this said v0.9.10 while the
// published release was v0.9.11.
//
//   node web/scripts/sync-latest-release.mjs          # write if changed
//   node web/scripts/sync-latest-release.mjs --check  # exit 1 if stale
//
// --check is the CI form: it makes drift a failing gate at PR time instead of a
// surprise after a production deploy. It only warns while the record is exactly
// one release behind a release published under 24h ago: that is the window in
// which release.yml's sync-release-record PR is waiting to merge, and neither a
// PR author nor an unrelated push to main can fix it. Past 24h, or more than one
// release behind, it fails again.

import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const REPO = "Hmbown/CodeWhale";
const here = dirname(fileURLToPath(import.meta.url));
const target = resolve(here, "..", "data", "latest-published-release.json");
const mirror = resolve(here, "..", "..", "docs", "public-surface-facts.json");
const cloudFacts = resolve(here, "..", "..", "docs", "cloud-facts", "stable.json");
const GRACE_MS = 24 * 60 * 60 * 1000;
const checkOnly = process.argv.includes("--check");

const headers = {
  accept: "application/vnd.github+json",
  "user-agent": "codewhale-facts-sync",
};
if (process.env.GITHUB_TOKEN) headers.authorization = `Bearer ${process.env.GITHUB_TOKEN}`;

const response = await fetch(`https://api.github.com/repos/${REPO}/releases/latest`, { headers });
if (!response.ok) {
  console.error(`[sync-latest-release] GitHub returned ${response.status}; leaving the file alone.`);
  process.exit(checkOnly ? 0 : 1);
}
const release = await response.json();

const tag = String(release.tag_name || "");
const version = tag.startsWith("v") ? tag.slice(1) : "";
const next = {
  tag,
  version,
  publishedAt: String(release.published_at || ""),
  url: `https://github.com/${REPO}/releases/tag/${tag}`,
};

// deriveLatestPublishedRelease() silently returns null on any shape violation,
// which would drop the fact entirely rather than report a bad one. Fail loudly.
if (!tag || !version || tag !== `v${version}` || !Number.isFinite(Date.parse(next.publishedAt))) {
  console.error(`[sync-latest-release] refusing to write an unusable release fact: ${JSON.stringify(next)}`);
  process.exit(1);
}

const readJson = (path) => {
  try { return JSON.parse(readFileSync(path, "utf8")); } catch { return null; }
};

const current = readJson(target);
const matrix = readJson(mirror);
const currentMirror = matrix?.latestPublishedRelease ?? null;
const cloud = readJson(cloudFacts);

const isCurrent = (fact) =>
  Boolean(fact) && fact.tag === next.tag && fact.publishedAt === next.publishedAt;
const cloudIsCurrent = (facts) =>
  Boolean(facts?.release) && facts.release.latest === next.version && facts.release.release_url === next.url;

// True when `recordedTag` is the published (non-draft, non-prerelease) release
// immediately before `next`, and `next` is younger than GRACE_MS. Any lookup
// failure answers false, so the check stays strict when in doubt.
async function isFreshlyOneBehind(recordedTag) {
  const age = Date.now() - Date.parse(next.publishedAt);
  if (!recordedTag || !(age >= 0 && age < GRACE_MS)) return false;
  try {
    const res = await fetch(`https://api.github.com/repos/${REPO}/releases?per_page=20`, { headers });
    if (!res.ok) return false;
    const tags = (await res.json())
      .filter((r) => !r.draft && !r.prerelease && Number.isFinite(Date.parse(r.published_at)))
      .sort((a, b) => Date.parse(b.published_at) - Date.parse(a.published_at))
      .map((r) => String(r.tag_name));
    return tags[0] === next.tag && tags[1] === recordedTag;
  } catch {
    return false;
  }
}

if (isCurrent(current) && isCurrent(currentMirror) && (checkOnly || cloudIsCurrent(cloud))) {
  console.log(`[sync-latest-release] already current at ${next.tag}`);
  process.exit(0);
}

if (checkOnly) {
  if (!isCurrent(current)) {
    console.error(
      `[sync-latest-release] stale: ${target} says ${current?.tag ?? "(missing)"}, GitHub says ${next.tag}`,
    );
  }
  if (!isCurrent(currentMirror)) {
    console.error(
      `[sync-latest-release] stale: docs/public-surface-facts.json says ${currentMirror?.tag ?? "(missing)"}, GitHub says ${next.tag}`,
    );
  }
  const recorded = current?.tag;
  if ((current?.tag ?? null) === (currentMirror?.tag ?? null) && (await isFreshlyOneBehind(recorded))) {
    console.warn(
      `[sync-latest-release] warning only: ${next.tag} was published under 24h ago and release.yml's ` +
        "sync-release-record job proposes the record as a PR. Merge that; this change does not need to.",
    );
    if (process.env.GITHUB_ACTIONS) {
      console.log(`::warning title=Release record catching up::${recorded} -> ${next.tag} is pending from release.yml`);
    }
    process.exit(0);
  }
  console.error("Run: npm --prefix web run sync:latest-release && node web/scripts/derive-facts.mjs");
  process.exit(1);
}

writeFileSync(target, `${JSON.stringify(next, null, 2)}\n`);

if (!matrix) {
  console.error(`[sync-latest-release] could not read ${mirror}; the mirror is now stale.`);
  process.exit(1);
}

// Preserve every key the matrix carries beyond the four synced fields (notably
// `sources`), so this stays a fact refresh and not a schema rewrite.
matrix.latestPublishedRelease = { ...currentMirror, ...next };
writeFileSync(mirror, `${JSON.stringify(matrix, null, 2)}\n`);

// stable.json is the unsigned cloud-facts authoring source; only the two
// release pointers move here. yanked/min_supported/notice stay human calls.
if (!cloud?.release) {
  console.error(`[sync-latest-release] could not read release in ${cloudFacts}; it is now stale.`);
  process.exit(1);
}
cloud.release.latest = next.version;
cloud.release.release_url = next.url;
writeFileSync(cloudFacts, `${JSON.stringify(cloud, null, 2)}\n`);

console.log(`[sync-latest-release] wrote ${next.tag} (${next.publishedAt}) to all three facts`);
