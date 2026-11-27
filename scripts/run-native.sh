#!/bin/bash
# Build (when needed) and run the official native application.
#   scripts/run-native.sh [--trust] [project] [file]
# --trust is the explicit equivalent of OpenIDEA's Trust Project dialog.
set -euo pipefail

repo_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
profile="${PROFILE:-debug}"
trust=0
args=()
for arg in "$@"; do
  case "$arg" in
    --trust) trust=1 ;;
    *) args+=("$arg") ;;
  esac
done

if [[ ! -x "$repo_root/experiments/zed-poc/target/$profile/rustidea-zed-workbench" ]]; then
  PROFILE="$profile" "$repo_root/scripts/build-native.sh" >/dev/null
fi

binary="$repo_root/experiments/zed-poc/target/$profile/rustidea-zed-workbench"
if [[ "$trust" == "1" ]]; then
  RUSTIDEA_ZED_TRUST=1 exec "$binary" "${args[@]+"${args[@]}"}"
else
  exec "$binary" "${args[@]+"${args[@]}"}"
fi
