<script lang="ts">
  // Yappy's mark: a pink lowercase "y" in the same hand-drawn display font as
  // the "yappy" wordmark in the header. Replaces the old cartoon mascot — same
  // props (size / talking / happy) so every call site keeps working. `size` is
  // the glyph height in px; `talking` gives it a little read-aloud bounce.
  let {
    size = 140,
    talking = false,
    happy = false,
  }: { size?: number; talking?: boolean; happy?: boolean } = $props();
</script>

<span
  class="y-mark"
  class:talking
  class:happy
  style="font-size: {size}px; width: {size}px; height: {size}px;"
  aria-hidden="true">y</span>

<style>
  .y-mark {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-family: var(--font-display);
    color: var(--pink-600);
    font-weight: 400;
    line-height: 1;
    transform: rotate(-5deg);
    user-select: none;
    -webkit-user-select: none;
    transition: transform 0.2s var(--ease-emph, ease);
  }
  .y-mark.talking { animation: y-bounce 0.42s ease-in-out infinite alternate; transform-origin: 50% 80%; }
  .y-mark.happy { animation: y-wiggle 1.3s ease-in-out infinite; }
  @keyframes y-bounce {
    from { transform: rotate(-5deg) translateY(0) scale(1); }
    to   { transform: rotate(-8deg) translateY(-7%) scale(1.07); }
  }
  @keyframes y-wiggle {
    0%, 100% { transform: rotate(-5deg); }
    50%      { transform: rotate(-10deg) scale(1.04); }
  }
</style>
