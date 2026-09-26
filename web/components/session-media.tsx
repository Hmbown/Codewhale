/**
 * <SessionMedia> — renders one entry from web/lib/media-manifest.ts.
 *
 * Two states, both honest:
 *
 *   pending   — a plainly labeled panel: "Recording pending release
 *               candidate". No mock terminal, no staged screenshot, no
 *               recycled clip. The panel exists so the absence of footage is
 *               visible and explained, not hidden.
 *
 *   published — a <video> with the declared poster, per-locale WebVTT
 *               captions, a transcript link, and a GIF fallback link. The
 *               reduced-motion contract is structural (REDUCED_MOTION_POLICY
 *               = "static-poster-no-autoplay"): preload="none", no autoPlay,
 *               user-initiated playback only. The optional GIF is a link,
 *               not an autoplaying reduced-motion substitute.
 *
 * Server component: no client JS, SSG-safe.
 */

import { REDUCED_MOTION_POLICY, type MediaAsset } from "@/lib/media-manifest";
import { getDocsShell, pickText } from "@/lib/i18n/dictionaries";
import { StatusBadge } from "./status-badge";

const MEDIA_PLAN_DOC =
  "https://github.com/Hmbown/CodeWhale/blob/main/docs/releases/v0.9.2-media-plan.md";
const REPO_BLOB_BASE = "https://github.com/Hmbown/CodeWhale/blob/main";

export function SessionMedia({ asset, locale = "en" }: { asset: MediaAsset; locale?: string }) {
  const t = getDocsShell(locale);

  if (asset.status === "pending") {
    return (
      <figure
        className="session-media session-media-pending"
        data-media-id={asset.id}
        data-media-status={asset.status}
        data-reduced-motion-policy={REDUCED_MOTION_POLICY}
      >
        <div className="session-media-stage">
          <StatusBadge kind="pending" locale={locale} label={asset.pendingLabel} />
          <p className="session-media-pending-note">
            {t.mediaPendingNote}
          </p>
        </div>
        <figcaption className="session-media-caption">
          <strong>{pickText(asset.title, locale)}</strong>
          <span>{pickText(asset.description, locale)}</span>
          <a href={MEDIA_PLAN_DOC} target="_blank" rel="noreferrer">
            {t.mediaPlanLink}
          </a>
        </figcaption>
      </figure>
    );
  }

  // status === "published": manifest tests require every field and verify
  // file presence/byte budgets plus poster dimensions. The human recording
  // checklist remains authoritative for actual video duration/dimensions.
  const poster = asset.poster!;
  const video = asset.video!;
  const captions = asset.captions ?? [];

  return (
    <figure
      className="session-media"
      data-media-id={asset.id}
      data-media-status={asset.status}
      data-reduced-motion-policy={REDUCED_MOTION_POLICY}
    >
      <video
        controls
        preload="none"
        poster={`/${poster.src}`}
        width={video.width}
        height={video.height}
        aria-label={pickText(poster.alt, locale)}
      >
        <source src={`/${video.src}`} type="video/mp4" />
        {captions.map((track) => (
          <track
            key={track.srclang}
            kind="captions"
            src={`/${track.src}`}
            srcLang={track.srclang}
            label={track.label}
            default={track.srclang === "en"}
          />
        ))}
      </video>
      <figcaption className="session-media-caption">
        <strong>{pickText(asset.title, locale)}</strong>
        <span>{pickText(asset.description, locale)}</span>
        {asset.gifFallback && (
          <a href={`/${asset.gifFallback.src}`}>
            {t.mediaGifFallback}
          </a>
        )}
        {asset.transcript && (
          <a href={`${REPO_BLOB_BASE}/${asset.transcript}`} target="_blank" rel="noreferrer">
            {t.mediaTranscript}
          </a>
        )}
      </figcaption>
    </figure>
  );
}
