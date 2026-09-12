# Pet review packet

Review the exact commit checked out (`git rev-parse HEAD`). Report the source SHA,
OS/toolchain, commands, actual nonzero pass/fail counts, and reproducible findings.
Synthetic fixtures prove local contracts, not provider or customer acceptance.

## Fast Linux or macOS review

```sh
npm --prefix pet ci --ignore-scripts
npm --prefix pet run check
npm --prefix pet run sync
git diff --exit-code -- crates/tui/src/tui/pet_watch/pet-native.js pet/ios/Resources
cargo fetch --locked --manifest-path pet/tui/Cargo.toml
./pet/verify.sh --no-swift
npm test
npm run check:web
```

The pet suite currently contains 63 tests: event occupancy/unknown coverage,
late failures, human request pairing, read-only local SSE reconnect/cursor
recovery, replay readiness and cancellation after garbage collection, bounded
long sessions and recorder process restart, deterministic world/score/PCM,
checkpoint integrity and continuation, immutable segment boundaries, pending input
retention, browser saves overlapping source changes, live-file freshness/restarts,
delayed browser reads crossing suspension, expression-version validation and legacy replay.
The standalone verifier compares 380 checkpoints across baseline and edge tapes,
each animated and still, under expression versions 1 and 2. Version 1 must also
match pinned pre-transformation golden digests. Omission of Swift is explicit in its output.
`npm test` requires the root and web dependencies described by the product.
`check:web` fetches public GitHub release metadata and requires network access.

For the full terminal, start with one build job on memory-limited machines:

```sh
cargo build --locked -j 1 -p codewhale-tui --bin codewhale-tui
cargo test --locked -j 1 -p codewhale-tui --lib tui::pet_watch::
```

Do not rerun a full library test link that is already exhausting the machine.
Report build/resource blockers and continue independent source/browser QA.
Manually inspect `/workbar watch`, narrow layouts, paste/Enter focus, session
save/reopen, immutable `/workbar watch export`, and import that exported JSON in
the browser. Unknown must remain legible; stale approvals must not resurrect.
Use a disposable session and synthetic/loopback telemetry for this assignment.

With FFmpeg's `ffplay` on PATH, explicitly enable `/workbar watch sound on`.
Check sound off/on, switching away from Watch and back, quiet mode, and a missing
or failed player. These must not stop the world or its checkpoint. No historical
sound should play after resuming. Unit tests use a capture sink or a disposable
child process; they never open an audio device. Actual output and listening
quality need separate manual evidence.

## Apple and Kotlin

```sh
./pet/verify.sh
./pet/scripts/check-apple.sh
./pet/ios/build.sh
PET_KOTLIN_LIB=/path/to/gradle/lib ./pet/android/verify.sh
```

Apple requires macOS, Xcode command-line tools, and XcodeGen for iOS. The iOS
script generates and builds the Simulator project directly from the shared
source, without first building macOS. Open `pet/ios/CodewhalePet.xcodeproj` to run.
`check-apple.sh` compiles the actual host and runs checkpoint/storage/recovery
workflows, including a generated two-hour synthetic unknown recording, segment
publication, a save conflict that must preserve the running world, and actual
live-file appends, pause/resume, in-place restarts, replacement and recreation. It writes
only disposable fixtures under ignored conformance results and temporary storage.
The same executable checks 27 tone/noise/long-clock PCM ranges at three sample
rates through the actual JavaScriptCore Float32 boundary, comparing sample bits
with the retained JSON path and rejecting malformed or nonfinite buffers.
Kotlin 2.3.0 can instead be on PATH; its verifier builds only the particle core
and conformance runner. The actual Compose application is a separate Gradle build:

```sh
cd pet/android
./gradlew --no-daemon assembleDebug lintDebug
./gradlew --no-daemon connectedDebugAndroidTest
```

Use JDK 17 and Android SDK 35; connect a device/emulator for the second command.
The eleven instrumentation tests run the real QuickJS binding, Kotlin renderer,
PCM cursor, storage/recovery, immutable segments and Compose lifecycle. A test-only
document provider delivers synthetic bytes through real descriptor IO; live pause,
background, restart and malformed input reach the actual ViewModel. See [Android](android/README.md).

## Review direction

Codewhale is a whale living in code. The dots can reorganize to show the work;
the whale is its home form, not a permanent silhouette restriction. Avoid the
old framing of a needy creature turning toward its owner. Evaluate readable,
causal transformations, continuity of particle identity, accessibility and
honest uncertainty. Version 2 supplies work-driven knots, strands, branches, layers, circulation and
open junctions; the same particle identities return to the whale at rest.

