#!/bin/bash
# Capture one live macOS window and normalize it to logical pixels.
#   tests/visual/capture-window.sh <owner substring> <output.png> [title substring] [metadata.json]
set -euo pipefail

owner="${1:?owner substring}"
output="${2:?output png}"
title="${3:-}"
metadata="${4:-}"

here="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
info="$(swift "$here/window.swift" "$owner" "$title" | head -1)"
if [[ -z "$info" ]]; then
  echo "No on-screen window matches owner '$owner' title '$title'" >&2
  exit 1
fi

IFS=$'\t' read -r id window_owner window_title x y width height <<<"$info"
tmp="$(mktemp -t rustidea-window).png"
trap 'rm -f "$tmp"' EXIT
screencapture -l "$id" -x "$tmp"
python3 "$here/normalize_window.py" "$tmp" "$output" --width "$width" --height "$height" \
  ${metadata:+--metadata "$metadata"}
echo "captured window $id ($window_owner / $window_title) ${width}x${height} -> $output"
