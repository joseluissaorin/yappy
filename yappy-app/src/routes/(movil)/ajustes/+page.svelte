<script lang="ts">
  // LA TRASTIENDA, tercera vida: aire de verdad. Gutters de página,
  // tarjetas despegadas del borde, targets de 44 puntos, el Deslizador de
  // la casa con imanes, muestras de voz INSTANTÁNEAS (precocinadas) que no
  // tocan lo que esté sonando, y el idioma de la interfaz a mano.
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { t, IDIOMAS_UI, NOMBRE_IDIOMA, fijarPreferenciaIdioma, preferenciaIdioma, type IdiomaUI } from "$lib/i18n";
  import { haptic } from "$lib/haptic";
  import Criatura from "$lib/Criatura.svelte";
  import Deslizador from "$lib/Deslizador.svelte";
  import { presionable } from "$lib/presionable";
  import { openUrl } from "@tauri-apps/plugin-opener";
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
  let probando = $state<string | null>(null);
  let cocinando = $state<string | null>(null);
  let puente = $state<ConfigPuenteMovil | null>(null);
  let codigoPegado = $state("");
  let puenteError = $state<string | null>(null);
  let idiomaPreferido = $state<IdiomaUI | "auto">("auto");
  let velocidad = $state(1.05);
  let probarTimer: ReturnType<typeof setTimeout> | undefined;

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
    velocidad = settings?.speed ?? 1.05;
    voices = await listVoices().catch(() => []);
    ttsListo = await isModelReady().catch(() => false);
    asrListo = await isAsrModelReady().catch(() => false);
    puente = await puenteMovilEstado().catch(() => null);
    idiomaPreferido = preferenciaIdioma();
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
    fijarTintaVoz(TINTAS_VOZ[indice % TINTAS_VOZ.length]);
    await setVoice(v.name).catch(() => {});
  }

  async function probar(v: Voice) {
    clearTimeout(probarTimer);
    cocinando = v.name;
    probando = null;
    try {
      const dur = await sampleVoice(v.name);
      cocinando = null;
      probando = v.name;
      // El cromo canta lo que dura la muestra de verdad.
      probarTimer = setTimeout(() => (probando = null), Math.max(1200, dur * 1000));
    } catch {
      cocinando = null;
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

  function cambiarIdiomaUI(v: string) {
    haptic("light");
    idiomaPreferido = v as IdiomaUI | "auto";
    fijarPreferenciaIdioma(idiomaPreferido);
  }
</script>

<div class="pagina">
  <header class="cabecera-pagina">
    <button class="yap-boton es-fantasma volver" use:presionable={{ hap: "soft" }} onclick={() => goto("/escuchar")} aria-label={$t("lector.volver")}>
      <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><path d="M19 12H5M11 18l-6-6 6-6"/></svg>
    </button>
    <h1>{$t("ajustes.titulo")}</h1>
  </header>

  {#if settings}
    <section class="grupo">
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
            <Criatura size={84} tinta={TINTAS_VOZ[i % TINTAS_VOZ.length]} cantando={probando === v.name || cocinando === v.name} />
            <strong>{v.name}</strong>
            <span class="cromo-desc">{cocinando === v.name ? $t("voces.cocinando") : v.description}</span>
          </button>
        {/each}
      </div>

      <div class="fila">
        <span>{$t("ajustes.velocidad")}</span>
        <span class="valor mono">{velocidad.toFixed(2).replace(".", ",")}×</span>
      </div>
      <Deslizador
        bind:value={velocidad}
        min={0.5}
        max={2}
        step={0.05}
        imanes={[1.0, 1.25, 1.5]}
        formatear={(v: number) => v.toFixed(2).replace(".", ",") + "×"}
        etiqueta={$t("ajustes.velocidad")}
        alSoltar={(v) => cambiarVelocidad(v)}
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

    <section class="grupo">
      <h2 class="yap-susurro">{$t("ajustes.tema")}</h2>
      <div class="yap-pestanas tema">
        <button class="yap-pestana" use:presionable class:es-activa={settings.app_theme === "cream"} onclick={() => cambiarTema("cream")}>{$t("ajustes.tema.papel")}</button>
        <button class="yap-pestana" use:presionable class:es-activa={settings.app_theme === "dark"} onclick={() => cambiarTema("dark")}>{$t("ajustes.tema.noche")}</button>
        <button class="yap-pestana" use:presionable class:es-activa={settings.app_theme === "system"} onclick={() => cambiarTema("system")}>{$t("ajustes.tema.sistema")}</button>
      </div>
      <label class="fila" for="idioma-ui">
        <span>{$t("ajustes.idioma_ui")}</span>
        <select id="idioma-ui" class="yap-campo" value={idiomaPreferido} onchange={(e) => cambiarIdiomaUI(e.currentTarget.value)}>
          <option value="auto">{$t("ajustes.idioma_auto")}</option>
          {#each IDIOMAS_UI as cod (cod)}
            <option value={cod}>{NOMBRE_IDIOMA[cod]}</option>
          {/each}
        </select>
      </label>
    </section>

    <section class="grupo">
      <h2 class="yap-susurro">{$t("ajustes.ordenador")}</h2>
      {#if puente?.token}
        <div class="fila">
          <span>{puente.nombre ?? "ordenador"}</span>
          <span class="yap-pildora es-ok">{$t("ajustes.vinculado")}</span>
        </div>
        <p class="pie-puente">{$t("ajustes.ordenador_texto_si")}</p>
        <button class="yap-boton" use:presionable onclick={async () => { await puenteDesvincular().catch(() => {}); puente = null; }}>
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

    <section class="grupo">
      <h2 class="yap-susurro">{$t("ajustes.modelos")}</h2>
      <div class="fila">
        <span>{$t("ajustes.modelo_voces")}</span>
        {#if ttsListo}
          <span class="yap-pildora es-ok">{$t("ajustes.descargado")}</span>
        {:else}
          <button class="yap-tecla chica" use:presionable onclick={() => downloadModel()}>{$t("ajustes.descargar")}</button>
        {/if}
      </div>
      <div class="fila">
        <span>{$t("ajustes.modelo_oido")}</span>
        {#if asrListo}
          <span class="yap-pildora es-ok">{$t("ajustes.descargado")}</span>
        {:else}
          <button class="yap-tecla chica" use:presionable onclick={() => downloadAsrModel()}>{$t("ajustes.descargar")}</button>
        {/if}
      </div>
    </section>

    <!-- El pie: quién hay detrás, con el loro paseando. -->
    <footer class="pie-amor">
      <Criatura size={44} andando mirando={-1} tinta={TINTAS_VOZ[0]} />
      <p>{$t("ajustes.amor")}</p>
      <button class="enlace-web" use:presionable={{ hap: "soft" }} onclick={() => { haptic("light"); openUrl("https://joseluissaorin.com").catch(() => {}); }}>
        joseluissaorin.com
      </button>
    </footer>
  {/if}
</div>

<style>
  /* El aire: gutters de página, tarjetas despegadas, hueco para la aguja. */
  .pagina {
    min-height: 100dvh;
    padding: calc(env(safe-area-inset-top) + 10px) 18px
      calc(env(safe-area-inset-bottom) + var(--aguja-hueco, 0px) + 28px);
    display: flex;
    flex-direction: column;
    gap: 16px;
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
  .grupo {
    background: var(--yap-superficie);
    border: 1px solid var(--yap-borde);
    border-radius: 20px;
    box-shadow: var(--yap-relieve);
    padding: 18px 16px 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .voces-pista {
    margin: -4px 0 2px;
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
    scrollbar-width: none;
    margin: 0 -4px;
  }
  .cromos::-webkit-scrollbar {
    display: none;
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
    min-height: 2.4em;
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
    min-height: 30px;
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
  select.yap-campo {
    max-width: 55%;
    min-height: 44px;
  }
  /* Los rótulos de sección, con el vaivén del collage. */
  .grupo:nth-of-type(odd) h2 {
    transform: rotate(-0.8deg);
  }
  .grupo:nth-of-type(even) h2 {
    transform: rotate(0.6deg);
  }
  .grupo h2 {
    display: inline-block;
  }

  .pie-amor {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 22px 10px 6px;
    text-align: center;
  }
  .pie-amor p {
    margin: 0;
    font-size: 13.5px;
    color: var(--yap-tinta-suave);
    transform: rotate(-0.7deg);
  }
  .enlace-web {
    border: 0;
    background: transparent;
    color: var(--yap-ultramar, #2f4bc4);
    font-weight: 800;
    font-size: 15px;
    padding: 6px 12px;
    cursor: pointer;
    text-decoration: underline;
    text-underline-offset: 4px;
    text-decoration-thickness: 2px;
    transform: rotate(0.5deg);
  }

  .pie-puente { margin: 0; font-size: 0.85rem; color: var(--yap-tinta-suave); }
  .chica { align-self: flex-start; padding: 10px 16px; font-size: 0.9rem; min-height: 44px; }
</style>
