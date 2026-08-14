#!/bin/bash
set -e

VERSION=$1
SPEC=$2

if [ -z "$VERSION" ] || [ -z "$SPEC" ]; then
  echo "Usage: publish-api-docs.sh <version> <openapi_json_path>"
  exit 1
fi

echo "→ Generating ${SPEC}..."
mkdir -p "$(dirname "$SPEC")"
cargo run -q -p kursal-core --features apiserver --bin gen_api_docs -- --out "$SPEC"

SPEC_ABS="$(cd "$(dirname "$SPEC")" && pwd)/$(basename "$SPEC")"
REPO="$(git -C "$(dirname "$SPEC_ABS")" rev-parse --show-toplevel)"

if [ -z "$(git -C "$REPO" status --porcelain -- "$SPEC_ABS")" ]; then
  echo "✓ Spec unchanged; nothing to push"
  exit 0
fi

echo "→ Pushing website..."
git -C "$REPO" add -- "$SPEC_ABS"

git -C "$REPO" commit --only -m "chore: openapi spec v${VERSION}" -- "$SPEC_ABS"
git -C "$REPO" push

echo "✓ Done! API spec published for v${VERSION}"