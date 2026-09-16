<script lang="ts">
  // El selector de aspecto, como el de la trastienda de la app: un raíl
  // hundido y la elegida sube como una tecla.
  //
  // La web arranca SIEMPRE en papel, no en «como el sistema». La casa es
  // de papel crema: quien llega de una búsqueda tiene que ver el papel,
  // no una versión nocturna que no ha pedido. Quien prefiera la noche la
  // elige aquí y se le recuerda.
  let { papel = "Papel", noche = "Noche", sistema = "Como el sistema", titulo = "Aspecto" } = $props();

  const CLAVE = "yappy.tema";
  let tema = $state("papel");

  $effect(() => {
    const g = localStorage.getItem(CLAVE);
    if (g === "papel" || g === "noche" || g === "sistema") tema = g;
  });

  function elige(nuevo: string) {
    tema = nuevo;
    document.documentElement.dataset.tema = nuevo;
    try { localStorage.setItem(CLAVE, nuevo); } catch {}
  }

  const opciones = $derived([
    { id: "papel", texto: papel },
    { id: "noche", texto: noche },
    { id: "sistema", texto: sistema },
  ]);
</script>

<div class="aspecto">
  <span class="susurro" style="margin:0">{titulo}</span>
  <div class="rail" role="group" aria-label={titulo}>
    {#each opciones as o (o.id)}
      <button type="button" class:elegida={tema === o.id} aria-pressed={tema === o.id} onclick={() => elige(o.id)}>
        {o.texto}
      </button>
    {/each}
  </div>
</div>

<style>
  .aspecto { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; }
  .rail {
    display: inline-flex;
    gap: 3px;
    padding: 3px;
    border-radius: 999px;
    background: var(--superficie-2);
    border: 1.5px solid var(--borde);
    box-shadow: var(--hundido);
  }
  .rail button {
    font-family: var(--fuente);
    font-weight: 600;
    font-size: 0.86rem;
    color: var(--tinta);
    background: transparent;
    border: 0;
    border-radius: 999px;
    padding: 6px 14px;
    cursor: pointer;
    white-space: nowrap;
    transition: transform 0.16s var(--seco);
  }
  .rail button:hover { color: var(--voz); }
  .rail button.elegida {
    background: var(--tecla);
    color: var(--tecla-tinta);
    box-shadow: var(--luz), var(--sombra-dura-corta);
  }
  .rail button:active { transform: translateY(1px); }
</style>
