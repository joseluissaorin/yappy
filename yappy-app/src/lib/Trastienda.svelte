<script lang="ts">
  // LA TRASTIENDA, tercera vida: aire de verdad. Gutters de página,
  // tarjetas despegadas del borde, targets de 44 puntos, el Deslizador de
  // la casa con imanes, muestras de voz INSTANTÁNEAS (precocinadas) que no
  // tocan lo que esté sonando, y el idioma de la interfaz a mano.
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";

  // El cajón y la página comparten este componente: «volver» significa
  // cerrar el cajón o navegar, según quién lo monte.
  let {
    alVolver = () => goto("/escuchar"),
    alImprenta = () => goto("/escuchar?imprenta=1"),
  }: { alVolver?: () => void; alImprenta?: () => void } = $props();
  import { t, IDIOMAS_UI, NOMBRE_IDIOMA, fijarPreferenciaIdioma, preferenciaIdioma, type IdiomaUI } from "$lib/i18n";
  import { haptic } from "$lib/haptic";
  import { pref } from "$lib/pref.svelte";
  import Criatura from "$lib/Criatura.svelte";
  import Deslizador from "$lib/Deslizador.svelte";
  import { presionable } from "$lib/presionable";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { TINTAS_VOZ, fijarTintaVoz, tintaVoz } from "$lib/voces";
  import { PALETA } from "$lib/juguete";
  import { compras, abrirPaywall, refrescarCompras, cargarCliente } from "$lib/compras.svelte";
  import { paseo, repetirPaseo, fijarEstadisticasPaseo } from "$lib/paseo.svelte";
  import { TROQUELES_BASE, aPoligono, aPuntosSvg } from "$lib/troquel";
  import { comprasGestionar } from "$lib/ipc";
  import {
    getSettings,
    setSettings,
    setAppTheme,
    setVoice,
    setSpeed,
    setQuality,
    setVozAlAzar,
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
    puenteProbar,
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

  // LA SONDA: al vincular (o a mano) el teléfono llama al ordenador y
  // enseña si contesta. Emparejar sin probar era emparejar a ciegas.
  let sonda = $state<{ estado: "nada" | "probando" | "ok" | "fallo"; nombre?: string; ms?: number; error?: string }>({ estado: "nada" });
  async function probarPuente() {
    if (sonda.estado === "probando") return;
    sonda = { estado: "probando" };
    try {
      const r = await puenteProbar();
      sonda = { estado: "ok", nombre: r.nombre, ms: r.ms };
      haptic("success");
    } catch (e) {
      sonda = { estado: "fallo", error: String(e) };
      haptic("error");
    }
  }
  async function vincular() {
    puenteError = null;
    try {
      puente = await puenteVincular(codigoPegado.trim());
      codigoPegado = "";
      haptic("success");
      void probarPuente();
    } catch (e) {
      puenteError = String(e);
    }
  }

  onMount(async () => {
    settings = await getSettings().catch(() => null);
    if (settings) pref.dosColumnas = settings.dos_columnas;
    velocidad = settings?.speed ?? 1.05;
    voices = await listVoices().catch(() => []);
    ttsListo = await isModelReady().catch(() => false);
    asrListo = await isAsrModelReady().catch(() => false);
    puente = await puenteMovilEstado().catch(() => null);
    if (puente?.token) void probarPuente();
    idiomaPreferido = preferenciaIdioma();
    await refrescarCompras();
    if (compras.pro) void cargarCliente();
  });

  // «parlanchín desde marzo de 2026»: la fecha, en el idioma de la casa.
  function fechaDesde(iso: string | null | undefined): string {
    if (!iso) return "";
    try {
      return new Intl.DateTimeFormat(idiomaPreferido === "auto" ? undefined : idiomaPreferido, { month: "long", year: "numeric" }).format(new Date(iso));
    } catch {
      return iso.slice(0, 10);
    }
  }
  function conHuecos(clave: string, vars: Record<string, string | number>): string {
    let out = $t(clave);
    for (const [k, v] of Object.entries(vars)) out = out.replaceAll(`{${k}}`, String(v));
    return out;
  }

  async function cambiarColumnas(dos: boolean) {
    if (!settings) return;
    haptic("light");
    settings.dos_columnas = dos;
    pref.dosColumnas = dos;
    await setSettings($state.snapshot(settings)).catch(() => {});
  }

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

  async function cambiarVozAlAzar() {
    if (!settings) return;
    haptic("medium");
    settings.voz_al_azar = !settings.voz_al_azar;
    await setVozAlAzar(settings.voz_al_azar).catch(() => {});
  }

  function cambiarIdiomaUI(v: string) {
    haptic("light");
    idiomaPreferido = v as IdiomaUI | "auto";
    fijarPreferenciaIdioma(idiomaPreferido);
  }
</script>

<div class="pagina">
  <header class="cabecera-pagina">
    <button class="yap-boton es-fantasma volver" use:presionable={{ hap: "soft" }} onclick={() => alVolver()} aria-label={$t("lector.volver")}>
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
            <span class="cromo-parche" style="background: {PALETA[i % PALETA.length]}"><Criatura size={72} tinta={TINTAS_VOZ[i % TINTAS_VOZ.length]} cantando={probando === v.name || cocinando === v.name} /></span>
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
      <div class="fila">
        <span>{$t("ajustes.voz_azar")}</span>
        <button
          class="yap-pestana dado"
          class:es-activa={settings.voz_al_azar}
          use:presionable={{ hap: "medium" }}
          onclick={cambiarVozAlAzar}
          aria-pressed={settings.voz_al_azar}
          aria-label={$t("ajustes.voz_azar")}
        >
          <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4.6 5.4 q7.2 -1.6 14.6 -0.2 q1.6 7.2 0.2 14.2 q-7.3 1.5 -14.5 0.1 q-1.5 -7 -0.3 -14.1 Z"/><circle cx="9" cy="9.4" r="1.25" fill="currentColor"/><circle cx="15.2" cy="14.8" r="1.25" fill="currentColor"/></svg>
        </button>
      </div>
    </section>

    <section class="grupo">
      <h2 class="yap-susurro">{$t("ajustes.tema")}</h2>
      <div class="yap-pestanas tema">
        <button class="yap-pestana" use:presionable class:es-activa={settings.app_theme === "cream"} onclick={() => cambiarTema("cream")}>{$t("ajustes.tema.papel")}</button>
        <button class="yap-pestana" use:presionable class:es-activa={settings.app_theme === "dark"} onclick={() => cambiarTema("dark")}>{$t("ajustes.tema.noche")}</button>
        <button class="yap-pestana" use:presionable class:es-activa={settings.app_theme === "system"} onclick={() => cambiarTema("system")}>{$t("ajustes.tema.sistema")}</button>
      </div>
      <div class="yap-pestanas tema">
        <button class="yap-pestana" use:presionable class:es-activa={!settings.dos_columnas} onclick={() => cambiarColumnas(false)}>{$t("ajustes.una_columna")}</button>
        <button class="yap-pestana" use:presionable class:es-activa={settings.dos_columnas} onclick={() => cambiarColumnas(true)}>{$t("ajustes.dos_columnas")}</button>
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

    {#if compras.disponible}
      <!-- LA CUERDA DEL LORO: quién eres para la tienda, y cuánta voz llevas hoy. -->
      <section class="grupo cuerda-caja" class:es-pro={compras.pro}>
        <h2 class="yap-susurro">{$t("pro.nombre")}</h2>
        <div class="cuerda-fila">
          <span class="cuerda-loro" aria-hidden="true">
            <Criatura size={64} tinta={$tintaVoz} estado={compras.pro ? "posado" : compras.cuota.agotada ? "avergonzado" : "posado"} cantando={compras.pro} />
            <svg class="cuerda-llave" viewBox="0 0 64 64"><g stroke="#2b2418" stroke-width="1.6" stroke-linejoin="round"><rect x="44" y="34.4" width="12" height="3.2" rx="1.4" fill="#e8b41a" /><path d="M56 36 c0 -4 4 -6 6 -3 c1.4 2 0 4 -2 4 c2 0 3.4 2 2 4 c-2 3 -6 1 -6 -3 Z" fill="#e8b41a" /></g></svg>
          </span>
          <div class="cuerda-texto">
            {#if compras.pro}
              <span class="yap-pildora es-ok">{$t("pro.estado_pro")}</span>
              {#if compras.cliente?.desde}
                <p class="pie-puente">{conHuecos("pro.desde", { fecha: fechaDesde(compras.cliente.desde) })}</p>
              {/if}
            {:else}
              <span class="yap-pildora">{$t("pro.estado_gratis")}</span>
              <p class="pie-puente mono-hoy">{conHuecos("pro.gratis_usados", { usado: Math.min(compras.cuota.usados, compras.cuota.limite), limite: compras.cuota.limite })}</p>
              <!-- Los huecos de la percha: una pegatina por documento, el hueco libre en línea de puntos. -->
              <div class="percha-huecos" aria-hidden="true">
                {#each Array.from({ length: compras.cuota.limite }, (_, i) => i) as i (i)}
                  {@const forma = TROQUELES_BASE[(i * 3) % TROQUELES_BASE.length]}
                  {@const lleno = i < compras.cuota.usados}
                  <span class="hueco" class:lleno style="--giro:{(i % 2 ? 1 : -1) * (3 + i)}deg">
                    <span class="hueco-pega" style="background: {PALETA[(i * 4 + 4) % PALETA.length]}; clip-path: {aPoligono(forma)}"></span>
                    <svg viewBox="0 0 100 100" preserveAspectRatio="none"><polygon points={aPuntosSvg(forma, lleno ? 0.8 : 0.96)} fill="none" stroke={lleno ? "#fffdf7" : "var(--yap-tinta-suave)"} stroke-width="1.6" stroke-dasharray="4 3" vector-effect="non-scaling-stroke" /></svg>
                  </span>
                {/each}
              </div>
            {/if}
          </div>
        </div>
        {#if compras.pro}
          {#if compras.cliente?.expira}
            <button class="yap-boton" use:presionable onclick={() => { haptic("light"); comprasGestionar().catch(() => {}); }}>{$t("pro.gestionar")}</button>
          {/if}
        {:else}
          <button class="yap-tecla chica tecla-cuerda" use:presionable={{ hap: "rigid" }} onclick={() => abrirPaywall("trastienda")}>{$t("pro.comprar")}</button>
        {/if}
      </section>
    {/if}

    <button class="grupo puerta-biblioteca" use:presionable onclick={alImprenta}>
      <span>{$t("trastienda.biblioteca")}</span>
      <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M5 12h14M13 6l6 6-6 6"/></svg>
    </button>

    <section class="grupo">
      <h2 class="yap-susurro">{$t("ajustes.ordenador")}</h2>
      {#if puente?.token}
        <div class="fila">
          <span>{puente.nombre ?? "ordenador"}</span>
          <span class="yap-pildora es-ok">{$t("ajustes.vinculado")}</span>
        </div>
        <p class="pie-puente">{$t("ajustes.ordenador_texto_si")}</p>
        {#if sonda.estado === "probando"}
          <p class="pie-puente sonda-linea"><span class="sonda-punto"></span>{$t("ajustes.probando")}</p>
        {:else if sonda.estado === "ok"}
          <p class="pie-puente sonda-linea es-ok">{conHuecos("ajustes.puente_ok", { nombre: sonda.nombre ?? puente.nombre ?? "ordenador", ms: sonda.ms ?? 0 })}</p>
        {:else if sonda.estado === "fallo"}
          <p class="pie-puente sonda-linea es-fallo">{$t("ajustes.puente_fallo")}</p>
          {#if sonda.error}<p class="pie-puente sonda-detalle">{sonda.error}</p>{/if}
        {/if}
        <div class="fila-teclas">
          <button class="yap-tecla chica" use:presionable onclick={probarPuente} disabled={sonda.estado === "probando"}>{$t("ajustes.probar")}</button>
          <button class="yap-boton" use:presionable onclick={async () => { await puenteDesvincular().catch(() => {}); puente = null; sonda = { estado: "nada" }; }}>
            {$t("ajustes.desvincular")}
          </button>
        </div>
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

    <section class="grupo">
      <h2 class="yap-susurro">{$t("trastienda.paseo")}</h2>
      <p class="pie-puente">{$t("trastienda.paseo_texto")}</p>
      <button class="yap-boton" use:presionable onclick={() => { haptic("medium"); void repetirPaseo(); }}>{$t("trastienda.paseo_tecla")}</button>
      <div class="fila">
        <span>{$t("trastienda.estadisticas")}</span>
        <button
          class="yap-pestana"
          class:es-activa={paseo.libreta.estadisticas !== false}
          use:presionable={{ hap: "light" }}
          onclick={() => void fijarEstadisticasPaseo(paseo.libreta.estadisticas === false)}
          aria-pressed={paseo.libreta.estadisticas !== false}
        >{paseo.libreta.estadisticas !== false ? "✓" : "·"}</button>
      </div>
      <p class="pie-puente">{$t("trastienda.estadisticas_texto")}</p>
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
  .grupo.puerta-biblioteca {
    flex-direction: row;
    align-items: center;
    justify-content: space-between;
    cursor: pointer;
    font: inherit;
    font-weight: 800;
    font-size: 15px;
    color: var(--yap-tinta);
    padding: 16px;
    text-align: left;
  }
  .grupo {
    position: relative;
    background: var(--yap-superficie);
    border: 1.5px solid var(--yap-borde);
    border-radius: 20px;
    box-shadow: 3px 4px 0 #ded7c2;
    padding: 20px 16px 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    margin-top: 12px;
  }
  /* La etiqueta de la caja: pegada al canto superior, como en un taller. */
  .grupo > :global(h2.yap-susurro) {
    position: absolute;
    top: -12px;
    left: 12px;
    margin: 0;
    background: var(--yap-papel);
    border: 1.5px solid var(--yap-borde);
    border-radius: 8px;
    padding: 3px 11px;
    transform: rotate(-1.6deg);
    box-shadow: 2px 2.5px 0 #ded7c2;
  }
  .cromo-parche {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 92px;
    height: 92px;
    border-radius: 50%;
    outline: 2px dashed rgba(43, 36, 24, 0.25);
    outline-offset: -7px;
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
  .sonda-linea { display: flex; align-items: center; gap: 8px; font-weight: 700; }
  .sonda-linea.es-ok { color: var(--yap-ok, #2f7a4a); }
  .sonda-linea.es-fallo { color: var(--yap-peligro, #9a4a3a); }
  .sonda-detalle { font-size: 0.75rem; opacity: 0.8; }
  .sonda-punto { width: 9px; height: 9px; border-radius: 50%; background: currentColor; animation: sonda-late 0.9s ease-in-out infinite alternate; }
  @keyframes sonda-late { from { opacity: 0.3; transform: scale(0.7); } to { opacity: 1; transform: scale(1); } }
  .fila-teclas { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; }

  /* ── La cuerda del loro ── */
  .cuerda-fila { display: flex; align-items: center; gap: 12px; }
  .cuerda-loro { position: relative; display: inline-block; width: 72px; height: 64px; flex: 0 0 auto; }
  .cuerda-llave { position: absolute; left: 0; top: 0; width: 64px; height: 64px; pointer-events: none; transform-origin: 60px 36px; }
  .es-pro .cuerda-llave { animation: cuerda-gira 3.6s linear infinite; }
  @keyframes cuerda-gira { from { transform: rotate(0); } to { transform: rotate(360deg); } }
  .cuerda-texto { display: flex; flex-direction: column; align-items: flex-start; gap: 6px; min-width: 0; flex: 1; }
  .mono-hoy { font-family: var(--yap-mono); font-size: 0.78rem; letter-spacing: 0.04em; }
  .percha-huecos { display: flex; gap: 8px; padding-top: 2px; }
  .hueco { position: relative; width: 34px; height: 34px; transform: rotate(var(--giro)); }
  .hueco-pega { position: absolute; inset: 0; opacity: 0; transition: opacity 0.4s ease, transform 0.42s cubic-bezier(0.24, 1.7, 0.44, 1); transform: scale(0.6); }
  .hueco.lleno .hueco-pega { opacity: 1; transform: scale(1); }
  .hueco svg { position: absolute; inset: 0; width: 100%; height: 100%; }
  .tecla-cuerda { background: var(--vivo, var(--voz-tinta, var(--yap-voz))); color: #fffdf7; box-shadow: 2px 3px 0 #2b2418; transform: rotate(-0.6deg); }
  @media (prefers-reduced-motion: reduce) { .es-pro .cuerda-llave { animation: none; } }
  .chica { align-self: flex-start; padding: 10px 16px; font-size: 0.9rem; min-height: 44px; }
</style>
