<script lang="ts">
  import { onMount } from "svelte";
  import { listCredits, listLicenses, type Credit, type LicenseDoc } from "$lib/ipc";

  let { open = false, onClose }: { open: boolean; onClose: () => void } = $props();

  let credits: Credit[] = $state([]);
  let licenses: LicenseDoc[] = $state([]);
  let activeTab: "credits" | "license" = $state("credits");
  let activeLicense: LicenseDoc | null = $state(null);

  $effect(() => {
    if (open && credits.length === 0) {
      listCredits().then((c) => (credits = c));
      listLicenses().then((l) => {
        licenses = l;
        if (l.length > 0) activeLicense = l[0];
      });
    }
  });
</script>

{#if open}
  <div class="overlay" onclick={onClose} role="dialog" tabindex="-1">
    <div class="sheet" onclick={(e) => e.stopPropagation()}>
      <header>
        <h2>credits &amp; licenses</h2>
        <button class="close" onclick={onClose} aria-label="close">×</button>
      </header>
      <div class="tabs">
        <button class:active={activeTab === "credits"} onclick={() => (activeTab = "credits")}>credits</button>
        <button class:active={activeTab === "license"} onclick={() => (activeTab = "license")}>license texts</button>
      </div>

      {#if activeTab === "credits"}
        <div class="body credits-body">
          <p class="intro">yappy stands on the shoulders of these incredible people and projects.</p>
          <ul class="credits-list">
            {#each credits as c}
              <li class="credit-row">
                <div class="cr-head">
                  <span class="cr-name">{c.name}</span>
                  <span class="cr-kind">{c.kind}</span>
                  <span class="cr-license">{c.license}</span>
                  {#if c.version && c.version !== "—"}<span class="cr-ver">{c.version}</span>{/if}
                </div>
                <p class="cr-note">{c.note}</p>
                <a href={c.url} class="cr-link" target="_blank" rel="noopener">{c.url}</a>
              </li>
            {/each}
          </ul>
        </div>
      {:else}
        <div class="body license-body">
          <aside class="license-list">
            {#each licenses as l}
              <button class:active={activeLicense === l} onclick={() => (activeLicense = l)}>{l.name}</button>
            {/each}
          </aside>
          <pre class="license-text">{activeLicense?.text ?? ""}</pre>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed; inset: 0; z-index: 320;
    background: rgba(26, 26, 26, 0.5);
    display: flex; align-items: center; justify-content: center;
    backdrop-filter: blur(6px);
  }
  .sheet {
    width: 740px; max-width: 96vw; max-height: 86vh; display: flex; flex-direction: column;
    background: var(--cream-100);
    border: 2.5px solid var(--ink-900);
    border-radius: 22px;
    box-shadow: var(--shadow-hd-3);
    padding: 18px 22px 20px;
    color: var(--ink-900);
    animation: pop 0.24s var(--ease-emph);
  }
  @keyframes pop { from { transform: scale(0.94); opacity: 0; } to { transform: scale(1); opacity: 1; } }
  header { display: flex; justify-content: space-between; align-items: center; }
  h2 { font-family: var(--font-display); font-size: 30px; margin: 0; color: var(--pink-600); font-weight: 400; }
  .close {
    width: 32px; height: 32px; border-radius: 999px;
    background: var(--surface); border: 2px solid var(--ink-900);
    font-size: 18px; font-weight: 700; box-shadow: 2px 2px 0 var(--ink-900);
  }
  .close:hover { background: var(--pink-300); }
  .tabs { display: flex; gap: 6px; margin: 14px 0 12px; padding: 4px; background: var(--cream-200); border: 2.5px solid var(--ink-900); border-radius: 999px; align-self: flex-start; box-shadow: 2px 2px 0 var(--ink-900); }
  .tabs button { padding: 6px 14px; font-size: 13px; font-weight: 700; border-radius: 999px; color: var(--ink-700); }
  .tabs button:hover { color: var(--ink-900); }
  .tabs button.active { background: var(--ink-900); color: var(--cream-100); }

  .body { overflow-y: auto; flex: 1; }
  .intro { color: var(--ink-500); margin: 0 0 14px; font-weight: 500; font-size: 14px; }
  .credits-list { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: 10px; }
  .credit-row {
    background: var(--surface); border: 2px solid var(--ink-900); border-radius: 14px;
    padding: 12px 14px; box-shadow: 2px 2px 0 var(--ink-900);
  }
  .cr-head { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; }
  .cr-name { font-weight: 700; font-size: 15px; }
  .cr-kind, .cr-license, .cr-ver {
    font-size: 10px; font-weight: 700; padding: 2px 7px; border-radius: 999px;
    background: var(--cream-200); border: 1.5px solid var(--ink-900); color: var(--ink-700);
  }
  .cr-kind { background: var(--lavender-300); }
  .cr-license { background: var(--mint-300); }
  .cr-note { color: var(--ink-700); font-size: 12px; line-height: 1.45; margin: 6px 0 4px; font-weight: 500; }
  .cr-link { font-family: var(--font-mono); font-size: 11px; color: var(--pink-600); font-weight: 700; word-break: break-all; }
  .cr-link:hover { text-decoration: underline; }

  .license-body { display: grid; grid-template-columns: 200px 1fr; gap: 12px; min-height: 360px; }
  .license-list { display: flex; flex-direction: column; gap: 6px; padding-right: 6px; overflow-y: auto; }
  .license-list button {
    padding: 8px 10px; text-align: left;
    background: var(--surface); border: 2px solid var(--ink-900); border-radius: 10px;
    box-shadow: 2px 2px 0 var(--ink-900);
    font-size: 12px; font-weight: 700; color: var(--ink-900);
  }
  .license-list button:hover { background: var(--cream-200); }
  .license-list button.active { background: var(--pink-300); }
  .license-text {
    margin: 0; padding: 14px;
    background: var(--cream-50);
    border: 2px solid var(--ink-900); border-radius: 12px;
    box-shadow: 2px 2px 0 var(--ink-900);
    font-family: var(--font-mono); font-size: 11px; line-height: 1.5;
    color: var(--ink-700); white-space: pre-wrap; word-wrap: break-word;
    overflow-y: auto; max-height: 50vh;
  }
</style>
