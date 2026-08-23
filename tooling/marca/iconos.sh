#!/usr/bin/env bash
# Regenera TODOS los iconos de la app desde brand/*.svg.
# Se generan, no se retocan a mano: si cambia el loro, se vuelve a correr esto.
#
# Pasos: render con Chrome headless (fondo transparente de verdad),
# `cargo tauri icon` para el juego completo (png, ico, iOS, Android),
# `iconutil` para un .icns con la retícula de macOS (margen + esquinas del
# sistema), y los template del tray en 44/88 px.
set -euo pipefail

RAIZ="$(cd "$(dirname "$0")/../.." && pwd)"
MARCA="$RAIZ/brand"
ICONOS="$RAIZ/yappy-app/src-tauri/icons"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

CHROME="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
[ -x "$CHROME" ] || CHROME="/Applications/Vivaldi.app/Contents/MacOS/Vivaldi"
[ -x "$CHROME" ] || { echo "No hay Chrome/Vivaldi para renderizar"; exit 1; }

render() { # render <svg> <tamaño> <salida.png>
  "$CHROME" --headless=new --screenshot="$3" --window-size="$2,$2" \
    --default-background-color=00000000 --hide-scrollbars \
    "file://$1" >/dev/null 2>&1
  local w
  w=$(sips -g pixelWidth "$3" | awk '/pixelWidth/{print $2}')
  [ "$w" = "$2" ] || { echo "render mal dimensionado: $3 ($w px)"; exit 1; }
}

echo "→ icono completo (tauri icon)"
render "$MARCA/icono-lleno.svg" 1024 "$TMP/icono-1024.png"
(cd "$RAIZ/yappy-app" && cargo tauri icon "$TMP/icono-1024.png" >/dev/null)

echo "→ icns con retícula macOS"
ICONSET="$TMP/yappy.iconset"
mkdir -p "$ICONSET"
for t in 16 32 128 256 512; do
  render "$MARCA/icono-macos.svg" "$t" "$ICONSET/icon_${t}x${t}.png"
  render "$MARCA/icono-macos.svg" "$((t * 2))" "$ICONSET/icon_${t}x${t}@2x.png"
done
iconutil -c icns "$ICONSET" -o "$ICONOS/icon.icns"

echo "→ tray template"
render "$MARCA/tray-template.svg" 44 "$ICONOS/yappy-tray-template.png"
render "$MARCA/tray-template.svg" 88 "$ICONOS/yappy-tray-template@2x.png"

echo "Listo: $ICONOS"
