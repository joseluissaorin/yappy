#!/usr/bin/env node
// El icono de la web y el manifiesto.
//
// Una sola fuente de verdad: brand/icono.svg, el cuadro coral con el loro que
// también usa la app. Se rasteriza a los dos tamaños que pide un navegador:
// 180 px (el que se lleva iOS al añadir a pantalla de inicio, y que vale de
// favicon moderno) y 512 px (el que pide el manifiesto para instalar).
//
//   node web/herramientas/iconos.mjs
//
// El manifiesto declara display "browser" a propósito: Yappy de verdad es una
// app nativa, y la web es su escaparate. Si se declarara "standalone", Android
// ofrecería instalar el escaparate como si fuese la app, que es justo lo que
// no queremos.

import { writeFileSync, statSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import sharp from "sharp";

const AQUI = dirname(fileURLToPath(import.meta.url));
const WEB = join(AQUI, "..");
const RAIZ = join(WEB, "..");
const PUBLIC = join(WEB, "public");
const FUENTE = join(RAIZ, "brand", "icono.svg");

const kb = (p) => `${(statSync(p).size / 1024).toFixed(0)} KB`;

// density: el SVG declara un viewBox de 64, así que hay que subirle la densidad
// para que librsvg lo dibuje grande en vez de rasterizar a 64 y ampliar borroso.
const rasterizar = async (lado, salida) => {
  await sharp(FUENTE, { density: Math.ceil((lado / 64) * 72) })
    .resize(lado, lado).png({ compressionLevel: 9 }).toFile(salida);
  console.log(`${salida} · ${kb(salida)}`);
};

await rasterizar(180, join(PUBLIC, "favicon.png"));
await rasterizar(512, join(PUBLIC, "icono-512.png"));

const manifiesto = {
  name: "Yappy",
  short_name: "Yappy",
  start_url: "/",
  display: "browser",
  theme_color: "#f7f2e7",
  background_color: "#f7f2e7",
  icons: [
    { src: "/favicon.png", sizes: "180x180", type: "image/png" },
    { src: "/icono-512.png", sizes: "512x512", type: "image/png" },
    // "maskable" para que Android recorte el cuadro coral a su antojo sin
    // comerse el loro: el icono ya trae margen de sobra alrededor del bicho.
    { src: "/icono-512.png", sizes: "512x512", type: "image/png", purpose: "maskable" },
  ],
};
const destino = join(PUBLIC, "manifiesto.webmanifest");
writeFileSync(destino, JSON.stringify(manifiesto, null, 2) + "\n");
console.log(`${destino} · ${kb(destino)}`);
