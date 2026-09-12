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

The pet suite currently contains 48 tests: event occupancy/unknown coverage,
late failures, human request pairing, read-only local SSE reconnect/cursor
recovery and cancellation after garbage collection, deterministic world/score/PCM,
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
  the real signal. Current Windows CI is required to verify the platform fix.
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

Hosted CI and Grokbot findings are separate evidence. Their actual results belong
on the pull request. Open implementation gaps are listed in [README.md](README.md).
