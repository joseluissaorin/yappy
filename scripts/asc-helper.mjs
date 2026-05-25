#!/usr/bin/env node
// App Store Connect REST API helper for Yappy.
// Bootstrapped from ~/Developer/Kalorie/scripts/asc-helper.mjs.
//
// Reads the personal `.p8` from ~/.appstoreconnect/private_keys/AuthKey_<KID>.p8.
// Signs JWTs with ES256 using ieee-p1363 encoding (Apple rejects DER).

import { createSign, createPrivateKey } from "node:crypto";
import { readFile } from "node:fs/promises";
import { homedir } from "node:os";
import { join } from "node:path";

const KEY_ID    = process.env.ASC_KEY_ID    ?? "4HVB5YWGWD";
const ISSUER_ID = process.env.ASC_ISSUER_ID ?? "1e05b19d-c430-4408-8af5-24d6623959c4";
const TEAM_ID   = process.env.ASC_TEAM_ID   ?? "9LYNY2477X";
const BUNDLE_ID = process.env.ASC_BUNDLE_ID ?? "com.yappy.app";

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

async function cmd_status() {
  const app = await getApp();
  if (!app) {
    console.log(`✗ no App in ASC for bundleId ${BUNDLE_ID}`);
    console.log(`  → create via https://appstoreconnect.apple.com/apps → My Apps → +`);
    console.log(`  → or via ASC API POST /apps (see scripts/asc-helper.mjs cmd_create_app)`);
    return;
  }
  console.log(`✓ App found: ${app.attributes.name} (id=${app.id}, bundleId=${app.attributes.bundleId})`);
  // List recent builds for this app
  const builds = await asc("GET", `/builds?filter[app]=${app.id}&sort=-uploadedDate&limit=5`);
  console.log(`  recent builds: ${builds.data.length}`);
  for (const b of builds.data) {
    console.log(`    - ${b.attributes.version} (${b.attributes.processingState}, uploaded ${b.attributes.uploadedDate})`);
  }
}

async function cmd_create_app() {
  // Creates a new App resource in ASC. Bundle ID must already exist in
  // Apple Developer portal (it does — we have a Distribution profile signed).
  const r = await asc("POST", "/apps", {
    data: {
      type: "apps",
      attributes: {
        bundleId: BUNDLE_ID,
        name: "Yappy",
        primaryLocale: "en-US",
        sku: "yappy-ios",
      },
    },
  });
  console.log("Created:", JSON.stringify(r.data, null, 2));
}

const sub = process.argv[2];
switch (sub) {
  case "status":     await cmd_status(); break;
  case "create-app": await cmd_create_app(); break;
  default:
    console.log("usage: asc-helper.mjs <status|create-app>");
    process.exit(1);
}
