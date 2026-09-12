#!/usr/bin/env bash
# Pasa capturar.sh por una lista de idiomas, reintentando SIN borrar lo ya
# capturado (el segundo intento rellena lo que faltó).
#   marketing/capturas/capturar-lote.sh es en fr …
set -uo pipefail
RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ESPERADAS="${ESPERADAS:-27}"
INTENTOS="${INTENTOS:-2}"
for L in "$@"; do
  for intento in $(seq 1 "$INTENTOS"); do
    "$RAIZ/marketing/capturas/capturar.sh" "$L" > "/tmp/captura-$L.log" 2>&1
    n=$(ls "${SHOTS_BASE:-$RAIZ/marketing/screenshots}/$L" 2>/dev/null | wc -l | tr -d ' ')
    if [ "$n" -ge "$ESPERADAS" ]; then echo "✓ $L ($n capturas, intento $intento) $(date +%H:%M)"; break; fi
    echo "✗ $L intento $intento: $n capturas — $(grep -E 'FAILED|booted|Exception' "/tmp/captura-$L.log" | head -2 | tr '\n' ' ') $(date +%H:%M)"
    ps -ef | grep -E "[m]aestro.cli.*${SIM_UDID:-}|[t]est-without-building.*${SIM_UDID:-}" | awk '{print $2}' | xargs kill -9 2>/dev/null; sleep 5
  done
done
