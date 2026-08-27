<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { goto, afterNavigate } from "$app/navigation";
  import { page } from "$app/stores";
  import { ready } from "$lib/platform";
  import { repro } from "$lib/reproduccion.svelte";
  import { colorDeArchivo } from "$lib/juguete";

  let { children } = $props();

  // LA SANGRE DE COLOR (docs/EL-JUGUETE.md §8): cuando algo suena, --vivo
  // toma el color de esa pieza y tiñe la aguja, el karaoke del cartel y
  // los acentos. Vive AQUÍ, en la raíz, porque /read está fuera del
  // caparazón móvil y este layout no se desmonta jamás.
  $effect(() => {
    if (typeof document === "undefined") return;
    const s = repro.snap;
    if (s && s.estado !== "inactivo" && s.doc_path) {
      document.documentElement.style.setProperty("--vivo", colorDeArchivo(s.doc_path));
    } else {
      document.documentElement.style.removeProperty("--vivo");
    }
  });

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
