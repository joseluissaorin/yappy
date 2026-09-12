# El paseo del loro: el onboarding y lo que mide

Catorce páginas del álbum en cinco actos. Cada una enseña algo, hace algo
o pregunta algo que cambia la app; el loro lo dice todo en voz alta (canal
de efectos, `decir_cmd` con velocidad e idioma) y camina por el borde
inferior de página en página: su sitio es el progreso. Saltar siempre se
puede («Luego»); el paseo se reanuda donde se dejó (`settings.paseo.paso`).

| # | Paso | Tipo | Efecto real |
|---|---|---|---|
| 1 | Hola | voz | confianza: sin cuenta, sin correo, sin nombre |
| 2 | Todo en tu teléfono | juego | sellos de idioma: tocar oye, dos veces elige (UI + lectura) |
| 3 | Elige tu pájaro | acción | voz + tinta (muestras empaquetadas, sin motor) |
| 4 | ¿Qué me vas a dar? | pregunta múltiple | decide la lección de compartir |
| 5 | ¿Cuándo me escuchas? | pregunta | «en la cama» enciende el tema noche |
| 6 | ¿A qué velocidad? | acción con demo | el deslizador se oye en vivo; fija `speed` |
| 7 | ¿Cuánto tienes por leer? | pregunta con cuenta | el valor, dicho con aritmética honesta |
| 8 | Lo tuyo, o un cuento | acción | enlace copiado (sin leer el portapapeles) o cuento de la casa (no ocupa percha) |
| 9 | La pantalla es el botón | lección en el cartel | tres casillas que se marcan solas (pausar, seguir, saltar) |
| 10 | Compartir es todo | acción con PiP | la hoja de compartir DIBUJADA, el loro sale contigo a Safari; se completa sola al llegar la pieza |
| 11 | Y te lo llevas puesto | información | la imprenta: audiolibro con capítulos, y de la Biblioteca a Archivos o Libros |
| 12 | Fíjame en favoritos | acción opcional | la hoja de compartir dibujada |
| 13 | Tu loro | revelación | la ficha cosida con lo elegido y lo escuchado |
| 14 | Tres a la vez | información | la percha explicada antes de cobrar |
| 15 | La cuerda | paywall | motivo «paseo», cabecera viva si el loro está leyendo, salida «empezar con tres en la percha» |

El paso 1 dice en claro QUÉ hace la app y QUÉ traga (artículos, PDF, EPUB,
Word, notas de voz, YouTube, texto copiado); el paso 4 lo repite como
susurro sobre las pegatinas. La lección del paso 9 enseña los CINCO mandos
reales del cartel, no una versión simplificada: tocar (pausa), la tecla
grande (seguir), el mando (frase), mantener el mando o deslizar (párrafo)
y el borde derecho (velocidad).

Después, el paseo sigue en la cinta como avisos del loro (una vez cada
uno): segunda apertura sin compartir, libro largo (pide la imprenta),
tercera apertura (favoritos). Repetible desde la trastienda.

## Dónde vive

- `settings.rs::Paseo` (la libreta) · `$lib/paseo.svelte.ts` (estado,
  voz, avisos) · `(movil)/paseo/+page.svelte` (las páginas) · lección de
  gestos en `read/+page.svelte` · cabecera viva en `Paywall.svelte`.
- Nativo: `Paseo.swift` (`UIPasteboard.hasURLs` sin aviso de pegado;
  `AVPictureInPictureController` con arranque automático al irse a otra
  app). Vídeo mudo: `resources/paseo/loro-comparte.mp4`
  (`tooling/paseo/render.sh`).
- Cuentos de la casa: `resources/cuentos/*.md` con cabecera YAML
  (`titulo`, `autor`, `idioma`); `cola_agregar_cuento_cmd` los mete en la
  cinta con `de_la_casa: true`. Recomendados en la web:
  `yappy.joseluissaorin.com/cuentos/` y `/en/stories/`.

## Lo que mide (estadísticas anónimas, Cloudflare propio)

Worker `analytics-proxy` + D1 de la casa (skill `analytics`), ficha de
ingesta `yappy` (solo escribe). Cliente en `$lib/analitica/`, apagable
desde la trastienda (`paseo.estadisticas`). Nunca viaja texto, título,
enlace ni nombre. Eventos:

`app_open` · `$screen paseo_<paso>` · `paseo_respuesta{paso,valor}` ·
`paseo_idioma` · `paseo_voz` · `paseo_velocidad` · `paseo_escucha{fuente}`
· `paseo_gesto{gesto}` · `paseo_recomendado_abierto` · `paseo_pip` ·
`paseo_compartido` · `paseo_favoritos` · `paseo_saltado{paso}` ·
`paseo_cuerda_vista` · `paseo_terminado{como,paso}` · `paseo_repetido` ·
`paywall_shown{source}` · `paywall_plan_selected{plan}` ·
`paywall_cta_tapped{plan,source}` · `purchase_completed{plan,source}` ·
`purchase_cancelled` · `purchase_failed` · `paywall_dismissed{source}` ·
`restore_tapped` · `percha_llena` · `aviso_cerrado{aviso}` ·
`$identify` (el id anónimo de RevenueCat, para unir paseo y cuerda).

