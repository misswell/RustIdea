#!/bin/bash
# Package the native (Zed/GPUI) binary as a macOS .app.
#
# Stable replacement is intentionally gated: RustIdea's existing stable bundle
# id and user-data migration need maintainer approval (see RUSTIDEA_NATIVE_APP.md).
# Set RUSTIDEA_STABLE_REPLACEMENT_APPROVED=1 after that approval to produce
# "RustIdea.app" with the com.guofeng.rustidea identifier.
set -euo pipefail

repo_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
app_version="${APP_VERSION:-0.4.0}"
build_number="${BUILD_NUMBER:-1}"
app_binary="${RUSTIDEA_NATIVE_APP_BINARY:-$repo_root/experiments/zed-poc/target/release/rustidea-zed-workbench}"
zed_revision="${ZED_REVISION:-2664a0aa1eae5dabf8f8273c03f88d53061ca7dc}"
source_revision="$(git -C "$repo_root" rev-parse HEAD)"
source_branch="$(git -C "$repo_root" branch --show-current)"

if [[ "${RUSTIDEA_STABLE_REPLACEMENT_APPROVED:-0}" == "1" ]]; then
  app_name="RustIdea"
  bundle_id="${RUSTIDEA_BUNDLE_ID:-com.guofeng.rustidea}"
  default_bundle="$repo_root/dist/$app_name.app"
else
  app_name="RustIdea Native Preview"
  bundle_id="${RUSTIDEA_BUNDLE_ID:-com.guofeng.rustidea.native-preview}"
  default_bundle="$repo_root/dist/RustIdea Native Preview.app"
fi
app_bundle="${APP_BUNDLE_PATH:-$default_bundle}"
contents="$app_bundle/Contents"
resources="$contents/Resources"
executable="rustidea-native"

if [[ ! -x "$app_binary" ]]; then
  echo "Missing native binary: $app_binary" >&2
  echo "Build it with scripts/build-native.sh (PROFILE=release)." >&2
  exit 1
fi
if [[ ! -f "$repo_root/third_party/zed/LICENSE-GPL" ]]; then
  echo "Missing third_party/zed/LICENSE-GPL. Run scripts/fetch-zed.sh." >&2
  exit 1
fi

rm -rf "$app_bundle"
mkdir -p "$contents/MacOS" "$resources"
cp "$app_binary" "$contents/MacOS/$executable"
chmod 0755 "$contents/MacOS/$executable"

if [[ -f "$repo_root/assets/RustIdea.icns" ]]; then
  cp "$repo_root/assets/RustIdea.icns" "$resources/RustIdea.icns"
fi

cp "$repo_root/third_party/zed/LICENSE-GPL" "$resources/LICENSE-GPL-3.0.txt"
cp "$repo_root/LICENSE" "$resources/LICENSE-RustIdea-Apache-2.0.txt"
cp "$repo_root/third_party/zed/LICENSE-APACHE" "$resources/LICENSE-Zed-Apache-2.0.txt"
cp "$repo_root/NOTICE" "$resources/NOTICE.txt"
cp "$repo_root/THIRD_PARTY_LICENSES.md" "$resources/THIRD_PARTY_LICENSES.md"
cp "$repo_root/THIRD_PARTY_RUST_LICENSES.md" "$resources/THIRD_PARTY_RUST_LICENSES.md"
cp -R "$repo_root/THIRD_PARTY_SOURCES" "$resources/THIRD_PARTY_SOURCES"
cp "$repo_root/RUSTIDEA_ZED_LICENSE_AUDIT.md" "$resources/RUSTIDEA_ZED_LICENSE_AUDIT.md"
cp "$repo_root/RUSTIDEA_UI_PARITY.md" "$resources/RUSTIDEA_UI_PARITY.md"
cp "$repo_root/RUSTIDEA_NATIVE_APP.md" "$resources/RUSTIDEA_NATIVE_APP.md"
cp "$repo_root/scripts/fetch-zed.sh" "$resources/fetch-zed.sh"
cp "$repo_root/scripts/zed-preview.entitlements" "$resources/native.entitlements"

cat > "$contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key><string>en</string>
  <key>CFBundleExecutable</key><string>$executable</string>
  <key>CFBundleIconFile</key><string>RustIdea</string>
  <key>CFBundleIdentifier</key><string>$bundle_id</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundleName</key><string>$app_name</string>
  <key>CFBundleDisplayName</key><string>$app_name</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$app_version</string>
  <key>CFBundleVersion</key><string>$build_number</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.developer-tools</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

cat > "$resources/README.txt" <<TEXT
$app_name $app_version

RustIdea is OpenIDEA's product with the Zed editor and a native GPUI shell.
This is a GPL-3.0-or-later combined work: it includes Zed editor and language
crates under the GNU General Public License version 3 or later. RustIdea and
OpenIDEA source remain available under Apache-2.0; the combined application is
distributed under GPL-3.0-or-later.

Source and reproducible build information:
- RustIdea: https://github.com/misswell/RustIdea
- revision: $source_revision
- branch: $source_branch
- Zed: https://github.com/zed-industries/zed at $zed_revision
- OpenIDEA token baseline: fe3e647abb804cffbfff6bd8de42e01762d6de40 (0.1.54)
- recreate the ignored Zed checkout with ./fetch-zed.sh (a copy is included)

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
Zed source: https://github.com/zed-industries/zed
Zed revision: $zed_revision
OpenIDEA baseline: fe3e647abb804cffbfff6bd8de42e01762d6de40
OpenIDEA source: https://github.com/l0ng-ai/openidea
TEXT

/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$contents/Info.plist" >/dev/null
echo "$app_bundle"
