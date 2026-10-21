// El gráfico de portada de Google Play (1024x500): papel, el loro coral y el nombre en Baloo 2.
import sharp from "sharp";
import { readdirSync, readFileSync, mkdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
const AQUI = dirname(fileURLToPath(import.meta.url));
const W = 1024, H = 500;
const grano = `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}"><filter id="g"><feTurbulence type="fractalNoise" baseFrequency="0.8" numOctaves="2" stitchTiles="stitch"/><feColorMatrix values="0 0 0 0 0.2  0 0 0 0 0.15  0 0 0 0 0.06  0 0 0 0.05 0"/></filter><rect width="${W}" height="${H}" filter="url(#g)"/></svg>`;
const esc = (x) => String(x).replace(/[<>&"']/g, (c) => ({ "<": "&lt;", ">": "&gt;", "&": "&amp;", '"': "&quot;", "'": "&apos;" }[c]));
// Reservas para las escrituras que Baloo 2, Literata y Space Mono no cubren.
const reserva = (l) => (["ja", "ko"].includes(l) ? "'Hiragino Sans', 'Apple SD Gothic Neo', " : l === "ar" ? "'Geeza Pro', " : l === "hi" ? "'Kohinoor Devanagari', " : "");
const texto = (t, l) => `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}">
  <text x="70" y="235" font-family="'Baloo 2', sans-serif" font-weight="800" font-size="150" letter-spacing="-3" fill="#2b2418">Yappy</text>
  <text x="74" y="310" font-family="${reserva(l)}'Literata', serif" font-weight="500" font-size="40" fill="#82755a">${esc(t[0])}</text>
  <text x="74" y="365" font-family="${reserva(l)}'Literata', serif" font-weight="500" font-size="40" fill="#82755a">${esc(t[1])}</text>
  <g transform="translate(74 405)"><rect width="330" height="52" rx="26" fill="#fffdf7" stroke="rgba(43,36,24,0.35)" stroke-width="2" stroke-dasharray="6 5"/>
  <text x="165" y="35" text-anchor="middle" font-family="${reserva(l)}'Space Mono', monospace" font-size="24" letter-spacing="${reserva(l) ? 1 : 4}" fill="#2b2418">${esc(t[2])}</text></g></svg>`;
// Las líneas salen del campo «portada» de copy/<idioma>-android.json.
const lineas = Object.fromEntries(
  readdirSync(join(AQUI, "copy")).filter((f) => f.endsWith("-android.json"))
    .map((f) => [f.replace("-android.json", ""), JSON.parse(readFileSync(join(AQUI, "copy", f), "utf8")).portada])
    .filter(([, t]) => t),
);
for (const [l, t] of Object.entries(lineas)) {
  const loro = await sharp(join(AQUI, "assets/loro/loro-coral.png")).resize({ height: 480 }).flop().png().toBuffer();
  mkdirSync(join(AQUI, `output/android-${l}`), { recursive: true });
  await sharp({ create: { width: W, height: H, channels: 4, background: "#f7f2e7" } })
    .composite([{ input: Buffer.from(grano), top: 0, left: 0 }, { input: loro, top: 10, left: 640 }, { input: Buffer.from(texto(t, l)), top: 0, left: 0 }])
    .flatten({ background: "#f7f2e7" }).png().toFile(join(AQUI, `output/android-${l}/portada.png`));
  console.log("✓ portada", l);
}
