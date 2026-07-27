#!/bin/bash

source "$(dirname "$0")/build-common.sh"
cd "$ROOT/kursal-tauri"

echo "==> ios (v$VERSION)"

rm -f src-tauri/gen/apple/build/arm64/Kursal.ipa

cargo clean -p audiopus_sys --release --target aarch64-apple-ios
cargo tauri ios build --config '{"build":{"beforeBuildCommand":""}}'
cp src-tauri/gen/apple/build/arm64/Kursal.ipa ../build/Kursal.ipa

echo "✓ ios done"
