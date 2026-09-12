#!/usr/bin/env node
// Genera los flujos Maestro por idioma a partir de los diccionarios de la
// app (marketing/i18n-dump.json). Maestro casa los textos como regex
// anclada: se escapan los especiales.
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const AQUI = dirname(fileURLToPath(import.meta.url));
const D = JSON.parse(readFileSync(join(AQUI, "..", "i18n-dump.json"), "utf8"));
const NOMBRE = {
  es: "Español", en: "English", fr: "Français", de: "Deutsch", it: "Italiano",
  pt: "Português", nl: "Nederlands", pl: "Polski", ro: "Română", sv: "Svenska",
  da: "Dansk", fi: "Suomi", et: "Eesti", lt: "Lietuvių", lv: "Latviešu",
  hr: "Hrvatski", sl: "Slovenščina", sk: "Slovenčina", cs: "Čeština",
  hu: "Magyar", el: "Ελληνικά", bg: "Български", uk: "Українська",
  ru: "Русский", tr: "Türkçe", ar: "العربية", hi: "हिन्दी",
  id: "Bahasa Indonesia", vi: "Tiếng Việt", ko: "한국어", ja: "日本語",
};
const IPAD = process.argv.includes("--ipad");
const BOCA = IPAD ? "94%,94%" : "86%,91%";
const rx = (s) => String(s).replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
const q = (s) => JSON.stringify(s);

