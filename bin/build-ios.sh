#!/bin/bash

source "$(dirname "$0")/build-common.sh"
cd "$ROOT/kursal-tauri"

echo "==> ios (v$VERSION)"

"$ROOT/bin/write-ios-version.sh"

SHORT="${VERSION%%-*}"
BUILD_NUMBER="$("$ROOT/bin/version-code.sh")"

rm -f src-tauri/gen/apple/build/arm64/Kursal.ipa

cargo clean -p audiopus_sys --release --target aarch64-apple-ios
tauri ios build --archive-only \
  --config "{\"version\":\"$SHORT\",\"bundle\":{\"iOS\":{\"bundleVersion\":\"$BUILD_NUMBER\"}},\"build\":{\"beforeBuildCommand\":\"\"}}"

xcodebuild -exportArchive \
  -archivePath src-tauri/gen/apple/build/kursal-app_iOS.xcarchive \
  -exportOptionsPlist "$ROOT/bin/ios-export-options.plist" \
  -exportPath src-tauri/gen/apple/build/arm64

cp src-tauri/gen/apple/build/arm64/Kursal.ipa ../build/Kursal.ipa
sign_artifact ../build/Kursal.ipa

echo "✓ ios done"
