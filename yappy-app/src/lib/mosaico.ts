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

const ALTO_BASE = 78;
const ALTO_TECHO = 340;
const HUECO_Y = 8;
const ANCHO_MINIMO = 96;

/// Empaqueta las piezas en filas por peso y devuelve la baldosa de cada id
/// más el alto total del tablero.
export function empaquetar(
  piezas: PiezaMosaico[],
  anchoTablero: number,
  altoUtil = 0,
): { baldosas: Map<string, Baldosa>; alto: number; factor: number } {
  const baldosas = new Map<string, Baldosa>();
  if (piezas.length === 0 || anchoTablero <= 0) {
    return { baldosas, alto: 0, factor: 1 };
  }

  // Peso objetivo por fila (Kalorica: 2,5 de base, tope de 4 tarjetas).
  const pesoMedio = piezas.reduce((s, p) => s + p.peso, 0) / piezas.length;
  const PESO_FILA = Math.max(2.5, pesoMedio * 2.5);
  // MODO PÓSTER: con pocas piezas, columnas gordas (la página se compone
  // entera, jamás queda medio vacía).
  const MAX_POR_FILA = piezas.length <= 2 ? 1 : piezas.length <= 4 ? 2 : 4;

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
    factor = Math.min(2.6, (altoUtil * 0.92 - huecos) / (altoTotal - huecos));
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
        filaAlto = fila[0].alto * factor;
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
