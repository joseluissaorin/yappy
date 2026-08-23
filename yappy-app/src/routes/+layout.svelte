<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/stores";
  import { ready } from "$lib/platform";

  let { children } = $props();

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
    const esMovil = plataforma === "ios" || plataforma === "android";
    const ruta = $page.url.pathname;
    // El móvil tiene jerarquía propia: su raíz es «Escuchar». Las rutas de
    // escritorio (el grupo (app) y sus ventanas) no existen en la mano.
    if (esMovil && (ruta === "/" || ruta.startsWith("/voices") || ruta.startsWith("/preferences") || ruta.startsWith("/history") || ruta.startsWith("/diagnostics") || ruta.startsWith("/library"))) {
      goto("/escuchar", { replaceState: true });
    }
  });
</script>

{@render children?.()}
