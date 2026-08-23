<script lang="ts">
  // «Escuchar»: la raíz del móvil. Sigue-donde-ibas, la cola con estados y
  // el botón de añadir. Todo lo demás vive detrás de un toque.
  import { onMount, onDestroy } from "svelte";
  import { goto } from "$app/navigation";
  import { t } from "$lib/i18n";
  import { haptic } from "$lib/haptic";
  import Criatura from "$lib/Criatura.svelte";
  import IconoTipo from "$lib/IconoTipo.svelte";
  import { reader } from "$lib/readerStore.svelte";
  import { ultimaEscucha, progresoDe, type UltimaEscucha } from "$lib/progreso";
  import {
    colaListar,
    colaAgregarUrl,
    colaAgregarArchivo,
    colaEliminar,
    colaReintentar,
    onColaActualizada,
    readDocument,
    readDocumentParagraphs,
    readClipboard,
    isModelReady,
    downloadModel,
    onModelDownload,
    type ItemCola,
    type DownloadProgress,
  } from "$lib/ipc";
  import { open as abrirDialogo } from "@tauri-apps/plugin-dialog";

  let items = $state<ItemCola[]>([]);
  let ultima = $state<UltimaEscucha | null>(null);
  let anadirAbierto = $state(false);
  let enlaceAbierto = $state(false);
  let enlace = $state("");
  let modeloListo = $state(true);
  let descargando = $state<DownloadProgress | null>(null);
  let cleanups: (() => void)[] = [];

  const recientes = $derived(items.slice(0, 12));

  onMount(async () => {
    ultima = ultimaEscucha();
    modeloListo = await isModelReady().catch(() => true);
    items = await colaListar().catch(() => []);
    cleanups.push(await onColaActualizada(async () => (items = await colaListar().catch(() => items))));
    cleanups.push(await onModelDownload((p) => {
      descargando = p;
      if (p.stage === "done" && p.overall_done >= p.overall_total) {
        descargando = null;
        modeloListo = true;
      }
    }));
  });
  onDestroy(() => cleanups.forEach((c) => c()));

  async function abrirItem(item: ItemCola) {
    if (item.estado !== "listo" || !item.ruta) return;
    haptic("light");
    try {
      const doc = await readDocument(item.ruta);
      reader.doc = doc;
      await goto("/read");
      const desde = progresoDe(item.ruta)?.parrafo ?? 0;
      await readDocumentParagraphs(doc.paragraphs, Math.min(desde, Math.max(0, doc.paragraphs.length - 1)));
    } catch (e) {
      console.error("abrirItem:", e);
    }
  }

  async function continuar() {
    if (!ultima) return;
    haptic("light");
    try {
      const doc = await readDocument(ultima.ruta);
      reader.doc = doc;
      await goto("/read");
      await readDocumentParagraphs(doc.paragraphs, Math.min(ultima.parrafo, Math.max(0, doc.paragraphs.length - 1)));
    } catch (e) {
      console.error("continuar:", e);
    }
  }

  async function pegarEnlace() {
    const url = enlace.trim();
    if (!url) return;
    haptic("medium");
    enlace = "";
    enlaceAbierto = false;
    anadirAbierto = false;
    try {
      await colaAgregarUrl(url.startsWith("http") ? url : `https://${url}`);
    } catch (e) {
      console.error(e);
    }
  }

  async function leerPortapapeles() {
    haptic("medium");
    anadirAbierto = false;
    await readClipboard().catch(() => {});
  }

  async function abrirArchivo() {
    anadirAbierto = false;
    try {
      const ruta = await abrirDialogo({
        multiple: false,
        filters: [
          {
            name: "Documentos",
            extensions: ["txt", "md", "markdown", "rtf", "docx", "doc", "odt", "pdf", "epub", "html", "htm"],
          },
        ],
      });
      if (typeof ruta === "string") await colaAgregarArchivo(ruta);
    } catch (e) {
      console.error(e);
    }
  }

  function minutosEstimados(chars: number | null): string {
    if (!chars) return "";
    const min = Math.max(1, Math.round(chars / 900));
    return `~${min} ${$t("biblioteca.min")}`;
  }

  function iconoDe(item: ItemCola): "articulo" | "video" | "audio" | "recorte" | "documento" {
    switch (item.tipo) {
      case "youtube":
        return "video";
      case "url":
        return "articulo";
      case "audio":
        return "audio";
      case "texto":
        return "recorte";
      default:
        return "documento";
    }
  }
