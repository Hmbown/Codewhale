#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
./macos/build.sh
mkdir -p conformance-results
node scripts/long-checkpoint.mjs
swiftc -O -parse-as-library swift/PetSim.swift swift/PetNativeCore.swift swift/PetHabitatStore.swift swift/PetAudioOutput.swift swift/PetHost.swift tests/apple-checkpoint.swift -o conformance-results/apple-checkpoint
./conformance-results/apple-checkpoint "$(pwd)" "$(pwd)/conformance-results"
