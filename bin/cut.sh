#!/bin/bash
# Prepare a stable or beta release: branch from dev, bump+tag, build.

source "$(dirname "$0")/release-lib.sh"

V="${1:?usage: cut.sh <version>}"
valid_semver "$V"
BASE="${V%%-*}"  # strip any -beta.N -> the release branch base
BRANCH="release/$BASE"

require_clean

if git show-ref --verify --quiet "refs/heads/$BRANCH"; then
  echo "==> continuing on existing $BRANCH"
  git checkout "$BRANCH"
else
  echo "==> branching $BRANCH from dev"
  git checkout dev
  git pull --ff-only origin dev
  git checkout -b "$BRANCH"
fi

just release "$V"  # verify (checks) + bump + changelog + commit + tag
just build

echo
echo "✓ built v$V on $BRANCH"
echo "  test the app, then:  just ship $V"
