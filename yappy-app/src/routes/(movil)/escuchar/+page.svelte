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
    radiosDe,
    tiltDe,
  } from "$lib/juguete";
  import {
    TROQUELES_BASE,
    CORAZON,
    ESTRELLA,
    troquelBase,
    aPoligono,
    aPuntosSvg,
    type Troquel,
  } from "$lib/troquel";
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
  // La voz está TRABAJANDO (sonando o cocinando). Una pausa abandonada NO
  // cuenta: la pieza pausada vuelve a ser una pieza normal con su título
  // (el pausa-zombi congelaba la frase y el tamaño gigante para siempre).
  const trabajando = $derived(
    repro.snap?.estado === "sonando" || repro.snap?.estado === "preparando",
  );
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
    // EL MAESTRO DE CEREMONIAS (docs/EL-ALBUM.md §E10): si al volver a la
    // percha hay una pieza recién COMPLETADA, el pájaro vuela hasta ella
    // (que ya luce su troquel de estrella) y la casa dice «fin».
    {
      const g = globalThis as unknown as { __yappyCompletadas?: Set<string> };
      const antes = g.__yappyCompletadas;
      const ahora = new Set(items.filter((i) => pctDe(i) >= 100).map((i) => i.id));
      if (antes) {
        for (const id of ahora) {
          if (!antes.has(id)) {
            setTimeout(() => {
              volar(id);
              if (!sonando) decir(getStore(t)("cartel.fin")).catch(() => {});
            }, 700);
            break;
          }
        }
      }
      g.__yappyCompletadas = ahora;
    }
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
      // Con una PILA de piezas sin estrenar, el fisgón sale más a mirar.
      const pila = items.filter((i) => i.estado === "listo" && pctDe(i) === 0).length;
      if (Math.random() < (pila >= 3 ? 0.2 : 0.55)) return;
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
  const SELLO_BOCA = TROQUELES_BASE.find((t) => t.nombre === "sello") ?? TROQUELES_BASE[0];

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

  // LOS TROQUELES (docs/EL-ALBUM.md): la forma de cada pieza. Corazón y
  // estrella están RESERVADAS (favorito y completada); el resto recibe su
  // forma base determinista con anti-choque entre vecinas.
  const troquelesVivos = $derived.by(() => {
    const m = new Map<string, Troquel>();
    let previo = "";
    for (const it of visibles) {
      if (it.favorito) {
        m.set(it.id, CORAZON);
        previo = CORAZON.nombre;
        continue;
      }
      if (pctDe(it) >= 100) {
        m.set(it.id, ESTRELLA);
        previo = ESTRELLA.nombre;
        continue;
      }
      let tq = troquelBase(it.id);
      if (tq.nombre === previo) {
        tq = TROQUELES_BASE[(TROQUELES_BASE.indexOf(tq) + 3) % TROQUELES_BASE.length];
      }
      m.set(it.id, tq);
      previo = tq.nombre;
    }
    return m;
  });

  // La regla nueve: en reposo se RECONOCE (si el título no cabe legible en
  // la ventana, se enseñan sus primeras palabras con «…»); al elegir, la
  // pieza crece y el título se LEE entero.
  function tituloEnVentana(titulo: string, vw: number, vh: number, completo: boolean): string {
    if (completo || titulo.length <= 18) return titulo;
    const area = Math.max(1, vw * (vh + 12));
    if (area / titulo.length >= 165) return titulo;
    const tope = Math.max(16, Math.floor(area / 165));
    const palabras = titulo.split(/\s+/);
    let corto = "";
    for (const p of palabras) {
      const sig = corto ? `${corto} ${p}` : p;
      if (sig.length > tope) break;
      corto = sig;
    }
    // Jamás trocear a media palabra: la primera entra entera aunque pase.
    if (!corto) corto = palabras[0];
    return corto.length < titulo.length ? `${corto}…` : titulo;
  }

  // Los pesos RENEGOCIADOS (Kalorica exagerado): con n grandes a la vez,
  // cada grande pesa max(3, 7−n): elegir una pieza la hace crecer ×6.
  let seleccionada = $state<string | null>(null);
  function pesoDe(item: ItemCola): number {
    const suenaGrande = idQueSuena && trabajando;
    const grandes =
      (seleccionada ? 1 : 0) + (suenaGrande && idQueSuena !== seleccionada ? 1 : 0);
    const pesoGrande = Math.max(3, 7 - grandes);
    if (item.id === seleccionada) return pesoGrande;
    // Solo mientras la voz TRABAJA la pieza es gigante; en pausa se queda
    // notable pero encogible (adiós al zombi expandido).
    if (item.id === idQueSuena) return trabajando ? pesoGrande : 2.2;
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
    { id: MARCA_ID, peso: 2.4, letras: 5 },
    {
      id: BOCA_ID,
      peso: bocaAbierta ? 26 : visibles.length === 0 ? 2.4 : 1.0,
      letras: 2,
    },
    ...visibles.map((i) => ({ id: i.id, peso: pesoDe(i), letras: Math.min(30, i.titulo.length) })),
  ]);
  const tablero = $derived(empaquetar(piezasTablero, anchoTablero));

  // Los datos del membrete: el susurro mono bajo la marca.
  const minutosTotales = $derived(visibles.reduce((s, i) => s + minutosDe(i), 0));

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
      const el = document.querySelector<HTMLElement>(
        `[data-pieza="${CSS.escape(id)}"] .parche, [data-pieza="${CSS.escape(id)}"] .baldosa-cuerpo`,
      );
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
      // El bibliotecario ASIENTE: la pieza queda en su sitio nuevo.
      celebra = true;
      setTimeout(() => (celebra = false), 430);
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
                <!-- El susurro del membrete: datos de verdad, en mono. -->
                <span class="membrete-datos">{visibles.length} {$t("cinta.piezas")} · {minutosTotales} {$t("cinta.min")}</span>
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
            <div class="baldosa-cuerpo {bocaAbierta ? 'boca-baldosa abierta' : 'boca-suelta'}" style={bocaAbierta ? `border-radius: ${radiosDe(BOCA_ID)};` : ""}>
              {#if !bocaAbierta}
                <!-- EL BUZÓN: un sello de correos crema con la ranura y el
                     ＋ garabateados a mano. Echar algo al correo del loro. -->
                <span class="capa parche-sombra" style="clip-path: {aPoligono(SELLO_BOCA)}"></span>
                <span class="capa boca-sello" style="clip-path: {aPoligono(SELLO_BOCA)}"></span>
                <svg class="costura" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
                  <polygon points={aPuntosSvg(SELLO_BOCA, 0.88)} />
                </svg>
                <button class="boca-toque" in:llega use:presionable={{ hap: "medium" }} onclick={() => (bocaAbierta = true)} aria-label={$t("escuchar.anadir")}>
                  <svg class="buzon" viewBox="0 0 44 36" aria-hidden="true">
                    <path d="M22 6 q0.7 5.2 -0.2 10.5 M16.2 11.4 q5.8 0.9 11.6 -0.3" />
                    <path d="M8 26 q14 5.5 28 -1.2" />
                  </svg>
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
            {@const tq = troquelesVivos.get(item.id) ?? troquelBase(item.id)}
            {@const clipExt = aPoligono(tq)}
            {@const clipInt = aPoligono(tq, 0.93)}
            {@const vw = (b.ancho * tq.ventana.w) / 100}
            {@const vh = (b.alto * tq.ventana.h) / 100 - 20}
            {@const enMarcha = esLaQueSuena && trabajando}
            {@const grandota = elegida || b.peso >= 3}
            {@const lineasCartel = enMarcha ? [] : cartel(tituloEnVentana(item.titulo, vw, vh, grandota), vw, Math.max(24, vh), $idiomaUI)}
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
                class="parche estado-{item.estado}"
                class:sacude={sacudida === item.id}
                class:suena={esLaQueSuena}
                class:elegida
                style="transform: translateX({arrastre?.id === item.id ? arrastre.dx : 0}px) rotate({arrastre?.id === item.id ? arrastre.dx / 26 : tiltDe(item.id)}deg); opacity: {arrastre?.id === item.id ? Math.max(0.25, 1 - Math.abs(arrastre.dx) / 340) : 1}; transition: {arrastre?.id === item.id ? 'none' : 'transform 0.3s cubic-bezier(0.34, 1.56, 0.64, 1)'};"
              >
                <!-- LA PEGATINA: sombra dura de contacto, borde de troquel
                     crema (dorado si está completada), cuerpo de tinta y la
                     costura de puntadas. Todas las capas comparten las 48
                     anclas: marcar favorito FUNDE la forma en corazón. -->
                <span class="capa parche-sombra" style="clip-path: {clipExt}"></span>
                <span class="capa parche-borde" class:dorado={pct >= 100 && !esLaQueSuena} style="clip-path: {clipExt}"></span>
                <span class="capa parche-cuerpo cuerpo-{item.estado}" style="clip-path: {clipInt}; {item.estado === 'listo' || esLaQueSuena ? `background: ${tinta};` : ''}">
                  {#if pct > 0 && (item.estado === "listo" || esLaQueSuena)}
                    <span class="marea" style="height: {pct}%; background: {honda}"></span>
                  {/if}
                </span>
                <svg class="costura" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
                  <polygon points={aPuntosSvg(tq, 0.87)} />
                </svg>
                <!-- LA VENTANA: la zona segura del troquel donde vive el
                     titular (o el teletipo) y su pastilla. -->
                <div class="ventana" style="left: {tq.ventana.x}%; top: {tq.ventana.y}%; width: {tq.ventana.w}%; height: {tq.ventana.h}%;">
                  {#if enMarcha}
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
                </div>
                <button class="baldosa-toque" use:presionable onclick={() => alternarSeleccion(item)} aria-label={item.titulo}></button>
                {#if elegida}
                  <!-- Las acciones, con los iconos DIBUJADOS de la casa. -->
                  <div class="acciones" in:llega={{ delay: 60 }}>
                    <button class="accion principal" use:presionable={{ hap: "rigid" }} style="color: {tinta}"
                      onclick={(e) => { e.stopPropagation(); abrirItem(item); }}>
                      <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor" aria-hidden="true"><path d="M8.2 5.2 Q9 4.4 10.1 5.1 L18.7 11 Q19.7 12 18.6 12.9 L10.2 18.9 Q9 19.6 8.5 18.4 Q7.5 12 8.2 5.2 Z"/></svg>
                      {$t("cola.escuchar")}
                    </button>
                    <button class="accion" use:presionable={{ hap: "soft" }} style="color: {tinta}" aria-label={item.favorito ? $t("pieza.quitar_favorito") : $t("pieza.favorito")}
                      onclick={(e) => { e.stopPropagation(); favoritoDirecto(item.id); }}>
                      <svg viewBox="0 0 24 24" width="19" height="19" fill={item.favorito ? "currentColor" : "none"} stroke="currentColor" stroke-width="2.1" stroke-linejoin="round" aria-hidden="true"><path d="M12 19.4 Q5.4 14.8 4.7 10 Q4.5 6.6 7.5 5.8 Q10.1 5.3 12 8.1 Q13.9 5.2 16.6 5.8 Q19.5 6.7 19.2 10.1 Q18.5 15 12 19.4 Z"/></svg>
                    </button>
                    <button class="accion" use:presionable={{ hap: "soft" }} style="color: {tinta}" aria-label={$t("pieza.renombrar")}
                      onclick={(e) => { e.stopPropagation(); menuPieza = item; renombrando = true; nuevoNombre = item.titulo; }}>
                      <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M16.6 3.6 q2.5 -1.5 3.9 0.3 q1.3 1.7 -0.7 3.5 L8.5 18.5 l-4.7 1.7 q-0.7 0.2 -0.5 -0.5 l1.6 -4.6 Z"/></svg>
                    </button>
                    <button class="accion" use:presionable={{ hap: "warning" }} style="color: {tinta}" aria-label={$t("cinta.borrar")}
                      onclick={(e) => { e.stopPropagation(); seleccionada = null; esconderYProgramar(item); }}>
                      <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" aria-hidden="true"><path d="M3.6 6.2 q8.4 -1 16.8 0 M8.3 6 q-0.2 -2.6 1.2 -2.9 q2.5 -0.5 5 0 q1.4 0.3 1.2 2.9 M6 6.4 q0.2 7.6 0.8 12.4 q0.1 1.6 1.7 1.8 q3.5 0.5 7 0 q1.6 -0.2 1.7 -1.8 q0.6 -4.8 0.8 -12.4"/></svg>
                    </button>
                  </div>
                {/if}
              </div>
            </article>
          {/if}
        {/if}
      {/each}
    </div>

    <!-- EL COLOFÓN: la página tiene final, como lo impreso. -->
    {#if visibles.length > 0}
      <footer class="colofon" aria-hidden="true">
        <svg viewBox="0 0 100 7" preserveAspectRatio="none" class="colofon-raya"><line x1="0" y1="6" x2="100" y2="1" /></svg>
        <span>yappy · {$t("ajustes.amor").toLocaleLowerCase()}</span>
      </footer>
    {/if}

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
            <span class="disco" aria-hidden="true" style="--tinta: {PALETA[b.name.length % PALETA.length]}">
              <i class="disco-cuerpo"></i><i class="disco-agujero"></i>
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

  <!-- LA TRASTIENDA: una etiqueta mono girada que corta el borde derecho
       de la página, como las etiquetas de los márgenes de un taller. -->
  <button
    class="etiqueta-trastienda"
    use:presionable={{ hap: "soft" }}
    onclick={() => { engranajeGira = true; setTimeout(() => goto("/ajustes"), 240); }}
  >
    <span class="engranaje" class:gira={engranajeGira} aria-hidden="true">
      <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3.4"/><path d="M12 2.8 l0.6 3.1 M12 21.2 l-0.5 -3 M21.2 12 l-3.1 0.5 M2.8 12 l3.1 -0.4 M18.5 5.6 l-2.2 2.1 M5.5 18.4 l2.2 -2 M18.4 18.5 l-2.1 -2.2 M5.6 5.5 l2.1 2.2"/></svg>
    </span>
    <span class="etiqueta-texto">{$t("cinta.trastienda")}</span>
  </button>

  <!-- El pájaro en vuelo: de la boca a la pieza recién llegada, en arco. -->
  {#if pajaro}
    <span class="pajaro-vuelo" style="left: {pajaro.x}px; top: {pajaro.y}px; transform: translate(-50%, -50%) scaleX({pajaro.girado})" aria-hidden="true">
      <Criatura size={30} estado="celebrando" tinta={$tintaVoz} />
    </span>
  {/if}

  <!-- El deshacer: cinco segundos de gracia tras lanzar una pieza. -->
  {#if deshacer}
    <div class="deshacer-aviso" in:fly={{ y: 26, duration: 260 }} out:fly={{ y: 26, duration: 200 }}>
      <span class="deshacer-loro" aria-hidden="true"><Criatura size={34} mirando={-1} tinta={$tintaVoz} /></span>
      <span class="deshacer-texto">{$t("cinta.fuera")}</span>
      <button class="deshacer-tecla" use:presionable={{ hap: "medium" }} onclick={deshacerBorrado}>{$t("comun.deshacer")}</button>
    </div>
  {/if}

  {#if menuPieza}
    <div class="velo" role="presentation" onclick={() => { menuPieza = null; renombrando = false; }}></div>
    <div class="hoja">
      <div class="hoja-asa"></div>
      <div class="hoja-cabeza">{$t("pieza.opciones")}</div>
      <svg class="ficha-forma" viewBox="0 0 100 100" aria-hidden="true">
        <polygon points={aPuntosSvg(troquelesVivos.get(menuPieza.id) ?? troquelBase(menuPieza.id))} fill={tintaDe(menuPieza)} />
      </svg>
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
          <svg viewBox="0 0 24 24" width="20" height="20" fill={menuPieza.favorito ? "var(--yap-voz, #e0502a)" : "none"} stroke="currentColor" stroke-width="1.9" stroke-linejoin="round" aria-hidden="true"><path d="M12 19.4 Q5.4 14.8 4.7 10 Q4.5 6.6 7.5 5.8 Q10.1 5.3 12 8.1 Q13.9 5.2 16.6 5.8 Q19.5 6.7 19.2 10.1 Q18.5 15 12 19.4 Z"/></svg>
          {menuPieza.favorito ? $t("pieza.quitar_favorito") : $t("pieza.favorito")}
        </button>
        <button class="tecla-menu" use:presionable onclick={() => { renombrando = true; nuevoNombre = menuPieza?.titulo ?? ""; }}>
          <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M16.6 3.6 q2.5 -1.5 3.9 0.3 q1.3 1.7 -0.7 3.5 L8.5 18.5 l-4.7 1.7 q-0.7 0.2 -0.5 -0.5 l1.6 -4.6 Z"/></svg>
          {$t("pieza.renombrar")}
        </button>
        <button class="tecla-menu peligro" use:presionable onclick={borrarPieza}>
          <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" aria-hidden="true"><path d="M3.6 6.2 q8.4 -1 16.8 0 M8.3 6 q-0.2 -2.6 1.2 -2.9 q2.5 -0.5 5 0 q1.4 0.3 1.2 2.9 M6 6.4 q0.2 7.6 0.8 12.4 q0.1 1.6 1.7 1.8 q3.5 0.5 7 0 q1.6 -0.2 1.7 -1.8 q0.6 -4.8 0.8 -12.4"/></svg>
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
  .marca-baldosa {
    background: var(--yap-superficie);
    border: 1.5px solid var(--yap-borde);
  }
  .membrete-datos {
    font-family: var(--yap-mono, ui-monospace, monospace);
    font-size: 10px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--yap-tinta-suave, #82755a);
    margin: 2px 2px 0 0;
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
    flex-direction: column;
    align-items: flex-end;
    justify-content: flex-end;
    gap: 1px;
    padding: 0 12px 6px 0;
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
  /* La etiqueta de la trastienda: mono girada, cortando el borde. */
  .etiqueta-trastienda {
    position: fixed;
    right: 0;
    top: 24%;
    z-index: 30;
    display: inline-flex;
    flex-direction: column;
    align-items: center;
    gap: 7px;
    width: 34px;
    padding: 11px 0 12px;
    border: 1.5px solid var(--yap-tinta, #2b2418);
    border-right: 0;
    border-radius: 10px 0 0 10px;
    background: var(--yap-superficie);
    color: var(--yap-tinta);
    box-shadow: -2.5px 3px 0 #ded7c2;
    cursor: pointer;
  }
  .etiqueta-trastienda .engranaje {
    writing-mode: horizontal-tb;
  }
  .etiqueta-texto {
    writing-mode: vertical-rl;
    font-family: var(--yap-mono, ui-monospace, monospace);
    font-size: 9.5px;
    font-weight: 700;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    line-height: 1;
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
    color: #f7f2e7;
    overflow: hidden;
    z-index: 1;
  }
  .baldosa.en-vuelo .parche {
    filter: brightness(1.07);
  }

  /* ── LA PEGATINA: las capas del troquel ── */
  .parche {
    position: relative;
    width: 100%;
    height: 100%;
  }
  .capa {
    position: absolute;
    inset: 0;
    /* El MORPH: mismas 48 anclas en todas las formas, así que el clip
       interpola solo (favorito funde a corazón, completada a estrella). */
    transition:
      clip-path 0.65s cubic-bezier(0.18, 1.5, 0.32, 1),
      background-color 0.45s ease;
  }
  .parche-sombra {
    background: #ded7c2;
    transform: translate(2.5px, 3.5px);
  }
  .parche-borde {
    background: #f7f2e7;
  }
  .parche-borde.dorado {
    background: var(--yap-dorado, #e8b41a);
  }
  .parche-cuerpo {
    overflow: hidden;
  }
  /* El material cede al presionar. (:global porque .pulsado llega en
     runtime y Svelte podaría el selector.) */
  :global(.parche:has(.pulsado) .parche-cuerpo) {
    transform: scale(0.965);
    transition: transform 0.14s ease;
  }
  /* La que suena RESPIRA (pulso sutil del cuerpo dentro del troquel). */
  .parche.suena .parche-cuerpo {
    animation: respira-parche 2.8s ease-in-out infinite;
  }
  @keyframes respira-parche {
    0%, 100% { transform: scale(1); }
    50% { transform: scale(1.02); }
  }
  .costura {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
    z-index: 3;
  }
  .costura polygon {
    fill: none;
    stroke: rgba(43, 36, 24, 0.3);
    stroke-width: 1.5;
    stroke-dasharray: 5 4;
    vector-effect: non-scaling-stroke;
  }
  /* LA VENTANA: la zona segura del troquel. */
  .ventana {
    position: absolute;
    z-index: 4;
    pointer-events: none;
    display: flex;
    flex-direction: column;
    padding-bottom: 20px;
  }
  /* Modo noche: todas duermen salvo la que suena. */
  :global(html[data-theme="dark"]) .parche:not(.suena) .parche-cuerpo,
  :global(html[data-theme="dark"]) .baldosa-cuerpo:not(.suena) {
    filter: brightness(0.52) saturate(0.6);
  }

  /* EL CARTEL vive dentro de la ventana. */
  .cartel {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    pointer-events: none;
  }
  /* Elegir tiene su POP además del FLIP: el sí se siente. */
  .parche.elegida {
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
    fill: #f7f2e7;
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
    flex: 1;
    min-height: 0;
    overflow: hidden;
    display: grid;
    pointer-events: none;
  }
  .teletipo p {
    grid-area: 1 / 1;
    margin: 0;
    align-self: center;
    color: #f7f2e7;
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
     el cartel: es un objeto encima, al pie de la ventana del troquel. */
  .pastilla {
    position: absolute;
    left: 50%;
    bottom: 0;
    transform: translateX(-50%);
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 3px 9px;
    border-radius: 999px;
    font-family: var(--yap-mono, ui-monospace, monospace);
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.05em;
    color: #f7f2e7;
    white-space: nowrap;
    max-width: 100%;
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
    background: #f7f2e7;
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
  /* El latido de la onda: las vecinas responden al toque. (:global porque
     .late se añade en runtime.) */
  :global(.parche.late),
  :global(.baldosa-cuerpo.late) {
    animation: late 0.3s ease;
  }
  @keyframes late {
    35% { transform: scale(0.972); }
  }

  .asomado-pieza {
    position: absolute;
    top: -15px;
    left: 12%;
    z-index: 0;
    pointer-events: none;
  }

  /* Los estados de cocina y error: rayas y rojo, planos. */
  .parche-cuerpo.cuerpo-pendiente,
  .parche-cuerpo.cuerpo-preparando {
    background: repeating-linear-gradient(-45deg, #8e8574 0 12px, #7c7466 12px 22px);
    background-size: 200% 100%;
    animation: cocinando 2.4s linear infinite;
  }
  @keyframes cocinando {
    to { background-position: -62px 0, 0 0; }
  }
  .parche-cuerpo.cuerpo-error {
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

  /* ── La boca: el sello de correos con el buzón dibujado ── */
  .boca-baldosa {
    background: var(--yap-superficie);
    border: 1.5px solid var(--yap-borde);
    color: var(--yap-tinta-suave, #82755a);
  }
  .boca-suelta {
    background: transparent;
    border: 0;
    color: var(--yap-tinta-suave, #82755a);
    overflow: visible;
  }
  .boca-sello {
    background: #f7f2e7;
  }
  .boca-toque {
    position: relative;
    z-index: 5;
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 2px;
    border: 0;
    background: transparent;
    color: inherit;
    cursor: pointer;
    padding: 0;
  }
  .buzon {
    width: 44px;
    height: 36px;
    animation: cruz-respira 5s ease-in-out infinite;
  }
  .buzon path {
    fill: none;
    stroke: currentColor;
    stroke-width: 2.6;
    stroke-linecap: round;
    animation: cruz-arcoiris 14s linear infinite;
  }
  @keyframes cruz-respira {
    0%, 86%, 100% { transform: scale(1) rotate(0deg); }
    90% { transform: scale(1.24, 0.82) rotate(-5deg); }
    95% { transform: scale(0.92, 1.12) rotate(3deg); }
  }
  /* El garabato recorre la paleta: la invitación de color. */
  @keyframes cruz-arcoiris {
    0% { stroke: #ff6b6b; }
    20% { stroke: #f94892; }
    40% { stroke: #5d5fef; }
    60% { stroke: #00a896; }
    80% { stroke: #ff8e3c; }
    100% { stroke: #ff6b6b; }
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
    box-shadow: 2.5px 3px 0 #ded7c2;
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
    border-radius: 14px;
    border: 1.5px solid var(--yap-tinta, #2b2418);
    background: var(--yap-papel, #f4f1ea);
    color: var(--yap-tinta, #2b2418);
    transform: rotate(-0.8deg);
    box-shadow: 3px 4px 0 #ded7c2;
  }
  .deshacer-loro {
    position: absolute;
    top: -26px;
    right: 22px;
    pointer-events: none;
  }
  .deshacer-texto {
    font-weight: 700;
    font-size: 14px;
  }
  .deshacer-tecla {
    border: 1.5px solid var(--yap-tinta, #2b2418);
    background: var(--vivo, var(--acento-voz, #e0502a));
    color: #fff6ef;
    font-weight: 800;
    font-size: 14px;
    padding: 8px 14px;
    border-radius: 10px;
    box-shadow: 2px 2.5px 0 #ded7c2;
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
  .ficha-forma {
    width: 44px;
    height: 44px;
    margin: 0 auto;
    transform: rotate(-4deg);
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
    box-shadow: 2.5px 3px 0 #ded7c2;
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
    border: 1.5px solid var(--yap-borde);
    box-shadow: 3px 4px 0 #ded7c2;
    overflow: hidden;
  }
  .disco {
    position: relative;
    width: 46px;
    height: 46px;
    border-radius: 50%;
    background: #f7f2e7;
    box-shadow: 2px 2.5px 0 #ded7c2;
    flex-shrink: 0;
  }
  .disco-cuerpo {
    position: absolute;
    inset: 4px;
    border-radius: 50%;
    background: var(--tinta, #5d5fef);
    outline: 1.5px dashed rgba(43, 36, 24, 0.3);
    outline-offset: -6px;
  }
  .disco-agujero {
    position: absolute;
    inset: 18px;
    border-radius: 50%;
    background: #f7f2e7;
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
    background: #f7f2e7;
    animation: onda-mini 0.9s ease-in-out infinite;
  }
  .mini-ondas i:nth-child(1) { height: 45%; }
  .mini-ondas i:nth-child(2) { height: 100%; animation-delay: 0.15s; }
  .mini-ondas i:nth-child(3) { height: 70%; animation-delay: 0.3s; }
  @keyframes onda-mini {
    0%, 100% { transform: scaleY(0.5); }
    50% { transform: scaleY(1); }
  }

  /* ── El colofón: la página tiene final ── */
  .colofon {
    margin: 16px 15px 4px;
    display: flex;
    flex-direction: column;
    gap: 7px;
    align-items: center;
    flex-shrink: 0;
  }
  .colofon-raya {
    width: 62%;
    height: 7px;
  }
  .colofon-raya line {
    stroke: var(--yap-voz, #e0502a);
    stroke-width: 1.6;
    stroke-dasharray: 7 6;
  }
  .colofon span {
    font-family: var(--yap-mono, ui-monospace, monospace);
    font-size: 9.5px;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--yap-tinta-suave, #82755a);
    text-align: center;
  }

  .rotulo-tramo {
    margin: 10px 15px 0;
    font-family: var(--yap-mono, ui-monospace, monospace);
    font-size: 11px;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--yap-tinta-suave);
  }
  
  /* ── El modelo ── */
  .modelo {
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    background: var(--yap-superficie);
    border: 1.5px solid var(--yap-borde);
    box-shadow: 3px 4px 0 #ded7c2;
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
