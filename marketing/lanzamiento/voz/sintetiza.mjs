#!/usr/bin/env node
// Sintetiza «Media voz» beat a beat con la voz de la app (Supertonic 3 vía
// yappy-cli), pega los wav con las pausas del guion y escribe la partitura
// (`tiempos.json`: inicio y fin de cada beat) que consume la animación.
//
//   node sintetiza.mjs [cuento.json] [salida/]     (caché por hash: solo re-sintetiza lo que cambia)
import { readFileSync, writeFileSync, existsSync, mkdirSync, statSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { resolve, dirname } from "node:path";
import { createHash } from "node:crypto";
import { fileURLToPath } from "node:url";

const aqui = dirname(fileURLToPath(import.meta.url));
const guionPath = resolve(process.argv[2] || resolve(aqui, "cuento.json"));
const salida = resolve(process.argv[3] || resolve(aqui, "beats"));
mkdirSync(salida, { recursive: true });

const CLI = resolve(aqui, "../../../target/release/yappy-cli");
const MODELO = `${process.env.HOME}/Library/Application Support/com.yappy.app/models/supertonic-3`;
const g = JSON.parse(readFileSync(guionPath, "utf8"));

function duracion(wav) {
  const r = spawnSync("ffprobe", ["-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", wav], { encoding: "utf8" });
  return parseFloat(r.stdout.trim());
}

// 1) síntesis (cacheada)
const wavs = [];
for (const b of g.beats) {
  const voz = g.voces[b.voz];
  const hash = createHash("sha1").update(`${voz}|${g.velocidad}|${b.texto}`).digest("hex").slice(0, 10);
  const wav = resolve(salida, `${b.id}-${hash}.wav`);
  if (!existsSync(wav) || statSync(wav).size < 1000) {
    process.stdout.write(`${b.id} (${voz}) … `);
    const r = spawnSync(CLI, ["--root", MODELO, "--voice", voz, "--lang", "es", "--speed", String(g.velocidad),
      "--text", b.texto, "--out", wav], { encoding: "utf8", env: { ...process.env, RUST_LOG: "error" } });
    if (r.status !== 0) { console.error(r.stderr); throw new Error(`falló ${b.id}`); }
    console.log(`${duracion(wav).toFixed(2)} s`);
  }
  wavs.push({ ...b, wav, dur: duracion(wav) });
}

// 2) partitura: pausa según lo que viene después (verso, cambio de escena, beat normal)
let t = 0.8; // aire antes de la primera frase
const partitura = [];
for (let i = 0; i < wavs.length; i++) {
  const b = wavs[i], sig = wavs[i + 1];
  partitura.push({ id: b.id, voz: b.voz, escena: b.escena, texto: b.texto, inicio: +t.toFixed(3), fin: +(t + b.dur).toFixed(3) });
  t += b.dur;
  if (!sig) break;
  if (b.voz === "lorca" || sig.voz === "lorca") t += g.pausa.verso;
  else if (sig.escena !== b.escena) t += g.pausa.escena;
  else t += g.pausa.beat;
}
const total = t + 4.0; // el silencio final del guion (⏸ 4 s)
writeFileSync(resolve(aqui, "tiempos.json"), JSON.stringify({ titulo: g.titulo, total: +total.toFixed(3), beats: partitura }, null, 1));

// 3) mezcla: un solo wav con cada beat en su sitio
const inputs = [], delays = [];
wavs.forEach((b, i) => { inputs.push("-i", b.wav); const ms = Math.round(partitura[i].inicio * 1000); delays.push(`[${i}:a]adelay=${ms}|${ms}[a${i}]`); });
const mix = wavs.map((_, i) => `[a${i}]`).join("") + `amix=inputs=${wavs.length}:normalize=0,apad=whole_dur=${total.toFixed(3)},alimiter=limit=0.95[out]`;
const voz = resolve(aqui, "media-voz.wav");
const r = spawnSync("ffmpeg", ["-v", "error", "-y", ...inputs, "-filter_complex", delays.join(";") + ";" + mix, "-map", "[out]", "-ar", "44100", "-ac", "1", voz], { encoding: "utf8" });
if (r.status !== 0) { console.error(r.stderr); process.exit(1); }
console.log(`✓ ${voz}  (${total.toFixed(1)} s, ${partitura.length} beats) · tiempos.json`);
