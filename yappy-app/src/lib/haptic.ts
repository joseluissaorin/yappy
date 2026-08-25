// La háptica de la casa. En iOS pasa por el puente nativo (Haptics.swift,
// generadores estáticos precalentados); en Android usa la vibración web;
// en escritorio calla. Sin await: el tacto no espera a nadie.
//
// La paleta y su gramática:
//   tick      posar el dedo en algo pulsable, detentes de dial, paso de
//             frase del cartel (sutilísimo, casi subliminal)
//   soft      contactos blandos (abrir hojas, asomar la boca)
//   light     toques de confirmación pequeños
//   medium    acciones (pausar/reanudar, encolar)
//   rigid     teclas físicas (el mando de piano, la tecla gigante)
//   heavy     aterrizajes (un segmento cae en la cinta, el ñam del loro)
//   success/warning/error  el resultado de algo (descarte, fallo, logro)

import { invoke } from "@tauri-apps/api/core";
import { platformName } from "$lib/platform";
import { get } from "svelte/store";

export type HapticKind =
  | "tick" | "selection"
  | "light" | "soft" | "medium" | "rigid" | "heavy"
  | "success" | "warning" | "error";

const VIBRA_ANDROID: Record<HapticKind, number | number[]> = {
  tick: 4,
  selection: 4,
  light: 8,
  soft: 8,
  medium: 14,
  rigid: 16,
  heavy: 26,
  success: [12, 40, 12],
  warning: [18, 60, 18],
  error: [24, 50, 24, 50, 24],
};

export function haptic(kind: HapticKind = "light"): void {
  const p = get(platformName);
  if (p === "ios") {
    invoke("haptic_cmd", { kind }).catch(() => {});
    return;
  }
  if (p === "android" && typeof navigator !== "undefined" && "vibrate" in navigator) {
    try {
      navigator.vibrate(VIBRA_ANDROID[kind] ?? 8);
    } catch {}
  }
}
