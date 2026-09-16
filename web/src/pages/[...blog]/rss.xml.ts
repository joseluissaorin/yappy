/**
 * Un RSS por idioma, en el slug traducido del blog de cada uno:
 * /blog/rss.xml, /en/blog/rss.xml…
 */
import rss from "@astrojs/rss";
import type { APIRoute, GetStaticPaths } from "astro";
import { postsPublicados, rutaPost, rutaPagina, rutaRss, ALCANCE } from "../../lib/rutas";
import { absoluta, t, type Idioma } from "../../i18n";

export const getStaticPaths = (() =>
  (ALCANCE.blog as readonly string[]).map((lang) => ({
    params: { blog: rutaRss(lang).replace(/^\//, "").replace(/\/rss\.xml$/, "") },
    props: { lang },
  }))) satisfies GetStaticPaths;

export const GET: APIRoute = async ({ props }) => {
  const lang = (props as { lang: Idioma }).lang;
  const posts = (await postsPublicados()).filter((p) => p.data.lang === lang);
  return rss({
    title: t(lang, "blog.title"),
    description: t(lang, "blog.description"),
    site: absoluta(rutaPagina(lang, "blog")),
    items: posts.map((p) => ({
      title: p.data.title,
      description: p.data.description,
      pubDate: p.data.fecha,
      link: absoluta(rutaPost(lang, p.data.slug)),
      categories: [p.data.racimo],
      author: "jl@joseluissaorin.com (José Luis Saorín)",
    })),
    customData: `<language>${lang}</language>`,
  });
};
