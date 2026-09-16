/**
 * El mapa de todas las rutas del sitio, en todos los idiomas. Lo usan el
 * router `[...ruta].astro` (que solo elige plantilla), el sitemap (que emite
 * el hreflang) y el RSS.
 *
 * Seis clases de ruta:
 *  - «pagina»: las fijas (portada, uso, guionizador, descargas, precios,
 *    voces, preguntas, privacidad, accesibilidad, cuentos, blog).
 *  - «convertir»: un formato de entrada por página (PDF, EPUB, Word...).
 *    Es el racimo de más intención: quien busca «pdf a audio» quiere esto.
 *  - «idioma»: una página por lengua que habla el loro, con su muestra.
 *  - «alternativa»: Yappy frente a cada rival, con la tabla de datos duros.
 *  - «post»: una entrada del blog.
 *  - «cuento»: un texto de dominio público para probar la app.
 *
 * Los racimos programáticos declaran EN QUÉ IDIOMAS existen. Empezar por
 * pocos y crecer es mejor que publicar 900 páginas traducidas a máquina.
 */
import { getCollection, type CollectionEntry } from "astro:content";
import { IDIOMAS, POR_DEFECTO, prefijo, slug, type Idioma } from "../i18n";
import { IDIOMAS as LENGUAS } from "./datos";

export type ClavePagina =
  | "portada" | "uso" | "guionizador" | "descargas" | "precios"
  | "voces" | "preguntas" | "privacidad" | "accesibilidad" | "cuentos" | "blog"
  | "convertir";

export const PAGINAS: ClavePagina[] = [
  "portada", "uso", "guionizador", "descargas", "precios",
  "voces", "preguntas", "privacidad", "accesibilidad", "cuentos", "blog",
  "convertir",
];

/** Los formatos que entran por la puerta. Uno por página. */
export const FORMATOS = ["pdf", "epub", "word", "articulos", "videos", "notas"] as const;
export type Formato = (typeof FORMATOS)[number];

/** Los rivales con ficha en src/content/rivales/. */
export const RIVALES = ["speechify", "naturalreader", "elevenreader", "voice-dream", "apple-vozalta"] as const;

/** En qué idiomas vive cada racimo. Crecer = añadir códigos aquí. */
export const ALCANCE = {
  convertir: IDIOMAS,                 // los 31: «pdf» es cabeza de búsqueda en toda tienda
  idiomas: ["es", "en"],              // 31 páginas × idioma: se crece despacio
  alternativas: ["es", "en"],         // la intención comparativa es sobre todo anglo e hispana
  blog: ["es", "en"],
  cuentos: ["es", "en"],
} as const;

const enAlcance = (racimo: keyof typeof ALCANCE, lang: Idioma): boolean =>
  (ALCANCE[racimo] as readonly string[]).includes(lang);

export interface Alternativa { lang: Idioma; ruta: string }

interface Base {
  lang: Idioma;
  /** Ruta absoluta desde la raíz, con barra final. */
  ruta: string;
  /** La misma página en los demás idiomas (incluida esta), para el hreflang. */
  alternativas: Alternativa[];
}

export interface RutaPagina extends Base { clase: "pagina"; clave: ClavePagina }
export interface RutaConvertir extends Base { clase: "convertir"; formato: Formato }
export interface RutaIdioma extends Base { clase: "idioma"; lengua: string }
export interface RutaRival extends Base { clase: "alternativa"; rival: string }
export interface RutaPost extends Base {
  clase: "post";
  entrada: CollectionEntry<"blog">;
  lastmod: Date;
}
export interface RutaCuento extends Base { clase: "cuento"; entrada: CollectionEntry<"cuentos"> }

export type Ruta = RutaPagina | RutaConvertir | RutaIdioma | RutaRival | RutaPost | RutaCuento;

// ── Constructores de ruta ──────────────────────────────────────────────

export function rutaPagina(lang: Idioma, clave: ClavePagina): string {
  const p = prefijo(lang);
  return clave === "portada" ? `${p}/` : `${p}/${slug(lang, clave)}/`;
}
export const rutaConvertir = (lang: Idioma, f: Formato): string =>
  `${prefijo(lang)}/${slug(lang, "convertir")}/${slug(lang, `formato_${f}`)}/`;
export const rutaIdioma = (lang: Idioma, lengua: string): string =>
  `${prefijo(lang)}/${slug(lang, "idiomas")}/${lengua}/`;
export const rutaRival = (lang: Idioma, rival: string): string =>
  `${prefijo(lang)}/${slug(lang, "alternativas")}/${rival}/`;
