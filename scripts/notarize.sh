#!/usr/bin/env bash
# Yappy — notarize the produced .dmg + staple the ticket.
#
# Requires a *Developer ID Application* certificate in the login keychain and an
# app-specific password for your Apple ID stored in keychain item "yappy-notary".
#
# To create the keychain item once:
#   xcrun notarytool store-credentials yappy-notary \
#     --apple-id you@example.com \
#     --team-id   YOUR_TEAM_ID \
#     --password  YOUR_APP_SPECIFIC_PASSWORD
#
# Then just `./scripts/notarize.sh` after `npm run tauri build`.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DMG_DIR="$ROOT/target/release/bundle/dmg"
APP_DIR="$ROOT/target/release/bundle/macos"

# Sanity-check: a Developer ID Application cert MUST be present, otherwise
# notarization will reject the upload. Print a clear pointer if missing.
if ! security find-identity -v -p codesigning 2>/dev/null | grep -q "Developer ID Application"; then
  echo "✗ no 'Developer ID Application' identity in the login keychain."
  echo
  echo "  Notarization only accepts builds signed with that specific cert."
  echo "  Your Apple Development / Apple Distribution certs (if any) won't work."
  echo
  echo "  → request one (free with your active membership) at:"
  echo "     https://developer.apple.com/account/resources/certificates/add"
  echo "     ... and pick 'Developer ID Application'."
  exit 1
fi

DMG="$(ls -t "$DMG_DIR"/*.dmg 2>/dev/null | head -1 || true)"
APP="$(ls -dt "$APP_DIR"/*.app 2>/dev/null | head -1 || true)"

if [[ -z "$DMG" || -z "$APP" ]]; then
  echo "✗ No .dmg or .app found. Run 'npm run tauri build' first."
  exit 1
fi

echo "→ Submitting $DMG to notarytool…"
xcrun notarytool submit "$DMG" --keychain-profile yappy-notary --wait

echo "→ Stapling notarization ticket to the .dmg"
xcrun stapler staple "$DMG"

echo "→ Stapling to the .app inside the .dmg's source tree as well"
xcrun stapler staple "$APP"

echo "→ Verifying"
spctl -a -t open --context context:primary-signature -v "$DMG" || true
codesign -dv --verbose=4 "$APP" 2>&1 | head -20

echo "✓ Done — $DMG is notarized and stapled."
