<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { goto, afterNavigate } from "$app/navigation";
  import { page } from "$app/stores";
  import { ready } from "$lib/platform";

  let { children } = $props();

  // El guard de rutas móviles tiene que vigilar CADA navegación, no solo
  // el primer montaje: un goto("/") posterior (o un enlace antiguo) caía
  // en la portada de escritorio dentro del teléfono.
  const RUTAS_ESCRITORIO = ["/voices", "/preferences", "/history", "/diagnostics", "/library"];
  let plataformaResuelta: string | null = null;
  function esRutaDeEscritorio(ruta: string): boolean {
    return ruta === "/" || RUTAS_ESCRITORIO.some((r) => ruta.startsWith(r));
  }
  afterNavigate((nav) => {
    const esMovil = plataformaResuelta === "ios" || plataformaResuelta === "android";
    const ruta = nav.to?.url.pathname ?? "";
    if (esMovil && esRutaDeEscritorio(ruta)) {
      goto("/escuchar", { replaceState: true });
    }
  });

  // El tema, aplicado ANTES de esperar a nada (el ajuste guardado viaja en
  // localStorage además de en settings.json para evitar el destello).
  if (typeof document !== "undefined") {
    try {
      const t = localStorage.getItem("yappy.tema");
      if (t === "dark" || t === "system" || t === "cream") {
        document.documentElement.dataset.theme = t;
      }
    } catch {}
  }

  onMount(async () => {
    const plataforma = await ready;
    plataformaResuelta = plataforma;
    const esMovil = plataforma === "ios" || plataforma === "android";
    const ruta = $page.url.pathname;
    // El móvil tiene jerarquía propia: su raíz es «Escuchar». Las rutas de
    // escritorio (el grupo (app) y sus ventanas) no existen en la mano.
    if (esMovil && esRutaDeEscritorio(ruta)) {
      goto("/escuchar", { replaceState: true });
    }
  });
</script>

{@render children?.()}
