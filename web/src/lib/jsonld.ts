/**
 * Los datos estructurados. Constructores puros que devuelven objetos planos;
 * el layout los serializa. Comparten `@id` para que Article.publisher,
 * SoftwareApplication.author y Person.worksFor se apunten entre sí en vez de
 * repetir la organización tres veces.
 */
import { absoluta, SITIO, type Idioma } from "../i18n";
import { CUERDA, ENLACES, MOTOR } from "./datos";

const CONTEXTO = "https://schema.org";
export const ID_PERSONA = `${SITIO}/#autor`;
export const ID_APP = `${SITIO}/#app`;
export const ID_SITIO = `${SITIO}/#sitio`;

export function persona(): object {
  return {
    "@context": CONTEXTO, "@type": "Person", "@id": ID_PERSONA,
    name: "José Luis Saorín Ferrer", url: ENLACES.autor,
    jobTitle: "Filólogo y programador",
    sameAs: [ENLACES.autor, "https://github.com/joseluissaorin"],
  };
}

export function sitioWeb(lang: Idioma): object {
  return {
    "@context": CONTEXTO, "@type": "WebSite", "@id": ID_SITIO,
    name: "Yappy", url: SITIO, inLanguage: lang,
    publisher: { "@id": ID_PERSONA },
  };
}

/** La app. Con `offers` reales: gratis, y la cuerda en sus tres formas. */
export function aplicacion(lang: Idioma, opciones: { nombre: string; descripcion: string }): object {
  return {
    "@context": CONTEXTO, "@type": "SoftwareApplication", "@id": ID_APP,
    name: opciones.nombre,
    description: opciones.descripcion,
    inLanguage: lang,
    applicationCategory: "UtilitiesApplication",
    applicationSubCategory: "Text to speech",
    operatingSystem: "iOS 17+, iPadOS 17+, macOS 12+, Windows 10+, Linux",
    url: SITIO,
    downloadUrl: ENLACES.appStore,
    installUrl: ENLACES.appStore,
    softwareVersion: "1.0.0",
    license: ENLACES.licencia,
    isAccessibleForFree: true,
    author: { "@id": ID_PERSONA },
    featureList: [
      `${MOTOR.idiomas} languages`, `${MOTOR.voces} voices`,
      "On-device speech synthesis", "PDF, EPUB, Word, articles and video",
      "M4B audiobooks with chapters", "Works offline", "Open source (MIT)",
    ],
    offers: [
      { "@type": "Offer", price: "0", priceCurrency: "EUR", name: "Yappy", category: "free" },
      { "@type": "Offer", price: "3.99", priceCurrency: "EUR", name: "Yappy Parlanchín", category: "subscription" },
      { "@type": "Offer", price: "29.99", priceCurrency: "EUR", name: "Yappy Parlanchín", category: "subscription" },
      { "@type": "Offer", price: "59.99", priceCurrency: "EUR", name: "Yappy Parlanchín", category: "one-time" },
    ],
  };
}

const sinHtml = (s: string): string =>
  s.replace(/<[^>]+>/g, "").replace(/&nbsp;/g, " ").replace(/&[a-z]+;/g, " ").replace(/\s+/g, " ").trim();

export function preguntas(lista: { q: string; a: string }[]): object {
  return {
    "@context": CONTEXTO, "@type": "FAQPage",
    mainEntity: lista.map((p) => ({
      "@type": "Question", name: sinHtml(p.q),
      acceptedAnswer: { "@type": "Answer", text: sinHtml(p.a) },
    })),
  };
}

export function migas(items: { nombre: string; ruta: string }[]): object {
  return {
    "@context": CONTEXTO, "@type": "BreadcrumbList",
    itemListElement: items.map((it, i) => ({
      "@type": "ListItem", position: i + 1, name: it.nombre, item: absoluta(it.ruta),
    })),
  };
}

export function articulo(a: {
  lang: Idioma; title: string; description: string; ruta: string;
  publicado: Date; actualizado?: Date; autor: string; imagen?: string;
}): object {
  const url = absoluta(a.ruta);
  return {
    "@context": CONTEXTO, "@type": "BlogPosting", "@id": url,
    mainEntityOfPage: { "@type": "WebPage", "@id": url },
    url, headline: a.title, description: a.description, inLanguage: a.lang,
    datePublished: a.publicado.toISOString(),
    dateModified: (a.actualizado ?? a.publicado).toISOString(),
    ...(a.imagen != null ? { image: absoluta(a.imagen) } : {}),
    author: { "@id": ID_PERSONA },
    publisher: { "@id": ID_PERSONA },
    about: { "@id": ID_APP },
  };
}

/** Un cuento de dominio público. */
export function relato(c: {
  lang: Idioma; title: string; autor: string; anio: number; ruta: string; palabras: number;
}): object {
  return {
    "@context": CONTEXTO, "@type": "ShortStory", "@id": absoluta(c.ruta),
    name: c.title, inLanguage: c.lang, url: absoluta(c.ruta),
    author: { "@type": "Person", name: c.autor },
    datePublished: String(c.anio),
    wordCount: c.palabras,
    isAccessibleForFree: true,
    license: "https://creativecommons.org/publicdomain/mark/1.0/",
  };
}

/** Una guía «cómo se hace»: las páginas de formato la usan. */
export function comoSeHace(h: {
  lang: Idioma; nombre: string; descripcion: string; pasos: { nombre: string; texto: string }[];
}): object {
  return {
    "@context": CONTEXTO, "@type": "HowTo",
    name: h.nombre, description: h.descripcion, inLanguage: h.lang,
    totalTime: "PT2M",
    estimatedCost: { "@type": "MonetaryAmount", currency: "EUR", value: "0" },
    tool: [{ "@type": "HowToTool", name: "Yappy" }],
    step: h.pasos.map((p, i) => ({
      "@type": "HowToStep", position: i + 1, name: p.nombre, text: p.texto,
    })),
  };
}

export { CUERDA };
