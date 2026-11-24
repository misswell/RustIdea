#!/bin/bash
set -euo pipefail

repo_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
source_svg="$repo_root/assets/RustIdea.svg"
iconset="$repo_root/assets/RustIdea.iconset"
output="$repo_root/assets/RustIdea.icns"

rm -rf "$iconset"
mkdir -p "$iconset"

rsvg-convert -w 1024 -h 1024 "$source_svg" -o "$iconset/icon_512x512@2x.png"
for size in 16 32 128 256 512; do
  rsvg-convert -w "$size" -h "$size" "$source_svg" -o "$iconset/icon_${size}x${size}.png"
  double=$((size * 2))
  rsvg-convert -w "$double" -h "$double" "$source_svg" -o "$iconset/icon_${size}x${size}@2x.png"
done

iconutil -c icns "$iconset" -o "$output"
rm -rf "$iconset"
echo "$output"
