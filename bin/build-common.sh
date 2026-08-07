#!/bin/bash
set -eo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

export TAURI_SIGNING_PRIVATE_KEY="$(cat "$ROOT/keys/publishing.key")"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(cat "$ROOT/keys/publishing.key.pwd")"

VERSION="$("$ROOT/bin/version.sh")"

# @tauri-apps/cli ships a prebuilt binary and is pinned by the lockfile.
# cargo-tauri is the fallback for anyone who has not run `bun install`.
tauri() {
  local tauri_cli="$ROOT/kursal-tauri/node_modules/.bin/tauri"
  if [ -x "$tauri_cli" ]; then
    "$tauri_cli" "$@"
  else
    cargo tauri "$@"
  fi
}

mkdir -p "$ROOT/build"
