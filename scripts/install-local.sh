#!/bin/sh
# Build the working tree (committed or not) and install it for this user. No sudo, no commit.
# Linux: the binary goes to ~/.cargo/bin, plus a desktop entry and icons for the app launcher.
#   If the vtt-git package is installed it shadows this one: `sudo pacman -R vtt-git` once.
# macOS: builds vtt.app and copies it to /Applications (see scripts/package-macos.sh).
set -e
cd "$(dirname "$0")/.."

if [ "$(uname -s)" = Darwin ]; then
  exec scripts/package-macos.sh --install
fi

cargo install --path . --locked --force
data="${XDG_DATA_HOME:-$HOME/.local/share}"
install -Dm644 packaging/vtt.desktop "$data/applications/vtt.desktop"
install -Dm644 packaging/icons/vtt.svg "$data/icons/hicolor/scalable/apps/vtt.svg"
for s in 16 32 48 64 128 256 512; do
  install -Dm644 "packaging/icons/vtt-$s.png" "$data/icons/hicolor/${s}x${s}/apps/vtt.png"
done
gtk-update-icon-cache -q -t "$data/icons/hicolor" 2>/dev/null || true
echo "Installed $(command -v vtt || echo ~/.cargo/bin/vtt). Restart any running vtt."
