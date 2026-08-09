#!/bin/bash
set -eo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

export TAURI_SIGNING_PRIVATE_KEY="$(cat "$ROOT/keys/publishing.key")"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(cat "$ROOT/keys/publishing.key.pwd")"

VERSION="$("$ROOT/bin/version.sh")"

# @tauri-apps/cli ships a prebuilt binary and is pinned by the lockfile.
# cargo-tauri is the fallback for anyone who has not run `bun install`.
tauri() {
  if [ -x "$ROOT/kursal-tauri/node_modules/.bin/tauri" ]; then
    bun run --cwd "$ROOT/kursal-tauri" tauri "$@"
  else
    cargo tauri "$@"
  fi
}

mkdir -p "$ROOT/build"
