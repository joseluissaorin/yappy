// EL TROQUEL: el motor de formas del álbum (docs/EL-ALBUM.md §E1). Cada
// documento es una pegatina troquelada con forma propia. TODAS las formas
// son polígonos normalizados de 48 anclas (0..100): mismo número de puntos
// en todas para que el clip-path pueda INTERPOLAR entre dos cualesquiera:
// marcar favorito funde la forma en corazón, completarla la funde en
// estrella. El morph es gratis; la geometría entera está diseñada para él.

export const ANCLAS = 48;

export interface Troquel {
  nombre: string;
  /// 48 puntos [x, y] en 0..100, en sentido horario desde arriba.
  puntos: [number, number][];
  /// Ventana segura del texto, en porcentaje de la celda.
  ventana: { x: number; y: number; w: number; h: number };
  /// Tope de líneas del título en reposo (las formas puntiagudas caben
  /// menos renglones que las redondas).
  lineas?: number;
}

// ── Utilidades geométricas ─────────────────────────────────────────────

/// Muestrea una curva paramétrica f(t) con t en [0,1) en ANCLAS puntos.
function parametrica(f: (t: number) => [number, number]): [number, number][] {
  const pts: [number, number][] = [];
  for (let i = 0; i < ANCLAS; i++) pts.push(f(i / ANCLAS));
  return pts;
}

/// Reparte ANCLAS puntos por las aristas de un polígono de esquinas VIVAS,
/// garantizando que cada esquina original queda incluida (sin redondearse).
function poligonal(esquinas: [number, number][]): [number, number][] {
  const n = esquinas.length;
  const largos: number[] = [];
  let total = 0;
  for (let i = 0; i < n; i++) {
    const a = esquinas[i];
    const b = esquinas[(i + 1) % n];
    const l = Math.hypot(b[0] - a[0], b[1] - a[1]);
    largos.push(l);
    total += l;
  }
  // Puntos por arista, proporcionales y con mínimo 1 (la esquina misma).
  const cupos = largos.map((l) => Math.max(1, Math.floor((l / total) * ANCLAS)));
  let sobra = ANCLAS - cupos.reduce((s, c) => s + c, 0);
  for (let i = 0; sobra !== 0; i = (i + 1) % n) {
    if (sobra > 0) {
      cupos[i] += 1;
      sobra -= 1;
    } else if (cupos[i] > 1) {
      cupos[i] -= 1;
      sobra += 1;
    }
  }
  const pts: [number, number][] = [];
  for (let i = 0; i < n; i++) {
    const a = esquinas[i];
    const b = esquinas[(i + 1) % n];
    for (let k = 0; k < cupos[i]; k++) {
      const u = k / cupos[i];
      pts.push([a[0] + (b[0] - a[0]) * u, a[1] + (b[1] - a[1]) * u]);
    }
  }
  return pts;
}

function redondear(pts: [number, number][]): [number, number][] {
  return pts.map(([x, y]) => [Math.round(x * 10) / 10, Math.round(y * 10) / 10]);
}

// ── El catálogo ────────────────────────────────────────────────────────

function circulo(): [number, number][] {
  return parametrica((t) => {
    const a = t * Math.PI * 2 - Math.PI / 2;
    return [50 + 48 * Math.cos(a), 50 + 48 * Math.sin(a)];
  });
}

function nube(): [number, number][] {
  // Círculo abollado: bultos orgánicos de vapor.
  return parametrica((t) => {
    const a = t * Math.PI * 2 - Math.PI / 2;
    const r = 41 + 5.5 * Math.sin(4 * a + 0.7) + 2.5 * Math.sin(7 * a);
    return [50 + r * Math.cos(a), 50 + r * Math.sin(a)];
  });
}

function flor(): [number, number][] {
  return parametrica((t) => {
    const a = t * Math.PI * 2 - Math.PI / 2;
    const r = 38 + 9 * Math.sin(6 * a);
    return [50 + r * Math.cos(a), 50 + r * Math.sin(a)];
  });
}

function sello(): [number, number][] {
  // El borde dentado del sello de correos.
  return parametrica((t) => {
    const a = t * Math.PI * 2 - Math.PI / 2;
    const r = 43 + 4.5 * Math.sin(11 * a);
    return [50 + r * Math.cos(a), 50 + r * Math.sin(a)];
  });
}

function rombo(): [number, number][] {
  return poligonal([
    [50, 1],
    [99, 50],
    [50, 99],
    [1, 50],
  ]);
}

function hexagono(): [number, number][] {
  const pts: [number, number][] = [];
  for (let i = 0; i < 6; i++) {
    const a = (i / 6) * Math.PI * 2 - Math.PI / 2;
    pts.push([50 + 47 * Math.cos(a), 50 + 47 * Math.sin(a)]);
  }
  return poligonal(pts);
}

