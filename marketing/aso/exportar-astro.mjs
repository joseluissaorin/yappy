#!/usr/bin/env node
// Exporta a CSV todas las búsquedas que Astro tiene puntuadas para «Yappy (ASO)»
// y marca si la ficha anterior (anterior.json) y la actual (propuesta.json) las cubren.
//   node marketing/aso/exportar-astro.mjs [fecha]
// Lee la base local de Astro (copia en /tmp para no bloquearla).
import { execSync } from "node:child_process";
import { readFileSync, writeFileSync, mkdirSync, copyFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
const AQUI = dirname(fileURLToPath(import.meta.url));
const FECHA = process.argv[2] ?? new Date().toISOString().slice(0, 10);
const ORIGEN = `${process.env.HOME}/Library/Containers/matteospada.it.ASO/Data/Library/Application Support/Astro`;
mkdirSync("/tmp/astro", { recursive: true });
for (const f of readdirSync(ORIGEN).filter((f) => f.startsWith("Model.sqlite"))) copyFileSync(join(ORIGEN, f), join("/tmp/astro", f));
const rows = JSON.parse(execSync(`sqlite3 -json /tmp/astro/Model.sqlite "select k.ZSTORE as tienda, k.ZTEXT as busqueda, k.ZPOPULARITY as popularidad, k.ZDIFFICULTY as dificultad, k.ZAPPSCOUNT as apps, datetime(k.ZLASTUPDATE + 978307200, 'unixepoch') as medido from ZKEYWORD k join ZAPPLICATION a on k.ZAPPLICATION = a.Z_PK where a.ZNAME = 'Yappy (ASO)' order by k.ZSTORE, k.ZPOPULARITY desc, k.ZDIFFICULTY asc;"`, { encoding: "utf8", maxBuffer: 1 << 26 }));
const P = JSON.parse(readFileSync(join(AQUI, "propuesta.json"), "utf8"));
const A = JSON.parse(readFileSync(join(AQUI, "anterior.json"), "utf8"));
// Qué localizaciones de ASC indexa cada tienda (cross-localización).
const LOC = {us:["en-US","en-GB","en-AU","en-CA","es-MX"],gb:["en-GB"],ca:["en-CA","fr-CA"],au:["en-AU","en-GB"],es:["es-ES","en-GB"],mx:["es-MX","en-GB"],de:["de-DE","en-GB"],fr:["fr-FR","en-GB"],it:["it","en-GB"],pt:["pt-PT","en-GB"],br:["pt-BR","en-GB"],nl:["nl-NL","en-GB"],pl:["pl","en-GB"],ro:["ro","en-GB"],se:["sv","en-GB"],dk:["da","en-GB"],fi:["fi","en-GB"],hr:["hr","en-GB"],sk:["sk","en-GB"],cz:["cs","en-GB"],hu:["hu","en-GB"],gr:["el","en-GB"],ua:["uk","en-GB"],ru:["ru","en-GB"],tr:["tr","en-GB"],sa:["ar-SA","en-GB"],in:["hi","en-GB"],id:["id","en-GB"],vn:["vi","en-GB"],kr:["ko","en-GB"],jp:["ja","en-US"],bg:["en-GB"],ee:["en-GB"],lv:["en-GB"],si:["en-GB"]};
const STOP = new Set(["to","de","a","em","en","di","da","the","for","i","e","y","och","og","ja","és","și","ve","và","ad","ed","à","le","la","van","na","u","v","s","z","w","et","do"]);
const norm = (s) => s.toLowerCase().normalize("NFC");
const toks = (s) => norm(s).split(/[\s,.:;!?()·、・&\-/]+/).filter(Boolean);
const cjk = (s) => /[぀-ヿ㐀-鿿가-힯]/.test(s);
// Aproximación: todas las palabras de la búsqueda están en UNA misma localización (Apple no combina entre localizaciones).
function cubre(ficha, q) {
  const texto = norm(ficha.join(" ")), ts = new Set(toks(ficha.join(",")));
  return toks(q).filter((w) => !STOP.has(w)).every((w) => (cjk(w) ? texto.includes(w) : ts.has(w) || ts.has(w.replace(/s$/, "")) || ts.has(w + "s")));
}
const csvq = (v) => { const s = String(v ?? ""); return /[",\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s; };
const out = ["tienda,busqueda,popularidad,dificultad,apps,medido,cubierta_antes,cubierta_ahora,locales_que_la_cubren_ahora"];
let st = { gan: 0, ganA: 0, ganP: 0 };
for (const r of rows) {
  const locs = LOC[r.tienda] || [];
  const ahora = locs.filter((l) => P[l] && cubre(P[l], r.busqueda));
  const antes = locs.filter((l) => A[l] && cubre(A[l], r.busqueda));
  if (r.popularidad >= 20 && r.dificultad <= 45) { st.gan++; if (antes.length) st.ganA++; if (ahora.length) st.ganP++; }
  out.push([r.tienda, r.busqueda, r.popularidad, r.dificultad, r.apps, r.medido, antes.length ? "sí" : "no", ahora.length ? "sí" : "no", ahora.join(" ")].map(csvq).join(","));
}
writeFileSync(join(AQUI, `astro-yappy-${FECHA}.csv`), out.join("\n") + "\n");
const fichas = ["locale,nombre_antes,subtitulo_antes,keywords_antes,nombre_ahora,subtitulo_ahora,keywords_ahora"];
for (const l of Object.keys(P)) fichas.push([l, ...A[l], ...P[l]].map(csvq).join(","));
writeFileSync(join(AQUI, `fichas-antes-despues-${FECHA}.csv`), fichas.join("\n") + "\n");
console.log(`${rows.length} búsquedas en ${new Set(rows.map((r) => r.tienda)).size} tiendas · ganables (pop≥20, dif≤45): ${st.gan}, cubiertas antes ${st.ganA}, ahora ${st.ganP}`);
