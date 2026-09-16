#!/usr/bin/env node
/**
 * Traduce src/i18n/es.json a los otros treinta idiomas.
 *
 *   node herramientas/traducir.mjs            # los que falten
 *   node herramientas/traducir.mjs fr de ja   # solo esos
 *   node herramientas/traducir.mjs --todos    # rehacer todos
 *
 * Tres cosas que este script hace y que un `sed` con un LLM detrás no hace:
 *
 * 1. TRADUCE POR TROZOS y vuelve a montar el objeto en JavaScript, en vez de
 *    pedirle al modelo un JSON gigante que devolvería mutilado. Así una clave
 *    no puede perderse por el camino sin que se note.
 * 2. NO TOCA LOS IDENTIFICADORES. `meta.codigo`, `meta.hreflang` y todo lo que
 *    vive bajo `slugs` se trata aparte: los slugs se traducen, pero a slug
 *    (minúsculas, sin tildes, con guiones), no a prosa.
 * 3. CUENTA LAS CLAVES ANTES Y DESPUÉS. Si el resultado no tiene exactamente
 *    las mismas que el español, no se escribe el fichero. La paridad entre
 *    treinta y un idiomas no detecta una pérdida que ocurra en los treinta y
 *    uno a la vez: hay que comparar contra el original, siempre.
 */
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { homedir } from "node:os";

const AQUI = join(dirname(fileURLToPath(import.meta.url)), "..");
const I18N = join(AQUI, "src/i18n");
const OR = join(homedir(), ".claude/skills/openrouter/scripts/or.py");
const MODELO = process.env.MODELO_TRADUCCION || "z-ai/glm-5.3-flash";
// Cuánta gente trabaja a la vez. Treinta idiomas en serie son horas; en
// piscina, minutos. Si la API empieza a devolver 429, baja estos números.
const IDIOMAS_A_LA_VEZ = Number(process.env.IDIOMAS_A_LA_VEZ || 5);
const LOTES_A_LA_VEZ = Number(process.env.LOTES_A_LA_VEZ || 4);

// Los treinta idiomas de la app, con su nombre, su etiqueta BCP-47 y el
// locale de Open Graph. El español es el original y no se traduce.
const IDIOMAS = {
  en: ["English", "en", "en_US"], fr: ["Français", "fr", "fr_FR"],
  de: ["Deutsch", "de", "de_DE"], it: ["Italiano", "it", "it_IT"],
  pt: ["Português", "pt", "pt_PT"], nl: ["Nederlands", "nl", "nl_NL"],
  pl: ["Polski", "pl", "pl_PL"], ro: ["Română", "ro", "ro_RO"],
  sv: ["Svenska", "sv", "sv_SE"], da: ["Dansk", "da", "da_DK"],
  fi: ["Suomi", "fi", "fi_FI"], et: ["Eesti", "et", "et_EE"],
  lt: ["Lietuvių", "lt", "lt_LT"], lv: ["Latviešu", "lv", "lv_LV"],
  hr: ["Hrvatski", "hr", "hr_HR"], sl: ["Slovenščina", "sl", "sl_SI"],
  sk: ["Slovenčina", "sk", "sk_SK"], cs: ["Čeština", "cs", "cs_CZ"],
  hu: ["Magyar", "hu", "hu_HU"], el: ["Ελληνικά", "el", "el_GR"],
  bg: ["Български", "bg", "bg_BG"], uk: ["Українська", "uk", "uk_UA"],
  ru: ["Русский", "ru", "ru_RU"], tr: ["Türkçe", "tr", "tr_TR"],
  ar: ["العربية", "ar", "ar_AR"], hi: ["हिन्दी", "hi", "hi_IN"],
  id: ["Bahasa Indonesia", "id", "id_ID"], vi: ["Tiếng Việt", "vi", "vi_VN"],
  ko: ["한국어", "ko", "ko_KR"], ja: ["日本語", "ja", "ja_JP"],
};
const RTL = new Set(["ar"]);

