// El casting: cada voz es un pájaro con su tinta, y la voz elegida ES tu
// loro en toda la app. Su tinta se filtra en los acentos (aguja, teclas,
// hilo) mezclada con el coral de marca al 72/28: la marca nunca muere,
// pero la app se atempera hacia quien le pone la voz.
import { writable } from "svelte/store";

export const TINTAS_VOZ = [
  "#e0502a", "#2f4bc4", "#e8b41a", "#2e7d5b", "#8a4fbe",
  "#c43e6a", "#1f8a9c", "#b8651f", "#5b6d2e", "#7a4a32",
];

const CLAVE = "yappy.tinta_voz";

function leerGuardada(): string {
  try {
    return localStorage.getItem(CLAVE) ?? TINTAS_VOZ[0];
  } catch {
    return TINTAS_VOZ[0];
  }
}

export const tintaVoz = writable<string>(
  typeof localStorage !== "undefined" ? leerGuardada() : TINTAS_VOZ[0],
);

/// Fija la tinta y la pinta en la raíz del documento para que el CSS de
/// cualquier página la respire vía var(--voz-tinta).
export function fijarTintaVoz(hex: string) {
  tintaVoz.set(hex);
  try {
    localStorage.setItem(CLAVE, hex);
  } catch {}
  aplicarTintaVoz(hex);
}

export function aplicarTintaVoz(hex?: string) {
  if (typeof document === "undefined") return;
  document.documentElement.style.setProperty("--voz-tinta", hex ?? leerGuardada());
}
