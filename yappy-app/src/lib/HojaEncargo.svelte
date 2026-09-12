<script lang="ts">
  // LA HOJA DE ENCARGO: brota sobre la percha desde una pieza (segunda
  // fila, menú de mantener pulsado o el aviso «esto es un libro»). Aquí se
  // decide QUÉ se imprime (los capítulos), CÓMO (uno o varios audiolibros)
  // y DÓNDE (este aparato o el ordenador). Misma hoja, mismo papel y misma
  // tinta que el menú de la pieza: la imprenta no es un sitio, es un gesto.
  import { onMount } from "svelte";
  import { backOut } from "svelte/easing";
  import { fade, fly } from "svelte/transition";
  import { t } from "$lib/i18n";
  import { haptic } from "$lib/haptic";
  import { presionable } from "$lib/presionable";
  import Criatura from "$lib/Criatura.svelte";
  import { tintaVoz } from "$lib/voces";
  import { troquelBase, aPuntosSvg } from "$lib/troquel";
  import { tonoHondo } from "$lib/juguete";
  import { exigeParlanchin, esErrorParlanchin, abrirPaywall } from "$lib/compras.svelte";
  import { capitulosDe, aMarkdown, minutosDe, rangoDicho, type Capitulo } from "$lib/capitulos";
  import { readDocument, imprentaEncargar, puenteMovilEstado, type ItemCola, type MotorEncargo } from "$lib/ipc";

  let {
    pieza,
    tinta,
    alCerrar,
    alEncargado,
  }: {
    pieza: ItemCola;
    tinta: string;
    alCerrar: () => void;
    alEncargado: (cuantos: number) => void;
  } = $props();

  let cargando = $state(true);
  let capitulos: Capitulo[] = $state([]);
  let elegidos: Set<number> = $state(new Set());
  let unoPorCapitulo = $state(false);
  let motor: MotorEncargo = $state("local");
  let puenteVinculado = $state(false);
  let error: string | null = $state(null);
  let encargando = $state(false);
  let parrafos: string[] = [];
  let kinds: string[] = [];

  const honda = $derived(tonoHondo(tinta));
  const nElegidos = $derived(capitulos.filter((c) => elegidos.has(c.n)).length);
  const charsElegidos = $derived(capitulos.filter((c) => elegidos.has(c.n)).reduce((n, c) => n + c.chars, 0));

  function conHuecos(clave: string, vars: Record<string, string | number>): string {
    let out = $t(clave);
    for (const [k, v] of Object.entries(vars)) out = out.replaceAll(`{${k}}`, String(v));
    return out;
  }

  onMount(async () => {
    puenteVinculado = !!(await puenteMovilEstado().catch(() => null))?.token;
    try {
      const doc = await readDocument(pieza.ruta ?? "");
      parrafos = doc.paragraphs ?? [];
      kinds = doc.paragraph_kinds ?? parrafos.map(() => "paragraph");
      capitulos = capitulosDe(parrafos, kinds, pieza.titulo);
      elegidos = new Set(capitulos.map((c) => c.n));
      // Un libro de verdad (varios capítulos, más de media hora) se
      // encarga por capítulos: cada uno es un encargo corto.
      const minutos = minutosDe(capitulos.reduce((n, c) => n + c.chars, 0));
      unoPorCapitulo = capitulos.length > 1 && minutos > 35;
    } catch (e) {
      error = String(e);
    } finally {
      cargando = false;
    }
  });

  function alternar(n: number) {
    haptic("light");
    const s = new Set(elegidos);
    if (s.has(n)) s.delete(n);
    else s.add(n);
    elegidos = s;
  }
  function todos(si: boolean) {
    haptic("light");
    elegidos = new Set(si ? capitulos.map((c) => c.n) : []);
  }

  async function encargar() {
    if (encargando) return;
    const caps = capitulos.filter((c) => elegidos.has(c.n)).sort((a, b) => a.n - b.n);
    if (caps.length === 0) {
      error = $t("imprenta.elige_alguno");
      return;
    }
    // La imprenta es cosa de parlanchines: sin cuerda, el paywall.
    if (!exigeParlanchin(motor === "ordenador" ? "puente" : "imprenta")) return;
    haptic("medium");
    encargando = true;
    error = null;
    try {
      const entero = caps.length === capitulos.length;
      let cuantos = 0;
      if (unoPorCapitulo && capitulos.length > 1) {
        for (const c of caps) {
          const titulo = `${pieza.titulo} · ${c.n + 1}. ${c.titulo}`.slice(0, 90);
          await imprentaEncargar(titulo, aMarkdown(parrafos, kinds, c.desde, c.hasta), { motor });
          cuantos += 1;
        }
      } else {
        const texto = caps.map((c) => aMarkdown(parrafos, kinds, c.desde, c.hasta)).join("\n\n");
        const titulo = entero
          ? pieza.titulo
          : `${pieza.titulo} · ${$t("imprenta.cap_corto")} ${rangoDicho(caps.map((c) => c.n))}`;
        await imprentaEncargar(titulo.slice(0, 90), texto, { motor });
        cuantos = 1;
      }
      alEncargado(cuantos);
    } catch (e) {
      if (esErrorParlanchin(e)) abrirPaywall(motor === "ordenador" ? "puente" : "imprenta");
      else error = String(e);
    } finally {
      encargando = false;
    }
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && alCerrar()} />
<div class="velo" role="presentation" transition:fade={{ duration: 180 }} onclick={alCerrar}></div>
<div
  class="hoja"
  role="dialog"
  aria-modal="true"
  aria-label={$t("imprenta.encargar_titulo")}
  style="--tinta: {tinta}; --honda: {honda}"
  in:fly={{ y: 260, duration: 340, easing: backOut }}
  out:fly={{ y: 260, duration: 220 }}
