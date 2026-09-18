#!/bin/sh
set -eu
cd "$(dirname "$0")"
node ../scripts/build-pet.mjs --apple
xcodegen generate --spec project.yml
xcodebuild -project CodewhalePet.xcodeproj -scheme CodewhalePet -configuration Debug -sdk iphonesimulator -destination 'generic/platform=iOS Simulator' -derivedDataPath build -disableAutomaticPackageResolution CODE_SIGNING_ALLOWED=NO build