function escudo(): [number, number][] {
  return poligonal([
    [8, 4],
    [50, 9],
    [92, 4],
    [95, 46],
    [78, 78],
    [50, 98],
    [22, 78],
    [5, 46],
  ]);
}

function etiqueta(): [number, number][] {
  // La etiqueta de precio: rectángulo con la punta de atar a la izquierda.
  return poligonal([
    [30, 4],
    [96, 4],
    [96, 96],
    [30, 96],
    [3, 50],
  ]);
}

function corazon(): [number, number][] {
  return parametrica((t) => {
    const a = t * Math.PI * 2;
    const x = 16 * Math.pow(Math.sin(a), 3);
    const y = 13 * Math.cos(a) - 5 * Math.cos(2 * a) - 2 * Math.cos(3 * a) - Math.cos(4 * a);
    return [50 + x * 2.9, 44 - y * 2.9];
  });
}

function estrella(): [number, number][] {
  const pts: [number, number][] = [];
  for (let i = 0; i < 10; i++) {
    const a = (i / 10) * Math.PI * 2 - Math.PI / 2;
    const r = i % 2 === 0 ? 49 : 23;
    pts.push([50 + r * Math.cos(a), 50 + r * Math.sin(a)]);
  }
  return poligonal(pts);
}

/// Las formas BASE (identidad de cada pieza). Corazón y estrella quedan
/// fuera: son RESERVADAS (favorito y completada).
export const TROQUELES_BASE: Troquel[] = [
  { nombre: "circulo", puntos: redondear(circulo()), ventana: { x: 15, y: 24, w: 70, h: 50 } },
  { nombre: "nube", puntos: redondear(nube()), ventana: { x: 16, y: 26, w: 68, h: 46 } },
  { nombre: "flor", puntos: redondear(flor()), ventana: { x: 24, y: 29, w: 52, h: 40 }, lineas: 2 },
  { nombre: "sello", puntos: redondear(sello()), ventana: { x: 16, y: 24, w: 68, h: 50 } },
  { nombre: "rombo", puntos: redondear(rombo()), ventana: { x: 26, y: 31, w: 48, h: 36 }, lineas: 2 },
  { nombre: "hexagono", puntos: redondear(hexagono()), ventana: { x: 16, y: 24, w: 68, h: 50 } },
  { nombre: "escudo", puntos: redondear(escudo()), ventana: { x: 17, y: 16, w: 66, h: 46 } },
  { nombre: "etiqueta", puntos: redondear(etiqueta()), ventana: { x: 33, y: 16, w: 58, h: 66 } },
];

/// El folio sin troquelar (para el rito de la llegada: de recto a forma).
export const RECTO: Troquel = {
  nombre: "recto",
  puntos: redondear(
    poligonal([
      [3, 3],
      [97, 3],
      [97, 97],
      [3, 97],
    ]),
  ),
  ventana: { x: 10, y: 12, w: 80, h: 68 },
};

export const CORAZON: Troquel = {
  nombre: "corazon",
  puntos: redondear(corazon()),
  ventana: { x: 18, y: 20, w: 64, h: 38 },
  lineas: 2,
};

export const ESTRELLA: Troquel = {
  nombre: "estrella",
  puntos: redondear(estrella()),
  ventana: { x: 27, y: 33, w: 46, h: 28 },
  lineas: 2,
};

// ── Render ─────────────────────────────────────────────────────────────

/// El polígono CSS de un troquel, opcionalmente encogido hacia el centro
/// (para el borde de troquel: la capa crema fuera, la tinta dentro).
export function aPoligono(t: Troquel, escala = 1): string {
  const pts = t.puntos
    .map(([x, y]) => `${(50 + (x - 50) * escala).toFixed(1)}% ${(50 + (y - 50) * escala).toFixed(1)}%`)
    .join(", ");
  return `polygon(${pts})`;
}

/// Los puntos para un <polygon> SVG (viewBox 0 0 100 100), para la costura.
export function aPuntosSvg(t: Troquel, escala = 1): string {
  return t.puntos
    .map(([x, y]) => `${(50 + (x - 50) * escala).toFixed(1)},${(50 + (y - 50) * escala).toFixed(1)}`)
    .join(" ");
}

/// La forma base determinista de un id (sin las reservadas).
export function troquelBase(id: string): Troquel {
  let h = 0;
  for (const c of id) h = (h * 33 + c.charCodeAt(0)) >>> 0;
  return TROQUELES_BASE[h % TROQUELES_BASE.length];
}
