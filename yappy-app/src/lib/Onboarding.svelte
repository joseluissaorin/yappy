<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
  import { openBrowserExtensions } from "$lib/ipc";
  import { isIOS, ready as platformReady } from "$lib/platform";

  let { open = false, onDone }: { open: boolean; onDone: () => void } = $props();

  // Resolve platform once so the template can switch on it. On iOS the
  // browser-extension flow doesn't apply (sandboxed UIKit apps can't pair
  // with desktop browsers) — we show a single welcome step and dismiss.
  let mobileOnboarding = $state(false);
  platformReady.then(() => {
    isIOS.subscribe((v) => (mobileOnboarding = v));
  });

  let step: number = $state(1);
  let pairedBrowsers: string[] = $state([]);
  let unlisteners: UnlistenFn[] = [];

  async function refreshStatus() {
    try {
      const b: any = await invoke("bridge_status");
      pairedBrowsers = (b?.connections || []).map((c: any) => c.browser);
    } catch {}
  }

  function revealExtensionFolder() {
    // The extension folder is shipped alongside the binary in production builds, and
    // lives at <repo>/extension/chromium in dev. We open it in Finder.
    const dev = "/Users/joseluissaorin/Dropbox/Jose Luis Hijo/Dev/Yappy/extension/chromium";
    revealItemInDir(dev).catch(() => openPath(dev).catch(() => {}));
  }

  function openExtensions(name: string) {
    openBrowserExtensions(name).catch((e) => console.warn("open extensions:", e));
  }
  const BROWSERS = ["Vivaldi", "Google Chrome", "Brave Browser", "Microsoft Edge", "Arc"];

  onMount(async () => {
    await refreshStatus();
    unlisteners.push(await listen<string>("bridge_paired", async (e) => {
      if (!pairedBrowsers.includes(e.payload)) pairedBrowsers = [...pairedBrowsers, e.payload];
      // Auto-advance to "done" when the first browser pairs.
      if (step === 2) step = 3;
      await refreshStatus();
    }));
    unlisteners.push(await listen<string>("bridge_disconnected", () => refreshStatus()));
  });
  onDestroy(() => unlisteners.forEach((u) => u()));
</script>

