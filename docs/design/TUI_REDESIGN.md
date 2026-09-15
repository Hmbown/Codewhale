# TUI redesign — Shoreline

Branch `tui-redesign`, 2026-09-15. The Codewhale TUI re-inks to the product
client's palette and moves toward the conventions the terminal-agent market
converged on. The whalemark is unchanged and is the constant across every
surface.

Two asks produced this. First: a more professional shell, closer to what the
market ships. Second: closer to the GPUI client, which is deliberately simple.
Both are the same move — `codehwhale-gpui` already *is* the product client, and
it already solved "calm, warm, one accent". The terminal should read as the same
product, not as a second design system.

## What changed

### 1. One palette, two clients (landed)

`SHORELINE_UI_THEME` and `SHORELINE_LIGHT_UI_THEME` in
`crates/palette/src/themes.rs` cite the same slots
`codehwhale-gpui/src/workspace/mod.rs:147-164` installs into GPUI's `Theme`.
The terminal's default was `underwater` — a saturated navy gradient with
decorative ambient life. It is a named theme now, not the ground the product
opens on.

| Role | Value | Note |
| --- | --- | --- |
| field (`surface_bg`) | `#211F23` | warm charcoal |
| plate (`panel_bg`, `composer_bg`) | `#2B282E` | raised |
| chrome (`header_bg`, `footer_bg`) | `#1A181C` | recessed |
| border | `#49424D` | |
| body | `#F2ECE5` | the whale's ivory survives |
| soft / muted / hint / dim | `#D9D2DC` / `#B0A7B2` / `#9A919F` / `#7E7583` | 4.5:1 floors |
| action, selection | `#90B9FF` / `#354967` | the one blue |
| live | `#7FD6C6` | |
| human | `#F6C453` | Signal Gold, unchanged |
| warning / danger / success | `#F0A868` / `#FF8FA8` / `#A3D977` | |
| mode ramp | `#7EB4E8` / `#B9DCEC` / `#AD88FF` / `#FF70A0` | the shipped ramp |

Light is the same system on warm paper (`#F5F0E9` field, `#245BC7` action).

Three mechanisms make it hold:

- **`adapt::theme_remap_active`** lists both new presets, so every direct
  `palette::TEXT_*` / `WHALE_*` call site lands on a Shoreline slot instead of
  a navy-tuned value. Without this the re-ink would be half-applied — the
  frame would repaint while every widget kept its cold grey.
- **The mode ramp is separate from `accent_primary`.** `mode_agent` must not
  equal the action blue: `adapt::theme_semantic_foreground_role` resolves mode
  slots *before* the action lane, so an equal value reads the action lane as
  `ModeAgent` and the ANSI-16 role matrix collapses. Found by
  `every_selectable_theme_keeps_mode_badges_distinct`; fixed in the theme, not
  the test.
- **Every audited pair clears its floor.** `contrast::theme_contrast_violations`
  runs over `SELECTABLE_THEMES`, so the new presets are held to 4.5:1 on the
  four surfaces and 3:1 for hint/dim/status/diff.

`scripts/export-design-tokens.py` reads only `(WHALE|LIGHT)_*_RGB`, so the new
`SHORELINE_*` constants deliberately leave `web/app/tokens.css` untouched —
`--check` still passes. When the web surfaces are re-inked, that pattern is the
one line to widen.

### 2. What the market actually does (surveyed, not assumed)

Seven of nine reference checkouts under `refs/` are terminal TUIs (codex,
opencode, kimi-code, grokbuild, oh-my-pi, piagent, prime-agent); `dsh` and
`openhands` are not TUIs at all. The convergent rules:

1. **No persistent top bar.** 7/7. Branding is launch-only — codex's session
   card and grokbuild's cwd bar are the first transcript cell, they scroll
   away.
2. **Exactly one status row, never two, never above the composer.** All five
   that have a status line.
3. **That row carries model + cwd + git branch + context % + cost.**
4. **Full-width composer.** 7/7. No reading-column cap — this is where the
   terminal convention and the desktop window legitimately differ, and the
   terminal wins here.
5. **Assistant turn = bare markdown on the field.** User turn = a filled
   background or one coloured glyph, never bold text alone.
