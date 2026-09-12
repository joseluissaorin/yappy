#!/usr/bin/env node
// Da de alta en RevenueCat (API v2) los productos de App Store de Yappy
// Parlanchín y los engancha a la entitlement y a los paquetes de la oferta
// «default». Idempotente. Necesita RC_SECRET (clave sk_… del proyecto).
const K = process.env.RC_SECRET;
if (!K) throw new Error('falta RC_SECRET');
const P = 'proj313ccd9a';
const APP = 'app6674838594'; // Yappy (App Store)
const ENTL = 'entl2d3ec325a6'; // yappy_parlanchín
const OFERTA = 'ofrng5e226e4b3f'; // default
const PRODUCTOS = [
  { store_identifier: 'com.joseluissaorin.yappy.parlanchin.mensual', type: 'subscription', display_name: 'Parlanchín Mensual', paquete: '$rc_monthly' },
  { store_identifier: 'com.joseluissaorin.yappy.parlanchin.anual', type: 'subscription', display_name: 'Parlanchín Anual', paquete: '$rc_annual' },
  { store_identifier: 'com.joseluissaorin.yappy.parlanchin.vida', type: 'non_consumable', display_name: 'Parlanchín De por vida', paquete: '$rc_lifetime' },
];
async function rc(method, path, body) {
  const r = await fetch(`https://api.revenuecat.com/v2${path}`, { method, headers: { Authorization: `Bearer ${K}`, 'Content-Type': 'application/json' }, body: body ? JSON.stringify(body) : undefined });
  const t = await r.text();
  if (!r.ok) throw new Error(`RC ${method} ${path} → ${r.status} ${t}`);
  return t ? JSON.parse(t) : null;
}
const existentes = (await rc('GET', `/projects/${P}/products?limit=100`)).items;
const paquetes = (await rc('GET', `/projects/${P}/offerings/${OFERTA}/packages?limit=50&expand=items.product`)).items;
const enEntl = new Set((await rc('GET', `/projects/${P}/entitlements/${ENTL}/products?limit=100`)).items.map((p) => p.id));
for (const p of PRODUCTOS) {
  let prod = existentes.find((e) => e.store_identifier === p.store_identifier && e.app_id === APP);
  if (!prod) {
    prod = await rc('POST', `/projects/${P}/products`, { store_identifier: p.store_identifier, app_id: APP, type: p.type, display_name: p.display_name });
    console.log(`✓ producto ${p.store_identifier} → ${prod.id}`);
  } else console.log(`producto existe ${p.store_identifier} → ${prod.id}`);
  if (!enEntl.has(prod.id)) {
    await rc('POST', `/projects/${P}/entitlements/${ENTL}/actions/attach_products`, { product_ids: [prod.id] });
    console.log(`  ✓ enganchado a la entitlement`);
  } else console.log('  ya en la entitlement');
  const pk = paquetes.find((x) => x.lookup_key === p.paquete);
  if (!pk) { console.log(`  ✗ no hay paquete ${p.paquete}`); continue; }
  const ya = (pk.products?.items ?? []).some((it) => it.product?.id === prod.id);
  if (!ya) {
    await rc('POST', `/projects/${P}/packages/${pk.id}/actions/attach_products`, { products: [{ product_id: prod.id, eligibility_criteria: 'all' }] });
    console.log(`  ✓ enganchado al paquete ${p.paquete}`);
  } else console.log(`  ya en el paquete ${p.paquete}`);
}
console.log('— resumen —');
const fin = (await rc('GET', `/projects/${P}/offerings/${OFERTA}/packages?limit=50&expand=items.product`)).items;
for (const pk of fin) console.log(pk.lookup_key, '→', (pk.products?.items ?? []).map((i) => `${i.product.store_identifier}@${i.product.app_id}`).join(', '));
