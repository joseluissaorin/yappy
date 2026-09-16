/**
 * llms.txt: el resumen del sitio para los modelos que lo lean. Se genera,
 * no se escribe a mano, porque el anterior se quedó diciendo «beta por
 * TestFlight» tres semanas después de que la app estuviera publicada.
 */
import type { APIRoute } from "astro";
import { absoluta } from "../i18n";
import { rutaPagina, rutaConvertir, FORMATOS, todasLasRutas } from "../lib/rutas";
import { CUERDA, ENLACES, MOTOR } from "../lib/datos";

export const GET: APIRoute = async () => {
  const rutas = await todasLasRutas();
  const n = rutas.length;
  const p = (clave: any) => absoluta(rutaPagina("es", clave));
  const e = (clave: any) => absoluta(rutaPagina("en", clave));

  const cuerpo = `# Yappy

> Yappy convierte cualquier texto (artículos web, PDF, EPUB, Word, ODT, vídeos y notas de voz) en voz natural, en 31 idiomas, con un modelo que corre entero en el aparato del usuario. Sin nube, sin cuentas y sin tarifa por carácter. Código abierto con licencia MIT. Publicada en la App Store para iPhone y iPad; escritorio para macOS, Windows y Linux; Android aún sin publicar.

Datos esenciales:
- Modelo de voz: ${MOTOR.modelo} (${MOTOR.fabricante}), unos ${MOTOR.parametros} de parámetros, licencia ${MOTOR.licenciaModelo} (el modelo, no la app). ${MOTOR.pesoEscritorio} en escritorio y ${MOTOR.pesoMovil} en móvil. Trabaja a nivel de carácter y va más rápido que el tiempo real en CPU.
- ${MOTOR.voces} voces, todas políglotas: la misma voz lee los ${MOTOR.idiomas} idiomas sin cambiar de timbre.
- El «guionizador» convierte lo escrito en lo dicho antes de sintetizar: fechas, horas, monedas, porcentajes, unidades, ordinales, números romanos con contexto, siglas por fonotáctica, abreviaturas y números deletreados con las gramáticas RBNF de Unicode CLDR (género y declinación incluidos) en los ${MOTOR.idiomas} idiomas. La verbalización anota en lugar de destruir, y por eso el karaoke es exacto.
- Privacidad: la síntesis, la transcripción (${MOTOR.oido}, local) y la extracción de artículos ocurren en el dispositivo. Tocan la red exactamente cuatro cosas: la descarga inicial del modelo, las URL que el usuario comparte, unas estadísticas anónimas de uso (solo en móvil, apagables, a un servidor del propio autor) y las compras vía Apple y RevenueCat.
- «El puente»: el ordenador sintetiza audiolibros .m4b con capítulos y los envía al móvil por una conexión directa cifrada (QUIC/iroh), emparejada con un código QR y revocable. Sin servidor intermedio.
- Precio: gratis, con escucha ilimitada y tres documentos a la vez en la «percha». «Yappy Parlanchín» (${CUERDA.mensual} al mes, ${CUERDA.anual} al año o ${CUERDA.vida} una sola vez) añade percha sin fondo, la imprenta de audiolibros y el puente. No hay prueba gratuita. La cuerda solo existe en iPhone y iPad.

## Páginas principales (español)
- [Portada](${p("portada")}): qué es, demo sonora en seis idiomas y demo del guionizador en WebAssembly.
- [Cómo se usa](${p("uso")}): guía por plataforma, el puente, Siri y atajos, ajustes.
- [El guionizador](${p("guionizador")}): de lo escrito a lo dicho, con demo en vivo.
- [Las voces](${p("voces")}): las diez voces y las 31 lenguas.
- [Precios](${p("precios")}): qué es gratis, qué cuesta y qué cobran los demás.
- [Accesibilidad](${p("accesibilidad")}): dislexia, TDAH, baja visión.
- [Descargas](${p("descargas")}), [Preguntas](${p("preguntas")}), [Privacidad](${p("privacidad")}).
- [Blog](${p("blog")}) y [Cuentos](${p("cuentos")}).

## Convertir (español)
${FORMATOS.map((f) => `- ${absoluta(rutaConvertir("es", f))}`).join("\n")}

## English
- [Home](${e("portada")}), [How it works](${e("uso")}), [The reading script](${e("guionizador")}), [Voices](${e("voces")}), [Pricing](${e("precios")}), [Accessibility](${e("accesibilidad")}), [Download](${e("descargas")}), [FAQ](${e("preguntas")}), [Privacy](${e("privacidad")}), [Blog](${e("blog")}).

## Otros idiomas
El sitio existe en 31 idiomas, con prefijo de idioma en la ruta (/fr/, /de/, /ja/…). El español vive en la raíz. Hay ${n} páginas en total; el índice está en ${absoluta("/sitemap.xml")}.

## Código
- [Repositorio](${ENLACES.github}): monorepo con la app (Tauri + Rust + Svelte), el motor, el guionizador y esta web. Licencia MIT.
- [App Store](${ENLACES.appStore})
`;
  return new Response(cuerpo, { headers: { "content-type": "text/plain; charset=utf-8" } });
};
