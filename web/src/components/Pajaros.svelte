<script lang="ts">
  // Los diez pájaros, y cada uno suena. La página decía «toca un pájaro y
  // te dice cómo suena» y no sonaba nada: una promesa que la página no
  // cumplía es peor que no hacerla.
  //
  // Las muestras van empaquetadas (10 ficheros de unos 35 KB) y se cargan
  // solo al tocar. Mientras suena, ese loro abre el pico con la envolvente
  // real del audio, como en la app.
  import Loro from "./Loro.svelte";

  let {
    voces = [] as { nombre: string; tinta: string }[],
    idioma = "es",
    etiqueta = "",
  } = $props();

  let sonando = $state<string | null>(null);
  let apertura = $state(0);
  let audio: HTMLAudioElement | null = null;
  let contexto: AudioContext | null = null;
  let analizador: AnalyserNode | null = null;
  let cuadro = 0;

  const fichero = (nombre: string) => `/muestras/${idioma}/${nombre.toLowerCase()}.m4a`;

  function mide() {
    if (analizador == null) return;
    const datos = new Uint8Array(analizador.frequencyBinCount);
    analizador.getByteTimeDomainData(datos);
    let suma = 0;
    for (const v of datos) suma += ((v - 128) / 128) ** 2;
    // La raíz cuadrática media, estirada: el pico abre casi del todo.
    apertura = Math.min(1, Math.sqrt(suma / datos.length) * 5.5);
    cuadro = requestAnimationFrame(mide);
  }

  function para() {
    cancelAnimationFrame(cuadro);
    audio?.pause();
    audio = null;
    sonando = null;
    apertura = 0;
  }

  async function toca(nombre: string) {
    if (sonando === nombre) { para(); return; }
    para();
    sonando = nombre;
    audio = new Audio(fichero(nombre));
    audio.addEventListener("ended", para);
    try {
      // La envolvente real necesita permiso del gesto: por eso se monta
      // aquí, dentro del clic, y no al cargar la página.
      contexto ??= new AudioContext();
      if (contexto.state === "suspended") await contexto.resume();
      analizador ??= contexto.createAnalyser();
      analizador.fftSize = 256;
      const fuente = contexto.createMediaElementSource(audio);
      fuente.connect(analizador);
      analizador.connect(contexto.destination);
      cuadro = requestAnimationFrame(mide);
    } catch {
      // Sin Web Audio el loro sigue hablando: con aleteo en vez de
      // envolvente. Se pierde precisión, no se pierde la escena.
      analizador = null;
    }
    try { await audio.play(); } catch { para(); }
  }
</script>

<div class="rejilla rejilla-4" style="margin-top:1.4rem">
  {#each voces as v, i (v.nombre)}
    <button
      class="hoja pajaro"
      type="button"
      style={`--giro:${(i % 2 ? 1 : -1) * (1 + (i % 3))}deg`}
      aria-pressed={sonando === v.nombre}
      aria-label={`${etiqueta} ${v.nombre}`}
      onclick={() => toca(v.nombre)}
    >
      <Loro
        size={62}
        tinta={v.tinta}
        estado={sonando === v.nombre ? "hablando" : "posado"}
        apertura={sonando === v.nombre && analizador != null ? apertura : null}
        cantando={sonando === v.nombre}
      />
      <strong>{v.nombre}</strong>
    </button>
  {/each}
</div>

<style>
  .pajaro {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.4rem;
    font-family: inherit;
    font-size: 1.1rem;
    color: var(--tinta);
    cursor: pointer;
    transition: transform 0.24s var(--sereno), box-shadow 0.24s var(--sereno);
  }
  .pajaro:hover { transform: rotate(var(--giro)) translate(-2px, -3px); box-shadow: var(--sombra-dura-alta); }
  .pajaro:active { transform: rotate(var(--giro)) translate(1px, 1px); box-shadow: var(--sombra-dura-corta); }
  .pajaro[aria-pressed="true"] { border-color: var(--voz); }
</style>
