<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
  import { readTextFile, writeTextFile } from "@tauri-apps/plugin-fs";
  import HotkeyPicker from "$lib/HotkeyPicker.svelte";
  import CreditsModal from "$lib/CreditsModal.svelte";
  import { isIOS } from "$lib/platform";
  import { notifyError } from "$lib/ui";
  import {
    LANGUAGES,
    type Settings,
    type Voice,
    type Quality,
    type AppTheme,
    type OcrEngine,
    type PlayerPositionPreset,
    type PlayerTheme,
    type BridgeStatus,
    getSettings,
    listVoices,
    isModelReady,
    setVoice,
    sampleVoice,
    setSpeed,
    setVolume,
    setSilence,
    setDefaultLang,
    setQuality,
    setVoiceOverride,
    setHotkey,
    setPlayerPreset,
    setPlayerTheme,
    setPlayerSize,
    setAppTheme,
    setOcrEngine,
    setSettings,
    setLaunchAtLogin,
    resetSettings,
    exportSettings,
    importSettings,
    requestMacosPermissions,
    bridgeStatus,
    bridgeRegenerateToken,
    bridgeClearPairing,
    setBridgeEnabled,
    openBrowserExtensions,
    getExtensionPath,
    revealExtensionFolder,
    onBridgePaired,
    onBridgeDisconnected,
    onBridgeTokenChanged,
    puenteEstado,
    puenteEmparejarNuevo,
    puenteRevocar,
    type EstadoPuente,
  } from "$lib/ipc";

  let settings: Settings | null = $state(null);
  let voices: Voice[] = $state([]);
  let modelReady = $state(false);
  let bridge: BridgeStatus | null = $state(null);
  let bridgeBusy = $state(false);
  let bridgeToastText: string | null = $state(null);
  let creditsOpen = $state(false);
  let puente = $state<EstadoPuente | null>(null);
  let puenteQR = $state<{ enlace: string; qr_svg: string } | null>(null);

  async function refrescarPuente() {
    try { puente = await puenteEstado(); } catch {}
  }
  async function emparejarMovil() {
    try {
      puenteQR = await puenteEmparejarNuevo();
      await refrescarPuente();
    } catch (e) { bridgeToast(String(e)); }
  }
  async function revocarPuente(prefijo: string) {
    try { await puenteRevocar(prefijo); await refrescarPuente(); } catch {}
  }

  const OVERRIDE_LANGS: string[] = ["en", "es", "fr", "de", "it", "pt", "nl", "ja", "ko", "ru"];
  let cleanups: (() => void)[] = [];

  onMount(async () => {
    settings = await getSettings();
    voices = await listVoices();
    modelReady = await isModelReady();
    await refreshBridgeStatus();
    cleanups.push(await onBridgePaired(() => refreshBridgeStatus()));
    cleanups.push(await onBridgeDisconnected(() => refreshBridgeStatus()));
    cleanups.push(await onBridgeTokenChanged(() => refreshBridgeStatus()));
  });
  onDestroy(() => cleanups.forEach((c) => c()));

  async function pickVoice(v: Voice) {
    if (!settings) return;
    settings = { ...settings, voice: v.name };
    await setVoice(v.name);
  }
  async function playVoiceSample(v: Voice, e?: Event) {
    e?.stopPropagation();
    if (modelReady) await sampleVoice(v.name);
  }
  async function changeSpeed(v: number) { if (!settings) return; settings = { ...settings, speed: v }; await setSpeed(v); }
  async function changeVolume(v: number) { if (!settings) return; settings = { ...settings, volume: v }; await setVolume(v); }
  async function changeSilence(v: number) { if (!settings) return; settings = { ...settings, silence_secs: v }; await setSilence(v); }
  async function changeLang(code: string) { if (!settings) return; settings = { ...settings, default_lang: code }; await setDefaultLang(code); }
  async function changeQuality(q: Quality) { if (!settings) return; settings = { ...settings, quality: q }; await setQuality(q); }
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
      settings = action === "read_now"
        ? { ...settings, hotkey_read_now: combo }
        : { ...settings, hotkey_pause_resume: combo };
    } catch (e) { notifyError(String(e)); }
  }
  async function changePlayerPreset(p: PlayerPositionPreset) { if (!settings) return; settings = { ...settings, player_position_preset: p }; await setPlayerPreset(p); }
  async function changePlayerTheme(t: PlayerTheme) { if (!settings) return; settings = { ...settings, player_theme: t }; await setPlayerTheme(t); }
  async function changePlayerSize(s: string) { if (!settings) return; settings = { ...settings, player_size: s }; await setPlayerSize(s); }
  async function changeAppTheme(t: AppTheme) { if (!settings) return; settings = { ...settings, app_theme: t }; await setAppTheme(t); document.documentElement.dataset.theme = t; }
  async function changeOcrEngine(e: OcrEngine) { if (!settings) return; settings = { ...settings, ocr_engine: e }; await setOcrEngine(e); }
  async function changePlayerOpacity(v: number) { if (!settings) return; settings = { ...settings, player_opacity: v }; await setSettings(settings); }
  async function changeAutoHide(v: number) { if (!settings) return; settings = { ...settings, player_autohide_secs: v }; await setSettings(settings); }
  async function doResetSettings() {
    if (!confirm("reset all settings to defaults? this can't be undone.")) return;
    settings = await resetSettings();
  }
  async function doExportSettings() {
    try {
      const json = await exportSettings();
      const path = await saveDialog({
        defaultPath: `yappy-settings-${new Date().toISOString().slice(0, 10)}.json`,
        filters: [{ name: "Yappy settings", extensions: ["json"] }],
      });
      if (path) await writeTextFile(path, json);
    } catch (e) { notifyError(String(e)); }
  }
  async function doImportSettings() {
    try {
      const path = await openDialog({ filters: [{ name: "Yappy settings", extensions: ["json"] }] });
      if (typeof path === "string") {
        const txt = await readTextFile(path);
        settings = await importSettings(txt);
      }
    } catch (e) { notifyError(String(e)); }
  }

  async function refreshBridgeStatus() { try { bridge = await bridgeStatus(); await refrescarPuente(); } catch {} }
  function bridgeToast(t: string) { bridgeToastText = t; setTimeout(() => (bridgeToastText = null), 2500); }
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
      bridgeToast(`new token — starts with ${fresh.slice(0, 8)}… (re-pair the extension)`);
    } catch (e) { bridgeToast(`regenerate failed: ${String(e).slice(0, 80)}`); }
    finally { bridgeBusy = false; }
  }
  async function rePairExtensions() {
    if (bridgeBusy) return;
    bridgeBusy = true;
    try {
      await bridgeClearPairing();
      await refreshBridgeStatus();
      bridgeToast("ready — next extension to connect will claim the token");
    } catch (e) { bridgeToast(`re-pair failed: ${String(e).slice(0, 80)}`); }
    finally { bridgeBusy = false; }
  }
  async function toggleBridge(enabled: boolean) { await setBridgeEnabled(enabled); await refreshBridgeStatus(); }
  function fmtAgo(unix: number): string {
    if (!unix) return "—";
    const diff = Math.max(0, Math.floor(Date.now() / 1000) - unix);
    if (diff < 60) return `${diff}s ago`;
    if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
    if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
    return `${Math.floor(diff / 86400)}d ago`;
  }