## Local evidence before publication (2026-09-12)

- Packaged pet: 57 tests passed; existing Whalesong consumers: 310 passed after
  the incremental Runtime importer change. These are overlapping suites, not
  additive coverage.
- Product Node gate: 66 package, 12 SDK and 446 web tests passed; production web
  check subsequently passed with the GitHub release fetch available.
- TypeScript/Rust/Swift/Kotlin: 380 identical checkpoints across both expression
  versions. Legacy golden digests are unchanged.
- Actual TUI: bounded worker, persistence/export/reopen/corruption and two-hour
  continuation exercised in a PTY. Five persistence, eleven paste and nine artifact
  guard tests passed in the product. The initial hosted macOS suite ran 15,418
  tests with one outdated Watch-tab golden; that header was corrected. Subsequent
  hosted product CI on `e69e99b` passed Linux, macOS and Windows tests, Rust lint
  and the applicable safety/security checks. Each later commit needs its own verdict.
- Apple: eleven checkpoint/live-file workflows passed; iOS Simulator exercised two-hour
  restoration and visible corruption recovery without overwriting the damaged file.
- Android: debug APK and lint build pass; nine native tests and both Compose
  lifecycle tests pass on
  Android 15 ARM64, including 4,800 frames, checkpoint continuation, exact PCM,
  lifecycle and storage recovery. The emulator audio sink runs without host
  speaker output; this does not establish physical listening or power quality.
- Recovery exports: the shared 32,000-bucket fixture exceeds the native autosave
  bound and restores its exact checkpoint. Android also exports 90,000 pending
  interactions from its real 64 MiB QuickJS heap after the 8 MiB autosave path
  rejects them. Apple save-conflict recovery preserves the live core, selected
  source and preference; its exported pose/score restores exactly.
- Segment continuation: 3,240,000 fixed ticks (30 synthetic hours in Still) produced
  263 immutable archives totaling 163.1 MB. Every rotation restored an exact
  checkpoint; the active file was at most 0.441 MB at sampled saves. Hourly
  post-GC Node heap samples ranged from 7.89 to 8.59 MB. This is shared-core
  simulation, not a continuous native-device, provider or listening test.
- Native segments: 12 TUI Watch tests pass, including immutable archive ordering
  and stale-writer rejection. Apple's generated two-hour recording archives
  5.64 MB and continues from a 0.31 MB active habitat; a failed save keeps the
  live world intact. Android rotates 4,096 applied inputs, rejects a competing
  writer, restores exact continuation and detects a changed archive.
  A constructed long-clock checkpoint also checks the Android renderer's
  phase/clock/jitter bounds against the shared core and continues in both engines.
- Browser: changing sources and reopening an earlier imported recording restores
  its pose and full future timeline. Earlier recordings remain available after
  reload. A 30-hour segment seeks from its retained origin through its final
  checkpoint with Still enabled. iOS Simulator exports an earlier recording
  through Files and reports a successful save.
- Browser save ordering: controller tests defer storage completion while changing
  worlds and accepting an input. The previous `5c70db4` controller fails by
  treating the pending save as a failure; the correction waits, archives the new
  input, and still preserves the source when a real failed save is canceled.
- Live freshness: the previous `45a1130` browser controller fails the stale-file
  attachment regression. Its native core also throws on live resume after normal
  host ticking because the Engine-only clock has not advanced. The shared resume
  path now uses the world clock, drops old voices and retains an exact checkpoint.
  Browser tests also discard a read started before suspension; this is controller
  evidence, not a claim about a physical device or browser file-picker support.
- Runtime fixture timing: the HTTP test now waits for recorded waiting/error
  coverage before answering, and for two sealed unknown bins before stopping.
  Its former fixed sleep occasionally stopped the child before those final bins
  on a busy runner. The receipt assertions and privacy checks remain unchanged.
- Continuous recording: the actual watch CLI rotates three replayable files and
  continues at the original live pathname. File tests cross the 24-hour sequence,
  count UTF-8 bytes, follow replacement with the real cursor, and preserve
  existing files on an archive collision or external replacement. History is
  retained on disk; this does not resume a recorder after process restart.
  A Grokbot finding also covers errors after successful publication: row/byte
  accounting now advances at replacement, before fallible cleanup or reporting.
  The previous `e71be2c` writer fails the new post-publication continuation test.
  Its first Windows CI run also found open handles preventing file replacement
  and CRLF point data rejecting the whale body. Rotation now closes both writer
  handles before replacement; point readers trim each row. The browser controller
  tests now use CRLF assets and fail against the previous controller. CLI tests
  use a test-only IPC preload for Windows shutdown-handler coverage because
  `child.kill('SIGINT')` forcibly terminates a Windows process; POSIX tests retain
  the real signal. The next Windows run passed 56/57 tests but found intermittent
  reader contention during replacement. Windows replacement now retries for
  under two seconds and rechecks destination identity, size and modification
  time before each attempt. Current Windows CI is required to verify that fix.
