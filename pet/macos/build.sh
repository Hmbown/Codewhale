#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
pet_dir=$(pwd)
node "$pet_dir/scripts/build-pet.mjs" --apple
app_dir="macos/CodewhalePet.app"
mkdir -p "$app_dir/Contents/MacOS" "$app_dir/Contents/Resources"
swiftc -O -target "${PET_MACOS_ARCH:-$(uname -m)}-apple-macos14.0" -parse-as-library swift/PetSim.swift swift/PetNativeCore.swift swift/PetHabitatStore.swift swift/PetAudioOutput.swift swift/PetHost.swift swift/CodewhalePetView.swift swift/PetHabitatView.swift macos/WhalePoints.swift macos/CodewhalePetApp.swift -o "$app_dir/Contents/MacOS/CodewhalePet"
cp "$pet_dir/dist/pet-native.js" "$app_dir/Contents/Resources/pet-native.js"
cp ios/Resources/demo.jsonl "$app_dir/Contents/Resources/demo.jsonl"
cp public/whale.png "$app_dir/Contents/Resources/whale.png"
python3 - "$app_dir/Contents/Info.plist" <<'PY'
import plistlib,sys
with open(sys.argv[1],'wb') as f:
 plistlib.dump({'CFBundleIdentifier':'dev.shannonlabs.CodewhalePet','CFBundleName':'Codewhale Pet','CFBundleDisplayName':'Codewhale Pet','CFBundleExecutable':'CodewhalePet','CFBundlePackageType':'APPL','CFBundleIconFile':'whale.png','CFBundleShortVersionString':'0.1.0','CFBundleVersion':'1','LSMinimumSystemVersion':'14.0','LSUIElement':True,'NSHighResolutionCapable':True},f)
PY
codesign --force --sign - "$app_dir"
codesign --verify --strict "$app_dir"
printf 'Built and locally signed %s\n' "$app_dir"
