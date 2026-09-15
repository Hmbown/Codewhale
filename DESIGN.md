---
name: Codewhale
description: Tidal Folio — a paper sheet read under the sea. Ivory paper and a serif title voice above the waterline, the whale's navy and blue below; one palette owned by the TUI tokens.
colors:
  # brand constants (brand/*.svg, shared with the TUI palette)
  brand-black: "#000000"
  brand-ink: "#070c1d"
  brand-navy: "#0c1531"
  brand-stage: "#142352"
  brand-ice: "#ddeef9"
  brand-cobalt: "#1535b2"
  brand-blue: "#6aa6dc"
  brand-cyan: "#78bce8"
  ombre-start: "#1535B2"
  ombre-end: "#6AA6DC"
  # the sheet — crates/palette/src/tokens.rs, exported to web/app/tokens.css (generated, never hand-edit)
  paper: "#f6f2e8"          # WHALE_TEXT_BODY, Whale Ivory — the page above the waterline
  paper-deep: "#e8eef8"     # LIGHT_ELEVATED — the shallows; cards and code-adjacent plates
  paper-card: "#fffdf8"     # LIGHT_PANEL — a raised sheet on the paper
  paper-edge: "#a9b8cf"     # LIGHT_BORDER
  ink: "#14213a"            # LIGHT_TEXT_BODY
  ink-soft: "#455168"       # LIGHT_TEXT_SOFT
  ink-mute: "#5b6780"       # LIGHT_TEXT_MUTED
  action: "#1535b2"         # WHALE_COBALT — links and controls on paper
  action-deep: "#142352"    # WHALE_COMPOSER — primary button fill, hover for links
  human: "#7a5500"          # LIGHT_HUMAN — Signal Gold at AA on ivory
  live: "#08766d"           # LIGHT_LIVE
  # the water — the same tokens, dark side
  bg: "#070c1d"             # WHALE_BG, the deep field and the footer seabed
  chrome: "#0c1531"         # WHALE_CHROME, terminal plates on either side of the waterline
  panel: "#101c40"
  composer: "#142352"
  elevated: "#1a2c63"
  border: "#2a3f72"
  text-body: "#f6f2e8"
  text-soft: "#b6c0d4"
  text-muted: "#93a0b8"
  action-on-dark: "#6aa6dc" # WHALE_ACTION, the sky end of the ombre
  ice: "#ddeef9"
  gold: "#f6c453"           # WHALE_HUMAN, the one gold thread in the water
typography:
  display:
    fontFamily: "Newsreader, Georgia, 'Times New Roman', serif"
    fontSize: "clamp(2.75rem, 6.4vw, 5.75rem)"
    fontWeight: 500
    lineHeight: 0.98
    letterSpacing: "-0.022em"
  heading:
    fontFamily: "Newsreader, Georgia, 'Times New Roman', serif"
    fontSize: "clamp(1.6rem, 2.9vw, 2.6rem)"
    fontWeight: 500
    lineHeight: 1.1
    letterSpacing: "-0.022em"
  subheading:
    fontFamily: "Shannon Sans, ui-sans-serif, system-ui, sans-serif"
    fontSize: "1.12rem"
    fontWeight: 600
    lineHeight: 1.25
  body:
    fontFamily: "Shannon Sans, ui-sans-serif, system-ui, sans-serif"
    fontSize: "1rem"
    fontWeight: 400
    lineHeight: 1.6
  rubric:
    fontFamily: "JetBrains Mono, ui-monospace, monospace"
    fontSize: "0.75rem"
    fontWeight: 500
    letterSpacing: "0.12em"
  code:
    fontFamily: "JetBrains Mono, ui-monospace, monospace"
    fontSize: "0.85rem"
    fontWeight: 400
    lineHeight: 1.55
rounded:
  none: "0px"
  sm: "5px"
  md: "6px"
  plate: "8px"
  pill: "999px"
spacing:
  xs: "4px"
  sm: "8px"
  md: "16px"
  lg: "24px"
  xl: "40px"
  section: "clamp(3.5rem, 7vw, 6rem)"
