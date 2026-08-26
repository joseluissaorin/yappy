<script lang="ts">
  // LA CINTA, tercera vida: la casa a medida. La página NO es un scroller:
  // cabecera fija (el loro, la marca y la trastienda A LA VISTA), la boca
  // de añadir, y la LISTA como único scroller (solo si de verdad desborda).
  // Fuera el riel decorativo: las tarjetas son las protagonistas. La aguja
  // vive en el layout (universal); aquí solo se pinta la verdad del espejo.
  import { onMount, onDestroy } from "svelte";
  import { flip } from "svelte/animate";
  import { backOut, cubicOut } from "svelte/easing";
  import { goto } from "$app/navigation";
  import { t } from "$lib/i18n";
  import { get as getStore } from "svelte/store";
  import { haptic } from "$lib/haptic";
  import { presionable } from "$lib/presionable";
  import Criatura from "$lib/Criatura.svelte";
  import IconoTipo from "$lib/IconoTipo.svelte";
  import { reader } from "$lib/readerStore.svelte";
  import { progresoDe } from "$lib/progreso";
  import { tintaVoz } from "$lib/voces";
  import { repro } from "$lib/reproduccion.svelte";
  import {
    colaListar,
    colaAgregarUrl,
    colaAgregarArchivo,
    colaAgregarPortapapeles,
    colaEliminar,
    colaReintentar,
    onColaActualizada,
    readDocument,
    readDocumentParagraphs,
    isModelReady,
    downloadModel,
    onModelDownload,
    type ItemCola,
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
  let bocaAbierta = $state(false);
  let enlace = $state("");
  let modeloListo = $state(true);
  let descargando = $state<DownloadProgress | null>(null);
  let cleanups: (() => void)[] = [];

  // El sistema nervioso: el nivel del espejo y la mirada del loro.
  const nivel = $derived(repro.nivel);
  const sonando = $derived(!!repro.snap && repro.snap.estado !== "inactivo");
  let mirada = $state({ x: 0, y: 0 });
  const durmiendo = $derived(
    typeof document !== "undefined" &&
      document.documentElement.dataset.theme === "dark" &&
      !sonando,
  );
  function seguirDedo(e: PointerEvent) {
    const w = window.innerWidth || 1;
    const h = window.innerHeight || 1;
    mirada = {
      x: Math.max(-1, Math.min(1, (e.clientX / w) * 2 - 1)),
      y: Math.max(-1, Math.min(1, (e.clientY / h) * 1.6 - 0.4)),
    };
  }

  let celebra = $state(false);
  let sacudida = $state<string | null>(null);
  let cascada = $state(true);
  let idsConocidos = new Set<string>();

  // Cada pieza LLEGA (no aparece): cae, se asienta con su rotación.
  function llega(_n: Element, o: { delay?: number } = {}) {
    return {
      delay: o.delay ?? 0,
      duration: 380,
      easing: backOut,
      css: (t: number) =>
        `transform: translateY(${-26 * (1 - t)}px) scale(${0.96 + 0.04 * t}) rotate(${(1 - t) * -1.6}deg); opacity: ${Math.min(1, t * 1.4)};`,
    };
  }
  function seVa(_n: Element) {
    return {
      duration: 200,
      easing: cubicOut,
      css: (t: number, u: number) => `transform: translateX(${u * 140}px) rotate(${u * 5}deg); opacity: ${t};`,
    };
  }
  function brincoDeLoro() {
    celebra = true;
    setTimeout(() => (celebra = false), 750);
  }

  onMount(async () => {
    modeloListo = await isModelReady().catch(() => true);
    items = await colaListar().catch(() => []);
    for (const it of items) idsConocidos.add(it.id);
    setTimeout(() => (cascada = false), 900);
    bobinas = ((await invoke("list_rendered_audiobooks_cmd").catch(() => [])) as Bobina[]) ?? [];
    cleanups.push(
      await onColaActualizada(async () => {
        const nuevos = await colaListar().catch(() => items);
        for (const it of nuevos) {
          if (!idsConocidos.has(it.id)) {
            idsConocidos.add(it.id);
            if (!cascada) {
              haptic("heavy");
              brincoDeLoro();
            }
          }
        }
        items = nuevos;
      }),
    );
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
  onDestroy(() => {
    cleanups.forEach((c) => c());
  });

  // ── Grosor: los minutos se ven ────────────────────────────────────────
  function minutosDe(item: ItemCola): number {
    return Math.max(1, Math.round((item.chars ?? 0) / 1000));
  }
  function altoDe(item: ItemCola): number {
    return Math.min(210, 78 + minutosDe(item) * 4);
  }
  function pctDe(item: ItemCola): number {
    if (!item.ruta) return 0;
    const p = progresoDe(item.ruta);
    if (!p || !p.total) return 0;
    return Math.min(100, Math.round(((p.parrafo + 1) / p.total) * 100));
  }

  // ── Abrir / reproducir (tocar una pieza ES el gesto de escuchar) ──────
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
      await readDocumentParagraphs(
        doc.paragraphs,
        Math.min(desde, Math.max(0, doc.paragraphs.length - 1)),
        undefined,
        undefined,
        undefined,
        { docPath: item.ruta, titulo: item.titulo },
      );
    } catch (e) {
      // El fichero ya no está (o no se pudo leer): que se NOTE. La pieza
      // se sacude, el loro se avergüenza y la háptica avisa.
      console.error("abrirItem:", e);
      haptic("error");
      sacudida = item.id;
      brincoDeLoro();
      setTimeout(() => (sacudida = null), 450);
    }
  }

  async function abrirBobina(b: Bobina) {
    haptic("light");
    await invoke("library_play_cmd", { path: b.path, fromStart: false }).catch(() => {});
    goto("/biblioteca/audiolibros");
  }

  // ── La boca: AÑADIR (pegar, un enlace, un archivo) ────────────────────
  let pegando = $state(false);
  async function pegarPortapapeles() {
    haptic("medium");
    pegando = true;
    try {
      await colaAgregarPortapapeles();
      bocaAbierta = false;
    } catch (e) {
      console.error("pegar:", e);
      haptic("error");
    } finally {
      pegando = false;
    }
  }
  async function pegarEnlace() {
    const url = enlace.trim();
    if (!url) return;
    haptic("medium");
    enlace = "";
    bocaAbierta = false;
    await colaAgregarUrl(url.startsWith("http") ? url : `https://${url}`).catch((e) => console.error(e));
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
    await readDocumentParagraphs(
      parrafos,
      0,
      undefined,
      undefined,
      {
        kinds,
        pausas: kinds.map((k) => (k === "heading1" ? 1.2 : k === "heading2" ? 0.9 : 0)),
        velocidades: kinds.map(() => 1),
      } as any,
      { titulo: tr("manual.titulo") },
    );
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

  // El icono de cada tipo de pieza, en el vocabulario de IconoTipo.
  function iconoDe(tipo: ItemCola["tipo"]): "articulo" | "video" | "audio" | "recorte" | "documento" {
    switch (tipo) {
      case "url": return "articulo";
      case "youtube": return "video";
      case "audio": return "audio";
      case "texto": return "recorte";
      default: return "documento";
    }
  }
</script>

<main class="cinta" data-tauri-drag-region onpointermove={seguirDedo} onpointerdown={seguirDedo}>
  <!-- EL PÓRTICO: la marca y la trastienda, siempre a la vista. -->
  <header class="portico">
    <div class="carrete">
      <Criatura
        size={46}
        mirando={-1}
        estado={celebra ? "celebrando" : repro.snap?.estado === "sonando" ? "hablando" : repro.snap?.estado === "pausa" ? "pausa" : durmiendo ? "dormido" : "posado"}
        apertura={repro.snap?.estado === "sonando" ? nivel : 0}
        {mirada}
        tinta={$tintaVoz}
      />
      <span class="marca-palabra">yappy</span>
    </div>
    <button class="portico-trastienda" use:presionable={{ hap: "soft" }} onclick={() => goto("/ajustes")}>
      <svg viewBox="0 0 24 24" width="21" height="21" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="12" cy="12" r="3.2"/><path d="M19.4 15a1.7 1.7 0 0 0 .34 1.87l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-1.87-.34 1.7 1.7 0 0 0-1.03 1.56V21a2 2 0 1 1-4 0v-.09a1.7 1.7 0 0 0-1.11-1.56 1.7 1.7 0 0 0-1.87.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.7 1.7 0 0 0 .34-1.87 1.7 1.7 0 0 0-1.56-1.03H3a2 2 0 1 1 0-4h.09a1.7 1.7 0 0 0 1.56-1.11 1.7 1.7 0 0 0-.34-1.87l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.7 1.7 0 0 0 1.87.34h.01a1.7 1.7 0 0 0 1.02-1.56V3a2 2 0 1 1 4 0v.09a1.7 1.7 0 0 0 1.03 1.56 1.7 1.7 0 0 0 1.87-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.7 1.7 0 0 0-.34 1.87v.01a1.7 1.7 0 0 0 1.56 1.02H21a2 2 0 1 1 0 4h-.09a1.7 1.7 0 0 0-1.56 1.03Z"/></svg>
      <span>{$t("cinta.trastienda")}</span>
    </button>
  </header>

  <!-- LA BOCA: añadir, con el pegar haciendo el trabajo él solo. -->
  <section class="boca" class:abierta={bocaAbierta}>
    {#if !bocaAbierta}
      <button class="boca-cerrada" use:presionable={{ hap: "soft" }} onclick={() => { haptic("light"); bocaAbierta = true; }}>
        <span class="boca-cruz" aria-hidden="true">＋</span>
        <span class="boca-rotulo">{$t("cinta.pega_aqui")}</span>
      </button>
    {:else}
      <div class="asomado" aria-hidden="true" in:llega><Criatura size={40} mirada={{ x: 0, y: 1 }} tinta={$tintaVoz} /></div>
      <div class="boca-abierta" in:llega>
        <button class="tecla-gorda protagonista" use:presionable={{ hap: "medium" }} onclick={pegarPortapapeles} disabled={pegando}>
          <IconoTipo tipo="portapapeles" size={22} /> {$t("cinta.portapapeles")}
        </button>
        <div class="enlace-fila">
          <!-- svelte-ignore a11y_autofocus -->
          <input
            class="yap-campo"
            type="url"
            bind:value={enlace}
            placeholder={$t("cinta.enlace_pista")}
            enterkeyhint="go"
            autocapitalize="off"
            autocorrect="off"
            spellcheck="false"
            onkeydown={(e) => e.key === "Enter" && pegarEnlace()}
          />
          <button class="yap-tecla" use:presionable onclick={pegarEnlace} disabled={!enlace.trim()}>{$t("cinta.a_la_cola")}</button>
        </div>
        <button class="tecla-gorda" use:presionable onclick={abrirArchivo}>
          <IconoTipo tipo="documento" size={22} /> {$t("cinta.archivo")}
        </button>
        <button class="boca-cerrar" onclick={() => (bocaAbierta = false)} aria-label={$t("comun.cerrar")}>▲</button>
      </div>
    {/if}
  </section>

  <!-- LA LISTA: el único scroller, y solo si de verdad desborda. -->
  <div class="lista">
    {#if !modeloListo}
      <section class="tarjeta modelo">
        {#if descargando}
          {@const pct = descargando.overall_done / Math.max(1, descargando.overall_total)}
          <div class="comiendo">
            <Criatura size={64} estado="comiendo" barriga={pct} cantando tinta={$tintaVoz} />
            <div class="comiendo-info">
              <div class="modelo-barra"><div style="width: {Math.round(pct * 100)}%"></div></div>
              <p>{descargando.file} · {Math.round(pct * 100)}%</p>
            </div>
          </div>
        {:else}
          <h3>{$t("escuchar.sin_voces")}</h3>
          <button class="yap-tecla" use:presionable onclick={() => downloadModel()}>{$t("ajustes.descargar")} (~380 MB)</button>
        {/if}
      </section>
    {/if}

    {#if vacia}
      <section class="vacia">
        <Criatura size={148} andando mirando={-1} {mirada} tinta={$tintaVoz} />
        <h1>{$t("cinta.vacia_titulo")}</h1>
        <p>{$t("cinta.vacia_texto")}</p>
        <button class="yap-tecla ensename" use:presionable={{ hap: "rigid" }} onclick={ensename}>{$t("cinta.ensename")}</button>
      </section>
    {/if}

    {#each items as item, i (item.id)}
      {@const pct = pctDe(item)}
      {@const esLaQueSuena = sonando && !!item.ruta && repro.snap?.doc_path === item.ruta}
      <article
        class="tarjeta pieza estado-{item.estado}" class:sacude={sacudida === item.id} class:suena={esLaQueSuena}
        animate:flip={{ duration: 300, easing: cubicOut }}
        in:llega={{ delay: cascada ? Math.min(i * 45, 360) : 0 }}
        out:seVa
        style="min-height: {altoDe(item)}px; transform: translateX({arrastre?.id === item.id ? arrastre.dx : 0}px) rotate({arrastre?.id === item.id ? arrastre.dx / 26 : 0}deg); opacity: {arrastre?.id === item.id ? Math.max(0.25, 1 - Math.abs(arrastre.dx) / 340) : 1};"
        onpointerdown={(e) => alTocar(e, item.id)}
        onpointermove={alMover}
        onpointerup={alSoltar}
        onpointercancel={() => { arranque = null; arrastre = null; }}
      >
        <button class="pieza-cuerpo" use:presionable onclick={() => abrirItem(item)}>
          {#if item.estado === "error"}
            <span class="pieza-tipo"><Criatura size={36} estado="avergonzado" /></span>
          {:else if esLaQueSuena}
            <span class="pieza-tipo"><Criatura size={36} mirando={-1} estado={repro.snap?.estado === "pausa" ? "pausa" : "hablando"} apertura={repro.snap?.estado === "sonando" ? nivel : 0} tinta={$tintaVoz} /></span>
          {:else}
            <span class="pieza-tipo"><IconoTipo tipo={iconoDe(item.tipo)} size={20} /></span>
          {/if}
          <span class="pieza-texto">
            <strong>{item.titulo}</strong>
            {#if esLaQueSuena}
              <span class="pieza-meta sonando-mini">
                <span class="mini-ondas" style="--nivel: {0.35 + nivel * 0.65}" aria-hidden="true"><i></i><i></i><i></i></span>
                {repro.snap?.estado === "pausa" ? $t("cinta.en_pausa") : $t("cinta.sonando")}
              </span>
            {:else if item.estado === "listo"}
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

    {#if bobinas.length > 0}
      <p class="rotulo-tramo">{$t("cinta.bobinas")}</p>
      {#each bobinas as b (b.path)}
        <article class="tarjeta bobina" in:llega>
          <button class="pieza-cuerpo" use:presionable onclick={() => abrirBobina(b)}>
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
  </div>
</main>

<style>
  /* La casa a medida: viewport exacto, CERO scroll de página. */
  .cinta {
    height: 100dvh;
    display: flex;
    flex-direction: column;
    padding: calc(env(safe-area-inset-top) + 8px) 0 0;
    overflow: hidden;
  }

  /* ── El pórtico ── */
  .portico {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 4px 18px 10px;
    flex-shrink: 0;
  }
  .carrete {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .marca-palabra {
    font-weight: 800;
    font-size: 30px;
    letter-spacing: -0.02em;
    color: var(--yap-voz, #e0502a);
    transform: rotate(-2deg);
  }
  .portico-trastienda {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 10px 16px;
    border-radius: 999px;
    border: 1.5px solid var(--yap-borde);
    background: var(--yap-superficie);
    color: var(--yap-tinta);
    font-weight: 700;
    font-size: 14px;
    box-shadow: var(--yap-relieve);
    cursor: pointer;
  }

  /* ── La boca ── */
  .boca {
    position: relative;
    margin: 0 18px 12px;
    border-radius: 18px;
    border: 2.5px dashed color-mix(in srgb, var(--yap-tinta-suave, #82755a) 55%, transparent);
    background: transparent;
    flex-shrink: 0;
  }
  .boca-cerrada {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
    padding: 16px;
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
  .asomado {
    position: absolute;
    top: -30px;
    right: 18px;
    pointer-events: none;
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
  .tecla-gorda.protagonista {
    border: 0;
    background: linear-gradient(180deg, var(--acento-voz-claro, #f4682e), var(--acento-voz, #e0502a));
    color: #fff6ef;
    font-size: 17px;
    padding: 18px 16px;
  }
  .tecla-gorda:disabled {
    opacity: 0.6;
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

  /* ── La lista: el único scroller ── */
  .lista {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overscroll-behavior: contain;
    -webkit-overflow-scrolling: touch;
    display: flex;
    flex-direction: column;
    gap: 13px;
    padding: 2px 18px calc(env(safe-area-inset-bottom) + var(--aguja-hueco, 0px) + 22px);
    scrollbar-width: none;
  }
  .lista::-webkit-scrollbar {
    display: none;
  }

  .tarjeta {
    position: relative;
    border-radius: 20px;
    touch-action: pan-y;
    flex-shrink: 0;
  }

  /* ── El vacío que enseña ── */
  .vacia {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    gap: 12px;
    flex: 1;
    padding: 4vh 10px;
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

  /* ── Las piezas ── */
  .pieza,
  .bobina {
    background: var(--yap-superficie);
    border: 1px solid var(--yap-borde);
    box-shadow: var(--yap-relieve);
    overflow: hidden;
  }
  .pieza.suena {
    border-color: color-mix(in srgb, var(--acento-voz, var(--yap-voz)) 55%, var(--yap-borde));
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--acento-voz, var(--yap-voz)) 16%, transparent), var(--yap-relieve);
  }
  .pieza-cuerpo {
    display: flex;
    align-items: center;
    gap: 14px;
    width: 100%;
    height: 100%;
    min-height: inherit;
    padding: 16px 18px;
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
  .sonando-mini {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    color: var(--acento-voz, var(--yap-voz));
    font-weight: 700;
  }
  .mini-ondas {
    display: inline-flex;
    align-items: flex-end;
    gap: 2px;
    height: 12px;
    transform: scaleY(var(--nivel, 0.6));
    transform-origin: 50% 100%;
    transition: transform 0.1s linear;
  }
  .mini-ondas i {
    width: 3px;
    border-radius: 2px;
    background: var(--acento-voz, var(--yap-voz));
    animation: onda-mini 0.9s ease-in-out infinite;
  }
  .mini-ondas i:nth-child(1) { height: 45%; }
  .mini-ondas i:nth-child(2) { height: 100%; animation-delay: 0.15s; }
  .mini-ondas i:nth-child(3) { height: 70%; animation-delay: 0.3s; }
  @keyframes onda-mini {
    0%, 100% { transform: scaleY(0.5); }
    50% { transform: scaleY(1); }
  }
  .estado-pendiente,
  .estado-preparando {
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
  .sacude {
    animation: sacudir 0.4s ease;
  }
  @keyframes sacudir {
    0%, 100% { transform: translateX(0); }
    25% { transform: translateX(-7px) rotate(-0.6deg); }
    55% { transform: translateX(7px) rotate(0.6deg); }
    80% { transform: translateX(-3px); }
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
    margin: 10px 2px 0;
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

  /* ── El modelo ── */
  .modelo {
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    background: var(--yap-superficie);
    border: 1px solid var(--yap-borde);
    box-shadow: var(--yap-relieve);
  }
  .comiendo {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .comiendo-info {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 8px;
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
