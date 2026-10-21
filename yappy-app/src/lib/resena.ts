// LA RESEÑA, lado de la interfaz. El motor (resena.rs) cuenta lo escuchado
// y avisa con `resena_momento` cuando llega un buen momento (documento
// entero, o 20 minutos en dos días). Aquí solo se confirma que el INSTANTE
// es bueno: la app a la vista, sin el muro abierto ni recién cerrado, fuera
// del paseo y fuera del modo de capturas. Entonces el motor lanza el aviso
// NATIVO de la tienda y se mide `review_requested` (sin nada del usuario).
import { onResenaMomento, resenaPedir, type MomentoResena } from "$lib/ipc";
import { muroReciente } from "$lib/compras.svelte";
import { medir } from "$lib/analitica";

let arrancada = false;

function instanteBueno(): boolean {
  if (typeof document === "undefined" || document.visibilityState !== "visible") return false;
  if (muroReciente()) return false;
  const { pathname, search } = window.location;
  if (pathname.startsWith("/paseo")) return false;
  // Capturas y ensayos (?tienda=sim, ?idioma=…): nunca.
  const q = new URLSearchParams(search);
  if (q.has("tienda") || q.has("idioma")) return false;
  return true;
}

export async function arrancarResena() {
  if (arrancada) return;
  arrancada = true;
  await onResenaMomento((motivo: MomentoResena) => {
    if (!instanteBueno()) return;
    // Un respiro: que el final del documento termine de pintarse antes.
    setTimeout(async () => {
      if (!instanteBueno()) return;
      try {
        if (await resenaPedir(motivo)) medir("review_requested", { motivo });
      } catch {
        /* sin tienda o sin motor: otra vez será */
      }
    }, 1800);
  });
}
