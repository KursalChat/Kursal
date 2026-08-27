#!/bin/bash

source "$(dirname "$0")/build-common.sh"
source "$(dirname "$0")/toolchain.sh" android
cd "$ROOT/kursal-tauri"

VERSION_CODE="$("$ROOT/bin/version-code.sh")"

echo "==> android aab (v$VERSION, code $VERSION_CODE)"

rm -rf src-tauri/gen/android/app/build/outputs/bundle/universalRelease/

for target in aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android; do
  cargo clean -p audiopus_sys --release --target "$target"
done

JAVA_HOME="${JAVA_HOME:-/Library/Java/JavaVirtualMachines/zulu-17.jdk/Contents/Home}"
export JAVA_HOME
export PATH="$JAVA_HOME/bin:$PATH"
tauri android build --aab --target aarch64 armv7 i686 x86_64 \
  --config "{\"build\":{\"beforeBuildCommand\":\"\"},\"bundle\":{\"android\":{\"versionCode\":$VERSION_CODE}}}"
cp src-tauri/gen/android/app/build/outputs/bundle/universalRelease/app-universal-release.aab ../build/Kursal.aab

echo "✓ android aab done"
