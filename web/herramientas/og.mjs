#!/usr/bin/env node
// La imagen de compartir (Open Graph), en español y en inglés.
//
// Es la única pieza de la web que se ve FUERA de la web: en WhatsApp, en X, en
// un canal de Slack. Por eso no se resuelve con una captura, sino con el cartel
// de la casa: papel crema con grano, titular en Baloo 2 ExtraBold con una
// palabra en azul y otra en coral, membrete en Space Mono y el loro asomando.
// Sin paywall y sin precios: ni Apple lo permite en material de tienda ni aquí
// se quiere vender antes de haber contado qué hace la cosa.
//
//   node web/herramientas/og.mjs              # es + en
//   node web/herramientas/og.mjs --idiomas en
//
// Salida: web/public/og.png (es) y web/public/og-en.png (en), 1200×630.

import { mkdirSync, existsSync, copyFileSync, statSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { homedir } from "node:os";
import { execFileSync } from "node:child_process";
import sharp from "sharp";

const AQUI = dirname(fileURLToPath(import.meta.url));
const WEB = join(AQUI, "..");
const RAIZ = join(WEB, "..");
const PUBLIC = join(WEB, "public");

const args = process.argv.slice(2);
const opt = (k, d) => (args.includes(k) ? args[args.indexOf(k) + 1] : d);
const IDIOMAS = opt("--idiomas", "es,en").split(",").map((s) => s.trim()).filter(Boolean);

const W = 1200, H = 630;
const PAPEL = "#f7f2e7";
const TINTA = "#2b2418";
const AZUL = "#2f4bc4";
const CORAL = "#e0502a";
const MEMBRETE = "#82755a";

// ── Las fuentes ──────────────────────────────────────────────────────
// sharp compone el SVG con librsvg, que resuelve las familias por fontconfig:
// no sabe nada de @font-face ni de woff2, solo mira las fuentes instaladas.
// Baloo 2 ya está en ~/Library/Fonts (la usa marketing/render.mjs); Space Mono
// solo vive en el repositorio como woff2, así que aquí se descomprime a TTF y se
// deja instalada. Es la misma fuente con la misma licencia (OFL), no una copia
// nueva, y sin ella el membrete se caería a Menlo, que es otro dibujo.
// OJO: en el SVG la familia va SIEMPRE entre comillas simples ('Baloo 2'); sin
// ellas librsvg no casa los nombres con espacio y se cae a un serif genérico.
const FUENTES = join(AQUI, "fuentes");
function asegurarSpaceMono() {
  const ttf = join(FUENTES, "SpaceMono-Regular.ttf");
  const instalada = join(homedir(), "Library", "Fonts", "SpaceMono-Regular.ttf");
  if (existsSync(instalada)) return true;
  try {
    if (!existsSync(ttf)) {
      mkdirSync(FUENTES, { recursive: true });
      const woff2 = join(PUBLIC, "fonts", "SpaceMono-Regular.woff2");
      execFileSync("python3", ["-c",
        `from fontTools.ttLib import TTFont\nf=TTFont(${JSON.stringify(woff2)})\nf.flavor=None\nf.save(${JSON.stringify(ttf)})`]);
    }
    copyFileSync(ttf, instalada);
    console.log("(Space Mono instalada en ~/Library/Fonts)");
    return true;
  } catch (e) {
    console.log(`(sin Space Mono, el membrete cae a Menlo: ${e.message})`);
    return false;
  }
}
asegurarSpaceMono();

const FONT_T = "'Baloo 2', sans-serif";
const FONT_M = "'Space Mono', 'Menlo', monospace";
const esc = (s) => String(s).replace(/[<>&"']/g, (c) => ({ "<": "&lt;", ">": "&gt;", "&": "&amp;", '"': "&quot;", "'": "&apos;" }[c]));
// Regla de anchura a ojo, la misma que usa marketing/render.mjs para que los
// titulares de la web y los de la tienda salgan del mismo tamaño aparente.
const ancho = (s) => [...s].reduce((a, ch) => a + (/[A-Z0-9ÁÉÍÓÚÑ]/.test(ch) ? 0.66 : /[ijl.,:;'!| ]/.test(ch) ? 0.3 : 0.56), 0);

// ── Los textos ───────────────────────────────────────────────────────
// Cada línea es una lista de trozos [texto, color]: así el color cae sobre la
// palabra, no sobre la línea entera, y se puede traducir sin tocar el dibujo.
const COPIA = {
  es: {
    salida: "og.png",
    lineas: [[["Cualquier ", TINTA], ["texto", AZUL], [",", TINTA]], [["en voz alta", CORAL], [".", TINTA]]],
    membrete: "TEXTO A VOZ · 31 IDIOMAS · SIN NUBE · CÓDIGO ABIERTO",
    pegatina: "sin\nnube",
  },
  en: {
    salida: "og-en.png",
    lineas: [[["Any ", TINTA], ["text", AZUL], [",", TINTA]], [["read aloud", CORAL], [".", TINTA]]],
    membrete: "TEXT TO SPEECH · 31 LANGUAGES · NO CLOUD · OPEN SOURCE",
    pegatina: "no\ncloud",
  },
};

// ── El papel: crema con grano ────────────────────────────────────────
// Turbulencia fractal a alfa 0,05. Sin esto el fondo plano delata que la imagen
// la ha escupido un script; con esto parece papel de verdad, que es el punto.
async function papel() {
  const grano = `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}">
    <filter id="g"><feTurbulence type="fractalNoise" baseFrequency="0.8" numOctaves="2" stitchTiles="stitch"/>
      <feColorMatrix values="0 0 0 0 0.2  0 0 0 0 0.15  0 0 0 0 0.06  0 0 0 0.05 0"/></filter>
    <rect width="${W}" height="${H}" filter="url(#g)"/></svg>`;
  return sharp({ create: { width: W, height: H, channels: 4, background: PAPEL } })
    .composite([{ input: Buffer.from(grano), top: 0, left: 0 }]).png().toBuffer();
}

// ── La pegatina troquelada ───────────────────────────────────────────
// Receta calcada de marketing/render.mjs (poligono/pegatina): borde crema que
// hace de troquel, relleno de color y costura discontinua por dentro. Se copia
// en vez de importarse porque render.mjs es un script, no un módulo: importarlo
// dispararía un render entero de la ficha de la tienda.
function poligono(forma, n, R, inset) {
  const pts = [];
  const cx = R, cy = R;
  if (forma === "sello") {
    for (let i = 0; i < n * 2; i++) { const a = (Math.PI * i) / n - Math.PI / 2; const rr = (i % 2 ? 0.86 : 1) * R * inset; pts.push([cx + rr * Math.cos(a), cy + rr * Math.sin(a)]); }
  } else { // estrella
    for (let i = 0; i < n * 2; i++) { const a = (Math.PI * i) / n - Math.PI / 2; const rr = (i % 2 ? 0.62 : 1) * R * inset; pts.push([cx + rr * Math.cos(a), cy + rr * Math.sin(a)]); }
  }
  return pts.map((p) => p.map((v) => v.toFixed(1)).join(",")).join(" ");
}

async function pegatina({ texto, color = AZUL, tamano = 190, puntas = 12, cuerpo = 46, giro = 9 }) {
  const R = tamano / 2;
  const caja = tamano + 80;
  const lineas = String(texto).split("\n");
  const lh = cuerpo * 1.02;
  const y0 = R - ((lineas.length - 1) * lh) / 2 + cuerpo * 0.35;
  const tspans = lineas.map((l, i) => `<tspan x="${R}" dy="${i === 0 ? 0 : lh}">${esc(l)}</tspan>`).join("");
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${caja}" height="${caja}">
    <defs><filter id="s" x="-20%" y="-20%" width="140%" height="150%"><feGaussianBlur in="SourceAlpha" stdDeviation="8"/><feOffset dy="10"/>
      <feComponentTransfer><feFuncA type="linear" slope="0.22"/></feComponentTransfer><feMerge><feMergeNode/><feMergeNode in="SourceGraphic"/></feMerge></filter></defs>
    <g transform="translate(40 40)" filter="url(#s)">
      <polygon points="${poligono("sello", puntas, R, 1)}" fill="#fffdf7"/>
      <polygon points="${poligono("sello", puntas, R, 0.93)}" fill="${color}"/>
      <polygon points="${poligono("sello", puntas, R, 0.80)}" fill="none" stroke="rgba(43,36,24,0.45)" stroke-width="3" stroke-dasharray="10 8"/>
      <text x="${R}" y="${y0}" text-anchor="middle" font-family="${FONT_T}" font-weight="800" font-size="${cuerpo}" fill="#fffdf7" letter-spacing="1">${tspans}</text>
    </g></svg>`;
  const plano = await sharp(Buffer.from(svg)).png().toBuffer();
  return sharp(plano).rotate(giro, { background: { r: 0, g: 0, b: 0, alpha: 0 } }).png().toBuffer();
}

// ── El loro ──────────────────────────────────────────────────────────
// El PNG de marca viene en un lienzo cuadrado de 2048 con mucho aire alrededor.
// Se recorta el aire (trim) ANTES de escalar: si no, «300 px de alto» serían 300
// px de lienzo y el bicho saldría diminuto.
async function loro(alto, giro) {
  const src = join(RAIZ, "marketing", "assets", "loro", "loro-coral.png");
  const recortado = await sharp(src).trim({ threshold: 1 }).png().toBuffer();
  return sharp(recortado).resize({ height: alto }).rotate(giro, { background: { r: 0, g: 0, b: 0, alpha: 0 } }).png().toBuffer();
}

// ── El titular ───────────────────────────────────────────────────────
// Dos líneas, cuerpo ajustado a la caja: el español es largo y el inglés corto,
// así que fijar un tamaño dejaría una de las dos versiones coja.
function titular(lineas, maxW, x, yBase, cuerpoMax) {
  const anchos = lineas.map((l) => ancho(l.map(([t]) => t).join("")));
  const fs = Math.min(cuerpoMax, Math.floor(maxW / Math.max(...anchos)));
  const lh = fs * 1.04;
  const textos = lineas.map((l, i) => {
    const trozos = l.map(([t, c]) => `<tspan fill="${c}">${esc(t)}</tspan>`).join("");
    // xml:space="preserve": sin esto librsvg se come el espacio final de
    // «Cualquier » y el titular sale pegado («Cualquiertexto»).
    return `<text xml:space="preserve" x="${x}" y="${yBase + i * lh}" font-family="${FONT_T}" font-weight="800" font-size="${fs}" letter-spacing="-1">${trozos}</text>`;
  }).join("");
  return { svg: textos, fin: yBase + (lineas.length - 1) * lh };
}

for (const idioma of IDIOMAS) {
  const c = COPIA[idioma];
  if (!c) { console.log(`! no hay copia para ${idioma}`); continue; }

  const X = 76;                 // margen izquierdo del bloque de texto
  const MAXW = 690;             // hasta donde puede llegar el titular sin tocar al loro
  const t = titular(c.lineas, MAXW, X, 268, 100);

  // El membrete va debajo del titular, no a una altura fija: si el cuerpo del
  // titular cambia entre idiomas, el aire entre ambos se mantiene.
  const yMembrete = t.fin + 84;
  // El membrete tampoco tiene cuerpo fijo: es una línea larga (cuatro reclamos
  // separados por interpunciones) y en inglés crece. Se ajusta para que no
  // invada nunca la columna del loro, con tope arriba para que no grite.
  const anchoMono = 0.6 + 0.2; // avance del glifo mono + interletrado, en «emes»
  const fsM = Math.min(20, Math.floor(740 / (c.membrete.length * anchoMono)));
  const texto = `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}">
    ${t.svg}
    <text x="${X}" y="${yMembrete}" font-family="${FONT_M}" font-size="${fsM}" letter-spacing="${fsM * 0.2}" fill="${MEMBRETE}">${esc(c.membrete)}</text>
  </svg>`;

  const pajaro = await loro(340, 5);
  const mPajaro = await sharp(pajaro).metadata();
  const sello = await pegatina({ texto: c.pegatina });
  const mSello = await sharp(sello).metadata();

  const lienzo = await sharp(await papel()).composite([
    { input: pajaro, left: W - mPajaro.width - 44, top: H - mPajaro.height - 26 },
    { input: sello, left: W - mSello.width - 30, top: 18 },
    { input: Buffer.from(texto), left: 0, top: 0 },
  ]).png().toBuffer();

  const salida = join(PUBLIC, c.salida);
  // El OG lo recomprime medio mundo; se guarda PNG a 9 de compresión y sin más
  // adornos, que es lo que entienden todos los previsualizadores.
  await sharp(lienzo).flatten({ background: PAPEL }).png({ compressionLevel: 9 }).toFile(salida);
  console.log(`${idioma}: ${salida} · ${(statSync(salida).size / 1024).toFixed(0)} KB`);
}
