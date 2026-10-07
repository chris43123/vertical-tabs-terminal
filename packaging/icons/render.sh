#!/bin/sh
# Regenerate the PNG sizes and vtt.ico from vtt.svg. Needs resvg and Python with Pillow.
# (macOS's vtt.icns is built from the PNGs by scripts/package-macos.sh.)
set -eu
cd "$(dirname "$0")"
for s in 16 24 32 48 64 128 256 512 1024; do
  resvg -w "$s" -h "$s" vtt.svg "vtt-$s.png"
done
python3 - <<'PY'
from PIL import Image
sizes = [16, 24, 32, 48, 64, 128, 256]
images = [Image.open(f"vtt-{s}.png") for s in sizes]
images[-1].save("vtt.ico", sizes=[(s, s) for s in sizes], append_images=images[:-1], bitmap_format="png")
PY
rm vtt-24.png
