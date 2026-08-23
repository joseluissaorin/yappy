<script lang="ts">
  // La Biblioteca: todo lo guardado, con su progreso. Documentos (lo que
  // entró por la cola) y el estante de audiolibros renderizados.
  import { onMount, onDestroy } from "svelte";
  import { goto } from "$app/navigation";
  import { t } from "$lib/i18n";
  import { haptic } from "$lib/haptic";
  import Criatura from "$lib/Criatura.svelte";
  import IconoTipo from "$lib/IconoTipo.svelte";
  import { reader } from "$lib/readerStore.svelte";
  import { progresoDe } from "$lib/progreso";
  import {
    colaListar,
    colaEliminar,
    onColaActualizada,
    readDocument,
    readDocumentParagraphs,
    type ItemCola,
  } from "$lib/ipc";

  let items = $state<ItemCola[]>([]);
  let busqueda = $state("");
  let cleanups: (() => void)[] = [];

  const listos = $derived(
    items
      .filter((i) => i.estado === "listo")
      .filter(
        (i) =>
          !busqueda.trim() ||
          i.titulo.toLowerCase().includes(busqueda.trim().toLowerCase()) ||
          i.origen.toLowerCase().includes(busqueda.trim().toLowerCase()),
      ),
  );

  onMount(async () => {
    items = await colaListar().catch(() => []);
    cleanups.push(await onColaActualizada(async () => (items = await colaListar().catch(() => items))));
  });
  onDestroy(() => cleanups.forEach((c) => c()));

  async function abrir(item: ItemCola) {
    if (!item.ruta) return;
    haptic("light");
    try {
      const doc = await readDocument(item.ruta);
      // El lector deriva su título del nombre de fichero; la ficha tiene
      // el título de verdad (el del artículo), así que se lo prestamos.
      doc.filename = item.titulo;
      reader.doc = doc;
      await goto("/read");
      const desde = progresoDe(item.ruta)?.parrafo ?? 0;
      await readDocumentParagraphs(doc.paragraphs, Math.min(desde, Math.max(0, doc.paragraphs.length - 1)));
    } catch (e) {
      console.error(e);
    }
  }

  function pctDe(item: ItemCola): number | null {
    if (!item.ruta) return null;
    const p = progresoDe(item.ruta);
    if (!p || p.total === 0) return null;
    return Math.min(100, Math.round((p.parrafo / p.total) * 100));
  }

  function minutos(chars: number | null): string {
    if (!chars) return "";
    return `~${Math.max(1, Math.round(chars / 900))} ${$t("biblioteca.min")}`;
  }
</script>

<div class="yap-buscador buscador">
  <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/></svg>
  <input type="search" bind:value={busqueda} placeholder={$t("biblioteca.buscar")} />
</div>

<button class="yap-tarjeta es-pulsable estante" onclick={() => goto("/biblioteca/audiolibros")}>
  <span class="estante-icono"><IconoTipo tipo="audio" size={22} /></span>
  <span class="estante-texto">
    <span class="estante-titulo">{$t("biblioteca.audiolibros")}</span>
    <span class="yap-susurro">.m4b</span>
  </span>
  <span class="estante-flecha">›</span>
</button>

<h2 class="yap-susurro seccion">{$t("biblioteca.documentos")}</h2>

{#if listos.length === 0}
  <div class="yap-hueco vacia">
    <Criatura size={64} />
    <h3>{$t("biblioteca.vacia.titulo")}</h3>
    <p>{$t("biblioteca.vacia.texto")}</p>
  </div>
{:else}
  <ul class="lista">
    {#each listos as item (item.id)}
      <li>
        <div
          class="yap-ficha item es-pulsable"
          role="button"
          tabindex="0"
          onclick={() => abrir(item)}
          onkeydown={(e) => (e.key === "Enter" || e.key === " ") && abrir(item)}
        >
          <span class="item-cuerpo">
            <span class="item-titulo">{item.titulo}</span>
            <span class="item-meta">
              {minutos(item.chars)}
              {#if pctDe(item) !== null}
                · {pctDe(item)}% {$t("biblioteca.continuar")}
              {/if}
            </span>
            {#if pctDe(item) !== null}
              <span class="item-barra"><span style="width:{pctDe(item)}%"></span></span>
            {/if}
          </span>
          <button
            class="yap-boton es-fantasma"
            aria-label={$t("cola.borrar")}
            onclick={(e) => {
              e.stopPropagation();
              haptic("warning");
              colaEliminar(item.id);
            }}>✕</button
          >
        </div>
      </li>
    {/each}
  </ul>
{/if}

<style>
  .buscador {
    margin: 4px 0 12px;
  }
  .estante {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    text-align: left;
    padding: 13px 15px;
    margin-bottom: 16px;
  }
  .estante-icono {
    display: inline-flex;
    color: var(--yap-voz);
  }
  .estante-texto {
    flex: 1;
    display: flex;
    flex-direction: column;
  }
  .estante-titulo {
    font-weight: 800;
    font-size: 1rem;
  }
  .estante-flecha {
    font-size: 1.4rem;
    color: var(--yap-tinta-suave);
  }
  .seccion {
    display: block;
    margin: 4px 0 8px;
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
    gap: 8px;
    padding: 11px 12px;
  }
  .item-cuerpo {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .item-titulo {
    font-weight: 700;
    font-size: 0.94rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .item-meta {
    font-size: 0.76rem;
    color: var(--yap-tinta-suave);
  }
  .item-barra {
    height: 4px;
    border-radius: 999px;
    background: var(--yap-superficie-2);
    box-shadow: var(--yap-hundido);
    overflow: hidden;
  }
  .item-barra span {
    display: block;
    height: 100%;
    background: var(--yap-dorado);
    border-radius: 999px;
  }
</style>