- Bounded Runtime input: a real local HTTP/SSE fixture supplies 260,001 records
  spanning nearly 29 hours of event timestamps. The importer retains fewer than
  512 events and 256 KiB of metadata, including a still-open human request. The
  published `43e6f02` transport fails the same fixture at record 155,859 when its
  cumulative raw journal reaches the old limit. These are synthetic timestamps,
  not a 29-hour provider or native companion run. Smaller fixtures compare recent
  buckets with full imports through pruning, late failures, shifted origins,
  request answers, automatic consent and turn completion.
- Authenticated read-only attachment to an existing local Runtime 0.9.13 session
  journal passed. An 18-second recorder run produced 45 contiguous unknown buckets
  and stopped cleanly in 41 ms after Ctrl+C, without treating historical work as
  current activity. This does not establish a new active provider turn or native
  companion acceptance. The old idle-stream shutdown failure is covered by a
  real HTTP regression fixture that forces garbage collection before closing.

These chronological receipts describe the source at each milestone; later
sections supersede earlier open items. Hosted CI and Grokbot findings are separate
evidence on the pull request. Current limits are listed in [README.md](README.md).


## Recorder process restart — September 12, 2026

`--resume` restarts a live recorder at its existing watched pathname. It validates
and archives the previous complete tape without changing its bytes, continues
numbered archives, and starts a fresh unknown segment. The existing host habitat
owns world continuation. No outage duration or activity is inferred from mtime.
A private empty sidecar uses Node SQLite's OS lock to exclude simultaneous
writers and release ownership on close or process death. It stores no events.

The local pet check passed 61 tests with zero failures. The focused recorder
checks cover repeated restarts, archive gaps, baseline/fresh live delivery,
invalid and incomplete source preservation, a competing writer, changed locks,
same-size external changes, SIGKILL followed by restart, and clean CLI shutdown.
Against the previous `6d960525` recorder, resuming rejects with EEXIST and a
same-size external rewrite is incorrectly accepted; the new guards reject it.
The required `npm test && npm run check:web` gate passed 66 package, 12 SDK and
446 web tests, with zero failures and two existing web-check warnings.

This changes the standalone recorder, its tests and documentation. World, score,
TUI and native bundle bytes are unchanged from the previously verified build.
Grokbot review and current-head Windows/macOS recorder CI are separate receipts.
Native active-session delivery, current macOS popover inspection and device
transport remain open. No release or deployment is implied.


The first restart commit `11c6fbfc` passed all 61 tests independently in
Codewhalebot on Node 22.19.0 with no remaining concrete source finding. macOS
recorder CI also passed. Windows passed 60 tests and canceled one on timeout:
the test-only IPC message listener kept a correctly rejected startup alive.
The same preload reproduced the hang locally; unreferencing its IPC channel
lets the rejected CLI exit with code 1. The process-death/restart test now uses
that IPC path on every host, while other POSIX cases still use actual SIGINT.
The fixture also waits for CLI readiness before shutdown: the archive link may
appear before replacement and before the signal handler is installed. A broad
local run exposed this early-stop race after the IPC fix. Production recorder
and native bundle bytes are unchanged in this follow-up.


## Runtime replay-to-live handoff — September 12, 2026

An old human request must stay unknown while its journal answer is still in the
replay backlog. The Runtime event endpoint now offers opt-in `stream.progress`
frames and advertises that capability. It declares live observation only after
both durable replay and the queued live tail have drained; broadcast lag returns
the stream to replaying while durable history catches up. These frames are
transport metadata, not journal events, and allocate no new sequence numbers.
Default SDK callers keep the existing event-only stream.

The pet requires that explicit handoff. Journal packets alone cannot establish
current observation. An older Runtime or SDK without progress support stops
input with a clear diagnostic and leaves the recorder unknown. In particular,
the earlier read-only Runtime 0.9.13 receipt above is historical evidence, not
compatibility evidence for this new live transport.

