// EL CARTEL: el motor tipográfico de los titulares (docs/EL-JUGUETE.md §5).
// El título llena el ANCHO y el ALTO de su baldosa con tipografía de madera
// de feria: cada línea es un SVG con preserveAspectRatio="none" cuyo viewBox
// es la anchura NATURAL medida de esa línea; al estirarse a la franja, los
// glifos se deforman en los dos ejes. Sin padding: la palabra es la pieza.

export interface LineaCartel {
  texto: string;
  /// Anchura natural de la línea a cuerpo 100 (el viewBox horizontal).
  vb: number;
  /// Reposo: anchura FORZADA del texto dentro del viewBox (textLength).
  tl?: number;
  /// Reposo: arranque x para centrar las líneas cortas.
  x?: number;
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

export interface LimitesCartel {
  /// Compresión mínima y estirón máximo tolerados por línea.
  min: number;
  max: number;
  /// Alto mínimo LEGIBLE de cada franja de línea, en píxeles.
  altoMinLinea?: number;
  /// Tope de líneas (las formas puntiagudas caben menos renglones).
  maxLineas?: number;
}

/// El cartel LIBRE de la pieza grande: deformación de feria casi sin freno.
export const LIMITES_GRITO: LimitesCartel = { min: 0.5, max: 6, altoMinLinea: 22 };
/// El cartel ACOTADO del reposo (la válvula dura de la regla nueve):
/// se deforma, llena y SE LEE; antes de aplastar, se sueltan palabras.
export const LIMITES_REPOSO: LimitesCartel = { min: 0.82, max: 1.35, altoMinLinea: 15, maxLineas: 4 };

function componer(
  palabras: string[],
  ancho: number,
  alto: number,
  lim: LimitesCartel,
): { lineas: string[]; nota: number; valido: boolean } {
  const topeLineas = Math.max(
    1,
    Math.min(lim.maxLineas ?? 5, palabras.length, Math.floor(alto / (lim.altoMinLinea ?? 19))),
  );
  let mejor: string[] = [palabras.join(" ")];
  let mejorNota = Infinity;
  let mejorValido = false;
  for (let n = 1; n <= topeLineas; n++) {
    const lineas = partir(palabras, n);
    const franja = alto / lineas.length;
    let nota = 0;
    let valido = true;
    for (const l of lineas) {
      const natural = medir(l) / VB_ALTO;
      const estira = ancho / franja / natural;
      if (estira < lim.min || estira > lim.max) valido = false;
      nota += estira < 1 ? 2.2 * Math.abs(Math.log(estira)) : Math.abs(Math.log(estira));
      if (estira < 0.5) nota += 6;
    }
    // Un reparto VÁLIDO gana siempre a uno inválido; a igualdad, la nota.
    if ((valido && !mejorValido) || (valido === mejorValido && nota < mejorNota - 1e-6)) {
      mejorNota = nota;
      mejor = lineas;
      mejorValido = valido;
    }
  }
  return { lineas: mejor, nota: mejorNota, valido: mejorValido };
}

/// EL CARTEL DE REPOSO, repensado (la legibilidad manda): cuerpo de línea
/// FIJO (nada de encoger con la celda), justificación suave por línea
/// (0.9 a 1.15) y recorte limpio con «…» cuando no cabe. Estable ante
/// cualquier reorganización: la pieza cambia de tamaño y el texto solo
/// gana o pierde renglones, jamás legibilidad.
export interface CartelReposo {
  lineas: LineaCartel[];
  /// Alto de cada renglón en píxeles (fijo).
  altoLinea: number;
}
export function cartelReposo(
  titulo: string,
  ancho: number,
  alto: number,
  idioma?: string,
  maxLineas = 3,
  altoLinea = 19,
): CartelReposo {
  const limpio = titulo.trim();
  if (!limpio || ancho <= 0 || alto <= 0) return { lineas: [], altoLinea };
  const mayus = limpio.toLocaleUpperCase(idioma || undefined);
  let palabras = mayus.split(/\s+/).filter(Boolean);
  if (palabras.length === 1 && palabras[0].length > 13) {
    palabras = trocear(palabras[0], Math.min(3, Math.ceil(palabras[0].length / 11)));
  }
  // EL CUERPO SE NEGOCIA POR PIEZA: antes de recortar UNA sola palabra,
  // el cuerpo baja punto a punto (hasta 15). Recortar es la última
  // defensa, no la primera.
  const capacidadCon = (c: number) => ((ancho * VB_ALTO) / c) * (1 / 0.9);
  let cuerpo = altoLinea;
  let lineas: string[] = [];
  let compuesto = false;
  for (let c = altoLinea; c >= 13; c -= 2) {
    const nc = Math.max(1, Math.min(Math.max(maxLineas, Math.min(5, Math.floor(alto / c))), Math.floor(alto / c), palabras.length));
    const cap = capacidadCon(c);
    const prueba = partir(palabras, nc);
    if (prueba.every((l) => medir(l) <= cap)) {
      cuerpo = c;
      lineas = prueba;
      compuesto = true;
      break;
    }
  }
  if (!compuesto) {
    cuerpo = 13;
    const nc = Math.max(1, Math.min(Math.min(5, Math.floor(alto / cuerpo)), palabras.length));
    const cap = capacidadCon(cuerpo);
    lineas = partir(palabras, nc);
    for (let corte = palabras.length - 1; corte >= 1; corte--) {
      const menos = [...palabras.slice(0, corte)];
      menos[menos.length - 1] = `${menos[menos.length - 1]}…`;
      const prueba = partir(menos, Math.min(nc, menos.length));
      if (prueba.every((l) => medir(l) <= cap)) {
        lineas = prueba;
        break;
      }
      if (corte === 1) lineas = prueba;
    }
  }
  const altoLineaFinal = cuerpo;
  // La caja NO se deforma (viewBox con la proporción exacta del hueco);
  // la única deformación es el textLength, acotado: comprimir hasta 0.9
  // (garantizado por el recorte) y estirar hasta 1.15; las líneas cortas
  // se centran en vez de estirarse de más.
  const objetivo = (ancho * VB_ALTO) / altoLineaFinal;
  return {
    lineas: lineas.map((texto) => {
      const natural = medir(texto);
      const tl = Math.min(objetivo, natural * 1.15);
      return { texto, vb: objetivo, tl, x: Math.max(0, (objetivo - tl) / 2) };
    }),
    altoLinea: altoLineaFinal,
  };
}

/// El cartel de una baldosa: tipografía que SE ESTIRA Y SE DEFORMA para
/// llenar el ancho y el alto de su ventana, sin padding. Con límites
/// acotados (reposo), si ningún reparto respeta la legibilidad se van
/// soltando palabras («…») antes que aplastar: llenar Y leerse.
export function cartel(
  titulo: string,
  ancho: number,
  alto: number,
  idioma?: string,
  limites: LimitesCartel = LIMITES_GRITO,
): LineaCartel[] {
  const limpio = titulo.trim();
  if (!limpio || ancho <= 0 || alto <= 0) return [];
  const mayus = limpio.toLocaleUpperCase(idioma || undefined);
  let palabras = mayus.split(/\s+/).filter(Boolean);
  if (palabras.length === 1 && palabras[0].length > 12) {
    palabras = trocear(palabras[0], Math.min(3, Math.ceil(palabras[0].length / 10)));
  }
  let intento = componer(palabras, ancho, alto, limites);
  if (!intento.valido && palabras.length > 1) {
    // La válvula dura: soltar palabras hasta que el cartel respete los
    // límites de legibilidad (el título entero espera en la pieza grande).
    for (let corte = palabras.length - 1; corte >= 1; corte--) {
      const conElipsis = [...palabras.slice(0, corte)];
      conElipsis[conElipsis.length - 1] = `${conElipsis[conElipsis.length - 1]}…`;
      const prueba = componer(conElipsis, ancho, alto, limites);
      if (prueba.valido) {
        intento = prueba;
        break;
      }
    }
  }
  return intento.lineas.map((texto) => ({ texto, vb: medir(texto) }));
}
