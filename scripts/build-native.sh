#!/bin/bash
# Build the one official RustIdea native application: Zed Editor + OpenIDEA
# backend + GPUI workbench. The legacy crates/rustidea app is frozen.
set -euo pipefail

repo_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
profile="${PROFILE:-release}"
workspace="$repo_root/experiments/zed-poc"
zed_dir="$repo_root/third_party/zed"

if [[ ! -f "$zed_dir/Cargo.toml" ]]; then
  echo "The pinned Zed checkout is missing." >&2
  echo "Run scripts/fetch-zed.sh first." >&2
  exit 1
fi

if [[ "$profile" == "release" ]]; then
  cargo_args=(build --locked --release -p rustidea-zed-workbench)
else
  cargo_args=(build --locked -p rustidea-zed-workbench)
fi

echo "Building RustIdea native ($profile) in $workspace"
(cd "$workspace" && cargo "${cargo_args[@]}")
binary="$workspace/target/$profile/rustidea-zed-workbench"
if [[ ! -x "$binary" ]]; then
  echo "Build finished without $binary" >&2
  exit 1
fi
echo "$binary"
