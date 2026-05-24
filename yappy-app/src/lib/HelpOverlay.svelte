<script lang="ts">
  let { open = false, onClose }: { open: boolean; onClose: () => void } = $props();
</script>

{#if open}
  <div class="overlay" onclick={onClose} role="dialog" tabindex="-1">
    <div class="sheet" onclick={(e) => e.stopPropagation()}>
      <header>
        <h2>shortcuts</h2>
        <button class="close" onclick={onClose} aria-label="close">×</button>
      </header>
      <ul>
        <li><span>read what i'm looking at</span><kbd-row><kbd>⌥</kbd><kbd>⌘</kbd><kbd>R</kbd></kbd-row></li>
        <li><span>pause / resume</span><kbd-row><kbd>⌥</kbd><kbd>⌘</kbd><kbd>space</kbd></kbd-row></li>
        <li><span>read clipboard</span><kbd-row><kbd>⌥</kbd><kbd>⌘</kbd><kbd>C</kbd></kbd-row></li>
        <li><span>stop reading</span><kbd-row><kbd>esc</kbd> <span class="hint">in player</span></kbd-row></li>
        <li><span>toggle pause (in player)</span><kbd-row><kbd>space</kbd></kbd-row></li>
        <li><span>skip back 15s</span><kbd-row><kbd>←</kbd> <span class="hint">in player</span></kbd-row></li>
        <li><span>skip forward 15s</span><kbd-row><kbd>→</kbd> <span class="hint">in player</span></kbd-row></li>
        <li><span>this overlay</span><kbd-row><kbd>⌘</kbd><kbd>/</kbd></kbd-row></li>
      </ul>
      <footer>
        press <kbd>esc</kbd> to close
      </footer>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed; inset: 0; z-index: 300;
    background: rgba(26, 26, 26, 0.42);
    display: flex; align-items: center; justify-content: center;
    backdrop-filter: blur(6px);
    -webkit-backdrop-filter: blur(6px);
    animation: in 0.18s ease;
  }
  @keyframes in { from { opacity: 0; } to { opacity: 1; } }
  .sheet {
    width: 460px; max-width: 92vw;
    background: var(--cream-100);
    border: 2.5px solid var(--ink-900);
    border-radius: 22px;
    box-shadow: var(--shadow-hd-3);
    padding: 22px 24px;
    color: var(--ink-900);
    animation: pop 0.24s var(--ease-emph);
  }
  @keyframes pop { from { transform: scale(0.92); opacity: 0; } to { transform: scale(1); opacity: 1; } }
  header {
    display: flex; align-items: center; justify-content: space-between;
    margin-bottom: 8px;
  }
  h2 { font-family: var(--font-display); font-size: 30px; margin: 0; font-weight: 400; color: var(--pink-600); }
  .close {
    width: 32px; height: 32px; border-radius: 999px;
    background: var(--surface); border: 2px solid var(--ink-900);
    font-size: 18px; font-weight: 700; color: var(--ink-900);
    box-shadow: 2px 2px 0 var(--ink-900);
  }
  .close:hover { background: var(--pink-300); }
  ul { list-style: none; margin: 14px 0 18px; padding: 0; }
  li {
    display: flex; justify-content: space-between; align-items: center;
    padding: 10px 0;
    border-bottom: 1.5px dashed var(--ink-300);
    font-size: 14px;
    font-weight: 600;
  }
  li:last-child { border-bottom: none; }
  kbd-row { display: inline-flex; gap: 4px; align-items: center; }
  .hint { color: var(--ink-500); font-size: 11px; font-weight: 600; margin-left: 4px; }
  footer { text-align: center; color: var(--ink-500); font-size: 13px; font-weight: 500; }
</style>
