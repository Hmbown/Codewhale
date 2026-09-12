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

### Foreground Watch

`/workbar watch` selects `RailPanel::Watch`. Existing dock cycling, tab hitboxes,
focus, dismissal, placement and resizing apply. The tank uses the work surface's
available body; it never overlays transcript cells. Narrow views keep the text
cue. Selecting Watch starts a read-only observer; it does not start an Engine
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

The workspace's existing QuickJS dependency runs the actual world and score on
one lazy worker thread: 64 MiB memory, 128 queued commands, at most ten seconds
of fixed-tick catch-up and a five-second execution ceiling per command (ten seconds for initial restore). It has
no host filesystem, network, provider or execution APIs. Queue loss or runtime
failure withholds the old frame and emits a localized warning toast; reopening
Watch retries. The UI event loop remains the only draw authority. Session changes
drop the previous observer. Reduced motion and attention holds suppress gait
animation; settled shell phases remain still. New facts can still change the
caption and pose. Chrome and body ink use existing palette roles; Failure ink
requires an actual error channel.

The portable Rust particle core is exported as `codewhale_tui::pet`, including
its deterministic conformance helpers. This makes its existing external
renderer contract explicit instead of hiding unused public fields behind a
dead-code exemption.

Watch saves its accepted tape and a versioned world checkpoint together under
the existing session artifact directory, `artifacts/pet/habitat.json`. It saves
every five seconds and attempts a final save when its command channel closes.
A process crash can lose work since the last successful checkpoint. A fresh
session without an ID remains in memory until it is saved by the session owner.

Reopening Watch in the same saved session restores the exact particle state,
seeded behavior stream, pod identities, interaction cursor and score cursor.
It then advances to the first unrecorded interval, which is explicitly unknown.
Historical approvals and tool spans cannot establish present activity. The
restored creature clock and the host's frame-freshness clock remain separate.

The sidecar reuses confined session artifact I/O, private atomic writes and an
advisory writer lock. A content revision rejects stale writers and external
edits. Invalid files, changed locks, unavailable storage and the 8 MiB native
habitat limit stop persistence with a localized warning; existing files remain
intact and the current in-memory recording can still be exported within that
limit. The host archives consumed history after 1,024 buckets or 4,096 applied
inputs before replacing the active habitat. Each archive starts with an exact
checkpoint, and the running world retires history only after storage succeeds.
The active QuickJS worker remains limited to 64 MiB; retained archives grow on
disk. The standalone live recorder separately rotates bounded JSONL segments at
the same watched pathname. Its `--resume` option preserves the previous tape in
an archive and starts unknown at that path; the saved world stays with its host.
A process-lifetime OS lock excludes competing recorders and releases on crash.
See `pet/README.md` for limits and recovery.

`/workbar watch export` writes a new immutable `artifacts/pet/replay-<id>.json`
and reports its path through the existing toast system and transcript. Import it in the web
habitat to replay the accepted telemetry and interactions. Export is available
after opening Watch in a saved session.

`/workbar watch sound on|off` controls app-local, default-off sound. The existing
QuickJS worker retains current score voices and calls the shared `PetNative.pcm`
renderer at 48 kHz. A single `ffplay` child receives bounded Float32LE stereo
packets through a four-slot queue. There is no second score, clock or event
interpreter. PCM rendering has a separate 500 ms interrupt budget; sound failure
does not invalidate the world or its recording. Muting, obscuring Watch, quiet
mode, stale frames and session changes stop the child and queued sound. The
audio thread reaps it; the UI never waits for playback or a pipe write. Delayed
and excess packets are discarded. A slow player cannot build an unbounded
backlog or turn a temporary stall into a permanently muted session. Reopening
begins at the current creature clock.

FFmpeg is optional and is never downloaded by the application. Missing `ffplay`,
or device failure mutes sound with a translated warning;
Watch remains usable. Sound status shares the existing caption only when it
fits after the full semantic and unknown-coverage cues.

The live Runtime SSE adapter and this foreground protocol adapter consume
different existing input contracts; they share event-v1 projection semantics,
the bucketer, world and score. The Runtime stream now has opt-in progress frames
at its own journal cursor. The companion waits for durable replay and queued
live delivery to finish before extending any request as current. These frames
are transport metadata; they do not enter the event journal or pet tape.
The Runtime importer separately pairs request
lifecycles and preserves failure receipt times. Its local SSE/CLI and browser
fixtures pass. Authenticated read-only observation of an existing local Runtime
journal is verified; new active-session delivery through the native companions
still needs acceptance QA. The
source and liveness contracts are documented in `pet/README.md`.

The full TUI binary builds offline, its foreground metadata projection test
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
