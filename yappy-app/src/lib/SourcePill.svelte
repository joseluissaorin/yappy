<script lang="ts">
  type Source =
    | { kind: "selection"; app_name?: string | null }
    | { kind: "active_document"; app_name: string; doc_kind: string }
    | { kind: "webpage"; app_name: string; url?: string | null; title?: string | null }
    | { kind: "ocr"; app_name?: string | null }
    | { kind: "manual" }
    | { kind: "clipboard" }
    | { kind: "file"; path: string; extension: string }
    | { kind: "history" };

  let { source = null, dim = false, compact = false }: { source?: Source | null; dim?: boolean; compact?: boolean } = $props();

  function shortFilename(p: string): string {
    const i = p.lastIndexOf("/");
    return i >= 0 ? p.slice(i + 1) : p;
  }
</script>

{#if source}
  <span class="pill" class:dim class:compact data-kind={source.kind}>
    {#if source.kind === "selection"}
      <span class="ic">✂︎</span>
      <span class="lbl">selection{source.app_name ? ` · ${source.app_name}` : ""}</span>
    {:else if source.kind === "active_document"}
      <span class="ic">📄</span>
      <span class="lbl">{source.app_name} document</span>
    {:else if source.kind === "webpage"}
      <span class="ic">🌐</span>
      <span class="lbl">{source.title || source.app_name}{compact ? "" : " · webpage"}</span>
    {:else if source.kind === "ocr"}
      <span class="ic">👁</span>
      <span class="lbl">screen ocr{source.app_name ? ` · ${source.app_name}` : ""}</span>
    {:else if source.kind === "manual"}
      <span class="ic">⌨︎</span>
      <span class="lbl">manual</span>
    {:else if source.kind === "clipboard"}
      <span class="ic">📋</span>
      <span class="lbl">clipboard</span>
    {:else if source.kind === "file"}
      <span class="ic">📑</span>
      <span class="lbl">.{source.extension} · {shortFilename(source.path)}</span>
    {:else if source.kind === "history"}
      <span class="ic">↺</span>
      <span class="lbl">replay</span>
    {/if}
  </span>
{/if}

<style>
  .pill {
    display: inline-flex; align-items: center; gap: 6px;
    padding: 3px 9px 3px 7px;
    background: var(--surface);
    border: 2px solid var(--ink-900);
    border-radius: 999px;
    font-size: 11px; font-weight: 700; color: var(--ink-900);
    box-shadow: 1.5px 1.5px 0 var(--ink-900);
    line-height: 1.3;
    max-width: 100%;
  }
  .pill .ic { font-size: 12px; line-height: 1; }
  .pill .lbl {
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
    max-width: 24ch;
  }
  .pill.dim { opacity: 0.7; box-shadow: 1px 1px 0 var(--ink-900); }
  .pill.compact { padding: 2px 7px; font-size: 10px; }
  .pill[data-kind="selection"] { background: var(--cream-200); }
  .pill[data-kind="active_document"] { background: var(--mint-300); }
  .pill[data-kind="webpage"] { background: var(--accent-3, var(--lavender-300)); background: #8ce0ff; }
  .pill[data-kind="ocr"] { background: var(--cream-300); }
  .pill[data-kind="manual"] { background: var(--surface); }
  .pill[data-kind="clipboard"] { background: var(--lavender-300); }
  .pill[data-kind="file"] { background: var(--pink-300); }
  .pill[data-kind="history"] { background: var(--cream-200); }
</style>
