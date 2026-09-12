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

The pet suite currently contains 40 tests: event occupancy/unknown coverage,
late failures, human request pairing, read-only local SSE reconnect/cursor
recovery and cancellation after garbage collection, deterministic world/score/PCM,
checkpoint integrity and continuation, expression-version validation and legacy replay.
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
workflows, including a generated two-hour synthetic unknown recording. It writes
only disposable fixtures under ignored conformance results and temporary storage.
Kotlin 2.3.0 can instead be on PATH; its verifier builds only the particle core
and conformance runner. The actual Compose application is a separate Gradle build:

```sh
cd pet/android
./gradlew --no-daemon assembleDebug lintDebug
./gradlew --no-daemon connectedDebugAndroidTest
```

Use JDK 17 and Android SDK 35; connect a device/emulator for the second command.
The seven instrumentation tests run the real QuickJS binding, Kotlin renderer,
PCM cursor, storage/recovery and Compose lifecycle. See [Android](android/README.md).

## Review direction

Codewhale is a whale living in code. The dots can reorganize to show the work;
the whale is its home form, not a permanent silhouette restriction. Avoid the
old framing of a needy creature turning toward its owner. Evaluate readable,
causal transformations, continuity of particle identity, accessibility and
honest uncertainty. Version 2 supplies work-driven knots, strands, branches, layers, circulation and
open junctions; the same particle identities return to the whale at rest.

## Local evidence before publication (2026-09-12)

- Packaged pet: 40 tests passed; existing Whalesong consumers: 306 passed after
  canonical source relocation. These are overlapping suites, not additive coverage.
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
- Apple: nine checkpoint workflows passed; iOS Simulator exercised two-hour
  restoration and visible corruption recovery without overwriting the damaged file.
- Android: debug APK and lint build pass; seven instrumentation tests pass on
  Android 15 ARM64, including 4,800 frames, checkpoint continuation, exact PCM,
  lifecycle and storage recovery. The emulator audio sink runs without host
  speaker output; this does not establish physical listening or power quality.
- Recovery exports: the shared 32,000-bucket fixture exceeds the native autosave
  bound and restores its exact checkpoint. Android also exports 90,000 pending
  interactions from its real 64 MiB QuickJS heap after the 8 MiB autosave path
  rejects them. Apple save-conflict recovery preserves the live core, selected
  source and preference; its exported pose/score restores exactly.
- Authenticated read-only attachment to an existing local Runtime 0.9.13 session
  journal passed. An 18-second recorder run produced 45 contiguous unknown buckets
  and stopped cleanly in 41 ms after Ctrl+C, without treating historical work as
  current activity. This does not establish a new active provider turn or native
  companion acceptance. The old idle-stream shutdown failure is covered by a
  real HTTP regression fixture that forces garbage collection before closing.

Hosted CI and Grokbot findings are separate evidence. Their actual results belong
on the pull request. Open implementation gaps are listed in [README.md](README.md).
