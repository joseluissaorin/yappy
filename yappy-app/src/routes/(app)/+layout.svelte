<script lang="ts">
  import "$lib/appShell.css";
  import { onMount, onDestroy } from "svelte";
  import { page } from "$app/stores";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { get as getStore } from "svelte/store";
  import HelpOverlay from "$lib/HelpOverlay.svelte";
  import CriaturaCameo from "$lib/CriaturaCameo.svelte";
  import AboutModal from "$lib/AboutModal.svelte";
  import Onboarding from "$lib/Onboarding.svelte";
  import Confetti from "$lib/Confetti.svelte";
  import { goPage, sectionForPath } from "$lib/nav";
  import { isIOS, isMobile, ready as platformReady, platformLocale, langCodeFromLocale } from "$lib/platform";
  import { fijarIdiomaDesdeLocale } from "$lib/i18n";
  import { startShareIntake } from "$lib/shareIntake";
  import {
    type Settings,
    type PlaybackSnapshot,
    getSettings,
    setSettings,
    readFile,
    onNav,
    onSynthError,
    onFirstRead,
    onModelMissing,
    onPlaybackState,
    togglePause,
    stopPlayback,
  } from "$lib/ipc";

  let { children } = $props();

  let settings: Settings | null = $state(null);
  let helpOpen = $state(false);
  let aboutOpen = $state(false);
  let onboardingOpen = $state(false);
  let showConfetti = $state(false);
  let dragOver = $state(false);
  let errorToast: string | null = $state(null);
  // Playback for the iOS mini-player bar (desktop uses the floating player window).
  let playback = $state<PlaybackSnapshot | null>(null);
  const mpActive = $derived(!!playback && (playback.playing || playback.paused) && !!playback.current_text);
  const mpPct = $derived(playback && playback.duration_secs > 0 ? Math.min(100, (playback.elapsed_secs / playback.duration_secs) * 100) : 0);
  function fmtTime(s: number): string {
    if (!isFinite(s) || s < 0) s = 0;
    const m = Math.floor(s / 60);
    const sec = Math.floor(s % 60);
    return `${m}:${sec.toString().padStart(2, "0")}`;
  }

  const current = $derived(sectionForPath($page.url.pathname));

  const AUDIO_EXTS = ["mp3", "m4a", "aac", "wav", "flac", "ogg", "oga", "opus", "caf", "amr"];
  const isAudioPath = (p: string) => AUDIO_EXTS.includes((p.split(".").pop() ?? "").toLowerCase());

  function showError(msg: string) {
    errorToast = msg;
    setTimeout(() => (errorToast = null), 6000);
  }

  let cleanups: (() => void)[] = [];

  onMount(async () => {
    settings = await getSettings();
    if (settings && !settings.first_launch_done) {
      onboardingOpen = true;
      await platformReady;
      const locale = getStore(platformLocale);
      // El idioma de la interfaz (las vistas compartidas ya están
      // localizadas; el resto del shell de escritorio, aún no).
      fijarIdiomaDesdeLocale(locale);
      const detectedLang = langCodeFromLocale(locale);
      if (detectedLang && detectedLang !== settings.default_lang) {
        settings = { ...settings, default_lang: detectedLang };
        await setSettings(settings);
      }
    }

    // iOS: share-sheet payload handler (URLs/text/transcripts). Boot once.
    platformReady.then((p) => {
      if (p === "ios") startShareIntake().catch((e) => console.error("[share] start failed:", e));
    });

    // App-global backend events.
    cleanups.push(await onNav((p) => {
      if (p === "about") aboutOpen = true;
      else goPage(p as any);
    }));
    cleanups.push(await onSynthError((m) => showError(m)));
    cleanups.push(await onFirstRead(() => (showConfetti = true)));
    cleanups.push(await onModelMissing(() => {}));
    cleanups.push(await onPlaybackState((s) => (playback = s)));

    // Window-level handlers (live the whole session in the main window).
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "/") {
        e.preventDefault();
        helpOpen = !helpOpen;
      }
      if (e.key === "Escape") {
        helpOpen = false;
        aboutOpen = false;
      }
    };
    window.addEventListener("keydown", onKey);
    cleanups.push(() => window.removeEventListener("keydown", onKey));

    const onErr = (e: Event) => showError(String((e as CustomEvent).detail));
    window.addEventListener("yappy:error", onErr);
    cleanups.push(() => window.removeEventListener("yappy:error", onErr));

    // A shared audio message was transcribed (iOS in-extension / handoff).
    const onTranscriptShare = () => goPage("transcribe");
    window.addEventListener("yappy:transcript", onTranscriptShare);
    cleanups.push(() => window.removeEventListener("yappy:transcript", onTranscriptShare));

    // File drop anywhere in the main window: audio → transcription, else reader.
    try {
      const win = getCurrentWindow();
      const u = await win.onDragDropEvent((ev) => {
        const t = ev.payload.type;
        if (t === "over") dragOver = true;
        else if (t === "leave") dragOver = false;
        else if (t === "drop") {
          dragOver = false;
          const paths = (ev.payload as any).paths as string[];
          if (paths && paths.length > 0) {
            if (isAudioPath(paths[0])) goPage("transcribe", { path: paths[0] });
            else readFile(paths[0]).catch((e) => showError(String(e)));
          }
        }
      });
      cleanups.push(u);
    } catch (e) {
      console.warn("drag-drop wire failed", e);
    }
  });

  onDestroy(() => cleanups.forEach((c) => c()));
</script>

