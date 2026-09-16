/**
 * Las colecciones:
 *  - blog: artículos en MDX, uno por idioma; `traduccionDe` agrupa versiones.
 *  - rivales: fichas YAML de la competencia, de las que salen las tablas
 *    «Yappy frente a X». Los datos duros viven aquí, fuera de la prosa, para
 *    que un cambio de precio actualice todas las páginas a la vez.
 *  - cuentos: textos de dominio público para probar la app.
 */
import { defineCollection, z } from "astro:content";
import { glob } from "astro/loaders";

const siNoParcial = z.union([z.boolean(), z.literal("parcial")]).nullable();

const blog = defineCollection({
  loader: glob({ pattern: "**/*.mdx", base: "./src/content/blog" }),
  schema: z.object({
    title: z.string(),
    description: z.string(),
    lang: z.string(),
    slug: z.string(),
    /** Racimo del estudio: formatos, privacidad, accesibilidad, oficio, comparativas. */
    racimo: z.string(),
    /** Slug del artículo original del que este es traducción. */
    traduccionDe: z.string().optional(),
    fecha: z.coerce.date(),
    actualizado: z.coerce.date().optional(),
    autor: z.string().default("jose-luis-saorin"),
    /** Audio del artículo leído por el propio Yappy, si lo hay. */
    audio: z.string().optional(),
    imagen: z.string().optional(),
    /** Preguntas del pie; salen también como FAQPage en JSON-LD. */
    faq: z.array(z.object({ q: z.string(), a: z.string() })).optional(),
    borrador: z.boolean().default(false),
  }),
});

const rivales = defineCollection({
  loader: glob({ pattern: "*.yaml", base: "./src/content/rivales" }),
  schema: z.object({
    nombre: z.string(),
    web: z.string().url(),
    tipo: z.enum(["saas", "app", "sistema", "opensource"]),
    /** Lo que se puede hacer sin pagar, dicho con sus palabras. */
    gratis_detalle: z.record(z.string()),
    precio_desde: z.string().nullable(),
    precio_anual: z.string().nullable(),
    /** Sí / no / parcial / null (= sin datos). Cuatro estados, no dos. */
    local: siNoParcial,
    sin_cuenta: siNoParcial,
    sin_conexion: siNoParcial,
    codigo_abierto: siNoParcial,
    tarifa_por_caracter: siNoParcial,
    idiomas: z.string().nullable(),
    formatos: z.string().nullable(),
    audiolibro_capitulos: siNoParcial,
    escritorio: siNoParcial,
    descripcion: z.record(z.string()),
    /** Dónde gana el rival. Si esta lista está vacía, la ficha miente. */
    gana_en: z.record(z.string()),
    verificado: z.boolean(),
    fecha_datos: z.string(),
  }),
});

const cuentos = defineCollection({
  loader: glob({ pattern: "**/*.mdx", base: "./src/content/cuentos" }),
  schema: z.object({
    title: z.string(),
    description: z.string(),
    lang: z.string(),
    slug: z.string(),
    autor: z.string(),
    anio: z.number(),
    minutos: z.number(),
    palabras: z.number(),
  }),
});

export const collections = { blog, rivales, cuentos };
