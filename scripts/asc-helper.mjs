#!/usr/bin/env node
// App Store Connect REST API helper for Yappy.
// Bootstrapped from ~/Developer/Kalorie/scripts/asc-helper.mjs.
//
// Commands:
//   status              report whether the Yappy app exists in ASC + recent builds
//   wait-for-app        block until the Yappy app entry exists in ASC
//   list-builds         show recent uploaded builds + their processing state
//   wait-for-build      block until a build for `version` is fully processed
//   create-beta-group   create the "Open Beta" group with public link enabled
//   add-build-to-beta   attach the latest processed build to the Open Beta group
//   submit-beta-review  push the build into Beta App Review (public link flow)
//   pipeline            END-TO-END: wait for app → wait for build → public TestFlight
//
// All commands take no positional args except `wait-for-build [version]`
// (defaults to whatever is in package.json's version field).

import { createSign, createPrivateKey } from "node:crypto";
import { readFile } from "node:fs/promises";
import { homedir } from "node:os";
import { join } from "node:path";

const KEY_ID    = process.env.ASC_KEY_ID    ?? "4HVB5YWGWD";
const ISSUER_ID = process.env.ASC_ISSUER_ID ?? "1e05b19d-c430-4408-8af5-24d6623959c4";
const TEAM_ID   = process.env.ASC_TEAM_ID   ?? "9LYNY2477X";
const BUNDLE_ID = process.env.ASC_BUNDLE_ID ?? "com.joseluissaorin.yappy";

const KEY_PATH = join(homedir(), ".appstoreconnect", "private_keys", `AuthKey_${KEY_ID}.p8`);

let PRIVATE_KEY;
async function getKey() {
  if (PRIVATE_KEY) return PRIVATE_KEY;
  const pem = await readFile(KEY_PATH, "utf-8");
  PRIVATE_KEY = createPrivateKey({ key: pem, format: "pem" });
  return PRIVATE_KEY;
}

function b64url(buf) {
  return Buffer.from(buf).toString("base64").replace(/=+$/, "").replace(/\+/g, "-").replace(/\//g, "_");
}

async function generateToken() {
  const key = await getKey();
  const iat = Math.floor(Date.now() / 1000);
  const header  = { alg: "ES256", kid: KEY_ID, typ: "JWT" };
  const payload = { iss: ISSUER_ID, iat, exp: iat + 1200, aud: "appstoreconnect-v1" };
  const head = b64url(JSON.stringify(header));
  const pay  = b64url(JSON.stringify(payload));
  const sig  = createSign("SHA256")
    .update(`${head}.${pay}`)
    .sign({ key, dsaEncoding: "ieee-p1363" });
  return `${head}.${pay}.${b64url(sig)}`;
}

async function asc(method, path, body) {
  const url = path.startsWith("http") ? path : `https://api.appstoreconnect.apple.com/v1${path}`;
  const token = await generateToken();
  const resp = await fetch(url, {
    method,
    headers: {
      Authorization: `Bearer ${token}`,
      "Content-Type": "application/json",
    },
    body: body ? JSON.stringify(body) : undefined,
  });
  const text = await resp.text();
  if (!resp.ok) {
    throw new Error(`ASC ${method} ${path} → ${resp.status}\n${text}`);
  }
  return text ? JSON.parse(text) : null;
}

async function getApp() {
  const r = await asc("GET", `/apps?filter[bundleId]=${encodeURIComponent(BUNDLE_ID)}`);
  return r.data[0] || null;
}

async function waitForApp({ intervalMs = 30_000, timeoutMs = 60 * 60 * 1000 } = {}) {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    const app = await getApp();
    if (app) {
      console.log(`✓ App registered: ${app.attributes.name} (id=${app.id})`);
      return app;
    }
    const elapsed = Math.floor((Date.now() - start) / 1000);
    process.stdout.write(`\r⏳ waiting for ${BUNDLE_ID} in ASC… (${elapsed}s)`);
    await new Promise((r) => setTimeout(r, intervalMs));
  }
  throw new Error(`timed out waiting for ${BUNDLE_ID} to appear in ASC after ${timeoutMs / 1000}s`);
}

async function listBuilds(app) {
  const r = await asc("GET", `/builds?filter[app]=${app.id}&limit=10`);
  return r.data;
}

