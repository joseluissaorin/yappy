// LOS CAPÍTULOS DE UN DOCUMENTO: la imprenta trabaja capítulo a capítulo
// (elegir cuáles, uno o varios audiolibros) y para eso hay que entender
// mínimamente dónde empieza cada uno en un PDF, unos apuntes, un Markdown
// o un EPUB. La verdad viene del lector de documentos (paragraph_kinds:
// heading1…heading6 detectados por tipografía o por «#»); aquí solo se
// decide el NIVEL que parte el libro y se reconstruye el Markdown que la
// imprenta entiende (los títulos con sus «#», para que el .yappy lleve sus
// capítulos).

export interface Capitulo {
  /// Índice del capítulo en el documento (0-based).
  n: number;
  titulo: string;
  /// Rango de párrafos [desde, hasta) del documento.
  desde: number;
  hasta: number;
  chars: number;
}

const NIVELES = ["heading1", "heading2", "heading3"] as const;

/// El nivel de título que parte el documento: el más alto (h1, si no h2,
/// si no h3) que dé al menos DOS capítulos y no lo pulverice (un título
/// por cada pocos párrafos no es un capítulo, es una lista de apartados).
export function nivelDeCapitulo(kinds: string[]): string | null {
  const total = kinds.length;
  for (const nivel of NIVELES) {
    const cuantos = kinds.filter((k) => k === nivel).length;
    if (cuantos >= 2 && cuantos <= Math.max(4, total / 3)) return nivel;
  }
  return null;
}

/// Parte el documento en capítulos. Sin títulos que valgan, un solo
/// capítulo con el documento entero (y el título del documento).
export function capitulosDe(paragraphs: string[], kinds: string[], tituloDoc: string): Capitulo[] {
  const nivel = nivelDeCapitulo(kinds);
  const chars = (a: number, b: number) => paragraphs.slice(a, b).reduce((n, p) => n + p.length, 0);
  if (!nivel) {
    return [{ n: 0, titulo: tituloDoc, desde: 0, hasta: paragraphs.length, chars: chars(0, paragraphs.length) }];
  }
  const cortes: number[] = [];
  kinds.forEach((k, i) => { if (k === nivel) cortes.push(i); });
  const caps: Capitulo[] = [];
  // Lo anterior al primer título (portada, preámbulo) va con el primer
  // capítulo si es corto; si es largo, es un capítulo propio.
  let tituloPrimero: string | null = null;
  if (cortes[0] > 0) {
    const preambulo = chars(0, cortes[0]);
    if (preambulo > 1500) {
      caps.push({ n: 0, titulo: tituloDoc, desde: 0, hasta: cortes[0], chars: preambulo });
    } else {
      // El primer capítulo absorbe el preámbulo pero conserva SU título.
      tituloPrimero = paragraphs[cortes[0]];
      cortes[0] = 0;
    }
  }
  for (let i = 0; i < cortes.length; i++) {
    const desde = cortes[i];
    const hasta = i + 1 < cortes.length ? cortes[i + 1] : paragraphs.length;
    const crudo = i === 0 && tituloPrimero != null ? tituloPrimero : kinds[desde] === nivel ? paragraphs[desde] : tituloDoc;
    const titulo = crudo.trim() || tituloDoc;
    caps.push({ n: caps.length, titulo, desde, hasta, chars: chars(desde, hasta) });
  }
  return caps;
}

const ALMOHADILLAS: Record<string, string> = {
  heading1: "# ",
  heading2: "## ",
  heading3: "### ",
  heading4: "#### ",
  heading5: "##### ",
  heading6: "###### ",
};

/// El Markdown que entiende la imprenta: cada título con sus «#» (así el
/// guion lo clasifica como título y el audiolibro lleva ese capítulo),
/// los separadores como «---», y el resto tal cual, a un párrafo por bloque.
export function aMarkdown(paragraphs: string[], kinds: string[], desde = 0, hasta = paragraphs.length): string {
  const bloques: string[] = [];
  for (let i = desde; i < hasta; i++) {
    const texto = (paragraphs[i] ?? "").trim();
    const kind = kinds[i] ?? "paragraph";
    if (kind === "hr") {
      bloques.push("---");
      continue;
    }
    if (!texto) continue;
    const prefijo = ALMOHADILLAS[kind];
    // Un título ya con almohadillas (Markdown crudo) no se duplica.
    bloques.push(prefijo && !texto.startsWith("#") ? prefijo + texto.replace(/\n+/g, " ") : texto);
  }
  return bloques.join("\n\n");
}

/// Minutos de escucha estimados (mil caracteres ≈ un minuto).
export function minutosDe(chars: number): number {
  return Math.max(1, Math.round(chars / 1000));
}

/// Un rango de capítulos elegidos, dicho corto: «2–4», «1, 3, 5».
export function rangoDicho(ns: number[]): string {
  const ord = [...ns].sort((a, b) => a - b).map((n) => n + 1);
  if (ord.length === 0) return "";
  const consecutivos = ord.every((v, i) => i === 0 || v === ord[i - 1] + 1);
  if (consecutivos && ord.length > 1) return `${ord[0]}–${ord[ord.length - 1]}`;
  return ord.join(", ");
}