for (const L of Object.keys(D)) {
  const t = (k) => { const v = D[L][k] ?? D.en[k]; if (v == null) throw new Error(`${L}: falta ${k}`); return v; };
  const otro = L === "fr" ? NOMBRE.de : NOMBRE.fr;
  const dir = join(AQUI, IPAD ? "flows-ipad" : "flows", L);
  mkdirSync(dir, { recursive: true });
  const shots = `marketing/${IPAD ? "screenshots-ipad" : "screenshots"}/${L}`;
  const tecla = t("paseo.pajaro.tecla").replace("{voz}", "James");

  const paseo = `appId: com.joseluissaorin.yappy
---
- launchApp:
    stopApp: false
- extendedWaitUntil:
    visible:
      text: ${q(rx(t("paseo.hola.titulo")))}
    timeout: 60000
- waitForAnimationToEnd:
    timeout: 3000
- takeScreenshot: ${shots}/01-paseo-hola
- tapOn:
    text: ${q(rx(t("paseo.hola.tecla")))}
- extendedWaitUntil:
    visible:
      text: ${q(rx(t("paseo.idiomas.titulo")))}
    timeout: 10000
- tapOn:
    text: ${q(rx(otro))}
- waitForAnimationToEnd:
    timeout: 2500
- takeScreenshot: ${shots}/02-paseo-idiomas
- tapOn:
    text: ${q(rx(t("paseo.sigue")))}
- extendedWaitUntil:
    visible:
      text: ${q(rx(t("paseo.pajaro.titulo")))}
    timeout: 10000
- tapOn:
    text: "James"
- waitForAnimationToEnd:
    timeout: 2500
- takeScreenshot: ${shots}/03-paseo-pajaro
- tapOn:
    text: ${q(rx(tecla))}
- extendedWaitUntil:
    visible:
      text: ${q(rx(t("paseo.que.titulo")))}
    timeout: 10000
- tapOn:
    text: ${q(rx(t("paseo.que.articulos")))}
- tapOn:
    text: ${q(rx(t("paseo.que.libros")))}
- waitForAnimationToEnd:
    timeout: 2000
- takeScreenshot: ${shots}/04-paseo-que
- tapOn:
    text: ${q(rx(t("paseo.sigue")))}
- extendedWaitUntil:
    visible:
      text: ${q(rx(t("paseo.cuando.titulo")))}
    timeout: 10000
- tapOn:
    text: ${q(rx(t("paseo.cuando.cocinando")))}
- waitForAnimationToEnd:
    timeout: 2000
- takeScreenshot: ${shots}/05-paseo-cuando
- tapOn:
    text: ${q(rx(t("paseo.sigue")))}
- extendedWaitUntil:
    visible:
      text: ${q(rx(t("paseo.velocidad.titulo")))}
    timeout: 10000
- waitForAnimationToEnd:
    timeout: 2000
- takeScreenshot: ${shots}/06-paseo-velocidad
- tapOn:
    text: ${q(rx(t("paseo.sigue")))}
- extendedWaitUntil:
    visible:
      text: ${q(rx(t("paseo.cuanto.titulo")))}
    timeout: 10000
- tapOn:
    text: ${q(rx(t("paseo.cuanto.montana")))}
- waitForAnimationToEnd:
    timeout: 2000
- takeScreenshot: ${shots}/07-paseo-cuanto
- tapOn:
    text: ${q(rx(t("paseo.sigue")))}
- extendedWaitUntil:
    visible:
      text: ${q(rx(t("paseo.escucha.titulo")))}
    timeout: 10000
- waitForAnimationToEnd:
    timeout: 2000
- takeScreenshot: ${shots}/08-paseo-escucha
- tapOn:
    text: ${q("(?i)" + rx(t("paseo.luego")))}
- extendedWaitUntil:
    visible:
      text: ${q(rx(t("paseo.gestos.titulo")))}
    timeout: 10000
- tapOn:
    text: ${q("(?i)" + rx(t("paseo.luego")))}
- extendedWaitUntil:
    visible:
      text: ${q(rx(t("paseo.compartir.titulo")))}
    timeout: 10000
- waitForAnimationToEnd:
    timeout: 2500
- takeScreenshot: ${shots}/09-paseo-compartir
- tapOn:
    text: ${q("(?i)" + rx(t("paseo.luego")))}
- extendedWaitUntil:
    visible:
      text: ${q(rx(t("paseo.exportar.titulo")))}
    timeout: 10000
- waitForAnimationToEnd:
    timeout: 2000
- takeScreenshot: ${shots}/10-paseo-exportar
- tapOn:
    text: ${q("(?i)" + rx(t("paseo.luego")))}
- extendedWaitUntil:
    visible:
      text: ${q(rx(t("paseo.favoritos.titulo")))}
    timeout: 10000
- waitForAnimationToEnd:
    timeout: 2000
- takeScreenshot: ${shots}/11-paseo-favoritos
- tapOn:
    text: ${q(rx(t("paseo.favoritos.hecho")))}
- extendedWaitUntil:
    visible:
      text: ${q(rx(t("paseo.resumen.titulo")))}
    timeout: 10000
- waitForAnimationToEnd:
    timeout: 2000
- takeScreenshot: ${shots}/12-paseo-resumen
- tapOn:
    text: ${q(rx(t("paseo.sigue")))}
- extendedWaitUntil:
    visible:
      text: ${q(rx(t("paseo.percha.titulo")))}
    timeout: 10000
- waitForAnimationToEnd:
    timeout: 2000
- takeScreenshot: ${shots}/13-paseo-percha
- tapOn:
    text: ${q(rx(t("paseo.sigue")))}
- extendedWaitUntil:
    visible:
      text: ${q(rx(t("pro.elige")))}
    timeout: 15000
- waitForAnimationToEnd:
    timeout: 3000
- takeScreenshot: ${shots}/14-paseo-cuerda
- scrollUntilVisible:
    element:
      text: ${q(rx(t("pro.empezar_percha")))}
    direction: DOWN
    timeout: 15000
- takeScreenshot: ${shots}/15-paseo-cuerda-pie
- tapOn:
    text: ${q(rx(t("pro.empezar_percha")))}
- extendedWaitUntil:
    visible:
      text: "yappy"
    timeout: 15000
- waitForAnimationToEnd:
    timeout: 3000
- tapOn:
    text: ${q(rx(t("aviso.cerrar")))}
    optional: true
- takeScreenshot: ${shots}/17-cinta-vacia
`;

  const anadir = `appId: com.joseluissaorin.yappy
---
- launchApp:
    stopApp: false
- tapOn:
    point: "30%,3%"
- tapOn:
    text: ${q(rx(t("aviso.cerrar")))}
    optional: true
# Abrir la boca solo si no está ya abierta (un fallo anterior la deja abierta).
- runFlow:
    when:
      notVisible:
        text: ${q(rx(t("cinta.portapapeles")))}
    commands:
      - tapOn:
          point: ${q(BOCA)}
- extendedWaitUntil:
    visible:
      text: ${q(rx(t("cinta.portapapeles")))}
    timeout: 8000
- waitForAnimationToEnd:
    timeout: 1500
- takeScreenshot: ${shots}/16-boca
- tapOn:
    text: ${q(rx(t("cinta.portapapeles")))}
- tapOn:
    text: "Permitir pegar|Allow Paste"
    optional: true
- waitForAnimationToEnd:
    timeout: 4000
`;

  const resto = (titulo) => `appId: com.joseluissaorin.yappy
---
- launchApp:
    stopApp: false
- tapOn:
    point: "30%,3%"
- tapOn:
    text: ${q(rx(t("aviso.cerrar")))}
    optional: true
- waitForAnimationToEnd:
    timeout: 3000
- takeScreenshot: ${shots}/20-cinta
- tapOn:
    text: ${q(rx(t("cinta.trastienda")))}
    optional: true
- waitForAnimationToEnd:
    timeout: 2000
- runFlow:
    when:
      notVisible:
        text: ${q(rx(t("ajustes.velocidad")))}
    commands:
      - tapOn:
          point: ${q(IPAD ? "97%,30%" : "96%,33%")}
- extendedWaitUntil:
    visible:
      text: ${q(rx(t("ajustes.velocidad")))}
    timeout: 8000
- waitForAnimationToEnd:
    timeout: 2000
- takeScreenshot: ${shots}/25-trastienda
- tapOn:
    text: ${q(rx(t("ajustes.tema.noche")))}
    optional: true
- waitForAnimationToEnd:
    timeout: 2500
- takeScreenshot: ${shots}/26-trastienda-noche
- tapOn:
    text: ${q(rx(t("ajustes.tema.papel")))}
    optional: true
- scrollUntilVisible:
    element:
      text: ${q(rx(t("pro.comprar")) + "|" + rx(t("pro.gestionar")))}
    direction: DOWN
    timeout: 20000
- waitForAnimationToEnd:
    timeout: 2000
- takeScreenshot: ${shots}/27-trastienda-cuerda
- tapOn:
    text: ${q(rx(t("pro.comprar")))}
- extendedWaitUntil:
    visible:
      text: ${q(rx(t("pro.elige")))}
    timeout: 12000
- waitForAnimationToEnd:
    timeout: 3000
- takeScreenshot: ${shots}/28-paywall
# Relanzar en vez de volver atrás: la cinta se conserva y no hay flecha que acertar.
- launchApp:
    stopApp: true
    arguments:
      AppleLanguages: "(${L})"
      AppleLocale: "${L}_${L.toUpperCase()}"
- extendedWaitUntil:
    visible:
      text: "yappy"
    timeout: 30000
- waitForAnimationToEnd:
    timeout: 3000
- tapOn:
    text: ${q(rx(t("aviso.cerrar")))}
    optional: true
- tapOn:
    text: ${q("(?i)" + rx(t("cinta.abrir_imprenta")))}
- waitForAnimationToEnd:
    timeout: 3000
- takeScreenshot: ${shots}/29-imprenta
- launchApp:
    stopApp: true
    arguments:
      AppleLanguages: "(${L})"
      AppleLocale: "${L}_${L.toUpperCase()}"
- extendedWaitUntil:
    visible:
      text: "yappy"
    timeout: 30000
- waitForAnimationToEnd:
    timeout: 3000
- tapOn:
    text: ${q(rx(t("aviso.cerrar")))}
    optional: true
- tapOn:
    text: ${q("(?i)" + rx(titulo))}
- waitForAnimationToEnd:
    timeout: 2000
- tapOn:
    text: ${q(rx(t("cola.escuchar")))}
- waitForAnimationToEnd:
    timeout: 3000
- takeScreenshot: ${shots}/21-lector-a
- waitForAnimationToEnd:
    timeout: 4000
- takeScreenshot: ${shots}/22-lector-b
- waitForAnimationToEnd:
    timeout: 4000
- takeScreenshot: ${shots}/23-lector-c
- takeScreenshot: ${shots}/24-cinta-sonando
`;
  writeFileSync(join(dir, "paseo.yaml"), paseo);
  writeFileSync(join(dir, "anadir.yaml"), anadir);
  // El título del texto que se toca para abrir el lector lo pone capturar.sh
  writeFileSync(join(dir, "resto.template.yaml"), resto("__TITULO__"));
}
console.log("flujos generados para", Object.keys(D).length, "idiomas");
