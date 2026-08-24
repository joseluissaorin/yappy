<script lang="ts">
  // LA CINTA: toda tu vida de escucha en un solo eje vertical. Lo que
  // suena (la aguja), la boca para añadir (pega aquí), lo que viene (los
  // segmentos, con su grosor proporcional a los minutos), las bobinas de
  // audiolibros y, al final, la trastienda. Sustituye de un golpe a la
  // cola, la biblioteca, el historial y las pestañas.
  import { onMount, onDestroy } from "svelte";
  import { goto } from "$app/navigation";
  import { t } from "$lib/i18n";
  import { get as getStore } from "svelte/store";
  import { haptic } from "$lib/haptic";
  import Criatura from "$lib/Criatura.svelte";
  import IconoTipo from "$lib/IconoTipo.svelte";
  import { reader } from "$lib/readerStore.svelte";
  import { progresoDe } from "$lib/progreso";
  import {
    colaListar,
    colaAgregarUrl,
    colaAgregarArchivo,
    colaEliminar,
    colaReintentar,
    onColaActualizada,
    onPlaybackState,
    playbackSnapshot,
    readDocument,
    readDocumentParagraphs,
    readClipboard,
    isModelReady,
    downloadModel,
    onModelDownload,
    type ItemCola,
    type PlaybackSnapshot,
    type DownloadProgress,
  } from "$lib/ipc";
  import { invoke } from "@tauri-apps/api/core";
  import { open as abrirDialogo } from "@tauri-apps/plugin-dialog";

  type Bobina = {
    name: string;
    path: string;
    duration_secs: number | null;
    chapter_count: number;
  };

  let items = $state<ItemCola[]>([]);
  let bobinas = $state<Bobina[]>([]);
  let playback = $state<PlaybackSnapshot | null>(null);
  let bocaAbierta = $state(false);
  let enlace = $state("");
  let modeloListo = $state(true);
  let descargando = $state<DownloadProgress | null>(null);
  let cleanups: (() => void)[] = [];

  const sonando = $derived(!!playback && (playback.playing || playback.paused));

  onMount(async () => {
    modeloListo = await isModelReady().catch(() => true);
    items = await colaListar().catch(() => []);
    bobinas = ((await invoke("list_rendered_audiobooks_cmd").catch(() => [])) as Bobina[]) ?? [];
    cleanups.push(await onColaActualizada(async () => (items = await colaListar().catch(() => items))));
    playback = await playbackSnapshot().catch(() => null);
    cleanups.push(await onPlaybackState((s) => (playback = s)));
    cleanups.push(
      await onModelDownload((p) => {
        descargando = p;
        if (p.stage === "done" && p.overall_done >= p.overall_total) {
          descargando = null;
          modeloListo = true;
        }
      }),
    );
  });
  onDestroy(() => cleanups.forEach((c) => c()));

  // ── Grosor: los minutos se ven ────────────────────────────────────────
  function minutosDe(item: ItemCola): number {
    return Math.max(1, Math.round((item.chars ?? 0) / 1000));
  }
  function altoDe(item: ItemCola): number {
    return Math.min(236, 74 + minutosDe(item) * 5);
  }
  function pctDe(item: ItemCola): number {
    if (!item.ruta) return 0;
    const p = progresoDe(item.ruta);
    if (!p || !p.total) return 0;
    return Math.min(100, Math.round(((p.parrafo + 1) / p.total) * 100));
  }

  // ── Abrir / reproducir ────────────────────────────────────────────────
  async function abrirItem(item: ItemCola) {
    if (item.estado === "error") {
      haptic("light");
      await colaReintentar(item.id).catch(() => {});
      return;
    }
    if (item.estado !== "listo" || !item.ruta) return;
    haptic("light");
    try {
      const doc = await readDocument(item.ruta);
      doc.filename = item.titulo;
      reader.doc = doc;
      await goto("/read");
      const desde = progresoDe(item.ruta)?.parrafo ?? 0;
      await readDocumentParagraphs(doc.paragraphs, Math.min(desde, Math.max(0, doc.paragraphs.length - 1)));
    } catch (e) {
      console.error("abrirItem:", e);
    }
  }

  async function abrirBobina(b: Bobina) {
    haptic("light");
    await invoke("library_play_cmd", { path: b.path, fromStart: false }).catch(() => {});
    goto("/biblioteca/audiolibros");
  }

  // ── La boca (pega aquí) ───────────────────────────────────────────────
  async function pegarEnlace() {
    const url = enlace.trim();
    if (!url) return;
    haptic("medium");
    enlace = "";
    bocaAbierta = false;
    await colaAgregarUrl(url.startsWith("http") ? url : `https://${url}`).catch((e) => console.error(e));
  }
  async function leerPortapapeles() {
    haptic("medium");
    bocaAbierta = false;
    await readClipboard().catch(() => {});
  }
  async function abrirArchivo() {
    bocaAbierta = false;
    const ruta = await abrirDialogo({
      multiple: false,
      filters: [
        {
          name: "Documentos",
          extensions: ["txt", "md", "markdown", "rtf", "docx", "doc", "odt", "pdf", "epub", "html", "htm"],
        },
      ],
    }).catch(() => null);
    if (typeof ruta === "string") await colaAgregarArchivo(ruta).catch(() => {});
  }

  // ── Enséñame: el loro lee su propio manual ────────────────────────────
  async function ensename() {
    haptic("medium");
    const tr = getStore(t);
    const parrafos = [
      tr("manual.titulo"),
      tr("manual.p1"),
      tr("manual.t2"),
      tr("manual.p2"),
      tr("manual.t3"),
      tr("manual.p3"),
      tr("manual.t4"),
      tr("manual.p4"),
      tr("manual.p5"),
    ];
    const kinds = ["heading1", "paragraph", "heading2", "paragraph", "heading2", "paragraph", "heading2", "paragraph", "paragraph"];
    reader.doc = {
      path: "",
      filename: tr("manual.titulo"),
      extension: "md",
      paragraphs: parrafos,
      char_count: parrafos.join(" ").length,
      loading: false,
      paragraph_pauses: kinds.map((k) => (k === "heading1" ? 1.2 : k === "heading2" ? 0.9 : 0)),
      paragraph_speed_mult: kinds.map(() => 1),
      paragraph_kinds: kinds,
    } as any;
    await goto("/read");
    await readDocumentParagraphs(parrafos, 0, undefined, undefined, {
      kinds,
      pausas: kinds.map((k) => (k === "heading1" ? 1.2 : k === "heading2" ? 0.9 : 0)),
      velocidades: kinds.map(() => 1),
    } as any);
  }

  // ── Arrastrar hacia fuera = quitar ────────────────────────────────────
  let arrastre = $state<{ id: string; dx: number } | null>(null);
  let arranque: { id: string; x: number; y: number; decidido: boolean } | null = null;

  function alTocar(e: PointerEvent, id: string) {
    arranque = { id, x: e.clientX, y: e.clientY, decidido: false };
  }
  function alMover(e: PointerEvent) {
    if (!arranque) return;
    const dx = e.clientX - arranque.x;
    const dy = e.clientY - arranque.y;
    if (!arranque.decidido) {
      if (Math.abs(dx) < 14 && Math.abs(dy) < 14) return;
      if (Math.abs(dy) > Math.abs(dx)) {
        arranque = null; // scroll vertical: no es un arrastre
        return;
      }
      arranque.decidido = true;
      (e.target as HTMLElement).setPointerCapture?.(e.pointerId);
    }
    arrastre = { id: arranque.id, dx };
  }
  async function alSoltar() {
    const a = arranque;
    const d = arrastre;
    arranque = null;
    if (!a || !d) {
      arrastre = null;
      return;
    }
    if (Math.abs(d.dx) > 110) {
      haptic("success");
      arrastre = { id: d.id, dx: d.dx > 0 ? 620 : -620 };
      setTimeout(async () => {
        await colaEliminar(d.id).catch(() => {});
        arrastre = null;
      }, 160);
    } else {
      arrastre = null;
    }
  }

  const vacia = $derived(items.length === 0 && bobinas.length === 0);
