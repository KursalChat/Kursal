#!/bin/bash
# Relay release build (linux amd64 + arm64).
# bin/build-relay.sh                             -> both arches, tarballs in dist/
# bin/build-relay.sh --push                      -> also push :{VERSION,beta|latest}
# bin/build-relay.sh --platform linux/amd64      -> one arch only, no emulation
# bin/build-relay.sh --platform linux/amd64 --push
#                                                -> also push :VERSION-amd64
# bin/build-relay.sh --merge                     -> join the per-arch tags into
#                                                   :{VERSION,beta|latest}

set -euo pipefail
cd "$(dirname "$0")/.."

VERSION="$(bin/version.sh)"
IMAGE="ghcr.io/kursalchat/relay"
BUILDER="kursal-relay"
PLATFORMS="linux/amd64,linux/arm64"
PUSH=0
MERGE=0

while [ $# -gt 0 ]; do
  case "$1" in
  --platform)
    PLATFORMS="${2:?--platform needs a value}"
    shift 2
    ;;
  --push)
    PUSH=1
    shift
    ;;
  --merge)
    MERGE=1
    shift
    ;;
  *)
    echo "unknown argument: $1" >&2
    exit 1
    ;;
  esac
done

case "$VERSION" in
*-*) FLOATING=beta ;;
*) FLOATING=latest ;;
esac

if [ "$MERGE" = 1 ]; then
  echo "==> merging $IMAGE:$VERSION-{amd64,arm64} into :{$VERSION,$FLOATING}"
  docker buildx imagetools create \
    -t "$IMAGE:$VERSION" -t "$IMAGE:$FLOATING" \
    "$IMAGE:$VERSION-amd64" "$IMAGE:$VERSION-arm64"
  exit 0
fi

docker buildx inspect "$BUILDER" >/dev/null 2>&1 \
  || docker buildx create --name "$BUILDER" --driver docker-container

echo "==> building binaries ($PLATFORMS)"
rm -rf dist/raw
docker buildx build --builder "$BUILDER" --platform "$PLATFORMS" -f docker/relay/Dockerfile \
  --target export --output type=local,dest=dist/raw .

# Normalise to the nested layout.
if [ "$PLATFORMS" = "${PLATFORMS#*,}" ]; then
  mkdir -p "dist/raw/linux_${PLATFORMS##*/}"
  mv dist/raw/kursal-relay "dist/raw/linux_${PLATFORMS##*/}/"
fi

for dir in dist/raw/linux_*; do
  arch=${dir##*_}
  arch=${arch/amd64/x86_64}
  arch=${arch/arm64/aarch64}
  tarball="dist/kursal-relay-linux-$arch.tar.gz"
  tar -czf "$tarball" -C "$dir" kursal-relay
  shasum -a 256 "$tarball" | tee "$tarball.sha256"
done
rm -rf dist/raw

if [ "$PUSH" = 1 ]; then
  if [ "$PLATFORMS" = "${PLATFORMS#*,}" ]; then
    TAGS=(-t "$IMAGE:$VERSION-${PLATFORMS##*/}")
  else
    TAGS=(-t "$IMAGE:$VERSION" -t "$IMAGE:$FLOATING")
  fi
  echo "==> pushing ${TAGS[*]}"
  docker buildx build --builder "$BUILDER" --platform "$PLATFORMS" -f docker/relay/Dockerfile \
    "${TAGS[@]}" --push .
fi