<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { goto } from "$app/navigation";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { reader } from "$lib/readerStore.svelte";
  import YappyMascot from "$lib/YappyMascot.svelte";
  import SoundWaves from "$lib/SoundWaves.svelte";
  import SourcePill from "$lib/SourcePill.svelte";
  import HistoryList from "$lib/HistoryList.svelte";
  import { isIOS } from "$lib/platform";
  import { goPage } from "$lib/nav";
  import { notifyError } from "$lib/ui";
  import {
    type Voice,
    type Settings,
    type PlaybackSnapshot,
    type CaptureInfo,
    type DownloadProgress,
    type BridgeStatus,
    downloadModel,
    isModelReady,
    getSettings,
    listVoices,
    setVoice,
    sampleVoice,
    readNow,
    readClipboard,
    stopPlayback,
    synthesizeText,
    readFile,
    readDocument,
    sampleDocumentPath,
    bridgeStatus,
    onPlaybackState,
    onCaptureInfo,
    onCaptureEmpty,
    onCaptureProgress,
    onModelDownload,
    onModelMissing,
    onBridgePaired,
    onBridgeDisconnected,
    onBridgeTokenChanged,
  } from "$lib/ipc";

  let voices: Voice[] = $state([]);
  let settings: Settings | null = $state(null);
  let modelReady = $state(false);
  let download: DownloadProgress | null = $state(null);
  let downloading = $state(false);
  let playback = $state<PlaybackSnapshot | null>(null);
  let captureInfo: CaptureInfo | null = $state(null);
  let captureStage = $state("");
  let bridge: BridgeStatus | null = $state(null);
  let captureEmptyToast = $state(false);
  let clipboardCandidate: string | null = $state(null);
  let testText = $state(
    "Paste or type anything here and Yappy reads it aloud — on your device, in 31 languages, without sending a single word to the cloud.",
  );
  let testing = $state(false);
  let testTextOpen = $state(false);

  const isPlaying = $derived(!!playback?.playing && !playback?.paused);
  const isThinking = $derived(captureStage === "thinking");

  let cleanups: (() => void)[] = [];

  onMount(async () => {
    voices = await listVoices();
    settings = await getSettings();
    modelReady = await isModelReady();
    try { bridge = await bridgeStatus(); } catch {}

    cleanups.push(await onPlaybackState((s) => (playback = s)));
    cleanups.push(await onCaptureInfo((info) => { captureInfo = info; captureStage = ""; }));
    cleanups.push(await onCaptureProgress((stage) => (captureStage = stage)));
    cleanups.push(await onCaptureEmpty(() => {
      captureEmptyToast = true;
      setTimeout(() => (captureEmptyToast = false), 3000);
    }));
    cleanups.push(await onModelDownload((p) => (download = p)));
    cleanups.push(await onModelMissing(() => (modelReady = false)));
    cleanups.push(await onBridgePaired(async () => { try { bridge = await bridgeStatus(); } catch {} }));
    cleanups.push(await onBridgeDisconnected(async () => { try { bridge = await bridgeStatus(); } catch {} }));
    cleanups.push(await onBridgeTokenChanged(async () => { try { bridge = await bridgeStatus(); } catch {} }));

    // iOS: probe the clipboard for fresh content to offer at launch.
    if ($isIOS) {
      try {
        const { readText } = await import("@tauri-apps/plugin-clipboard-manager");
        const text = ((await readText()) ?? "").trim();
        if (text && text.length > 12 && text.length < 50_000) {
          const dismissed = localStorage.getItem("yappy:clipboard:dismissed");
          if (dismissed !== text) clipboardCandidate = text;
        }
      } catch (e) { console.warn("[clipboard] read failed:", e); }
    }
  });
  onDestroy(() => cleanups.forEach((c) => c()));

  async function startDownload() {
    downloading = true;
    try {
      await downloadModel();
      modelReady = await isModelReady();
    } catch (e) { notifyError(String(e)); }
    finally { downloading = false; }
  }
  async function pickVoice(v: Voice) {
    if (!settings) return;
    settings = { ...settings, voice: v.name };
    await setVoice(v.name);
  }
  async function playVoiceSample(v: Voice, e?: Event) {
    e?.stopPropagation();
    if (modelReady) await sampleVoice(v.name);
  }
  async function runTest() {
    testing = true;
    try {
      captureInfo = { source: { kind: "manual" } };
      await synthesizeText(testText);
    } catch (e) { notifyError(String(e)); }
    finally { setTimeout(() => (testing = false), 1500); }
  }
  // Open a document: iOS renders it in the immersive in-page reader (/read);
  // desktop opens its full editor window.
  async function openDoc(path: string) {
    if ($isIOS) {
      reader.doc = await readDocument(path);
      await goto("/read");
    } else {
      await readFile(path);
    }
  }
  async function openFile() {
    try {
      const path = await openDialog({
        multiple: false,
        filters: [{ name: "Documents", extensions: ["txt", "md", "markdown", "rtf", "docx", "doc", "odt", "pdf", "epub", "html", "htm"] }],
      });
      if (typeof path === "string") await openDoc(path);
    } catch (e) { notifyError(String(e)); }
  }
  async function acceptClipboardCandidate() {
    if (!clipboardCandidate) return;
    const text = clipboardCandidate;
    clipboardCandidate = null;
    await synthesizeText(text);
  }
  function dismissClipboardCandidate() {
    if (clipboardCandidate) {
      try { localStorage.setItem("yappy:clipboard:dismissed", clipboardCandidate); } catch {}
    }
    clipboardCandidate = null;
  }

  function downloadPercent(): number {
    if (!download || download.overall_total === 0) return 0;
    return Math.min(100, Math.round((download.overall_done / download.overall_total) * 100));
  }
  function downloadDoneMB(): number {
    return download ? Math.round((download.overall_done / 1024 / 1024) * 10) / 10 : 0;
  }
  function downloadTotalMB(): number {
    return download ? Math.round((download.overall_total / 1024 / 1024) * 10) / 10 : 0;
  }
