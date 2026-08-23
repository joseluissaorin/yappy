# Relanzamiento de Yappy: plan de obra y estado

Rama: `relanzamiento`. Este documento es el mapa del relanzamiento completo
(agosto 2026) y se actualiza al cerrar cada tramo. La auditoría que lo origina
está resumida al final.

## Las fases

- [ ] **0 · Higiene**: commit de preservación (hecho, `7da9806` en main),
      arreglo del fallback de normalización, fuentes empaquetadas en local,
      CI con `cargo test`, appId de Maestro corregido.
- [ ] **1 · Sistema visual**: tokens `--yap-*` (papel/tinta/voz/ultramar/
      subrayador), gramática del relieve, Baloo 2 + Literata + Space Mono
      locales, el loro (criatura a dos tintas con desregistro), iconos
      regenerados, `tooling/marca` (HTML → Chrome headless → PNG).
- [ ] **2 · Móvil**: jerarquía propia (Escuchar / Biblioteca + reproductor
      persistente), deep links `yappy://` de verdad, widget escribiendo su
      clave, App Intents, Share Extension con PDF/EPUB/DOCX/imagen y cola
      `queue.json`, extracción por tipo, sesión de audio con un solo dueño,
      Now Playing arreglado, streaming primero.
- [ ] **3 · Guionizador**: el Guion como IR con spans (original ↔ hablado),
      intérprete RBNF sobre datos CLDR vendorizados (31 idiomas), clasificador
      semiótico con contexto, tablas declarativas por idioma (romanos RAE,
      fechas, horas, monedas, unidades, abreviaturas, letras), ritmo en vivo,
      karaoke por spans, tests de tabla masivos.
- [ ] **4 · Escritorio**: biblioteca como ventana principal (índice SQLite
      sobre los proyectos existentes), editor de Guion unificado (fusión de
      /document y /read), temas reales sobre los tokens, interfaz consciente
      de plataforma (nada de «for your mac» en Windows), `⌥⌘V` implementado,
      ajustes fantasma conectados o eliminados, dedupe de defuddle/extensión.
- [ ] **5 · Puente**: iroh en `yappy-core`, emparejar por QR + código corto,
      «convertir en el ordenador» con audio Opus por stream y reanudación.
- [ ] **6 · Web**: `yappy.joseluissaorin.com` en Cloudflare Pages: demo
      sonora real, guionizador en WASM, descargas por plataforma, changelog.
- [ ] **Final**: README nuevo, build macOS verificado, build iOS simulador,
      merge a main.

## Decisiones de diseño (cerradas)

### Paleta: dos tintas sobre papel

La metáfora: la tinta del texto (ultramar) y la tinta de la voz (coral); el
desregistro entre ambas es el producto. El subrayador dorado sigue a la voz.

| Token | Claro | Oscuro | Uso |
|---|---|---|---|
| `--yap-papel` | `#f7f2e7` | `#191b22` | fondo; el papel de la casa |
| `--yap-superficie` | `#fffdf7` | `#232631` | tarjetas, fichas |
| `--yap-superficie-2` | `#f3ecdb` | `#2b2f3d` | bandejas, raíles |
| `--yap-tinta` | `#2b2418` | `#f2edda` | texto |
| `--yap-tinta-suave` | `#82755a` | `#a9a48c` | metadatos |
| `--yap-voz` | `#e0502a` | `#ff7a4d` | primaria: teclas, reproducción |
| `--yap-voz-tecla` | degradado `#f4682e→#e0502a` | ídem aclarado | el relieve de tecla |
| `--yap-ultramar` | `#2f4bc4` | `#93a5ff` | lo textual: capítulos, enlaces |
| `--yap-subrayador` | `rgba(232,180,26,.42)` | `rgba(232,180,26,.34)` | karaoke |
| `--yap-ok` / `--yap-peligro` | `#3d8b40` / `#b3261e` | aclarados | estados |
| `--yap-borde` | `#e6dcc6` | `#3a3d4a` | hairlines |

Gramática del relieve (heredada de Andarama, reescrita para Yappy): lo que se
pulsa sube (filo de luz arriba, sombra corta), lo que recibe contenido se
hunde. Clases `.yap-tarjeta`, `.yap-tecla`, `.yap-hueco`, `.yap-ficha`,
`.yap-pestanas`, `.yap-buscador`. Grano de papel `feTurbulence` a alfa 0.04,
apagado en oscuro. `prefers-reduced-motion` respetado en todo.

