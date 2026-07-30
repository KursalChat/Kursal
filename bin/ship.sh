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

# ── stable: PR into main, publish, back-merge PR into dev, clean up ───────────
echo "==> ship stable v$V ($BRANCH -> main)"

confirm "push $BRANCH (with tag v$V) to origin?"
git push --follow-tags origin "$BRANCH"

RELEASE_PR="$(open_pr main "$BRANCH" "release: v$V" "Release v$V. Opened by \`just ship\`.")"
echo "  release PR: $RELEASE_PR"

confirm "merge that PR into main? (N leaves it open, nothing published)"
gh pr merge "$RELEASE_PR" --merge

echo "  main updated."
confirm "publish v$V now (github + homebrew + ghcr relay + api docs)? (N = main is public but unpublished; run 'just publish' later)"
just publish

echo "==> back-merge main -> dev"
BACKMERGE_PR="$(open_pr dev main "chore: back-merge v$V into dev" "Back-merge of v$V. Opened by \`just ship\`.")"
echo "  back-merge PR: $BACKMERGE_PR"
gh pr merge "$BACKMERGE_PR" --merge

echo "==> clean up"
git fetch origin --prune
git checkout dev
git pull --ff-only origin dev
git branch -d "$BRANCH" 2>/dev/null || true

echo
echo "✓ v$V released."
