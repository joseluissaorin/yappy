import type { APIRoute } from "astro";
import { absoluta } from "../i18n";

export const GET: APIRoute = () =>
  new Response(
    `User-agent: *\nAllow: /\n\nSitemap: ${absoluta("/sitemap.xml")}\n`,
    { headers: { "content-type": "text/plain; charset=utf-8" } },
  );
