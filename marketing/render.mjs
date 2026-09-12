#!/usr/bin/env node
// El taller de capturas de Yappy: compone las pantallas crudas del
// simulador en marcos de Apple sobre el papel de la casa, con pegatinas
// troqueladas, el loro y los títulos en Baloo 2.
//
//   node marketing/render.mjs <idioma> [--device iphone|ipad] [--layout nombre] [--asc <locale-asc>]
//
// Entradas: marketing/layouts/<layout>.json, marketing/copy/<idioma>.json,
//           marketing/screenshots[-ipad]/<idioma>/*.png (cae a es/ si falta)
// Salida:   marketing/output/<locale-asc>/<device>/NN.png
import { readFileSync, mkdirSync, existsSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import sharp from "sharp";

const AQUI = dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
const L = args[0] || "es";
const opt = (k, d) => (args.includes(k) ? args[args.indexOf(k) + 1] : d);
const DEVICE = opt("--device", "iphone");
const LAYOUT = opt("--layout", DEVICE === "ipad" ? "ipad" : "iphone");
const ASC = opt("--asc", null);

const layout = JSON.parse(readFileSync(join(AQUI, "layouts", `${LAYOUT}.json`), "utf8"));
const copy = JSON.parse(readFileSync(join(AQUI, "copy", `${L}.json`), "utf8"));
const rawDir = join(AQUI, DEVICE === "ipad" ? "screenshots-ipad" : "screenshots", L);
const rawFallback = join(AQUI, DEVICE === "ipad" ? "screenshots-ipad" : "screenshots", "es");
const outDir = join(AQUI, "output", ASC || L, DEVICE);
mkdirSync(outDir, { recursive: true });

const G = layout.global;
const [SLOT_W, SLOT_H] = layout.slot_size;
const BEZEL = join(AQUI, G.bezel_png);
const [BW, BH] = G.bezel_dims;
const SO = G.screen_offset;
const RADIUS = G.screen_corner_radius_px;
const RTL = ["ar", "he"].includes(L);
const CJK = ["ja", "ko", "zh"].includes(L);

const esc = (s) => String(s).replace(/[<>&"']/g, (c) => ({ "<": "&lt;", ">": "&gt;", "&": "&amp;", '"': "&quot;", "'": "&apos;" }[c]));
const r = Math.round;
const FONT_T = `'Baloo 2', sans-serif`;
const FONT_S = `'Literata', serif`;
const FONT_M = `'Space Mono', 'Menlo', monospace`;
// Reservas para las escrituras que Baloo 2 no cubre.
const extra = CJK ? "'Hiragino Sans', 'Apple SD Gothic Neo', 'PingFang SC', " : L === "ar" ? "'Geeza Pro', " : L === "hi" ? "'Kohinoor Devanagari', 'Devanagari Sangam MN', " : "";
const extraS = CJK ? "'Hiragino Mincho ProN', 'Apple SD Gothic Neo', " : L === "ar" ? "'Geeza Pro', " : L === "hi" ? "'Kohinoor Devanagari', " : "";
const FONT_T_L = extra + FONT_T;
const FONT_S_L = extraS + FONT_S;
const ancho = (s) => [...s].reduce((a, ch) => a + (/[　-鿿가-힯＀-￯]/.test(ch) ? 1.0 : /[A-Z0-9ÁÉÍÓÚÑÄÖÜМШЩ]/.test(ch) ? 0.66 : /[ijl.,:;'!| ]/.test(ch) ? 0.3 : 0.56), 0);

// ── El papel: crema con grano ────────────────────────────────────────
async function papel(w, h, color) {
  const grano = `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}">
    <filter id="g"><feTurbulence type="fractalNoise" baseFrequency="0.8" numOctaves="2" stitchTiles="stitch"/>
      <feColorMatrix values="0 0 0 0 0.2  0 0 0 0 0.15  0 0 0 0 0.06  0 0 0 ${G.grain_opacity ?? 0.05} 0"/></filter>
    <rect width="${w}" height="${h}" filter="url(#g)"/></svg>`;
  return sharp({ create: { width: w, height: h, channels: 4, background: color } })
    .composite([{ input: Buffer.from(grano), top: 0, left: 0 }]).png().toBuffer();
}

// ── El aparato: pantalla recortada dentro del marco de Apple, con sombra ─
async function aparato(spec) {
  let p = join(rawDir, spec.screen);
  if (!existsSync(p)) p = join(rawFallback, spec.screen);
  if (!existsSync(p)) throw new Error(`falta la captura ${spec.screen} (${L})`);
  const mask = Buffer.from(`<svg xmlns="http://www.w3.org/2000/svg" width="${SO.w}" height="${SO.h}"><rect width="${SO.w}" height="${SO.h}" rx="${RADIUS}" fill="#fff"/></svg>`);
  let fuente = await sharp(p).png().toBuffer();
  if (DEVICE === "ipad") {
    // La barra del iPad pinta la fecha en el idioma del SISTEMA del simulador
    // (español), no en el de la app: se tapa con el color de fondo de la barra.
    const { data, info } = await sharp(fuente).extract({ left: 700, top: 10, width: 4, height: 4 }).raw().toBuffer({ resolveWithObject: true });
    const [r0, g0, b0] = [data[0], data[1], data[2]];
    const tapa = Buffer.from(`<svg xmlns="http://www.w3.org/2000/svg" width="480" height="64"><rect width="480" height="64" fill="rgb(${r0},${g0},${b0})"/></svg>`);
    fuente = await sharp(fuente).composite([{ input: tapa, left: 138, top: 0 }]).png().toBuffer();
  }
  const screen = await sharp(fuente).resize(SO.w, SO.h, { fit: "cover", position: "top" }).composite([{ input: mask, blend: "dest-in" }]).png().toBuffer();
  const full = await sharp({ create: { width: BW, height: BH, channels: 4, background: { r: 0, g: 0, b: 0, alpha: 0 } } })
    .composite([{ input: screen, top: SO.y, left: SO.x }, { input: BEZEL }]).png().toBuffer();
  let buf = await sharp(full).resize(r(BW * spec.scale), r(BH * spec.scale)).png().toBuffer();
  const m0 = await sharp(buf).metadata();
  const pad = 90;
  // Sombra: la silueta del aparato, teñida de tinta cálida, desenfocada y desplazada.
  // La silueta: RGB a tinta cálida conservando el alfa del aparato.
  const silueta = await sharp(buf).linear([0, 0, 0, 1], [64, 46, 12, 0]).png().toBuffer();
  const sombra = await sharp({ create: { width: m0.width + pad * 2, height: m0.height + pad * 2, channels: 4, background: { r: 0, g: 0, b: 0, alpha: 0 } } })
    .composite([{ input: silueta, top: pad + 30, left: pad }]).blur(24).png().toBuffer();
  const sombraSuave = await sharp(sombra).composite([{ input: Buffer.from(`<svg width="${m0.width + pad * 2}" height="${m0.height + pad * 2}"><rect width="100%" height="100%" fill="white" fill-opacity="${G.device_shadow ?? 0.3}"/></svg>`), blend: "dest-in" }]).png().toBuffer();
  buf = await sharp({ create: { width: m0.width + pad * 2, height: m0.height + pad * 2, channels: 4, background: { r: 0, g: 0, b: 0, alpha: 0 } } })
    .composite([{ input: sombraSuave, top: 0, left: 0 }, { input: buf, top: pad, left: pad }]).png().toBuffer();
  if (spec.rotate) buf = await sharp(buf).rotate(spec.rotate, { background: { r: 0, g: 0, b: 0, alpha: 0 } }).png().toBuffer();
  const meta = await sharp(buf).metadata();
  return { buf, w: meta.width, h: meta.height };
}

// ── Las pegatinas troqueladas ────────────────────────────────────────
function poligono(forma, n, R, inset) {
  const pts = [];
  const cx = R, cy = R;
  if (forma === "estrella") {
    for (let i = 0; i < n * 2; i++) { const a = (Math.PI * i) / n - Math.PI / 2; const rr = (i % 2 ? 0.62 : 1) * R * inset; pts.push([cx + rr * Math.cos(a), cy + rr * Math.sin(a)]); }
  } else if (forma === "sello") {
    for (let i = 0; i < n * 2; i++) { const a = (Math.PI * i) / n - Math.PI / 2; const rr = (i % 2 ? 0.86 : 1) * R * inset; pts.push([cx + rr * Math.cos(a), cy + rr * Math.sin(a)]); }
  } else if (forma === "banderin") {
    const w = R * inset, h = R * inset;
    pts.push([cx - w, cy - h * 0.8], [cx + w, cy - h * 0.8], [cx + w * 0.9, cy + h * 0.25], [cx, cy + h], [cx - w * 0.9, cy + h * 0.25]);
  } else if (forma === "rombo") {
    pts.push([cx, cy - R * inset], [cx + R * inset, cy], [cx, cy + R * inset], [cx - R * inset, cy]);
  } else if (forma === "etiqueta") {
    const w = R * inset, h = R * inset * 0.55;
    pts.push([cx - w, cy - h], [cx + w * 0.75, cy - h], [cx + w, cy], [cx + w * 0.75, cy + h], [cx - w, cy + h]);
  } else if (forma === "corazon") {
    for (let i = 0; i < 60; i++) { const t = (Math.PI * 2 * i) / 60; const x = 16 * Math.pow(Math.sin(t), 3); const y = -(13 * Math.cos(t) - 5 * Math.cos(2 * t) - 2 * Math.cos(3 * t) - Math.cos(4 * t)); pts.push([cx + (x / 17) * R * inset, cy + (y / 17) * R * inset + R * 0.05]); }
  } else { // ovalo
    for (let i = 0; i < 48; i++) { const a = (Math.PI * 2 * i) / 48; pts.push([cx + R * inset * Math.cos(a), cy + R * inset * 0.64 * Math.sin(a)]); }
  }
  return pts.map((p) => p.map((v) => v.toFixed(1)).join(",")).join(" ");
}

async function pegatina(d, canvasW) {
  const R = d.size_px / 2;
  const box = d.size_px + 80;
  const lineas = String(d.text || "").split("\n");
  const fs = d.font_size_px || 64;
  const lh = fs * 1.02;
  const y0 = R - ((lineas.length - 1) * lh) / 2 + fs * 0.35;
  const texto = lineas.map((l, i) => `<tspan x="${R}" dy="${i === 0 ? 0 : lh}">${esc(l)}</tspan>`).join("");
  const n = d.points || 12;
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${box}" height="${box}">
    <defs><filter id="s" x="-20%" y="-20%" width="140%" height="150%"><feGaussianBlur in="SourceAlpha" stdDeviation="8"/><feOffset dy="10"/>
      <feComponentTransfer><feFuncA type="linear" slope="0.22"/></feComponentTransfer><feMerge><feMergeNode/><feMergeNode in="SourceGraphic"/></feMerge></filter></defs>
    <g transform="translate(40 40)" filter="url(#s)">
      <polygon points="${poligono(d.shape, n, R, 1)}" fill="#fffdf7"/>
      <polygon points="${poligono(d.shape, n, R, 0.93)}" fill="${d.color}"/>
      <polygon points="${poligono(d.shape, n, R, 0.80)}" fill="none" stroke="rgba(43,36,24,0.45)" stroke-width="3" stroke-dasharray="10 8"/>
      <text x="${R}" y="${y0}" text-anchor="middle" font-family="${FONT_T_L}" font-weight="800" font-size="${fs}" fill="${d.text_color || "#fffdf7"}" letter-spacing="${d.letter_spacing ?? 1}">${texto}</text>
    </g></svg>`;
  let buf = await sharp(Buffer.from(svg)).png().toBuffer();
  if (d.rotate) buf = await sharp(buf).rotate(d.rotate, { background: { r: 0, g: 0, b: 0, alpha: 0 } }).png().toBuffer();
  return colocar(buf, d, canvasW);
}

async function imagen(d, canvasW) {
  let buf = await sharp(join(AQUI, d.src)).resize({ width: d.width_px }).png().toBuffer();
  if (d.flip) buf = await sharp(buf).flop().png().toBuffer();
  if (d.rotate) buf = await sharp(buf).rotate(d.rotate, { background: { r: 0, g: 0, b: 0, alpha: 0 } }).png().toBuffer();
  if (d.opacity != null && d.opacity < 1) {
    const m = await sharp(buf).metadata();
    buf = await sharp(buf).composite([{ input: Buffer.from(`<svg width="${m.width}" height="${m.height}"><rect width="100%" height="100%" fill="white" fill-opacity="${d.opacity}"/></svg>`), blend: "dest-in" }]).png().toBuffer();
  }
  return colocar(buf, d, canvasW);
}

async function glifo(d, canvasW) {
  // Los glifos (cifras) van siempre en Baloo 2: tiene dígitos en todos los idiomas.
  const bw = Math.min(r(Math.max(d.font_size_px * 1.6, ancho(String(d.text)) * d.font_size_px * 1.25 + 40)), canvasW - 1);
  const bh = Math.min(r(d.font_size_px * 1.5), SLOT_H - 1);
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${bw}" height="${bh}"><text x="50%" y="50%" text-anchor="middle" dominant-baseline="central" font-family="${FONT_T}" font-weight="800" font-size="${d.font_size_px}" fill="${d.color}" opacity="${d.opacity ?? 0.9}">${esc(d.text)}</text></svg>`;
  let buf = await sharp(Buffer.from(svg)).png().toBuffer();
  if (d.rotate) buf = await sharp(buf).rotate(d.rotate, { background: { r: 0, g: 0, b: 0, alpha: 0 } }).png().toBuffer();
  return colocar(buf, d, canvasW);
}

// Etiqueta de máquina (Space Mono, mayúsculas espaciadas), como el membrete de la app.
async function etiqueta(d, canvasW) {
  const fs = d.font_size_px || 34;
  const txt = CJK ? String(d.text) : String(d.text).toUpperCase();
  const w = r(ancho(txt) * fs * 1.2 + fs * 0.18 * txt.length + 70), h = r(fs * 2.2);
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${w + 40}" height="${h + 40}">
    <g transform="translate(20 20)"><rect width="${w}" height="${h}" rx="${h / 2}" fill="${d.color || "#fffdf7"}" stroke="rgba(43,36,24,0.35)" stroke-width="2" stroke-dasharray="6 5"/>
    <text x="${w / 2}" y="${h / 2 + fs * 0.36}" text-anchor="middle" font-family="${extra}${FONT_M}" font-size="${fs}" letter-spacing="${fs * 0.18}" fill="${d.text_color || "#2b2418"}">${esc(txt)}</text></g></svg>`;
  let buf = await sharp(Buffer.from(svg)).png().toBuffer();
  if (d.rotate) buf = await sharp(buf).rotate(d.rotate, { background: { r: 0, g: 0, b: 0, alpha: 0 } }).png().toBuffer();
  return colocar(buf, d, canvasW);
}

async function colocar(buf, d, canvasW) {
  let meta = await sharp(buf).metadata();
  const cx = r(canvasW * d.x), cy = r(SLOT_H * d.y);
  let left = d.anchor === "left" ? cx : d.anchor === "right" ? cx - meta.width : cx - meta.width / 2;
  let top = d.anchor_y === "top" ? cy : d.anchor_y === "bottom" ? cy - meta.height : cy - meta.height / 2;
  left = r(left); top = r(top);
  // Las piezas pueden sangrar por los bordes: se recortan, no se empujan.
  const x0 = Math.max(0, left), y0 = Math.max(0, top);
  const x1 = Math.min(canvasW, left + meta.width), y1 = Math.min(SLOT_H, top + meta.height);
  if (x1 <= x0 || y1 <= y0) return null;
  if (x0 !== left || y0 !== top || x1 !== left + meta.width || y1 !== top + meta.height) {
    buf = await sharp(buf).extract({ left: x0 - left, top: y0 - top, width: x1 - x0, height: y1 - y0 }).png().toBuffer();
  }
  return { input: buf, left: x0, top: y0 };
}

async function piezas(lista, canvasW, localizar, nSlots = 1) {
  const out = [];
  for (const d0 of lista || []) {
    // Las x de las piezas van en unidades de slot (0,5 = centro del primero, 1,5 = del segundo).
    const d = { ...localizar(d0), x: d0.x / nSlots };
    let c = null;
    if (d.type === "pegatina") c = await pegatina(d, canvasW);
    else if (d.type === "imagen") c = await imagen(d, canvasW);
    else if (d.type === "glifo") c = await glifo(d, canvasW);
    else if (d.type === "etiqueta") c = await etiqueta(d, canvasW);
    if (c) out.push(c);
  }
  return out;
}

// ── El título: Baloo 2, ≤2 líneas, se encoge pero nunca se corta ────
function titulo(cap, slotW, yPct, align = "center") {
  const maxW = slotW * (G.title_max_w ?? 0.88);
  const cx = align === "left" ? slotW * (1 - (G.title_max_w ?? 0.88)) / 2 : slotW / 2;
  const anchorT = align === "left" ? "start" : "middle";
  const t = String(cap.title || "").trim();
  const words = t.split(/\s+/);
  const wrap = (n) => { const o = []; let cur = ""; for (const w of words) { if (cur && [...(cur + " " + w)].length > n) { o.push(cur); cur = w; } else cur = cur ? cur + " " + w : w; } if (cur) o.push(cur); return o; };
  let n = G.title_wrap_chars || 14, lines = wrap(n);
  while (lines.length > (cap.max_lines || G.title_max_lines || 2) && n < 60) { n += 2; lines = wrap(n); }
  const longest = Math.max(...lines.map(ancho), 1);
  let fs = Math.min(cap.title_size_px || G.title_size_px, Math.floor(maxW / longest));
  fs = Math.max(56, fs);
  const lh = fs * (G.title_line_height || 1.0);
  const yT = SLOT_H * yPct + fs * 0.9;
  const tsp = lines.map((l, i) => `<tspan x="${cx}" dy="${i ? lh : 0}">${esc(l)}</tspan>`).join("");
  const sub = String(cap.subtitle || "").trim();
  const sfs = cap.subtitle_size_px || G.subtitle_size_px;
  const sw = sub.split(/\s+/); const sl = []; let cur = "";
  const maxChars = Math.floor(maxW / (sfs * (CJK ? 1.0 : 0.5)));
  for (const w of sw) { if (cur && [...(cur + " " + w)].length > maxChars) { sl.push(cur); cur = w; } else cur = cur ? cur + " " + w : w; }
  if (cur) sl.push(cur);
  const yS = yT + (lines.length - 1) * lh + fs * 0.5 + sfs * 0.9;
  const stsp = sl.map((l, i) => `<tspan x="${cx}" dy="${i ? sfs * 1.32 : 0}">${esc(l)}</tspan>`).join("");
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${slotW}" height="${SLOT_H}"${RTL ? ' direction="rtl"' : ""}>
    <text x="${cx}" y="${yT}" text-anchor="${anchorT}" font-family="${FONT_T_L}" font-weight="800" font-size="${fs}" letter-spacing="${G.title_letter_spacing ?? -1}" fill="${cap.title_color || G.title_color}">${tsp}</text>
    <text x="${cx}" y="${yS}" text-anchor="${anchorT}" font-family="${FONT_S_L}" font-weight="500" font-size="${sfs}" fill="${G.subtitle_color}">${stsp}</text></svg>`;
  const fin = yS + (sl.length - 1) * sfs * 1.32 + sfs * 0.35;
  return { buf: Buffer.from(svg), fin };
}

const capFor = (slot) => copy.slots.find((s) => s.slot === slot) || {};
const localizar = (d) => (d.copy_id && copy.stickers?.[d.copy_id] != null ? { ...d, text: copy.stickers[d.copy_id] } : d);

async function grupo(g) {
  const slots = g.type === "panorama" ? g.slots : [g.slot];
  const canvasW = SLOT_W * slots.length;
  const comps = [];
  comps.push(...(await piezas(g.fondo, canvasW, localizar, slots.length)));
  const devices = g.devices || (g.device ? [g.device] : []);
  for (const dv of devices) {
    const { buf, w, h } = await aparato(dv);
    const c = await colocar(buf, { x: dv.x / slots.length, y: dv.y, anchor_y: "top" }, canvasW);
    if (c) comps.push(c);
  }
  // Los títulos se calculan antes para colocar las piezas «bajo_titulo» justo debajo.
  const titulos = {};
  for (const s of slots) {
    const cap = capFor(s);
    if (!cap.title) continue;
    const yPct = g.caption_y?.[s] ?? G.caption_y;
    titulos[s] = titulo(cap, SLOT_W, yPct, g.caption_align?.[s] || "center");
  }
  const piezasG = (g.piezas || []).map((p) => {
    if (!p.bajo_titulo) return p;
    const s = slots[Math.min(slots.length - 1, Math.floor(p.x))];
    const fin = titulos[s]?.fin ?? SLOT_H * 0.2;
    return { ...p, y: (fin + (p.hueco_px ?? 70)) / SLOT_H, anchor_y: "center" };
  });
  comps.push(...(await piezas(piezasG, canvasW, localizar, slots.length)));
  for (const s of slots) {
    if (titulos[s]) comps.push({ input: titulos[s].buf, top: 0, left: (s - slots[0]) * SLOT_W });
  }
  const bg = await papel(canvasW, SLOT_H, g.color || G.paper);
  const lienzo = await sharp(bg).composite(comps).png().toBuffer();
  for (let i = 0; i < slots.length; i++) {
    const out = join(outDir, `${String(slots[i]).padStart(2, "0")}.png`);
    await sharp(lienzo).extract({ left: i * SLOT_W, top: 0, width: SLOT_W, height: SLOT_H }).flatten({ background: g.color || G.paper }).png().toFile(out);
  }
  console.log(`  ✓ ${slots.join(",")}`);
}

console.log(`render ${L} · ${DEVICE} · ${LAYOUT} → ${outDir}`);
for (const g of layout.groups) await grupo(g);
