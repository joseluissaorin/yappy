<script lang="ts">
  import { onMount } from "svelte";

  let { onDone }: { onDone?: () => void } = $props();
  let pieces: { x: number; delay: number; color: string; size: number; rot: number; xDrift: number }[] = $state([]);

  onMount(() => {
    const colors = ["#FF80AB", "#FFB3C8", "#FFD3E0", "#C3EEDB", "#D8C8FF", "#FFE78A"];
    pieces = Array.from({ length: 80 }, () => ({
      x: Math.random() * 100,
      delay: Math.random() * 0.4,
      color: colors[Math.floor(Math.random() * colors.length)],
      size: 6 + Math.random() * 8,
      rot: Math.random() * 360,
      xDrift: -40 + Math.random() * 80,
    }));
    const t = setTimeout(() => onDone?.(), 3200);
    return () => clearTimeout(t);
  });
</script>

<div class="confetti">
  {#each pieces as p}
    <span
      class="piece"
      style="
        left: {p.x}%;
        background: {p.color};
        width: {p.size}px;
        height: {p.size * 0.4}px;
        animation-delay: {p.delay}s;
        --xdrift: {p.xDrift}px;
        --rot: {p.rot}deg;
      "
    ></span>
  {/each}
</div>

<style>
  .confetti {
    position: fixed; inset: 0; pointer-events: none; z-index: 200;
    overflow: hidden;
  }
  .piece {
    position: absolute; top: -20px;
    border-radius: 999px;
    transform: rotate(var(--rot));
    animation: fall 2.6s ease-out forwards;
  }
  @keyframes fall {
    0% { transform: translate3d(0, -20px, 0) rotate(var(--rot)); opacity: 1; }
    100% { transform: translate3d(var(--xdrift), 110vh, 0) rotate(calc(var(--rot) + 720deg)); opacity: 0; }
  }
</style>
