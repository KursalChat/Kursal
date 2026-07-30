#!/bin/bash
# Assemble build/latest.json + build/SHA256SUMS.txt

source "$(dirname "$0")/build-common.sh"
cd "$ROOT"

BUILD="$ROOT/build"
PUB_DATE="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
BASE_URL="https://app.kursal.chat/v/v$VERSION"
PUBKEY="$(jq -r '.plugins.updater.pubkey' kursal-tauri/src-tauri/tauri.conf.json)"

NOTES="$(git cliff --latest --strip all 2>/dev/null || true)"

need() { [ -f "$BUILD/$1" ] || { echo "gen-manifest: missing build/$1" >&2; exit 1; }; }
sig()  { need "$1"; need "$1.sig"; cat "$BUILD/$1.sig"; }
sha()  { need "$1"; shasum -a 256 "$BUILD/$1" | awk '{print $1}'; }

echo "==> generating latest.json for v$VERSION"

jq -n \
  --arg version "$VERSION" \
  --arg notes   "$NOTES" \
  --arg date    "$PUB_DATE" \
  --arg pubkey  "$PUBKEY" \
  --arg win_url "$BASE_URL/Kursal_x64-setup.exe"    --arg win_sig "$(sig Kursal_x64-setup.exe)"   --arg win_sha "$(sha Kursal_x64-setup.exe)" \
  --arg mac_a_url "$BASE_URL/Kursal.app.tar.gz"     --arg mac_a_sig "$(sig Kursal.app.tar.gz)"     --arg mac_a_sha "$(sha Kursal.app.tar.gz)" \
  --arg mac_x_url "$BASE_URL/Kursal_x64.app.tar.gz" --arg mac_x_sig "$(sig Kursal_x64.app.tar.gz)" --arg mac_x_sha "$(sha Kursal_x64.app.tar.gz)" \
  --arg la_url  "$BASE_URL/Kursal_arm.AppImage"     --arg la_sig  "$(sig Kursal_arm.AppImage)"    --arg la_sha  "$(sha Kursal_arm.AppImage)" \
  --arg lar_url "$BASE_URL/Kursal_arm.rpm"          --arg lar_sig "$(sig Kursal_arm.rpm)"         --arg lar_sha "$(sha Kursal_arm.rpm)" \
  --arg lad_url "$BASE_URL/Kursal_arm.deb"          --arg lad_sig "$(sig Kursal_arm.deb)"         --arg lad_sha "$(sha Kursal_arm.deb)" \
  --arg lx_url  "$BASE_URL/Kursal_x64.AppImage"     --arg lx_sig  "$(sig Kursal_x64.AppImage)"    --arg lx_sha  "$(sha Kursal_x64.AppImage)" \
  --arg lxr_url "$BASE_URL/Kursal_x64.rpm"          --arg lxr_sig "$(sig Kursal_x64.rpm)"         --arg lxr_sha "$(sha Kursal_x64.rpm)" \
  --arg lxd_url "$BASE_URL/Kursal_x64.deb"          --arg lxd_sig "$(sig Kursal_x64.deb)"         --arg lxd_sha "$(sha Kursal_x64.deb)" \
  --arg and_url "$BASE_URL/Kursal.apk"              --arg and_sha "$(sha Kursal.apk)" \
  --arg ios_url "$BASE_URL/Kursal.ipa"              --arg ios_sha "$(sha Kursal.ipa)" \
  '{
    version: $version,
    notes: $notes,
    pub_date: $date,
    pubkey: $pubkey,
    platforms: {
      "windows-x86_64":    { url: $win_url,   signature: $win_sig,   sha256: $win_sha },
      "darwin-aarch64":    { url: $mac_a_url, signature: $mac_a_sig, sha256: $mac_a_sha },
      "darwin-x86_64":     { url: $mac_x_url, signature: $mac_x_sig, sha256: $mac_x_sha },
      "linux-aarch64":     { url: $la_url,    signature: $la_sig,    sha256: $la_sha },
      "linux-aarch64-rpm": { url: $lar_url,   signature: $lar_sig,   sha256: $lar_sha },
      "linux-aarch64-deb": { url: $lad_url,   signature: $lad_sig,   sha256: $lad_sha },
      "linux-x86_64":      { url: $lx_url,    signature: $lx_sig,    sha256: $lx_sha },
      "linux-x86_64-rpm":  { url: $lxr_url,   signature: $lxr_sig,   sha256: $lxr_sha },
      "linux-x86_64-deb":  { url: $lxd_url,   signature: $lxd_sig,   sha256: $lxd_sha },
      "android":           { url: $and_url,   sha256: $and_sha },
      "ios":               { url: $ios_url,   sha256: $ios_sha }
    }
  }' > "$BUILD/latest.json"

# Published-artifact checksums, one file for the whole release (mirrors what the
# relay build already ships as .sha256 sidecars).
: > "$BUILD/SHA256SUMS.txt"
for f in \
  Kursal_x64-setup.exe Kursal_x64.exe \
  Kursal.dmg Kursal_x64.dmg \
  Kursal.app.tar.gz Kursal_x64.app.tar.gz \
  Kursal_x64.AppImage Kursal_x64.rpm Kursal_x64.deb \
  Kursal_arm.AppImage Kursal_arm.rpm Kursal_arm.deb \
  Kursal.apk
  # Kursal.ipa
  ; do
  [ -f "$BUILD/$f" ] && ( cd "$BUILD" && shasum -a 256 "$f" >> SHA256SUMS.txt )
done

echo "✓ latest.json + SHA256SUMS.txt written for v$VERSION"
if [ -z "$NOTES" ]; then
  echo "  note: git-cliff produced no release notes for this tag (latest.json notes are empty)"
fi
