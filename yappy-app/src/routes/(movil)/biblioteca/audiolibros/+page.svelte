<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { get as getStore } from "svelte/store";
  import { isIOS } from "$lib/platform";
  import { notifyError } from "$lib/ui";

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

  async function refreshLibrary() {
    if (!getStore(isIOS)) return;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      libraryItems = (await invoke("list_rendered_audiobooks_cmd")) as LibraryItem[];
      invoke("library_reindex_spotlight_cmd").catch((e) => console.warn("[spotlight]", e));
    } catch (e) {
      console.warn("[library] refresh failed:", e);
    }
  }
  async function openChapterPicker(item: LibraryItem) {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      openChapters = (await invoke("library_chapters_cmd", { path: item.path })) as ChapterEntry[];
      openChaptersForPath = item.path;
    } catch (e) { console.warn("[chapters] failed:", e); }
  }
  function closeChapterPicker() { openChaptersForPath = null; openChapters = []; }
  async function jumpToChapter(secs: number) {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("library_seek_cmd", { secs });
    closeChapterPicker();
    await pollLibraryStatus();
  }
  async function libraryPlay(item: LibraryItem, fromStart = false) {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("library_play_cmd", { path: item.path, fromStart });
      startLibraryPolling();
    } catch (e) { console.warn("[library] play failed:", e); }
  }
  async function libraryPause() {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("library_pause_cmd");
    await pollLibraryStatus();
  }
  async function libraryResume() {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("library_resume_cmd");
    await pollLibraryStatus();
  }
  async function libraryStop() {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("library_stop_cmd");
    stopLibraryPolling();
    libraryStatus = { current_path: null, position_secs: 0, duration_secs: 0, playing: false };
    await refreshLibrary();
  }
  async function librarySeek(delta: number) {
    const { invoke } = await import("@tauri-apps/api/core");
    const next = Math.max(0, libraryStatus.position_secs + delta);
    await invoke("library_seek_cmd", { secs: next });
    await pollLibraryStatus();
  }
  async function libraryDelete(item: LibraryItem) {
    if (!confirm(`delete '${item.name}'? this cannot be undone.`)) return;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("library_delete_cmd", { path: item.path });
      await refreshLibrary();
    } catch (e) { notifyError(`delete failed: ${e}`); }
  }
  async function pollLibraryStatus() {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      libraryStatus = (await invoke("library_status_cmd")) as LibraryStatus;
    } catch {}
  }
  function startLibraryPolling() {
    if (libraryPollInterval) return;
    libraryPollInterval = window.setInterval(pollLibraryStatus, 1000);
  }
  function stopLibraryPolling() {
    if (libraryPollInterval) { clearInterval(libraryPollInterval); libraryPollInterval = null; }
  }
  async function shareLibraryItem(path: string) {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("share_file_cmd", { path });
    } catch (e) { console.warn("[library] share failed:", e); }
  }
  function libraryFmtTime(s: number): string {
    if (!isFinite(s) || s <= 0) return "0:00";
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    const sec = Math.floor(s % 60);
    if (h > 0) return `${h}:${String(m).padStart(2, "0")}:${String(sec).padStart(2, "0")}`;
    return `${m}:${String(sec).padStart(2, "0")}`;
  }

  onMount(refreshLibrary);
  onDestroy(stopLibraryPolling);
</script>

<section class="lib-wrap">
  <header class="section-head">
    <h2>library</h2>
    <p>your rendered audiobooks — tap to play, share, or open in books.</p>
  </header>
  {#if libraryStatus.current_path && libraryStatus.duration_secs > 0}
    <div class="card lib-now-playing">
      <div class="lnp-title">{libraryStatus.current_path.split("/").pop() ?? ""}</div>
      <div class="lnp-bar"><div class="lnp-fill" style="--w: {Math.min(100, (libraryStatus.position_secs / libraryStatus.duration_secs) * 100)}%"></div></div>
      <div class="lnp-meta">
        <span>{libraryFmtTime(libraryStatus.position_secs)} / {libraryFmtTime(libraryStatus.duration_secs)}</span>
        <span class="muted">{libraryFmtTime(Math.max(0, libraryStatus.duration_secs - libraryStatus.position_secs))} left</span>
      </div>
      <div class="lnp-controls">
        <button class="btn" onclick={() => librarySeek(-15)} aria-label="back 15 seconds">−15s</button>
        {#if libraryStatus.playing}
          <button class="btn-pink" onclick={libraryPause} aria-label="pause">❚❚ pause</button>
        {:else}
          <button class="btn-pink" onclick={libraryResume} aria-label="resume">▶ resume</button>
        {/if}
        <button class="btn" onclick={() => librarySeek(15)} aria-label="forward 15 seconds">+15s</button>
        <button class="btn" onclick={libraryStop} aria-label="stop">stop</button>
      </div>
    </div>
  {/if}

  <div class="card pref-card">
    {#if libraryItems.length === 0}
      <p class="muted">no audiobooks yet. render one from a document — it'll appear here.</p>
    {:else}
      <ul class="lib-list" aria-label="saved audiobooks">
        {#each libraryItems as item (item.path)}
          <li class="lib-card" class:current={libraryStatus.current_path === item.path}>
            <div class="lib-card-head">
              <div class="lib-card-icon" aria-hidden="true">🎧</div>
              <div class="lib-card-body">
                <div class="lib-card-name">{item.name}</div>
                <div class="lib-card-sub">
                  {#if item.duration_secs}{libraryFmtTime(item.duration_secs)}{:else}{(item.size / (1024 * 1024)).toFixed(1)} MB{/if}
                  {#if item.chapter_count > 0} · {item.chapter_count} chapters{/if}
                  · {new Date(item.mtime_ms).toLocaleDateString()}
                </div>
                {#if item.resume_secs > 5 && libraryStatus.current_path !== item.path}
                  <div class="lib-resume-hint">resume from {libraryFmtTime(item.resume_secs)}</div>
                {/if}
              </div>
            </div>
            <div class="lib-card-actions">
              {#if libraryStatus.current_path === item.path}
                {#if libraryStatus.playing}
                  <button class="btn-pink" onclick={libraryPause} aria-label="pause">❚❚</button>
                {:else}
                  <button class="btn-pink" onclick={libraryResume} aria-label="resume">▶</button>
                {/if}
              {:else}
                <button class="btn-pink" onclick={() => libraryPlay(item)} aria-label={`play ${item.name}`}>▶ play</button>
              {/if}
              {#if item.resume_secs > 5}
                <button class="btn-outline" onclick={() => libraryPlay(item, true)} aria-label={`play ${item.name} from start`} title="play from start">⤺</button>
              {/if}
              {#if item.chapter_count > 0}
                <button class="btn-outline" onclick={() => openChapterPicker(item)} aria-label={`browse chapters of ${item.name}`} title="chapters">☰</button>
              {/if}
              <button class="btn-outline" onclick={() => shareLibraryItem(item.path)} aria-label={`share ${item.name}`}>📤</button>
              <button class="btn-outline danger" onclick={() => libraryDelete(item)} aria-label={`delete ${item.name}`}>🗑</button>
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
          <h3>chapters</h3>
          <button class="chapter-close" onclick={closeChapterPicker} aria-label="close chapters">✕</button>
        </header>
        {#if openChapters.length === 0}
          <p class="muted">no chapters in this file.</p>
        {:else}
          <ul class="chapter-list" aria-label="chapter list">
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
