#!/bin/bash
set -eo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERSION="$("$ROOT/bin/version.sh")"

core="${VERSION%%-*}"
pre="${VERSION#"$core"}"
IFS=. read -r major minor patch <<<"$core"

case "$pre" in
"")
    # 999 keeps a final release above every beta of the same version.
    num=999
    ;;
-beta) num=0 ;;
-beta.*[!0-9]*) num="" ;;
-beta.*) num="${pre#-beta.}" ;;
*) num="" ;;
esac

if [ -z "$num" ]; then
    echo "error: unsupported prerelease in $VERSION" >&2
    exit 1
fi

if [ "$minor" -ge 100 ] || [ "$patch" -ge 100 ]; then
    echo "error: $VERSION does not fit the versionCode scheme" >&2
    exit 1
fi

echo "$((major * 10000000 + minor * 100000 + patch * 1000 + num))"
