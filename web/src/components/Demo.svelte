<script lang="ts">
  // La demo sonora: seis lenguas, seis ficheros reales sintetizados por el
  // propio motor en un portátil corriente. Nada de esto es una grabación de
  // un locutor: si la web vende voz, la web tiene que sonar.
  //
  // Las frases están elegidas para que se oiga el guionizador trabajando:
  // todas llevan cifras, siglos y monedas que se dicen distinto de como se
  // escriben. Lo marcado es lo que el guionizador reescribe.
  let { etiquetaPlay = "Reproducir", nota = "" }: { etiquetaPlay?: string; nota?: string } = $props();

  const frases: Record<string, { nombre: string; html: string }> = {
    es: { nombre: "español", html: 'En <mark>1492</mark>, tres carabelas zarparon de Palos. El <mark>siglo XV</mark> terminaba, y nadie a bordo sabía que llevaban el mapa del mundo por dibujar.' },
    en: { nombre: "english", html: 'In <mark>1985</mark>, a letter arrived forty years late. <mark>Henry VIII</mark> never read it, but the <mark>21st century</mark> did.' },
    fr: { nombre: "français", html: 'Le <mark>1er</mark> janvier, <mark>Louis XIV</mark> fit servir <mark>3,50 €</mark> de chocolat chaud. Le <mark>XIXe siècle</mark> en parlerait encore.' },
    de: { nombre: "deutsch", html: 'Am <mark>24. Mai</mark> wanderten wir <mark>7 km</mark> im <mark>19. Jahrhundert</mark>. Es war <mark>14:30 Uhr</mark>, und niemand hatte es eilig.' },
    it: { nombre: "italiano", html: 'Il <mark>1º maggio</mark>, <mark>Luigi XIV</mark> assaggiò il gelato del <mark>XX secolo</mark>: <mark>3,50 €</mark> ben spesi.' },
    pt: { nombre: "português", html: 'No <mark>século XIX</mark>, D. Pedro II leu <mark>1.234</mark> páginas num verão. Hoje seriam <mark>14</mark> minutos por dia.' },
  };

  let lengua = $state("es");
  let sonando = $state(false);
  let audio: HTMLAudioElement | undefined = $state();

  function elegir(l: string) {
    lengua = l;
    sonando = false;
    if (audio) { audio.pause(); audio.currentTime = 0; }
  }
  function alternar() {
    if (!audio) return;
    if (audio.paused) { void audio.play(); sonando = true; }
    else { audio.pause(); sonando = false; }
  }
</script>

<div class="demo-idiomas" role="group" aria-label="idioma de la muestra">
  {#each Object.entries(frases) as [codigo, f] (codigo)}
    <button type="button" aria-pressed={lengua === codigo} onclick={() => elegir(codigo)}>{f.nombre}</button>
  {/each}
</div>

<p class="demo-frase" lang={lengua}>{@html frases[lengua]!.html}</p>

<div class="demo-controles">
  <button class="reproducir" type="button" onclick={alternar} aria-label={etiquetaPlay} aria-pressed={sonando}>
    {#if sonando}
      <svg viewBox="0 0 24 24" width="22" height="22" fill="currentColor" aria-hidden="true"><rect x="6" y="4" width="4.2" height="16" rx="1.4"/><rect x="13.8" y="4" width="4.2" height="16" rx="1.4"/></svg>
    {:else}
      <svg viewBox="0 0 24 24" width="22" height="22" fill="currentColor" aria-hidden="true"><path d="M8 5.2c0-.9 1-1.5 1.8-1l9 6.8c.7.5.7 1.5 0 2l-9 6.8c-.8.5-1.8 0-1.8-1z"/></svg>
    {/if}
  </button>
  <p class="demo-nota">{nota}</p>
</div>

<audio bind:this={audio} src={`/audio/demo-${lengua}.m4a`} preload="none" onended={() => (sonando = false)}></audio>