<header class="topbar" data-tauri-drag-region>
  <div class="topbar-inner" data-tauri-drag-region>
    <button class="logo" onclick={() => goPage("home")} aria-label="Yappy, go to home">yappy</button>
    <nav class="nav" aria-label="primary navigation">
      <button class:active={current === "home"} onclick={() => goPage("home")} aria-current={current === "home" ? "page" : undefined}>home</button>
      <button class:active={current === "voices"} onclick={() => goPage("voices")} aria-current={current === "voices" ? "page" : undefined}>voices</button>
      <button class:active={current === "transcribe"} onclick={() => goPage("transcribe")} aria-current={current === "transcribe" ? "page" : undefined}>transcribe</button>
      <button class:active={current === "library"} onclick={() => goPage("library")} aria-current={current === "library" ? "page" : undefined}>library</button>
      <button class:active={current === "preferences"} onclick={() => goPage("preferences")} aria-current={current === "preferences" ? "page" : undefined}>preferences</button>
      <button class:active={current === "history"} onclick={() => goPage("history")} aria-current={current === "history" ? "page" : undefined}>history</button>
      <button class:active={current === "diagnostics"} onclick={() => goPage("diagnostics")} aria-current={current === "diagnostics" ? "page" : undefined}>diagnostics</button>
    </nav>
    <div class="actions">
      <button class="btn-ghost icon-only" title="help (⌘/)" onclick={() => (helpOpen = true)} aria-label="help">?</button>
      <button class="btn-ghost icon-only" title="about yappy" onclick={() => (aboutOpen = true)} aria-label="about Yappy">ⓘ</button>
    </div>
  </div>
</header>

<main>
  <div class="main-inner">
    {@render children?.()}
  </div>
</main>

{#if dragOver}
  <div class="drop-overlay">
    <div class="drop-inner">
      <div class="drop-icon">📥</div>
      <h2>drop to read</h2>
      <p>.pdf · .docx · .rtf · .md · .txt · .html · audio and more</p>
    </div>
  </div>
{/if}

<CriaturaCameo size={44} />
<HelpOverlay open={helpOpen} onClose={() => (helpOpen = false)} />
<AboutModal open={aboutOpen} onClose={() => (aboutOpen = false)} />
<Onboarding open={onboardingOpen} onDone={async () => {
  onboardingOpen = false;
  if (settings) { settings = { ...settings, first_launch_done: true }; await setSettings(settings); }
}} />

{#if showConfetti}<Confetti onDone={() => (showConfetti = false)} />{/if}
{#if errorToast}<div class="toast danger">⚠ {errorToast}</div>{/if}

<!-- iOS mini-player: compact bottom bar (Spotify / Apple Music style). Desktop
     uses the separate floating player window instead. -->
{#if $isMobile && mpActive && playback}
  <div class="miniplayer">
    <div class="mp-progress" style="width: {mpPct}%"></div>
    <button class="mp-play" onclick={() => togglePause()} aria-label={playback.paused ? "resume" : "pause"}>
      {#if playback.paused}
        <svg width="18" height="18" viewBox="0 0 14 14" fill="currentColor"><path d="M3 1.5C3 0.7 3.85 0.25 4.5 0.7L12.5 6.2c0.6 0.4 0.6 1.3 0 1.7l-8 5.5c-0.7 0.4-1.5 0-1.5-0.8V1.5Z"/></svg>
      {:else}
        <svg width="18" height="18" viewBox="0 0 14 14" fill="currentColor"><rect x="2.5" y="2" width="3.2" height="10" rx="1"/><rect x="8.3" y="2" width="3.2" height="10" rx="1"/></svg>
      {/if}
    </button>
    <div class="mp-text">
      <div class="mp-title">{playback.current_text || "Yappy"}</div>
      <div class="mp-time">{playback.paused ? "paused" : "reading"} · {fmtTime(playback.elapsed_secs)} / {fmtTime(playback.duration_secs)}</div>
    </div>
    <button class="mp-stop" onclick={() => stopPlayback()} aria-label="stop">
      <svg width="13" height="13" viewBox="0 0 14 14" fill="currentColor"><rect x="2" y="2" width="10" height="10" rx="2"/></svg>
    </button>
  </div>
{/if}

<style>
  .miniplayer {
    position: fixed; left: 12px; right: 12px;
    bottom: calc(12px + env(safe-area-inset-bottom, 0px));
    z-index: 30;
    display: flex; align-items: center; gap: 12px;
    padding: 10px 12px;
    background: var(--ink-900); color: var(--cream-100);
    border-radius: 18px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.28);
    overflow: hidden;
  }
  .mp-progress {
    position: absolute; top: 0; left: 0; height: 3px;
    background: var(--pink-500); transition: width 0.3s linear;
  }
  .mp-play {
    flex: 0 0 auto; width: 42px; height: 42px; border-radius: 13px;
    background: var(--pink-500); color: var(--ink-900);
    display: flex; align-items: center; justify-content: center;
  }
  .mp-text { flex: 1; min-width: 0; }
  .mp-title {
    font-weight: 700; font-size: 14px; line-height: 1.25;
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }
  .mp-time { font-size: 11px; opacity: 0.7; font-variant-numeric: tabular-nums; margin-top: 1px; }
  .mp-stop {
    flex: 0 0 auto; width: 34px; height: 34px; border-radius: 11px;
    background: rgba(255, 255, 255, 0.16); color: var(--cream-100);
    display: flex; align-items: center; justify-content: center;
  }
  .mp-stop:active { background: rgba(255, 255, 255, 0.3); }
</style>
