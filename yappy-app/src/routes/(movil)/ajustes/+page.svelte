<script lang="ts">
  // Ajustes del móvil: lo esencial, en una página. Voz, velocidad, calidad,
  // idioma, aspecto y los dos modelos del dispositivo.
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { t } from "$lib/i18n";
  import { haptic } from "$lib/haptic";
  import Criatura from "$lib/Criatura.svelte";
  import { presionable } from "$lib/presionable";
  import { TINTAS_VOZ, fijarTintaVoz } from "$lib/voces";
  import {
    getSettings,
    setAppTheme,
    setVoice,
    setSpeed,
    setQuality,
    setDefaultLang,
    listVoices,
    sampleVoice,
    isModelReady,
    downloadModel,
    isAsrModelReady,
    downloadAsrModel,
    puenteMovilEstado,
    puenteVincular,
    puenteDesvincular,
    type ConfigPuenteMovil,
    LANGUAGES,
    type Settings,
    type Voice,
    type Quality,
  } from "$lib/ipc";

  let settings = $state<Settings | null>(null);
  let voices = $state<Voice[]>([]);
  let ttsListo = $state(false);
  let asrListo = $state(false);
  let vozAbierta = $state(false);
  let probando = $state<string | null>(null);
  let puente = $state<ConfigPuenteMovil | null>(null);
  let codigoPegado = $state("");
  let puenteError = $state<string | null>(null);

  async function vincular() {
    puenteError = null;
    try {
      puente = await puenteVincular(codigoPegado.trim());
      codigoPegado = "";
      haptic("success");
    } catch (e) {
      puenteError = String(e);
    }
  }

  onMount(async () => {
    settings = await getSettings().catch(() => null);
    voices = await listVoices().catch(() => []);
    ttsListo = await isModelReady().catch(() => false);
    asrListo = await isAsrModelReady().catch(() => false);
    puente = await puenteMovilEstado().catch(() => null);
  });

  async function cambiarTema(tema: "cream" | "dark" | "system") {
    if (!settings) return;
    haptic("light");
    settings.app_theme = tema;
    document.documentElement.dataset.theme = tema;
    try {
      localStorage.setItem("yappy.tema", tema);
    } catch {}
    await setAppTheme(tema).catch(() => {});
  }

  async function elegirVoz(v: Voice, indice: number) {
    if (!settings) return;
    haptic("light");
    settings.voice = v.name;
    vozAbierta = false;
    fijarTintaVoz(TINTAS_VOZ[indice % TINTAS_VOZ.length]);
    await setVoice(v.name).catch(() => {});
  }

  async function probar(v: Voice) {
    probando = v.name;
    try {
      await sampleVoice(v.name);
    } finally {
      setTimeout(() => (probando = null), 600);
    }
  }

  async function cambiarVelocidad(v: number) {
    if (!settings) return;
    settings.speed = v;
    await setSpeed(v).catch(() => {});
  }

  async function cambiarCalidad(q: Quality) {
    if (!settings) return;
    haptic("light");
    settings.quality = q;
    await setQuality(q).catch(() => {});
  }

  async function cambiarIdioma(l: string) {
    if (!settings) return;
    settings.default_lang = l;
    await setDefaultLang(l).catch(() => {});
  }
</script>

<header class="cabecera-pagina">
  <button class="yap-boton es-fantasma" onclick={() => goto("/escuchar")} aria-label="volver">←</button>
  <h1>{$t("ajustes.titulo")}</h1>
</header>

