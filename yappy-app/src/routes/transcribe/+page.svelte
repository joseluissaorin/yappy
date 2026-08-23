<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { goto } from "$app/navigation";
  import Transcribe from "$lib/Transcribe.svelte";
  import { isIOS } from "$lib/platform";
  import { onTranscribeFile } from "$lib/ipc";

  // On desktop this page is its own window; the backend emits `transcribe_file`
  // with a dropped/associated audio path. On iOS it's a client route reached via
  // the nav / share flow.
  let queuedPath: string | null = $state(null);
  let queuedNonce = $state(0);
  let cleanup: (() => void) | null = null;

  onMount(async () => {
    cleanup = await onTranscribeFile((path) => {
      queuedPath = path;
      queuedNonce++;
    });
  });
  onDestroy(() => cleanup?.());
</script>

<div class="transcribe-window" data-tauri-drag-region>
  <header class="tw-head" data-tauri-drag-region>
    {#if $isIOS}
      <button class="btn-ghost" onclick={() => goto("/")} aria-label="back to Yappy">← back</button>
    {/if}
  </header>
  <Transcribe {queuedPath} {queuedNonce} />
</div>

<style>
  .transcribe-window {
    min-height: 100vh;
    background: var(--bg, #fff8d7);
    /* Full-bleed under the status bar / home indicator (viewport-fit=cover);
       env() is 0 on desktop. */
    padding: calc(16px + env(safe-area-inset-top, 0px)) 20px
             calc(28px + env(safe-area-inset-bottom, 0px));
  }
  .tw-head {
    min-height: 28px;
    display: flex;
    align-items: center;
    /* leave room for the macOS overlay traffic lights */
    padding-left: 4px;
  }
  .tw-head .btn-ghost { font-weight: 700; }
</style>
