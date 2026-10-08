# DSH bundle skin — palette via the documented theme API

Status: accepted for v0.9.9 (owner decision 2026-08-16).
Supersedes the 0.9.8 `--skin` CSS export, which is dead on arrival by
construction: `dsh-client-ui-layout` writes alias tokens as inline
`body.style.setProperty(...)` (`lib/client.js:375` in 0.1.0-rc.6), and inline
vars beat any stylesheet rule. The export is removed; the palette now rides
the one mechanism DSH actually supports.

## Goal

`dsh --profile codewhale` (the `install-bundle` profile) renders in the
Codewhale palette — Blue Stage dark and light — with an explicit Whale
Brothers / Codewhale identity lockup, applied through
`ctx.theme.overrideTokens`, the documented token-level override in
`@deepseek-ai/dsh-client-ui-theme`. No build toolchain, no runtime deps, no
injection hacks.

## Verified seams (dsh 0.1.0-rc.6, installed at
`/Users/hunterbown/.npm-global/lib/node_modules/@deepseek-ai/dsh`)

1. `ThemeService.overrideTokens(source: string, tokens: Record<string,
   {light: string, dark: string}>): () => void` — stacks a partial token
   layer over the active theme; later layers win per-token; returns a
   disposer that removes exactly this layer.
   (`dsh-client-ui-theme/lib/types/client/index.d.ts:167`)
2. Client plugins: `package.json` gains
   `dsh.client: { platform: "web", immediately: true, inject:
   ["@deepseek-ai/dsh-client-ui-theme"] }` and `exports["./client"]`.
   `lib/client.js` is a plain script of the form
   `window.__ModuleLoader__.load({ id, factory })` — mirror the wrapper
   boilerplate from `dsh-client-ui-theme/lib/client.js` verbatim.
   The module must also export `inject = ["theme"]`: cordis 4 exposes a
   sibling plugin's service on `ctx` only through the plugin's own `inject`
   (reading `ctx.theme` without it throws `cannot get property "theme"
   without inject`, which fails the web boot). The package-level
   `dsh.client.inject` only orders the boot manifest.
3. Overlay row insert: `cordis.patch.yml` gains
   `- insert: [{ id: codewhale-skin, name: codewhale-dsh-bundle }]` under
   the existing root-entry list, appending our entry last (patch rows apply
   in order; last wins).

## Design

### Rust — token table as the single source of truth

`crates/tui/src/integrations/dsh/skin.rs`:

- New: `pub(crate) struct SkinTokens` / `pub(crate) fn skin_tokens() ->
  BTreeMap<String, (String, String)>` — alias name → (light, dark), both
  rendered from the real TUI palette (`crates/palette/src`, Blue Stage
  dark + light). Port every mapping from today's `alias_map()` +
  `theme_block()`.
- New: `pub(crate) fn bundle_client_js() -> String` — renders the client
  half: `__ModuleLoader__.load` wrapper + `factory` whose module applies
  `ctx.theme?.overrideTokens("codewhale-dsh-bundle", TOKENS)` inside
  `ctx.effect(() => ...)` and returns the disposer, with
  `exports.inject = ["theme"]` so cordis defers `apply` until the theme
  service exists (plus a belt-and-braces `if (!ctx.theme) return;`).
  TOKENS is a JSON
  literal rendered from `skin_tokens()`; values are palette constants only —
  no secrets, no user data, no environment. Include a
  `codewhale-skin/<version>` comment header for diffability.
- Delete: `skin_css()`, `skin_preview_html()`, `SKIN_FILE`,
  `SKIN_PREVIEW_FILE`, and their call sites. The `--skin` flag survives with
  new semantics (below). Keep `hex()`/`mark_data_uri()` only if still used.

### Bundle — dual-face plugin

`bundle.rs::render_bundle_files`:

