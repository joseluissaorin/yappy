#!/usr/bin/env node
// El gemelo Android del asc-helper: habla con Google Play (androidpublisher
// v3) usando la cuenta de servicio de ~/.config/yappy-play/. Sin deps.
//
//   node scripts/play-helper.mjs token
//   node scripts/play-helper.mjs status
//   node scripts/play-helper.mjs upload <fichero.aab|.apk> [internal|alpha|beta|production]
import { readFileSync } from "node:fs";
import { createSign } from "node:crypto";
import { homedir } from "node:os";

const PAQUETE = process.env.PLAY_PACKAGE || "com.yappy.app";
const SA = JSON.parse(
  readFileSync(`${homedir()}/.config/yappy-play/service-account.json`, "utf8"),
);

const b64 = (o) => Buffer.from(JSON.stringify(o)).toString("base64url");

async function token() {
  const now = Math.floor(Date.now() / 1000);
  const sin = `${b64({ alg: "RS256", typ: "JWT" })}.${b64({
    iss: SA.client_email,
    scope: "https://www.googleapis.com/auth/androidpublisher",
    aud: "https://oauth2.googleapis.com/token",
    iat: now,
    exp: now + 3600,
  })}`;
  const firma = createSign("RSA-SHA256").update(sin).sign(SA.private_key, "base64url");
  const r = await fetch("https://oauth2.googleapis.com/token", {
    method: "POST",
    headers: { "Content-Type": "application/x-www-form-urlencoded" },
    body: new URLSearchParams({
      grant_type: "urn:ietf:params:oauth:grant-type:jwt-bearer",
      assertion: `${sin}.${firma}`,
    }),
  });
  if (!r.ok) throw new Error(`token: ${r.status} ${await r.text()}`);
  return (await r.json()).access_token;
}

const BASE = `https://androidpublisher.googleapis.com/androidpublisher/v3/applications/${PAQUETE}`;

async function api(tok, ruta, opts = {}) {
  const r = await fetch(`${BASE}${ruta}`, {
    ...opts,
    headers: { Authorization: `Bearer ${tok}`, "Content-Type": "application/json", ...(opts.headers ?? {}) },
  });
  const cuerpo = await r.text();
  if (!r.ok) throw new Error(`${ruta}: ${r.status} ${cuerpo.slice(0, 400)}`);
  return cuerpo ? JSON.parse(cuerpo) : {};
}

async function status() {
  const tok = await token();
  try {
    const e = await api(tok, "/edits", { method: "POST", body: "{}" });
    console.log(JSON.stringify({ ok: true, paquete: PAQUETE, edit: e.id }));
    await api(tok, `/edits/${e.id}`, { method: "DELETE" }).catch(() => {});
  } catch (err) {
    console.log(JSON.stringify({ ok: false, paquete: PAQUETE, error: String(err.message) }));
    process.exitCode = 1;
  }
}

async function upload(fichero, pista = "internal") {
  const tok = await token();
  const edit = await api(tok, "/edits", { method: "POST", body: "{}" });
  const esApk = fichero.toLowerCase().endsWith(".apk");
  const tipo = esApk ? "apks" : "bundles";
  const mime = esApk ? "application/vnd.android.package-archive" : "application/octet-stream";
  const subida = await fetch(
    `https://androidpublisher.googleapis.com/upload/androidpublisher/v3/applications/${PAQUETE}/edits/${edit.id}/${tipo}?uploadType=media`,
    { method: "POST", headers: { Authorization: `Bearer ${tok}`, "Content-Type": mime }, body: readFileSync(fichero) },
  );
  const artefacto = JSON.parse(await subida.text());
  if (!subida.ok) throw new Error(`upload: ${JSON.stringify(artefacto).slice(0, 400)}`);
  const version = artefacto.versionCode;
  await api(tok, `/edits/${edit.id}/tracks/${pista}`, {
    method: "PUT",
    body: JSON.stringify({ releases: [{ versionCodes: [String(version)], status: "completed" }] }),
  });
  const fin = await api(tok, `/edits/${edit.id}:commit`, { method: "POST", body: "{}" });
  console.log(JSON.stringify({ ok: true, pista, versionCode: version, edit: fin.id }));
}

const [cmd, ...args] = process.argv.slice(2);
try {
  if (cmd === "token") console.log(await token());
  else if (cmd === "status") await status();
  else if (cmd === "upload") await upload(args[0], args[1]);
  else {
    console.error("uso: play-helper.mjs token|status|upload <fichero> [pista]");
    process.exitCode = 2;
  }
} catch (e) {
  console.error(String(e.message ?? e));
  process.exitCode = 1;
}
