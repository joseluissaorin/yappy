# El loro no sabe leer · Guía de producción

*16 de septiembre de 2026. Vídeo de lanzamiento de Yappy (20:00). Rodaje en una hora, montaje hoy, publicación mañana 17. Todo lo que se cita vive en `Dev/Yappy/marketing/lanzamiento/` (Dropbox) salvo el guion, que sigue en Ulysses → Vídeos → Yappy lanzamiento.*

---

## 0 · Lo que ya está hecho esta mañana

| Qué | Estado | Dónde |
|---|---|---|
| **Yappy 1.0.0 en la App Store** | Publicada a las 9:30 (`READY_FOR_SALE`). Apple tarda entre una y veinticuatro horas en propagarla a todas las tiendas. | https://apps.apple.com/app/id6773110015 |
| **Pegatina «ya disponible»** (la del título animado, antes «muy pronto») | Renderizada, ES y EN, 13 s, 2880×2160, 30 fps, con alfa. | `out/ya-disponible-es-alfa.mov` · `out/out-now-en-alfa.mov` · previsualizaciones `out/*-preview.mp4` |
| **«Media voz» leído por la app** | Sintetizado: Emily narra, Sarah dice los versos de Lorca. 3:53. | `voz/media-voz.wav` · partitura `voz/tiempos.json` |
| **«Media voz» animado** | Rodado en 4K con la voz mezclada (ver §5). | `out/media-voz.mp4` |
| **Anuncio de 52 s** (el teaser con el final «ya disponible» sobre un mosaico de sus planos) | Hecho y revisado con Gemini. | `out/anuncio-ya-disponible-es.mp4` |
| **Lo que lee la app** (Ibn Zamrak, Agustín, la fecha en seis lenguas, los commits, «El cable tiraba…») | Sintetizado. | `voz/app/*.wav` |
| **Milán, 384** (0b y 6) y los **rótulos con alfa** (nota de rescate, QUIERO LEER · Y MOVERME · A LA VEZ, sellos «15 · IX · 2026», «LA PRUEBA DE FUEGO», «Milán · año 384») | Hechos. | `out/milan-acto1-0b.mp4` · `out/milan-acto2-6.mp4` · `out/rotulo-*-alfa.mov` |
| **Web** | Los enlaces de TestFlight apuntan ya a la App Store; desplegada y comprobada. | yappy.joseluissaorin.com |
| **Guion** | El del 13-09 (segunda versión) es el bueno. Dropbox está sincronizado. | Ulysses · `GUION · El loro no sabe leer.md` |
| **Voz en off numerada** | 61 tomas, 2.684 palabras, unos 20 min leídos. | `GUION-VOZ-EN-OFF.md` (también en Ulysses) |

## 1 · Lo que cambia respecto al guion del 13

El guion pedía la Alhambra, el autobús del Albaicín, dos actrices, una señora y un niño. Nada de eso cabe en una hora. La estructura, el texto y los tiempos **no cambian**; cambia el registro de varios capítulos:

| Cap. | En el guion | Hoy |
|---|---|---|
| 0 · La Alhambra habla | película, Granada | **teatro de papel animado**: la banda epigráfica dibujada a dos tintas, la cámara la recorre de derecha a izquierda, el poema en karaoke. Lo hago yo. Tu voz no cambia una palabra («aquí, delante de la pared» funciona igual sobre el dibujo). |
| 0b y 6 · Milán, 384 | teatro de papel | igual: **animado**, mismo motor que el cuento. |
| 1 · Flexiones | seis viñetas por Granada | **tres viñetas en casa** (suelo, cocina, auriculares). Los tres rótulos se ponen en montaje. |
| 4 · La prueba de fuego | película + títeres | las dos viñetas se ruedan en casa (gafas; mano con la pegatina). Los títeres, animados. |
| 5 · Media voz | rodaje en el autobús | **animado, dibujado a mano en JavaScript**, leído por la app. No hay nada que rodar. |
| 0c y 7 · Tú a cámara | mirador, tarde y noche | en casa, **mismo encuadre**, una con luz de día y otra con la persiana bajada y una lámpara. |
| 2 y 3 · La mesa | mesa de collage (manim) | **tu mesa real**, tus manos, los papeles escritos a mano. Es más verdad y no depende de nadie. Los sellos, la nota de rescate, el teletipo y el diagrama de cinco cajas los pongo yo encima con alfa. |

## 2 · Retoques de texto antes de grabar

Cuatro tomas tienen fechas que ya no cuadran con publicar el 17. Lee la versión corregida (está así en `GUION-VOZ-EN-OFF.md`):

