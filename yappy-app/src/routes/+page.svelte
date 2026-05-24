<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
  import { readTextFile, writeTextFile } from "@tauri-apps/plugin-fs";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import DiagBlock from "$lib/DiagBlock.svelte";
  import YappyMascot from "$lib/YappyMascot.svelte";
  import SoundWaves from "$lib/SoundWaves.svelte";
  import HelpOverlay from "$lib/HelpOverlay.svelte";
  import AboutModal from "$lib/AboutModal.svelte";
  import HistoryList from "$lib/HistoryList.svelte";
  import Confetti from "$lib/Confetti.svelte";
  import SourcePill from "$lib/SourcePill.svelte";
  import HotkeyPicker from "$lib/HotkeyPicker.svelte";
  import CreditsModal from "$lib/CreditsModal.svelte";
  import Onboarding from "$lib/Onboarding.svelte";
  import {
    LANGUAGES,
    type CaptureInfo,
    type DownloadProgress,
    type PlaybackSnapshot,
    type Quality,
    type Settings,
    type Voice,
    downloadModel,
    getSettings,
    isModelReady,
    listVoices,
    onCaptureEmpty,
    onCaptureInfo,
    onCaptureProgress,
    onFirstRead,
    onModelDownload,
    onModelMissing,
    onNav,
    onPlaybackState,
    onSynthError,
    readClipboard,
    readFile,
    readNow,
    requestMacosPermissions,
    resetSettings,
    exportSettings,
    importSettings,
    sampleVoice,
    setLaunchAtLogin,
    setAppTheme,
    bridgeStatus,
    bridgeRegenerateToken,
    bridgeClearPairing,
    setBridgeEnabled,
    openBrowserExtensions,
    getExtensionPath,
    revealExtensionFolder,
    revealLogFile,
    getLogPath,
    tailLog,
    onBridgePaired,
    onBridgeDisconnected,
    onBridgeTokenChanged,
    type BridgeStatus,
    setDefaultLang,
    setHotkey,
    setOcrEngine,
    setPlayerPreset,
    setPlayerSize,
    setPlayerTheme,
    setQuality,
    setSettings,
    setSilence,
    setSpeed,
    setVoice,
    setVoiceOverride,
    setVolume,
    stopPlayback,
    synthesizeText,
    type AppTheme,
    type OcrEngine,
    type PlayerPositionPreset,
    type PlayerTheme,
  } from "$lib/ipc";

  let voices: Voice[] = $state([]);
  let settings: Settings | null = $state(null);
  let modelReady: boolean = $state(false);
  let download: DownloadProgress | null = $state(null);
  let downloading: boolean = $state(false);
  let activeSection: "home" | "voices" | "preferences" | "history" | "diagnostics" = $state("home");
  let captureEmptyToast: boolean = $state(false);
  let synthError: string | null = $state(null);
  let testText = $state(
    "yappy is a tiny, local text-to-speech app. press the hotkey anywhere on your mac and i'll read what you're looking at, out loud — without sending a single word to the cloud.",
  );
  let testing = $state(false);
  let testTextOpen = $state(false);
  let voiceSearch = $state("");
  let helpOpen = $state(false);
  let aboutOpen = $state(false);
  let creditsOpen = $state(false);
  let onboardingOpen = $state(false);
  let showConfetti = $state(false);
  let playback = $state<PlaybackSnapshot | null>(null);
  let captureInfo: CaptureInfo | null = $state(null);
  let captureStage = $state(""); // "thinking" when smart_capture is running
  let dragOver = $state(false);
  let bridge: BridgeStatus | null = $state(null);
  let bridgeBusy = $state(false);
  let bridgeToastText: string | null = $state(null);
  let homeLogTail: string = $state("");

  async function refreshBridgeStatus() {
    try { bridge = await bridgeStatus(); } catch {}
  }
  function bridgeToast(t: string) {
    bridgeToastText = t;
    setTimeout(() => (bridgeToastText = null), 2500);
  }
  async function copyToken() {
    if (!bridge?.token) return;
    try { await navigator.clipboard.writeText(bridge.token); bridgeToast("token copied"); } catch {}
  }
  async function regenerateToken() {
    if (bridgeBusy) return;
    if (!confirm("Regenerate token? Every paired extension will be disconnected and must re-pair.")) return;
    bridgeBusy = true;
    try {
      const fresh = await bridgeRegenerateToken();
      await refreshBridgeStatus();
      console.log("Yappy bridge: regenerated token", fresh);
      bridgeToast(`new token — starts with ${fresh.slice(0, 8)}… (re-pair the extension)`);
    } catch (e) {
      console.error("regenerateToken failed", e);
      bridgeToast(`regenerate failed: ${String(e).slice(0, 80)}`);
    } finally { bridgeBusy = false; }
  }
  async function rePairExtensions() {
    if (bridgeBusy) return;
    bridgeBusy = true;
    try {
      await bridgeClearPairing();
      await refreshBridgeStatus();
      bridgeToast("ready — next extension to connect will claim the token");
    } catch (e) {
      console.error("rePairExtensions failed", e);
      bridgeToast(`re-pair failed: ${String(e).slice(0, 80)}`);
    } finally { bridgeBusy = false; }
  }
  async function toggleBridge(enabled: boolean) {
    await setBridgeEnabled(enabled);
    await refreshBridgeStatus();
  }
  function fmtAgo(unix: number): string {
    if (!unix) return "—";
    const diff = Math.max(0, Math.floor(Date.now() / 1000) - unix);
    if (diff < 60) return `${diff}s ago`;
    if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
    if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
    return `${Math.floor(diff / 86400)}d ago`;
  }

  let unlisteners: (() => void)[] = [];

  onMount(async () => {
    voices = await listVoices();
    settings = await getSettings();
    modelReady = await isModelReady();
    if (settings && !settings.first_launch_done) {
      onboardingOpen = true;
    }

    unlisteners.push(await onModelDownload((p) => (download = p)));
    unlisteners.push(await onCaptureEmpty(() => {
      captureEmptyToast = true;
      setTimeout(() => (captureEmptyToast = false), 3000);
    }));
    unlisteners.push(await onCaptureInfo((info) => {
      captureInfo = info;
      captureStage = "";
    }));
    unlisteners.push(await onCaptureProgress((stage) => (captureStage = stage)));
    unlisteners.push(await onModelMissing(() => (modelReady = false)));
    unlisteners.push(await onSynthError((m) => {
      synthError = m;
      setTimeout(() => (synthError = null), 6000);
    }));
    unlisteners.push(await onNav((p) => {
      if (p === "voices" || p === "preferences" || p === "history") activeSection = p as any;
      else if (p === "about") aboutOpen = true;
    }));
    unlisteners.push(await onPlaybackState((s) => (playback = s)));
    unlisteners.push(await onFirstRead(() => (showConfetti = true)));

    // Global keyboard shortcuts inside the main window.
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
    unlisteners.push(() => window.removeEventListener("keydown", onKey));

    // Tauri file drop events
    try {
      const win = getCurrentWindow();
      const u1 = await win.onDragDropEvent((ev) => {
        const t = ev.payload.type;
        if (t === "over") dragOver = true;
        else if (t === "leave") dragOver = false;
        else if (t === "drop") {
          dragOver = false;
          const paths = (ev.payload as any).paths as string[];
          if (paths && paths.length > 0) {
            readFile(paths[0]).catch((e) => (synthError = String(e)));
          }
        }
      });
      unlisteners.push(u1);
    } catch (e) {
      console.warn("drag-drop wire failed", e);
    }

    // Browser extension bridge — initial fetch + live updates.
    refreshBridgeStatus();
    unlisteners.push(await onBridgePaired(() => refreshBridgeStatus()));
    unlisteners.push(await onBridgeDisconnected(() => refreshBridgeStatus()));
    unlisteners.push(await onBridgeTokenChanged(() => refreshBridgeStatus()));
  });

  onDestroy(() => unlisteners.forEach((u) => u()));

  async function startDownload() {
    downloading = true;
    try {
      await downloadModel();
      modelReady = await isModelReady();
    } catch (e) {
      synthError = String(e);
    } finally {
      downloading = false;
    }
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

  async function changeSpeed(v: number) {
    if (!settings) return;
    settings = { ...settings, speed: v };
    await setSpeed(v);
  }
  async function changeVolume(v: number) {
    if (!settings) return;
    settings = { ...settings, volume: v };
    await setVolume(v);
  }
  async function changeSilence(v: number) {
    if (!settings) return;
    settings = { ...settings, silence_secs: v };
    await setSilence(v);
  }
  async function changeLang(code: string) {
    if (!settings) return;
    settings = { ...settings, default_lang: code };
    await setDefaultLang(code);
  }
  async function changeQuality(q: Quality) {
    if (!settings) return;
    settings = { ...settings, quality: q };
    await setQuality(q);
  }
  async function changeVoiceOverride(lang: string, voice: string) {
    if (!settings) return;
    const overrides = { ...settings.voice_overrides };
    if (voice === "__inherit__") {
      delete overrides[lang];
      settings = { ...settings, voice_overrides: overrides };
      await setVoiceOverride(lang, null);
    } else {
      overrides[lang] = voice;
      settings = { ...settings, voice_overrides: overrides };
      await setVoiceOverride(lang, voice);
    }
  }

  async function changeHotkey(action: "read_now" | "pause_resume", combo: string) {
    if (!settings) return;
    try {
      await setHotkey(action, combo);
      settings =
        action === "read_now"
          ? { ...settings, hotkey_read_now: combo }
          : { ...settings, hotkey_pause_resume: combo };
    } catch (e) {
      synthError = String(e);
    }
  }

  async function changePlayerPreset(p: PlayerPositionPreset) {
    if (!settings) return;
    settings = { ...settings, player_position_preset: p };
    await setPlayerPreset(p);
  }
  async function changePlayerTheme(t: PlayerTheme) {
    if (!settings) return;
    settings = { ...settings, player_theme: t };
    await setPlayerTheme(t);
  }
  async function changePlayerSize(s: string) {
    if (!settings) return;
    settings = { ...settings, player_size: s };
    await setPlayerSize(s);
  }
  async function changeAppTheme(t: AppTheme) {
    if (!settings) return;
    settings = { ...settings, app_theme: t };
    await setAppTheme(t);
    document.documentElement.dataset.theme = t;
  }
  async function changeOcrEngine(e: OcrEngine) {
    if (!settings) return;
    settings = { ...settings, ocr_engine: e };
    await setOcrEngine(e);
  }
  async function changePlayerOpacity(v: number) {
    if (!settings) return;
    settings = { ...settings, player_opacity: v };
    await setSettings(settings);
  }
  async function changeAutoHide(v: number) {
    if (!settings) return;
    settings = { ...settings, player_autohide_secs: v };
    await setSettings(settings);
  }
  async function doResetSettings() {
    if (!confirm("reset all settings to defaults? this can't be undone.")) return;
    const s = await resetSettings();
    settings = s;
  }
  async function doExportSettings() {
    try {
      const json = await exportSettings();
      const path = await saveDialog({
        defaultPath: `yappy-settings-${new Date().toISOString().slice(0,10)}.json`,
        filters: [{ name: "Yappy settings", extensions: ["json"] }],
      });
      if (path) await writeTextFile(path, json);
    } catch (e) {
      synthError = String(e);
    }
  }
  async function doImportSettings() {
    try {
      const path = await openDialog({
        filters: [{ name: "Yappy settings", extensions: ["json"] }],
      });
      if (typeof path === "string") {
        const txt = await readTextFile(path);
        const s = await importSettings(txt);
        settings = s;
      }
    } catch (e) {
      synthError = String(e);
    }
  }

  async function runTest() {
    testing = true;
    try {
      captureInfo = { source: { kind: "manual" } };
      await synthesizeText(testText);
    } catch (e) {
      synthError = String(e);
    } finally {
      setTimeout(() => (testing = false), 1500);
    }
  }

  async function openFile() {
    try {
      const path = await openDialog({
        multiple: false,
        filters: [
          { name: "Documents", extensions: ["txt", "md", "markdown", "rtf", "docx", "doc", "odt", "pdf", "epub", "html", "htm"] },
        ],
      });
      if (typeof path === "string") {
        await readFile(path);
      }
    } catch (e) {
      synthError = String(e);
    }
  }

  function downloadPercent(): number {
    if (!download || download.overall_total === 0) return 0;
    return Math.min(100, Math.round((download.overall_done / download.overall_total) * 100));
  }
  function downloadDoneMB(): number {
    if (!download) return 0;
    return Math.round((download.overall_done / 1024 / 1024) * 10) / 10;
  }
  function downloadTotalMB(): number {
    if (!download) return 0;
    return Math.round((download.overall_total / 1024 / 1024) * 10) / 10;
  }

  const filteredVoices = $derived(
    voices.filter((v) => {
      const q = voiceSearch.trim().toLowerCase();
      if (!q) return true;
      return (
        v.name.toLowerCase().includes(q) ||
        v.description.toLowerCase().includes(q) ||
        v.tags.some((t) => t.toLowerCase().includes(q))
      );
    }),
  );

  const OVERRIDE_LANGS: string[] = ["en", "es", "fr", "de", "it", "pt", "nl", "ja", "ko", "ru"];

  const isPlaying = $derived(!!playback?.playing && !playback?.paused);
  const isThinking = $derived(captureStage === "thinking");
</script>

<header class="topbar" data-tauri-drag-region>
  <div class="topbar-inner" data-tauri-drag-region>
    <button class="logo" onclick={() => (activeSection = "home")}>yappy</button>
    <nav class="nav">
      <button class:active={activeSection === "home"} onclick={() => (activeSection = "home")}>home</button>
      <button class:active={activeSection === "voices"} onclick={() => (activeSection = "voices")}>voices</button>
      <button class:active={activeSection === "preferences"} onclick={() => (activeSection = "preferences")}>preferences</button>
      <button class:active={activeSection === "history"} onclick={() => (activeSection = "history")}>history</button>
      <button class:active={activeSection === "diagnostics"} onclick={() => (activeSection = "diagnostics")}>diagnostics</button>
    </nav>
    <div class="actions">
      <button class="btn-ghost icon-only" title="help (⌘/)" onclick={() => (helpOpen = true)}>?</button>
      <button class="btn-ghost icon-only" title="about yappy" onclick={() => (aboutOpen = true)}>ⓘ</button>
    </div>
  </div>
</header>

<main>
  <div class="main-inner">
  {#if activeSection === "home"}
    <!-- v3: one giant focal panel that morphs to the current state, secondary actions
         below as a compact row, then recents + voices. No marketing copy, no equal-weight
         action cards — the primary thing is reading, everything else supports it. -->

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
      </section>
    {:else}
      <!-- ── PRIMARY PANEL: morphs to the current state ───────────────────── -->
      <section class="read-panel" class:playing={isPlaying} class:paused={playback?.paused} class:thinking={isThinking}>
        <div class="read-panel-status">
          <span class="rp-status">
            <span class="rp-dot" class:on={modelReady}></span>
            {modelReady ? "voices ready" : "model not installed"}
          </span>
          <span class="rp-sep">·</span>
          <button class="rp-voice" onclick={() => (activeSection = "voices")} title="change voice">
            <span class="voice-dot" data-id={voices.find(v => v.name === settings?.voice)?.id || "F1"}></span>
            {settings?.voice ?? "—"}
          </button>
          <span class="rp-sep">·</span>
          <button class="rp-bridge" onclick={() => (activeSection = "preferences")} class:on={bridge && bridge.connections.length > 0}>
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
          <button class="rp-primary" onclick={() => readNow()}>
            <span class="rp-primary-icon"><YappyMascot size={64} talking={isPlaying} /></span>
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
            <button class="rp-stop" onclick={() => stopPlayback()} title="stop reading (Esc)">
              <svg width="14" height="14" viewBox="0 0 14 14" fill="currentColor"><rect x="2" y="2" width="10" height="10" rx="2"/></svg>
              stop
            </button>
          {/if}
        </div>

        <!-- Secondary actions — one row, compact, equal weight to each other -->
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

      <!-- ── RECENTS: horizontal scroll of mini-cards, not a vertical list ── -->
      <section class="recents-v3">
        <div class="qv-head">
          <h2>recent</h2>
          <button class="link" onclick={() => (activeSection = "history")}>all →</button>
        </div>
        <HistoryList compact={true} max={5} />
      </section>

      <!-- ── VOICE QUICK-SWITCH ───────────────────────────────────────────── -->
      <section class="quickvoices">
        <div class="qv-head">
          <h2>switch voice</h2>
          <button class="link" onclick={() => (activeSection = "voices")}>browse all 10 →</button>
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
  {:else if activeSection === "voices"}
    <section class="voices-grid-wrap">
      <header class="section-head">
        <h2>voices</h2>
        <p>click a card to select. tap the speaker icon to hear a sample.</p>
      </header>
      <div class="voice-controls">
        <input class="search" type="search" placeholder="search voices…" bind:value={voiceSearch} />
      </div>
      <div class="voices-grid">
        {#each filteredVoices as v}
          <div class="voice-card" class:active={settings?.voice === v.name} role="button" tabindex="0"
            onclick={() => pickVoice(v)}
            onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); pickVoice(v); } }}>
            <div class="vc-head">
              <div class="voice-avatar" data-id={v.id}>{v.name[0]}</div>
              <button class="sample-btn" onclick={(e) => playVoiceSample(v, e)} title="sample">
                <svg width="11" height="11" viewBox="0 0 14 14" fill="currentColor"><path d="M3 1.5C3 0.7 3.85 0.25 4.5 0.7L12.5 6.2c0.6 0.4 0.6 1.3 0 1.7l-8 5.5c-0.7 0.4-1.5 0-1.5-0.8V1.5Z"/></svg>
              </button>
            </div>
            <div class="voice-meta">
              <div class="voice-name">{v.name}</div>
              <div class="voice-tags">{#each v.tags as t}<span class="voice-tag">{t}</span>{/each}</div>
            </div>
            <p class="voice-desc">{v.description}</p>
            <div class="voice-actions">
              <span class="voice-id">{v.id} · {v.gender.toLowerCase()}</span>
              {#if settings?.voice === v.name}<span class="voice-active-pill">selected</span>{/if}
            </div>
          </div>
        {/each}
      </div>
    </section>
  {:else if activeSection === "preferences"}
    {#if settings}
      <section class="prefs">
        <header class="section-head">
          <h2>preferences</h2>
          <p>tune voices, speed, quality, hotkeys, per-language defaults, the floating player, and more.</p>
        </header>

        <!-- Default voice (prominent at the top) -->
        <div class="card pref-card default-voice-card">
          <div class="pref-row block">
            <div>
              <div class="pref-label">your default voice</div>
              <div class="pref-sub">used unless a per-language override below kicks in.</div>
            </div>
            {#each [voices.find((v) => v.name === settings!.voice) ?? voices[0]] as cur}
              {#if cur}
                <div class="dv-row">
                  <div class="voice-avatar lg" data-id={cur.id}>{cur.name[0]}</div>
                  <div class="dv-meta">
                    <div class="dv-name">{cur.name}</div>
                    <div class="dv-desc">{cur.description}</div>
                  </div>
                  <button class="btn-outline" onclick={(e) => playVoiceSample(cur, e)} disabled={!modelReady}>
                    <svg width="10" height="10" viewBox="0 0 14 14" fill="currentColor"><path d="M3 1.5C3 0.7 3.85 0.25 4.5 0.7L12.5 6.2c0.6 0.4 0.6 1.3 0 1.7l-8 5.5c-0.7 0.4-1.5 0-1.5-0.8V1.5Z"/></svg>
                    sample
                  </button>
                  <select value={settings.voice} onchange={(e) => pickVoice(voices.find((v) => v.name === (e.target as HTMLSelectElement).value)!)}>
                    {#each voices as v}<option value={v.name}>{v.name} — {v.gender.toLowerCase()}</option>{/each}
                  </select>
                </div>
              {/if}
            {/each}
          </div>
        </div>

        <div class="card pref-card">
          <div class="pref-row">
            <div>
              <div class="pref-label">quality</div>
              <div class="pref-sub">balanced is the studio default. best is slower but cleaner.</div>
            </div>
            <div class="seg">
              <button class:active={settings.quality === "fast"} onclick={() => changeQuality("fast")}>fast</button>
              <button class:active={settings.quality === "balanced"} onclick={() => changeQuality("balanced")}>balanced</button>
              <button class:active={settings.quality === "best"} onclick={() => changeQuality("best")}>best</button>
            </div>
          </div>
          <div class="pref-row">
            <div>
              <div class="pref-label">speed</div>
              <div class="pref-sub">{settings.speed.toFixed(2)}× · 1.0× is studio default</div>
            </div>
            <input type="range" min="0.7" max="2.0" step="0.05" value={settings.speed}
              oninput={(e) => changeSpeed(parseFloat((e.target as HTMLInputElement).value))} />
          </div>
          <div class="pref-row">
            <div>
              <div class="pref-label">volume</div>
              <div class="pref-sub">{Math.round(settings.volume * 100)}% · applied on every read</div>
            </div>
            <input type="range" min="0" max="1.5" step="0.05" value={settings.volume}
              oninput={(e) => changeVolume(parseFloat((e.target as HTMLInputElement).value))} />
          </div>
          <div class="pref-row">
            <div>
              <div class="pref-label">silence between paragraphs</div>
              <div class="pref-sub">{settings.silence_secs.toFixed(2)}s · breathing room between paragraphs</div>
            </div>
            <input type="range" min="0" max="1.5" step="0.05" value={settings.silence_secs}
              oninput={(e) => changeSilence(parseFloat((e.target as HTMLInputElement).value))} />
          </div>
        </div>

        <div class="card pref-card">
          <div class="pref-row">
            <div>
              <div class="pref-label">default language</div>
              <div class="pref-sub">used when yappy can't auto-detect a paragraph.</div>
            </div>
            <select value={settings.default_lang} onchange={(e) => changeLang((e.target as HTMLSelectElement).value)}>
              {#each LANGUAGES as l}
                <option value={l.code}>{l.flag ? l.flag + " " : ""}{l.label}</option>
              {/each}
            </select>
          </div>
          <div class="pref-row">
            <div>
              <div class="pref-label">auto-detect language per paragraph</div>
              <div class="pref-sub">switches voice & language at paragraph boundaries.</div>
            </div>
            <label class="toggle">
              <input type="checkbox" bind:checked={settings.auto_lang_detect}
                onchange={async () => settings && setSettings(settings)} />
              <span class="slider"></span>
            </label>
          </div>
          <div class="pref-row">
            <div>
              <div class="pref-label">karaoke highlight in player</div>
              <div class="pref-sub">shows the current sentence in the floating player.</div>
            </div>
            <label class="toggle">
              <input type="checkbox" bind:checked={settings.karaoke_in_player}
                onchange={async () => settings && setSettings(settings)} />
              <span class="slider"></span>
            </label>
          </div>
          <div class="pref-row">
            <div>
              <div class="pref-label">save reading history</div>
              <div class="pref-sub">last {settings.history_max} reads, locally on this mac.</div>
            </div>
            <label class="toggle">
              <input type="checkbox" bind:checked={settings.save_history}
                onchange={async () => settings && setSettings(settings)} />
              <span class="slider"></span>
            </label>
          </div>
        </div>

        <div class="card pref-card overrides">
          <div class="pref-row block">
            <div>
              <div class="pref-label">per-language voices</div>
              <div class="pref-sub">pick a different voice per language. a spanish voice for spanish, japanese for japanese, etc.</div>
            </div>
          </div>
          <div class="overrides-grid">
            {#each OVERRIDE_LANGS as lang}
              {@const lbl = LANGUAGES.find((l) => l.code === lang)}
              <div class="override-row">
                <span class="override-lang">{lbl?.flag ?? ""} {lbl?.label ?? lang}</span>
                <select value={settings.voice_overrides[lang] ?? "__inherit__"}
                  onchange={(e) => changeVoiceOverride(lang, (e.target as HTMLSelectElement).value)}>
                  <option value="__inherit__">use my default ({settings.voice})</option>
                  {#each voices as v}<option value={v.name}>{v.name} — {v.gender.toLowerCase()}</option>{/each}
                </select>
                <button class="btn-ghost tiny" disabled={!modelReady}
                  onclick={() => modelReady && sampleVoice(settings!.voice_overrides[lang] ?? settings!.voice, lang)} title="sample">
                  <svg width="10" height="10" viewBox="0 0 14 14" fill="currentColor"><path d="M3 1.5C3 0.7 3.85 0.25 4.5 0.7L12.5 6.2c0.6 0.4 0.6 1.3 0 1.7l-8 5.5c-0.7 0.4-1.5 0-1.5-0.8V1.5Z"/></svg>
                </button>
              </div>
            {/each}
          </div>
        </div>

        <div class="card pref-card">
          <div class="pref-row">
            <div>
              <div class="pref-label">hotkey — read what i'm looking at</div>
              <div class="pref-sub">reads selection, then active document, then OCRs the focused window.</div>
            </div>
            <HotkeyPicker value={settings.hotkey_read_now} onSave={(c) => changeHotkey("read_now", c)} />
          </div>
          <div class="pref-row">
            <div>
              <div class="pref-label">hotkey — pause / resume</div>
              <div class="pref-sub">same key toggles. stop from the player or tray.</div>
            </div>
            <HotkeyPicker value={settings.hotkey_pause_resume} onSave={(c) => changeHotkey("pause_resume", c)} />
          </div>
          <div class="pref-row">
            <div>
              <div class="pref-label">accessibility permission</div>
              <div class="pref-sub">needed so yappy can capture selected text from any app.</div>
            </div>
            <button class="btn-outline" onclick={() => requestMacosPermissions()}>open system settings…</button>
          </div>
        </div>

        <!-- Floating player customization -->
        <div class="card pref-card">
          <div class="pref-row block">
            <div>
              <div class="pref-label">floating player</div>
              <div class="pref-sub">where it lives, how big it is, what it shows.</div>
            </div>
            <div class="player-prefs">
              <div class="pref-line">
                <span class="lbl">position</span>
                <div class="position-grid">
                  {#each ["top-left","top-center","top-right","bottom-left","bottom-center","bottom-right"] as p}
                    <button class="pos-cell" class:active={settings.player_position_preset === p}
                      onclick={() => changePlayerPreset(p as PlayerPositionPreset)} title={p.replace("-", " ")}>
                      <span class="dot"></span>
                    </button>
                  {/each}
                </div>
              </div>
              <div class="pref-line">
                <span class="lbl">size</span>
                <div class="seg">
                  <button class:active={settings.player_size === "slim"} onclick={() => changePlayerSize("slim")}>slim</button>
                  <button class:active={settings.player_size === "regular"} onclick={() => changePlayerSize("regular")}>regular</button>
                  <button class:active={settings.player_size === "large"} onclick={() => changePlayerSize("large")}>large</button>
                </div>
              </div>
              <div class="pref-line">
                <span class="lbl">theme</span>
                <div class="seg">
                  <button class:active={settings.player_theme === "cream"} onclick={() => changePlayerTheme("cream")}>cream</button>
                  <button class:active={settings.player_theme === "dark"} onclick={() => changePlayerTheme("dark")}>dark</button>
                  <button class:active={settings.player_theme === "translucent"} onclick={() => changePlayerTheme("translucent")}>translucent</button>
                </div>
              </div>
              <div class="pref-line">
                <span class="lbl">opacity</span>
                <input type="range" min="0.5" max="1" step="0.05" value={settings.player_opacity}
                  oninput={(e) => changePlayerOpacity(parseFloat((e.target as HTMLInputElement).value))} />
                <span class="lbl-num">{Math.round(settings.player_opacity * 100)}%</span>
              </div>
              <div class="pref-line">
                <span class="lbl">auto-hide</span>
                <input type="range" min="0" max="60" step="1" value={settings.player_autohide_secs}
                  oninput={(e) => changeAutoHide(parseInt((e.target as HTMLInputElement).value))} />
                <span class="lbl-num">{settings.player_autohide_secs === 0 ? "off" : settings.player_autohide_secs + "s"}</span>
              </div>
              <div class="pref-line toggles">
                <label class="mini-toggle">
                  <input type="checkbox" bind:checked={settings.player_show_source} onchange={async () => settings && setSettings(settings)} />
                  <span>show source pill</span>
                </label>
                <label class="mini-toggle">
                  <input type="checkbox" bind:checked={settings.player_show_waves} onchange={async () => settings && setSettings(settings)} />
                  <span>show sound waves</span>
                </label>
                <label class="mini-toggle">
                  <input type="checkbox" bind:checked={settings.player_pinned} onchange={async () => settings && setSettings(settings)} />
                  <span>always pinned</span>
                </label>
                <label class="mini-toggle">
                  <input type="checkbox" checked={!settings.player_compact} onchange={(e) => { if (settings) { settings = { ...settings, player_compact: !(e.target as HTMLInputElement).checked }; setSettings(settings); } }} />
                  <span>start expanded</span>
                </label>
              </div>
            </div>
          </div>
        </div>

        <!-- App appearance + behaviours -->
        <div class="card pref-card">
          <div class="pref-row">
            <div>
              <div class="pref-label">app theme</div>
              <div class="pref-sub">cream is the default cozy look. dark for late-night sessions.</div>
            </div>
            <div class="seg">
              <button class:active={settings.app_theme === "cream"} onclick={() => changeAppTheme("cream")}>cream</button>
              <button class:active={settings.app_theme === "dark"} onclick={() => changeAppTheme("dark")}>dark</button>
              <button class:active={settings.app_theme === "system"} onclick={() => changeAppTheme("system")}>system</button>
            </div>
          </div>
          <div class="pref-row">
            <div>
              <div class="pref-label">ocr engine</div>
              <div class="pref-sub">"auto" picks apple vision on macos, paddleocr elsewhere. paddleocr is bundled — works offline on every platform.</div>
            </div>
            <div class="seg">
              <button class:active={settings.ocr_engine === "auto"} onclick={() => changeOcrEngine("auto")}>auto</button>
              <button class:active={settings.ocr_engine === "applevision"} onclick={() => changeOcrEngine("applevision")}>apple vision</button>
              <button class:active={settings.ocr_engine === "paddle"} onclick={() => changeOcrEngine("paddle")}>paddleocr</button>
            </div>
          </div>
          <div class="pref-row">
            <div>
              <div class="pref-label">notify when done</div>
              <div class="pref-sub">show a system notification when a reading finishes.</div>
            </div>
            <label class="toggle">
              <input type="checkbox" bind:checked={settings.notify_on_done} onchange={async () => settings && setSettings(settings)} />
              <span class="slider"></span>
            </label>
          </div>
          <div class="pref-row">
            <div>
              <div class="pref-label">sound effects</div>
              <div class="pref-sub">tiny chimes on ready / done / error.</div>
            </div>
            <label class="toggle">
              <input type="checkbox" bind:checked={settings.sound_effects} onchange={async () => settings && setSettings(settings)} />
              <span class="slider"></span>
            </label>
          </div>
          <div class="pref-row">
            <div>
              <div class="pref-label">launch at login</div>
              <div class="pref-sub">opens yappy quietly into the menu bar on startup.</div>
            </div>
            <label class="toggle">
              <input type="checkbox" bind:checked={settings.launch_at_login}
                onchange={async (e) => settings && setLaunchAtLogin((e.target as HTMLInputElement).checked).catch((er) => synthError = String(er))} />
              <span class="slider"></span>
            </label>
          </div>
        </div>

        <!-- Browser extension -->
        <div class="card pref-card bridge-card">
          <div class="bridge-head">
            <div>
              <div class="pref-label">browser extension</div>
              <div class="pref-sub">
                yappy can read the cleaned-up text from any chromium browser. install the extension, pair it once, done.
              </div>
            </div>
            <label class="toggle">
              <input type="checkbox" checked={bridge?.enabled ?? true}
                onchange={(e) => toggleBridge((e.target as HTMLInputElement).checked)} />
              <span class="slider"></span>
            </label>
          </div>

          <div class="bridge-status">
            <div class="bridge-pill" class:on={bridge && bridge.connections.length > 0}>
              {#if bridge && bridge.connections.length > 0}
                <span class="dot"></span> {bridge.connections.length} paired
              {:else if bridge?.enabled === false}
                <span class="dot off"></span> disabled
              {:else if bridge?.token}
                <span class="dot pending"></span> waiting for extension
              {:else}
                <span class="dot pending"></span> first-launch — token not set
              {/if}
            </div>
            <span class="bridge-port">127.0.0.1:{bridge?.port ?? 47898}</span>
          </div>

          {#if bridge && bridge.connections.length > 0}
            <ul class="bridge-conns">
              {#each bridge.connections as c}
                <li>
                  <span class="conn-browser">🌐 {c.browser}</span>
                  <span class="conn-meta">connected {fmtAgo(c.connected_at)} · seen {fmtAgo(c.last_seen)}</span>
                </li>
              {/each}
            </ul>
          {/if}

          <div class="pref-row token-row">
            <div style="min-width:0;">
              <div class="pref-label">pairing token</div>
              <div class="pref-sub" style="font-family: var(--font-mono); font-size: 11px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">
                {bridge?.token || "(none yet — first extension to connect will claim it)"}
              </div>
            </div>
            <div class="bridge-actions">
              <button class="btn-outline" onclick={copyToken} disabled={!bridge?.token}>copy</button>
              <button class="btn-outline" onclick={rePairExtensions} disabled={bridgeBusy}>re-pair</button>
              <button class="btn-outline danger" onclick={regenerateToken} disabled={bridgeBusy}>regenerate</button>
            </div>
          </div>

          <!-- Install the bundled extension into chromium-based browsers. -->
          <div class="install-card">
            <div>
              <strong>install the yappy extension</strong>
              <div class="pref-sub" style="margin-top: 2px;">
                yappy bundles the extension. click "reveal" to find the folder, then in chrome://extensions enable
                <strong>developer mode</strong> and choose <strong>load unpacked</strong> → pick that folder.
              </div>
            </div>
            <div class="bridge-actions">
              <button class="btn-outline" onclick={async () => { try { await revealExtensionFolder(); } catch (e) { bridgeToast(`couldn't open folder: ${e}`); } }}>
                reveal folder
              </button>
              <button class="btn-outline" onclick={async () => { try { const p = await getExtensionPath(); await navigator.clipboard.writeText(p); bridgeToast("path copied"); } catch (e) { bridgeToast(`copy failed: ${e}`); } }}>
                copy path
              </button>
            </div>
          </div>

          <div class="bridge-help">
            <strong>which button?</strong>
            <ul style="padding-left: 18px; margin: 4px 0 8px;">
              <li><strong>re-pair</strong> — clears the desktop's token and disconnects every extension. Whichever extension reconnects first claims the new pairing automatically. Use this when an extension shows "not paired" but you trust it.</li>
              <li><strong>regenerate</strong> — assigns a brand-new random token AND disconnects every extension. Old tokens are dead. Use this if you suspect a token leaked. You'll then need to <em>re-pair</em> to let the extension claim the new token.</li>
            </ul>
            <strong>typical recovery</strong>
            <ol>
              <li>click <em>re-pair</em>.</li>
              <li>open the extension in your browser and press its toolbar icon (or click "reconnect" in its popup).</li>
              <li>the first hello claims the desktop's empty token — paired.</li>
            </ol>
            <div class="bridge-help-row">
              <span class="pref-sub">need to open the extension page?</span>
              <button class="btn-outline" onclick={() => openBrowserExtensions("Google Chrome")}>chrome</button>
              <button class="btn-outline" onclick={() => openBrowserExtensions("Vivaldi")}>vivaldi</button>
              <button class="btn-outline" onclick={() => openBrowserExtensions("Brave Browser")}>brave</button>
              <button class="btn-outline" onclick={() => openBrowserExtensions("Microsoft Edge")}>edge</button>
              <button class="btn-outline" onclick={() => openBrowserExtensions("Arc")}>arc</button>
            </div>
          </div>

          {#if bridgeToastText}
            <div class="bridge-toast">{bridgeToastText}</div>
          {/if}
        </div>

        <!-- Maintenance -->
        <div class="card pref-card maintenance">
          <div class="pref-row">
            <div>
              <div class="pref-label">credits &amp; licenses</div>
              <div class="pref-sub">see who built the libraries and models yappy uses.</div>
            </div>
            <button class="btn-outline" onclick={() => (creditsOpen = true)}>open credits…</button>
          </div>
          <div class="pref-row">
            <div>
              <div class="pref-label">backup &amp; restore</div>
              <div class="pref-sub">export all settings as a json file or import one to sync across machines.</div>
            </div>
            <div style="display:flex; gap:8px;">
              <button class="btn-outline" onclick={doExportSettings}>export…</button>
              <button class="btn-outline" onclick={doImportSettings}>import…</button>
            </div>
          </div>
          <div class="pref-row">
            <div>
              <div class="pref-label">reset all settings</div>
              <div class="pref-sub">restore defaults. doesn't touch your history or model files.</div>
            </div>
            <button class="btn-outline danger" onclick={doResetSettings}>reset…</button>
          </div>
        </div>
      </section>
    {/if}
  {:else if activeSection === "history"}
    <section class="hist-wrap">
      <header class="section-head">
        <h2>history</h2>
        <p>your recent reads. replay anything, anytime.</p>
      </header>
      <div class="card pref-card"><HistoryList /></div>
    </section>
  {:else if activeSection === "diagnostics"}
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
            <button class="btn-outline" onclick={async () => { try { await revealLogFile(); } catch (e) { synthError = String(e); } }}>reveal log file</button>
            <button class="btn-outline" onclick={async () => { try { const p = await getLogPath(); await navigator.clipboard.writeText(p); } catch (e) { synthError = String(e); } }}>copy path</button>
            <button class="btn-outline" onclick={async () => { try { const t = await tailLog(64); homeLogTail = t; } catch (e) { synthError = String(e); } }}>show last 64 kb</button>
          </div>
        </div>
        {#if homeLogTail}
          <pre class="home-log-tail">{homeLogTail}</pre>
        {/if}
      </div>
    </section>
  {/if}
  </div>
</main>

{#if dragOver}
  <div class="drop-overlay">
    <div class="drop-inner">
      <div class="drop-icon">📥</div>
      <h2>drop to read</h2>
      <p>.pdf · .docx · .rtf · .md · .txt · .html and more</p>
    </div>
  </div>
{/if}

<HelpOverlay open={helpOpen} onClose={() => (helpOpen = false)} />
<AboutModal open={aboutOpen} onClose={() => (aboutOpen = false)} />
<CreditsModal open={creditsOpen} onClose={() => (creditsOpen = false)} />
<Onboarding open={onboardingOpen} onDone={async () => {
  onboardingOpen = false;
  if (settings) { settings = { ...settings, first_launch_done: true }; await setSettings(settings); }
}} />

{#if showConfetti}<Confetti onDone={() => (showConfetti = false)} />{/if}
{#if synthError}<div class="toast danger">⚠ {synthError}</div>{/if}
{#if captureEmptyToast}<div class="toast">no text found — try selecting first, or focus a supported app.</div>{/if}

<style>
.topbar {
  position: fixed; top: 0; left: 0; right: 0; z-index: 5;
  padding: 14px 24px;
  padding-left: 96px;
  user-select: none;
  -webkit-user-select: none;
  height: 60px;
  box-sizing: border-box;
}
.topbar-inner { display: flex; align-items: center; gap: 16px; justify-content: space-between; max-width: 1200px; margin: 0 auto; user-select: none; -webkit-user-select: none; }
.topbar-inner > * { cursor: default; }
.topbar .logo, .topbar .nav, .topbar .actions { cursor: pointer; }
.logo {
  font-family: var(--font-display);
  font-size: 32px; font-weight: 400;
  color: var(--pink-600);
  line-height: 1; padding: 0 6px;
  transform: rotate(-2deg);
  cursor: pointer; transition: transform 0.18s var(--ease-emph);
}
.logo:hover { transform: rotate(-3deg) scale(1.04); }
.nav { display: flex; gap: 2px; flex-wrap: wrap; }
.nav button {
  padding: 8px 12px; border-radius: 999px; color: var(--ink-700);
  font-size: 14px; font-weight: 700; transition: all 0.15s ease;
}
.nav button:hover { background: var(--cream-200); }
.nav button.active { background: var(--ink-900); color: var(--cream-100); }
.actions { display: flex; gap: 4px; }
.btn-ghost.icon-only {
  width: 32px; height: 32px; padding: 0; font-size: 16px; font-weight: 700;
  border-radius: 999px; color: var(--ink-500);
  display: flex; align-items: center; justify-content: center;
}
.btn-ghost.icon-only:hover { background: var(--cream-200); color: var(--ink-900); }

/* main is the full-window scroll container.
   Scrollbar lives on the actual window edge. Content gets centered by .main-inner. */
main {
  height: 100%;
  overflow-y: auto;
  overflow-x: hidden;
  scrollbar-gutter: stable;
}
.main-inner {
  max-width: 1100px;
  margin: 0 auto;
  padding: 100px 64px 64px;
}

/* HERO */
.hero { display: grid; grid-template-columns: 210px 1fr; gap: 32px; align-items: center; margin: 12px 0 40px; }
.hero-illust { display: flex; justify-content: center; }
.hero-title {
  font-family: var(--font-sans);
  font-size: 56px; line-height: 1.05; letter-spacing: -0.025em; font-weight: 700; margin: 0 0 14px;
  color: var(--ink-900);
}
.hero-sub { font-size: 19px; color: var(--ink-700); max-width: 38ch; line-height: 1.5; margin: 0 0 22px; font-weight: 500; }
.hero-cta { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; }
.hero-cta-hint { color: var(--ink-500); font-size: 14px; display: inline-flex; align-items: center; gap: 6px; font-weight: 500; flex-basis: 100%; }
.hero-cta-hint :global(kbd) { margin: 0 2px; }

.now-source {
  display: flex; align-items: center; gap: 10px; flex-wrap: wrap;
  margin-top: 18px; min-height: 28px;
}
.now-label { color: var(--ink-500); font-size: 13px; font-weight: 700; }
.np-mini { display: inline-flex; align-items: center; }
.thinking-chip {
  display: inline-flex; align-items: center; gap: 8px;
  padding: 4px 14px; background: var(--cream-200);
  border: 2.5px solid var(--ink-900); border-radius: 999px;
  font-size: 13px; font-weight: 700; color: var(--ink-900);
  box-shadow: 2px 2px 0 var(--ink-900);
}
.thinking-chip .dots { display: inline-flex; gap: 3px; }
.thinking-chip .dots i {
  width: 5px; height: 5px; border-radius: 999px; background: var(--ink-900);
  animation: think 1.05s ease-in-out infinite;
}
.thinking-chip .dots i:nth-child(2) { animation-delay: 0.15s; }
.thinking-chip .dots i:nth-child(3) { animation-delay: 0.3s; }
@keyframes think {
  0%, 60%, 100% { opacity: 0.25; transform: translateY(0); }
  30% { opacity: 1; transform: translateY(-2px); }
}

/* Cards */
.block-row { display: flex; flex-direction: column; margin-bottom: 36px; }
.model-card { padding: 24px; }
.model-card-head { display: flex; justify-content: space-between; align-items: center; gap: 18px; flex-wrap: wrap; }
.model-title { font-size: 19px; font-weight: 700; }
.model-sub { color: var(--ink-500); font-size: 14px; margin-top: 4px; font-weight: 500; }

.progress-row { margin-top: 18px; }
.progress-track {
  height: 14px; background: var(--cream-200); border: 2.5px solid var(--ink-900); border-radius: 999px; overflow: hidden; box-shadow: 2px 2px 0 var(--ink-900);
}
.progress-bar {
  height: 100%; width: var(--w, 0%);
  background: linear-gradient(90deg, var(--pink-500), var(--pink-600));
  transition: width 0.3s var(--ease-emph);
}
.progress-stats { display: flex; justify-content: space-between; margin-top: 8px; font-size: 13px; color: var(--ink-700); font-weight: 600; }
.progress-pct { color: var(--pink-600); }
.progress-file { font-family: var(--font-mono); font-size: 11px; color: var(--ink-500); margin-top: 6px; }

.try-card { padding: 24px; }
.try-head { margin-bottom: 14px; }
.try-title { font-size: 19px; font-weight: 700; }
.try-sub { color: var(--ink-500); font-size: 14px; margin-top: 4px; font-weight: 500; }
.try-card textarea {
  width: 100%; resize: vertical; min-height: 90px; padding: 14px;
  background: var(--cream-50);
  border: 2.5px solid var(--ink-900); border-radius: 14px;
  color: var(--ink-900); font-size: 15px; line-height: 1.55;
  font-family: var(--font-sans); font-weight: 500;
  box-shadow: 2px 2px 0 var(--ink-900);
}
.try-card textarea:focus { outline: none; box-shadow: 3px 3px 0 var(--pink-500); border-color: var(--pink-500); }
.try-actions { margin-top: 14px; display: flex; gap: 10px; align-items: center; }

/* Features */
.features { display: grid; grid-template-columns: 1fr 1fr; gap: 14px 50px; margin: 40px 0; padding-top: 28px; border-top: 2.5px dashed var(--ink-900); }
.feature h2 { font-family: var(--font-sans); font-size: 22px; font-weight: 700; margin: 12px 0 6px; letter-spacing: -0.01em; }
.feature p { color: var(--ink-700); font-size: 15px; line-height: 1.5; margin: 0; max-width: 40ch; font-weight: 500; }

/* ─── DASHBOARD ────────────────────────────────────────────────────────────
   App-first home screen. Status strip on top, primary action cards in a 2x2 grid,
   then "now reading" banner, recent reads, voice quick-switch. */

.status-strip {
  display: flex; flex-wrap: wrap; gap: 8px;
  margin: 8px 0 22px;
}
.status-pill {
  display: inline-flex; align-items: center; gap: 8px;
  padding: 6px 12px;
  background: var(--cream-100);
  border: 2px solid var(--ink-900);
  border-radius: 999px;
  box-shadow: 1.5px 1.5px 0 var(--ink-900);
  font-size: 12px; font-weight: 700;
}
.status-pill.on { background: var(--pink-300); }
.status-pill.playing { background: var(--pink-500); }
.status-pill .lbl { color: var(--ink-500); text-transform: lowercase; font-size: 11px; }
.status-pill .status-value {
  background: transparent; border: 0; padding: 0;
  color: var(--ink-900); font-weight: 700; cursor: pointer;
  display: inline-flex; align-items: center; gap: 6px;
}
.status-pill button.status-value:hover { text-decoration: underline; }
.status-pill .voice-dot {
  width: 9px; height: 9px; border-radius: 50%;
  background: var(--pink-500); border: 1.5px solid var(--ink-900);
}
.status-pill .voice-dot[data-id^="M"] { background: var(--lavender-500); }
.status-pill :global(kbd) {
  font-family: var(--font-mono); font-size: 10px;
  padding: 1px 5px; border-radius: 4px;
  background: var(--cream-200); border: 1.5px solid var(--ink-900); color: var(--ink-900);
}
.status-pill .mini-stop {
  background: var(--ink-900); color: var(--cream-100);
  width: 18px; height: 18px; border-radius: 999px;
  display: inline-flex; align-items: center; justify-content: center;
  font-size: 14px; line-height: 1; padding: 0;
  border: 0; cursor: pointer;
}
.status-pill .mini-stop:hover { background: var(--pink-600, #ff5f95); }

/* Primary action grid: 2x2 cards. Each is a single decisive action. */
.actions-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 14px;
  margin-bottom: 28px;
}
.action-card {
  background: var(--cream-100);
  border: 2.5px solid var(--ink-900);
  border-radius: 18px;
  padding: 22px 22px 18px;
  box-shadow: 3px 3px 0 var(--ink-900);
  display: flex; flex-direction: column; gap: 6px;
  text-align: left;
  cursor: pointer;
  transition: transform 0.12s var(--ease-emph), box-shadow 0.12s var(--ease-emph), background 0.15s ease;
  min-height: 148px;
  font-family: var(--font-sans);
  color: var(--ink-900);
}
.action-card:hover { transform: translate(-1px, -1px); box-shadow: 5px 5px 0 var(--ink-900); background: var(--cream-200); }
.action-card:active { transform: translate(1px, 1px); box-shadow: 1px 1px 0 var(--ink-900); }
.action-card.primary { background: var(--pink-300); }
.action-card.primary:hover { background: var(--pink-500); }
.action-card .action-icon {
  margin-bottom: 4px;
  display: flex; align-items: center;
}
.action-card .action-icon.icon-emoji { font-size: 36px; line-height: 1; }
.action-card .action-title {
  font-size: 19px; font-weight: 700; line-height: 1.15;
  letter-spacing: -0.01em;
}
.action-card .action-sub {
  font-size: 13px; color: var(--ink-700); font-weight: 500; line-height: 1.4;
}
.action-card .action-hint {
  margin-top: auto; padding-top: 8px;
  font-size: 11px; color: var(--ink-500); font-weight: 700;
  display: inline-flex; align-items: center; gap: 4px;
}
.action-card .action-hint :global(kbd) {
  font-family: var(--font-mono); font-size: 10px;
  padding: 1px 5px; border-radius: 4px;
  background: var(--cream-200); border: 1.5px solid var(--ink-900); color: var(--ink-900);
}

/* The text-paste action is a card too, but with a textarea instead of click. */
.action-card.text-card { cursor: text; }
.action-card.text-card:hover { transform: none; box-shadow: 3px 3px 0 var(--ink-900); background: var(--cream-100); }
.action-card.text-card textarea {
  flex: 1; min-height: 56px;
  width: 100%;
  background: var(--surface, #fff);
  border: 2px dashed var(--ink-300);
  border-radius: 10px;
  padding: 10px 12px;
  font-family: var(--font-sans);
  font-size: 13px; line-height: 1.4;
  color: var(--ink-900);
  resize: none;
  outline: none;
}
.action-card.text-card textarea:focus { border-color: var(--pink-500); border-style: solid; }
.action-card.text-card .text-card-actions { display: flex; gap: 6px; }

/* Small button variants for inside action cards */
.btn-pink.small, .btn-outline.small {
  padding: 6px 12px; font-size: 12px;
}

/* "Now reading" banner — separate from action grid */
.now-banner {
  display: flex; align-items: center; gap: 10px;
  margin: -12px 0 22px;
  padding: 10px 16px;
  background: var(--cream-200);
  border: 2px solid var(--ink-900);
  border-radius: 12px;
  font-size: 13px; font-weight: 600;
}
.now-banner .now-label { color: var(--ink-500); font-weight: 700; font-size: 12px; }
.now-banner .np-mini { margin-left: auto; }

/* Recent reads on home */
.recents { margin: 20px 0 28px; }

/* ─── HOME v3 ──────────────────────────────────────────────────────────────
   One big focal panel; secondary actions are a row of small buttons;
   recents + voice quick-switch below. Dense, app-like, no marketing. */

.read-panel {
  margin: 12px 0 28px;
  padding: 22px 26px 20px;
  background: var(--cream-100);
  border: 2.5px solid var(--ink-900);
  border-radius: 22px;
  box-shadow: 4px 4px 0 var(--ink-900);
  display: flex; flex-direction: column; gap: 14px;
  position: relative;
  transition: background 0.3s ease;
}
.read-panel.playing { background: linear-gradient(to bottom right, var(--cream-100) 50%, #ffe4ee 100%); }
.read-panel.thinking { background: var(--cream-200); }

.read-panel-status {
  display: flex; align-items: center; gap: 8px; flex-wrap: wrap;
  font-size: 12px; font-weight: 700; color: var(--ink-500);
}
.rp-status { display: inline-flex; align-items: center; gap: 6px; color: var(--ink-700); }
.rp-dot {
  width: 7px; height: 7px; border-radius: 999px;
  background: var(--ink-300);
}
.rp-dot.on { background: #2ea35e; box-shadow: 0 0 0 3px rgba(46,163,94,0.18); }
.rp-sep { opacity: 0.4; }
.rp-voice, .rp-bridge {
  background: transparent; border: 0; padding: 0;
  color: var(--ink-700); font-weight: 700; cursor: pointer;
  display: inline-flex; align-items: center; gap: 5px; font-size: 12px;
  font-family: var(--font-sans);
}
.rp-voice:hover, .rp-bridge:hover { color: var(--ink-900); text-decoration: underline; }
.rp-bridge.on { color: var(--ink-900); }
.rp-voice .voice-dot {
  width: 10px; height: 10px; border-radius: 50%;
  background: var(--pink-500); border: 1.5px solid var(--ink-900);
}
.rp-voice .voice-dot[data-id^="M"] { background: var(--lavender-500); }

.rp-state-line {
  display: inline-flex; align-items: center; gap: 10px;
  padding: 8px 14px;
  background: var(--cream-200);
  border: 2px solid var(--ink-900);
  border-radius: 12px;
  font-size: 13px; font-weight: 700; color: var(--ink-900);
  align-self: flex-start;
}
.rp-state-line.subtle {
  background: transparent; border-color: var(--ink-300);
  color: var(--ink-500); font-weight: 600;
}
.rp-state-line .rp-label {
  font-size: 11px; color: var(--ink-500); text-transform: uppercase;
  letter-spacing: 0.04em; font-weight: 800;
}

/* The big primary read button */
.rp-cta {
  display: flex; align-items: stretch; gap: 12px;
}
.rp-primary {
  flex: 1;
  display: flex; align-items: center; gap: 18px;
  padding: 18px 22px;
  background: var(--pink-500);
  border: 2.5px solid var(--ink-900);
  border-radius: 18px;
  box-shadow: 3px 3px 0 var(--ink-900);
  color: var(--ink-900);
  font-family: var(--font-sans);
  cursor: pointer;
  text-align: left;
  transition: transform 0.12s var(--ease-emph), box-shadow 0.12s var(--ease-emph), background 0.15s ease;
}
.rp-primary:hover { background: var(--pink-600, #ff5f95); transform: translate(-1px, -1px); box-shadow: 5px 5px 0 var(--ink-900); }
.rp-primary:active { transform: translate(1px, 1px); box-shadow: 1px 1px 0 var(--ink-900); }
.rp-primary-icon { flex-shrink: 0; }
.rp-primary-text { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
.rp-primary-title { font-size: 22px; font-weight: 700; letter-spacing: -0.015em; line-height: 1.15; }
.rp-primary-sub {
  font-size: 12px; color: var(--ink-700); font-weight: 600;
  display: inline-flex; align-items: center; gap: 8px; flex-wrap: wrap;
}
.rp-primary-sub :global(kbd) {
  font-family: var(--font-mono); font-size: 10px;
  padding: 1px 5px; border-radius: 4px;
  background: var(--cream-100); border: 1.5px solid var(--ink-900); color: var(--ink-900);
}

.rp-stop {
  display: inline-flex; align-items: center; gap: 5px;
  padding: 0 16px;
  background: var(--cream-100);
  border: 2.5px solid var(--ink-900);
  border-radius: 16px;
  box-shadow: 3px 3px 0 var(--ink-900);
  color: var(--ink-900); font-weight: 700; font-size: 13px;
  font-family: var(--font-sans); cursor: pointer;
  transition: transform 0.12s var(--ease-emph), background 0.15s ease;
}
.rp-stop:hover { transform: translate(-1px, -1px); background: var(--pink-300); }

/* Secondary action row */
.rp-secondary {
  display: flex; gap: 6px; flex-wrap: wrap;
}
.rp-sec {
  display: inline-flex; align-items: center; gap: 6px;
  padding: 8px 14px;
  background: transparent;
  border: 2px solid var(--ink-300);
  border-radius: 12px;
  color: var(--ink-700); font-weight: 700; font-size: 12px;
  font-family: var(--font-sans); cursor: pointer;
  transition: border-color 0.15s ease, background 0.15s ease, color 0.15s ease;
}
.rp-sec:hover { border-color: var(--ink-900); background: var(--cream-200); color: var(--ink-900); }
.rp-sec.active { border-color: var(--ink-900); background: var(--cream-200); color: var(--ink-900); }
.rp-sec .emoji { font-size: 13px; }
.rp-sec .hint {
  margin-left: 2px;
  font-family: var(--font-mono); font-size: 10px;
  padding: 1px 5px; border-radius: 4px;
  background: var(--cream-200); border: 1.5px solid var(--ink-300); color: var(--ink-700);
}

.rp-paste {
  padding: 14px;
  background: var(--cream-200);
  border: 2px dashed var(--ink-300);
  border-radius: 12px;
  display: flex; flex-direction: column; gap: 10px;
}
.rp-paste textarea {
  width: 100%;
  background: var(--surface, #fff);
  border: 2px solid var(--ink-300);
  border-radius: 10px;
  padding: 10px 12px;
  font-family: var(--font-sans);
  font-size: 14px; line-height: 1.5; color: var(--ink-900);
  resize: vertical; outline: none;
}
.rp-paste textarea:focus { border-color: var(--pink-500); }
.rp-paste-actions { display: flex; gap: 6px; }

.recents-v3 { margin: 12px 0 24px; }

/* On narrow widths, primary CTA goes single column */
@media (max-width: 760px) {
  .rp-cta { flex-direction: column; }
  .rp-primary-title { font-size: 18px; }
  .rp-primary { padding: 14px 16px; gap: 12px; }
}

/* Quick voices */
.quickvoices { margin: 18px 0 8px; }
.qv-head { display: flex; justify-content: space-between; align-items: end; margin: 0 0 14px; }
.qv-head h2 { font-family: var(--font-sans); font-size: 22px; margin: 0; letter-spacing: -0.015em; font-weight: 700; }
.link { color: var(--pink-600); font-size: 14px; font-weight: 700; padding: 8px 12px; border-radius: 999px; }
.link:hover { background: var(--cream-200); }
.qv-row { display: flex; flex-wrap: wrap; gap: 10px; }
.voice-pill {
  display: inline-flex; align-items: center; gap: 8px;
  padding: 8px 16px;
  background: var(--surface);
  border: 2.5px solid var(--ink-900);
  border-radius: 999px;
  font-size: 14px; font-weight: 700;
  color: var(--ink-900);
  box-shadow: 2px 2px 0 var(--ink-900);
  transition: transform 0.12s var(--ease-emph), background 0.15s ease;
}
.voice-pill:hover { background: var(--cream-200); transform: translate(-1px, -1px); box-shadow: 3px 3px 0 var(--ink-900); }
.voice-pill.active { background: var(--pink-500); color: var(--ink-900); }
.voice-pill .dot { width: 10px; height: 10px; border-radius: 50%; background: var(--ink-900); border: 1.5px solid var(--ink-900); }
.voice-pill .dot[data-id^="F"] { background: var(--pink-500); }
.voice-pill .dot[data-id^="M"] { background: var(--lavender-500); }

/* Voices grid */
.voices-grid-wrap { max-width: 1080px; margin: 0 auto; }
.voice-controls { margin: 0 0 18px; display: flex; gap: 12px; }
.search {
  flex: 1; padding: 12px 16px;
  background: var(--surface); border: 2.5px solid var(--ink-900);
  border-radius: 999px; font-size: 14px; font-weight: 600;
  color: var(--ink-900); box-shadow: 2px 2px 0 var(--ink-900);
}
.search:focus { outline: none; box-shadow: 3px 3px 0 var(--pink-500); border-color: var(--pink-500); }

.section-head { margin-bottom: 22px; }
.section-head h2 { font-family: var(--font-sans); font-size: 32px; margin: 0; letter-spacing: -0.025em; font-weight: 700; }
.section-head p { color: var(--ink-500); margin: 4px 0 0; font-size: 15px; font-weight: 500; }
.voices-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 16px; }
.voice-card {
  background: var(--surface); border: 2.5px solid var(--ink-900); border-radius: 22px; padding: 18px;
  text-align: left; transition: transform 0.18s var(--ease-out), box-shadow 0.18s var(--ease-out);
  display: flex; flex-direction: column; gap: 10px;
  box-shadow: 3px 3px 0 var(--ink-900); color: var(--ink-900);
}
.voice-card:hover { transform: translate(-2px, -2px); box-shadow: 6px 6px 0 var(--ink-900); }
.voice-card.active { background: var(--pink-300); }
.vc-head { display: flex; justify-content: space-between; align-items: flex-start; }
.voice-avatar {
  width: 48px; height: 48px; border-radius: 14px;
  display: flex; align-items: center; justify-content: center;
  font-family: var(--font-sans); font-weight: 700; font-size: 20px;
  background: var(--cream-200); border: 2.5px solid var(--ink-900); color: var(--ink-900);
  box-shadow: 2px 2px 0 var(--ink-900);
}
.voice-avatar[data-id^="M"] { background: var(--lavender-300); }
.voice-avatar[data-id^="F"] { background: var(--pink-300); }
.sample-btn {
  width: 32px; height: 32px; border-radius: 10px;
  display: flex; align-items: center; justify-content: center;
  background: var(--surface); border: 2.5px solid var(--ink-900); color: var(--ink-900);
  box-shadow: 2px 2px 0 var(--ink-900);
}
.sample-btn:hover { background: var(--pink-500); }
.voice-meta { display: flex; flex-direction: column; gap: 4px; }
.voice-name { font-weight: 700; font-size: 17px; letter-spacing: -0.01em; }
.voice-tags { display: flex; gap: 6px; flex-wrap: wrap; }
.voice-tag {
  font-size: 11px; padding: 3px 9px; border-radius: 999px;
  background: var(--cream-200); color: var(--ink-700); font-weight: 700;
  border: 1.5px solid var(--ink-900);
}
.voice-desc { font-size: 13px; color: var(--ink-700); margin: 4px 0 8px; line-height: 1.45; flex: 1; font-weight: 500; }
.voice-actions { display: flex; justify-content: space-between; align-items: center; }
.voice-id { font-family: var(--font-mono); font-size: 11px; color: var(--ink-500); font-weight: 700; }
.voice-active-pill { font-size: 11px; padding: 3px 10px; border-radius: 999px; background: var(--ink-900); color: var(--cream-100); font-weight: 700; }

/* Preferences */
.prefs { max-width: 820px; margin: 0 auto; }
.pref-card { padding: 4px 22px; margin-bottom: 16px; }
.pref-row { display: flex; justify-content: space-between; align-items: center; gap: 24px; padding: 20px 0; border-bottom: 2px dashed var(--ink-300); }
.pref-row.block { display: block; }
.pref-row:last-child { border-bottom: none; }
.pref-label { font-size: 15px; font-weight: 700; }
.pref-sub { color: var(--ink-500); font-size: 13px; margin-top: 4px; font-weight: 500; }

.seg { display: inline-flex; padding: 3px; gap: 2px; background: var(--cream-200); border: 2.5px solid var(--ink-900); border-radius: 999px; box-shadow: 2px 2px 0 var(--ink-900); }
.seg button { padding: 6px 14px; font-size: 13px; font-weight: 700; border-radius: 999px; color: var(--ink-700); transition: all 0.12s ease; }
.seg button:hover { color: var(--ink-900); }
.seg button.active { background: var(--ink-900); color: var(--cream-100); }

input[type="range"] {
  -webkit-appearance: none; appearance: none; height: 14px; width: 220px;
  background: var(--cream-200);
  border: 2.5px solid var(--ink-900);
  border-radius: 999px; outline: none;
  box-shadow: 2px 2px 0 var(--ink-900);
}
input[type="range"]::-webkit-slider-thumb {
  -webkit-appearance: none; appearance: none;
  width: 22px; height: 22px; border-radius: 50%;
  background: var(--pink-500); border: 2.5px solid var(--ink-900);
  cursor: grab; box-shadow: 2px 2px 0 var(--ink-900);
}
select {
  background: var(--surface); border: 2.5px solid var(--ink-900); color: var(--ink-900);
  padding: 8px 14px; border-radius: 12px; min-width: 200px;
  font-size: 14px; font-weight: 700;
  box-shadow: 2px 2px 0 var(--ink-900); font-family: var(--font-sans);
}

.overrides .overrides-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 10px 16px; margin-top: 14px; padding-bottom: 14px; }
.override-row { display: flex; align-items: center; gap: 8px; padding: 6px 0; }
.override-lang { flex: 1; font-weight: 700; font-size: 14px; color: var(--ink-900); }
.override-row select { min-width: 0; flex: 1.4; padding: 6px 10px; font-size: 13px; }
.btn-ghost.tiny {
  width: 28px; height: 28px; padding: 0;
  display: flex; align-items: center; justify-content: center;
  border-radius: 8px; background: var(--cream-200); border: 2px solid var(--ink-900);
  box-shadow: 2px 2px 0 var(--ink-900);
}
.btn-ghost.tiny:hover { background: var(--pink-500); }
.btn-ghost.tiny:disabled { opacity: 0.4; cursor: not-allowed; }

.toggle { position: relative; display: inline-block; width: 52px; height: 30px; }
.toggle input { opacity: 0; width: 0; height: 0; }
.toggle .slider {
  position: absolute; cursor: pointer; inset: 0;
  background-color: var(--cream-200);
  border: 2.5px solid var(--ink-900);
  border-radius: 999px; transition: background 0.2s;
  box-shadow: 2px 2px 0 var(--ink-900);
}
.toggle .slider::before {
  position: absolute; content: ""; height: 20px; width: 20px;
  left: 2px; top: 2px; background: var(--ink-900); border-radius: 50%;
  transition: transform 0.2s var(--ease-emph);
}
.toggle input:checked + .slider { background: var(--pink-500); }
.toggle input:checked + .slider::before { transform: translateX(22px); }

.hist-wrap, .diagnostics { max-width: 780px; margin: 0 auto; }

.toast {
  position: fixed; bottom: 28px; left: 50%; transform: translateX(-50%);
  padding: 14px 22px; border-radius: 999px;
  background: var(--surface); border: 2.5px solid var(--ink-900);
  box-shadow: var(--shadow-hd-2);
  font-size: 14px; color: var(--ink-900); font-weight: 600;
  z-index: 100;
  animation: toast-in 0.3s var(--ease-out);
}
.toast.danger { background: var(--pink-300); }
@keyframes toast-in {
  from { opacity: 0; transform: translate(-50%, 10px); }
  to { opacity: 1; transform: translate(-50%, 0); }
}

/* Drop overlay */
.drop-overlay {
  position: fixed; inset: 0; z-index: 250;
  background: rgba(255, 248, 215, 0.92);
  display: flex; align-items: center; justify-content: center;
  border: 4px dashed var(--pink-600);
  animation: drop-in 0.2s ease;
}
@keyframes drop-in { from { opacity: 0; } to { opacity: 1; } }
.drop-inner { text-align: center; }
.drop-icon { font-size: 64px; margin-bottom: 10px; animation: bounce 0.6s ease-in-out infinite alternate; }
@keyframes bounce { from { transform: translateY(0); } to { transform: translateY(-8px); } }
.drop-inner h2 { font-family: var(--font-display); font-size: 48px; margin: 0 0 8px; color: var(--pink-600); transform: rotate(-2deg); font-weight: 400; }
.drop-inner p { color: var(--ink-700); font-weight: 600; font-size: 16px; }

/* Default voice prominent row */
.default-voice-card { padding: 4px 22px 22px; background: linear-gradient(180deg, var(--pink-300) 0%, var(--surface) 38%); }
.dv-row { display: flex; align-items: center; gap: 14px; flex-wrap: wrap; margin-top: 14px; }
.voice-avatar.lg { width: 58px; height: 58px; border-radius: 18px; font-size: 24px; }
.dv-meta { flex: 1; min-width: 200px; }
.dv-name { font-weight: 700; font-size: 18px; }
.dv-desc { color: var(--ink-700); font-size: 13px; font-weight: 500; }
.dv-row select { min-width: 180px; }

/* Player customization */
.player-prefs { display: flex; flex-direction: column; gap: 14px; margin-top: 14px; padding: 6px 0 6px; }
.pref-line { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; }
.pref-line .lbl { font-size: 13px; font-weight: 700; color: var(--ink-500); min-width: 90px; }
.pref-line .lbl-num { font-family: var(--font-mono); font-size: 12px; color: var(--ink-700); font-weight: 700; min-width: 40px; }
.position-grid {
  display: grid; grid-template-columns: repeat(3, 28px); grid-template-rows: repeat(2, 28px);
  gap: 4px; padding: 4px;
  background: var(--cream-50);
  border: 2.5px solid var(--ink-900); border-radius: 12px;
  box-shadow: 2px 2px 0 var(--ink-900);
}
.pos-cell {
  width: 28px; height: 28px; border-radius: 6px;
  background: var(--surface); border: 1.5px solid var(--ink-900);
  display: flex; align-items: center; justify-content: center;
  transition: background 0.12s ease;
}
.pos-cell:hover { background: var(--cream-200); }
.pos-cell.active { background: var(--pink-500); }
.pos-cell .dot { width: 6px; height: 6px; background: var(--ink-900); border-radius: 999px; }
.pref-line.toggles { gap: 18px 22px; }
.mini-toggle { display: inline-flex; align-items: center; gap: 6px; font-size: 13px; font-weight: 600; color: var(--ink-700); cursor: pointer; }
.mini-toggle input { width: 16px; height: 16px; }
.maintenance .btn-outline.danger { background: var(--pink-300); }

/* Browser extension card */
.bridge-card { padding: 22px 22px 18px; }
.bridge-head { display: flex; justify-content: space-between; align-items: flex-start; gap: 24px; }
.bridge-status {
  display: flex; align-items: center; gap: 14px;
  margin: 14px 0 8px;
}
.bridge-pill {
  display: inline-flex; align-items: center; gap: 8px;
  padding: 5px 12px; border-radius: 999px;
  background: var(--cream-200);
  border: 2px solid var(--ink-900);
  font-size: 12px; font-weight: 700; color: var(--ink-700);
  box-shadow: 1.5px 1.5px 0 var(--ink-900);
}
.bridge-pill.on { background: var(--pink-300); color: var(--ink-900); }
.bridge-pill .dot { width: 8px; height: 8px; border-radius: 999px; background: var(--ink-700); }
.bridge-pill.on .dot { background: #2ea35e; box-shadow: 0 0 0 3px rgba(46,163,94,0.18); animation: pulse 1.4s ease-in-out infinite; }
.bridge-pill .dot.off { background: var(--ink-300); }
.bridge-pill .dot.pending { background: #e8a73a; }
@keyframes pulse { 0%,100% { transform: scale(1); } 50% { transform: scale(1.25); } }
.bridge-port { font-family: var(--font-mono); font-size: 11px; color: var(--ink-500); font-weight: 700; }

.bridge-conns {
  list-style: none; padding: 0; margin: 6px 0 12px;
  display: flex; flex-direction: column; gap: 6px;
}
.bridge-conns li {
  display: flex; justify-content: space-between; align-items: center; gap: 10px;
  padding: 8px 12px;
  background: var(--cream-100); border: 1.5px solid var(--ink-300);
  border-radius: 10px;
  font-size: 12px;
}
.conn-browser { font-weight: 700; color: var(--ink-900); }
.conn-meta { font-size: 11px; color: var(--ink-500); font-weight: 600; }

.token-row { border-top: 2px dashed var(--ink-300); padding-top: 14px; margin-top: 4px; gap: 14px; }
.bridge-actions { display: flex; gap: 6px; flex-shrink: 0; }
.bridge-actions .btn-outline.danger { background: var(--pink-300); }

.bridge-help {
  margin-top: 14px;
  padding: 12px 14px;
  background: var(--cream-100); border: 1.5px solid var(--ink-300);
  border-radius: 10px; font-size: 12px; color: var(--ink-700); line-height: 1.5;
}
.bridge-help ol { padding-left: 18px; margin: 4px 0 8px; }
.bridge-help-row { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; margin-top: 4px; }
.bridge-help-row .btn-outline { padding: 4px 10px; font-size: 11px; }

.bridge-toast {
  margin-top: 12px;
  padding: 8px 14px;
  background: var(--pink-500);
  border: 2px solid var(--ink-900);
  border-radius: 10px;
  font-size: 12px; font-weight: 700; color: var(--ink-900);
  box-shadow: 2px 2px 0 var(--ink-900);
}

.home-log-tail {
  margin-top: 14px;
  padding: 12px 14px;
  background: var(--ink-900);
  color: var(--cream-100);
  border-radius: 10px;
  font-family: var(--font-mono);
  font-size: 11px;
  line-height: 1.4;
  max-height: 360px;
  overflow: auto;
  white-space: pre-wrap;
  word-break: break-all;
}

.install-card {
  display: flex; align-items: center; justify-content: space-between;
  gap: 16px;
  margin: 14px 0 6px;
  padding: 12px 16px;
  background: var(--cream-200);
  border: 2px dashed var(--ink-900);
  border-radius: 10px;
  font-size: 12px; color: var(--ink-700); line-height: 1.5;
}
.install-card strong { color: var(--ink-900); }

/* Responsive */
@media (max-width: 760px) {
  .main-inner { padding: 96px 24px 48px; }
  .topbar { padding: 12px 14px 12px 86px; }
  .nav { gap: 0; }
  .nav button { padding: 6px 8px; font-size: 13px; }
  .actions { display: none; }
  .hero { grid-template-columns: 1fr; gap: 16px; text-align: center; }
  .hero-illust { transform: scale(0.85); }
  .hero-title { font-size: 36px; }
  .hero-sub { font-size: 15px; margin: 0 auto 16px; }
  .hero-cta { justify-content: center; }
  .hero-sub, .now-source { justify-content: center; }
  .features { grid-template-columns: 1fr; gap: 8px 0; }
  .actions-grid { grid-template-columns: 1fr; }
  .action-card { min-height: 0; padding: 18px; }
  .overrides .overrides-grid { grid-template-columns: 1fr; }
  .pref-row { flex-direction: column; align-items: stretch; gap: 12px; }
  .pref-row > *:last-child { align-self: flex-end; }
  input[type="range"] { width: 100%; }
  select { width: 100%; }
}
@media (max-width: 540px) {
  .topbar { padding-left: 76px; }
  .nav { display: none; }
  .hero-title { font-size: 30px; }
  .voices-grid { grid-template-columns: 1fr; }
}
</style>
