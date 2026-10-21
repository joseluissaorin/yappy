// EXPORTAR: la pieza sale a Markdown, HTML o PDF con la marca de Yappy.
// En el móvil, Rust escribe en la caché y se abre la hoja de compartir del
// sistema (guardar en Archivos, mandar, imprimir…); en el escritorio, el
// diálogo de guardar elige el destino. Libre para todos: sin puerta.

import { get } from "svelte/store";
import { save } from "@tauri-apps/plugin-dialog";
import { t, idiomaUI } from "$lib/i18n";
import { isMobile } from "$lib/platform";
import { exportarDocumento, shareFile, logToBackend, colaListar, type FormatoExportar, type PeticionExportar } from "$lib/ipc";

export type FuenteExportar = Omit<PeticionExportar, "formato" | "marca" | "destino">;

const FILTROS: Record<FormatoExportar, { name: string; extensions: string[] }> = {
  md: { name: "Markdown", extensions: ["md"] },
  html: { name: "HTML", extensions: ["html"] },
  pdf: { name: "PDF", extensions: ["pdf"] },
};

/// El nombre que propone el diálogo de guardar (Rust hace lo mismo en el móvil).
function nombrePropuesto(titulo: string, ext: string): string {
  let base = titulo.replace(/[\/\\:*?"<>|\u0000-\u001f]/g, " ").replace(/\s+/g, " ").trim();
  base = base.replace(/\.(epub|pdf|docx|md|txt|html?|yappy)$/i, "").trim();
  if ([...base].length > 80) base = [...base].slice(0, 80).join("").trim();
  return `${base || "Yappy"}.${ext}`;
}

/// La marca de agua en el idioma de la interfaz, con la fecha a su manera.
export function marcaDeHoy() {
  const tr = get(t);
  let fecha: string;
  try {
    fecha = new Intl.DateTimeFormat(get(idiomaUI), { dateStyle: "long" }).format(new Date());
  } catch {
    fecha = new Date().toISOString().slice(0, 10);
  }
  return { leido_con: tr("exportar.leido_con"), exportado: tr("exportar.fecha").replace("{fecha}", fecha) };
}

/// Exporta y entrega. Devuelve "compartido", "guardado", "cancelado" o
/// lanza el error (el que llama decide cómo contarlo).
export async function exportar(formato: FormatoExportar, fuente: FuenteExportar): Promise<"compartido" | "guardado" | "cancelado"> {
  const marca = marcaDeHoy();
  // Desde el cartel no se sabe el origen: si el fichero es una pieza de la
  // cinta, ella guarda la URL (y el título bonito).
  if (fuente.ruta && !fuente.origen) {
    try {
      const pieza = (await colaListar()).find((i) => i.ruta === fuente.ruta);
      if (pieza) fuente = { ...fuente, origen: pieza.origen, titulo: pieza.titulo || fuente.titulo };
    } catch {}
  }
  if (get(isMobile)) {
    const ruta = await exportarDocumento({ ...fuente, formato, marca });
    await shareFile(ruta);
    return "compartido";
  }
  const destino = await save({
    defaultPath: nombrePropuesto(fuente.titulo ?? "Yappy", formato),
    filters: [FILTROS[formato]],
  });
  if (!destino) return "cancelado";
  await exportarDocumento({ ...fuente, formato, marca, destino });
  return "guardado";
}

export function contarError(e: unknown) {
  logToBackend("error", "exportar", String(e)).catch(() => {});
}
