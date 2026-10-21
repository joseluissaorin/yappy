#!/usr/bin/env node
// LA VOZ PREGRABADA DEL PASEO: renderiza de antemano todo lo que el loro dice
// en el paseo (los 31 diccionarios + la primera frase de los cuentos de la
// casa), con la voz del loro, para que cada paso suene AL INSTANTE.
//
//   node tooling/paseo/voz.mjs                 regenera lo que falte o haya cambiado y borra lo huérfano
//   node tooling/paseo/voz.mjs --comprobar     solo avisa (sale con 1 si hay audios viejos, ausentes o huérfanos)
//   node tooling/paseo/voz.mjs --solo es,en    limita los idiomas
//   node tooling/paseo/voz.mjs --forzar        re-sintetiza todo
//   node tooling/paseo/voz.mjs --subir         sube al espejo R2 lo que no esté ya allí
//
// DENTRO DEL PAQUETE solo van español e inglés (EMPAQUETADOS); los otros 29
// idiomas viven en el espejo R2 de la casa
// (https://modelos.yappy.joseluissaorin.com/paseo-voz/<fichero>) y la app
// los descarga al arrancar en ese idioma o al elegirlo (paseo_voz.rs). Aquí
// se guardan en tooling/paseo/voz-remota/ (fuera de git: se regeneran).
// `--subir` necesita ~/.claude/skills/cloudflare/credentials.env y un token
// temporal en /tmp/cf-tmp-token.json (skill de Cloudflare: acuñar, usar,
// revocar).
//
// Cada fichero lleva en el nombre el hash del texto que dice:
//   <lang>.<clave>.<hash>.m4a
// (hash = FNV-1a de 64 bits sobre «lang\ntexto», 10 cifras hex). La app
// calcula el mismo hash con el texto que va a decir (src-tauri/src/paseo_voz.rs):
// si el texto cambió y nadie regeneró, el hash no casa, la app sintetiza en
// vivo y este script lo marca como VIEJO. El manifiesto
// (tooling/paseo/voz-manifiesto.json) guarda texto, duración y tamaño.
//
// Requisitos: target/release/yappy-cli (cargo build --release -p yappy-core
// --bin yappy-cli), el modelo Supertonic 3 en Application Support, ffmpeg y
// afconvert (macOS). Formato: HE-AAC mono, 24 kHz, 24 kbps (.m4a), sin
// silencio en cabeza ni cola; lo reproducen AVAudioPlayer (iOS) y
// MediaPlayer (Android) sin códecs extra.
import { readFileSync, writeFileSync, existsSync, mkdirSync, readdirSync, unlinkSync, statSync, rmSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { resolve, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { tmpdir } from "node:os";

const aqui = dirname(fileURLToPath(import.meta.url));
const raiz = resolve(aqui, "../..");
const DIR_I18N = join(raiz, "yappy-app/src/lib/i18n");
const DIR_CUENTOS = join(raiz, "yappy-app/resources/cuentos");
const SALIDA = join(raiz, "yappy-app/resources/paseo-voz");
const REMOTA = join(aqui, "voz-remota");
const EMPAQUETADOS = new Set(["es", "en"]);
const dirDe = (lang) => (EMPAQUETADOS.has(lang) ? SALIDA : REMOTA);
const ESPEJO = "https://modelos.yappy.joseluissaorin.com/paseo-voz";
const BUCKET = "yappy-modelos";
const MANIFIESTO = join(aqui, "voz-manifiesto.json");
const CLI = join(raiz, "target/release/yappy-cli");
const MODELO = process.env.YAPPY_MODELO || `${process.env.HOME}/Library/Application Support/com.yappy.app/models/supertonic-3`;

// La voz del loro: la de fábrica (Settings::default().voice). Si cambia,
// cambia también VOZ_LORO en src-tauri/src/paseo_voz.rs y regenera con --forzar.
const RECETA = { voz: "Alex", velocidad: 1.0, pasos: 12, semilla: 7, lufs: -23, kbps: 24, codec: "aach", hz: 24000 };

// Lo que el paseo DICE (paseo.svelte.ts / +page.svelte). Lo demás es solo texto.
const DICHAS = [
  /^paseo\.dice\.(?!gestos$|cuerda$)/,
  /^paseo\.hola_corto$/,
  /^paseo\.velocidad\.demo$/,
  /^paseo\.avisos\.gracias$/,
  /^paseo\.(que|cuando)\.comentario\./,
  /^paseo\.cuanto\.dice\./,
];

const args = process.argv.slice(2);
const comprobar = args.includes("--comprobar");
const forzar = args.includes("--forzar");
const subir = args.includes("--subir");
const soloIdx = args.indexOf("--solo");
const solo = soloIdx >= 0 ? new Set(args[soloIdx + 1].split(",")) : null;

/// FNV-1a de 64 bits: el mismo cálculo que `paseo_voz::huella` en Rust.
export function huella(lang, texto) {
  let h = 0xcbf29ce484222325n;
  for (const b of Buffer.from(`${lang}\n${texto}`, "utf8")) {
    h ^= BigInt(b);
    h = (h * 0x100000001b3n) & 0xffffffffffffffffn;
  }
  return h.toString(16).padStart(16, "0").slice(0, 10);
}

function diccionario(fichero) {
  const src = readFileSync(fichero, "utf8")
    .replace(/^\s*export default\s*/m, "return ")
    .replace(/\}\s*as\s+Record<string,\s*string>\s*;?\s*$/, "};");
  return new Function(src)();
}

/// La primera frase de un cuento, igual que `parsear_cuento` en commands.rs
/// (corte en el primer . ! ? … pasado el byte 20; si no, 160 bytes).
function primeraFrase(crudo) {
  if (!crudo.startsWith("---")) return null;
  const resto = crudo.slice(3);
  const k = resto.indexOf("\n---");
  if (k < 0) return null;
  const cabecera = resto.slice(0, k);
  const cuerpo = resto.slice(k + 4).trim();
  let idioma = "es";
  for (const l of cabecera.split("\n")) {
    const i = l.indexOf(":");
    if (i > 0 && l.slice(0, i).trim() === "idioma") idioma = l.slice(i + 1).trim().replace(/^["']|["']$/g, "");
  }
  const primera = (cuerpo.split("\n").find((l) => l.trim()) || "").trim();
  const bytes = Buffer.from(primera, "utf8");
  let corte = Math.min(bytes.length, 160);
  let i = 0;
  for (const c of primera) {
    if (i > 20 && ".!?…".includes(c)) {
      corte = i + Buffer.byteLength(c);
      break;
    }
    i += Buffer.byteLength(c);
  }
  return { idioma, texto: bytes.subarray(0, corte).toString("utf8") };
}

// 1) Lo que hay que decir.
const quiero = []; // { lang, clave, texto, fichero }
const dinamicas = [];
for (const f of readdirSync(DIR_I18N).filter((f) => /^[a-z]{2}\.ts$/.test(f)).sort()) {
  const lang = f.slice(0, 2);
  if (solo && !solo.has(lang)) continue;
  const d = diccionario(join(DIR_I18N, f));
  for (const [clave, texto] of Object.entries(d)) {
    if (!DICHAS.some((r) => r.test(clave))) continue;
    if (texto.includes("{")) {
      dinamicas.push(`${lang}:${clave}`);
      continue;
    }
    const corta = clave.replace(/^paseo\./, "");
    quiero.push({ lang, clave: corta, texto, fichero: `${lang}.${corta}.${huella(lang, texto)}.m4a` });
  }
}
for (const f of existsSync(DIR_CUENTOS) ? readdirSync(DIR_CUENTOS).filter((f) => f.endsWith(".md") && f !== "README.md") : []) {
  const c = primeraFrase(readFileSync(join(DIR_CUENTOS, f), "utf8"));
  if (!c || (solo && !solo.has(c.idioma))) continue;
  const clave = `cuento.${f.slice(0, -3)}`;
  quiero.push({ lang: c.idioma, clave, texto: c.texto, fichero: `${c.idioma}.${clave}.${huella(c.idioma, c.texto)}.m4a` });
}

// 2) Lo que ya hay (en el paquete o en la carpeta de lo remoto).
mkdirSync(SALIDA, { recursive: true });
mkdirSync(REMOTA, { recursive: true });
const hayEn = (dir) => readdirSync(dir).filter((f) => f.endsWith(".m4a")).map((f) => join(dir, f));
const hayRutas = [...hayEn(SALIDA), ...hayEn(REMOTA)];
const hay = new Set(hayRutas.map((r) => r.slice(r.lastIndexOf("/") + 1)));
const queridos = new Set(quiero.map((q) => join(dirDe(q.lang), q.fichero)));
const faltan = quiero.filter((q) => forzar || !existsSync(join(dirDe(q.lang), q.fichero)));
const huerfanos = hayRutas.filter((r) => {
  const f = r.slice(r.lastIndexOf("/") + 1);
  return !queridos.has(r) && (!solo || solo.has(f.slice(0, 2)));
});
const viejos = faltan.filter((q) => [...hay].some((h) => h.startsWith(`${q.lang}.${q.clave}.`)));

if (dinamicas.length) console.log(`(con partes dinámicas, en vivo: ${dinamicas.length})`);
console.log(`${quiero.length} frases · ${quiero.length - faltan.length} al día · ${faltan.length} por hacer (${viejos.length} con texto cambiado) · ${huerfanos.length} huérfanos`);
for (const v of viejos.slice(0, 20)) console.log(`  VIEJO ${v.lang}.${v.clave}`);

if (comprobar && !subir) process.exit(faltan.length || huerfanos.length ? 1 : 0);

// 3) Sintetizar lo que falte (una sola carga del modelo por pasada).
if (faltan.length) {
  if (!existsSync(CLI)) throw new Error(`falta ${CLI}: cargo build --release -p yappy-core --bin yappy-cli`);
  const tmp = join(tmpdir(), `paseo-voz-${process.pid}`);
  mkdirSync(tmp, { recursive: true });
  const lote = faltan.map((q, i) =>
    JSON.stringify({ texto: q.texto, lang: q.lang, voz: RECETA.voz, velocidad: RECETA.velocidad, pasos: RECETA.pasos, semilla: RECETA.semilla, out: join(tmp, `${i}.wav`) }),
  );
  writeFileSync(join(tmp, "lote.jsonl"), lote.join("\n"));
  console.log(`sintetizando ${faltan.length} frases con ${RECETA.voz}…`);
  const r = spawnSync(CLI, ["lote", "--root", MODELO, "--in", join(tmp, "lote.jsonl")], {
    stdio: ["ignore", "inherit", "inherit"],
    env: { ...process.env, RUST_LOG: "error" },
  });
  if (r.status !== 0) throw new Error("yappy-cli lote falló");

  faltan.forEach((q, i) => {
    const wav = join(tmp, `${i}.wav`);
    const limpio = join(tmp, `${i}-n.wav`);
    // Sin silencio en cabeza (que suene al instante) ni en cola, sonoridad
    // pareja entre frases y un respiro mínimo para que el códec no muerda
    // el final.
    const recorte = "silenceremove=start_periods=1:start_threshold=-50dB:start_silence=0.02";
    let e = spawnSync("ffmpeg", ["-v", "error", "-y", "-i", wav, "-af",
      `${recorte},areverse,${recorte.replace("0.02", "0.05")},areverse,loudnorm=I=${RECETA.lufs}:TP=-1.5:LRA=11,apad=pad_dur=0.1,aresample=${RECETA.hz}`,
      "-ac", "1", limpio]);
    if (e.status !== 0) throw new Error(`ffmpeg ${q.fichero}: ${e.stderr}`);
    const destino = join(dirDe(q.lang), q.fichero);
    e = spawnSync("afconvert", ["-f", "m4af", "-d", RECETA.codec, "-b", String(RECETA.kbps * 1000), "-q", "127", limpio, destino]);
    if (e.status !== 0) throw new Error(`afconvert ${q.fichero} (hace falta macOS): ${e.stderr}`);
  });
  rmSync(tmp, { recursive: true, force: true });
}

// 4) Fuera lo huérfano (textos que cambiaron o claves que ya no existen).
for (const r of huerfanos) unlinkSync(r);

// 5) El manifiesto.
const anterior = existsSync(MANIFIESTO) ? JSON.parse(readFileSync(MANIFIESTO, "utf8")) : { clips: {} };
const clips = solo ? anterior.clips || {} : {};
for (const l of solo || []) delete clips[l];
for (const q of quiero) {
  const p = join(dirDe(q.lang), q.fichero);
  const dur = spawnSync("ffprobe", ["-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", p], { encoding: "utf8" });
  (clips[q.lang] ||= {})[q.clave] = { fichero: q.fichero, empaquetado: EMPAQUETADOS.has(q.lang), texto: q.texto, segundos: +parseFloat(dur.stdout).toFixed(2), bytes: statSync(p).size };
}
const total = Object.values(clips).flatMap((c) => Object.values(c));
const bytes = total.reduce((a, c) => a + c.bytes, 0);
const enPaquete = total.filter((c) => c.empaquetado).reduce((a, c) => a + c.bytes, 0);
const segundos = total.reduce((a, c) => a + c.segundos, 0);
writeFileSync(MANIFIESTO, JSON.stringify({ receta: RECETA, ficheros: total.length, bytes, bytes_en_paquete: enPaquete, segundos: +segundos.toFixed(1), clips }, null, 1) + "\n");
console.log(`✓ ${total.length} audios · ${(bytes / 1048576).toFixed(2)} MB en total · ${(enPaquete / 1048576).toFixed(2)} MB dentro del paquete (${[...EMPAQUETADOS].join(", ")}) · ${(segundos / 60).toFixed(1)} min de voz`);

// 6) Subir al espejo R2 lo que falte allí (HEAD por fichero; solo sube lo
//    que no está o cambió de tamaño). El nombre lleva el hash: nunca se
//    sobrescribe un audio con otro texto, así que la caché larga es segura.
if (subir) {
  const { homedir } = await import("node:os");
  const env = Object.fromEntries(
    readFileSync(`${homedir()}/.claude/skills/cloudflare/credentials.env`, "utf8")
      .split("\n").filter((l) => l.includes("=") && !l.startsWith("#"))
      .map((l) => l.replace(/^export\s+/, "").split("=")).map(([k, v]) => [k.trim(), v.trim().replace(/^["']|["']$/g, "")]),
  );
  const token = JSON.parse(readFileSync("/tmp/cf-tmp-token.json", "utf8")).value;
  const cola = quiero.slice();
  let subidos = 0, iguales = 0, fallos = 0;
  async function obrero() {
    for (let q = cola.shift(); q; q = cola.shift()) {
      const ruta = join(dirDe(q.lang), q.fichero);
      const cuerpo = readFileSync(ruta);
      const h = await fetch(`${ESPEJO}/${q.fichero}`, { method: "HEAD" }).catch(() => null);
      if (h && h.ok && Number(h.headers.get("content-length")) === cuerpo.length) { iguales++; continue; }
      const r = await fetch(`https://api.cloudflare.com/client/v4/accounts/${env.CF_ACCOUNT_ID}/r2/buckets/${BUCKET}/objects/paseo-voz/${q.fichero}`, {
        method: "PUT",
        headers: { Authorization: `Bearer ${token}`, "Content-Type": "audio/mp4", "Cache-Control": "public, max-age=31536000, immutable" },
        body: cuerpo,
      });
      if (r.ok) subidos++;
      else { fallos++; console.error(`  ✗ ${q.fichero} → ${r.status} ${(await r.text()).slice(0, 200)}`); }
    }
  }
  await Promise.all(Array.from({ length: 8 }, obrero));
  console.log(`R2: ${subidos} subidos · ${iguales} ya estaban · ${fallos} fallos`);
  if (fallos) process.exit(1);
}
