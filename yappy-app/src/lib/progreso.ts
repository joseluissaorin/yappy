// Progreso de escucha por documento, y «lo último que sonó», en
// localStorage: sobrevive reinicios, no necesita backend, y se borra con
// la app. Clave por ruta del documento.
export interface UltimaEscucha {
  ruta: string;
  titulo: string;
  parrafo: number;
  total_parrafos: number;
  cuando_unix: number;
}

const CLAVE_ULTIMA = "yappy.ultima_escucha";
const PREFIJO_DOC = "yappy.progreso.";

export function guardarProgreso(u: UltimaEscucha) {
  try {
    localStorage.setItem(CLAVE_ULTIMA, JSON.stringify(u));
    localStorage.setItem(
      PREFIJO_DOC + u.ruta,
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

export function progresoDe(ruta: string): { parrafo: number; total: number } | null {
  try {
    const crudo = localStorage.getItem(PREFIJO_DOC + ruta);
    if (!crudo) return null;
    const p = JSON.parse(crudo);
    return { parrafo: p.parrafo ?? 0, total: p.total ?? 0 };
  } catch {
    return null;
  }
}