Leer el embudo:
`curl "$EP/funnel?app=yappy&steps=paseo_hola,paseo_escucha,paseo_compartido,paywall_shown,purchase_completed" -H "Authorization: Bearer $(cat ~/.analytics/admin.token)"`
(`$screen` va como `%24screen`).

## Verificado (9-09-2026, simulador iPhone 17 Pro Max)

Flujos `maestro/flows/paseo.yaml` (los catorce pasos, 17 capturas en
/tmp/paseo) y `paseo-trastienda.yaml`. Con la app recién instalada y
`SIMCTL_CHILD_YAPPY_TIENDA=sim`:

- Los catorce pasos pasan sin fallos. La libreta guardada al terminar:
  `voz James`, `que [articulos, libros]`, `cuando cocinando`,
  `cuanto montana`, `gestos [pausar, seguir, saltar]`, `segundos 59`,
  `hecho true`, y `first_launch_done` cerrado.
- El cuento «A la deriva» suena con karaoke desde la página 8 y sigue
  sonando bajo el paywall: la cabecera viva enseña al loro hablando con
  la frase del karaoke y «Te está leyendo "A la deriva"».
- El cuento de la casa NO ocupa percha (el resumen dice 0/3).
- Las voces se descargan solas del espejo antes del primer paso.
- Estadísticas: 144 eventos en el worker propio tras las pruebas, con el
  embudo `app_open → paseo_voz → paseo_escucha → paywall_shown` en
  5/5/5/4 usuarios y ninguna compra (correcto: el guion nunca compra).

### Trampas cazadas

- Guardar los ajustes ENTEROS desde el paseo pisaba lo cambiado por el
  camino (la voz volvía a «Alex»): por eso existe `set_paseo_cmd`, que
  solo toca `paseo`.
- El `$effect` del paso de la cuerda debe leer `compras.abierto` SIEMPRE
  (si solo se lee dentro de una rama, Svelte no lo toma por dependencia
  y el paseo se queda colgado al cerrar el paywall).
- En las pruebas, la pantalla solo pausa lo que YA suena: el guion tiene
  que esperar a que la voz arranque antes de los gestos.

### Pendiente

- El vídeo del loro en PiP se ve en la página, pero el PiP de verdad
  (flotando sobre Safari) solo se puede comprobar en un iPhone.
- Falta el plural de «1 piezas» en el membrete de la cinta (necesita una
  clave nueva en los 31 diccionarios).
- Los cuentos: solo hay uno por idioma (Quiroga en español, Chopin en
  inglés). Los de José Luis van en `resources/cuentos/`.


## La pasada de claridad y controles (9-09-2026, tarde)

Cuatro quejas de José Luis, las cuatro atendidas:

1. **El paseo no explicaba nada.** Ahora el paso 1 dice qué hace y qué
   admite, el paso 4 lista los formatos, el paso de compartir lleva la
   hoja de compartir dibujada con Yappy señalado, y hay un paso NUEVO
   («Y te lo llevas puesto») que explica la imprenta y cómo sale el
   audiolibro a Archivos o a Libros.
2. **El paywall se salía.** Ya no rota mientras se arrastra, el telón
   recorta, y la hoja se cierra de tres maneras: el tirador (franja
   superior con `touch-action: none`, si no el navegador se queda el
   gesto para el scroll), tocando fuera, y la tecla de cerrar.
3. **Los controles.** El dial de velocidad ya no se arma al pulsar
   botones o abrir el taller (`sobreUnControl`), y pide 28 px de arrastre
   vertical con dirección clara antes de aparecer. El mando usa
   `setPointerCapture`: sin ella, mover el dedo dos píxeles fuera de la
   tecla se comía el `pointerup` y el salto no ocurría (el mando
   «inestable»); mantener pulsado ahora repite el salto de párrafo.
4. **La voz se paraba a mitad de párrafo.** Bug del motor, el más grave:
   `chunker::split_too_long` RECONSTRUÍA el texto (partía por comas,
   recortaba y volvía a unir con «, »), y `guion::trocear`, que localiza
   cada trozo buscándolo literal en el hablado, no lo encontraba y lo
   DESCARTABA en silencio. Ahora el troceador devuelve rebanadas exactas
   por índice y `trocear` nunca descarta: si no localiza un trozo, lo
   emite igual. Pruebas de regresión en `chunker::tests_largos` y
   `guion::tests_voz_entera`; con el troceador viejo fallan cinco.
