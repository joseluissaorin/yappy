#!/usr/bin/env bash
# Renderiza (iPhone) cada idioma en cuanto tiene sus 27 capturas y no tiene salida aún.
cd "$(dirname "$0")"
while true; do
  for f in copy/*.json; do
    L=$(basename "$f" .json)
    n=$(ls screenshots/$L 2>/dev/null | wc -l | tr -d ' ')
    if [ "$n" -ge 27 ] && [ ! -f "output/_rejillas/$L-iphone.png" ]; then
      sleep 20  # que la última captura esté escrita del todo
      node render.mjs "$L" > /dev/null 2>&1 && node grid.mjs "$L" iphone > /dev/null 2>&1 && echo "render $L $(date +%H:%M)"
    fi
  done
  sleep 60
done
