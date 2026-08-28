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
  import { RECTO, TROQUELES_BASE, aPoligono } from "$lib/troquel";
  import { startShareIntake, drainPending } from "$lib/shareIntake";
  import { sembrarProgreso } from "$lib/progreso";
  import { progresoTodo } from "$lib/ipc";
  import { arrancarEspejo, reconciliar, repro } from "$lib/reproduccion.svelte";
  import { listen } from "@tauri-apps/api/event";
  import { readClipboard, colaAgregarArchivo, puenteVincular, logToBackend } from "$lib/ipc";
  import { invoke } from "@tauri-apps/api/core";
  import { open as abrirDialogo } from "@tauri-apps/plugin-dialog";

  let { children } = $props();
  let cleanups: (() => void)[] = [];

  // EL RITO DE LA LLEGADA (docs/EL-ALBUM.md §E9): el papel cae recto y el
  // loro lo TROQUELA a picotazos: a mitad de escena el clip pasa del folio
  // a la forma del parche (morph de 48 anclas) con tres sacudidas.
  let troquelado = $state(false);
  const formaLlegada = $derived(
    TROQUELES_BASE[(llegada.titulo ? llegada.titulo.length : 0) % TROQUELES_BASE.length],
  );
  $effect(() => {
    if (!llegada.titulo) return;
    troquelado = false;
    const t = setTimeout(() => (troquelado = true), 1250);
    return () => clearTimeout(t);
  });

  // La aguja vive en TODAS las páginas del móvil; en el cartel (/read) el
  // escenario entero YA es el mando, así que ahí se esconde.
  const enCartel = $derived(page.url.pathname.startsWith("/read"));
  const agujaViva = $derived(!enCartel && !!repro.snap && repro.snap.estado !== "inactivo");

  // (La sangre de color --vivo vive en el layout RAÍZ: /read queda fuera
  // de este caparazón y la variable debe sobrevivir a la navegación.)

  // Reconciliación tras CADA navegación: lo primero es pintar la verdad.
  afterNavigate(() => {
    reconciliar();
  });

  onMount(async () => {
    const plataforma = await ready;
    window.addEventListener("error", (e) => {
      logToBackend("error", "webview", `${e.message} @ ${e.filename}:${e.lineno}`);
    });
    window.addEventListener("unhandledrejection", (e) => {
      logToBackend("error", "webview", `promesa sin capturar: ${e.reason}`);
    });
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
    // La verdad duradera del progreso pisa la caché local al abrir.
    progresoTodo()
      .then((todo) => sembrarProgreso(todo))
      .catch(() => {});

    // EL TECLADO EMPUJA: la altura del teclado vive en --teclado y las
    // hojas con campos de texto suben con muelle para dejarle sitio.
    const vv = window.visualViewport;
    if (vv) {
      const alTeclado = () => {
        const alto = Math.max(0, window.innerHeight - vv.height - vv.offsetTop);
        document.documentElement.style.setProperty("--teclado", `${Math.round(alto)}px`);
      };
      vv.addEventListener("resize", alTeclado);
      vv.addEventListener("scroll", alTeclado);
      cleanups.push(() => {
        vv.removeEventListener("resize", alTeclado);
        vv.removeEventListener("scroll", alTeclado);
      });
    }

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
            // Sin filtros (el selector de iOS los casaba mal y dejaba todo
            // gris) y con el error REAL en el log: un fallo silencioso aquí
            // costó una ronda entera de diagnóstico.
            try {
              const ruta = await abrirDialogo({ multiple: false });
              logToBackend("info", "picker", `elegido: ${JSON.stringify(ruta)}`);
              if (typeof ruta === "string" && ruta) {
                await colaAgregarArchivo(ruta);
                logToBackend("info", "picker", "encolado");
              }
            } catch (e) {
              logToBackend("error", "picker", `fallo del selector: ${e}`);
            }
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
  <!-- EL GRANO: el papel se nota en toda la casa (materia, regla 5). -->
  <div class="grano" aria-hidden="true"></div>
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
      <div
        class="papel yap-bloque"
        class:troquelado
        style="clip-path: {troquelado ? aPoligono(formaLlegada) : aPoligono(RECTO)};"
      >
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

  /* El grano de papel: turbulencia sutil multiplicada sobre todo. */
  .grano {
    position: fixed;
    inset: 0;
    z-index: 96;
    pointer-events: none;
    opacity: 0.05;
    mix-blend-mode: multiply;
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='220' height='220'%3E%3Cfilter id='g'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='2'/%3E%3C/filter%3E%3Crect width='220' height='220' filter='url(%23g)'/%3E%3C/svg%3E");
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
    transition: clip-path 0.55s cubic-bezier(0.18, 1.5, 0.32, 1);
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
    0%, 46% {
      transform: scale(0.96);
    }
    52% {
      transform: scale(1.13) rotate(-5deg);
    }
    57% {
      transform: scale(1.02) rotate(1deg);
    }
    62% {
      transform: scale(1.11) rotate(-4deg);
    }
    67% {
      transform: scale(1.01) rotate(1deg);
    }
    72% {
      transform: scale(1.09) rotate(-3deg);
    }
    78%, 100% {
      transform: scale(1);
    }
  }
  /* Las sacudidas del troquelado: el papel recibe los picotazos. */
  .papel.troquelado {
    animation: recibe-picos 0.45s ease;
  }
  @keyframes recibe-picos {
    0%, 100% { transform: rotate(-3deg); }
    25% { transform: rotate(-4.6deg) translateY(1.5px); }
    55% { transform: rotate(-1.6deg) translateY(-1px); }
    80% { transform: rotate(-3.6deg); }
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
