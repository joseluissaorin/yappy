# Qué tengo que hacer · lanzamiento de Yappy

*Escrito el 16-09-2026 después de comprobar el estado real de cada pieza.
Complementa a `GUIA-PRODUCCION.md` (que cuenta el cómo); esto es la lista de
lo que falta, en orden.*

---

## 0 · Lo que ya está hecho (no lo toques)

| Pieza | Estado comprobado |
|---|---|
| **Yappy 1.0.0 en la App Store** | Publicada y **propagada**: la tienda española ya devuelve la ficha a un cliente sin sesión (`itunes.apple.com/lookup?id=6773110015`). |
| **La web** | `yappy.joseluissaorin.com` responde 200; Astro en 31 idiomas, sin rastro de TestFlight. |
| **El cuento «Media voz»** | `out/media-voz.mp4`, 4K, voz mezclada, cuatro rondas de crítica corregidas. |
| **Milán, 384** | `out/milan-acto1-0b.mp4` y `out/milan-acto2-6.mp4`, tres rondas corregidas. |
| **Los siete rótulos con alfa** | `out/rotulo-*-alfa.mov`. |
| **La pegatina «ya disponible» / «out now»** | ES y EN, con alfa, más el fotograma final para la miniatura. |
| **El anuncio de 52 s** | `out/anuncio-ya-disponible-es.mp4`. |

## 1 · El cuello de botella: **el rodaje sigue sin existir**

`rodaje/voz/`, `rodaje/planos/` y `rodaje/pantallas/` están **vacías**. Sin
eso no hay vídeo largo, y sin vídeo largo no hay nada que publicar mañana.
Es lo primero y lo único que importa hoy.

### 1.1 · La hora de rodaje (§3 de la guía de producción)

- **10 min previos**: las siete hojas a rotulador (PLAN VERDAD · EL JUGUETE ·
  EL ÁLBUM DE LA BANDADA · EL PASEO DEL LORO · LAS VOCES SIN MURO · YAPPY
  PARLANCHÍN · RELANZAMIENTO), la hoja de la cuenta (`2,1 + 18,6 + 128,5 +
  50,8 = 200 MB`), el párrafo cortado y pegado con celo. Atrezo: libro,
  gafas, olla, cuchara, **auriculares de cable**, el iPhone con Yappy, el Mac
  con la cola de la imprenta llena. Modo avión en el móvil que graba.
- **0–35 min · la voz**: las 61 tomas de `GUION-VOZ-EN-OFF.md`, de arriba
  abajo, diciendo el número antes de cada una. **Lee la versión corregida**
  de las tomas 16, 45, 53, 60 y 61 (las fechas del guion del 13 ya no
  cuadran). Graba 10 s de silencio de sala antes de empezar.
- **35–43 min · tú a cámara**: 0c con luz de día (tomas 11 y 12) y 7 con la
  persiana bajada (57, 58, 59). **Mismo encuadre exacto** en los dos bloques.
- **43–58 min · los mudos**: flexiones (suelo, cocina, auriculares), la
  prueba de fuego (las gafas; el dedo que tira de la pegatina), el taller
  (las siete hojas, la cuenta, el celo, el Mac) y 15 s de manos quietas.
- 4K · 25 fps · horizontal · foco y exposición bloqueados · sin zoom digital
  · cinco segundos de más a cada lado de cada clip.

### 1.2 · Los diez minutos de pantalla (desde el sofá)

Grabación de pantalla del iPhone, **vertical, un vídeo por gesto**: la percha
con arrastre/corazón/estrella y el loro que vuela · la barra de descarga ·
compartir desde Safari hasta que aparece la pegatina · el cartel en
«preparando» y sonando · el karaoke leyendo «El cable tiraba un poco cada vez
que respiraba.» · la hoja de encargo por capítulos · el paseo con el loro en
PiP.

> **Por qué el karaoke de pantalla no es opcional**: Gemini tomó el karaoke
> final del cuento por un error hasta que se le explicó. El espectador tiene
> que haber visto el karaoke real del iPhone en el capítulo 3.4 **antes** de
> llegar al cuento. Si falta ese plano, el final del cuento no se entiende.

### 1.3 · Dónde dejarlo

`rodaje/voz/`, `rodaje/planos/`, `rodaje/pantallas/`. Está en Dropbox: en
cuanto lo copies se ve desde aquí y empieza el trabajo de montaje (el .srt,
los audios de la app, los rótulos por capítulo).

## 2 · El montaje (esta tarde y esta noche)

DaVinci, proyecto 3840×2160 a 25 fps.

1. Primero la voz: corta las 61 tomas y colócalas con las pausas. **Esa pista
   es el reloj de todo lo demás.**
2. Los planos en su capítulo; los `.mov` con alfa en pista superior, sin
   fundidos añadidos.
3. Revela en Laboratorios Saorín **solo la película** (tus planos). La
   animación no se revela.
4. Música: nada hasta 0c · funk del teaser muy bajo en 1 · se va en 2 ·
   cuerda desde el primer verso de Lorca del cuento hasta el final del 8.
   Cama con licencia en `~/Music/auto-davinci/musica/`.
5. Exporta H.265 10 bits, 4K, 60 Mb/s, audio AAC 256.

**Si a las 23:00 falta algo, este es el orden de sacrificio**: 1) el largo con
el cuento aunque falten rótulos · 2) el anuncio de 52 s (ya está hecho) ·
3) Milán (si no llega, la cita de Agustín sobre papel vacío funciona) ·
4) la Alhambra (tu voz sobre negro con el poema en karaoke funciona).

## 3 · Publicar (mañana, 17)

