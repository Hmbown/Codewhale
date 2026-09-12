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

The pet suite currently contains 33 tests: event occupancy/unknown coverage,
late failures, human request pairing, read-only local SSE reconnect/cursor
recovery, deterministic world/score/PCM, checkpoint integrity and continuation.
The standalone verifier compares 190 checkpoints across baseline and edge tapes,
each animated and still. Omission of Swift is explicit in its output.
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
`check-apple.sh` compiles the actual host and runs eight checkpoint/storage/recovery
workflows, including a generated two-hour synthetic unknown recording. It writes
only disposable fixtures under ignored conformance results and temporary storage.
Kotlin 2.3.0 can instead be on PATH; its verifier builds only the particle core
and conformance runner. The Compose view and Android application remain unverified.

## Review direction

Codewhale is a whale living in code. The dots can reorganize to show the work;
the whale is its home form, not a permanent silhouette restriction. Avoid the
old framing of a needy creature turning toward its owner. Evaluate readable,
causal transformations, continuity of particle identity, accessibility and
honest uncertainty. The current seven gaits are a baseline, not the design ceiling.

## Local evidence before publication (2026-09-12)

- Packaged pet: 33 tests passed; existing Whalesong consumers: 306 passed after
  canonical source relocation. These are overlapping suites, not additive coverage.
- Product Node gate: 66 package, 12 SDK and 446 web tests passed; production web
  check subsequently passed with the GitHub release fetch available.
- TypeScript/Rust/Swift: 190 identical checkpoints before and after source relocation.
- Actual TUI: bounded worker, persistence/export/reopen/corruption and two-hour
  continuation exercised in a PTY. Five persistence, eleven paste and nine artifact
  guard tests passed in the product; full Rust unit suite was not run.
- Apple: eight checkpoint workflows passed; iOS Simulator exercised two-hour
  restoration and visible corruption recovery without overwriting the damaged file.
- Previous deterministic Kotlin parity, browser workflows and synthetic audio
  receipts exist locally; they do not establish Android app or listening quality.

Hosted CI and Grokbot findings are separate evidence. Their actual results belong
on the pull request. Open implementation gaps are listed in [README.md](README.md).
