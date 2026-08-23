<script lang="ts">
  // Ajustes del móvil: lo esencial, en una página. Voz, velocidad, calidad,
  // idioma, aspecto y los dos modelos del dispositivo.
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { t } from "$lib/i18n";
  import { haptic } from "$lib/haptic";
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

  onMount(async () => {
    settings = await getSettings().catch(() => null);
    voices = await listVoices().catch(() => []);
    ttsListo = await isModelReady().catch(() => false);
    asrListo = await isAsrModelReady().catch(() => false);
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

  async function elegirVoz(v: Voice) {
    if (!settings) return;
    haptic("light");
    settings.voice = v.name;
    vozAbierta = false;
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
      <button class="yap-pestana" class:es-activa={settings.app_theme === "cream"} onclick={() => cambiarTema("cream")}>{$t("ajustes.tema.papel")}</button>
      <button class="yap-pestana" class:es-activa={settings.app_theme === "dark"} onclick={() => cambiarTema("dark")}>{$t("ajustes.tema.noche")}</button>
      <button class="yap-pestana" class:es-activa={settings.app_theme === "system"} onclick={() => cambiarTema("system")}>{$t("ajustes.tema.sistema")}</button>
    </div>
  </section>

  <section class="yap-bloque grupo">
    <h2 class="yap-susurro">{$t("ajustes.voz")}</h2>
    <button class="fila" onclick={() => (vozAbierta = !vozAbierta)}>
      <span>{$t("ajustes.voz_defecto")}</span>
      <span class="valor">{settings.voice} {vozAbierta ? "▴" : "▾"}</span>
    </button>
    {#if vozAbierta}
      <ul class="voces yap-enter">
        {#each voices as v (v.name)}
          <li>
            <button class="voz" class:elegida={settings.voice === v.name} onclick={() => elegirVoz(v)}>
              <span class="voz-nombre">{v.name}</span>
              <span class="voz-desc">{v.description}</span>
            </button>
            <button
              class="yap-boton es-fantasma"
              aria-label="escuchar una muestra"
              onclick={(e) => {
                e.stopPropagation();
                probar(v);
              }}>{probando === v.name ? "…" : "▶"}</button
            >
          </li>
        {/each}
      </ul>
    {/if}
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
      <button class="yap-pestana" class:es-activa={settings.quality === "fast"} onclick={() => cambiarCalidad("fast")}>{$t("ajustes.calidad.rapida")}</button>
      <button class="yap-pestana" class:es-activa={settings.quality === "balanced"} onclick={() => cambiarCalidad("balanced")}>{$t("ajustes.calidad.equilibrada")}</button>
      <button class="yap-pestana" class:es-activa={settings.quality === "best"} onclick={() => cambiarCalidad("best")}>{$t("ajustes.calidad.mejor")}</button>
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
          <option value={l.code}>{l.flag ?? ""} {l.label}</option>
        {/each}
      </select>
    </label>
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
  .cabecera-pagina {
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
</style>
