import type { APIRoute } from "astro";
import { todasLasRutas } from "../lib/rutas";
import { urlset, xml } from "../lib/sitemap";
export const GET: APIRoute = async () =>
  xml(urlset((await todasLasRutas()).filter((r) => r.clase === "convertir" || r.clase === "alternativa")));
