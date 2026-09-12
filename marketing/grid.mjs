// Rejilla de revisión: los 10 slots de un idioma en una sola imagen.
import sharp from "sharp";
import { readdirSync, mkdirSync } from "node:fs";
const [L = "es", device = "iphone"] = process.argv.slice(2);
const dir = `output/${L}/${device}`;
const files = readdirSync(dir).filter((f) => /^\d\d\.png$/.test(f)).sort();
const W = 360, H = Math.round(W * (device === "ipad" ? 2752 / 2064 : 2796 / 1290)), cols = 5;
const rows = Math.ceil(files.length / cols);
const comps = await Promise.all(files.map(async (f, i) => ({ input: await sharp(`${dir}/${f}`).resize(W, H).png().toBuffer(), left: (i % cols) * (W + 12), top: Math.floor(i / cols) * (H + 12) })));
mkdirSync("output/_rejillas", { recursive: true });
await sharp({ create: { width: cols * (W + 12), height: rows * (H + 12), channels: 3, background: "#d9d0bd" } }).composite(comps).png().toFile(`output/_rejillas/${L}-${device}.png`);
console.log(`output/_rejillas/${L}-${device}.png`);