async function buildState({ version, app }) {
  app ||= await getApp();
  if (!app) throw new Error("App not in ASC yet");
  // sort=-uploadedDate: sin él, ASC devolvía primero un build antiguo ya
  // procesado y wait-for-build daba por terminada una subida que seguía
  // en cocción.
  const r = await asc("GET", `/builds?filter[app]=${app.id}&filter[preReleaseVersion.version]=${encodeURIComponent(version)}&sort=-uploadedDate&limit=10`);
  return r.data;
}

async function waitForBuild({ version, intervalMs = 60_000, timeoutMs = 45 * 60 * 1000 }) {
  const start = Date.now();
  const app = await getApp();
  if (!app) throw new Error("App not in ASC — run wait-for-app first");
  while (Date.now() - start < timeoutMs) {
    const builds = await buildState({ version, app });
    if (builds.length > 0) {
      const b = builds[0];
      const state = b.attributes.processingState;
      const elapsed = Math.floor((Date.now() - start) / 1000);
      process.stdout.write(`\r⏳ build ${version} (#${b.attributes.version}) state=${state} (${elapsed}s)  `);
      if (state === "VALID") {
        console.log(`\n✓ build #${b.attributes.version} (id=${b.id}) processed and valid`);
        return b;
      }
      if (state === "INVALID" || state === "FAILED") {
        throw new Error(`build ${version} processing FAILED: ${JSON.stringify(b.attributes)}`);
      }
    } else {
      const elapsed = Math.floor((Date.now() - start) / 1000);
      process.stdout.write(`\r⏳ no build for version ${version} yet… (${elapsed}s)`);
    }
    await new Promise((r) => setTimeout(r, intervalMs));
  }
  throw new Error(`timed out waiting for build ${version} to finish processing`);
}

async function getOrCreateOpenBetaGroup(app) {
  const groups = await asc("GET", `/betaGroups?filter[app]=${app.id}`);
  let group = groups.data.find((g) => g.attributes.publicLinkEnabled);
  if (group) {
    console.log(`✓ existing public-link beta group: ${group.attributes.name} (id=${group.id})`);
    return group;
  }
  // Create one with publicLinkEnabled.
  console.log(`→ creating public Beta group "Open Beta"`);
  const r = await asc("POST", "/betaGroups", {
    data: {
      type: "betaGroups",
      attributes: {
        name: "Open Beta",
        publicLinkEnabled: true,
        publicLinkLimitEnabled: false,
      },
      relationships: {
        app: { data: { type: "apps", id: app.id } },
      },
    },
  });
  console.log(`✓ created public beta group id=${r.data.id}, publicLink=${r.data.attributes.publicLink}`);
  return r.data;
}

async function addBuildToBetaGroup({ buildId, groupId }) {
  await asc("POST", `/betaGroups/${groupId}/relationships/builds`, {
    data: [{ type: "builds", id: buildId }],
  });
  console.log(`✓ added build ${buildId} to beta group ${groupId}`);
}

async function submitForBetaReview({ buildId }) {
  // POST /betaAppReviewSubmissions with relationship to the build.
  try {
    const r = await asc("POST", "/betaAppReviewSubmissions", {
      data: {
        type: "betaAppReviewSubmissions",
        relationships: {
          build: { data: { type: "builds", id: buildId } },
        },
      },
    });
    console.log(`✓ submitted build ${buildId} for Beta App Review (submission id=${r.data.id})`);
    return r.data;
  } catch (e) {
    if (String(e.message).includes("Beta App Review is required for this Test Group")) {
      console.log(`(already submitted)`);
      return null;
    }
    if (String(e.message).includes("ENTITY_ERROR.ATTRIBUTE.INVALID")) {
      console.log(`(beta review submission rejected — likely missing Beta App Description or Test Information; complete that in ASC and retry)`);
      console.log(e.message);
      return null;
    }
    throw e;
  }
}

// ─── CLI commands ────────────────────────────────────────────────────────

async function cmd_status() {
  const app = await getApp();
  if (!app) {
    console.log(`✗ no App in ASC for bundleId ${BUNDLE_ID}`);
    console.log(`  → create via https://appstoreconnect.apple.com/apps → My Apps → +`);
    return;
  }
  console.log(`✓ App: ${app.attributes.name} (id=${app.id}, bundleId=${app.attributes.bundleId})`);
  const builds = await listBuilds(app);
  console.log(`  recent builds (${builds.length}):`);
  for (const b of builds) {
    console.log(`    - #${b.attributes.version} state=${b.attributes.processingState} uploaded=${b.attributes.uploadedDate}`);
  }
}

async function cmd_waitForApp() {
  await waitForApp();
}

