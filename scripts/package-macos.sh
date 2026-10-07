#!/bin/sh
# Build vtt.app and a drag-to-Applications .dmg into dist/.
#
# The app is universal (Apple Silicon + Intel) when both Rust targets are installed:
#   rustup target add aarch64-apple-darwin x86_64-apple-darwin
# otherwise it is built for this Mac only.
#
# Usage: scripts/package-macos.sh [--install]
#   --install  also copy vtt.app to /Applications (replacing an older copy)
set -eu
cd "$(dirname "$0")/.."

install=false
[ "${1:-}" = "--install" ] && install=true

version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
export MACOSX_DEPLOYMENT_TARGET=11.0

# Build each installed Apple target, or the host target without rustup.
bins=""
installed=$(rustup target list --installed 2>/dev/null || true)
for target in aarch64-apple-darwin x86_64-apple-darwin; do
  if printf '%s\n' "$installed" | grep -qx "$target"; then
    cargo build --release --locked --target "$target"
    bins="$bins target/$target/release/vtt"
  fi
done
if [ -z "$bins" ]; then
  cargo build --release --locked
  bins="target/release/vtt"
fi
set -- $bins
if [ $# -gt 1 ]; then arch=universal; else arch=$(uname -m); fi

dist=dist
app="$dist/vtt.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"

# One binary holding every architecture that was built.
lipo -create -output "$app/Contents/MacOS/vtt" "$@"
sed "s/@VERSION@/$version/g" packaging/macos/Info.plist > "$app/Contents/Info.plist"

# Icon: an .iconset from the pre-rendered PNGs, compiled to .icns.
iconset=$(mktemp -d)/vtt.iconset
mkdir -p "$iconset"
icons=packaging/icons
for s in 16 32 128 256 512; do
  cp "$icons/vtt-$s.png" "$iconset/icon_${s}x${s}.png"
  cp "$icons/vtt-$((s * 2)).png" "$iconset/icon_${s}x${s}@2x.png"
done
iconutil -c icns -o "$app/Contents/Resources/vtt.icns" "$iconset"
rm -rf "$(dirname "$iconset")"

cp README.md LICENSE config.example.toml "$app/Contents/Resources/"

# Ad-hoc signature: required to run on Apple Silicon. (Not notarized: a downloaded copy
# needs right-click > Open the first time, or `xattr -dr com.apple.quarantine vtt.app`.)
codesign --force --deep --sign - "$app"

# Disk image with an Applications shortcut to drag the app onto.
staging=$(mktemp -d)
cp -R "$app" "$staging/"
ln -s /Applications "$staging/Applications"
dmg="$dist/vtt-$version-macos-$arch.dmg"
rm -f "$dmg"
hdiutil create -volname "vtt $version" -srcfolder "$staging" -ov -format UDZO "$dmg" >/dev/null
rm -rf "$staging"

# A zip of the app for those who'd rather not mount an image.
(cd "$dist" && rm -f "vtt-$version-macos-$arch.zip" && ditto -c -k --keepParent vtt.app "vtt-$version-macos-$arch.zip")

echo "Built $app, $dmg and $dist/vtt-$version-macos-$arch.zip"

if $install; then
  rm -rf /Applications/vtt.app
  cp -R "$app" /Applications/
  echo "Installed /Applications/vtt.app"
fi
