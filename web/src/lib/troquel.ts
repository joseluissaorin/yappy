// EL TROQUEL, en la web. Puerto fiel de yappy-app/src/lib/troquel.ts: las
// mismas formas, las mismas 48 anclas, la misma geometría. La app corta sus
// pegatinas con este molde y la web corta las suyas con el mismo, para que
// una captura y una sección se reconozcan como parientes.
//
// Todas las formas tienen EXACTAMENTE 48 puntos para que un clip-path pueda
// interpolar entre dos cualesquiera. En la web eso permite que una pegatina
// se funda en corazón al pasar el ratón sin una sola línea de JavaScript.

export const ANCLAS = 48;

export interface Troquel {
  nombre: string;
  puntos: [number, number][];
  ventana: { x: number; y: number; w: number; h: number };
}

function parametrica(f: (t: number) => [number, number]): [number, number][] {
  const pts: [number, number][] = [];
  for (let i = 0; i < ANCLAS; i++) pts.push(f(i / ANCLAS));
  return pts;
}

/// Reparte las 48 anclas por las aristas de un polígono de esquinas VIVAS,
/// garantizando que cada esquina original queda incluida (sin redondearse).
function poligonal(esquinas: [number, number][]): [number, number][] {
  const n = esquinas.length;
  const largos: number[] = [];
  let total = 0;
  for (let i = 0; i < n; i++) {
    const a = esquinas[i]!;
    const b = esquinas[(i + 1) % n]!;
    const l = Math.hypot(b[0] - a[0], b[1] - a[1]);
    largos.push(l);
    total += l;
  }
  const cupos = largos.map((l) => Math.max(1, Math.floor((l / total) * ANCLAS)));
  let sobra = ANCLAS - cupos.reduce((s, c) => s + c, 0);
  for (let i = 0; sobra !== 0; i = (i + 1) % n) {
    if (sobra > 0) { cupos[i]! += 1; sobra -= 1; }
    else if (cupos[i]! > 1) { cupos[i]! -= 1; sobra += 1; }
  }
  const pts: [number, number][] = [];
  for (let i = 0; i < n; i++) {
    const a = esquinas[i]!;
    const b = esquinas[(i + 1) % n]!;
    for (let k = 0; k < cupos[i]!; k++) {
      const u = k / cupos[i]!;
      pts.push([a[0] + (b[0] - a[0]) * u, a[1] + (b[1] - a[1]) * u]);
    }
  }
  return pts;
}

const redondear = (pts: [number, number][]): [number, number][] =>
  pts.map(([x, y]) => [Math.round(x * 10) / 10, Math.round(y * 10) / 10]);

// ── El catálogo ────────────────────────────────────────────────────────

const circulo = () => parametrica((t) => {
  const a = t * Math.PI * 2 - Math.PI / 2;
  return [50 + 48 * Math.cos(a), 50 + 48 * Math.sin(a)];
});

/// Círculo abollado: bultos orgánicos de vapor.
const nube = () => parametrica((t) => {
  const a = t * Math.PI * 2 - Math.PI / 2;
  const r = 41 + 5.5 * Math.sin(4 * a + 0.7) + 2.5 * Math.sin(7 * a);
  return [50 + r * Math.cos(a), 50 + r * Math.sin(a)];
});

const flor = () => parametrica((t) => {
  const a = t * Math.PI * 2 - Math.PI / 2;
  const r = 38 + 9 * Math.sin(6 * a);
  return [50 + r * Math.cos(a), 50 + r * Math.sin(a)];
});

/// El borde dentado del sello de correos.
const sello = () => parametrica((t) => {
  const a = t * Math.PI * 2 - Math.PI / 2;
  const r = 43 + 4.5 * Math.sin(11 * a);
  return [50 + r * Math.cos(a), 50 + r * Math.sin(a)];
});

const rombo = () => poligonal([[50, 1], [99, 50], [50, 99], [1, 50]]);

