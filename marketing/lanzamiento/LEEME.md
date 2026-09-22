# El taller del lanzamiento (16-09-2026)

Todo lo que se hizo para publicar Yappy 1.0.0 y el vídeo «El loro no sabe leer».
Los documentos (`GUIA-PRODUCCION.md`, `GUION-VOZ-EN-OFF.md`) tienen copia en
Ulysses → Vídeos → Yappy lanzamiento. Los renders grandes (`out/`) no van a git.

## Piezas

| Qué | Dónde | Cómo se rehace |
|---|---|---|
| Renderizador de fotogramas (Puppeteer sobre el Chrome instalado; la página pinta el segundo `?t=`) | `herramientas/fotogramas.mjs` | `node herramientas/fotogramas.mjs pagina.html out/seq --dur 13 --fps 30 [--alfa] [--jpg] [--escala 2] [--query "lang=en"]` |
| Ensamblar la secuencia (ProRes 4444 con alfa, o H.265; audio opcional; grano con `YAPPY_GRANO=5`) | `herramientas/ensambla.sh` | `herramientas/ensambla.sh out/seq salida.mov 30 --alfa` |
| Pegatina «ya disponible» / «out now» (el título animado del teaser, antes «muy pronto») | `pegatina/ya-disponible.html` (+ el original `titulo-muy-pronto.original.html`) | `?lang=es|en&fondo=negro|papel` · 2880×2160 · 30 fps · 13 s · silbido a 5,6 s |
| Cuento «Media voz», dibujado fotograma a fotograma | `cuento/media-voz.html` + `cuento/tiempos.js` | 1920×1080 de diseño, se rueda con `--escala 2` → 3840×2160 · 25 fps · `?fondo=no` para alfa · `?grano=si` solo para fotos fijas |
| La voz del cuento (Supertonic 3 vía `yappy-cli`; Emily narra, Sarah dice a Lorca) | `voz/cuento.json` → `voz/sintetiza.mjs` → `voz/media-voz.wav` + `voz/tiempos.json` | `node voz/sintetiza.mjs` (caché por hash; cambiar voz o texto y volver a lanzar) |
| Lo demás que lee la app en el vídeo (Ibn Zamrak, Agustín, la fecha en seis lenguas, los commits, «El cable tiraba…») | `voz/app/*.wav` | ver el bloque `S …` en el historial o repetir con `yappy-cli --voice James --lang es --text …` |
| Rótulos con alfa (nota de rescate, gritos QUIERO LEER · Y MOVERME · A LA VEZ, sellos) | `rotulos/rotulo.html` → `out/rotulo-*-alfa.mov` | `?tipo=rescate|grito|sello&texto=…&dur=N` · 1920×1080 de diseño, `--escala 2 --alfa` |
| Milán, 384 (dos actos: 0b con Agustín y la tarjeta; 6 con el loro que repite) | `milan/milan.html` → `out/milan-acto1-0b.mp4` (45 s) · `out/milan-acto2-6.mp4` (60 s, con la cita de James a los 22 s) | `?acto=1|2` y los momentos por URL (`entra`, `pagina`, `tarjeta`, `loro`, `habla`, `ladea`) |
| Crítica con Gemini (vídeo a N fps + audio, prompt de director; guarda la crítica en Markdown) | `herramientas/critica-gemini.py` → `out/critica-gemini-*.md` | `python3 herramientas/critica-gemini.py video-1080.mp4 --fps 5 --modelo gemini-pro-latest [--guion texto.md] [--prompt p.txt] -o critica.md` (subir la copia a 1080p, no el 4K) |
| **Vertical 9:16 (TikTok/Reels)**: el mismo anuncio sobre papel, con la marca arriba y «SIN INTERNET · SIN CUENTAS» abajo; final a sangre con la pegatina reencuadrada | `out/anuncio-ya-disponible-es-916.mp4` (1080×1920, 24 fps, 52 s) · marco `pegatina/marco-916.html` → `out/marco-916-es.png` · pegatina vertical `out/ya-disponible-es-916-alfa.mov` | la pegatina admite `?w=&h=` (todo escala con K = w/2880): `node herramientas/fotogramas.mjs pegatina/ya-disponible.html out/seq-916-es --dur 10.7 --fps 24 --alfa --w 1080 --h 1920 --query "lang=es&w=1080&h=1920"` |
| Anuncio de 50 s: el teaser con el final nuevo | `out/anuncio-ya-disponible-es.mp4` | el teaser hasta 39,67 s + mosaico 3×2 de los seis planos crudos (`out/mosaico-teaser.mp4`) + la pegatina con alfa recortada a 10,6 s; el audio del teaser con fundido |

## Trampas que ya costaron tiempo

- **feTurbulence a 4K por fotograma = 0,5 fps.** El grano del papel se pone en ffmpeg (`noise`), no en el SVG.
- **PNG a 4K en Chrome es lento**: sin alfa, `--jpg` (calidad 94) va cuatro veces más rápido.
- `crop` con `t` en la expresión falla al inicializar; para un zoom lento sobre un fotograma congelado usar `zoompan` con `d=1`.
- El contenido de un «recuerdo» (óvalo recortado) hay que **trasladarlo dentro del óvalo**: `recuerdo(cx, cy, rx, ry, contenido, k, ox, oy, escala)`.
- Puppeteer global no trae Chrome: `executablePath` apunta al Google Chrome de `/Applications` (o `CHROME=…`).
- La ficha de App Store se publica con `node scripts/asc-helper.mjs release 1.0.0` (nuevo subcomando: `appStoreVersionReleaseRequests`; solo desde `PENDING_DEVELOPER_RELEASE`).
- **Gemini como crítico**: pedir `fps=5` en `video_metadata` (por defecto muestrea a 1 fps y no ve el movimiento). Un cuento de 4 min a 5 fps son ~85k tokens. Contarle las decisiones deliberadas (voz sintética, karaoke) o las tomará por errores.
- **Un recorte central del 4:3 no sirve**: los rótulos («LEE TODO», «artículos · LIBROS · apuntes») van de borde a borde y se decapitan. Por eso el 9:16 pega el 4:3 entero sobre papel y usa las bandas para decir lo que el anuncio no decía.
- **`overlay` con `eof_action=pass` ignora `repeatlast`**: una imagen fija de un solo fotograma desaparece al instante. Para un marco estático, `-loop 1` en la entrada y `shortest=1` en el overlay.
