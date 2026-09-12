#!/usr/bin/env bash
# Captura las pantallas crudas de Yappy en el simulador para UN idioma.
#   marketing/capturas/capturar.sh <idioma>
# Arranque determinista: cierra la app, rearma el paseo en settings.json,
# vacía la cinta, lanza con -AppleLanguages y la tienda simulada, y
# encadena los flujos Maestro generados (paseo → 3 textos → resto).
set -uo pipefail
L="$1"
RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SIM="${SIM_UDID:-5CCF8C4F-140A-41B8-8ABA-BC68853DAD1D}"
APP=com.joseluissaorin.yappy
MAESTRO="$HOME/.maestro/bin/maestro"
FLOWS="${FLOWS_BASE:-$RAIZ/marketing/capturas/flows}/$L"
OUT="${SHOTS_BASE:-$RAIZ/marketing/screenshots}/$L"
mkdir -p "$OUT"
cd "$RAIZ"

maestro_run() { # <flow> <timeout s>
  timeout "$2" "$MAESTRO" --device "$SIM" test "$1" 2>&1 | grep -E "FAILED|WARNED|Exception|Error" | grep -v "(Optional)" | head -5
  ps -ef | grep -E "[m]aestro.cli.*$SIM|[t]est-without-building.*$SIM" | awk '{print $2}' | xargs kill -9 2>/dev/null
  sleep 2
}

echo "══ $L ══ $(date +%H:%M:%S)"
# El simulador dedicado puede aparecer apagado (otra sesión hace shutdown all): levantarlo.
if ! xcrun simctl list devices booted | grep -q "$SIM"; then
  echo "(simulador apagado: arrancando)"; timeout 100 xcrun simctl boot "$SIM" 2>/dev/null; sleep 15
fi
timeout 60 xcrun simctl terminate "$SIM" $APP 2>/dev/null; sleep 1
DATA="$(timeout 60 xcrun simctl get_app_container "$SIM" $APP data)"
SETTINGS="$DATA/Library/Application Support/$APP/settings.json"
if [ -f "$SETTINGS" ]; then
  node -e '
    const fs=require("fs");const p=process.argv[1];const s=JSON.parse(fs.readFileSync(p,"utf8"));
    s.first_launch_done=false; s.app_theme="cream"; s.voice="James"; s.speed=1.1;
    s.paseo=Object.assign(s.paseo||{},{hecho:false,paso:0,que:[],cuando:"",cuanto:"",gestos:[],compartido:false,avisos:[],aperturas:0,segundos_escuchados:0});
    fs.writeFileSync(p,JSON.stringify(s,null,2)); fs.writeFileSync(p+".bak",JSON.stringify(s,null,2));' "$SETTINGS"
else
  echo "(sin settings.json: primer arranque de verdad)"
fi
rm -rf "$DATA/Library/Application Support/$APP/cola"
# Progreso de lectura y demás rastros de la cinta anterior
rm -rf "$DATA/Library/Application Support/$APP/progreso.json"
timeout 60 xcrun simctl status_bar "$SIM" override --time "9:41" --batteryState charged --batteryLevel 100 --cellularBars 4 --wifiBars 3 >/dev/null 2>&1
SIMCTL_CHILD_YAPPY_TIENDA=sim timeout 90 xcrun simctl launch "$SIM" $APP -AppleLanguages "($L)" -AppleLocale "${L}_$(echo "$L" | tr a-z A-Z)" 2>&1 | sed 's/^/  launch: /'
sleep 8
timeout 60 xcrun simctl io "$SIM" screenshot "/tmp/yappy-shots/arranque-$L.png" >/dev/null 2>&1

maestro_run "$FLOWS/paseo.yaml" 240

for i in 0 1 2; do
  node -e 'const t=require(process.argv[1])[+process.argv[2]];process.stdout.write(t.titulo+"\n\n"+t.cuerpo)' "$RAIZ/marketing/textos/$L.json" $i | timeout 60 xcrun simctl pbcopy "$SIM"
  sleep 1
  maestro_run "$FLOWS/anadir.yaml" 90
  sleep 4
done

TITULO="$(node -e 'const t=require(process.argv[1])[2];process.stdout.write(t.titulo)' "$RAIZ/marketing/textos/$L.json" | sed 's/[.*+?^${}()|[\]\\]/\\&/g')"
sed "s|__TITULO__|$TITULO|" "$FLOWS/resto.template.yaml" > "$FLOWS/resto.yaml"
maestro_run "$FLOWS/resto.yaml" 300

timeout 60 xcrun simctl terminate "$SIM" $APP 2>/dev/null
echo "── $L: $(ls "$OUT" | wc -l | tr -d ' ') capturas $(date +%H:%M:%S)"
