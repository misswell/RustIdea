#!/bin/bash
set -euo pipefail

repo_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
app_version="${APP_VERSION:-0.1.1}"
app_bundle="$repo_root/dist/RustIdea.app"
contents="$app_bundle/Contents"

if [[ ! -x "$repo_root/target/release/rustidea" ]]; then
  echo "Missing target/release/rustidea; run cargo build --release -p rustidea first." >&2
  exit 1
fi

rm -rf "$app_bundle"
mkdir -p "$contents/MacOS" "$contents/Resources"
cp "$repo_root/target/release/rustidea" "$contents/MacOS/rustidea"
cp "$repo_root/LICENSE" "$repo_root/NOTICE" \
  "$repo_root/THIRD_PARTY_LICENSES.md" \
  "$repo_root/THIRD_PARTY_RUST_LICENSES.md" "$contents/Resources/"
cp -R "$repo_root/THIRD_PARTY_SOURCES" "$contents/Resources/"

cat > "$contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key><string>en</string>
  <key>CFBundleExecutable</key><string>rustidea</string>
  <key>CFBundleIdentifier</key><string>com.guofeng.rustidea</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundleName</key><string>RustIdea</string>
  <key>CFBundleDisplayName</key><string>RustIdea</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$app_version</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.developer-tools</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$contents/Info.plist" >/dev/null
echo "$app_bundle"
