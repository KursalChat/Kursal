#!/bin/bash

source "$(dirname "$0")/build-common.sh"
source "$(dirname "$0")/toolchain.sh" android
cd "$ROOT/kursal-tauri"

echo "==> android (v$VERSION)"

rm -rf src-tauri/gen/android/app/build/outputs/apk/universal/release/

cargo clean -p audiopus_sys --release --target aarch64-linux-android
JAVA_HOME=/Library/Java/JavaVirtualMachines/zulu-17.jdk/Contents/Home PATH=$JAVA_HOME/bin:$PATH cargo tauri android build --apk --target aarch64 --config '{"build":{"beforeBuildCommand":""}}'
cp -r src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk ../build/Kursal.apk

echo "✓ android done"
