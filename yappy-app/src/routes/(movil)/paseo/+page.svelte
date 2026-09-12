<script lang="ts">
  // EL PASEO DEL LORO: catorce páginas del álbum. Cada una enseña algo, hace
  // algo o pregunta algo que cambia la app; el loro lo dice todo en voz
  // alta y camina por el borde inferior de página en página (su posición
  // es el progreso). Saltar siempre se puede. Nada pide teclado ni permisos.
  import { onMount, onDestroy, tick } from "svelte";
  import { avisosPedir, avisosEstado } from "$lib/ipc";

  // LOS AVISOS: el permiso se pide aquí, explicando para qué (el audiolibro
  // que termina con el teléfono en el bolsillo). Tras pedirlo, se consulta
  // el estado unos segundos y el loro lo dice.
  let avisosEstadoN = $state<0 | 1 | 2>(0);
  let avisosPedidos = $state(false);
  async function pedirAvisos() {
    haptic("medium");
    avisosPedidos = true;
    await avisosPedir().catch(() => {});
    for (let i = 0; i < 40; i++) {
      await new Promise((r) => setTimeout(r, 400));
      const e = (await avisosEstado().catch(() => 0)) as 0 | 1 | 2;
      if (e !== 0) {
        avisosEstadoN = e;
        break;
      }
    }
    if (avisosEstadoN === 1) {
      haptic("success");
      void decirPaso("paseo.avisos.gracias");
    }
  }
  import { fly, fade, scale } from "svelte/transition";
  import { backOut, cubicOut } from "svelte/easing";
  import { goto } from "$app/navigation";
  import { get as getStore } from "svelte/store";
  import { t, idiomaUI, IDIOMAS_UI, NOMBRE_IDIOMA, frase, fijarPreferenciaIdioma, type IdiomaUI } from "$lib/i18n";
  import { haptic } from "$lib/haptic";
  import { presionable } from "$lib/presionable";
  import { plop, pop, boing, rasga, tick as foleyTick } from "$lib/foley";
  import Criatura from "$lib/Criatura.svelte";
  import Deslizador from "$lib/Deslizador.svelte";
  import Confetti from "$lib/Confetti.svelte";
  import { TINTAS_VOZ, tintaVoz, fijarTintaVoz } from "$lib/voces";
  import { PALETA, tonoHondo, SERENO } from "$lib/juguete";
  import { TROQUELES_BASE, aPoligono, aPuntosSvg, type Troquel } from "$lib/troquel";
  import { reader } from "$lib/readerStore.svelte";
  import { repro } from "$lib/reproduccion.svelte";
  import { compras, abrirPaywall } from "$lib/compras.svelte";
  import { medir, medirPantalla } from "$lib/analitica";
  import {
    paseo,
    PASOS,
    OPCIONES_QUE,
    OPCIONES_CUANDO,
    OPCIONES_CUANTO,
    GESTOS,
    cargarPaseo,
    cargarCuentos,
    decirPaso,
    callar,
    siguiente,
    saltarPaso,
    irAlPaso,
    responder,
    terminarPaseo,
    piezasPropias,
    registrarCompartido,
    gestosCompletos,
    type Paso,
  } from "$lib/paseo.svelte";
  import {
    listVoices,
    getSettings,
    setVoice,
    setSpeed,
    setDefaultLang,
    setAppTheme,
    sampleVoice,
    colaAgregarCuento,
    colaAgregarPortapapeles,
    colaAgregarUrl,
    readDocument,
    readDocumentParagraphs,
    onColaActualizada,
    pipIniciar,
    pipParar,
    type Voice,
    type ItemCola,
  } from "$lib/ipc";
  import { openUrl } from "@tauri-apps/plugin-opener";

  const paso = $derived<Paso>(PASOS[Math.max(0, Math.min(PASOS.length - 1, paseo.paso))]);
  const ultimo = PASOS.length - 1;

  let voces = $state<Voice[]>([]);
  let vozElegida = $state("Alex");
  let velocidad = $state(1.05);
  let idiomaTocado = $state<IdiomaUI | null>(null);
  let cuentoTocado = $state<string | null>(null);
  let listo = $state(false);
  let celebra = $state(false);
  let enlaceManual = $state("");
  let cleanups: (() => void)[] = [];
  let velocidadTimer: ReturnType<typeof setTimeout> | undefined;
  let huecoPip: HTMLDivElement | undefined = $state();
  let pipVivo = $state(false);
  let cuerdaAbierta = false;

  function f(clave: string, vars: Record<string, string | number> = {}): string {
    let s = $t(clave);
    for (const [k, v] of Object.entries(vars)) s = s.replaceAll(`{${k}}`, String(v));
    return s;
  }

  // Las formas de las pegatinas de opciones, deterministas por posición.
  const forma = (i: number): Troquel => TROQUELES_BASE[(i * 3 + 1) % TROQUELES_BASE.length];
  const tinta = (i: number): string => PALETA[(i * 4 + 2) % PALETA.length];

  // El loro dice el paso al entrar en él.
  $effect(() => {
    const p = paso;
    if (!listo) return;
    const vars = { voz: vozElegida };
    if (p === "gestos" || p === "cuerda") return;
    void decirPaso(`paseo.dice.${p}`, vars);
  });

  // El paso de la cuerda: se abre el paywall y, al cerrarse, termina el
  // paseo. `abierto` se lee SIEMPRE (si solo se leyera en la rama del
  // cierre, Svelte no lo tomaría por dependencia y el paseo se quedaría
  // colgado en esta página al cerrar la hoja).
  $effect(() => {
    const abierto = compras.abierto;
    if (!listo || paso !== "cuerda") return;
    if (!compras.disponible) {
      void terminarPaseo("fin");
      return;
    }
    if (!cuerdaAbierta) {
      cuerdaAbierta = true;
      medir("paseo_cuerda_vista");
      abrirPaywall("paseo");
    } else if (!abierto) {
      void terminarPaseo("fin");
    }
  });

  // Al salir del paso de compartir, el loro en PiP se recoge.
  $effect(() => {
    if (paso !== "compartir" && pipVivo) {
      pipVivo = false;
      pipParar().catch(() => {});
    }
  });

  onMount(async () => {
    if (!paseo.cargado) await cargarPaseo();
    if (!paseo.activo) {
      goto("/escuchar", { replaceState: true });
      return;
    }
    const s = await getSettings().catch(() => null);
    if (s) {
      vozElegida = s.voice;
      velocidad = s.speed;
    }
    voces = await listVoices().catch(() => []);
    await cargarCuentos();
    paseo.piezasAntes = await piezasPropias();
    listo = true;
    medirPantalla(`paseo_${paso}`);
    rasga();
    // Compartir: la página se completa sola cuando llega una pieza propia.
    cleanups.push(
      await onColaActualizada(async () => {
        if (paso !== "compartir") return;
        const ahora = await piezasPropias();
        if (ahora > paseo.piezasAntes) {
          await registrarCompartido();
          pipVivo = false;
          pipParar().catch(() => {});
          celebra = true;
          haptic("success");
          plop();
          setTimeout(() => {
            celebra = false;
            void siguiente();
          }, 1600);
        }
      }),
    );
    const alVolver = async () => {
      if (document.visibilityState === "visible") await cargarCuentos();
    };
    document.addEventListener("visibilitychange", alVolver);
    cleanups.push(() => document.removeEventListener("visibilitychange", alVolver));
  });
  onDestroy(() => {
    cleanups.forEach((c) => c());
    callar();
    if (pipVivo) pipParar().catch(() => {});
  });

  // ── Acciones por paso ──────────────────────────────────────────────────
  async function tocarIdioma(cod: IdiomaUI) {
    haptic("light");
    if (idiomaTocado === cod) {
      // Segundo toque: elegir.
      fijarPreferenciaIdioma(cod);
      await setDefaultLang(cod).catch(() => {});
      medir("paseo_idioma", { idioma: cod });
      plop();
      return;
    }
    idiomaTocado = cod;
    foleyTick();
    void decirPaso("paseo.hola_corto", {}, undefined, cod).then(() => {});
    // La frase en SU idioma: el diccionario de esa lengua, no el nuestro.
    const linea = frase(cod, "paseo.hola_corto");
    if (linea && linea !== "paseo.hola_corto") {
      const { hablar } = await import("$lib/ipc");
      hablar(linea, undefined, cod).catch(() => {});
    }
  }

  async function elegirVoz(v: Voice, i: number) {
    haptic("medium");
    vozElegida = v.name;
    fijarTintaVoz(TINTAS_VOZ[i % TINTAS_VOZ.length]);
    await setVoice(v.name).catch(() => {});
    sampleVoice(v.name).catch(() => {});
    medir("paseo_voz", { voz: v.name });
  }

  async function tocarQue(id: string) {
    haptic("light");
    const estaba = paseo.libreta.que.includes(id);
    await responder("que", id);
    if (!estaba) {
      boing();
      void decirPaso(`paseo.que.comentario.${id}`);
    }
  }

  async function tocarCuando(id: string) {
    haptic("medium");
    await responder("cuando", id);
    plop();
    if (id === "cama") {
      document.documentElement.dataset.theme = "dark";
      try {
        localStorage.setItem("yappy.tema", "dark");
      } catch {}
      await setAppTheme("dark").catch(() => {});
    }
    void decirPaso(`paseo.cuando.comentario.${id}`);
  }

  function cambiarVelocidad(v: number) {
    velocidad = v;
    clearTimeout(velocidadTimer);
    velocidadTimer = setTimeout(async () => {
      await setSpeed(v).catch(() => {});
      medir("paseo_velocidad", { velocidad: Math.round(v * 100) / 100 });
      void decirPaso("paseo.velocidad.demo", {}, v);
    }, 260);
  }

  async function tocarCuanto(id: string) {
    haptic("medium");
    await responder("cuanto", id);
    plop();
    void decirPaso(`paseo.cuanto.dice.${id}`);
  }

  /// Abrir una pieza en el cartel y pasar al paso de los gestos.
  async function abrirPieza(item: ItemCola) {
    if (!item.ruta) return;
    try {
      const doc = await readDocument(item.ruta);
      doc.filename = item.titulo;
      reader.doc = doc;
      await irAlPaso(PASOS.indexOf("gestos"));
      await readDocumentParagraphs(doc.paragraphs, 0, undefined, undefined, undefined, {
        docPath: item.ruta,
        titulo: item.titulo,
      });
      medir("paseo_escucha", { fuente: item.de_la_casa ? "cuento" : "propio" });
      await goto("/read");
    } catch (e) {
      console.error("paseo abrir:", e);
    }
  }

  async function leerLoCopiado() {
    haptic("medium");
    try {
      const item = await colaAgregarPortapapeles();
      if (item.estado === "listo" && item.ruta) {
        await abrirPieza(item);
      } else {
        // Un enlace se prepara en segundo plano: suena en cuanto esté; el
        // paseo sigue por la lección de compartir.
        medir("paseo_escucha", { fuente: "enlace" });
        await irAlPaso(PASOS.indexOf("compartir"));
      }
    } catch (e) {
      console.error("paseo portapapeles:", e);
      paseo.enlaceCopiado = false;
    }
  }

  async function tocarCuento(id: string, primera: string) {
    haptic("light");
    if (cuentoTocado !== id) {
      cuentoTocado = id;
      foleyTick();
      const { hablar } = await import("$lib/ipc");
      hablar(primera).catch(() => {});
      return;
    }
    haptic("medium");
    try {
      const item = await colaAgregarCuento(id);
      await abrirPieza(item);
    } catch (e) {
      console.error("paseo cuento:", e);
    }
  }

  async function pegarEnlaceManual() {
    const url = enlaceManual.trim();
    if (!url) return;
    haptic("medium");
    enlaceManual = "";
    await colaAgregarUrl(url.startsWith("http") ? url : `https://${url}`).catch(() => {});
  }

  // Compartir: el enlace recomendado y el loro que sale contigo.
  const leccionCompartir = $derived.by(() => {
    const q = paseo.libreta.que;
    // Los artículos primero: es la única lección con destino propio (la
    // web de la casa) y guía en PiP, así que es la que nunca falla.
    for (const id of ["articulos", "libros", "apuntes", "notas", "videos", "correos"]) {
      if (q.includes(id)) return id;
    }
    return "articulos";
  });
  const urlRecomendado = $derived($idiomaUI === "es" ? "https://yappy.joseluissaorin.com/cuentos/" : "https://yappy.joseluissaorin.com/en/stories/");

  async function abrirRecomendado() {
    haptic("light");
    medir("paseo_recomendado_abierto");
    await empezarPip();
    openUrl(urlRecomendado).catch(() => {});
  }

  async function empezarPip() {
    if (pipVivo || !huecoPip) return;
    await tick();
    const r = huecoPip.getBoundingClientRect();
    try {
      await pipIniciar(r.left, r.top, r.width, r.height);
      pipVivo = true;
      medir("paseo_pip");
    } catch {
      pipVivo = false;
    }
  }

  async function valeVoy() {
    haptic("medium");
    pop();
    await empezarPip();
  }

  async function favoritosHecho() {
    haptic("success");
    medir("paseo_favoritos", { hecho: true });
    await siguiente();
  }

  // Resumen: las etiquetas legibles.
  const etiquetaCuando = $derived(paseo.libreta.cuando ? $t(`paseo.cuando.${paseo.libreta.cuando}`) : "");
  const etiquetasQue = $derived(paseo.libreta.que.map((q) => $t(`paseo.que.${q}`)).join(" · "));
  const oido = $derived.by(() => {
    const s = Math.max(0, Math.round(paseo.libreta.segundos_escuchados));
    return s < 60 ? f("paseo.resumen.segundos", { n: s }) : f("paseo.resumen.minutos", { n: Math.round(s / 60) });
  });

  // La percha: cuántas piezas propias hay.
  let propias = $state(0);
  $effect(() => {
    if (paso === "percha" || paso === "resumen") void piezasPropias().then((n) => (propias = n));
  });
