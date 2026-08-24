<script lang="ts">
  // LA PALABRA GIGANTE. La pantalla es el botón: mientras Yappy lee, la
  // frase que suena ocupa la pantalla entera en Literata de cartel, con el
  // subrayador dorado barriéndola en tiempo real. Tocar en cualquier sitio
  // pausa y reanuda; deslizar a los lados salta de párrafo; arrastrar por
  // el borde derecho cambia la velocidad con el número estampado gigante.
  // La interfaz solo existe cuando la voz calla: en pausa suben las tres
  // teclas de piano del mando, y desde ahí se llega al guion (el texto
  // completo) y al taller (voz, ritmo, exportar, el puente).
  import { onMount, onDestroy } from "svelte";
  import { goto } from "$app/navigation";
  import { reader } from "$lib/readerStore.svelte";
  import { guardarProgreso } from "$lib/progreso";
  import { t } from "$lib/i18n";
  import { get } from "svelte/store";
  import { isMobile } from "$lib/platform";
  import { haptic } from "$lib/haptic";
  import {
    type PlaybackSnapshot,
    type Voice,
    type Settings,
    readDocumentParagraphs,
    stopPlayback,
    togglePause,
    onPlaybackState,
    playbackSnapshot,
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

  const doc = $derived(reader.doc);
  const paras = $derived(doc?.paragraphs ?? []);
  const kinds = $derived(doc?.paragraph_kinds ?? []);
  const pausesDefault = $derived(doc?.paragraph_pauses ?? []);
  function defaultPause(i: number): number {
    return pausesDefault[i] ?? 0;
  }
  const title = $derived((doc?.filename ?? "").replace(/\.[^.]+$/, "").replace(/[-_]/g, " "));

  // Ajustes por párrafo (paridad con el editor de escritorio): se conservan
  // en el mismo fichero de proyecto, y el taller los expone.
  type ParaState = { voice: string | null; speed: number | null; pauseBefore: number | null; kind: string };
  let overrides = $state<ParaState[]>([]);
  let rhythmMult = $state(1.0);
  let docVoice = $state<string | null>(null);
  let voices = $state<Voice[]>([]);
  let settings = $state<Settings | null>(null);

  let playback = $state<PlaybackSnapshot | null>(null);
  let baseIndex = $state(0);
  let guionAbierto = $state(false);
  let tallerAbierto = $state(false);
  let tweakIndex = $state(-1);
  let voicePickerFor = $state<"doc" | "para" | null>(null);
  let cleanups: (() => void)[] = [];

  // Exportación y puente.
  let rendering = $state(false);
  let renderProgress = $state<{ index: number; total: number; stage: string } | null>(null);
  let exportDone = $state<{ path: string } | null>(null);
  let puenteVinculado = $state(false);
  let puenteOcupado = $state(false);
  let puenteEtapa = $state<string | null>(null);
  let toast = $state<string | null>(null);

  const isPlaying = $derived(!!playback?.playing && !playback?.paused);
  const isPaused = $derived(!!playback?.paused);
  const activo = $derived(isPlaying || isPaused);
  const currentPara = $derived(
    activo && playback ? baseIndex + (playback.current_paragraph_index ?? 0) : -1,
  );
  const globalSpeed = $derived(settings?.speed ?? 1.05);

  // ── La frase que suena, por code points (los índices llegan en chars) ──
  const letrasParrafo = $derived(currentPara >= 0 ? Array.from(paras[currentPara] ?? "") : []);
  const oIni = $derived(Math.min(playback?.current_origen_ini ?? 0, letrasParrafo.length));
  const oFin = $derived(Math.min(Math.max(playback?.current_origen_fin ?? 0, oIni), letrasParrafo.length));
  const hayOrigen = $derived(oFin > oIni);
  const dichas = $derived(hayOrigen ? letrasParrafo.slice(0, oIni).join("").trim() : "");
  const frase = $derived(
    (oFin > oIni ? letrasParrafo.slice(oIni, oFin).join("") : letrasParrafo.join("")).trim(),
  );
  const porVenir = $derived(hayOrigen ? letrasParrafo.slice(oFin).join("").trim() : "");
  const esTitulo = $derived((kinds[currentPara] ?? "").startsWith("heading"));

  // Tamaño de cartel: cuanto más corta la frase, más grita.
  const cuerpoFrase = $derived.by(() => {
    const c = Math.max(16, frase.length);
    return Math.round(Math.max(30, Math.min(80, 46 * Math.sqrt(150 / c))));
  });

  // ── El barrido dorado: avanza al ritmo estimado de la voz ─────────────
  let barrido = $state(0);
  let rafId = 0;
  let claveFrase = $state("");
  $effect(() => {
    const clave = `${currentPara}·${oIni}·${oFin}`;
    if (clave !== claveFrase) {
      claveFrase = clave;
      barrido = 0;
      cancelAnimationFrame(rafId);
      if (isPlaying && frase) {
        const dur = Math.max(0.9, (frase.length * 0.062) / effectiveSpeedForPlay());
        const t0 = performance.now();
        const paso = (ahora: number) => {
          barrido = Math.min(1, (ahora - t0) / (dur * 1000));
          if (barrido < 1 && isPlaying) rafId = requestAnimationFrame(paso);
        };
        rafId = requestAnimationFrame(paso);
      }
    } else if (!isPlaying) {
      cancelAnimationFrame(rafId);
    } else if (isPlaying && barrido === 0 && frase) {
      const dur = Math.max(0.9, (frase.length * 0.062) / effectiveSpeedForPlay());
      const t0 = performance.now();
      const paso = (ahora: number) => {
        barrido = Math.min(1, (ahora - t0) / (dur * 1000));
        if (barrido < 1 && isPlaying) rafId = requestAnimationFrame(paso);
      };
      rafId = requestAnimationFrame(paso);
    }
  });

  // Progreso persistente por párrafo.
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

  const customised = $derived(
    rhythmMult !== 1.0 || docVoice !== null || overrides.some((o) => o.voice || o.speed != null || o.pauseBefore != null),
  );
  const docVoiceName = $derived(
    docVoice ? (voices.find((v) => v.id === docVoice || v.name === docVoice)?.name ?? docVoice) : null,
  );

  function clampSpeed(s: number) { return Math.max(0.3, Math.min(3.0, s)); }
  function effectiveSpeedForPlay() { return clampSpeed(globalSpeed * rhythmMult); }
  function flashToast(msg: string) { toast = msg; setTimeout(() => (toast = null), 2400); }

  onMount(async () => {
    if (!doc) { goto(get(isMobile) ? "/escuchar" : "/"); return; }
    voices = await listVoices().catch(() => []);
    settings = await getSettings().catch(() => null);
    seedOverrides();
    await tryRestoreProject();
    cleanups.push(await onPlaybackStarting((p: any) => {
      if (typeof p?.base_paragraph_index === "number") baseIndex = p.base_paragraph_index;
    }));
    playback = await playbackSnapshot().catch(() => null);
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
        flashToast(get(t)("lector.t_vuelto"));
      }
    }));
    cleanups.push(await onAudiobookRenderDone((p) => {
      rendering = false;
      renderProgress = null;
      exportDone = { path: p.path };
      flashToast(get(t)("lector.t_guardado"));
    }));
  });
  onDestroy(() => {
    cancelAnimationFrame(rafId);
    cleanups.forEach((c) => c());
  });

  function seedOverrides() {
    overrides = paras.map((_, i) => ({
      voice: null,
      speed: null,
      pauseBefore: null,
      kind: kinds[i] || "paragraph",
    }));
  }

  // ── Proyecto (mismo esquema v2 que el editor de escritorio) ───────────
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
      } catch { /* mejor esfuerzo */ }
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
      if (typeof parsed?.rhythm_mult === "number") rhythmMult = parsed.rhythm_mult;
      if (typeof parsed?.doc_voice === "string" || parsed?.doc_voice === null) docVoice = parsed.doc_voice;
    } catch { /* proyecto viejo o ilegible: se ignora */ }
  }

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
  function back() { goto(get(isMobile) ? "/escuchar" : "/"); }

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
    flashToast(get(t)("lector.t_reiniciado"));
  }

  // ── El puente y la exportación ────────────────────────────────────────
  async function convertirEnOrdenador() {
    if (puenteOcupado || !doc) return;
    tallerAbierto = false;
    haptic("medium");
    puenteOcupado = true;
    puenteEtapa = "0";
    flashToast(get(t)("lector.t_enviado"));
    try {
      await puenteConvertir(title, paras.join("\n\n"));
    } catch (e) {
      puenteOcupado = false;
      puenteEtapa = null;
      flashToast(String(e));
    }
  }
  async function exportAudiobook() {
    if (rendering || !doc) return;
    haptic("medium");
    rendering = true;
    exportDone = null;
    try {
      const out = await audiobookExportPath(title);
      await renderAudiobook(
        title,
        paras.map((text, i) => ({
          text,
          voice: overrides[i]?.voice ?? docVoice ?? null,
          speed: overrides[i]?.speed ?? null,
          pauseBefore: (overrides[i]?.pauseBefore ?? defaultPause(i)) * rhythmMult,
          kind: overrides[i]?.kind ?? "paragraph",
        })),
        out,
      );
    } catch (e) {
      rendering = false;
      flashToast(get(t)("lector.t_fallo_export") + String(e));
    }
  }
  async function shareExport() {
    if (!exportDone) return;
    try { await shareFile(exportDone.path); } catch (e) { flashToast(String(e)); }
  }

  // ── Gestos del escenario: tocar, deslizar, y el dial del borde ────────
  let gesto: { x: number; y: number; t: number; borde: boolean; velocidadInicial: number } | null = null;
  let dialVisible = $state(false);
  let dialValor = $state(1.0);

  function escenarioDown(e: PointerEvent) {
    if (guionAbierto || tallerAbierto || tweakIndex >= 0) return;
    const ancho = (e.currentTarget as HTMLElement).clientWidth;
    gesto = {
      x: e.clientX,
      y: e.clientY,
      t: performance.now(),
      borde: e.clientX > ancho - 60 && activo,
      velocidadInicial: effectiveSpeedForPlay(),
    };
    if (gesto.borde) {
      dialValor = gesto.velocidadInicial;
      dialVisible = true;
    }
  }
  function escenarioMove(e: PointerEvent) {
    if (!gesto) return;
    if (gesto.borde) {
      const dv = (gesto.y - e.clientY) / 220;
      dialValor = clampSpeed(Math.round((gesto.velocidadInicial + dv) * 20) / 20);
    }
  }
  async function escenarioUp(e: PointerEvent) {
    const g = gesto;
    gesto = null;
    if (!g) return;
    const dx = e.clientX - g.x;
    const dy = e.clientY - g.y;
    const dt = performance.now() - g.t;
    if (g.borde) {
      dialVisible = false;
      if (Math.abs(dialValor - g.velocidadInicial) >= 0.05) {
        rhythmMult = clampSpeed(dialValor / globalSpeed);
        scheduleSave();
        if (activo && currentPara >= 0) await readFrom(currentPara);
      }
      return;
    }
    if (Math.abs(dx) > 72 && Math.abs(dx) > Math.abs(dy) * 1.4 && activo) {
      const destino = dx < 0 ? currentPara + 1 : currentPara - 1;
      if (destino >= 0 && destino < paras.length) await readFrom(destino);
      return;
    }
    if (Math.abs(dx) < 12 && Math.abs(dy) < 12 && dt < 450) {
      if (activo) await toggle();
    }
  }