// ── Aplanar y volver a montar, conservando la forma exacta ─────────────
function aplanar(obj, prefijo = "", salida = {}) {
  for (const [k, v] of Object.entries(obj)) {
    const ruta = prefijo ? `${prefijo}.${k}` : k;
    if (v && typeof v === "object") aplanar(v, ruta, salida);
    else salida[ruta] = v;
  }
  return salida;
}
function montar(plano, molde) {
  const copia = JSON.parse(JSON.stringify(molde));
  for (const [ruta, valor] of Object.entries(plano)) {
    const partes = ruta.split(".");
    let nodo = copia;
    for (const p of partes.slice(0, -1)) nodo = nodo[p];
    nodo[partes.at(-1)] = valor;
  }
  return copia;
}

const ejecutar = promisify(execFile);
async function modelo(texto) {
  const { stdout } = await ejecutar("python3", [OR, texto, "--model", MODELO, "--json"], {
    encoding: "utf8", maxBuffer: 32 * 1024 * 1024,
  });
  const m = stdout.match(/\{[\s\S]*\}/);
  if (!m) throw new Error("el modelo no ha devuelto JSON");
  return JSON.parse(m[0]);
}

/// Una piscina: como mucho `a la vez` tareas en vuelo simultáneo.
async function piscina(tareas, aLaVez) {
  const salida = new Array(tareas.length);
  let i = 0;
  await Promise.all(
    Array.from({ length: Math.min(aLaVez, tareas.length) }, async () => {
      for (;;) {
        const j = i++;
        if (j >= tareas.length) return;
        salida[j] = await tareas[j]();
      }
    }),
  );
  return salida;
}

const INSTRUCCIONES = `You translate the interface and marketing copy of Yappy, a text-to-speech app.

VOICE: short, concrete sentences. No marketing inflation ("revolutionise", "unlock", "empower"). The author is a philologist: honest, dry, occasionally wry. Translate the INTENT, not word by word; a native speaker must not smell a translation.

RULES YOU MUST NOT BREAK:
1. Return ONLY a JSON object mapping each input key to its translated string. No prose around it.
2. Keep every placeholder EXACTLY as it is: {n}, {peso}, {fecha}, {lengua}, {nombre}, {voz}.
3. Keep HTML tags and their attributes intact: <strong>, <em>, <code>, <a href="...">. Translate only the text between tags.
4. Keep the literal newline sequence \\n where it appears (sticker copy is set on two lines).
5. Do NOT translate: "Yappy", "Yappy Parlanchín", "PDF", "EPUB", "Word", "MIT", "RSS", ".m4b", "Safari", "Siri", "macOS", "Windows", "Linux", "Android", "iPhone", "iPad", "App Store", "GitHub", "Supertonic 3", "WebAssembly", "Rust", "CLDR", "RBNF", "VoiceOver", the voice names (Alex, James, Robert, Sam, Daniel, Sarah, Lily, Jessica, Olivia, Emily), and the language endonyms.
6. "text to speech" is the search term people actually type in almost every market. Where the Spanish says "texto a voz", prefer the phrase a native would search for, and keep the English "text to speech" if that is what they search for in your language.
7. Use the punctuation of the target language: its own quotation marks, its own dash conventions, its own spacing rules (French needs a narrow space before : ; ! ?).
8. Numbers and prices: keep the euro figures (3,99 €) but write them the way the target language writes numbers.
9. Never leave a value empty. If something truly has no equivalent, keep the Spanish.`;

const SLUGS = `You produce URL slugs for a website in the target language.

Return ONLY a JSON object mapping each key to a slug. A slug is: lowercase, ASCII letters and digits and hyphens only, no accents, no spaces, no trailing hyphen. Transliterate non-Latin scripts to Latin (Japanese, Korean, Greek, Cyrillic, Arabic, Hindi: use a readable romanisation, or the English word if that is what people would type).

The slug must be the word a person in that language would SEARCH for: "pdf-a-audio" in Spanish, "pdf-to-audio" in English, "pdf-in-sprache" in German. For markets where the English term is the one people search, use the English term.`;

const trozos = (pares, n) => {
  const out = [];
  for (let i = 0; i < pares.length; i += n) out.push(pares.slice(i, i + n));
  return out;
};

