#!/bin/bash
# Capture one live macOS window and normalize it to logical pixels.
#   tests/visual/capture-window.sh <owner substring> <output.png> [title substring] [metadata.json]
#
# Refuses to capture while the GUI session is locked: a locked display does not
# present new GPU frames, so the capture returns a stale/partial frame (shapes
# without text) that looks like a real screenshot but is not the running UI.
# Set RUSTIDEA_ALLOW_LOCKED=1 only when capturing an app that renders through
# WebKit (its surface stays in the window server) and you know why.
set -euo pipefail

owner="${1:?owner substring}"
output="${2:?output png}"
title="${3:-}"
metadata="${4:-}"

here="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"

if [[ "${RUSTIDEA_ALLOW_LOCKED:-0}" != "1" ]]; then
  if ! swift "$here/session-locked.swift" >/dev/null; then
    echo "The GUI session is locked (or the display is asleep)." >&2
    echo "Unlock the Mac and keep it awake (caffeinate -d -u) before capturing;" >&2
    echo "a locked display returns stale frames that look deceptively empty." >&2
    exit 2
  fi
fi

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
