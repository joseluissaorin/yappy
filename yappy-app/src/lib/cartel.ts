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
