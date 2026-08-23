<script lang="ts">
  // El cameo: cada mucho rato el loro cruza la ventana por abajo y se va,
  // sin tocar nada. No gasta el guiño con nadie delante (document.hidden),
  // no existe si el sistema pide menos movimiento, y un tercio de las
  // veces vuelve por donde vino.
  import { onMount } from "svelte";
  import Criatura from "./Criatura.svelte";

  let { size = 54 }: { size?: number } = $props();

  let visible = $state(false);
  let duracion = $state(15);
  let mirando = $state<1 | -1>(1);
  let timer: ReturnType<typeof setTimeout> | undefined;

  function programar(minMs: number, maxMs: number) {
    clearTimeout(timer);
    timer = setTimeout(cruzar, minMs + Math.random() * (maxMs - minMs));
  }

  function cruzar() {
    if (document.hidden) {
      programar(60_000, 180_000);
      return;
    }
    mirando = Math.random() < 0.35 ? -1 : 1;
    duracion = 13 + Math.random() * 6;
    visible = true;
    setTimeout(() => {
      visible = false;
      programar(240_000, 540_000);
    }, duracion * 1000 + 400);
  }

  onMount(() => {
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)");
    if (reduce.matches) return;
    programar(80_000, 200_000);
    return () => clearTimeout(timer);
  });
</script>

{#if visible}
  <div
    class="cameo"
    class:vuelve={mirando === -1}
    style="--dur:{duracion}s"
    aria-hidden="true"
  >
    <!-- El desplazamiento y el brinco van en elementos distintos: dos
         animaciones de transform sobre el mismo nodo se pisan. -->
    <Criatura {size} andando {mirando} />
  </div>
{/if}

<style>
  .cameo {
    position: fixed;
    bottom: 4px;
    left: 0;
    z-index: 60;
    pointer-events: none;
    animation: cruza var(--dur) linear forwards;
  }
  .cameo.vuelve {
    animation-name: cruza-vuelta;
  }
  @keyframes cruza {
    from {
      transform: translateX(-80px);
    }
    to {
      transform: translateX(calc(100vw + 80px));
    }
  }
  @keyframes cruza-vuelta {
    from {
      transform: translateX(calc(100vw + 80px));
    }
    to {
      transform: translateX(-80px);
    }
  }
</style>
