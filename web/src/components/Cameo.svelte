<script lang="ts">
  // EL CAMEO. Cada tantos minutos un loro cruza el pie de la ventana y se
  // va. Es el guiño de la app (CriaturaCameo) traído a la web, con su
  // misma ley: no gasta el guiño con nadie delante. No sale si la pestaña
  // está oculta, no sale si se ha pedido quietud, y no sale nunca dos
  // veces seguidas por el mismo lado.
  import { onMount, onDestroy } from "svelte";
  import Loro from "./Loro.svelte";

  const TINTAS = ["#e0502a", "#2f4bc4", "#e8b41a", "#2e7d5b", "#8a4fbe", "#c43e6a", "#1f8a9c"];

  let cruzando = $state(false);
  let haciaLaDerecha = $state(true);
  let tinta = $state(TINTAS[0]!);
  let travesia = $state(16);
  let relojes: ReturnType<typeof setTimeout>[] = [];

  function programa() {
    // Entre cuatro y nueve minutos: lo justo para que sorprenda y no canse.
    const espera = 240000 + Math.random() * 300000;
    relojes.push(setTimeout(() => {
      if (document.hidden) { programa(); return; }
      haciaLaDerecha = Math.random() < 0.5;
      tinta = TINTAS[Math.floor(Math.random() * TINTAS.length)]!;
      travesia = 13 + Math.random() * 6;
      cruzando = true;
      relojes.push(setTimeout(() => { cruzando = false; programa(); }, travesia * 1000));
    }, espera));
  }

  onMount(() => {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    programa();
  });
  onDestroy(() => relojes.forEach(clearTimeout));
</script>

{#if cruzando}
  <span
    class="cameo"
    class:derecha={haciaLaDerecha}
    style="--travesia:{travesia}s"
    aria-hidden="true"
  >
    <Loro size={34} {tinta} andando mirando={haciaLaDerecha ? -1 : 1} etiqueta="" />
  </span>
{/if}

<style>
  .cameo {
    position: fixed;
    bottom: 6px;
    left: -60px;
    z-index: 5;
    pointer-events: none;
    line-height: 0;
    animation: cruza var(--travesia) linear forwards;
  }
  .cameo.derecha { animation-name: cruza-derecha; }
  @keyframes cruza-derecha { to { transform: translateX(calc(100vw + 120px)); } }
  @keyframes cruza {
    from { transform: translateX(calc(100vw + 120px)); }
    to { transform: translateX(0); }
  }
  @media (prefers-reduced-motion: reduce) { .cameo { display: none; } }
</style>
