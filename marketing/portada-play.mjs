// El gráfico de portada de Google Play (1024x500): papel, el loro coral y el nombre en Baloo 2.
import sharp from "sharp";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
const AQUI = dirname(fileURLToPath(import.meta.url));
const W = 1024, H = 500;
const grano = `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}"><filter id="g"><feTurbulence type="fractalNoise" baseFrequency="0.8" numOctaves="2" stitchTiles="stitch"/><feColorMatrix values="0 0 0 0 0.2  0 0 0 0 0.15  0 0 0 0 0.06  0 0 0 0.05 0"/></filter><rect width="${W}" height="${H}" filter="url(#g)"/></svg>`;
const texto = (t) => `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}">
  <text x="70" y="235" font-family="'Baloo 2', sans-serif" font-weight="800" font-size="150" letter-spacing="-3" fill="#2b2418">Yappy</text>
  <text x="74" y="310" font-family="'Literata', serif" font-weight="500" font-size="40" fill="#82755a">${t[0]}</text>
  <text x="74" y="365" font-family="'Literata', serif" font-weight="500" font-size="40" fill="#82755a">${t[1]}</text>
  <g transform="translate(74 405)"><rect width="330" height="52" rx="26" fill="#fffdf7" stroke="rgba(43,36,24,0.35)" stroke-width="2" stroke-dasharray="6 5"/>
  <text x="165" y="35" text-anchor="middle" font-family="'Space Mono', monospace" font-size="24" letter-spacing="4" fill="#2b2418">${t[2]}</text></g></svg>`;
const lineas = { es: ["Cualquier texto,", "en voz alta.", "SIN NUBE"], en: ["Any text,", "read aloud.", "NO CLOUD"] };
for (const [l, t] of Object.entries(lineas)) {
  const loro = await sharp(join(AQUI, "assets/loro/loro-coral.png")).resize({ height: 480 }).flop().png().toBuffer();
  await sharp({ create: { width: W, height: H, channels: 4, background: "#f7f2e7" } })
    .composite([{ input: Buffer.from(grano), top: 0, left: 0 }, { input: loro, top: 10, left: 640 }, { input: Buffer.from(texto(t)), top: 0, left: 0 }])
    .flatten({ background: "#f7f2e7" }).png().toFile(join(AQUI, `output/android-${l}/portada.png`));
  console.log("✓ portada", l);
}
