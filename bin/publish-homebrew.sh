#!/bin/bash
set -e

VERSION=$1
CASK_FILE=$2

if [ -z "$VERSION" ] || [ -z "$CASK_FILE" ]; then
  echo "Usage: publish-homebrew.sh <version> <cask_file>"
  exit 1
fi

echo "→ Computing checksums..."

if [ ! -f "./build/Kursal.dmg" ]; then
  echo "Error: ./build/Kursal.dmg not found"
  exit 1
fi

if [ ! -f "./build/Kursal_x64.dmg" ]; then
  echo "Error: ./build/Kursal_x64.dmg not found"
  exit 1
fi

ARM_SHA=$(shasum -a 256 "./build/Kursal.dmg"       | awk '{print $1}')
INTEL_SHA=$(shasum -a 256 "./build/Kursal_x64.dmg" | awk '{print $1}')

echo "  ARM:   ${ARM_SHA}"
echo "  Intel: ${INTEL_SHA}"

echo "→ Writing ${CASK_FILE}..."
cat > "${CASK_FILE}" <<EOF
cask "kursal" do
  version "${VERSION}"

  on_arm do
    url "https://app.kursal.chat/v/v#{version}/Kursal.dmg"
    sha256 "${ARM_SHA}"
  end

  on_intel do
    url "https://app.kursal.chat/v/v#{version}/Kursal_x64.dmg"
    sha256 "${INTEL_SHA}"
  end

  name "Kursal"
  desc "Peer-to-peer, end-to-end encrypted messaging that puts you in control. No servers. No tracking. Just your private conversations."
  homepage "https://kursal.chat"

  auto_updates true

  app "Kursal.app"

  postflight do
    system_command "/usr/bin/xattr",
      args: ["-dr", "com.apple.quarantine", "#{appdir}/Kursal.app"],
      sudo: false
  end

  binary "#{appdir}/Kursal.app/Contents/MacOS/kursal", target: "kursal"
end
EOF

CASK_ABS="$(cd "$(dirname "$CASK_FILE")" && pwd)/$(basename "$CASK_FILE")"
CASK_REPO=$(dirname "$(dirname "$CASK_ABS")")

git -C "$CASK_REPO" add "$CASK_ABS"
if git -C "$CASK_REPO" diff --cached --quiet; then
  echo "✓ Cask unchanged; nothing to push"
else
  echo "→ Pushing homebrew tap..."
  git -C "$CASK_REPO" commit -m "kursal ${VERSION}"
  git -C "$CASK_REPO" push
fi

echo "✓ Done! Cask updated to v${VERSION}"