- `package.json` gains `dsh.client` (per seam 2) and
  `exports` covering `"."` (`./lib/index.js`), `"./client"` (`./lib/client.js`) and `"./package.json"` — Node exports maps are exhaustive, and both the cordis loader (bare import) and `dsh-client-modules` (`require.resolve("<name>/package.json")`) need their subpath.
- Emits `lib/index.js` (trivial Node cordis plugin: `apply` is a no-op; it
  exists so the entry mounts) and `lib/client.js`
  (`bundle_client_js()`).
- `cordis.patch.yml` gains the insert row (seam 3) — only when skin enabled.
- `install-bundle [--skin true|false]` (default **true**) and
  `update --skin true|false`: `false` regenerates the bundle without the
  client half and without the insert row. Decision recorded in the receipt
  (`skin: bool`, `skin_sha256`: SHA-256 of the rendered TOKENS JSON).
- Stale detection: a missing/modified `lib/client.js` while the receipt says
  skin=true (or present while receipt says false) reports `stale-config`,
  same as a drifted patch. `remove-bundle` deletes the client half with the
  rest of the Codewhale-owned bundle files.
- `connect --skin` stays accepted, means true, and now only controls the
  future bundle (the `--patch` overlay path never carries code — palette is
  bundle-profile only; `launch --profile web|headless` stays overlay-only).

### Failure handling

- `theme` service never provided (non-web composition) → the client entry
  stays pending on its `inject`; the Node half never throws.
- `overrideTokens` validation errors surface in the browser console with
  source id `codewhale-dsh-bundle` (dsh behavior; we do not catch/swallow).
- pnpm missing → unchanged refusal (existing behavior).

## Tests

1. Token table: every key starts `--dsw-alias-`; every value has non-empty
   light AND dark; both schemes differ where the palette differs (at least
   bg + label-primary); the rendered TOKENS JSON round-trips through
   serde_json back to the table.
2. `bundle_client_js()` snapshot test (deterministic given version + table);
   asserts the source id string and `ctx.effect` disposal shape; asserts no
   `skin_css`/`<style` output remains.
3. Bundle files: with skin on, `package.json` parses and carries
   `dsh.client.inject` containing `@deepseek-ai/dsh-client-ui-theme`;
   `cordis.patch.yml` ends with the insert row; with skin off, neither
   exists. Receipt carries `skin` + `skin_sha256`.
4. Stale detection unit tests for present/absent/modified client half vs
   receipt decision.
5. Live check (manual, this machine — dsh 0.1.0-rc.6 + pnpm installed):
   `codewhale integrations dsh install-bundle`, `launch`, then a browser
   screenshot asserting `body` background equals the Codewhale surface color
   in BOTH schemes (flip `ui-theme.preference`); `remove-bundle` restores
   stock DSH.

## Docs

- `docs/INTEGRATIONS_DSH.md`: rewrite the Skin section — from "unsupported
  overlay, never injected" to "applied through the bundle profile via
  `overrideTokens`, on by default, `--skin false` disables, disposable and
  reversed by `remove-bundle`". Remove the CSS/preview paragraphs.
- CHANGELOG (release branch, not this PR): `Added` (palette via documented
  theme API) + `Removed` (dead CSS/preview export, with the inline-vars
  reason).

## Ocean scene (v0.9.9 addendum, owner request 2026-08-17)

The palette alone recolors DSH; it does not make it *look* like Codewhale.
`crates/tui/src/integrations/dsh/scene.js` (owned by `scene.rs`,
`include_str!`) is a plain-script fragment that `skin::bundle_client_js(true)`
splices into the client half. It defines `createOcean(palette)`; the client
module calls it inside a second `ctx.effect`, starts it, follows
`theme/change`, and stops/unmounts on dispose.

Verified seam: `dsh-client-modules` serves only `/plugins/<id>/client.js`
(+ `.map`) per client package (`lib/index.js:321-327`), so a sibling
`lib/scene.js` would never be fetched — hence the splice, and no separate
file in the bundle. `overrideTokens` validates only the `{light, dark}`
shape, not the token name, so `--dsw-specific-sidebar-fill` can ride the same
layer as the alias tokens.

