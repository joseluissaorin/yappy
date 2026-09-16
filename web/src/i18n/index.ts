/**
 * Motor de idiomas de la web de Yappy.
 *
 * Cada idioma es un `src/i18n/{codigo}.json` con la misma forma que `es.json`.
 * Se descubren solos con `import.meta.glob`, así que añadir un idioma es
 * añadir un fichero (y `astro.config.mjs` lee el mismo directorio). El
 * español vive en la raíz; los otros treinta llevan prefijo. Las claves que
 * falten caen al español, para que una traducción a medias no rompa el build.
 *
 * Los slugs se traducen: `/en/pdf-to-audio/` vale más que `/en/pdf-a-audio/`.
 */
import es from "./es.json";

export type Cadenas = typeof es;
export type Idioma = string;

const modulos = import.meta.glob<{ default: Cadenas }>("./*.json", { eager: true });

export const CADENAS: Record<Idioma, Cadenas> = Object.fromEntries(
  Object.entries(modulos)
    .map(([ruta, mod]) => [ruta.replace(/^\.\//, "").replace(/\.json$/, ""), mod.default] as const)
    .sort(([a], [b]) => (a === "es" ? -1 : b === "es" ? 1 : a.localeCompare(b))),
);

export const POR_DEFECTO: Idioma = "es";
export const IDIOMAS: Idioma[] = Object.keys(CADENAS);

const DERECHA_IZQUIERDA = new Set(["ar", "he", "fa", "ur"]);

export function cadenas(lang: Idioma): Cadenas {
  return CADENAS[lang] ?? CADENAS[POR_DEFECTO]!;
}

export function direccion(lang: Idioma): "ltr" | "rtl" {
  const declarada = (cadenas(lang).meta as { dir?: string }).dir;
  if (declarada === "rtl" || declarada === "ltr") return declarada;
  return DERECHA_IZQUIERDA.has(lang.split("-")[0]!) ? "rtl" : "ltr";
}

function hondo(obj: unknown, ruta: string): unknown {
  return ruta.split(".").reduce<unknown>(
    (acc, k) => (acc != null && typeof acc === "object" ? (acc as Record<string, unknown>)[k] : undefined),
    obj,
  );
}

/**
 * Cadena por ruta con puntos («portada.hero.titular»). Si el idioma no la
 * tiene, se devuelve la española; si tampoco, la propia ruta, que canta en
 * pantalla y se ve enseguida.
 */
export function t(lang: Idioma, ruta: string, vars?: Record<string, string | number>): string {
  let v = hondo(cadenas(lang), ruta);
  if (v == null) v = hondo(CADENAS[POR_DEFECTO], ruta);
  if (typeof v !== "string") return ruta;
  if (vars == null) return v;
  return v.replace(/\{(\w+)\}/g, (m, k: string) => (vars[k] != null ? String(vars[k]) : m));
}

/** Un objeto o una lista entera, con la misma caída al español. */
export function tv<T = unknown>(lang: Idioma, ruta: string): T {
  const v = hondo(cadenas(lang), ruta);
  if (v != null) return v as T;
  return hondo(CADENAS[POR_DEFECTO], ruta) as T;
}

/** ¿Existe esta clave en este idioma, sin caer al español? */
export function tiene(lang: Idioma, ruta: string): boolean {
  return hondo(cadenas(lang), ruta) != null;
}

export function prefijo(lang: Idioma): string {
  return lang === POR_DEFECTO ? "" : `/${lang}`;
}

export function slug(lang: Idioma, clave: string): string {
  return t(lang, `slugs.${clave}`);
}

export const SITIO = "https://yappy.joseluissaorin.com";
export const absoluta = (ruta: string): string => `${SITIO}${ruta}`;

export function hreflangDe(lang: Idioma): string {
  return (cadenas(lang).meta as { hreflang?: string }).hreflang ?? lang;
}
export function ogLocaleDe(lang: Idioma): string {
  return (cadenas(lang).meta as { ogLocale?: string }).ogLocale ?? lang;
}
export function nombreDe(lang: Idioma): string {
  return (cadenas(lang).meta as { nombre?: string }).nombre ?? lang;
}