</script>

{#if !modeloListo}
  <section class="yap-cabecera instalar">
    <Criatura size={72} andando />
    <div>
      <h2 class="yap-grito" style="font-size:1.3rem">Las voces aún no están</h2>
      {#if descargando}
        <p class="pie">{descargando.overall_done}/{descargando.overall_total} · {Math.round((descargando.bytes_done / Math.max(1, descargando.bytes_total)) * 100)}%</p>
        <div class="barra"><span style="width:{(descargando.bytes_done / Math.max(1, descargando.bytes_total)) * 100}%"></span></div>
      {:else}
        <button class="yap-tecla" onclick={() => downloadModel()}>{$t("ajustes.descargar")} (~380 MB)</button>
      {/if}
    </div>
  </section>
{/if}

{#if ultima}
  <button class="yap-tarjeta es-pulsable sigue" onclick={continuar}>
    <span class="yap-susurro">{$t("escuchar.sigue")}</span>
    <span class="sigue-titulo">{ultima.titulo}</span>
    <span class="sigue-barra"><span
        style="width:{ultima.total_parrafos > 0 ? (ultima.parrafo / ultima.total_parrafos) * 100 : 0}%"
      ></span></span>
  </button>
{/if}

<section class="cola">
  <div class="cola-cabecera">
    <h2 class="yap-susurro">{$t("escuchar.cola")}</h2>
    <button
      class="yap-tecla anadir"
      onclick={() => {
        haptic("light");
        anadirAbierto = !anadirAbierto;
      }}>＋ {$t("escuchar.anadir")}</button
    >
  </div>

  {#if anadirAbierto}
    <div class="yap-bloque anadir-menu yap-enter">
      <button class="anadir-opcion" onclick={() => (enlaceAbierto = true)}><IconoTipo tipo="enlace" size={18} /> {$t("escuchar.pegar_enlace")}</button>
      <button class="anadir-opcion" onclick={leerPortapapeles}><IconoTipo tipo="portapapeles" size={18} /> {$t("escuchar.leer_portapapeles")}</button>
      <button class="anadir-opcion" onclick={abrirArchivo}><IconoTipo tipo="documento" size={18} /> {$t("escuchar.abrir_archivo")}</button>
    </div>
  {/if}

  {#if enlaceAbierto}
    <div class="yap-bloque anadir-menu yap-enter">
      <span class="yap-susurro">{$t("escuchar.enlace_titulo")}</span>
      <div class="enlace-fila">
        <!-- svelte-ignore a11y_autofocus -->
        <input
          class="yap-campo"
          type="url"
          bind:value={enlace}
          placeholder={$t("escuchar.enlace_placeholder")}
          autofocus
          onkeydown={(e) => e.key === "Enter" && pegarEnlace()}
        />
        <button class="yap-tecla" onclick={pegarEnlace}>{$t("escuchar.enlace_boton")}</button>
      </div>
    </div>
  {/if}

  {#if recientes.length === 0}
    <div class="yap-hueco vacia">
      <Criatura size={64} />
      <h3>{$t("escuchar.vacia.titulo")}</h3>
      <p>{$t("escuchar.vacia.texto")}</p>
    </div>
  {:else}
    <ul class="lista">
      {#each recientes as item (item.id)}
        <li>
          <div
            class="yap-ficha item"
            class:es-pulsable={item.estado === "listo"}
            role="button"
            tabindex="0"
            onclick={() => abrirItem(item)}
            onkeydown={(e) => (e.key === "Enter" || e.key === " ") && abrirItem(item)}
          >
            <span class="item-icono"><IconoTipo tipo={iconoDe(item)} size={20} /></span>
            <span class="item-cuerpo">
              <span class="item-titulo">{item.titulo}</span>
              <span class="item-estado" class:preparando={item.estado === "preparando"} class:error={item.estado === "error"}>
                {#if item.estado === "listo"}
                  {$t("estado.listo")} · {minutosEstimados(item.chars)}
                {:else if item.estado === "preparando"}
                  {$t("estado.preparando")}
                {:else if item.estado === "error"}
                  {$t("estado.error")}{item.error ? `: ${item.error}` : ""}
                {:else}
                  {$t("estado.pendiente")}
                {/if}
              </span>
            </span>
            <span class="item-acciones">
              {#if item.estado === "error"}
                <button
                  class="yap-boton es-fantasma"
                  onclick={(e) => {
                    e.stopPropagation();
                    colaReintentar(item.id);
                  }}>↻</button
                >
              {/if}
              <button
                class="yap-boton es-fantasma"
                aria-label={$t("cola.borrar")}
                onclick={(e) => {
                  e.stopPropagation();
                  haptic("warning");
                  colaEliminar(item.id);
                }}>✕</button
              >
            </span>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .instalar {
    display: flex;
    gap: 14px;
    align-items: center;
    padding: 16px;
    margin-bottom: 14px;
  }
  .instalar .pie {
    color: var(--yap-tinta-suave);
    margin: 4px 0;
  }
  .barra {
    height: 6px;
    border-radius: 999px;
    background: var(--yap-superficie-2);
    box-shadow: var(--yap-hundido);
    overflow: hidden;
  }
  .barra span {
    display: block;
    height: 100%;
    background: var(--yap-voz);
  }
  .sigue {
    display: flex;
    flex-direction: column;
    gap: 8px;
    width: 100%;
    text-align: left;
    padding: 14px 16px;
    margin-bottom: 16px;
  }
  .sigue-titulo {
    font-weight: 700;
    font-size: 1.05rem;
  }
  .sigue-barra {
    height: 5px;
    border-radius: 999px;
    background: var(--yap-superficie-2);
    box-shadow: var(--yap-hundido);
    overflow: hidden;
  }
  .sigue-barra span {
    display: block;
    height: 100%;
    background: var(--yap-dorado);
    border-radius: 999px;
  }
  .cola-cabecera {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin: 6px 0 10px;
  }
  .anadir {
    padding: 7px 13px;
    font-size: 0.88rem;
  }
  .anadir-menu {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 10px;
    margin-bottom: 10px;
  }
  .anadir-opcion {
    display: flex;
    align-items: center;
    gap: 9px;
    text-align: left;
    padding: 11px 12px;
    border-radius: 10px;
    font-weight: 600;
  }
  .anadir-opcion:active {
    background: var(--yap-superficie-2);
  }
  .enlace-fila {
    display: flex;
    gap: 8px;
    margin-top: 8px;
  }
  .enlace-fila input {
    flex: 1;
    min-width: 0;
  }
  .vacia {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 8px;
    padding: 26px 18px;
  }
  .vacia h3 {
    margin: 0;
    font-weight: 800;
  }
  .vacia p {
    margin: 0;
    color: var(--yap-tinta-suave);
    font-size: 0.9rem;
    max-width: 30ch;
  }
  .lista {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
  }
  .item-icono {
    flex: none;
    display: inline-flex;
    color: var(--yap-ultramar);
  }
  .item-cuerpo {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .item-titulo {
    font-weight: 700;
    font-size: 0.92rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .item-estado {
    font-size: 0.76rem;
    color: var(--yap-tinta-suave);
  }
  .item-estado.preparando {
    color: var(--yap-ultramar);
  }
  .item-estado.error {
    color: var(--yap-peligro);
  }
  .item-acciones {
    display: flex;
    gap: 2px;
  }
  .item-acciones .yap-boton {
    padding: 6px 9px;
  }
</style>
