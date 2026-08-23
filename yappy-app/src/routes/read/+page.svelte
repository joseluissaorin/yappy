<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { goto } from "$app/navigation";
  import { reader } from "$lib/readerStore.svelte";
  import { guardarProgreso } from "$lib/progreso";
  import { haptic } from "$lib/haptic";
  import {
    type PlaybackSnapshot,
    type Voice,
    type Settings,
    type ParagraphSpec,
    readDocumentParagraphs,
    stopPlayback,
    togglePause,
    onPlaybackState,
    onPlaybackStarting,
    listVoices,
    getSettings,
    saveProject,
    loadProject,
    renderAudiobook,
    audiobookExportPath,
    shareFile,
    onAudiobookRenderProgress,
    onAudiobookRenderDone,
    puenteMovilEstado,
    puenteConvertir,
    onPuenteProgreso,
  } from "$lib/ipc";

  // Immersive single-column mobile reader WITH editor parity: per-document rhythm
  // + voice, and per-paragraph speed / pause / voice overrides — persisted to the
  // same project file the desktop editor uses, and used to render an .m4b
  // audiobook. Tap a paragraph to read from there; tap its ⋯ to adjust it.
  const doc = $derived(reader.doc);
  const paras = $derived(doc?.paragraphs ?? []);
  const kinds = $derived(doc?.paragraph_kinds ?? []);
  const pausesDefault = $derived(doc?.paragraph_pauses ?? []);
  function defaultPause(i: number): number {
    return pausesDefault[i] ?? 0;
  }
  const title = $derived((doc?.filename ?? "document").replace(/\.[^.]+$/, "").replace(/[-_]/g, " "));

  // Per-paragraph editable state (parity with the desktop editor's ParaState).
  // null override = inherit the document/global value.
  type ParaState = { voice: string | null; speed: number | null; pauseBefore: number | null; kind: string };
  let overrides = $state<ParaState[]>([]);
  // Document-level controls.
  let rhythmMult = $state(1.0);
  let docVoice = $state<string | null>(null); // null = global voice from settings

  let voices = $state<Voice[]>([]);
  let settings = $state<Settings | null>(null);

  // Chapters = heading paragraphs.
  const chapters = $derived(
    paras
      .map((text, index) => ({ text, index, level: Number((kinds[index] || "").replace("heading", "")) || 0 }))
      .filter((p) => (kinds[p.index] || "").startsWith("heading")),
  );

  let playback = $state<PlaybackSnapshot | null>(null);
  let baseIndex = $state(0);
  let chaptersOpen = $state(false);
  let settingsOpen = $state(false);
  let tweakIndex = $state(-1); // per-paragraph adjust sheet (-1 = closed)
  let voicePickerFor = $state<"doc" | "para" | null>(null);
  let paraEls: (HTMLElement | null)[] = $state([]);
  let cleanups: (() => void)[] = [];

  // Export state.
  let rendering = $state(false);
  let renderProgress = $state<{ index: number; total: number; stage: string } | null>(null);
  let exportDone = $state<{ path: string } | null>(null);
  let puenteVinculado = $state(false);
  let puenteOcupado = $state(false);
  let puenteEtapa = $state<string | null>(null);
  let toast = $state<string | null>(null);

  const isPlaying = $derived(!!playback?.playing && !playback?.paused);
  const isPaused = $derived(!!playback?.paused);
  const currentPara = $derived(
    (isPlaying || isPaused) && playback ? baseIndex + (playback.current_paragraph_index ?? 0) : -1,
  );
  const globalSpeed = $derived(settings?.speed ?? 1.05);

  // Progreso persistente: cada vez que avanza el párrafo que suena, se
  // apunta dónde vamos. «Sigue donde ibas» y la Biblioteca leen esto.
  $effect(() => {
    if (currentPara >= 0 && doc?.path && paras.length > 0) {
      guardarProgreso({
        ruta: doc.path,
        titulo: title,
        parrafo: currentPara,
        total_parrafos: paras.length,
        cuando_unix: Math.floor(Date.now() / 1000),
      });
    }
  });
  const docVoiceName = $derived(
    docVoice ? (voices.find((v) => v.id === docVoice || v.name === docVoice)?.name ?? docVoice) : "default voice",
  );
  // Has the reader been customised away from the plain defaults?
  const customised = $derived(
    rhythmMult !== 1.0 || docVoice !== null || overrides.some((o) => o.voice || o.speed != null || o.pauseBefore != null),
  );

  function clampSpeed(s: number) { return Math.max(0.3, Math.min(3.0, s)); }
  function effectiveSpeedForPlay() { return clampSpeed(globalSpeed * rhythmMult); }
  function flashToast(msg: string) { toast = msg; setTimeout(() => (toast = null), 2400); }

  onMount(async () => {
    if (!doc) { goto("/"); return; }
    voices = await listVoices().catch(() => []);
    settings = await getSettings().catch(() => null);

    // Seed per-paragraph state from the document's markdown rhythm hints, then
    // try to restore any saved project (parity with the desktop editor).
    seedOverrides();
    await tryRestoreProject();

    cleanups.push(await onPlaybackStarting((p: any) => {
      if (typeof p?.base_paragraph_index === "number") baseIndex = p.base_paragraph_index;
    }));
    cleanups.push(await onPlaybackState((s) => (playback = s)));
    cleanups.push(await onAudiobookRenderProgress((p) => (renderProgress = p)));
    puenteVinculado = !!(await puenteMovilEstado().catch(() => null))?.token;
    cleanups.push(await onPuenteProgreso((p) => {
      if (p.etapa === "sintetizando" && p.total) {
        puenteEtapa = `${p.hecho}/${p.total}`;
      } else if (p.etapa === "codificando") {
        puenteEtapa = "…";
      } else if (p.etapa === "hecho") {
        puenteEtapa = null;
        puenteOcupado = false;
        flashToast("audiobook back from your computer — in your Library");
      }
    }));
    cleanups.push(await onAudiobookRenderDone((p) => {
      rendering = false;
      renderProgress = null;
      exportDone = { path: p.path };
      flashToast("audiobook saved to your Library");
    }));
  });
  onDestroy(() => cleanups.forEach((c) => c()));

  function seedOverrides() {
    overrides = paras.map((_, i) => ({
      voice: null,
      speed: null,
      pauseBefore: null,
      kind: kinds[i] || "paragraph",
    }));
  }

  // ── Project persistence (same v2 schema as the desktop editor) ──────────────
  let saveTimer: ReturnType<typeof setTimeout> | undefined;
  function scheduleSave() {
    if (!doc?.path) return;
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(async () => {
      try {
        const snap = {
          version: 2,
          doc_path: doc!.path,
          paragraphs: paras.map((text, i) => ({
            text,
            voice: overrides[i]?.voice ?? null,
            speed: overrides[i]?.speed ?? null,
            pauseBefore: overrides[i]?.pauseBefore ?? null,
            kind: overrides[i]?.kind ?? "paragraph",
          })),
          rhythm_mult: rhythmMult,
          doc_voice: docVoice,
          saved_at: new Date().toISOString(),
        };
        await saveProject(doc!.path, JSON.stringify(snap));
      } catch { /* best-effort */ }
    }, 500);
  }
  async function tryRestoreProject() {
    if (!doc?.path) return;
    try {
      const json = await loadProject(doc.path);
      if (!json) return;
      const parsed = JSON.parse(json);
      if (Array.isArray(parsed?.paragraphs) && parsed.paragraphs.length === paras.length) {
        overrides = parsed.paragraphs.map((p: any, i: number) => ({
          voice: p.voice ?? null,
          speed: p.speed ?? null,
          pauseBefore: p.pauseBefore ?? null,
          kind: p.kind ?? kinds[i] ?? "paragraph",
        }));
      }
      if (typeof parsed.rhythm_mult === "number") rhythmMult = parsed.rhythm_mult;
      if (typeof parsed.doc_voice === "string" || parsed.doc_voice === null) docVoice = parsed.doc_voice ?? null;
      flashToast("restored your reading settings");
    } catch { /* ignore */ }
  }

  // ── Playback ────────────────────────────────────────────────────────────────
  async function readFrom(index: number) {
    haptic("light");
    baseIndex = index;
    await readDocumentParagraphs(paras, index, docVoice ?? undefined, effectiveSpeedForPlay(), {
      kinds: overrides.map((o) => o.kind ?? "paragraph"),
      pausas: overrides.map((o, i) => (o.pauseBefore ?? defaultPause(i)) * rhythmMult),
      velocidades: overrides.map((o) => {
        const base = settings?.speed ?? 1.05;
        return o.speed ? Math.max(0.25, Math.min(2.0, o.speed / base)) : 1.0;
      }),
      voces: overrides.map((o) => o.voice ?? null),
    });
  }
  async function toggle() { haptic("medium"); await togglePause(); }
  function back() { goto("/"); }
  function jumpToChapter(index: number) { chaptersOpen = false; readFrom(index); }

  // ── Per-paragraph adjusters ──────────────────────────────────────────────────
  function setParaSpeed(i: number, v: number | null) { overrides[i].speed = v; scheduleSave(); }
  function setParaPause(i: number, v: number | null) { overrides[i].pauseBefore = v; scheduleSave(); }
  function setParaVoice(i: number, v: string | null) { overrides[i].voice = v; voicePickerFor = null; scheduleSave(); }
  function setDocVoice(v: string | null) { docVoice = v; voicePickerFor = null; scheduleSave(); }
  function resetAll() {
    haptic("warning");
    seedOverrides();
    rhythmMult = 1.0;
    docVoice = null;
    scheduleSave();
    flashToast("reset to defaults");
  }

  function progressPct(): number {
    if (currentPara < 0 || paras.length === 0) return 0;
    return Math.min(100, ((currentPara + 1) / paras.length) * 100);
  }

  // ── Convertir en el ordenador (el puente) ───────────────────────────────────
  async function convertirEnOrdenador() {
    if (puenteOcupado || !doc) return;
    settingsOpen = false;
    haptic("medium");
    puenteOcupado = true;
    puenteEtapa = "0";
    flashToast("sent to your computer — it will come back on its own");
    try {
      await puenteConvertir(title, paras.join("\n\n"));
    } catch (e) {
      puenteOcupado = false;
      puenteEtapa = null;
      flashToast(String(e));
    }
  }

  // ── Export to .m4b ───────────────────────────────────────────────────────────
  async function exportAudiobook() {
    if (rendering || !doc) return;
    settingsOpen = false;
    haptic("medium");
    try {
      const base = title;
      let lastChapterAt = -1;
      const specs: ParagraphSpec[] = paras.map((text, i) => {
        const o = overrides[i] ?? { voice: null, speed: null, pauseBefore: null, kind: kinds[i] || "paragraph" };
        const baseSpeed = o.speed ?? globalSpeed;
        const hasPause = (o.pauseBefore ?? 0) > 0;
        let chapterTitle: string | null = null;
        if (/^heading[1-6]$/.test(o.kind)) {
          chapterTitle = text.trim() || `Chapter ${lastChapterAt + 2}`;
          lastChapterAt = i;
        } else if (i === 0 && lastChapterAt === -1) {
          chapterTitle = base;
          lastChapterAt = 0;
        }
        return {
          text,
          voice: o.voice,
          speed: clampSpeed(baseSpeed * rhythmMult),
          pause_before: hasPause ? (o.pauseBefore as number) / Math.max(0.1, rhythmMult) : null,
          chapter_title: chapterTitle,
        };
      });
      const path = await audiobookExportPath(base);
      rendering = true;
      exportDone = null;
      renderProgress = { index: 0, total: specs.length, stage: "synth" };
      await renderAudiobook(specs, path, { title: base, album: base });
    } catch (e) {
      rendering = false;
      renderProgress = null;
      flashToast("export failed: " + String(e));
    }
  }
  async function shareExport() {
    if (!exportDone) return;
    try { await shareFile(exportDone.path); } catch (e) { flashToast(String(e)); }
  }