async function cmd_listBuilds() {
  const app = await getApp();
  if (!app) { console.log("no App"); return; }
  const builds = await listBuilds(app);
  for (const b of builds) {
    console.log(JSON.stringify({
      build: b.attributes.version,
      preReleaseVersion: b.attributes.preReleaseVersion,
      state: b.attributes.processingState,
      uploaded: b.attributes.uploadedDate,
    }));
  }
}

async function cmd_waitForBuild() {
  const version = process.argv[3] ?? "0.1.0";
  await waitForBuild({ version });
}

async function cmd_createBetaGroup() {
  const app = await getApp();
  if (!app) { console.log("no App"); process.exit(1); }
  await getOrCreateOpenBetaGroup(app);
}

async function cmd_addBuildToBeta() {
  const version = process.argv[3] ?? "0.1.0";
  const app = await getApp();
  const builds = await buildState({ version, app });
  if (builds.length === 0) { console.log("no build for that version"); process.exit(1); }
  const group = await getOrCreateOpenBetaGroup(app);
  await addBuildToBetaGroup({ buildId: builds[0].id, groupId: group.id });
}

async function cmd_submitBetaReview() {
  const version = process.argv[3] ?? "0.1.0";
  const app = await getApp();
  const builds = await buildState({ version, app });
  if (builds.length === 0) { console.log("no build for that version"); process.exit(1); }
  await submitForBetaReview({ buildId: builds[0].id });
}

async function cmd_pipeline() {
  const version = process.argv[3] ?? "0.1.0";
  console.log(`📦 TestFlight public-link pipeline for Yappy ${version}`);
  console.log(`──────────────────────────────────────────────────────────`);
  console.log(`Step 1: wait for "${BUNDLE_ID}" to exist in App Store Connect`);
  const app = await waitForApp();
  console.log();
  console.log(`Step 2: ensure public Beta group exists`);
  const group = await getOrCreateOpenBetaGroup(app);
  console.log();
  console.log(`Step 3: wait for build ${version} to finish ASC processing`);
  const build = await waitForBuild({ version });
  console.log();
  console.log(`Step 4: attach build to public beta group`);
  await addBuildToBetaGroup({ buildId: build.id, groupId: group.id });
  console.log();
  console.log(`Step 5: submit for Beta App Review (public link)`);
  await submitForBetaReview({ buildId: build.id });
  console.log();
  console.log(`✓ Pipeline complete. Public link: ${group.attributes.publicLink ?? "(check ASC TestFlight tab)"}`);
  console.log(`  Beta App Review typically completes in 24–48h.`);
}

async function cmd_internalGroup() {
  // TestFlight interno: grupo isInternalGroup con hasAccessToAllBuilds, sin
  // Beta App Review. Los probadores deben ser usuarios del equipo en ASC.
  const app = await getApp();
  if (!app) { console.log("no App"); process.exit(1); }
  const groups = await asc("GET", `/betaGroups?filter[app]=${app.id}`);
  let group = groups.data.find((g) => g.attributes.isInternalGroup);
  if (!group) {
    const r = await asc("POST", "/betaGroups", {
      data: {
        type: "betaGroups",
        attributes: { name: "Equipo", isInternalGroup: true, hasAccessToAllBuilds: true },
        relationships: { app: { data: { type: "apps", id: app.id } } },
      },
    });
    group = r.data;
    console.log(`✓ grupo interno creado: ${group.attributes.name} (id=${group.id})`);
  } else {
    console.log(`✓ grupo interno ya existe: ${group.attributes.name} (id=${group.id})`);
    if (!group.attributes.hasAccessToAllBuilds) {
      await asc("PATCH", `/betaGroups/${group.id}`, {
        data: { type: "betaGroups", id: group.id, attributes: { hasAccessToAllBuilds: true } },
      });
      console.log("✓ acceso automático a todos los builds activado");
    }
  }
  const users = await asc("GET", "/users?limit=200");
  const testers = await asc("GET", `/betaTesters?filter[betaGroups]=${group.id}&limit=200`);
  const ya = new Set(testers.data.map((t) => (t.attributes.email || "").toLowerCase()));
  for (const u of users.data) {
    const email = (u.attributes.username || "").toLowerCase();
    if (!email || ya.has(email)) continue;
    try {
      await asc("POST", "/betaTesters", {
        data: {
          type: "betaTesters",
          attributes: { email, firstName: u.attributes.firstName, lastName: u.attributes.lastName },
          relationships: { betaGroups: { data: [{ type: "betaGroups", id: group.id }] } },
        },
      });
      console.log(`✓ probador interno dado de alta: ${email}`);
    } catch (e) {
      console.log(`(no se pudo dar de alta a ${email}: ${String(e.message).split("\n")[1] ?? "error"})`);
    }
  }
  console.log("✓ TestFlight interno listo: cada build procesado llega solo al grupo.");
}

