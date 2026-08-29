<script lang="ts">
  // LA BIBLIOTECA DE AUDIOLIBROS, compartida entre el móvil y el
  // escritorio. El motor de audio es DUAL: en iOS manda AVAudioPlayer
  // (comandos library_*); en escritorio, un <audio> del propio webview
  // (asset protocol). Y si la pieza es un .yappy, el KARAOKE de frase en
  // curso se pinta desde sus tiempos.
  import { onMount, onDestroy } from "svelte";
  import { get as getStore } from "svelte/store";
  import { invoke, convertFileSrc } from "@tauri-apps/api/core";
  import { isIOS } from "$lib/platform";
  import { t } from "$lib/i18n";
  import { notifyError } from "$lib/ui";
  import { libraryTiempos, libraryAudioSrc, type TiempoFrase } from "$lib/ipc";
  import ImprentaEncargos from "$lib/ImprentaEncargos.svelte";
  import { onImprentaActualizada } from "$lib/ipc";

  // La página móvil trae su propia cabecera (safe area + volver): silencia
  // la del shell de escritorio con esta prop.
  let { conCabecera = true }: { conCabecera?: boolean } = $props();

  type LibraryItem = {
    name: string;
    path: string;
    size: number;
    mtime_ms: number;
    duration_secs: number | null;
    chapter_count: number;
    first_chapter_title: string | null;
    resume_secs: number;
  };
  type LibraryStatus = {
    current_path: string | null;
    position_secs: number;
    duration_secs: number;
    playing: boolean;
  };
  type ChapterEntry = { title: string; start_secs: number };

  let libraryItems: LibraryItem[] = $state([]);
  let libraryStatus: LibraryStatus = $state({ current_path: null, position_secs: 0, duration_secs: 0, playing: false });
  let libraryPollInterval: number | null = null;
  let openChaptersForPath: string | null = $state(null);
  let openChapters: ChapterEntry[] = $state([]);

  // El karaoke del .yappy: los tiempos de la pieza que suena.
  let tiemposActuales: TiempoFrase[] = $state([]);
  const fraseActual = $derived.by(() => {
    if (!tiemposActuales.length) return null;
    const p = libraryStatus.position_secs;
    return tiemposActuales.find((f) => p >= f.ini_s && p < f.fin_s) ?? null;
  });

  // ── El motor de ESCRITORIO: un <audio> del webview ────────────────────
  let audioEl: HTMLAudioElement | null = null;
  let rutaEscritorio: string | null = null;

  const esIOS = () => getStore(isIOS);

  async function cargarTiempos(path: string) {
    tiemposActuales = path.toLowerCase().endsWith(".yappy")
      ? await libraryTiempos(path).catch(() => [])
      : [];
  }

  async function refreshLibrary() {
    try {
      libraryItems = (await invoke("list_rendered_audiobooks_cmd")) as LibraryItem[];
      if (esIOS()) invoke("library_reindex_spotlight_cmd").catch(() => {});
    } catch (e) {
      console.warn("[library] refresh failed:", e);
    }
  }

  async function libraryPlay(item: LibraryItem, fromStart = false) {
    try {
      await cargarTiempos(item.path);
      if (esIOS()) {
        await invoke("library_play_cmd", { path: item.path, fromStart });
      } else {
        const audioPath = await libraryAudioSrc(item.path);
        if (!audioEl) return;
        rutaEscritorio = item.path;
        audioEl.src = convertFileSrc(audioPath);
        audioEl.currentTime = fromStart ? 0 : item.resume_secs || 0;
        await audioEl.play();
      }
      startLibraryPolling();
    } catch (e) {
      console.warn("[library] play failed:", e);
      notifyError(String(e));
    }
  }
  async function libraryPause() {
    if (esIOS()) await invoke("library_pause_cmd");
    else audioEl?.pause();
    await pollLibraryStatus();
  }
  async function libraryResume() {
    if (esIOS()) await invoke("library_resume_cmd");
    else await audioEl?.play();
    await pollLibraryStatus();
  }
  async function libraryStop() {
    if (esIOS()) await invoke("library_stop_cmd");
    else if (audioEl) {
      audioEl.pause();
      audioEl.removeAttribute("src");
      rutaEscritorio = null;
    }
    stopLibraryPolling();
    libraryStatus = { current_path: null, position_secs: 0, duration_secs: 0, playing: false };
    tiemposActuales = [];
    await refreshLibrary();
  }
  async function librarySeekTo(secs: number) {
    if (esIOS()) await invoke("library_seek_cmd", { secs });
    else if (audioEl) audioEl.currentTime = secs;
    await pollLibraryStatus();
  }
  async function librarySeek(delta: number) {
    await librarySeekTo(Math.max(0, libraryStatus.position_secs + delta));
  }
  async function libraryDelete(item: LibraryItem) {
    if (!confirm($t("biblioteca.confirmar_borrado").replace("{nombre}", item.name))) return;
    try {
      await invoke("library_delete_cmd", { path: item.path });
      await refreshLibrary();
    } catch (e) {
      notifyError(String(e));
    }
  }
  async function pollLibraryStatus() {
    if (esIOS()) {
      try {
        libraryStatus = (await invoke("library_status_cmd")) as LibraryStatus;
      } catch {}
    } else if (audioEl && rutaEscritorio) {
      libraryStatus = {
        current_path: rutaEscritorio,
        position_secs: audioEl.currentTime || 0,
        duration_secs: audioEl.duration || 0,
        playing: !audioEl.paused,
      };
    }
  }
  function startLibraryPolling() {
    if (libraryPollInterval) return;
    libraryPollInterval = window.setInterval(pollLibraryStatus, 500);
  }
  function stopLibraryPolling() {
    if (libraryPollInterval) {
      clearInterval(libraryPollInterval);
      libraryPollInterval = null;
    }
  }
  async function openChapterPicker(item: LibraryItem) {
    try {
      openChapters = (await invoke("library_chapters_cmd", { path: item.path })) as ChapterEntry[];
      openChaptersForPath = item.path;
    } catch {}
  }
  function closeChapterPicker() {
    openChaptersForPath = null;
    openChapters = [];
  }
  async function jumpToChapter(secs: number) {
    await librarySeekTo(secs);
    closeChapterPicker();
  }
  async function shareLibraryItem(path: string) {
    try {
      await invoke("share_file_cmd", { path });
    } catch (e) {
      console.warn("[library] share failed:", e);
    }
  }
  function libraryFmtTime(s: number): string {
    if (!isFinite(s) || s <= 0) return "0:00";
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    const sec = Math.floor(s % 60);
    if (h > 0) return `${h}:${String(m).padStart(2, "0")}:${String(sec).padStart(2, "0")}`;
    return `${m}:${String(sec).padStart(2, "0")}`;
  }
  const nombreQueSuena = $derived.by(() => {
    const p = libraryStatus.current_path;
    if (!p) return "";
    return libraryItems.find((i) => i.path === p)?.name ?? p.split("/").pop() ?? "";
  });

  let limpiarImprenta: (() => void) | null = null;
  onMount(async () => {
    refreshLibrary();
    // Cuando la imprenta termina un encargo, el libro nuevo aparece.
    limpiarImprenta = await onImprentaActualizada(() => refreshLibrary());
  });
  onDestroy(() => {
    stopLibraryPolling();
    audioEl?.pause();
    limpiarImprenta?.();
  });