components:
  button-primary:
    backgroundColor: "{colors.action-deep}"
    textColor: "{colors.paper}"
    rounded: "{rounded.sm}"
    padding: "12px 22px"
    typography: "{typography.body}"
  button-secondary:
    backgroundColor: "transparent"
    textColor: "{colors.action-deep}"
    border: "1px solid {colors.action-deep}"
    rounded: "{rounded.sm}"
  terminal-plate:
    backgroundColor: "{colors.bg}"
    border: "1px solid rgb(221 238 249 / 0.22)"
    rounded: "{rounded.plate}"
    shadow: "0 30px 60px -24px rgb(7 12 29 / 0.75)"
  nav:
    backgroundColor: "rgb(246 242 232 / 0.94)"
    textColor: "{colors.ink}"
    height: "62px"
---

## Overview

The website is a folio read under the sea. The top of every page is paper —
Whale Ivory, the very ink the terminal paints on its dark stage — set with a
large serif title voice and thin navy rules. Every page descends: the
homepage through a waterline into the whale's navy for the surfaces, the
composer, and the community; every other page through a shorter waterline
into the footer, which is the seabed. The terminal capture floats at the
waterline like a lantern. The direction in one line, from the founder:
**"like it's a scroll we're reading under the sea."** Serious product,
manuscript character, the product's own ocean ombre.

This replaces the all-dark "Tideline stage" website of 2026-09-01. The TUI
and the signed-in app keep their dense dark workbench; the website is the
paper the product is read from. The web-specific product truth is in
`PRODUCT.md` next to this file.

## Anti-slop rules

Hard rules, not taste notes.

1. **One drawn thing.** The water in `web/components/strata.tsx` is the only
   illustration on the site: four translucent strata of the palette, softened
   like an ink wash, with a few fine current lines. No photographs of water,
   no stock imagery, no second illustration, no turbulence or grain filters.
   The strata are geometry drawn from tokens — never a hex of their own.
2. **No gradients as decoration elsewhere.** The ombre `#1535B2 → #6AA6DC`
   lives in the mark, the wordmark, and the water. The ocean column's field is
   the site's own chrome → bg descent — the navy water column is the TUI's
   selectable `underwater` theme, not the ground the terminal opens on. That
   ground is Shoreline, the warm charcoal and single blue shared with the GPUI
   client (`docs/design/TUI_REDESIGN.md`). No spotlight glows, gradient text,
   or gradient rules on paper.
3. **One shadow.** The terminal plate at the waterline casts one soft, offset,
   blue shadow. Nothing else on the site has a drop shadow.
4. **No generic SaaS scaffolding.** No icon-card grids, logo walls,
   testimonials, hero metrics, or badge soup. Sections are ruled columns and
   fact lists on paper.
5. **No fabricated evidence.** The one screenshot is the founder's own capture
   of the v0.9.12 development build, captioned as exactly that. No invented
   transcripts, benchmarks, or mockups; pending media stays `pending`.
6. **No cloud claims.** Availability is stated per surface as it is today —
   terminal released, web app account sign-in available with the workbench a
   development preview, desktop a development build, cloud computers not
   available yet — and the page changes when the state does.
7. **Two dials, exact names.** Plan / Work / Operate and Ask / Auto-Review /
   Full Access are typeset literally and never ranked; Full Access is a
   choice, never described as a default.
8. **No text below the floors** (12px functional, 11.2px rubrics) and no
   text/background pair under 4.5:1 on either side of the waterline.

## Colors

One palette, owned by `crates/palette/src/tokens.rs` and exported to
`web/app/tokens.css` by `scripts/export-design-tokens.py` — both the
`WHALE_*` dark tokens (`--whale-*`) and the Blue Stage light preset's
`LIGHT_*` tokens (`--light-*`). `web/app/globals.css` maps them to the site's
semantic names and never repeats a hex:

- **Above the waterline (`:root`)** — `--paper` is Whale Ivory
  (`WHALE_TEXT_BODY`), `--ink` the light preset's navy, `--indigo` cobalt for
  links and outlines, `--indigo-deep` brand navy for the primary fill and
  hover, `--signal-gold` and `--jade` at their light-preset AA values.
- **Below the waterline** — one rule (`.ocean-column, .site-footer,
  html[data-theme="dark"] .docs-theme`) re-inks the same names with the dark
  whale tokens, so a component is written once and reads correctly on either
  side. `--indigo` becomes the sky blue `WHALE_ACTION`; the mark becomes the
  white silhouette.
