# La web de Yappy

`yappy.joseluissaorin.com` · Astro 5 estático · Cloudflare Pages (proyecto `yappy`)

Treinta y un idiomas, un blog, páginas de formato y de comparación. El
español vive en la raíz (ahí llevaba dos años y ahí apuntan las fichas de
la App Store); los otros treinta cuelgan de su prefijo.

## Cómo se trabaja

```bash
npm install
npm run dev          # servidor de desarrollo
npm run build        # a dist/
npm run desplegar    # build + wrangler pages deploy dist --project-name=yappy
```

Herramientas:

```bash
npm run verificar-i18n          # ¿cuadran los 31 diccionarios con es.json?
node herramientas/mirar.mjs     # abre dist/ en un navegador, mide y avisa
node herramientas/mirar.mjs --ancho 414 --fotos /precios/
npm run traducir                # los idiomas que falten, desde es.json
npm run traducir -- fr de ja    # solo esos
npm run assets                  # capturas + og + iconos + muestras
```

## Dónde está cada cosa

| Carpeta | Qué hay |
|---|---|
| `src/i18n/*.json` | Un fichero por idioma. **Añadir un idioma es añadir un fichero**: el `astro.config.mjs` lee el directorio y el resto se genera solo. |
| `src/i18n/index.ts` | El motor: `t()`, `tv()`, caída al español, slugs traducidos, hreflang. |
| `src/lib/datos.ts` | **Los hechos**: precios, enlaces, motor, voces, idiomas. Fuente única: ninguna página inventa una cifra. |
| `src/lib/rutas.ts` | El censo de todas las rutas de todos los idiomas. El cerebro. |
| `src/lib/troquel.ts` | El motor de formas de las pegatinas, portado tal cual de la app. |
| `src/lib/jsonld.ts` | Los datos estructurados, con `@id` cruzados. |
| `src/pages/[...ruta].astro` | El único generador: solo elige plantilla. |
| `src/paginas/` | Una plantilla por tipo de página. |
| `src/components/` | Las piezas. `Loro.svelte` es el mismo loro de la app. |
| `src/styles/` | `papel.css` (el sustrato), `album.css` (las pegatinas), `sitio.css` (los muebles). |
| `src/content/blog/<idioma>/` | Los artículos, en MDX. El inglés lleva `traduccionDe`. |
| `src/content/rivales/*.yaml` | Los datos duros de la competencia. Cambiar un precio aquí actualiza todas las comparativas, en todos los idiomas. |
| `public/` | Fuentes, audio, capturas, el WASM del guionizador, `_headers`, `_redirects`. |

## Las reglas de la casa

1. **Papel siempre**: fondo crema con grano. El blanco absoluto no existe.
2. **Materia o nada**: borde de troquel, sombra dura de contacto, costura,
   celo. Nada flota, nada está del todo recto.
3. **Tres voces tipográficas**: el grito (Baloo 2 negra), el susurro (mono
   en versalitas espaciadas) y la lectura (Literata). Nada se escribe fuera
   de esas tres.
4. **El dorado se gana**: solo para lo que de verdad destaca.
5. **Las cifras, en `datos.ts`**. Si una página escribe «3,99 €» a mano,
   está mal.
6. **Ortografía**: tildes y eñes siempre; la raya al modo español, pegada
   al inciso; comillas angulares. Lo ve una persona, y esa persona es
   filóloga.

## Lo que hay que saber antes de tocar el copy

- El modelo son **99 millones de parámetros** (la ficha vieja decía 66: era
  un error) y su licencia es **OpenRAIL-M**, no MIT. MIT es la app.
- **Sí hay analítica anónima** en la app móvil, encendida por defecto y
  apagable, a un servidor del propio autor. La FAQ vieja decía que no.
- **No hay prueba gratuita**, a propósito. Nunca escribir «prueba gratis».
- **Android no está publicada** y los binarios de escritorio 1.0.0 todavía
  no están en GitHub Releases. No prometer ninguna de las dos cosas.
- **Google Play no tiene enlace** hasta que exista: `ENLACES.googlePlay` es
  `null` y los botones lo respetan.

## Añadir un idioma

```bash
npm run traducir -- nb          # escribe src/i18n/nb.json desde es.json
npm run verificar-i18n          # comprueba claves y marcadores
npm run build
```

El menú, el hreflang, el sitemap y el RSS se enteran solos.

## Ampliar un racimo a más idiomas

Los racimos programáticos (formatos, lenguas, comparativas, blog) declaran
en qué idiomas existen, en `ALCANCE`, dentro de `src/lib/rutas.ts`. Crecer
es añadir códigos a esa lista. Es mejor empezar por pocos y crecer que
publicar novecientas páginas traducidas a máquina.