</script>

<main class="cinta" data-tauri-drag-region>
  <!-- El carrete de arranque: la marca como principio de la cinta. -->
  <header class="carrete">
    <Criatura size={44} andando={sonando} cantando={!!playback?.playing} />
    <span class="marca-palabra">yappy</span>
  </header>

  <div class="riel">
    {#if !modeloListo}
      <section class="segmento modelo yap-bloque">
        {#if descargando}
          <div class="modelo-barra"><div style="width: {Math.round((descargando.overall_done / Math.max(1, descargando.overall_total)) * 100)}%"></div></div>
          <p>{descargando.file} · {Math.round((descargando.overall_done / Math.max(1, descargando.overall_total)) * 100)}%</p>
        {:else}
          <h3>{$t("ajustes.sin_voces")}</h3>
          <button class="yap-tecla" onclick={() => downloadModel()}>{$t("ajustes.descargar")} (~380 MB)</button>
        {/if}
      </section>
    {/if}

    {#if sonando}
      <!-- LA AGUJA: lo que suena ahora, clavado en la cinta. -->
      <button class="aguja" onclick={() => goto("/read")}>
        <span class="aguja-punta" aria-hidden="true"></span>
        <span class="aguja-cuerpo">
          <span class="aguja-estado">{playback?.paused ? $t("cinta.en_pausa") : $t("cinta.sonando")}</span>
          <strong class="aguja-titulo">{(reader.doc?.filename ?? playback?.current_text ?? "").split("\n")[0].replace(/\.[^.]+$/, "").slice(0, 72)}</strong>
        </span>
        {#if playback?.playing}
          <span class="ondas" aria-hidden="true"><i></i><i></i><i></i><i></i></span>
        {/if}
      </button>
    {/if}

    <!-- LA BOCA: el principio de la cinta siempre está abierto. -->
    <section class="segmento boca" class:abierta={bocaAbierta}>
      {#if !bocaAbierta}
        <button class="boca-cerrada" onclick={() => { haptic("light"); bocaAbierta = true; }}>
          <span class="boca-cruz" aria-hidden="true">＋</span>
          <span class="boca-rotulo">{$t("cinta.pega_aqui")}</span>
        </button>
      {:else}
        <div class="boca-abierta">
          <div class="enlace-fila">
            <!-- svelte-ignore a11y_autofocus -->
            <input
              class="yap-campo"
              type="url"
              bind:value={enlace}
              placeholder="https://…"
              autofocus
              onkeydown={(e) => e.key === "Enter" && pegarEnlace()}
            />
            <button class="yap-tecla" onclick={pegarEnlace}>{$t("cinta.a_la_cola")}</button>
          </div>
          <button class="tecla-gorda" onclick={leerPortapapeles}>
            <IconoTipo tipo="portapapeles" size={22} /> {$t("cinta.portapapeles")}
          </button>
          <button class="tecla-gorda" onclick={abrirArchivo}>
            <IconoTipo tipo="documento" size={22} /> {$t("cinta.archivo")}
          </button>
          <button class="boca-cerrar" onclick={() => (bocaAbierta = false)} aria-label={$t("lector.hecho")}>▲</button>
        </div>
      {/if}
    </section>

    {#if vacia}
      <section class="vacia">
        <Criatura size={148} andando />
        <h1>{$t("cinta.vacia_titulo")}</h1>
        <p>{$t("cinta.vacia_texto")}</p>
        <button class="yap-tecla ensename" onclick={ensename}>{$t("cinta.ensename")}</button>
      </section>
    {/if}

    <!-- LOS SEGMENTOS: cada pieza, con su grosor en minutos. -->
    {#each items as item (item.id)}
      {@const pct = pctDe(item)}
      <article
        class="segmento pieza estado-{item.estado}"
        style="min-height: {altoDe(item)}px; transform: translateX({arrastre?.id === item.id ? arrastre.dx : 0}px); opacity: {arrastre?.id === item.id ? Math.max(0.25, 1 - Math.abs(arrastre.dx) / 340) : 1};"
        onpointerdown={(e) => alTocar(e, item.id)}
        onpointermove={alMover}
        onpointerup={alSoltar}
        onpointercancel={() => { arranque = null; arrastre = null; }}
      >
        <button class="pieza-cuerpo" onclick={() => abrirItem(item)}>
          <span class="pieza-tipo"><IconoTipo tipo={item.tipo} size={20} /></span>
          <span class="pieza-texto">
            <strong>{item.titulo}</strong>
            {#if item.estado === "listo"}
              <span class="pieza-meta">~{minutosDe(item)} {$t("cinta.min")}{#if pct > 0} · {pct}% {$t("cinta.escuchado")}{/if}</span>
            {:else if item.estado === "error"}
              <span class="pieza-meta error">{$t("cinta.error")}</span>
            {:else}
              <span class="pieza-meta">{$t("cinta.preparando")}…</span>
            {/if}
          </span>
        </button>
        {#if pct > 0 && item.estado === "listo"}
          <span class="pieza-progreso" style="height: {pct}%"></span>
        {/if}
      </article>
    {/each}

    <!-- LAS BOBINAS: los audiolibros, gordos como carretes. -->
    {#if bobinas.length > 0}
      <p class="rotulo-tramo">{$t("cinta.bobinas")}</p>
      {#each bobinas as b (b.path)}
        <article class="segmento bobina">
          <button class="pieza-cuerpo" onclick={() => abrirBobina(b)}>
            <span class="bobina-carrete" aria-hidden="true">
              <svg viewBox="0 0 44 44" width="40" height="40" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round"><circle cx="22" cy="22" r="17"/><circle cx="22" cy="22" r="5"/><path d="M22 5v6M22 33v6M5 22h6M33 22h6M10 10l4.4 4.4M29.6 29.6 34 34M34 10l-4.4 4.4M14.4 29.6 10 34"/></svg>
            </span>
            <span class="pieza-texto">
              <strong>{b.name.replace(/\.m4b$/, "")}</strong>
              <span class="pieza-meta">{b.duration_secs ? Math.round(b.duration_secs / 60) + " " + $t("cinta.min") + " · " : ""}{b.chapter_count} cap.</span>
            </span>
          </button>
        </article>
      {/each}
    {/if}

    <!-- LA TRASTIENDA: el final de la cinta. -->
    <button class="segmento trastienda" onclick={() => goto("/ajustes")}>
      <strong>{$t("cinta.trastienda")}</strong>
      <span>{$t("cinta.trastienda_pista")}</span>
    </button>
  </div>
</main>

<style>
  .cinta {
    min-height: 100dvh;
    padding: calc(env(safe-area-inset-top) + 10px) 16px calc(env(safe-area-inset-bottom) + 26px);
    display: flex;
    flex-direction: column;
  }
  .carrete {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 2px 14px;
  }
  .marca-palabra {
    font-weight: 800;
    font-size: 30px;
    letter-spacing: -0.02em;
    color: var(--yap-voz, #e0502a);
    transform: rotate(-2deg);
  }

  /* El riel: la cinta con sus perforaciones a la izquierda. */
  .riel {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding-left: 26px;
  }
  .riel::before {
    content: "";
    position: absolute;
    left: 4px;
    top: 0;
    bottom: 0;
    width: 12px;
    border-radius: 6px;
    background:
      radial-gradient(circle at 6px 10px, var(--yap-papel) 2.6px, transparent 3px) 0 0 / 12px 26px,
      var(--yap-ultramar, #2f4bc4);
    opacity: 0.85;
  }

  .segmento {
    position: relative;
    border-radius: 18px;
    touch-action: pan-y;
  }

  /* ── La aguja ── */
  .aguja {
    position: relative;
    display: flex;
    align-items: center;
    gap: 12px;
    border: 0;
    text-align: left;
    background: var(--yap-tecla-fondo, linear-gradient(180deg, #f4682e, #e0502a));
    color: #fff6ef;
    border-radius: 18px;
    padding: 16px 16px 16px 18px;
    box-shadow: var(--yap-relieve-alto, 0 10px 24px rgba(64, 46, 12, 0.2));
    cursor: pointer;
  }
  .aguja-punta {
    position: absolute;
    left: -26px;
    top: 50%;
    width: 30px;
    height: 6px;
    border-radius: 3px;
    background: #fff6ef;
    transform: translateY(-50%);
    box-shadow: 0 1px 3px rgba(64, 46, 12, 0.3);
  }
  .aguja-cuerpo {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    flex: 1;
  }
  .aguja-estado {
    font-family: var(--yap-mono, ui-monospace, monospace);
    font-size: 11px;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    opacity: 0.85;
  }
  .aguja-titulo {
    font-weight: 800;
    font-size: 17px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ondas {
    display: inline-flex;
    align-items: flex-end;
    gap: 3px;
    height: 26px;
  }
  .ondas i {
    width: 5px;
    border-radius: 3px;
    background: #fff6ef;
    animation: onda 0.9s ease-in-out infinite;
  }
  .ondas i:nth-child(1) { height: 40%; animation-delay: 0s; }
  .ondas i:nth-child(2) { height: 90%; animation-delay: 0.15s; }
  .ondas i:nth-child(3) { height: 60%; animation-delay: 0.3s; }
  .ondas i:nth-child(4) { height: 80%; animation-delay: 0.45s; }
  @keyframes onda {
    0%, 100% { transform: scaleY(0.5); }
    50% { transform: scaleY(1); }
  }

  /* ── La boca ── */
  .boca {
    border: 2.5px dashed color-mix(in srgb, var(--yap-tinta-suave, #82755a) 55%, transparent);
    background: transparent;
  }
  .boca-cerrada {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
    padding: 18px;
    border: 0;
    background: transparent;
    color: var(--yap-tinta-suave, #82755a);
    font-family: var(--yap-mono, ui-monospace, monospace);
    font-size: 13px;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    cursor: pointer;
  }
  .boca-cruz {
    font-size: 20px;
    font-weight: 700;
  }
  .boca-abierta {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px;
  }
  .enlace-fila {
    display: flex;
    gap: 8px;
  }
  .enlace-fila input {
    flex: 1;
    min-width: 0;
  }
  .tecla-gorda {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 16px;
    border: 1px solid var(--yap-borde);
    border-radius: 14px;
    background: var(--yap-superficie);
    color: var(--yap-tinta);
    font-weight: 700;
    font-size: 16px;
    box-shadow: var(--yap-relieve);
    cursor: pointer;
  }
  .tecla-gorda:active {
    transform: translateY(1px);
    box-shadow: var(--yap-hundido);
  }
  .boca-cerrar {
    position: absolute;
    top: 6px;
    right: 10px;
    border: 0;
    background: transparent;
    color: var(--yap-tinta-suave);
    font-size: 12px;
    padding: 6px;
    cursor: pointer;
  }

  /* ── El vacío que enseña ── */
  .vacia {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 12px;
    padding: 12vh 10px 8vh;
  }
  .vacia h1 {
    margin: 8px 0 0;
    font-size: 30px;
    font-weight: 800;
    letter-spacing: -0.02em;
    line-height: 1.1;
    transform: rotate(-1.2deg);
  }
  .vacia p {
    margin: 0;
    color: var(--yap-tinta-suave);
    max-width: 30ch;
  }
  .ensename {
    margin-top: 10px;
    font-size: 18px;
    padding: 16px 30px;
  }

  /* ── Los segmentos ── */
  .pieza,
  .bobina {
    background: var(--yap-superficie);
    border: 1px solid var(--yap-borde);
    box-shadow: var(--yap-relieve);
    overflow: hidden;
    transition: transform 0.16s ease, opacity 0.16s ease;
  }
  .pieza-cuerpo {
    display: flex;
    align-items: center;
    gap: 14px;
    width: 100%;
    height: 100%;
    min-height: inherit;
    padding: 16px;
    border: 0;
    background: transparent;
    color: var(--yap-tinta);
    text-align: left;
    cursor: pointer;
  }
  .pieza-tipo {
    color: var(--yap-ultramar, #2f4bc4);
    flex-shrink: 0;
  }
  .pieza-texto {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .pieza-texto strong {
    font-weight: 800;
    font-size: 17px;
    line-height: 1.2;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .pieza-meta {
    font-family: var(--yap-mono, ui-monospace, monospace);
    font-size: 11.5px;
    letter-spacing: 0.08em;
    color: var(--yap-tinta-suave);
  }
  .pieza-meta.error {
    color: var(--yap-voz, #e0502a);
  }
  .estado-pendiente,
  .estado-extrayendo,
  .estado-transcribiendo {
    background:
      repeating-linear-gradient(-45deg, transparent 0 12px, color-mix(in srgb, var(--yap-borde) 45%, transparent) 12px 22px),
      var(--yap-superficie);
    background-size: 200% 100%;
    animation: cocinando 2.4s linear infinite;
  }
  @keyframes cocinando {
    to { background-position: -62px 0, 0 0; }
  }
  .estado-error {
    border-color: var(--yap-voz, #e0502a);
  }
  .pieza-progreso {
    position: absolute;
    left: 0;
    bottom: 0;
    width: 5px;
    background: var(--yap-dorado, #e8b41a);
    border-radius: 0 3px 3px 0;
  }

  .rotulo-tramo {
    margin: 12px 0 0;
    font-family: var(--yap-mono, ui-monospace, monospace);
    font-size: 11px;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--yap-tinta-suave);
  }
  .bobina-carrete {
    color: var(--yap-ultramar, #2f4bc4);
    flex-shrink: 0;
  }

  /* ── La trastienda ── */
  .trastienda {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    margin-top: 18px;
    padding: 16px;
    border: 1px solid var(--yap-borde);
    background: var(--yap-superficie-2, var(--yap-superficie));
    color: var(--yap-tinta);
    box-shadow: var(--yap-hundido);
    cursor: pointer;
    text-align: left;
  }
  .trastienda strong {
    font-weight: 800;
    font-size: 16px;
  }
  .trastienda span {
    font-size: 12.5px;
    color: var(--yap-tinta-suave);
  }

  /* ── El modelo ── */
  .modelo {
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .modelo-barra {
    height: 10px;
    border-radius: 6px;
    background: var(--yap-superficie-2);
    overflow: hidden;
  }
  .modelo-barra div {
    height: 100%;
    background: var(--yap-dorado, #e8b41a);
    transition: width 0.3s ease;
  }
</style>