- **Terminal plates** (`pre.code-block`, the screenshot frame, the install
  composer) are always the terminal's own navy, on paper or in water.
- The brand ombre exists in the mark, the wordmark, and the water only.
- State colours (`success`, `warning`, `error`, `human`) carry meaning and
  never convey state alone.

## Typography

Three faces with distinct roles:

- **Newsreader 400/500 (+ italic)** — the display voice: `h1`, `h2`, the
  gain columns' titles, the chapter title on the water. Book weight, tracking
  −0.022em, `text-wrap: balance`. Loaded through `next/font/google` as
  `--font-serif`. Never used below 1.3rem.
- **Shannon Sans variable 100–900** — body, buttons, links and small headings.
  `--font-body` and the historic `--font-display`/condensed role share one local
  upright face; their existing weights and scale distinguish the roles. Measure
  ≤ 70ch. The font and its OFL notice live in `web/public/brand/fonts/`.
- **JetBrains Mono 400/500** — code, the `cw` dot chain, the plate's rubric
  (`AGENTIC COMPUTING, ON YOUR TERMS`), the running heads (`02 / YOUR MODELS`).
  These rubrics are the only tracked uppercase on the site.

Han locales drop the tracking and set the serif slots in the CJK serif stack.

## Layout

- One container (`--container: min(100% - 2rem, 76rem)`); every gutter aligns
  with the nav.
- **The plate** (`.folio-hero`): two columns at ≥ 1050px — copy left, water
  right; the terminal spans the left column's second row, the chapter marker
  sits on the water bottom-right. Below 1050px it stacks: copy, terminal,
  marker; the water still rises from the plate's floor.
- **Reading sections** (`.folio-section`): serif `h2` (max 24ch), an optional
  lede (max 40rem), then either three ruled columns (`.folio-gain-grid`) or a
  two-column chapter (`.folio-chapter-grid`) with a fact list on the right.
- **The waterline** (`.folio-waterline`, and `.site-footer-waterline` on every
  other page): a band of the water, paper above, deep below.
- **The ocean column**: the surfaces list, the composer install band (bracketed
  by Signal Gold and Operate violet, as in the TUI), community, footer.
- Breakpoints: 1050px (plate stacks), 760px (columns stack), 520px (compact
  nav, full-width buttons). No horizontal overflow at 390px, ever.

## Motion

The page is complete and static. Motion answers a person's action: a hover
draws a rule, a press stamps the copy button, the compact sheet settles in.
The one ambient moment is the ocean column's 90-second breath, opacity only,
gated on `prefers-reduced-motion: no-preference`. No scroll-reveal.

## Components

- **Nav**: paper at 94%, hairline below. Left: navy mark + navy wordmark as
  one link. Centre: Product · Models · Plugins · Pricing · Docs. Right: theme (docs
  only), locale, stars, Sign in / Create account, one filled Install button.
  The compact sheet adds Start · Install · FAQ · Community · Contribute.
- **Buttons**: `.folio-button` — brand-navy fill (primary) or navy outline
  (secondary), body face, sentence case. Portal buttons on secondary pages
  keep their mono meta style but use the same inks.
- **Fact lists** (`.folio-fact-list`, `.folio-availability-list`): hairline
  rows, mono term on paper / serif term in the water, body description.
- **Terminal plate**: the screenshot at native 1136×698 with a chrome-navy
  caption carrying the build line and the `cw` dot chain.
- **Footer**: the waterline band, then the seabed with the inverted wordmark.

## Do's and Don'ts

Do
- Derive every fact from the repo; one owner per number.
- Write a component once and let the below-the-waterline rule re-ink it.
- Keep the whale mark and wordmark together in the nav; wordmark alone in the footer.
- Meet AA and the 12px floor on paper and in the water before shipping.

Don't
- Add a second illustration, a photograph, a shadow, or a gradient on paper.
- Claim cloud execution, a released desktop app, or a default of Full Access.
- Add page-local copy; extend `lib/content/` and the dictionaries.
- Restore the scroll-reveal or any per-section entrance motion.
