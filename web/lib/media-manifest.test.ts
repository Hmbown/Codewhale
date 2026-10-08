/**
 * Media-manifest contracts for the real-session media surface.
 *
 * Two enforceable states:
 *   pending   — intent only: no asset fields, and no files may exist under
 *               web/public/media/. The component must render the visible
 *               "recording pending release candidate" state instead of any
 *               imagery. This is what keeps the site honest until the v0.9.2
 *               dogfood recording (#4906) exists.
 *   published — complete or nothing: poster, video, per-locale captions,
 *               transcript, and GIF fallback all present. The suite checks
 *               file presence/bytes and PNG dimensions; video dimensions and
 *               duration are declared metadata whose physical verification
 *               remains in the recording checklist.
 *
 * The reduced-motion contract is structural: no autoplay anywhere, poster is
 * the static default, and the GIF is offered only as a link. The test asserts
 * the component source carries that contract.
 */
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { createHash } from "node:crypto";
import { describe, expect, it } from "vitest";
import {
  getMediaAsset,
  MEDIA_ASSETS,
  MEDIA_BUDGETS,
  MEDIA_PUBLIC_DIR,
  REDUCED_MOTION_POLICY,
  TERMINAL_SCREENSHOT,
  type MediaAsset,
} from "./media-manifest";
import { ALL_LOCALES } from "./i18n/config";
import { getDocsShell } from "./i18n/dictionaries";

// Captions are required for locales that ship a complete pack, not for every
// routed locale: `locales` also includes `partial` locales, which route with an
// English-fallback pack. Promising a caption track per routed locale would be a
// claim the recording cannot meet, and the manifest exists to keep those claims
// honest.
const shippedLocales = ALL_LOCALES.filter((l) => l.status === "shipped").map((l) => l.code);

const webRoot = new URL("../", import.meta.url);
const repoRoot = new URL("../../", import.meta.url);

function publicFileBytes(src: string): number {
  return statSync(new URL(`public/${src}`, webRoot)).size;
}

function pngDimensions(src: string): [number, number] {
  const image = readFileSync(new URL(`public/${src}`, webRoot));
  expect(image.subarray(1, 4).toString("ascii")).toBe("PNG");
  return [image.readUInt32BE(16), image.readUInt32BE(20)];
}

describe("media manifest integrity", () => {
  it("keeps the terminal capture tied to its measured file and captured build", () => {
    const src = TERMINAL_SCREENSHOT.src.slice(1);
    const bytes = readFileSync(new URL(`public/${src}`, webRoot));
    expect(createHash("sha256").update(bytes).digest("hex")).toBe(TERMINAL_SCREENSHOT.sha256);
    expect(pngDimensions(src)).toEqual([TERMINAL_SCREENSHOT.width, TERMINAL_SCREENSHOT.height]);
    expect(TERMINAL_SCREENSHOT.sourceCommit).toMatch(/^[0-9a-f]{40}$/);
  });

  it("has unique asset ids and complete localized copy", () => {
    const ids = MEDIA_ASSETS.map((a) => a.id);
    expect(new Set(ids).size).toBe(ids.length);
    for (const asset of MEDIA_ASSETS) {
      for (const pair of [asset.title, asset.description, asset.pendingLabel]) {
        expect(pair.en.trim().length, `${asset.id} en`).toBeGreaterThan(0);
        expect(pair.zh.trim().length, `${asset.id} zh`).toBeGreaterThan(0);
      }
      expect(asset.id).toMatch(/^[a-z0-9-]+$/);
    }
  });

  it("keeps pending entries asset-free on disk and in the manifest", () => {
    const mediaDir = new URL(`public/${MEDIA_PUBLIC_DIR}/`, webRoot);
    const onDisk = existsSync(mediaDir) ? readdirSync(mediaDir) : [];

    for (const asset of MEDIA_ASSETS.filter((a) => a.status === "pending")) {
      expect(asset.poster, asset.id).toBeUndefined();
      expect(asset.video, asset.id).toBeUndefined();
      expect(asset.captions, asset.id).toBeUndefined();
      expect(asset.gifFallback, asset.id).toBeUndefined();
      expect(asset.transcript, asset.id).toBeUndefined();
      // No staged or fabricated file may exist for a pending asset.
      for (const file of onDisk) {
        expect(file.startsWith(asset.id), `${asset.id}: unexpected file ${file}`).toBe(false);
      }
      // The pending state must say why, in both locales.
      expect(asset.pendingLabel.en).toContain("pending release candidate");
      expect(asset.pendingLabel.zh.length).toBeGreaterThan(0);
    }
  });

  it("requires published entries to be complete and satisfy enforceable budgets", () => {
    for (const asset of MEDIA_ASSETS.filter((a) => a.status === "published")) {
      assertPublishedAsset(asset);
    }
  });

  it("keeps budgets aligned with the canonical screenshot contract", () => {
    // The site screenshot contract pins 1280×720 under 500 KB; session-media
    // posters follow the same budget so the two surfaces stay consistent.
    expect(MEDIA_BUDGETS.poster).toEqual({ width: 1280, height: 720, maxBytes: 500_000 });
    expect(MEDIA_BUDGETS.video.width).toBe(MEDIA_BUDGETS.poster.width);
    expect(MEDIA_BUDGETS.video.height).toBe(MEDIA_BUDGETS.poster.height);
    expect(MEDIA_BUDGETS.video.maxDurationSeconds).toBeLessThanOrEqual(120);
    expect(MEDIA_BUDGETS.gifFallback.maxBytes).toBeGreaterThan(0);
    for (const locale of shippedLocales) {
      expect(MEDIA_BUDGETS.captionLocales).toContain(locale);
    }
  });
});

