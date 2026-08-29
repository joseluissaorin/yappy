<script lang="ts">
  // LA IMPRENTA: las fichas de los encargos de audiolibros. Vive dentro
  // de la biblioteca (los encargos en marcha arriba, los libros hechos
  // debajo) y es la misma en el móvil y en el escritorio.
  import { onMount, onDestroy } from "svelte";
  import { t } from "$lib/i18n";
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
  import { invoke } from "@tauri-apps/api/core";

  let encargos: Encargo[] = $state([]);
  let abierto: string | null = $state(null);
  let limpiar: (() => void) | null = null;

  const activos = $derived(
    encargos.filter((e) => !["hecho", "cancelado"].includes(e.estado)),
  );
  const recientes = $derived(
    encargos.filter((e) => e.estado === "hecho").slice(-3).reverse(),
  );

  onMount(async () => {
    encargos = await imprentaListar().catch(() => []);
    limpiar = await onImprentaActualizada((lista) => (encargos = lista));
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
    if (e.estado === "descargando" && e.bytes_total > 0)
      return Math.round((e.bytes_hechos / e.bytes_total) * 100);
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
      // El artefacto vive en la biblioteca (document_dir): compartir directo.
      const { documentDir, join } = await import("@tauri-apps/api/path");
      const ruta = await join(await documentDir(), e.artefacto);
      await invoke("share_file_cmd", { path: ruta });
    } catch (err) {
      console.warn("[imprenta] yappy:", err);
    }
  }
</script>