</script>

<div class="reader" data-tauri-drag-region>
  <header class="r-head">
    <button class="r-icon" onclick={back} aria-label="back to Yappy">←</button>
    <div class="r-title">{title}</div>
    <button class="r-icon" class:dot={customised} onclick={() => (settingsOpen = true)} aria-label="reading settings">⚙</button>
    {#if chapters.length > 0}
      <button class="r-icon" onclick={() => (chaptersOpen = true)} aria-label="chapters">☰</button>
    {/if}
  </header>
  <div class="r-progress"><div class="r-progress-fill" style="width: {progressPct()}%"></div></div>

  <main class="r-body">
    {#each paras as text, i}
      {@const kind = kinds[i] || "paragraph"}
      {#if kind === "hr"}
        <hr class="r-hr" />
      {:else}
        {@const o = overrides[i]}
        {@const tweaked = !!o && (o.voice || o.speed != null || o.pauseBefore != null)}
        <div class="r-row" class:active={i === currentPara}>
          <button
            class="r-para k-{kind}"
            class:active={i === currentPara}
            bind:this={paraEls[i]}
            onclick={() => readFrom(i)}
          >
            {text}
          </button>
          <button class="r-tweak" class:on={tweaked} onclick={() => { tweakIndex = i; haptic("light"); }} aria-label="adjust this paragraph">⋯</button>
        </div>
      {/if}
    {/each}
    <div class="r-end">· · ·</div>
  </main>

  <!-- Floating reading control -->
  <div class="r-dock">
    {#if isPlaying || isPaused}
      <button class="r-play" onclick={toggle} aria-label={isPaused ? "resume" : "pause"}>
        {#if isPaused}
          <svg width="22" height="22" viewBox="0 0 14 14" fill="currentColor"><path d="M3 1.5C3 0.7 3.85 0.25 4.5 0.7L12.5 6.2c0.6 0.4 0.6 1.3 0 1.7l-8 5.5c-0.7 0.4-1.5 0-1.5-0.8V1.5Z"/></svg>
        {:else}
          <svg width="22" height="22" viewBox="0 0 14 14" fill="currentColor"><rect x="2.5" y="2" width="3.2" height="10" rx="1"/><rect x="8.3" y="2" width="3.2" height="10" rx="1"/></svg>
        {/if}
      </button>
      <div class="r-dock-meta">
        <div class="r-dock-title">{isPaused ? "paused" : "reading aloud"}</div>
        <div class="r-dock-sub">paragraph {currentPara + 1} of {paras.length} · {effectiveSpeedForPlay().toFixed(2)}×</div>
      </div>
      <button class="r-dock-stop" onclick={() => stopPlayback()} aria-label="stop">
        <svg width="14" height="14" viewBox="0 0 14 14" fill="currentColor"><rect x="2" y="2" width="10" height="10" rx="2"/></svg>
      </button>
    {:else}
      <button class="r-readall" onclick={() => readFrom(0)}>
        <svg width="16" height="16" viewBox="0 0 14 14" fill="currentColor"><path d="M3 1.5C3 0.7 3.85 0.25 4.5 0.7L12.5 6.2c0.6 0.4 0.6 1.3 0 1.7l-8 5.5c-0.7 0.4-1.5 0-1.5-0.8V1.5Z"/></svg>
        read aloud
      </button>
    {/if}
  </div>

  <!-- Render-progress strip -->
  {#if rendering && renderProgress}
    <div class="r-render">
      <div class="r-render-bar"><div style="width: {renderProgress.total ? (renderProgress.index / renderProgress.total) * 100 : 0}%"></div></div>
      <div class="r-render-txt">building audiobook · {renderProgress.stage} {renderProgress.index}/{renderProgress.total}</div>
    </div>
  {/if}

  {#if toast}<div class="r-toast">{toast}</div>{/if}

  <!-- ── Chapters bottom sheet ── -->
  {#if chaptersOpen}
    <div class="r-sheet-scrim" onclick={() => (chaptersOpen = false)} role="presentation"></div>
    <div class="r-sheet">
      <div class="r-sheet-grip"></div>
      <div class="r-sheet-head">chapters</div>
      <div class="r-sheet-list">
        {#each chapters as ch}
          <button class="r-chapter lvl-{ch.level}" class:active={ch.index === currentPara} onclick={() => jumpToChapter(ch.index)}>
            <span class="r-chapter-dot"></span>
            {ch.text}
          </button>
        {/each}
      </div>
    </div>
  {/if}

  <!-- ── Document reading-settings sheet ── -->
  {#if settingsOpen}
    <div class="r-sheet-scrim" onclick={() => { settingsOpen = false; voicePickerFor = null; }} role="presentation"></div>
    <div class="r-sheet">
      <div class="r-sheet-grip"></div>
      <div class="r-sheet-head">reading settings</div>

      <div class="ctl">
        <div class="ctl-row"><span class="ctl-label">pace</span><span class="ctl-val">{rhythmMult.toFixed(2)}×</span></div>
        <input class="slider" type="range" min="0.5" max="2.0" step="0.05" bind:value={rhythmMult} oninput={scheduleSave} aria-label="reading pace" />
        <div class="ctl-hint">scales every section's speed and pauses, like the desktop editor's rhythm dial.</div>
      </div>

      <button class="ctl-pick" onclick={() => (voicePickerFor = voicePickerFor === "doc" ? null : "doc")}>
        <span class="ctl-label">voice</span>
        <span class="ctl-pick-val">{docVoiceName} <span class="caret">▾</span></span>
      </button>
      {#if voicePickerFor === "doc"}
        <div class="voice-list">
          <button class="voice-opt" class:on={docVoice === null} onclick={() => setDocVoice(null)}>default voice</button>
          {#each voices as v}
            <button class="voice-opt" class:on={docVoice === v.id || docVoice === v.name} onclick={() => setDocVoice(v.id)}>
              {v.name}<span class="voice-tag">{v.tags?.[0] ?? v.gender}</span>
            </button>
          {/each}
        </div>
      {/if}

      <button class="btn-export" onclick={exportAudiobook} disabled={rendering}>
        {rendering ? "building…" : "💾 save as audiobook (.m4b)"}
      </button>
      {#if puenteVinculado}
        <button class="sheet-row primary" onclick={convertirEnOrdenador} disabled={puenteOcupado}>
          {puenteOcupado ? `converting on your computer… ${puenteEtapa ?? ""}` : "🖥 convert on your computer"}
        </button>
      {/if}
      {#if exportDone}
        <div class="export-done">
          <span>✓ saved to your Library</span>
          <div class="export-actions">
            <button onclick={() => { settingsOpen = false; goto("/library"); }}>open Library</button>
            <button onclick={shareExport}>share</button>
          </div>
        </div>
      {/if}

      {#if customised}
        <button class="btn-reset" onclick={resetAll}>reset all to defaults</button>
      {/if}
    </div>
  {/if}

  <!-- ── Per-paragraph adjust sheet ── -->
  {#if tweakIndex >= 0}
    {@const o = overrides[tweakIndex]}
    <div class="r-sheet-scrim" onclick={() => { tweakIndex = -1; voicePickerFor = null; }} role="presentation"></div>
    <div class="r-sheet">
      <div class="r-sheet-grip"></div>
      <div class="r-sheet-head">adjust paragraph {tweakIndex + 1}</div>
      <div class="tweak-preview">{paras[tweakIndex]}</div>

      <div class="ctl">
        <div class="ctl-row">
          <span class="ctl-label">speed</span>
          <span class="ctl-val">{(o.speed ?? globalSpeed).toFixed(2)}× {#if o.speed == null}<em>(inherit)</em>{/if}</span>
        </div>
        <input class="slider" type="range" min="0.5" max="2.0" step="0.05" value={o.speed ?? globalSpeed}
          oninput={(e) => setParaSpeed(tweakIndex, parseFloat((e.target as HTMLInputElement).value))} aria-label="paragraph speed" />
        {#if o.speed != null}<button class="link-reset" onclick={() => setParaSpeed(tweakIndex, null)}>reset to inherit</button>{/if}
      </div>

      <div class="ctl">
        <div class="ctl-row">
          <span class="ctl-label">pause before</span>
          <span class="ctl-val">{(o.pauseBefore ?? 0).toFixed(1)}s</span>
        </div>
        <input class="slider" type="range" min="0" max="5" step="0.1" value={o.pauseBefore ?? 0}
          disabled={tweakIndex === 0}
          oninput={(e) => setParaPause(tweakIndex, parseFloat((e.target as HTMLInputElement).value) || null)} aria-label="pause before" />
        {#if tweakIndex === 0}<div class="ctl-hint">the first paragraph can't have a leading pause.</div>{/if}
      </div>

      <button class="ctl-pick" onclick={() => (voicePickerFor = voicePickerFor === "para" ? null : "para")}>
        <span class="ctl-label">voice</span>
        <span class="ctl-pick-val">
          {o.voice ? (voices.find((v) => v.id === o.voice || v.name === o.voice)?.name ?? o.voice) : "inherit"} <span class="caret">▾</span>
        </span>
      </button>
      {#if voicePickerFor === "para"}
        <div class="voice-list">
          <button class="voice-opt" class:on={o.voice === null} onclick={() => setParaVoice(tweakIndex, null)}>inherit (document voice)</button>
          {#each voices as v}
            <button class="voice-opt" class:on={o.voice === v.id || o.voice === v.name} onclick={() => setParaVoice(tweakIndex, v.id)}>
              {v.name}<span class="voice-tag">{v.tags?.[0] ?? v.gender}</span>
            </button>
          {/each}
        </div>
      {/if}

      <div class="tweak-foot">
        <button class="btn-play-para" onclick={() => { tweakIndex = -1; readFrom(o ? overrides.indexOf(o) : 0); }}>▶ read from here</button>
        <button class="btn-done" onclick={() => { tweakIndex = -1; voicePickerFor = null; }}>done</button>
      </div>
    </div>
  {/if}
</div>

<style>
  .reader {
    min-height: 100vh;
    background: var(--bg, #fff8d7);
    color: var(--ink-900);
    display: flex;
    flex-direction: column;
  }
  /* Header */
  .r-head {
    position: sticky; top: 0; z-index: 5;
    display: flex; align-items: center; gap: 10px;
    padding: calc(10px + env(safe-area-inset-top, 0px)) 14px 10px;
    background: color-mix(in srgb, var(--bg, #fff8d7) 88%, transparent);
    backdrop-filter: blur(8px); -webkit-backdrop-filter: blur(8px);
  }
  .r-icon {
    flex: 0 0 auto; width: 38px; height: 38px; border-radius: 12px;
    display: flex; align-items: center; justify-content: center;
    font-size: 18px; font-weight: 700; color: var(--ink-900);
    background: var(--bg-2, #fffdf2); border: 2px solid var(--ink-900);
    box-shadow: 2px 2px 0 var(--ink-900); position: relative;
  }
  .r-icon.dot::after {
    content: ""; position: absolute; top: -3px; right: -3px;
    width: 10px; height: 10px; border-radius: 50%;
    background: var(--pink-500); border: 2px solid var(--bg-2, #fffdf2);
  }
  .r-icon:active { transform: translate(1px, 1px); box-shadow: 1px 1px 0 var(--ink-900); }
  .r-title {
    flex: 1; min-width: 0; text-align: center;
    font-family: var(--font-display); font-size: 19px; color: var(--pink-600);
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
    text-transform: capitalize;
  }
  .r-progress { height: 3px; background: var(--ink-100); }
  .r-progress-fill { height: 100%; background: var(--pink-500); transition: width 0.4s ease; }

  /* Body — comfortable reading column */
  .r-body {
    flex: 1;
    padding: 18px 22px calc(120px + env(safe-area-inset-bottom, 0px));
    max-width: 70ch; margin: 0 auto; width: 100%; box-sizing: border-box;
  }
  .r-row { display: flex; align-items: flex-start; gap: 4px; margin: 0 -12px 4px; }
  .r-row.active { background: var(--pink-300); border-radius: 12px; }
  .r-para {
    display: block; flex: 1; min-width: 0; text-align: left;
    font-family: var(--font-sans); font-size: 18px; line-height: 1.72;
    color: var(--ink-900); font-weight: 500;
    padding: 6px 12px; border-radius: 12px;
    border-left: 3px solid transparent; background: transparent;
    transition: background 0.2s ease, border-color 0.2s ease;
    cursor: pointer; -webkit-tap-highlight-color: transparent;
  }
  .r-para:active { background: var(--cream-200); }
  .r-para.active { border-left-color: var(--pink-600); }
  .r-tweak {
    flex: 0 0 auto; width: 30px; height: 30px; margin-top: 6px; border-radius: 9px;
    font-size: 16px; font-weight: 800; color: var(--ink-500);
    background: transparent; border: none; opacity: 0.5;
    -webkit-tap-highlight-color: transparent;
  }
  .r-tweak.on { opacity: 1; color: var(--pink-600); background: var(--pink-100, #ffe6f1); }
  .r-tweak:active { transform: scale(0.92); }
  .k-heading1 { font-family: var(--font-display); font-size: 30px; line-height: 1.2; color: var(--pink-600); margin-top: 22px; font-weight: 400; }
  .k-heading2 { font-family: var(--font-display); font-size: 24px; line-height: 1.25; color: var(--ink-900); margin-top: 18px; font-weight: 400; }
  .k-heading3 { font-weight: 800; font-size: 19px; margin-top: 12px; }
  .k-quote { font-style: italic; color: var(--ink-700); border-left: 3px solid var(--ink-300); }
  .k-code { font-family: var(--font-mono); font-size: 15px; background: var(--ink-100); }
  .r-hr { border: none; border-top: 2px dashed var(--ink-300); margin: 22px 12px; }
  .r-end { text-align: center; color: var(--ink-300); letter-spacing: 6px; padding: 20px 0; }

  /* Floating dock */
  .r-dock {
    position: fixed; left: 14px; right: 14px;
    bottom: calc(16px + env(safe-area-inset-bottom, 0px));
    z-index: 10; display: flex; align-items: center; gap: 12px;
  }
  .r-readall {
    margin: 0 auto; display: flex; align-items: center; gap: 8px;
    padding: 14px 26px; border-radius: 999px;
    background: var(--pink-500); color: var(--ink-900);
    border: 2.5px solid var(--ink-900); box-shadow: 4px 4px 0 var(--ink-900);
    font-weight: 800; font-size: 16px;
  }
  .r-readall:active { transform: translate(2px, 2px); box-shadow: 1px 1px 0 var(--ink-900); }
  .r-play, .r-dock-stop {
    flex: 0 0 auto; display: flex; align-items: center; justify-content: center;
    border: 2.5px solid var(--ink-900);
  }
  .r-play { width: 52px; height: 52px; border-radius: 16px; background: var(--pink-500); color: var(--ink-900); box-shadow: 4px 4px 0 var(--ink-900); }
  .r-dock-meta {
    flex: 1; min-width: 0; background: var(--ink-900); color: var(--cream-100);
    border-radius: 14px; padding: 8px 14px; box-shadow: 0 6px 20px rgba(0,0,0,0.25);
  }
  .r-dock-title { font-weight: 800; font-size: 14px; }
  .r-dock-sub { font-size: 11px; opacity: 0.75; font-variant-numeric: tabular-nums; }
  .r-dock-stop { width: 46px; height: 46px; border-radius: 14px; background: var(--bg-2, #fffdf2); color: var(--ink-900); box-shadow: 3px 3px 0 var(--ink-900); }

  /* Render progress + toast */
  .r-render {
    position: fixed; left: 14px; right: 14px; bottom: calc(82px + env(safe-area-inset-bottom, 0px));
    z-index: 11; background: var(--ink-900); color: var(--cream-100);
    border-radius: 14px; padding: 10px 14px; box-shadow: 0 6px 20px rgba(0,0,0,0.3);
  }
  .r-render-bar { height: 5px; border-radius: 3px; background: rgba(255,255,255,0.2); overflow: hidden; margin-bottom: 6px; }
  .r-render-bar > div { height: 100%; background: var(--pink-400, #ff8fc0); transition: width 0.3s ease; }
  .r-render-txt { font-size: 12px; font-weight: 600; font-variant-numeric: tabular-nums; }
  .r-toast {
    position: fixed; left: 50%; transform: translateX(-50%);
    bottom: calc(150px + env(safe-area-inset-bottom, 0px)); z-index: 30;
    background: var(--ink-900); color: var(--cream-100); font-weight: 700; font-size: 13px;
    padding: 10px 18px; border-radius: 999px; box-shadow: 0 6px 20px rgba(0,0,0,0.3);
    animation: toast-in 0.2s ease;
  }
  @keyframes toast-in { from { opacity: 0; transform: translate(-50%, 8px); } to { opacity: 1; transform: translate(-50%, 0); } }

  /* Bottom sheets (shared) */
  .r-sheet-scrim { position: fixed; inset: 0; background: rgba(0,0,0,0.28); z-index: 20; }
  .r-sheet {
    position: fixed; left: 0; right: 0; bottom: 0; z-index: 21;
    background: var(--bg-2, #fffdf2); border-top: 3px solid var(--ink-900);
    border-radius: 24px 24px 0 0; padding: 10px 18px calc(24px + env(safe-area-inset-bottom, 0px));
    max-height: 80vh; overflow-y: auto; box-shadow: 0 -10px 30px rgba(0,0,0,0.2);
    animation: sheet-up 0.22s ease;
  }
  @keyframes sheet-up { from { transform: translateY(100%); } to { transform: translateY(0); } }
  .r-sheet-grip { width: 44px; height: 5px; border-radius: 3px; background: var(--ink-300); margin: 4px auto 12px; }
  .r-sheet-head { font-family: var(--font-display); font-size: 22px; color: var(--pink-600); margin-bottom: 8px; }
  .r-chapter {
    display: flex; align-items: center; gap: 10px; width: 100%; text-align: left;
    padding: 12px 8px; border-bottom: 2px dashed var(--ink-300);
    font-size: 16px; font-weight: 700; color: var(--ink-900);
  }
  .r-chapter.lvl-2 { padding-left: 22px; font-weight: 600; font-size: 15px; color: var(--ink-700); }
  .r-chapter.lvl-3 { padding-left: 38px; font-weight: 500; font-size: 14px; color: var(--ink-700); }
  .r-chapter-dot { flex: 0 0 auto; width: 8px; height: 8px; border-radius: 50%; background: var(--pink-500); }
  .r-chapter.active { color: var(--pink-600); }
  .r-chapter:active { background: var(--cream-200); }

  /* Controls inside sheets */
  .ctl { margin: 14px 0; }
  .ctl-row { display: flex; justify-content: space-between; align-items: baseline; margin-bottom: 8px; }
  .ctl-label { font-weight: 800; font-size: 15px; color: var(--ink-900); }
  .ctl-val { font-size: 14px; font-weight: 700; color: var(--pink-600); font-variant-numeric: tabular-nums; }
  .ctl-val em { color: var(--ink-500); font-style: normal; font-weight: 600; font-size: 12px; }
  .ctl-hint { font-size: 12px; color: var(--ink-500); margin-top: 6px; line-height: 1.4; }
  .slider { width: 100%; accent-color: var(--pink-500); height: 28px; }
  .slider:disabled { opacity: 0.4; }
  .link-reset { background: none; border: none; color: var(--pink-600); font-weight: 700; font-size: 13px; padding: 6px 0 0; }
  .ctl-pick {
    display: flex; justify-content: space-between; align-items: center; width: 100%;
    padding: 14px 14px; margin: 8px 0; border-radius: 14px;
    background: var(--cream-100, #fffdf2); border: 2px solid var(--ink-300);
    font-size: 15px; color: var(--ink-900);
  }
  .ctl-pick-val { font-weight: 700; color: var(--ink-700); }
  .caret { color: var(--ink-500); }
  .voice-list { max-height: 240px; overflow-y: auto; border: 2px solid var(--ink-300); border-radius: 14px; margin: 0 0 8px; }
  .voice-opt {
    display: flex; justify-content: space-between; align-items: center; width: 100%; text-align: left;
    padding: 12px 14px; border-bottom: 1px solid var(--ink-200, #eee);
    font-size: 15px; font-weight: 600; color: var(--ink-900); background: transparent;
  }
  .voice-opt:last-child { border-bottom: none; }
  .voice-opt.on { background: var(--pink-300); color: var(--ink-900); }
  .voice-tag { font-size: 11px; font-weight: 700; color: var(--ink-500); text-transform: lowercase; }
  .btn-export {
    width: 100%; margin-top: 14px; padding: 15px; border-radius: 16px;
    background: var(--pink-500); color: var(--ink-900);
    border: 2.5px solid var(--ink-900); box-shadow: 3px 3px 0 var(--ink-900);
    font-weight: 800; font-size: 16px;
  }
  .btn-export:disabled { opacity: 0.6; }
  .btn-export:active { transform: translate(2px, 2px); box-shadow: 1px 1px 0 var(--ink-900); }
  .export-done { margin-top: 12px; padding: 12px 14px; border-radius: 14px; background: var(--pink-300); border: 2px solid var(--ink-900); }
  .export-done > span { font-weight: 800; }
  .export-actions { display: flex; gap: 10px; margin-top: 10px; }
  .export-actions button {
    flex: 1; padding: 10px; border-radius: 12px; font-weight: 700; font-size: 14px;
    background: var(--bg-2, #fffdf2); border: 2px solid var(--ink-900); color: var(--ink-900);
  }
  .btn-reset { width: 100%; margin-top: 12px; padding: 12px; border-radius: 14px; background: transparent; border: 2px dashed var(--ink-300); color: var(--ink-500); font-weight: 700; }
  .tweak-preview {
    font-size: 14px; color: var(--ink-700); line-height: 1.5; max-height: 84px; overflow: hidden;
    padding: 10px 12px; background: var(--cream-100, #fffdf2); border-radius: 12px; border: 2px solid var(--ink-200, #eee);
    margin-bottom: 6px;
  }
  .tweak-foot { display: flex; gap: 10px; margin-top: 14px; }
  .btn-play-para { flex: 1; padding: 13px; border-radius: 14px; background: var(--ink-900); color: var(--cream-100); border: none; font-weight: 800; font-size: 15px; }
  .btn-done { flex: 0 0 auto; padding: 13px 22px; border-radius: 14px; background: var(--pink-500); color: var(--ink-900); border: 2.5px solid var(--ink-900); font-weight: 800; }
</style>
