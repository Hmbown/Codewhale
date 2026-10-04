# Canonical Whale v2 source

The Rust performance core is adapted from `Hmbown/codewhale-app/src/whale` at
commit `bc1044887b4ef973e444b6f8f83f9389daffee2a`. Its executable design authority
is the same commit's `vendor/whale-character-v2/{acting,rig,props,braille,habitat}.js`.
Codewhale's owner authorized this selected character core and authoring data for
this MIT-licensed library. This export includes no app bindings, owner classifier,
private tool inventory, credentials or session data.

`animation.json` is the unchanged authored pose/clip/spring/rate export;
`mark-data.js` is the unchanged official mark geometry. The Rust rig evaluates
those contours at runtime; animations are not sampled poster cycles. Three short
clips absent from the JSON (HmmBeat, Puff and Blink) remain the native core's
transcription of the canonical `acting.js` definitions.

Selected public fixtures come from the same commit: 153 pose/geometry/packed
motion samples, 32 owner scenarios and their 51 receipts, and 51 terminal stills.
The scenario contexts are authored fixtures, not captured customer sessions.

The terminal color plane retains native ink. `ColoredGrid::paint_with_contrast`
offers a presentation-only contrast floor on measured grounds; the studio uses
3:1 after painting its water. This changes neither packed geometry nor canonical
colors. The ordinary `paint` adapter keeps native ink without this adjustment.

SHA-256 of the embedded authoring files:

- `animation.json`: `ff4e7af2b71aeaa30b6f7b7927cd544dc12bdf51d32387a21ec1ee0bb4fbdb85`
- `mark-data.js`: `e24525bfeecd4df33bfdf3e6b33eedf2e111a1d63e6d7d459d9927f220776a42`
