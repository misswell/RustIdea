#!/bin/bash
set -euo pipefail

repo_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
notary_profile="${NOTARY_PROFILE:-agentspace-notary}"
app_version="${APP_VERSION:-0.1.1}"
dist_dir="$repo_root/dist"
app_bundle="$dist_dir/RustIdea.app"
universal_binary="$dist_dir/rustidea-universal"
archive="$dist_dir/RustIdea-$app_version-macos-universal.zip"
upload_archive="$dist_dir/RustIdea-$app_version-macos-universal-upload.zip"
identity="${DEVELOPER_ID_APPLICATION:-Developer ID Application: Guofeng Liu (U8U443D7ZL)}"

cd "$repo_root"

xcrun notarytool history --keychain-profile "$notary_profile" >/dev/null

cargo build --locked --release -p rustidea --target aarch64-apple-darwin
cargo build --locked --release -p rustidea --target x86_64-apple-darwin
mkdir -p "$dist_dir"
lipo -create \
  target/aarch64-apple-darwin/release/rustidea \
  target/x86_64-apple-darwin/release/rustidea \
  -output "$universal_binary"

RUSTIDEA_APP_BINARY="$universal_binary" APP_VERSION="$app_version" \
  "$repo_root/scripts/package-macos.sh"

codesign --force --options runtime --timestamp --sign "$identity" "$app_bundle"
codesign --verify --deep --strict --verbose=2 "$app_bundle"
lipo -info "$app_bundle/Contents/MacOS/rustidea" | grep -q arm64
lipo -info "$app_bundle/Contents/MacOS/rustidea" | grep -q x86_64

ditto -c -k --keepParent "$app_bundle" "$upload_archive"
xcrun notarytool submit "$upload_archive" \
  --keychain-profile "$notary_profile" \
  --no-s3-acceleration --wait --timeout 1h --output-format json \
  > "$dist_dir/notary-result.json"
jq -e '.status == "Accepted"' "$dist_dir/notary-result.json" >/dev/null

xcrun stapler staple "$app_bundle"
xcrun stapler validate "$app_bundle"
codesign --verify --deep --strict --verbose=2 "$app_bundle"
spctl --assess --type execute --verbose=4 "$app_bundle"

ditto -c -k --keepParent "$app_bundle" "$archive"
shasum -a 256 "$archive"