Design: one near whale (≈0.34 × viewport width, clamped 240–560 px) and one
far, smaller, fainter whale on slow linear crossings (75–110 s) with a
gentle sine in Y, pitch clamped to ±0.12 rad; paths are biased to the lower
half (near) and the top edge (far) so neither crosses the composer card. The
silhouette is a single filled shape (no eye) at ~5:1 length:height: blunt
rounded head, long back with a low soft dorsal hump about two thirds back,
slightly convex belly, thin tail stock, a HORIZONTAL fluke drawn as a wide,
low, notched T seen with a hint of perspective (lobes sweep back, the lower
one a touch longer for the downward curl — never a vertical fish tail), and
one long pectoral flipper (~1/3 body length) sweeping down and back from a
third of the way along the body. The fluke flexes ±10° about the tail stock
(added to the path under a rotation, so no point math); when a whale is in
the top third it occasionally releases a short bubble stream from the head.
A 16-fish school of `><>` / `><o>` glyphs (14 px code font, scaled
0.85–1.25) follows a lissajous leader with damped steering and facing
hysteresis; 26 stroked bubbles; a two-stop depth gradient. Layer order:
gradient, far whale, bubbles, spouts, fish, near whale. Palette per scheme
is derived from the skin's `surface_bg` / `accent_primary` / `text_body` /
`text_dim`; canvas alphas (near 0.64 light / 0.78 dark, far 0.4 / 0.46) are
deliberately visible through the lighter veil.

Visibility: `--dsw-alias-bg-base` → rgba α 0.42 and
`--dsw-specific-sidebar-fill` → rgba α 0.78 (`scene::ocean_veil_tokens`),
merged over the opaque `TOKENS` only when the scene is on. The lighter main
veil makes the cast unmistakable while the stronger sidebar layer keeps
navigation distinct. Every other layer stays opaque.

Budget and guards: rAF capped at ~30 fps, `visibilitychange` pause,
`prefers-reduced-motion: reduce` → one settled static frame, DPR ≤ 2,
typed arrays reused, no per-frame string/array allocation. Off switch:
`localStorage["codewhale.ocean"] = "off"` or body class
`codewhale-ocean-off` (both also skip the veil); `window.__codewhaleOcean`
exposes `start/stop/setIntensity/setScheme`. Config: `update --ocean
true|false` (default on; receipt `ocean`, package.json `codewhale.ocean` +
`codewhale.ocean_scene_sha256`); the client-half byte check makes an ocean
toggle or a drifted scene report `stale-config`.

Live check (this machine, dsh 0.1.0-rc.6, headless Chromium): canvas
present (`fixed`, `z-index:-1`, `pointer-events:none`), two frames 700 ms
apart differ, console clean, dark via `prefers-color-scheme` follows through
`theme/change`, reduced-motion frame is static. Screenshots:
`docs/design/assets/dsh-ocean-light.png`, `docs/design/assets/dsh-ocean-dark.png`.

## Whale Brothers / Codewhale identity (v0.9.9 addendum)

`brand.js` renders a plugin-owned top-right lockup through DSH's additive
`shell.overlay` slot, with the literal hierarchy
`WHALE BROTHERS` / `CODEWHALE` / `× DEEPSEEK HARNESS`. It is deliberately
additive: no DSH-owned branding, controls, or DOM classes are replaced. The
surface is token-driven and pointer-inert, collapses to a compact whale mark
below 760 px via media CSS, and unmounts with the client plugin. `package.json`
records `brand_sha256` so generated bundle identity covers the lockup as well
as the palette and ocean.

## Out of scope (decided)

Title/favicon, persona text, DSH-owned layout, any non-bundle injection path,
supporting dsh newer than the verified rc.6 (reported
honestly as `stale-version`).
