#!/bin/bash
set -eo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

awk '
  /^\[/ { in_wp = ($0 == "[workspace.package]") }
  in_wp && /^[[:space:]]*version[[:space:]]*=/ {
    gsub(/^[[:space:]]*version[[:space:]]*=[[:space:]]*"/, "")
    gsub(/".*$/, "")
    print
    exit
  }
' "$ROOT/Cargo.toml"
