// Acción Svelte «presionable»: la física del pulsado en el MOMENTO de
// posar el dedo, no al soltar. Añade la clase .pulsado (hundimiento) y un
// tick háptico inmediato; el significado (medium, rigid…) lo pone cada
// botón en su onclick. Uso: <button use:presionable> o
// <button use:presionable={{ hap: "rigid" }}>.

import { haptic, type HapticKind } from "$lib/haptic";

export function presionable(el: HTMLElement, opts: { hap?: HapticKind } = {}) {
  let hap = opts.hap ?? "tick";
  const abajo = () => {
    haptic(hap);
    el.classList.add("pulsado");
  };
  const arriba = () => el.classList.remove("pulsado");
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
