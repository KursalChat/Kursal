#!/bin/bash

source "$(dirname "$0")/build-common.sh"
cd "$ROOT/kursal-tauri"

echo "==> macos aarch64 (v$VERSION)"
rm -rf ../target/aarch64-apple-darwin/release/bundle/
cargo tauri build --bundles app,dmg,updater --target aarch64-apple-darwin --config '{"build":{"beforeBuildCommand":""}}'
cp ../target/aarch64-apple-darwin/release/bundle/dmg/Kursal_*.dmg            ../build/Kursal.dmg
cp ../target/aarch64-apple-darwin/release/bundle/macos/Kursal.app.tar.gz     ../build/Kursal.app.tar.gz
cp ../target/aarch64-apple-darwin/release/bundle/macos/Kursal.app.tar.gz.sig ../build/Kursal.app.tar.gz.sig

echo "==> macos x86_64 (v$VERSION)"
rm -rf ../target/x86_64-apple-darwin/release/bundle
cargo clean -p audiopus_sys --release --target x86_64-apple-darwin
OPUS_NO_PKG=1 OPUS_STATIC=1 OPUS_LIB_DIR="$ROOT/bin/deps/mac-x64" \
  cargo tauri build --bundles app,dmg,updater --target x86_64-apple-darwin --config '{"build":{"beforeBuildCommand":""}}'
cp ../target/x86_64-apple-darwin/release/bundle/dmg/Kursal_*.dmg            ../build/Kursal_x64.dmg
cp ../target/x86_64-apple-darwin/release/bundle/macos/Kursal.app.tar.gz     ../build/Kursal_x64.app.tar.gz
cp ../target/x86_64-apple-darwin/release/bundle/macos/Kursal.app.tar.gz.sig ../build/Kursal_x64.app.tar.gz.sig

echo "✓ macos done"
