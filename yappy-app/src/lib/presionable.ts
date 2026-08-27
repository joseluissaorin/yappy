// Acción Svelte «presionable»: la física del pulsado en el MOMENTO de
// posar el dedo, no al soltar. Añade la clase .pulsado (hundimiento) y un
// tick háptico inmediato; el significado (medium, rigid…) lo pone cada
// botón en su onclick. Como en Kalorica, SOLTAR también se siente: un tick
// al levantar el dedo (doble contacto). Uso: <button use:presionable> o
// <button use:presionable={{ hap: "rigid" }}>.

import { haptic, type HapticKind } from "$lib/haptic";

export function presionable(el: HTMLElement, opts: { hap?: HapticKind } = {}) {
  let hap = opts.hap ?? "tick";
  let hundido = false;
  const abajo = () => {
    haptic(hap);
    hundido = true;
    el.classList.add("pulsado");
  };
  const arriba = (e: PointerEvent) => {
    el.classList.remove("pulsado");
    // El contacto de vuelta: solo al SOLTAR de verdad (no al salirse).
    if (hundido && e.type === "pointerup") haptic("tick");
    hundido = false;
  };
  el.addEventListener("pointerdown", abajo);
  el.addEventListener("pointerup", arriba);
  el.addEventListener("pointercancel", arriba);
  el.addEventListener("pointerleave", arriba);
  return {
    update(nuevas: { hap?: HapticKind } = {}) {
      hap = nuevas.hap ?? "tick";
    },
    destroy() {
      el.removeEventListener("pointerdown", abajo);
      el.removeEventListener("pointerup", arriba);
      el.removeEventListener("pointercancel", arriba);
      el.removeEventListener("pointerleave", arriba);
    },
  };
}
