// EL MOSAICO, v2: el empaquetador de la cinta, portado del tablero de
// comidas favoritas de Kalorica (food-preferences.tsx) y llevado más allá
// (docs/EL-JUGUETE.md). La idea entera: cada pieza tiene un PESO, las filas
// se empaquetan hasta un peso objetivo, y cuando un peso cambia (se elige,
// suena, llega una nueva) TODO el tablero se recoloca: el FLIP con muelles
// lo hace físico. Como en el original: SIN hueco horizontal (losa continua,
// borde con borde) y con las filas creciendo en ALTO con su peso medio.

export interface PiezaMosaico {
  id: string;
  peso: number;
  /// Longitud del título, para garantizar anchos mínimos legibles.
  letras: number;
}

export interface Baldosa {
  x: number;
  y: number;
  ancho: number;
  alto: number;
  peso: number;
}

const ALTO_BASE = 88;
const ALTO_TECHO = 360;
// El cuaderno: las filas se PISAN un pelín (solape negativo), como
// pegatinas pegadas una tras otra sin dejar ver el papel.
const HUECO_Y = -6;
const ANCHO_MINIMO = 96;
// EL ZIGZAG de los iPhone de una columna: cada pegatina ocupa como una
// columna y media (≈2/3 del ancho), más CUADRADA, y se alternan pegadas
// al margen izquierdo y al derecho; al no chocar de lleno, se montan más.
const ALTO_BASE_ZIG = 148;
const SOLAPE_ZIG = -34;
// El sangrado: las pegatinas se meten un pelín por los márgenes, como
// pegadas sin miramientos (el patrón llena el papel de verdad).
const SANGRADO_ZIG = 7;

function empaquetarZigzag(
  piezas: PiezaMosaico[],
  anchoTablero: number,
  altoUtil: number,
  dos: boolean,
): { baldosas: Map<string, Baldosa>; alto: number; factor: number } {
  const baldosas = new Map<string, Baldosa>();
  // En pantalla grande el zigzag se vuelve TRESBOLILLO: piezas algo más
  // estrechas y cada una avanza solo medio alto, quedando escalonadas
  // lado a lado (columna izquierda y derecha alternadas, como un panal).
  const anchoBase = dos ? 0.58 : 0.7;
  const anchoGorda = dos ? 0.74 : 0.86;
  const altoBase = dos ? 132 : ALTO_BASE_ZIG;
  const avanceDe = (alto: number) => (dos ? alto * 0.54 : alto + SOLAPE_ZIG);
  const colocar = (factor: number) => {
    let y = 0;
    let fondo = 0;
    piezas.forEach((p, i) => {
      let semilla = 0;
      for (const c of p.id) semilla = (semilla * 31 + c.charCodeAt(0)) >>> 0;
      const ruido = 0.92 + ((semilla >> 4) % 17) / 100;
      const gorda = p.peso >= 5;
      const ancho = Math.min(
        anchoTablero,
        Math.max(anchoTablero * (gorda ? anchoGorda : anchoBase), 26 + p.letras * 3.1),
      );
      const crudo = Math.min(ALTO_TECHO, (altoBase + p.peso * 18) * ruido);
      // La ELEGIDA (peso gordo) no se deja aplastar: la ficha desplegada
      // necesita su cuerpo; las demás absorben el apretón (con un suelo
      // digno: las formas puntiagudas chafadas daban pena).
      const suelo = p.peso >= 5 ? crudo * 0.82 : 106;
      const alto = Math.max(suelo, Math.min(crudo * factor, ancho * 1.12));
      const x = i % 2 === 0 ? -SANGRADO_ZIG : anchoTablero - ancho + SANGRADO_ZIG;
      baldosas.set(p.id, { x, y, ancho, alto, peso: p.peso });
      fondo = Math.max(fondo, y + alto);
      y += avanceDe(alto);
    });
    return fondo;
  };

  // La ley del pliego en AMBOS sentidos: si sobra papel las pegatinas
  // crecen (techo CUADRADO: alto ≤ 1.12×ancho); si el cuaderno se pasa
  // del alto útil, se aprietan (suelo de legibilidad), para que el
  // colofón nunca choque con los fijos.
  let factor = 1;
  let altoTotal = colocar(1);
  if (altoUtil > 0 && altoTotal > 0) {
    if (altoTotal < altoUtil * 0.9) {
      factor = Math.min(2.2, (altoUtil * 0.97) / altoTotal);
    } else if (altoTotal > altoUtil) {
      factor = Math.max(0.62, (altoUtil * 0.99) / altoTotal);
    }
    if (factor > 1.01 || factor < 0.99) {
      altoTotal = colocar(factor);
    }
  }
  return { baldosas, alto: altoTotal, factor };
}