1. **YouTube**, cuando el máster esté procesado en 4K:
   - Título: «El loro no sabe leer · Yappy, un mes después».
   - Descripción: tesis en tres líneas + capítulos con tiempos + App Store +
     web + créditos (minibeca de XMihura, Supertonic 3 de Supertone, Claude
     de Anthropic, Ibn Zamrak en versión libre, Lorca «Romance sonámbulo»
     1928, Agustín *Confesiones* VI, 3).
   - Miniatura: `out/ya-disponible-es-final.png` sobre el plano del Albaicín.
2. **X**: el anuncio de 52 s + «Yappy ya está en la App Store. Lee cualquier
   texto en voz alta, en el teléfono, sin internet y sin cuentas.» y detrás el
   hilo largo. **El hilo de Android (`marketing/hilos/2026-09-15-android.md`)
   está escrito pero tiene un dato mal: dice 66 millones de parámetros. Son
   ~99 M. Corrígelo antes de publicarlo** (ver §5).
3. **App Store**: nada que hacer. Ya comprobado que la ficha es pública.
4. **TikTok / Reels**: **hecho** — `out/anuncio-ya-disponible-es-916.mp4`
   (1080×1920, 52 s). El 4:3 va entero sobre papel (un recorte central
   decapitaba los rótulos), con la marca arriba y **SIN INTERNET · SIN
   CUENTAS** abajo —que es justo lo que Gemini echaba en falta—, y el final
   a sangre con la pegatina reencuadrada de verdad en 9:16. Míralo antes de
   subirlo. En inglés no se puede hacer todavía: el anuncio solo existe en
   español.

### Tres mejoras del anuncio que Gemini pidió y siguen sin hacer

Solo tú puedes hacerlas (necesitan el proyecto de DaVinci), y son veinte
minutos bien gastados:

- Empezar por la grabación de pantalla (compartir el Quijote), no por el
  espejo: los tres primeros segundos deciden en TikTok.
- Dos rótulos gigantes entre los casos de uso: **SIN INTERNET** y **SIN
  CUENTAS**. Es lo único que nadie más puede decir, y el anuncio no lo dice.
- La palabra «artículos» del rótulo de 0:17 sale cortada por arriba.

## 4 · Google Play (no depende de ti, pero ten el gatillo listo)

La app de Android está entera: paquete firmado, ficha en dos idiomas,
capturas, las diez declaraciones de contenido. Lo que falta es que Google
verifique los documentos de la **cuenta de organización** (DUNS 349710952,
titular jl@joseluissaorin.com).

Cuando llegue ese correo, el mismo día:

1. Crear la app **de nuevo** en la cuenta de organización (no transferir),
   borrando antes el borrador de la cuenta personal si Play no deja reutilizar
   `com.joseluissaorin.yappy`.
2. `node scripts/play-helper.mjs ficha marketing` (textos, 8 capturas,
   portada, icono, contacto) y rehacer las diez declaraciones.
3. Subir el AAB y enviar a producción **ese mismo día**.
4. Publicar el hilo de Android con el final feliz.

La cuenta de servicio de `play-helper` ya es administradora de la
organización, así que la parte automatizable arranca sin más trámite.

## 5 · Tres datos que el material publicado dice mal

La web nueva ya los dice bien. **La ficha de la App Store y el README, no.**

1. **~99 millones de parámetros, no 66.** Lo dicen así los 32
   `marketing/metadata/*/description.txt` y el README (línea 24). Verificado
   en la tarjeta de Supertonic 3.
2. **Sí hay analítica anónima** (solo móvil, de fábrica, apagable, a un
   Worker propio con D1). El README dice «no telemetry». Contarlo entero
   convence más que negarlo.
3. **El modelo es OpenRAIL-M, no MIT.** MIT es la app. Importa para quien
   publique un pódcast hecho con Yappy.

Y dos cosas que el copy no puede prometer todavía: **Android no está
publicada** y **los binarios de escritorio 1.0.0 no están en GitHub
Releases** (allí sigue la v0.1.0 de mayo marcada como pre-release). Ojo con
la toma 60 del guion: promete «el escritorio después», que sí es cierto, pero
no digas que ya se puede descargar.

Arreglar la ficha son dos comandos (`push-metadata` + una revisión de Apple
al vuelo, porque la descripción se puede cambiar sin versión nueva). El
README, un commit.

## 6 · Lo de la semana que viene (no es de hoy, pero no se olvida)

- **Astro / ASO**: la app que se mide en Astro sigue siendo la provisional
  «Yappy (ASO)» (id 101). Ahora que la real está publicada, cámbiala por
  ella: es la única forma de ver si las 106 búsquedas ganables que se
  cubrieron el 12-09 están dando puestos. Los datos base están en
  `marketing/aso/astro-yappy-2026-09-12.csv`.
- **Las primeras reseñas** son lo que más mueve la conversión los primeros
  días. Sin pedirlas dentro de la app no llegan.
- **La analítica propia** (Worker + D1) ya está recogiendo el embudo del
  paseo: a los tres o cuatro días habrá datos suficientes para ver dónde se
  cae la gente.

---

## La lista corta

- [ ] Rodar la voz (61 tomas) y los planos · **hoy, una hora**
- [ ] Las siete grabaciones de pantalla del iPhone · **hoy, diez minutos**
- [ ] Copiarlo todo a `rodaje/` (Dropbox)
- [ ] Montar en DaVinci y exportar el máster · **esta noche**
- [ ] Subir a YouTube · **mañana**
- [ ] Anuncio + hilo en X (con el 66 → 99 corregido) · **mañana**
- [x] Reencuadrar el anuncio a 9:16 para TikTok/Reels — falta que lo veas
- [ ] Corregir los tres datos de la ficha y el README
- [ ] Google Play, en cuanto verifiquen la cuenta
- [ ] Cambiar la app de Astro por la real