// ─── La ficha de la App Store (metadatos, capturas, categorías, precio…) ──
// Añadido en septiembre de 2026 para dejar la ficha entera desde el terminal.
// Fuentes: marketing/metadata/<locale-asc>/*.txt y marketing/output/<locale-asc>/{iphone,ipad}/NN.png

import { readdir, stat } from "node:fs/promises";
import { createHash } from "node:crypto";
import { dirname } from "node:path";
import { fileURLToPath } from "node:url";

const RAIZ = join(dirname(fileURLToPath(import.meta.url)), "..");
const METADATA_DIR = join(RAIZ, "marketing", "metadata");
const OUTPUT_DIR = join(RAIZ, "marketing", "output");

async function leerFichero(locale, nombre) {
  try { return (await readFile(join(METADATA_DIR, locale, nombre), "utf-8")).trim(); } catch { return null; }
}
async function localesEnMetadata() {
  const out = [];
  for (const e of await readdir(METADATA_DIR)) {
    if ((await stat(join(METADATA_DIR, e))).isDirectory()) out.push(e);
  }
  return out.sort();
}
function detalleError(e) {
  const m = String(e.message);
  const i = m.indexOf("\n");
  try { const j = JSON.parse(m.slice(i + 1)); return j.errors?.map((x) => `${x.code}: ${x.detail}`).join(" | ") ?? m; } catch { return m.split("\n")[0]; }
}

async function versionEditable(app, versionString) {
  const r = await asc("GET", `/apps/${app.id}/appStoreVersions?filter[platform]=IOS&limit=10`);
  const exacta = r.data.find((v) => v.attributes.versionString === versionString);
  if (exacta) return exacta;
  // Si hay una versión aún sin enviar, se renombra a la pedida (la 1.0 vacía
  // que crea ASC al dar de alta la app).
  const abierta = r.data.find((v) => ["PREPARE_FOR_SUBMISSION", "DEVELOPER_REJECTED", "REJECTED", "METADATA_REJECTED", "WAITING_FOR_REVIEW"].includes(v.attributes.appStoreState));
  if (abierta && versionString) {
    console.log(`→ renombrando la versión ${abierta.attributes.versionString} (${abierta.attributes.appStoreState}) a ${versionString}`);
    const p = await asc("PATCH", `/appStoreVersions/${abierta.id}`, {
      data: { type: "appStoreVersions", id: abierta.id, attributes: { versionString } },
    });
    return p.data;
  }
  const c = await asc("POST", "/appStoreVersions", {
    data: { type: "appStoreVersions", attributes: { versionString, platform: "IOS", releaseType: "MANUAL" },
      relationships: { app: { data: { type: "apps", id: app.id } } } },
  });
  console.log(`→ versión ${versionString} creada`);
  return c.data;
}

