#!/bin/zsh
# Convierte una secuencia PNG en vídeo.
#   ensambla.sh <secuencia/> <salida.mov|mp4> [fps=30] [--alfa] [--audio pista.wav] [--audio-en segundos]
# --alfa → ProRes 4444 con canal alfa (para poner encima de otra cosa en Resolve)
# sin --alfa → H.265 10 bits (hvc1) con fondo opaco, listo para ver.
set -e
SEQ=$1; OUT=$2; FPS=${3:-30}; shift 3 2>/dev/null || shift $#
ALFA=0; AUDIO=""; AUDIO_EN=0
while [[ $# -gt 0 ]]; do
  case $1 in
    --alfa) ALFA=1;;
    --audio) AUDIO=$2; shift;;
    --audio-en) AUDIO_EN=$2; shift;;
  esac; shift
done
EXT=png; ls "$SEQ"/00000.jpg >/dev/null 2>&1 && EXT=jpg
IN=(-framerate $FPS -i "$SEQ/%05d.$EXT")
GRANO=""; [[ -n $YAPPY_GRANO ]] && GRANO="-vf noise=alls=${YAPPY_GRANO}:allf=t+u"
if [[ -n $AUDIO ]]; then
  MS=$(python3 -c "print(int(float('$AUDIO_EN')*1000))")
  IN+=(-i "$AUDIO" -filter_complex "[1:a]adelay=${MS}|${MS},apad[a]" -map 0:v -map "[a]" -shortest -c:a aac -b:a 256k)
fi
if [[ $ALFA == 1 ]]; then
  ffmpeg -v error -y "${IN[@]}" -c:v prores_ks -profile:v 4444 -pix_fmt yuva444p10le -vendor apl0 "$OUT"
else
  ffmpeg -v error -y "${IN[@]}" $GRANO -c:v hevc_videotoolbox -b:v 60M -tag:v hvc1 -pix_fmt p010le \
    -color_primaries bt709 -color_trc bt709 -colorspace bt709 -movflags +faststart "$OUT"
fi
echo "✓ $OUT"
