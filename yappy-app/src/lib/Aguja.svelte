<script lang="ts">
  // LA AGUJA UNIVERSAL: si algo suena o se prepara, está aquí, en CUALQUIER
  // página del móvil. Título real, estado honesto (sonando / en pausa /
  // preparando con su cuenta), pausa y seguir SIN entrar a nada, y
  // deslizarla hacia fuera la mata del todo (detener de verdad, con época).
  // Tocarla lleva al cartel, restaurando el documento si el lector perdió
  // su estado (relanzamiento).
  import { goto } from "$app/navigation";
  import { t } from "$lib/i18n";
  import { haptic } from "$lib/haptic";
  import { presionable } from "$lib/presionable";
  import Criatura from "$lib/Criatura.svelte";
  import { tintaVoz } from "$lib/voces";
  import { repro } from "$lib/reproduccion.svelte";
  import { reader } from "$lib/readerStore.svelte";
  import { pausar, reanudar, stopPlayback, readDocument } from "$lib/ipc";

  const snap = $derived(repro.snap);
  const estado = $derived(snap?.estado ?? "inactivo");
  const titulo = $derived(
    (snap?.titulo || snap?.current_text || "").split("\n")[0].replace(/\.[^.]+$/, "").slice(0, 72),
  );

  // ── Deslizar hacia fuera = detener del todo ───────────────────────────
  let dx = $state(0);
  let muriendo = $state(false);
  let arranque: { x: number; y: number; decidido: boolean } | null = null;

  function alTocar(e: PointerEvent) {
    arranque = { x: e.clientX, y: e.clientY, decidido: false };
  }
  function alMover(e: PointerEvent) {
    if (!arranque || muriendo) return;
    const ddx = e.clientX - arranque.x;
    const ddy = e.clientY - arranque.y;
    if (!arranque.decidido) {
      if (Math.abs(ddx) < 12 && Math.abs(ddy) < 12) return;
      if (Math.abs(ddy) > Math.abs(ddx)) {
        arranque = null;
        return;
      }
      arranque.decidido = true;
      (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
    }
    dx = ddx;
  }
  let finArrastre = 0;
  async function alSoltar() {
    const decidido = arranque?.decidido;
    arranque = null;
    if (!decidido) {
      dx = 0;
      return;
    }
    // El navegador dispara un click fantasma tras el arrastre: se veta.
    finArrastre = performance.now();
    if (Math.abs(dx) > 120) {
      // Detener de verdad: el motor avanza la época y todo lo de esta
      // sesión (audio en vuelo incluido) muere con ella.
      muriendo = true;
      haptic("heavy");
      dx = dx > 0 ? 640 : -640;
      await stopPlayback().catch(() => {});
      setTimeout(() => {
        muriendo = false;
        dx = 0;
      }, 300);
    } else {
      dx = 0;
    }
  }

  async function alternar(e: Event) {
    e.stopPropagation();
    if (estado === "pausa") {
      haptic("medium");
      await reanudar().catch(() => {});
    } else if (estado === "sonando") {
      haptic("medium");
      await pausar().catch(() => {});
    }
  }

  async function abrirCartel() {
    if (muriendo || Math.abs(dx) > 4) return;
    if (performance.now() - finArrastre < 400) return;
    haptic("light");
    if (!reader.doc && snap?.doc_path) {
      try {
        reader.doc = await readDocument(snap.doc_path);
        if (snap.titulo) reader.doc.filename = snap.titulo;
      } catch {
        /* el fichero ya no está: el cartel enseñará lo que sepa */
      }
    }
    if (reader.doc) goto("/read");
  }
</script>

{#if snap && estado !== "inactivo"}
  <div class="aguja-marco">
    <div
      class="aguja"
      role="toolbar"
      aria-label={titulo}
      class:muere={muriendo}
      style="transform: translateX({dx}px) rotate({dx / 40}deg); opacity: {muriendo ? 0 : Math.max(0.3, 1 - Math.abs(dx) / 320)};"
      onpointerdown={alTocar}
      onpointermove={alMover}
      onpointerup={alSoltar}
      onpointercancel={() => { arranque = null; dx = 0; }}
    >
      <button class="aguja-abrir" onclick={abrirCartel}>
        <span class="aguja-loro">
          <Criatura
            size={40}
            mirando={-1}
            estado={estado === "sonando" ? "hablando" : estado === "pausa" ? "pausa" : "comiendo"}
            apertura={estado === "sonando" ? repro.nivel : 0}
            cantando={estado === "preparando"}
            tinta={$tintaVoz}
          />
        </span>
        <span class="aguja-cuerpo">
          <span class="aguja-estado">
            {#if estado === "preparando"}
              {$t("aguja.preparando")}{#if snap.total > 0}&nbsp;· {snap.chunks_cocinados}/{snap.total}{/if}
            {:else if estado === "pausa"}
              {$t("cinta.en_pausa")}
            {:else}
              {$t("cinta.sonando")}
            {/if}
          </span>
          <strong class="aguja-titulo">{titulo}</strong>
        </span>
        {#if estado === "sonando"}
          <span class="ondas" aria-hidden="true" style="--nivel: {0.35 + repro.nivel * 0.65}"><i></i><i></i><i></i><i></i></span>
        {/if}
      </button>
      {#if estado !== "preparando"}
        <button
          class="aguja-mando"
          use:presionable={{ hap: "medium" }}
          onclick={alternar}
          aria-label={estado === "pausa" ? $t("aguja.seguir") : $t("aguja.pausar")}
        >
          {#if estado === "pausa"}
            <svg viewBox="0 0 24 24" width="26" height="26" fill="currentColor"><path d="M7 4.8c0-1.1 1.2-1.8 2.2-1.2l11.5 7.2c0.9 0.6 0.9 1.9 0 2.4L9.2 20.4C8.2 21 7 20.3 7 19.2z"/></svg>
          {:else}
            <svg viewBox="0 0 24 24" width="26" height="26" fill="currentColor"><rect x="6" y="4.5" width="4.2" height="15" rx="1.6"/><rect x="13.8" y="4.5" width="4.2" height="15" rx="1.6"/></svg>
          {/if}
        </button>
      {/if}
    </div>
  </div>
{/if}

<style>
  .aguja-marco {
    position: fixed;
    left: 14px;
    right: 14px;
    bottom: calc(env(safe-area-inset-bottom) + 12px);
    z-index: 40;
    pointer-events: none;
  }
  .aguja {
    pointer-events: auto;
    display: flex;
    align-items: center;
    gap: 4px;
    /* La sangre de color: la aguja se tiñe del color de la pieza que
       suena (--vivo, docs/EL-JUGUETE.md §8); plano, sin degradado. */
    background: var(--vivo, var(--acento-voz, #e0502a));
    color: #fff6ef;
    border-radius: 20px;
    padding: 10px 10px 10px 12px;
    box-shadow: 0 4px 0 rgba(43, 36, 24, 0.22);
    transition: transform 0.24s cubic-bezier(0.2, 0.9, 0.3, 1.15), opacity 0.2s ease;
    touch-action: pan-y;
  }
  .aguja.muere {
    transition: transform 0.28s ease-in, opacity 0.26s ease-in;
  }
  .aguja-abrir {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 11px;
    border: 0;
    background: transparent;
    color: inherit;
    text-align: left;
    padding: 0;
    cursor: pointer;
  }
  .aguja-loro {
    flex-shrink: 0;
    display: inline-flex;
    background: color-mix(in srgb, var(--vivo, var(--acento-voz, #e0502a)) 55%, #f7f2e7);
    border-radius: 13px;
    padding: 3px;
  }
  .aguja-cuerpo {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
    flex: 1;
  }
  .aguja-estado {
    font-family: var(--yap-mono, ui-monospace, monospace);
    font-size: 10.5px;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    opacity: 0.85;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .aguja-titulo {
    font-weight: 800;
    font-size: 15.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ondas {
    display: inline-flex;
    align-items: flex-end;
    gap: 3px;
    height: 22px;
    margin-right: 4px;
    transform: scaleY(var(--nivel, 0.6));
    transform-origin: 50% 100%;
    transition: transform 0.1s linear;
    flex-shrink: 0;
  }
  .ondas i {
    width: 4.5px;
    border-radius: 3px;
    background: #fff6ef;
    animation: onda-aguja 0.9s ease-in-out infinite;
  }
  .ondas i:nth-child(1) { height: 40%; }
  .ondas i:nth-child(2) { height: 90%; animation-delay: 0.15s; }
  .ondas i:nth-child(3) { height: 60%; animation-delay: 0.3s; }
  .ondas i:nth-child(4) { height: 80%; animation-delay: 0.45s; }
  @keyframes onda-aguja {
    0%, 100% { transform: scaleY(0.5); }
    50% { transform: scaleY(1); }
  }
  .aguja-mando {
    flex-shrink: 0;
    width: 52px;
    height: 52px;
    border-radius: 16px;
    border: 0;
    background: color-mix(in srgb, var(--vivo, var(--acento-voz, #e0502a)) 45%, #f7f2e7);
    color: #fff6ef;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
  }
  @media (prefers-reduced-motion: reduce) {
    .ondas i { animation: none; }
  }
</style>
