# Dot-whale habitat and Watch

The underwater transcript uses the portable 980-point whale for its completion
visitor. The whale is the home form of a persistent field of dots. Version 2
reorganizes those dots into reasoning knots, woven code, filesystem branches,
browser layers, circulating traffic and open decision junctions. Original
recordings retain version 1 behavior; new habitats use version 2. This slice
replaces the old `≈≈>` / spray / fluke glyph sequence. The explicit Watch panel
adds foreground telemetry through the same world and score used by native and
web hosts, with opt-in stereo PCM output through FFmpeg's `ffplay`.

## Ownership

- `crates/tui/src/tui/ambient_life/pet_sim.rs` owns the dependency-free Rust port
  of the TypeScript reference core, with typed `ChannelId` and `Archetype`.
- `whale-points.tsv` is the embedded authored body. `PetSim::whale()` constructs
  it using the existing `mulberry32(0xC0FFEE)` particle stream.
- `pet_widget.rs` owns particle-to-braille rendering. The portable Rust runner
  and ratatui demo use these same product files, rather than maintaining copies.
- `pet_cameo.rs` is the completion presentation inside the existing habitat.
  It consumes `WhaleCameo` and `AmbientActivity`; it never reads a wall clock,
  schedules a frame, or classifies model/tool text.

## Completion is a bounded observation

A completion does not identify which Whalesong category was last active. The
single visitor therefore uses `other · drift`, with observed coverage. Existing
`AmbientActivity::Subagents` presents three fixed, staggered peer slots, each
labelled `agent · pod`. At rest the three peers remain three animals; the core's
six-satellite working gait is not multiplied into eighteen animals.

The complete creature and its caption are preflighted against `is_open_water`.
A text collision with either withholds the entire creature. Off-screen peers
are withheld instead of clamping them into an overlapping stack. On a 40-column
surface the middle peer remains visible. Content always owns its cells.

The core is stepped at fixed 30 Hz into two bounded raster caches. A frame is
selected solely from the supplied completion age, so skipped draws, rewind,
resizes, and rendering another session do not alter its pose. There are no new
random choices. The existing theme supplies the habitat ink; the cameo adds no
status-bar colour meaning and never spends Failure red.

The shell already owns an 800 ms completion light pulse and a 600 ms life fade.
It now passes the motion-gated clock through that full 1.4-second interval. The
ocean still clips its light pulse at 800 ms. The low-level cameo retains the
2.4-second safety limit from its predecessor; the shell's life fade ends first.
Reduced motion exits before constructing marks or initializing the caches.

The generic widget retains the core's unknown rendering and the literal
`· unobserved` cue. If the whole cue cannot fit, it withholds the whole widget.
Sleep dimming does not set unknown coverage.

## Evidence boundaries

### `/pet` habitat

`/pet` (or `/pet on`) opens the full habitat over the existing shell. The pet
has no workbar panel: no dock cycling, tab, placement or dismissal applies. The
tank uses the whole content viewport and never overlays transcript cells; narrow
views keep the text cue. Opening it starts a read-only observer; it does not start an Engine
turn or silently substitute demonstration data. It begins with unknown coverage
and does not backfill events from before attachment.

`ui/event_loop.rs` passes accepted foreground Engine events through the existing
`core::protocol_parity::event_to_protocol` projection. The pet allowlist keeps
only lifecycle kind, stable ids, tool names, channel and outcome. Prompt text,
reasoning, arguments, results, paths and routing credentials never enter the
worker queue or pet tape. No session context or KV-cache prefix is changed.

The generated `pet_watch/pet-native.js` comes from the canonical source in `pet/src/core`:

```sh
npm --prefix pet ci --ignore-scripts
npm --prefix pet run sync
```

The Engine adapter emits normalized event-v1 observations, then uses the existing
`compilePetTelemetry` function. A sealed 400 ms observation bucket appears at
the next world boundary; the world's accepted tape remains the replay authority.
Liveness pulses do not count as repeated tool calls. Silence does not stretch
occupancy across gaps. Error receipts create a current onset, rather than moving
a failure back to the operation's start. Typed `ShellPhase::Waiting/Approval`
can extend an already witnessed human request, preserving the outstanding request without approach-to-owner steering.

The presentation companion now owns the one persistent world. Terminal Watch,
full-screen habitat, browser, macOS window, Apple Shared and Android Shared
attach to its authenticated loopback snapshots. View interpolation and stillness
never advance or modify the world. The existing Ratatui draw loop owns pixel
placement and cleanup; a successful Kitty query selects pixels, otherwise the
view uses braille. Full habitat focus uses the existing modal stack.

The complete ownership, source selection, storage/reconnect, graphics, audio,
legacy habitat and mobile contracts live in [pet/SHARED.md](../../pet/SHARED.md).
That document supersedes the former per-session Watch worker and app-local
live-world ownership. Completion cameos remain bounded transcript decorations.

The live Runtime SSE adapter and this foreground protocol adapter consume
different existing input contracts; they share event-v1 projection semantics,
the bucketer, world and score. The Runtime stream now has opt-in progress frames
at its own journal cursor. The standalone file recorder waits for durable replay and queued
live delivery to finish before extending any request as current. These frames
are transport metadata; they do not enter the event journal or pet tape.
The Runtime importer separately pairs request
lifecycles and preserves failure receipt times. Its local SSE/CLI and browser
fixtures pass. Typed synthetic Engine events have also passed through the actual
Runtime journal/SSE endpoint, recorder process and Apple file watcher/PetHost:
a fresh request appeared at 1,233 ms and cleared after its answer at 2,000 ms.
That provider-free fixture proves active native file delivery; physical-phone
transport and visual popover inspection are separate evidence. The source and
liveness contracts are documented in `pet/README.md`.

Earlier phase evidence: the full TUI binary built offline, its foreground metadata projection test
passes, and the real PTY walkthrough covers Watch selection, keyboard, mouse
and resizing down to 40x12. Receipts and source hashes are in
`CW/artifacts/pet-review-20260912/`. Hosted macOS testing on the initial branch ran 15,418 tests with one Watch-tab
golden mismatch; its header is corrected in the transformation update. The
subsequent `e69e99b` passed the applicable Linux, macOS and Windows product tests,
lint and security checks. Later changes require their own hosted verdict.

Local render fixtures exercise the actual ocean, glyph and ambient-life modules
with literal shell enums extracted from `underwater.rs`; they do not run an
Engine session. The portable conformance runner compares TypeScript, Rust and
Swift across the original tape and an additional all-channel/error-burst/sleep
tape, each with motion enabled and disabled. A missing tool or failed widget
build now fails that runner instead of printing a false success.

Full product builds, real session acceptance, packaged applications, audio,
mobile builds and hosted CI remain separate proof requirements. The local
fixture and conformance checks do not establish those outcomes.