async function cmd_pushMetadata() {
  const versionString = process.argv[3];
  if (!versionString) throw new Error("uso: push-metadata <versión>");
  const app = await getApp();
  const version = await versionEditable(app, versionString);
  console.log(`versión ${version.attributes.versionString} (${version.attributes.appStoreState}, id=${version.id})`);
  const infos = await asc("GET", `/apps/${app.id}/appInfos?limit=5`);
  const appInfo = infos.data.find((i) => i.attributes.state !== "READY_FOR_SALE") ?? infos.data[0];
  const vLocs = await asc("GET", `/appStoreVersions/${version.id}/appStoreVersionLocalizations?limit=200`);
  const porLocaleV = Object.fromEntries(vLocs.data.map((d) => [d.attributes.locale, d]));
  const aLocs = await asc("GET", `/appInfos/${appInfo.id}/appInfoLocalizations?limit=200`);
  const porLocaleA = Object.fromEntries(aLocs.data.map((d) => [d.attributes.locale, d]));
  const locales = await localesEnMetadata();
  const solo = process.argv[4];
  let ok = 0;
  for (const locale of locales) {
    if (solo && locale !== solo) continue;
    const [name, subtitle, description, keywords, promo, marketingUrl, supportUrl, privacyUrl, whatsNew] = await Promise.all(
      ["name", "subtitle", "description", "keywords", "promotional_text", "marketing_url", "support_url", "privacy_url", "release_notes"].map((f) => leerFichero(locale, `${f}.txt`)));
    let appOk = true, verOk = true;
    const aAttrs = {};
    if (name) aAttrs.name = name;
    if (subtitle) aAttrs.subtitle = subtitle;
    if (privacyUrl) aAttrs.privacyPolicyUrl = privacyUrl;
    try {
      if (porLocaleA[locale]) {
        await asc("PATCH", `/appInfoLocalizations/${porLocaleA[locale].id}`, { data: { type: "appInfoLocalizations", id: porLocaleA[locale].id, attributes: aAttrs } });
      } else {
        await asc("POST", "/appInfoLocalizations", { data: { type: "appInfoLocalizations", attributes: { locale, ...aAttrs },
          relationships: { appInfo: { data: { type: "appInfos", id: appInfo.id } } } } });
      }
    } catch (e) { appOk = false; console.log(`   [${locale} app-level] ${detalleError(e)}`); }
    // Al crear la localización de app, ASC crea sola la de versión: releer.
    if (!porLocaleV[locale]) {
      const otra = await asc("GET", `/appStoreVersions/${version.id}/appStoreVersionLocalizations?limit=200`);
      for (const d of otra.data) porLocaleV[d.attributes.locale] = d;
    }
    const vAttrs = {};
    if (description) vAttrs.description = description;
    if (keywords) vAttrs.keywords = keywords;
    if (promo) vAttrs.promotionalText = promo;
    if (marketingUrl) vAttrs.marketingUrl = marketingUrl;
    if (supportUrl) vAttrs.supportUrl = supportUrl;
    if (whatsNew) vAttrs.whatsNew = whatsNew;
    try {
      if (porLocaleV[locale]) {
        await asc("PATCH", `/appStoreVersionLocalizations/${porLocaleV[locale].id}`, { data: { type: "appStoreVersionLocalizations", id: porLocaleV[locale].id, attributes: vAttrs } });
      } else {
        await asc("POST", "/appStoreVersionLocalizations", { data: { type: "appStoreVersionLocalizations", attributes: { locale, ...vAttrs },
          relationships: { appStoreVersion: { data: { type: "appStoreVersions", id: version.id } } } } });
      }
    } catch (e) { verOk = false; console.log(`   [${locale} version-level] ${detalleError(e)}`); }
    console.log(`  ${appOk && verOk ? "✓" : "✗"} ${locale}`);
    if (appOk && verOk) ok++;
  }
  console.log(`\n${ok}/${solo ? 1 : locales.length} idiomas al día.`);
}

// Capturas: marketing/output/<locale-asc>/iphone/NN.png (6,9") e ipad/NN.png (13")
const TIPOS = { iphone: "APP_IPHONE_67", ipad: "APP_IPAD_PRO_3GEN_129" };

async function conjuntoCapturas(versionLocId, tipo) {
  const ex = await asc("GET", `/appStoreVersionLocalizations/${versionLocId}/appScreenshotSets?limit=20`);
  const f = ex.data.find((d) => d.attributes.screenshotDisplayType === tipo);
  if (f) return f.id;
  const c = await asc("POST", "/appScreenshotSets", { data: { type: "appScreenshotSets", attributes: { screenshotDisplayType: tipo },
    relationships: { appStoreVersionLocalization: { data: { type: "appStoreVersionLocalizations", id: versionLocId } } } } });
  return c.data.id;
}
async function vaciarConjunto(setId) {
  const ex = await asc("GET", `/appScreenshotSets/${setId}/appScreenshots?limit=20`);
  for (const s of ex.data) { try { await asc("DELETE", `/appScreenshots/${s.id}`); } catch {} }
}
async function subirCaptura(setId, ruta) {
  const buf = await readFile(ruta);
  const fileName = ruta.split("/").pop();
  const c = await asc("POST", "/appScreenshots", { data: { type: "appScreenshots", attributes: { fileName, fileSize: buf.length },
    relationships: { appScreenshotSet: { data: { type: "appScreenshotSets", id: setId } } } } });
  for (const op of c.data.attributes.uploadOperations || []) {
    const headers = {}; for (const h of op.requestHeaders || []) headers[h.name] = h.value;
    const r = await fetch(op.url, { method: op.method, headers, body: buf.subarray(op.offset, op.offset + op.length) });
    if (!r.ok) throw new Error(`subida fallida: ${r.status} ${await r.text()}`);
  }
  await asc("PATCH", `/appScreenshots/${c.data.id}`, { data: { type: "appScreenshots", id: c.data.id,
    attributes: { uploaded: true, sourceFileChecksum: createHash("md5").update(buf).digest("hex") } } });
}

