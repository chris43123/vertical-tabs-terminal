#!/bin/sh
# Build the Linux release packages into dist/:
#   vtt-<version>-linux-<arch>.tar.gz   binary, desktop entry, icons, docs
#   vtt-<version>-<arch>.AppImage       single-file app (appimagetool is downloaded if missing)
#   vtt_<version>-1_<debarch>.deb       with cargo-deb installed
#   vtt-<version>-1.<arch>.rpm          with cargo-generate-rpm installed
#
# Usage: scripts/package-linux.sh
set -eu
cd "$(dirname "$0")/.."

version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
arch=$(uname -m)
dist=dist
mkdir -p "$dist"

cargo build --release --locked
bin=target/release/vtt

# Install the app's files under a prefix ($1), laid out like /usr.
install_tree() {
  install -Dm755 "$bin" "$1/bin/vtt"
  install -Dm644 packaging/vtt.desktop "$1/share/applications/vtt.desktop"
  install -Dm644 packaging/icons/vtt.svg "$1/share/icons/hicolor/scalable/apps/vtt.svg"
  for s in 16 32 48 64 128 256 512; do
    install -Dm644 "packaging/icons/vtt-$s.png" "$1/share/icons/hicolor/${s}x${s}/apps/vtt.png"
  done
  install -Dm644 README.md "$1/share/doc/vtt/README.md"
  install -Dm644 config.example.toml "$1/share/doc/vtt/config.example.toml"
  install -Dm644 LICENSE "$1/share/licenses/vtt/LICENSE"
}

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# Tarball: extract and copy (or symlink) the tree into ~/.local or /usr/local.
name="vtt-$version-linux-$arch"
install_tree "$work/$name"
tar -C "$work" -czf "$dist/$name.tar.gz" "$name"
echo "Built $dist/$name.tar.gz"

# AppImage. The windowing and GPU libraries come from the host, like other terminals'
# AppImages; the binary itself only links glibc.
appdir="$work/vtt.AppDir"
install_tree "$appdir/usr"
cp packaging/vtt.desktop "$appdir/vtt.desktop"
cp packaging/icons/vtt-256.png "$appdir/vtt.png"
ln -s vtt.png "$appdir/.DirIcon"
ln -s usr/bin/vtt "$appdir/AppRun"
tool=${APPIMAGETOOL:-}
if [ -z "$tool" ]; then
  tool=$(command -v appimagetool || true)
fi
if [ -z "$tool" ]; then
  tool=target/appimagetool-$arch.AppImage
  if [ ! -x "$tool" ]; then
    curl -fsSL -o "$tool" \
      "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-$arch.AppImage"
    chmod +x "$tool"
  fi
fi
# Without FUSE (containers, CI), appimagetool can still run by extracting itself.
APPIMAGE_EXTRACT_AND_RUN=1 ARCH=$arch "$tool" --no-appstream "$appdir" "$dist/vtt-$version-$arch.AppImage"
echo "Built $dist/vtt-$version-$arch.AppImage"

if cargo deb --version >/dev/null 2>&1; then
  cargo deb --no-build --output "$dist/"
else
  echo "Skipping .deb: cargo install cargo-deb"
fi

if cargo generate-rpm --version >/dev/null 2>&1; then
  cargo generate-rpm --output "$dist/"
else
  echo "Skipping .rpm: cargo install cargo-generate-rpm"
fi
