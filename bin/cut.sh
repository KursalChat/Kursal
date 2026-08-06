#!/bin/bash
# Prepare a stable or beta release: branch from dev, bump+tag, push. CI builds it.

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

confirm "push $BRANCH (with tag v$V)? this starts the release build on CI"
git push --follow-tags origin "$BRANCH"

echo
echo "✓ pushed v$V on $BRANCH. CI is building every platform."
echo "  watch it:  gh run watch"
echo "  then test the draft release, and:  just ship $V"
