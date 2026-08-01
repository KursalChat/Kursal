#!/bin/bash

source "$(dirname "$0")/build-common.sh"

build_linux() {
  local name="$1" platform="$2" suffix="$3"
  local container="kursal-linux-$name"

  docker volume create "kursal-cargo-registry-$name"
  docker volume create "kursal-cargo-target-$name"

  _run() {
    docker run \
      --name "$container" \
      --platform "$platform" \
      -v "$ROOT":/workspace \
      -v "kursal-cargo-registry-$name":/root/.cargo/registry \
      -v "kursal-cargo-target-$name":/root/kursal-target \
      -w /workspace/kursal-tauri \
      ubuntu:22.04 bash -c '
        if [ ! -f /root/.setup-done ]; then
          echo "==> Running first-time setup..." &&
          apt-get update && apt-get install -y \
            curl wget file unzip build-essential \
            libssl-dev pkg-config libasound2-dev \
            libgtk-3-dev libwebkit2gtk-4.1-dev \
            libayatana-appindicator3-dev librsvg2-dev \
            fuse libfuse2 squashfs-tools ninja-build python3-pip \
            protobuf-compiler xdg-utils libclang-dev clang \
            autoconf automake libtool libopus-dev cmake &&
          pip3 install --upgrade meson &&
          curl https://sh.rustup.rs -sSf | sh -s -- -y &&
          curl -fsSL https://bun.sh/install | bash &&
          touch /root/.setup-done &&
          echo "==> Setup complete!"
        else
          echo "==> Setup already done, skipping."
        fi &&
        source ~/.cargo/env &&
        export PATH="$HOME/.bun/bin:$PATH" &&
        bun install --frozen-lockfile &&
        rustup upgrade &&
        rm -rf /root/kursal-target/release/bundle/ &&
        export TAURI_SIGNING_PRIVATE_KEY=$(cat /workspace/keys/publishing.key) &&
        export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=$(cat /workspace/keys/publishing.key.pwd) &&
        export CARGO_TARGET_DIR=/root/kursal-target &&
        export APPIMAGE_EXTRACT_AND_RUN=1 &&
        bun run tauri build --bundles deb,rpm,appimage,updater --config '"'"'{"build":{"beforeBuildCommand":""}}'"'"' &&
        cp /root/kursal-target/release/bundle/deb/*.deb                /workspace/build/Kursal_$1.deb &&
        cp /root/kursal-target/release/bundle/deb/*.deb.sig            /workspace/build/Kursal_$1.deb.sig &&
        cp /root/kursal-target/release/bundle/rpm/*.rpm                /workspace/build/Kursal_$1.rpm &&
        cp /root/kursal-target/release/bundle/rpm/*.rpm.sig            /workspace/build/Kursal_$1.rpm.sig &&
        cp /root/kursal-target/release/bundle/appimage/*.AppImage      /workspace/build/Kursal_$1.AppImage &&
        cp /root/kursal-target/release/bundle/appimage/*.AppImage.sig  /workspace/build/Kursal_$1.AppImage.sig
      ' bash "$suffix"
  }

  if docker container inspect "$container" &>/dev/null; then
    docker start -ai "$container"
  else
    _run
  fi
  docker container stop "$container"
}

echo "==> linux (v$VERSION)"
orb start || echo "orb start failed, continuing anyway"

build_linux x64 linux/amd64 x64
build_linux arm64 linux/arm64 arm

echo "✓ linux done"
