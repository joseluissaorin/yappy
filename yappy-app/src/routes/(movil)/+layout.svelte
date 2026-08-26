<script lang="ts">
  // El caparazón del móvil, versión radical: NO hay cabecera, NO hay
  // pestañas, NO hay minirreproductor. La cinta (escuchar) es la casa, el
  // cartel (/read) es la lectura, y este caparazón solo guarda la puerta:
  // el guard de plataforma, la ingesta de lo compartido, las acciones de
  // deep link y LA LLEGADA: la escena en que lo compartido cae en el pico.
  import { onMount, onDestroy } from "svelte";
  import { goto, afterNavigate } from "$app/navigation";
  import { page } from "$app/state";
  import { ready, platformLocale } from "$lib/platform";
  import { get as getStore } from "svelte/store";
  import { fijarIdiomaDesdeLocale } from "$lib/i18n";
  import Criatura from "$lib/Criatura.svelte";
  import Aguja from "$lib/Aguja.svelte";
  import { haptic } from "$lib/haptic";
  import { llegada } from "$lib/llegada.svelte";
  import { aplicarTintaVoz, tintaVoz } from "$lib/voces";
  import { startShareIntake, drainPending } from "$lib/shareIntake";
  import { arrancarEspejo, reconciliar, repro } from "$lib/reproduccion.svelte";
  import { listen } from "@tauri-apps/api/event";
  import { readClipboard, colaAgregarArchivo, puenteVincular } from "$lib/ipc";
  import { invoke } from "@tauri-apps/api/core";
  import { open as abrirDialogo } from "@tauri-apps/plugin-dialog";

  let { children } = $props();
  let cleanups: (() => void)[] = [];

  // La aguja vive en TODAS las páginas del móvil; en el cartel (/read) el
  // escenario entero YA es el mando, así que ahí se esconde.
  const enCartel = $derived(page.url.pathname.startsWith("/read"));
  const agujaViva = $derived(!enCartel && !!repro.snap && repro.snap.estado !== "inactivo");

  // Reconciliación tras CADA navegación: lo primero es pintar la verdad.
  afterNavigate(() => {
    reconciliar();
  });

  onMount(async () => {
    const plataforma = await ready;
    if (plataforma !== "ios" && plataforma !== "android") {
      goto("/", { replaceState: true });
      return;
    }
    // Override de desarrollo para capturas: ?idioma=es|en.
    const idiomaForzado = new URLSearchParams(window.location.search).get("idioma");
    fijarIdiomaDesdeLocale(idiomaForzado ?? getStore(platformLocale));
    aplicarTintaVoz();
    await arrancarEspejo();
    startShareIntake();

    // Acciones que llegan por deep link (widget, Spotlight, atajos).
    cleanups.push(
      await listen<{ tipo: string; path: string | null; datos?: string | null }>("yappy_accion", async (ev) => {
        const a = ev.payload;
        switch (a.tipo) {
          case "pair":
            if (a.datos) {
              try {
                await puenteVincular("yappy://pair?" + a.datos);
                haptic("success");
                goto("/ajustes");
              } catch (e) {
                console.error("pair:", e);
              }
            }
            break;
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
</script>

<div class="movil" style="--aguja-hueco: {agujaViva ? '84px' : '0px'}">
  {@render children?.()}

  {#if !enCartel}
    <Aguja />
  {/if}

  {#if llegada.titulo}
    <!-- La llegada: el papel cae, el loro lo atrapa, y empieza a hablar. -->
    <div
      class="llegada"
      role="presentation"
      onpointerdown={() => (llegada.titulo = null)}
    >
      <div class="papel yap-bloque">
        <span class="papel-lineas" aria-hidden="true"></span>
        <strong>{llegada.titulo}</strong>
      </div>
      <div class="comensal">
        <Criatura size={132} cantando tinta={$tintaVoz} />
      </div>
    </div>
  {/if}
</div>

<style>
  .movil {
    min-height: 100dvh;
    background: var(--yap-papel);
  }

  /* ── La llegada ── */
  .llegada {
    position: fixed;
    inset: 0;
    z-index: 90;
    background: color-mix(in srgb, var(--yap-papel) 94%, transparent);
    backdrop-filter: blur(8px);
    -webkit-backdrop-filter: blur(8px);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    animation: llegada-fondo 2.1s ease forwards;
  }
  .papel {
    max-width: 76vw;
    padding: 18px 20px 22px;
    transform: rotate(-3deg);
    font-family: var(--yap-lectura, Georgia, serif);
    font-size: 19px;
    line-height: 1.3;
    animation: papel-cae 1.15s cubic-bezier(0.25, 0.9, 0.3, 1.15) both;
  }
  .papel-lineas {
    display: block;
    height: 8px;
    margin-bottom: 10px;
    background:
      linear-gradient(var(--yap-borde) 2px, transparent 2px) 0 0 / 100% 4px;
    opacity: 0.9;
  }
  .comensal {
    animation: pico-atrapa 2.1s ease both;
    transform-origin: 50% 85%;
  }
  @keyframes papel-cae {
    0% {
      transform: translateY(-58vh) rotate(9deg);
      opacity: 0;
    }
    45% {
      opacity: 1;
    }
    100% {
      transform: translateY(0) rotate(-3deg);
      opacity: 1;
    }
  }
  @keyframes pico-atrapa {
    0%, 48% {
      transform: scale(0.96);
    }
    58% {
      transform: scale(1.14) rotate(-4deg);
    }
    68% {
      transform: scale(1);
    }
    100% {
      transform: scale(1);
    }
  }
  @keyframes llegada-fondo {
    0%, 78% {
      opacity: 1;
    }
    100% {
      opacity: 0;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .papel, .comensal, .llegada {
      animation: none;
    }
  }
</style>
