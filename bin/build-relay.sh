#!/bin/bash
# Relay release build (linux amd64 + arm64).
# bin/build-relay.sh          -> tarballs in dist/ and build/
# bin/build-relay.sh --push   -> also push ghcr.io/kursalchat/relay:{VERSION,latest}

set -euo pipefail
cd "$(dirname "$0")/.."

VERSION="$(bin/version.sh)"
IMAGE="ghcr.io/kursalchat/relay"
PLATFORMS="linux/amd64,linux/arm64"
BUILDER="kursal-relay"

docker buildx inspect "$BUILDER" >/dev/null 2>&1 \
  || docker buildx create --name "$BUILDER" --driver docker-container

echo "==> building binaries ($PLATFORMS)"
rm -rf dist/raw
docker buildx build --builder "$BUILDER" --platform "$PLATFORMS" -f docker/relay/Dockerfile \
  --target export --output type=local,dest=dist/raw .

for dir in dist/raw/linux_*; do
  arch=${dir##*_}
  arch=${arch/amd64/x86_64}
  arch=${arch/arm64/aarch64}
  tarball="dist/kursal-relay-$VERSION-linux-$arch.tar.gz"
  tar -czf "$tarball" -C "$dir" kursal-relay
  shasum -a 256 "$tarball" | tee "$tarball.sha256"
done
rm -rf dist/raw

if [[ "${1:-}" == "--push" ]]; then
  case "$VERSION" in *-*) FLOATING=beta ;; *) FLOATING=latest ;; esac
  echo "==> pushing $IMAGE:{$VERSION,$FLOATING}"
  docker buildx build --builder "$BUILDER" --platform "$PLATFORMS" -f docker/relay/Dockerfile \
    -t "$IMAGE:$VERSION" -t "$IMAGE:$FLOATING" --push .
fi
