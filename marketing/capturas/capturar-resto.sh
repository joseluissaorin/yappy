#!/usr/bin/env bash
# Repite SOLO el flujo «resto» de un idioma (la cinta ya tiene sus tres textos).
set -uo pipefail
L="$1"; RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SIM="${SIM_UDID:-$(cat /tmp/yappy-sim-capturas.txt)}"; APP=com.joseluissaorin.yappy
FLOWS="$RAIZ/marketing/capturas/flows/$L"; cd "$RAIZ"
timeout 60 xcrun simctl terminate "$SIM" $APP 2>/dev/null; sleep 1
SIMCTL_CHILD_YAPPY_TIENDA=sim timeout 90 xcrun simctl launch "$SIM" $APP -AppleLanguages "($L)" -AppleLocale "${L}_$(echo "$L" | tr a-z A-Z)" >/dev/null 2>&1
sleep 8
TITULO="$(node -e 'const t=require(process.argv[1])[2];process.stdout.write(t.titulo)' "$RAIZ/marketing/textos/$L.json" | sed 's/[.*+?^${}()|[\]\\]/\\&/g')"
sed "s|__TITULO__|$TITULO|" "$FLOWS/resto.template.yaml" > "$FLOWS/resto.yaml"
timeout 300 "$HOME/.maestro/bin/maestro" --device "$SIM" test "$FLOWS/resto.yaml" 2>&1 | grep -E "FAILED|WARNED|Exception" | grep -v "(Optional)" | head -5
ps -ef | grep -E "[m]aestro.cli|[t]est-without-building" | awk '{print $2}' | xargs kill -9 2>/dev/null
echo "── resto $L hecho $(date +%H:%M:%S)"
