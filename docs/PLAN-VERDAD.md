# PLAN VERDAD: el esqueleto que no miente, y la cara que lo merece

Fecha: 2026-08-26. Objetivo: ejecutar la doctrina completa acordada con José Luis
(estabilidad y previsibilidad de la reproducción como cimiento; después el loro,
el cuerpo nativo, los mandos, el cartel, la boca, las muestras y la localización)
y subir el resultado a TestFlight.

La máxima sigue mandando: que un niño de seis quiera jugar sin escuchar nada,
y que un filólogo de sesenta la use a diario sin estorbo.

## Diagnóstico verificado en el código (2026-08-26)

- `playback.rs`: ya hay época (`session_id`) y pausa a nivel de mezclador
  (instantánea). NO hay: estado explícito (preparando/sonando/pausa/inactivo),
  revisión monótona, título/ruta en el snapshot, saltos por frase/párrafo,
  ni arranque en pausa.
- `mobile.rs` + `NowPlaying.swift`: pantalla de bloqueo cableada, PERO
  `cb_toggle` comprueba `s.playing` (que sigue `true` en pausa): reanudar desde
  la pantalla de bloqueo no funciona jamás. Saltos remotos: ±15 s (no frases).
  Sin observadores de interrupciones (llamada, auriculares fuera). Sin carátula.
- `AudioSession.swift`: categoría `.playback` + `.spokenAudio` ✓,
  `UIBackgroundModes audio` ✓. El audio en segundo plano debería funcionar;
  hay que verificarlo y blindar interrupciones.
- `sample_voice`: sintetiza EN VIVO la presentación (lenta en el móvil) y hace
  `playback.stop()`: mata la sesión del documento. Doble violación.
- `read/+page.svelte`: la pantalla-botón reanuda con cualquier toque tras cerrar
  una hoja (el «autoplay» que ve José Luis); tocar un párrafo del guion SIEMPRE
  arranca audio aunque estuvieras en pausa; el subrayado es una banda horizontal
  sobre el bloque (mentira con varias líneas); `font-variation wght` modulado
  con el nivel remaqueta el párrafo entero (el «vibrado»); el porvenir no cruza
  fronteras de párrafo y se esfuma; salir deja la sesión viva sin forma de
  matarla (no hay detener en el móvil).
- `escuchar/+page.svelte`: el riel de puntos no se lee como cinta y el parallax
  lo despega al hacer scroll; la página entera es scroller (rebota aun vacía);
  la trastienda solo se descubre llegando al final.
- `Criatura.svelte`: bola frontal con ojos. Sin pico ganchudo, sin cola, sin
  ala: no es un loro.
- `i18n.ts`: solo es/en, arranca en «en» (el «paste here»). Yappy habla 31
  idiomas (`LANGUAGES` en ipc.ts).
- `ajustes/+page.svelte`: `input[type=range]` nativo de navegador, tarjetas a
  ras de borde, sin selector de idioma de interfaz.

## F1. El motor de la verdad (Rust)

- `PlaybackSnapshot` gana: `estado` («inactivo» | «preparando» | «sonando» |
  «pausa»), `revision` (monótona, por emisión), `titulo`, `doc_path`,
  `chunks_cocinados` (cocina visible).
- Comandos nuevos del controlador: `Preparando{session_id, titulo, doc_path}`
  (lo emite `read_internal` justo tras reclamar la sesión), `Fallo{session_id}`
  (si la síntesis muere antes del primer chunk, el estado no se queda colgado
  en preparando), `SaltarChunk{delta}` (salto por frase DENTRO de lo ya
  sintetizado: instantáneo, sin resintetizar), `SaltarParrafo{delta}`,
  `NewSession{arranque_pausado}` (reposicionar desde el guion sin sonar).
- Convención musical en atrás: >1,2 s dentro de la frase = principio de la
  frase; si no, la anterior.
- Los saltos aterrizan siempre en chunks con texto real (los silencios entre
  párrafos se saltan).
- `read_document_paragraphs_cmd` gana `start_paused`.
- Tests: transiciones del estado y elección de destino de salto como funciones
  puras con tests; suite existente intacta.

## F2. El puente nativo honesto (Swift + mobile.rs)

- `cb_toggle`: mirar `paused`, no `playing`. (El bug que hacía imposible
  reanudar desde la pantalla de bloqueo.)
- Pista anterior/siguiente = frase anterior/siguiente (la MISMA semántica que
  dentro de la app), en vez de ±15 s.
- Observadores de `AVAudioSession.interruptionNotification` (llamada, Siri) y
  `routeChangeNotification` (auriculares fuera → pausa inmediata). Rearranque
  solo si el sistema lo pide explícitamente (`shouldResume`).
- Carátula en la pantalla de bloqueo: el loro nuevo.
- `yappy_efecto_play(path) -> duración`: un canal de AVAudioPlayer para sonidos
  cortos locales (muestras de voz, foley futuro) que NO toca ni la sesión de
  lectura ni el Now Playing.

## F3. El espejo (frontend): una sola verdad pintada

- `$lib/reproduccion.svelte.ts`: EL store. Recibe `playback_state`, descarta
  revisiones viejas, reconcilia con `playback_snapshot` al montar, al volver
  del fondo (`visibilitychange`) y tras navegar. Nadie más deduce.
