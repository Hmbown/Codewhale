/**
 * <Strata> — the folio's water, drawn as geometry.
 *
 * Four translucent strata of the GPUI palette stacked from the light end of
 * the ombre down to the deep charcoal field, each edge softened the way an
 * ink wash bleeds into paper, with a few fine current lines in the primary
 * blue and the human tan riding the crests. Every colour is a palette token
 * read from the cascade (`currentColor` and the `--gpui-*` custom
 * properties), so the drawing re-inks with the theme and never carries a
 * hex of its own.
 *
 * Two compositions share one vocabulary:
 *
 *   hero — the water rises from the bottom right of the plate and leaves the
 *          top left as paper for the title. Anchored to the plate's floor.
 *   band — a horizontal waterline: paper above, deep field below. Used at
 *          the homepage descent and before the footer on every other page.
 *
 * Decorative and static: `aria-hidden`, no animation, no interaction. This
 * is the one illustrated element on the site; nothing else is drawn.
 */

type Variant = "hero" | "band";

const FILTERS = (
  <defs>
    <filter id="cw-wash-wide" x="-20%" y="-40%" width="140%" height="180%" colorInterpolationFilters="sRGB">
      <feGaussianBlur stdDeviation="34" />
    </filter>
    <filter id="cw-wash" x="-20%" y="-40%" width="140%" height="180%" colorInterpolationFilters="sRGB">
      <feGaussianBlur stdDeviation="20" />
    </filter>
    <filter id="cw-wash-tight" x="-20%" y="-40%" width="140%" height="180%" colorInterpolationFilters="sRGB">
      <feGaussianBlur stdDeviation="11" />
    </filter>
    <filter id="cw-wash-edge" x="-20%" y="-40%" width="140%" height="180%" colorInterpolationFilters="sRGB">
      <feGaussianBlur stdDeviation="5" />
    </filter>
  </defs>
);

function HeroStrata() {
  return (
    <svg
      className="folio-strata"
      viewBox="0 0 1440 1000"
      preserveAspectRatio="xMidYMax slice"
      aria-hidden="true"
      focusable="false"
    >
      {FILTERS}
      {/* Paper haze — a breath of the primary's light end over the top right
          so the sheet is not flat where the water will rise. */}
      <ellipse cx="1180" cy="120" rx="520" ry="200" fill="var(--gpui-primary-dark)" opacity="0.4" filter="url(#cw-wash-wide)" />
      {/* Sky — the light end of the ombre, the first stratum. */}
      <path
        d="M -240 690 C 160 640, 420 720, 660 570 C 880 430, 1040 280, 1240 210 C 1340 175, 1440 150, 1700 110 L 1700 1120 L -240 1120 Z"
        fill="var(--gpui-primary-dark)"
        opacity="0.42"
        filter="url(#cw-wash-wide)"
      />
      {/* Mid water. */}
      <path
        d="M -240 800 C 140 770, 400 830, 620 700 C 830 575, 1000 440, 1200 370 C 1330 325, 1500 280, 1700 250 L 1700 1120 L -240 1120 Z"
        fill="var(--gpui-stage-raised)"
        opacity="0.62"
        filter="url(#cw-wash)"
      />
      {/* The stage plate's muted surface. */}
      <path
        d="M -240 890 C 200 860, 430 905, 650 810 C 870 715, 1060 580, 1260 520 C 1400 478, 1540 445, 1700 410 L 1700 1120 L -240 1120 Z"
        fill="var(--gpui-stage-muted)"
        opacity="0.9"
        filter="url(#cw-wash-tight)"
      />
      {/* The deep field. Its top edge is the crispest, the way the darkest
          wash dries with a line. */}
      <path
        d="M -240 965 C 220 945, 470 975, 710 910 C 910 855, 1110 740, 1310 680 C 1430 645, 1560 615, 1700 1120 L 1700 1120 L -240 1120 Z"
        fill="var(--gpui-stage-deep)"
        filter="url(#cw-wash-edge)"
      />
      {/* Current lines: fine strokes riding the crests. Primary blue on the
          water, ink on the paper, one tan thread for the human mark. */}
      <g fill="none" strokeLinecap="round">
        <path d="M 980 330 C 1100 285, 1200 255, 1340 215 C 1420 192, 1520 170, 1700 140" stroke="var(--gpui-primary-dark)" strokeOpacity="0.5" strokeWidth="1.1" />
        <path d="M 760 640 C 900 560, 1040 470, 1200 405 C 1330 352, 1480 310, 1700 275" stroke="var(--gpui-primary-dark)" strokeOpacity="0.36" strokeWidth="1" />
        <path d="M 620 860 C 840 780, 1040 660, 1240 590 C 1400 535, 1560 500, 1700 470" stroke="var(--gpui-primary-dark)" strokeOpacity="0.3" strokeWidth="1" />
        <path d="M 420 960 C 640 930, 860 860, 1060 780 C 1240 708, 1420 650, 1700 600" stroke="var(--gpui-tan)" strokeOpacity="0.45" strokeWidth="0.9" />
        <path d="M 1010 60 C 1100 110, 1140 180, 1120 260" stroke="var(--ink)" strokeOpacity="0.14" strokeWidth="0.9" />
        <path d="M 1060 40 C 1170 100, 1230 190, 1210 300 C 1200 340, 1180 370, 1150 400" stroke="var(--ink)" strokeOpacity="0.12" strokeWidth="0.9" />
        <path d="M 1150 20 C 1290 80, 1360 170, 1340 300" stroke="var(--ink)" strokeOpacity="0.1" strokeWidth="0.9" />
      </g>
    </svg>
  );
}

