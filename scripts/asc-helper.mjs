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
  const r = await asc("GET", `/builds?filter[app]=${app.id}&filter[preReleaseVersion.version]=${encodeURIComponent(version)}&limit=10`);
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
  default:
    console.log("usage: asc-helper.mjs <status|wait-for-app|list-builds|wait-for-build|create-beta-group|add-build-to-beta|submit-beta-review|pipeline>");
    process.exit(1);
}
