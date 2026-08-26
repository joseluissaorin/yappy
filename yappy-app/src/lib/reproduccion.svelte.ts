// EL ESPEJO: la única verdad de reproducción que se pinta. Recibe los
// snapshots del motor (evento playback_state), descarta revisiones viejas
// (los eventos rezagados no resucitan estados muertos) y RECONCILIA
// pidiendo el snapshot real al montar, al volver del fondo y tras navegar.
// Ninguna vista vuelve a deducir nada: todas pintan `repro.snap`.
import {
  playbackSnapshot,
  onPlaybackState,
  onNivel,
  type PlaybackSnapshot,
} from "$lib/ipc";

export const repro = $state<{
  snap: PlaybackSnapshot | null;
  nivel: number;
}>({ snap: null, nivel: 0 });

let ultimaRevision = 0;
let arrancado = false;

function acepta(s: PlaybackSnapshot) {
  // La revisión es monótona por vida del motor; si llega una menor es un
  // evento rezagado de una época muerta: a la basura.
  if (s.revision !== undefined && s.revision < ultimaRevision) return;
  ultimaRevision = s.revision ?? ultimaRevision;
  repro.snap = s;
  if (s.estado !== "sonando") repro.nivel = 0;
}

/// Pide la verdad al motor y la pinta. La reconciliación: lo primero al
/// montar cualquier vista, al volver del fondo, tras cualquier navegación.
export async function reconciliar() {
  try {
    const s = await playbackSnapshot();
    // El snapshot pedido SIEMPRE es más fresco o igual que lo pintado:
    // adoptarlo aunque la revisión coincida (idempotente).
    if (s.revision >= ultimaRevision) {
      ultimaRevision = s.revision;
      repro.snap = s;
      if (s.estado !== "sonando") repro.nivel = 0;
    }
  } catch {
    /* el motor aún no está: se reconcilia en el próximo intento */
  }
}

/// Arranca el espejo una sola vez por vida del webview: listeners de
/// snapshot y de nivel, y reconciliación al volver del fondo.
export async function arrancarEspejo() {
  if (arrancado) {
    await reconciliar();
    return;
  }
  arrancado = true;
  await reconciliar();
  await onPlaybackState(acepta);
  await onNivel((v) => {
    repro.nivel = v;
  });
  if (typeof document !== "undefined") {
    document.addEventListener("visibilitychange", () => {
      if (document.visibilityState === "visible") reconciliar();
    });
  }
}
