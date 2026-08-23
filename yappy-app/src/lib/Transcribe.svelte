<script lang="ts">
  import { onMount } from "svelte";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import TranscriptList from "$lib/TranscriptList.svelte";
  import {
    type DownloadProgress,
    type TranscriptResult,
    isAsrModelReady,
    downloadAsrModel,
    transcribeAudio,
    transcribeSample,
    onAsrModelDownload,
    onTranscribeProgress,
  } from "$lib/ipc";

  // When the parent (drag-drop / file-association) hands us an audio path, we
  // auto-transcribe it. Bumping `queuedNonce` re-triggers even for the same path.
  let { queuedPath = null, queuedNonce = 0 }: { queuedPath?: string | null; queuedNonce?: number } = $props();

  const AUDIO_EXTS = ["mp3", "m4a", "aac", "wav", "flac", "ogg", "oga", "opus", "caf", "amr"];

  let ready = $state(false);
  let checking = $state(true);
  let downloading = $state(false);
  let download: DownloadProgress | null = $state(null);
  let transcribing = $state(false);
  let stage = $state("");
  let result: TranscriptResult | null = $state(null);
  let currentName = $state("");
  let error: string | null = $state(null);
  let copied = $state(false);
  let refreshKey = $state(0);
  let lastNonce = -1;

  onMount(async () => {
    ready = await isAsrModelReady().catch(() => false);
    checking = false;
    await onAsrModelDownload((p) => (download = p));
    await onTranscribeProgress((s) => (stage = s));
  });

  $effect(() => {
    if (queuedPath && queuedNonce !== lastNonce) {
      lastNonce = queuedNonce;
      transcribe(queuedPath, "File");
    }
  });

  async function startDownload() {
    downloading = true;
    error = null;
    try {
      await downloadAsrModel();
      ready = await isAsrModelReady();
    } catch (e) {
      error = `download failed: ${e}`;
    } finally {
      downloading = false;
    }
  }

  async function pick() {
    const sel = await openDialog({
      multiple: false,
      filters: [{ name: "Audio", extensions: AUDIO_EXTS }],
    });
    if (typeof sel === "string") transcribe(sel, "File");
  }

  async function runSample() {
    if (transcribing) return;
    error = null;
    result = null;
    transcribing = true;
    stage = "transcribing";
    currentName = "sample clip";
    try {
      result = await transcribeSample();
      refreshKey++;
    } catch (e) {
      error = String(e);
    } finally {
      transcribing = false;
      stage = "";
    }
  }

  async function transcribe(path: string, source: string) {
    if (transcribing) return;
    error = null;
    result = null;
    transcribing = true;
    stage = "transcribing";
    currentName = path.split(/[/\\]/).pop() ?? path;
    try {
      result = await transcribeAudio(path, undefined, source);
      refreshKey++;
    } catch (e) {
      const msg = String(e);
      error = msg.includes("not downloaded") ? "transcription model isn't downloaded yet." : msg;
      if (msg.includes("not downloaded")) ready = false;
    } finally {
      transcribing = false;
      stage = "";
    }
  }

  async function copyResult() {
    if (!result) return;
    try {
      await navigator.clipboard.writeText(result.text);
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch {}
  }

  function pct(): number {
    if (!download || download.overall_total === 0) return 0;
    return Math.min(100, Math.round((download.overall_done / download.overall_total) * 100));
  }
  function mb(n: number): number {
    return Math.round((n / 1024 / 1024) * 10) / 10;
  }
</script>

<section class="transcribe">
  <header class="head">
    <h2>transcribe</h2>
    <p class="sub">turn any audio — a voice note, a recording — into text, on-device.</p>
  </header>

  {#if checking}
    <div class="card pad"><p class="note">checking model…</p></div>
  {:else if !ready}
    <!-- Cold start: ASR model not installed. -->
    <div class="card pad model-gate">
      <div class="model-title">get the transcription model</div>
      <div class="note">one-time download · about <strong>670 mb</strong> · Parakeet, 25 languages · then offline forever.</div>
      <button class="btn-pink" onclick={startDownload} disabled={downloading}>
        {#if downloading}downloading…{:else}download model{/if}
      </button>
      {#if downloading || download}
        <div class="progress-row">
          <div class="progress-track"><div class="progress-bar" style="width: {pct()}%"></div></div>
          <div class="progress-stats">
            <span>{mb(download?.overall_done ?? 0)} mb / {mb(download?.overall_total ?? 0)} mb</span>
            <span>{pct()}%</span>
          </div>
          {#if download}<div class="progress-file">{download.file}</div>{/if}
        </div>
      {/if}
      {#if error}<p class="err">{error}</p>{/if}
    </div>
  {:else}
    <!-- Drop / pick zone -->
    <button class="card pad dropzone" onclick={pick} disabled={transcribing}>
      {#if transcribing}
        <div class="spinner"></div>
        <div class="dz-title">transcribing {currentName}…</div>
        <div class="note">{stage || "working"}</div>
      {:else}
        <div class="dz-icon">🎙️</div>
        <div class="dz-title">drop an audio file, or click to choose</div>
        <div class="note">mp3, m4a, wav, flac, ogg, opus…</div>
      {/if}
    </button>

    <div class="sample-row">
      <button class="btn-outline" onclick={runSample} disabled={transcribing}>▶ try a sample clip</button>
    </div>

    {#if error}<p class="err">{error}</p>{/if}

    {#if result}
      <div class="card pad result">
        <div class="result-head">
          <span class="note">{currentName}{result.audio_secs > 0 ? ` · ${Math.round(result.audio_secs)}s` : ""}</span>
          <button class="btn-outline tiny" onclick={copyResult}>{copied ? "copied!" : "copy"}</button>
        </div>
        <p class="transcript">{result.text || "(no speech detected)"}</p>
      </div>
    {/if}

    <div class="card pad">
      <h3 class="section-h">recent transcripts</h3>
      <TranscriptList {refreshKey} />
    </div>
  {/if}
</section>

<style>
  .transcribe { max-width: 760px; margin: 0 auto; display: flex; flex-direction: column; gap: 16px; }
  .head h2 { font-family: var(--font-display); font-size: 40px; margin: 0; color: var(--pink-600); transform: rotate(-1deg); font-weight: 400; }
  .head .sub { color: var(--ink-700); font-weight: 600; margin: 4px 0 0; }
  .pad { padding: 20px; }
  .note { color: var(--ink-500); font-size: 13px; font-weight: 600; margin: 0; }
  .model-gate { display: flex; flex-direction: column; gap: 12px; align-items: flex-start; }
  .model-title { font-family: var(--font-display); font-size: 24px; color: var(--ink-900); }
  .dropzone {
    width: 100%; text-align: center; cursor: pointer; border-style: dashed; border-width: 3px;
    display: flex; flex-direction: column; align-items: center; gap: 8px; padding: 36px 20px;
    background: var(--bg-2); transition: transform 0.12s ease;
  }
  .dropzone:hover:not(:disabled) { transform: translate(-1px, -1px); }
  .dropzone:disabled { cursor: default; opacity: 0.9; }
  .dz-icon { font-size: 44px; }
  .dz-title { font-family: var(--font-display); font-size: 22px; color: var(--ink-900); }
  .result-head { display: flex; justify-content: space-between; align-items: center; margin-bottom: 10px; }
  .transcript { white-space: pre-wrap; line-height: 1.5; color: var(--ink-900); font-weight: 500; margin: 0; }
  .section-h { font-family: var(--font-display); font-size: 20px; margin: 0 0 6px; color: var(--ink-700); }
  .err { color: var(--pink-700); font-weight: 700; font-size: 13px; margin: 0; }
  .progress-row { width: 100%; }
  .progress-track { width: 100%; height: 12px; background: var(--ink-100); border: 2px solid var(--ink-900); border-radius: 8px; overflow: hidden; }
  .progress-bar { height: 100%; background: var(--pink-500); transition: width 0.2s ease; }
  .progress-stats { display: flex; justify-content: space-between; font-size: 12px; font-weight: 700; color: var(--ink-700); margin-top: 4px; }
  .progress-file { font-size: 11px; color: var(--ink-500); margin-top: 2px; font-family: var(--font-mono); }
  .btn-outline.tiny { padding: 4px 10px; font-size: 12px; box-shadow: 1.5px 1.5px 0 var(--ink-900); }
  .spinner {
    width: 28px; height: 28px; border: 3px solid var(--ink-300); border-top-color: var(--pink-500);
    border-radius: 50%; animation: spin 0.8s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
