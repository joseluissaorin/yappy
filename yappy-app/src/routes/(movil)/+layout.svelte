<script lang="ts">
  // El caparazón del móvil: cabecera con el loro, contenido, el
  // minirreproductor persistente y dos pestañas (Escuchar / Biblioteca).
  // Este grupo de rutas SOLO existe en el móvil; en escritorio se redirige
  // al shell de ventanas.
  import { onMount, onDestroy } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/stores";
  import { ready, platformLocale } from "$lib/platform";
  import { get as getStore } from "svelte/store";
  import { t, fijarIdiomaDesdeLocale } from "$lib/i18n";
  import Criatura from "$lib/Criatura.svelte";
  import CriaturaCameo from "$lib/CriaturaCameo.svelte";
  import { haptic } from "$lib/haptic";
  import {
    onPlaybackState,
    onPlaybackStarting,
    togglePause,
    type PlaybackSnapshot,
  } from "$lib/ipc";
  import { startShareIntake, drainPending } from "$lib/shareIntake";
  import { listen } from "@tauri-apps/api/event";
  import { readClipboard, colaAgregarArchivo } from "$lib/ipc";
  import { invoke } from "@tauri-apps/api/core";
  import { open as abrirDialogo } from "@tauri-apps/plugin-dialog";

  let { children } = $props();

  let playback = $state<PlaybackSnapshot | null>(null);
  let tituloSonando = $state("");
  let cleanups: (() => void)[] = [];

  const hayAudio = $derived(!!playback && (playback.playing || playback.paused));
  const seccion = $derived($page.url.pathname.startsWith("/biblioteca") ? "biblioteca" : "escuchar");

  onMount(async () => {
    const plataforma = await ready;
    if (plataforma !== "ios" && plataforma !== "android") {
      // Escritorio: este grupo no es para ti.
      goto("/", { replaceState: true });
      return;
    }
    // Override de desarrollo para capturas: ?idioma=es|en.
    const idiomaForzado = new URLSearchParams(window.location.search).get("idioma");
    fijarIdiomaDesdeLocale(idiomaForzado ?? getStore(platformLocale));
    cleanups.push(await onPlaybackState((s) => (playback = s)));
    cleanups.push(
      await onPlaybackStarting((p) => {
        tituloSonando = (p.text_preview ?? "").split("\n")[0].slice(0, 60);
      }),
    );
    startShareIntake();

    // Acciones que llegan por deep link (widget, Spotlight, atajos).
    cleanups.push(
      await listen<{ tipo: string; path: string | null }>("yappy_accion", async (ev) => {
        const a = ev.payload;
        switch (a.tipo) {
          case "shared":
            await drainPending();
            break;
          case "read-clipboard":
            await readClipboard().catch(() => {});
            break;
          case "open": {
            const ruta = await abrirDialogo({
              multiple: false,
              filters: [
                {
                  name: "Documentos",
                  extensions: ["txt", "md", "markdown", "rtf", "docx", "doc", "odt", "pdf", "epub", "html", "htm"],
                },
              ],
            }).catch(() => null);
            if (typeof ruta === "string") await colaAgregarArchivo(ruta).catch(() => {});
            break;
          }
          case "library":
            if (a.path) {
              await invoke("library_play_cmd", { path: a.path, fromStart: false }).catch(() => {});
              goto("/biblioteca/audiolibros");
            }
            break;
        }
      }),
    );
  });
  onDestroy(() => cleanups.forEach((c) => c()));

  async function alternar() {
    haptic("medium");
    await togglePause();
  }
</script>

