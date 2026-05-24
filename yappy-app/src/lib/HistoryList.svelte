<script lang="ts">
  import { onMount } from "svelte";
  import { type HistoryEntry, clearHistory, getHistory, replayHistory, langLabel } from "$lib/ipc";

  let { compact = false, max = 0 }: { compact?: boolean; max?: number } = $props();
  let entries: HistoryEntry[] = $state([]);
  let loading = $state(true);
  let visibleEntries = $derived(max > 0 ? entries.slice(0, max) : entries);

  async function refresh() {
    loading = true;
    const h = await getHistory();
    entries = h.entries;
    loading = false;
  }
  async function clearAll() {
    await clearHistory();
    entries = [];
  }
  async function replay(id: string) {
    await replayHistory(id);
  }

  onMount(refresh);

  function fmtAgo(unix: number): string {
    const now = Math.floor(Date.now() / 1000);
    const diff = now - unix;
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
  function shortSource(s: string): string {
    const map: Record<string, string> = {
      Selection: "selection",
      ActiveDocument: "document",
      Ocr: "screen ocr",
      manual: "manual",
      sample: "voice sample",
      clipboard: "clipboard",
      history: "replay",
    };
    return map[s] ?? s.toLowerCase();
  }
</script>

<div class="hist">
  <div class="hist-head">
    <p class="note">
      {#if loading}loading…{:else if entries.length === 0}nothing here yet — read something!{:else}{entries.length} read{entries.length === 1 ? "" : "s"}{/if}
    </p>
    {#if entries.length > 0}
      <button class="btn-outline tiny" onclick={clearAll}>clear all</button>
    {/if}
  </div>

  {#each visibleEntries as e}
    <article class="entry" class:compact>
      <div class="meta">
        <span class="ago">{fmtAgo(e.started_at)}</span>
        <span class="dot">·</span>
        <span class="src">{shortSource(e.source)}</span>
        {#if e.app_name}<span class="dot">·</span><span class="app">{e.app_name}</span>{/if}
        <span class="dot">·</span>
        <span class="lang">{langLabel(e.lang)}</span>
        <span class="dot">·</span>
        <span class="voice">{e.voice}</span>
        <span class="dot">·</span>
        <span class="dur">{fmtSecs(e.duration_secs)}</span>
      </div>
      <p class="preview">{e.text.length > 220 ? e.text.slice(0, 220) + "…" : e.text}</p>
      <button class="btn-outline tiny play" onclick={() => replay(e.id)}>
        <svg width="10" height="10" viewBox="0 0 14 14" fill="currentColor"><path d="M3 1.5C3 0.7 3.85 0.25 4.5 0.7L12.5 6.2c0.6 0.4 0.6 1.3 0 1.7l-8 5.5c-0.7 0.4-1.5 0-1.5-0.8V1.5Z"/></svg>
        play again
      </button>
    </article>
  {/each}
</div>

<style>
  .hist { padding: 4px 0; }
  .hist-head { display: flex; justify-content: space-between; align-items: center; margin-bottom: 10px; }
  .note { color: var(--ink-500); font-size: 13px; margin: 0; font-weight: 600; }
  .entry {
    padding: 14px 0;
    border-bottom: 2px dashed var(--ink-300);
    position: relative;
  }
  .entry:last-child { border-bottom: none; }
  .meta {
    display: flex; flex-wrap: wrap; gap: 6px; align-items: center;
    color: var(--ink-500); font-size: 12px; font-weight: 600;
    margin-bottom: 6px;
  }
  .meta .dot { opacity: 0.5; }
  .meta .app { font-weight: 700; color: var(--ink-700); }
  .preview {
    font-size: 14px; line-height: 1.45; color: var(--ink-900); font-weight: 500;
    margin: 0 0 10px; max-width: 70ch;
  }
  .btn-outline.tiny {
    padding: 4px 10px; font-size: 12px; gap: 4px;
    box-shadow: 1.5px 1.5px 0 var(--ink-900);
  }
  .btn-outline.tiny:hover { transform: translate(-1px, -1px); box-shadow: 2.5px 2.5px 0 var(--ink-900); }
  .entry.compact { padding: 10px 0; }
  .entry.compact .preview { -webkit-line-clamp: 2; line-clamp: 2; display: -webkit-box; -webkit-box-orient: vertical; overflow: hidden; margin-bottom: 6px; }
  .entry.compact .meta { font-size: 11px; gap: 5px; margin-bottom: 4px; }
</style>