The local pet check passed 63 tests, with zero failures or cancellations. The
required `npm test && npm run check:web` gate passed 66 package, 14 SDK and 446
web tests; web checks reported zero errors and two existing warnings. Three
actual Runtime Rust tests passed for opt-in endpoint negotiation, queued answers
at handoff, and broadcast-lag recovery; the existing event-only handoff test also
passed. The TypeScript SDK declarations passed their compiler check.

A real loopback HTTP fixture delays a 60-second-old request's answer. The
published `c342329f` transport marks that pending historical request current;
the corrected transport keeps it unknown. Further fixtures verify that the old
answer arrives before readiness, a fresh request becomes visible, replay reentry
suppresses observation, and unsupported streams cancel before accepting input.
The 260,001-record bounded-input fixture still passes with readiness markers.
These are synthetic event fixtures with real transports, not provider calls.

World, score and native bundle bytes are unchanged. Hosted CI, independent
Codewhalebot QA, and active-session native delivery remain separate evidence.


## Apple audio and scope audit — September 12, 2026

Apple now transfers the shared generator's Float32 channels directly from
JavaScriptCore. Each channel's type, length and finite samples are checked; its
borrowed pointer is copied immediately while the JS value remains rooted.
The JSON PCM method remains available to QuickJS consumers. There is one score
and sample generator.

Actual AVAudioEngine playback exposed schedules more than a second in the past
after a 750 ms UI stall. Presentation now rebases against the current world and
retires stale voices after long gaps. Completion callbacks carry numeric tickets
back to the main actor, preventing an old completion from retiring a newer node.
A sound failure mutes and persists the sound preference while world ticks and
recording exports continue. The previous Apple host stops the world on the same
injected PCM failure; the corrected host advances from 400 ms to 933 ms and exports.

Local final-source receipts:

- Pet checks: 63 passed, zero failed/canceled. Required product gate: 66 package,
  14 SDK and 446 web tests passed; web check has zero errors and two existing warnings.
- Apple: all 11 checkpoint/live-file/storage workflows and 27 exact PCM ranges
  pass, plus empty/invalid range and four malformed-buffer checks. macOS builds,
  signs and verifies; iOS Simulator builds from the same final Swift sources.
- Real AVAudioEngine: 150 frames complete with a forced 750 ms stall. All 12
  scheduled onsets remain in the future (42–118 ms); the prior output drifts
  down to 1,703 ms late. This proves scheduling, not subjective listening quality.
- An 80-second synthetic profile renders 122 voices. Audio-frame p95 falls from
  70.6 ms on JSON transport to 23.4 ms on final typed transfer, with the same
  final particle digest and sample peak. An earlier typed-transfer run had zero
  frames over 33 ms; the final run had six wall-time outliers (worst 738 ms) on
  the concurrently used machine. This is not a hard frame-time guarantee.
- The regenerated bundle passes 12 actual TUI Watch tests and all 11 Android
  instrumentation tests together, plus APK assembly and lint. The score and
  particle algorithms are unchanged.

The original A–F workstreams map to implemented, buildable source:

| Workstream | Implementation and evidence |
| --- | --- |
| A — World | Shared seeded behavior state machine, environment, interaction journal, doze/wake, persistent pod identities and versioned forms. Browser/Apple/Android replay and checkpoint workflows; 380 four-port conformance checkpoints. |
| B — Audio | One thirteen-category score and PCM generator; browser WebAudio, Apple AVAudioEngine, Android AudioTrack and opt-in TUI ffplay. Deterministic PCM, native cursor tests, Apple output/stall/failure receipts. Listening quality remains a separate human assessment. |
| C — TUI | Existing Engine events feed Watch through the canonical bucketer; bounded cameo honors open-water collision, palette and motion rules. Actual product build, PTY interaction and 12 Watch checks. |
| D — macOS | Locally signed LSUIElement application, menu-bar controls, launch-at-login option, Still/sound/source settings and DispatchSource live files. Real process persistence and host workflows; current popover inspection is unverified. |
| E — Mobile | SwiftUI Simulator app and Compose Android app use the same core and local-file contract. iOS build plus prior Simulator restoration/export/recovery; Android APK/lint plus 11 real-engine/device-lifecycle tests. No automatic phone link is claimed. |
| F — Live authority | Existing Engine journal and opt-in Runtime replay progress feed one event-v1 bucketer. Synthetic Engine → actual Runtime/SSE → recorder → native Apple host passes without provider calls or raw prompt capture. |

Source and build instructions are published on PR #6110. Commit-specific
Grokbot review and GitHub checks are recorded there. Shipping, signing for public
distribution and subjective audiovisual acceptance are not implied by these
local builds or by a prior commit's green CI.
