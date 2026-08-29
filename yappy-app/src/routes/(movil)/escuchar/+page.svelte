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
  import { scale, fade, fly } from "svelte/transition";
  import { goto } from "$app/navigation";
  import { t, idiomaUI } from "$lib/i18n";
  import { get as getStore } from "svelte/store";
  import { haptic } from "$lib/haptic";
  import { pref } from "$lib/pref.svelte";
  import { plop, pop, tick as foleyTick, rasga, boing, vuelo as foleyVuelo } from "$lib/foley";
  import { presionable } from "$lib/presionable";
  import Criatura from "$lib/Criatura.svelte";
  import Trastienda from "$lib/Trastienda.svelte";
  import IconoTipo from "$lib/IconoTipo.svelte";
  import { reader } from "$lib/readerStore.svelte";
  import { progresoDe } from "$lib/progreso";
  import { tintaVoz, TINTAS_VOZ } from "$lib/voces";
  import { repro } from "$lib/reproduccion.svelte";
  import { empaquetar, type Baldosa } from "$lib/mosaico";
  import { cartel, cartelReposo, VB_ALTO, VB_BASE } from "$lib/cartel";
  import {
    SERENO,
    PALETA,
    colorDe,
    indiceDe,
    tonoHondo,
    radiosDe,
    tiltDe,
    jitterDe,
  } from "$lib/juguete";
  import {
    TROQUELES_BASE,
    CORAZON,
    ESTRELLA,
    troquelBase,
    troquelPara,
    FICHA,
    CORAZON_ANCHO,
    ESTRELLA_ANCHA,
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
    colaReintentarArchivo,
    getSettings,
    libraryImportYappy,
    onColaActualizada,
    stopPlayback,
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
  function vuelveAlSello(_n: Element) {
    return {
      duration: 200,
      easing: cubicOut,
      css: (t: number, u: number) =>
        `transform: translate(${u * 26}%, ${u * 18}%) scale(${1 - u * 0.55}); opacity: ${t}; transform-origin: 92% 100%;`,
    };
  }
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
  // EL GIVE de la pegatina-marca: si alguien tira de ella, cede un poco
  // con resistencia y vuelve con muelle, plop incluido.
  let marcaTira = $state<{ x0: number; y0: number; dx: number; dy: number } | null>(null);
  let marcaFueTiron = false;
  function marcaDown(e: PointerEvent) {
    marcaTira = { x0: e.clientX, y0: e.clientY, dx: 0, dy: 0 };
    marcaFueTiron = false;
  }
  function marcaMove(e: PointerEvent) {
    if (!marcaTira) return;
    const dx = (e.clientX - marcaTira.x0) * 0.32;
    const dy = (e.clientY - marcaTira.y0) * 0.32;
    marcaTira = {
      ...marcaTira,
      dx: Math.max(-16, Math.min(16, dx)),
      dy: Math.max(-14, Math.min(14, dy)),
    };
    if (Math.abs(dx) > 4 || Math.abs(dy) > 4) {
      if (!marcaFueTiron) haptic("tick");
      marcaFueTiron = true;
    }
  }
  function marcaUp() {
    if (!marcaTira) return;
    const fue = marcaFueTiron;
    marcaTira = null;
    if (fue) {
      haptic("soft");
      plop();
    }
  }
  function tocarMarca() {
    if (marcaFueTiron) return;
    const ahora = performance.now();
    if (ahora - ultimoTapMarca < 360) {
      // Doble pulsación: la voltereta entera del loro.
      haptic("heavy");
      voltereta = true;
      setTimeout(() => (voltereta = false), 900);
    }
    ultimoTapMarca = ahora;
    haptic("success");
    boing();
    celebra = true;
    marcaGelatina = true;
    setTimeout(() => {
      celebra = false;
      marcaGelatina = false;
    }, 950);
  }

  onMount(async () => {
    getSettings()
      .then((s) => (pref.dosColumnas = s.dos_columnas))
      .catch(() => {});
    try {
      etiquetaTop = parseFloat(localStorage.getItem("yappy.etiqueta.top") ?? "") || 0;
    } catch {}
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
    setTimeout(() => (trastiendaLista = true), 2600);
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
      if (Math.random() < 0.5 || !listos.length) {
        // Desde un borde de la pantalla: medio cuerpo y a esconderse.
        const lados = ["izq", "der", "abajo"] as const;
        const lado = lados[Math.floor(Math.random() * lados.length)];
        asomadoBorde = {
          lado,
          pos: 16 + Math.floor(Math.random() * 52),
          tinta: TINTAS_VOZ[Math.floor(Math.random() * TINTAS_VOZ.length)],
        };
        // El jefe gira la mirada hacia su pájaro mientras dura la asomada.
        mirada = {
          x: lado === "izq" ? -0.9 : lado === "der" ? 0.9 : 0,
          y: lado === "abajo" ? 0.9 : 0.15,
        };
        setTimeout(() => (asomadoBorde = null), 2800);
      } else {
        asomadoEn = listos[Math.floor(Math.random() * listos.length)].id;
        setTimeout(() => (asomadoEn = null), 2600);
      }
    }, 26000);
    cleanups.push(() => clearInterval(fisgon));
    // La marca respira sola: la ola espontánea, de tarde en tarde y solo
    // en silencio (sin háptica: nadie la ha tocado).
    const olaSola = setInterval(() => {
      if (document.hidden || marcaGelatina) return;
      if (Math.random() < 0.6) {
        marcaGelatina = true;
        setTimeout(() => (marcaGelatina = false), 980);
      }
    }, 34000);
    cleanups.push(() => clearInterval(olaSola));
    const pasitos = setInterval(() => {
      if (sonando || document.hidden || jefePicotea) return;
      jefePos = { x: Math.floor(Math.random() * 52) - 10, y: 0 };
    }, 47000);
    cleanups.push(() => clearInterval(pasitos));
    const expedicion = setInterval(() => {
      if (Math.random() < 0.45) elJefeReorganiza();
      else pasea();
    }, 75000);
    cleanups.push(() => clearInterval(expedicion));
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
    // SIN inflar: el 100% (la estrella dorada) es solo para lo terminado
    // de verdad (parrafo == total, que se escribe al fin natural). El «+1»
    // doraba media cinta con solo abrir un documento.
    return Math.min(100, Math.round((p.parrafo / p.total) * 100));
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
      // Abrir una pieza DISTINTA de la que suena MATA la sesión anterior
      // (guardando su marea): una sola verdad sonando, jamás dos.
      if (repro.snap && repro.snap.estado !== "inactivo" && repro.snap.doc_path && repro.snap.doc_path !== item.ruta) {
        await stopPlayback().catch(() => {});
      }
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
    // TODO gris. El backend ya sabe decir «no puedo con esto». Y el error
    // REAL al log: un fallo silencioso aquí escondió el bug una ronda.
    try {
      const ruta = await abrirDialogo({ multiple: false });
      logToBackend("info", "picker", `elegido: ${JSON.stringify(ruta)}`);
      if (typeof ruta === "string" && ruta) {
        if (ruta.toLowerCase().endsWith(".yappy")) {
          // Un audiolibro de la casa: directo a la biblioteca.
          await libraryImportYappy(ruta);
          logToBackend("info", "picker", "yappy importado");
          bobinas = ((await invoke("list_rendered_audiobooks_cmd").catch(() => [])) as Bobina[]) ?? [];
        } else {
          await colaAgregarArchivo(ruta);
          logToBackend("info", "picker", "encolado");
        }
      }
    } catch (e) {
      logToBackend("error", "picker", `fallo del selector: ${e}`);
    }
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
  let aterrizando: string | null = null;
  let recogida = $state(false);
  let vuelo = $state({ dx: 0, dy: 0, estira: 1, giro: 0, vx: 0, vy: 0 });
  let seMovioEnVuelo = false;
  let vueloPrevio = { x: 0, y: 0, t: 0 };
  let asomadoEn = $state<string | null>(null);
  let asomadoBorde = $state<null | { lado: "izq" | "der" | "abajo"; pos: number; tinta: string }>(null);
  // El jefe se mueve: pasitos ociosos y expediciones de reorganización.
  let jefePos = $state({ x: 0, y: 0 });
  let jefePicotea = $state(false);
  let jefeVolando = $state(false);
  let jefeGiro = $state(1);
  let jefeAngulo = $state(0);
  let jefeAterriza = $state(false);
  let plumas = $state<{ x: number; y: number }[]>([]);

  /// Tocar al jefe: DESPEGA, da una vuelta amplia por la pantalla con dos
  /// puntos de control al azar, y aterriza en su percha.
  function vuelaJefe() {
    if (jefeVolando) return;
    haptic("medium");
    jefeVolando = true;
    const w = window.innerWidth;
    const h = window.innerHeight;
    // CADA VUELO ES OTRO: cinco formas (picado de caza, rasante, doble
    // valle, tirabuzón, gaviota) con parámetros al azar: nunca repite.
    const variante = Math.floor(Math.random() * 5);
    const fondo = {
      x: w * (0.45 + Math.random() * 0.42),
      y: h - 160 - Math.random() * 120,
    };
    const cielo = { x: w * (0.14 + Math.random() * 0.2), y: -40 + Math.random() * 30 };
    const fondo2 = {
      x: w * (0.14 + Math.random() * 0.3),
      y: h - 200 - Math.random() * 140,
    };
    const t0 = performance.now();
    const dur =
      variante === 2 ? 2600 : variante === 3 ? 2400 : variante === 4 ? 2900 : 2050;
    foleyVuelo(dur);
    let xPrevio = 0;
    let yPrevio = 0;
    const sacudidasVuelo = new Map<string, number>();
    const jefeEl = document.querySelector<HTMLElement>(".loro-jefe");
    const base = jefeEl?.getBoundingClientRect();
    const bez = (a: { x: number; y: number }, c: { x: number; y: number }, b: { x: number; y: number }, e: number) => {
      const inv = 1 - e;
      return {
        x: inv * inv * a.x + 2 * inv * e * c.x + e * e * b.x,
        y: inv * inv * a.y + 2 * inv * e * c.y + e * e * b.y,
      };
    };
    const origen = { x: 0, y: 0 };
    const punto = (u: number) => {
      if (variante === 1) {
        // RASANTE: baja rápido a media altura y cruza la pantalla a ras,
        // rozando la fila de pegatinas, y vuelve en arco alto.
        const ras = { x: w - 90, y: h * (0.42 + Math.random() * 0.001) + fondo.y * 0.12 };
        if (u < 0.5) {
          const v = u / 0.5;
          const e = v * v;
          return bez(origen, { x: w * 0.1, y: ras.y + 60 }, ras, e);
        }
        const v = (u - 0.5) / 0.5;
        const e = 1 - Math.pow(1 - v, 2.2);
        return bez(ras, { x: w * 0.5, y: -60 }, origen, e);
      }
      if (variante === 2) {
        // DOBLE VALLE: dos picados encadenados antes de volver.
        if (u < 0.4) {
          const v = u / 0.4;
          const e = v * v * v;
          return bez(origen, cielo, fondo, e);
        }
        if (u < 0.7) {
          const v = (u - 0.4) / 0.3;
          const e = v < 0.5 ? 2 * v * v : 1 - Math.pow(-2 * v + 2, 2) / 2;
          return bez(fondo, { x: (fondo.x + fondo2.x) / 2, y: h * 0.3 }, fondo2, e);
        }
        const v = (u - 0.7) / 0.3;
        const e = 1 - Math.pow(1 - v, 2.4);
        return bez(fondo2, { x: w * 0.06, y: h * 0.25 }, origen, e);
      }
      if (variante === 3) {
        // TIRABUZÓN: sube, hace un RIZO completo en el aire y baja.
        const centro = { x: w * (0.4 + Math.random() * 0.2), y: h * 0.34 };
        const radio = 70 + Math.random() * 40;
        if (u < 0.3) {
          const v = u / 0.3;
          const e = v * v;
          return bez(origen, { x: w * 0.1, y: h * 0.1 }, { x: centro.x, y: centro.y - radio }, e);
        }
        if (u < 0.72) {
          const v = (u - 0.3) / 0.42;
          const th = -Math.PI / 2 + v * Math.PI * 2;
          return { x: centro.x + radio * Math.cos(th), y: centro.y + radio * Math.sin(th) };
        }
        const v = (u - 0.72) / 0.28;
        const e = 1 - Math.pow(1 - v, 2.2);
        return bez({ x: centro.x, y: centro.y - radio }, { x: w * 0.08, y: h * 0.12 }, origen, e);
      }
      if (variante === 4) {
        // GAVIOTA: planeo en ese amplio, sin picado: puro vaivén.
        const lado = { x: w - 80, y: h * (0.3 + Math.random() * 0.15) };
        const bajo = { x: w * (0.2 + Math.random() * 0.2), y: h * (0.62 + Math.random() * 0.12) };
        if (u < 0.38) {
          const v = u / 0.38;
          const e = v < 0.5 ? 2 * v * v : 1 - Math.pow(-2 * v + 2, 2) / 2;
          return bez(origen, { x: w * 0.55, y: h * 0.05 }, lado, e);
        }
        if (u < 0.72) {
          const v = (u - 0.38) / 0.34;
          const e = v < 0.5 ? 2 * v * v : 1 - Math.pow(-2 * v + 2, 2) / 2;
          return bez(lado, { x: w * 0.75, y: h * 0.66 }, bajo, e);
        }
        const v = (u - 0.72) / 0.28;
        const e = 1 - Math.pow(1 - v, 2);
        return bez(bajo, { x: -30, y: h * 0.3 }, origen, e);
      }
      // PICADO DE CAZA clásico.
      if (u < 0.55) {
        const v = u / 0.55;
        const e = v * v * v;
        return bez(origen, cielo, fondo, e);
      }
      const v = (u - 0.55) / 0.45;
      const e = 1 - Math.pow(1 - v, 2.4);
      return bez(fondo, { x: w * 0.9, y: h * 0.3 }, origen, e);
    };
    const paso = (t: number) => {
      const u = Math.min(1, (t - t0) / dur);
      const { x, y } = punto(u);
      const dx = x - xPrevio;
      const dy = y - yPrevio;
      // El giro HONESTO: hacia la derecha se mira a la derecha (el dibujo
      // base con mirando=-1 ya mira derecha con scaleX(1)).
      if (Math.abs(dx) > 0.6) jefeGiro = dx >= 0 ? 1 : -1;
      // La INCLINACIÓN sigue la tangente DE VERDAD: en el picado el
      // cuerpo se pone casi vertical.
      const ang = Math.atan2(dy, Math.abs(dx) + 0.001) * (180 / Math.PI);
      jefeAngulo = Math.max(-64, Math.min(64, ang)) * (jefeGiro === 1 ? 1 : -1);
      xPrevio = x;
      yPrevio = y;
      jefePos = { x, y };
      if (base) {
        const px = base.left + 42 + x;
        const py = base.top + 42 + y;
        const ahora = performance.now();
        for (const it of visibles) {
          const ultimo = sacudidasVuelo.get(it.id) ?? 0;
          if (ahora - ultimo < 900) continue;
          const el = document.querySelector<HTMLElement>(`[data-pieza="${CSS.escape(it.id)}"]`);
          if (!el) continue;
          const r = el.getBoundingClientRect();
          const cx = r.left + r.width / 2;
          const cy = r.top + r.height / 2;
          if (Math.hypot(px - cx, py - cy) < Math.max(90, r.width * 0.45)) {
            sacudidasVuelo.set(it.id, ahora);
            haptic("tick");
            const cuerpo = el.querySelector<HTMLElement>(".baldosa-cuerpo");
            cuerpo?.classList.add("late");
            setTimeout(() => cuerpo?.classList.remove("late"), 300);
          }
        }
      }
      if (u < 1) {
        requestAnimationFrame(paso);
      } else {
        jefePos = { x: 0, y: 0 };
        jefeGiro = 1;
        jefeAngulo = 0;
        jefeVolando = false;
        jefeAterriza = true;
        plumas = [
          { x: 18 + Math.random() * 20, y: 30 },
          { x: 45 + Math.random() * 18, y: 26 },
        ];
        setTimeout(() => {
          jefeAterriza = false;
          plumas = [];
        }, 780);
        haptic("soft");
        brincoDeLoro();
      }
    };
    requestAnimationFrame(paso);
  }
  // El paseante: un loro que cruza la pantalla por DETRÁS de las
  // pegatinas, asomando por los huecos.
  let paseante = $state<null | { x: number; y: number; tinta: string; rumbo: 1 | -1 }>(null);

  // La etiqueta de la trastienda se ARRASTRA arriba y abajo (y recuerda
  // su sitio); el toque sin arrastre sigue abriendo la trastienda.
  let etiquetaTop = $state(0);
  let etqArr = $state<null | { y0: number; top0: number; movida: boolean }>(null);
  function etqDown(e: PointerEvent) {
    etqArr = { y0: e.clientY, top0: etiquetaTop || window.innerHeight * 0.24, movida: false };
    haptic("tick");
  }
  let ultimoDetent = 0;
  function etqMove(e: PointerEvent) {
    if (!etqArr) return;
    const dy = e.clientY - etqArr.y0;
    if (Math.abs(dy) > 7 && !etqArr.movida) {
      etqArr.movida = true;
      (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
    }
    const min = window.innerHeight * 0.1;
    const max = window.innerHeight * 0.72;
    etiquetaTop = Math.min(max, Math.max(min, etqArr.top0 + dy));
    // La CORONA: un detent háptico cada 14px de recorrido, como la
    // corona del reloj: se SIENTE cuánto has girado.
    const detent = Math.round(etiquetaTop / 14);
    if (detent !== ultimoDetent) {
      ultimoDetent = detent;
      haptic("tick");
      foleyTick();
    }
  }
  // El click sintético (accesibilidad, automatización) llega DESPUÉS del
  // pointerup: esta marca evita el doble toggle por ambos caminos.
  let toggleReciente = 0;
  function etqToggle() {
    haptic("soft");
    rasga();
    engranajeGira = !trastiendaAbierta;
    trastiendaAbierta = !trastiendaAbierta;
    trastiendaLista = true;
    toggleReciente = performance.now();
  }
  function etqClick() {
    if (performance.now() - toggleReciente < 500) return;
    etqToggle();
  }
  function etqUp() {
    if (!etqArr) return;
    const fueArrastre = etqArr.movida;
    etqArr = null;
    if (fueArrastre) {
      haptic("soft");
      try {
        localStorage.setItem("yappy.etiqueta.top", String(Math.round(etiquetaTop)));
      } catch {}
    } else {
      etqToggle();
    }
  }
  let trastiendaAbierta = $state(false);
  let trastiendaLista = $state(false);

  /// El jefe vuela hasta una pieza al azar, la picotea y la INTERCAMBIA
  /// con su vecina (reorganiza de verdad, con persistencia). Solo en
  /// silencio y de tarde en tarde: es su trabajo, no un estorbo.
  async function elJefeReorganiza() {
    if (sonando || levantada || bocaAbierta || visibles.length < 3) return;
    const i = Math.floor(Math.random() * (visibles.length - 1));
    const pieza = visibles[i];
    const el = document.querySelector<HTMLElement>(`[data-pieza="${CSS.escape(pieza.id)}"]`);
    const jefeEl = document.querySelector<HTMLElement>(".loro-jefe");
    if (!el || !jefeEl) return;
    const r = el.getBoundingClientRect();
    const j = jefeEl.getBoundingClientRect();
    jefePos = { x: r.left + r.width / 2 - (j.left - jefePos.x) - 42, y: r.top - (j.top - jefePos.y) - 30 };
    setTimeout(() => {
      jefePicotea = true;
      haptic("tick");
      setTimeout(() => (jefePicotea = false), 520);
      // El intercambio: la pieza y su vecina se cambian el sitio.
      const idxA = items.findIndex((it) => it.id === pieza.id);
      const vecina = visibles[i + 1];
      const idxB = items.findIndex((it) => it.id === vecina?.id);
      if (idxA >= 0 && idxB >= 0) {
        const copia = [...items];
        [copia[idxA], copia[idxB]] = [copia[idxB], copia[idxA]];
        items = copia;
        colaReordenar(pieza.id, idxB).catch(() => {});
      }
      setTimeout(() => (jefePos = { x: 0, y: 0 }), 700);
    }, 950);
  }

  /// El paseo: cruza por un hueco entre filas, detrás de las pegatinas.
  function pasea() {
    if (sonando || paseante || document.hidden) return;
    const filas = [...new Set([...tablero.baldosas.values()].map((bb) => bb.y))].sort((a, b) => a - b);
    if (filas.length < 2) return;
    const y = filas[1 + Math.floor(Math.random() * (filas.length - 1))] - 26;
    const rumbo = Math.random() < 0.5 ? 1 : -1;
    const tinta = TINTAS_VOZ[Math.floor(Math.random() * TINTAS_VOZ.length)];
    const ancho = anchoTablero || 400;
    let x = rumbo === 1 ? -60 : ancho + 60;
    paseante = { x, y, tinta, rumbo };
    const t0 = performance.now();
    const dur = 8500;
    const paso = (t: number) => {
      const u = (t - t0) / dur;
      if (u >= 1 || !paseante) {
        paseante = null;
        return;
      }
      x = rumbo === 1 ? -60 + u * (ancho + 120) : ancho + 60 - u * (ancho + 120);
      paseante = { x, y, tinta, rumbo };
      requestAnimationFrame(paso);
    };
    requestAnimationFrame(paso);
  }

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
          usados[usados.length - 1] === idx ||
          usados[usados.length - 2] === idx ||
          usados[usados.length - 3] === idx;
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
      const largo = it.titulo.length > 16;
      if (it.favorito) {
        const c = largo ? CORAZON_ANCHO : CORAZON;
        m.set(it.id, c);
        previo = "corazon";
        continue;
      }
      if (pctDe(it) >= 100) {
        const e = largo ? ESTRELLA_ANCHA : ESTRELLA;
        m.set(it.id, e);
        previo = "estrella";
        continue;
      }
      // EL TROQUEL A MEDIDA: la forma sirve al título.
      let tq = troquelPara(it.id, it.titulo.length);
      if (tq.nombre === previo) {
        const pool = TROQUELES_BASE.filter((t) =>
          it.titulo.length > 26
            ? ["nube", "escudo", "etiqueta"].includes(t.nombre)
            : t.nombre !== previo,
        );
        tq = pool[(pool.findIndex((t) => t.nombre === tq.nombre) + 1 + pool.length) % pool.length] ?? tq;
      }
      m.set(it.id, tq);
      previo = tq.nombre;
    }
    return m;
  });


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
    // Un loro curiosea detrás de la pegatina pulsada.
    if (asomadoEn !== item.id) {
      asomadoEn = item.id;
      setTimeout(() => {
        if (asomadoEn === item.id) asomadoEn = null;
      }, 1600);
    }
    const eligiendo = seleccionada !== item.id;
    seleccionada = eligiendo ? item.id : null;
    // La FICHA desplegada siempre a la vista: el reparto del cuaderno
    // puede haberla movido, así que se la persigue con el scroll.
    if (eligiendo) {
      setTimeout(() => {
        document
          .querySelector(`[data-pieza="${CSS.escape(item.id)}"]`)
          ?.scrollIntoView({ behavior: "smooth", block: "nearest" });
      }, 380);
    }
    // La voz dice el título en voz baja (solo con la casa en silencio).
    if (eligiendo && !sonando && item.estado === "listo") {
      decir(item.titulo, item.ruta ?? item.id).catch(() => {});
    }
  }

  // LA INTERFAZ DEFINITIVA (dictada): el mosaico es SOLO pegatinas. El
  // loro jefe vive arriba a la izquierda; el «yappy» reactivo abajo a la
  // izquierda; el botón de añadir, grande, abajo a la derecha; la
  // trastienda sigue cortando el borde. La bandada asoma desde detrás de
  // las pegatinas y desde los BORDES de la pantalla.
  const piezasTablero = $derived(
    visibles.map((i) => ({ id: i.id, peso: pesoDe(i), letras: Math.min(30, i.titulo.length) })),
  );
  // El alto útil del pliego: la lista menos el colofón y los aires.
  let altoLista = $state(0);
  const tablero = $derived(
    empaquetar(piezasTablero, anchoTablero, Math.max(0, altoLista - 250), pref.dosColumnas),
  );
  // Factor de dispositivo (el Pro Max respira más) y factor del pliego.
  const fDispositivo = $derived(Math.min(1.18, Math.max(1, anchoTablero / 402)));
  const cuerpoReposo = $derived(
    Math.round(19 * fDispositivo * Math.min(1.3, Math.sqrt(tablero.factor))),
  );

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
      if (id === levantada || id === aterrizando) continue;
      const antes = foto.get(id);
      if (!antes) continue;
      const el = document.querySelector<HTMLElement>(`[data-pieza="${CSS.escape(id)}"]`);
      if (!el) continue;
      const jit = jitterDe(id);
      const dx = antes.x - (b.x + jit.dx);
      const dy = antes.y - (b.y + jit.dy);
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
  let selloPop = $state(false);
  function volar(aId: string) {
    // El sello celebra: por esa boca entró la pieza.
    selloPop = true;
    setTimeout(() => (selloPop = false), 520);
    requestAnimationFrame(() => {
      const desde = document.querySelector(".boca-fija")?.getBoundingClientRect();
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
      plop();
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
  <div class="lista" bind:clientHeight={altoLista} ontouchmove={(e) => { if (levantada) e.preventDefault(); }}>
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
      {#if paseante}
        <div class="paseante" style="left: {paseante.x}px; top: {paseante.y}px; transform: scaleX({paseante.rumbo === 1 ? -1 : 1})" aria-hidden="true">
          <Criatura size={40} andando mirando={-1} tinta={paseante.tinta} />
        </div>
      {/if}
      {#each piezasTablero as ficha, i (ficha.id)}
        {@const b = tablero.baldosas.get(ficha.id)}
        {#if b}
          {@const item = visibles.find((v) => v.id === ficha.id)}
          {#if item}
            {@const pct = item.id === idQueSuena && repro.snap ? Math.min(100, Math.round((repro.snap.elapsed_secs / Math.max(1, repro.snap.duration_secs)) * 100)) : pctDe(item)}
            {@const esLaQueSuena = item.id === idQueSuena}
            {@const enVuelo = levantada === item.id}
            {@const elegida = seleccionada === item.id}
            {@const jit = jitterDe(item.id)}
            {@const tinta = tintaDe(item)}
            {@const honda = tonoHondo(tinta)}
            {@const tqBase = troquelesVivos.get(item.id) ?? troquelBase(item.id)}
            {@const tq = elegida ? FICHA : tqBase}
            {@const clipExt = aPoligono(tq)}
            {@const clipInt = aPoligono(tq, 0.93)}
            {@const vw = (b.ancho * tq.ventana.w) / 100}
            {@const vh = (b.alto * tq.ventana.h) / 100 - 20}
            {@const enMarcha = esLaQueSuena && trabajando}
            {@const grandota = elegida}
            {@const lineasCartel = enMarcha || !grandota ? [] : cartel(item.titulo, vw, Math.max(24, vh), $idiomaUI)}
            {@const reposo = enMarcha || grandota ? null : cartelReposo(item.titulo, vw, Math.max(20, vh), $idiomaUI, tq.lineas ?? 3, cuerpoReposo)}
            <article
              data-pieza={item.id}
              class="baldosa" class:en-vuelo={enVuelo}
              in:llega={{ delay: cascada ? Math.min(i * 40, 360) : 0 }}
              out:seVa
              style="left: {b.x + jit.dx}px; top: {b.y + jit.dy}px; width: {b.ancho}px; height: {b.alto}px; z-index: {enVuelo ? 40 : elegida ? 10 : esLaQueSuena ? 8 : item.favorito ? 4 : 1}; {enVuelo || aterrizando === item.id ? `transition: ${recogida ? 'transform 0.18s cubic-bezier(0.3, 1.4, 0.6, 1)' : 'none'}; transform: translate(${vuelo.dx}px, ${vuelo.dy}px) scale(${1.09 * vuelo.estira}, ${1.09 * (2 - vuelo.estira)}) rotate(${vuelo.giro}deg);` : ''}"
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
                  {:else if reposo && reposo.lineas.length > 0}
                    <!-- El reposo: renglones de cuerpo FIJO (19px), con
                         justificación suave; la reorganización solo cambia
                         cuántos renglones caben, jamás la legibilidad. -->
                    <div class="cartel reposo" aria-hidden="true">
                      {#each reposo.lineas as linea (linea.texto)}
                        <svg viewBox="0 0 {linea.vb} {VB_ALTO}" preserveAspectRatio="none" style="height: {reposo.altoLinea}px; flex: none;">
                          <text x={linea.x ?? 0} y={VB_BASE} textLength={linea.tl ?? linea.vb} lengthAdjust="spacingAndGlyphs">{linea.texto}</text>
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
                      {item.estado === "error" ? $t("cola.reintentar") : $t("cola.escuchar")}
                    </button>
                    {#if item.estado === "error" && item.tipo === "url"}
                      <!-- El RESCATE: reintentar por la copia de la Wayback
                           Machine (la pieza avisa de que es la archivada). -->
                      <button class="accion" use:presionable={{ hap: "soft" }} style="color: {tinta}" aria-label={$t("pieza.del_archivo")}
                        onclick={(e) => { e.stopPropagation(); seleccionada = null; colaReintentarArchivo(item.id).catch(() => {}); }}>
                        <svg viewBox="0 0 24 24" width="19" height="19" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4.5 7.2 Q12 3.8 19.5 7.2 M5.2 7.5 L5.6 18.2 Q12 20.6 18.4 18.2 L18.8 7.5 M9.6 11.2 Q12 12.4 14.4 11.2"/></svg>
                      </button>
                    {/if}
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

  <!-- EL LORO JEFE: arriba a la izquierda, vigilando la casa entera. -->
  <button
    class="loro-jefe"
    class:voltereta
    class:picotea={jefePicotea}
    class:volando={jefeVolando}
    class:aterriza={jefeAterriza}
    style="transform: translate({jefePos.x}px, {jefePos.y}px) scaleX({jefeGiro}) rotate({jefeAngulo}deg)"
    onclick={vuelaJefe}
    aria-label="yappy"
  >
    {#each plumas as pl, k (k)}
      <span class="pluma" style="left: {pl.x}px; top: {pl.y}px; animation-delay: {k * 120}ms" aria-hidden="true"></span>
    {/each}
    <Criatura
      size={84}
      mirando={-1}
      estado={jefeVolando || celebra ? "celebrando" : repro.snap?.estado === "sonando" ? "hablando" : repro.snap?.estado === "pausa" ? "pausa" : durmiendo ? "dormido" : "posado"}
      volando={jefeVolando}
      apertura={repro.snap?.estado === "sonando" ? nivel : 0}
      {mirada}
      tinta={$tintaVoz}
    />
  </button>

  <!-- EL «yappy» REACTIVO: abajo a la izquierda, con su ola y sus datos. -->
  <button
    class="marca-fija"
    onclick={tocarMarca}
    onpointerdown={marcaDown}
    onpointermove={marcaMove}
    onpointerup={marcaUp}
    onpointercancel={() => (marcaTira = null)}
    style="transform: translate({marcaTira?.dx ?? 0}px, {marcaTira?.dy ?? 0}px) rotate({-2 + (marcaTira?.dx ?? 0) * 0.14}deg); transition: {marcaTira ? 'none' : 'transform 0.5s cubic-bezier(0.24, 1.7, 0.44, 1)'};"
    aria-label="yappy"
  >
    <span class="marca-palabra" class:gelatina={marcaGelatina}>
      {#each "yappy".split("") as letra, k (k)}
        <i class="letra" style="animation-delay: {k * 45}ms; color: {PALETA[(k * 3 + 1) % PALETA.length]}; transform: rotate({k % 2 === 0 ? -3.5 : 3}deg) translateY({k % 2 === 0 ? -1 : 1.5}px)">{letra}</i>
      {/each}
    </span>
    <span class="membrete-datos">{visibles.length} {$t("cinta.piezas")} · {minutosTotales} {$t("cinta.min")}</span>
  </button>

  <!-- LA BOCA: el sello de añadir, GRANDE, abajo a la derecha. -->
  <button class="boca-fija" class:pop={selloPop} use:presionable={{ hap: "medium" }} onclick={() => { pop(); bocaAbierta = true; }} aria-label={$t("escuchar.anadir")}>
    <span class="capa parche-sombra" style="clip-path: {aPoligono(SELLO_BOCA)}"></span>
    <span class="capa boca-sello" style="clip-path: {aPoligono(SELLO_BOCA)}"></span>
    <svg class="costura" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
      <polygon points={aPuntosSvg(SELLO_BOCA, 0.85)} />
    </svg>
    <svg class="buzon" viewBox="0 0 44 36" aria-hidden="true">
      <path d="M22 4.6 q0.9 6.6 -0.2 13.2 M14.8 10.8 q7.2 1.1 14.4 -0.3" />
      <path d="M7 27 q15 6 30 -1.4" />
    </svg>
  </button>

  <!-- La boca abierta: la hoja brota del sello, esquina abajo-derecha. -->
  {#if bocaAbierta}
    <div class="velo" role="presentation" transition:fade={{ duration: 190 }} onclick={() => (bocaAbierta = false)}></div>
    <div
      class="boca-hoja"
      in:scale={{ duration: 340, start: 0.42, easing: backOut }}
      out:vuelveAlSello
    >
      <div class="asomado-boca" aria-hidden="true"><Criatura size={40} mirada={{ x: 0, y: 1 }} tinta={$tintaVoz} /></div>
      <button class="boca-cerrar cruz-gira" onclick={() => { haptic("light"); bocaAbierta = false; }} aria-label={$t("comun.cerrar")}>＋</button>
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
    </div>
  {/if}

  <!-- La bandada asoma también desde los BORDES de la pantalla. -->
  {#if asomadoBorde}
    <div
      class="asomado-borde lado-{asomadoBorde.lado}"
      style={asomadoBorde.lado === "abajo" ? `left: ${asomadoBorde.pos}%` : `top: ${asomadoBorde.pos}%`}
      aria-hidden="true"
    >
      <Criatura size={46} mirando={asomadoBorde.lado === "der" ? 1 : -1} {mirada} tinta={asomadoBorde.tinta} />
    </div>
  {/if}

  <!-- LA TRASTIENDA: una etiqueta mono girada que corta el borde derecho
       de la página, como las etiquetas de los márgenes de un taller. -->
<!-- EL CAJÓN DE LA TRASTIENDA: siempre renderizado fuera del
       viewport, con su pestaña pegada al borde. Pulsar la pestaña lo
       desliza SOBRE la pantalla principal; volver lo devuelve. -->
  {#if !trastiendaAbierta}
    <!-- El TOQUE de la etiqueta vive FUERA del cajón desplazado: un botón
         invisible con su misma geometría (el hit-testing de un hijo cuyo
         padre está fuera de pantalla es traicionero). -->
    <button
      class="etiqueta-toque"
      style={etiquetaTop ? `top: ${etiquetaTop}px` : ""}
      onclick={etqClick}
      onpointerdown={etqDown}
      onpointermove={etqMove}
      onpointerup={etqUp}
      onpointercancel={() => (etqArr = null)}
      aria-label={$t("cinta.trastienda")}
    ></button>
  {/if}
  <div class="cajon-trastienda" class:abierto={trastiendaAbierta}>
      <button
    class="etiqueta-trastienda"
    class:arrastrada={!!etqArr}
    style={etiquetaTop ? `top: ${etiquetaTop}px` : ""}
    onclick={etqClick}
    onpointerdown={etqDown}
    onpointermove={etqMove}
    onpointerup={etqUp}
    onpointercancel={() => (etqArr = null)}
  >
    <span class="engranaje" class:gira={engranajeGira} aria-hidden="true">
      <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3.4"/><path d="M12 2.8 l0.6 3.1 M12 21.2 l-0.5 -3 M21.2 12 l-3.1 0.5 M2.8 12 l3.1 -0.4 M18.5 5.6 l-2.2 2.1 M5.5 18.4 l2.2 -2 M18.4 18.5 l-2.1 -2.2 M5.6 5.5 l2.1 2.2"/></svg>
    </span>
    <span class="etiqueta-texto">{$t("cinta.trastienda")}</span>
  </button>
    <div class="cajon-cuerpo">
      {#if trastiendaLista}
        <Trastienda alVolver={() => (trastiendaAbierta = false)} />
      {/if}
    </div>
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
      <span class="deshacer-loro" aria-hidden="true"><Criatura size={34} mirando={-1} tinta={$tintaVoz} /></span>
      <span class="deshacer-texto">{$t("cinta.fuera")}</span>
      <button class="deshacer-tecla" use:presionable={{ hap: "medium" }} onclick={deshacerBorrado}>{$t("comun.deshacer")}</button>
    </div>
  {/if}

  {#if menuPieza}
    <div class="velo" role="presentation" transition:fade={{ duration: 180 }} onclick={() => { menuPieza = null; renombrando = false; }}></div>
    <div class="hoja" in:fly={{ y: 260, duration: 340, easing: backOut }} out:fly={{ y: 260, duration: 220 }}>
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
  /* ── Los tres fijos de la interfaz definitiva ── */
  .loro-jefe {
    position: fixed;
    top: calc(env(safe-area-inset-top) + 2px);
    left: 10px;
    z-index: 31;
    border: 0;
    background: transparent;
    padding: 0;
    cursor: pointer;
    transition: transform 0.9s cubic-bezier(0.3, 1.25, 0.4, 1);
  }
  .loro-jefe.volando {
    transition: none;
  }
  .loro-jefe.aterriza {
    animation: aterrizaje 0.5s cubic-bezier(0.24, 1.7, 0.44, 1);
  }
  @keyframes aterrizaje {
    0% { scale: 1 0.78; translate: 0 6px; }
    55% { scale: 0.94 1.1; translate: 0 -3px; }
    100% { scale: 1 1; translate: 0 0; }
  }
  /* Las plumitas del aterrizaje: dos trazos crema que caen meciéndose. */
  .pluma {
    position: absolute;
    width: 10px;
    height: 4px;
    border-radius: 60% 40% 55% 45%;
    background: #f7f2e7;
    border: 1px solid #d8d2c4;
    animation: pluma-cae 0.75s ease-in both;
    pointer-events: none;
  }
  @keyframes pluma-cae {
    0% { transform: translateY(0) rotate(0deg); opacity: 1; }
    100% { transform: translateY(34px) rotate(140deg); opacity: 0; }
  }
  .loro-jefe.picotea {
    animation: picoteo-jefe 0.5s ease;
  }
  @keyframes picoteo-jefe {
    0%, 100% { rotate: 0deg; }
    30% { rotate: 16deg; translate: 6px 10px; }
    55% { rotate: 4deg; }
    75% { rotate: 14deg; translate: 5px 9px; }
  }
  .paseante {
    position: absolute;
    z-index: 0;
    pointer-events: none;
  }
  .loro-jefe.voltereta {
    animation: voltereta 0.85s cubic-bezier(0.34, 1.3, 0.5, 1);
  }
  .marca-fija {
    position: fixed;
    left: 18px;
    bottom: calc(env(safe-area-inset-bottom) + var(--aguja-hueco, 0px) + 6px);
    z-index: 31;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    /* UNA PEGATINA de verdad: la palabra y los datos comparten parche
       crema con canto irregular, sombra dura y su costura. */
    border: 1.5px solid #8a765a;
    border-radius: 15px 19px 14px 21px / 18px 14px 20px 15px;
    background: var(--yap-superficie, #fdf9ee);
    padding: 8px 13px 8px 11px;
    box-shadow: 2.5px 3px 0 #ded7c2;
    cursor: pointer;
    transition: bottom 0.3s cubic-bezier(0.34, 1.56, 0.64, 1);
  }
  .marca-fija::after {
    content: "";
    position: absolute;
    inset: 4px;
    border: 1.5px dashed color-mix(in srgb, var(--yap-tinta, #2b2418) 36%, transparent);
    border-radius: 12px 15px 11px 17px / 14px 11px 16px 12px;
    pointer-events: none;
  }
  .membrete-datos {
    font-family: var(--yap-mono, ui-monospace, monospace);
    font-size: 10px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--yap-tinta-suave, #82755a);
    margin-left: 3px;
  }
  .boca-fija {
    position: fixed;
    right: 12px;
    bottom: calc(env(safe-area-inset-bottom) + var(--aguja-hueco, 0px) + 12px);
    z-index: 31;
    width: 92px;
    height: 92px;
    border: 0;
    background: transparent;
    padding: 0;
    cursor: pointer;
    transition: bottom 0.3s cubic-bezier(0.34, 1.56, 0.64, 1);
  }
  .boca-fija.pop {
    animation: pop-sello 0.5s cubic-bezier(0.24, 1.7, 0.44, 1);
  }
  @keyframes pop-sello {
    0% { transform: scale(1); }
    45% { transform: scale(1.17) rotate(-4deg); }
    100% { transform: scale(1); }
  }
  .boca-fija .buzon {
    position: relative;
    z-index: 5;
    width: 52px;
    height: 44px;
    margin: 24px auto 0;
    display: block;
  }
  /* La hoja que brota del sello, esquina abajo-derecha. */
  .boca-hoja {
    position: fixed;
    right: 12px;
    left: 12px;
    bottom: calc(env(safe-area-inset-bottom) + var(--aguja-hueco, 0px) + 12px + var(--teclado, 0px));
    transition: bottom 0.34s cubic-bezier(0.3, 1.2, 0.4, 1);
    z-index: 52;
    transform-origin: 92% 100%;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 20px 14px 14px;
    border: 1.5px solid var(--yap-tinta, #2b2418);
    border-radius: 18px;
    background: var(--yap-papel, #f4f1ea);
    box-shadow: 4px 5px 0 #ded7c2;
  }
  /* La bandada asomando por los bordes. */
  .asomado-borde {
    position: fixed;
    z-index: 25;
    pointer-events: none;
  }
  .asomado-borde.lado-izq {
    left: 0;
    animation: asoma-izq 2.8s ease both;
  }
  .asomado-borde.lado-der {
    right: 0;
    animation: asoma-der 2.8s ease both;
  }
  .asomado-borde.lado-abajo {
    bottom: 0;
    animation: asoma-abajo 2.8s ease both;
  }
  @keyframes asoma-izq {
    0%, 100% { transform: translateX(-52px); }
    22%, 78% { transform: translateX(-8px) rotate(4deg); }
  }
  @keyframes asoma-der {
    0%, 100% { transform: translateX(52px); }
    22%, 78% { transform: translateX(8px) rotate(-4deg); }
  }
  @keyframes asoma-abajo {
    0%, 100% { transform: translateY(56px); }
    22%, 78% { transform: translateY(10px) rotate(-3deg); }
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
  /* El «yappy» de recortes: cada letra con su color y su giro propios,
     como pegada de una revista distinta. */
  .marca-palabra {
    font-weight: 900;
    font-size: 42px;
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
  .etiqueta-toque {
    position: fixed;
    right: 0;
    top: 24%;
    z-index: 48;
    width: 44px;
    height: 172px;
    border: 0;
    background: transparent;
    padding: 0;
    touch-action: none;
    cursor: pointer;
  }
  .cajon-trastienda {
    position: fixed;
    inset: 0;
    z-index: 47;
    transform: translateX(100%);
    transition: transform 0.5s cubic-bezier(0.28, 1.2, 0.36, 1);
    pointer-events: none;
  }
  .cajon-trastienda.abierto {
    transform: translateX(0);
  }
  .cajon-cuerpo {
    position: absolute;
    inset: 0;
    background: var(--yap-papel, #f4f1ea);
    box-shadow: -5px 0 0 #ded7c2;
    overflow-y: auto;
    -webkit-overflow-scrolling: touch;
    pointer-events: auto;
  }
  .etiqueta-trastienda {
    position: absolute;
    /* Su ancho exacto: el canto derecho TOCA el margen de la pantalla
       (estaba a 13px del borde y se veía flotando). */
    left: -34px;
    top: 24%;
    pointer-events: auto;
    transition: top 0.35s cubic-bezier(0.3, 1.3, 0.5, 1);
    touch-action: none;
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
  .etiqueta-trastienda.arrastrada {
    transition: none;
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
    /* Aire para los fijos: el jefe arriba, la marca y el sello abajo. */
    padding: 68px 5px calc(env(safe-area-inset-bottom) + var(--aguja-hueco, 0px) + 118px);
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
    /* El colofón viaja FLUIDO cuando el cuaderno cambia de alto (antes
       saltaba de golpe al desplegarse la última pegatina). */
    transition: height 0.6s cubic-bezier(0.18, 1.5, 0.32, 1);
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
    padding-bottom: 13px;
  }
  /* Modo noche: todas duermen salvo la que suena. */
  :global(html[data-theme="dark"]) .parche:not(.suena) .parche-cuerpo,
  :global(html[data-theme="dark"]) .baldosa-cuerpo:not(.suena) {
    filter: brightness(0.52) saturate(0.6);
  }

  .titulo-plano {
    margin: auto 0;
    color: #f7f2e7;
    font-weight: 800;
    line-height: 1.16;
    letter-spacing: -0.01em;
    text-align: center;
    display: -webkit-box;
    -webkit-box-orient: vertical;
    overflow: hidden;
    overflow-wrap: anywhere;
  }

  /* EL CARTEL vive dentro de la ventana. */
  .cartel {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    pointer-events: none;
  }
  .cartel.reposo {
    flex: none;
    margin: auto 0;
    justify-content: center;
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
  /* La pestaña cosida: la etiqueta de tela que cuelga del troquel, con
     su costura arriba y su sombra dura. Nada de píldoras digitales. */
  .pastilla {
    position: absolute;
    left: 50%;
    bottom: -11px;
    transform: translateX(-50%) rotate(-1.4deg);
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 4px 10px 3px;
    border-radius: 2px 2px 8px 8px;
    border: 1.3px solid rgba(247, 242, 231, 0.85);
    border-top: 1.3px dashed rgba(247, 242, 231, 0.9);
    box-shadow: 1.5px 2.5px 0 rgba(43, 36, 24, 0.22);
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
  .boca-sello {
    background: #f7f2e7;
  }
  .boca-fija .boca-sello {
    background: var(--vivo, var(--yap-voz, #e0502a));
  }
  .boca-fija .costura polygon {
    stroke: rgba(247, 242, 231, 0.6);
  }
  .boca-fija .buzon path {
    stroke: #f7f2e7;
    animation: none;
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
    left: 6px;
    right: 6px;
    bottom: var(--teclado, 0px);
    transition: bottom 0.34s cubic-bezier(0.3, 1.2, 0.4, 1);
    z-index: 51;
    background: var(--yap-papel);
    border: 1.5px solid var(--yap-tinta, #2b2418);
    border-bottom: 0;
    border-radius: 26px 16px 0 0;
    box-shadow: 4px -3px 0 #ded7c2;
    padding: 12px 18px calc(env(safe-area-inset-bottom) + 18px);
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .hoja-asa {
    width: 52px;
    height: 0;
    border-top: 2.5px dashed var(--yap-borde);
    margin: 2px auto 4px;
  }
  /* Renombrar: escribir sobre la línea de puntos de una ficha, no en una
     caja de formulario de oficina. */
  .hoja :global(.yap-campo) {
    background: transparent;
    border: 0;
    border-bottom: 2.5px dashed var(--yap-borde);
    border-radius: 0;
    font-family: var(--yap-lectura, Georgia, serif);
    font-size: 19px;
    text-align: center;
    padding: 8px 4px;
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
    margin: auto 15px 4px;
    padding-top: 16px;
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
