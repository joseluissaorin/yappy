# El taller de la App Store

Capturas, ficha y subida a App Store Connect de Yappy en 31 idiomas, sin
tocar Figma: el simulador hace las fotos, `render.mjs` las viste con el
papel de la casa y `scripts/asc-helper.mjs` las sube.

## Piezas

| Qué | Dónde |
|---|---|
| Textos de muestra que se pegan en la cinta (3 por idioma) | `textos/<idioma>.json` |
| Flujos Maestro por idioma (se generan, no se editan) | `capturas/generar-flujos.mjs` → `capturas/flows/<idioma>/` (`--ipad` → `flows-ipad/`) |
| Runner de un idioma / lote con reintento / solo el tramo final | `capturas/capturar.sh`, `capturas/capturar-lote.sh`, `capturas/capturar-resto.sh` |
| Capturas crudas | `screenshots/<idioma>/NN-*.png` (27 por idioma), `screenshots-ipad/` |
| Títulos y pegatinas por idioma | `copy/<idioma>.json` |
| Composición (marco de Apple, pegatinas, loro) | `layouts/iphone.json`, `layouts/ipad.json`, `render.mjs` |
| Salida por locale de ASC | `output/<locale-asc>/{iphone,ipad}/NN.png` (`render-todo.sh`) |
| Rejilla de revisión | `grid.mjs <idioma> [iphone|ipad]` → `output/_rejillas/` |
| Ficha de la App Store (nombre, subtítulo, descripción, palabras clave…) | `metadata/<locale-asc>/*.txt` |

## El circuito

```bash
node marketing/capturas/generar-flujos.mjs            # flujos iPhone
marketing/capturas/capturar-lote.sh es en fr …        # capturas (≈10 min por idioma)
marketing/render-todo.sh iphone                       # componer todos los idiomas
node scripts/asc-helper.mjs push-metadata 0.3.0       # ficha (32 storefronts)
node scripts/asc-helper.mjs push-screenshots 0.3.0    # capturas
node scripts/asc-helper.mjs listing-status
```

El simulador de capturas es propio («Yappy Capturas», UDID en
`/tmp/yappy-sim-capturas.txt`); si se corrompe, se borra y se recrea
(`simctl create` + `simctl install` del `.app` de `gen/apple/build/arm64-sim`;
el modelo de voces baja solo del espejo). Maestro no admite dos flujos a la
vez en el mismo Mac: iPhone e iPad van en serie.

## Los diez slots

1-2 abanico: la cinta con tres pegatinas y el lector con karaoke · 3 los diez
pájaros · 4 «Nada sale de tu iPhone» (idiomas) · 5 la boca «+» · 6 el lector
con números · 7 «Y te lo llevas puesto» (audiolibros) · 8 la trastienda de
noche · 9 «Hola, soy tu loro» · 10 «Dale cuerda al loro».
