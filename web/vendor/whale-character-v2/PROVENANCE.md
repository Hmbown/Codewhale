# whale-character-v2 (vendored)

The v2 whale performance — the same Director, rig and props the Codewhale
desktop app (GPUI) and its web client draw — copied unchanged so the website
plays the real character instead of a second motion implementation.

- Source: `codewhale-app/vendor/whale-character-v2/` at `bc1044887b4e`
  (2026-10-01). Files: `mark-data.js`, `rig.js`, `props.js`, `acting.js`,
  `habitat.js`.
- Publication approved by the owner on 2026-10-04 for the public website.
  The same character core is already published as authoring data and a Rust
  port in `codewhale-ratatui` (MIT; `assets/whale-motion/`, `src/whale_motion/`).
- Do not edit these files here. Re-copy from the source when the character
  changes, and update the commit above.
- Loaded by `web/lib/whale-motion.ts`; drawn by `web/components/whale-live.tsx`.
