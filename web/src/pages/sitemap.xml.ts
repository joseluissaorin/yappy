/** El índice: cuatro sitemaps, uno por naturaleza de página. */
import type { APIRoute } from "astro";
import { absoluta } from "../i18n";
import { xml } from "../lib/sitemap";

export const GET: APIRoute = () => {
  const hoy = new Date().toISOString().slice(0, 10);
  const hojas = ["sitemap-paginas.xml", "sitemap-formatos.xml", "sitemap-idiomas.xml", "sitemap-blog.xml"];
  const cuerpo = hojas
    .map((h) => `  <sitemap><loc>${absoluta("/" + h)}</loc><lastmod>${hoy}</lastmod></sitemap>`)
    .join("\n");
  return xml(`<?xml version="1.0" encoding="UTF-8"?>\n<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${cuerpo}\n</sitemapindex>\n`);
};