- **16**: «La presentación de resultados **era** el quince de septiembre. Este vídeo es la presentación de resultados, con dos días de retraso.»
- **45**: «Y **hace unos días**, un script mío de traducciones…»
- **53**: «Se quitó bastante: una biblioteca entera, **la semana pasada**.»
- **60**: «Lo que sí sé es lo que viene. **Ya está en la App Store, desde hoy.** Android, en cuanto Google apruebe la cuenta; el escritorio después; el código, abierto, cuando esté a la altura. Y esta noche…» Solo si de verdad vas a dejar el Mac imprimiendo la primera tanda esta noche. Si no, quita la frase entera de la biblioteca pública: no prometas lo que no se ve.
- **61**: el poema es un borrador para medir. Si tienes uno tuyo, léelo; si no, lee el borrador tal cual, funciona.

## 3 · El rodaje: sesenta minutos

**Antes de arrancar el reloj** (diez minutos que no cuentan): escribe a rotulador, en siete hojas A4, los títulos PLAN VERDAD · EL JUGUETE · EL ÁLBUM DE LA BANDADA · EL PASEO DEL LORO · LAS VOCES SIN MURO · YAPPY PARLANCHÍN · RELANZAMIENTO. En otra hoja, la cuenta a mano: `2,1 + 18,6 + 128,5 + 50,8 = 200 MB`. Imprime (o escribe) un párrafo cualquiera, córtalo con tijera por la mitad y pégalo con celo. Ten a mano: un libro de papel, las gafas, la olla, una cuchara, **auriculares de cable** (los del cuento), el iPhone con Yappy, el Mac con la cola de la imprenta llena. Modo avión en el teléfono con el que grabas. Objetivo limpio.

**Cámara**: 4K a 25 fps, horizontal, exposición y foco bloqueados en cada plano (mantén pulsado en la pantalla). Nunca zoom digital. Los planos de manos, muy cerca. Cada clip, cinco segundos de más al principio y al final.

| min | qué | cómo | tomas |
|---|---|---|---|
| 0–5 | **Voz en off, calentamiento** | Micro a un palmo, en la habitación con más tela. Graba 10 s de silencio de sala. Lee la toma 01 tres veces hasta encontrar el tono: bajo, como quien señala algo. | ninguna |
| 5–35 | **Voz en off, las 61 tomas** | Se lee de arriba abajo de `GUION-VOZ-EN-OFF.md`, en un solo fichero o por capítulos. Antes de cada toma di el número («dieciséis») y deja un segundo. Si te equivocas, no pares: repite la frase entera desde el principio y sigue. Respeta las pausas marcadas ⏸ callándote de verdad. Las tomas 11-12 y 57-59 se graban también a cámara luego, pero grábalas aquí igualmente por si el sonido de cámara no vale. | 61 |
| 35–43 | **Tú a cámara: 0c y 7** | Plano medio corto, cámara a la altura de los ojos, tú ligeramente descentrado, fondo con profundidad (una puerta, una estantería lejos). Primero **0c** con luz de día: tomas 11 y 12, con los dos segundos de silencio entre ellas, mirando a cámara. Luego baja la persiana, enciende una lámpara lateral y graba **7**: tomas 57, 58, 59, mismo encuadre exacto. Dos veces cada bloque. | 4 |
| 43–49 | **Flexiones** (mudo) | 1) Suelo: libro abierto debajo, bajas, lees una línea, subes; a la cuarta la cabeza cae sobre la página. 2) Cocina: el libro apoyado contra la olla con una cuchara, hierve, salpica. 3) Cierras el libro, te pones los auriculares, te vas de espaldas por una puerta. Cámara fija en los tres. | 3 |
| 49–53 | **La prueba de fuego** | 1) La mesilla: mano que se quita las gafas, las deja, el teléfono queda boca abajo; que se vea que sigue sonando (súbele el volumen a Yappy). 20 s sin cortar. 2) Un dedo (el tuyo sirve, o el de un niño si lo hay) tira de una pegatina de la percha y la suelta; que rebote. | 2 |
| 53–58 | **El taller** | Sobre la mesa, desde arriba o en picado: a) las siete hojas caen una a una y la mano las abanica; b) la hoja de la cuenta, la mano la alisa; c) el párrafo con celo, de cerca; d) el Mac con la cola de la imprenta llena, en un plano de cinco segundos, y otro de la pantalla más cerca. | 5 |
| 58–60 | **Material de reserva** | Un plano de tus manos quietas sobre la mesa vacía, 15 s. Es el papel en blanco del vídeo: se usa siempre. | 1 |

