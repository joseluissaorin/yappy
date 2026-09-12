#!/usr/bin/env node
// Crea en App Store Connect el catálogo de YAPPY PARLANCHÍN (idempotente):
//   grupo de suscripciones → mensual + anual (localizaciones, disponibilidad
//   mundial, precios con base en España en euros y equalización) → la compra
//   única «de por vida» (no consumible, con su calendario de precios).
// La captura de revisión se sube aparte con --step screenshot cuando existe
// marketing/paywall-revision.png.
//
// Uso: node scripts/crear-parlanchin.mjs [--step grupo|productos|locs|availability|prices|screenshot|all]

import { readFileSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import { createSign, createHash } from 'node:crypto';
import { homedir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const KEY_ID = process.env.ASC_KEY_ID || '4HVB5YWGWD';
const ISSUER_ID = process.env.ASC_ISSUER_ID || '1e05b19d-c430-4408-8af5-24d6623959c4';
const BUNDLE_ID = 'com.joseluissaorin.yappy';
const PRIVATE_KEY = readFileSync(`${homedir()}/.appstoreconnect/private_keys/AuthKey_${KEY_ID}.p8`, 'utf8');
const V1 = 'https://api.appstoreconnect.apple.com/v1';
const V2 = 'https://api.appstoreconnect.apple.com/v2';
const AQUI = dirname(fileURLToPath(import.meta.url));
const SCREENSHOT = join(AQUI, '../marketing/paywall-revision.png');

const b64url = (b) => Buffer.from(b).toString('base64').replace(/=+$/, '').replace(/\+/g, '-').replace(/\//g, '_');
function token() {
  const now = Math.floor(Date.now() / 1000);
  const h = b64url(JSON.stringify({ alg: 'ES256', kid: KEY_ID, typ: 'JWT' }));
  const p = b64url(JSON.stringify({ iss: ISSUER_ID, iat: now, exp: now + 1200, aud: 'appstoreconnect-v1' }));
  const s = createSign('SHA256').update(`${h}.${p}`).sign({ key: PRIVATE_KEY, dsaEncoding: 'ieee-p1363' });
  return `${h}.${p}.${b64url(s)}`;
}
async function asc(method, path, body, intento = 0) {
  const url = path.startsWith('http') ? path : `${V1}${path}`;
  const r = await fetch(url, {
    method,
    headers: { Authorization: `Bearer ${token()}`, 'Content-Type': 'application/json' },
    body: body ? JSON.stringify(body) : undefined,
  });
  const t = await r.text();
  if (!r.ok) {
    if ((r.status >= 500 || r.status === 429) && intento < 4) {
      await new Promise((res) => setTimeout(res, 1500 * (intento + 1)));
      return asc(method, path, body, intento + 1);
    }
    throw new Error(`ASC ${method} ${path} → ${r.status}\n${t}`);
  }
  return t ? JSON.parse(t) : null;
}
async function ascAll(path) {
  let out = [];
  let url = path;
  while (url) {
    const r = await asc('GET', url);
    out = out.concat(r.data ?? []);
    url = r.links?.next ?? null;
  }
  return out;
}

// ── El catálogo ──────────────────────────────────────────────────────────
const GRUPO_REF = 'Yappy Parlanchín';
const NOTA_REVISION =
  'Yappy Parlanchín removes the free tier\'s daily limit of 15 minutes of synthesized voice, and unlocks the ' +
  'audiobook printing queue ("la imprenta") and the desktop bridge. No account or login is required. ' +
  'How to reach the paywall: open Settings (the gear / "la trastienda") → "Yappy Parlanchín" box → "Dale cuerda al loro"; ' +
  'or simply listen for more than 15 minutes in a day. Monthly and yearly are auto-renewable subscriptions in the same group; ' +
  '"De por vida" is a one-time non-consumable purchase that grants the same entitlement forever.';

const SUBS = [
  { key: 'mensual', productId: 'com.joseluissaorin.yappy.parlanchin.mensual', referenceName: 'Parlanchín Mensual', period: 'ONE_MONTH', eur: ['4.00', '3.99', '4.49'] },
  { key: 'anual', productId: 'com.joseluissaorin.yappy.parlanchin.anual', referenceName: 'Parlanchín Anual', period: 'ONE_YEAR', eur: ['30.00', '29.99', '30.99'] },
];
const VIDA = { productId: 'com.joseluissaorin.yappy.parlanchin.vida', referenceName: 'Parlanchín De por vida', eur: ['60.00', '59.99', '60.99'] };

// name ≤30, description ≤45 (límites duros de ASC). m/a/v = mensual/anual/vida.
const LOCS = {
  'en-US': { m: 'Yappy Parlanchín · Monthly', a: 'Yappy Parlanchín · Yearly', v: 'Yappy Parlanchín · Lifetime', d: 'Unlimited voice, audiobooks and bridge', dv: 'Unlimited voice forever, one payment' },
  'en-GB': { m: 'Yappy Parlanchín · Monthly', a: 'Yappy Parlanchín · Yearly', v: 'Yappy Parlanchín · Lifetime', d: 'Unlimited voice, audiobooks and bridge', dv: 'Unlimited voice forever, one payment' },
  'es-ES': { m: 'Yappy Parlanchín · Mensual', a: 'Yappy Parlanchín · Anual', v: 'Yappy Parlanchín · De por vida', d: 'Voz sin límite, imprenta y puente', dv: 'Voz sin límite para siempre, un pago' },
  'es-MX': { m: 'Yappy Parlanchín · Mensual', a: 'Yappy Parlanchín · Anual', v: 'Yappy Parlanchín · De por vida', d: 'Voz sin límite, imprenta y puente', dv: 'Voz sin límite para siempre, un pago' },
  'fr-FR': { m: 'Yappy Parlanchín · Mensuel', a: 'Yappy Parlanchín · Annuel', v: 'Yappy Parlanchín · À vie', d: 'Voix illimitée, livres audio et pont', dv: 'Voix illimitée pour toujours, un paiement' },
  'fr-CA': { m: 'Yappy Parlanchín · Mensuel', a: 'Yappy Parlanchín · Annuel', v: 'Yappy Parlanchín · À vie', d: 'Voix illimitée, livres audio et pont', dv: 'Voix illimitée pour toujours, un paiement' },
  'de-DE': { m: 'Yappy Parlanchín · Monatlich', a: 'Yappy Parlanchín · Jährlich', v: 'Yappy Parlanchín · Lebenslang', d: 'Unbegrenzte Stimme, Hörbücher, Brücke', dv: 'Unbegrenzte Stimme für immer, einmalig' },
  it: { m: 'Yappy Parlanchín · Mensile', a: 'Yappy Parlanchín · Annuale', v: 'Yappy Parlanchín · A vita', d: 'Voce illimitata, audiolibri e ponte', dv: 'Voce illimitata per sempre, un pagamento' },
  'pt-PT': { m: 'Yappy Parlanchín · Mensal', a: 'Yappy Parlanchín · Anual', v: 'Yappy Parlanchín · Vitalício', d: 'Voz sem limite, audiolivros e ponte', dv: 'Voz sem limite para sempre, um pagamento' },
  'pt-BR': { m: 'Yappy Parlanchín · Mensal', a: 'Yappy Parlanchín · Anual', v: 'Yappy Parlanchín · Vitalício', d: 'Voz sem limite, audiolivros e ponte', dv: 'Voz sem limite para sempre, um pagamento' },
  'nl-NL': { m: 'Yappy Parlanchín · Maandelijks', a: 'Yappy Parlanchín · Jaarlijks', v: 'Yappy Parlanchín · Levenslang', d: 'Onbeperkte stem, luisterboeken en brug', dv: 'Onbeperkte stem voor altijd, eenmalig' },
  pl: { m: 'Yappy Parlanchín · Miesięczny', a: 'Yappy Parlanchín · Roczny', v: 'Yappy Parlanchín · Na zawsze', d: 'Głos bez limitu, audiobooki i most', dv: 'Głos bez limitu na zawsze, jedna opłata' },
  ro: { m: 'Yappy Parlanchín · Lunar', a: 'Yappy Parlanchín · Anual', v: 'Yappy Parlanchín · Pe viață', d: 'Voce nelimitată, audiobook-uri și punte', dv: 'Voce nelimitată pentru totdeauna, o plată' },
  sv: { m: 'Yappy Parlanchín · Månad', a: 'Yappy Parlanchín · År', v: 'Yappy Parlanchín · Livstid', d: 'Obegränsad röst, ljudböcker och bro', dv: 'Obegränsad röst för alltid, en betalning' },
  da: { m: 'Yappy Parlanchín · Månedlig', a: 'Yappy Parlanchín · Årlig', v: 'Yappy Parlanchín · Livstid', d: 'Ubegrænset stemme, lydbøger og bro', dv: 'Ubegrænset stemme for altid, én betaling' },
  fi: { m: 'Yappy Parlanchín · Kuukausi', a: 'Yappy Parlanchín · Vuosi', v: 'Yappy Parlanchín · Ikuinen', d: 'Rajaton ääni, äänikirjat ja silta', dv: 'Rajaton ääni ikuisesti, yksi maksu' },
  hr: { m: 'Yappy Parlanchín · Mjesečno', a: 'Yappy Parlanchín · Godišnje', v: 'Yappy Parlanchín · Doživotno', d: 'Neograničen glas, audioknjige i most', dv: 'Neograničen glas zauvijek, jedna uplata' },
  sk: { m: 'Yappy Parlanchín · Mesačne', a: 'Yappy Parlanchín · Ročne', v: 'Yappy Parlanchín · Navždy', d: 'Neobmedzený hlas, audioknihy a most', dv: 'Neobmedzený hlas navždy, jedna platba' },
  cs: { m: 'Yappy Parlanchín · Měsíčně', a: 'Yappy Parlanchín · Ročně', v: 'Yappy Parlanchín · Navždy', d: 'Neomezený hlas, audioknihy a most', dv: 'Neomezený hlas navždy, jedna platba' },
  hu: { m: 'Yappy Parlanchín · Havi', a: 'Yappy Parlanchín · Éves', v: 'Yappy Parlanchín · Örökre', d: 'Korlátlan hang, hangoskönyvek és híd', dv: 'Korlátlan hang örökre, egy fizetés' },
  el: { m: 'Yappy Parlanchín · Μηνιαίο', a: 'Yappy Parlanchín · Ετήσιο', v: 'Yappy Parlanchín · Για πάντα', d: 'Απεριόριστη φωνή, ηχοβιβλία και γέφυρα', dv: 'Απεριόριστη φωνή για πάντα, μία πληρωμή' },
  uk: { m: 'Yappy Parlanchín · Щомісяця', a: 'Yappy Parlanchín · Щороку', v: 'Yappy Parlanchín · Назавжди', d: 'Безлімітний голос, аудіокниги й міст', dv: 'Безлімітний голос назавжди, один платіж' },
  ru: { m: 'Yappy Parlanchín · Ежемесячно', a: 'Yappy Parlanchín · Ежегодно', v: 'Yappy Parlanchín · Навсегда', d: 'Безлимитный голос, аудиокниги и мост', dv: 'Безлимитный голос навсегда, один платёж' },
  tr: { m: 'Yappy Parlanchín · Aylık', a: 'Yappy Parlanchín · Yıllık', v: 'Yappy Parlanchín · Ömür boyu', d: 'Sınırsız ses, sesli kitaplar ve köprü', dv: 'Sonsuza dek sınırsız ses, tek ödeme' },
  'ar-SA': { m: 'Yappy Parlanchín · شهري', a: 'Yappy Parlanchín · سنوي', v: 'Yappy Parlanchín · مدى الحياة', d: 'صوت بلا حدود وكتب صوتية وجسر', dv: 'صوت بلا حدود للأبد، دفعة واحدة' },
  hi: { m: 'Yappy Parlanchín · मासिक', a: 'Yappy Parlanchín · वार्षिक', v: 'Yappy Parlanchín · आजीवन', d: 'असीमित आवाज़, ऑडियोबुक और ब्रिज', dv: 'हमेशा के लिए असीमित आवाज़, एक भुगतान' },
  id: { m: 'Yappy Parlanchín · Bulanan', a: 'Yappy Parlanchín · Tahunan', v: 'Yappy Parlanchín · Selamanya', d: 'Suara tanpa batas, buku audio, jembatan', dv: 'Suara tanpa batas selamanya, sekali bayar' },
  vi: { m: 'Yappy Parlanchín · Hàng tháng', a: 'Yappy Parlanchín · Hàng năm', v: 'Yappy Parlanchín · Trọn đời', d: 'Giọng không giới hạn, sách nói và cầu nối', dv: 'Giọng không giới hạn mãi mãi, một lần trả' },
  ko: { m: 'Yappy Parlanchín · 월간', a: 'Yappy Parlanchín · 연간', v: 'Yappy Parlanchín · 평생', d: '무제한 음성, 오디오북, 브리지', dv: '평생 무제한 음성, 한 번 결제' },
  ja: { m: 'Yappy Parlanchín · 月額', a: 'Yappy Parlanchín · 年額', v: 'Yappy Parlanchín · 買い切り', d: '無制限の音声、オーディオブック、ブリッジ', dv: '永久に無制限の音声、一回払い' },
};
for (const [loc, v] of Object.entries(LOCS)) {
  for (const f of ['m', 'a', 'v']) if (v[f].length > 30) throw new Error(`${loc} ${f} >30 (${v[f].length}): ${v[f]}`);
  for (const f of ['d', 'dv']) if (v[f].length > 45) throw new Error(`${loc} ${f} >45 (${v[f].length}): ${v[f]}`);
}
// El nombre del GRUPO tal como lo verá el usuario en «Suscripciones».
const GRUPO_LOCS = { 'en-US': 'Yappy Parlanchín', 'es-ES': 'Yappy Parlanchín' };

const args = process.argv.slice(2);
const step = args.includes('--step') ? args[args.indexOf('--step') + 1] : 'all';
const want = (s) => step === 'all' || step === s;
const corto = (e) => e.message.split('\n').slice(0, 3).join(' | ').slice(0, 320);

async function getApp() {
  const r = await asc('GET', `/apps?filter[bundleId]=${BUNDLE_ID}&limit=1`);
  if (!r.data?.length) throw new Error(`sin app para ${BUNDLE_ID}`);
  return r.data[0];
}

async function ensureGrupo(appId) {
  const grupos = await ascAll(`/apps/${appId}/subscriptionGroups?limit=50`);
  let g = grupos.find((x) => x.attributes.referenceName === GRUPO_REF);
  if (!g) {
    const c = await asc('POST', '/subscriptionGroups', {
      data: { type: 'subscriptionGroups', attributes: { referenceName: GRUPO_REF }, relationships: { app: { data: { type: 'apps', id: appId } } } },
    });
    g = c.data;
    console.log(`  ✓ grupo creado → ${g.id}`);
  } else console.log(`  grupo existe → ${g.id}`);
  const locs = await ascAll(`/subscriptionGroups/${g.id}/subscriptionGroupLocalizations?limit=50`);
  const have = new Set(locs.map((l) => l.attributes.locale));
  for (const [locale, name] of Object.entries(GRUPO_LOCS)) {
    if (have.has(locale)) continue;
    try {
      await asc('POST', '/subscriptionGroupLocalizations', {
        data: { type: 'subscriptionGroupLocalizations', attributes: { locale, name, customAppName: 'Yappy' }, relationships: { subscriptionGroup: { data: { type: 'subscriptionGroups', id: g.id } } } },
      });
      console.log(`  ✓ loc del grupo ${locale}`);
    } catch (e) { console.log(`    ✗ loc grupo ${locale}: ${corto(e)}`); }
  }
  return g.id;
}

async function ensureSub(groupId, p) {
  const existing = await ascAll(`/subscriptionGroups/${groupId}/subscriptions?limit=50`);
  const found = existing.find((s) => s.attributes.productId === p.productId);
  if (found) { console.log(`  producto existe: ${p.productId} → ${found.id} (${found.attributes.state})`); return found.id; }
  const c = await asc('POST', '/subscriptions', {
    data: {
      type: 'subscriptions',
      attributes: { name: p.referenceName, productId: p.productId, subscriptionPeriod: p.period, familySharable: false, groupLevel: 1, reviewNote: NOTA_REVISION },
      relationships: { group: { data: { type: 'subscriptionGroups', id: groupId } } },
    },
  });
  console.log(`  ✓ creado ${p.productId} → ${c.data.id}`);
  return c.data.id;
}

async function ensureSubLocs(p, subId) {
  const existing = await ascAll(`/subscriptions/${subId}/subscriptionLocalizations?limit=50`);
  const have = new Set(existing.map((l) => l.attributes.locale));
  let ok = 0, fail = 0;
  for (const [locale, v] of Object.entries(LOCS)) {
    if (have.has(locale)) { ok++; continue; }
    try {
      await asc('POST', '/subscriptionLocalizations', {
        data: { type: 'subscriptionLocalizations', attributes: { locale, name: p.key === 'mensual' ? v.m : v.a, description: v.d }, relationships: { subscription: { data: { type: 'subscriptions', id: subId } } } },
      });
      ok++;
    } catch (e) { fail++; console.log(`    ✗ loc ${locale}: ${corto(e)}`); }
  }
  console.log(`  locs: ${ok} ok, ${fail} mal`);
}

async function ensureSubAvailability(subId) {
  try {
    const cur = await asc('GET', `/subscriptions/${subId}/subscriptionAvailability`);
    if (cur?.data) { console.log('  disponibilidad ya puesta'); return; }
  } catch { /* 404 = sin poner */ }
  const territorios = await ascAll('/territories?limit=200');
  await asc('POST', '/subscriptionAvailabilities', {
    data: {
      type: 'subscriptionAvailabilities',
      attributes: { availableInNewTerritories: true },
      relationships: { subscription: { data: { type: 'subscriptions', id: subId } }, availableTerritories: { data: territorios.map((t) => ({ type: 'territories', id: t.id })) } },
    },
  });
  console.log(`  ✓ disponible en ${territorios.length} territorios`);
}

async function puntoEsp(listaUrl, candidatos) {
  const puntos = await ascAll(listaUrl);
  for (const c of candidatos) {
    const hit = puntos.find((pp) => pp.attributes.customerPrice === c);
    if (hit) return { punto: hit, precio: c };
  }
  const muestra = puntos.slice(0, 40).map((pp) => pp.attributes.customerPrice).join(', ');
  throw new Error(`sin punto de precio ESP para ${candidatos.join('/')} (muestra: ${muestra})`);
}

async function ensureSubPrices(p, subId) {
  const current = await ascAll(`/subscriptions/${subId}/prices?limit=200`);
  if (current.length > 100) { console.log(`  precios ya puestos (${current.length})`); return; }
  const { punto, precio } = await puntoEsp(`/subscriptions/${subId}/pricePoints?filter[territory]=ESP&limit=200`, p.eur);
  console.log(`  punto ESP ${precio} € → ${punto.id.slice(0, 24)}…`);
  const eq = await ascAll(`${V1}/subscriptionPricePoints/${punto.id}/equalizations?limit=200`);
  const points = [punto, ...eq];
  console.log(`  poniendo precio en ${points.length} territorios…`);
  let ok = 0, fail = 0;
  for (const pp of points) {
    try {
      await asc('POST', '/subscriptionPrices', {
        data: { type: 'subscriptionPrices', relationships: { subscription: { data: { type: 'subscriptions', id: subId } }, subscriptionPricePoint: { data: { type: 'subscriptionPricePoints', id: pp.id } } } },
      });
      ok++;
    } catch (e) { fail++; if (fail <= 3) console.log(`    ✗ precio ${pp.id.slice(0, 16)}…: ${corto(e)}`); }
  }
  console.log(`  precios: ${ok} ok, ${fail} mal`);
}

async function subirCaptura(tipo, relKey, id) {
  const buf = await readFile(SCREENSHOT);
  const create = await asc('POST', `/${tipo}`, {
    data: { type: tipo, attributes: { fileName: 'paywall.png', fileSize: buf.length }, relationships: { [relKey]: { data: { type: relKey === 'subscription' ? 'subscriptions' : 'inAppPurchases', id } } } },
  });
  const ssId = create.data.id;
  for (const op of create.data.attributes.uploadOperations ?? []) {
    const headers = {};
    for (const h of op.requestHeaders ?? []) headers[h.name] = h.value;
    const r = await fetch(op.url, { method: op.method, headers, body: buf.subarray(op.offset, op.offset + op.length) });
    if (!r.ok) throw new Error(`trozo: ${r.status} ${await r.text()}`);
  }
  const md5 = createHash('md5').update(buf).digest('hex');
  await asc('PATCH', `/${tipo}/${ssId}`, { data: { type: tipo, id: ssId, attributes: { uploaded: true, sourceFileChecksum: md5 } } });
  console.log('  ✓ captura de revisión subida');
}
async function ensureSubScreenshot(subId) {
  try { const cur = await asc('GET', `/subscriptions/${subId}/appStoreReviewScreenshot`); if (cur?.data) { console.log('  captura ya subida'); return; } } catch {}
  await subirCaptura('subscriptionAppStoreReviewScreenshots', 'subscription', subId);
}

// ── De por vida (IAP no consumible) ──────────────────────────────────────
async function ensureVida(appId) {
  const existing = await ascAll(`/apps/${appId}/inAppPurchasesV2?limit=50`);
  const found = existing.find((i) => i.attributes.productId === VIDA.productId);
  if (found) { console.log(`  producto existe: ${VIDA.productId} → ${found.id} (${found.attributes.state})`); return found.id; }
  const c = await asc('POST', `${V2}/inAppPurchases`, {
    data: { type: 'inAppPurchases', attributes: { name: VIDA.referenceName, productId: VIDA.productId, inAppPurchaseType: 'NON_CONSUMABLE', reviewNote: NOTA_REVISION }, relationships: { app: { data: { type: 'apps', id: appId } } } },
  });
  console.log(`  ✓ creado ${VIDA.productId} → ${c.data.id}`);
  return c.data.id;
}
async function ensureVidaLocs(iapId) {
  const existing = await ascAll(`${V2}/inAppPurchases/${iapId}/inAppPurchaseLocalizations?limit=50`);
  const have = new Set(existing.map((l) => l.attributes.locale));
  let ok = 0, fail = 0;
  for (const [locale, v] of Object.entries(LOCS)) {
    if (have.has(locale)) { ok++; continue; }
    try {
      await asc('POST', '/inAppPurchaseLocalizations', {
        data: { type: 'inAppPurchaseLocalizations', attributes: { locale, name: v.v, description: v.dv }, relationships: { inAppPurchaseV2: { data: { type: 'inAppPurchases', id: iapId } } } },
      });
      ok++;
    } catch (e) { fail++; console.log(`    ✗ loc ${locale}: ${corto(e)}`); }
  }
  console.log(`  locs: ${ok} ok, ${fail} mal`);
}
async function ensureVidaAvailability(iapId) {
  try { const cur = await asc('GET', `${V2}/inAppPurchases/${iapId}/inAppPurchaseAvailability`); if (cur?.data) { console.log('  disponibilidad ya puesta'); return; } } catch {}
  const territorios = await ascAll('/territories?limit=200');
  await asc('POST', '/inAppPurchaseAvailabilities', {
    data: { type: 'inAppPurchaseAvailabilities', attributes: { availableInNewTerritories: true }, relationships: { inAppPurchase: { data: { type: 'inAppPurchases', id: iapId } }, availableTerritories: { data: territorios.map((t) => ({ type: 'territories', id: t.id })) } } },
  });
  console.log(`  ✓ disponible en ${territorios.length} territorios`);
}
async function ensureVidaPrices(iapId) {
  // OJO: el GET del calendario devuelve un recurso hueco aunque no haya
  // precios; lo que cuenta son los manualPrices.
  try {
    const manuales = await ascAll(`${V1}/inAppPurchasePriceSchedules/${iapId}/manualPrices?limit=50`);
    if (manuales.length) { console.log(`  calendario de precios ya puesto (${manuales.length} manual)`); return; }
  } catch { /* 404 = sin calendario */ }
  const { punto, precio } = await puntoEsp(`${V2}/inAppPurchases/${iapId}/pricePoints?filter[territory]=ESP&limit=200`, VIDA.eur);
  console.log(`  punto ESP ${precio} € → ${punto.id.slice(0, 24)}…`);
  await asc('POST', '/inAppPurchasePriceSchedules', {
    data: {
      type: 'inAppPurchasePriceSchedules',
      relationships: {
        inAppPurchase: { data: { type: 'inAppPurchases', id: iapId } },
        baseTerritory: { data: { type: 'territories', id: 'ESP' } },
        manualPrices: { data: [{ type: 'inAppPurchasePrices', id: '${price1}' }] },
      },
    },
    included: [{ type: 'inAppPurchasePrices', id: '${price1}', attributes: { startDate: null }, relationships: { inAppPurchasePricePoint: { data: { type: 'inAppPurchasePricePoints', id: punto.id } } } }],
  });
  console.log('  ✓ calendario de precios (base España, equalizado)');
}
async function ensureVidaScreenshot(iapId) {
  try { const cur = await asc('GET', `${V2}/inAppPurchases/${iapId}/appStoreReviewScreenshot`); if (cur?.data) { console.log('  captura ya subida'); return; } } catch {}
  await subirCaptura('inAppPurchaseAppStoreReviewScreenshots', 'inAppPurchaseV2', iapId);
}

// ── Marcha ───────────────────────────────────────────────────────────────
const app = await getApp();
console.log(`── app ${app.id} (${BUNDLE_ID}) ──`);
const groupId = await ensureGrupo(app.id);
for (const p of SUBS) {
  console.log(`── ${p.productId} ──`);
  const subId = await ensureSub(groupId, p);
  if (want('locs')) await ensureSubLocs(p, subId);
  if (want('availability')) await ensureSubAvailability(subId);
  if (want('prices')) await ensureSubPrices(p, subId);
  if (want('screenshot')) { try { await ensureSubScreenshot(subId); } catch (e) { console.log(`  (captura pendiente: ${corto(e)})`); } }
  // Un PATCH vacío fuerza a ASC a recalcular el estado (se queda pegado en MISSING_METADATA).
  try { await asc('PATCH', `/subscriptions/${subId}`, { data: { type: 'subscriptions', id: subId, attributes: { familySharable: false } } }); } catch {}
  const fin = await asc('GET', `/subscriptions/${subId}`);
  console.log(`  estado: ${fin.data.attributes.state}`);
}
console.log(`── ${VIDA.productId} ──`);
const iapId = await ensureVida(app.id);
if (want('locs')) await ensureVidaLocs(iapId);
if (want('availability')) await ensureVidaAvailability(iapId);
if (want('prices')) await ensureVidaPrices(iapId);
if (want('screenshot')) { try { await ensureVidaScreenshot(iapId); } catch (e) { console.log(`  (captura pendiente: ${corto(e)})`); } }
const finV = await asc('GET', `${V2}/inAppPurchases/${iapId}`);
console.log(`  estado: ${finV.data.attributes.state}`);
