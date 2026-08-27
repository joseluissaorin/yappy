// EL JUGUETE: el idioma físico de la casa (docs/EL-JUGUETE.md). Aquí viven
// los muelles con nombre, la paleta viva y las formas de fieltro: todo lo
// que hace que la interfaz sea un juguete coherente en vez de una pantalla.

/// Los tres muelles con nombre. SECO para el contacto (rápido, un pelín de
/// rebote), SERENO para el tablero (recolocaciones amplias), REBOTÓN para
/// las llegadas (entra pasándose de frenada).
export const SECO = "0.16s cubic-bezier(0.3, 1.4, 0.6, 1)";
export const SERENO = "0.6s cubic-bezier(0.18, 1.5, 0.32, 1)";
export const REBOTON = "0.42s cubic-bezier(0.24, 1.7, 0.44, 1)";

/// La paleta viva: planos saturados con blanco legible encima.
export const PALETA = [
  "#FF6B6B",
  "#F94892",
  "#A166F2",
  "#5D5FEF",
  "#2E9BF7",
  "#00A896",
  "#2EC96E",
  "#FF8E3C",
  "#E84393",
  "#7C4DFF",
];

function hashDe(id: string): number {
  let h = 0;
  for (const c of id) h = (h * 33 + c.charCodeAt(0)) >>> 0;
  return h;
}

/// El color propio de cada pieza: determinista, vivo y plano.
export function colorDe(id: string): string {
  return PALETA[hashDe(id) % PALETA.length];
}

/// El color de un documento por su NOMBRE de fichero (no la ruta entera:
/// iOS migra el contenedor en cada update y el color debe sobrevivir).
/// Es el mismo que ve la baldosa y el que bebe --vivo en el layout.
export function colorDeArchivo(ruta: string): string {
  const nombre = ruta.split("/").pop() || ruta;
  return colorDe(nombre);
}

/// El mismo color, hundido: tinta sobre tinta (numerales, mareas). Sin
/// transparencias: se calcula el hexadecimal sólido.
export function tonoHondo(hex: string): string {
  const n = parseInt(hex.slice(1), 16);
  const f = 0.74;
  const r = Math.round(((n >> 16) & 255) * f);
  const g = Math.round(((n >> 8) & 255) * f);
  const b = Math.round((n & 255) * f);
  return `#${((r << 16) | (g << 8) | b).toString(16).padStart(6, "0")}`;
}

/// El índice de paleta de un id (para ajustar choques entre vecinas).
export function indiceDe(id: string): number {
  return hashDe(id) % PALETA.length;
}

/// LA FORMA de cada pieza: cuatro familias de recorte, para que el tablero
/// sea un collage de verdad y no una retícula de rectángulos clónicos.
///   fieltro   canto blando muy asimétrico
///   sesgada   papel cortado a tijera (cuadrilátero irregular, esquinas vivas)
///   mordida   dos esquinas enormes enfrentadas, dos mínimas
///   canto     casi blob: la piedra pulida
export interface Forma {
  radios: string;
  clip: string;
}
export function formaDe(id: string): Forma {
  const h = hashDe(id);
  const v = (i: number, a: number, b: number) => a + (((h >> (i * 3)) & 7) * (b - a)) / 7;
  const fam = (h >> 2) % 4;
  if (fam === 0) {
    const r = (i: number) => Math.round(v(i, 12, 34));
    return {
      radios: `${r(0)}px ${r(1)}px ${r(2)}px ${r(3)}px / ${r(4)}px ${r(5)}px ${r(6)}px ${r(7)}px`,
      clip: "",
    };
  }
  if (fam === 1) {
    const d = (i: number) => v(i, 0, 4.5).toFixed(1);
    return {
      radios: "6px",
      clip: `polygon(${d(0)}% ${d(1)}%, ${(100 - v(2, 0, 4.5)).toFixed(1)}% ${d(3)}%, 100% ${(100 - v(4, 0, 4.5)).toFixed(1)}%, ${d(5)}% 100%)`,
    };
  }
  if (fam === 2) {
    const g = Math.round(v(0, 32, 44));
    const p = Math.round(v(1, 6, 12));
    return { radios: `${g}px ${p}px ${g}px ${p}px / ${g}px ${p}px ${g}px ${p}px`, clip: "" };
  }
  const r = (i: number) => Math.round(v(i, 26, 44));
  return {
    radios: `${r(0)}px ${r(1)}px ${r(2)}px ${r(3)}px / ${r(4)}px ${r(5)}px ${r(6)}px ${r(7)}px`,
    clip: "",
  };
}

/// Compatibilidad: el canto blando simple (la familia fieltro).
export function radiosDe(id: string): string {
  return formaDe(id).radios;
}

/// La inclinación de collage: cada pieza pegada a mano, ±2.4 grados.
export function tiltDe(id: string): number {
  const h = hashDe(id);
  return (((h >> 3) % 49) - 24) / 10;
}
