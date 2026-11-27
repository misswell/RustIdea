#!/bin/bash
# Release the native RustIdea application. The legacy crates/rustidea app is a
# frozen regression baseline; see RUSTIDEA_NATIVE_APP.md.
set -euo pipefail

repo_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
notary_profile="${NOTARY_PROFILE:-agentspace-notary}"
app_version="${APP_VERSION:-0.4.0}"
build_number="${BUILD_NUMBER:-1}"
dist_dir="$repo_root/dist"
identity="${DEVELOPER_ID_APPLICATION:-Developer ID Application: Guofeng Liu (U8U443D7ZL)}"
entitlements="$repo_root/scripts/zed-preview.entitlements"
zed_revision="${ZED_REVISION:-2664a0aa1eae5dabf8f8273c03f88d53061ca7dc}"

if [[ "${RUSTIDEA_STABLE_REPLACEMENT_APPROVED:-0}" == "1" ]]; then
  app_name="RustIdea"
  archive="$dist_dir/RustIdea-$app_version-macos-arm64.zip"
else
  app_name="RustIdea Native Preview"
  archive="$dist_dir/RustIdea-Native-Preview-$app_version-macos-arm64.zip"
fi
app_bundle="$dist_dir/$app_name.app"
upload_archive="$archive.upload.zip"
checksum_file="$archive.sha256"

cd "$repo_root"
xcrun notarytool history --keychain-profile "$notary_profile" >/dev/null

PROFILE=release "$repo_root/scripts/build-native.sh"

APP_VERSION="$app_version" BUILD_NUMBER="$build_number" \
RUSTIDEA_NATIVE_APP_BINARY="$repo_root/experiments/zed-poc/target/release/rustidea-zed-workbench" \
ZED_REVISION="$zed_revision" \
  "$repo_root/scripts/package-native-macos.sh"

codesign --force --options runtime --timestamp \
  --entitlements "$entitlements" --sign "$identity" "$app_bundle"
codesign --verify --deep --strict --verbose=2 "$app_bundle"

signature_entitlements="$(codesign -d --entitlements - "$app_bundle" 2>/dev/null)"
if ! printf '%s\n' "$signature_entitlements" | grep -q 'com.apple.security.cs.allow-jit'; then
  echo "Signed app is missing the JIT entitlement required by Wasmtime." >&2
  exit 1
fi
if ! printf '%s\n' "$signature_entitlements" | grep -q 'com.apple.security.cs.allow-unsigned-executable-memory'; then
  echo "Signed app is missing the unsigned executable memory entitlement." >&2
  exit 1
fi

linked_libraries="$(otool -L "$app_bundle/Contents/MacOS/rustidea-native")"
if printf '%s\n' "$linked_libraries" | grep -Eiq 'webkit|webview|javascriptcore|tauri|wry'; then
  echo "Forbidden WebView runtime linked into the native binary:" >&2
  printf '%s\n' "$linked_libraries" >&2
  exit 1
fi
if printf '%s\n' "$linked_libraries" | grep -q 'rustidea-editor'; then
  echo "The legacy editor is linked into the native binary." >&2
  exit 1
fi

ditto -c -k --keepParent "$app_bundle" "$upload_archive"
xcrun notarytool submit "$upload_archive" \
  --keychain-profile "$notary_profile" \
  --no-s3-acceleration --wait --timeout 1h --output-format json \
  > "$dist_dir/notary-result-native.json"
jq -e '.status == "Accepted"' "$dist_dir/notary-result-native.json" >/dev/null

xcrun stapler staple "$app_bundle"
xcrun stapler validate "$app_bundle"
codesign --verify --deep --strict --verbose=2 "$app_bundle"
spctl --assess --type execute --verbose=4 "$app_bundle"

rm -f "$archive" "$checksum_file"
ditto -c -k --keepParent "$app_bundle" "$archive"
shasum -a 256 "$archive" | tee "$checksum_file"
