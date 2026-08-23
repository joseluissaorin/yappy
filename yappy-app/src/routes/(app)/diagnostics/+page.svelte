<script lang="ts">
  import { onMount } from "svelte";
  import DiagBlock from "$lib/DiagBlock.svelte";
  import { isIOS } from "$lib/platform";
  import {
    type Settings,
    type AudioSelfTest,
    revealLogFile,
    getLogPath,
    tailLog,
    isModelReady,
    isAsrModelReady,
    getSettings,
    getTranscripts,
    audioSelfTest,
  } from "$lib/ipc";
  import { notifyError } from "$lib/ui";

  let logTail: string = $state("");

  // Audio round-trip self-test: synthesize speech, then transcribe it back.
  let selfTest: AudioSelfTest | null = $state(null);
  let testing = $state(false);
  async function runAudioTest() {
    testing = true;
    selfTest = null;
    try {
      selfTest = await audioSelfTest();
    } catch (e) {
      notifyError(String(e));
    } finally {
      testing = false;
    }
  }

  // iOS status panel state.
  let ttsReady = $state(false);
  let asrReady = $state(false);
  let settings: Settings | null = $state(null);
  let transcriptCount = $state(0);

  onMount(async () => {
    if (!$isIOS) return;
    ttsReady = await isModelReady().catch(() => false);
    asrReady = await isAsrModelReady().catch(() => false);
    settings = await getSettings().catch(() => null);
    transcriptCount = (await getTranscripts().catch(() => ({ entries: [] }))).entries.length;
  });
</script>

