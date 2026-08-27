#!/bin/bash

source "$(dirname "$0")/build-common.sh"
source "$(dirname "$0")/toolchain.sh" android
cd "$ROOT/kursal-tauri"

echo "==> android aab (v$VERSION)"

"$ROOT/bin/write-android-version.sh"

rm -rf src-tauri/gen/android/app/build/outputs/bundle/universalRelease/
rm -rf src-tauri/gen/android/app/src/main/jniLibs

JAVA_HOME="${JAVA_HOME:-/Library/Java/JavaVirtualMachines/zulu-17.jdk/Contents/Home}"
export JAVA_HOME
export PATH="$JAVA_HOME/bin:$PATH"

for pair in aarch64:aarch64-linux-android \
  armv7:armv7-linux-androideabi \
  i686:i686-linux-android \
  x86_64:x86_64-linux-android; do
  tauri_target="${pair%%:*}"
  rust_target="${pair##*:}"

  OPUS_LIB_DIR="$(opus_lib_dir_for "$rust_target")"
  export OPUS_LIB_DIR
  [ -f "$OPUS_LIB_DIR/libopus.a" ] || {
    echo "error: missing $OPUS_LIB_DIR/libopus.a (run bin/build-opus.sh)" >&2
    exit 1
  }

  cargo clean -p audiopus_sys --release --target "$rust_target"

  echo "==> android aab $tauri_target"
  tauri android build --aab --target "$tauri_target" \
    --config '{"build":{"beforeBuildCommand":""}}'
done

rm -rf src-tauri/gen/android/app/build/outputs/bundle/universalRelease/
(cd src-tauri/gen/android && ./gradlew bundleUniversalRelease -x rustBuildUniversalRelease)

cp src-tauri/gen/android/app/build/outputs/bundle/universalRelease/app-universal-release.aab ../build/Kursal.aab

echo "✓ android aab done"