</script>

<div
  class="escenario"
  class:leyendo={isPlaying}
  onpointerdown={escenarioDown}
  onpointermove={escenarioMove}
  onpointerup={escenarioUp}
>
  <!-- El hilo de progreso: lo único que existe mientras la voz habla. -->
  <div class="hilo"><div class="hilo-lleno" style="width: {paras.length ? Math.min(100, ((Math.max(currentPara, 0) + 1) / paras.length) * 100) : 0}%"></div></div>

  {#if activo}
    <!-- EL CARTEL -->
    <section class="cartel" class:atenuado={isPaused}>
      {#if dichas}
        <p class="dichas">{dichas}</p>
      {/if}
      <p
        class="frase"
        class:titulo={esTitulo}
        style="font-size: {cuerpoFrase}px; background-size: {Math.round(barrido * 100)}% 0.34em;"
      >
        {frase}
      </p>
      {#if porVenir}
        <p class="porvenir">{porVenir}</p>
      {/if}
    </section>

    {#if isPaused}
      <!-- Cromo mínimo, solo en pausa. -->
      <header class="pausa-arriba">
        <button class="p-icono" onclick={back} aria-label={$t("lector.volver")}>
          <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><path d="M19 12H5M11 18l-6-6 6-6"/></svg>
        </button>
        <span class="p-titulo">{title}</span>
        <button class="p-icono" onclick={() => (guionAbierto = true)} aria-label={$t("cartel.guion")}>
          <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"><path d="M4 7h16M4 12h16M4 17h10"/></svg>
        </button>
        <button class="p-icono" class:pendiente={customised} onclick={() => (tallerAbierto = true)} aria-label={$t("cartel.taller")}>
          <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14.7 6.3a4 4 0 0 0-5.2 5.2L4 17v3h3l5.5-5.5a4 4 0 0 0 5.2-5.2l-2.6 2.6-2.1-2.1z"/></svg>
        </button>
      </header>

      <!-- EL MANDO: tres teclas de piano. -->
      <nav class="mando">
        <button class="tecla-piano" onclick={() => currentPara > 0 && readFrom(currentPara - 1)} aria-label={$t("mando.atras")}>
          <svg viewBox="0 0 24 24" width="26" height="26" fill="currentColor"><path d="M18 6v12L9.5 12zM8 6H5.6v12H8z"/></svg>
        </button>
        <button class="tecla-piano seguir" onclick={toggle}>
          <svg viewBox="0 0 24 24" width="30" height="30" fill="currentColor"><path d="M7 4.8c0-1.1 1.2-1.8 2.2-1.2l11.5 7.2c0.9 0.6 0.9 1.9 0 2.4L9.2 20.4C8.2 21 7 20.3 7 19.2z"/></svg>
          <span>{$t("mando.seguir")}</span>
        </button>
        <button class="tecla-piano" onclick={() => currentPara < paras.length - 1 && readFrom(currentPara + 1)} aria-label={$t("mando.adelante")}>
          <svg viewBox="0 0 24 24" width="26" height="26" fill="currentColor"><path d="M6 6v12l8.5-6zM16 6h2.4v12H16z"/></svg>
        </button>
      </nav>
    {/if}
  {:else}
    <!-- LA PORTADA: aún no suena (o terminó). -->
    <section class="portada">
      <header class="pausa-arriba portada-arriba">
        <button class="p-icono" onclick={back} aria-label={$t("lector.volver")}>
          <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><path d="M19 12H5M11 18l-6-6 6-6"/></svg>
        </button>
        <span class="p-titulo"></span>
        <button class="p-icono" onclick={() => (guionAbierto = true)} aria-label={$t("cartel.guion")}>
          <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"><path d="M4 7h16M4 12h16M4 17h10"/></svg>
        </button>
        <button class="p-icono" onclick={() => (tallerAbierto = true)} aria-label={$t("cartel.taller")}>
          <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14.7 6.3a4 4 0 0 0-5.2 5.2L4 17v3h3l5.5-5.5a4 4 0 0 0 5.2-5.2l-2.6 2.6-2.1-2.1z"/></svg>
        </button>
      </header>
      <h1 class="portada-titulo">{title}</h1>
      <p class="portada-meta">{paras.length} ¶</p>
      <button class="leer-gigante" onclick={() => readFrom(0)}>
        <svg viewBox="0 0 24 24" width="30" height="30" fill="currentColor"><path d="M7 4.8c0-1.1 1.2-1.8 2.2-1.2l11.5 7.2c0.9 0.6 0.9 1.9 0 2.4L9.2 20.4C8.2 21 7 20.3 7 19.2z"/></svg>
        {$t("cartel.leer")}
      </button>
    </section>
  {/if}

  {#if dialVisible}
    <div class="dial">
      <strong>{dialValor.toFixed(2).replace(".", ",")}×</strong>
      <span>{$t("cartel.velocidad")}</span>
    </div>
  {/if}

  {#if rendering && renderProgress}
    <div class="tira-render">
      <div class="tira-barra"><div style="width: {renderProgress.total ? (renderProgress.index / renderProgress.total) * 100 : 0}%"></div></div>
      <span>{$t("lector.creando")} · {renderProgress.index}/{renderProgress.total}</span>
    </div>
  {/if}
  {#if toast}<div class="brindis">{toast}</div>{/if}

  <!-- ── EL GUION: el texto completo, para saltar y ajustar ── -->
  {#if guionAbierto}
    <div class="velo" role="presentation" onclick={() => (guionAbierto = false)}></div>
    <div class="hoja">
      <div class="hoja-asa"></div>
      <div class="hoja-cabeza">{$t("cartel.guion")}</div>
      <div class="guion-lista">
        {#each paras as p, i}
          <div class="guion-fila" class:actual={i === currentPara} class:es-titulo={(kinds[i] ?? "").startsWith("heading")}>
            <button class="guion-parrafo" onclick={() => { guionAbierto = false; readFrom(i); }}>{p}</button>
            <button
              class="guion-ajustar"
              class:tocado={!!(overrides[i]?.voice || overrides[i]?.speed != null || overrides[i]?.pauseBefore != null)}
              onclick={() => { tweakIndex = i; haptic("light"); }}
              aria-label={$t("lector.ajustar_parrafo")}
            >⋯</button>
          </div>
        {/each}
      </div>
    </div>
  {/if}

  <!-- ── EL TALLER: voz, ritmo, exportar, el puente ── -->
  {#if tallerAbierto}
    <div class="velo" role="presentation" onclick={() => { tallerAbierto = false; voicePickerFor = null; }}></div>
    <div class="hoja">
      <div class="hoja-asa"></div>
      <div class="hoja-cabeza">{$t("cartel.taller")}</div>

      <div class="ctl">
        <div class="ctl-fila"><span class="ctl-rotulo">{$t("lector.ritmo")}</span><span class="ctl-valor">{rhythmMult.toFixed(2)}×</span></div>
        <input class="deslizador" type="range" min="0.5" max="2.0" step="0.05" bind:value={rhythmMult} oninput={scheduleSave} aria-label={$t("lector.ritmo")} />
        <div class="ctl-pista">{$t("lector.ritmo_pista")}</div>
      </div>

      <button class="ctl-selector" onclick={() => (voicePickerFor = voicePickerFor === "doc" ? null : "doc")}>
        <span class="ctl-rotulo">{$t("lector.voz")}</span>
        <span class="ctl-selector-valor">{docVoiceName ?? $t("lector.voz_defecto")} <span class="caret">▾</span></span>
      </button>
      {#if voicePickerFor === "doc"}
        <div class="lista-voces">
          <button class="voz-opcion" class:activa={docVoice === null} onclick={() => setDocVoice(null)}>{$t("lector.voz_defecto")}</button>
          {#each voices as v}
            <button class="voz-opcion" class:activa={docVoice === v.id || docVoice === v.name} onclick={() => setDocVoice(v.id)}>
              {v.name}<span class="voz-etiqueta">{v.tags?.[0] ?? v.gender}</span>
            </button>
          {/each}
        </div>
      {/if}

      <button class="tecla-exportar" onclick={exportAudiobook} disabled={rendering}>
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="vertical-align: -2px"><path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20V4a2 2 0 0 0-2-2H6.5A2.5 2.5 0 0 0 4 4.5v15z"/><path d="M6.5 17H20v5H6.5a2.5 2.5 0 0 1 0-5z"/></svg>
        {rendering ? $t("lector.creando_corto") : $t("lector.guardar_m4b")}
      </button>
      {#if puenteVinculado}
        <button class="tecla-exportar puente" onclick={convertirEnOrdenador} disabled={puenteOcupado}>
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="vertical-align: -2px"><rect x="2" y="4" width="20" height="13" rx="2"/><path d="M8 21h8M12 17v4"/></svg>
          {puenteOcupado ? `${$t("lector.convirtiendo")} ${puenteEtapa ?? ""}` : $t("lector.convertir")}
        </button>
      {/if}
      {#if exportDone}
        <div class="exporte-hecho">
          <span><svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round" style="vertical-align: -1px"><path d="M20 6 9 17l-5-5"/></svg> {$t("lector.guardado")}</span>
          <div class="exporte-acciones">
            <button onclick={shareExport}>{$t("lector.compartir")}</button>
          </div>
        </div>
      {/if}
      {#if customised}
        <button class="tecla-reiniciar" onclick={resetAll}>{$t("lector.reiniciar_todo")}</button>
      {/if}
    </div>
  {/if}

  <!-- ── Ajustar un párrafo (desde el guion) ── -->
  {#if tweakIndex >= 0}
    {@const o = overrides[tweakIndex]}
    <div class="velo" role="presentation" onclick={() => { tweakIndex = -1; voicePickerFor = null; }}></div>
    <div class="hoja">
      <div class="hoja-asa"></div>
      <div class="hoja-cabeza">{$t("lector.ajustar")} {tweakIndex + 1}</div>
      <div class="tweak-vista">{paras[tweakIndex]}</div>

      <div class="ctl">
        <div class="ctl-fila">
          <span class="ctl-rotulo">{$t("lector.velocidad")}</span>
          <span class="ctl-valor">{(o.speed ?? globalSpeed).toFixed(2)}× {#if o.speed == null}<em>{$t("lector.heredada")}</em>{/if}</span>
        </div>
        <input class="deslizador" type="range" min="0.5" max="2.0" step="0.05" value={o.speed ?? globalSpeed}
          oninput={(e) => setParaSpeed(tweakIndex, parseFloat((e.target as HTMLInputElement).value))} aria-label={$t("lector.velocidad")} />
        {#if o.speed != null}<button class="enlace-reiniciar" onclick={() => setParaSpeed(tweakIndex, null)}>{$t("lector.heredar")}</button>{/if}
      </div>

      <div class="ctl">
        <div class="ctl-fila">
          <span class="ctl-rotulo">{$t("lector.pausa_antes")}</span>
          <span class="ctl-valor">{(o.pauseBefore ?? 0).toFixed(1)}s</span>
        </div>
        <input class="deslizador" type="range" min="0" max="5" step="0.1" value={o.pauseBefore ?? 0}
          disabled={tweakIndex === 0}
          oninput={(e) => setParaPause(tweakIndex, parseFloat((e.target as HTMLInputElement).value) || null)} aria-label={$t("lector.pausa_antes")} />
        {#if tweakIndex === 0}<div class="ctl-pista">{$t("lector.primera_pausa")}</div>{/if}
      </div>

      <button class="ctl-selector" onclick={() => (voicePickerFor = voicePickerFor === "para" ? null : "para")}>
        <span class="ctl-rotulo">{$t("lector.voz")}</span>
        <span class="ctl-selector-valor">
          {o.voice ? (voices.find((v) => v.id === o.voice || v.name === o.voice)?.name ?? o.voice) : $t("lector.heredada_palabra")} <span class="caret">▾</span>
        </span>
      </button>
      {#if voicePickerFor === "para"}
        <div class="lista-voces">
          <button class="voz-opcion" class:activa={o.voice === null} onclick={() => setParaVoice(tweakIndex, null)}>{$t("lector.heredar_doc")}</button>
          {#each voices as v}
            <button class="voz-opcion" class:activa={o.voice === v.id || o.voice === v.name} onclick={() => setParaVoice(tweakIndex, v.id)}>
              {v.name}<span class="voz-etiqueta">{v.tags?.[0] ?? v.gender}</span>
            </button>
          {/each}
        </div>
      {/if}

      <div class="tweak-pie">
        <button class="tecla-exportar" onclick={() => { const i = tweakIndex; tweakIndex = -1; readFrom(i); }}>▶ {$t("lector.leer_desde_aqui")}</button>
        <button class="tecla-hecho" onclick={() => { tweakIndex = -1; voicePickerFor = null; }}>{$t("lector.hecho")}</button>
      </div>
    </div>
  {/if}
</div>

<style>
  .escenario {
    position: fixed;
    inset: 0;
    background: var(--yap-papel);
    overflow: hidden;
    touch-action: pan-y;
    user-select: none;
    -webkit-user-select: none;
  }

  .hilo {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 4px;
    background: color-mix(in srgb, var(--yap-borde) 60%, transparent);
    z-index: 4;
  }
  .hilo-lleno {
    height: 100%;
    background: var(--yap-voz, #e0502a);
    transition: width 0.4s ease;
  }

  /* ── El cartel ── */
  .cartel {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    justify-content: center;
    padding: calc(env(safe-area-inset-top) + 34px) 26px calc(env(safe-area-inset-bottom) + 40px);
    gap: 20px;
    transition: opacity 0.25s ease;
  }
  .cartel.atenuado {
    opacity: 0.45;
  }
  .dichas,
  .porvenir {
    font-family: var(--yap-lectura, Georgia, serif);
    color: var(--yap-tinta-suave, #82755a);
    margin: 0;
    line-height: 1.35;
    font-size: 16px;
    display: -webkit-box;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .dichas {
    -webkit-line-clamp: 4;
    color: color-mix(in srgb, var(--yap-ultramar, #2f4bc4) 62%, var(--yap-papel));
    mask-image: linear-gradient(to top, black 55%, transparent 100%);
    -webkit-mask-image: linear-gradient(to top, black 55%, transparent 100%);
  }
  .porvenir {
    -webkit-line-clamp: 3;
    opacity: 0.55;
    mask-image: linear-gradient(to bottom, black 30%, transparent 100%);
    -webkit-mask-image: linear-gradient(to bottom, black 30%, transparent 100%);
  }
  .frase {
    margin: 0;
    font-family: var(--yap-lectura, Georgia, serif);
    font-weight: 720;
    font-variation-settings: "opsz" 40;
    line-height: 1.14;
    letter-spacing: -0.014em;
    overflow-wrap: break-word;
    color: var(--yap-tinta);
    background-image: linear-gradient(color-mix(in srgb, var(--yap-dorado, #e8b41a) 46%, transparent), color-mix(in srgb, var(--yap-dorado, #e8b41a) 46%, transparent));
    background-repeat: no-repeat;
    background-position: 0 92%;
    transition: background-size 0.12s linear;
    box-decoration-break: clone;
    -webkit-box-decoration-break: clone;
  }
  .frase.titulo {
    color: var(--yap-voz, #e0502a);
    letter-spacing: -0.02em;
  }

  /* ── Cromo de pausa ── */
  .pausa-arriba {
    position: absolute;
    top: calc(env(safe-area-inset-top) + 12px);
    left: 14px;
    right: 14px;
    display: flex;
    align-items: center;
    gap: 8px;
    z-index: 6;
  }
  .p-titulo {
    flex: 1;
    min-width: 0;
    text-align: center;
    font-weight: 800;
    font-size: 14px;
    color: var(--yap-voz, #e0502a);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .p-icono {
    width: 46px;
    height: 46px;
    border-radius: 14px;
    border: 1.5px solid var(--yap-tinta);
    background: var(--yap-superficie);
    color: var(--yap-tinta);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    box-shadow: var(--yap-relieve);
    flex-shrink: 0;
    cursor: pointer;
  }
  .p-icono:active {
    transform: translateY(1px);
    box-shadow: var(--yap-hundido);
  }
  .p-icono.pendiente::after {
    content: "";
    position: absolute;
    margin-left: 30px;
    margin-top: -30px;
    width: 9px;
    height: 9px;
    border-radius: 999px;
    background: var(--yap-dorado, #e8b41a);
  }

  /* ── El mando de piano ── */
  .mando {
    position: absolute;
    left: 14px;
    right: 14px;
    bottom: calc(env(safe-area-inset-bottom) + 16px);
    display: grid;
    grid-template-columns: 1fr 2.1fr 1fr;
    gap: 10px;
    z-index: 6;
    animation: mando-sube 0.28s cubic-bezier(0.2, 0.9, 0.3, 1.15) both;
  }
  @keyframes mando-sube {
    from { transform: translateY(120%); opacity: 0; }
    to { transform: translateY(0); opacity: 1; }
  }
  .tecla-piano {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
    min-height: 86px;
    border-radius: 20px;
    border: 2px solid var(--yap-tinta);
    background: var(--yap-superficie);
    color: var(--yap-tinta);
    font-weight: 800;
    font-size: 18px;
    box-shadow: 0 4px 0 var(--yap-tinta), var(--yap-relieve-alto, 0 10px 22px rgba(64, 46, 12, 0.16));
    cursor: pointer;
  }
  .tecla-piano:active {
    transform: translateY(4px);
    box-shadow: 0 0 0 var(--yap-tinta), var(--yap-hundido);
  }
  .tecla-piano.seguir {
    background: var(--yap-tecla-fondo, linear-gradient(180deg, #f4682e, #e0502a));
    color: #fff6ef;
    border-color: color-mix(in srgb, var(--yap-tinta) 70%, #7a2810);
  }

  /* ── La portada ── */
  .portada {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 14px;
    padding: 26px;
    text-align: center;
  }
  .portada-arriba {
    top: calc(env(safe-area-inset-top) + 12px);
  }
  .portada-titulo {
    margin: 0;
    font-family: var(--yap-lectura, Georgia, serif);
    font-weight: 780;
    font-size: clamp(30px, 9vw, 46px);
    line-height: 1.1;
    letter-spacing: -0.02em;
    color: var(--yap-voz, #e0502a);
    transform: rotate(-1.2deg);
    max-width: 22ch;
  }
  .portada-meta {
    margin: 0;
    font-family: var(--yap-mono, ui-monospace, monospace);
    font-size: 12px;
    letter-spacing: 0.14em;
    color: var(--yap-tinta-suave);
  }
  .leer-gigante {
    margin-top: 14px;
    display: inline-flex;
    align-items: center;
    gap: 14px;
    padding: 22px 34px;
    border-radius: 24px;
    border: 2px solid color-mix(in srgb, var(--yap-tinta) 70%, #7a2810);
    background: var(--yap-tecla-fondo, linear-gradient(180deg, #f4682e, #e0502a));
    color: #fff6ef;
    font-weight: 800;
    font-size: 22px;
    box-shadow: 0 5px 0 var(--yap-tinta), var(--yap-relieve-alto, 0 14px 30px rgba(64, 46, 12, 0.2));
    cursor: pointer;
  }
  .leer-gigante:active {
    transform: translateY(5px);
    box-shadow: 0 0 0 var(--yap-tinta), var(--yap-hundido);
  }

  /* ── El dial de velocidad ── */
  .dial {
    position: absolute;
    inset: 0;
    z-index: 8;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 4px;
    background: color-mix(in srgb, var(--yap-papel) 72%, transparent);
    backdrop-filter: blur(6px);
    -webkit-backdrop-filter: blur(6px);
    pointer-events: none;
  }
  .dial strong {
    font-size: clamp(72px, 26vw, 130px);
    font-weight: 800;
    letter-spacing: -0.04em;
    color: var(--yap-voz, #e0502a);
    transform: rotate(-2deg);
  }
  .dial span {
    font-family: var(--yap-mono, ui-monospace, monospace);
    font-size: 12px;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    color: var(--yap-tinta-suave);
  }

  /* ── Hojas (guion, taller, ajuste) ── */
  .velo {
    position: fixed;
    inset: 0;
    z-index: 10;
    background: rgba(43, 36, 24, 0.35);
  }
  .hoja {
    position: fixed;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: 11;
    max-height: 82dvh;
    overflow-y: auto;
    background: var(--yap-papel);
    border-radius: 22px 22px 0 0;
    box-shadow: 0 -12px 40px rgba(64, 46, 12, 0.28);
    padding: 10px 16px calc(env(safe-area-inset-bottom) + 18px);
    display: flex;
    flex-direction: column;
    gap: 12px;
    animation: hoja-sube 0.26s ease both;
  }
  @keyframes hoja-sube {
    from { transform: translateY(30%); opacity: 0.4; }
    to { transform: translateY(0); opacity: 1; }
  }
  .hoja-asa {
    width: 44px;
    height: 5px;
    border-radius: 3px;
    background: var(--yap-borde);
    margin: 2px auto 0;
  }
  .hoja-cabeza {
    font-family: var(--yap-mono, ui-monospace, monospace);
    font-size: 11px;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    color: var(--yap-tinta-suave);
    text-align: center;
  }

  .guion-lista {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .guion-fila {
    display: flex;
    align-items: flex-start;
    gap: 4px;
    border-radius: 12px;
  }
  .guion-fila.actual {
    background: color-mix(in srgb, var(--yap-dorado, #e8b41a) 22%, transparent);
  }
  .guion-parrafo {
    flex: 1;
    text-align: left;
    border: 0;
    background: transparent;
    color: var(--yap-tinta);
    font-family: var(--yap-lectura, Georgia, serif);
    font-size: 16px;
    line-height: 1.5;
    padding: 8px 6px 8px 10px;
    cursor: pointer;
  }
  .guion-fila.es-titulo .guion-parrafo {
    font-weight: 800;
    color: var(--yap-voz, #e0502a);
    font-size: 18px;
  }
  .guion-ajustar {
    border: 0;
    background: transparent;
    color: var(--yap-tinta-suave);
    font-size: 18px;
    padding: 8px 10px;
    cursor: pointer;
  }
  .guion-ajustar.tocado {
    color: var(--yap-dorado, #e8b41a);
  }

  .ctl {
    display: flex;
    flex-direction: column;
    gap: 6px;
    background: var(--yap-superficie);
    border: 1px solid var(--yap-borde);
    border-radius: 14px;
    padding: 12px 14px;
  }
  .ctl-fila {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }
  .ctl-rotulo {
    font-family: var(--yap-mono, ui-monospace, monospace);
    font-size: 11px;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--yap-tinta-suave);
  }
  .ctl-valor {
    font-weight: 800;
  }
  .ctl-pista {
    font-size: 12px;
    color: var(--yap-tinta-suave);
  }
  .deslizador {
    width: 100%;
    accent-color: var(--yap-voz, #e0502a);
  }
  .ctl-selector {
    display: flex;
    justify-content: space-between;
    align-items: center;
    background: var(--yap-superficie);
    border: 1px solid var(--yap-borde);
    border-radius: 14px;
    padding: 14px;
    color: var(--yap-tinta);
    cursor: pointer;
  }
  .ctl-selector-valor {
    font-weight: 800;
  }
  .lista-voces {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .voz-opcion {
    display: flex;
    justify-content: space-between;
    align-items: center;
    border: 1px solid var(--yap-borde);
    background: var(--yap-superficie);
    border-radius: 12px;
    padding: 12px 14px;
    color: var(--yap-tinta);
    font-weight: 700;
    cursor: pointer;
  }
  .voz-opcion.activa {
    border-color: var(--yap-voz, #e0502a);
    background: color-mix(in srgb, var(--yap-voz, #e0502a) 10%, var(--yap-superficie));
  }
  .voz-etiqueta {
    font-family: var(--yap-mono, ui-monospace, monospace);
    font-size: 10px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--yap-tinta-suave);
  }
  .tecla-exportar {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
    padding: 15px;
    border-radius: 14px;
    border: 0;
    background: var(--yap-tecla-fondo, linear-gradient(180deg, #f4682e, #e0502a));
    color: #fff6ef;
    font-weight: 800;
    font-size: 15px;
    box-shadow: var(--yap-relieve);
    cursor: pointer;
  }
  .tecla-exportar.puente {
    background: var(--yap-ultramar, #2f4bc4);
  }
  .tecla-exportar:disabled {
    opacity: 0.6;
  }
  .tecla-reiniciar,
  .tecla-hecho,
  .enlace-reiniciar {
    border: 0;
    background: transparent;
    color: var(--yap-tinta-suave);
    font-weight: 700;
    padding: 8px;
    cursor: pointer;
  }
  .exporte-hecho {
    display: flex;
    justify-content: space-between;
    align-items: center;
    background: color-mix(in srgb, var(--yap-dorado, #e8b41a) 16%, var(--yap-superficie));
    border: 1px solid var(--yap-borde);
    border-radius: 12px;
    padding: 10px 14px;
    font-weight: 700;
  }
  .exporte-acciones button {
    border: 0;
    background: transparent;
    color: var(--yap-ultramar, #2f4bc4);
    font-weight: 800;
    cursor: pointer;
  }
  .tweak-vista {
    font-family: var(--yap-lectura, Georgia, serif);
    font-size: 14px;
    color: var(--yap-tinta-suave);
    display: -webkit-box;
    -webkit-line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .tweak-pie {
    display: flex;
    gap: 10px;
    justify-content: space-between;
    align-items: center;
  }

  .tira-render {
    position: absolute;
    left: 14px;
    right: 14px;
    bottom: calc(env(safe-area-inset-bottom) + 110px);
    z-index: 7;
    background: var(--yap-superficie);
    border: 1px solid var(--yap-borde);
    border-radius: 12px;
    padding: 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 12px;
    color: var(--yap-tinta-suave);
    box-shadow: var(--yap-relieve);
  }
  .tira-barra {
    height: 8px;
    border-radius: 5px;
    background: var(--yap-superficie-2);
    overflow: hidden;
  }
  .tira-barra div {
    height: 100%;
    background: var(--yap-dorado, #e8b41a);
  }
  .brindis {
    position: absolute;
    left: 50%;
    bottom: calc(env(safe-area-inset-bottom) + 120px);
    transform: translateX(-50%);
    z-index: 9;
    background: var(--yap-tinta);
    color: var(--yap-papel);
    border-radius: 999px;
    padding: 10px 18px;
    font-weight: 700;
    font-size: 14px;
    max-width: 84vw;
    text-align: center;
  }
</style>
