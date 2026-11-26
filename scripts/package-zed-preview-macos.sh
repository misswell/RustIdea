#!/bin/bash
set -euo pipefail

repo_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
app_version="${APP_VERSION:-0.3.0}"
build_number="${BUILD_NUMBER:-1}"
app_name="RustIdea Zed Preview"
app_binary="${RUSTIDEA_ZED_APP_BINARY:-$repo_root/experiments/zed-poc/target/release/rustidea-zed-workbench}"
app_bundle="${APP_BUNDLE_PATH:-$repo_root/dist/$app_name.app}"
contents="$app_bundle/Contents"
resources="$contents/Resources"
zed_revision="${ZED_REVISION:-2664a0aa1eae5dabf8f8273c03f88d53061ca7dc}"
source_revision="$(git -C "$repo_root" rev-parse HEAD)"
source_branch="$(git -C "$repo_root" branch --show-current)"

if [[ ! -x "$app_binary" ]]; then
  echo "Missing Zed workbench binary: $app_binary" >&2
  echo "Build it with: (cd experiments/zed-poc && cargo build --locked --release -p rustidea-zed-workbench)" >&2
  exit 1
fi

if [[ ! -f "$repo_root/third_party/zed/LICENSE-GPL" ]]; then
  echo "Missing third_party/zed/LICENSE-GPL. Run scripts/fetch-zed.sh first." >&2
  exit 1
fi

rm -rf "$app_bundle"
mkdir -p "$contents/MacOS" "$resources"
cp "$app_binary" "$contents/MacOS/rustidea-zed-workbench"
chmod 0755 "$contents/MacOS/rustidea-zed-workbench"

if [[ -f "$repo_root/assets/RustIdea.icns" ]]; then
  cp "$repo_root/assets/RustIdea.icns" "$resources/RustIdea.icns"
fi

cp "$repo_root/third_party/zed/LICENSE-GPL" "$resources/LICENSE.txt"
cp "$repo_root/LICENSE" "$resources/LICENSE-RustIdea-Apache-2.0.txt"
cp "$repo_root/third_party/zed/LICENSE-APACHE" "$resources/LICENSE-Zed-Apache-2.0.txt"
cp "$repo_root/NOTICE" "$resources/NOTICE.txt"
cp "$repo_root/THIRD_PARTY_LICENSES.md" "$resources/THIRD_PARTY_LICENSES.md"
cp "$repo_root/THIRD_PARTY_RUST_LICENSES.md" "$resources/THIRD_PARTY_RUST_LICENSES.md"
cp -R "$repo_root/THIRD_PARTY_SOURCES" "$resources/THIRD_PARTY_SOURCES"
cp "$repo_root/RUSTIDEA_ZED_LICENSE_AUDIT.md" "$resources/RUSTIDEA_ZED_LICENSE_AUDIT.md"
cp "$repo_root/RUSTIDEA_ZED_POC_REPORT.md" "$resources/RUSTIDEA_ZED_POC_REPORT.md"
cp "$repo_root/RUSTIDEA_ZED_INTEGRATION_P0.md" "$resources/RUSTIDEA_ZED_INTEGRATION_P0.md"
cp "$repo_root/scripts/fetch-zed.sh" "$resources/fetch-zed.sh"
cp "$repo_root/scripts/zed-preview.entitlements" "$resources/zed-preview.entitlements"
cp -R "$repo_root/third_party/zed/assets" "$resources/ZedAssets"

cat > "$resources/README.txt" <<TEXT
RustIdea Zed Preview $app_version

This is a GPL-3.0-or-later combined-work Preview. It includes the Zed editor
and language crates under the GNU General Public License version 3 or later.
RustIdea and OpenIDEA source remains available under Apache-2.0; the combined
application is distributed under GPL-3.0-or-later.

Source and reproducible build information:
- RustIdea: https://github.com/misswell/RustIdea
- revision: $source_revision
- branch: $source_branch
- Zed: https://github.com/zed-industries/zed at $zed_revision
- recreate the ignored Zed checkout with ./fetch-zed.sh (a copy is included)

Launch with a project directory:
  open -a "RustIdea Zed Preview" --args /path/to/project

The app uses GPUI and Zed's native editor. It contains no WebView, WebKit,
Tauri, React, CodeMirror, Monaco, or xterm.js runtime.

The hardened runtime signature includes the Apple JIT entitlements required by
the pinned tree-sitter/Wasmtime grammar engine. They are limited to
com.apple.security.cs.allow-jit and
com.apple.security.cs.allow-unsigned-executable-memory.
TEXT

cat > "$resources/SOURCE_CODE.txt" <<TEXT
RustIdea source: https://github.com/misswell/RustIdea
RustIdea revision: $source_revision
RustIdea branch: $source_branch
Zed upstream: https://github.com/zed-industries/zed
Zed revision: $zed_revision

Reproduce:
  git clone https://github.com/misswell/RustIdea.git
  git -C RustIdea checkout $source_revision
  cd RustIdea
  scripts/fetch-zed.sh
  cd experiments/zed-poc
  cargo build --locked --release -p rustidea-zed-workbench

experiments/zed-poc/Cargo.lock pins the complete Rust dependency graph.
TEXT

cat > "$contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key><string>en</string>
  <key>CFBundleExecutable</key><string>rustidea-zed-workbench</string>
  <key>CFBundleIconFile</key><string>RustIdea</string>
  <key>CFBundleIdentifier</key><string>com.guofeng.rustidea.zedpreview</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundleName</key><string>RustIdea Zed Preview</string>
  <key>CFBundleDisplayName</key><string>RustIdea Zed Preview</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$app_version</string>
  <key>CFBundleVersion</key><string>$build_number</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.developer-tools</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSHumanReadableCopyright</key><string>GPL-3.0-or-later combined work. RustIdea includes Zed and OpenIDEA components.</string>
</dict>
</plist>
PLIST

/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$contents/Info.plist" >/dev/null
echo "$app_bundle"