function assertPublishedAsset(asset: MediaAsset): void {
  const { poster, video, captions, gifFallback, transcript } = asset;
  expect(poster, `${asset.id}.poster`).toBeTruthy();
  expect(video, `${asset.id}.video`).toBeTruthy();
  expect(captions, `${asset.id}.captions`).toBeTruthy();
  expect(gifFallback, `${asset.id}.gifFallback`).toBeTruthy();
  expect(transcript, `${asset.id}.transcript`).toBeTruthy();

  expect(pngDimensions(poster!.src)).toEqual([poster!.width, poster!.height]);
  expect([poster!.width, poster!.height]).toEqual([
    MEDIA_BUDGETS.poster.width,
    MEDIA_BUDGETS.poster.height,
  ]);
  expect(publicFileBytes(poster!.src)).toBeLessThanOrEqual(MEDIA_BUDGETS.poster.maxBytes);

  expect([video!.width, video!.height]).toEqual([
    MEDIA_BUDGETS.video.width,
    MEDIA_BUDGETS.video.height,
  ]);
  expect(video!.durationSeconds).toBeLessThanOrEqual(MEDIA_BUDGETS.video.maxDurationSeconds);
  expect(publicFileBytes(video!.src)).toBeLessThanOrEqual(MEDIA_BUDGETS.video.maxBytes);

  expect(publicFileBytes(gifFallback!.src)).toBeLessThanOrEqual(
    MEDIA_BUDGETS.gifFallback.maxBytes,
  );

  const captionLangs = captions!.map((t) => t.srclang);
  for (const locale of MEDIA_BUDGETS.captionLocales) {
    expect(captionLangs, `${asset.id} missing ${locale} captions`).toContain(locale);
  }
  for (const track of captions!) {
    expect(track.src.endsWith(".vtt"), track.src).toBe(true);
    expect(publicFileBytes(track.src), track.src).toBeGreaterThan(0);
    expect(track.label.trim().length).toBeGreaterThan(0);
  }

  expect(existsSync(new URL(transcript!, repoRoot)), `${asset.id}.transcript`).toBe(true);
}

describe("session media component contract", () => {
  const component = readFileSync(
    new URL("../components/session-media.tsx", import.meta.url),
    "utf8",
  );

  it("renders the visible pending state instead of any imagery", () => {
    expect(component).toContain('asset.status === "pending"');
    expect(component).toContain("session-media-pending");
    expect(component).toContain("asset.pendingLabel");
    // Pending copy states plainly that nothing is recorded yet, both locales.
    expect(component).toContain("t.mediaPendingNote");
    expect(getDocsShell("en").mediaPendingNote).toContain("There is no recording yet");
    expect(getDocsShell("zh").mediaPendingNote).toContain("还没有录像");
  });

  it("carries the structural reduced-motion contract: no autoplay, ever", () => {
    expect(component).toContain("REDUCED_MOTION_POLICY");
    expect(REDUCED_MOTION_POLICY).toBe("static-poster-no-autoplay");
    // No autoplay as a JSX/DOM attribute (the policy string and prose may
    // mention the word; the attribute must never appear).
    expect(component).not.toMatch(/autoPlay\s*[={]/);
    expect(component).not.toMatch(/\sautoplay\s*[="]/);
    expect(component).toContain('preload="none"');
    expect(component).toContain("controls");
    expect(component).toContain("poster={`/${poster.src}`}");
  });

  it("wires captions, transcript, and the GIF fallback for published assets", () => {
    expect(component).toContain('kind="captions"');
    expect(component).toContain("srcLang={track.srclang}");
    expect(component).toContain("asset.gifFallback");
    expect(component).toContain("asset.transcript");
  });

  it("exposes machine-checkable state hooks", () => {
    expect(component).toContain("data-media-id={asset.id}");
    expect(component).toContain("data-media-status={asset.status}");
    expect(component).toContain("data-reduced-motion-policy={REDUCED_MOTION_POLICY}");
  });

  it("documents the recording plan location from the pending state", () => {
    expect(component).toContain("docs/releases/v0.9.2-media-plan.md");
    expect(existsSync(new URL("docs/releases/v0.9.2-media-plan.md", repoRoot))).toBe(true);
  });

  it("keeps the first-fleet-session entry addressable for the guide page", () => {
    const asset = getMediaAsset("first-fleet-session");
    expect(asset).toBeTruthy();
    expect(["pending", "published"]).toContain(asset!.status);
    const guide = readFileSync(
      new URL("../app/[locale]/docs/guide/page.tsx", import.meta.url),
      "utf8",
    );
    expect(guide).toContain('getMediaAsset("first-fleet-session")');
    expect(guide).toContain("<SessionMedia");
  });
});
