<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { save as dialogSave } from "@tauri-apps/plugin-dialog";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import SoundWaves from "$lib/SoundWaves.svelte";
  import SourcePill from "$lib/SourcePill.svelte";
  import {
    type CaptureInfo,
    type PlaybackSnapshot,
    type Settings,
    type Voice,
    getSettings,
    listVoices,
    onCaptureInfo,
    onCaptureProgress,
    onPlaybackState,
    onPlaybackStarting,
    saveCurrentAudio,
    setSpeed as setSpeedCmd,
    setVoice,
    setVolume as setVolumeCmd,
    setPlayerPosition,
    setSettings,
    skip,
    stopPlayback,
    togglePause,
  } from "$lib/ipc";

  if (typeof document !== "undefined") {
    document.body.classList.add("transparent");
  }

  let snap: PlaybackSnapshot = $state({
    playing: false, paused: false, current_text: "", current_index: 0,
    current_paragraph_index: 0, total: 0, total_paragraphs: 0,
    elapsed_secs: 0, duration_secs: 0, volume: 1.0, output_sample_rate: 44100,
  });
  let voices: Voice[] = $state([]);
  let settings: Settings | null = $state(null);
  let capture: CaptureInfo | null = $state(null);
  let captureStage: string = $state(""); // "thinking" etc.
  let voicePickerOpen = $state(false);
  let speedPickerOpen = $state(false);
  let expanded = $state(false);
  let pinned = $state(false);
  let savedToast = $state(false);
  let unlisten: (() => void)[] = [];
  let pulse = $state(0);
  let pulseTimer: ReturnType<typeof setInterval>;
  let dragHandle: HTMLDivElement | undefined = $state();
  let savedSize: { w: number; h: number } | null = null;

  async function setPickerOpen(name: "voice" | "speed", on: boolean) {
    // When a picker opens, temporarily resize the player window so the popover isn't clipped.
    try {
      const w = getCurrentWindow();
      const sf = await w.scaleFactor();
      if (on) {
        const cur = await w.outerSize();
        if (!savedSize) savedSize = { w: cur.width, h: cur.height };
        const targetH = name === "voice" ? 360 : 190;
        const { PhysicalSize } = await import("@tauri-apps/api/dpi");
        await w.setSize(new PhysicalSize(Math.max(cur.width, Math.round(420 * sf)), Math.round(targetH * sf)));
      } else if (savedSize) {
        const { PhysicalSize } = await import("@tauri-apps/api/dpi");
        await w.setSize(new PhysicalSize(savedSize.w, savedSize.h));
        savedSize = null;
      }
    } catch {}
    if (name === "voice") voicePickerOpen = on; else speedPickerOpen = on;
  }

  onMount(async () => {
    voices = await listVoices();
    settings = await getSettings();
    snap.volume = settings.volume;
    pinned = settings.player_pinned;
    expanded = !settings.player_compact;
    // playback_state's current_text and current_index now reflect the chunk
    // actually being HEARD (computed from played_samples). We no longer mirror
    // chunk_synthesized into current_text — synth runs ahead of playback so
    // doing that made the mini-player display jump several chunks ahead.
    unlisten.push(await onPlaybackState((s) => (snap = { ...snap, ...s })));
    unlisten.push(await onPlaybackStarting(({ text_preview }) => {
      // Use the text preview as a placeholder until the first playback_state arrives.
      snap = { ...snap, current_text: text_preview, playing: true, paused: false };
      captureStage = "";
    }));
    unlisten.push(await onCaptureInfo((info) => {
      capture = info;
      captureStage = "";
    }));
    unlisten.push(await onCaptureProgress((stage) => (captureStage = stage)));
    pulseTimer = setInterval(() => (pulse = (pulse + 1) % 4), 320);
    // Wire drag handle to Tauri's window dragging.
    queueMicrotask(() => {
      if (dragHandle) {
        dragHandle.addEventListener("mousedown", async (e) => {
          if (e.button !== 0) return;
          try {
            const w = getCurrentWindow();
            await w.startDragging();
          } catch {}
        });
      }
    });
  });
  onDestroy(() => {
    unlisten.forEach((u) => u());
    clearInterval(pulseTimer);
  });

  function progressPct(): number {
    if (snap.duration_secs <= 0) return 0;
    return Math.min(100, (snap.elapsed_secs / snap.duration_secs) * 100);
  }
  function fmtTime(s: number): string {
    if (!isFinite(s) || s < 0) s = 0;
    const m = Math.floor(s / 60);
    const sec = Math.floor(s % 60);
    return `${m}:${sec.toString().padStart(2, "0")}`;
  }
  async function selectVoice(v: Voice) {
    if (!settings) return;
    settings = { ...settings, voice: v.name };
    await setVoice(v.name);
    setPickerOpen("voice", false);
  }
  function avatarLetter(name: string): string { return name[0] ?? "?"; }
  function voiceById(name: string | undefined): Voice | undefined {
    return voices.find(v => v.name === name);
  }
  async function setVol(v: number) {
    snap = { ...snap, volume: v };
    await setVolumeCmd(v);
  }
  async function setSpeedVal(s: number) {
    if (!settings) return;
    settings = { ...settings, speed: s };
    await setSpeedCmd(s);
    setPickerOpen("speed", false);
  }
  async function saveAudio() {
    try {
      const path = await dialogSave({
        defaultPath: `yappy-${Date.now()}.wav`,
        filters: [{ name: "WAV audio", extensions: ["wav"] }],
      });
      if (path) {
        await saveCurrentAudio(path);
        savedToast = true;
        setTimeout(() => (savedToast = false), 2500);
      }
    } catch {}
  }
  async function saveCurrentPosition() {
    try {
      const w = getCurrentWindow();
      const pos = await w.outerPosition();
      await setPlayerPosition(pos.x, pos.y);
      savedToast = true;
      setTimeout(() => (savedToast = false), 2000);
    } catch {}
  }
  async function togglePin() {
    pinned = !pinned;
    if (settings) await setSettings({ ...settings, player_pinned: pinned });
  }
  async function toggleExpand() {
    expanded = !expanded;
    if (settings) await setSettings({ ...settings, player_compact: !expanded });
    // Resize the window to fit.
    try {
      const w = getCurrentWindow();
      const sf = await w.scaleFactor();
      const targetHeight = expanded ? Math.round(190 * sf) : Math.round(94 * sf);
      const cur = await w.outerSize();
      await w.setSize(new (await import("@tauri-apps/api/dpi")).PhysicalSize(cur.width, targetHeight));
    } catch {}
  }

  async function stopFully() {
    // Stop = definitive: kills playback AND clears current source pill / text.
    stopPlayback();
    snap = { ...snap, playing: false, paused: false, elapsed_secs: 0 };
  }

  async function restartFromStart() {
    // Jump to the beginning of the current session and play.
    skip(-99999);
    if (snap.paused || !snap.playing) {
      // resume — togglePause turns paused→playing.
      togglePause();
    }
  }

  async function closePlayer() {
    // Just hide the window. Doesn't touch playback — user can leave audio playing
    // and close the visual chrome. If they want full silence, the Stop button does that.
    try {
      const w = getCurrentWindow();
      await w.hide();
    } catch {}
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") stopFully();
    else if (e.key === " ") { e.preventDefault(); togglePause(); }
    else if (e.key === "ArrowLeft") { e.preventDefault(); skip(-15); }
    else if (e.key === "ArrowRight") { e.preventDefault(); skip(15); }
    else if (e.key === "r" || e.key === "R") { e.preventDefault(); restartFromStart(); }
  }
