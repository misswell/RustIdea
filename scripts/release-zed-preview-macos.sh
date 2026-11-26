#!/bin/bash
set -euo pipefail

repo_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
notary_profile="${NOTARY_PROFILE:-agentspace-notary}"
app_version="${APP_VERSION:-0.3.0}"
build_number="${BUILD_NUMBER:-1}"
dist_dir="$repo_root/dist"
app_name="RustIdea Zed Preview"
app_bundle="$dist_dir/$app_name.app"
archive="$dist_dir/RustIdea-Zed-Preview-$app_version-macos-arm64.zip"
upload_archive="$dist_dir/RustIdea-Zed-Preview-$app_version-macos-arm64-upload.zip"
checksum_file="$archive.sha256"
identity="${DEVELOPER_ID_APPLICATION:-Developer ID Application: Guofeng Liu (U8U443D7ZL)}"

cd "$repo_root"

xcrun notarytool history --keychain-profile "$notary_profile" >/dev/null

APP_VERSION="$app_version" BUILD_NUMBER="$build_number" "$repo_root/scripts/package-zed-preview-macos.sh"

if [[ "$(lipo -archs "$app_bundle/Contents/MacOS/rustidea-zed-workbench")" != "arm64" ]]; then
  echo "Expected an arm64 Preview binary." >&2
  exit 1
fi

codesign --force --options runtime --timestamp --sign "$identity" "$app_bundle"
codesign --verify --deep --strict --verbose=2 "$app_bundle"

linked_libraries="$(otool -L "$app_bundle/Contents/MacOS/rustidea-zed-workbench")"
if printf '%s\n' "$linked_libraries" | grep -Eiq 'webkit|webview|javascriptcore|tauri|wry'; then
  echo "Forbidden WebView runtime linked into the Preview binary:" >&2
  printf '%s\n' "$linked_libraries" >&2
  exit 1
fi

ditto -c -k --keepParent "$app_bundle" "$upload_archive"
xcrun notarytool submit "$upload_archive" \
  --keychain-profile "$notary_profile" \
  --no-s3-acceleration --wait --timeout 1h --output-format json \
  > "$dist_dir/notary-result-zed-preview.json"
jq -e '.status == "Accepted"' "$dist_dir/notary-result-zed-preview.json" >/dev/null

xcrun stapler staple "$app_bundle"
xcrun stapler validate "$app_bundle"
codesign --verify --deep --strict --verbose=2 "$app_bundle"
spctl --assess --type execute --verbose=4 "$app_bundle"

rm -f "$archive" "$checksum_file"
ditto -c -k --keepParent "$app_bundle" "$archive"
shasum -a 256 "$archive" | tee "$checksum_file"