</script>

<div class="paseo" style="--vivo-paseo: {$tintaVoz}">
  {#if listo}
    <!-- Luego: saltar el paso (en todos menos el primero). -->
    {#if paso !== "hola" && paso !== "cuerda"}
      <button class="luego" use:presionable={{ hap: "soft" }} onclick={saltarPaso}>{$t("paseo.luego")}</button>
    {/if}

    {#key paseo.paso}
      <section class="pagina yap-bloque" in:fly={{ x: 60, duration: 420, easing: backOut }} out:fly={{ x: -60, duration: 180, easing: cubicOut }}>
        <span class="celo" aria-hidden="true"></span>

        {#if paso === "hola"}
          <div class="escena">
            <Criatura size={168} cantando={paseo.hablando} tinta={$tintaVoz} />
            <h1 class="yap-grito titulo">{$t("paseo.hola.titulo")}</h1>
            <p class="lectura">{$t("paseo.dice.hola")}</p>
            <p class="lectura chica">{$t("paseo.hola.que_hace")}</p>
            <button class="yap-tecla grande" use:presionable={{ hap: "rigid" }} onclick={siguiente}>{$t("paseo.hola.tecla")}</button>
          </div>

        {:else if paso === "idiomas"}
          <div class="escena">
            <Criatura size={96} cantando={paseo.hablando} tinta={$tintaVoz} />
            <h1 class="yap-grito titulo">{$t("paseo.idiomas.titulo")}</h1>
            <p class="lectura">{$t("paseo.dice.idiomas")}</p>
            <p class="yap-susurro pista">{$t("paseo.idiomas.pista")}</p>
            <div class="sellos">
              {#each IDIOMAS_UI as cod, i (cod)}
                <button
                  class="sello"
                  class:tocado={idiomaTocado === cod}
                  class:elegido={$idiomaUI === cod}
                  style="--tinta:{PALETA[i % PALETA.length]}; --giro:{((i % 3) - 1) * 2.4}deg"
                  use:presionable={{ hap: "soft" }}
                  onclick={() => tocarIdioma(cod)}
                >
                  {NOMBRE_IDIOMA[cod]}
                  {#if $idiomaUI === cod}<span class="visto" aria-hidden="true">✓</span>{/if}
                </button>
              {/each}
            </div>
            <button class="yap-tecla grande" use:presionable={{ hap: "rigid" }} onclick={siguiente}>{$t("paseo.sigue")}</button>
          </div>

        {:else if paso === "pajaro"}
          <div class="escena">
            <h1 class="yap-grito titulo">{$t("paseo.pajaro.titulo")}</h1>
            <p class="lectura">{$t("paseo.dice.pajaro")}</p>
            <div class="cromos">
              {#each voces as v, i (v.name)}
                <button class="cromo" class:elegido={vozElegida === v.name} use:presionable={{ hap: "soft" }} onclick={() => elegirVoz(v, i)}>
                  <span class="parche" style="background: {PALETA[i % PALETA.length]}"><Criatura size={56} tinta={TINTAS_VOZ[i % TINTAS_VOZ.length]} cantando={vozElegida === v.name && paseo.hablando} /></span>
                  <strong>{v.name}</strong>
                </button>
              {/each}
            </div>
            <button class="yap-tecla grande" use:presionable={{ hap: "rigid" }} onclick={siguiente}>{f("paseo.pajaro.tecla", { voz: vozElegida })}</button>
          </div>

        {:else if paso === "que"}
          <div class="escena">
            <Criatura size={84} cantando={paseo.hablando} tinta={$tintaVoz} />
            <h1 class="yap-grito titulo">{$t("paseo.que.titulo")}</h1>
            <p class="lectura">{$t("paseo.dice.que")}</p>
            <p class="yap-susurro pista">{$t("paseo.que.formatos")}</p>
            <div class="pegatinas">
              {#each OPCIONES_QUE as id, i (id)}
                {@const activa = paseo.libreta.que.includes(id)}
                <button class="pegatina" class:activa style="--tinta:{tinta(i)}; --honda:{tonoHondo(tinta(i))}; --clip:{aPoligono(forma(i))}; --clip-in:{aPoligono(forma(i), 0.92)}; --giro:{activa ? 0 : ((i % 2) * 2 - 1) * 3}deg" use:presionable={{ hap: "soft" }} onclick={() => tocarQue(id)} aria-pressed={activa}>
                  <span class="capa sombra"></span><span class="capa borde"></span><span class="capa cuerpo"></span>
                  <svg class="costura" viewBox="0 0 100 100" preserveAspectRatio="none"><polygon points={aPuntosSvg(forma(i), 0.82)} fill="none" stroke="var(--honda)" stroke-width="1.4" stroke-dasharray="3 2.6" vector-effect="non-scaling-stroke" /></svg>
                  <span class="etiqueta">{$t(`paseo.que.${id}`)}</span>
                </button>
              {/each}
            </div>
            <button class="yap-tecla grande" use:presionable={{ hap: "rigid" }} onclick={siguiente} disabled={paseo.libreta.que.length === 0}>{$t("paseo.sigue")}</button>
          </div>

        {:else if paso === "cuando"}
          <div class="escena">
            <Criatura size={84} cantando={paseo.hablando} tinta={$tintaVoz} />
            <h1 class="yap-grito titulo">{$t("paseo.cuando.titulo")}</h1>
            <p class="lectura">{$t("paseo.dice.cuando")}</p>
            <div class="pegatinas cuatro">
              {#each OPCIONES_CUANDO as id, i (id)}
                {@const activa = paseo.libreta.cuando === id}
                <button class="pegatina" class:activa style="--tinta:{tinta(i + 2)}; --honda:{tonoHondo(tinta(i + 2))}; --clip:{aPoligono(forma(i + 1))}; --clip-in:{aPoligono(forma(i + 1), 0.92)}; --giro:{activa ? 0 : ((i % 2) * 2 - 1) * 3}deg" use:presionable={{ hap: "soft" }} onclick={() => tocarCuando(id)} aria-pressed={activa}>
                  <span class="capa sombra"></span><span class="capa borde"></span><span class="capa cuerpo"></span>
                  <svg class="costura" viewBox="0 0 100 100" preserveAspectRatio="none"><polygon points={aPuntosSvg(forma(i + 1), 0.82)} fill="none" stroke="var(--honda)" stroke-width="1.4" stroke-dasharray="3 2.6" vector-effect="non-scaling-stroke" /></svg>
                  <span class="etiqueta">{$t(`paseo.cuando.${id}`)}</span>
                </button>
              {/each}
            </div>
            <button class="yap-tecla grande" use:presionable={{ hap: "rigid" }} onclick={siguiente} disabled={!paseo.libreta.cuando}>{$t("paseo.sigue")}</button>
          </div>

        {:else if paso === "velocidad"}
          <div class="escena">
            <Criatura size={110} cantando={paseo.hablando} tinta={$tintaVoz} />
            <h1 class="yap-grito titulo">{$t("paseo.velocidad.titulo")}</h1>
            <p class="lectura">{$t("paseo.dice.velocidad")}</p>
            <div class="dial">
              <span class="valor mono">{velocidad.toFixed(2).replace(".", ",")}×</span>
              <Deslizador bind:value={velocidad} min={0.7} max={1.8} step={0.05} imanes={[1.0, 1.25, 1.5]} formatear={(v: number) => v.toFixed(2).replace(".", ",") + "×"} etiqueta={$t("ajustes.velocidad")} alSoltar={(v) => cambiarVelocidad(v)} />
            </div>
            <button class="enlace" use:presionable={{ hap: "soft" }} onclick={() => decirPaso("paseo.velocidad.demo", {}, velocidad)}>{$t("paseo.velocidad.oir")}</button>
            <button class="yap-tecla grande" use:presionable={{ hap: "rigid" }} onclick={siguiente}>{$t("paseo.sigue")}</button>
          </div>

        {:else if paso === "cuanto"}
          <div class="escena">
            <Criatura size={84} cantando={paseo.hablando} tinta={$tintaVoz} />
            <h1 class="yap-grito titulo">{$t("paseo.cuanto.titulo")}</h1>
            <p class="lectura">{$t("paseo.dice.cuanto")}</p>
            <div class="pegatinas tres">
              {#each OPCIONES_CUANTO as id, i (id)}
                {@const activa = paseo.libreta.cuanto === id}
                <button class="pegatina" class:activa style="--tinta:{tinta(i + 5)}; --honda:{tonoHondo(tinta(i + 5))}; --clip:{aPoligono(forma(i + 3))}; --clip-in:{aPoligono(forma(i + 3), 0.92)}; --giro:{activa ? 0 : ((i % 2) * 2 - 1) * 3}deg" use:presionable={{ hap: "soft" }} onclick={() => tocarCuanto(id)} aria-pressed={activa}>
                  <span class="capa sombra"></span><span class="capa borde"></span><span class="capa cuerpo"></span>
                  <svg class="costura" viewBox="0 0 100 100" preserveAspectRatio="none"><polygon points={aPuntosSvg(forma(i + 3), 0.82)} fill="none" stroke="var(--honda)" stroke-width="1.4" stroke-dasharray="3 2.6" vector-effect="non-scaling-stroke" /></svg>
                  <span class="etiqueta">{$t(`paseo.cuanto.${id}`)}</span>
                </button>
              {/each}
            </div>
            {#if paseo.libreta.cuanto}
              <p class="lectura chica" in:fade>{$t(`paseo.cuanto.dice.${paseo.libreta.cuanto}`)}</p>
            {/if}
            <button class="yap-tecla grande" use:presionable={{ hap: "rigid" }} onclick={siguiente} disabled={!paseo.libreta.cuanto}>{$t("paseo.sigue")}</button>
          </div>

        {:else if paso === "escucha"}
          <div class="escena">
            <Criatura size={84} cantando={paseo.hablando} tinta={$tintaVoz} />
            <h1 class="yap-grito titulo">{$t("paseo.escucha.titulo")}</h1>
            {#if paseo.enlaceCopiado}
              <p class="lectura">{$t("paseo.escucha.copiado")}</p>
              <button class="yap-tecla grande" use:presionable={{ hap: "rigid" }} onclick={leerLoCopiado}>{$t("paseo.escucha.leer_copiado")}</button>
              <p class="yap-susurro pista">{$t("paseo.escucha.o_cuento")}</p>
            {:else}
              <p class="lectura">{$t("paseo.dice.escucha")}</p>
            {/if}
            {#if paseo.cuentos.length}
              <div class="pegatinas cuentos">
                {#each paseo.cuentos as c, i (c.id)}
                  {@const tocado = cuentoTocado === c.id}
                  <button class="pegatina cuento" class:activa={tocado} style="--tinta:{tinta(i + 7)}; --honda:{tonoHondo(tinta(i + 7))}; --clip:{aPoligono(forma(i + 5))}; --clip-in:{aPoligono(forma(i + 5), 0.92)}; --giro:{tocado ? 0 : ((i % 2) * 2 - 1) * 3}deg" use:presionable={{ hap: "soft" }} onclick={() => tocarCuento(c.id, c.primera_frase)}>
                    <span class="capa sombra"></span><span class="capa borde"></span><span class="capa cuerpo"></span>
                    <svg class="costura" viewBox="0 0 100 100" preserveAspectRatio="none"><polygon points={aPuntosSvg(forma(i + 5), 0.82)} fill="none" stroke="var(--honda)" stroke-width="1.4" stroke-dasharray="3 2.6" vector-effect="non-scaling-stroke" /></svg>
                    <span class="etiqueta"><strong>{c.titulo}</strong><small>{c.autor}</small></span>
                  </button>
                {/each}
              </div>
              <p class="yap-susurro pista">{cuentoTocado ? $t("paseo.escucha.otra_vez") : $t("paseo.escucha.toca")}</p>
            {:else}
              <div class="enlace-fila">
                <input class="yap-campo" type="url" bind:value={enlaceManual} placeholder={$t("cinta.enlace_pista")} enterkeyhint="go" autocapitalize="off" autocorrect="off" spellcheck="false" onkeydown={(e) => e.key === "Enter" && pegarEnlaceManual()} />
                <button class="yap-tecla" use:presionable onclick={pegarEnlaceManual} disabled={!enlaceManual.trim()}>{$t("cinta.a_la_cola")}</button>
              </div>
            {/if}
          </div>

        {:else if paso === "gestos"}
          <div class="escena">
            <Criatura size={96} cantando={paseo.hablando} tinta={$tintaVoz} />
            <h1 class="yap-grito titulo">{$t("paseo.gestos.titulo")}</h1>
            <p class="lectura">{$t("paseo.gestos.texto")}</p>
            <ul class="casillas">
              {#each GESTOS as g (g)}
                <li class:hecho={paseo.libreta.gestos.includes(g)}><span class="casilla" aria-hidden="true">{paseo.libreta.gestos.includes(g) ? "✓" : ""}</span>{$t(`paseo.gestos.${g}`)}</li>
              {/each}
            </ul>
            {#if repro.snap && repro.snap.estado !== "inactivo"}
              <button class="yap-tecla grande" use:presionable={{ hap: "rigid" }} onclick={() => goto("/read")}>{$t("paseo.gestos.volver")}</button>
            {/if}
            <button class="enlace" use:presionable={{ hap: "soft" }} onclick={siguiente}>{gestosCompletos() ? $t("paseo.sigue") : $t("paseo.luego")}</button>
          </div>

        {:else if paso === "compartir"}
          <div class="escena">
            {#if celebra}<Confetti />{/if}
            <div class="hueco-pip" bind:this={huecoPip} class:vivo={pipVivo} aria-hidden="true">
              {#if !pipVivo}
                <Criatura size={120} cantando={paseo.hablando} tinta={$tintaVoz} mirando={-1} />
              {/if}
            </div>
            <h1 class="yap-grito titulo">{$t("paseo.compartir.titulo")}</h1>
            <p class="lectura">{$t(`paseo.compartir.${leccionCompartir}`)}</p>
            <!-- La hoja de compartir, dibujada: el gesto se ve antes de hacerlo. -->
            <svg class="hoja-compartir" viewBox="0 0 240 96" aria-hidden="true">
              <rect x="6" y="6" width="228" height="84" rx="16" fill="#fffdf7" stroke="#2b2418" stroke-width="2" transform="rotate(-1 120 48)" />
              <g transform="translate(20 22)">
                <rect x="0" y="0" width="40" height="40" rx="11" fill="#e0502a" stroke="#2b2418" stroke-width="2" />
                <circle cx="20" cy="18" r="6.5" fill="#f7f2e7" /><path d="M12.5 30 c4 -5.5 11 -5.5 15 0" fill="none" stroke="#f7f2e7" stroke-width="2.6" stroke-linecap="round" />
              </g>
              <text x="40" y="80" text-anchor="middle" font-size="11" font-weight="800" fill="#2b2418">Yappy</text>
              {#each [80, 132, 184] as x, i (x)}
                <rect x={x} y="22" width="40" height="40" rx="11" fill={["#f3ecdb", "#e6dcc6", "#f3ecdb"][i]} stroke="#82755a" stroke-width="1.6" stroke-dasharray="4 3" />
              {/each}
            </svg>
            {#if leccionCompartir === "articulos"}
              <!-- EL ENLACE RECOMENDADO: un enlace de verdad, como los que
                   llegan por WhatsApp. Tocarlo abre Safari y el loro sale
                   en PiP a señalar el botón de compartir. -->
              <button class="enlace-cuento" use:presionable={{ hap: "rigid" }} onclick={abrirRecomendado}>
                <span class="enlace-icono" aria-hidden="true"><Criatura size={38} tinta={$tintaVoz} mirando={-1} /></span>
                <span class="enlace-texto">
                  <span class="enlace-titulo">{paseo.cuentos[0]?.titulo ?? $t("paseo.compartir.recomendado")}</span>
                  <span class="enlace-url">{urlRecomendado.replace("https://", "")}</span>
                </span>
                <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M7 17 L17 7 M9 7 h8 v8"/></svg>
              </button>
              <p class="yap-susurro pista">{$t("paseo.compartir.enlace_pista")}</p>
            {:else}
              <button class="yap-tecla grande" use:presionable={{ hap: "rigid" }} onclick={valeVoy}>{$t("paseo.compartir.voy")}</button>
            {/if}
            <p class="yap-susurro pista">{pipVivo ? $t("paseo.compartir.te_espero") : $t("paseo.compartir.pista")}</p>
            <div class="enlace-fila">
              <input class="yap-campo" type="url" bind:value={enlaceManual} placeholder={$t("cinta.enlace_pista")} enterkeyhint="go" autocapitalize="off" autocorrect="off" spellcheck="false" onkeydown={(e) => e.key === "Enter" && pegarEnlaceManual()} />
              <button class="yap-boton" use:presionable onclick={pegarEnlaceManual} disabled={!enlaceManual.trim()}>{$t("cinta.a_la_cola")}</button>
            </div>
          </div>

        {:else if paso === "exportar"}
          <div class="escena">
            <Criatura size={84} cantando={paseo.hablando} tinta={$tintaVoz} />
            <h1 class="yap-grito titulo">{$t("paseo.exportar.titulo")}</h1>
            <p class="lectura">{$t("paseo.dice.exportar")}</p>
            <!-- La bobina: lo escuchado se queda como audiolibro con capítulos. -->
            <svg class="bobina" viewBox="0 0 200 110" aria-hidden="true">
              <g stroke="#2b2418" stroke-width="2" stroke-linejoin="round">
                <rect x="10" y="16" width="120" height="78" rx="10" fill={$tintaVoz} transform="rotate(-2 70 55)" />
                <circle cx="160" cy="55" r="34" fill="#f3ecdb" />
                <circle cx="160" cy="55" r="22" fill="none" stroke="#82755a" stroke-width="1.6" stroke-dasharray="5 4" />
                <circle cx="160" cy="55" r="7" fill="#fffdf7" />
              </g>
              <path d="M26 36 h84 M26 50 h70 M26 64 h84 M26 78 h52" stroke="#fffdf7" stroke-width="3" stroke-linecap="round" opacity="0.9" />
              <path d="M132 44 h18 M132 66 h18" stroke="#2b2418" stroke-width="2.4" stroke-linecap="round" />
            </svg>
            <ol class="pasos-lista">
              <li>{$t("paseo.exportar.p1")}</li>
              <li>{$t("paseo.exportar.p2")}</li>
              <li>{$t("paseo.exportar.p3")}</li>
            </ol>
            <button class="yap-tecla grande" use:presionable={{ hap: "rigid" }} onclick={siguiente}>{$t("paseo.sigue")}</button>
          </div>

        {:else if paso === "avisos"}
          <div class="escena">
            <Criatura size={84} cantando={paseo.hablando} tinta={$tintaVoz} mirada={{ x: 0.4, y: -0.6 }} />
            <h1 class="yap-grito titulo">{$t("paseo.avisos.titulo")}</h1>
            <p class="lectura">{$t("paseo.dice.avisos")}</p>
            <!-- El aviso dibujado a mano: el globo del loro con el libro hecho. -->
            <svg class="globo-aviso" viewBox="0 0 240 96" aria-hidden="true">
              <rect x="8" y="10" width="224" height="66" rx="18" fill="#fffdf7" stroke="#2b2418" stroke-width="2" transform="rotate(-1.2 120 43)" />
              <path d="M40 76 l-10 16 l22 -14" fill="#fffdf7" stroke="#2b2418" stroke-width="2" stroke-linejoin="round" />
              <rect x="24" y="24" width="38" height="38" rx="11" fill={$tintaVoz} stroke="#2b2418" stroke-width="2" />
              <path d="M34 43 q9 -9 18 0 M36 50 h14" stroke="#fffdf7" stroke-width="3" stroke-linecap="round" fill="none" />
              <path d="M76 34 h110 M76 48 h84 M76 62 h60" stroke="#82755a" stroke-width="4" stroke-linecap="round" opacity="0.55" />
              <path d="M200 20 l3 7 7 1 -5 5 1 7 -6 -3 -6 3 1 -7 -5 -5 7 -1 Z" fill="#e8b41a" stroke="#2b2418" stroke-width="1.5" />
            </svg>
            <p class="lectura chica">{$t("paseo.avisos.texto")}</p>
            {#if avisosEstadoN === 1}
              <p class="lectura ok">{$t("paseo.avisos.gracias")}</p>
              <button class="yap-tecla grande" use:presionable={{ hap: "rigid" }} onclick={siguiente}>{$t("paseo.sigue")}</button>
            {:else if avisosEstadoN === 2}
              <p class="lectura chica">{$t("paseo.avisos.denegado")}</p>
              <button class="yap-tecla grande" use:presionable={{ hap: "rigid" }} onclick={siguiente}>{$t("paseo.sigue")}</button>
            {:else}
              <button class="yap-tecla grande" use:presionable={{ hap: "rigid" }} disabled={avisosPedidos} onclick={pedirAvisos}>{$t("paseo.avisos.tecla")}</button>
              <button class="enlace" use:presionable={{ hap: "soft" }} onclick={siguiente}>{$t("paseo.avisos.luego")}</button>
            {/if}
          </div>

        {:else if paso === "favoritos"}
          <div class="escena">
            <h1 class="yap-grito titulo">{$t("paseo.favoritos.titulo")}</h1>
            <p class="lectura">{$t("paseo.dice.favoritos")}</p>
            <!-- La hoja de compartir dibujada a mano: Yappy al principio, con su estrella. -->
            <svg class="hoja-compartir" viewBox="0 0 240 120" aria-hidden="true">
              <rect x="6" y="8" width="228" height="104" rx="16" fill="#fffdf7" stroke="#2b2418" stroke-width="2" transform="rotate(-1 120 60)" />
              <g transform="translate(24 30)">
                <rect x="0" y="0" width="44" height="44" rx="12" fill="#e0502a" stroke="#2b2418" stroke-width="2" />
                <circle cx="22" cy="20" r="7" fill="#f7f2e7" /><path d="M14 33 c4 -6 12 -6 16 0" fill="none" stroke="#f7f2e7" stroke-width="2.6" stroke-linecap="round" />
                <path d="M46 -6 l3 7 7 1 -5 5 1 7 -6 -3 -6 3 1 -7 -5 -5 7 -1 Z" fill="#e8b41a" stroke="#2b2418" stroke-width="1.5" />
              </g>
              {#each [90, 146, 202] as x, i (x)}
                <rect x={x} y="30" width="44" height="44" rx="12" fill={["#f3ecdb", "#e6dcc6", "#f3ecdb"][i]} stroke="#82755a" stroke-width="1.6" stroke-dasharray="4 3" />
              {/each}
              <path d="M28 96 h40 M98 96 h30 M154 96 h30 M210 96 h20" stroke="#82755a" stroke-width="3" stroke-linecap="round" opacity="0.6" />
            </svg>
            <ol class="pasos-lista">
              <li>{$t("paseo.favoritos.p1")}</li>
              <li>{$t("paseo.favoritos.p2")}</li>
              <li>{$t("paseo.favoritos.p3")}</li>
            </ol>
            <button class="yap-tecla grande" use:presionable={{ hap: "rigid" }} onclick={favoritosHecho}>{$t("paseo.favoritos.hecho")}</button>
          </div>

        {:else if paso === "resumen"}
          <div class="escena">
            <Criatura size={84} cantando={paseo.hablando} tinta={$tintaVoz} />
            <h1 class="yap-grito titulo">{$t("paseo.resumen.titulo")}</h1>
            <div class="ficha-loro" style="--tinta:{$tintaVoz}" in:scale={{ start: 0.86, duration: 520, easing: backOut }}>
              <span class="ficha-costura" aria-hidden="true"></span>
              <p class="linea"><span class="yap-susurro">{$t("paseo.resumen.voz")}</span><strong>{vozElegida}</strong></p>
              <p class="linea"><span class="yap-susurro">{$t("ajustes.velocidad")}</span><strong>{velocidad.toFixed(2).replace(".", ",")}×</strong></p>
              {#if etiquetaCuando}<p class="linea"><span class="yap-susurro">{$t("paseo.resumen.cuando")}</span><strong>{etiquetaCuando}</strong></p>{/if}
              {#if etiquetasQue}<p class="linea"><span class="yap-susurro">{$t("paseo.resumen.que")}</span><strong>{etiquetasQue}</strong></p>{/if}
              <p class="linea"><span class="yap-susurro">{$t("paseo.resumen.oido")}</span><strong>{oido}</strong></p>
              <p class="linea"><span class="yap-susurro">{$t("paseo.resumen.percha")}</span><strong>{propias} / {compras.cuota.limite}</strong></p>
            </div>
            <p class="lectura chica">{$t("paseo.dice.resumen")}</p>
            <button class="yap-tecla grande" use:presionable={{ hap: "rigid" }} onclick={siguiente}>{$t("paseo.sigue")}</button>
          </div>

        {:else if paso === "percha"}
          <div class="escena">
            <div class="percha-dibujo" aria-hidden="true">
              <Criatura size={110} cantando={paseo.hablando} tinta={$tintaVoz} />
              <span class="barra"></span>
              <div class="huecos">
                {#each Array.from({ length: compras.cuota.limite }, (_, i) => i) as i (i)}
                  {@const lleno = i < propias}
                  <span class="hueco" class:lleno style="--giro:{(i % 2 ? 1 : -1) * (3 + i)}deg">
                    <span class="hueco-pega" style="background: {PALETA[(i * 4 + 4) % PALETA.length]}; clip-path: {aPoligono(TROQUELES_BASE[(i * 3) % TROQUELES_BASE.length])}"></span>
                    <svg viewBox="0 0 100 100" preserveAspectRatio="none"><polygon points={aPuntosSvg(TROQUELES_BASE[(i * 3) % TROQUELES_BASE.length], lleno ? 0.8 : 0.96)} fill="none" stroke={lleno ? "#fffdf7" : "var(--yap-tinta-suave)"} stroke-width="1.6" stroke-dasharray="4 3" vector-effect="non-scaling-stroke" /></svg>
                  </span>
                {/each}
              </div>
            </div>
            <h1 class="yap-grito titulo">{$t("paseo.percha.titulo")}</h1>
            <p class="lectura">{$t("paseo.dice.percha")}</p>
            <button class="yap-tecla grande" use:presionable={{ hap: "rigid" }} onclick={siguiente}>{$t("paseo.sigue")}</button>
          </div>

        {:else if paso === "cuerda"}
          <div class="escena">
            <Criatura size={120} cantando tinta={$tintaVoz} />
            <h1 class="yap-grito titulo">{$t("paseo.cuerda.titulo")}</h1>
          </div>
        {/if}
      </section>
    {/key}

    <!-- El loro camina por el borde inferior: su sitio es el progreso. -->
    <div class="paseante" style="left: calc(6% + {paseo.paso / ultimo} * 80%); transition: left {SERENO};" aria-hidden="true">
      <Criatura size={46} andando mirando={-1} tinta={$tintaVoz} cantando={paseo.hablando} />
    </div>
    <div class="suelo" aria-hidden="true"></div>
  {/if}
</div>

<style>
  .paseo {
    position: relative;
    min-height: 100dvh;
    padding: calc(env(safe-area-inset-top) + 14px) 14px calc(env(safe-area-inset-bottom) + 92px);
    display: flex;
    align-items: flex-start;
    justify-content: center;
    overflow-x: hidden;
  }
  .luego {
    position: fixed;
    top: calc(env(safe-area-inset-top) + 12px);
    right: 16px;
    z-index: 5;
    border: 0;
    background: transparent;
    font-family: var(--yap-mono);
    font-size: 0.72rem;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--yap-tinta-suave);
    padding: 10px 12px;
    min-height: 40px;
  }
  .pagina {
    position: relative;
    width: min(100%, 460px);
    margin-top: 28px;
    background: var(--yap-superficie);
    border: 1.5px solid var(--yap-borde);
    border-radius: 26px;
    box-shadow: 4px 5px 0 #ded7c2;
    padding: 26px 18px 22px;
    transform: rotate(-0.4deg);
  }
  :global([data-theme="dark"]) .pagina { box-shadow: 4px 5px 0 rgba(0, 0, 0, 0.5); }
  .celo {
    position: absolute;
    width: 54px;
    height: 16px;
    background: rgba(232, 180, 26, 0.35);
    border: 1px solid rgba(232, 180, 26, 0.5);
    top: -7px;
    left: 28px;
    transform: rotate(-8deg);
  }
  .escena {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    text-align: center;
  }
  .titulo {
    margin: 2px 0 0;
    font-size: clamp(22px, 6.4vw, 27px);
    color: var(--yap-tinta);
    transform: rotate(-0.8deg);
    text-wrap: balance;
  }
  .lectura {
    margin: 0;
    font-family: var(--yap-lectura);
    font-size: 16px;
    line-height: 1.5;
    color: var(--yap-tinta);
    max-width: 34ch;
    text-wrap: pretty;
  }
  .lectura.chica { font-size: 14px; color: var(--yap-tinta-suave); }
  .pista { margin: 0; }
  .yap-tecla.grande {
    margin-top: 6px;
    width: 100%;
    min-height: 54px;
    font-size: 17px;
    font-weight: 800;
    border-radius: 18px;
    background: var(--vivo-paseo, var(--yap-voz));
    color: #fffdf7;
    box-shadow: 3px 4px 0 #2b2418;
    transform: rotate(-0.4deg);
  }
  .enlace {
    border: 0;
    background: transparent;
    color: var(--yap-ultramar);
    font: inherit;
    font-weight: 700;
    font-size: 13.5px;
    text-decoration: underline;
    text-underline-offset: 4px;
    text-decoration-thickness: 2px;
    padding: 8px 10px;
    min-height: 40px;
  }

  /* Los sellos de idioma. */
  .sellos {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 8px;
    max-height: 260px;
    overflow-y: auto;
    padding: 4px 2px;
  }
  .sello {
    position: relative;
    border: 1.5px solid var(--yap-tinta);
    background: var(--yap-papel);
    color: var(--yap-tinta);
    font-family: var(--yap-mono);
    font-size: 11px;
    letter-spacing: 0.06em;
    padding: 7px 10px;
    border-radius: 8px;
    transform: rotate(var(--giro));
    box-shadow: 2px 2px 0 #ded7c2;
    min-height: 36px;
  }
  .sello.tocado { background: var(--tinta); color: #fffdf7; border-color: var(--tinta); }
  .sello.elegido { outline: 2.5px solid #e8b41a; outline-offset: 1px; }
  .visto { margin-left: 5px; font-weight: 800; }

  /* Los cromos de voz. */
  .cromos {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(88px, 1fr));
    gap: 8px;
    width: 100%;
  }
  .cromo {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 8px 4px 8px;
    border-radius: 16px;
    border: 2px solid var(--yap-borde);
    background: var(--yap-superficie);
    color: var(--yap-tinta);
    box-shadow: var(--yap-relieve);
    font-size: 12.5px;
    font-weight: 800;
  }
  .cromo.elegido { border-color: var(--vivo-paseo, var(--yap-voz)); transform: rotate(-1.4deg) scale(1.04); }
  .parche { display: inline-flex; align-items: center; justify-content: center; width: 68px; height: 68px; border-radius: 50%; outline: 2px dashed rgba(43, 36, 24, 0.25); outline-offset: -6px; }

  /* Las pegatinas de respuesta. */
  .pegatinas {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 6px;
    width: 100%;
  }
  .pegatinas.cuatro { grid-template-columns: repeat(2, 1fr); }
  .pegatinas.tres { grid-template-columns: repeat(3, 1fr); }
  .pegatinas.cuentos { grid-template-columns: repeat(auto-fit, minmax(120px, 1fr)); }
  .pegatina {
    position: relative;
    height: 104px;
    border: 0;
    background: transparent;
    padding: 0;
    color: #fffdf7;
    font: inherit;
    transform: rotate(var(--giro)) scale(0.96);
    transition: transform 0.42s cubic-bezier(0.24, 1.7, 0.44, 1), filter 0.2s;
    filter: saturate(0.7) brightness(1.02);
  }
  .pegatina.cuento { height: 126px; }
  .pegatina.activa { transform: rotate(var(--giro)) scale(1.06); filter: none; z-index: 1; }
  .capa { position: absolute; inset: 0; clip-path: var(--clip); }
  .capa.sombra { background: #ded7c2; transform: translate(3px, 4px); }
  .capa.borde { background: var(--yap-superficie); }
  .capa.cuerpo { background: var(--tinta); clip-path: var(--clip-in); }
  .costura { position: absolute; inset: 0; width: 100%; height: 100%; pointer-events: none; }
  .etiqueta {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
    padding: 18px 12px;
    font-weight: 800;
    font-size: 13px;
    line-height: 1.1;
    text-shadow: 0 1px 0 var(--honda);
  }
  .etiqueta small { font-family: var(--yap-mono); font-size: 9px; letter-spacing: 0.06em; text-transform: uppercase; margin-top: 4px; opacity: 0.95; }

  .dial { width: 100%; display: flex; flex-direction: column; gap: 6px; }
  .valor { font-family: var(--yap-mono); font-size: 1.4rem; font-weight: 700; color: var(--yap-tinta); }

  .enlace-fila { display: flex; gap: 8px; width: 100%; }
  .enlace-fila input { flex: 1; min-width: 0; }

  .casillas { list-style: none; margin: 0; padding: 0; width: 100%; display: flex; flex-direction: column; gap: 8px; text-align: left; }
  .casillas li { display: flex; align-items: center; gap: 10px; font-weight: 700; font-size: 15px; color: var(--yap-tinta-suave); }
  .casillas li.hecho { color: var(--yap-tinta); }
  .casilla { display: inline-flex; align-items: center; justify-content: center; width: 26px; height: 26px; border: 2px solid var(--yap-tinta); border-radius: 8px; font-weight: 800; color: var(--yap-ok); background: var(--yap-papel); transform: rotate(-3deg); }

  .hueco-pip { width: 150px; height: 150px; display: flex; align-items: flex-end; justify-content: center; }
  /* El enlace recomendado: una tarjeta-enlace, azul y subrayada como un
     enlace de verdad, con el loro en la esquina. */
  .enlace-cuento {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 12px 14px;
    border: 1.5px solid var(--yap-tinta, #2b2418);
    border-radius: 16px 13px 17px 12px / 13px 17px 12px 16px;
    background: var(--yap-superficie);
    color: var(--yap-tinta);
    box-shadow: 3px 3px 0 #ded7c2;
    text-align: left;
    font: inherit;
    cursor: pointer;
    transform: rotate(-0.8deg);
  }
  .enlace-icono { flex: 0 0 auto; }
  .enlace-texto { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
  .enlace-titulo { font-weight: 800; font-size: 15.5px; line-height: 1.15; }
  .enlace-url {
    font-family: var(--yap-mono, ui-monospace, monospace);
    font-size: 11.5px;
    color: var(--yap-ultramar, #2f4bc4);
    text-decoration: underline;
    text-underline-offset: 3px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .enlace-cuento svg { color: var(--yap-ultramar, #2f4bc4); flex: 0 0 auto; }
  .hueco-pip.vivo { border: 2px dashed var(--yap-borde); border-radius: 18px; }

  .hoja-compartir { width: 100%; max-width: 300px; }
  .bobina { width: 100%; max-width: 260px; }
  .pasos-lista { margin: 0; padding-left: 20px; text-align: left; font-size: 14.5px; line-height: 1.5; color: var(--yap-tinta); display: flex; flex-direction: column; gap: 4px; }

  .ficha-loro {
    position: relative;
    width: 100%;
    background: var(--yap-papel);
    border: 2px solid var(--tinta);
    border-radius: 18px;
    padding: 16px 14px 12px;
    box-shadow: 3px 4px 0 #ded7c2;
    transform: rotate(-1.2deg);
    display: flex;
    flex-direction: column;
    gap: 6px;
    text-align: left;
  }
  .ficha-costura { position: absolute; inset: 6px; border: 1.5px dashed color-mix(in srgb, var(--tinta) 60%, transparent); border-radius: 12px; pointer-events: none; }
  .linea { margin: 0; display: flex; justify-content: space-between; align-items: baseline; gap: 10px; font-size: 15px; color: var(--yap-tinta); border-bottom: 1.5px dotted var(--yap-borde); padding-bottom: 4px; }
  .linea strong { text-align: right; }

  .percha-dibujo { position: relative; display: flex; flex-direction: column; align-items: center; }
  .percha-dibujo .barra { width: 200px; height: 10px; border-radius: 5px; background: #b8651f; border: 1.5px solid #2b2418; margin-top: -8px; }
  .huecos { display: flex; gap: 14px; margin-top: 8px; }
  .hueco { position: relative; width: 44px; height: 44px; transform: rotate(var(--giro)); }
  .hueco-pega { position: absolute; inset: 0; opacity: 0; transform: scale(0.6); transition: opacity 0.4s ease, transform 0.42s cubic-bezier(0.24, 1.7, 0.44, 1); }
  .hueco.lleno .hueco-pega { opacity: 1; transform: scale(1); }
  .hueco svg { position: absolute; inset: 0; width: 100%; height: 100%; }

  .paseante { position: fixed; bottom: calc(env(safe-area-inset-bottom) + 26px); z-index: 4; pointer-events: none; }
  .suelo { position: fixed; left: 8%; right: 8%; bottom: calc(env(safe-area-inset-bottom) + 24px); height: 2px; border-bottom: 2px dashed var(--yap-borde); pointer-events: none; }

  @media (prefers-reduced-motion: reduce) {
    .paseante { transition: none !important; }
    .pegatina { transition: none; }
  }
  .globo-aviso { width: min(78vw, 300px); height: auto; }
  .lectura.chica { font-size: 14px; color: var(--yap-tinta-suave, #82755a); }
  .lectura.ok { font-weight: 800; color: var(--yap-ok, #2f7a4a); }
</style>