</script>

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
          <input type="checkbox" bind:checked={settings.auto_lang_detect} onchange={async () => settings && setSettings(settings)} />
          <span class="slider"></span>
        </label>
      </div>
      <div class="pref-row">
        <div>
          <div class="pref-label">karaoke highlight in player</div>
          <div class="pref-sub">shows the current sentence in the floating player.</div>
        </div>
        <label class="toggle">
          <input type="checkbox" bind:checked={settings.karaoke_in_player} onchange={async () => settings && setSettings(settings)} />
          <span class="slider"></span>
        </label>
      </div>
      <div class="pref-row">
        <div>
          <div class="pref-label">save reading history</div>
          <div class="pref-sub">last {settings.history_max} reads, locally on this mac.</div>
        </div>
        <label class="toggle">
          <input type="checkbox" bind:checked={settings.save_history} onchange={async () => settings && setSettings(settings)} />
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

    {#if !$isIOS}
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
    {/if}

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
      {#if !$isIOS}
      <div class="pref-row">
        <div>
          <div class="pref-label">launch at login</div>
          <div class="pref-sub">opens yappy quietly into the menu bar on startup.</div>
        </div>
        <label class="toggle">
          <input type="checkbox" bind:checked={settings.launch_at_login}
            onchange={async (e) => settings && setLaunchAtLogin((e.target as HTMLInputElement).checked).catch((er) => notifyError(String(er)))} />
          <span class="slider"></span>
        </label>
      </div>
      {/if}
    </div>

    {#if !$isIOS}
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
    {/if}

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

  <!-- ── El puente: tu iPhone usa este ordenador para convertir ────────── -->
  <section class="pref-card">
    <h2>phone bridge</h2>
    <div class="pref-body">
      <div class="pref-row">
        <div>
          <div class="pref-label">let your phone render here</div>
          <div class="pref-sub">
            queue a whole book from your iPhone and this computer synthesizes
            it overnight — the finished audiobook lands back on the phone.
            end-to-end encrypted (iroh); no accounts.
          </div>
        </div>
        <button class="btn-pink" onclick={emparejarMovil}>pair a phone</button>
      </div>
      {#if puenteQR}
        <div class="puente-qr">
          <!-- eslint-disable-next-line svelte/no-at-html-tags -->
          {@html puenteQR.qr_svg}
          <div class="puente-qr-texto">
            <p>scan with the iPhone <strong>Camera</strong> app and tap
            «Open in Yappy». Or copy the link and paste it in the phone's
            Settings → Your computer.</p>
            <button class="btn-outline" onclick={async () => { try { await navigator.clipboard.writeText(puenteQR!.enlace); bridgeToast("link copied"); } catch {} }}>copy pairing link</button>
          </div>
        </div>
      {/if}
      {#if puente && puente.tokens.length > 0}
        <div class="pref-row">
          <div>
            <div class="pref-label">paired phones</div>
            <div class="pref-sub">each pairing is a revocable token.</div>
          </div>
        </div>
        {#each puente.tokens as t (t)}
          <div class="pref-row puente-token">
            <code>{t}…</code>
            <button class="btn-outline danger" onclick={() => revocarPuente(t)}>revoke</button>
          </div>
        {/each}
      {/if}
    </div>
  </section>
{/if}

<CreditsModal open={creditsOpen} onClose={() => (creditsOpen = false)} />