{#if settings}
  <section class="yap-bloque grupo">
    <h2 class="yap-susurro">{$t("ajustes.tema")}</h2>
    <div class="yap-pestanas tema">
      <button class="yap-pestana" use:presionable class:es-activa={settings.app_theme === "cream"} onclick={() => cambiarTema("cream")}>{$t("ajustes.tema.papel")}</button>
      <button class="yap-pestana" use:presionable class:es-activa={settings.app_theme === "dark"} onclick={() => cambiarTema("dark")}>{$t("ajustes.tema.noche")}</button>
      <button class="yap-pestana" use:presionable class:es-activa={settings.app_theme === "system"} onclick={() => cambiarTema("system")}>{$t("ajustes.tema.sistema")}</button>
    </div>
  </section>

  <section class="yap-bloque grupo">
    <h2 class="yap-susurro">{$t("ajustes.voz")}</h2>
    <p class="voces-pista">{$t("voces.pista")}</p>
    <div class="cromos" role="listbox" aria-label={$t("voces.titulo")}>
      {#each voices as v, i (v.name)}
        <button
          use:presionable={{ hap: "soft" }}
          class="cromo"
          class:elegido={settings.voice === v.name}
          role="option"
          aria-selected={settings.voice === v.name}
          onclick={() => {
            elegirVoz(v, i);
            probar(v);
          }}
        >
          <Criatura size={84} tinta={TINTAS_VOZ[i % TINTAS_VOZ.length]} cantando={probando === v.name} />
          <strong>{v.name}</strong>
          <span class="cromo-desc">{v.description}</span>
        </button>
      {/each}
    </div>
    <div class="fila">
      <span>{$t("ajustes.velocidad")}</span>
      <span class="valor mono">{settings.speed.toFixed(2)}×</span>
    </div>
    <input
      class="deslizador"
      type="range"
      min="0.5"
      max="2"
      step="0.05"
      value={settings.speed}
      oninput={(e) => cambiarVelocidad(parseFloat(e.currentTarget.value))}
    />
    <div class="fila">
      <span>{$t("ajustes.calidad")}</span>
    </div>
    <div class="yap-pestanas tema">
      <button class="yap-pestana" use:presionable class:es-activa={settings.quality === "fast"} onclick={() => cambiarCalidad("fast")}>{$t("ajustes.calidad.rapida")}</button>
      <button class="yap-pestana" use:presionable class:es-activa={settings.quality === "balanced"} onclick={() => cambiarCalidad("balanced")}>{$t("ajustes.calidad.equilibrada")}</button>
      <button class="yap-pestana" use:presionable class:es-activa={settings.quality === "best"} onclick={() => cambiarCalidad("best")}>{$t("ajustes.calidad.mejor")}</button>
    </div>
    <label class="fila" for="idioma-preferido">
      <span>{$t("ajustes.idioma")}</span>
      <select
        id="idioma-preferido"
        class="yap-campo"
        value={settings.default_lang}
        onchange={(e) => cambiarIdioma(e.currentTarget.value)}
      >
        {#each LANGUAGES as l (l.code)}
          <option value={l.code}>{l.label}</option>
        {/each}
      </select>
    </label>
  </section>

  <section class="yap-bloque grupo">
    <h2 class="yap-susurro">{$t("ajustes.ordenador")}</h2>
    {#if puente?.token}
      <div class="fila">
        <span>{puente.nombre ?? "ordenador"}</span>
        <span class="yap-pildora es-ok">{$t("ajustes.vinculado")}</span>
      </div>
      <p class="pie-puente">{$t("ajustes.ordenador_texto_si")}</p>
      <button class="yap-boton" onclick={async () => { await puenteDesvincular().catch(() => {}); puente = null; }}>
        {$t("ajustes.desvincular")}
      </button>
    {:else}
      <p class="pie-puente">{$t("ajustes.ordenador_texto_no")}</p>
      <input
        class="yap-campo"
        type="text"
        bind:value={codigoPegado}
        placeholder="yappy://pair?d=…"
        onkeydown={(e) => e.key === "Enter" && vincular()}
      />
      {#if puenteError}<p class="pie-puente" style="color: var(--yap-peligro)">{puenteError}</p>{/if}
      <button class="yap-tecla chica" use:presionable onclick={vincular} disabled={!codigoPegado.trim()}>
        {$t("ajustes.vincular")}
      </button>
    {/if}
  </section>

  <section class="yap-bloque grupo">
    <h2 class="yap-susurro">{$t("ajustes.modelos")}</h2>
    <div class="fila">
      <span>{$t("ajustes.modelo_voces")}</span>
      {#if ttsListo}
        <span class="yap-pildora es-ok">{$t("ajustes.descargado")}</span>
      {:else}
        <button class="yap-tecla chica" onclick={() => downloadModel()}>{$t("ajustes.descargar")}</button>
      {/if}
    </div>
    <div class="fila">
      <span>{$t("ajustes.modelo_oido")}</span>
      {#if asrListo}
        <span class="yap-pildora es-ok">{$t("ajustes.descargado")}</span>
      {:else}
        <button class="yap-tecla chica" onclick={() => downloadAsrModel()}>{$t("ajustes.descargar")}</button>
      {/if}
    </div>
  </section>
{/if}

<style>
  .voces-pista {
    margin: 0 0 10px;
    font-size: 13px;
    color: var(--yap-tinta-suave);
  }
  .cromos {
    display: flex;
    gap: 10px;
    overflow-x: auto;
    scroll-snap-type: x mandatory;
    padding: 4px 2px 10px;
    -webkit-overflow-scrolling: touch;
  }
  .cromo {
    scroll-snap-align: center;
    flex: 0 0 128px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 14px 10px 12px;
    border-radius: 18px;
    border: 2px solid var(--yap-borde);
    background: var(--yap-superficie);
    color: var(--yap-tinta);
    box-shadow: var(--yap-relieve);
    cursor: pointer;
  }
  .cromo.elegido {
    border-color: var(--yap-voz, #e0502a);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--yap-voz, #e0502a) 25%, transparent), var(--yap-relieve-alto, 0 10px 22px rgba(64,46,12,0.16));
    transform: rotate(-1.4deg);
  }
  .cromo strong {
    font-weight: 800;
    font-size: 15px;
  }
  .cromo-desc {
    font-size: 10.5px;
    color: var(--yap-tinta-suave);
    text-align: center;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .cabecera-pagina {
    padding-top: calc(env(safe-area-inset-top) + 10px);
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 2px 0 12px;
  }
  .cabecera-pagina h1 {
    margin: 0;
    font-size: 1.35rem;
    font-weight: 800;
  }
  .grupo {
    padding: 14px 14px 12px;
    margin-bottom: 12px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .fila {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    font-weight: 600;
    font-size: 0.95rem;
    width: 100%;
    text-align: left;
  }
  .valor {
    color: var(--yap-tinta-suave);
    font-weight: 600;
  }
  .mono {
    font-family: var(--yap-mono);
    font-size: 0.85rem;
  }
  .tema {
    align-self: flex-start;
  }
  .deslizador {
    width: 100%;
    accent-color: var(--yap-voz);
  }
  .voces {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .voces li {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .voz {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    padding: 8px 10px;
    border-radius: 10px;
    text-align: left;
  }
  .voz.elegida {
    background: var(--yap-voz-suave);
  }
  .voz-nombre {
    font-weight: 700;
    font-size: 0.9rem;
  }
  .voz-desc {
    font-size: 0.74rem;
    color: var(--yap-tinta-suave);
  }
  select.yap-campo {
    max-width: 55%;
  }
  .pie-puente { margin: 0; font-size: 0.85rem; color: var(--yap-tinta-suave); }
  .chica { align-self: flex-start; padding: 8px 14px; font-size: 0.9rem; }
</style>
