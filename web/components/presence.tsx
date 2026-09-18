"use client";

/**
 * <PresenceDot> and <WhaleOrb> — the desktop client's two shapes, on the web.
 *
 * The GPUI app says presence with exactly two forms: a 6px lamp beside a word
 * (`workspace/dock/whale.rs:786`) and a round port holding the live particle
 * whale (`:702`). The palette already moved to that client; these are the
 * shapes that went with it.
 *
 * Two rules come from the app and are load-bearing here, not decoration:
 *
 * 1. The dot never speaks alone. `presence_color` notes that "the word is
 *    already the honest claim; this only colours it" — so the colour is a
 *    second channel, never the only one. Colour-blind readers and screen
 *    readers get the same sentence everyone else does.
 * 2. A saturated hue reads neon at dot size. The app steps its success hue
 *    back in alpha for exactly this. The web palette's state hues are already
 *    the warm-muted set (`--gpui-moss` / `--gpui-rust` / `--gpui-tan`), so
 *    they need no alpha step — the rule is inherited, the number is not.
 */

import { useEffect, useMemo, useRef, useState } from "react";

export type Presence = "live" | "attention" | "idle" | "human";

const DOT_COLOR: Record<Presence, string> = {
  live: "var(--gpui-moss)",
  attention: "var(--gpui-rust)",
  human: "var(--gpui-tan)",
  idle: "var(--gpui-ink-mute)",
};

/**
 * A presence lamp and the word it belongs to. `label` is required: a bare
 * coloured dot is a claim nobody can read.
 */
export function PresenceDot({
  presence,
  label,
  className = "",
}: {
  presence: Presence;
  label: string;
  className?: string;
}) {
  return (
    <span className={`inline-flex items-center gap-1.5 ${className}`}>
      <span
        aria-hidden="true"
        className="inline-block h-1.5 w-1.5 shrink-0 rounded-full"
        style={{ backgroundColor: DOT_COLOR[presence] }}
      />
      <span>{label}</span>
    </span>
  );
}

/** The app's particle style, verbatim: rgb(144,185,255) at 0.7 (`pet/tests.rs:5`). */
const PARTICLE = "rgba(144, 185, 255, 0.7)";
const POINT_COUNT = 190;
const VIEW = 512;

/**
 * The whale as the client draws it: points, not a silhouette.
 *
 * Sampled from the same `WHALE_MARK` outline the footer mark uses, so the
 * brand shape has one source. Points drift on a slow sine — one orchestrated
 * motion, not a loop of effects — and hold still under `prefers-reduced-motion`.
 */
export function WhaleOrb({
  size = 104,
  label = "Codewhale",
  path,
  className = "",
}: {
  size?: number;
  label?: string;
  /** The brand outline to sample. Pass `WHALE_MARK` from `./whale`. */
  path: string;
  className?: string;
}) {
  const pathRef = useRef<SVGPathElement | null>(null);
  const [points, setPoints] = useState<Array<[number, number]>>([]);
  const [still, setStill] = useState(true);

  useEffect(() => {
    const el = pathRef.current;
    if (!el) return;
    const total = el.getTotalLength();
    const sampled: Array<[number, number]> = [];
    for (let i = 0; i < POINT_COUNT; i += 1) {
      const p = el.getPointAtLength((i / POINT_COUNT) * total);
      sampled.push([p.x, p.y]);
    }
    setPoints(sampled);
  }, [path]);

  useEffect(() => {
    const query = window.matchMedia("(prefers-reduced-motion: reduce)");
    const apply = () => setStill(query.matches);
    apply();
    query.addEventListener("change", apply);
    return () => query.removeEventListener("change", apply);
  }, []);

  // Deterministic per-point phase: the drift must not resample every render.
  const phases = useMemo(
    () => points.map((_, i) => ((i * 37) % 100) / 100),
    [points],
  );

  return (
    <span
      className={`codewhale-orb inline-flex items-center justify-center overflow-hidden rounded-full ${className}`}
      style={{ width: size, height: size }}
      role="img"
      aria-label={label}
    >
      <svg viewBox={`0 0 ${VIEW} ${VIEW}`} width={size} height={size} aria-hidden="true">
        <path ref={pathRef} d={path} fill="none" stroke="none" />
        {points.map(([x, y], i) => (
          <circle
            key={i}
            cx={x}
            cy={y}
            r={4}
            fill={PARTICLE}
            style={
              still
                ? undefined
                : {
                    animation: `codewhale-orb-drift 6s ease-in-out ${(
                      phases[i] * -6
                    ).toFixed(2)}s infinite`,
                  }
            }
          />
        ))}
      </svg>
    </span>
  );
}
