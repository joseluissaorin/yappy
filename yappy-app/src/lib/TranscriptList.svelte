<script lang="ts">
  import { onMount } from "svelte";
  import { type Transcript, clearTranscripts, deleteTranscript, getTranscripts, langLabel } from "$lib/ipc";

  let { compact = false, max = 0, refreshKey = 0 }: { compact?: boolean; max?: number; refreshKey?: number } =
    $props();
  let entries: Transcript[] = $state([]);
  let loading = $state(true);
  let copiedId: string | null = $state(null);
  let visibleEntries = $derived(max > 0 ? entries.slice(0, max) : entries);

  export async function refresh() {
    loading = true;
    const t = await getTranscripts();
    entries = t.entries;
    loading = false;
  }
  async function clearAll() {
    await clearTranscripts();
    entries = [];
  }
  async function remove(id: string) {
    await deleteTranscript(id);
    entries = entries.filter((e) => e.id !== id);
  }
  async function copy(e: Transcript) {
    try {
      await navigator.clipboard.writeText(e.text);
      copiedId = e.id;
      setTimeout(() => (copiedId = copiedId === e.id ? null : copiedId), 1500);
    } catch {}
  }

  onMount(refresh);
  // Re-fetch whenever the parent bumps refreshKey (e.g. after a new transcript).
  $effect(() => {
    refreshKey;
    refresh();
  });

  function fmtAgo(unix: number): string {
    const diff = Math.floor(Date.now() / 1000) - unix;
    if (diff < 60) return "just now";
    if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
    if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
    return `${Math.floor(diff / 86400)}d ago`;
  }
  function fmtSecs(s: number): string {
    if (!isFinite(s) || s <= 0) return "—";
    const m = Math.floor(s / 60);
    const sec = Math.floor(s % 60);
    return `${m}:${sec.toString().padStart(2, "0")}`;
  }
</script>

<div class="hist">
  <div class="hist-head">
    <p class="note">
      {#if loading}loading…{:else if entries.length === 0}no transcripts yet — share or open an audio file!{:else}{entries.length} transcript{entries.length === 1 ? "" : "s"}{/if}
    </p>
    {#if entries.length > 0}
      <button class="btn-outline tiny" onclick={clearAll}>clear all</button>
    {/if}
  </div>

  {#each visibleEntries as e (e.id)}
    <article class="entry" class:compact>
      <div class="meta">
        <span class="ago">{fmtAgo(e.created_at)}</span>
        <span class="dot">·</span>
        <span class="src">{e.source.toLowerCase()}</span>
        {#if e.filename}<span class="dot">·</span><span class="app">{e.filename}</span>{/if}
        {#if e.language}<span class="dot">·</span><span class="lang">{langLabel(e.language)}</span>{/if}
        {#if e.duration_secs > 0}<span class="dot">·</span><span class="dur">{fmtSecs(e.duration_secs)}</span>{/if}
      </div>
      <p class="preview">{e.text.length > 320 ? e.text.slice(0, 320) + "…" : e.text}</p>
      <div class="row">
        <button class="btn-outline tiny" onclick={() => copy(e)}>{copiedId === e.id ? "copied!" : "copy"}</button>
        <button class="btn-outline tiny" onclick={() => remove(e.id)}>delete</button>
      </div>
    </article>
  {/each}
</div>

<style>
  .hist { padding: 4px 0; }
  .hist-head { display: flex; justify-content: space-between; align-items: center; margin-bottom: 10px; }
  .note { color: var(--ink-500); font-size: 13px; margin: 0; font-weight: 600; }
  .entry { padding: 14px 0; border-bottom: 2px dashed var(--ink-300); position: relative; }
  .entry:last-child { border-bottom: none; }
  .meta {
    display: flex; flex-wrap: wrap; gap: 6px; align-items: center;
    color: var(--ink-500); font-size: 12px; font-weight: 600; margin-bottom: 6px;
  }
  .meta .dot { opacity: 0.5; }
  .meta .app { font-weight: 700; color: var(--ink-700); }
  .preview {
    font-size: 14px; line-height: 1.45; color: var(--ink-900); font-weight: 500;
    margin: 0 0 10px; max-width: 70ch; white-space: pre-wrap;
  }
  .row { display: flex; gap: 8px; }
  .btn-outline.tiny {
    padding: 4px 10px; font-size: 12px; gap: 4px;
    box-shadow: 1.5px 1.5px 0 var(--ink-900);
  }
  .btn-outline.tiny:hover { transform: translate(-1px, -1px); box-shadow: 2.5px 2.5px 0 var(--ink-900); }
  .entry.compact { padding: 10px 0; }
  .entry.compact .preview { -webkit-line-clamp: 2; line-clamp: 2; display: -webkit-box; -webkit-box-orient: vertical; overflow: hidden; margin-bottom: 6px; }
  .entry.compact .meta { font-size: 11px; gap: 5px; margin-bottom: 4px; }
</style>
