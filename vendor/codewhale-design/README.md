# Codewhale GPUI design

`tokens.json` is the versioned semantic design authority. Desktop consumes
`tokens.rs`; GPUI mobile vendors this folder and consumes the same Rust data;
web-next vendors this folder and imports `tokens.css`. No network access or
sibling checkout is required to build a consumer.

Change the JSON here, increment its version, run `python3 design/generate.py`,
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
selection and primary-hover opacity values outside the inclusive 0–1 range.
