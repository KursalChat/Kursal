#!/bin/bash
source "$(dirname "$0")/build-common.sh"

BUILD="$ROOT/build"
fail=0
err() { echo "  ✗ $1" >&2; fail=1; }

echo "==> preflight for v$VERSION"

# Every artifact the release publishes
required=(
  Kursal_x64-setup.exe Kursal_x64-setup.exe.sig
  Kursal.dmg Kursal_x64.dmg
  Kursal.app.tar.gz Kursal.app.tar.gz.sig
  Kursal_x64.app.tar.gz Kursal_x64.app.tar.gz.sig
  Kursal_x64.AppImage Kursal_x64.AppImage.sig
  Kursal_x64.rpm Kursal_x64.rpm.sig
  Kursal_x64.deb Kursal_x64.deb.sig
  Kursal_arm.AppImage Kursal_arm.AppImage.sig
  Kursal_arm.rpm Kursal_arm.rpm.sig
  Kursal_arm.deb Kursal_arm.deb.sig
  Kursal.apk Kursal.ipa
  latest.json
)
for f in "${required[@]}"; do
  [ -f "$BUILD/$f" ] || err "missing: build/$f"
done

if [ -f "$BUILD/latest.json" ]; then
  mver="$(jq -r '.version' "$BUILD/latest.json")"
  [ "$mver" = "$VERSION" ] || err "latest.json version ($mver) != $VERSION (stale manifest? run: just gen-manifest)"

  if jq -e '[.platforms[] | (.signature // "-"), .sha256] | any(. == "")' "$BUILD/latest.json" >/dev/null; then
    err "latest.json has empty signature/sha256 fields"
  fi
fi

if [ "$fail" -ne 0 ]; then
  echo "✗ preflight failed — not ready to publish" >&2
  exit 1
fi
echo "✓ preflight passed"