/// Empaqueta las piezas en filas por peso y devuelve la baldosa de cada id
/// más el alto total del tablero.
export function empaquetar(
  piezas: PiezaMosaico[],
  anchoTablero: number,
  altoUtil = 0,
  permitirDos = false,
): { baldosas: Map<string, Baldosa>; alto: number; factor: number } {
  const baldosas = new Map<string, Baldosa>();
  if (piezas.length === 0 || anchoTablero <= 0) {
    return { baldosas, alto: 0, factor: 1 };
  }

  // Peso objetivo por fila (Kalorica: 2,5 de base, tope de 4 tarjetas).
  const pesoMedio = piezas.reduce((s, p) => s + p.peso, 0) / piezas.length;
  const PESO_FILA = Math.max(2.5, pesoMedio * 2.5);
  // MODO PÓSTER: con pocas piezas, columnas gordas. Y LA REGLA DE LAS
  // COLUMNAS: dos solo en los iPhone grandes (≥415pt); en los pequeños,
  // UNA (las pegatinas estiradas a lo alto eran un espanto).
  // DOS columnas solo si el usuario las pidió Y la pantalla da de sí.
  const dosColumnas = permitirDos && anchoTablero >= 415;
  // EL CUADERNO para todos: zigzag en una columna, tresbolillo en los
  // grandes (salvo la pieza única, que es un póster a todo lo ancho).
  if (piezas.length >= 2) {
    return empaquetarZigzag(piezas, anchoTablero, altoUtil, dosColumnas);
  }
  const MAX_POR_FILA =
    piezas.length <= 2 ? 1 : piezas.length <= 6 ? (dosColumnas ? 2 : 1) : dosColumnas ? 4 : 2;

  const filas: PiezaMosaico[][] = [];
  let fila: PiezaMosaico[] = [];
  let pesoFila = 0;
  for (const p of piezas) {
    if (
      pesoFila === 0 ||
      (fila.length < MAX_POR_FILA && pesoFila + p.peso <= PESO_FILA * 1.3)
    ) {
      fila.push(p);
      pesoFila += p.peso;
    } else {
      filas.push(fila);
      fila = [p];
      pesoFila = p.peso;
    }
  }
  if (fila.length > 0) filas.push(fila);

  let y = 0;
  for (const f of filas) {
    const pesoTotal = f.reduce((s, p) => s + p.peso, 0);
    const medio = pesoTotal / f.length;
    // La fila de la grande es también más ALTA (Kalorica: base + media×12;
    // aquí, exagerado: base + media×26, con techo para la boca gigante).
    // Y con RUIDO determinista por fila: nada de retícula de reloj.
    let semilla = 0;
    for (const c of f[0].id) semilla = (semilla * 31 + c.charCodeAt(0)) >>> 0;
    const ruido = 0.9 + ((semilla >> 4) % 21) / 100;
    const alto = Math.min(ALTO_TECHO, (ALTO_BASE + medio * 26) * ruido);
    const anchoUtil = anchoTablero;

    // Anchos crudos por peso, con mínimo legible por longitud de título.
    let anchos = f.map((p) => (anchoUtil * p.peso) / pesoTotal);
    const minimos = f.map((p) =>
      Math.max(ANCHO_MINIMO, Math.min(anchoUtil, 26 + p.letras * 3.4)),
    );
    if (anchos.some((a, i) => a < minimos[i])) {
      const garantizados = anchos.map((a, i) => Math.max(a, minimos[i]));
      const total = garantizados.reduce((s, a) => s + a, 0);
      if (total > anchoUtil) {
        const escala = anchoUtil / total;
        anchos = garantizados.map((a) => a * escala);
      } else {
        const sobra = anchoUtil - total;
        const pesoSobra = f.reduce(
          (s, p, i) => (anchos[i] >= minimos[i] ? s + p.peso : s),
          0,
        );
        anchos = anchos.map((a, i) => {
          if (a < minimos[i]) return minimos[i];
          if (pesoSobra > 0) return minimos[i] + (sobra * f[i].peso) / pesoSobra;
          return a;
        });
      }
    }

    // Borde con borde: cero hueco horizontal, la losa es continua.
    let x = 0;
    f.forEach((p, i) => {
      baldosas.set(p.id, { x, y, ancho: anchos[i], alto, peso: p.peso });
      x += anchos[i];
    });
    y += alto + HUECO_Y;
  }

  let altoTotal = Math.max(0, y - HUECO_Y);

  // LA LEY DEL PLIEGO: si la composición no llena el alto útil, TODA la
  // página se estira (los altos de fila escalan; las ventanas, que son
  // porcentuales, escalan solas). La pantalla siempre está compuesta.
  let factor = 1;
  if (altoUtil > 0 && altoTotal > 0 && altoTotal < altoUtil * 0.78) {
    const huecos = HUECO_Y * Math.max(0, filas.length - 1);
    factor = Math.min(2.6, (altoUtil * 0.97 - huecos) / (altoTotal - huecos));
    if (factor > 1.01) {
      const escaladas = new Map<string, Baldosa>();
      let yAcum = 0;
      let filaY = -1;
      let filaAlto = 0;
      const porFila: Baldosa[][] = [];
      for (const b of baldosas.values()) {
        if (b.y !== filaY) {
          filaY = b.y;
          porFila.push([]);
        }
        porFila[porFila.length - 1].push(b);
      }
      let idx = 0;
      const ids = [...baldosas.keys()];
      for (const fila of porFila) {
        // El techo de proporción: una pegatina jamás es más alta que
        // 1.2 veces el ancho medio de su fila (nada de chicles).
        const anchoMedio = fila.reduce((s2, b) => s2 + b.ancho, 0) / fila.length;
        filaAlto = Math.min(fila[0].alto * factor, anchoMedio * 1.35);
        for (const b of fila) {
          escaladas.set(ids[idx], { ...b, y: yAcum, alto: filaAlto });
          idx += 1;
        }
        yAcum += filaAlto + HUECO_Y;
      }
      return { baldosas: escaladas, alto: Math.max(0, yAcum - HUECO_Y), factor };
    }
  }
  return { baldosas, alto: altoTotal, factor };
}