{#if open}
  <div class="overlay" onclick={onDone} role="dialog" tabindex="-1">
    <div class="sheet" onclick={(e) => e.stopPropagation()}>
      <header>
        <div class="stepper">
          <span class="dot" class:on={step >= 1}></span>
          <span class="dot" class:on={step >= 2}></span>
          <span class="dot" class:on={step >= 3}></span>
        </div>
        <button class="skip" onclick={onDone}>skip for now</button>
      </header>

      {#if step === 1}
        <div class="step welcome">
          <h2>welcome to yappy</h2>
          {#if mobileOnboarding}
            <p>local, friendly text-to-speech for your phone. open a document, paste any text, or share an article from any app — yappy reads it aloud in 31 languages, on-device.</p>
            <div class="buttons">
              <button class="btn-pink" onclick={onDone}>get started →</button>
            </div>
          {:else}
            <p>local, friendly text-to-speech for your mac. press <kbd>⌥⌘R</kbd> anywhere to hear what you're looking at, in 31 languages, on-device.</p>
            <div class="buttons">
              <button class="btn-pink" onclick={() => (step = 2)}>set up browsers →</button>
              <button class="btn-outline" onclick={onDone}>i'll do this later</button>
            </div>
          {/if}
        </div>
      {:else if step === 2}
        <div class="step extension">
          <h2>install the browser extension</h2>
          <p>
            so yappy can read any article — paywalls, navigation, ads stripped out — straight from your browser.
            two clicks, no token to copy.
          </p>
          <div class="install-row">
            <button class="btn-pink" onclick={revealExtensionFolder}>
              <svg width="14" height="14" viewBox="0 0 14 14" fill="currentColor"><path d="M1 3.5 C1 3 1.4 2.5 2 2.5 H5.5 L6.5 3.5 H12 C12.5 3.5 13 4 13 4.5 V11.5 C13 12 12.5 12.5 12 12.5 H2 C1.4 12.5 1 12 1 11.5 Z"/></svg>
              reveal extension folder
            </button>
          </div>
          <ol class="steps">
            <li>
              open your browser's extensions page —
              <div class="browser-buttons">
                {#each BROWSERS as b}
                  <button class="browser-btn" onclick={() => openExtensions(b)}>{b}</button>
                {/each}
              </div>
            </li>
            <li>turn on <em>Developer mode</em> in the top right</li>
            <li>click <em>Load unpacked</em> → drag the folder we just revealed</li>
          </ol>
          <p class="hint">yappy is listening. as soon as the extension is loaded it pairs itself — you'll see the chip below light up.</p>
          <div class="paired-status">
            {#if pairedBrowsers.length > 0}
              <span class="paired-pill">✓ paired with <strong>{pairedBrowsers.join(", ")}</strong></span>
            {:else}
              <span class="paired-pill waiting">
                <span class="pulse"></span>
                waiting for the extension…
              </span>
            {/if}
          </div>
          <div class="buttons">
            <button class="btn-outline" onclick={() => (step = 1)}>← back</button>
            {#if pairedBrowsers.length > 0}
              <button class="btn-pink" onclick={() => (step = 3)}>let's go →</button>
            {:else}
              <button class="btn-outline" onclick={() => (step = 3)}>skip this</button>
            {/if}
          </div>
        </div>
      {:else if step === 3}
        <div class="step done">
          <h2>you're set</h2>
          <p class="big">
            press <kbd>⌥</kbd><kbd>⌘</kbd><kbd>R</kbd> anywhere — yappy reads whatever you're looking at.
          </p>
          {#if pairedBrowsers.length > 0}
            <p class="hint">
              connected to <strong>{pairedBrowsers.join(", ")}</strong>. when you hit the hotkey on a tab,
              yappy asks the extension for the clean article — no ocr, no permissions, no clipboard juggling.
            </p>
          {/if}
          <button class="btn-pink big" onclick={onDone}>start using yappy</button>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed; inset: 0; z-index: 320;
    background: rgba(26, 26, 26, 0.45);
    display: flex; align-items: center; justify-content: center;
    backdrop-filter: blur(6px);
    animation: in 0.18s ease;
  }
  @keyframes in { from { opacity: 0; } to { opacity: 1; } }
  .sheet {
    width: 560px; max-width: 96vw;
    background: var(--cream-100);
    border: 2.5px solid var(--ink-900);
    border-radius: 22px;
    box-shadow: var(--shadow-hd-3);
    padding: 20px 28px 24px;
    color: var(--ink-900);
    animation: pop 0.22s var(--ease-emph);
  }
  @keyframes pop { from { transform: scale(0.95); opacity: 0; } to { transform: scale(1); opacity: 1; } }
  header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 10px; }
  .stepper { display: flex; gap: 6px; }
  .stepper .dot { width: 10px; height: 10px; border-radius: 50%; border: 2px solid var(--ink-900); background: var(--cream-200); }
  .stepper .dot.on { background: var(--pink-500); }
  .skip { background: transparent; border: none; color: var(--ink-500); font-size: 12px; font-weight: 700; }
  .skip:hover { color: var(--ink-900); }
  .step { padding: 8px 0 4px; }
  .step h2 { font-family: var(--font-display); font-size: 34px; margin: 0 0 8px; color: var(--pink-600); font-weight: 400; transform: rotate(-1deg); }
  .step p { color: var(--ink-700); font-weight: 500; font-size: 14px; line-height: 1.55; margin: 0 0 14px; }
  .step p.big { font-size: 18px; }
  .step .hint { font-size: 12px; color: var(--ink-500); margin: 4px 0 0; }
  .buttons { display: flex; gap: 10px; justify-content: space-between; align-items: center; margin-top: 18px; flex-wrap: wrap; }
  .install-row { margin: 10px 0 14px; }
  .install-row .btn-pink { display: inline-flex; align-items: center; gap: 8px; }
  ol.steps { padding-left: 18px; margin: 12px 0; }
  ol.steps li { color: var(--ink-700); font-size: 13px; font-weight: 500; line-height: 1.6; }
  ol.steps code { background: var(--cream-200); border-radius: 6px; padding: 1px 6px; font-family: var(--font-mono); font-size: 11px; }
  .browser-buttons { display: inline-flex; flex-wrap: wrap; gap: 6px; margin-top: 6px; }
  .browser-btn {
    padding: 3px 10px; font-size: 12px; font-weight: 700;
    background: var(--surface); border: 2px solid var(--ink-900); border-radius: 999px;
    color: var(--ink-900); cursor: pointer;
    box-shadow: 1.5px 1.5px 0 var(--ink-900);
    transition: transform 0.1s ease;
  }
  .browser-btn:hover { background: var(--pink-300); transform: translate(-1px, -1px); box-shadow: 2.5px 2.5px 0 var(--ink-900); }
  .paired-status { margin-top: 14px; padding: 8px 0; border-top: 2px dashed var(--ink-300); }
  .paired-pill {
    display: inline-flex; align-items: center; gap: 6px;
    padding: 6px 14px; border-radius: 999px;
    background: var(--mint-300); border: 2px solid var(--ink-900);
    font-size: 13px; font-weight: 700; color: var(--ink-900);
    box-shadow: 2px 2px 0 var(--ink-900);
  }
  .paired-pill.waiting { background: var(--cream-200); }
  .pulse { display: inline-block; width: 8px; height: 8px; border-radius: 999px; background: var(--ink-900); animation: pulse 1.05s ease-in-out infinite; }
  @keyframes pulse { 0%, 100% { opacity: 0.25; } 50% { opacity: 1; } }
  .step.done { text-align: center; padding: 18px 0; }
  .step.done h2 { margin-bottom: 14px; }
  .btn-pink.big { font-size: 16px; padding: 14px 28px; }
  kbd { font-family: var(--font-mono); font-size: 13px; padding: 2px 8px; background: var(--surface); border: 2px solid var(--ink-900); border-radius: 7px; box-shadow: 2px 2px 0 var(--ink-900); margin: 0 2px; }
</style>