### Tipografía

- Interfaz: **Baloo 2** variable (OFL, local).
- Lectura (el guion): **Literata** variable (OFL, local; subsets latin,
  latin-ext, cyrillic, greek, vietnamese para cubrir los 31 idiomas).
- Datos: **Space Mono** (OFL, local).
- Se elimina el `@import` de Google Fonts (contradicción con local-first).

### La criatura

**El loro.** Razón: «yap» es parloteo; el loro repite en voz alta lo que le
llega, en el idioma que le enseñes (31 idiomas). Dibujo: blob a la manera de
la criatura de Andarama, tinta plana ultramar con desregistro coral, cara de
tres trazos, rotado ~-3°. SVG a mano (~2 KB), nada generado.
Comportamientos: canta notas mientras suena audio; camina sobre la barra de
progreso (su posición es el playhead); cameo ocasional; estados vacíos con
viñetas propias (percha vacía, página arrugada, huevo incubando = descarga
del modelo). Icono: cuadro coral, esquinas ~23 %, loro encima.

### Jerarquía móvil

Dos secciones (Escuchar, Biblioteca) + reproductor persistente + engranaje.
«Escuchar»: sigue-donde-ibas + cola con estados + botón «+» (pegar enlace /
portapapeles / abrir archivo). «Biblioteca»: documentos con progreso,
audiolibros, transcripciones, búsqueda. Transcribir deja de ser pestaña: el
audio compartido es una fuente más de la cola. Voces se elige en contexto.
Rutas: grupo `(movil)` propio; `(app)` queda para escritorio; el layout raíz
redirige por plataforma.

### El Guion (IR del guionizador)

```rust
Guion { fuente, idioma_base, piezas: Vec<Pieza> }
Pieza { clase /* titulo1..6, parrafo, cita, lista, separador, verso */,
        texto_original, spans: Vec<Span>, idioma,
        pausa_antes_s, mult_velocidad, voz_override }
Span  { rango_original: Range<usize> /* chars */, texto_hablado, clase }
```

El audio se mapea a spans y los spans al original: karaoke indestructible.
La verbalización anota, no destruye. Capas: idioma por frase (whatlang →
lingua-rs opcional) → clasificador semiótico con contexto → RBNF/CLDR
(cardinal + ordinal con género) → tablas por idioma → tests de tabla.
Los datos CLDR (rbnf XML) se vendorizan en `crates/yappy-core/data/rbnf/`.

### Puente

iroh 1.0 (QUIC, clave pública, relays de n0). Emparejar: QR con
`yappy://pair?nodo=…&token=…` + código corto tecleable. El token, al llavero.
Protocolo en `yappy-core` (posiblemente crate `yappy-puente`), audio de
vuelta en Opus por el mismo stream, reanudable.

## Estado de la auditoría (23-08-2026, resumen)

Lo sólido: yappy-core (supertonic port, chunker, playback con karaoke por
muestras reales), audiobook.rs (átomo chpl), os_win.rs, y en iOS más de lo
esperado (Share Extension embebida, Now Playing completo, keepalive,
biblioteca .m4b, Live Activity escrita).

Lo roto o fantasma: normalización solo es/en con fallback que corrompe los
otros 29 idiomas (romanos vs. MAYÚSCULAS: «EL CID» → «EL 599»; ordinales:
«21º» → «2primero»; regla del % muerta); ritmo markdown solo en export;
karaoke roto al normalizar (busca substring); deep links sin handler (Quick
Actions, widget y Spotlight solo abren la app); widget lee `last_played_title`
que nadie escribe; FluidAudio enlazado y muerto mientras el ASR descarga
670 MB de ONNX; sin CFBundleDocumentTypes; tres sitios peleando por
AVAudioSession; título de Now Playing del documento equivocado; temas y ~10
ajustes sin conectar; `⌥⌘V` anunciado en tres sitios y sin implementar;
fuentes desde Google CDN; defuddle.js ×4; /read duplica /document;
Onboarding.svelte:33 con path absoluto de esta máquina; 6 tests en 21 k
líneas y sin CI; Maestro con appId desactualizado.

Andarama (referencia visual) vive en `Dev/ull360`: tokens en
`packages/ui/src/theme.css`, el relieve en `apps/studio/src/index.css`,
la criatura en `packages/ui/brand/anda-criatura.svg` y `Criatura.tsx`,
fuentes en `packages/ui/brand/fonts/`, marca en `tooling/marca/`.
