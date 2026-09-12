#!/usr/bin/env bash
# Renderiza todos los idiomas capturados en las carpetas de locale de ASC.
#   marketing/render-todo.sh [iphone|ipad|ambos]
set -uo pipefail
cd "$(dirname "$0")"
MODO="${1:-iphone}"
asc_de() { case "$1" in es) echo "es-ES es-MX";; en) echo "en-US en-GB en-AU en-CA";; fr) echo "fr-FR fr-CA";; pt) echo "pt-PT pt-BR";; de) echo "de-DE";; nl) echo "nl-NL";; ar) echo "ar-SA";; *) echo "$1";; esac; }
for f in copy/*.json; do
  L=$(basename "$f" .json)
  for dev in iphone ipad; do
    [ "$MODO" = ambos ] || [ "$MODO" = "$dev" ] || continue
    src="screenshots"; [ "$dev" = ipad ] && src="screenshots-ipad"
    n=$(ls "$src/$L" 2>/dev/null | wc -l | tr -d ' ')
    if [ "$n" -lt 20 ]; then echo "· $L/$dev: sin capturas suficientes ($n), se salta"; continue; fi
    locales="$(asc_de "$L")"
    for asc in $locales; do
      node render.mjs "$L" --device "$dev" --asc "$asc" > /dev/null 2>&1 && echo "✓ $asc/$dev" || echo "✗ $asc/$dev"
    done
  done
done
