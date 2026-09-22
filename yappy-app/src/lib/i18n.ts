// La i18n de la casa, tercera vida: TODOS los idiomas que Yappy habla
// tienen interfaz (31 diccionarios en $lib/i18n/, mismas claves que es.ts).
// El idioma sigue al del sistema, con override manual en la trastienda.
// El español es primera lengua del proyecto; el inglés, la red de seguridad.
import { derived, writable } from "svelte/store";
import { platformName } from "$lib/platform";

import es from "./i18n/es";
import en from "./i18n/en";
import fr from "./i18n/fr";
import de from "./i18n/de";
import it from "./i18n/it";
import pt from "./i18n/pt";
import nl from "./i18n/nl";
import pl from "./i18n/pl";
import ro from "./i18n/ro";
import sv from "./i18n/sv";
import da from "./i18n/da";
import fi from "./i18n/fi";
import et from "./i18n/et";
import lt from "./i18n/lt";
import lv from "./i18n/lv";
import hr from "./i18n/hr";
import sl from "./i18n/sl";
import sk from "./i18n/sk";
import cs from "./i18n/cs";
import hu from "./i18n/hu";
import el from "./i18n/el";
import bg from "./i18n/bg";
import uk from "./i18n/uk";
import ru from "./i18n/ru";
import tr from "./i18n/tr";
import ar from "./i18n/ar";
import hi from "./i18n/hi";
import id from "./i18n/id";
import vi from "./i18n/vi";
import ko from "./i18n/ko";
import ja from "./i18n/ja";

export const IDIOMAS_UI = [
  "es", "en", "fr", "de", "it", "pt", "nl", "pl", "ro", "sv", "da", "fi",
  "et", "lt", "lv", "hr", "sl", "sk", "cs", "hu", "el", "bg", "uk", "ru",
  "tr", "ar", "hi", "id", "vi", "ko", "ja",
] as const;
export type IdiomaUI = (typeof IDIOMAS_UI)[number];

const DICCIONARIOS: Record<IdiomaUI, Record<string, string>> = {
  es, en, fr, de, it, pt, nl, pl, ro, sv, da, fi, et, lt, lv, hr, sl, sk,
  cs, hu, el, bg, uk, ru, tr, ar, hi, id, vi, ko, ja,
};

const CLAVE_PREFERENCIA = "yappy.idioma_ui";

function preferenciaGuardada(): IdiomaUI | "auto" {
  try {
    const v = localStorage.getItem(CLAVE_PREFERENCIA);
    if (v && (v === "auto" || (IDIOMAS_UI as readonly string[]).includes(v))) {
      return v as IdiomaUI | "auto";
    }
  } catch {}
  return "auto";
}

/// Resuelve un locale del sistema («es-ES», «pt_BR», «zh-Hant»…) al idioma
/// de interfaz soportado más cercano. Inglés como red de seguridad.
export function resolverIdioma(locale: string | null | undefined): IdiomaUI {
  const l = (locale ?? "").toLowerCase().replace("_", "-");
  const base = l.split("-")[0];
  if ((IDIOMAS_UI as readonly string[]).includes(base)) return base as IdiomaUI;
  // Noruego cae al danés (mutuamente legibles); gallego/catalán al español.
  if (base === "nb" || base === "nn" || base === "no") return "da";
  if (base === "gl" || base === "ca" || base === "eu") return "es";
  return "en";
}

export const idiomaUI = writable<IdiomaUI>("en");

let ultimoLocale: string | null = null;

export function fijarIdiomaDesdeLocale(locale: string | null | undefined) {
  ultimoLocale = locale ?? null;
  const pref = preferenciaGuardada();
  idiomaUI.set(pref === "auto" ? resolverIdioma(locale) : pref);
}

/// El override manual de la trastienda: «auto» vuelve a seguir al sistema.
export function fijarPreferenciaIdioma(codigo: IdiomaUI | "auto") {
  try {
    localStorage.setItem(CLAVE_PREFERENCIA, codigo);
  } catch {}
  idiomaUI.set(codigo === "auto" ? resolverIdioma(ultimoLocale) : codigo);
}

export function preferenciaIdioma(): IdiomaUI | "auto" {
  return preferenciaGuardada();
}

/// El nombre de cada idioma, en su propio idioma (para el selector).
export const NOMBRE_IDIOMA: Record<IdiomaUI, string> = {
  es: "Español", en: "English", fr: "Français", de: "Deutsch", it: "Italiano",
  pt: "Português", nl: "Nederlands", pl: "Polski", ro: "Română", sv: "Svenska",
  da: "Dansk", fi: "Suomi", et: "Eesti", lt: "Lietuvių", lv: "Latviešu",
  hr: "Hrvatski", sl: "Slovenščina", sk: "Slovenčina", cs: "Čeština",
  hu: "Magyar", el: "Ελληνικά", bg: "Български", uk: "Українська",
  ru: "Русский", tr: "Türkçe", ar: "العربية", hi: "हिन्दी",
  id: "Bahasa Indonesia", vi: "Tiếng Việt", ko: "한국어", ja: "日本語",
};

/// Una frase en OTRO idioma (los sellos de idioma del paseo: el loro dice
/// «hola» en la lengua que tocas). Cae a inglés y a español.
export function frase(idioma: IdiomaUI, clave: string): string {
  return DICCIONARIOS[idioma]?.[clave] ?? DICCIONARIOS.en[clave] ?? DICCIONARIOS.es[clave] ?? clave;
}

/// En Android, las marcas del otro lado no valen: el navegador es Chrome,
/// la cuenta es de Google y el teléfono no es un iPhone. Los diccionarios
/// se escribieron para el iPhone (31 idiomas); esta adaptación es la misma
/// en todos porque los nombres de marca no se traducen.
const MARCAS_ANDROID: [RegExp, string][] = [
  [/Safari/g, "Chrome"],
  [/\bApple\b/g, "Google"],
  [/\biPhone\b/g, "Android"],
];
export function adaptarMarcas(texto: string, plataforma: string): string {
  if (plataforma !== "android") return texto;
  let s = texto;
  for (const [re, con] of MARCAS_ANDROID) s = s.replace(re, con);
  return s;
}

/// t("clave") reactivo: $t en los componentes. Cae a inglés y luego a
/// español antes de rendirse a la clave cruda.
export const t = derived([idiomaUI, platformName], ([$l, $p]) => (clave: string): string => {
  return adaptarMarcas(
    DICCIONARIOS[$l][clave] ?? DICCIONARIOS.en[clave] ?? DICCIONARIOS.es[clave] ?? clave,
    $p,
  );
});
