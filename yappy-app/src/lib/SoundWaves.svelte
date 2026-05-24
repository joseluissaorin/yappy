<script lang="ts">
  let {
    active = false,
    height = 24,
    bars = 18,
  }: { active?: boolean; height?: number; bars?: number } = $props();
</script>

<div class="waves" style="--h: {height}px;" class:active>
  {#each Array(bars) as _, i}
    <span class="bar" style="--delay: {i * 0.07}s; --base: {0.35 + ((i * 37) % 60) / 100};"></span>
  {/each}
</div>

<style>
  .waves {
    display: flex;
    align-items: center;
    gap: 3px;
    height: var(--h, 24px);
  }
  .bar {
    display: block;
    width: 3px;
    height: 35%;
    background: var(--ink-900);
    border-radius: 999px;
    transform-origin: center;
    transition: opacity 0.3s ease;
    opacity: 0.35;
  }
  .waves.active .bar {
    background: var(--pink-500);
    opacity: 1;
    animation: pulse 0.9s ease-in-out infinite;
    animation-delay: var(--delay);
  }
  @keyframes pulse {
    0%, 100% { transform: scaleY(var(--base, 0.4)); }
    50%      { transform: scaleY(1.0); }
  }
</style>
