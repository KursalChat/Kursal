#!/bin/bash

source "$(dirname "$0")/build-common.sh"
source "$(dirname "$0")/toolchain.sh" android
cd "$ROOT/kursal-tauri"

echo "==> android (v$VERSION)"

"$ROOT/bin/write-android-version.sh"

rm -rf src-tauri/gen/android/app/build/outputs/apk/universal/release/
# Packaging takes whatever sits in jniLibs, and this builds aarch64 only.
rm -rf src-tauri/gen/android/app/src/main/jniLibs

JAVA_HOME="${JAVA_HOME:-/Library/Java/JavaVirtualMachines/zulu-17.jdk/Contents/Home}"
export JAVA_HOME
export PATH="$JAVA_HOME/bin:$PATH"

cargo clean -p audiopus_sys --release --target aarch64-linux-android
tauri android build --apk --target aarch64 --config '{"build":{"beforeBuildCommand":""}}'
cp -r src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk ../build/Kursal.apk
sign_artifact ../build/Kursal.apk

echo "✓ android done"
