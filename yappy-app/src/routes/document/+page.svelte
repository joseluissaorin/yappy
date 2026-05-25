<script lang="ts">
  import { onMount, onDestroy, tick } from "svelte";
  import { open as dialogOpen, save as dialogSave } from "@tauri-apps/plugin-dialog";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import YappyMascot from "$lib/YappyMascot.svelte";
  import SoundWaves from "$lib/SoundWaves.svelte";
  import {
    type DocumentLoaded,
    type PlaybackSnapshot,
    type Voice,
    type Settings,
    type ParagraphSpec,
    listVoices,
    getSettings,
    setVoice as setGlobalVoice,
    onDocumentLoaded,
    onDocumentError,
    onPlaybackState,
    onPlaybackStarting,
    onChunkSynthesized,
    onSynthError,
    onAudiobookRenderProgress,
    onAudiobookRenderDone,
    readDocumentParagraphs,
    readFile,
    getCurrentDocument,
    documentWindowReady,
    renderAudiobook,
    revealLogFile,
    tailLog,
    logToBackend,
    saveProject,
    loadProject,
    saveCurrentAudio,
    skip,
    stopPlayback,
    togglePause,
  } from "$lib/ipc";

  // Per-paragraph editor state. Voice/speed/pause are nullable: null = inherit global.
  // `kind` is a hint from markdown parsing ("heading1", "list", "hr", …) — used
  // for visual styling AND to set sensible default pauses/speeds.
  type ParaState = {
    text: string;
    voice: string | null;
    speed: number | null;
    pauseBefore: number | null;
    kind: string;
  };

  // ── document state ─────────────────────────────────────────────────────────
  let doc: DocumentLoaded | null = $state(null);
  let paragraphs: ParaState[] = $state([]);
  /// Tracks which paragraph (by index) is currently being edited inline. -1 = none.
  let editingIndex = $state(-1);
  let editingText = $state("");
  /// Index of paragraph whose settings drawer is open (-1 = none).
  let settingsOpenIndex = $state(-1);

  // ── playback / synth state ─────────────────────────────────────────────────
  let snap: PlaybackSnapshot = $state({
    playing: false, paused: false, current_text: "", current_index: 0,
    current_paragraph_index: 0, total: 0, total_paragraphs: 0,
    elapsed_secs: 0, duration_secs: 0, volume: 1.0, output_sample_rate: 44100,
  });
  let baseParagraphIndex = $state(0);
  let activeParagraphIndex = $state(-1);
  let voices: Voice[] = $state([]);
  let settings: Settings | null = $state(null);
  let voicePickerForParagraph = $state(-1); // index of paragraph whose voice picker is open
  let loading = $state(true);
  let errorMsg: string | null = $state(null);
  let dragOver = $state(false);
  let savedToast: string | null = $state(null);

  // Audiobook export state.
  let rendering = $state(false);
  let renderProgress: { index: number; total: number; stage: string } | null = $state(null);

  // Sleep timer state. `sleepUntil` is a wall-clock timestamp (ms); once
  // Date.now() crosses it we call stopPlayback(). A reactive setInterval
  // ticks the UI countdown every second.
  let sleepUntil = $state<number | null>(null);
  let sleepMenuOpen = $state(false);
  let sleepTickInterval: number | null = null;

  function setSleepIn(secs: number) {
    sleepMenuOpen = false;
    if (sleepTickInterval) { clearInterval(sleepTickInterval); sleepTickInterval = null; }
    if (secs <= 0) {
      sleepUntil = null;
      return;
    }
    sleepUntil = Date.now() + secs * 1000;
    sleepTickInterval = window.setInterval(() => {
      if (sleepUntil != null && Date.now() >= sleepUntil) {
        sleepUntil = null;
        if (sleepTickInterval) { clearInterval(sleepTickInterval); sleepTickInterval = null; }
        stopPlayback();
      } else {
        // Force reactive refresh of the countdown display.
        sleepUntil = sleepUntil;
      }
    }, 1000);
  }
  function toggleSleepMenu() {
    sleepMenuOpen = !sleepMenuOpen;
  }

  // Per-document playback bookmark — "remember where I am" so the next
  // open of this same document resumes at that paragraph. Keyed by doc
  // path (or document filename if no path). Lives in localStorage so it
  // survives app restarts without a backend round-trip.
  function bookmarkKey(): string | null {
    if (!doc) return null;
    return `yappy:bookmark:${doc.path ?? doc.filename}`;
  }
  function saveBookmark() {
    const k = bookmarkKey();
    if (!k) return;
    const idx = snap.current_paragraph_index;
    if (idx == null || idx < 0) return;
    try { localStorage.setItem(k, String(idx)); } catch {}
    flashToast(`bookmarked at paragraph ${idx + 1}`);
  }
  function loadBookmark(): number | null {
    const k = bookmarkKey();
    if (!k) return null;
    try {
      const raw = localStorage.getItem(k);
      if (!raw) return null;
      const n = parseInt(raw, 10);
      return isFinite(n) && n >= 0 ? n : null;
    } catch { return null; }
  }

  // Doc-window-local rhythm multiplier. Adjusts the markdown-driven speed pattern
  // and pause spacing without touching the global voice speed in Preferences.
  // 0.5 = much slower / longer pauses; 1.0 = as-parsed; 2.0 = much faster / tighter.
  // Persisted in the project autosave file alongside paragraph state.
  let rhythmMult = $state(1.0);

  // Sidebar (chapter nav). Hidden by default for short docs; user toggles.
  let sidebarOpen = $state(true);

  // Derived list of "chapters" — every paragraph whose kind is a heading. Heading
  // level controls indent. Includes the global index so click-to-scroll works.
  type Chapter = { index: number; text: string; level: number };
  let chapters: Chapter[] = $derived(
    paragraphs
      .map((p, i): Chapter | null => {
        const m = /^heading([1-6])$/.exec(p.kind);
        return m ? { index: i, text: p.text, level: parseInt(m[1], 10) } : null;
      })
      .filter((c): c is Chapter => c !== null),
  );

  /// Set of chapter-indices (positions in `chapters[]`) currently selected for
  /// the "render selection" flow. Empty set = render everything (default).
  let selectedChapterIdx = $state<Set<number>>(new Set<number>());
  function toggleChapterSelection(ci: number) {
    const next = new Set(selectedChapterIdx);
    if (next.has(ci)) next.delete(ci); else next.add(ci);
    selectedChapterIdx = next;
  }
  function clearSelection() { selectedChapterIdx = new Set(); }
  function selectAllChapters() {
    selectedChapterIdx = new Set(chapters.map((_, i) => i));
  }
  /// Resolve the paragraph-index range each chapter covers — from its heading
  /// paragraph to (the next heading paragraph - 1) OR end-of-document.
  function chapterParagraphRange(ci: number): { start: number; end: number } {
    const start = chapters[ci].index;
    const end = ci + 1 < chapters.length ? chapters[ci + 1].index - 1 : paragraphs.length - 1;
    return { start, end };
  }

  /// Find the chapter that *contains* the currently-active paragraph (largest
  /// heading index ≤ activeParagraphIndex). -1 if none.
  let activeChapter = $derived.by(() => {
    if (activeParagraphIndex < 0 || chapters.length === 0) return -1;
    let last = -1;
    for (let i = 0; i < chapters.length; i++) {
      if (chapters[i].index <= activeParagraphIndex) last = i;
      else break;
    }
    return last;
  });

  let unlisten: Array<() => void> = [];
  let scrollContainer: HTMLDivElement | undefined = $state();
  let docBar: HTMLElement | undefined = $state();

  // Build identifier so we can verify the running DMG matches the latest code.
  const BUILD_ID = "v0.1.0-doc-rev14-undo-cross";

  // Tail of the log file shown inline if user clicks "view log" while loading.
  let logViewerOpen = $state(false);
  let logTail = $state("");

  // Watchdog: if we're stuck on "loading" for 10s, flip to a clear error state.
  let watchdogTimer: ReturnType<typeof setTimeout> | undefined = undefined;
  let watchdogTripped = $state(false);

  // Find & replace bar state.
  let findOpen = $state(false);
  let findQuery = $state("");
  let replaceWith = $state("");
  let findCaseSensitive = $state(false);

  // Autosave state — debounce paragraph mutations.
  let autosaveTimer: ReturnType<typeof setTimeout> | undefined = undefined;
  let autosavedToast = $state(false);

  // Undo/redo. Snapshot-based: every mutation pushes the PRIOR paragraphs array
  // onto the history stack. Limit ~100 entries (~5MB worst case at ~50KB/paragraph
  // set, but real-world more like a few KB).
  let undoStack: { paragraphs: ParaState[]; rhythmMult: number }[] = $state([]);
  let redoStack: { paragraphs: ParaState[]; rhythmMult: number }[] = $state([]);
  const HISTORY_LIMIT = 100;

  /// Snapshot current state BEFORE a mutation. Clears redo stack (a new branch).
  /// Call this at the start of every mutating action.
  function commit() {
    undoStack = [
      ...undoStack,
      { paragraphs: paragraphs.map((p) => ({ ...p })), rhythmMult },
    ];
    if (undoStack.length > HISTORY_LIMIT) undoStack = undoStack.slice(-HISTORY_LIMIT);
    redoStack = [];
  }
  function undo() {
    const prev = undoStack[undoStack.length - 1];
    if (!prev) return;
    redoStack = [...redoStack, { paragraphs: paragraphs.map((p) => ({ ...p })), rhythmMult }];
    undoStack = undoStack.slice(0, -1);
    paragraphs = prev.paragraphs.map((p) => ({ ...p }));
    rhythmMult = prev.rhythmMult;
    scheduleAutosave();
    flashToast("undo");
  }
  function redo() {
    const next = redoStack[redoStack.length - 1];
    if (!next) return;
    undoStack = [...undoStack, { paragraphs: paragraphs.map((p) => ({ ...p })), rhythmMult }];
    redoStack = redoStack.slice(0, -1);
    paragraphs = next.paragraphs.map((p) => ({ ...p }));
    rhythmMult = next.rhythmMult;
    scheduleAutosave();
    flashToast("redo");
  }
  async function openLogViewer() {
    try {
      logTail = await tailLog(32);
      logViewerOpen = true;
    } catch (e) {
      console.warn("tailLog failed", e);
    }
  }
  async function revealLog() {
    try { await revealLogFile(); } catch (e) { console.warn(e); }
  }

  // ── lifecycle ──────────────────────────────────────────────────────────────
  onMount(async () => {
    // Every step is logged to BOTH the JS console (for DevTools) and the backend
    // tracing pipe (so yappy.log shows the full sequence even without DevTools).
    const log = (msg: string) => { console.log("[doc] " + msg); logToBackend("info", "doc/onMount", msg); };
    const warn = (msg: string) => { console.warn("[doc] " + msg); logToBackend("warn", "doc/onMount", msg); };

    log("mounting document window");

    // ─── STEP 1: pull whatever the backend has cached for us. ───────────────
    // This is the FIRST thing we do — no awaits before it — so a slow voice list
    // or a missing event listener can't keep us from rendering the document.
    try {
      log("calling getCurrentDocument");
      const initial = await getCurrentDocument();
      log("getCurrentDocument returned: " + (initial ? `${initial.filename} (${initial.paragraphs.length} paragraphs, loading=${initial.loading ?? false})` : "null"));
      if (initial) receiveDocument(initial);
    } catch (e) {
      warn("getCurrentDocument failed: " + String(e));
    }

    // ─── STEP 2: subscribe to document_loaded BEFORE the handshake. ─────────
    // If parsing is still in flight (LOADING was returned by step 1) we need
    // this listener live before we ask the backend to re-emit the full payload.
    try {
      log("subscribing to document_loaded");
      unlisten.push(await onDocumentLoaded((d) => {
        log(`document_loaded event: ${d.filename} (${d.paragraphs.length} paragraphs, loading=${d.loading ?? false})`);
        receiveDocument(d);
      }));
    } catch (e) {
      warn("onDocumentLoaded subscribe failed: " + String(e));
    }
    try {
      log("subscribing to document_error");
      unlisten.push(await onDocumentError(({ filename: fn, error }) => {
        log(`document_error event: ${fn}: ${error}`);
        errorMsg = `couldn't parse ${fn}: ${error}`;
        loading = false;
        doc = null;
      }));
    } catch (e) {
      warn("onDocumentError subscribe failed: " + String(e));
    }

    // ─── STEP 3: handshake. Backend re-emits whatever is in current_document. ─
    try {
      log("calling documentWindowReady");
      await documentWindowReady();
      log("documentWindowReady returned");
    } catch (e) {
      warn("documentWindowReady failed: " + String(e));
    }

    // ─── STEP 4: everything else (voices/settings/playback/render events) — ─
    // these are non-blocking for the document view; if they fail we still show
    // the document, just without voice picker etc.
    Promise.all([listVoices(), getSettings()])
      .then(([vs, s]) => { voices = vs; settings = s; log(`voices+settings loaded (voice=${s.voice})`); })
      .catch((e) => warn("voices/settings load failed: " + String(e)));

    try {
      // playback_state now drives karaoke. current_index reflects the chunk
      // actually being heard (computed from played_samples in the audio thread),
      // not the synth task's progress — which races far ahead of playback.
      unlisten.push(await onPlaybackState((s) => {
        const prevIdx = activeParagraphIndex;
        snap = { ...snap, ...s };
        // current_paragraph_index from the backend identifies the PARAGRAPH being
        // heard (across sentence-split chunks). Falls back to current_index for
        // very-old payloads in case rolling forward catches a stale snapshot.
        const para = (s as any).current_paragraph_index;
        const newIdx = baseParagraphIndex + (typeof para === "number" ? para : s.current_index);
        if (newIdx !== prevIdx) {
          activeParagraphIndex = newIdx;
          queueMicrotask(() => {
            const el = scrollContainer?.querySelector<HTMLElement>(
              `[data-paragraph-index="${activeParagraphIndex}"]`,
            );
            if (el) el.scrollIntoView({ behavior: "smooth", block: "center" });
          });
        }
      }));
      unlisten.push(await onPlaybackStarting((p) => {
        if (typeof p?.base_paragraph_index === "number") baseParagraphIndex = p.base_paragraph_index;
        activeParagraphIndex = baseParagraphIndex; // start at the first paragraph
        snap = { ...snap, playing: true, paused: false };
      }));
      // chunk_synthesized stays unsubscribed here — synth completion is unrelated
      // to which paragraph is currently HEARD.
      unlisten.push(await onSynthError((e) => {
        errorMsg = String(e);
        warn("synth error: " + String(e));
      }));
      unlisten.push(await onAudiobookRenderProgress((p) => { renderProgress = p; }));
      unlisten.push(await onAudiobookRenderDone((p) => {
        rendering = false;
        renderProgress = null;
        flashToast(`audiobook saved → ${p.path.split("/").pop() ?? p.path}`);
      }));
    } catch (e) {
      warn("playback/audiobook subscribe failed: " + String(e));
    }

    // Wire drag-drop on this window so dropped files load here directly.
    try {
      const w = getCurrentWindow();
      const u = await w.onDragDropEvent((ev) => {
        if (ev.payload.type === "over" || ev.payload.type === "enter") dragOver = true;
        else if (ev.payload.type === "leave") dragOver = false;
        else if (ev.payload.type === "drop") {
          dragOver = false;
          const paths = (ev.payload as any).paths as string[];
          console.log("[doc] dropped paths:", paths);
          if (paths?.length) {
            // Drop on a doc window REPLACES this window's content (swap behaviour).
            // Use our own label as the target so we don't spawn yet another window.
            const myLabel = getCurrentWindow().label;
            readFile(paths[0], myLabel).catch((e) => {
              errorMsg = `couldn't open: ${e}`;
              console.error("[doc] readFile failed", e);
            });
          }
        }
      });
      unlisten.push(u);
    } catch (e) {
      console.warn("[doc] drag-drop wire failed", e);
    }

    // Bulletproof window drag (v2): listen at the WINDOW level in capture phase
    // so we see the event before any inner element can swallow it. Only attempts
    // the drag when the click landed on a node within the doc-bar header and
    // wasn't an interactive control.
    const dragHandler = async (e: MouseEvent) => {
      if (e.button !== 0) return;
      const target = e.target as HTMLElement | null;
      if (!target) return;
      // Only drag when the click started inside our top bar (or, for non-doc
      // sub-areas, the empty parts of the reader header).
      const inBar = target.closest(".doc-bar");
      if (!inBar) return;
      // Skip if the click is on a real interactive element.
      if (target.closest("button, input, select, textarea, a, [role='button'], .vp-menu, .voice-dropdown")) return;
      try {
        const w = getCurrentWindow();
        console.log("[doc] startDragging()");
        await w.startDragging();
      } catch (err) {
        console.warn("[doc] startDragging failed", err);
      }
    };
    window.addEventListener("mousedown", dragHandler, { capture: true });
    unlisten.push(() => window.removeEventListener("mousedown", dragHandler, { capture: true }));

    // After 600ms, if we still have no doc, stop showing the loading spinner.
    setTimeout(() => { loading = false; }, 600);
  });

  onDestroy(() => unlisten.forEach((u) => u()));

  // ── Word count + estimated duration ────────────────────────────────────────
  function wordsIn(text: string): number {
    return text.trim().split(/\s+/).filter(Boolean).length;
  }
  /// Estimate reading duration in seconds at the given speed (default global).
  /// 180 wpm at speed=1.0 is a typical "audiobook" cadence; scaled inversely by speed.
  function estDurationSec(text: string, speed: number = settings?.speed ?? 1.05): number {
    const w = wordsIn(text);
    const wpm = 180 * speed;
    return (w / wpm) * 60;
  }
  function fmtMs(secs: number): string {
    if (!isFinite(secs) || secs <= 0) return "0:00";
    const m = Math.floor(secs / 60);
    const s = Math.floor(secs % 60);
    if (m >= 60) {
      const h = Math.floor(m / 60);
      return `${h}:${(m - h * 60).toString().padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
    }
    return `${m}:${s.toString().padStart(2, "0")}`;
  }
  // Derived totals — recompute when paragraphs or speed changes.
  let totalWords = $derived(paragraphs.reduce((sum, p) => sum + wordsIn(p.text), 0));
  let totalDurationSec = $derived(
    paragraphs.reduce((sum, p) => sum + estDurationSec(p.text, p.speed ?? settings?.speed ?? 1.05), 0),
  );

  // ── Find & replace ─────────────────────────────────────────────────────────
  function paragraphMatches(text: string, q: string): boolean {
    if (!q) return false;
    return findCaseSensitive ? text.includes(q) : text.toLowerCase().includes(q.toLowerCase());
  }
  function replaceInParagraph(text: string, q: string, r: string): string {
    if (!q) return text;
    if (findCaseSensitive) return text.split(q).join(r);
    // Case-insensitive replace.
    const re = new RegExp(q.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), "gi");
    return text.replace(re, r);
  }
  function replaceAll() {
    if (!findQuery) return;
    commit();
    const before = paragraphs.length;
    let changes = 0;
    paragraphs = paragraphs.map((p) => {
      const newText = replaceInParagraph(p.text, findQuery, replaceWith);
      if (newText !== p.text) changes++;
      return { ...p, text: newText };
    });
    flashToast(`replaced in ${changes}/${before} paragraphs`);
    scheduleAutosave();
  }

  // ── Project autosave ──────────────────────────────────────────────────────
  function projectSnapshot() {
    return {
      version: 2,
      doc_path: doc?.path,
      paragraphs: paragraphs.map((p) => ({
        text: p.text,
        voice: p.voice,
        speed: p.speed,
        pauseBefore: p.pauseBefore,
        kind: p.kind,
      })),
      rhythm_mult: rhythmMult,
      saved_at: new Date().toISOString(),
    };
  }
  function scheduleAutosave() {
    if (!doc?.path) return;
    if (autosaveTimer) clearTimeout(autosaveTimer);
    autosaveTimer = setTimeout(async () => {
      try {
        const snap = projectSnapshot();
        await saveProject(doc!.path, JSON.stringify(snap));
        autosavedToast = true;
        setTimeout(() => (autosavedToast = false), 1200);
      } catch (e) {
        logToBackend("warn", "doc/autosave", "save failed: " + String(e));
      }
    }, 600);
  }
  async function tryRestoreProject(path: string) {
    try {
      const json = await loadProject(path);
      if (!json) return false;
      const parsed = JSON.parse(json);
      if (!parsed?.paragraphs || !Array.isArray(parsed.paragraphs)) return false;
      paragraphs = parsed.paragraphs.map((p: any) => ({
        text: p.text ?? "",
        voice: p.voice ?? null,
        speed: p.speed ?? null,
        pauseBefore: p.pauseBefore ?? null,
        kind: p.kind ?? "paragraph",
      }));
      if (typeof parsed.rhythm_mult === "number") {
        rhythmMult = parsed.rhythm_mult;
      }
      logToBackend("info", "doc/autosave", `restored ${paragraphs.length} paragraphs from project file (rhythmMult=${rhythmMult})`);
      flashToast("restored your previous edits");
      return true;
    } catch (e) {
      logToBackend("warn", "doc/autosave", "restore failed: " + String(e));
      return false;
    }
  }

  // ── Watchdog ──────────────────────────────────────────────────────────────
  function armWatchdog() {
    if (watchdogTimer) clearTimeout(watchdogTimer);
    watchdogTripped = false;
    watchdogTimer = setTimeout(async () => {
      if (loading || (doc && doc.loading)) {
        watchdogTripped = true;
        try { logTail = await tailLog(32); } catch {}
        logToBackend("error", "doc/watchdog", "10s elapsed without document_loaded (FULL); flipping to error card");
      }
    }, 10_000);
  }
  function disarmWatchdog() {
    if (watchdogTimer) { clearTimeout(watchdogTimer); watchdogTimer = undefined; }
    watchdogTripped = false;
  }

  function receiveDocument(d: DocumentLoaded) {
    const prevLoading = loading;
    const prevDocFilename = doc?.filename ?? "null";
    doc = d;
    if (d.loading) {
      // Show the parsing state; keep paragraphs empty for now. We'll be re-emitted
      // when parsing finishes (or document_error fires).
      paragraphs = [];
      loading = true;
    } else {
      // Apply parsed rhythm metadata as defaults: headings get a pre-pause and a
      // slight slowdown, list items get a tiny pause, hrs get a big pause.
      // The user can still override any field via the per-paragraph settings panel.
      const pauses = d.paragraph_pauses ?? [];
      const speedMults = d.paragraph_speed_mult ?? [];
      const kinds = d.paragraph_kinds ?? [];
      const globalSpeed = settings?.speed ?? 1.05;
      paragraphs = d.paragraphs.map((t, i) => {
        const pauseBefore = pauses[i] && pauses[i] > 0 ? pauses[i] : null;
        const mult = speedMults[i] ?? 1.0;
        const speed = mult !== 1.0 ? Math.max(0.5, Math.min(2.0, globalSpeed * mult)) : null;
        return {
          text: t,
          voice: null,
          speed,
          pauseBefore,
          kind: kinds[i] ?? "paragraph",
        };
      });
      loading = false;
    }
    baseParagraphIndex = 0;
    activeParagraphIndex = -1;
    editingIndex = -1;
    settingsOpenIndex = -1;
    errorMsg = null;
    // Fresh document = fresh undo history. The user wouldn't expect Cmd-Z to
    // undo across different open files.
    undoStack = [];
    redoStack = [];
    logToBackend("info", "doc/receiveDocument",
      `applied: filename=${d.filename} paragraphs=${d.paragraphs.length} loading=${d.loading ?? false} ` +
      `(was: filename=${prevDocFilename} loading=${prevLoading})`);

    if (d.loading) {
      armWatchdog();
    } else {
      disarmWatchdog();
      // Try to restore saved overrides for this file. We do this AFTER setting
      // paragraphs from the freshly parsed text so if no project file exists we
      // still have a working editor; if a project does exist, it overrides.
      tryRestoreProject(d.path);
    }
    try { getCurrentWindow().setFocus(); } catch {}
  }

  // ── actions ────────────────────────────────────────────────────────────────
  async function pickFile() {
    try {
      const path = await dialogOpen({
        multiple: false,
        filters: [
          { name: "Documents", extensions: ["txt", "md", "markdown", "rtf", "docx", "doc", "odt", "pdf", "epub", "html", "htm"] },
        ],
      });
      // The doc window's pickFile = "swap content". Target our own label.
      const myLabel = getCurrentWindow().label;
      if (typeof path === "string") await readFile(path, myLabel);
    } catch (e) {
      errorMsg = `couldn't open: ${e}`;
    }
  }

  /// HTML-escape a string so we can safely use {@html} for highlight rendering.
  function escapeHtml(s: string): string {
    return s
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;")
      .replace(/'/g, "&#39;");
  }
  /// Loose substring search ignoring case + collapsing whitespace differences.
  function findChunkOffset(full: string, chunk: string): { start: number; end: number } | null {
    const c = chunk.trim();
    if (!c || c.length < 3) return null;
    // Exact first.
    let idx = full.indexOf(c);
    if (idx >= 0) return { start: idx, end: idx + c.length };
    // Case-insensitive.
    const lf = full.toLowerCase();
    const lc = c.toLowerCase();
    idx = lf.indexOf(lc);
    if (idx >= 0) return { start: idx, end: idx + c.length };
    // Loose: collapse runs of whitespace.
    const normalize = (s: string) => s.replace(/\s+/g, " ").trim();
    const nf = normalize(full).toLowerCase();
    const nc = normalize(c).toLowerCase();
    idx = nf.indexOf(nc);
    if (idx >= 0) {
      // Map back to original by walking char-by-char — close-enough offsets.
      return { start: idx, end: idx + nc.length };
    }
    return null;
  }
  /// Render a paragraph's text with the currently-playing chunk highlighted
  /// inline (and the preceding text dimmed as "already read").
  function renderParaWithKaraoke(full: string, chunk: string): string {
    const range = findChunkOffset(full, chunk);
    if (!range) return escapeHtml(full);
    const before = escapeHtml(full.slice(0, range.start));
    const active = escapeHtml(full.slice(range.start, range.end));
    const after = escapeHtml(full.slice(range.end));
    return `<span class="kar-read">${before}</span><mark class="kar-active">${active}</mark><span class="kar-upcoming">${after}</span>`;
  }

  /// Scroll the given paragraph into view inside the reader.
  function jumpToParagraph(idx: number) {
    queueMicrotask(() => {
      const el = scrollContainer?.querySelector<HTMLElement>(
        `[data-paragraph-index="${idx}"]`,
      );
      if (el) el.scrollIntoView({ behavior: "smooth", block: "start" });
    });
  }

  /// Multiplied speed for this playback — combines global speed × doc-window rhythm slider.
  function effectiveSpeedForPlay(): number {
    const base = settings?.speed ?? 1.05;
    return Math.max(0.3, Math.min(3.0, base * rhythmMult));
  }

  async function playAll() {
    if (!doc || paragraphs.length === 0) return;
    await readDocumentParagraphs(paragraphs.map((p) => p.text), 0, undefined, effectiveSpeedForPlay());
  }

  async function playFrom(index: number, voiceOverride?: string) {
    if (!doc) return;
    voicePickerForParagraph = -1;
    const v = voiceOverride ?? paragraphs[index]?.voice ?? undefined;
    await readDocumentParagraphs(paragraphs.map((p) => p.text), index, v, effectiveSpeedForPlay());
  }

  function startEdit(index: number) {
    editingIndex = index;
    editingText = paragraphs[index].text;
    voicePickerForParagraph = -1;
    settingsOpenIndex = -1;
    tick().then(() => {
      const ta = document.querySelector<HTMLTextAreaElement>(`textarea[data-edit-index="${index}"]`);
      if (ta) {
        ta.focus();
        ta.setSelectionRange(ta.value.length, ta.value.length);
      }
    });
  }

  function saveEdit() {
    if (editingIndex < 0) return;
    if (paragraphs[editingIndex].text === editingText.trim()) {
      // No-op edit; don't pollute the history.
      editingIndex = -1;
      editingText = "";
      return;
    }
    commit();
    paragraphs[editingIndex].text = editingText.trim();
    editingIndex = -1;
    editingText = "";
    flashToast("paragraph saved");
    scheduleAutosave();
  }

  function cancelEdit() {
    editingIndex = -1;
    editingText = "";
  }

  function setParaVoice(i: number, voice: string | null) {
    if (paragraphs[i].voice === voice) return;
    commit();
    paragraphs[i].voice = voice;
    paragraphs = paragraphs;
    scheduleAutosave();
  }
  function setParaSpeed(i: number, speed: number | null) {
    if (paragraphs[i].speed === speed) return;
    commit();
    paragraphs[i].speed = speed;
    paragraphs = paragraphs;
    scheduleAutosave();
  }
  function setParaPause(i: number, pause: number | null) {
    if (paragraphs[i].pauseBefore === pause) return;
    commit();
    paragraphs[i].pauseBefore = pause;
    paragraphs = paragraphs;
    scheduleAutosave();
  }

  function moveParagraph(i: number, delta: -1 | 1) {
    const j = i + delta;
    if (j < 0 || j >= paragraphs.length) return;
    commit();
    const tmp = paragraphs[i];
    paragraphs[i] = paragraphs[j];
    paragraphs[j] = tmp;
    paragraphs = paragraphs;
    scheduleAutosave();
  }
  /// Move a paragraph from one index to another (drag-and-drop reorder).
  function moveParagraphTo(fromIdx: number, toIdx: number) {
    if (fromIdx === toIdx || fromIdx < 0 || toIdx < 0
        || fromIdx >= paragraphs.length || toIdx >= paragraphs.length) return;
    commit();
    const [moved] = paragraphs.splice(fromIdx, 1);
    paragraphs.splice(toIdx, 0, moved);
    paragraphs = paragraphs;
    scheduleAutosave();
  }
  function deleteParagraph(i: number) {
    commit();
    paragraphs.splice(i, 1);
    paragraphs = paragraphs;
    settingsOpenIndex = -1;
    flashToast("paragraph removed");
    scheduleAutosave();
  }
  function insertParagraphAfter(i: number) {
    commit();
    paragraphs.splice(i + 1, 0, { text: "", voice: null, speed: null, pauseBefore: null, kind: "paragraph" });
    paragraphs = paragraphs;
    startEdit(i + 1);
    scheduleAutosave();
  }
  function appendParagraph() {
    commit();
    paragraphs.push({ text: "", voice: null, speed: null, pauseBefore: null, kind: "paragraph" });
    paragraphs = paragraphs;
    startEdit(paragraphs.length - 1);
    scheduleAutosave();
  }

  /// Resolve which paragraphs to include in a render based on chapter selection.
  /// Empty selection = entire document.
  function selectedParagraphIndices(): number[] {
    if (selectedChapterIdx.size === 0) {
      return Array.from({ length: paragraphs.length }, (_, i) => i);
    }
    const set = new Set<number>();
    for (const ci of selectedChapterIdx) {
      const { start, end } = chapterParagraphRange(ci);
      for (let i = start; i <= end; i++) set.add(i);
    }
    return Array.from(set).sort((a, b) => a - b);
  }

  async function playSelection() {
    if (!doc || paragraphs.length === 0) return;
    const indices = selectedParagraphIndices();
    if (indices.length === 0) return;
    // The backend joins consecutive paragraphs with "\n\n". If our selection has
    // gaps we still want them spoken as a continuous run — so we pluck out the
    // chosen paragraph TEXTS into a fresh list and read from paragraph 0.
    const subset = indices.map((i) => paragraphs[i].text);
    await readDocumentParagraphs(subset, 0, undefined, effectiveSpeedForPlay());
  }

  async function renderToWav() {
    if (rendering) return;
    if (!doc || paragraphs.length === 0) return;
    const base = doc.filename.replace(/\.[^.]+$/, "");
    // Default to .m4b — produces a real audiobook with embedded chapter
    // markers that Apple Books / Audiobookshelf / VLC navigate. The "WAV"
    // filter is still offered for users who want raw uncompressed audio.
    const defaultFilename = `${base} (audiobook).m4b`;
    try {
      const path = await dialogSave({
        defaultPath: defaultFilename,
        filters: [
          { name: "M4B audiobook (AAC + chapters)", extensions: ["m4b"] },
          { name: "WAV audio (uncompressed)", extensions: ["wav"] },
        ],
      });
      if (!path) return;
      // Honor chapter selection: empty set = whole document; otherwise only the
      // selected chapters' paragraphs are rendered (in document order).
      const sel = selectedParagraphIndices();
      const subset = sel.map((i) => paragraphs[i]);
      rendering = true;
      renderProgress = { index: 0, total: subset.length, stage: "synth" };
      const globalSpeed = settings?.speed ?? 1.05;
      const clampSpeed = (s: number) => Math.max(0.3, Math.min(3.0, s));
      // Track the LAST heading title we've already used to mark a chapter so
      // we don't re-emit a chapter on every paragraph under that heading.
      let lastChapterEmittedAt = -1;
      const specs: ParagraphSpec[] = subset.map((p, localIdx) => {
        const baseSpeed = p.speed ?? globalSpeed;
        const baseHasPause = (p.pauseBefore ?? 0) > 0;
        let chapterTitle: string | null = null;
        // Headings start a new chapter. The first paragraph of the export ALWAYS
        // gets a chapter (book opener), even if it's body text — players display
        // a single "Chapter 1" entry instead of "no chapters" for short files.
        if (/^heading[1-6]$/.test(p.kind)) {
          chapterTitle = p.text.trim() || `Chapter ${lastChapterEmittedAt + 2}`;
          lastChapterEmittedAt = localIdx;
        } else if (localIdx === 0 && lastChapterEmittedAt === -1) {
          chapterTitle = base;
          lastChapterEmittedAt = 0;
        }
        return {
          text: p.text,
          voice: p.voice,
          speed: clampSpeed(baseSpeed * rhythmMult),
          pause_before: baseHasPause ? (p.pauseBefore as number) / Math.max(0.1, rhythmMult) : null,
          chapter_title: chapterTitle,
        };
      });
      logToBackend("info", "doc/render",
        `renderToWav: path=${path} rhythmMult=${rhythmMult.toFixed(2)} globalSpeed=${globalSpeed.toFixed(2)} paragraphs=${specs.length} chapters=${specs.filter(s=>s.chapter_title).length}`);
      await renderAudiobook(specs, path, { title: base, album: base });
    } catch (e) {
      rendering = false;
      renderProgress = null;
      errorMsg = `render failed: ${e}`;
    }
  }

  function flashToast(msg: string) {
    savedToast = msg;
    setTimeout(() => (savedToast = null), 2200);
  }

  async function downloadCurrent() {
    if (snap.duration_secs < 0.1) {
      flashToast("nothing to save yet — play something first");
      return;
    }
    try {
      const base = doc?.filename.replace(/\.[^.]+$/, "") ?? "yappy";
      const path = await dialogSave({
        defaultPath: `${base}.wav`,
        filters: [{ name: "WAV audio", extensions: ["wav"] }],
      });
      if (path) {
        await saveCurrentAudio(path);
        flashToast("saved to " + (path.split("/").pop() ?? "wav"));
      }
    } catch (e) {
      errorMsg = `save failed: ${e}`;
    }
  }

  function fmtTime(s: number): string {
    if (!isFinite(s) || s < 0) s = 0;
    const m = Math.floor(s / 60);
    const sec = Math.floor(s % 60);
    return `${m}:${sec.toString().padStart(2, "0")}`;
  }

  function avatarLetter(name: string): string { return name[0] ?? "?"; }

  async function changeGlobalVoice(name: string) {
    if (!settings) return;
    settings = { ...settings, voice: name };
    await setGlobalVoice(name);
    flashToast(`voice → ${name}`);
  }

  function onKey(e: KeyboardEvent) {
    // Cmd/Ctrl+F → find. Always available.
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "f") {
      e.preventDefault();
      findOpen = true;
      queueMicrotask(() => {
        document.querySelector<HTMLInputElement>(".find-bar input.find-q")?.focus();
      });
      return;
    }
    // Undo/redo. Available outside editing mode. Inside editing mode they
    // belong to the textarea's native undo stack — let those pass through.
    if (editingIndex < 0) {
      if ((e.metaKey || e.ctrlKey) && !e.shiftKey && e.key.toLowerCase() === "z") {
        e.preventDefault();
        undo();
        return;
      }
      if (((e.metaKey || e.ctrlKey) && e.shiftKey && e.key.toLowerCase() === "z")
          || ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "y")) {
        e.preventDefault();
        redo();
        return;
      }
    }
    // Ignore other shortcuts when editing or typing in the find bar.
    const inFindBar = (e.target as HTMLElement)?.closest?.(".find-bar") != null;
    if (editingIndex >= 0 || inFindBar) {
      if (e.key === "Escape") {
        if (editingIndex >= 0) cancelEdit();
        else findOpen = false;
      }
      else if ((e.metaKey || e.ctrlKey) && e.key === "Enter") { e.preventDefault(); saveEdit(); }
      return;
    }
    if (e.key === " ") { e.preventDefault(); togglePause(); }
    else if (e.key === "Escape") stopPlayback();
    else if (e.key === "ArrowLeft") { e.preventDefault(); skip(-15); }
    else if (e.key === "ArrowRight") { e.preventDefault(); skip(15); }
  }