6. **Tool calls are collapsible cards or one-line headers**, not bare lines.
7. **One accent over a near-black field.** `#141414` recurs as the base field
   in three separate projects.

Against that list the TUI is already close on 1, 4, 5 and 6 — `work_surface::height`
returns 0 unless a panel is explicitly open, the composer is full width, and
tool receipts are typed cells. The live deviations are 2 and 3, and the launch
screen's proportions.

### 3. Deliberately not done

- **The composer stays full width.** The GPUI client centres a 690px column;
  no terminal agent does. Narrowing it here would be the desktop's layout
  applied where the medium disagrees.
- **The ocean is not deleted.** `underwater` keeps its water column, ombre and
  ambient life. It is selectable, and `/theme` lists it. Nothing is lost.

## Remaining work

### The one-status-row merge (highest value, not started)

Two rows sit under the composer:

- slot 6, the posture bar — `frame.rs:1820-1835` →
  `phase_strip::tideline_footer_from_app` (`phase_strip.rs:1270`) →
  `render_tideline_footer` (`phase_strip.rs:953`).
- slot 7, the metrics line — `frame.rs:1841-1844` → `render_info_row`
  (`frame.rs:387`), fed by `info_segments` (`frame.rs:61`).

They are separate systems with separate shed ladders, separate hitbox
registries (`register_footer_count_targets`, `register_info_interaction_targets`)
and separate golden families, so this is a merge, not a deletion.

The shape that fits the existing design: **the footer becomes the one row**,
gaining a route segment and a cost segment. `TidelineFooterFacts` already
carries `context_percent` (it uses it for the ≥80% cap warning), and
`phase_strip::route_identity_fields` — which the info line already calls — is
the shared route formatter, so both segments are one call each. `metrics_line`
then defaults to `ChromeRowPreset::Hidden` and the transcript gets the line
back.

**The blocker to solve first, not after:** the metrics line owns two pointer
targets — `/model` on the route segment and the context inspector on the meter.
Hiding the row without re-homing them removes two interactions users have
today. The footer's count rects are the precedent to copy: paint the segment,
return its `Rect`, register the target. Do that in the same commit that
hides the row, or the slice ships a regression.

### The launch screen

`underwater::launch_empty_state` (`underwater.rs:1948`) rides the hero mark
(14×6, `MarkSize::Large`) beside three text lines and then a session card. Two
things read as splash rather than product: the mark is unindented at the top
left, and its six rows are taller than the text column beside it, so `New
session` floats in the mark's dead space. The GPUI launch is a centred compact
lockup — mark, wordmark, one heading, one line of subtitle. Worth doing; the
golden churn is contained to the launch surfaces.

### Adjacent, out of scope here

- Two `main` test failures pre-date this branch and are unrelated:
  `runtime_api::tests::set_config_rejects_unknown_key_with_bad_request`
  (the code says `unknown setting`, the test expects `unknown config key` —
  both present at `4a85cb7877`) and
  `tui::views::tests::every_settings_row_reaches_a_store`
  (`auto_compact_threshold_percent` is a schema row `Settings::set` never
  accepted; also present at `4a85cb7877`).
- `arrow_navigation_wraps_at_picker_edges` and
  `theme_picker_uses_shared_settings_controller` are order-dependent under a
  bare `cargo test`: they read the real user theme directory, so a
  `custom:` row changes the last picker row. Run tests through
  `scripts/dev-test.sh`, which supplies the hermetic HOME.
- `crates/tui/src/tui/work_surface/tideline.rs:207-263` (`tideline_rail_groups`,
  the RUNS/WHALES/FLEET/WORK/CONTEXT groups) is `#[allow(dead_code)]`
  translation scaffolding. The live rail paints `RailPanel` dock tabs. It
  should be deleted, not reimplemented.

## Verification

Goldens record cell symbols only — `golden_harness.rs:27-49` dumps
`cell.symbol()`, so **a palette change is invisible to every golden buffer**.
The `ink plane` the harness documents at `:84-94` would close that hole and is
documented but unimplemented. Until it exists, palette regressions are caught
by the contrast audit and by a real terminal capture, not by a golden.

What was run, with counts, is recorded in the three commits on this branch.
