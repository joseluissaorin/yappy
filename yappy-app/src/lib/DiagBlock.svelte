<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { captureDiagnostics, type CaptureDiagnostics } from "$lib/ipc";

  let diag: CaptureDiagnostics | null = $state(null);
  let loading = $state(false);
  let interval: ReturnType<typeof setInterval>;

  async function refresh() {
    loading = true;
    try {
      diag = await captureDiagnostics();
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    refresh();
  });
  onDestroy(() => clearInterval(interval));
</script>

<div class="diag">
  <div class="diag-head">
    <p class="diag-note">Watch what Yappy detects when you focus another app.</p>
    <button class="btn-secondary" onclick={refresh} disabled={loading}>{loading ? "Reading…" : "Refresh"}</button>
  </div>

  {#if diag}
    <dl>
      <dt>Front app</dt>
      <dd>{diag.front_app ?? "—"}</dd>
      <dt>Selection</dt>
      <dd class:empty={!diag.selection_preview}>{diag.selection_preview || "no selection"}</dd>
      <dt>Active document</dt>
      <dd class:empty={!diag.active_doc_preview}>{diag.active_doc_preview || "no integration for this app"}</dd>
      <dt>Clipboard</dt>
      <dd class:empty={!diag.clipboard_preview}>{diag.clipboard_preview || "empty"}</dd>
    </dl>
  {/if}
</div>

<style>
  .diag { padding: 10px 0; }
  .diag-head { display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px; }
  .diag-note { color: var(--ink-500); font-size: 13px; margin: 0; font-weight: 500; }
  dl {
    display: grid; grid-template-columns: 140px 1fr; gap: 10px 14px;
    margin: 14px 0 0; font-size: 13px;
  }
  dt { color: var(--ink-500); padding-top: 2px; font-weight: 700; }
  dd {
    margin: 0; word-break: break-word; max-height: 140px; overflow-y: auto;
    background: var(--cream-50);
    border: 2px solid var(--ink-900);
    padding: 8px 12px; border-radius: 12px;
    box-shadow: 2px 2px 0 var(--ink-900);
    font-weight: 500;
  }
  dd.empty { color: var(--ink-500); font-style: italic; font-weight: 500; }
</style>
