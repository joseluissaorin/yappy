// EL CARTEL: el motor tipográfico de los titulares (docs/EL-JUGUETE.md §5).
// El título llena el ANCHO y el ALTO de su baldosa con tipografía de madera
// de feria: cada línea es un SVG con preserveAspectRatio="none" cuyo viewBox
// es la anchura NATURAL medida de esa línea; al estirarse a la franja, los
// glifos se deforman en los dos ejes. Sin padding: la palabra es la pieza.

export interface LineaCartel {
  texto: string;
  /// Anchura natural de la línea a cuerpo 100 (el viewBox horizontal).
  vb: number;
}

/// Alto del viewBox de cada línea a cuerpo 100: hueco arriba para las
/// tildes de las mayúsculas (Á, É…) y abajo para colas (Q, Ç).
export const VB_ALTO = 104;
/// Línea de base dentro del viewBox.
export const VB_BASE = 84;

let ctx: CanvasRenderingContext2D | null = null;
let fuente = "";
const cacheMedidas = new Map<string, number>();

function medidor(): CanvasRenderingContext2D | null {
  if (typeof document === "undefined") return null;
  if (!ctx) {
    ctx = document.createElement("canvas").getContext("2d");
    if (ctx) {
      const familia = getComputedStyle(document.body).fontFamily || "system-ui";
      fuente = `800 100px ${familia}`;
      ctx.font = fuente;
    }
  }
  return ctx;
}

/// Anchura natural de un texto a cuerpo 100, con caché.
function medir(texto: string): number {
  const guardada = cacheMedidas.get(texto);
  if (guardada !== undefined) return guardada;
  const c = medidor();
  const w = c ? c.measureText(texto).width : texto.length * 58;
  cacheMedidas.set(texto, Math.max(30, w));
  return cacheMedidas.get(texto)!;
}

/// Partición balanceada en n líneas por anchura MEDIDA (no por letras).
function partir(palabras: string[], n: number): string[] {
  if (n <= 1) return [palabras.join(" ")];
  const total = medir(palabras.join(" "));
  const objetivo = total / n;
  const lineas: string[] = [];
  let linea = "";
  for (const p of palabras) {
    const candidata = linea ? `${linea} ${p}` : p;
    if (linea && medir(candidata) > objetivo * 1.06 && lineas.length < n - 1) {
      lineas.push(linea);
      linea = p;
    } else {
      linea = candidata;
    }
  }
  if (linea) lineas.push(linea);
  return lineas;
}

/// Trocea una palabra larguísima (URLs, palabras alemanas) en n pedazos.
function trocear(palabra: string, n: number): string[] {
  const paso = Math.ceil(palabra.length / n);
  const trozos: string[] = [];
  for (let i = 0; i < palabra.length; i += paso) trozos.push(palabra.slice(i, i + paso));
  return trozos;
}

/// El cartel de una baldosa: elige el número de líneas (1 a 4) que menos
/// deforma en conjunto (la válvula de legibilidad: mejor otra línea que
/// aplastar de más) y devuelve cada línea con su anchura natural.
export function cartel(titulo: string, ancho: number, alto: number, idioma?: string): LineaCartel[] {
  const limpio = titulo.trim();
  if (!limpio || ancho <= 0 || alto <= 0) return [];
  const mayus = limpio.toLocaleUpperCase(idioma || undefined);
  let palabras = mayus.split(/\s+/).filter(Boolean);
  if (palabras.length === 1 && palabras[0].length > 12) {
    palabras = trocear(palabras[0], Math.min(3, Math.ceil(palabras[0].length / 10)));
  }
  // La válvula: mejor OTRA línea que aplastar de más (la compresión por
  // debajo de la mitad castiga fuerte; el estirón gordo apenas).
  const maxLineas = Math.min(5, palabras.length, Math.max(1, Math.floor(alto / 19)));
  let mejor: string[] = [mayus];
  let mejorNota = Infinity;
  for (let n = 1; n <= maxLineas; n++) {
    const lineas = partir(palabras, n);
    const franja = alto / lineas.length;
    let nota = 0;
    for (const l of lineas) {
      const natural = medir(l) / VB_ALTO;
      const pintada = ancho / franja;
      const estira = pintada / natural;
      nota += estira < 1 ? 2.2 * Math.abs(Math.log(estira)) : Math.abs(Math.log(estira));
      if (estira < 0.5) nota += 6;
    }
    if (nota < mejorNota - 1e-6) {
      mejorNota = nota;
      mejor = lineas;
    }
  }
  return mejor.map((texto) => ({ texto, vb: medir(texto) }));
}
