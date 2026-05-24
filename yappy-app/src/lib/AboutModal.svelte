<script lang="ts">
  import YappyMascot from "$lib/YappyMascot.svelte";
  let { open = false, onClose }: { open: boolean; onClose: () => void } = $props();
</script>

{#if open}
  <div class="overlay" onclick={onClose} role="dialog" tabindex="-1">
    <div class="sheet" onclick={(e) => e.stopPropagation()}>
      <div class="mascot"><YappyMascot size={92} happy /></div>
      <h2>yappy</h2>
      <p class="ver">version 0.1.0</p>
      <p class="desc">
        the free, open source, local text-to-speech app for your mac.
        reads anything in any app, with supertonic 3, on your device.
      </p>
      <ul class="bits">
        <li><span>voice model</span><strong>supertonic 3 — 99M params, ONNX</strong></li>
        <li><span>languages</span><strong>31</strong></li>
        <li><span>sample rate</span><strong>44.1 kHz</strong></li>
        <li><span>license</span><strong>MIT (app) · OpenRAIL-M (model)</strong></li>
      </ul>
      <footer>
        <button class="btn-outline" onclick={onClose}>close</button>
      </footer>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed; inset: 0; z-index: 300;
    background: rgba(26, 26, 26, 0.45);
    display: flex; align-items: center; justify-content: center;
    backdrop-filter: blur(6px);
  }
  .sheet {
    width: 440px; max-width: 92vw;
    background: var(--cream-100);
    border: 2.5px solid var(--ink-900);
    border-radius: 24px;
    box-shadow: var(--shadow-hd-3);
    padding: 24px 28px;
    color: var(--ink-900);
    text-align: center;
    animation: pop 0.24s var(--ease-emph);
  }
  @keyframes pop { from { transform: scale(0.92); opacity: 0; } to { transform: scale(1); opacity: 1; } }
  .mascot { display: flex; justify-content: center; margin-bottom: 6px; }
  h2 { font-family: var(--font-display); font-size: 42px; margin: 0; color: var(--pink-600); transform: rotate(-2deg); font-weight: 400; }
  .ver { color: var(--ink-500); margin: 4px 0 14px; font-weight: 600; font-size: 13px; }
  .desc { color: var(--ink-700); font-size: 14px; line-height: 1.55; margin: 0 0 18px; font-weight: 500; max-width: 36ch; margin-inline: auto; }
  ul.bits { list-style: none; padding: 0; margin: 0 0 16px; }
  ul.bits li {
    display: flex; justify-content: space-between; padding: 7px 0;
    border-bottom: 1.5px dashed var(--ink-300); font-size: 13px;
    font-weight: 600; text-align: left;
  }
  ul.bits li:last-child { border-bottom: none; }
  ul.bits span { color: var(--ink-500); }
  ul.bits strong { color: var(--ink-900); font-weight: 700; }
  footer { display: flex; justify-content: center; }
</style>
