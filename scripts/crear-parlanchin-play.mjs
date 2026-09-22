#!/usr/bin/env node
// El gemelo Android de crear-parlanchin.mjs: da de alta en Google Play las
// tres cuerdas de Yappy Parlanchín (dos suscripciones con su plan base y un
// producto único) y activa los planes. Idempotente. Usa la cuenta de
// servicio de play-helper. Precios base en EUR (España); el resto de países
// se convierte automáticamente.
import { readFileSync } from "node:fs";
import { createSign } from "node:crypto";
import { homedir } from "node:os";

const PAQUETE = process.env.PLAY_PACKAGE || "com.joseluissaorin.yappy.android";
const SA = JSON.parse(readFileSync(`${homedir()}/.config/yappy-play/service-account.json`, "utf8"));
const b64 = (o) => Buffer.from(JSON.stringify(o)).toString("base64url");
async function token() {
  const now = Math.floor(Date.now() / 1000);
  const sin = `${b64({ alg: "RS256", typ: "JWT" })}.${b64({ iss: SA.client_email, scope: "https://www.googleapis.com/auth/androidpublisher", aud: "https://oauth2.googleapis.com/token", iat: now, exp: now + 3600 })}`;
  const firma = createSign("RSA-SHA256").update(sin).sign(SA.private_key, "base64url");
  const r = await fetch("https://oauth2.googleapis.com/token", { method: "POST", headers: { "Content-Type": "application/x-www-form-urlencoded" }, body: new URLSearchParams({ grant_type: "urn:ietf:params:oauth:grant-type:jwt-bearer", assertion: `${sin}.${firma}` }) });
  if (!r.ok) throw new Error(`token: ${r.status} ${await r.text()}`);
  return (await r.json()).access_token;
}
const BASE = `https://androidpublisher.googleapis.com/androidpublisher/v3/applications/${PAQUETE}`;
async function api(tok, ruta, opts = {}) {
  const r = await fetch(`${BASE}${ruta}`, { ...opts, headers: { Authorization: `Bearer ${tok}`, "Content-Type": "application/json" } });
  const cuerpo = await r.text();
  if (!r.ok) throw new Error(`${opts.method ?? "GET"} ${ruta}: ${r.status} ${cuerpo.slice(0, 500)}`);
  return cuerpo ? JSON.parse(cuerpo) : {};
}

const SUSCRIPCIONES = [
  { id: "com.joseluissaorin.yappy.parlanchin.mensual", plan: "mensual", periodo: "P1M", eur: [3, 990000000], usd: [3, 990000000],
    es: ["Yappy Parlanchín mensual", "Percha sin fondo, imprenta de audiolibros y puente con el ordenador, un mes."],
    en: ["Yappy Parlanchín monthly", "Bottomless perch, audiobook press and bridge to your computer, for a month."] },
  { id: "com.joseluissaorin.yappy.parlanchin.anual", plan: "anual", periodo: "P1Y", eur: [29, 990000000], usd: [29, 990000000],
    es: ["Yappy Parlanchín anual", "Percha sin fondo, imprenta de audiolibros y puente con el ordenador, un año."],
    en: ["Yappy Parlanchín yearly", "Bottomless perch, audiobook press and bridge to your computer, for a year."] },
];
const VIDA = { sku: "com.joseluissaorin.yappy.parlanchin.vida", eur: 59990000,
  es: ["Yappy Parlanchín de por vida", "Percha sin fondo, imprenta de audiolibros y puente con el ordenador, para siempre."],
  en: ["Yappy Parlanchín lifetime", "Bottomless perch, audiobook press and bridge to your computer, forever."] };

const tok = await token();
for (const s of SUSCRIPCIONES) {
  let existe = null;
  try { existe = await api(tok, `/subscriptions/${s.id}`); } catch {}
  const cuerpo = {
    packageName: PAQUETE, productId: s.id,
    listings: [
      { languageCode: "es-ES", title: s.es[0], description: s.es[1] },
      { languageCode: "en-US", title: s.en[0], description: s.en[1] },
    ],
    basePlans: [{
      basePlanId: s.plan,
      autoRenewingBasePlanType: { billingPeriodDuration: s.periodo, gracePeriodDuration: "P7D", resubscribeState: "RESUBSCRIBE_STATE_ACTIVE", prorationMode: "SUBSCRIPTION_PRORATION_MODE_CHARGE_ON_NEXT_BILLING_DATE", legacyCompatible: true, accountHoldDuration: "P30D" },
      regionalConfigs: [{ regionCode: "ES", newSubscriberAvailability: true, price: { currencyCode: "EUR", units: String(s.eur[0]), nanos: s.eur[1] } }],
      otherRegionsConfig: { usdPrice: { currencyCode: "USD", units: String(s.usd[0]), nanos: s.usd[1] }, eurPrice: { currencyCode: "EUR", units: String(s.eur[0]), nanos: s.eur[1] }, newSubscriberAvailability: true },
    }],
  };
  if (!existe) {
    const r = await api(tok, `/subscriptions?productId=${s.id}&regionsVersion.version=2022/02`, { method: "POST", body: JSON.stringify(cuerpo) });
    console.log(`✓ suscripción ${s.id} creada (${r.basePlans?.[0]?.state})`);
  } else console.log(`suscripción existe ${s.id} (${existe.basePlans?.map((b) => b.basePlanId + ":" + b.state).join(",")})`);
  const ahora = await api(tok, `/subscriptions/${s.id}`);
  const plan = ahora.basePlans?.find((b) => b.basePlanId === s.plan);
  if (plan && plan.state !== "ACTIVE") {
    await api(tok, `/subscriptions/${s.id}/basePlans/${s.plan}:activate`, { method: "POST", body: JSON.stringify({ packageName: PAQUETE, productId: s.id, basePlanId: s.plan }) });
    console.log(`  ✓ plan base ${s.plan} activado`);
  } else console.log(`  plan base ${s.plan}: ${plan?.state}`);
}
// El producto único
let vida = null;
try { vida = await api(tok, `/inappproducts/${VIDA.sku}`); } catch {}
const cuerpoVida = {
  packageName: PAQUETE, sku: VIDA.sku, status: "active", purchaseType: "managedUser",
  defaultLanguage: "es-ES", defaultPrice: { priceMicros: String(VIDA.eur), currency: "EUR" },
  listings: { "es-ES": { title: VIDA.es[0], description: VIDA.es[1] }, "en-US": { title: VIDA.en[0], description: VIDA.en[1] } },
};
if (!vida) {
  const r = await api(tok, `/inappproducts?autoConvertMissingPrices=true`, { method: "POST", body: JSON.stringify(cuerpoVida) });
  console.log(`✓ producto único ${VIDA.sku} (${r.status})`);
} else console.log(`producto único existe ${VIDA.sku} (${vida.status})`);
console.log("— resumen —");
const subs = await api(tok, `/subscriptions`);
for (const s of subs.subscriptions ?? []) console.log(s.productId, s.basePlans.map((b) => `${b.basePlanId}:${b.state}`).join(","));
const iaps = await api(tok, `/inappproducts`);
for (const i of iaps.inappproduct ?? []) console.log(i.sku, i.status, i.defaultPrice);
