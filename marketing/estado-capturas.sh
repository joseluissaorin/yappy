#!/usr/bin/env bash
# Cuántas capturas hay por idioma (iPhone e iPad) y cuáles faltan.
cd "$(dirname "$0")"
for f in copy/*.json; do L=$(basename "$f" .json); a=$(ls screenshots/$L 2>/dev/null | wc -l | tr -d ' '); b=$(ls screenshots-ipad/$L 2>/dev/null | wc -l | tr -d ' '); printf "%-3s iphone:%2s ipad:%2s %s\n" "$L" "$a" "$b" "$([ "$a" -ge 27 ] && echo ok || echo FALTA)"; done
