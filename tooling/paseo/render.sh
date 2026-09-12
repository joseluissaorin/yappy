#!/bin/zsh
# Renderiza loro-comparte.mp4 (720×720, 24 fps, 6 s, sin audio, bucle limpio)
# a partir de loro-comparte.html, fotograma a fotograma con Chrome headless.
#   uso: ./render.sh                 → renderiza los 144 fotogramas y monta el mp4 + png
#        ./render.sh 2.35            → un fotograma de prueba en /tmp/frames/prueba.png
#        ./render.sh --foto f.png t  → (interno) un fotograma
# Chrome --headless=new en macOS NO sale solo tras --screenshot: se espera a que
# el png esté escrito y estable, y se le mata.
set -uo pipefail
AQUI="$(cd "$(dirname "$0")" && pwd)"
HTML="$AQUI/loro-comparte.html"
RAIZ="$(cd "$AQUI/../.." && pwd)"
SALIDA="$RAIZ/yappy-app/resources/paseo"
FRAMES=/tmp/frames
FFMPEG=/opt/homebrew/bin/ffmpeg
FFPROBE=/opt/homebrew/bin/ffprobe
N=144   # 24 fps × 6 s; el fotograma 144 cae en t=6 ≡ t=0 (primero y último idénticos)

# El navegador: Chrome, Chromium o Vivaldi (si no hay ninguno: npx playwright).
CHROME=""
for c in "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \
         "/Applications/Chromium.app/Contents/MacOS/Chromium" \
         "/Applications/Vivaldi.app/Contents/MacOS/Vivaldi"; do
  [[ -x "$c" ]] && { CHROME="$c"; break; }
done
[[ -n "$CHROME" ]] || { echo "No hay Chrome/Chromium/Vivaldi; prueba con npx playwright" >&2; exit 1; }

foto() { # foto <png> <t>
  local png="$1" t="$2" dir="/tmp/paseo-chrome-$$-$RANDOM$RANDOM"
  rm -f "$png"
  "$CHROME" --headless=new --disable-gpu --no-first-run --no-default-browser-check \
    --user-data-dir="$dir" --force-device-scale-factor=1 \
    --screenshot="$png" --window-size=720,720 --hide-scrollbars --virtual-time-budget=1000 \
    "file://$HTML?t=$t" >/dev/null 2>&1 &
  local pid=$! i=0 a b
  while (( i < 300 )); do
    if [[ -s "$png" ]]; then
      a=$(stat -f %z "$png"); sleep 0.25; b=$(stat -f %z "$png")
      [[ "$a" == "$b" ]] && break
    fi
    sleep 0.1; (( i++ ))
  done
  kill "$pid" 2>/dev/null; wait "$pid" 2>/dev/null
  rm -rf "$dir"
  [[ -s "$png" ]]
}

mkdir -p "$FRAMES" "$SALIDA"
if [[ "${1:-}" == "--foto" ]]; then foto "$2" "$3"; exit $?; fi
if [[ $# -ge 1 ]]; then foto "$FRAMES/prueba.png" "$1"; echo "$FRAMES/prueba.png (t=$1)"; exit 0; fi

setopt +o nomatch
rm -f "$FRAMES"/f_*.png 2>/dev/null
# 6 fotogramas en paralelo; t = (i-1)·6/143 para que f_144 sea t=6 ≡ t=0
seq 1 $N | xargs -P 6 -I{} zsh -c 'i={}; "'"$0"'" --foto "'"$FRAMES"'/f_$(printf %03d $i).png" "$(printf %.5f $(( (i - 1) * 6.0 / 143 )))"'
n=$(ls "$FRAMES"/f_*.png 2>/dev/null | wc -l | tr -d " ")
[[ "$n" -eq "$N" ]] || { echo "faltan fotogramas: $n/$N" >&2; exit 1; }

"$FFMPEG" -y -loglevel error -framerate 24 -i "$FRAMES/f_%03d.png" \
  -c:v libx264 -pix_fmt yuv420p -profile:v main -crf 18 -movflags +faststart -an -t 6 \
  "$SALIDA/loro-comparte.mp4"
# El fotograma representativo (t≈2,95 s): flecha dibujada, icono asentado, pico abierto.
cp "$FRAMES/f_071.png" "$SALIDA/loro-comparte.png"
"$FFPROBE" -v error -show_entries stream=codec_type,codec_name,profile,width,height,r_frame_rate,duration,nb_frames -of compact=p=0 "$SALIDA/loro-comparte.mp4"
ls -la "$SALIDA"
