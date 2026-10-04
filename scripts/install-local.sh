#!/bin/sh
# Build the working tree (committed or not) and install it to ~/.cargo/bin. No sudo, no commit.
# If the vtt-git package is installed it shadows this one: `sudo pacman -R vtt-git` once.
set -e
cd "$(dirname "$0")/.."
cargo install --path . --locked --force
install -Dm644 packaging/vtt.desktop "${XDG_DATA_HOME:-$HOME/.local/share}/applications/vtt.desktop"
echo "Installed $(command -v vtt || echo ~/.cargo/bin/vtt). Restart any running vtt."
