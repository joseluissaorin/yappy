<script lang="ts">
  // LA COLA DE LA IMPRENTA, como hoja sobre la percha: se abre desde el
  // chip de abajo o desde la puerta de la trastienda. Los encargos en
  // marcha con sus mandos; los recién hechos, para compartir. El audiolibro
  // terminado ya ha LLEGADO a la percha como pegatina: aquí solo se ve el
  // taller, no se escucha nada.
  import { onMount, onDestroy } from "svelte";
  import { backOut } from "svelte/easing";
  import { fade, fly } from "svelte/transition";
  import { invoke } from "@tauri-apps/api/core";
  import { t } from "$lib/i18n";
  import { presionable } from "$lib/presionable";
  import Criatura from "$lib/Criatura.svelte";
  import { tintaVoz } from "$lib/voces";
  import {
    imprentaListar,
    imprentaPausar,
    imprentaReanudar,
    imprentaCancelar,
    imprentaQuitar,
    imprentaReordenar,
    imprentaEditar,
    imprentaM4b,
    onImprentaActualizada,
    type Encargo,
  } from "$lib/ipc";

  let { alCerrar }: { alCerrar: () => void } = $props();

  let encargos: Encargo[] = $state([]);
  let limpiar: (() => void) | null = null;
  const activos = $derived(encargos.filter((e) => !["hecho", "cancelado"].includes(e.estado)));
  const hechos = $derived(encargos.filter((e) => e.estado === "hecho").slice(-4).reverse());
  const trabajando = (e: Encargo) => ["sintetizando", "codificando", "empaquetando", "descargando"].includes(e.estado);

  onMount(async () => {
    encargos = await imprentaListar().catch(() => []);
    limpiar = await onImprentaActualizada((l) => (encargos = l));
  });
  onDestroy(() => limpiar?.());

  function etiquetaEstado(e: Encargo): string {
    switch (e.estado) {
      case "en_cola": return $t("imprenta.en_cola");
      case "sintetizando": return $t("imprenta.sintetizando");
      case "codificando": return $t("imprenta.codificando");
      case "empaquetando": return $t("imprenta.empaquetando");
      case "descargando": return $t("imprenta.descargando");
      case "pausado": return $t("imprenta.pausado");
      case "error": return $t("imprenta.fallo");
      default: return e.estado;
    }
  }
  function pct(e: Encargo): number {
    if (e.estado === "descargando" && e.bytes_total > 0) return Math.round((e.bytes_hechos / e.bytes_total) * 100);
    if (e.piezas_total > 0) return Math.round((e.piezas_hechas / e.piezas_total) * 100);
    return 0;
  }
  function eta(e: Encargo): string {
    if (e.estado !== "sintetizando" || e.segundos_por_pieza <= 0 || e.piezas_total === 0) return "";
    const restante = (e.piezas_total - e.piezas_hechas) * e.segundos_por_pieza;
    if (restante < 90) return `· ~${Math.max(1, Math.round(restante))} s`;
    return `· ~${Math.round(restante / 60)} min`;
  }
  async function compartirM4b(e: Encargo) {
    if (!e.artefacto) return;
    try {
      const ruta = await imprentaM4b(e.artefacto);
      await invoke("share_file_cmd", { path: ruta });
    } catch (err) {
      console.warn("[imprenta] m4b:", err);
    }
  }
  async function compartirYappy(e: Encargo) {
    if (!e.artefacto) return;
    try {
      const { documentDir, join } = await import("@tauri-apps/api/path");
      await invoke("share_file_cmd", { path: await join(await documentDir(), e.artefacto) });
    } catch (err) {
      console.warn("[imprenta] yappy:", err);
    }
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && alCerrar()} />
<div class="velo" role="presentation" transition:fade={{ duration: 180 }} onclick={alCerrar}></div>
<div class="hoja" role="dialog" aria-modal="true" aria-label={$t("imprenta.titulo")} in:fly={{ y: 260, duration: 340, easing: backOut }} out:fly={{ y: 260, duration: 220 }}>
  <div class="hoja-asa"></div>
  <div class="cabeza">
    <div aria-hidden="true">
      <Criatura size={52} tinta={$tintaVoz} mirando={-1} estado={activos.some(trabajando) ? "comiendo" : "posado"} barriga={activos.some(trabajando) ? 0.7 : 0} />
    </div>
    <span class="titulo">{$t("imprenta.titulo")}</span>
    <button class="cerrar" use:presionable={{ hap: "soft" }} onclick={alCerrar} aria-label={$t("comun.cerrar")}>
      <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" aria-hidden="true"><path d="M6 6 L18 18 M18 6 L6 18"/></svg>
    </button>
  </div>

  {#if activos.length === 0 && hechos.length === 0}
    <p class="pista">{$t("imprenta.vacia")}</p>
  {/if}
  <ul class="fichas">
    {#each activos as e (e.id)}
      <li class="ficha" class:con-error={e.estado === "error"}>
        <div class="linea">
          <span class="ficha-titulo">{e.titulo}</span>
          <span class="motor">{e.motor === "ordenador" || e.espejo ? $t("imprenta.motor_ordenador") : $t("imprenta.motor_aqui")}</span>
        </div>
        <div class="linea sutil">
          <span>{etiquetaEstado(e)}
            {#if e.piezas_total > 0 && ["sintetizando", "en_cola", "pausado"].includes(e.estado)} · {e.piezas_hechas}/{e.piezas_total} {eta(e)}{/if}
            {#if e.estado === "descargando" && e.bytes_total > 0} · {(e.bytes_hechos / 1048576).toFixed(1)}/{(e.bytes_total / 1048576).toFixed(1)} MB{/if}
          </span>
          <span class="pct">{pct(e)}%</span>
        </div>
        <div class="barra"><div class="barra-llena" style="width: {pct(e)}%"></div></div>
        {#if e.estado === "error" && e.error}<p class="error">{e.error}</p>{/if}
        {#if trabajando(e) && e.frase_actual}<p class="teletipo">«{e.frase_actual}»</p>{/if}
        {#if !e.espejo}
          <div class="mandos">
            {#if trabajando(e) || e.estado === "en_cola"}
              <button class="mando" use:presionable onclick={() => imprentaPausar(e.id)} aria-label={$t("comun.pausa")}>
                <svg viewBox="0 0 24 24" width="15" height="15" fill="currentColor" aria-hidden="true"><rect x="6" y="5" width="4.4" height="14" rx="1.6"/><rect x="13.6" y="5" width="4.4" height="14" rx="1.6"/></svg>
              </button>
            {/if}
            {#if ["pausado", "error"].includes(e.estado)}
              <button class="mando" use:presionable={{ hap: "rigid" }} onclick={() => imprentaReanudar(e.id)} aria-label={$t("comun.reanudar")}>
                <svg viewBox="0 0 24 24" width="15" height="15" fill="currentColor" aria-hidden="true"><path d="M8 5.6 Q8.8 4.8 9.9 5.5 L18 11 Q19 12 18 12.9 L9.9 18.5 Q8.8 19.2 8.3 18 Q7.6 12 8 5.6 Z"/></svg>
              </button>
              <button class="mando" use:presionable onclick={() => imprentaEditar(e.id, { motor: e.motor === "ordenador" ? "local" : "ordenador" })} aria-label={$t("imprenta.cambiar_motor")}>
                <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M7 8 h10 M13.5 4.5 L17 8 L13.5 11.5 M17 16 H7 M10.5 12.5 L7 16 L10.5 19.5"/></svg>
              </button>
            {/if}
            {#if e.estado === "en_cola"}
              <button class="mando" use:presionable onclick={() => imprentaReordenar(e.id, -1)} aria-label={$t("imprenta.subir")}>
                <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" aria-hidden="true"><path d="M12 19 V6 M6.5 11 L12 5.5 L17.5 11"/></svg>
              </button>
            {/if}
            <button class="mando peligro" use:presionable={{ hap: "warning" }} onclick={() => imprentaCancelar(e.id)} aria-label={$t("comun.cancelar")}>
              <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" aria-hidden="true"><path d="M6 6 L18 18 M18 6 L6 18"/></svg>
            </button>
          </div>
        {/if}
      </li>
    {/each}
    {#each hechos as e (e.id)}
      <li class="ficha hecha">
        <div class="linea">
          <span class="ficha-titulo">{e.titulo}</span>
          <span class="motor">{$t("imprenta.hecho")}</span>
        </div>
        <div class="mandos">
          <button class="mando con-texto" use:presionable onclick={() => compartirYappy(e)} aria-label={$t("lector.compartir") + " .yappy"}>
            <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 15.5 V5 M8.6 8.2 L12 4.8 L15.4 8.2 M5.5 12.5 v6 q0 1.4 1.4 1.4 h10.2 q1.4 0 1.4 -1.4 v-6"/></svg>
            .yappy
          </button>
          <button class="mando con-texto" use:presionable onclick={() => compartirM4b(e)} aria-label={$t("lector.compartir") + " .m4b"}>
            <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 15.5 V5 M8.6 8.2 L12 4.8 L15.4 8.2 M5.5 12.5 v6 q0 1.4 1.4 1.4 h10.2 q1.4 0 1.4 -1.4 v-6"/></svg>
            .m4b
          </button>
          <button class="mando" use:presionable onclick={() => imprentaQuitar(e.id)} aria-label={$t("comun.cerrar")}>
            <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" aria-hidden="true"><path d="M6 6 L18 18 M18 6 L6 18"/></svg>
          </button>
        </div>
      </li>
    {/each}
  </ul>
</div>

<style>
  .velo { position: fixed; inset: 0; z-index: 50; background: rgba(43, 36, 24, 0.35); }
  .hoja {
    position: fixed;
    left: 6px;
    right: 6px;
    bottom: 0;
    z-index: 51;
    max-height: 80dvh;
    overflow-y: auto;
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
  .hoja-asa { width: 52px; height: 0; border-top: 2.5px dashed var(--yap-borde); margin: 2px auto 4px; }
  .cabeza { display: flex; align-items: center; gap: 12px; }
  .titulo { flex: 1; font-weight: 800; font-size: 20px; transform: rotate(-0.6deg); }
  .cerrar {
    width: 40px;
    height: 40px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 1px solid var(--yap-borde);
    border-radius: 12px;
    background: var(--yap-superficie);
    color: var(--yap-tinta);
    box-shadow: 2px 2px 0 #ded7c2;
    cursor: pointer;
  }
  .pista { margin: 0; font-size: 13px; color: var(--yap-tinta-suave, #82755a); }
  .fichas { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 10px; }
  .ficha {
    background: var(--yap-superficie);
    border: 1.5px dashed color-mix(in srgb, var(--yap-tinta, #2b2418) 32%, transparent);
    border-radius: 14px 17px 13px 18px / 16px 13px 18px 14px;
    padding: 11px 13px;
    box-shadow: 2px 2.5px 0 #ded7c2;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .ficha.con-error { border-color: var(--yap-peligro, #9a4a3a); }
  .ficha.hecha { border-style: solid; border-color: var(--yap-dorado, #e8b41a); }
  .linea { display: flex; justify-content: space-between; align-items: baseline; gap: 8px; }
  .linea.sutil { font-size: 12px; color: var(--yap-tinta-suave, #82755a); }
  .ficha-titulo { font-weight: 800; font-size: 14.5px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; flex: 1; min-width: 0; }
  .motor { font-family: var(--yap-mono, ui-monospace, monospace); font-size: 10px; letter-spacing: 0.08em; text-transform: uppercase; color: var(--yap-tinta-suave, #82755a); flex-shrink: 0; }
  .pct { font-family: var(--yap-mono, ui-monospace, monospace); }
  .barra { height: 7px; border-radius: 5px; background: color-mix(in srgb, var(--yap-borde, #d8d0bd) 55%, transparent); overflow: hidden; }
  .barra-llena { height: 100%; border-radius: 5px; background: var(--vivo, var(--yap-voz, #e0502a)); transition: width 0.5s ease; }
  .error { margin: 0; font-size: 12px; color: var(--yap-peligro, #9a4a3a); }
  .teletipo { margin: 0; font-family: var(--yap-lectura, Georgia, serif); font-size: 13.5px; }
  .mandos { display: flex; gap: 6px; flex-wrap: wrap; }
  .mando {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    min-height: 38px;
    border: 1px solid var(--yap-borde);
    background: var(--yap-papel);
    color: var(--yap-tinta);
    border-radius: 11px;
    padding: 5px 11px;
    box-shadow: 2px 2px 0 #ded7c2;
    cursor: pointer;
    font: inherit;
    font-family: var(--yap-mono, ui-monospace, monospace);
    font-size: 11px;
    font-weight: 700;
  }
  .mando.peligro { color: var(--yap-peligro, #9a4a3a); }
</style>
