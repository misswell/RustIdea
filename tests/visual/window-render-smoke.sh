#!/bin/bash
# Capture a real GPUI window and assert that it painted both quads and glyphs.
# The headless renderer test passes without `font-kit`, so only a window proves
# that the shipped renderer can draw text.
set -euo pipefail

here="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "$here/../.." && pwd)"
binary="${RUSTIDEA_WINDOW_PROBE:-$repo_root/experiments/zed-poc/target/release/rustidea-window-probe}"

if [[ ! -x "$binary" ]]; then
  echo "Missing window probe binary: $binary" >&2
  echo "Build it with: cargo build --release -p rustidea-zed-workbench" >&2
  exit 1
fi

log="$(mktemp -t rustidea-window-probe)"
image="$(mktemp -t rustidea-window-probe).png"
trap 'rm -f "$log" "$image"' EXIT

"$binary" >"$log" 2>&1 &
probe_pid=$!
trap 'kill -TERM "$probe_pid" 2>/dev/null || true; wait "$probe_pid" 2>/dev/null || true; rm -f "$log" "$image"' EXIT

info=""
for _ in $(seq 1 30); do
  info="$(swift "$here/window.swift" rustidea-window-probe "Window Probe" | head -1 || true)"
  [[ -n "$info" ]] && break
  sleep 0.5
done
if [[ -z "$info" ]]; then
  echo "The probe window never appeared." >&2
  tail -5 "$log" >&2 || true
  exit 1
fi

"$here/capture-window.sh" rustidea-window-probe "$image" "Window Probe" >/dev/null

python3 - "$image" <<'PY'
import sys
from PIL import Image

image = Image.open(sys.argv[1]).convert("RGB")
red = dark = 0
for y in range(20, 100):
    for x in range(20, 340):
        r, g, b = image.getpixel((x, y))
        if r > 200 and g < 80 and b < 80:
            red += 1
        elif r < 60 and g < 60 and b < 60:
            dark += 1

print(f"window probe pixels: red={red} glyphs={dark}")
if red < 5000:
    sys.exit("the probe window did not paint its red box")
if dark < 50:
    sys.exit("the probe window painted no text: gpui_apple/font-kit is missing")
PY
