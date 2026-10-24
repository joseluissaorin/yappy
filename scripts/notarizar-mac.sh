#!/bin/bash
# Firma con Developer ID, notariza y grapa el .dmg de escritorio que sale de CI
# (allí se firma ad hoc, con "-"). Uso:
#   scripts/notarizar-mac.sh Yappy_1.0.1_aarch64.dmg  →  Yappy_1.0.1_aarch64.dmg firmado al lado, en ./notarizado/
# La notarización usa la misma clave de App Store Connect que asc-helper.mjs.
set -euo pipefail

DMG_IN="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
RAIZ="$(cd "$(dirname "$0")/.." && pwd)"
IDENTIDAD="Developer ID Application: Jose Luis Saorin Ferrer (9LYNY2477X)"
ENT="$RAIZ/yappy-app/src-tauri/Entitlements.plist"
KEY_ID="${ASC_KEY_ID:-4HVB5YWGWD}"
ISSUER="${ASC_ISSUER_ID:-1e05b19d-c430-4408-8af5-24d6623959c4}"
KEY="$HOME/.appstoreconnect/private_keys/AuthKey_${KEY_ID}.p8"

TMP="$(mktemp -d)"
SALIDA="$(dirname "$DMG_IN")/notarizado"
mkdir -p "$SALIDA"

# 1. Sacar el .app del dmg.
hdiutil attach -nobrowse -readonly -mountpoint "$TMP/vol" "$DMG_IN" >/dev/null
APP_NOMBRE="$(cd "$TMP/vol" && ls -d *.app | head -1)"
ditto "$TMP/vol/$APP_NOMBRE" "$TMP/$APP_NOMBRE"
hdiutil detach "$TMP/vol" >/dev/null
APP="$TMP/$APP_NOMBRE"

# 2. Firmar de dentro afuera: bibliotecas y ejecutables sueltos, luego el bundle.
find "$APP/Contents" -type f \( -name "*.dylib" -o -name "*.so" -o -perm -111 \) \
  ! -path "*/Contents/MacOS/*" -print0 |
  while IFS= read -r -d '' f; do
    if file "$f" | grep -q "Mach-O"; then
      codesign --force --timestamp --options runtime --sign "$IDENTIDAD" "$f"
    fi
  done
for f in "$APP"/Contents/MacOS/*; do
  codesign --force --timestamp --options runtime --entitlements "$ENT" --sign "$IDENTIDAD" "$f"
done
codesign --force --timestamp --options runtime --entitlements "$ENT" --sign "$IDENTIDAD" "$APP"
codesign --verify --deep --strict --verbose=2 "$APP"

# 3. Volver a empaquetar el dmg (con el enlace a Aplicaciones) y firmarlo.
mkdir "$TMP/dmg"
ditto "$APP" "$TMP/dmg/$APP_NOMBRE"
ln -s /Applications "$TMP/dmg/Applications"
DMG_OUT="$SALIDA/$(basename "$DMG_IN")"
rm -f "$DMG_OUT"
hdiutil create -volname "${APP_NOMBRE%.app}" -srcfolder "$TMP/dmg" -ov -format UDZO "$DMG_OUT" >/dev/null
codesign --force --timestamp --sign "$IDENTIDAD" "$DMG_OUT"

# 4. Notarizar y grapar.
xcrun notarytool submit "$DMG_OUT" --key "$KEY" --key-id "$KEY_ID" --issuer "$ISSUER" --wait
xcrun stapler staple "$DMG_OUT"
spctl -a -t open --context context:primary-signature -vv "$DMG_OUT"

rm -rf "$TMP"
echo "Listo: $DMG_OUT"
