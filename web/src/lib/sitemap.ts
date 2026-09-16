/**
 * El sitemap se escribe a mano porque @astrojs/sitemap no sabe emitir el
 * bloque `xhtml:link` de alternativas por URL, y con 31 idiomas eso es
 * justamente lo que hay que decirle a Google.
 */
import { absoluta, hreflangDe, POR_DEFECTO } from "../i18n";
import type { Ruta } from "./rutas";

const esc = (s: string): string =>
  s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

const prioridadDe = (r: Ruta): string =>
  r.clase === "pagina" && r.clave === "portada" ? "1.0"
  : r.clase === "pagina" ? "0.8"
  : r.clase === "convertir" || r.clase === "alternativa" ? "0.7"
  : r.clase === "post" ? "0.6"
  : "0.5";

export function urlset(rutas: Ruta[]): string {
  const hoy = new Date().toISOString().slice(0, 10);
  const urls = rutas.map((r) => {
    const lastmod = r.clase === "post" ? r.lastmod.toISOString().slice(0, 10) : hoy;
    const preferida = r.alternativas.find((a) => a.lang === "en")
      ?? r.alternativas.find((a) => a.lang === POR_DEFECTO)
      ?? r.alternativas[0]!;
    const enlaces = [
      ...r.alternativas.map((a) =>
        `    <xhtml:link rel="alternate" hreflang="${hreflangDe(a.lang)}" href="${esc(absoluta(a.ruta))}" />`),
      `    <xhtml:link rel="alternate" hreflang="x-default" href="${esc(absoluta(preferida.ruta))}" />`,
    ].join("\n");
    return `  <url>\n    <loc>${esc(absoluta(r.ruta))}</loc>\n    <lastmod>${lastmod}</lastmod>\n    <priority>${prioridadDe(r)}</priority>\n${enlaces}\n  </url>`;
  }).join("\n");

  return `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">\n${urls}\n</urlset>\n`;
}

export const xml = (cuerpo: string): Response =>
  new Response(cuerpo, { headers: { "content-type": "application/xml; charset=utf-8" } });
