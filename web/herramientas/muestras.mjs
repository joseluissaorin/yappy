#!/usr/bin/env node
// Las muestras de voz que suenan en la web.
//
// La app lleva 310 muestras (10 pájaros × 31 idiomas) en
// yappy-app/resources/muestras/<voz>-<idioma>.m4a. La web no necesita las 310:
// con el español y el inglés se cubre a casi todo el que llega, y cada m4a que
// se sube a Pages es peso que alguien acaba descargando sin pedirlo.
//
// Se copian, no se enlazan: `astro build` sigue enlaces simbólicos de forma
// desigual según el adaptador, y un fallo aquí se traduce en una web muda.
//
//   node web/herramientas/muestras.mjs                  # es + en
//   node web/herramientas/muestras.mjs --idiomas es,en,fr
//
// Salida: web/public/muestras/<idioma>/<voz>.m4a (el idioma ya va en la carpeta,
// así que el nombre del fichero se queda solo con el nombre del pájaro).

import { mkdirSync, copyFileSync, existsSync, statSync, readdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const AQUI = dirname(fileURLToPath(import.meta.url));
const WEB = join(AQUI, "..");
const RAIZ = join(WEB, "..");
const ORIGEN = join(RAIZ, "yappy-app", "resources", "muestras");
const DESTINO = join(WEB, "public", "muestras");

const args = process.argv.slice(2);
const opt = (k, d) => (args.includes(k) ? args[args.indexOf(k) + 1] : d);
const IDIOMAS = opt("--idiomas", "es,en").split(",").map((s) => s.trim()).filter(Boolean);
const FORZAR = args.includes("--forzar");

// Los diez pájaros, en el orden en que los enseña la app.
const VOCES = ["alex", "james", "robert", "sam", "daniel", "sarah", "lily", "jessica", "olivia", "emily"];

if (!existsSync(ORIGEN)) {
  console.error(`no están las muestras en ${ORIGEN}`);
  process.exit(1);
}

let total = 0, copiadas = 0;
for (const idioma of IDIOMAS) {
  const dir = join(DESTINO, idioma);
  mkdirSync(dir, { recursive: true });
  for (const voz of VOCES) {
    const entrada = join(ORIGEN, `${voz}-${idioma}.m4a`);
    if (!existsSync(entrada)) { console.log(`  ! falta ${voz}-${idioma}.m4a`); continue; }
    const salida = join(dir, `${voz}.m4a`);
    // Reejecutable: solo se copia si no está o si la muestra de la app es nueva.
    if (FORZAR || !existsSync(salida) || statSync(salida).mtimeMs < statSync(entrada).mtimeMs) {
      copyFileSync(entrada, salida);
      copiadas++;
    }
    total += statSync(salida).size;
  }
  const peso = readdirSync(dir).reduce((a, f) => a + statSync(join(dir, f)).size, 0);
  console.log(`${idioma}: ${readdirSync(dir).length} muestras · ${(peso / 1024).toFixed(0)} KB`);
}
console.log(`${copiadas} copiadas · ${(total / 1024).toFixed(0)} KB en ${DESTINO}`);