- La aguja se muda al layout del móvil: visible en TODAS las páginas (menos
  /read) cuando hay sesión: título real, estado honesto (sonando / en pausa /
  preparando con su cuenta), botón pausa/seguir AHÍ, deslizarla = detener del
  todo (con háptica de peso), tocarla = al cartel (restaurando el documento por
  `doc_path` si el store del lector está vacío).
- Prohibición de autoplay por navegación: el audio solo nace de un gesto sobre
  un mando de reproducir (los caminos de compartir/abrir item son gestos).

## F4. El loro de verdad (Criatura 2.0)

- Silueta primero: candidatos en negro, probados a 34/84/160 px vía qlmanage;
  solo se riggea el que se lea como loro en la mancha.
- Anatomía: pico ganchudo en dos piezas (la superior NUNCA se deforma; hablar
  = cae la mandíbula inferior + la cabeza atrás un punto), anillo ocular, ala
  festoneada, tres plumas de cola, patas de dos dedos. Perfil, con `mirando`
  para girarse.
- El rig entero se conserva: estados, parpadeo doble, acicalado (ahora de
  verdad: la cabeza al ala), mirada, barriga, tintas del casting, respiración.
- Misma API de props: ningún callsite cambia.

## F5. La cinta sin riel + la casa a medida

- Fuera el riel de puntos y el parallax. Tarjetas protagonistas a todo lo
  ancho con aire.
- La página NO es scroller: cabecera fija (loro + yappy + botón de ajustes CON
  etiqueta), boca fija, y la lista como único scroller (solo si desborda,
  `overscroll-behavior: contain`).
- La boca es AÑADIR: botonazo «Pegar» (lee el portapapeles él solo), «Un
  enlace», «Un archivo». El rótulo deja de mentir.
- Barras de scroll de navegador ocultas en todo el móvil.

## F6. El cartel dice la verdad

- Fuera la modulación de `wght` (el vibrado). La reactividad vive en subrayado,
  halo y pico.
- Karaoke por palabras multilínea: la parte dicha es un span en línea cuyo
  subrayado sigue los saltos de línea; avanza palabra a palabra con el
  estimador y se recalibra con cada frase real.
- Teleprompter: el porvenir cruza fronteras de párrafo (siempre se ve lo que
  viene), y las dichas también.
- Pausar con un toque en cualquier sitio SIGUE; reanudar SOLO con la tecla
  «seguir» (se acabó el autoplay accidental tras cerrar una hoja).
- Saltos instantáneos: tocar el mando salta por frase vía motor (sin
  resíntesis) cuando el destino ya está cocinado; párrafo con readFrom.
- El guion: tocar un párrafo en pausa REPOSICIONA en pausa (no arranca);
  seguimiento automático de la fila actual con respeto al scroll manual y
  píldora «volver a lo que suena».

## F7. Mandos con oficio + trastienda que respira

- `Deslizador.svelte` propio: pulgar grande con relieve, pista con la tinta de
  la voz, burbuja de valor al arrastrar, imanes con tick en 1×/1,25×/1,5×.
  Sustituye a todos los `input[type=range]` del móvil.
- Muelles en todas las teclas: transición de transform/sombra/color con
  sobreimpulso al soltar (clase global + .pulsado ya existente).
- Trastienda: gutters de página, tarjetas despegadas, targets ≥44 pt, selector
  de idioma de interfaz.

## F8. Las muestras instantáneas

- Cocina en segundo plano: al estar el modelo listo (y sin sesión activa), se
  sintetizan las 10 presentaciones en calidad rápida y se cachean como wav en
  `app_data/muestras/`. Tocar un cromo = `yappy_efecto_play` del fichero:
  instantáneo, sin tocar la sesión del documento.
- Si aún no está cocinada: se cocina al vuelo (corta, rápida) con el loro
  pensando; nunca silencio mudo.
- `sample_for_voice` cubre los 31 idiomas.

## F9. Localización total (31 idiomas)

- `$lib/i18n/`: un diccionario por idioma hablado (en, es, fr, de, it, pt, nl,
  pl, ro, sv, da, fi, et, lt, lv, hr, sl, sk, cs, hu, el, bg, uk, ru, tr, ar,
  hi, id, vi, ko, ja). El idioma de la interfaz sigue al sistema, con override
  manual en ajustes.
- Caza de hardcodes (placeholders, aria-labels, el «https://…», textos de la
  extensión).

## F10. Verificación y lanzamiento

- Build sim (iPhone 17, iOS 26) + walkthrough con capturas por fase: cinta
  nueva, aguja con pausa desde fuera, deslizar-para-detener, cartel karaoke
  multilínea, teleprompter, guion siguiendo, loro nuevo en todos los tamaños,
  trastienda nueva, muestras instantáneas, idiomas.
- Matriz hostil con el dedo: pausar desde la cinta, matar la aguja, machacar
  adelante ×5, cambiar de documento a mitad de frase, cerrar hojas y tocar
  (NO debe reanudar), volver de ajustes (NO debe sonar nada).
- cargo test (core + app) en verde.
- Bump 0.2.0.6, build dispositivo con los rituales (rm -rf gen/apple/build,
  PATH con /usr/bin delante), subida, VALID en ASC, informe.

Pendientes que este plan NO cubre (declarados): restyle del reproductor m4b,
foley procedural (el canal efecto ya queda listo), Android rebuild, y la
verificación física en el iPhone de José Luis (háptica, bloqueo, llamadas,
segundo plano real).
