#!/usr/bin/env node
/**
 * El aviso a los buscadores.
 *
 * Google retiró en 2023 el «ping» de sitemaps: a Google solo se le avisa
 * por Search Console, y eso pide una cuenta autorizada. Lo que sigue
 * abierto es INDEXNOW, el protocolo que comparten Bing, Yandex, Seznam y
 * Naver: se deja una clave en la raíz del sitio y se envía la lista de
 * URL. Bing alimenta además a DuckDuckGo y a las búsquedas de varios
 * asistentes, así que no es poca cosa para un sitio que estrena 588
 * páginas.
 *
 *   node herramientas/avisar.mjs              # todas las del sitemap
 *   node herramientas/avisar.mjs --ver        # enseña qué enviaría y no envía
 */
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const AQUI = join(dirname(fileURLToPath(import.meta.url)), "..");
const SITIO = "https://yappy.joseluissaorin.com";
const HOST = new URL(SITIO).host;
const soloVer = process.argv.includes("--ver");

// La clave vive como fichero suelto en public/: ese es todo el mecanismo
// de propiedad que pide el protocolo.
const clave = readdirSync(join(AQUI, "public"))
  .find((f) => /^[0-9a-f]{32,}\.txt$/.test(f))?.replace(/\.txt$/, "");
if (!clave) {
  console.error("No encuentro la clave de IndexNow en public/. Genera una:");
  console.error('  C=$(openssl rand -hex 24); echo -n "$C" > public/$C.txt');
  process.exit(1);
}

// Las URL salen de los sitemaps ya construidos: una sola fuente de verdad.
const urls = [];
for (const f of readdirSync(join(AQUI, "dist")).filter((f) => /^sitemap-.*\.xml$/.test(f))) {
  for (const m of readFileSync(join(AQUI, "dist", f), "utf8").matchAll(/<loc>([^<]+)<\/loc>/g)) {
    urls.push(m[1]);
  }
}
if (!urls.length) { console.error("No hay sitemaps en dist/. Construye antes."); process.exit(1); }

console.log(`${urls.length} URL · clave ${clave.slice(0, 8)}… · ${SITIO}/${clave}.txt`);
if (soloVer) { console.log(urls.slice(0, 5).join("\n"), "\n…"); process.exit(0); }

// La clave tiene que ser alcanzable ANTES de enviar nada, o el buscador
// rechaza el lote entero sin decir por qué.
const prueba = await fetch(`${SITIO}/${clave}.txt`);
if (!prueba.ok || (await prueba.text()).trim() !== clave) {
  console.error(`✗ ${SITIO}/${clave}.txt no responde con la clave (${prueba.status}). Despliega antes de avisar.`);
  process.exit(1);
}
console.log("✓ la clave se alcanza desde fuera");

// IndexNow acepta hasta 10.000 por lote; se trocea igual, por prudencia.
const lotes = [];
for (let i = 0; i < urls.length; i += 500) lotes.push(urls.slice(i, i + 500));

for (const [i, lote] of lotes.entries()) {
  for (let intento = 1; ; intento++) {
    const r = await fetch("https://api.indexnow.org/indexnow", {
      method: "POST",
      headers: { "content-type": "application/json; charset=utf-8" },
      body: JSON.stringify({ host: HOST, key: clave, keyLocation: `${SITIO}/${clave}.txt`, urlList: lote }),
    });
    // 200 = recibido; 202 = recibido y la clave se validará después. El 403
    // del primer lote suele ser que aún no han ido a leerla: se reintenta.
    console.log(`  lote ${i + 1}/${lotes.length} (${lote.length} URL) → ${r.status}${intento > 1 ? ` (intento ${intento})` : ""}`);
    if (r.status < 400 || intento >= 4) break;
    await new Promise((s) => setTimeout(s, 12000));
  }
  await new Promise((s) => setTimeout(s, 1500));
}
