#!/bin/bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# The vendored libs are gitignored but persist on disk between builds, so skip
# the (slow) rebuild when they are already present. FORCE=1 to rebuild anyway.
if [ -z "${FORCE:-}" ] \
  && [ -f "$ROOT/bin/deps/mac-x64/libopus.a" ] \
  && [ -f "$ROOT/bin/deps/win/opus.lib" ] \
  && [ -f "$ROOT/bin/deps/android-arm64/libopus.a" ] \
  && [ -f "$ROOT/bin/deps/ios-arm64/libopus.a" ]; then
  echo "opus libs already present (FORCE=1 to rebuild); skipping"
  exit 0
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# To bump: set OPUS_VERSION, then get the new hash with
#   curl -fsSL https://downloads.xiph.org/releases/opus/opus-<version>.tar.gz | shasum -a 256
# and paste it into OPUS_SHA256. Available versions: https://downloads.xiph.org/releases/opus/
OPUS_VERSION="1.4"
OPUS_SHA256="c9b32b4253be5ae63d1ff16eea06b94b5f0f2951b7a02aceef58e3a3ce49c51f"
OPUS_SRC="$WORK/opus-$OPUS_VERSION"

echo "fetching opus $OPUS_VERSION"
curl -fsSL -o "$WORK/opus.tar.gz" "https://downloads.xiph.org/releases/opus/opus-$OPUS_VERSION.tar.gz"
echo "$OPUS_SHA256  $WORK/opus.tar.gz" | shasum -a 256 -c - >/dev/null \
  || { echo "error: opus tarball sha256 mismatch"; exit 1; }
tar xzf "$WORK/opus.tar.gz" -C "$WORK"
echo "opus source: $OPUS_SRC"

common=(
  -G Ninja
  -DCMAKE_POLICY_VERSION_MINIMUM=3.5
  -DCMAKE_BUILD_TYPE=Release
  -DBUILD_SHARED_LIBS=OFF
  -DOPUS_BUILD_SHARED_LIBRARY=OFF
  -DOPUS_BUILD_TESTING=OFF
  -DOPUS_BUILD_PROGRAMS=OFF
)

# ── macOS x86_64 ──────────────────────────────────────────────────────────────
echo "==> opus x86_64-apple-darwin"
cmake -S "$OPUS_SRC" -B "$WORK/mac" "${common[@]}" \
  -DCMAKE_OSX_ARCHITECTURES=x86_64 \
  -DCMAKE_C_FLAGS="-fPIC"
cmake --build "$WORK/mac" --target opus
mkdir -p "$ROOT/bin/deps/mac-x64"
cp "$WORK/mac/libopus.a" "$ROOT/bin/deps/mac-x64/libopus.a"

# ── windows x86_64 (msvc) ─────────────────────────────────────────────────────
echo "==> opus x86_64-pc-windows-msvc"
XWIN="$HOME/Library/Caches/cargo-xwin/xwin"
if [ ! -d "$XWIN/crt/include" ]; then
  echo "xwin sdk missing at $XWIN; fetching (large download)"
  xwin --accept-license splat --output "$XWIN"
fi

# clang-cl mimics MSVC, so opus skips the per-file -msse4.1/-mavx flags those
# code paths need; disable them and let runtime CPU detection fall back to C.
win_inc="/imsvc $XWIN/crt/include /imsvc $XWIN/sdk/include/ucrt /imsvc $XWIN/sdk/include/um /imsvc $XWIN/sdk/include/shared"
cmake -S "$OPUS_SRC" -B "$WORK/win" "${common[@]}" \
  -DCMAKE_SYSTEM_NAME=Windows \
  -DCMAKE_SYSTEM_PROCESSOR=AMD64 \
  -DCMAKE_C_COMPILER=clang-cl \
  -DCMAKE_C_COMPILER_TARGET=x86_64-pc-windows-msvc \
  -DCMAKE_AR="$(command -v llvm-lib)" \
  -DCMAKE_C_FLAGS="--target=x86_64-pc-windows-msvc $win_inc" \
  -DCMAKE_TRY_COMPILE_TARGET_TYPE=STATIC_LIBRARY \
  -DOPUS_X86_MAY_HAVE_SSE4_1=OFF \
  -DOPUS_X86_MAY_HAVE_AVX=OFF
cmake --build "$WORK/win" --target opus
mkdir -p "$ROOT/bin/deps/win"
cp "$WORK/win/opus.lib" "$ROOT/bin/deps/win/opus.lib"

# ── android arm64 ─────────────────────────────────────────────────────────────
echo "==> opus aarch64-linux-android"
ANDROID_HOME="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
NDK="${NDK_HOME:-${ANDROID_NDK_HOME:-$(ls -d "$ANDROID_HOME"/ndk/* 2>/dev/null | sort -V | tail -1)}}"
[ -f "$NDK/build/cmake/android.toolchain.cmake" ] || { echo "error: android NDK not found (set ANDROID_NDK_HOME)"; exit 1; }
# API 26 = minSdk for AAudio; the toolchain file picks the correct host prebuilt.
cmake -S "$OPUS_SRC" -B "$WORK/android" "${common[@]}" \
  -DCMAKE_TOOLCHAIN_FILE="$NDK/build/cmake/android.toolchain.cmake" \
  -DANDROID_ABI=arm64-v8a \
  -DANDROID_PLATFORM=android-26 \
  -DCMAKE_C_FLAGS="-fPIC"
cmake --build "$WORK/android" --target opus
mkdir -p "$ROOT/bin/deps/android-arm64"
cp "$WORK/android/libopus.a" "$ROOT/bin/deps/android-arm64/libopus.a"

# ── ios arm64 ─────────────────────────────────────────────────────────────────
echo "==> opus aarch64-apple-ios"
# Deployment target must match tauri.conf.json iOS minimumSystemVersion.
cmake -S "$OPUS_SRC" -B "$WORK/ios" "${common[@]}" \
  -DCMAKE_SYSTEM_NAME=iOS \
  -DCMAKE_OSX_ARCHITECTURES=arm64 \
  -DCMAKE_OSX_SYSROOT=iphoneos \
  -DCMAKE_OSX_DEPLOYMENT_TARGET=15.6 \
  -DCMAKE_C_FLAGS="-fPIC"
cmake --build "$WORK/ios" --target opus
mkdir -p "$ROOT/bin/deps/ios-arm64"
cp "$WORK/ios/libopus.a" "$ROOT/bin/deps/ios-arm64/libopus.a"

echo "✅ opus libs built:"
echo "   bin/deps/mac-x64/libopus.a"
echo "   bin/deps/win/opus.lib"
echo "   bin/deps/android-arm64/libopus.a"
echo "   bin/deps/ios-arm64/libopus.a"
