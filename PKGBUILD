pkgname=kursal
pkgver="$(sed -n 's/^version = "\(.*\)"/\1/p' "$startdir/Cargo.toml" | head -n1 | tr '-' '_')"
pkgrel=1
pkgdesc='Peer-to-peer, end-to-end encrypted messaging that puts you in control'
arch=('x86_64' 'aarch64')
url='https://kursal.chat'
license=('AGPL-3.0-only')
depends=(
  'alsa-lib'
  'gtk3'
  'hicolor-icon-theme'
  'libayatana-appindicator'
  'librsvg'
  'openssl'
  'opus'
  'webkit2gtk-4.1'
  'xdg-utils'
)
makedepends=(
  'autoconf'
  'automake'
  'bun'
  'clang'
  'cmake'
  'git'
  'libtool'
  'meson'
  'ninja'
  'pkgconf'
  'protobuf'
  'rustup'
)
optdepends=(
  'gnome-keyring: secret storage'
  'keepassxc: secret storage'
  'kwallet: secret storage'
)
options=('!debug' '!lto')

prepare() {
  rustup toolchain install stable --profile minimal --component llvm-tools-preview
  export RUSTUP_TOOLCHAIN=stable
  export CARGO_TARGET_DIR="$srcdir/target"

  cd "$startdir"
  cargo fetch --locked --target "$CARCH-unknown-linux-gnu"

  cd kursal-tauri
  bun install --frozen-lockfile
}

build() {
  export RUSTUP_TOOLCHAIN=stable
  export CARGO_TARGET_DIR="$srcdir/target"
  export CFLAGS="${CFLAGS//-O2/}"
  export CXXFLAGS="${CXXFLAGS//-O2/}"

  cd "$startdir/kursal-tauri"
  bun run tauri build --no-bundle
}

package() {
  local icons="$startdir/kursal-tauri/src-tauri/icons"

  install -Dm755 "$srcdir/target/release/kursal" \
    "$pkgdir/usr/bin/kursal"
  install -Dm644 "$icons/32x32.png" \
    "$pkgdir/usr/share/icons/hicolor/32x32/apps/kursal.png"
  install -Dm644 "$icons/128x128.png" \
    "$pkgdir/usr/share/icons/hicolor/128x128/apps/kursal.png"
  install -Dm644 "$icons/128x128@2x.png" \
    "$pkgdir/usr/share/icons/hicolor/256x256@2/apps/kursal.png"

  install -Dm644 /dev/stdin "$pkgdir/usr/share/applications/kursal.desktop" <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=Kursal
Comment=Peer-to-peer, end-to-end encrypted messaging
Exec=kursal %U
Icon=kursal
Terminal=false
Categories=Network;InstantMessaging;Chat;
MimeType=x-scheme-handler/kursal;
StartupWMClass=kursal
DESKTOP

  install -Dm644 "$startdir/LICENSE" \
    "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
}
