#!/bin/sh
set -eu

# Pin the exact Zed upstream used by the experiment. The checkout is not
# committed; this script recreates it from a clean RustIdea clone.
ZED_REV=2664a0aa1eae5dabf8f8273c03f88d53061ca7dc
ZED_DIR=${1:-third_party/zed}

if [ ! -d "$ZED_DIR/.git" ]; then
    mkdir -p "$(dirname "$ZED_DIR")"
    git init "$ZED_DIR"
    git -C "$ZED_DIR" remote add origin https://github.com/zed-industries/zed.git
fi

git -C "$ZED_DIR" fetch --depth 1 origin "$ZED_REV"
git -C "$ZED_DIR" checkout -q FETCH_HEAD
ZED_PATCH="$(cd "$(dirname "$0")/.." && pwd)/experiments/zed-poc/patches/zed-tree-sitter-0.25.patch"
if git -C "$ZED_DIR" apply --check "$ZED_PATCH" 2>/dev/null; then
    git -C "$ZED_DIR" apply "$ZED_PATCH"
fi
printf '%s\n' "$ZED_REV" > "$ZED_DIR/.rustidea-zed-rev"
