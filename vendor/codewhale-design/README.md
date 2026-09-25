# codewhale-design

Keep every Codewhale surface on one set of design tokens.

This repository is the source of truth for Codewhale's colours, type, spacing,
radii and motion constants. The desktop app, the website, the terminal UI
(`codewhale-ratatui`) and the mobile app each vendor a copy and verify it at
build time, so no consumer needs the network or a sibling checkout.

Update tokens: edit `tokens.json`, bump its version, run `python3 generate.py`,
then run `scripts/sync-to.sh <consumer-repo>` for each consumer.

## Details

`tokens.json` is the versioned semantic design authority. Desktop consumes
`tokens.rs`; GPUI mobile vendors this folder and consumes the same Rust data;
web-next vendors this folder and imports `tokens.css`. No network access or
sibling checkout is required to build a consumer.

Change the JSON here, increment its version, run `python3 generate.py`,
and copy this entire folder into each consumer's `vendor/codewhale-design`.
Run the generator with `--check` in each repository. Generated files include the
source digest; consumer tests reject drift. Layout remains native to each
surface. Focus and reduced-motion settings must remain accessible on each host.

Typography and palette come from the shipping desktop theme. Icons retain the
24-unit, 1.7-pixel rounded stroke family; semantic names and accessible labels
stay with the host controls. Desktop spring constants and reduced-motion poll
cadence are shared without introducing decorative animation on web or phones.

CSS consumers use `font-family: var(--font-family), var(--font-fallbacks), sans-serif`
to keep the shared family and CJK fallbacks together. The generator rejects
selection and primary-hover opacity values outside the inclusive 0–1 range,
and any palette that misses WCAG AA contrast in either mode: every text role
at 4.5:1 and `border_strong` at 3:1 on `background`, `surface`, `sidebar`,
`hover` and `selected`, and `primary_foreground` on `primary` at 4.5:1. A new
colour, such as a named blue, enters the tokens only once it passes.
