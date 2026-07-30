#!/bin/bash
# Shared helpers for the release orchestration scripts
set -eo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

die() { echo "✗ $*" >&2; exit 1; }

require_clean() {
  git diff --quiet && git diff --cached --quiet \
    || die "working tree not clean: commit or stash first"
}

confirm() {
  local ans
  read -r -p "  $1 [y/N] " ans </dev/tty || die "no terminal to confirm on"
  [[ "$ans" == [yY] ]] || die "aborted."
}

# open_pr <base> <head> <title> <body> - prints the PR url, reusing an open one
open_pr() {
  local base="$1" head="$2" title="$3" body="$4" url
  url="$(gh pr list --base "$base" --head "$head" --state open --json url --jq '.[0].url')"
  if [ -z "$url" ]; then
    url="$(gh pr create --base "$base" --head "$head" --title "$title" --body "$body")"
  fi
  printf '%s\n' "$url"
}

valid_semver() {
  [[ "$1" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$ ]] \
    || die "invalid version: $1 (expected e.g. 0.2.0 or 0.3.0-beta.1)"
}
