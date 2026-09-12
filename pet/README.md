# Codewhale: a whale living in code

The dots are Codewhale's material. The whale is its recognizable home form;
real activity can reorganize that material into a living expression of the work.
This is an evolving audiovisual instrument, not a mascot that asks for attention.

The current implementation has a persistent 980-particle world, thirteen event
categories, deterministic audio, replay and checkpoints. Version 2 reorganizes
the same particles into knots for reasoning, woven strands for code, branches for
filesystem activity, scanning layers for browsing, and circulating paths for
network traffic. Human input opens a junction in the field. It does not steer
the creature toward its owner. The whale re-forms as activity settles.
These are authored expressions of measured activity, not generated pictures of
arbitrary task content. Missing telemetry stays visibly unknown.

## Build from this repository

Requires Node 22.13 or later and Python 3 for the local static server.
No sibling repository, investor folder, credentials or provider call is needed.

```sh
npm --prefix pet ci --ignore-scripts
npm --prefix pet run check
npm --prefix pet start
```

Open <http://127.0.0.1:4632/pet.html>. Wild is a simulated creature; Event demo
is synthetic telemetry. Import trace / tape accepts event-v1, OTLP, existing
Codewhale session/Runtime journals and saved pet recordings. Sound requires an
explicit gesture. Still preserves a static semantic pose. Follow local tape
uses the browser's File System Access API; other browsers support import/export.

```sh
# Regenerate embedded QuickJS and Apple bundles from the same source.
npm --prefix pet run sync
# Full product terminal; use /workbar watch and /workbar watch export.
cargo build --locked -p codewhale-tui --bin codewhale-tui
# Small standalone braille renderer, without the full product link.
cargo fetch --locked --manifest-path pet/tui/Cargo.toml
cd pet
./verify.sh --no-swift
```

`./verify.sh` without the flag additionally requires Swift and compares all
three ports. The script fails for a missing tool or failed build; it does not
silently call an omitted platform verified. See [QA.md](QA.md) for native builds.

The [Android application](android/README.md) builds with its Gradle wrapper and
JDK 17. Compose, native audio, checkpoint import/export and lifecycle behavior
are exercised on an Android 15 emulator; CI uploads the debug APK and test reports.

In the full terminal, `/workbar watch sound on` enables the shared score and
`/workbar watch sound off` mutes it. Sound starts off each time the application
opens. Install FFmpeg with `ffplay` on PATH to use this optional output; Watch
and recording work without it. The terminal streams the core's stereo 48 kHz
PCM to one player process. Hiding Watch, opening a modal, quiet mode, or stale
telemetry presentation suspends output. Reopening starts at the current clock,
without playing the intervening history. Player failure mutes sound and reports
a warning while the world continues. `/workbar watch sound` shows its status.

## One source of meaning

`src/core/pet-telemetry.ts` derives 400 ms buckets from normalized Whalesong
event-v1. Measured span occupancy selects the channel; onset counts, repeat
density, error receipts, human requests and agent identities carry other facts.
Container spans are excluded. Open-ended spans without observation coverage,
disconnects and missing intervals are unknown, not successful idle time.

`pet-world.ts` advances a 30 Hz creature clock and named seeded random streams.
It journals accepted telemetry, interactions, behaviors and persistent pod slots.
`pet-audio.ts` schedules voices from that same clock. Noise uses absolute sample
positions, so buffer partitioning cannot change the score. Audio mixing is a
host concern. `pet-native.ts` exposes this exact core to QuickJS/JavaScriptCore.
The TUI, Apple and Android hosts do not carry their own event bucketers or score schedulers.

The Rust particle implementation lives in the product's
`crates/tui/src/tui/ambient_life/pet_sim.rs`; this package's runner imports it.
Swift and Kotlin particle ports must match its conformance digests. Generated
native bundles are committed so the product Rust build needs no Node compiler.
Run `npm run sync` after changing core source; the generated-byte check in CI
rejects a stale bundle. Local Whalesong consumers use aliases to these same
canonical files, not separately maintained source copies.

The original Whalesong importer, signal model, schema and browser storage code
are included because they are actual dependencies of the pet. Their original
Apache-2.0 [license](LICENSE) and [notice](NOTICE) are retained. Rust files imported
from the product retain that repository's license.

## Live recording

The adapter only reads the existing Runtime journal endpoint
`GET /v1/threads/{id}/events`. It never starts a turn.

```sh
cd pet
node scripts/pet.mjs --runtime=http://127.0.0.1:7878 --thread=THREAD_ID --output=pet.jsonl
node scripts/pet.mjs --input=trace.jsonl --output=other.pet.jsonl --watch
node scripts/pet.mjs --demo --output=demo.pet.jsonl
```

Choose an existing thread and an unused output path. Optional authentication
comes from `CODEWHALE_RUNTIME_TOKEN`; tokens are rejected in URLs. Only plain
HTTP loopback IP origins are accepted. Redirects, invalid envelopes and cursor
holes are rejected; reconnects resume from the last accepted Runtime cursor.
The recorder seals the preceding observation interval against a fixed clock.
It retains request lifetimes and counts a delayed error once at receipt time.
Raw prompts, arguments, results and tokens do not enter the pet recording.

The thin wire is JSONL, one flat version-1 `PetBucket` per line: PetState plus
`sequence`, `simTimeMs`, `durationMs`, thirteen-element `onsets` and `activeMs`,
`errors`, `agentIds` and `waiting`. Sequence starts at zero in 400 ms steps.
Saved replay JSON contains this accepted tape and the interaction journal;
a versioned checkpoint also contains particle, random-stream and score cursors.
Recordings also store `expressionVersion`: new worlds use version 2; recordings
without this field retain the original version 1 particle and behavior rules.
Checkpoints must agree with the recording's expression version. Unsupported or
mismatched versions are rejected before changing the current world. This keeps
old saved habitats replayable while letting new worlds change their visual form.
The older TSV is a particle conformance tape and cannot preserve audio onsets.

macOS watches `~/.codewhale/pet-state`. iOS watches `pet-state` in Documents.
File replacement and source restart are handled by the existing host watchers.
Authenticated read-only attachment has been exercised against a running local
Runtime 0.9.13 and an existing session journal. Old completed work remains unknown
at the recorder's current clock. The recorder also closes an idle stream after
garbage collection. Receiving new active-session work through native companion
surfaces still needs acceptance QA.

## Persistence and current limits

The browser commits checkpoint and recording together with an optimistic
IndexedDB revision. A saved live source reopens as Replay until explicitly
reattached. TUI Watch saves under the owning session's `artifacts/pet/habitat.json`;
Apple hosts keep source-specific files in Application Support/CodewhalePet.
Android keeps separate wild/demo/recording habitats in private app storage.
Native writes are private and atomic, use a writer lock and content revision,
and preserve corrupt files or external edits. Recovery is visible in the UI.

Modern checkpoints hydrate without replaying historical simulation. Live resume
starts unknown and drops old human requests. Apple legacy wild preferences
migrate only after a successful checkpoint save. Native autosave is every five
seconds, so a crash may lose work since the last successful save.

Native habitats are limited to 8 MiB; the QuickJS worker has a 64 MiB memory
limit. Two-hour restoration has been exercised, but serializing full long tapes
still costs seconds in QuickJS. History rotation and long-session performance
remain open. Physical Android device acceptance, macOS popover inspection,
active-session companion acceptance, content-driven forms and final listening/power quality
are unfinished. This branch is a reviewable prototype, not a release candidate.
