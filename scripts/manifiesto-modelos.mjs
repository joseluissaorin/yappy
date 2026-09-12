#!/usr/bin/env node
// Escribe y sube el MANIFIESTO de los modelos al espejo de R2: la lista de
// ficheros (con tamaño real, leído del propio bucket) que la extensión de
// Background Assets descarga durante la instalación de iOS, y que el
// descargador de la app usa como fuente de verdad en todas las plataformas.
//
//   node scripts/manifiesto-modelos.mjs            # escribe y sube
//   node scripts/manifiesto-modelos.mjs --solo-ver # solo imprime
//
// Necesita ~/.claude/skills/cloudflare/credentials.env y un token temporal
// en /tmp/cf-tmp-token.json (ver la skill de Cloudflare: acuñar, usar, revocar).
import { readFileSync } from 'node:fs';
import { homedir } from 'node:os';

const ESPEJO = 'https://modelos.yappy.joseluissaorin.com/supertonic-3';
const BUCKET = 'yappy-modelos';
const ONNX = ['duration_predictor.onnx', 'text_encoder.onnx', 'vector_estimator.onnx', 'vocoder.onnx'];
const AUX = ['tts.json', 'unicode_indexer.json'];
const VOCES = ['M1', 'M2', 'M3', 'M4', 'M5', 'F1', 'F2', 'F3', 'F4', 'F5'].map((v) => `${v}.json`);

async function tamano(url) {
  const r = await fetch(url, { method: 'HEAD' });
  if (!r.ok) throw new Error(`HEAD ${url} → ${r.status}`);
  return Number(r.headers.get('content-length'));
}

async function manifiesto(variante) {
  const ficheros = [];
  for (const f of ONNX) {
    const url = `${ESPEJO}/${variante}/onnx/${f}`;
    ficheros.push({ id: `onnx/${f}`, url, bytes: await tamano(url), esencial: true });
  }
  for (const f of AUX) {
    const url = `${ESPEJO}/onnx/${f}`;
    ficheros.push({ id: `onnx/${f}`, url, bytes: await tamano(url), esencial: true });
  }
  for (const f of VOCES) {
    const url = `${ESPEJO}/voice_styles/${f}`;
    ficheros.push({ id: `voice_styles/${f}`, url, bytes: await tamano(url), esencial: true });
  }
  return { version: 1, modelo: 'supertonic-3', variante, ficheros };
}

const soloVer = process.argv.includes('--solo-ver');
const env = Object.fromEntries(
  readFileSync(`${homedir()}/.claude/skills/cloudflare/credentials.env`, 'utf8')
    .split('\n').filter((l) => l.includes('=') && !l.startsWith('#'))
    .map((l) => l.replace(/^export\s+/, '').split('=')).map(([k, v]) => [k.trim(), v.trim().replace(/^["']|["']$/g, '')]),
);
const cuenta = env.CF_ACCOUNT_ID;

for (const [nombre, variante] of [['manifiesto-ios.json', 'fp16'], ['manifiesto-fp16.json', 'fp16'], ['manifiesto-fp32.json', 'fp32']]) {
  const m = await manifiesto(variante);
  const total = m.ficheros.reduce((s, f) => s + f.bytes, 0);
  console.log(`${nombre}: ${m.ficheros.length} ficheros, ${(total / 1e6).toFixed(1)} MB (${variante})`);
  if (soloVer) continue;
  const token = JSON.parse(readFileSync('/tmp/cf-tmp-token.json', 'utf8')).value;
  const r = await fetch(`https://api.cloudflare.com/client/v4/accounts/${cuenta}/r2/buckets/${BUCKET}/objects/supertonic-3/${nombre}`, {
    method: 'PUT',
    headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
    body: JSON.stringify(m, null, 1),
  });
  console.log(`  subida → ${r.status}`);
}
