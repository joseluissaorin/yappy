// Formato de atajos por plataforma: los glifos ⌥⌘ son de Mac; en Windows y
// Linux se escriben Ctrl+Alt+R. También el nombre del aparato («tu Mac»,
// «tu PC») para el copy que lo necesite.
import { derived } from "svelte/store";
import { platformName } from "$lib/platform";

export const esMac = derived(platformName, ($p) => $p === "macos");

/// «alt+cmd+r» / «ctrl+alt+r» → «⌥⌘R» en Mac, «Ctrl+Alt+R» en el resto.
export function formatearAtajo(combo: string, plataforma: string): string {
  const partes = combo
    .toLowerCase()
    .split("+")
    .map((p) => p.trim())
    .filter(Boolean);
  if (plataforma === "macos") {
    const mapa: Record<string, string> = {
      cmd: "⌘", command: "⌘", super: "⌘", meta: "⌘",
      alt: "⌥", option: "⌥", shift: "⇧", ctrl: "⌃", control: "⌃",
    };
    const orden = ["⌃", "⌥", "⇧", "⌘"];
    const mods = partes.filter((p) => mapa[p]).map((p) => mapa[p]);
    const teclas = partes.filter((p) => !mapa[p]).map((p) => p.toUpperCase());
    return orden.filter((m) => mods.includes(m)).join("") + teclas.join("");
  }
  const bonita: Record<string, string> = {
    cmd: "Win", command: "Win", super: "Win", meta: "Win",
    alt: "Alt", option: "Alt", shift: "Shift", ctrl: "Ctrl", control: "Ctrl",
  };
  return partes.map((p) => bonita[p] ?? p.toUpperCase()).join("+");
}

/// El atajo de leer-ahora tal como se enseña en pantalla.
export const atajoLeer = derived(platformName, ($p) =>
  formatearAtajo($p === "macos" ? "alt+cmd+r" : "ctrl+alt+r", $p),
);

export const nombreAparato = derived(platformName, ($p) =>
  $p === "macos" ? "your Mac" : $p === "windows" ? "your PC" : "your computer",
);