**Fuera de la hora, desde el sofá, diez minutos**: grabación de pantalla del iPhone (Ajustes → Centro de control → Grabar pantalla), en vertical, un vídeo por gesto: la percha con arrastre, corazón, estrella y el loro que vuela · la barra de descarga con el loro comiendo (si ya está bajado, no pasa nada: se usa la del vídeo 2) · compartir desde Safari hasta que aparece la pegatina · el cartel en «preparando» y sonando · el karaoke leyendo «El cable tiraba un poco cada vez que respiraba.» (pega la frase en una nota y compártela) · la hoja de encargo con capítulos · el paseo con el loro en PiP.

**Dónde dejarlo**: `rodaje/voz/` (la voz, wav o m4a, como salga), `rodaje/planos/` (la cámara), `rodaje/pantallas/` (el iPhone). Está en Dropbox: en cuanto lo copies lo veo.

## 4 · Lo que lee la app (lo sintetizo yo)

1. La versión libre del poema de Ibn Zamrak, voz de James (grave). Con karaoke.
2. Agustín, *Confesiones* VI, 3, voz de James; suena dos veces (0b y 6).
3. La tarjeta de la fecha en es · en · fr · de · it · ja, seguidas.
4. Los cinco mensajes de commit, voz por defecto, teletipo.
5. «El cable tiraba un poco cada vez que respiraba.», Emily, karaoke grande (aparece en 3.4 y cierra el cuento: es la rima).
6. «Media voz» entero: **hecho** (`voz/media-voz.wav`).

## 5 · El cuento animado

«Media voz» se dibuja a mano en JavaScript, fotograma a fotograma, con el mismo motor que el título animado (una página HTML determinista que pinta el segundo `t`, y Puppeteer la fotografía). Papel crema, dos tintas (coral y ultramar) y una tercera, el verde de Lorca, que solo aparece cuando habla la voz del libro. Ocho escenas siguiendo la partitura de la voz: la subida (el autobús como recortable, los nombres de las calles pasando), el cable (dos nucas, un cable que no llega), la ventana (el sol en la mejilla, la bolsa de pipas), el apoyo, la parada que pasa, el secreto (el patio de segundo como recuerdo dentro de un óvalo), la última parada (el motor que se apaga: el papel deja de vibrar), y el hombro, con la última frase en karaoke a pantalla completa. Subtítulos en español integrados; en ningún plano aparece la app ni se dice «Yappy».

Salida: `out/media-voz.mp4` (3840×2160, 25 fps, con la voz mezclada) y `out/media-voz-alfa.mov` sin fondo por si lo quieres sobre otra cosa. Música: no va dentro; la pones tú en Resolve (§6).

## 6 · El montaje (esta tarde y esta noche)

**Yo**: el cuento (§5) · Milán, 384 y su vuelta en 6 · la Alhambra dibujada · los rótulos con alfa (QUIERO LEER · Y MOVERME · A LA VEZ; la nota de rescate de la prueba de fuego; el sello «15 · IX · 2026»; LA PRUEBA DE FUEGO; los papelitos tachados «servidor · cuenta · datos»; la marquesina de formatos; el diagrama de cinco cajas que se completa) · los audios de la app (§4) · el karaoke de «El cable tiraba…» · los subtítulos ES del vídeo entero (me pasas tu voz y te devuelvo el .srt en una hora) · la ficha de créditos. Todo cae en `out/` con nombre de capítulo.

**Tú, en DaVinci**: proyecto 3840×2160 a 25 fps. Primero la voz: corta las 61 tomas y colócalas con las pausas del guion; esa pista es el reloj de todo. Luego los planos en su capítulo. Los .mov con alfa se ponen en una pista superior, sin fundidos salvo el que traen. Revela en Laboratorios Saorín solo la película (tus planos): **la animación no se revela** (la realidad se revela, el papel no). Música: sin música hasta 0c; entra el funk del teaser muy bajo en 1; se va en 2; orquesta o cuerda desde el primer verso de Lorca del cuento (0:55 dentro del corto) hasta el final del 8. En `~/Music/auto-davinci/musica/cuerda-emotiva/` y `neoclasico-intimo/` hay cama con licencia. Exporta H.265 10 bits 4K, 60 Mb/s, audio AAC 256.

**Orden de prioridad si a las 23:00 falta algo**: 1) el largo con el cuento, aunque falten rótulos; 2) el anuncio de 55 s (el teaser con el final nuevo, lo monto yo hoy: `out/anuncio-ya-disponible-es.mp4`); 3) Milán animado (si no llega, la cita de Agustín va sobre papel vacío con la voz de James: funciona); 4) la Alhambra dibujada (si no llega, tu voz sobre negro y el poema en karaoke; funciona también).

