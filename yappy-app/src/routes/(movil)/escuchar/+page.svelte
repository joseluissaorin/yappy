<script lang="ts">
  // LA PERCHA, cuarta vida (docs/EL-JUGUETE.md): la pantalla deja de ser
  // tres estratos y pasa a ser un cuerpo. El tablero ES la percha: la boca
  // de añadir es una baldosa más (crece con FLIP hasta tragarse el tablero),
  // los titulares son carteles de madera de feria que llenan su pieza
  // deformándose, la que suena respira y muestra las palabras que se dicen,
  // y la bandada (el loro y sus pájaros) habita el tablero: vuela cuando
  // llega algo, se asoma en los ratos muertos y DICE los títulos en alto.
  import { onMount, onDestroy } from "svelte";
  import { backOut, cubicOut } from "svelte/easing";
  import { fly } from "svelte/transition";
  import { goto } from "$app/navigation";
  import { t, idiomaUI } from "$lib/i18n";
  import { get as getStore } from "svelte/store";
  import { haptic } from "$lib/haptic";
  import { presionable } from "$lib/presionable";
  import Criatura from "$lib/Criatura.svelte";
  import IconoTipo from "$lib/IconoTipo.svelte";
  import { reader } from "$lib/readerStore.svelte";
  import { progresoDe } from "$lib/progreso";
  import { tintaVoz } from "$lib/voces";
  import { repro } from "$lib/reproduccion.svelte";
  import { empaquetar, type Baldosa } from "$lib/mosaico";
  import { cartel, VB_ALTO, VB_BASE } from "$lib/cartel";
  import {
    SERENO,
    PALETA,
    colorDe,
    indiceDe,
    tonoHondo,
    formaDe,
    radiosDe,
    tiltDe,
  } from "$lib/juguete";
  import {
    logToBackend,
    colaListar,
    colaAgregarUrl,
    colaAgregarArchivo,
    colaAgregarPortapapeles,
    colaEliminar,
    colaFavorito,
    colaRenombrar,
    colaReordenar,
    colaReintentar,
    onColaActualizada,
    readDocument,
    readDocumentParagraphs,
    isModelReady,
    downloadModel,
    onModelDownload,
    decir,
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
  let marcaGelatina = $state(false);
  let voltereta = $state(false);
  let ultimoTapMarca = 0;
  let engranajeGira = $state(false);
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
  // Al salir: si la pieza fue LANZADA ya voló con su propio impulso y no
  // debe reaparecer; el resto se despide con su giro.
  let idLanzada: string | null = null;
  function seVa(n: Element) {
    const id = (n as HTMLElement).dataset?.pieza;
    if (id && id === idLanzada) return { duration: 0, css: () => "opacity: 0;" };
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
  function tocarMarca() {
    const ahora = performance.now();
    if (ahora - ultimoTapMarca < 360) {
      // Doble pulsación: la voltereta entera del loro.
      haptic("heavy");
      voltereta = true;
      setTimeout(() => (voltereta = false), 900);
    }
    ultimoTapMarca = ahora;
    haptic("success");
    celebra = true;
    marcaGelatina = true;
    setTimeout(() => {
      celebra = false;
      marcaGelatina = false;
    }, 950);
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
              brincoDeLoro();
              volar(it.id);
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
    // Las asomadas: en ratos muertos, un pájaro curiosea tras una pieza.
    const fisgon = setInterval(() => {
      if (sonando || asomadoEn || document.hidden) return;
      if (Math.random() < 0.55) return;
      const listos = items.filter((i) => i.estado === "listo");
      if (!listos.length) return;
      asomadoEn = listos[Math.floor(Math.random() * listos.length)].id;
      setTimeout(() => (asomadoEn = null), 2600);
    }, 32000);
    cleanups.push(() => clearInterval(fisgon));
  });
  onDestroy(() => {
    cleanups.forEach((c) => c());
    clearTimeout(timerBorrado);
  });

  // ── Grosor y progreso ─────────────────────────────────────────────────
  function minutosDe(item: ItemCola): number {
    return Math.max(1, Math.round((item.chars ?? 0) / 1000));
  }
  function pctDe(item: ItemCola): number {
    if (!item.ruta) return 0;
    const p = progresoDe(item.ruta);
    if (!p || !p.total) return 0;
    return Math.min(100, Math.round(((p.parrafo + 1) / p.total) * 100));
  }

  // ── Abrir / reproducir (tocar una pieza ES el gesto de escuchar) ──────
  async function abrirItem(item: ItemCola) {
    logToBackend("info", "cinta", `abrirItem ${item.id} estado=${item.estado}`);
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
      const desde = progresoDe(item.ruta)?.parrafo ?? 0;
      if (desde > 0) {
        // Hay progreso guardado: PRIMERO eliges (seguir donde ibas o desde
        // el principio, en la portada) y DESPUÉS se cocina la voz.
        await goto("/read");
        return;
      }
      // Pieza fresca: la voz se pone a cocinar ANTES de navegar, así el
      // cartel nace ya en «preparando» (sin fotogramas de portada).
      await readDocumentParagraphs(doc.paragraphs, 0, undefined, undefined, undefined, {
        docPath: item.ruta,
        titulo: item.titulo,
      });
      await goto("/read");
    } catch (e) {
      // El fichero ya no está (o no se pudo leer): que se NOTE.
      console.error("abrirItem:", e);
      logToBackend("error", "cinta", `abrirItem falló: ${e}`);
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

  // ── La boca-baldosa: AÑADIR vive DENTRO del mosaico ───────────────────
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
    // SIN filtros: el selector de iOS no casaba las extensiones y dejaba
    // TODO gris. El backend ya sabe decir «no puedo con esto».
    const ruta = await abrirDialogo({ multiple: false }).catch(() => null);
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

  // ── EL MOSAICO VIVO: pesos renegociados, losa continua, FLIP ──────────
  let arrastre = $state<{ id: string; dx: number } | null>(null);
  let menuPieza = $state<ItemCola | null>(null);
  let renombrando = $state(false);
  let nuevoNombre = $state("");
  let arranque: { id: string; x: number; y: number; decidido: "no" | "swipe" } | null = null;
  let holdTimer: ReturnType<typeof setTimeout> | undefined;
  let anchoTablero = $state(0);
  let levantada = $state<string | null>(null);
  let recogida = $state(false);
  let vuelo = $state({ dx: 0, dy: 0, estira: 1, giro: 0, vx: 0, vy: 0 });
  let seMovioEnVuelo = false;
  let vueloPrevio = { x: 0, y: 0, t: 0 };
  let asomadoEn = $state<string | null>(null);

  const BOCA_ID = "@boca";
  const MARCA_ID = "@marca";
  const TRAS_ID = "@trastienda";

  // Piezas ocultas: lanzadas o descartadas, a la espera del «deshacer».
  let ocultas = $state<Set<string>>(new Set());
  const visibles = $derived(items.filter((i) => !ocultas.has(i.id)));

  // La pieza que SUENA (por ruta del snapshot). Su color es la sangre.
  const idQueSuena = $derived(
    sonando ? (visibles.find((i) => !!i.ruta && i.ruta === repro.snap?.doc_path)?.id ?? null) : null,
  );

  // Los colores del tablero: por NOMBRE de fichero (estable entre updates
  // y compartido con --vivo), pero con AJUSTE de choques: dos vecinas no
  // repiten color. La que suena conserva SIEMPRE su color base (la sangre
  // del layout debe coincidir); las demás se apartan de ella.
  const coloresVivos = $derived.by(() => {
    const m = new Map<string, string>();
    const usados: number[] = [];
    for (const it of visibles) {
      const base = indiceDe(it.ruta ? (it.ruta.split("/").pop() ?? it.ruta) : it.id);
      let idx = base;
      if (it.id !== idQueSuena) {
        // Ni la vecina de la izquierda ni la de ENCIMA (filas de ~3):
        // el collage no repite color pegado a color.
        const choca = () =>
          usados[usados.length - 1] === idx || usados[usados.length - 3] === idx;
        for (let intentos = 0; choca() && intentos < 4; intentos++) {
          idx = (idx + 3) % PALETA.length;
        }
      }
      usados.push(idx);
      m.set(it.id, PALETA[idx]);
    }
    return m;
  });
  function tintaDe(item: ItemCola): string {
    return coloresVivos.get(item.id) ?? colorDe(item.id);
  }

  // Los pesos RENEGOCIADOS (Kalorica exagerado): con n grandes a la vez,
  // cada grande pesa max(3, 7−n): elegir una pieza la hace crecer ×6.
  let seleccionada = $state<string | null>(null);
  function pesoDe(item: ItemCola): number {
    const grandes =
      (seleccionada ? 1 : 0) + (idQueSuena && idQueSuena !== seleccionada ? 1 : 0);
    const pesoGrande = Math.max(3, 7 - grandes);
    if (item.id === seleccionada) return pesoGrande;
    if (item.id === idQueSuena) return pesoGrande;
    if (item.favorito) return 2.0;
    if (item.estado === "error") return 1.1;
    // Los minutos SE VEN: una pieza gorda abulta hasta el doble.
    return 1 + Math.min(0.9, minutosDe(item) / 22);
  }

  function alternarSeleccion(item: ItemCola) {
    haptic("medium");
    onda(item.id);
    const eligiendo = seleccionada !== item.id;
    seleccionada = eligiendo ? item.id : null;
    // La voz dice el título en voz baja (solo con la casa en silencio).
    if (eligiendo && !sonando && item.estado === "listo") {
      decir(item.titulo).catch(() => {});
    }
  }

  // El tablero ES la casa entera: no hay cabecera aparte. La primera fila
  // son las piezas del SISTEMA (crema, para distinguirse de las vivas):
  // la MARCA (el loro posado encima y el «yappy» de recortes de colores),
  // la TRASTIENDA (el engranaje) y la BOCA de añadir (que abierta pesa 26
  // y el FLIP la hace crecer hasta tragarse el tablero).
  const piezasTablero = $derived([
    { id: MARCA_ID, peso: 2.1, letras: 5 },
    { id: TRAS_ID, peso: 0.9, letras: 3 },
    {
      id: BOCA_ID,
      peso: bocaAbierta ? 26 : visibles.length === 0 ? 2.4 : 0.9,
      letras: 2,
    },
    ...visibles.map((i) => ({ id: i.id, peso: pesoDe(i), letras: Math.min(30, i.titulo.length) })),
  ]);
  const tablero = $derived(empaquetar(piezasTablero, anchoTablero));

  // FLIP con muelle DESDE EL RECT VIVO: la foto se toma en $effect.pre
  // (ANTES de que Svelte mueva el DOM), así que recoge dónde está cada
  // pieza VISUALMENTE en ese instante, aunque venga volando de un muelle
  // interrumpido: nada de teletransportes al reordenar en vivo. Después,
  // el $effect normal (DOM ya en su sitio nuevo) planta la inversa y
  // suelta el muelle. Es el alma del juguete.
  let fotoPrevia = new Map<string, { x: number; y: number; w: number; h: number }>();
  $effect.pre(() => {
    void tablero.baldosas;
    if (typeof document === "undefined") return;
    const mosaicoEl = document.querySelector<HTMLElement>(".mosaico");
    if (!mosaicoEl) return;
    const marco = mosaicoEl.getBoundingClientRect();
    const m = new Map<string, { x: number; y: number; w: number; h: number }>();
    mosaicoEl.querySelectorAll<HTMLElement>("[data-pieza]").forEach((el) => {
      const id = el.dataset.pieza;
      if (!id) return;
      const r = el.getBoundingClientRect();
      m.set(id, { x: r.left - marco.left, y: r.top - marco.top, w: r.width, h: r.height });
    });
    fotoPrevia = m;
  });
  $effect(() => {
    const nuevas = tablero.baldosas;
    if (typeof document === "undefined") return;
    const foto = fotoPrevia;
    if (foto.size === 0) return;
    for (const [id, b] of nuevas) {
      if (id === levantada) continue;
      const antes = foto.get(id);
      if (!antes) continue;
      const el = document.querySelector<HTMLElement>(`[data-pieza="${CSS.escape(id)}"]`);
      if (!el) continue;
      const dx = antes.x - b.x;
      const dy = antes.y - b.y;
      const s = Math.sqrt(((antes.w / b.ancho) * antes.h) / b.alto);
      if (Math.abs(dx) < 1 && Math.abs(dy) < 1 && Math.abs(s - 1) < 0.012) continue;
      el.style.transition = "none";
      el.style.transform = `translate(${dx}px, ${dy}px) scale(${s})`;
      requestAnimationFrame(() => {
        el.style.transition = `transform ${SERENO}`;
        el.style.transform = "";
      });
    }
  });

  // La onda: al tocar una pieza, las demás laten en cascada desde ella.
  function onda(desdeId: string) {
    const ids = visibles.map((i) => i.id);
    const centro = ids.indexOf(desdeId);
    ids.forEach((id, i) => {
      if (id === desdeId) return;
      const el = document.querySelector<HTMLElement>(`[data-pieza="${CSS.escape(id)}"] .baldosa-cuerpo`);
      if (!el) return;
      setTimeout(() => {
        el.classList.add("late");
        setTimeout(() => el.classList.remove("late"), 300);
      }, Math.abs(i - centro) * 24);
    });
  }

  // El vuelo del pájaro: cuando llega una pieza, sale de la boca y vuela
  // EN ARCO hasta la recién nacida, la picotea y la pieza late.
  let pajaro = $state<{ x: number; y: number; girado: number } | null>(null);
  function volar(aId: string) {
    requestAnimationFrame(() => {
      const desde = document
        .querySelector(`[data-pieza="${CSS.escape(BOCA_ID)}"]`)
        ?.getBoundingClientRect();
      const hasta = document
        .querySelector(`[data-pieza="${CSS.escape(aId)}"]`)
        ?.getBoundingClientRect();
      if (!desde || !hasta) {
        haptic("heavy");
        return;
      }
      const x0 = desde.left + desde.width / 2;
      const y0 = desde.top + desde.height / 2;
      const x1 = hasta.left + hasta.width / 2;
      const y1 = hasta.top + Math.min(26, hasta.height / 2);
      const cx = (x0 + x1) / 2;
      const cy = Math.min(y0, y1) - 90;
      const t0 = performance.now();
      const dur = 620;
      const paso = (t: number) => {
        const u = Math.min(1, (t - t0) / dur);
        const e = u < 0.5 ? 2 * u * u : 1 - Math.pow(-2 * u + 2, 2) / 2;
        const ix = (1 - e) * (1 - e) * x0 + 2 * (1 - e) * e * cx + e * e * x1;
        const iy = (1 - e) * (1 - e) * y0 + 2 * (1 - e) * e * cy + e * e * y1;
        pajaro = { x: ix, y: iy, girado: x1 >= x0 ? -1 : 1 };
        if (u < 1) requestAnimationFrame(paso);
        else {
          pajaro = null;
          haptic("heavy");
          onda(aId);
        }
      };
      requestAnimationFrame(paso);
    });
  }

  function alTocar(e: PointerEvent, id: string) {
    arranque = { id, x: e.clientX, y: e.clientY, decidido: "no" };
    clearTimeout(holdTimer);
    holdTimer = setTimeout(() => {
      // Posar el dedo un instante LEVANTA la baldosa (con su pop): si la
      // mueves, reordenas el tablero en vivo; si la sueltas ahí, su menú.
      if (arranque && arranque.decidido === "no") {
        haptic("rigid");
        levantada = arranque.id;
        recogida = true;
        setTimeout(() => (recogida = false), 200);
        vuelo = { dx: 0, dy: 0, estira: 1, giro: 0, vx: 0, vy: 0 };
        vueloPrevio = { x: arranque.x, y: arranque.y, t: performance.now() };
        seMovioEnVuelo = false;
      }
    }, 160);
  }

  // El índice de inserción: cuenta cuántas baldosas (sin la levantada)
  // quedan ANTES del dedo en orden de lectura del tablero actual.
  function indiceDesde(px: number, py: number): number {
    const b = tablero.baldosas;
    let indice = 0;
    for (const item of visibles) {
      if (item.id === levantada) continue;
      const bb = b.get(item.id);
      if (!bb) continue;
      const cx = bb.x + bb.ancho / 2;
      const cy = bb.y + bb.alto / 2;
      if (cy < py - bb.alto / 2) indice += 1;
      else if (Math.abs(cy - py) <= bb.alto / 2 + 5 && cx < px) indice += 1;
    }
    return indice;
  }

  function alMover(e: PointerEvent) {
    if (!arranque) return;
    const dx = e.clientX - arranque.x;
    const dy = e.clientY - arranque.y;
    if (levantada === arranque.id) {
      // La velocidad manda la deformación: estirón con el movimiento,
      // compresión perpendicular, y el cuerpo se ladea hacia donde va.
      const ahora = performance.now();
      const dt = Math.max(8, ahora - vueloPrevio.t);
      const vx = (e.clientX - vueloPrevio.x) / dt;
      const vy = (e.clientY - vueloPrevio.y) / dt;
      vueloPrevio = { x: e.clientX, y: e.clientY, t: ahora };
      const rapidez = Math.min(1.2, Math.hypot(vx, vy));
      const estira = 1 + rapidez * 0.09;
      const giro = Math.max(-8, Math.min(8, vx * 9));
      vuelo = { dx, dy, estira, giro, vx, vy };
      if (Math.abs(dx) + Math.abs(dy) > 6) seMovioEnVuelo = true;
      // El reorden EN VIVO: la posición del dedo (en coordenadas del
      // tablero) decide el hueco, y el resto se recoloca con muelle.
      const mosaicoEl = document.querySelector<HTMLElement>(".mosaico");
      if (!mosaicoEl) return;
      const r = mosaicoEl.getBoundingClientRect();
      // El índice llega en espacio de VISIBLES; se traduce a items (que
      // puede llevar alguna oculta esperando su «deshacer»).
      const destinoVis = indiceDesde(e.clientX - r.left, e.clientY - r.top);
      const actual = items.findIndex((i) => i.id === levantada);
      if (actual >= 0) {
        const copia = [...items];
        const [pieza] = copia.splice(actual, 1);
        const visSin = copia.filter((i) => !ocultas.has(i.id));
        const idxIns =
          destinoVis >= visSin.length ? copia.length : copia.indexOf(visSin[destinoVis]);
        copia.splice(idxIns, 0, pieza);
        if (copia.some((it, k) => it.id !== items[k]?.id)) {
          haptic("tick");
          items = copia;
        }
      }
      return;
    }
    if (arranque.decidido === "no") {
      if (Math.abs(dx) < 14 && Math.abs(dy) < 14) return;
      clearTimeout(holdTimer);
      if (Math.abs(dy) > Math.abs(dx)) {
        arranque = null; // scroll vertical de la lista
        return;
      }
      arranque.decidido = "swipe";
      (e.target as HTMLElement).setPointerCapture?.(e.pointerId);
    }
    if (arranque.decidido === "swipe") arrastre = { id: arranque.id, dx };
  }
  async function alSoltar() {
    clearTimeout(holdTimer);
    const d = arrastre;
    if (levantada) {
      const id = levantada;
      const movida = seMovioEnVuelo;
      const impulso = vuelo.vx;
      levantada = null;
      vuelo = { dx: 0, dy: 0, estira: 1, giro: 0, vx: 0, vy: 0 };
      arranque = null;
      if (!movida) {
        // Levantada y soltada en el sitio: su menú.
        haptic("soft");
        menuPieza = items.find((i) => i.id === id) ?? null;
        renombrando = false;
        return;
      }
      // LANZADA: si sale despedida con velocidad horizontal alta, es
      // borrar (con «deshacer»: el borrado real espera cinco segundos).
      if (Math.abs(impulso) > 0.9) {
        lanzar(id, impulso);
        return;
      }
      haptic("success");
      const indice = items.findIndex((i) => i.id === id);
      if (indice >= 0) await colaReordenar(id, indice).catch(() => {});
      return;
    }
    arranque = null;
    if (!d) {
      arrastre = null;
      return;
    }
    if (Math.abs(d.dx) > 110) {
      haptic("success");
      arrastre = { id: d.id, dx: d.dx > 0 ? 620 : -620 };
      setTimeout(() => {
        const it = items.find((i) => i.id === d.id);
        arrastre = null;
        if (it) esconderYProgramar(it);
      }, 170);
    } else {
      arrastre = null;
    }
  }

  // ── Lanzar para borrar, con deshacer ──────────────────────────────────
  let deshacer = $state<{ id: string; titulo: string } | null>(null);
  let timerBorrado: ReturnType<typeof setTimeout> | undefined;
  function lanzar(id: string, vx: number) {
    const item = items.find((i) => i.id === id);
    if (!item) return;
    haptic("warning");
    const el = document.querySelector<HTMLElement>(`[data-pieza="${CSS.escape(id)}"]`);
    if (el) {
      const destinoX = (vx > 0 ? 1 : -1) * (window.innerWidth + 280);
      el.style.transition = "transform 0.5s cubic-bezier(0.2, 0.5, 0.5, 1), opacity 0.5s ease";
      el.style.transform = `translate(${destinoX}px, ${vuelo.dy - 40}px) rotate(${vx * 34}deg)`;
      el.style.opacity = "0.5";
    }
    setTimeout(() => esconderYProgramar(item), 240);
  }
  function esconderYProgramar(item: ItemCola) {
    // Si había otro borrado pendiente, se consuma ya (solo un deshacer).
    if (deshacer) {
      const previa = deshacer.id;
      clearTimeout(timerBorrado);
      colaEliminar(previa).catch(() => {});
    }
    idLanzada = item.id;
    ocultas.add(item.id);
    ocultas = new Set(ocultas);
    if (seleccionada === item.id) seleccionada = null;
    deshacer = { id: item.id, titulo: item.titulo };
    timerBorrado = setTimeout(async () => {
      const id = deshacer?.id;
      deshacer = null;
      if (id) {
        await colaEliminar(id).catch(() => {});
        items = await colaListar().catch(() => items);
        ocultas.delete(id);
        ocultas = new Set(ocultas);
      }
    }, 5000);
  }
  function deshacerBorrado() {
    haptic("success");
    clearTimeout(timerBorrado);
    if (deshacer) {
      const id = deshacer.id;
      deshacer = null;
      idLanzada = null;
      const el = document.querySelector<HTMLElement>(`[data-pieza="${CSS.escape(id)}"]`);
      if (el) {
        el.style.transition = "";
        el.style.transform = "";
        el.style.opacity = "";
      }
      ocultas.delete(id);
      ocultas = new Set(ocultas);
    }
  }

  // ── El menú de la pieza (favorito, renombrar, quitar) ─────────────────
  async function alternarFavorito() {
    if (!menuPieza) return;
    const id = menuPieza.id;
    menuPieza = null;
    await favoritoDirecto(id);
  }
  async function favoritoDirecto(id: string) {
    haptic("medium");
    const item = items.find((i) => i.id === id);
    if (!item) return;
    await colaFavorito(id, !item.favorito).catch(() => {});
    items = await colaListar().catch(() => items);
    onda(id);
  }
  async function guardarNombre() {
    if (!menuPieza || !nuevoNombre.trim()) return;
    haptic("success");
    await colaRenombrar(menuPieza.id, nuevoNombre.trim()).catch(() => {});
    items = await colaListar().catch(() => items);
    menuPieza = null;
    renombrando = false;
  }
  async function borrarPieza() {
    if (!menuPieza) return;
    const item = menuPieza;
    menuPieza = null;
    haptic("warning");
    esconderYProgramar(item);
  }

  const vacia = $derived(visibles.length === 0 && bobinas.length === 0);

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
  <!-- LA LISTA: el único scroller. NO hay cabecera: la marca, la
       trastienda y la boca son PIEZAS del propio mosaico. -->
  <div class="lista" ontouchmove={(e) => { if (levantada) e.preventDefault(); }}>
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

    <!-- EL MOSAICO: la percha. La boca-baldosa primera, las piezas después. -->
    <div class="mosaico" bind:clientWidth={anchoTablero} style="height: {tablero.alto}px">
      {#each piezasTablero as ficha, i (ficha.id)}
        {@const b = tablero.baldosas.get(ficha.id)}
        {#if b && ficha.id === MARCA_ID}
          <!-- LA MARCA: el loro vive posado sobre esta pieza, y el «yappy»
               es un recorte de letras de colores (nota de rescate). -->
          <article
            data-pieza={MARCA_ID}
            class="baldosa"
            style="left: {b.x}px; top: {b.y}px; width: {b.ancho}px; height: {b.alto}px; z-index: 3;"
          >
            <span class="loro-percha" class:voltereta>
              <Criatura
                size={Math.min(64, b.alto * 0.78)}
                mirando={-1}
                estado={celebra ? "celebrando" : repro.snap?.estado === "sonando" ? "hablando" : repro.snap?.estado === "pausa" ? "pausa" : durmiendo ? "dormido" : "posado"}
                apertura={repro.snap?.estado === "sonando" ? nivel : 0}
                {mirada}
                tinta={$tintaVoz}
              />
            </span>
            <div class="baldosa-cuerpo marca-baldosa" style="border-radius: {radiosDe(MARCA_ID)}; transform: rotate({tiltDe(MARCA_ID)}deg);">
              <button class="marca-toque" use:presionable={{ hap: "light" }} onclick={tocarMarca} aria-label="yappy">
                <span class="marca-palabra" class:gelatina={marcaGelatina}>
                  {#each "yappy".split("") as letra, k (k)}
                    <i class="letra" style="animation-delay: {k * 45}ms; color: {PALETA[(k * 3 + 1) % PALETA.length]}; transform: rotate({k % 2 === 0 ? -3.5 : 3}deg) translateY({k % 2 === 0 ? -1 : 1.5}px)">{letra}</i>
                  {/each}
                </span>
              </button>
            </div>
          </article>
        {:else if b && ficha.id === TRAS_ID}
          <!-- LA TRASTIENDA: su engranaje, pieza del mosaico. -->
          <article
            data-pieza={TRAS_ID}
            class="baldosa"
            style="left: {b.x}px; top: {b.y}px; width: {b.ancho}px; height: {b.alto}px; z-index: 2;"
          >
            <div class="baldosa-cuerpo trastienda-baldosa" style="border-radius: {radiosDe(TRAS_ID)}; transform: rotate({tiltDe(TRAS_ID)}deg);">
              <button
                class="trastienda-toque"
                use:presionable={{ hap: "soft" }}
                onclick={() => { engranajeGira = true; setTimeout(() => goto("/ajustes"), 240); }}
                aria-label={$t("cinta.trastienda")}
              >
                <span class="engranaje" class:gira={engranajeGira}>
                  <svg viewBox="0 0 24 24" width="26" height="26" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="12" cy="12" r="3.2"/><path d="M19.4 15a1.7 1.7 0 0 0 .34 1.87l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-1.87-.34 1.7 1.7 0 0 0-1.03 1.56V21a2 2 0 1 1-4 0v-.09a1.7 1.7 0 0 0-1.11-1.56 1.7 1.7 0 0 0-1.87.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.7 1.7 0 0 0 .34-1.87 1.7 1.7 0 0 0-1.56-1.03H3a2 2 0 1 1 0-4h.09a1.7 1.7 0 0 0 1.56-1.11 1.7 1.7 0 0 0-.34-1.87l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.7 1.7 0 0 0 1.87.34h.01a1.7 1.7 0 0 0 1.02-1.56V3a2 2 0 1 1 4 0v.09a1.7 1.7 0 0 0 1.03 1.56 1.7 1.7 0 0 0 1.87-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.7 1.7 0 0 0-.34 1.87v.01a1.7 1.7 0 0 0 1.56 1.02H21a2 2 0 1 1 0 4h-.09a1.7 1.7 0 0 0-1.56 1.03Z"/></svg>
                </span>
                {#if b.ancho > 118}
                  <span class="trastienda-rotulo">{$t("cinta.trastienda")}</span>
                {/if}
              </button>
            </div>
          </article>
        {:else if b && ficha.id === BOCA_ID}
          <!-- LA BOCA-BALDOSA: una pieza del mosaico que SE CONVIERTE en el
               menú de añadir cuando la pulsas (su peso salta y el FLIP la
               hace crecer hasta tragarse el tablero). -->
          <article
            data-pieza={BOCA_ID}
            class="baldosa"
            style="left: {b.x}px; top: {b.y}px; width: {b.ancho}px; height: {b.alto}px; z-index: {bocaAbierta ? 30 : 2};"
          >
            <div class="baldosa-cuerpo boca-baldosa" class:abierta={bocaAbierta} style="border-radius: {radiosDe(BOCA_ID)};">
              {#if !bocaAbierta}
                <button class="boca-toque" in:llega use:presionable={{ hap: "medium" }} onclick={() => (bocaAbierta = true)} aria-label={$t("escuchar.anadir")}>
                  <span class="boca-cruz" aria-hidden="true">＋</span>
                  {#if b.alto > 100 || b.ancho > 150}
                    <span class="boca-rotulo">{$t("cinta.pega_aqui")}</span>
                  {/if}
                </button>
              {:else}
                <div class="boca-dentro">
                  <div class="asomado-boca" aria-hidden="true" in:llega><Criatura size={38} mirada={{ x: 0, y: 1 }} tinta={$tintaVoz} /></div>
                  <button class="boca-cerrar cruz-gira" onclick={() => { haptic("light"); bocaAbierta = false; }} aria-label={$t("comun.cerrar")}>＋</button>
                  <div in:llega={{ delay: 40 }}>
                    <button class="tecla-gorda protagonista" use:presionable={{ hap: "medium" }} onclick={pegarPortapapeles} disabled={pegando}>
                      <IconoTipo tipo="portapapeles" size={22} /> {$t("cinta.portapapeles")}
                    </button>
                  </div>
                  <div class="enlace-fila" in:llega={{ delay: 90 }}>
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
                  <div in:llega={{ delay: 140 }}>
                    <button class="tecla-gorda" use:presionable onclick={abrirArchivo}>
                      <IconoTipo tipo="documento" size={22} /> {$t("cinta.archivo")}
                    </button>
                  </div>
                </div>
              {/if}
            </div>
          </article>
        {:else if b}
          {@const item = visibles.find((v) => v.id === ficha.id)}
          {#if item}
            {@const pct = item.id === idQueSuena && repro.snap ? Math.min(100, Math.round((repro.snap.elapsed_secs / Math.max(1, repro.snap.duration_secs)) * 100)) : pctDe(item)}
            {@const esLaQueSuena = item.id === idQueSuena}
            {@const enVuelo = levantada === item.id}
            {@const elegida = seleccionada === item.id}
            {@const tinta = tintaDe(item)}
            {@const honda = tonoHondo(tinta)}
            {@const forma = formaDe(item.id)}
            {@const lineasCartel = esLaQueSuena ? [] : cartel(item.titulo, b.ancho, b.alto, $idiomaUI)}
            <article
              data-pieza={item.id}
              class="baldosa" class:en-vuelo={enVuelo}
              in:llega={{ delay: cascada ? Math.min(i * 40, 360) : 0 }}
              out:seVa
              style="left: {b.x}px; top: {b.y}px; width: {b.ancho}px; height: {b.alto}px; z-index: {enVuelo ? 40 : elegida ? 10 : esLaQueSuena ? 8 : item.favorito ? 4 : 1}; {enVuelo ? `transition: ${recogida ? 'transform 0.18s cubic-bezier(0.3, 1.4, 0.6, 1)' : 'none'}; transform: translate(${vuelo.dx}px, ${vuelo.dy}px) scale(${1.09 * vuelo.estira}, ${1.09 * (2 - vuelo.estira)}) rotate(${vuelo.giro}deg);` : ''}"
              onpointerdown={(e) => alTocar(e, item.id)}
              onpointermove={alMover}
              onpointerup={alSoltar}
              onpointercancel={() => { clearTimeout(holdTimer); arranque = null; arrastre = null; levantada = null; vuelo = { dx: 0, dy: 0, estira: 1, giro: 0, vx: 0, vy: 0 }; }}
            >
              <!-- La asomada: un pájaro curiosea tras el canto superior
                   (el de la voz si suena; avergonzado si la pieza falló). -->
              {#if esLaQueSuena || asomadoEn === item.id || item.estado === "error"}
                <span class="asomado-pieza" aria-hidden="true">
                  <Criatura
                    size={30}
                    mirando={-1}
                    estado={item.estado === "error" ? "avergonzado" : esLaQueSuena ? (repro.snap?.estado === "pausa" ? "pausa" : "hablando") : "posado"}
                    apertura={esLaQueSuena && repro.snap?.estado === "sonando" ? nivel : 0}
                    {mirada}
                    tinta={esLaQueSuena ? $tintaVoz : tinta}
                  />
                </span>
              {/if}
              <div
                class="baldosa-cuerpo estado-{item.estado}"
                class:sacude={sacudida === item.id}
                class:suena={esLaQueSuena}
                class:elegida
                style="{item.estado === 'listo' || esLaQueSuena ? `background: ${tinta};` : ''} border-radius: {forma.radios}; {!esLaQueSuena && forma.clip ? `clip-path: ${forma.clip};` : ''} transform: translateX({arrastre?.id === item.id ? arrastre.dx : 0}px) rotate({arrastre?.id === item.id ? arrastre.dx / 26 : tiltDe(item.id)}deg); opacity: {arrastre?.id === item.id ? Math.max(0.25, 1 - Math.abs(arrastre.dx) / 340) : 1}; transition: {arrastre?.id === item.id ? 'none' : 'transform 0.3s cubic-bezier(0.34, 1.56, 0.64, 1), border-radius 0.3s ease, background-color 0.45s ease'};"
              >
                <!-- La línea de flotación: lo ya escuchado sube como marea. -->
                {#if pct > 0 && (item.estado === "listo" || esLaQueSuena)}
                  <span class="marea" style="height: {pct}%; background: {honda}"></span>
                {/if}
                <!-- EL CARTEL: el titular llena ancho y alto deformándose. -->
                {#if esLaQueSuena}
                  <div class="teletipo" aria-hidden="true">
                    {#key repro.snap?.current_text}
                      <p in:fly={{ y: 22, duration: 340 }} out:fly={{ y: -22, duration: 220 }}>{repro.snap?.current_text || item.titulo}</p>
                    {/key}
                  </div>
                {:else if lineasCartel.length > 0}
                  <div class="cartel" aria-hidden="true">
                    {#each lineasCartel as linea (linea.texto)}
                      <svg viewBox="0 0 {linea.vb} {VB_ALTO}" preserveAspectRatio="none">
                        <text x="0" y={VB_BASE} textLength={linea.vb} lengthAdjust="spacingAndGlyphs">{linea.texto}</text>
                      </svg>
                    {/each}
                  </div>
                {/if}
                <!-- La etiqueta: una PASTILLA sólida (tinta honda), nunca
                     texto suelto peleándose con el cartel. En reposo, solo
                     los minutos; los estados hablan cuando toca. -->
                {#if esLaQueSuena}
                  <span class="pastilla" style="background: {honda}">
                    <span class="mini-ondas" style="--nivel: {0.35 + nivel * 0.65}" aria-hidden="true"><i></i><i></i><i></i></span>
                    {repro.snap?.estado === "pausa" ? $t("cinta.en_pausa") : $t("cinta.sonando")}
                  </span>
                {:else if item.estado === "listo"}
                  <span class="pastilla" style="background: {honda}">
                    <IconoTipo tipo={iconoDe(item.tipo)} size={11} />
                    {minutosDe(item)}′
                  </span>
                {:else if item.estado === "error"}
                  <span class="pastilla" style="background: #7a1712">{$t("cinta.error")}</span>
                {:else}
                  <span class="pastilla" style="background: #6f6757">{$t("cinta.preparando")}…</span>
                {/if}
                {#if item.favorito}
                  <span class="estrella" aria-hidden="true">
                    <svg viewBox="0 0 24 24" width="21" height="21" fill="var(--yap-dorado, #e8b41a)" stroke="var(--yap-tinta)" stroke-width="1.4" stroke-linejoin="round"><path d="M12 2.6l2.7 5.8 6.3.7-4.7 4.3 1.3 6.2-5.6-3.2-5.6 3.2 1.3-6.2L3 9.1l6.3-.7z"/></svg>
                  </span>
                {/if}
                <!-- El sello: escuchada entera, huella de pata. -->
                {#if pct >= 100 && !esLaQueSuena}
                  <span class="sello-pata" aria-hidden="true" in:llega>
                    <svg viewBox="0 0 24 24" width="20" height="20" fill="#ffffff"><ellipse cx="7" cy="7.6" rx="2.1" ry="2.9" transform="rotate(-18 7 7.6)"/><ellipse cx="12" cy="5.8" rx="2.1" ry="3"/><ellipse cx="17" cy="7.6" rx="2.1" ry="2.9" transform="rotate(18 17 7.6)"/><path d="M12 10.2c3.4 0 6 2.5 6 5.2 0 2.2-1.7 3.6-3.4 3.2-1.1-0.2-1.8-0.7-2.6-0.7s-1.5 0.5-2.6 0.7C7.7 19 6 17.6 6 15.4c0-2.7 2.6-5.2 6-5.2z"/></svg>
                  </span>
                {/if}
                <button class="baldosa-toque" use:presionable onclick={() => alternarSeleccion(item)} aria-label={item.titulo}></button>
                {#if elegida}
                  <div class="acciones" in:llega={{ delay: 60 }}>
                    <button class="accion principal" use:presionable={{ hap: "rigid" }} style="color: {tinta}"
                      onclick={(e) => { e.stopPropagation(); abrirItem(item); }}>
                      <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor"><path d="M7 4.8c0-1.1 1.2-1.8 2.2-1.2l11.5 7.2c0.9 0.6 0.9 1.9 0 2.4L9.2 20.4C8.2 21 7 20.3 7 19.2z"/></svg>
                      {$t("cola.escuchar")}
                    </button>
                    <button class="accion" use:presionable={{ hap: "soft" }} style="color: {tinta}" aria-label={item.favorito ? $t("pieza.quitar_favorito") : $t("pieza.favorito")}
                      onclick={(e) => { e.stopPropagation(); favoritoDirecto(item.id); }}>
                      <svg viewBox="0 0 24 24" width="19" height="19" fill={item.favorito ? "currentColor" : "none"} stroke="currentColor" stroke-width="2" stroke-linejoin="round"><path d="M12 2.6l2.7 5.8 6.3.7-4.7 4.3 1.3 6.2-5.6-3.2-5.6 3.2 1.3-6.2L3 9.1l6.3-.7z"/></svg>
                    </button>
                    <button class="accion" use:presionable={{ hap: "soft" }} style="color: {tinta}" aria-label={$t("pieza.renombrar")}
                      onclick={(e) => { e.stopPropagation(); menuPieza = item; renombrando = true; nuevoNombre = item.titulo; }}>
                      <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M17 3a2.8 2.8 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5z"/></svg>
                    </button>
                    <button class="accion" use:presionable={{ hap: "warning" }} style="color: {tinta}" aria-label={$t("cinta.borrar")}
                      onclick={(e) => { e.stopPropagation(); seleccionada = null; esconderYProgramar(item); }}>
                      <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M3 6h18M8 6V4a1 1 0 0 1 1-1h6a1 1 0 0 1 1 1v2m3 0v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6"/></svg>
                    </button>
                  </div>
                {/if}
              </div>
            </article>
          {/if}
        {/if}
      {/each}
    </div>

    {#if vacia}
      <section class="vacia">
        <Criatura size={148} andando mirando={-1} {mirada} tinta={$tintaVoz} />
        <h1>{$t("cinta.vacia_titulo")}</h1>
        <p>{$t("cinta.vacia_texto")}</p>
        <button class="yap-tecla ensename" use:presionable={{ hap: "rigid" }} onclick={ensename}>{$t("cinta.ensename")}</button>
      </section>
    {/if}

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

  <!-- El pájaro en vuelo: de la boca a la pieza recién llegada, en arco. -->
  {#if pajaro}
    <span class="pajaro-vuelo" style="left: {pajaro.x}px; top: {pajaro.y}px; transform: translate(-50%, -50%) scaleX({pajaro.girado})" aria-hidden="true">
      <Criatura size={30} estado="celebrando" tinta={$tintaVoz} />
    </span>
  {/if}

  <!-- El deshacer: cinco segundos de gracia tras lanzar una pieza. -->
  {#if deshacer}
    <div class="deshacer-aviso" in:fly={{ y: 26, duration: 260 }} out:fly={{ y: 26, duration: 200 }}>
      <span class="deshacer-texto">{$t("cinta.fuera")}</span>
      <button class="deshacer-tecla" use:presionable={{ hap: "medium" }} onclick={deshacerBorrado}>{$t("comun.deshacer")}</button>
    </div>
  {/if}

  {#if menuPieza}
    <div class="velo" role="presentation" onclick={() => { menuPieza = null; renombrando = false; }}></div>
    <div class="hoja">
      <div class="hoja-asa"></div>
      <div class="hoja-cabeza">{$t("pieza.opciones")}</div>
      <p class="menu-titulo">{menuPieza.titulo}</p>
      {#if renombrando}
        <!-- svelte-ignore a11y_autofocus -->
        <input class="yap-campo" type="text" bind:value={nuevoNombre} autofocus enterkeyhint="done"
          onkeydown={(e) => e.key === "Enter" && guardarNombre()} placeholder={$t("pieza.renombrar_pista")} />
        <button class="tecla-menu principal" use:presionable onclick={guardarNombre} disabled={!nuevoNombre.trim()}>
          {$t("comun.guardar")}
        </button>
      {:else}
        <button class="tecla-menu" use:presionable onclick={alternarFavorito}>
          <svg viewBox="0 0 24 24" width="20" height="20" fill={menuPieza.favorito ? "var(--yap-dorado, #e8b41a)" : "none"} stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"><path d="M12 2.6l2.7 5.8 6.3.7-4.7 4.3 1.3 6.2-5.6-3.2-5.6 3.2 1.3-6.2L3 9.1l6.3-.7z"/></svg>
          {menuPieza.favorito ? $t("pieza.quitar_favorito") : $t("pieza.favorito")}
        </button>
        <button class="tecla-menu" use:presionable onclick={() => { renombrando = true; nuevoNombre = menuPieza?.titulo ?? ""; }}>
          <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M17 3a2.8 2.8 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5z"/></svg>
          {$t("pieza.renombrar")}
        </button>
        <button class="tecla-menu peligro" use:presionable onclick={borrarPieza}>
          <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><path d="M3 6h18M8 6V4a1 1 0 0 1 1-1h6a1 1 0 0 1 1 1v2m3 0v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6"/></svg>
          {$t("cinta.borrar")}
        </button>
      {/if}
    </div>
  {/if}
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

  /* ── Las piezas del SISTEMA: la marca, la trastienda y la boca viven
     DENTRO del mosaico, en crema, para distinguirse de las piezas vivas. ── */
  .marca-baldosa,
  .trastienda-baldosa {
    background: var(--yap-superficie);
    border: 1.5px solid var(--yap-borde);
  }
  .loro-percha {
    position: absolute;
    top: -34px;
    left: 10px;
    z-index: 4;
    pointer-events: none;
    display: inline-flex;
  }
  .loro-percha.voltereta {
    animation: voltereta 0.85s cubic-bezier(0.34, 1.3, 0.5, 1);
  }
  @keyframes voltereta {
    0% { transform: rotate(0deg) scale(1); }
    55% { transform: rotate(-300deg) scale(1.15); }
    100% { transform: rotate(-360deg) scale(1); }
  }
  .marca-toque {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: flex-end;
    justify-content: flex-end;
    padding: 0 10px 4px 0;
    border: 0;
    background: transparent;
    cursor: pointer;
  }
  /* El «yappy» de recortes: cada letra con su color y su giro propios,
     como pegada de una revista distinta. */
  .marca-palabra {
    font-weight: 900;
    font-size: 40px;
    letter-spacing: -0.03em;
    display: inline-flex;
    line-height: 1;
  }
  .marca-palabra .letra {
    display: inline-block;
    font-style: normal;
  }
  .marca-palabra.gelatina .letra {
    animation: gelatina 0.55s cubic-bezier(0.36, 0.07, 0.19, 0.97) both;
  }
  @keyframes gelatina {
    0% { transform: scale(1, 1); }
    25% { transform: translateY(-4px) scale(1.22, 0.78); }
    45% { transform: translateY(1px) scale(0.86, 1.16); }
    65% { transform: scale(1.1, 0.92); }
    85% { transform: scale(0.97, 1.03); }
    100% { transform: scale(1, 1); }
  }
  .trastienda-toque {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 3px;
    border: 0;
    background: transparent;
    color: var(--yap-tinta);
    cursor: pointer;
    padding: 0;
  }
  .trastienda-rotulo {
    font-family: var(--yap-mono, ui-monospace, monospace);
    font-size: 10px;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--yap-tinta-suave);
  }
  .engranaje {
    display: inline-flex;
    transition: transform 0.5s cubic-bezier(0.34, 1.4, 0.5, 1);
  }
  .engranaje.gira {
    transform: rotate(180deg);
  }

  /* ── La lista: el único scroller. La losa, pegada a los márgenes. ── */
  .lista {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overscroll-behavior: contain;
    -webkit-overflow-scrolling: touch;
    display: flex;
    flex-direction: column;
    gap: 13px;
    /* Aire arriba para el loro, que asoma por encima de la primera fila. */
    padding: 40px 5px calc(env(safe-area-inset-bottom) + var(--aguja-hueco, 0px) + 22px);
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
    margin: 0 13px;
  }

  /* ── El vacío que enseña (debajo de la boca-baldosa) ── */
  .vacia {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    gap: 12px;
    flex: 1;
    padding: 3vh 10px;
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

  /* ── El mosaico: la losa viva ── */
  .mosaico {
    position: relative;
    width: 100%;
    flex-shrink: 0;
  }
  .baldosa {
    position: absolute;
    will-change: transform;
    touch-action: pan-y;
  }
  .baldosa-cuerpo {
    position: relative;
    width: 100%;
    height: 100%;
    border: 0;
    color: #ffffff;
    overflow: hidden;
    z-index: 1;
  }
  .baldosa.en-vuelo .baldosa-cuerpo {
    filter: brightness(1.07);
  }
  /* El material cede: al presionar, las esquinas se ablandan. (:global
     porque .pulsado la pone el presionable en runtime y Svelte podaría
     el selector al no verla en la plantilla.) */
  :global(.baldosa-cuerpo:has(.pulsado)) {
    border-radius: 30px 26px 32px 28px / 28px 32px 26px 30px !important;
  }
  /* La que suena RESPIRA: sus radios ondulan en bucle. */
  .baldosa-cuerpo.suena {
    animation: respira-fieltro 2.8s ease-in-out infinite;
  }
  @keyframes respira-fieltro {
    0%, 100% { border-radius: 24px 30px 22px 30px / 30px 22px 30px 24px; }
    50% { border-radius: 34px 22px 34px 24px / 24px 34px 22px 34px; }
  }
  /* Modo noche: todas duermen salvo la que suena. */
  :global(html[data-theme="dark"]) .baldosa-cuerpo:not(.suena) {
    filter: brightness(0.52) saturate(0.6);
  }

  /* EL CARTEL: pegado a los cantos laterales y al de arriba (5 px de
     respiración para que las esquinas no muerdan letras) y con la FRANJA
     de la pastilla reservada abajo: la última línea nunca muere debajo. */
  .cartel {
    position: absolute;
    left: 5px;
    right: 5px;
    top: 2px;
    bottom: 30px;
    display: flex;
    flex-direction: column;
    z-index: 3;
    pointer-events: none;
  }
  .baldosa-cuerpo.elegida .cartel {
    bottom: 56px;
  }
  /* Elegir tiene su POP además del FLIP: el sí se siente. */
  .baldosa-cuerpo.elegida {
    animation: pop-elegida 0.5s cubic-bezier(0.24, 1.7, 0.44, 1);
  }
  @keyframes pop-elegida {
    0% { transform: scale(0.95); }
    100% { transform: scale(1); }
  }
  .cartel svg {
    display: block;
    width: 100%;
    flex: 1;
    min-height: 0;
  }
  .cartel text {
    fill: #ffffff;
    font-weight: 800;
    font-size: 100px;
    letter-spacing: -0.01em;
  }

  /* La marea: lo escuchado sube desde abajo, borde nítido. */
  .marea {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: 2;
    pointer-events: none;
    transition: height 0.6s ease;
  }

  /* El teletipo de la que suena: las palabras que se están diciendo. */
  .teletipo {
    position: absolute;
    inset: 6px 8px 26px;
    z-index: 3;
    overflow: hidden;
    display: grid;
    pointer-events: none;
  }
  .teletipo p {
    grid-area: 1 / 1;
    margin: 0;
    align-self: center;
    color: #ffffff;
    font-weight: 800;
    font-size: 17px;
    line-height: 1.22;
    letter-spacing: -0.01em;
    display: -webkit-box;
    -webkit-line-clamp: 4;
    line-clamp: 4;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  /* La pastilla: la etiqueta sólida (tinta honda) que no pelea jamás con
     el cartel: es un objeto encima, no texto suelto. */
  .pastilla {
    position: absolute;
    right: 8px;
    bottom: 7px;
    z-index: 4;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 4px 9px;
    border-radius: 999px;
    font-family: var(--yap-mono, ui-monospace, monospace);
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.05em;
    color: #ffffff;
    white-space: nowrap;
    max-width: calc(100% - 12px);
    overflow: hidden;
    pointer-events: none;
  }

  .baldosa-toque {
    position: absolute;
    inset: 0;
    z-index: 5;
    border: 0;
    background: transparent;
    cursor: pointer;
    padding: 0;
  }

  .acciones {
    position: absolute;
    left: 8px;
    right: 8px;
    bottom: 8px;
    display: flex;
    gap: 7px;
    align-items: center;
    z-index: 6;
  }
  .accion {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
    border: 0;
    background: #ffffff;
    border-radius: 12px;
    height: 38px;
    min-width: 38px;
    padding: 0 10px;
    font-weight: 800;
    font-size: 14px;
    cursor: pointer;
  }
  .accion.principal {
    flex: 1;
  }
  /* El latido de la onda: las vecinas responden al toque. */
  .baldosa-cuerpo.late {
    animation: late 0.3s ease;
  }
  @keyframes late {
    35% { transform: scale(0.972); }
  }

  .estrella {
    position: absolute;
    top: -8px;
    right: 12px;
    transform: rotate(10deg);
    z-index: 6;
    pointer-events: none;
  }
  .sello-pata {
    position: absolute;
    top: 6px;
    right: 8px;
    transform: rotate(14deg);
    z-index: 4;
    pointer-events: none;
  }
  .asomado-pieza {
    position: absolute;
    top: -15px;
    left: 12%;
    z-index: 0;
    pointer-events: none;
  }

  /* Los estados de cocina y error: rayas y rojo, planos. */
  .baldosa-cuerpo.estado-pendiente,
  .baldosa-cuerpo.estado-preparando {
    background: repeating-linear-gradient(-45deg, #8e8574 0 12px, #7c7466 12px 22px);
    background-size: 200% 100%;
    animation: cocinando 2.4s linear infinite;
  }
  @keyframes cocinando {
    to { background-position: -62px 0, 0 0; }
  }
  .baldosa-cuerpo.estado-error {
    background: #b3261e;
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

  /* ── La boca-baldosa ── */
  .boca-baldosa {
    background: var(--yap-superficie);
    border: 2.5px dashed color-mix(in srgb, var(--yap-tinta-suave, #82755a) 55%, var(--yap-superficie));
    color: var(--yap-tinta-suave, #82755a);
  }
  .boca-toque {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 4px;
    border: 0;
    background: transparent;
    color: inherit;
    cursor: pointer;
    padding: 0;
  }
  .boca-cruz {
    font-size: 26px;
    font-weight: 700;
    display: inline-block;
    animation:
      cruz-respira 5s ease-in-out infinite,
      cruz-arcoiris 14s linear infinite;
  }
  @keyframes cruz-respira {
    0%, 86%, 100% { transform: scale(1) rotate(0deg); }
    90% { transform: scale(1.28, 0.8) rotate(-6deg); }
    95% { transform: scale(0.9, 1.14) rotate(3deg); }
  }
  /* El ＋ recorre la paleta: la invitación de color. */
  @keyframes cruz-arcoiris {
    0% { color: #ff6b6b; }
    20% { color: #f94892; }
    40% { color: #5d5fef; }
    60% { color: #00a896; }
    80% { color: #ff8e3c; }
    100% { color: #ff6b6b; }
  }
  .boca-rotulo {
    font-family: var(--yap-mono, ui-monospace, monospace);
    font-size: 12px;
    letter-spacing: 0.16em;
    text-transform: uppercase;
  }
  .boca-dentro {
    position: relative;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 10px;
    padding: 14px;
    height: 100%;
  }
  .asomado-boca {
    position: absolute;
    top: -26px;
    right: 16px;
    pointer-events: none;
  }
  .cruz-gira {
    font-size: 22px;
    font-weight: 700;
    transform: rotate(45deg);
    transition: transform 0.35s cubic-bezier(0.34, 1.56, 0.64, 1);
  }
  .boca-cerrar {
    position: absolute;
    top: 2px;
    right: 8px;
    border: 0;
    background: transparent;
    color: var(--yap-tinta-suave);
    padding: 6px;
    cursor: pointer;
    z-index: 2;
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
    padding: 15px 16px;
    border: 1px solid var(--yap-borde);
    border-radius: 14px;
    background: var(--yap-papel);
    color: var(--yap-tinta);
    font-weight: 700;
    font-size: 16px;
    box-shadow: var(--yap-relieve);
    cursor: pointer;
  }
  .tecla-gorda.protagonista {
    border: 0;
    background: var(--vivo, var(--acento-voz, #e0502a));
    color: #fff6ef;
    font-size: 17px;
    padding: 17px 16px;
  }
  .tecla-gorda:disabled {
    opacity: 0.6;
  }

  /* ── El pájaro en vuelo y el deshacer ── */
  .pajaro-vuelo {
    position: fixed;
    z-index: 70;
    pointer-events: none;
  }
  .deshacer-aviso {
    position: fixed;
    left: 16px;
    right: 16px;
    bottom: calc(env(safe-area-inset-bottom) + var(--aguja-hueco, 0px) + 16px);
    z-index: 60;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 12px 14px;
    border-radius: 16px;
    background: var(--yap-tinta, #2b2418);
    color: #fff6ef;
  }
  .deshacer-texto {
    font-weight: 700;
    font-size: 14px;
  }
  .deshacer-tecla {
    border: 0;
    background: #fff6ef;
    color: var(--yap-tinta, #2b2418);
    font-weight: 800;
    font-size: 14px;
    padding: 8px 14px;
    border-radius: 10px;
    cursor: pointer;
  }

  /* ── El menú de la pieza ── */
  .velo {
    position: fixed;
    inset: 0;
    z-index: 50;
    background: rgba(43, 36, 24, 0.35);
  }
  .hoja {
    position: fixed;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: 51;
    background: var(--yap-papel);
    border-radius: 22px 22px 0 0;
    box-shadow: 0 -12px 40px rgba(64, 46, 12, 0.28);
    padding: 10px 18px calc(env(safe-area-inset-bottom) + 18px);
    display: flex;
    flex-direction: column;
    gap: 10px;
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
  .menu-titulo {
    margin: 0;
    font-weight: 800;
    font-size: 16px;
    text-align: center;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .tecla-menu {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 15px 16px;
    border: 1px solid var(--yap-borde);
    border-radius: 14px;
    background: var(--yap-superficie);
    color: var(--yap-tinta);
    font-weight: 700;
    font-size: 16px;
    box-shadow: var(--yap-relieve);
    cursor: pointer;
  }
  .tecla-menu.principal {
    border: 0;
    background: var(--vivo, var(--acento-voz, #e0502a));
    color: #fff6ef;
    justify-content: center;
  }
  .tecla-menu.peligro {
    color: var(--yap-voz, #e0502a);
    border-color: color-mix(in srgb, var(--yap-voz, #e0502a) 45%, var(--yap-borde));
  }
  .tecla-menu:disabled {
    opacity: 0.55;
  }

  /* ── Bobinas ── */
  .bobina {
    background: var(--yap-superficie);
    border: 1px solid var(--yap-borde);
    box-shadow: var(--yap-relieve);
    overflow: hidden;
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
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .pieza-meta {
    font-family: var(--yap-mono, ui-monospace, monospace);
    font-size: 11.5px;
    letter-spacing: 0.08em;
    color: var(--yap-tinta-suave);
  }
  .mini-ondas {
    display: inline-flex;
    align-items: flex-end;
    gap: 2px;
    height: 11px;
    transform: scaleY(var(--nivel, 0.6));
    transform-origin: 50% 100%;
    transition: transform 0.1s linear;
  }
  .mini-ondas i {
    width: 3px;
    border-radius: 2px;
    background: #ffffff;
    animation: onda-mini 0.9s ease-in-out infinite;
  }
  .mini-ondas i:nth-child(1) { height: 45%; }
  .mini-ondas i:nth-child(2) { height: 100%; animation-delay: 0.15s; }
  .mini-ondas i:nth-child(3) { height: 70%; animation-delay: 0.3s; }
  @keyframes onda-mini {
    0%, 100% { transform: scaleY(0.5); }
    50% { transform: scaleY(1); }
  }

  .rotulo-tramo {
    margin: 10px 15px 0;
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

  @media (prefers-reduced-motion: reduce) {
    .boca-cruz,
    .baldosa-cuerpo.suena,
    .marca-palabra.gelatina .letra,
    .loro-percha.voltereta {
      animation: none;
    }
  }
</style>
