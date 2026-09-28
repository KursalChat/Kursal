#!/bin/bash
set -eo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERSION="$("$ROOT/bin/version.sh")"
BUILD_NUMBER="$("$ROOT/bin/version-code.sh")"
SHORT="${VERSION%%-*}"

APPLE="$ROOT/kursal-tauri/src-tauri/gen/apple"

[ -d "$APPLE" ] || {
    echo "error: ios project missing, run 'tauri ios init'" >&2
    exit 1
}

set_key() {
    /usr/libexec/PlistBuddy -c "Set :$2 $3" "$1" 2>/dev/null ||
        /usr/libexec/PlistBuddy -c "Add :$2 string $3" "$1"
}

plist="$APPLE/KursalShare/Info.plist"
[ -f "$plist" ] || {
    echo "error: missing $plist" >&2
    exit 1
}
set_key "$plist" CFBundleShortVersionString "$SHORT"
set_key "$plist" CFBundleVersion "$BUILD_NUMBER"

echo "==> ios version $SHORT (build $BUILD_NUMBER)"