{#if $isIOS}
  <!-- ── iOS: status & storage (the desktop capture page makes no sense here) ── -->
  <section class="diagnostics">
    <header class="section-head">
      <h2>status</h2>
      <p>your on-device models, activity, and logs.</p>
    </header>

    <div class="card pref-card">
      <div class="st-row">
        <div class="st-ico" class:on={ttsReady}>🗣️</div>
        <div class="st-body">
          <div class="st-title">reading voices</div>
          <div class="st-sub">Supertonic · ~380 MB · 31 languages</div>
        </div>
        <div class="st-badge" class:on={ttsReady}>{ttsReady ? "ready" : "not installed"}</div>
      </div>
      <div class="st-row">
        <div class="st-ico" class:on={asrReady}>🎙️</div>
        <div class="st-body">
          <div class="st-title">transcription model</div>
          <div class="st-sub">Parakeet TDT · ~670 MB · 25 languages</div>
        </div>
        <div class="st-badge" class:on={asrReady}>{asrReady ? "ready" : "not installed"}</div>
      </div>
    </div>

    <div class="card pref-card">
      <div class="st-stats">
        <div class="st-stat">
          <div class="st-num">{settings?.successful_reads ?? 0}</div>
          <div class="st-lbl">things read aloud</div>
        </div>
        <div class="st-stat">
          <div class="st-num">{transcriptCount}</div>
          <div class="st-lbl">transcripts</div>
        </div>
        <div class="st-stat">
          <div class="st-num">100%</div>
          <div class="st-lbl">on-device</div>
        </div>
      </div>
      <p class="st-note">everything runs locally on your iPhone — nothing is sent to the cloud.</p>
    </div>

    <div class="card pref-card">
      <div class="pref-row block">
        <div>
          <div class="pref-label">audio round-trip self-test</div>
          <div class="pref-sub">synthesizes a sentence, then transcribes that audio back — proves the voice engine produces real, intelligible speech.</div>
        </div>
        <button class="btn-pink" onclick={runAudioTest} disabled={testing} style="margin-top:10px;">
          {testing ? "testing…" : "▶ run audio self-test"}
        </button>
      </div>
      {#if selfTest}
        <div class="selftest" class:ok={selfTest.ok}>
          <div class="selftest-verdict">{selfTest.ok ? "✓ audio is real speech" : "⚠ no audio signal"}</div>
          <div class="selftest-stats">
            synthesized {selfTest.synth_secs.toFixed(1)}s · level (rms) {selfTest.rms.toFixed(3)} · peak {selfTest.peak.toFixed(2)}
          </div>
          <div class="selftest-heard">heard back: <strong>“{selfTest.heard}”</strong></div>
        </div>
      {/if}
    </div>

    <div class="card pref-card">
      <div class="pref-row">
        <div>
          <div class="pref-label">log file</div>
          <div class="pref-sub">backend events (model loads, synth, transcription). share this if something gets stuck.</div>
        </div>
        <div style="display:flex; gap:8px; flex-wrap:wrap;">
          <button class="btn-outline" onclick={async () => { try { const p = await getLogPath(); await navigator.clipboard.writeText(p); } catch (e) { notifyError(String(e)); } }}>copy path</button>
          <button class="btn-outline" onclick={async () => { try { logTail = await tailLog(64); } catch (e) { notifyError(String(e)); } }}>show last 64 kb</button>
        </div>
      </div>
      {#if logTail}<pre class="home-log-tail">{logTail}</pre>{/if}
    </div>
  </section>
{:else}
  <!-- ── Desktop: capture diagnostics (selection / active doc / OCR / clipboard) ── -->
  <section class="diagnostics">
    <header class="section-head">
      <h2>diagnostics</h2>
      <p>what yappy sees right now. useful for debugging capture issues.</p>
    </header>
    <div class="card pref-card"><DiagBlock /></div>

    <div class="card pref-card" style="margin-top:14px;">
      <div class="pref-row">
        <div>
          <div class="pref-label">log file</div>
          <div class="pref-sub">
            every backend event yappy emits — including PDF parse times, ocr stages, synth lifecycle. share this with me to debug "stuck on X" issues.
          </div>
        </div>
        <div style="display:flex; gap:8px; flex-wrap:wrap;">
          <button class="btn-outline" onclick={async () => { try { await revealLogFile(); } catch (e) { notifyError(String(e)); } }}>reveal log file</button>
          <button class="btn-outline" onclick={async () => { try { const p = await getLogPath(); await navigator.clipboard.writeText(p); } catch (e) { notifyError(String(e)); } }}>copy path</button>
          <button class="btn-outline" onclick={async () => { try { logTail = await tailLog(64); } catch (e) { notifyError(String(e)); } }}>show last 64 kb</button>
        </div>
      </div>
      {#if logTail}<pre class="home-log-tail">{logTail}</pre>{/if}
    </div>
  </section>
{/if}

<style>
  .st-row { display: flex; align-items: center; gap: 14px; padding: 12px 0; border-bottom: 2px dashed var(--ink-300); }
  .st-row:last-child { border-bottom: none; }
  .st-ico {
    flex: 0 0 auto; width: 48px; height: 48px; border-radius: 14px; font-size: 24px;
    display: flex; align-items: center; justify-content: center;
    background: var(--ink-100); border: 2px solid var(--ink-300); filter: grayscale(0.6); opacity: 0.7;
  }
  .st-ico.on { background: var(--pink-300); border-color: var(--ink-900); filter: none; opacity: 1; }
  .st-body { flex: 1; min-width: 0; }
  .st-title { font-weight: 800; font-size: 15px; color: var(--ink-900); }
  .st-sub { font-size: 12px; color: var(--ink-500); font-weight: 600; }
  .st-badge {
    flex: 0 0 auto; font-size: 12px; font-weight: 800; padding: 5px 11px; border-radius: 999px;
    background: var(--ink-100); color: var(--ink-500); border: 2px solid var(--ink-300);
  }
  .st-badge.on { background: var(--pink-500); color: var(--ink-900); border-color: var(--ink-900); }
  .st-stats { display: flex; justify-content: space-around; text-align: center; padding: 6px 0 4px; }
  .st-num { font-family: var(--font-display); font-size: 34px; color: var(--pink-600); line-height: 1; }
  .st-lbl { font-size: 11px; font-weight: 700; color: var(--ink-500); margin-top: 4px; }
  .st-note { text-align: center; font-size: 12px; color: var(--ink-500); font-weight: 600; margin: 12px 0 0; }
  .selftest { margin-top: 12px; padding: 12px 14px; border-radius: 14px; background: var(--ink-100); border: 2px solid var(--ink-300); }
  .selftest.ok { background: var(--pink-300); border-color: var(--ink-900); }
  .selftest-verdict { font-weight: 800; font-size: 15px; color: var(--ink-900); }
  .selftest-stats { font-size: 11px; color: var(--ink-700); font-weight: 600; margin-top: 3px; font-variant-numeric: tabular-nums; }
  .selftest-heard { font-size: 14px; color: var(--ink-900); margin-top: 6px; line-height: 1.4; }
</style>
