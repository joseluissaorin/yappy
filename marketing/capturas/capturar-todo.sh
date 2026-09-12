#!/usr/bin/env bash
# Pasa capturar.sh por una lista de idiomas, con reintento si el simulador
# se cae a mitad (SimRenderServer/backboardd mueren bajo presión de memoria).
#   marketing/capturas/capturar-todo.sh es en fr …
set -uo pipefail
RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ESPERADAS="${ESPERADAS:-27}"
for L in "$@"; do
  for intento in 1 2 3; do
    rm -rf "$RAIZ/marketing/screenshots/$L"
    "$RAIZ/marketing/capturas/capturar.sh" "$L" > "/tmp/captura-$L.log" 2>&1
    n=$(ls "$RAIZ/marketing/screenshots/$L" 2>/dev/null | wc -l | tr -d ' ')
    if [ "$n" -ge "$ESPERADAS" ]; then echo "✓ $L ($n capturas, intento $intento)"; break; fi
    echo "✗ $L intento $intento: $n capturas — $(grep -E 'FAILED|booted' "/tmp/captura-$L.log" | head -2 | tr '\n' ' ')"
    ps -ef | grep -E "[m]aestro.cli|[t]est-without-building" | awk '{print $2}' | xargs kill -9 2>/dev/null; sleep 5
  done
done
