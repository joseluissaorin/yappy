#!/usr/bin/env node
// Las capturas de la app, listas para la web.
//
// Las crudas del simulador (1320×2868, pantalla pura, sin marco) viven en
// marketing/screenshots/<idioma>/NN-nombre.png y pesan 2,5 MB cada una: servirlas
// tal cual sería una grosería. Aquí se reducen a la mitad y se guardan en los dos
// formatos que el navegador sabe elegir solo con <picture>: AVIF para quien pueda
// y WebP para el resto. No se genera PNG a propósito: en 2026 no queda nadie que
// no entienda WebP, y cada formato de más es peso muerto en el repositorio.
//
//   node web/herramientas/capturas.mjs                 # es + en, las nueve
//   node web/herramientas/capturas.mjs --idiomas es    # solo español
//   node web/herramientas/capturas.mjs --ancho 860     # otro ancho
//   node web/herramientas/capturas.mjs --forzar        # rehacer aunque estén al día
//
// Es reejecutable: si el destino ya existe y es más nuevo que el origen, no se toca.
// Así se puede llamar desde un `npm run` antes de cada build sin pagar el peaje.

import { mkdirSync, statSync, existsSync, readdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import sharp from "sharp";

const AQUI = dirname(fileURLToPath(import.meta.url));
const WEB = join(AQUI, "..");
const RAIZ = join(WEB, "..");
const CRUDAS = join(RAIZ, "marketing", "screenshots");
const DESTINO = join(WEB, "public", "capturas");

const args = process.argv.slice(2);
const opt = (k, d) => (args.includes(k) ? args[args.indexOf(k) + 1] : d);
// 645 px = la mitad justa de una pantalla de 1290. La web las enseña dentro de un
// marco de unos 268 px, así que incluso en una retina de 3x sobra resolución.
const ANCHO = Number(opt("--ancho", 645));
const IDIOMAS = opt("--idiomas", "es,en").split(",").map((s) => s.trim()).filter(Boolean);
const FORZAR = args.includes("--forzar");

// Calidades tanteadas a ojo sobre estas capturas concretas (mucho papel plano,
// tipografía nítida): por debajo de esto el texto del lector empieza a emborronarse.
const CAL_WEBP = Number(opt("--webp", 82));
const CAL_AVIF = Number(opt("--avif", 55));

// El mapa: nombre que usa la web ← fichero crudo del simulador.
// Los nombres de la web son del dominio, no del flujo de capturas: así, si mañana
// el flujo renumera las pantallas, solo cambia esta tabla y no el HTML.
const MAPA = {
  cinta: "20-cinta.png",              // la pantalla principal: las pegatinas de los documentos
  lector: "21-lector-a.png",          // el lector con el karaoke marcando la frase
  pajaros: "03-paseo-pajaro.png",     // elegir voz: los diez pájaros de colores
  trastienda: "25-trastienda.png",    // los ajustes, sobre papel
  noche: "26-trastienda-noche.png",   // los mismos ajustes en modo noche
  // OJO: no hay captura real de la imprenta. El flujo Maestro toca «abrir la
  // imprenta» con el loro de prueba y la app responde con el muro de la cuerda,
  // así que 29-imprenta.png es en realidad el paywall CON PRECIOS y no puede
  // salir a la web (normativa de Apple y regla de la casa). Mientras no se
  // recapture con una cuenta con cuerda, se usa la trastienda, que al menos
  // enseña la fila «la imprenta y los audiolibros».
  imprenta: "27-trastienda-cuerda.png",
  percha: "13-paseo-percha.png",      // tres a la vez: la percha
  compartir: "09-paseo-compartir.png",// compartir desde Safari
  hola: "01-paseo-hola.png",          // hola, soy tu loro
};

// Si un idioma no tiene la captura (los flujos fallan de vez en cuando en alguna
// lengua), se cae al español, que es el juego más completo. Mismo criterio que
// marketing/render.mjs, para que la web y la ficha de la tienda no se contradigan.
function origen(idioma, fichero) {
  const propio = join(CRUDAS, idioma, fichero);
  if (existsSync(propio)) return propio;
  const respaldo = join(CRUDAS, "es", fichero);
  if (existsSync(respaldo)) return respaldo;
  return null;
}

// Al día = el destino existe y no es más viejo que el origen. Comparar fechas en
// vez de hashes basta aquí: las crudas solo cambian cuando se vuelve a capturar.
const alDia = (salida, entrada) => {
  if (FORZAR || !existsSync(salida)) return false;
  return statSync(salida).mtimeMs >= statSync(entrada).mtimeMs;
};

const kb = (n) => `${(n / 1024).toFixed(0)} KB`;

let total = 0;
for (const idioma of IDIOMAS) {
  const dir = join(DESTINO, idioma);
  mkdirSync(dir, { recursive: true });
  const pesoIdioma = { webp: 0, avif: 0 };
  console.log(`\n${idioma}`);
  for (const [nombre, fichero] of Object.entries(MAPA)) {
    const entrada = origen(idioma, fichero);
    if (!entrada) {
      console.log(`  ! falta ${fichero} en ${idioma}, se salta`);
      continue;
    }
    // Un solo decode del PNG grande y dos encodes: sharp no reutiliza el pipeline
    // entre formatos, pero sí podemos reutilizar el búfer ya redimensionado.
    const base = await sharp(entrada).resize({ width: ANCHO, withoutEnlargement: true }).toBuffer();
    const tareas = [
      [join(dir, `${nombre}.webp`), (s) => s.webp({ quality: CAL_WEBP, effort: 6 })],
      [join(dir, `${nombre}.avif`), (s) => s.avif({ quality: CAL_AVIF, effort: 6 })],
    ];
    for (const [salida, codec] of tareas) {
      if (!alDia(salida, entrada)) await codec(sharp(base)).toFile(salida);
      const peso = statSync(salida).size;
      pesoIdioma[salida.endsWith(".avif") ? "avif" : "webp"] += peso;
      total += peso;
    }
    const w = statSync(join(dir, `${nombre}.webp`)).size;
    const a = statSync(join(dir, `${nombre}.avif`)).size;
    console.log(`  ${nombre.padEnd(11)} webp ${kb(w).padStart(7)}   avif ${kb(a).padStart(7)}`);
  }
  console.log(`  ·· total ${idioma}: webp ${kb(pesoIdioma.webp)} · avif ${kb(pesoIdioma.avif)}`);
}
console.log(`\ntodo: ${kb(total)} en ${DESTINO}`);

// Aviso de higiene: si alguien deja capturas de idiomas que la web ya no sirve,
// que se vean, porque en el repositorio no las va a echar nadie de menos.
for (const d of existsSync(DESTINO) ? readdirSync(DESTINO, { withFileTypes: true }) : []) {
  if (d.isDirectory() && !IDIOMAS.includes(d.name)) console.log(`(sobra la carpeta capturas/${d.name})`);
}