async function cmd_pushScreenshots() {
  const versionString = process.argv[3];
  const solo = process.argv[4];
  if (!versionString) throw new Error("uso: push-screenshots <versión> [locale]");
  const app = await getApp();
  const version = await versionEditable(app, versionString);
  const vLocs = await asc("GET", `/appStoreVersions/${version.id}/appStoreVersionLocalizations?limit=200`);
  const porLocale = Object.fromEntries(vLocs.data.map((d) => [d.attributes.locale, d]));
  const locales = (await readdir(OUTPUT_DIR)).filter((e) => !e.startsWith(".") && !e.startsWith("_")).sort();
  let ok = 0, n = 0;
  for (const locale of locales) {
    if (solo && locale !== solo) continue;
    if (!(await stat(join(OUTPUT_DIR, locale))).isDirectory()) continue;
    n++;
    const loc = porLocale[locale];
    if (!loc) { console.log(`  ⚠ ${locale}: sin localización en ASC (haz push-metadata antes)`); continue; }
    try {
      const partes = [];
      for (const [carpeta, tipo] of Object.entries(TIPOS)) {
        // SOLO_DEVICE=iphone|ipad sube solo ese tipo y no toca el otro set.
        if (process.env.SOLO_DEVICE && process.env.SOLO_DEVICE !== carpeta) continue;
        const dir = join(OUTPUT_DIR, locale, carpeta);
        let pngs = [];
        try { pngs = (await readdir(dir)).filter((f) => /^\d\d\.png$/.test(f)).sort(); } catch { continue; }
        if (!pngs.length) continue;
        const setId = await conjuntoCapturas(loc.id, tipo);
        await vaciarConjunto(setId);
        for (const f of pngs) await subirCaptura(setId, join(dir, f));
        partes.push(`${carpeta}:${pngs.length}`);
      }
      console.log(`  ✓ ${locale} (${partes.join(", ")})`);
      ok++;
    } catch (e) { console.log(`  ✗ ${locale}: ${detalleError(e)}`); }
  }
  console.log(`\n${ok}/${n} idiomas con capturas.`);
}

