<script lang="ts">
  // La biblioteca de audiolibros vive en un componente COMPARTIDO con el
  // escritorio: un solo código, dos motores de audio. Aquí, la página
  // móvil lo viste con la cabecera de la casa (safe area, volver, título
  // ladeado) y silencia la cabecera del shell de escritorio.
  import { goto } from "$app/navigation";
  import { t } from "$lib/i18n";
  import { presionable } from "$lib/presionable";
  import BibliotecaAudiolibros from "$lib/BibliotecaAudiolibros.svelte";
</script>

<div class="pagina">
  <header class="cabecera-pagina">
    <button class="yap-boton es-fantasma volver" use:presionable={{ hap: "soft" }} onclick={() => goto("/escuchar")} aria-label={$t("lector.volver")}>
      <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><path d="M19 12H5M11 18l-6-6 6-6"/></svg>
    </button>
    <h1>{$t("biblioteca.titulo")}</h1>
  </header>
  <BibliotecaAudiolibros conCabecera={false} />
</div>

<style>
  .pagina {
    min-height: 100dvh;
    padding: calc(env(safe-area-inset-top) + 10px) 18px
      calc(env(safe-area-inset-bottom) + var(--aguja-hueco, 0px) + 28px);
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .cabecera-pagina {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 2px 0 2px;
  }
  .cabecera-pagina h1 {
    margin: 0;
    font-size: 1.5rem;
    font-weight: 800;
    transform: rotate(-0.6deg);
  }
  .volver {
    width: 44px;
    height: 44px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: 14px;
  }
</style>
