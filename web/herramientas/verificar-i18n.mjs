#!/usr/bin/env node
/**
 * La aduana de los idiomas. Compara cada src/i18n/*.json contra es.json.
 *
 * La trampa que esto evita: la paridad entre traducciones NO detecta una
 * pérdida que ocurra en todas a la vez. Si un reemplazo masivo se come una
 * clave en los treinta y un ficheros, todos siguen «cuadrando» entre sí.
 * Por eso aquí se compara siempre contra el ORIGINAL, y se cuenta.
 *
 *   node herramientas/verificar-i18n.mjs
 *   node herramientas/verificar-i18n.mjs --sin-traducir   # además, lo que sigue en español
 */
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const I18N = join(dirname(fileURLToPath(import.meta.url)), "..", "src", "i18n");
const verSinTraducir = process.argv.includes("--sin-traducir");

function aplanar(obj, prefijo = "", salida = {}) {
  for (const [k, v] of Object.entries(obj)) {
    const ruta = prefijo ? `${prefijo}.${k}` : k;
    if (v && typeof v === "object") aplanar(v, ruta, salida);
    else salida[ruta] = v;
  }
  return salida;
}

const leer = (c) => JSON.parse(readFileSync(join(I18N, `${c}.json`), "utf8"));
const es = aplanar(leer("es"));
const claves = Object.keys(es);
const idiomas = readdirSync(I18N).filter((f) => f.endsWith(".json")).map((f) => f.slice(0, -5)).sort();

// Los marcadores tienen que sobrevivir a la traducción: si {n} se convierte
// en {número}, la interpolación deja de funcionar y sale el literal.
const marcadores = (s) => [...new Set(String(s).match(/\{\w+\}/g) ?? [])].sort().join(",");

console.log(`Original: es.json, ${claves.length} claves. ${idiomas.length} idiomas.\n`);
let malos = 0;

for (const c of idiomas) {
  if (c === "es") continue;
  const d = aplanar(leer(c));
  const faltan = claves.filter((k) => d[k] == null);
  const sobran = Object.keys(d).filter((k) => !claves.includes(k));
  const rotos = claves.filter((k) => d[k] != null && marcadores(es[k]) !== marcadores(d[k]));
  const vacias = claves.filter((k) => typeof d[k] === "string" && d[k].trim() === "");
  const iguales = claves.filter((k) => typeof es[k] === "string" && d[k] === es[k] && es[k].length > 14);

  const mal = faltan.length || sobran.length || rotos.length || vacias.length;
  if (mal) malos++;
  const resumen = [
    faltan.length && `${faltan.length} sin clave`,
    sobran.length && `${sobran.length} de más`,
    rotos.length && `${rotos.length} con el marcador roto`,
    vacias.length && `${vacias.length} vacías`,
    iguales.length && `${iguales.length} en español`,
  ].filter(Boolean).join(" · ");
  console.log(`${mal ? "✗" : "✓"} ${c.padEnd(3)} ${Object.keys(d).length} claves${resumen ? "  " + resumen : ""}`);
  if (faltan.length) console.log(`     faltan: ${faltan.slice(0, 5).join(", ")}${faltan.length > 5 ? "…" : ""}`);
  if (rotos.length) console.log(`     rotos:  ${rotos.slice(0, 3).join(", ")}`);
  if (verSinTraducir && iguales.length) console.log(`     iguales: ${iguales.slice(0, 6).join(", ")}`);
}

console.log(malos ? `\n${malos} idioma(s) con problemas.` : "\nTodos cuadran.");
process.exit(malos ? 1 : 0);