</script>

<svelte:window on:keydown={onKey} />

<div class="doc-window">
  <!-- ── TOP BAR ──────────────────────────────────────────────────────────── -->
  <!-- Drag handling: mousedown on the bar (anywhere except buttons) calls
       getCurrentWindow().startDragging(). data-tauri-drag-region is also set
       as a backup for environments where mousedown propagation is weird. -->
  <header class="doc-bar" data-tauri-drag-region bind:this={docBar}>
    <div class="doc-bar-left" data-tauri-drag-region>
      <YappyMascot size={36} talking={snap.playing && !snap.paused} />
      <div class="doc-meta" data-tauri-drag-region>
        <h1 class="doc-title" data-tauri-drag-region>{doc?.filename ?? "open a document"}</h1>
        {#if doc}
          <p class="doc-sub" data-tauri-drag-region>
            <span class="ext-pill">{doc.extension || "doc"}</span>
            · {paragraphs.length} paragraphs
            · {totalWords.toLocaleString()} words
            · ~{fmtMs(totalDurationSec)}
            {#if autosavedToast}
              <span class="autosave-flash">✓ saved</span>
            {/if}
          </p>
        {:else if loading}
          <p class="doc-sub" data-tauri-drag-region>parsing…</p>
        {:else}
          <p class="doc-sub" data-tauri-drag-region>no document loaded</p>
        {/if}
      </div>
    </div>
    <div class="doc-bar-right" data-tauri-drag-region>
      {#if doc}
        <!-- global voice for play-all -->
        <div class="voice-dropdown">
          <button class="btn voice-btn" onclick={() => (voicePickerForParagraph = voicePickerForParagraph === -2 ? -1 : -2)}>
            <span class="avatar" data-id={voices.find(v => v.name === settings?.voice)?.id || "F1"}>
              {avatarLetter(settings?.voice ?? "?")}
            </span>
            <span>{settings?.voice ?? "voice"}</span>
            <span class="caret">▾</span>
          </button>
          {#if voicePickerForParagraph === -2}
            <div class="vp-menu">
              {#each voices as v}
                <button class="vp-item" class:active={settings?.voice === v.name} onclick={() => { changeGlobalVoice(v.name); voicePickerForParagraph = -1; }}>
                  <span class="avatar small" data-id={v.id}>{avatarLetter(v.name)}</span>
                  <span class="vp-name">{v.name}</span>
                  <span class="vp-tags">{v.tags.slice(0, 2).join(" · ")}</span>
                </button>
              {/each}
            </div>
          {/if}
        </div>

        {#if snap.playing && !snap.paused}
          <button class="btn primary" onclick={() => togglePause()}>
            <svg width="13" height="13" viewBox="0 0 14 14" fill="currentColor"><rect x="2" y="1" width="3" height="12" rx="1"/><rect x="9" y="1" width="3" height="12" rx="1"/></svg>
            pause
          </button>
        {:else if snap.paused}
          <button class="btn primary" onclick={() => togglePause()}>
            <svg width="13" height="13" viewBox="0 0 14 14" fill="currentColor"><path d="M3 1.5C3 0.7 3.85 0.25 4.5 0.7L12.5 6.2c0.6 0.4 0.6 1.3 0 1.7l-8 5.5c-0.7 0.4-1.5 0-1.5-0.8V1.5Z"/></svg>
            resume
          </button>
        {:else}
          <button class="btn primary" onclick={playAll}>
            <svg width="13" height="13" viewBox="0 0 14 14" fill="currentColor"><path d="M3 1.5C3 0.7 3.85 0.25 4.5 0.7L12.5 6.2c0.6 0.4 0.6 1.3 0 1.7l-8 5.5c-0.7 0.4-1.5 0-1.5-0.8V1.5Z"/></svg>
            read all
          </button>
        {/if}

        <!-- Doc-window rhythm slider. Multiplies global speed × per-paragraph speed.
             Applied at PLAY and at RENDER. Persisted in the project file. -->
        <div class="rhythm-control" data-tauri-drag-region={false} title="rhythm multiplier — scales markdown rhythm + global speed for this doc">
          <span class="rhythm-label">rhythm</span>
          <input
            type="range" min="0.5" max="2.0" step="0.05"
            bind:value={rhythmMult}
            oninput={() => scheduleAutosave()}
          />
          <span class="rhythm-val">{rhythmMult.toFixed(2)}×</span>
        </div>
        <!-- Undo / redo -->
        <button class="btn" onclick={undo} disabled={undoStack.length === 0} title="undo (⌘Z)">↶</button>
        <button class="btn" onclick={redo} disabled={redoStack.length === 0} title="redo (⌘⇧Z)">↷</button>
        <button class="btn" onclick={() => skip(-15)} title="back 15s" aria-label="rewind 15 seconds">−15s</button>
        <button class="btn" onclick={() => skip(15)} title="forward 15s" aria-label="skip forward 15 seconds">+15s</button>
        <button class="btn" onclick={saveBookmark} title="bookmark this position" aria-label="bookmark current position">🔖</button>
        <button class="btn stop" onclick={stopPlayback} title="stop everything" aria-label="stop playback">stop</button>
        <button class="btn" onclick={downloadCurrent} title="save current playback as .wav">
          ↓ session.wav
        </button>
        <button class="btn audiobook" onclick={renderToWav} disabled={rendering} title="render the whole document as an .m4b audiobook with embedded chapters (or .wav)">
          {#if rendering}
            rendering {renderProgress?.index ?? 0}/{renderProgress?.total ?? 0}…
          {:else}
            🎧 render audiobook
          {/if}
        </button>
      {/if}
      <button class="btn" onclick={pickFile} title="open another file">
        {doc ? "swap" : "open…"}
      </button>
      {#if chapters.length > 0}
        <button
          class="btn sidebar-toggle"
          class:active={sidebarOpen}
          onclick={() => (sidebarOpen = !sidebarOpen)}
          title={`${chapters.length} chapter${chapters.length === 1 ? "" : "s"} — click to toggle nav`}
        >
          ☰ {chapters.length}
        </button>
      {/if}
    </div>
  </header>

  <!-- ── progress strip ──────────────────────────────────────────────────── -->
  {#if (snap.playing || snap.paused) && snap.duration_secs > 0.1}
    <div class="progress-strip">
      <div class="bar"><div class="fill" style="--w: {Math.min(100, (snap.elapsed_secs / snap.duration_secs) * 100)}%"></div></div>
      <div class="time">
        {fmtTime(snap.elapsed_secs)} / {fmtTime(snap.duration_secs)}
        <span class="time-remaining">· {fmtTime(Math.max(0, snap.duration_secs - snap.elapsed_secs))} left</span>
      </div>
      <SoundWaves active={snap.playing && !snap.paused} height={14} bars={9} />
      <!-- Sleep timer button (audiobook convention). Shows current countdown
           when active; tap opens a small chooser. -->
      <button class="sleep-btn" class:on={sleepUntil != null} onclick={toggleSleepMenu} title="sleep timer">
        {#if sleepUntil != null}
          🌙 {fmtTime(Math.max(0, (sleepUntil - Date.now()) / 1000))}
        {:else}
          🌙
        {/if}
      </button>
    </div>
  {/if}
  {#if sleepMenuOpen}
    <div class="sleep-menu">
      {#each [{ label: "5 min", secs: 300 }, { label: "15 min", secs: 900 }, { label: "30 min", secs: 1800 }, { label: "60 min", secs: 3600 }] as opt}
        <button onclick={() => setSleepIn(opt.secs)}>{opt.label}</button>
      {/each}
      {#if sleepUntil != null}
        <button class="cancel" onclick={() => setSleepIn(0)}>cancel</button>
      {/if}
    </div>
  {/if}

  <!-- ── audiobook rendering strip ───────────────────────────────────────── -->
  {#if rendering && renderProgress}
    <div class="render-strip">
      <span class="rs-label">🎧 rendering audiobook — {renderProgress.stage}</span>
      <div class="bar"><div class="fill" style="--w: {renderProgress.total > 0 ? (renderProgress.index / renderProgress.total) * 100 : 0}%"></div></div>
      <span class="rs-progress">{renderProgress.index}/{renderProgress.total}</span>
    </div>
  {/if}

  <!-- ── find & replace bar ──────────────────────────────────────────────── -->
  {#if findOpen}
    <div class="find-bar">
      <input class="find-q" bind:value={findQuery} placeholder="find…" />
      <input class="find-r" bind:value={replaceWith} placeholder="replace with…" />
      <label class="find-cs">
        <input type="checkbox" bind:checked={findCaseSensitive} /> Aa
      </label>
      <span class="find-count">
        {#if findQuery}
          {paragraphs.filter((p) => paragraphMatches(p.text, findQuery)).length}/{paragraphs.length} match
        {/if}
      </span>
      <button class="btn" onclick={replaceAll} disabled={!findQuery}>replace all</button>
      <button class="btn" onclick={() => { findOpen = false; findQuery = ""; replaceWith = ""; }}>close (esc)</button>
    </div>
  {/if}

  <!-- ── error banner ────────────────────────────────────────────────────── -->
  {#if errorMsg}
    <div class="err-banner">
      <span>⚠ {errorMsg}</span>
      <button class="btn-tiny" onclick={() => (errorMsg = null)}>×</button>
    </div>
  {/if}

  <!-- ── content row: sidebar + reader ──────────────────────────────────── -->
  <div class="content-row">
    {#if sidebarOpen && chapters.length > 0}
      <aside class="chapter-sidebar">
        <div class="cs-head">
          <span class="cs-title">chapters</span>
          <span class="cs-count">{chapters.length}</span>
        </div>
        {#if selectedChapterIdx.size > 0}
          <div class="cs-selection-bar">
            <span class="cs-sel-count">{selectedChapterIdx.size} selected</span>
            <div class="cs-sel-actions">
              <button class="btn-tiny" onclick={playSelection}>▶ play</button>
              <button class="btn-tiny" onclick={() => { renderToWav(); }}>🎧 render</button>
              <button class="btn-tiny ghost" onclick={clearSelection}>clear</button>
            </div>
          </div>
        {:else}
          <div class="cs-help">
            <span class="cs-hint">click a chapter to jump · tick boxes to render only selection</span>
            <button class="btn-tiny ghost" onclick={selectAllChapters} title="select every chapter">all</button>
          </div>
        {/if}
        <ul class="cs-list">
          {#each chapters as c, i (c.index)}
            <li
              class="cs-item"
              class:active={i === activeChapter}
              class:selected={selectedChapterIdx.has(i)}
              class:level-1={c.level === 1}
              class:level-2={c.level === 2}
              class:level-3={c.level === 3}
              class:level-4={c.level === 4}
              class:level-5={c.level === 5}
              class:level-6={c.level === 6}
            >
              <input
                type="checkbox"
                checked={selectedChapterIdx.has(i)}
                onchange={() => toggleChapterSelection(i)}
                onclick={(e) => e.stopPropagation()}
                aria-label="select chapter for render"
              />
              <button onclick={() => jumpToParagraph(c.index)} title={c.text}>
                <span class="cs-marker">{c.level === 1 ? "§" : c.level <= 3 ? "›" : "·"}</span>
                <span class="cs-text">{c.text}</span>
              </button>
            </li>
          {/each}
        </ul>
      </aside>
    {/if}
  <!-- ── reader area ─────────────────────────────────────────────────────── -->
  <div class="reader" bind:this={scrollContainer}>
    {#if loading || (doc && doc.loading)}
      <div class="loading-screen">
        <div class="loading-card" class:tripped={watchdogTripped}>
          <div class="loading-mascot">
            <YappyMascot size={120} talking={!watchdogTripped} />
          </div>
          {#if !watchdogTripped}
            <div class="loading-spinner-ring"></div>
          {/if}

          <h2 class="loading-title">
            {#if watchdogTripped}something's stuck{:else}parsing your file{/if}
          </h2>
          <p class="loading-filename">{doc?.filename ?? "…"}</p>

          <!-- Big, animated progress bar. Indeterminate (pdf_oxide doesn't expose
               per-page progress) but visually unmistakable. -->
          <div class="loading-progress">
            <div class="loading-progress-bar"></div>
          </div>

          <div class="loading-stages">
            {#if doc?.extension === "pdf"}
              <div class="loading-stage active">
                <span class="stage-dot"></span>
                reading PDF (pdf_oxide engine)
              </div>
              <div class="loading-stage">
                <span class="stage-dot"></span>
                if no text layer → rasterize + OCR (PaddleOCR, bundled)
              </div>
            {:else if doc?.extension === "epub"}
              <div class="loading-stage active"><span class="stage-dot"></span> unpacking EPUB</div>
              <div class="loading-stage"><span class="stage-dot"></span> joining chapters</div>
            {:else if doc?.extension === "docx" || doc?.extension === "doc" || doc?.extension === "odt"}
              <div class="loading-stage active"><span class="stage-dot"></span> extracting document text</div>
              <div class="loading-stage"><span class="stage-dot"></span> splitting into paragraphs</div>
            {:else}
              <div class="loading-stage active"><span class="stage-dot"></span> extracting text</div>
              <div class="loading-stage"><span class="stage-dot"></span> splitting into paragraphs</div>
            {/if}
          </div>

          <p class="loading-hint">
            {#if doc?.extension === "pdf"}
              pdfs with text take ~1 second per dozen pages.<br>
              scanned pdfs run page-by-page OCR — minutes for long ones.
            {:else}
              this normally takes a couple of seconds.
            {/if}
            <br><span class="loading-timeout">timeout is 3 minutes — yappy will tell you if something hangs.</span>
          </p>

          {#if watchdogTripped}
            <div class="watchdog-msg">
              <p><strong>10 seconds have passed without a parsed document.</strong></p>
              <p>The log tail below shows what the backend last did. If it shows "parse OK" but you're still seeing this screen, that's a frontend-listener bug — share the log with me.</p>
            </div>
          {/if}

          <div class="loading-debug">
            <button class="btn-tiny" onclick={openLogViewer}>view recent log</button>
            <button class="btn-tiny" onclick={revealLog}>reveal log file</button>
            {#if watchdogTripped}
              <button class="btn-tiny" onclick={pickFile}>pick another file</button>
            {:else}
              <button class="btn-tiny" onclick={stopPlayback}>cancel</button>
            {/if}
          </div>

          {#if logViewerOpen || watchdogTripped}
            <div class="log-viewer">
              <div class="log-viewer-head">
                <strong>recent log (last 32 kb)</strong>
                {#if !watchdogTripped}
                  <button class="btn-tiny" onclick={() => (logViewerOpen = false)}>close</button>
                {/if}
              </div>
              <pre>{logTail || "(empty)"}</pre>
            </div>
          {/if}
        </div>
      </div>
    {:else if doc}
      <article class="paragraphs">
        {#each paragraphs as p, i (i)}
          {#if (p.pauseBefore ?? 0) > 0 && i > 0}
            <div class="pause-marker" title={`${p.pauseBefore?.toFixed(1)}s silence before this paragraph`}>
              ⏸ {p.pauseBefore?.toFixed(1)}s pause
            </div>
          {/if}
          <section
            class="paragraph"
            class:active={activeParagraphIndex === i}
            class:editing={editingIndex === i}
            class:overridden={p.voice || p.speed}
            class:kind-h1={p.kind === "heading1"}
            class:kind-h2={p.kind === "heading2"}
            class:kind-h3={p.kind === "heading3"}
            class:kind-h4={p.kind === "heading4"}
            class:kind-h5={p.kind === "heading5"}
            class:kind-h6={p.kind === "heading6"}
            class:kind-list={p.kind === "list"}
            class:kind-quote={p.kind === "quote"}
            class:kind-hr={p.kind === "hr"}
            data-paragraph-index={i}
          >
            <div class="para-num" title={`${wordsIn(p.text)} words · ~${fmtMs(estDurationSec(p.text, p.speed ?? settings?.speed ?? 1.05))}`}>
              <span class="num">{i + 1}</span>
              <span class="words">{wordsIn(p.text)}w</span>
              <span class="dur">{fmtMs(estDurationSec(p.text, p.speed ?? settings?.speed ?? 1.05))}</span>
              {#if activeParagraphIndex === i}
                <SoundWaves active={snap.playing && !snap.paused} height={10} bars={4} />
              {/if}
              {#if p.voice || p.speed}
                <span class="override-dot" title="this paragraph has overrides">●</span>
              {/if}
            </div>

            {#if editingIndex === i}
              <div class="para-edit">
                <textarea
                  bind:value={editingText}
                  data-edit-index={i}
                  rows={Math.min(12, Math.max(3, editingText.split("\n").length + 1))}
                  placeholder="paragraph text…"
                ></textarea>
                <div class="edit-actions">
                  <button class="btn-tiny primary" onclick={saveEdit}>save (⌘↩)</button>
                  <button class="btn-tiny" onclick={cancelEdit}>cancel (esc)</button>
                </div>
              </div>
            {:else}
              <button
                type="button"
                class="para-text"
                onclick={() => playFrom(i)}
                title="read from this paragraph"
              >
                {#if activeParagraphIndex === i && snap.current_text && (snap.playing || snap.paused)}
                  <!-- Active paragraph: render with chunk-level karaoke. The currently
                       playing chunk is wrapped in <mark class="kar-active">, preceding
                       text is dimmed, upcoming text is normal. Updates every audio tick. -->
                  {@html renderParaWithKaraoke(p.text, snap.current_text)}
                {:else}
                  {p.text || "(empty paragraph)"}
                {/if}
              </button>
              <div class="para-actions">
                <button class="btn-tiny play-here" onclick={() => playFrom(i)} title="read from here">
                  ▶ play
                </button>
                <button class="btn-tiny" onclick={() => startEdit(i)} title="edit this paragraph">
                  ✎ edit
                </button>
                <button class="btn-tiny" onclick={() => (settingsOpenIndex = settingsOpenIndex === i ? -1 : i)} class:active={settingsOpenIndex === i} title="voice / speed / pause for this paragraph">
                  ⚙ {p.voice ? `voice: ${p.voice}` : ""}{p.voice && p.speed ? " · " : ""}{p.speed ? `${p.speed.toFixed(2)}×` : ""}{(!p.voice && !p.speed) ? "tweak" : ""}
                </button>
                <span class="reorder">
                  <button class="btn-tiny" onclick={() => moveParagraph(i, -1)} disabled={i === 0} title="move up">↑</button>
                  <button class="btn-tiny" onclick={() => moveParagraph(i, 1)} disabled={i === paragraphs.length - 1} title="move down">↓</button>
                </span>
                <button class="btn-tiny danger" onclick={() => deleteParagraph(i)} title="delete this paragraph">×</button>
              </div>

              {#if settingsOpenIndex === i}
                <!-- The per-paragraph override panel. Voice / speed / pause-before. -->
                <div class="para-settings">
                  <div class="ps-row">
                    <span class="ps-lbl">voice</span>
                    <div class="vd-wrap">
                      <button class="btn-tiny" onclick={() => (voicePickerForParagraph = voicePickerForParagraph === i ? -1 : i)}>
                        {#if p.voice}
                          <span class="avatar small" data-id={voices.find(v => v.name === p.voice)?.id || "F1"}>{avatarLetter(p.voice)}</span>
                          {p.voice}
                        {:else}
                          inherit ({settings?.voice ?? "—"})
                        {/if}
                      </button>
                      {#if voicePickerForParagraph === i}
                        <div class="vp-menu small">
                          <div class="vp-hint">voice for ¶{i + 1}</div>
                          <button class="vp-item" onclick={() => { setParaVoice(i, null); voicePickerForParagraph = -1; }}>
                            <span class="avatar small">↻</span>
                            <span class="vp-name">inherit global</span>
                          </button>
                          {#each voices as v}
                            <button class="vp-item" class:active={p.voice === v.name} onclick={() => { setParaVoice(i, v.name); voicePickerForParagraph = -1; }}>
                              <span class="avatar small" data-id={v.id}>{avatarLetter(v.name)}</span>
                              <span class="vp-name">{v.name}</span>
                              <span class="vp-tags">{v.tags.slice(0, 2).join(" · ")}</span>
                            </button>
                          {/each}
                        </div>
                      {/if}
                    </div>
                  </div>

                  <div class="ps-row">
                    <span class="ps-lbl">speed</span>
                    <input type="range" min="0.5" max="2" step="0.05"
                      value={p.speed ?? settings?.speed ?? 1.05}
                      oninput={(e) => setParaSpeed(i, parseFloat((e.target as HTMLInputElement).value))} />
                    <span class="ps-val">{(p.speed ?? settings?.speed ?? 1.05).toFixed(2)}×</span>
                    {#if p.speed !== null}
                      <button class="btn-tiny ghost" onclick={() => setParaSpeed(i, null)} title="reset to global">reset</button>
                    {/if}
                  </div>

                  <div class="ps-row">
                    <span class="ps-lbl">pause before</span>
                    <input type="range" min="0" max="5" step="0.1"
                      value={p.pauseBefore ?? 0}
                      oninput={(e) => setParaPause(i, parseFloat((e.target as HTMLInputElement).value))}
                      disabled={i === 0} />
                    <span class="ps-val">{(p.pauseBefore ?? 0).toFixed(1)}s</span>
                    {#if (p.pauseBefore ?? 0) > 0}
                      <button class="btn-tiny ghost" onclick={() => setParaPause(i, null)} title="remove pause">reset</button>
                    {/if}
                  </div>

                  <div class="ps-row insert-row">
                    <button class="btn-tiny" onclick={() => insertParagraphAfter(i)}>
                      + insert paragraph below
                    </button>
                  </div>
                </div>
              {/if}
            {/if}
          </section>
        {/each}

        <!-- Append-paragraph at the bottom of the document. -->
        <div class="append-row">
          <button class="btn-tiny" onclick={appendParagraph}>+ add paragraph at end</button>
        </div>
      </article>
    {:else}
      <!-- empty: no document, not loading -->
      <div class="empty">
        <YappyMascot size={120} />
        <h2>read a document</h2>
        <p>pick a file or drop one anywhere on this window.</p>
        <button class="btn primary big" onclick={pickFile}>
          choose a file…
        </button>
        <p class="hint">.pdf · .docx · .rtf · .md · .txt · .epub · .html</p>
      </div>
    {/if}
  </div>
  </div><!-- /.content-row -->

  <!-- Tiny build-version stamp so we can confirm which DMG is running. -->
  <div class="build-stamp" title="build identifier">{BUILD_ID}</div>

  {#if savedToast}<div class="saved-toast">{savedToast}</div>{/if}
  {#if dragOver}
    <div class="drop-overlay">
      <div class="drop-card">
        <div class="drop-icon">📥</div>
        <h2>drop to read</h2>
        <p>.pdf · .docx · .rtf · .md · .txt · .epub · .html</p>
      </div>
    </div>
  {/if}
</div>

<style>
  .doc-window {
    position: fixed; inset: 0;
    display: flex; flex-direction: column;
    background: var(--bg, #fff8d7);
    color: var(--ink-900);
    font-family: var(--font-sans);
  }

  /* ── TOP BAR ───────────────────────────────────────────────────────────── */
  .doc-bar {
    display: flex; align-items: center; justify-content: space-between;
    /* Top padding = 36px because Overlay-style title bar owns the top ~32px;
       drag-region mousedown only fires below that line. Left padding = 84px
       to clear the traffic-light buttons. */
    padding: 36px 20px 14px 84px;
    border-bottom: 2.5px solid var(--ink-900);
    background: var(--cream-100);
    -webkit-app-region: drag;
    flex-shrink: 0;
    gap: 16px;
  }
  .doc-bar-left { display: flex; align-items: center; gap: 12px; min-width: 0; -webkit-app-region: no-drag; }
  .doc-meta { min-width: 0; }
  .doc-title {
    font-family: var(--font-sans);
    font-size: 17px; font-weight: 700; margin: 0;
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
    max-width: 280px;
  }
  .doc-sub { font-size: 11px; color: var(--ink-500); font-weight: 600; margin: 2px 0 0; }
  .ext-pill {
    display: inline-block; padding: 1px 6px; border-radius: 5px;
    background: var(--pink-300); color: var(--ink-900);
    border: 1.5px solid var(--ink-900);
    font-size: 9px; font-weight: 800; text-transform: uppercase;
  }
  .doc-bar-right { display: flex; align-items: center; gap: 6px; -webkit-app-region: no-drag; flex-wrap: wrap; }

  /* ── BUTTONS ───────────────────────────────────────────────────────────── */
  .btn {
    display: inline-flex; align-items: center; gap: 5px;
    height: 30px; padding: 0 10px;
    background: var(--surface, #fff);
    border: 2px solid var(--ink-900);
    border-radius: 9px;
    box-shadow: 1.5px 1.5px 0 var(--ink-900);
    color: var(--ink-900); font-weight: 700; font-size: 11px;
    font-family: var(--font-sans); cursor: pointer; white-space: nowrap;
    transition: transform 0.12s var(--ease-emph), box-shadow 0.12s var(--ease-emph);
  }
  .btn:hover { transform: translate(-1px, -1px); box-shadow: 2.5px 2.5px 0 var(--ink-900); background: var(--cream-200); }
  .btn:active { transform: translate(1px, 1px); box-shadow: 0.5px 0.5px 0 var(--ink-900); }
  .btn:disabled { opacity: 0.4; cursor: not-allowed; transform: none; box-shadow: 1px 1px 0 var(--ink-900); }
  .btn.primary { background: var(--pink-500); }
  .btn.primary:hover { background: var(--pink-600, #ff5f95); }
  .btn.stop { background: var(--pink-300); }
  .btn.big { height: 40px; padding: 0 18px; font-size: 13px; }

  /* ── PROGRESS STRIP ────────────────────────────────────────────────────── */
  .progress-strip {
    display: flex; align-items: center; gap: 12px;
    padding: 8px 20px;
    background: var(--cream-200);
    border-bottom: 2px solid var(--ink-900);
  }
  .progress-strip .bar {
    flex: 1; height: 8px;
    border: 1.5px solid var(--ink-900);
    background: var(--cream-300);
    border-radius: 999px; overflow: hidden;
  }
  .progress-strip .fill { height: 100%; width: var(--w, 0%); background: var(--pink-500); transition: width 0.2s linear; }
  .progress-strip .time { font-family: var(--font-mono); font-size: 11px; color: var(--ink-700); font-weight: 700; min-width: 70px; display: flex; gap: 8px; align-items: baseline; }
  .progress-strip .time-remaining { color: var(--ink-500); font-weight: 500; }

  /* Sleep timer button — moon emoji, lights up when timer is active.
     Tap toggles the small chooser below the progress strip. */
  .sleep-btn { background: transparent; border: 1px solid var(--ink-200, #e5e5e5); border-radius: 999px; padding: 4px 10px; font-size: 12px; font-weight: 700; font-family: var(--font-mono); cursor: pointer; color: var(--ink-700); }
  .sleep-btn.on { background: var(--ink-900, #111); color: white; border-color: var(--ink-900, #111); }
  .sleep-menu { display: flex; gap: 8px; padding: 8px 16px; flex-wrap: wrap; background: var(--ink-50, #fff8d7); border-radius: 12px; margin: 8px 0; }
  .sleep-menu button { background: white; border: 1px solid var(--ink-200, #e5e5e5); border-radius: 999px; padding: 6px 14px; font-size: 13px; cursor: pointer; }
  .sleep-menu button:hover { border-color: var(--pink-500); color: var(--pink-500); }
  .sleep-menu button.cancel { color: var(--ink-500); margin-left: auto; }

  /* ── ERROR BANNER ──────────────────────────────────────────────────────── */
  .err-banner {
    display: flex; align-items: center; justify-content: space-between;
    padding: 8px 18px; background: var(--pink-300); color: var(--ink-900);
    border-bottom: 2px solid var(--ink-900); font-weight: 700; font-size: 12px;
  }
  .err-banner .btn-tiny { background: var(--ink-900); color: var(--cream-100); }

  /* Content row: sidebar + reader side by side. */
  .content-row {
    flex: 1; min-height: 0;
    display: flex;
    overflow: hidden;
  }

  /* Chapter sidebar */
  .chapter-sidebar {
    width: 240px;
    flex-shrink: 0;
    background: var(--cream-100);
    border-right: 2px solid var(--ink-300);
    overflow-y: auto;
    padding: 12px 0 32px;
  }
  .cs-head {
    display: flex; justify-content: space-between; align-items: baseline;
    padding: 4px 18px 10px;
    border-bottom: 1.5px dashed var(--ink-300);
    margin-bottom: 8px;
  }
  .cs-title { font-size: 11px; text-transform: uppercase; letter-spacing: 0.06em; color: var(--ink-500); font-weight: 800; }
  .cs-count { font-family: var(--font-mono); font-size: 11px; color: var(--ink-500); font-weight: 700; }
  .cs-list { list-style: none; padding: 0; margin: 0; }
  .cs-item button {
    width: 100%;
    display: flex; align-items: flex-start; gap: 8px;
    padding: 5px 14px;
    background: transparent;
    border: 0;
    border-left: 3px solid transparent;
    color: var(--ink-700);
    font-family: var(--font-sans);
    font-size: 12px; font-weight: 600;
    line-height: 1.35;
    text-align: left;
    cursor: pointer;
    transition: background 0.12s ease, color 0.12s ease, border-color 0.12s ease;
  }
  .cs-item button:hover { background: var(--cream-200); color: var(--ink-900); }
  .cs-item.active button {
    background: var(--pink-100, #ffe4ee);
    color: var(--ink-900);
    border-left-color: var(--pink-500);
    font-weight: 700;
  }
  .cs-item.level-1 button { padding-left: 4px; font-size: 13px; font-weight: 700; color: var(--ink-900); }
  .cs-item.level-2 button { padding-left: 14px; }
  .cs-item.level-3 button { padding-left: 26px; font-size: 11px; }
  .cs-item.level-4 button { padding-left: 38px; font-size: 11px; }
  .cs-item.level-5 button { padding-left: 50px; font-size: 10px; }
  .cs-item.level-6 button { padding-left: 62px; font-size: 10px; }
  .cs-item.selected button { background: var(--pink-100, #ffe4ee); }
  .cs-item input[type="checkbox"] {
    width: 13px; height: 13px;
    accent-color: var(--pink-500);
    margin: 9px 0 0 14px;
    flex-shrink: 0;
    cursor: pointer;
  }
  .cs-item { display: flex; align-items: flex-start; }
  .cs-item > button { flex: 1; }

  .cs-selection-bar {
    display: flex; flex-direction: column; gap: 6px;
    padding: 8px 14px;
    margin: 0 8px 8px;
    background: var(--pink-100, #ffe4ee);
    border: 2px solid var(--pink-500);
    border-radius: 10px;
  }
  .cs-sel-count {
    font-family: var(--font-sans); font-size: 11px; font-weight: 800;
    color: var(--ink-900); text-transform: uppercase; letter-spacing: 0.04em;
  }
  .cs-sel-actions { display: flex; gap: 4px; flex-wrap: wrap; }

  .cs-help {
    display: flex; align-items: center; justify-content: space-between; gap: 8px;
    padding: 4px 14px 8px;
  }
  .cs-hint {
    font-size: 10px; color: var(--ink-500); line-height: 1.35;
    font-style: italic; flex: 1;
  }
  .cs-marker {
    flex-shrink: 0;
    color: var(--pink-500);
    font-weight: 800;
    margin-top: 1px;
    min-width: 10px;
  }
  .cs-text {
    overflow: hidden; text-overflow: ellipsis;
    display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical;
  }

  .btn.sidebar-toggle.active { background: var(--cream-200); }

  /* ── READER ────────────────────────────────────────────────────────────── */
  .reader {
    flex: 1; min-height: 0;
    overflow-y: auto;
    scrollbar-gutter: stable;
    padding: 24px 0 80px;
  }
  .paragraphs {
    max-width: 760px; margin: 0 auto; padding: 0 32px;
    display: flex; flex-direction: column; gap: 14px;
  }

  .paragraph {
    display: grid;
    grid-template-columns: 32px 1fr;
    gap: 8px 14px;
    padding: 14px 14px 14px 18px;
    border: 2px solid transparent;
    border-radius: 14px;
    background: transparent;
    transition: background 0.15s ease, border-color 0.15s ease, transform 0.12s ease;
  }
  .paragraph:hover { background: var(--cream-100); border-color: var(--ink-300); }
  .paragraph.active {
    background: var(--pink-100, #ffe4ee);
    border-color: var(--pink-500);
    box-shadow: 2px 2px 0 var(--ink-900);
  }
  .paragraph.editing { background: var(--cream-100); border-color: var(--ink-900); }

  /* Markdown-aware visual styling. Headings stand out, list items indent, hr is a divider. */
  .paragraph.kind-h1 .para-text { font-size: 26px; font-weight: 800; line-height: 1.22; letter-spacing: -0.022em; }
  .paragraph.kind-h2 .para-text { font-size: 22px; font-weight: 700; line-height: 1.28; letter-spacing: -0.018em; }
  .paragraph.kind-h3 .para-text { font-size: 19px; font-weight: 700; line-height: 1.32; letter-spacing: -0.012em; }
  .paragraph.kind-h4 .para-text { font-size: 17px; font-weight: 700; line-height: 1.36; }
  .paragraph.kind-h5 .para-text { font-size: 16px; font-weight: 700; line-height: 1.4; }
  .paragraph.kind-h6 .para-text { font-size: 15px; font-weight: 700; line-height: 1.45; text-transform: uppercase; letter-spacing: 0.04em; color: var(--ink-700); }
  .paragraph.kind-h1, .paragraph.kind-h2, .paragraph.kind-h3,
  .paragraph.kind-h4, .paragraph.kind-h5, .paragraph.kind-h6 { margin-top: 12px; }
  .paragraph.kind-list { margin-left: 16px; }
  .paragraph.kind-list .para-text::before {
    content: "•";
    margin-right: 8px;
    color: var(--pink-500);
    font-weight: 800;
  }
  .paragraph.kind-quote { border-left: 4px solid var(--pink-300); padding-left: 14px; background: var(--cream-100); }
  .paragraph.kind-quote .para-text { font-style: italic; color: var(--ink-700); }
  .paragraph.kind-hr {
    border: none; border-radius: 0;
    padding: 8px 0;
    background: transparent !important;
    box-shadow: none !important;
    pointer-events: none;
  }
  .paragraph.kind-hr::after {
    content: ""; display: block;
    border-top: 3px dashed var(--ink-300);
    margin: 4px 32px;
  }
  .paragraph.kind-hr .para-num, .paragraph.kind-hr .para-text, .paragraph.kind-hr .para-actions { display: none; }

  .para-num {
    grid-column: 1; grid-row: 1 / span 2;
    display: flex; flex-direction: column; align-items: center; gap: 6px;
    padding-top: 2px;
  }
  .para-num .num {
    font-family: var(--font-mono);
    font-size: 11px; font-weight: 700; color: var(--ink-500);
  }
  .para-num .words, .para-num .dur {
    font-family: var(--font-mono); font-size: 9px;
    color: var(--ink-300); font-weight: 700;
    margin-top: 1px;
  }
  .paragraph.active .para-num .num,
  .paragraph.active .para-num .words,
  .paragraph.active .para-num .dur { color: var(--ink-900); }

  .para-text {
    grid-column: 2; grid-row: 1;
    background: transparent; border: 0; padding: 0; margin: 0;
    text-align: left; cursor: pointer;
    font-family: var(--font-sans);
    font-size: 17px; line-height: 1.6;
    color: var(--ink-900);
  }
  .para-text:hover { color: var(--pink-600, #d4007b); }

  /* Karaoke marks: chunk-level highlight inside the active paragraph. */
  .para-text :global(.kar-read)     { color: var(--ink-500); }
  .para-text :global(.kar-active)   {
    background: var(--pink-300);
    color: var(--ink-900);
    padding: 0 2px;
    border-radius: 4px;
    box-shadow: 0 0 0 1px var(--pink-500);
    /* Subtle pulse animation for the active chunk so it visually breathes. */
    animation: kar-pulse 1.8s ease-in-out infinite;
  }
  .para-text :global(.kar-upcoming) { color: var(--ink-900); }
  @keyframes kar-pulse {
    0%, 100% { box-shadow: 0 0 0 1px var(--pink-500); }
    50% { box-shadow: 0 0 0 3px rgba(255, 128, 171, 0.4); }
  }

  .para-actions {
    grid-column: 2; grid-row: 2;
    display: flex; gap: 6px; flex-wrap: wrap;
    margin-top: 6px;
  }

  .btn-tiny {
    display: inline-flex; align-items: center; gap: 4px;
    padding: 3px 9px;
    background: var(--surface, #fff);
    border: 1.5px solid var(--ink-900);
    border-radius: 7px;
    box-shadow: 1px 1px 0 var(--ink-900);
    color: var(--ink-700); font-size: 11px; font-weight: 700;
    font-family: var(--font-sans); cursor: pointer; line-height: 1;
    transition: transform 0.12s var(--ease-emph), background 0.15s ease;
  }
  .btn-tiny:hover { background: var(--cream-200); color: var(--ink-900); transform: translate(-1px, -1px); box-shadow: 2px 2px 0 var(--ink-900); }
  .btn-tiny.primary { background: var(--pink-500); color: var(--ink-900); }
  .btn-tiny.danger { color: var(--pink-600, #d4007b); border-color: var(--pink-500); }
  .btn-tiny.danger:hover { background: var(--pink-300); color: var(--ink-900); }
  .btn-tiny.ghost { background: transparent; border-color: var(--ink-300); box-shadow: none; padding: 2px 7px; font-size: 10px; }
  .btn-tiny.ghost:hover { box-shadow: 1px 1px 0 var(--ink-300); background: var(--cream-200); }
  .btn-tiny.play-here { background: var(--pink-300); }
  .btn-tiny:disabled { opacity: 0.4; cursor: not-allowed; }
  .btn-tiny.active { background: var(--pink-300); color: var(--ink-900); }
  .reorder { display: inline-flex; gap: 3px; }

  /* Per-paragraph settings panel (voice / speed / pause + insert) */
  .para-settings {
    grid-column: 2; grid-row: 3;
    margin-top: 10px;
    padding: 10px 12px;
    background: var(--cream-100);
    border: 2px solid var(--ink-300);
    border-radius: 10px;
    display: flex; flex-direction: column; gap: 8px;
  }
  .paragraph.overridden { background: linear-gradient(to right, var(--cream-100) 0%, var(--cream-100) 100%); border-color: var(--ink-300); }
  .paragraph.overridden:hover { background: var(--cream-200); }
  .override-dot { color: var(--pink-500); font-size: 12px; line-height: 1; margin-top: 2px; }
  .ps-row {
    display: flex; align-items: center; gap: 10px;
    font-size: 12px;
  }
  .ps-row.insert-row { margin-top: 2px; padding-top: 8px; border-top: 1px dashed var(--ink-300); }
  .ps-lbl { font-size: 11px; color: var(--ink-500); font-weight: 700; text-transform: uppercase; letter-spacing: 0.04em; min-width: 80px; }
  .ps-val { font-family: var(--font-mono); font-size: 11px; color: var(--ink-700); font-weight: 700; min-width: 42px; }
  .ps-row input[type="range"] {
    flex: 1; height: 6px;
    background: var(--cream-200);
    border: 1.5px solid var(--ink-900);
    border-radius: 999px; outline: none;
    -webkit-appearance: none; appearance: none;
  }
  .ps-row input[type="range"]:disabled { opacity: 0.4; }
  .ps-row input[type="range"]::-webkit-slider-thumb {
    -webkit-appearance: none; width: 14px; height: 14px; border-radius: 50%;
    background: var(--pink-500); border: 1.5px solid var(--ink-900); cursor: grab;
  }

  /* Pause-between-paragraphs visual marker */
  .pause-marker {
    align-self: center;
    padding: 3px 12px;
    background: var(--cream-200);
    border: 1.5px dashed var(--ink-300);
    border-radius: 999px;
    font-family: var(--font-mono); font-size: 10px; font-weight: 700; color: var(--ink-500);
  }

  /* "Add paragraph at end" footer row */
  .append-row {
    margin: 18px 0 0;
    text-align: center;
  }

  /* Rhythm slider in the doc-bar top right area */
  .rhythm-control {
    display: inline-flex; align-items: center; gap: 6px;
    padding: 3px 10px;
    background: var(--cream-200);
    border: 2px solid var(--ink-900);
    border-radius: 9px;
    box-shadow: 1.5px 1.5px 0 var(--ink-900);
    font-size: 11px; font-weight: 700;
    color: var(--ink-900);
  }
  .rhythm-control .rhythm-label { color: var(--ink-500); text-transform: lowercase; font-weight: 700; }
  .rhythm-control input[type="range"] {
    width: 90px; height: 4px;
    background: var(--cream-100);
    border: 1.5px solid var(--ink-900);
    border-radius: 999px; outline: none;
    -webkit-appearance: none; appearance: none;
  }
  .rhythm-control input[type="range"]::-webkit-slider-thumb {
    -webkit-appearance: none; width: 12px; height: 12px; border-radius: 50%;
    background: var(--pink-500); border: 1.5px solid var(--ink-900); cursor: grab;
  }
  .rhythm-control .rhythm-val {
    font-family: var(--font-mono); font-size: 10px;
    min-width: 32px; color: var(--ink-700);
  }

  /* Find & replace bar */
  .find-bar {
    display: flex; align-items: center; gap: 8px;
    padding: 8px 20px;
    background: var(--cream-200);
    border-bottom: 2px solid var(--ink-900);
  }
  .find-bar input {
    flex: 1; padding: 6px 10px; font-size: 12px;
    background: var(--surface, #fff);
    border: 2px solid var(--ink-900);
    border-radius: 8px;
    font-family: var(--font-sans);
    color: var(--ink-900); outline: none;
  }
  .find-bar input.find-q { max-width: 220px; }
  .find-bar input.find-r { max-width: 220px; }
  .find-bar .find-cs {
    display: inline-flex; align-items: center; gap: 4px;
    font-size: 11px; font-weight: 700; color: var(--ink-700);
    padding: 4px 8px;
    border: 1.5px solid var(--ink-900); border-radius: 7px;
    background: var(--cream-100);
  }
  .find-bar .find-count {
    font-family: var(--font-mono); font-size: 11px;
    color: var(--ink-500); font-weight: 700;
    min-width: 80px;
  }
  .autosave-flash {
    margin-left: 8px;
    color: #2ea35e; font-weight: 700;
    animation: autosave-fade 1.2s ease forwards;
  }
  @keyframes autosave-fade { 0% { opacity: 0; } 30%,70% { opacity: 1; } 100% { opacity: 0; } }

  /* Watchdog error state styling */
  .loading-card.tripped {
    border-color: var(--pink-500);
    box-shadow: 5px 5px 0 var(--pink-500);
  }
  .watchdog-msg {
    margin: 10px 0 14px;
    padding: 12px 16px;
    background: var(--pink-300);
    border: 2px solid var(--ink-900);
    border-radius: 10px;
    font-size: 12px; color: var(--ink-900); text-align: left;
  }
  .watchdog-msg p { margin: 4px 0; }

  /* Audiobook render progress strip */
  .render-strip {
    display: flex; align-items: center; gap: 12px;
    padding: 8px 20px;
    background: var(--pink-300);
    border-bottom: 2px solid var(--ink-900);
    font-size: 12px; font-weight: 700; color: var(--ink-900);
  }
  .render-strip .bar {
    flex: 1; height: 8px;
    border: 1.5px solid var(--ink-900);
    background: var(--cream-300);
    border-radius: 999px; overflow: hidden;
  }
  .render-strip .fill { height: 100%; width: var(--w, 0%); background: var(--pink-500); transition: width 0.3s ease; }
  .render-strip .rs-progress { font-family: var(--font-mono); }
  .btn.audiobook { background: var(--lavender-300, #e6dbff); }
  .btn.audiobook:hover { background: var(--lavender-500, #d6bfff); }

  /* ── EDIT MODE ─────────────────────────────────────────────────────────── */
  .para-edit { grid-column: 2; grid-row: 1 / span 2; display: flex; flex-direction: column; gap: 8px; }
  .para-edit textarea {
    width: 100%;
    background: var(--surface, #fff);
    border: 2px dashed var(--ink-900);
    border-radius: 10px;
    padding: 12px 14px;
    font-family: var(--font-sans);
    font-size: 16px; line-height: 1.55;
    color: var(--ink-900);
    resize: vertical;
    outline: none;
  }
  .para-edit textarea:focus { border-style: solid; box-shadow: 3px 3px 0 var(--pink-500); }
  .edit-actions { display: flex; gap: 6px; }

  /* ── VOICE DROPDOWNS ───────────────────────────────────────────────────── */
  .voice-dropdown, .vd-wrap { position: relative; }
  .voice-btn { padding: 0 8px 0 6px; gap: 6px; }
  .voice-btn .caret { font-size: 9px; opacity: 0.6; }
  .avatar {
    width: 20px; height: 20px; border-radius: 6px;
    display: flex; align-items: center; justify-content: center;
    font-family: var(--font-sans); font-weight: 700; font-size: 11px;
    background: var(--pink-300);
    border: 1.5px solid var(--ink-900);
    color: var(--ink-900);
  }
  .avatar.small { width: 22px; height: 22px; font-size: 11px; }
  .avatar[data-id^="M"] { background: var(--lavender-300, #d8c8ff); }
  .avatar[data-id^="F"] { background: var(--pink-300); }

  .vp-menu {
    position: absolute; right: 0; top: calc(100% + 4px);
    background: var(--cream-100);
    border: 2.5px solid var(--ink-900);
    border-radius: 14px;
    padding: 6px;
    box-shadow: 3px 3px 0 var(--ink-900);
    z-index: 50;
    min-width: 220px;
    max-height: 360px; overflow-y: auto;
    display: flex; flex-direction: column; gap: 2px;
  }
  .vp-menu.small { right: auto; left: 0; min-width: 200px; max-height: 300px; }
  .vp-hint { font-size: 10px; color: var(--ink-500); font-weight: 700; padding: 4px 8px 2px; text-transform: uppercase; letter-spacing: 0.05em; }
  .vp-item {
    display: flex; align-items: center; gap: 8px;
    padding: 6px 8px; border-radius: 9px;
    color: var(--ink-900); text-align: left; font-size: 12px;
    background: transparent; border: 0; cursor: pointer;
  }
  .vp-item:hover { background: var(--cream-200); }
  .vp-item.active { background: var(--pink-300); }
  .vp-name { font-weight: 700; flex: 1; }
  .vp-tags { color: var(--ink-500); font-size: 10px; font-weight: 600; }

  /* ── EMPTY / DROP / TOAST ──────────────────────────────────────────────── */
  .empty {
    display: flex; flex-direction: column; align-items: center; gap: 14px;
    padding: 80px 32px;
    color: var(--ink-500);
    text-align: center;
  }
  .empty h2 { font-family: var(--font-sans); font-size: 28px; margin: 0; color: var(--ink-900); }
  .empty p { margin: 0; }
  .empty .muted { color: var(--ink-500); font-size: 13px; font-weight: 600; }
  .empty .hint { font-size: 11px; color: var(--ink-500); font-family: var(--font-mono); margin-top: 4px; }

  /* ───────────────────────────────────────────────────────────────────
     LOADING SCREEN — extremely clear, big, communicative.
     ─────────────────────────────────────────────────────────────────── */
  .loading-screen {
    display: flex; align-items: center; justify-content: center;
    min-height: 480px;
    padding: 32px 20px;
  }
  .loading-card {
    width: 100%;
    max-width: 520px;
    background: var(--cream-100);
    border: 3px solid var(--ink-900);
    border-radius: 24px;
    box-shadow: 5px 5px 0 var(--ink-900);
    padding: 36px 36px 30px;
    text-align: center;
    position: relative;
  }
  .loading-mascot {
    display: flex; justify-content: center;
    margin-bottom: 4px;
    position: relative; z-index: 2;
  }
  .loading-spinner-ring {
    position: absolute;
    top: 22px; left: 50%;
    transform: translateX(-50%);
    width: 150px; height: 150px;
    border: 4px solid transparent;
    border-top-color: var(--pink-500);
    border-right-color: var(--pink-500);
    border-radius: 999px;
    animation: spin 1.2s linear infinite;
    z-index: 1;
  }
  @keyframes spin { to { transform: translate(-50%, 0) rotate(360deg); } }

  .loading-title {
    font-family: var(--font-sans);
    font-size: 26px; font-weight: 700;
    letter-spacing: -0.02em;
    margin: 16px 0 6px;
    color: var(--ink-900);
  }
  .loading-filename {
    font-family: var(--font-mono);
    font-size: 14px; font-weight: 700;
    color: var(--ink-700);
    margin: 0 0 22px;
    word-break: break-all;
    padding: 6px 14px;
    background: var(--cream-200);
    border: 1.5px solid var(--ink-900);
    border-radius: 999px;
    display: inline-block;
    max-width: 100%;
  }

  /* The progress bar — big, animated, unmistakable. */
  .loading-progress {
    width: 100%;
    height: 14px;
    background: var(--cream-200);
    border: 2px solid var(--ink-900);
    border-radius: 999px;
    overflow: hidden;
    box-shadow: inset 1px 1px 0 rgba(0, 0, 0, 0.08);
    margin: 4px 0 20px;
  }
  .loading-progress-bar {
    height: 100%; width: 35%;
    background: linear-gradient(90deg, var(--pink-500) 0%, var(--pink-300) 50%, var(--pink-500) 100%);
    background-size: 200% 100%;
    border-radius: 999px;
    animation: loading-march 1.4s cubic-bezier(0.4, 0, 0.6, 1) infinite,
               loading-shimmer 2s linear infinite;
  }
  @keyframes loading-march {
    0%   { transform: translateX(-100%); }
    100% { transform: translateX(250%); }
  }
  @keyframes loading-shimmer {
    0% { background-position: 200% 0; }
    100% { background-position: -200% 0; }
  }

  /* Stage list */
  .loading-stages {
    display: flex; flex-direction: column; gap: 8px;
    margin: 8px 0 18px;
    text-align: left;
  }
  .loading-stage {
    display: flex; align-items: center; gap: 10px;
    font-size: 13px;
    color: var(--ink-500); font-weight: 600;
    padding: 6px 12px;
    background: transparent;
    border-radius: 8px;
    transition: background 0.2s ease, color 0.2s ease;
  }
  .loading-stage.active {
    color: var(--ink-900);
    background: var(--cream-200);
  }
  .stage-dot {
    width: 8px; height: 8px;
    border-radius: 999px;
    background: var(--ink-300);
    flex-shrink: 0;
  }
  .loading-stage.active .stage-dot {
    background: var(--pink-500);
    animation: stage-pulse 1.2s ease-in-out infinite;
  }
  @keyframes stage-pulse {
    0%, 100% { transform: scale(1); box-shadow: 0 0 0 0 rgba(255, 128, 171, 0.5); }
    50% { transform: scale(1.4); box-shadow: 0 0 0 6px rgba(255, 128, 171, 0); }
  }

  .loading-hint {
    font-size: 12px;
    color: var(--ink-500);
    line-height: 1.5;
    margin: 0;
  }
  .loading-timeout {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--ink-500);
  }
  .loading-debug {
    display: flex; justify-content: center; gap: 6px;
    margin-top: 14px;
  }
  .log-viewer {
    margin-top: 14px;
    text-align: left;
    background: var(--ink-900);
    color: var(--cream-100);
    border: 2px solid var(--ink-900);
    border-radius: 10px;
    overflow: hidden;
  }
  .log-viewer-head {
    display: flex; justify-content: space-between; align-items: center;
    padding: 6px 10px;
    background: rgba(255,255,255,0.05);
    font-size: 12px;
  }
  .log-viewer pre {
    margin: 0;
    padding: 10px 12px;
    font-family: var(--font-mono);
    font-size: 11px;
    line-height: 1.4;
    color: var(--cream-100);
    max-height: 280px;
    overflow: auto;
    white-space: pre-wrap;
    word-break: break-all;
  }

  .drop-overlay {
    position: absolute; inset: 0;
    display: flex; align-items: center; justify-content: center;
    background: rgba(255, 248, 215, 0.85);
    backdrop-filter: blur(4px);
    z-index: 999;
    pointer-events: none;
  }
  .drop-card {
    padding: 28px 40px;
    background: var(--cream-100);
    border: 3px dashed var(--ink-900);
    border-radius: 18px;
    text-align: center;
    box-shadow: 4px 4px 0 var(--ink-900);
  }
  .drop-card .drop-icon { font-size: 42px; }
  .drop-card h2 { font-family: var(--font-sans); margin: 4px 0 2px; font-size: 24px; color: var(--ink-900); }
  .drop-card p { font-size: 12px; color: var(--ink-500); margin: 0; font-family: var(--font-mono); }

  /* Tiny build identifier — confirms which DMG you're running. */
  .build-stamp {
    position: absolute; bottom: 6px; right: 10px;
    font-family: var(--font-mono); font-size: 9px;
    color: var(--ink-500); opacity: 0.45;
    user-select: text;
    pointer-events: auto;
    z-index: 50;
  }
  .build-stamp:hover { opacity: 0.9; }

  .saved-toast {
    position: absolute; bottom: 22px; left: 50%; transform: translateX(-50%);
    padding: 8px 18px; border-radius: 999px;
    background: var(--pink-500); border: 2px solid var(--ink-900);
    font-size: 12px; font-weight: 700; color: var(--ink-900);
    box-shadow: 2px 2px 0 var(--ink-900);
    animation: toast 2.2s ease forwards;
    z-index: 1000;
  }
  @keyframes toast {
    0% { opacity: 0; transform: translate(-50%, 10px); }
    15%, 85% { opacity: 1; transform: translate(-50%, 0); }
    100% { opacity: 0; transform: translate(-50%, -8px); }
  }
</style>
