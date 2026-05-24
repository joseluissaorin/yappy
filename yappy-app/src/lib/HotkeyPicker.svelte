<script lang="ts">
  let { value = "", onSave }: { value: string; onSave: (combo: string) => void | Promise<void> } = $props();

  let recording = $state(false);
  let buffer = $state<string[]>([]);
  let error = $state<string | null>(null);

  function formatCombo(parts: string[]): string {
    return parts.join("+");
  }

  function start() {
    buffer = [];
    error = null;
    recording = true;
  }

  function onKey(e: KeyboardEvent) {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "Escape") {
      recording = false;
      buffer = [];
      return;
    }
    const parts: string[] = [];
    if (e.metaKey) parts.push("cmd");
    if (e.altKey) parts.push("alt");
    if (e.ctrlKey) parts.push("ctrl");
    if (e.shiftKey) parts.push("shift");
    const key = e.key.toLowerCase();
    // Ignore modifier-only presses; wait for a real key.
    if (["meta", "alt", "control", "shift", "dead"].includes(key)) {
      buffer = parts;
      return;
    }
    const named: Record<string, string> = {
      " ": "space",
      arrowleft: "left",
      arrowright: "right",
      arrowup: "up",
      arrowdown: "down",
    };
    const normalized = named[key] ?? key;
    parts.push(normalized);
    if (parts.length < 2) {
      error = "use at least one modifier (cmd / alt / ctrl / shift)";
      buffer = parts;
      return;
    }
    const combo = formatCombo(parts);
    buffer = parts;
    recording = false;
    try {
      const r = onSave(combo);
      if (r instanceof Promise) r.catch((err) => (error = String(err)));
    } catch (err) {
      error = String(err);
    }
  }

  function prettify(c: string): string {
    return c
      .split("+")
      .map((p) => {
        switch (p) {
          case "cmd": return "⌘";
          case "alt": return "⌥";
          case "ctrl": return "⌃";
          case "shift": return "⇧";
          case "space": return "space";
          case "left": return "←";
          case "right": return "→";
          case "up": return "↑";
          case "down": return "↓";
          default: return p.toUpperCase();
        }
      })
      .join(" + ");
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="hk-picker">
  {#if recording}
    <div class="hk-rec">
      <span class="hk-dot"></span>
      <span class="hk-text">{buffer.length ? prettify(buffer.join("+")) : "press your shortcut…"}</span>
      <button class="btn-ghost tiny" onclick={() => (recording = false)}>cancel</button>
    </div>
  {:else}
    <button class="hk-display" onclick={start}>{prettify(value)}</button>
    <button class="btn-ghost tiny" onclick={start} title="record new combo">change</button>
  {/if}
</div>
{#if error}<div class="hk-err">{error}</div>{/if}

<style>
  .hk-picker { display: inline-flex; align-items: center; gap: 8px; }
  .hk-display {
    padding: 6px 12px;
    background: var(--surface);
    border: 2.5px solid var(--ink-900);
    border-radius: 12px;
    font-family: var(--font-mono);
    font-size: 12px; font-weight: 700; color: var(--ink-900);
    box-shadow: 2px 2px 0 var(--ink-900);
    transition: transform 0.12s var(--ease-emph);
  }
  .hk-display:hover { transform: translate(-1px, -1px); box-shadow: 3px 3px 0 var(--ink-900); }
  .hk-rec {
    display: inline-flex; align-items: center; gap: 10px;
    padding: 6px 12px;
    background: var(--pink-300);
    border: 2.5px solid var(--pink-600);
    border-radius: 12px;
    box-shadow: 2px 2px 0 var(--ink-900);
  }
  .hk-dot {
    display: inline-block; width: 10px; height: 10px; border-radius: 50%;
    background: var(--pink-600); animation: blink 0.9s ease-in-out infinite;
  }
  @keyframes blink { 0%, 100% { opacity: 1; } 50% { opacity: 0.3; } }
  .hk-text { font-family: var(--font-mono); font-weight: 700; font-size: 12px; }
  .btn-ghost.tiny { font-size: 11px; padding: 3px 8px; border-radius: 8px; font-weight: 700; color: var(--ink-700); background: transparent; }
  .btn-ghost.tiny:hover { background: var(--cream-200); }
  .hk-err { margin-top: 4px; color: var(--danger, #d9534f); font-size: 11px; font-weight: 600; }
</style>