</script>

<svelte:window on:keydown={onKey} />

<div class="player" class:expanded class:playing={snap.playing && !snap.paused} class:pinned>
  <div class="player-row top">
    <!-- drag handle on the left edge: 4px gripper -->
    <div class="grip" bind:this={dragHandle} role="presentation" title="drag to move"></div>

    <button class="play-btn" onclick={() => togglePause()} aria-label={snap.paused || !snap.playing ? "resume" : "pause"}>
      {#if snap.playing && !snap.paused}
        <svg width="13" height="13" viewBox="0 0 14 14" fill="currentColor"><rect x="2" y="1" width="3" height="12" rx="1"/><rect x="9" y="1" width="3" height="12" rx="1"/></svg>
      {:else}
        <svg width="13" height="13" viewBox="0 0 14 14" fill="currentColor"><path d="M3 1.5C3 0.7 3.85 0.25 4.5 0.7L12.5 6.2c0.6 0.4 0.6 1.3 0 1.7l-8 5.5c-0.7 0.4-1.5 0-1.5-0.8V1.5Z"/></svg>
      {/if}
    </button>

    <div class="player-main">
      <div class="player-top-line">
        {#if captureStage}
          <span class="thinking">
            <span class="dots"><i></i><i></i><i></i></span>
            thinking…
          </span>
        {:else if capture?.source}
          <SourcePill source={capture.source as any} compact />
        {/if}
        <span class="time">{fmtTime(snap.elapsed_secs)} / {fmtTime(snap.duration_secs)}</span>
        {#if snap.total > 0}
          <span class="sep">·</span>
          <span class="chunk">{snap.current_index + 1}/{snap.total}</span>
        {/if}
      </div>
      <div class="player-text" class:paused={snap.paused}>
        {#if snap.current_text}
          {snap.current_text}
        {:else if snap.playing}
          <span class="muted">preparing audio<span class="dots-text">{".".repeat(pulse + 1)}</span></span>
        {:else}
          <span class="muted">ready — press ⌥⌘R</span>
        {/if}
      </div>
      <div class="progress-track">
        <div class="progress-bar" style="--w: {progressPct()}%"></div>
      </div>
    </div>

    <div class="player-actions">
      <!-- Restart: jump to start of current session and play (R key) -->
      <button class="ico-btn" onclick={restartFromStart} data-tip="restart from start (R)"
        disabled={snap.duration_secs < 0.1}>
        <svg width="12" height="12" viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round">
          <path d="M11.5 7 C11.5 9.5 9.5 11.5 7 11.5 C4.5 11.5 2.5 9.5 2.5 7 C2.5 4.5 4.5 2.5 7 2.5 C8.4 2.5 9.6 3.1 10.5 4 M10.5 1.5 V4 H8"/>
        </svg>
      </button>
      <!-- Skip −15 / +15 -->
      <button class="ico-btn" onclick={() => skip(-15)} data-tip="back 15s (←)">
        <svg width="13" height="13" viewBox="0 0 14 14" fill="currentColor"><path d="M6.5 2.5 V0.5 L2 4 L6.5 7.5 V5.5 C9.5 5.5 11 7 11 9 C11 11 9.5 12 8 12 H6 V13.5 H8 C10.5 13.5 12.5 11.8 12.5 9 C12.5 6.2 10.5 4 6.5 4 Z"/></svg>
      </button>
      <button class="ico-btn" onclick={() => skip(15)} data-tip="forward 15s (→)">
        <svg width="13" height="13" viewBox="0 0 14 14" fill="currentColor"><path d="M7.5 2.5 V0.5 L12 4 L7.5 7.5 V5.5 C4.5 5.5 3 7 3 9 C3 11 4.5 12 6 12 H8 V13.5 H6 C3.5 13.5 1.5 11.8 1.5 9 C1.5 6.2 3.5 4 7.5 4 Z"/></svg>
      </button>
      <!-- STOP: big, pink, labelled. Definitive — kills audio AND aborts synthesis. -->
      <button class="stop-btn" onclick={stopFully} data-tip="stop reading (Esc)">
        <svg width="11" height="11" viewBox="0 0 12 12" fill="currentColor"><rect x="2" y="2" width="8" height="8" rx="1.5"/></svg>
        <span>stop</span>
      </button>
      <!-- Secondary controls -->
      <button class="ico-btn" class:active={pinned} onclick={togglePin} data-tip={pinned ? "unpin player" : "pin player (stay visible)"}>
        <svg width="13" height="13" viewBox="0 0 14 14" fill="currentColor"><path d="M9.5 1.2 C10 1.7 10 2.5 9.5 3 L9 3.5 L10.5 5 L11 4.5 C11.5 4 12.3 4 12.8 4.5 C13.3 5 13.3 5.8 12.8 6.3 L8.3 10.8 L7.7 11.4 L7.4 11.7 C6.9 12.2 6.1 12.2 5.6 11.7 C5.1 11.2 5.1 10.4 5.6 9.9 L5.9 9.6 L4.4 8.1 L4.1 8.4 C3.6 8.9 2.8 8.9 2.3 8.4 C1.8 7.9 1.8 7.1 2.3 6.6 L2.6 6.3 L7.1 1.8 C7.6 1.3 8.4 1.3 8.9 1.8 Z M2 13 L5.2 9.8 L4.2 8.8 L1 12 Z"/></svg>
      </button>
      <button class="ico-btn" onclick={toggleExpand} data-tip={expanded ? "collapse controls" : "expand for voice, speed, save…"}>
        {#if expanded}
          <svg width="10" height="10" viewBox="0 0 14 14" fill="currentColor"><path d="M2 7 H12" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg>
        {:else}
          <svg width="10" height="10" viewBox="0 0 14 14" fill="currentColor"><path d="M2 7 H12 M7 2 V12" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg>
        {/if}
      </button>
      <button class="ico-btn close-x" onclick={closePlayer} data-tip="hide window (keeps audio if playing)">
        <svg width="11" height="11" viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"><path d="M3 3 L11 11 M11 3 L3 11"/></svg>
      </button>
    </div>
  </div>

  {#if expanded}
    <div class="player-row bot">
      <button class="speed-btn" onclick={() => setPickerOpen("speed", !speedPickerOpen)} title="speed">
        {settings ? settings.speed.toFixed(2) + "×" : "1.05×"}
      </button>
      <button class="voice-btn" onclick={() => setPickerOpen("voice", !voicePickerOpen)} title="voice">
        <span class="avatar" data-id={voiceById(settings?.voice)?.id || "F3"}>{settings ? avatarLetter(settings.voice) : "?"}</span>
        <span class="voice-name">{settings?.voice ?? "voice"}</span>
      </button>
      <div class="vol-block">
        <span class="lbl">vol</span>
        <input type="range" min="0" max="1.5" step="0.05" value={snap.volume}
          oninput={(e) => setVol(parseFloat((e.target as HTMLInputElement).value))} />
      </div>
      <SoundWaves active={snap.playing && !snap.paused} height={14} bars={9} />
      <button class="ico-btn labelled" onclick={saveAudio} data-tip="save current reading as a .wav file" disabled={snap.duration_secs < 0.1}>
        <svg width="12" height="12" viewBox="0 0 14 14" fill="currentColor"><path d="M3 1 H8 L11 4 V12 C11 12.5 10.5 13 10 13 H3 C2.5 13 2 12.5 2 12 V2 C2 1.5 2.5 1 3 1 Z M7.5 2 V4.5 H10 L7.5 2 Z M6.5 6 V8.5 H8.5 L6.5 11 L4.5 8.5 H6.5 Z"/></svg>
        <span class="ico-label">save wav</span>
      </button>
      <button class="ico-btn labelled" onclick={saveCurrentPosition} data-tip="save this spot as the player's default position">
        <svg width="12" height="12" viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="2"><path d="M7 1.5 C5 1.5 3.5 3 3.5 5 C3.5 7.5 7 12 7 12 C7 12 10.5 7.5 10.5 5 C10.5 3 9 1.5 7 1.5 Z M7 5 m -1.4 0 a 1.4 1.4 0 1 0 2.8 0 a 1.4 1.4 0 1 0 -2.8 0"/></svg>
        <span class="ico-label">pin pos</span>
      </button>
    </div>
  {/if}

  {#if voicePickerOpen}
    <div class="popover voice-popover" onclick={(e) => e.stopPropagation()}>
      {#each voices as v}
        <button class="vp-item" class:active={settings?.voice === v.name} onclick={() => selectVoice(v)}>
          <span class="avatar small" data-id={v.id}>{avatarLetter(v.name)}</span>
          <span class="vp-name">{v.name}</span>
          <span class="vp-tags">{v.tags.slice(0, 2).join(" · ")}</span>
        </button>
      {/each}
    </div>
  {/if}

  {#if speedPickerOpen}
    <div class="popover speed-popover" onclick={(e) => e.stopPropagation()}>
      {#each [0.8, 0.9, 1.0, 1.05, 1.15, 1.3, 1.5, 1.75, 2.0] as s}
        <button class="sp-item" class:active={Math.abs((settings?.speed ?? 1.05) - s) < 0.001} onclick={() => setSpeedVal(s)}>
          {s.toFixed(2)}×
        </button>
      {/each}
    </div>
  {/if}

  {#if savedToast}<div class="saved-toast">saved ✓</div>{/if}
</div>

<style>
:global(body.transparent) { background: transparent; }

.player {
  position: fixed; inset: 0;
  display: flex; flex-direction: column;
  padding: 6px;
  background: transparent;
}
.player-row { display: flex; align-items: center; gap: 8px; }
.player-row.top {
  flex: 1;
  padding: 9px 10px 9px 8px;
  background: var(--cream-100);
  border: 2.5px solid var(--ink-900);
  border-radius: 22px;
  box-shadow: 3px 3px 0 var(--ink-900);
  color: var(--ink-900);
  transition: box-shadow 0.3s ease, border-color 0.3s ease;
}
.player.playing .player-row.top {
  box-shadow: 3px 3px 0 var(--pink-500);
  border-color: var(--pink-600);
}
.player.pinned .player-row.top { background: var(--cream-200); }
.player-row.bot {
  margin-top: 5px;
  padding: 6px 10px;
  background: var(--surface);
  border: 2px solid var(--ink-900);
  border-radius: 18px;
  box-shadow: 2px 2px 0 var(--ink-900);
}

.grip {
  width: 4px; height: 36px;
  background: var(--ink-300);
  border-radius: 999px;
  cursor: grab;
  margin-right: 2px;
  flex-shrink: 0;
  transition: background 0.15s ease;
}
.grip:hover { background: var(--ink-700); }
.grip:active { cursor: grabbing; }

.play-btn {
  width: 36px; height: 36px; border-radius: 12px;
  display: flex; align-items: center; justify-content: center;
  background: var(--pink-500);
  border: 2.5px solid var(--ink-900);
  color: var(--ink-900);
  box-shadow: 2px 2px 0 var(--ink-900);
  transition: transform 0.12s var(--ease-emph), box-shadow 0.12s var(--ease-emph);
  flex-shrink: 0;
}
.play-btn:hover { transform: translate(-1px, -1px); box-shadow: 3px 3px 0 var(--ink-900); }
.play-btn:active { transform: translate(1px, 1px); box-shadow: 1px 1px 0 var(--ink-900); }

.player-main { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
.player-top-line {
  display: flex; align-items: center; gap: 6px;
  font-size: 11px; color: var(--ink-500); font-weight: 700;
  overflow: hidden;
}
.player-top-line :global(.pill) { flex-shrink: 1; }
.player-top-line .time { font-family: var(--font-mono); margin-left: auto; flex-shrink: 0; }
.player-top-line .sep { opacity: 0.4; }

.thinking {
  display: inline-flex; align-items: center; gap: 6px;
  padding: 2px 9px;
  background: var(--cream-200);
  border: 2px solid var(--ink-900);
  border-radius: 999px;
  font-size: 11px; font-weight: 700; color: var(--ink-900);
  box-shadow: 1.5px 1.5px 0 var(--ink-900);
}
.thinking .dots { display: inline-flex; gap: 2px; }
.thinking .dots i {
  width: 4px; height: 4px; border-radius: 999px;
  background: var(--ink-900);
  animation: think 1.05s ease-in-out infinite;
}
.thinking .dots i:nth-child(2) { animation-delay: 0.15s; }
.thinking .dots i:nth-child(3) { animation-delay: 0.3s; }
@keyframes think {
  0%, 60%, 100% { opacity: 0.25; transform: translateY(0); }
  30% { opacity: 1; transform: translateY(-2px); }
}

.player-text {
  font-size: 13px; font-weight: 600; line-height: 1.32;
  display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical;
  overflow: hidden; text-overflow: ellipsis;
  color: var(--ink-900);
}
.player-text.paused { opacity: 0.55; }
.player-text .muted { color: var(--ink-500); font-weight: 500; }
.player-text .dots-text { display: inline-block; font-family: var(--font-mono); }

.progress-track {
  margin-top: 4px; height: 4px;
  background: var(--cream-300);
  border: 1.5px solid var(--ink-900);
  border-radius: 999px; overflow: hidden;
}
.progress-bar {
  height: 100%; width: var(--w, 0%);
  background: var(--pink-500);
  transition: width 0.2s linear;
}

.player-actions { display: flex; gap: 4px; align-items: center; flex-shrink: 0; }
.ico-btn, .voice-btn, .speed-btn {
  height: 28px; min-width: 28px; padding: 0 6px;
  border-radius: 9px;
  display: inline-flex; align-items: center; justify-content: center;
  background: var(--surface);
  border: 2px solid var(--ink-900);
  color: var(--ink-900);
  box-shadow: 1.5px 1.5px 0 var(--ink-900);
  transition: transform 0.12s var(--ease-emph), box-shadow 0.12s var(--ease-emph), background 0.15s ease;
  font-weight: 700; font-size: 11px;
  gap: 6px;
}
.speed-btn { padding: 0 10px; }
.ico-btn.labelled {
  width: auto; padding: 0 8px;
  flex-direction: column;
  gap: 1px;
  height: 32px;
}
.ico-btn.labelled .ico-label {
  font-size: 9px; font-weight: 700; line-height: 1;
  color: var(--ink-700);
}

/* Close (X) button — distinct pink */
.ico-btn.close-x { background: var(--pink-300); }
.ico-btn.close-x:hover { background: var(--pink-500); }

/* Stop button — primary danger. Big, pink, unmissable. */
.stop-btn {
  height: 28px;
  padding: 0 10px;
  border-radius: 9px;
  display: inline-flex; align-items: center; gap: 5px;
  background: var(--pink-500);
  border: 2px solid var(--ink-900);
  color: var(--ink-900);
  font-family: var(--font-sans);
  font-weight: 700; font-size: 11px;
  box-shadow: 1.5px 1.5px 0 var(--ink-900);
  transition: transform 0.12s var(--ease-emph), box-shadow 0.12s var(--ease-emph), background 0.15s ease;
}
.stop-btn:hover { transform: translate(-1px, -1px); box-shadow: 2.5px 2.5px 0 var(--ink-900); background: var(--pink-600, #ff5f95); }
.stop-btn:active { transform: translate(1px, 1px); box-shadow: 0.5px 0.5px 0 var(--ink-900); }
.stop-btn:disabled { opacity: 0.35; cursor: not-allowed; box-shadow: 1px 1px 0 var(--ink-900); }

/* CSS hover tooltips. Faster + nicer than native browser tooltips. */
[data-tip] { position: relative; }
[data-tip]:hover::after {
  content: attr(data-tip);
  position: absolute;
  bottom: calc(100% + 6px);
  left: 50%; transform: translateX(-50%);
  background: var(--ink-900);
  color: var(--cream-100);
  font-family: var(--font-sans);
  font-weight: 600;
  font-size: 11px;
  padding: 4px 9px;
  border-radius: 8px;
  border: 1.5px solid var(--ink-900);
  white-space: nowrap;
  pointer-events: none;
  z-index: 1000;
  box-shadow: 2px 2px 0 var(--ink-900);
  animation: tip-in 0.12s ease-out 0.35s both;
}
[data-tip]:hover::before {
  content: "";
  position: absolute;
  bottom: calc(100% + 1px);
  left: 50%; transform: translateX(-50%);
  border: 5px solid transparent;
  border-top-color: var(--ink-900);
  pointer-events: none;
  z-index: 1000;
  animation: tip-in 0.12s ease-out 0.35s both;
}
@keyframes tip-in { from { opacity: 0; transform: translateX(-50%) translateY(-3px); } to { opacity: 1; transform: translateX(-50%) translateY(0); } }
.voice-btn { padding: 0 10px 0 4px; }
.voice-btn .voice-name { font-size: 11px; }
.ico-btn:hover, .voice-btn:hover, .speed-btn:hover {
  transform: translate(-1px, -1px); box-shadow: 2.5px 2.5px 0 var(--ink-900); background: var(--cream-200);
}
.ico-btn:active, .voice-btn:active, .speed-btn:active {
  transform: translate(1px, 1px); box-shadow: 0.5px 0.5px 0 var(--ink-900);
}
.ico-btn.active { background: var(--pink-300); }
.ico-btn:disabled { opacity: 0.4; cursor: not-allowed; }

.avatar {
  width: 20px; height: 20px; border-radius: 6px;
  display: flex; align-items: center; justify-content: center;
  font-family: var(--font-sans); font-weight: 700; font-size: 10px;
  background: var(--pink-300);
  border: 1.5px solid var(--ink-900);
  color: var(--ink-900);
}
.avatar[data-id^="M"] { background: var(--lavender-300); }
.avatar[data-id^="F"] { background: var(--pink-300); }
.avatar.small { width: 22px; height: 22px; font-size: 11px; }

.vol-block { display: flex; align-items: center; gap: 8px; flex: 1; min-width: 0; max-width: 180px; }
.vol-block .lbl { font-size: 11px; color: var(--ink-500); font-weight: 700; }
.vol-block input[type="range"] {
  flex: 1; height: 8px;
  background: var(--cream-200);
  border: 1.5px solid var(--ink-900);
  border-radius: 999px; outline: none;
  -webkit-appearance: none; appearance: none;
}
.vol-block input[type="range"]::-webkit-slider-thumb {
  -webkit-appearance: none; width: 14px; height: 14px; border-radius: 50%;
  background: var(--pink-500); border: 1.5px solid var(--ink-900); cursor: grab;
}

.popover {
  position: absolute;
  background: var(--cream-100);
  border: 2.5px solid var(--ink-900);
  border-radius: 16px;
  padding: 6px;
  box-shadow: 3px 3px 0 var(--ink-900);
  z-index: 10;
}
/* Popovers render INSIDE the (resized-taller) player window, below the top row. */
.voice-popover {
  left: 8px; right: 8px; top: 56px; bottom: 8px;
  max-height: none;
  width: auto;
  overflow-y: auto;
  display: flex; flex-direction: column; gap: 3px;
}
.speed-popover {
  left: 8px; right: 8px; top: 56px;
  display: grid; grid-template-columns: repeat(3, 1fr);
  gap: 4px;
  width: auto;
}
.vp-item {
  display: flex; align-items: center; gap: 10px;
  padding: 6px 9px; border-radius: 9px;
  color: var(--ink-900); text-align: left; font-size: 13px;
}
.vp-item:hover { background: var(--cream-200); }
.vp-item.active { background: var(--pink-300); }
.vp-name { font-weight: 700; flex: 1; }
.vp-tags { color: var(--ink-500); font-size: 11px; font-weight: 600; }
.sp-item {
  padding: 5px 6px; font-size: 11px; font-weight: 700;
  color: var(--ink-900); border-radius: 7px;
}
.sp-item:hover { background: var(--cream-200); }
.sp-item.active { background: var(--pink-500); }

.saved-toast {
  position: absolute; bottom: -8px; left: 50%; transform: translateX(-50%);
  padding: 4px 12px; border-radius: 999px;
  background: var(--pink-500); border: 2px solid var(--ink-900);
  font-size: 11px; font-weight: 700; color: var(--ink-900);
  animation: toast 2s ease forwards;
}
@keyframes toast {
  0% { opacity: 0; transform: translate(-50%, 10px); }
  20%, 80% { opacity: 1; transform: translate(-50%, 0); }
  100% { opacity: 0; transform: translate(-50%, -10px); }
}
</style>
