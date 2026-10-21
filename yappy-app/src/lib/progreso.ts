// Progreso de escucha por documento, y «lo último que sonó». La VERDAD
// duradera vive en el BACKEND (progreso.json, escrito por el motor
// mientras suena: sobrevive a cierres bruscos y a updates); localStorage
// es solo la caché de lectura síncrona, con clave por NOMBRE de fichero
// (las rutas absolutas mueren con cada migración del contenedor iOS).
export interface UltimaEscucha {
  ruta: string;
  titulo: string;
  parrafo: number;
  total_parrafos: number;
  cuando_unix: number;
}

const CLAVE_ULTIMA = "yappy.ultima_escucha";
const PREFIJO_DOC = "yappy.progreso.";

/// La clave estable: el nombre del fichero, jamás la ruta entera.
function nombreDe(ruta: string): string {
  return ruta.split("/").pop() || ruta;
}

export function guardarProgreso(u: UltimaEscucha) {
  try {
    localStorage.setItem(CLAVE_ULTIMA, JSON.stringify(u));
    localStorage.setItem(
      PREFIJO_DOC + nombreDe(u.ruta),
      JSON.stringify({ parrafo: u.parrafo, total: u.total_parrafos, cuando: u.cuando_unix }),
    );
  } catch {
    /* almacenamiento lleno o privado: no pasa nada */
  }
}

export function ultimaEscucha(): UltimaEscucha | null {
  try {
    const crudo = localStorage.getItem(CLAVE_ULTIMA);
    return crudo ? (JSON.parse(crudo) as UltimaEscucha) : null;
  } catch {
    return null;
  }
}

export function progresoDe(ruta: string): { parrafo: number; total: number; cuando: number } | null {
  try {
    const crudo =
      localStorage.getItem(PREFIJO_DOC + nombreDe(ruta)) ??
      // Compatibilidad con lo guardado por ruta absoluta (una migración).
      localStorage.getItem(PREFIJO_DOC + ruta);
    if (!crudo) return null;
    const p = JSON.parse(crudo);
    return { parrafo: p.parrafo ?? 0, total: p.total ?? 0, cuando: p.cuando ?? 0 };
  } catch {
    return null;
  }
}

/// LA SIEMBRA: al arrancar, la verdad del backend (que siguió escribiendo
/// aunque la app muriera sonando) pisa la caché local si va por delante.
export function sembrarProgreso(todo: Record<string, { parrafo: number; total: number }>) {
  try {
    for (const [nombre, p] of Object.entries(todo)) {
      const clave = PREFIJO_DOC + nombre;
      const local = localStorage.getItem(clave);
      let mejor = true;
      if (local) {
        const l = JSON.parse(local);
        mejor = (p.parrafo ?? 0) > (l.parrafo ?? 0);
      }
      if (mejor) {
        localStorage.setItem(
          clave,
          JSON.stringify({ parrafo: p.parrafo, total: p.total, cuando: 0 }),
        );
      }
    }
  } catch {
    /* la caché es prescindible */
  }
}

/// El párrafo donde reanudar un documento que hoy tiene `totalActual`
/// párrafos. Los EPUB importados antes del 21-10-2026 salían en el móvil
/// como un párrafo por fichero del libro (6 en vez de 305): su índice
/// guardado no vale tal cual y se lleva a la MISMA PROPORCIÓN del libro
/// nuevo. Solo con lo que guardó el lector (`cuando` > 0), cuyo total es
/// siempre el del documento entero; lo sembrado desde el motor (cuando 0)
/// puede traer el total parcial de la cocina y no se reescala.
export function parrafoReanudable(ruta: string, totalActual: number): number {
  const p = progresoDe(ruta);
  if (!p || !p.total || p.parrafo <= 0 || totalActual <= 0) return 0;
  let parrafo = p.parrafo;
  const esEpub = ruta.toLowerCase().endsWith(".epub");
  if (esEpub && p.cuando > 0 && p.total * 3 <= totalActual) {
    parrafo = Math.floor((p.parrafo / p.total) * totalActual);
  }
  return Math.min(parrafo, Math.max(0, totalActual - 1));
}
