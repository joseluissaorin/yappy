#!/usr/bin/env node
// Rueda una página HTML determinista (que pinta el estado del segundo `?t=`)
// fotograma a fotograma con Puppeteer y la deja como secuencia PNG.
//
//   node fotogramas.mjs <pagina.html> <salida/> --dur 13 [--fps 30] [--w 2880] [--h 2160]
//                      [--alfa] [--desde 0] [--query "lang=es"] [--escala 1]
//
// Luego `ensambla.sh` convierte la secuencia en ProRes 4444 (alfa) o H.265.
// La página NO debe usar tiempo real: todo sale de `t`. Así el render es
// exacto y reanudable (los PNG que ya existen se saltan).
import puppeteer from "/opt/homebrew/lib/node_modules/puppeteer/lib/esm/puppeteer/puppeteer.js";
import { existsSync, mkdirSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

const args = process.argv.slice(2);
const pos = args.filter((a) => !a.startsWith("--"));
const opt = (k, d) => { const i = args.indexOf(`--${k}`); return i >= 0 ? args[i + 1] : d; };
const flag = (k) => args.includes(`--${k}`);
if (pos.length < 2) { console.error("uso: fotogramas.mjs <pagina.html> <salida/> --dur N [--fps 30]"); process.exit(1); }

const pagina = resolve(pos[0]), salida = resolve(pos[1]);
const dur = parseFloat(opt("dur", "0")), fps = parseFloat(opt("fps", "30"));
const W = parseInt(opt("w", "2880")), H = parseInt(opt("h", "2160"));
const escala = parseFloat(opt("escala", "1"));
const desde = parseFloat(opt("desde", "0"));
const query = opt("query", "");
const alfa = flag("alfa");
const jpg = flag("jpg") && !alfa; // sin alfa, JPEG (Chrome lo codifica 4× más rápido que PNG a 4K)
const paralelo = parseInt(opt("hilos", "4"));
mkdirSync(salida, { recursive: true });

const total = Math.round(dur * fps);
const browser = await puppeteer.launch({
  headless: true,
  executablePath: process.env.CHROME || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  args: ["--no-sandbox", "--disable-gpu", "--force-device-scale-factor=1", "--hide-scrollbars",
         "--font-render-hinting=none", "--enable-font-antialiasing"],
});

const nombre = (i) => `${String(i).padStart(5, "0")}.${jpg ? "jpg" : "png"}`;
let hechos = 0; const t0 = Date.now();
async function trabajador(indices) {
  const page = await browser.newPage();
  await page.setViewport({ width: W, height: H, deviceScaleFactor: escala });
  for (const i of indices) {
    const destino = resolve(salida, nombre(i));
    if (existsSync(destino)) { hechos++; continue; }
    const t = (desde + i / fps).toFixed(4);
    const url = `${pathToFileURL(pagina).href}?t=${t}${query ? "&" + query : ""}`;
    await page.goto(url, { waitUntil: "load" });
    await page.evaluate(async () => { await document.fonts.ready; if (window.__listo) await window.__listo; });
    await page.screenshot({ path: destino, omitBackground: alfa, type: jpg ? "jpeg" : "png", ...(jpg ? { quality: 94 } : {}) });
    hechos++;
    if (hechos % 25 === 0 || hechos === total) {
      const s = (Date.now() - t0) / 1000;
      process.stdout.write(`\r${hechos}/${total}  ${(hechos / s).toFixed(1)} fps  eta ${Math.round((total - hechos) / (hechos / s))} s   `);
    }
  }
  await page.close();
}
const lotes = Array.from({ length: paralelo }, () => []);
for (let i = 0; i < total; i++) lotes[i % paralelo].push(i);
await Promise.all(lotes.map(trabajador));
await browser.close();
console.log(`\n✓ ${total} fotogramas en ${salida}`);
