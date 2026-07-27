#!/bin/bash

source "$(dirname "$0")/build-common.sh"
cd "$ROOT/kursal-tauri"

echo "==> windows x86_64 (v$VERSION)"

rm -f ../target/x86_64-pc-windows-msvc/release/kursal-app.exe
rm -rf ../target/x86_64-pc-windows-msvc/release/bundle/

cargo clean -p audiopus_sys --release --target x86_64-pc-windows-msvc
OPUS_NO_PKG=1 OPUS_STATIC=1 OPUS_LIB_DIR="$ROOT/bin/deps/win" \
  cargo tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc --config '{"build":{"beforeBuildCommand":""}}'

cp ../target/x86_64-pc-windows-msvc/release/kursal-app.exe                         ../build/Kursal_x64.exe
cp ../target/x86_64-pc-windows-msvc/release/bundle/nsis/Kursal_*_x64-setup.exe     ../build/Kursal_x64-setup.exe
cp ../target/x86_64-pc-windows-msvc/release/bundle/nsis/Kursal_*_x64-setup.exe.sig ../build/Kursal_x64-setup.exe.sig

upx --best --lzma ../build/Kursal_x64.exe

echo "✓ windows x86_64 done"