const hexagono = () => {
  const pts: [number, number][] = [];
  for (let i = 0; i < 6; i++) {
    const a = (i / 6) * Math.PI * 2 - Math.PI / 2;
    pts.push([50 + 47 * Math.cos(a), 50 + 47 * Math.sin(a)]);
  }
  return poligonal(pts);
};

const escudo = () => poligonal([
  [8, 4], [50, 9], [92, 4], [95, 46], [78, 78], [50, 98], [22, 78], [5, 46],
]);

/// La etiqueta de precio: rectángulo con la punta de atar a la izquierda.
const etiqueta = () => poligonal([[30, 4], [96, 4], [96, 96], [30, 96], [3, 50]]);

const corazon = () => parametrica((t) => {
  const a = t * Math.PI * 2;
  const x = 16 * Math.pow(Math.sin(a), 3);
  const y = 13 * Math.cos(a) - 5 * Math.cos(2 * a) - 2 * Math.cos(3 * a) - Math.cos(4 * a);
  return [50 + x * 2.9, 44 - y * 2.9];
});

const estrella = () => {
  const pts: [number, number][] = [];
  for (let i = 0; i < 10; i++) {
    const a = (i / 10) * Math.PI * 2 - Math.PI / 2;
    const r = i % 2 === 0 ? 49 : 23;
    pts.push([50 + r * Math.cos(a), 50 + r * Math.sin(a)]);
  }
  return poligonal(pts);
};

/// La banderola: la cinta de premio, para los sellos de «gratis».
const banderin = () => poligonal([
  [6, 6], [94, 6], [94, 70], [50, 97], [6, 70],
]);

export const TROQUELES: Record<string, Troquel> = {
  circulo: { nombre: "circulo", puntos: redondear(circulo()), ventana: { x: 20, y: 27, w: 60, h: 46 } },
  nube: { nombre: "nube", puntos: redondear(nube()), ventana: { x: 19, y: 28, w: 62, h: 42 } },
  flor: { nombre: "flor", puntos: redondear(flor()), ventana: { x: 24, y: 29, w: 52, h: 40 } },
  sello: { nombre: "sello", puntos: redondear(sello()), ventana: { x: 16, y: 24, w: 68, h: 50 } },
  rombo: { nombre: "rombo", puntos: redondear(rombo()), ventana: { x: 26, y: 31, w: 48, h: 36 } },
  hexagono: { nombre: "hexagono", puntos: redondear(hexagono()), ventana: { x: 16, y: 24, w: 68, h: 50 } },
  escudo: { nombre: "escudo", puntos: redondear(escudo()), ventana: { x: 17, y: 16, w: 66, h: 46 } },
  etiqueta: { nombre: "etiqueta", puntos: redondear(etiqueta()), ventana: { x: 33, y: 16, w: 58, h: 66 } },
  corazon: { nombre: "corazon", puntos: redondear(corazon()), ventana: { x: 22, y: 20, w: 56, h: 44 } },
  estrella: { nombre: "estrella", puntos: redondear(estrella()), ventana: { x: 25, y: 32, w: 50, h: 36 } },
  banderin: { nombre: "banderin", puntos: redondear(banderin()), ventana: { x: 12, y: 16, w: 76, h: 48 } },
};

export type NombreTroquel = keyof typeof TROQUELES;

/// El polígono a escala interior, desde el centro (50,50). Sirve para las
/// capas: 1 = el borde de troquel crema, 0.93 = el cuerpo de tinta,
/// 0.80 = por donde pasa la costura.
export function poligono(t: Troquel, escala = 1): string {
  return t.puntos
    .map(([x, y]) => `${(50 + (x - 50) * escala).toFixed(1)}% ${(50 + (y - 50) * escala).toFixed(1)}%`)
    .join(", ");
}

/// Lo mismo en coordenadas de un viewBox 0 0 100 100, para los SVG.
export function puntosSvg(t: Troquel, escala = 1): string {
  return t.puntos
    .map(([x, y]) => `${(50 + (x - 50) * escala).toFixed(1)},${(50 + (y - 50) * escala).toFixed(1)}`)
    .join(" ");
}