async function cmd_prepareListing() {
  // Todo lo que no es texto ni imagen: categorías, edades, contacto de
  // revisión, precio (gratis), disponibilidad mundial, copyright.
  const versionString = process.argv[3];
  if (!versionString) throw new Error("uso: prepare-listing <versión>");
  const app = await getApp();
  const version = await versionEditable(app, versionString);
  const infos = await asc("GET", `/apps/${app.id}/appInfos?limit=5`);
  const appInfo = infos.data.find((i) => i.attributes.state !== "READY_FOR_SALE") ?? infos.data[0];

  // 1. Categorías
  try {
    await asc("PATCH", `/appInfos/${appInfo.id}`, { data: { type: "appInfos", id: appInfo.id, relationships: {
      primaryCategory: { data: { type: "appCategories", id: "PRODUCTIVITY" } },
      secondaryCategory: { data: { type: "appCategories", id: "EDUCATION" } } } } });
    console.log("✓ categorías: Productividad / Educación");
  } catch (e) { console.log(`✗ categorías: ${detalleError(e)}`); }

  // 2. Clasificación por edades (4+): nada de nada.
  try {
    const ar = await asc("GET", `/appInfos/${appInfo.id}/ageRatingDeclaration`);
    const attrs = {
      advertising: false, alcoholTobaccoOrDrugUseOrReferences: "NONE", contests: "NONE", gambling: false,
      gamblingSimulated: "NONE", healthOrWellnessTopics: false, lootBox: false, medicalOrTreatmentInformation: "NONE",
      messagingAndChat: false, parentalControls: false, profanityOrCrudeHumor: "NONE", sexualContentGraphicAndNudity: "NONE",
      sexualContentOrNudity: "NONE", horrorOrFearThemes: "NONE", matureOrSuggestiveThemes: "NONE", unrestrictedWebAccess: false,
      userGeneratedContent: false, violenceCartoonOrFantasy: "NONE", violenceRealisticProlongedGraphicOrSadistic: "NONE",
      violenceRealistic: "NONE", gunsOrOtherWeapons: "NONE", socialMedia: false, ageAssurance: false,
    };
    const intentar = async (a) => asc("PATCH", `/ageRatingDeclarations/${ar.data.id}`, { data: { type: "ageRatingDeclarations", id: ar.data.id, attributes: a } });
    try { await intentar(attrs); }
    catch (e) {
      // Quitar los atributos que esta versión de la API no conozca y reintentar.
      const malos = [...String(e.message).matchAll(/\/data\/attributes\/(\w+)/g)].map((m) => m[1]);
      for (const m of malos) delete attrs[m];
      await intentar(attrs);
      if (malos.length) console.log(`  (sin ${malos.join(", ")})`);
    }
    console.log("✓ clasificación por edades: 4+");
  } catch (e) { console.log(`✗ edades: ${detalleError(e)}`); }

  // 3. Datos de contacto para la revisión
  try {
    const attrs = {
      contactFirstName: "José Luis", contactLastName: "Saorín Ferrer", contactPhone: "+34622512078", contactEmail: "jlsf2005@gmail.com",
      demoAccountRequired: false,
      notes: [
        "Yappy reads any text aloud with an on-device model (Supertonic 3, ~200 MB, downloaded once on first launch from our own Cloudflare store). No account is needed.",
        "Main entry point: the iOS Share Sheet (share an article from Safari, a PDF from Files, or a YouTube link) or the + button on the tape (paste text or a link). Two sample stories are bundled and play without the model.",
        "In-app purchases (Yappy Parlanchín: monthly, yearly, lifetime) unlock an unlimited perch (the free tier holds 3 documents at a time), the audiobook press and the desktop bridge. They are handled by StoreKit through RevenueCat; sandbox purchases work with a sandbox Apple account.",
        "Optional anonymous usage statistics (no names, texts or links) go to our own Cloudflare Worker; they can be switched off in the settings drawer. Privacy policy: https://yappy.joseluissaorin.com/en/privacy/",
      ].join("\n\n"),
    };
    const ex = await asc("GET", `/appStoreVersions/${version.id}/appStoreReviewDetail`).catch(() => null);
    if (ex?.data) {
      await asc("PATCH", `/appStoreReviewDetails/${ex.data.id}`, { data: { type: "appStoreReviewDetails", id: ex.data.id, attributes: attrs } });
    } else {
      await asc("POST", "/appStoreReviewDetails", { data: { type: "appStoreReviewDetails", attributes: attrs,
        relationships: { appStoreVersion: { data: { type: "appStoreVersions", id: version.id } } } } });
    }
    console.log("✓ contacto de revisión y notas");
  } catch (e) { console.log(`✗ contacto de revisión: ${detalleError(e)}`); }

  // 4. Copyright y lanzamiento manual
  try {
    await asc("PATCH", `/appStoreVersions/${version.id}`, { data: { type: "appStoreVersions", id: version.id,
      attributes: { copyright: "2026 José Luis Saorín Ferrer", releaseType: "MANUAL" } } });
    console.log("✓ copyright y lanzamiento manual tras la aprobación");
  } catch (e) { console.log(`✗ copyright: ${detalleError(e)}`); }

  // 5. Precio: gratis (punto de precio 0 en la base España)
  try {
    const ya = await asc("GET", `/apps/${app.id}/appPriceSchedule/manualPrices?include=appPricePoint&limit=5`).catch(() => null);
    if (ya?.data?.length) {
      console.log(`✓ precio ya fijado (${ya.included?.[0]?.attributes?.customerPrice ?? "?"})`);
    } else {
      let pts = await asc("GET", `/apps/${app.id}/appPricePoints?filter[territory]=ESP&limit=200`);
      let gratis = pts.data.find((p) => Number(p.attributes.customerPrice) === 0);
      while (!gratis && pts.links?.next) { pts = await asc("GET", pts.links.next); gratis = pts.data.find((p) => Number(p.attributes.customerPrice) === 0); }
      if (!gratis) throw new Error("no encuentro el punto de precio 0 en ESP");
      await asc("POST", "/appPriceSchedules", {
        data: { type: "appPriceSchedules", relationships: {
          app: { data: { type: "apps", id: app.id } },
          baseTerritory: { data: { type: "territories", id: "ESP" } },
          manualPrices: { data: [{ type: "appPrices", id: "${gratis}" }] } } },
        included: [{ type: "appPrices", id: "${gratis}", attributes: { startDate: null },
          relationships: { appPricePoint: { data: { type: "appPricePoints", id: gratis.id } } } }],
      });
      console.log("✓ precio: gratis (base España)");
    }
  } catch (e) { console.log(`✗ precio: ${detalleError(e)}`); }

  // 6. Disponibilidad: todos los territorios (v2, hay que enumerarlos todos)
  try {
    const ya = await asc("GET", `/apps/${app.id}/appAvailabilityV2`).catch(() => null);
    if (ya?.data) {
      console.log("✓ disponibilidad ya configurada");
    } else {
      let terr = [], r = await asc("GET", "/territories?limit=200");
      terr.push(...r.data.map((t) => t.id));
      while (r.links?.next) { r = await asc("GET", r.links.next); terr.push(...r.data.map((t) => t.id)); }
      await asc("POST", "https://api.appstoreconnect.apple.com/v2/appAvailabilities", {
        data: { type: "appAvailabilities", attributes: { availableInNewTerritories: true },
          relationships: { app: { data: { type: "apps", id: app.id } },
            territoryAvailabilities: { data: terr.map((t) => ({ type: "territoryAvailabilities", id: `\${ta-${t}}` })) } } },
        included: terr.map((t) => ({ type: "territoryAvailabilities", id: `\${ta-${t}}`, attributes: { available: true },
          relationships: { territory: { data: { type: "territories", id: t } } } })),
      });
      console.log(`✓ disponibilidad: ${terr.length} territorios`);
    }
  } catch (e) { console.log(`✗ disponibilidad: ${detalleError(e)}`); }

  // 7. Derechos de contenido de terceros
  try {
    await asc("PATCH", `/apps/${app.id}`, { data: { type: "apps", id: app.id, attributes: { contentRightsDeclaration: "DOES_NOT_USE_THIRD_PARTY_CONTENT" } } });
    console.log("✓ declaración de derechos de contenido");
  } catch (e) { console.log(`✗ derechos: ${detalleError(e)}`); }
}

