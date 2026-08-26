#!/usr/bin/env node
// Verifica los 31 diccionarios contra el canon (es.ts): mismas claves,
// ninguna pasarela pendiente, sin la raya al estilo inglés (con espacios
// a ambos lados; la secuencia se construye por programa para no llevarla
// dentro), y con el final «: » de lector.t_fallo_export conservado.
import { readFileSync, readdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const dir = join(dirname(fileURLToPath(import.meta.url)), "../yappy-app/src/lib/i18n");
const RAYA_INGLESA = [" ", "—", " "].join("");

function claves(src) {
  const out = [];
  for (const m of src.matchAll(/^\s*"([^"]+)":/gm)) out.push(m[1]);
  return out;
}

const canon = claves(readFileSync(join(dir, "es.ts"), "utf8"));
if (canon.length < 100) {
  console.error(`el canon parece roto: ${canon.length} claves`);
  process.exit(1);
}

let fallo = false;
const ficheros = readdirSync(dir).filter((f) => f.endsWith(".ts")).sort();
for (const f of ficheros) {
  const src = readFileSync(join(dir, f), "utf8");
  const problemas = [];
  if (src.includes('from "./en"')) problemas.push("PASARELA pendiente (reexporta en)");
  if (src.includes(RAYA_INGLESA)) problemas.push("contiene la raya inglesa con espacios");
  if (f !== "es.ts") {
    const k = claves(src);
    if (k.length !== canon.length) problemas.push(`${k.length} claves (canon: ${canon.length})`);
    const set = new Set(k);
    const faltan = canon.filter((c) => !set.has(c));
    if (faltan.length) problemas.push(`faltan: ${faltan.slice(0, 5).join(", ")}${faltan.length > 5 ? "…" : ""}`);
    const canonSet = new Set(canon);
    const sobran = k.filter((c) => !canonSet.has(c));
    if (sobran.length) problemas.push(`sobran: ${sobran.slice(0, 5).join(", ")}`);
    const m = src.match(/"lector\.t_fallo_export":\s*"([^"]*)"/);
    if (m && !/[:：]\s?$/.test(m[1])) problemas.push(`t_fallo_export sin «: » final: ${JSON.stringify(m[1])}`);
  }
  if (problemas.length) {
    fallo = true;
    console.log(`✘ ${f}: ${problemas.join(" · ")}`);
  } else {
    console.log(`✓ ${f}`);
  }
}
process.exit(fallo ? 1 : 0);
