// La web de Yappy: papel, pegatinas troqueladas y un loro, en 31 idiomas.
//
// Estática entera (Cloudflare Pages, proyecto «yappy»). El español vive en la
// raíz porque ahí llevaba dos años y ahí apuntan las fichas de la App Store;
// los otros treinta cuelgan de su prefijo. Los idiomas se descubren solos:
// añadir un idioma es añadir un fichero a src/i18n/.
import { defineConfig } from "astro/config";
import { readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import mdx from "@astrojs/mdx";
import svelte from "@astrojs/svelte";

const aqui = dirname(fileURLToPath(import.meta.url));
const idiomas = readdirSync(join(aqui, "src/i18n"))
  .filter((f) => f.endsWith(".json"))
  .map((f) => f.replace(/\.json$/, ""))
  .sort();

export default defineConfig({
  site: "https://yappy.joseluissaorin.com",
  output: "static",
  trailingSlash: "always",
  build: { format: "directory", inlineStylesheets: "auto" },
  compressHTML: true,
  integrations: [mdx(), svelte()],
  i18n: {
    defaultLocale: "es",
    locales: idiomas.length ? idiomas : ["es"],
    routing: { prefixDefaultLocale: false, redirectToDefaultLocale: false },
  },
  image: { service: { entrypoint: "astro/assets/services/sharp" } },
  vite: { build: { assetsInlineLimit: 2048 } },
});