</script>

{#if !modelReady}
  <!-- Cold start: model isn't installed. Make download the singular path. -->
  <section class="block-row">
    <div class="card model-card">
      <div class="model-card-head">
        <div>
          <div class="model-title">install voices</div>
          <div class="model-sub">one-time download · about <strong>{downloadTotalMB() || 380} mb</strong> · then offline forever.</div>
        </div>
        <button class="btn-pink" onclick={startDownload} disabled={downloading}>
          {#if downloading}downloading…{:else}download voices{/if}
        </button>
      </div>
      {#if downloading || download}
        <div class="progress-row">
          <div class="progress-track"><div class="progress-bar" style="--w: {downloadPercent()}%"></div></div>
          <div class="progress-stats">
            <span>{downloadDoneMB()} mb / {downloadTotalMB()} mb</span>
            <span class="progress-pct">{downloadPercent()}%</span>
          </div>
          {#if download}<div class="progress-file">{download.file}</div>{/if}
        </div>
      {/if}
    </div>

    {#if $isIOS}
      <div class="card tip-card">
        <div class="tip-icon">📤</div>
        <div class="tip-body">
          <div class="tip-title">how to read anything aloud</div>
          <div class="tip-sub">in any iOS app — Safari, Notes, Mail, Messages — tap the share button, then choose <strong>Yappy</strong>. the article or selection gets read aloud here.</div>
        </div>
      </div>
    {/if}

    {#if clipboardCandidate}
      <div class="card clipboard-banner">
        <div class="clip-icon">📋</div>
        <div class="clip-body">
          <div class="clip-title">read your clipboard?</div>
          <div class="clip-preview">{clipboardCandidate.slice(0, 120)}{clipboardCandidate.length > 120 ? "…" : ""}</div>
        </div>
        <div class="clip-actions">
          <button class="btn-pink" onclick={acceptClipboardCandidate} aria-label="read clipboard content aloud">read it</button>
          <button class="btn-outline" onclick={dismissClipboardCandidate} aria-label="dismiss clipboard banner">no thanks</button>
        </div>
      </div>
    {/if}
  </section>
{:else if $isIOS}
  <!-- ── MOBILE HOME: designed around what iOS can actually do — type/paste
       text, open a document, transcribe audio, read the clipboard, or share
       content in. No desktop hotkey / screen-capture concepts. ─────────────── -->
  <section class="m-hero">
    <div class="m-brand"><YappyMascot size={56} happy /></div>
    <h1 class="m-title">read anything aloud</h1>
    <p class="m-sub">type or paste text — Yappy reads it on-device in 31 languages.</p>
    <div class="card m-compose">
      <textarea
        bind:value={testText}
        rows="5"
        placeholder="paste or type anything here…"
        aria-label="text to read aloud"
      ></textarea>
      <div class="m-compose-actions">
        <button class="btn-pink" onclick={() => runTest()} disabled={testing || !testText.trim()}>
          {testing ? "reading…" : "read it aloud"}
        </button>
        <button class="btn-outline" onclick={() => (testText = "")} disabled={!testText.trim()}>clear</button>
      </div>
    </div>
  </section>

  <div class="m-tiles">
    <button class="m-tile" onclick={() => readClipboard()}>
      <span class="m-tile-ico">📋</span><span class="m-tile-label">read clipboard</span>
    </button>
    <button class="m-tile" onclick={openFile}>
      <span class="m-tile-ico">📄</span><span class="m-tile-label">open a document</span>
    </button>
    <button class="m-tile" onclick={() => goPage("transcribe")}>
      <span class="m-tile-ico">🎙️</span><span class="m-tile-label">transcribe audio</span>
    </button>
  </div>

  <div class="m-samplelink">
    <button class="link" onclick={async () => { try { await openDoc(await sampleDocumentPath()); } catch (e) { notifyError(String(e)); } }}>
      open a sample document →
    </button>
  </div>

  <section class="quickvoices">
    <div class="qv-head">
      <h2>voice</h2>
      <button class="link" onclick={() => goPage("voices")}>all 10 →</button>
    </div>
    <div class="qv-row">
      {#each voices as v}
        <button class="voice-pill" class:active={settings?.voice === v.name} onclick={() => pickVoice(v)} title={v.description}>
          <span class="dot" data-id={v.id}></span>
          {v.name}
        </button>
      {/each}
    </div>
  </section>

  <div class="card tip-card">
    <div class="tip-icon">📤</div>
    <div class="tip-body">
      <div class="tip-title">share into Yappy</div>
      <div class="tip-sub">in any app — Safari, Notes, WhatsApp — tap the share button, then <strong>Yappy</strong>. articles get read aloud; voice notes get transcribed.</div>
    </div>
  </div>

  <section class="recents-v3">
    <div class="qv-head">
      <h2>recent</h2>
      <button class="link" onclick={() => goPage("history")}>all →</button>
    </div>
    <HistoryList compact={true} max={5} />
  </section>
{:else}
  <section class="read-panel" class:playing={isPlaying} class:paused={playback?.paused} class:thinking={isThinking}>
    <div class="read-panel-status">
      <span class="rp-status">
        <span class="rp-dot" class:on={modelReady}></span>
        {modelReady ? "voices ready" : "model not installed"}
      </span>
      <span class="rp-sep">·</span>
      <button class="rp-voice" onclick={() => goPage("voices")} title="change voice">
        <span class="voice-dot" data-id={voices.find(v => v.name === settings?.voice)?.id || "F1"}></span>
        {settings?.voice ?? "—"}
      </button>
      <span class="rp-sep">·</span>
      <button class="rp-bridge" onclick={() => goPage("preferences")} class:on={bridge && bridge.connections.length > 0}>
        {#if bridge && bridge.connections.length > 0}
          🌐 {bridge.connections.length} browser{bridge.connections.length === 1 ? "" : "s"}
        {:else}
          🌐 extension setup
        {/if}
      </button>
    </div>

    {#if isThinking}
      <div class="rp-state-line">
        <span class="thinking-chip">
          <span class="dots"><i></i><i></i><i></i></span>
          figuring out what to read…
        </span>
      </div>
    {:else if isPlaying || playback?.paused}
      <div class="rp-state-line">
        {#if captureInfo?.source}
          <span class="rp-label">{playback?.paused ? "paused" : "reading"}</span>
          <SourcePill source={captureInfo.source} compact />
        {:else}
          <span class="rp-label">{playback?.paused ? "paused" : "reading"}</span>
        {/if}
        <SoundWaves active={isPlaying} height={14} bars={9} />
      </div>
    {:else if captureInfo?.source}
      <div class="rp-state-line subtle">
        <span class="rp-label">last read</span>
        <SourcePill source={captureInfo.source} compact />
      </div>
    {/if}

    <div class="rp-cta">
      <button class="rp-primary" onclick={() => readNow()} aria-label={isPlaying && !playback?.paused ? "currently reading aloud, tap to pause" : playback?.paused ? "resume reading" : "read what I am looking at"}>
        <span class="rp-primary-icon" aria-hidden="true"><YappyMascot size={64} talking={isPlaying} /></span>
        <span class="rp-primary-text">
          <span class="rp-primary-title">
            {#if isPlaying && !playback?.paused}reading aloud…
            {:else if playback?.paused}resume reading
            {:else}read what i'm looking at
            {/if}
          </span>
          <span class="rp-primary-sub">
            <kbd>⌥</kbd><kbd>⌘</kbd><kbd>R</kbd>
            <span>selection · screenshot · active document · paired browser</span>
          </span>
        </span>
      </button>
      {#if isPlaying || playback?.paused}
        <button class="rp-stop" onclick={() => stopPlayback()} title="stop reading (Esc)" aria-label="stop reading">
          <svg width="14" height="14" viewBox="0 0 14 14" fill="currentColor" aria-hidden="true"><rect x="2" y="2" width="10" height="10" rx="2"/></svg>
          stop
        </button>
      {/if}
    </div>

    <div class="rp-secondary">
      <button class="rp-sec" onclick={() => readClipboard()} title="read whatever's on your clipboard">
        <span class="emoji">📋</span> clipboard
        <kbd class="hint">⌥⌘V</kbd>
      </button>
      <button class="rp-sec" onclick={openFile} title="open a .pdf / .docx / .epub / .md / .txt …">
        <span class="emoji">📄</span> open a document
      </button>
      <button class="rp-sec" onclick={() => testTextOpen = !testTextOpen} class:active={testTextOpen}>
        <span class="emoji">✎</span> paste text
      </button>
    </div>

    {#if testTextOpen}
      <div class="rp-paste">
        <textarea
          bind:value={testText}
          placeholder="paste or type anything — 31 languages, auto-detected"
          rows="3"
        ></textarea>
        <div class="rp-paste-actions">
          <button class="btn-pink small" onclick={() => runTest()} disabled={testing || !testText.trim()}>
            {testing ? "reading…" : "read this"}
          </button>
          <button class="btn-outline small" onclick={() => { testText = ""; testTextOpen = false; }}>clear</button>
        </div>
      </div>
    {/if}
  </section>

  <section class="recents-v3">
    <div class="qv-head">
      <h2>recent</h2>
      <button class="link" onclick={() => goPage("history")}>all →</button>
    </div>
    <HistoryList compact={true} max={5} />
  </section>

  <section class="quickvoices">
    <div class="qv-head">
      <h2>switch voice</h2>
      <button class="link" onclick={() => goPage("voices")}>browse all 10 →</button>
    </div>
    <div class="qv-row">
      {#each voices as v}
        <button class="voice-pill" class:active={settings?.voice === v.name} onclick={() => pickVoice(v)} title={v.description}>
          <span class="dot" data-id={v.id}></span>
          {v.name}
        </button>
      {/each}
    </div>
  </section>
{/if}

{#if captureEmptyToast}<div class="toast">no text found — try selecting first, or focus a supported app.</div>{/if}

<style>
  /* ── Mobile home (iOS) ─────────────────────────────────────────────── */
  .m-hero { margin-bottom: 18px; }
  .m-brand { margin-bottom: 2px; }
  .m-title {
    font-family: var(--font-display); font-size: 34px; font-weight: 400;
    color: var(--pink-600); margin: 0 0 4px; transform: rotate(-1deg);
  }
  .m-sub { color: var(--ink-700); font-weight: 600; font-size: 14px; margin: 0 0 14px; }
  .m-compose { padding: 14px; }
  .m-compose textarea {
    width: 100%; box-sizing: border-box; resize: vertical; min-height: 120px;
    border: 2px solid var(--ink-300); border-radius: 14px; padding: 12px 14px;
    font-family: var(--font-sans); font-size: 16px; line-height: 1.5; color: var(--ink-900);
    background: var(--bg-2);
  }
  .m-compose textarea:focus { outline: 3px solid var(--pink-400); border-color: var(--ink-900); }
  .m-compose-actions { display: flex; gap: 10px; margin-top: 12px; }
  .m-compose-actions .btn-pink { flex: 1; }

  .m-tiles { display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 10px; margin: 18px 0; }
  .m-tile {
    display: flex; flex-direction: column; align-items: center; gap: 8px;
    padding: 16px 8px; border-radius: 16px; background: var(--bg-2);
    border: 2.5px solid var(--ink-900); box-shadow: 3px 3px 0 var(--ink-900);
    transition: transform 0.12s ease, box-shadow 0.12s ease; cursor: pointer;
  }
  .m-tile:active { transform: translate(2px, 2px); box-shadow: 1px 1px 0 var(--ink-900); }
  .m-tile-ico { font-size: 26px; }
  .m-tile-label { font-size: 12px; font-weight: 700; color: var(--ink-700); text-align: center; }
  .m-samplelink { text-align: center; margin: -4px 0 6px; }

  .m-now {
    display: flex; align-items: center; gap: 10px; margin-bottom: 14px;
    padding: 10px 14px; border-radius: 14px; background: var(--pink-300);
    border: 2.5px solid var(--ink-900); box-shadow: 3px 3px 0 var(--ink-900);
  }
  .m-now-label { font-weight: 700; color: var(--ink-900); flex: 1; }
  .m-now-stop {
    padding: 6px 14px; border-radius: 999px; background: var(--ink-900);
    color: var(--cream-100); font-weight: 700; font-size: 13px;
  }
</style>
