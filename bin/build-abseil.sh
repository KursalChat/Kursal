#!/bin/bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

if [ -z "${FORCE:-}" ] \
  && [ -f "$ROOT/bin/deps/mac-arm64/abseil/lib/pkgconfig/absl_base.pc" ] \
  && [ -f "$ROOT/bin/deps/mac-x64/abseil/lib/pkgconfig/absl_base.pc" ]; then
  echo "abseil libs already present (FORCE=1 to rebuild); skipping"
  exit 0
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# To bump: set ABSEIL_VERSION, then get the new hash with
#   curl -fsSL https://github.com/abseil/abseil-cpp/releases/download/<version>/abseil-cpp-<version>.tar.gz | shasum -a 256
# Must stay >= 20240722: that is the minimum webrtc-audio-processing-2's meson.build accepts.
# Releases: https://github.com/abseil/abseil-cpp/releases
ABSEIL_VERSION="20240722.0"
ABSEIL_SHA256="f50e5ac311a81382da7fa75b97310e4b9006474f9560ac46f54a9967f07d4ae3"
ABSEIL_SRC="$WORK/abseil-cpp-$ABSEIL_VERSION"

echo "fetching abseil $ABSEIL_VERSION"
curl -fsSL -o "$WORK/abseil.tar.gz" \
  "https://github.com/abseil/abseil-cpp/releases/download/$ABSEIL_VERSION/abseil-cpp-$ABSEIL_VERSION.tar.gz"
echo "$ABSEIL_SHA256  $WORK/abseil.tar.gz" | shasum -a 256 -c - >/dev/null \
  || { echo "error: abseil tarball sha256 mismatch"; exit 1; }
tar xzf "$WORK/abseil.tar.gz" -C "$WORK"
echo "abseil source: $ABSEIL_SRC"

# ABSL_ENABLE_INSTALL also installs the absl_*.pc files that pkg-config needs.
# C++17 must match the standard webrtc-audio-processing-sys builds its wrapper with;
# abseil refuses to link against consumers built with a different one.
# The deployment target must match tauri.conf.json macOS minimumSystemVersion.
common=(
  -G Ninja
  -DCMAKE_POLICY_VERSION_MINIMUM=3.5
  -DCMAKE_BUILD_TYPE=Release
  -DBUILD_SHARED_LIBS=OFF
  -DBUILD_TESTING=OFF
  -DABSL_ENABLE_INSTALL=ON
  -DABSL_PROPAGATE_CXX_STD=ON
  -DCMAKE_CXX_STANDARD=17
  -DCMAKE_POSITION_INDEPENDENT_CODE=ON
  -DCMAKE_OSX_DEPLOYMENT_TARGET=14.0
)

# ── macOS arm64 ───────────────────────────────────────────────────────────────
echo "==> abseil aarch64-apple-darwin"
rm -rf "$ROOT/bin/deps/mac-arm64/abseil"
cmake -S "$ABSEIL_SRC" -B "$WORK/arm64" "${common[@]}" \
  -DCMAKE_OSX_ARCHITECTURES=arm64 \
  -DCMAKE_INSTALL_PREFIX="$ROOT/bin/deps/mac-arm64/abseil"
cmake --build "$WORK/arm64"
cmake --install "$WORK/arm64" >/dev/null

# ── macOS x86_64 ──────────────────────────────────────────────────────────────
echo "==> abseil x86_64-apple-darwin"
rm -rf "$ROOT/bin/deps/mac-x64/abseil"
cmake -S "$ABSEIL_SRC" -B "$WORK/x64" "${common[@]}" \
  -DCMAKE_OSX_ARCHITECTURES=x86_64 \
  -DCMAKE_INSTALL_PREFIX="$ROOT/bin/deps/mac-x64/abseil"
cmake --build "$WORK/x64"
cmake --install "$WORK/x64" >/dev/null

echo "✅ abseil libs built:"
echo "   bin/deps/mac-arm64/abseil"
echo "   bin/deps/mac-x64/abseil"
