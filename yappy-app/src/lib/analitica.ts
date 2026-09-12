// LAS ESTADÍSTICAS ANÓNIMAS: qué se mide y con qué manga. Se apagan desde
// la trastienda (ajuste `paseo.estadisticas`); apagadas, `medir` no hace
// nada. Nunca viaja texto del usuario, títulos, enlaces ni nombres: solo
// nombres de evento y valores de una lista cerrada.
import { get } from "svelte/store";
import { getVersion } from "@tauri-apps/api/app";
import { createAnalytics, type Analytics } from "$lib/analitica/cliente";
import { ANALITICA } from "$lib/analitica/config";
import { platformName } from "$lib/platform";
import { comprasUsuario } from "$lib/ipc";

let cliente: Analytics | null = null;
let activa = true;
let arrancada = false;

const almacen = {
  getItem: (k: string) => {
    try {
      return localStorage.getItem(k);
    } catch {
      return null;
    }
  },
  setItem: (k: string, v: string) => {
    try {
      localStorage.setItem(k, v);
    } catch {}
  },
};

/// Enciende (o apaga) la medición. Se llama al arrancar con el ajuste.
export function fijarEstadisticas(encendidas: boolean) {
  activa = encendidas;
}

export async function arrancarAnalitica(encendidas: boolean) {
  activa = encendidas;
  if (arrancada) return;
  arrancada = true;
  let version = "0";
  try {
    version = await getVersion();
  } catch {}
  cliente = createAnalytics({
    endpoint: ANALITICA.endpoint,
    appToken: ANALITICA.ficha,
    storage: almacen,
    platform: get(platformName),
    appVersion: version,
  });
  medir("app_open");
  // La identidad anónima de la tienda, para unir el paseo con la cuerda.
  try {
    const u = await comprasUsuario();
    if (u && activa) cliente.identify(u);
  } catch {}
  if (typeof document !== "undefined") {
    document.addEventListener("visibilitychange", () => {
      if (document.visibilityState !== "visible") void cliente?.flush();
    });
  }
}

/// Un evento con propiedades pequeñas y de lista cerrada.
export function medir(evento: string, props?: Record<string, string | number | boolean>) {
  if (!activa || !cliente) return;
  cliente.track(evento, props);
}

/// Una pantalla (o un paso del paseo).
export function medirPantalla(nombre: string, props?: Record<string, string | number | boolean>) {
  if (!activa || !cliente) return;
  cliente.screen(nombre, props);
}

export function vaciarAnalitica() {
  void cliente?.flush();
}
