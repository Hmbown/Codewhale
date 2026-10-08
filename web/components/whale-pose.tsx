/**
 * The v2 whale character's still poses (public/whale/*.svg, exported from
 * the whale-character-v2 kit by scripts/export-whale-poses.py). The web
 * shows the reduced-motion pose of each action; it never animates by
 * swapping posters.
 *
 * A pose carries meaning, so pick the one that matches what the page is
 * about: `search` for "not found", `hmm` for an error, `busy` while loading,
 * `done` for a finished action, `needs` when the reader must act, and `pod`
 * only where parallel agents are the subject. Decorative by default (the
 * surrounding words say the same thing); pass `label` when the pose alone
 * carries information.
 */
export const WHALE_POSES = [
  "rest",
  "listen",
  "think",
  "busy",
  "read",
  "search",
  "write",
  "run",
  "browse",
  "talk",
  "pod",
  "needs",
  "done",
  "hmm",
  "sleep",
  "computer",
  "connect",
] as const;

export type WhalePoseName = (typeof WHALE_POSES)[number];

export function WhalePose({
  pose,
  label,
  className = "",
  priority = false,
}: {
  pose: WhalePoseName;
  label?: string;
  className?: string;
  priority?: boolean;
}) {
  return (
    // Static, cacheable SVG; next/image adds nothing for a vector file.
    // eslint-disable-next-line @next/next/no-img-element
    <img
      src={`/whale/${pose}.svg`}
      alt={label ?? ""}
      aria-hidden={label ? undefined : true}
      width={124}
      height={124}
      className={`whale-pose ${className}`.trim()}
      data-pose={pose}
      decoding="async"
      loading={priority ? "eager" : "lazy"}
      fetchPriority={priority ? "high" : undefined}
      draggable={false}
    />
  );
}