>
  <div class="hoja-asa"></div>
  <button class="cerrar cruz-gira" use:presionable={{ hap: "soft" }} onclick={alCerrar} aria-label={$t("comun.cerrar")}>＋</button>

  <!-- LA PEGATINA DE LA PIEZA, grande, con el loro asomado detrás. -->
  <div class="cabeza">
    <div class="pegatina" aria-hidden="true">
      <div class="loro-asomado"><Criatura size={64} tinta={$tintaVoz} mirando={-1} mirada={{ x: -0.3, y: 0.8 }} cantando={encargando} /></div>
      <svg class="pega-sombra" viewBox="0 0 100 100" aria-hidden="true"><polygon points={aPuntosSvg(troquelBase(pieza.id))} fill="#ded7c2" /></svg>
      <svg class="pega-cuerpo" viewBox="0 0 100 100" aria-hidden="true">
        <polygon points={aPuntosSvg(troquelBase(pieza.id))} fill="#f7f2e7" />
        <polygon points={aPuntosSvg(troquelBase(pieza.id), 0.93)} fill={tinta} />
        <polygon points={aPuntosSvg(troquelBase(pieza.id), 0.85)} fill="none" stroke="rgba(247,242,231,0.75)" stroke-width="1.2" stroke-dasharray="2.6 2.2" />
      </svg>
      <span class="pega-titulo">{pieza.titulo}</span>
    </div>
    <div class="cabeza-texto">
      <span class="susurro">{$t("imprenta.encargar_titulo")}</span>
      <p class="pista">{$t("imprenta.nota_fondo")}</p>
    </div>
  </div>

  {#if cargando}
    <div class="cargando">
      <Criatura size={56} tinta={$tintaVoz} estado="comiendo" barriga={0.4} mirando={-1} />
      <p class="pista">{$t("imprenta.cargando")}</p>
    </div>
  {:else}
    {#if capitulos.length > 1}
      <div class="fila">
        <span class="susurro">{$t("imprenta.capitulos")} · {capitulos.length}</span>
        <span class="todos">
          <button class="enlace" onclick={() => todos(true)}>{$t("imprenta.todos")}</button>
          ·
          <button class="enlace" onclick={() => todos(false)}>{$t("imprenta.ninguno")}</button>
        </span>
      </div>
      <ul class="caps" aria-label={$t("imprenta.capitulos")}>
        {#each capitulos as c (c.n)}
          <li>
            <button class="cap" class:elegido={elegidos.has(c.n)} use:presionable={{ hap: "light" }} onclick={() => alternar(c.n)} aria-pressed={elegidos.has(c.n)}>
              <span class="casilla">{elegidos.has(c.n) ? "✓" : ""}</span>
              <span class="cap-n">{c.n + 1}</span>
              <span class="cap-titulo">{c.titulo}</span>
              <span class="cap-min">~{minutosDe(c.chars)}′</span>
            </button>
          </li>
        {/each}
      </ul>
      <button class="conmutador" class:activo={unoPorCapitulo} use:presionable={{ hap: "light" }} onclick={() => (unoPorCapitulo = !unoPorCapitulo)} aria-pressed={unoPorCapitulo}>
        <span class="casilla">{unoPorCapitulo ? "✓" : ""}</span>
        <span>{$t("imprenta.uno_por_capitulo")}</span>
      </button>
    {/if}

    <!-- DÓNDE: dos sellos, el teléfono y el ordenador. -->
    <div class="fila"><span class="susurro">{$t("imprenta.donde")}</span></div>
    <div class="sellos">
      <button class="sello" class:activo={motor === "local"} use:presionable onclick={() => (motor = "local")} aria-pressed={motor === "local"}>
        <svg viewBox="0 0 24 24" width="26" height="26" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="7" y="2.8" width="10" height="18.4" rx="2.6"/><path d="M10.5 18.6 h3"/></svg>
        <span>{$t("imprenta.motor_aqui")}</span>
      </button>
      <button class="sello" class:activo={motor === "ordenador"} use:presionable disabled={!puenteVinculado} onclick={() => (motor = "ordenador")} aria-pressed={motor === "ordenador"}>
        <svg viewBox="0 0 24 24" width="26" height="26" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="3" y="4.5" width="18" height="12" rx="2.4"/><path d="M8.5 20 h7 M12 16.5 v3.5"/></svg>
        <span>{$t("imprenta.motor_ordenador")}</span>
      </button>
    </div>
    {#if !puenteVinculado}
      <p class="pista">{$t("imprenta.ordenador_sin_vincular")}</p>
    {/if}

    {#if error}<p class="error">{error}</p>{/if}
    <button class="pestana-gorda" use:presionable={{ hap: "rigid" }} disabled={encargando || nElegidos === 0} onclick={encargar} aria-label={$t("imprenta.encargar")}>
      <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M6 8 V4.8 q0 -0.8 0.8 -0.8 h10.4 q0.8 0 0.8 0.8 V8 M4.5 8 h15 q1 0 1 1 v5.5 q0 1 -1 1 h-15 q-1 0 -1 -1 V9 q0 -1 1 -1 Z M7 15.5 h10 V19 q0 1 -1 1 H8 q-1 0 -1 -1 Z"/></svg>
      {$t("imprenta.encargar")}
      <span class="pestana-nota">
        {#if capitulos.length > 1}{nElegidos}/{capitulos.length} · {/if}~{minutosDe(charsElegidos)}′
      </span>
    </button>
  {/if}
</div>

<style>
  /* El mismo papel que el menú de la pieza de la percha. */
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
    bottom: 0;
    z-index: 51;
    max-height: 90dvh;
    overflow-y: auto;
    background: var(--yap-papel);
    border: 1.5px solid var(--yap-tinta, #2b2418);
    border-bottom: 0;
    border-radius: 26px 16px 0 0;
    box-shadow: 4px -3px 0 #ded7c2;
    padding: 12px 18px calc(env(safe-area-inset-bottom) + 18px);
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .hoja-asa {
    width: 52px;
    height: 0;
    border-top: 2.5px dashed var(--yap-borde);
    margin: 2px auto 4px;
  }
  .cerrar {
    position: absolute;
    top: 12px;
    right: 14px;
    width: 40px;
    height: 40px;
    border: 1.5px solid var(--yap-tinta, #2b2418);
    border-radius: 50%;
    background: var(--yap-superficie);
    color: var(--yap-tinta);
    font-size: 26px;
    line-height: 1;
    box-shadow: 2px 2px 0 #ded7c2;
    cursor: pointer;
    transform: rotate(45deg);
  }
  .cabeza { display: flex; align-items: center; gap: 14px; padding-top: 18px; }
  .pegatina {
    position: relative;
    width: 128px;
    height: 118px;
    flex: 0 0 auto;
    transform: rotate(-4deg);
  }
  .loro-asomado { position: absolute; top: -36px; left: 4px; z-index: 0; }
  .pega-sombra, .pega-cuerpo { position: absolute; inset: 0; width: 100%; height: 100%; }
  .pega-sombra { translate: 4px 5px; z-index: 1; }
  .pega-cuerpo { z-index: 2; }
  .pega-titulo {
    position: absolute;
    inset: 22% 16%;
    z-index: 3;
    display: flex;
    align-items: center;
    justify-content: center;
    text-align: center;
    color: #f7f2e7;
    font-weight: 800;
    font-size: 12.5px;
    line-height: 1.1;
    text-transform: uppercase;
    letter-spacing: -0.01em;
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    -webkit-box-orient: vertical;
  }
  .cabeza-texto { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 6px; padding-right: 44px; }
  .susurro {
    font-family: var(--yap-mono, ui-monospace, monospace);
    font-size: 10.5px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--yap-tinta-suave, #82755a);
  }
  .cargando { display: flex; flex-direction: column; align-items: center; gap: 8px; padding: 12px 0; }
  .pista { margin: 0; font-size: 13px; color: var(--yap-tinta-suave, #82755a); }
  .error { margin: 0; font-size: 12.5px; color: var(--yap-peligro, #9a4a3a); }
  .fila { display: flex; justify-content: space-between; align-items: center; gap: 8px; }
  .todos { font-size: 12px; color: var(--yap-tinta-suave, #82755a); }
  .enlace {
    border: 0;
    background: none;
    padding: 6px 2px;
    color: var(--yap-tinta);
    font: inherit;
    font-size: 12.5px;
    font-weight: 700;
    text-decoration: underline;
    text-underline-offset: 3px;
    cursor: pointer;
  }
  .caps { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 7px; max-height: 32dvh; overflow-y: auto; }
  /* Los capítulos son fichas del color de la pieza: elegida, se levanta. */
  .cap {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 9px;
    min-height: 46px;
    padding: 6px 10px;
    border: 1.5px solid var(--yap-borde);
    border-radius: 14px 11px 15px 12px / 12px 15px 11px 14px;
    background: var(--yap-superficie);
    color: var(--yap-tinta-suave, #82755a);
    font: inherit;
    text-align: left;
    cursor: pointer;
    transition: transform 0.16s cubic-bezier(0.3, 1.4, 0.6, 1), box-shadow 0.16s ease, background-color 0.2s ease, color 0.2s ease;
  }
  .cap.elegido {
    background: var(--tinta);
    border-color: var(--yap-tinta, #2b2418);
    color: #fffdf7;
    box-shadow: 2.5px 3px 0 #2b2418;
    transform: rotate(-0.4deg);
  }
  .cap:nth-child(even).elegido { transform: rotate(0.4deg); }
  .casilla {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border: 2px solid currentColor;
    border-radius: 8px;
    font-size: 14px;
    font-weight: 800;
    flex: 0 0 auto;
  }
  .cap-n { font-family: var(--yap-mono, ui-monospace, monospace); font-size: 12px; font-weight: 700; flex: 0 0 auto; }
  .cap-titulo { flex: 1; min-width: 0; font-size: 14px; font-weight: 800; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .cap-min { flex: 0 0 auto; font-family: var(--yap-mono, ui-monospace, monospace); font-size: 11px; opacity: 0.85; }
  .conmutador {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 12px;
    border: 1.5px dashed var(--yap-borde);
    border-radius: 14px;
    background: none;
    color: var(--yap-tinta-suave, #82755a);
    font: inherit;
    font-size: 13.5px;
    font-weight: 700;
    text-align: left;
    cursor: pointer;
  }
  .conmutador.activo { color: var(--yap-tinta); border-style: solid; border-color: var(--yap-tinta); background: var(--yap-superficie); }
  .conmutador .casilla { color: var(--yap-ok, #2f7a4a); }
  /* Los sellos del motor: redondos, con costura, en la tinta de la pieza. */
  .sellos { display: flex; gap: 14px; justify-content: center; }
  .sello {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    width: 128px;
    height: 84px;
    border: 1.5px dashed var(--yap-tinta-suave, #82755a);
    border-radius: 42% 58% 55% 45% / 50% 44% 56% 50%;
    background: var(--yap-superficie);
    color: var(--yap-tinta-suave, #82755a);
    font: inherit;
    font-size: 12.5px;
    font-weight: 800;
    cursor: pointer;
    transform: rotate(-2deg);
    transition: transform 0.16s cubic-bezier(0.3, 1.4, 0.6, 1), box-shadow 0.16s ease, background-color 0.2s ease;
  }
  .sello:nth-child(2) { transform: rotate(2deg); }
  .sello.activo {
    background: var(--tinta);
    border: 1.5px solid var(--yap-tinta, #2b2418);
    color: #fffdf7;
    box-shadow: 2.5px 3px 0 #2b2418;
  }
  .sello:disabled { opacity: 0.4; cursor: default; }
  .pestana-gorda {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
    width: 100%;
    padding: 16px 18px;
    border: 1.5px solid var(--yap-tinta, #2b2418);
    border-radius: 20px 14px 22px 15px / 15px 22px 14px 20px;
    background: var(--tinta, var(--yap-voz));
    box-shadow: 3px 4px 0 #2b2418;
    color: #f7f2e7;
    font: inherit;
    font-weight: 800;
    font-size: 18px;
    cursor: pointer;
    transform: rotate(-0.6deg);
  }
  .pestana-gorda:active { transform: translate(2px, 2px) rotate(-0.6deg); box-shadow: 1px 2px 0 #2b2418; }
  .pestana-gorda:disabled { opacity: 0.55; box-shadow: none; transform: none; }
  .pestana-nota {
    font-family: var(--yap-mono, ui-monospace, monospace);
    font-size: 11.5px;
    font-weight: 700;
    letter-spacing: 0.06em;
    opacity: 0.9;
  }
</style>