function BandStrata() {
  return (
    <svg
      viewBox="0 0 1600 400"
      preserveAspectRatio="xMidYMid slice"
      aria-hidden="true"
      focusable="false"
    >
      {FILTERS}
      <path
        d="M -160 150 C 220 110, 520 190, 820 140 C 1100 95, 1340 160, 1760 120 L 1760 520 L -160 520 Z"
        fill="var(--gpui-primary-dark)"
        opacity="0.45"
        filter="url(#cw-wash-wide)"
      />
      <path
        d="M -160 220 C 240 190, 520 250, 820 215 C 1100 180, 1360 235, 1760 200 L 1760 520 L -160 520 Z"
        fill="var(--gpui-stage-raised)"
        opacity="0.7"
        filter="url(#cw-wash)"
      />
      <path
        d="M -160 285 C 240 260, 540 305, 840 280 C 1120 255, 1380 300, 1760 270 L 1760 520 L -160 520 Z"
        fill="var(--gpui-stage-muted)"
        opacity="0.92"
        filter="url(#cw-wash-tight)"
      />
      <path
        d="M -160 345 C 240 325, 560 360, 860 340 C 1140 322, 1400 352, 1760 335 L 1760 520 L -160 520 Z"
        fill="var(--gpui-stage)"
        filter="url(#cw-wash-edge)"
      />
      <g fill="none" strokeLinecap="round">
        <path d="M -40 175 C 300 140, 560 205, 860 165 C 1120 130, 1380 180, 1660 150" stroke="var(--gpui-primary-dark)" strokeOpacity="0.45" strokeWidth="1.1" />
        <path d="M 120 250 C 420 225, 700 270, 980 245 C 1220 223, 1440 262, 1680 235" stroke="var(--gpui-primary-dark)" strokeOpacity="0.3" strokeWidth="1" />
        <path d="M -40 320 C 300 300, 620 335, 900 315 C 1160 297, 1420 330, 1680 305" stroke="var(--gpui-tan)" strokeOpacity="0.42" strokeWidth="0.9" />
      </g>
    </svg>
  );
}

export function Strata({ variant }: { variant: Variant }) {
  return variant === "hero" ? <HeroStrata /> : <BandStrata />;
}
