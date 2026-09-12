# Android habitat

This is a runnable Compose application, backed by the same committed world and
score bundle as the terminal and Apple hosts. The existing Kotlin particle
renderer receives that world's state and persistent pod slots. It owns no
telemetry bucketer or score scheduler. The app has no Internet permission.

Use JDK 17 and an Android SDK with Platform 35 and Build Tools 35.0.0. Set
`ANDROID_HOME` to that SDK, or set `sdk.dir` in an untracked `local.properties`.
The Gradle wrapper pins and verifies Gradle 8.14.5. Dependencies are pinned in
`build.gradle.kts`; the first build needs Google Maven and Maven Central.

```sh
cd pet/android
./gradlew --no-daemon assembleDebug lintDebug
# With an Android device/emulator connected:
./gradlew --no-daemon connectedDebugAndroidTest
adb install -r build/outputs/apk/debug/CodewhalePet-debug.apk
```

Minimum Android version is 8.0 (API 26). Local device verification uses an
Android 15/API 35 ARM64 emulator. This does not establish physical-device sound,
battery use, or acceptance on all supported Android versions.

Wild is a simulated creature. Event demo is synthetic telemetry. More → Import
recording opens the same `petReplayVersion: 1` exports as the other hosts.
Checkpoint-bearing recordings resume their exact world; exports without a
checkpoint replay from the beginning. Missing expression versions retain v1.
Both kinds can be exported again. There is no live companion connection yet.

Sound starts off on each process launch. One native AudioTrack receives the
core's stereo 48 kHz float PCM. A bounded queue drops late output. Pause,
backgrounding, audio-focus loss and headphone disconnection stop sound; an
audio failure leaves the world and saves running. Still also honors the system
animator-duration setting. Color is accompanied by semantic text and TalkBack
descriptions. Portrait and landscape share the same dots.

Each mode has a separate private, atomic recording, saved every five seconds
and on suspension. Revision checks reject competing writers. Invalid files are
retained. More → Start fresh habitat preserves the previous file as a recovery
copy; More → Export previous world makes that copy available outside the app.

QuickJS is provided by `app.cash.zipline:zipline:1.27.0`. It runs on one worker,
with a 64 MiB heap and evaluation deadlines. Two standard ES2022 method shims
cover that binding's older runtime. There are no Java host bindings or remote
script loads. Gradle packages `../ios/Resources/pet-native.js` and its demo
directly; run `npm --prefix pet run sync` after changing the canonical core.

Five instrumentation tests exercise the real embedded engine, 4,800 shared
world frames and Kotlin digests, 3,000 additional checkpoint continuation
frames, version/import boundaries, sample-exact PCM, atomic storage/recovery,
and the Compose pause/still/audio/background lifecycle. The separate
`verify.sh` retains all 380 pure Kotlin conformance checkpoints.
