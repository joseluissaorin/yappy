#!/usr/bin/env node
/**
 * El ojo: abre dist/ en un navegador de verdad, mide y fotografía.
 *
 *   node herramientas/mirar.mjs                      # revisa las rutas de siempre
 *   node herramientas/mirar.mjs /precios/ /en/       # solo esas
 *   node herramientas/mirar.mjs --ancho 414          # a lo ancho que quieras
 *   node herramientas/mirar.mjs --fotos              # además, capturas en /tmp/yweb
 *
 * Informa de tres cosas que una captura no cuenta: si la página se sale por
 * el lado (y QUIÉN la saca), si hay texto que se desborda de su caja, y los
 * errores de consola.
 */
import { createServer } from "node:http";
import { readFile, stat, mkdir, writeFile } from "node:fs/promises";
import { join, extname, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import puppeteer from "puppeteer";

const RAIZ = join(dirname(fileURLToPath(import.meta.url)), "..", "dist");
const SALIDA = "/tmp/yweb";
const args = process.argv.slice(2);
const opt = (k, d) => (args.includes(k) ? args[args.indexOf(k) + 1] : d);
const ANCHO = Number(opt("--ancho", 1280));
const FOTOS = args.includes("--fotos");
const rutas = args.filter((a) => a.startsWith("/"));

const TIPOS = {
  ".html": "text/html; charset=utf-8", ".css": "text/css; charset=utf-8",
  ".js": "text/javascript; charset=utf-8", ".json": "application/json",
  ".svg": "image/svg+xml", ".png": "image/png", ".webp": "image/webp",
  ".avif": "image/avif", ".woff2": "font/woff2", ".m4a": "audio/mp4",
  ".wasm": "application/wasm", ".xml": "application/xml", ".txt": "text/plain; charset=utf-8",
};

const servidor = createServer(async (req, res) => {
  try {
    let ruta = decodeURIComponent(req.url.split("?")[0]);
    let f = join(RAIZ, ruta);
    if ((await stat(f).catch(() => null))?.isDirectory()) f = join(f, "index.html");
    else if (!extname(f)) f = join(RAIZ, ruta, "index.html");
    const cuerpo = await readFile(f);
    res.writeHead(200, { "content-type": TIPOS[extname(f)] ?? "application/octet-stream" });
    res.end(cuerpo);
  } catch {
    res.writeHead(404, { "content-type": "text/html" });
    res.end("404");
  }
});
await new Promise((r) => servidor.listen(0, r));
const BASE = `http://localhost:${servidor.address().port}`;

const POR_DEFECTO = ["/", "/como-se-usa/", "/guionizador/", "/precios/", "/voces/",
  "/preguntas/", "/accesibilidad/", "/descargas/", "/convertir/pdf-a-audio/", "/en/"];

const navegador = await puppeteer.launch({ headless: true });
await mkdir(SALIDA, { recursive: true });
let problemas = 0;

for (const ruta of rutas.length ? rutas : POR_DEFECTO) {
  const pagina = await navegador.newPage();
  await pagina.setViewport({ width: ANCHO, height: 1000, deviceScaleFactor: 1 });
  const consola = [];
  pagina.on("console", (m) => { if (m.type() === "error") consola.push(m.text()); });
  pagina.on("pageerror", (e) => consola.push(String(e)));

  const respuesta = await pagina.goto(BASE + ruta, { waitUntil: "networkidle0", timeout: 30000 });
  await new Promise((r) => setTimeout(r, 350));

  const informe = await pagina.evaluate(() => {
    const vw = document.documentElement.clientWidth;
    const sw = document.documentElement.scrollWidth;
    const fuera = [];
    const rebosa = [];
    for (const el of document.querySelectorAll("body *")) {
      const e = getComputedStyle(el);
      if (e.position === "fixed" || e.display === "none" || e.visibility === "hidden") continue;
      const r = el.getBoundingClientRect();
      if (r.width === 0 && r.height === 0) continue;
      // Quien saca la página por la derecha, salvo lo que está pegado a
      // propósito (el collage sangra por los bordes y eso es intencionado).
      // Lo que vive dentro de un contenedor con scroll propio (la tabla
      // comparativa, por ejemplo) se sale de la pantalla a propósito: se
      // arrastra con el dedo. No es un desbordamiento, es un carrusel.
      let enScroller = false;
      for (let a = el.parentElement; a && a !== document.body; a = a.parentElement) {
        const ea = getComputedStyle(a);
        if (ea.overflowX === "auto" || ea.overflowX === "scroll") { enScroller = true; break; }
      }
      if (r.right > vw + 1 && e.position !== "absolute" && !enScroller) {
        fuera.push(`${el.tagName.toLowerCase()}.${(el.className || "").toString().split(" ")[0]} R=${Math.round(r.right)} W=${Math.round(r.width)}`);
      }
      // El collage sangra por los bordes a propósito: solo molesta si es mucho.
      if (el.scrollWidth > el.clientWidth + 40 && e.overflowX === "visible") {
        rebosa.push(`${el.tagName.toLowerCase()}.${(el.className || "").toString().split(" ")[0]} ${el.scrollWidth}>${el.clientWidth}`);
      }
    }
    return {
      vw, sw,
      titulo: document.title,
      h1: document.querySelectorAll("h1").length,
      fuera: [...new Set(fuera)].slice(0, 8),
      rebosa: [...new Set(rebosa)].slice(0, 8),
      alto: document.documentElement.scrollHeight,
    };
  });

  const mal = informe.sw > informe.vw + 1 || informe.fuera.length || informe.rebosa.length
    || consola.length || informe.h1 !== 1 || respuesta.status() !== 200;
  if (mal) problemas++;
  console.log(`${mal ? "✗" : "✓"} ${ruta}  ${informe.vw}→${informe.sw}px  alto ${informe.alto}  h1×${informe.h1}`);
  if (informe.fuera.length) console.log(`    se sale: ${informe.fuera.join(" | ")}`);
  if (informe.rebosa.length) console.log(`    rebosa: ${informe.rebosa.join(" | ")}`);
  if (consola.length) console.log(`    consola: ${[...new Set(consola)].slice(0, 3).join(" | ")}`);

  if (FOTOS) {
    const nombre = (ruta === "/" ? "portada" : ruta.replace(/^\/|\/$/g, "").replace(/\//g, "-")) + `_${ANCHO}`;
    await pagina.screenshot({ path: join(SALIDA, `${nombre}.png`), fullPage: true });
  }
  await pagina.close();
}

await navegador.close();
servidor.close();
console.log(problemas ? `\n${problemas} página(s) con algo que mirar.` : "\nTodo limpio.");
process.exit(0);
