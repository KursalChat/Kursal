#!/bin/bash
# Start a hotfix branch off main.

source "$(dirname "$0")/release-lib.sh"

V="${1:?usage: cut-hotfix.sh <version>}"
valid_semver "$V"
BRANCH="hotfix/$V"

require_clean

echo "==> branching $BRANCH from main"
git checkout main
git pull --ff-only
git checkout -b "$BRANCH"

echo
echo "✓ on $BRANCH. now:"
echo "  1. make the fix + commit it"
echo "  2. just release $V     (verify + bump + tag)"
echo "  3. just build"
echo "  4. just ship $V"