</script>

<!-- El altavoz del escritorio (invisible; en iOS ni se usa). -->
<audio
  bind:this={audioEl}
  onplay={pollLibraryStatus}
  onpause={pollLibraryStatus}
  onended={libraryStop}
  style="display: none"
></audio>

<section class="lib-wrap">
  {#if conCabecera}
    <header class="section-head">
      <h2>{$t("biblioteca.titulo")}</h2>
      <p>{$t("biblioteca.subtitulo")}</p>
    </header>
  {:else}
    <p class="lib-pista">{$t("biblioteca.subtitulo")}</p>
  {/if}
  <!-- LA IMPRENTA: los encargos en marcha, encima de los libros hechos. -->
  <ImprentaEncargos />
  {#if libraryStatus.current_path && libraryStatus.duration_secs > 0}
    <div class="card lib-now-playing">
      <div class="lnp-title">{nombreQueSuena}</div>
      {#if fraseActual}
        <!-- EL KARAOKE del .yappy: la frase que la voz dice AHORA. -->
        <div class="lnp-karaoke">{fraseActual.texto}</div>
      {/if}
      <div class="lnp-bar"><div class="lnp-fill" style="--w: {Math.min(100, (libraryStatus.position_secs / libraryStatus.duration_secs) * 100)}%"></div></div>
      <div class="lnp-meta">
        <span>{libraryFmtTime(libraryStatus.position_secs)} / {libraryFmtTime(libraryStatus.duration_secs)}</span>
        <span class="muted">−{libraryFmtTime(Math.max(0, libraryStatus.duration_secs - libraryStatus.position_secs))}</span>
      </div>
      <div class="lnp-controls">
        <button class="btn" onclick={() => librarySeek(-15)} aria-label={$t("biblioteca.atras_15")}>−15s</button>
        {#if libraryStatus.playing}
          <button class="btn-pink" onclick={libraryPause} aria-label={$t("comun.pausa")}>
            <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor" aria-hidden="true"><rect x="6" y="5" width="4.4" height="14" rx="1.6"/><rect x="13.6" y="5" width="4.4" height="14" rx="1.6"/></svg>
            {$t("comun.pausa")}
          </button>
        {:else}
          <button class="btn-pink" onclick={libraryResume} aria-label={$t("comun.reanudar")}>
            <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor" aria-hidden="true"><path d="M8 5.6 Q8.8 4.8 9.9 5.5 L18 11 Q19 12 18 12.9 L9.9 18.5 Q8.8 19.2 8.3 18 Q7.6 12 8 5.6 Z"/></svg>
            {$t("comun.reanudar")}
          </button>
        {/if}
        <button class="btn" onclick={() => librarySeek(15)} aria-label={$t("biblioteca.adelante_15")}>+15s</button>
        <button class="btn" onclick={libraryStop} aria-label={$t("comun.parar")}>{$t("comun.parar")}</button>
      </div>
    </div>
  {/if}

  <div class="card pref-card">
    {#if libraryItems.length === 0}
      <p class="muted">{$t("biblioteca.vacia")}</p>
    {:else}
      <ul class="lib-list" aria-label={$t("biblioteca.titulo")}>
        {#each libraryItems as item (item.path)}
          <li class="lib-card" class:current={libraryStatus.current_path === item.path}>
            <div class="lib-card-head">
              <div class="lib-card-icon" aria-hidden="true">
                <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" aria-hidden="true"><path d="M4.5 13.2 Q4.2 6.4 12 6.2 Q19.8 6.4 19.5 13.2 M4.6 13 q-1.8 0.6 -1.2 3 q0.5 2.2 2.4 1.9 q1 -0.2 0.9 -1.3 l-0.3 -2.6 q-0.1 -1.2 -1.8 -1 z M19.4 13 q1.8 0.6 1.2 3 q-0.5 2.2 -2.4 1.9 q-1 -0.2 -0.9 -1.3 l0.3 -2.6 q0.1 -1.2 1.8 -1 z"/></svg>
              </div>
              <div class="lib-card-body">
                <div class="lib-card-name">{item.name}</div>
                <div class="lib-card-sub">
                  {#if item.duration_secs}{libraryFmtTime(item.duration_secs)}{:else}{(item.size / (1024 * 1024)).toFixed(1)} MB{/if}
                  {#if item.chapter_count > 0} · {item.chapter_count} {$t("biblioteca.capitulos")}{/if}
                  {#if item.path.toLowerCase().endsWith(".yappy")} · yappy{/if}
                  · {new Date(item.mtime_ms).toLocaleDateString()}
                </div>
                {#if item.resume_secs > 5 && libraryStatus.current_path !== item.path}
                  <div class="lib-resume-hint">{$t("biblioteca.retomar")} {libraryFmtTime(item.resume_secs)}</div>
                {/if}
              </div>
            </div>
            <div class="lib-card-actions">
              {#if libraryStatus.current_path === item.path}
                {#if libraryStatus.playing}
                  <button class="btn-pink" onclick={libraryPause} aria-label={$t("comun.pausa")}>
                    <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor" aria-hidden="true"><rect x="6" y="5" width="4.4" height="14" rx="1.6"/><rect x="13.6" y="5" width="4.4" height="14" rx="1.6"/></svg>
                  </button>
                {:else}
                  <button class="btn-pink" onclick={libraryResume} aria-label={$t("comun.reanudar")}>
                    <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor" aria-hidden="true"><path d="M8 5.6 Q8.8 4.8 9.9 5.5 L18 11 Q19 12 18 12.9 L9.9 18.5 Q8.8 19.2 8.3 18 Q7.6 12 8 5.6 Z"/></svg>
                  </button>
                {/if}
              {:else}
                <button class="btn-pink" onclick={() => libraryPlay(item)} aria-label={$t("comun.escuchar")}>
                  <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor" aria-hidden="true"><path d="M8 5.6 Q8.8 4.8 9.9 5.5 L18 11 Q19 12 18 12.9 L9.9 18.5 Q8.8 19.2 8.3 18 Q7.6 12 8 5.6 Z"/></svg>
                  {$t("comun.escuchar")}
                </button>
              {/if}
              {#if item.resume_secs > 5}
                <button class="btn-outline" onclick={() => libraryPlay(item, true)} aria-label={$t("biblioteca.desde_el_principio")} title={$t("biblioteca.desde_el_principio")}>
                  <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" aria-hidden="true"><path d="M6 5 v14 M18.5 6.5 L9.5 12 L18.5 17.5 Z"/></svg>
                </button>
              {/if}
              {#if item.chapter_count > 0}
                <button class="btn-outline" onclick={() => openChapterPicker(item)} aria-label={$t("biblioteca.capitulos")} title={$t("biblioteca.capitulos")}>
                  <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" aria-hidden="true"><path d="M5 7 h14 M5 12 h14 M5 17 h9"/></svg>
                </button>
              {/if}
              <button class="btn-outline" onclick={() => shareLibraryItem(item.path)} aria-label={$t("lector.compartir")}>
                <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 15.5 V5 M8.6 8.2 L12 4.8 L15.4 8.2 M5.5 12.5 v6 q0 1.4 1.4 1.4 h10.2 q1.4 0 1.4 -1.4 v-6"/></svg>
              </button>
              <button class="btn-outline danger" onclick={() => libraryDelete(item)} aria-label={$t("comun.borrar")}>
                <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M5.5 7.5 h13 M9.8 7 V5.4 q0 -0.9 0.9 -0.9 h2.6 q0.9 0 0.9 0.9 V7 M7 7.8 l0.7 10.4 q0.1 1.2 1.3 1.2 h6 q1.2 0 1.3 -1.2 L17 7.8"/></svg>
              </button>
            </div>
          </li>
        {/each}
      </ul>
    {/if}
  </div>

  {#if openChaptersForPath != null}
    <div class="chapter-overlay" onclick={closeChapterPicker} role="dialog" tabindex="-1">
      <div class="chapter-sheet" onclick={(e) => e.stopPropagation()}>
        <header class="chapter-head">
          <h3>{$t("biblioteca.capitulos")}</h3>
          <button class="chapter-close" onclick={closeChapterPicker} aria-label={$t("comun.cerrar")}>
            <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" aria-hidden="true"><path d="M6 6 L18 18 M18 6 L6 18"/></svg>
          </button>
        </header>
        {#if openChapters.length === 0}
          <p class="muted">{$t("biblioteca.sin_capitulos")}</p>
        {:else}
          <ul class="chapter-list" aria-label={$t("biblioteca.capitulos")}>
            {#each openChapters as ch, i}
              <li>
                <button
                  class="chapter-row"
                  class:active={libraryStatus.current_path === openChaptersForPath &&
                    libraryStatus.position_secs >= ch.start_secs &&
                    (i + 1 >= openChapters.length || libraryStatus.position_secs < openChapters[i + 1].start_secs)}
                  onclick={() => {
                    if (libraryStatus.current_path === openChaptersForPath) {
                      jumpToChapter(ch.start_secs);
                    } else {
                      const it = libraryItems.find((x) => x.path === openChaptersForPath);
                      if (it) { libraryPlay(it).then(() => jumpToChapter(ch.start_secs)); }
                    }
                  }}>
                  <span class="chapter-num">{i + 1}.</span>
                  <span class="chapter-title">{ch.title}</span>
                  <span class="chapter-time">{libraryFmtTime(ch.start_secs)}</span>
                </button>
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    </div>
  {/if}
</section>

<style>
  .lib-pista {
    margin: 0 0 2px;
    font-size: 13px;
    color: var(--yap-tinta-suave, #82755a);
  }
  .lnp-karaoke {
    font-family: var(--yap-lectura, Georgia, serif);
    font-size: 15px;
    line-height: 1.35;
    margin: 6px 0 8px;
    color: var(--yap-tinta, #2b2418);
    opacity: 0.92;
  }
</style>