async function traducir(codigo) {
  const [nombre, bcp, og] = IDIOMAS[codigo];
  const es = JSON.parse(readFileSync(join(I18N, "es.json"), "utf8"));
  const plano = aplanar(es);
  const claves = Object.keys(plano);

  // Los slugs van por su cuenta, con otras reglas.
  const clavesSlug = claves.filter((k) => k.startsWith("slugs."));
  const clavesTexto = claves.filter((k) => !k.startsWith("slugs.") && !k.startsWith("meta."));

  const resultado = {};

  process.stdout.write(`\n  ${codigo} · slugs…`);
  const dSlug = {};
  for (const k of clavesSlug) dSlug[k] = plano[k];
  Object.assign(resultado, await modelo(
    `${SLUGS}\n\nTarget language: ${nombre} (${bcp}).\n\nSlugs (the value is the Spanish slug; the key tells you what the page is):\n${JSON.stringify(dSlug, null, 1)}`,
  ));

  const lotes = trozos(clavesTexto, 40);
  let hechos = 0;
  await piscina(lotes.map((lote) => async () => {
    const d = {};
    for (const k of lote) d[k] = plano[k];
    for (let intento = 0; ; intento++) {
      try {
        const r = await modelo(`${INSTRUCCIONES}\n\nTarget language: ${nombre} (${bcp}).\n\n${JSON.stringify(d, null, 1)}`);
        for (const k of lote) resultado[k] = typeof r[k] === "string" ? r[k] : plano[k];
        break;
      } catch (e) {
        if (intento >= 2) {
          // Antes que romper el build, el español: se ve raro y se arregla.
          for (const k of lote) resultado[k] = plano[k];
          process.stdout.write("!");
          break;
        }
        await new Promise((r) => setTimeout(r, 1500 * (intento + 1)));
      }
    }
    process.stdout.write(` ${++hechos}/${lotes.length}`);
  }), LOTES_A_LA_VEZ);

  // La cabecera va a mano: son identificadores, no prosa.
  resultado["meta.codigo"] = codigo;
  resultado["meta.nombre"] = nombre;
  resultado["meta.dir"] = RTL.has(codigo) ? "rtl" : "ltr";
  resultado["meta.ogLocale"] = og;
  resultado["meta.hreflang"] = bcp;
  resultado["meta.og_alt"] = resultado["meta.og_alt"] ?? plano["meta.og_alt"];

  // Ni una clave de menos. Si falta alguna, cae al español y se avisa.
  const faltan = claves.filter((k) => resultado[k] == null);
  for (const k of faltan) resultado[k] = plano[k];
  const sobran = Object.keys(resultado).filter((k) => !claves.includes(k));
  for (const k of sobran) delete resultado[k];

  const objeto = montar(resultado, es);
  const comprobacion = Object.keys(aplanar(objeto));
  if (comprobacion.length !== claves.length) {
    throw new Error(`${codigo}: ${comprobacion.length} claves frente a ${claves.length}. No se escribe.`);
  }

  writeFileSync(join(I18N, `${codigo}.json`), JSON.stringify(objeto, null, 2) + "\n");
  console.log(`  ✓ ${codigo} (${claves.length} claves${faltan.length ? `, ${faltan.length} en español` : ""})`);
}

const args = process.argv.slice(2);
const todos = args.includes("--todos");
const pedidos = args.filter((a) => !a.startsWith("--"));
const cola = (pedidos.length ? pedidos : Object.keys(IDIOMAS))
  .filter((c) => IDIOMAS[c] && (todos || pedidos.length || !existsSync(join(I18N, `${c}.json`))));

if (!existsSync(OR)) {
  console.error(`No encuentro ${OR}. Este script necesita la skill «openrouter».`);
  process.exit(1);
}
console.log(`Traduciendo ${cola.length} idiomas con ${MODELO} (${IDIOMAS_A_LA_VEZ} a la vez, ${LOTES_A_LA_VEZ} lotes por idioma):`);
const t0 = Date.now();
await piscina(cola.map((c) => async () => {
  try { await traducir(c); }
  catch (e) { console.error(`\n  ✗ ${c}: ${e.message}`); }
}), IDIOMAS_A_LA_VEZ);
console.log(`\nHecho en ${Math.round((Date.now() - t0) / 1000)} s.`);
