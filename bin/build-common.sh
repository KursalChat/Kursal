#!/bin/bash
set -eo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

export TAURI_SIGNING_PRIVATE_KEY="$(cat "$ROOT/keys/publishing.key")"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(cat "$ROOT/keys/publishing.key.pwd")"

VERSION="$("$ROOT/bin/version.sh")"

mkdir -p "$ROOT/build"