async function cmd_attachBuild() {
  const versionString = process.argv[3];
  if (!versionString) throw new Error("uso: attach-build <versión> [build]");
  const app = await getApp();
  const version = await versionEditable(app, versionString);
  const builds = await buildState({ version: versionString, app });
  const pedido = process.argv[4];
  const b = pedido ? builds.find((x) => x.attributes.version === pedido) : builds.find((x) => x.attributes.processingState === "VALID");
  if (!b) throw new Error("no hay build válido para esa versión");
  await asc("PATCH", `/appStoreVersions/${version.id}/relationships/build`, { data: { type: "builds", id: b.id } });
  console.log(`✓ build ${b.attributes.version} enganchado a la versión ${versionString}`);
}

async function cmd_listingStatus() {
  const app = await getApp();
  const vs = await asc("GET", `/apps/${app.id}/appStoreVersions?filter[platform]=IOS&limit=5&include=build`);
  for (const v of vs.data) {
    const locs = await asc("GET", `/appStoreVersions/${v.id}/appStoreVersionLocalizations?limit=200`);
    let conCapturas = 0, conTexto = 0;
    for (const l of locs.data) {
      if (l.attributes.description) conTexto++;
      const sets = await asc("GET", `/appStoreVersionLocalizations/${l.id}/appScreenshotSets?include=appScreenshots&limit=20`);
      if (sets.data.some((s) => (s.relationships?.appScreenshots?.data?.length ?? 0) > 0)) conCapturas++;
    }
    console.log(`${v.attributes.versionString} ${v.attributes.appStoreState} build=${v.relationships?.build?.data?.id ?? "-"} idiomas=${locs.data.length} con_texto=${conTexto} con_capturas=${conCapturas}`);
  }
}

const sub = process.argv[2];
switch (sub) {
  case "status":              await cmd_status(); break;
  case "wait-for-app":        await cmd_waitForApp(); break;
  case "list-builds":         await cmd_listBuilds(); break;
  case "wait-for-build":      await cmd_waitForBuild(); break;
  case "create-beta-group":   await cmd_createBetaGroup(); break;
  case "add-build-to-beta":   await cmd_addBuildToBeta(); break;
  case "submit-beta-review":  await cmd_submitBetaReview(); break;
  case "pipeline":            await cmd_pipeline(); break;
  case "internal-group":      await cmd_internalGroup(); break;
  case "push-metadata":       await cmd_pushMetadata(); break;
  case "push-screenshots":    await cmd_pushScreenshots(); break;
  case "prepare-listing":     await cmd_prepareListing(); break;
  case "attach-build":        await cmd_attachBuild(); break;
  case "listing-status":      await cmd_listingStatus(); break;
  default:
    console.log("usage: asc-helper.mjs <status|wait-for-app|list-builds|wait-for-build|create-beta-group|add-build-to-beta|submit-beta-review|pipeline|internal-group|push-metadata <v> [locale]|push-screenshots <v> [locale]|prepare-listing <v>|attach-build <v> [build]|listing-status>");
    process.exit(1);
}