{#if activos.length > 0 || recientes.length > 0}
  <section class="imprenta">
    <header class="section-head">
      <h2>{$t("imprenta.titulo")}</h2>
    </header>
    <ul class="fichas" aria-label={$t("imprenta.titulo")}>
      {#each activos as e (e.id)}
        <li class="ficha" class:con-error={e.estado === "error"}>
          <button class="ficha-toque" onclick={() => (abierto = abierto === e.id ? null : e.id)}>
            <div class="ficha-linea">
              <span class="ficha-titulo">{e.titulo}</span>
              <span class="ficha-motor">{e.motor === "ordenador" || e.espejo ? $t("imprenta.motor_ordenador") : $t("imprenta.motor_aqui")}</span>
            </div>
            <div class="ficha-linea sutil">
              <span>{etiquetaEstado(e)}
                {#if e.piezas_total > 0 && ["sintetizando", "en_cola", "pausado"].includes(e.estado)}
                  · {e.piezas_hechas}/{e.piezas_total} {eta(e)}
                {/if}
                {#if e.estado === "descargando" && e.bytes_total > 0}
                  · {(e.bytes_hechos / 1048576).toFixed(1)}/{(e.bytes_total / 1048576).toFixed(1)} MB
                {/if}
              </span>
              <span class="ficha-pct">{pct(e)}%</span>
            </div>
            <div class="barra"><div class="barra-llena" style="width: {pct(e)}%"></div></div>
            {#if e.estado === "error" && e.error}
              <div class="ficha-error">{e.error}</div>
            {/if}
            {#if abierto === e.id && e.frase_actual}
              <div class="teletipo">«{e.frase_actual}»</div>
            {/if}
          </button>
          {#if !e.espejo}
            <div class="mandos">
              {#if ["sintetizando", "en_cola", "codificando", "descargando"].includes(e.estado)}
                <button class="mando" onclick={() => imprentaPausar(e.id)} aria-label={$t("comun.pausa")}>
                  <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor" aria-hidden="true"><rect x="6" y="5" width="4.4" height="14" rx="1.6"/><rect x="13.6" y="5" width="4.4" height="14" rx="1.6"/></svg>
                </button>
              {/if}
              {#if ["pausado", "error"].includes(e.estado)}
                <button class="mando" onclick={() => imprentaReanudar(e.id)} aria-label={$t("comun.reanudar")}>
                  <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor" aria-hidden="true"><path d="M8 5.6 Q8.8 4.8 9.9 5.5 L18 11 Q19 12 18 12.9 L9.9 18.5 Q8.8 19.2 8.3 18 Q7.6 12 8 5.6 Z"/></svg>
                </button>
                <button class="mando" onclick={() => imprentaEditar(e.id, { motor: e.motor === "ordenador" ? "local" : "ordenador" })} title={$t("imprenta.cambiar_motor")} aria-label={$t("imprenta.cambiar_motor")}>
                  <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M7 8 h10 M13.5 4.5 L17 8 L13.5 11.5 M17 16 H7 M10.5 12.5 L7 16 L10.5 19.5"/></svg>
                </button>
              {/if}
              {#if e.estado === "en_cola"}
                <button class="mando" onclick={() => imprentaReordenar(e.id, -1)} aria-label={$t("imprenta.subir")}>
                  <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" aria-hidden="true"><path d="M12 19 V6 M6.5 11 L12 5.5 L17.5 11"/></svg>
                </button>
              {/if}
              <button class="mando peligro" onclick={() => imprentaCancelar(e.id)} aria-label={$t("comun.cancelar")}>
                <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" aria-hidden="true"><path d="M6 6 L18 18 M18 6 L6 18"/></svg>
              </button>
            </div>
          {/if}
        </li>
      {/each}
      {#each recientes as e (e.id)}
        <li class="ficha hecha">
          <div class="ficha-linea">
            <span class="ficha-titulo">{e.titulo}</span>
            <span class="ficha-motor">{$t("imprenta.hecho")}</span>
          </div>
          <div class="mandos">
            <button class="mando" onclick={() => compartirYappy(e)} title=".yappy" aria-label={$t("lector.compartir") + " .yappy"}>
              <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 15.5 V5 M8.6 8.2 L12 4.8 L15.4 8.2 M5.5 12.5 v6 q0 1.4 1.4 1.4 h10.2 q1.4 0 1.4 -1.4 v-6"/></svg>
              <span class="mando-texto">.yappy</span>
            </button>
            <button class="mando" onclick={() => compartirM4b(e)} title=".m4b" aria-label={$t("lector.compartir") + " .m4b"}>
              <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 15.5 V5 M8.6 8.2 L12 4.8 L15.4 8.2 M5.5 12.5 v6 q0 1.4 1.4 1.4 h10.2 q1.4 0 1.4 -1.4 v-6"/></svg>
              <span class="mando-texto">.m4b</span>
            </button>
            <button class="mando" onclick={() => imprentaQuitar(e.id)} aria-label={$t("comun.cerrar")}>
              <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" aria-hidden="true"><path d="M6 6 L18 18 M18 6 L6 18"/></svg>
            </button>
          </div>
        </li>
      {/each}
    </ul>
  </section>
{/if}

<style>
  .imprenta {
    margin-bottom: 14px;
  }
  .fichas {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .ficha {
    background: var(--yap-superficie, #fdf9ee);
    border: 1.5px solid var(--yap-borde, #d8d0bd);
    border-radius: 14px 17px 13px 18px / 16px 13px 18px 14px;
    padding: 11px 13px;
    box-shadow: 2px 2.5px 0 #ded7c2;
  }
  .ficha.con-error {
    border-color: #9a4a3a;
  }
  .ficha.hecha {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    opacity: 0.92;
  }
  .ficha-toque {
    display: block;
    width: 100%;
    text-align: left;
    background: none;
    border: 0;
    padding: 0;
    cursor: pointer;
    color: inherit;
    font: inherit;
  }
  .ficha-linea {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 8px;
  }
  .ficha-titulo {
    font-weight: 700;
    font-size: 14.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ficha-motor {
    font-family: var(--yap-mono, ui-monospace, monospace);
    font-size: 10px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--yap-tinta-suave, #82755a);
    flex-shrink: 0;
  }
  .ficha-linea.sutil {
    font-size: 12px;
    color: var(--yap-tinta-suave, #82755a);
    margin-top: 3px;
  }
  .barra {
    height: 7px;
    border-radius: 5px;
    background: color-mix(in srgb, var(--yap-borde, #d8d0bd) 55%, transparent);
    overflow: hidden;
    margin-top: 6px;
  }
  .barra-llena {
    height: 100%;
    border-radius: 5px;
    background: var(--vivo, var(--yap-acento, #e4572e));
    transition: width 0.5s ease;
  }
  .ficha-error {
    margin-top: 6px;
    font-size: 12px;
    color: #9a4a3a;
  }
  .teletipo {
    margin-top: 7px;
    font-family: var(--yap-lectura, Georgia, serif);
    font-size: 13.5px;
    color: var(--yap-tinta, #2b2418);
    opacity: 0.9;
  }
  .mandos {
    display: flex;
    gap: 6px;
    margin-top: 8px;
    flex-wrap: wrap;
  }
  .ficha.hecha .mandos {
    margin-top: 0;
  }
  .mando {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    border: 1.5px solid var(--yap-borde, #d8d0bd);
    background: transparent;
    color: var(--yap-tinta, #2b2418);
    border-radius: 9px;
    padding: 5px 9px;
    cursor: pointer;
    font: inherit;
    font-size: 12px;
  }
  .mando.peligro {
    color: #9a4a3a;
    border-color: color-mix(in srgb, #9a4a3a 45%, transparent);
  }
  .mando-texto {
    font-family: var(--yap-mono, ui-monospace, monospace);
    font-size: 10.5px;
  }
</style>
