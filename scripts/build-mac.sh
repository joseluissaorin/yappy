#!/usr/bin/env bash
# Yappy — production macOS build with auto-cert detection.
#
# Picks the strongest available signing identity from the login keychain:
#   1. Developer ID Application   →  full, notarizable, distributable .dmg
#   2. Apple Distribution         →  Mac App Store track (warns user)
#   3. Apple Development          →  local-only (won't pass Gatekeeper after Download)
#   4. None                       →  unsigned (Tauri default, useful for CI)
#
# After a successful build with a "Developer ID Application" identity, run
# scripts/notarize.sh to staple a notarization ticket.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT/yappy-app"

# --- conda's `xattr` shadows macOS's; the Tauri DMG step calls /usr/bin/xattr ---
export PATH="/usr/bin:/usr/local/bin:$PATH"

# --- Detach stale Yappy DMG mounts (they break bundle_dmg.sh) ----------
# When you open a previous DMG without ejecting, /Volumes/Yappy stays mounted.
# bundle_dmg.sh's hdiutil call then errors out trying to create a new mount.
for vol in /Volumes/Yappy*; do
  if [[ -d "$vol" ]]; then
    echo "→ detaching stale mount: $vol"
    hdiutil detach "$vol" -force >/dev/null 2>&1 || true
  fi
done

# --- Identity discovery ---------------------------------------------------
ids=$(security find-identity -v -p codesigning 2>/dev/null || true)
echo "── available signing identities ──"
echo "$ids" | sed -e 's/^/   /'
echo

pick_identity() {
  # Priority: Developer ID Application > Apple Distribution > Apple Development.
  # First match wins.
  for needle in "Developer ID Application" "Apple Distribution" "Apple Development"; do
    line=$(echo "$ids" | grep -F "$needle" | head -1 || true)
    if [[ -n "$line" ]]; then
      # The CN is in quotes: extract whatever's between the first pair.
      cn=$(echo "$line" | sed -E 's/.*"([^"]+)".*/\1/')
      echo "$cn"
      return 0
    fi
  done
  echo ""
}

CHOSEN="$(pick_identity)"

if [[ -z "$CHOSEN" ]]; then
  echo "⚠ no signing identity in the login keychain — building unsigned."
  unset APPLE_SIGNING_IDENTITY
else
  echo "→ using identity: $CHOSEN"
  export APPLE_SIGNING_IDENTITY="$CHOSEN"
fi

# --- Warn about non-distributable identities ------------------------------
case "$CHOSEN" in
  "Developer ID Application"*)
    echo "✓ this build CAN be distributed outside the Mac App Store (after notarization)."
    ;;
  "Apple Distribution"*)
    echo "⚠ Apple Distribution cert detected — this is for Mac App Store / TestFlight only."
    echo "  For free download distribution you need a *Developer ID Application* cert."
    echo "  → https://developer.apple.com/account/resources/certificates/add  →  'Developer ID Application'"
    ;;
  "Apple Development"*)
    echo "⚠ Apple Development cert detected — this build will NOT pass Gatekeeper on other Macs."
    echo "  Local testing only. For distribution, request a *Developer ID Application* cert:"
    echo "  → https://developer.apple.com/account/resources/certificates/add  →  'Developer ID Application'"
    ;;
esac
echo

# --- Build ---------------------------------------------------------------
echo "→ npm run tauri build"
npm run tauri build

DMG_DIR="$ROOT/target/release/bundle/dmg"
APP_DIR="$ROOT/target/release/bundle/macos"
if [[ -d "$DMG_DIR" ]]; then
  DMG="$(ls -t "$DMG_DIR"/*.dmg 2>/dev/null | head -1 || true)"
  APP="$(ls -dt "$APP_DIR"/*.app 2>/dev/null | head -1 || true)"
  echo
  echo "✓ built:"
  [[ -n "$DMG" ]] && echo "   $DMG"
  [[ -n "$APP" ]] && echo "   $APP"
  case "$CHOSEN" in
    "Developer ID Application"*)
      echo
      echo "next step: notarize"
      echo "   ./scripts/notarize.sh"
      ;;
  esac
fi