export const rutaPost = (lang: Idioma, s: string): string =>
  `${prefijo(lang)}/${slug(lang, "blog")}/${s}/`;
export const rutaCuento = (lang: Idioma, s: string): string =>
  `${prefijo(lang)}/${slug(lang, "cuentos")}/${s}/`;
export const rutaRss = (lang: Idioma): string =>
  `${prefijo(lang)}/${slug(lang, "blog")}/rss.xml`;

/**
 * Parámetro `[...ruta]` a partir de la ruta: sin barras en los extremos, y
 * `undefined` para la raíz (si no, `/` no sale como index.html).
 */
export function aParam(ruta: string): string | undefined {
  const limpia = ruta.replace(/^\/+/, "").replace(/\/+$/, "");
  return limpia === "" ? undefined : limpia;
}

// ── El censo ───────────────────────────────────────────────────────────

export async function postsPublicados(): Promise<CollectionEntry<"blog">[]> {
  const posts = await getCollection("blog", (e) => import.meta.env.DEV || !e.data.borrador);
  return posts.sort((a, b) => b.data.fecha.getTime() - a.data.fecha.getTime());
}

const grupoDe = (e: CollectionEntry<"blog">): string => e.data.traduccionDe ?? e.data.slug;

export async function todasLasRutas(): Promise<Ruta[]> {
  const rutas: Ruta[] = [];

  // Las páginas fijas, con su slug traducido en cada idioma.
  for (const clave of PAGINAS) {
    const idiomas = clave === "blog" ? ALCANCE.blog : clave === "cuentos" ? ALCANCE.cuentos : IDIOMAS;
    const alts = (idiomas as readonly string[]).map((l) => ({ lang: l, ruta: rutaPagina(l, clave) }));
    for (const lang of idiomas as readonly string[]) {
      rutas.push({ clase: "pagina", clave, lang, ruta: rutaPagina(lang, clave), alternativas: alts });
    }
  }

  // Los formatos: una página por puerta de entrada.
  for (const formato of FORMATOS) {
    const idiomas = IDIOMAS.filter((l) => enAlcance("convertir", l));
    const alts = idiomas.map((l) => ({ lang: l, ruta: rutaConvertir(l, formato) }));
    for (const lang of idiomas) {
      rutas.push({ clase: "convertir", formato, lang, ruta: rutaConvertir(lang, formato), alternativas: alts });
    }
  }

  // Las lenguas que habla el loro.
  for (const lengua of LENGUAS) {
    const idiomas = IDIOMAS.filter((l) => enAlcance("idiomas", l));
    const alts = idiomas.map((l) => ({ lang: l, ruta: rutaIdioma(l, lengua.codigo) }));
    for (const lang of idiomas) {
      rutas.push({ clase: "idioma", lengua: lengua.codigo, lang, ruta: rutaIdioma(lang, lengua.codigo), alternativas: alts });
    }
  }

  // Los rivales.
  for (const rival of RIVALES) {
    const idiomas = IDIOMAS.filter((l) => enAlcance("alternativas", l));
    const alts = idiomas.map((l) => ({ lang: l, ruta: rutaRival(l, rival) }));
    for (const lang of idiomas) {
      rutas.push({ clase: "alternativa", rival, lang, ruta: rutaRival(lang, rival), alternativas: alts });
    }
  }

  // El blog, agrupando cada artículo con sus traducciones.
  const posts = await postsPublicados();
  const grupos = new Map<string, CollectionEntry<"blog">[]>();
  for (const p of posts) grupos.set(grupoDe(p), [...(grupos.get(grupoDe(p)) ?? []), p]);
  for (const post of posts) {
    const hermanos = grupos.get(grupoDe(post)) ?? [post];
    const alts = IDIOMAS.flatMap((l) => {
      const v = hermanos.find((h) => h.data.lang === l);
      return v == null ? [] : [{ lang: l, ruta: rutaPost(l, v.data.slug) }];
    });
    rutas.push({
      clase: "post", lang: post.data.lang, ruta: rutaPost(post.data.lang, post.data.slug),
      entrada: post, alternativas: alts, lastmod: post.data.actualizado ?? post.data.fecha,
    });
  }

  // Los cuentos: no son traducciones unos de otros, así que cada uno va solo.
  const cuentos = await getCollection("cuentos");
  for (const c of cuentos) {
    const ruta = rutaCuento(c.data.lang, c.data.slug);
    rutas.push({ clase: "cuento", lang: c.data.lang, ruta, entrada: c, alternativas: [{ lang: c.data.lang, ruta }] });
  }

  return rutas;
}

export { POR_DEFECTO };
