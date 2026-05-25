#!/usr/bin/env bash
# Build a signed Release IPA for iOS and upload it to App Store Connect.
#
# Prerequisites (manual, browser-driven):
#   1. Apple Developer portal → Identifiers → com.yappy.app:
#      enable App Groups + Increased Memory Limit
#   2. Identifiers → App Groups: register group.com.yappy.app
#      and add it to com.yappy.app
#   3. App Store Connect → My Apps → + → New App:
#      name=Yappy, bundleId=com.yappy.app, sku=yappy-ios, lang=en-US
#
# After all three are done, `node scripts/asc-helper.mjs status` should show
# the app. Then this script:
#   - regenerates the Xcode project so the App Group entitlement is fresh
#   - builds a signed Release IPA via cargo tauri ios build
#   - calls scripts/asc-helper.mjs pipeline to: wait for ASC processing,
#     create the public Beta group, attach the build, submit for review.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT/yappy-app"

# Refresh ORT iOS framework + .cargo/config.toml if user wiped vendor/
if [ ! -d "$ROOT/vendor/ort-ios" ]; then
  echo "→ vendor/ort-ios missing, re-fetching"
  "$ROOT/scripts/fetch-ort-ios.sh"
fi

# Clean stale build artifacts that conflict with the new entitlements
rm -rf src-tauri/gen/apple/build
rm -rf src-tauri/gen/apple/Externals/arm64/{debug,release} 2>/dev/null

echo "→ building signed Release IPA for device (aarch64-apple-ios)"
cargo tauri ios build --target aarch64

IPA="$ROOT/yappy-app/src-tauri/gen/apple/build/arm64/Yappy.ipa"
if [ ! -f "$IPA" ]; then
  echo "ERROR: no IPA at $IPA"
  exit 1
fi
echo "✓ IPA produced: $(ls -lh "$IPA" | awk '{print $5}')"

# `cargo tauri ios build` already runs `xcodebuild -exportArchive` with the
# ExportOptions.plist (method=app-store-connect, destination=upload), which
# uploads the IPA to ASC as part of the export. Verify by polling.
echo
echo "→ waiting for build to surface + process in App Store Connect…"
node "$ROOT/scripts/asc-helper.mjs" pipeline 0.1.0
