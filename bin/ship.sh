#!/bin/bash
# Publish a prepared release. Runs from the release/* or hotfix/* branch you cut.
source "$(dirname "$0")/release-lib.sh"

V="${1:?usage: ship.sh <version>}"
valid_semver "$V"
BASE="${V%%-*}"

BRANCH="$(git branch --show-current)"
[[ "$BRANCH" =~ ^(release|hotfix)/ ]] \
  || die "not on a release/* or hotfix/* branch (on: ${BRANCH:-detached HEAD})"

CARGO_V="$("$ROOT/bin/version.sh")"
[[ "$CARGO_V" == "$V" ]] \
  || die "version mismatch: asked to ship $V but Cargo.toml is $CARGO_V (run 'just release $V' first?)"

git rev-parse -q --verify "refs/tags/v$V" >/dev/null \
  || die "tag v$V not found (run 'just release $V' first?)"

require_clean
just preflight

# ── beta / prerelease: publish from the branch, main is untouched ─────────────
if [[ "$V" == *-* ]]; then
  echo "==> ship beta v$V from $BRANCH"
  confirm "push $BRANCH (with tag v$V) to origin?"
  git push --follow-tags origin "$BRANCH"
  confirm "publish-beta v$V (github pre-release + beta manifest slot)?"
  just publish-beta
  echo
  echo "✓ beta v$V shipped. cut more betas on $BRANCH, or finalize with 'just cut $BASE && just ship $BASE'."
  exit 0
fi

# ── stable: merge to main, publish, back-merge to dev, clean up ───────────────
echo "==> ship stable v$V ($BRANCH -> main)"
git checkout main
git pull --ff-only
git merge --no-ff "$BRANCH" -m "Merge $BRANCH into main"

echo "  merged $BRANCH into main locally (not pushed yet)."
confirm "push main + tags to origin? (N leaves the merge local, nothing published)"
git push --follow-tags origin main

echo "  main pushed."
confirm "publish v$V now (github + homebrew + ghcr relay + api docs)? (N = main is public but unpublished; run 'just publish' later)"
just publish

echo "==> back-merge to dev + clean up"
git checkout dev
git pull --ff-only
git merge --no-ff "$BRANCH" -m "Merge $BRANCH into dev"
git push origin dev

git branch -d "$BRANCH"
if git ls-remote --exit-code --heads origin "$BRANCH" >/dev/null 2>&1; then
  git push origin --delete "$BRANCH"
fi

echo
echo "✓ v$V released."