<div class="movil">
  <header class="cabecera">
    <button class="marca" onclick={() => goto("/escuchar")} aria-label="Yappy">
      <Criatura size={34} andando={hayAudio && !playback?.paused} cantando={hayAudio && !playback?.paused} />
      <span class="palabra">yappy</span>
    </button>
    <button
      class="engranaje"
      onclick={() => {
        haptic("light");
        goto("/ajustes");
      }}
      aria-label={$t("nav.ajustes")}
    >
      <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>
    </button>
  </header>

  <main class="contenido yap-enter">
    {@render children?.()}
  </main>

  {#if hayAudio}
    <button class="miniplayer yap-bloque" onclick={() => goto("/read")} aria-label="abrir el reproductor">
      <span class="mini-loro">
        <Criatura size={30} andando={!playback?.paused} />
      </span>
      <span class="mini-texto">
        <span class="mini-titulo">{tituloSonando || playback?.current_text?.slice(0, 60) || "…"}</span>
        <span class="mini-barra">
          <span
            class="mini-progreso"
            style="width: {playback && playback.duration_secs > 0
              ? Math.min(100, (playback.elapsed_secs / playback.duration_secs) * 100)
              : 0}%"
          ></span>
        </span>
      </span>
      <span
        class="mini-boton"
        role="button"
        tabindex="0"
        onclick={(e) => {
          e.stopPropagation();
          alternar();
        }}
        onkeydown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.stopPropagation();
            alternar();
          }
        }}
        aria-label={playback?.paused ? $t("player.reanudar") : $t("player.pausar")}
      >
        {#if playback?.paused}
          <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor"><path d="M8 5v14l11-7z"/></svg>
        {:else}
          <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor"><path d="M6 5h4v14H6zM14 5h4v14h-4z"/></svg>
        {/if}
      </span>
    </button>
  {/if}

  <nav class="pestanas-abajo" aria-label="secciones">
    <button
      class="pestana-abajo"
      class:activa={seccion === "escuchar"}
      onclick={() => {
        haptic("light");
        goto("/escuchar");
      }}
    >
      <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><path d="M3 18v-6a9 9 0 0 1 18 0v6"/><path d="M21 19a2 2 0 0 1-2 2h-1a2 2 0 0 1-2-2v-3a2 2 0 0 1 2-2h3zM3 19a2 2 0 0 0 2 2h1a2 2 0 0 0 2-2v-3a2 2 0 0 0-2-2H3z"/></svg>
      <span>{$t("nav.escuchar")}</span>
    </button>
    <button
      class="pestana-abajo"
      class:activa={seccion === "biblioteca"}
      onclick={() => {
        haptic("light");
        goto("/biblioteca");
      }}
    >
      <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20"/><path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z"/></svg>
      <span>{$t("nav.biblioteca")}</span>
    </button>
  </nav>

  <CriaturaCameo size={46} />
</div>

<style>
  .movil {
    min-height: 100dvh;
    display: flex;
    flex-direction: column;
    padding-top: env(safe-area-inset-top);
  }
  .cabecera {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 16px 6px;
  }
  .marca {
    display: inline-flex;
    align-items: center;
    gap: 8px;
  }
  .palabra {
    font-weight: 800;
    font-size: 22px;
    letter-spacing: -0.02em;
    color: var(--yap-voz);
    transform: rotate(-2deg);
  }
  .engranaje {
    color: var(--yap-tinta-suave);
    padding: 8px;
    border-radius: 999px;
  }
  .engranaje:active {
    background: var(--yap-superficie-2);
  }
  .contenido {
    flex: 1;
    padding: 4px 16px 12px;
    overflow-y: auto;
  }
  .miniplayer {
    position: sticky;
    bottom: 0;
    margin: 0 10px 8px;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    border-radius: 16px;
    box-shadow: var(--yap-relieve-alto);
    text-align: left;
    width: calc(100% - 20px);
  }
  .mini-loro {
    flex: none;
  }
  .mini-texto {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .mini-titulo {
    font-weight: 700;
    font-size: 0.9rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .mini-barra {
    height: 4px;
    border-radius: 999px;
    background: var(--yap-superficie-2);
    box-shadow: var(--yap-hundido);
    overflow: hidden;
  }
  .mini-progreso {
    display: block;
    height: 100%;
    background: var(--yap-voz);
    border-radius: 999px;
    transition: width 0.3s linear;
  }
  .mini-boton {
    flex: none;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 38px;
    height: 38px;
    border-radius: 999px;
    background: var(--yap-tecla);
    color: var(--yap-tecla-tinta);
    box-shadow: var(--yap-relieve);
  }
  .pestanas-abajo {
    display: flex;
    border-top: 1px solid var(--yap-borde);
    background: var(--yap-superficie);
    box-shadow: var(--yap-luz);
    padding-bottom: env(safe-area-inset-bottom);
  }
  .pestana-abajo {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 3px;
    padding: 9px 0 7px;
    font-size: 0.72rem;
    font-weight: 600;
    color: var(--yap-tinta-suave);
  }
  .pestana-abajo.activa {
    color: var(--yap-voz);
  }
</style>