## 7 · Publicar mañana, 17

1. **Web**: hecho hoy; las cuatro páginas (`/`, `/descargas`, `/en`, `/en/downloads`) enlazan a la App Store.
2. **YouTube** (mañana, cuando el máster esté subido y procesado en 4K):
   - Título: «El loro no sabe leer · Yappy, un mes después»
   - Descripción: la tesis en tres líneas + los capítulos con sus tiempos (los saco del máster) + App Store + web + créditos (la minibeca de XMihura, Supertonic 3 de Supertone, Claude de Anthropic, Ibn Zamrak versión libre, Lorca «Romance sonámbulo» 1928, Agustín *Confesiones* VI, 3).
   - Miniatura: el último fotograma de la pegatina sobre el plano del Albaicín (`out/ya-disponible-es-final.png` con alfa).
3. **X**: el anuncio de 55 s + la frase «Yappy ya está en la App Store. Lee cualquier texto en voz alta, en el teléfono, sin internet y sin cuentas.» y el hilo (la escaleta de veinte tuits está en la servilleta; el de Android del 15 ya está en `marketing/hilos/`).
4. **TikTok / Reels**: el anuncio en 9:16 queda pendiente (el título «yappy» ocupa todo el ancho del 4:3 y no sobrevive a un recorte vertical: hay que reencuadrarlo en la pegatina). Si hace falta mañana, se hace en media hora.
5. **App Store**: nada que hacer; comprobar por la mañana que la ficha aparece en `apps.apple.com` desde un teléfono sin sesión.

## 8 · Lo que dijo Gemini (visionado a 5 fps, con audio)

Cada pieza terminada se ha pasado por Gemini Pro como vídeo (cinco fotogramas por segundo, el audio dentro) con un prompt de director de animación. Las críticas completas están en `out/critica-gemini-*.md`. Lo que era un fallo real se ha corregido en la versión 2 de cada pieza; lo que sigue depende de tu montaje:

**Del cuento (5/10 en la primera versión, 6/10 en la segunda; la tercera es la que hay en `out/`):** en la segunda ronda se corrigió además la melena de ella (Gemini la tomaba por un chico y eso confundía quién habla), la bolsa de pipas que tapaba el respaldo, el poste de PARADA que pisaba el marco de la ventana, la señora que bajaba en vertical, el apoyo lineal y la respiración del final, que ahora tensa el cable de forma visible. En la primera ronda se había corregido el hervor de la línea con el motor apagado, la señora que atravesaba la chapa del autobús, los ojos gigantes del verso de la plata, el recuerdo negro de «anoche», el lavado verde demasiado fuerte, el nudo «con los dientes» que no se veía y el móvil flotando sin brazo. El karaoke final se queda (lo tomó por un error de render porque no conocía el karaoke de la app): por eso es importante que en el capítulo 3.4 se vea el karaoke real del iPhone antes del cuento, para que la rima se lea.

**De Milán (4/10 en la primera, 6/10 en la segunda; la tercera es la que hay en `out/`):** en la segunda ronda, el pivote del ladeo del loro (giraba desde la esquina, no desde el cuello), la sombra de contacto en el alféizar, la tarjeta más a la derecha y dos segundos más, los pasos de Agustín y su mirada hacia Ambrosio, la parábola del vuelo y el pico suavizado. En la primera se había corregido el vuelo del loro (arco, aleteo visible, rebote al posarse), el pico movido con la envolvente real de la voz, Agustín que entra andando y se inclina al asombrarse, la página que se dobla, la tarjeta con gravedad, las sombras coherentes con la ventana y el paisaje verde pasado a ultramar. Tu voz va encima; el acto 1 es mudo a propósito.

**Del anuncio (6,5/10):** corregido el final (mosaico de los seis planos del teaser bajo el título, en vez de la acera congelada; tres segundos más corto) y la pegatina (línea «APP STORE · iPHONE Y iPAD» grande y en tinta). Lo que no puedo tocar sin el proyecto de DaVinci, y que merece la pena si tienes veinte minutos:
- Empezar por la grabación de pantalla (compartir el Quijote) y no por el espejo: los tres primeros segundos deciden en TikTok.
- Dos rótulos gigantes nuevos entre los casos de uso: **SIN INTERNET** y **SIN CUENTAS**. Es lo que nadie más puede decir y el anuncio no lo dice.
- La palabra «artículos» del rótulo de 0:17 sale cortada por arriba.
