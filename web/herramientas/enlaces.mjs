#!/usr/bin/env node
/**
 * El grafo de enlaces de dist/. Busca tres cosas:
 *
 *  - HUÉRFANAS: páginas a las que no llega ni un enlace desde dentro. Existen
 *    en el sitemap pero nadie puede llegar a ellas navegando, y Google las
 *    trata en consecuencia.
 *  - ROTOS: enlaces internos que no llevan a ninguna parte.
 *  - FLOJAS: páginas con un solo enlace entrante. No es un error, pero es
 *    una señal: un racimo entero colgando de un hilo.
 *
 *   node herramientas/enlaces.mjs
 */
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const RAIZ = join(dirname(fileURLToPath(import.meta.url)), "..", "dist");
const paginas = [];
(function anda(d) {
  for (const e of readdirSync(d)) {
    const f = join(d, e);
    if (statSync(f).isDirectory()) anda(f);
    else if (e === "index.html") paginas.push(("/" + relative(RAIZ, d).replace(/\\/g, "/") + "/").replace("//", "/"));
  }
})(RAIZ);

const todas = new Set(paginas);
const entrantes = new Map([...todas].map((p) => [p, 0]));
const rotos = [];
// Lo que no es una página pero sí existe (sitemaps, RSS, ficheros sueltos).
const ficheros = new Set();
(function anda(d) {
  for (const e of readdirSync(d)) {
    const f = join(d, e);
    if (statSync(f).isDirectory()) anda(f);
    else ficheros.add("/" + relative(RAIZ, f).replace(/\\/g, "/"));
  }
})(RAIZ);

for (const p of todas) {
  const f = p === "/" ? join(RAIZ, "index.html") : join(RAIZ, p, "index.html");
  const html = readFileSync(f, "utf8");
  const vistos = new Set();
  for (const m of html.matchAll(/href="(\/[^"#?]*)"/g)) {
    const crudo = m[1];
    let destino = crudo.endsWith("/") ? crudo : crudo + "/";
    destino = destino.replace("//", "/");
    if (todas.has(destino)) {
      if (destino !== p && !vistos.has(destino)) { vistos.add(destino); entrantes.set(destino, entrantes.get(destino) + 1); }
    } else if (!ficheros.has(crudo)) {
      rotos.push(`${p} → ${crudo}`);
    }
  }
}

const huerfanas = [...entrantes].filter(([, n]) => n === 0).map(([p]) => p);
const flojas = [...entrantes].filter(([, n]) => n === 1).map(([p]) => p);

console.log(`${todas.size} páginas.`);
console.log(huerfanas.length ? `✗ huérfanas (${huerfanas.length}): ${huerfanas.slice(0, 15).join(", ")}` : "✓ ninguna huérfana");
console.log(rotos.length ? `✗ enlaces rotos (${rotos.length}): ${[...new Set(rotos)].slice(0, 10).join(" · ")}` : "✓ ningún enlace roto");
console.log(flojas.length ? `· con un solo enlace entrante (${flojas.length}): ${flojas.slice(0, 10).join(", ")}` : "✓ todas con dos o más enlaces entrantes");
process.exit(huerfanas.length || rotos.length ? 1 : 0);
