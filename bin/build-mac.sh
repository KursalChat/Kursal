#!/bin/bash

source "$(dirname "$0")/build-common.sh"
cd "$ROOT/kursal-tauri"

# Must match bin/build-abseil.sh and tauri.conf.json macOS minimumSystemVersion.
export MACOSX_DEPLOYMENT_TARGET=14.0

check_no_homebrew() {
  if otool -L "$1" | grep -q /opt/homebrew; then
    echo "error: $1 links Homebrew dylibs:"
    otool -L "$1" | grep /opt/homebrew
    echo "hint: 'cargo clean -p webrtc-audio-processing-sys --release' and rebuild"
    exit 1
  fi
}

echo "==> macos aarch64 (v$VERSION)"
rm -rf ../target/aarch64-apple-darwin/release/bundle/
cargo clean -p webrtc-audio-processing-sys --release --target aarch64-apple-darwin
PKG_CONFIG_PATH="$ROOT/bin/deps/mac-arm64/abseil/lib/pkgconfig" \
  cargo tauri build --bundles app,dmg,updater --target aarch64-apple-darwin --config '{"build":{"beforeBuildCommand":""}}'
check_no_homebrew ../target/aarch64-apple-darwin/release/bundle/macos/Kursal.app/Contents/MacOS/kursal
cp ../target/aarch64-apple-darwin/release/bundle/dmg/Kursal_*.dmg            ../build/Kursal.dmg
cp ../target/aarch64-apple-darwin/release/bundle/macos/Kursal.app.tar.gz     ../build/Kursal.app.tar.gz
cp ../target/aarch64-apple-darwin/release/bundle/macos/Kursal.app.tar.gz.sig ../build/Kursal.app.tar.gz.sig

# Both builds exceed the 14 GB CI runner disk. Clean up on CI
if [ -n "${CI:-}" ]; then
  rm -rf ../target/aarch64-apple-darwin
fi

echo "==> macos x86_64 (v$VERSION)"
rm -rf ../target/x86_64-apple-darwin/release/bundle
cargo clean -p audiopus_sys --release --target x86_64-apple-darwin
cargo clean -p webrtc-audio-processing-sys --release --target x86_64-apple-darwin
PKG_CONFIG_PATH="$ROOT/bin/deps/mac-x64/abseil/lib/pkgconfig" \
OPUS_NO_PKG=1 OPUS_STATIC=1 OPUS_LIB_DIR="$ROOT/bin/deps/mac-x64" \
  cargo tauri build --bundles app,dmg,updater --target x86_64-apple-darwin --config '{"build":{"beforeBuildCommand":""}}'
check_no_homebrew ../target/x86_64-apple-darwin/release/bundle/macos/Kursal.app/Contents/MacOS/kursal
cp ../target/x86_64-apple-darwin/release/bundle/dmg/Kursal_*.dmg            ../build/Kursal_x64.dmg
cp ../target/x86_64-apple-darwin/release/bundle/macos/Kursal.app.tar.gz     ../build/Kursal_x64.app.tar.gz
cp ../target/x86_64-apple-darwin/release/bundle/macos/Kursal.app.tar.gz.sig ../build/Kursal_x64.app.tar.gz.sig

echo "✓ macos done"
