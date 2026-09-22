# Yappy Parlanchín: la cuerda del loro

La monetización de Yappy, decidida el 9 de septiembre de 2026. Solo muerde
en iOS (donde hay tienda); en escritorio (MIT, GitHub) y en Android (aún sin
Play) `compras::es_pro()` es verdadero y nada cambia.

## La metáfora

El loro de prueba es un juguete de cuerda con una **percha de tres
documentos a la vez**. No es un límite cronológico sino de aforo: quita una
pieza de la cinta y cabe otra; el cuarto documento no cabe y el loro pide
cuerda (la percha dibujada, tres pegatinas colgadas y la cuarta resbalando).
Leer lo que está en la percha no tiene límite. Darle cuerda es girar la
llave dorada de su espalda: **Yappy Parlanchín**. Con cuerda, la percha no
tiene fondo, la imprenta admite encargos (audiolibros enteros) y el puente
con el ordenador funciona.

| Cuerda | Producto (ASC y RevenueCat) | Precio (base España) |
|---|---|---|
| Mensual | `com.joseluissaorin.yappy.parlanchin.mensual` | 3,99 € (Apple no tiene el punto 4,00) |
| Anual | `com.joseluissaorin.yappy.parlanchin.anual` | 29,99 € |
| De por vida | `com.joseluissaorin.yappy.parlanchin.vida` (no consumible) | 59,99 € |

En Google Play (app `com.joseluissaorin.yappy.android`, creada el 22-09-2026) las
suscripciones son `parlanchin_mensual` y `parlanchin_anual` (plan base
`mensual` / `anual`: Play no admite puntos en los ids de suscripción) y el
producto único es `com.joseluissaorin.yappy.parlanchin.vida`; script
`scripts/crear-parlanchin-play.mjs`. En RevenueCat viven como
`parlanchin_mensual:mensual`, `parlanchin_anual:anual` y `…vida` en la app
`app81837ed704` (Play), sobre la misma entitlement y los mismos paquetes.

RevenueCat: proyecto `proj313ccd9a`, app `app6674838594` (App Store),
entitlement `yappy_parlanchín`, oferta `default` con `$rc_monthly`,
`$rc_annual`, `$rc_lifetime`. Clave pública del SDK en `compras.rs`.

## Las piezas

- **Swift** `gen/apple/Sources/yappy-app/Compras.swift`: el SDK de RevenueCat
  (SwiftPM, `purchases-ios-spm`) hablado por el puente C: peticiones
  numeradas que responden por callback con JSON; `pro(bool)` en cada cambio
  de la entitlement; `yappy_compras_es_pro()` lee la caché del SDK.
- **Rust** `compras.rs`: la tabla de `oneshot` por petición, el atómico
  `PRO`, los comandos `compras_*_cmd`, el evento `pro_cambio`, y la tienda
  SIMULADA (solo `debug_assertions`) para ensayar en el simulador.
- **Rust** `cuota.rs`: el aforo de la percha (`DOCUMENTOS_GRATIS = 3`),
  calculado en vivo sobre la cola (sin persistencia propia). `cuota::aforo`
  es la puerta del ALTA en `cola.rs` (URL, web viva, texto, archivo, audio):
  con la percha llena y sin cuerda emite `cuota_agotada` y devuelve
  `Err("parlanchin")`; el caparazón abre el paywall con la viñeta de la
  percha. Leer no tiene puerta.
- **Puertas**: `imprenta_encargar_cmd`, `puente_convertir_cmd` y
  `render_audiobook_cmd` devuelven `Err("parlanchin")` sin cuerda.
- **Frontend** `$lib/compras.svelte.ts` (el espejo: pro, cuota, ofertas,
  motivo), `$lib/Paywall.svelte` (la página del álbum), la caja «Yappy
  Parlanchín» en la trastienda, y la puerta de la imprenta en `/read`.
- **Catálogo**: `scripts/crear-parlanchin.mjs` (ASC, idempotente; `--step
  screenshot` sube `marketing/paywall-revision.png`) y
  `scripts/revenuecat-parlanchin.mjs` (productos, entitlement, paquetes).

## Ensayo en el simulador

StoreKit no tiene cuenta de sandbox en el simulador. Con una compilación de
depuración, abrir la app con `?tienda=sim` (o `?tienda=sim&pro=1`) enciende
la tienda de mentira: ofertas fijas en euros y compra que sale bien a los
dos segundos. La compra REAL solo se prueba en dispositivo (TestFlight o
cuenta de sandbox).

## Lo que queda en manos de José Luis (paso visual en la web de ASC)

La PRIMERA suscripción de una app no puede enviarse a revisión por la API:
hay que abrir la versión en App Store Connect, sección «Compras dentro de
la app y suscripciones», pulsar «Seleccionar» y marcar las tres cuerdas
antes de enviar la versión. Después, las siguientes van por API.
