<script lang="ts">
  // EL PAYWALL DEL LORO: la página del álbum donde el loro pide cuerda.
  //
  // La metáfora manda: el loro de prueba es un juguete de cuerda con una
  // PERCHA de tres documentos a la vez; el cuarto no cabe y se cae (la
  // percha dibujada, tres pegatinas colgadas y una resbalando). Darle
  // cuerda es girar la llave dorada de su espalda: la llave tiembla al
  // elegir plan, gira mientras la tienda contesta, y al comprar el loro
  // celebra con su bandada y llueven recortes. Las tres cuerdas son pegatinas troqueladas de la casa
  // (sello, estrella, corazón), con su costura y su sombra dura. Ley del
  // álbum: papel, una tinta por pieza, tres voces tipográficas, humor.
  import { onMount } from "svelte";
  import { fly, fade, scale } from "svelte/transition";
  import { backOut, cubicOut } from "svelte/easing";
  import { t, idiomaUI } from "$lib/i18n";
  import { haptic } from "$lib/haptic";
  import { presionable } from "$lib/presionable";
  import { boing, plop, pop, rasga, tick as foleyTick } from "$lib/foley";
  import Criatura from "$lib/Criatura.svelte";
  import Confetti from "$lib/Confetti.svelte";
  import { TINTAS_VOZ, tintaVoz } from "$lib/voces";
  import { PALETA, tonoHondo } from "$lib/juguete";
  import { TROQUELES_BASE, CORAZON, ESTRELLA, aPoligono, aPuntosSvg, type Troquel } from "$lib/troquel";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import {
    compras,
    cerrarPaywall,
    cargarOfertas,
    comprar,
    restaurar,
    refrescarCompras,
    type MotivoPaywall,
  } from "$lib/compras.svelte";
  import type { Paquete, TipoPaquete } from "$lib/ipc";
  import { repro } from "$lib/reproduccion.svelte";
  import { medir } from "$lib/analitica";

  // ── Texto con huecos: «ahorras un {pct} %» ───────────────────────────
  function f(clave: string, vars: Record<string, string | number>): string {
    let s = $t(clave);
    for (const [k, v] of Object.entries(vars)) s = s.replaceAll(`{${k}}`, String(v));
    return s;
  }

  // ── Las tres cuerdas: forma, tinta y orden fijos ─────────────────────
  const SELLO = TROQUELES_BASE.find((x) => x.nombre === "sello")!;
  // dy: la ventana visual de cada forma no está en su centro geométrico
  // (la estrella y el corazón cargan arriba): el texto se sube un poco.
  type Cuerda = { tipo: TipoPaquete; troquel: Troquel; tinta: string; peso: number; giro: number; dy: number };
  const CUERDAS: Cuerda[] = [
    { tipo: "mensual", troquel: SELLO, tinta: PALETA[4], peso: 1, giro: -3.5, dy: 0 },
    { tipo: "anual", troquel: ESTRELLA, tinta: PALETA[7], peso: 1.3, giro: 1.5, dy: -6 },
    { tipo: "vida", troquel: CORAZON, tinta: PALETA[1], peso: 1, giro: 3, dy: -10 },
  ];
  const porTipo = $derived(
    Object.fromEntries(compras.paquetes.map((p) => [p.tipo, p])) as Partial<Record<TipoPaquete, Paquete>>,
  );
  let elegida = $state<TipoPaquete>("anual");
  const paqueteElegido = $derived(porTipo[elegida] ?? null);

  // ── Los números que cuentan (moneda del usuario, formato del sistema) ─
  function moneda(n: number, cod: string | null | undefined): string {
    try {
      if (cod) return new Intl.NumberFormat($idiomaUI, { style: "currency", currency: cod, maximumFractionDigits: 2 }).format(n);
    } catch {}
    return n.toFixed(2);
  }
  const ahorro = $derived.by(() => {
    const m = porTipo.mensual;
    const a = porTipo.anual;
    if (!m || !a || !m.precio_num || !a.precio_num) return null;
    const pct = Math.round((1 - a.precio_num / (m.precio_num * 12)) * 100);
    return pct > 0 ? pct : null;
  });
  function equivalente(p: Paquete | null): string | null {
    if (!p) return null;
    if (p.tipo === "anual" && p.precio_num) return f("pro.equivale", { precio: moneda(p.precio_num / 12, p.moneda) });
    if (p.tipo === "vida") return `${$t("pro.un_pago")} · ${$t("pro.para_siempre")}`;
    if (p.tipo === "mensual") return $t("pro.cancelas");
    return null;
  }
  function periodoDe(tipo: TipoPaquete): string {
    return tipo === "mensual" ? $t("pro.al_mes") : tipo === "anual" ? $t("pro.al_ano") : $t("pro.un_pago");
  }
  function nombreDe(tipo: TipoPaquete): string {
    return tipo === "mensual" ? $t("pro.mensual") : tipo === "anual" ? $t("pro.anual") : $t("pro.vida");
  }

  // ── La viñeta por motivo ─────────────────────────────────────────────
  const motivo = $derived<MotivoPaywall>(compras.motivo);
  // LA CABECERA VIVA: si el loro está leyendo, se le ve hablar con la
  // frase del karaoke debajo. Nadie compra una promesa: se compra lo que
  // se está oyendo.
  const leyendo = $derived(!!repro.snap && repro.snap.estado === "sonando" && !!repro.snap.titulo);
  const tituloLeyendo = $derived(repro.snap?.titulo ?? "");
  const fraseLeyendo = $derived((repro.snap?.current_text ?? "").trim());
  const titulo = $derived(
    motivo === "paseo" ? $t("pro.paseo_titulo")
    : motivo === "percha" ? $t("pro.percha_titulo")
    : motivo === "imprenta" ? $t("pro.imprenta_titulo")
    : motivo === "puente" ? $t("pro.puente_titulo")
    : $t("pro.cuerda_titulo"),
  );
  const texto = $derived(
    motivo === "paseo" ? (leyendo ? f("pro.paseo_texto", { titulo: tituloLeyendo }) : $t("pro.paseo_texto_silencio"))
    : motivo === "percha" ? $t("pro.percha_texto")
    : motivo === "imprenta" ? $t("pro.imprenta_texto")
    : motivo === "puente" ? $t("pro.puente_texto")
    : $t("pro.cuerda_texto"),
  );

  // ── El estado de la escena ───────────────────────────────────────────
  let comprando = $state(false);
  let restaurando = $state(false);
  let hecho = $state(false);
  let error = $state<string | null>(null);
  let aviso = $state<string | null>(null);
  let verguenza = $state(false);
  let llaveTiembla = $state(false);
  let mirada = $state({ x: 0, y: 0 });
  let cierreTimer: ReturnType<typeof setTimeout> | undefined;

  function seguirDedo(e: PointerEvent) {
    const w = window.innerWidth || 1;
    const h = window.innerHeight || 1;
    mirada = {
      x: Math.max(-1, Math.min(1, (e.clientX / w) * 2 - 1)),
      y: Math.max(-1, Math.min(1, (e.clientY / h) * 2 - 1)),
    };
  }

  function elegir(tipo: TipoPaquete) {
    if (comprando) return;
    if (elegida !== tipo) {
      elegida = tipo;
      medir("paywall_plan_selected", { plan: tipo });
      haptic("medium");
      boing();
    } else {
      haptic("tick");
      foleyTick();
    }
    llaveTiembla = true;
    setTimeout(() => (llaveTiembla = false), 520);
  }

  function fallo(msg: string | null) {
    error = msg ?? $t("pro.error");
    verguenza = true;
    haptic("error");
    setTimeout(() => (verguenza = false), 1200);
  }

  async function darCuerda() {
    if (!paqueteElegido || comprando) return;
    error = null;
    aviso = null;
    comprando = true;
    haptic("rigid");
    pop();
    medir("paywall_cta_tapped", { plan: paqueteElegido.tipo, source: compras.motivo });
    try {
      const r = await comprar(paqueteElegido.id);
      if (r.cancelado) {
        medir("purchase_cancelled", { plan: paqueteElegido.tipo });
        haptic("soft");
        return;
      }
      if (r.error) {
        medir("purchase_failed", { plan: paqueteElegido.tipo });
        fallo(null);
        return;
      }
      if (r.ok && r.pro) {
        medir("purchase_completed", { plan: paqueteElegido.tipo, source: compras.motivo });
        celebrar();
      } else if (r.ok) {
        // Compró pero la entitlement aún no llegó: refrescar y confiar.
        await refrescarCompras();
        if (compras.pro) celebrar();
        else fallo(null);
      }
    } catch {
      fallo(null);
    } finally {
      comprando = false;
    }
  }

  async function yaEraParlanchin() {
    if (restaurando || comprando) return;
    error = null;
    aviso = null;
    restaurando = true;
    haptic("light");
    medir("restore_tapped");
    try {
      const r = await restaurar();
      if (r.error) {
        fallo(null);
      } else if (r.pro) {
        aviso = $t("pro.restaurado");
        celebrar();
      } else {
        aviso = $t("pro.nada_que_restaurar");
        haptic("warning");
      }
    } catch {
      fallo(null);
    } finally {
      restaurando = false;
    }
  }

  function celebrar() {
    hecho = true;
    haptic("success");
    plop();
    clearTimeout(cierreTimer);
    cierreTimer = setTimeout(cerrar, 3400);
  }

  function cerrar() {
    clearTimeout(cierreTimer);
    haptic("soft");
    rasga();
    if (!hecho) medir("paywall_dismissed", { source: compras.motivo });
    cerrarPaywall();
    // La escena vuelve a nacer limpia la próxima vez.
    setTimeout(() => {
      hecho = false;
      error = null;
      aviso = null;
    }, 400);
  }

  function abrirEnlace(url: string) {
    haptic("light");
    openUrl(url).catch(() => {});
  }
  const URL_TERMINOS = "https://www.apple.com/legal/internet-services/itunes/dev/stdeula/";
  const urlPrivacidad = $derived(
    $idiomaUI === "es" ? "https://yappy.joseluissaorin.com/preguntas" : "https://yappy.joseluissaorin.com/en/faq",
  );

  // ── El tirón para cerrar ────────────────────────────────────────────
  // La hoja se cierra deslizándola hacia abajo, como cualquier hoja de
  // iOS. Solo arrastra si la hoja está arriba del todo (si no, el dedo
  // está haciendo scroll dentro).
  let arrastre = $state(0);
  let hoja: HTMLDivElement | undefined = $state();
  let tirando: { y: number } | null = null;
  function tirarDown(e: PointerEvent) {
    (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
    tirando = { y: e.clientY };
  }
  function tirarMove(e: PointerEvent) {
    if (!tirando) return;
    const dy = e.clientY - tirando.y;
    // Hacia arriba no se arrastra: la hoja no sube más.
    arrastre = dy <= 0 ? 0 : dy < 140 ? dy : 140 + (dy - 140) * 0.3;
  }
  function tirarUp() {
    if (!tirando) return;
    const soltado = arrastre;
    tirando = null;
    arrastre = 0;
    if (soltado > 90) {
      haptic("soft");
      cerrar();
    }
  }

  // Al abrirse: sonido de papel, y la cuerda favorita por delante.
  $effect(() => {
    if (compras.abierto) {
      rasga();
      haptic("soft");
      elegida = "anual";
      arrastre = 0;
      medir("paywall_shown", { source: compras.motivo });
      if (!compras.paquetes.length) void cargarOfertas(true);
    }
  });
  onMount(() => () => clearTimeout(cierreTimer));

  // La bandada del pie: tres pájaros con tintas distintas a la tuya.
  const bandada = $derived(TINTAS_VOZ.filter((c) => c !== $tintaVoz).slice(0, 3));
  // Las pegatinas colgadas de la percha (formas y tintas fijas, como en la cinta).
  const COLGADAS = [
    { troquel: TROQUELES_BASE.find((x) => x.nombre === "nube")!, tinta: PALETA[4], giro: -6 },
    { troquel: TROQUELES_BASE.find((x) => x.nombre === "hexagono")!, tinta: PALETA[6], giro: 3 },
    { troquel: TROQUELES_BASE.find((x) => x.nombre === "flor")!, tinta: PALETA[2], giro: -2 },
  ];
  const QUE_CAE = TROQUELES_BASE.find((x) => x.nombre === "circulo")!;
</script>

{#if compras.abierto}
  <div
    class="telon"
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    aria-label={$t("pro.nombre")}
    transition:fade={{ duration: 220 }}
    onpointermove={seguirDedo}
    onclick={(e) => {
      if (e.target === e.currentTarget) cerrar();
    }}
    onkeydown={(e) => e.key === "Escape" && cerrar()}
  >
    <div
      class="pagina yap-bloque"
      role="document"
      class:tirando={arrastre > 0}
      bind:this={hoja}
      style="transform: translateY({arrastre}px) rotate({arrastre > 0 ? 0 : -0.5}deg);"
      in:fly={{ y: 48, duration: 480, easing: backOut }}
      out:fly={{ y: 40, duration: 220, easing: cubicOut }}
    >
      <!-- El celo de la esquina: la página está pegada al álbum. -->
      <span class="celo celo-a" aria-hidden="true"></span>
      <span class="celo celo-b" aria-hidden="true"></span>
      <!-- EL TIRADOR: su franja se arrastra hacia abajo para cerrar. Tiene
           `touch-action: none` para que el gesto sea suyo y no del scroll. -->
      <div
        class="tirador-zona"
        role="button"
        tabindex="0"
        aria-label={$t("comun.cerrar")}
        onpointerdown={tirarDown}
        onpointermove={tirarMove}
        onpointerup={tirarUp}
        onpointercancel={tirarUp}
        onkeydown={(e) => (e.key === "Enter" || e.key === " ") && cerrar()}
      >
        <span class="tirador" aria-hidden="true"></span>
      </div>

      <button class="cerrar" use:presionable={{ hap: "soft" }} onclick={cerrar} aria-label={$t("comun.cerrar")}>
        <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round"><path d="M6.5 6.2 L17.6 17.9 M17.4 6.4 L6.3 17.7"/></svg>
      </button>

      {#if hecho}
        <!-- ── LA CELEBRACIÓN: el loro recupera la voz ── -->
        <Confetti />
        <div class="escena celebra" in:scale={{ start: 0.86, duration: 520, easing: backOut }}>
          <div class="loro-marco">
            <Criatura size={156} estado="celebrando" cantando tinta={$tintaVoz} {mirada} />
            <svg class="llave gira-rapido" viewBox="0 0 64 64" aria-hidden="true">
              <g stroke="#2b2418" stroke-width="1.6" stroke-linejoin="round">
                <rect x="44" y="34.4" width="12" height="3.2" rx="1.4" fill="#e8b41a" />
                <path d="M56 36 c0 -4 4 -6 6 -3 c1.4 2 0 4 -2 4 c2 0 3.4 2 2 4 c-2 3 -6 1 -6 -3 Z" fill="#e8b41a" />
              </g>
            </svg>
          </div>
          <div class="bandada">
            {#each bandada as tinta, i (tinta)}
              <span class="pajaro" style="--i:{i}"><Criatura size={44} cantando estado="celebrando" {tinta} mirando={i % 2 ? -1 : 1} /></span>
            {/each}
          </div>
          <h2 class="yap-grito titulo">{$t("pro.gracias_titulo")}</h2>
          <p class="lectura">{aviso ?? $t("pro.gracias_texto")}</p>
          <button class="yap-tecla tecla-grande" use:presionable={{ hap: "rigid" }} onclick={cerrar}>{$t("comun.hecho")}</button>
        </div>
      {:else}
        <!-- ── LA VIÑETA: el loro y sus atrezzos ── -->
        <div class="escena">
          <div class="loro-marco" class:en-percha={motivo === "percha"}>
            <Criatura
              size={148}
              estado={verguenza ? "avergonzado" : comprando || leyendo ? "hablando" : motivo === "percha" ? "avergonzado" : "posado"}
              apertura={leyendo ? repro.nivel : null}
              cantando={comprando}
              tinta={$tintaVoz}
              {mirada}
            />
            {#if leyendo && fraseLeyendo}
              <!-- Lo que está diciendo ahora mismo, en la tinta de la pieza. -->
              <p class="leyendo" in:fade>
                <span class="yap-susurro">{$t("pro.leyendo")}</span>
                <span class="frase">{fraseLeyendo}</span>
              </p>
            {/if}
            <!-- La llave de cuerda en la espalda: tiembla al elegir, gira al comprar. -->
            <svg class="llave" class:tiembla={llaveTiembla} class:gira={comprando} viewBox="0 0 64 64" aria-hidden="true">
              <g stroke="#2b2418" stroke-width="1.6" stroke-linejoin="round">
                <rect x="44" y="34.4" width="12" height="3.2" rx="1.4" fill="#e8b41a" />
                <path d="M56 36 c0 -4 4 -6 6 -3 c1.4 2 0 4 -2 4 c2 0 3.4 2 2 4 c-2 3 -6 1 -6 -3 Z" fill="#e8b41a" />
              </g>
            </svg>
            {#if motivo === "percha"}
              <!-- La percha: la barra bajo las patas, tres pegatinas colgadas
                   y la cuarta resbalando por el borde. -->
              <div class="percha" aria-hidden="true">
                <span class="barra"></span>
                <span class="barra-sombra"></span>
                {#each COLGADAS as c, i (c.troquel.nombre)}
                  <span class="colgada" style="--i:{i}; --giro:{c.giro}deg">
                    <span class="hilo"></span>
                    <span class="pega" style="background:{c.tinta}; clip-path:{aPoligono(c.troquel)}"></span>
                    <svg class="pega-costura" viewBox="0 0 100 100" preserveAspectRatio="none"><polygon points={aPuntosSvg(c.troquel, 0.8)} fill="none" stroke={tonoHondo(c.tinta)} stroke-width="1.6" stroke-dasharray="4 3" vector-effect="non-scaling-stroke" /></svg>
                  </span>
                {/each}
                <span class="que-cae">
                  <span class="pega" style="background:{PALETA[7]}; clip-path:{aPoligono(QUE_CAE)}"></span>
                  <svg class="pega-costura" viewBox="0 0 100 100" preserveAspectRatio="none"><polygon points={aPuntosSvg(QUE_CAE, 0.8)} fill="none" stroke={tonoHondo(PALETA[7])} stroke-width="1.6" stroke-dasharray="4 3" vector-effect="non-scaling-stroke" /></svg>
                </span>
              </div>
              <span class="tampon">{compras.cuota.usados}/{compras.cuota.limite}</span>
            {:else if motivo === "imprenta"}
              <!-- La pila de libros recién impresos. -->
              <svg class="libros" viewBox="0 0 60 44" aria-hidden="true">
                <g stroke="#2b2418" stroke-width="2" stroke-linejoin="round">
                  <rect x="6" y="28" width="46" height="9" rx="2" fill={PALETA[4]} transform="rotate(-2 29 32)" />
                  <rect x="10" y="18" width="40" height="9" rx="2" fill={PALETA[7]} transform="rotate(1.5 30 22)" />
                  <rect x="14" y="8" width="34" height="9" rx="2" fill={PALETA[1]} transform="rotate(-1 31 12)" />
                </g>
                <path d="M20 12 h20 M17 22 h26 M13 32 h32" stroke="#fffdf7" stroke-width="1.6" stroke-linecap="round" opacity="0.8" />
              </svg>
            {:else if motivo === "puente"}
              <!-- El ordenador al otro lado del puente. -->
              <svg class="ordenador" viewBox="0 0 64 44" aria-hidden="true">
                <g stroke="#2b2418" stroke-width="2" stroke-linejoin="round">
                  <rect x="12" y="6" width="40" height="26" rx="3" fill="#fffdf7" transform="rotate(-1.5 32 19)" />
                  <path d="M6 36 h52 l-3 4 h-46 Z" fill="#f3ecdb" />
                </g>
                <path d="M19 14 h22 M19 19 h16 M19 24 h20" stroke={PALETA[3]} stroke-width="2" stroke-linecap="round" />
              </svg>
            {/if}
          </div>

          <h2 class="yap-grito titulo">{titulo}</h2>
          <p class="lectura">{texto}</p>

          <!-- Lo que trae la cuerda: cuatro líneas, iconos a mano. -->
          <ul class="ventajas">
            <li>
              <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 12 c2 -5 4 -5 6 0 s4 5 6 0 s3 -4 5 0" /></svg>
              <span>{$t("pro.ventaja_voz")}</span>
            </li>
            <li>
              <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 19 h14 M6 15 h12 M7 11 h10 M8 7 h8" /></svg>
              <span>{$t("pro.ventaja_imprenta")}</span>
            </li>
            <li>
              <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3 16 c4 -9 14 -9 18 0 M6 16 v3 M18 16 v3 M12 12 v7" /></svg>
              <span>{$t("pro.ventaja_puente")}</span>
            </li>
            <li>
              <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 20 c-6 -4 -9 -8 -8 -12 c1 -4 6 -4 8 0 c2 -4 7 -4 8 0 c1 4 -2 8 -8 12 Z" /></svg>
              <span>{$t("pro.ventaja_loro")}</span>
            </li>
          </ul>

          <!-- Las tres cuerdas: pegatinas troqueladas. -->
          <p class="yap-susurro elige">{$t("pro.elige")}</p>
          <div class="cuerdas" role="radiogroup" aria-label={$t("pro.elige")}>
            {#each CUERDAS as c (c.tipo)}
              {@const p = porTipo[c.tipo] ?? null}
              {@const activa = elegida === c.tipo}
              <button
                class="cuerda"
                class:activa
                class:sin-precio={!p}
                role="radio"
                aria-checked={activa}
                style="--tinta:{c.tinta}; --honda:{tonoHondo(c.tinta)}; --peso:{c.peso}; --giro:{activa ? 0 : c.giro}deg; --dy:{c.dy}px; --clip:{aPoligono(c.troquel)}; --clip-in:{aPoligono(c.troquel, 0.93)};"
                use:presionable={{ hap: "soft" }}
                onclick={() => elegir(c.tipo)}
                disabled={comprando}
              >
                <span class="capa sombra" aria-hidden="true"></span>
                <span class="capa borde" aria-hidden="true"></span>
                <span class="capa cuerpo" aria-hidden="true"></span>
                <svg class="costura" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
                  <polygon points={aPuntosSvg(c.troquel, 0.84)} fill="none" stroke="var(--honda)" stroke-width="1.4" stroke-dasharray="3 2.6" vector-effect="non-scaling-stroke" />
                </svg>
                <span class="etiqueta">
                  {#if p}
                    <strong class="precio yap-grito">{p.precio}</strong>
                    <span class="periodo">{periodoDe(c.tipo)}</span>
                  {:else}
                    <strong class="precio yap-grito nombre-solo">{nombreDe(c.tipo)}</strong>
                  {/if}
                </span>
                <!-- El nombre de la cuerda: la pastilla cosida al canto inferior. -->
                <span class="pastilla">{nombreDe(c.tipo)}</span>
                {#if c.tipo === "anual"}
                  <span class="tampon-cuerda izquierda">{$t("pro.mas_pedida")}</span>
                  {#if ahorro}
                    <span class="tampon-cuerda derecha">{f("pro.ahorras", { pct: ahorro })}</span>
                  {/if}
                {/if}
                {#if c.tipo === "vida"}
                  <span class="tampon-cuerda derecha">{$t("pro.para_siempre")}</span>
                {/if}
              </button>
            {/each}
          </div>

          {#if compras.cargandoOfertas && !compras.paquetes.length}
            <p class="yap-susurro nota">{$t("pro.cargando")}</p>
          {:else if !compras.paquetes.length}
            <p class="nota peligro">{$t("pro.sin_ofertas")}</p>
            <button class="yap-boton" use:presionable onclick={() => cargarOfertas(true)}>{$t("cinta.error")}</button>
          {:else if equivalente(paqueteElegido)}
            <p class="yap-susurro nota">{equivalente(paqueteElegido)}</p>
          {/if}

          <button
            class="yap-tecla tecla-grande"
            class:cocinando={comprando}
            use:presionable={{ hap: "rigid" }}
            onclick={darCuerda}
            disabled={!paqueteElegido || comprando}
          >
            {#if comprando}
              <span class="giro-llave" aria-hidden="true">
                <svg viewBox="0 0 24 24" width="18" height="18"><g stroke="currentColor" stroke-width="2" stroke-linejoin="round" fill="none"><path d="M3 12 h9" /><path d="M12 12 c0 -3.5 4 -5 5.5 -2.5 c1 1.7 0 2.5 -1.5 2.5 c1.5 0 2.5 .8 1.5 2.5 c-1.5 2.5 -5.5 1 -5.5 -2.5 Z" /></g></svg>
              </span>
              {$t("pro.comprando")}
            {:else}
              {$t("pro.comprar")}
            {/if}
          </button>

          {#if error}
            <p class="nota peligro" in:fly={{ y: 6, duration: 200 }}>{error}</p>
          {:else if aviso}
            <p class="nota" in:fly={{ y: 6, duration: 200 }}>{aviso}</p>
          {:else}
            <p class="nota">{$t("pro.cancelas")}</p>
          {/if}

          <button class="enlace restaurar" use:presionable={{ hap: "soft" }} onclick={yaEraParlanchin} disabled={restaurando || comprando}>
            {restaurando ? $t("pro.cargando") : $t("pro.restaurar")}
          </button>

          <p class="legal">{$t("pro.renovacion")}</p>
          <div class="enlaces">
            <button class="enlace" use:presionable={{ hap: "soft" }} onclick={() => abrirEnlace(URL_TERMINOS)}>{$t("pro.terminos")}</button>
            <span aria-hidden="true">·</span>
            <button class="enlace" use:presionable={{ hap: "soft" }} onclick={() => abrirEnlace(urlPrivacidad)}>{$t("pro.privacidad")}</button>
          </div>

          <button class="enlace luego" use:presionable={{ hap: "soft" }} onclick={cerrar}>{motivo === "paseo" ? $t("pro.empezar_percha") : $t("pro.luego")}</button>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  /* ── El telón: papel empañado sobre lo que hubiera ── */
  .telon {
    position: fixed;
    inset: 0;
    z-index: 93;
    /* Nada asoma por los bordes: la hoja va rotada y sus esquinas se
       salían de la pantalla en los teléfonos pequeños. */
    overflow: hidden;
    background: color-mix(in srgb, var(--yap-papel) 78%, #2b2418 8%);
    backdrop-filter: blur(9px);
    -webkit-backdrop-filter: blur(9px);
    display: flex;
    align-items: flex-end;
    justify-content: center;
    padding: calc(env(safe-area-inset-top) + 18px) 10px calc(env(safe-area-inset-bottom) + 8px);
    overscroll-behavior: contain;
  }
  /* ── La página del álbum ── */
  .pagina {
    position: relative;
    width: min(100%, 460px);
    max-height: 100%;
    overflow-y: auto;
    overscroll-behavior: contain;
    touch-action: pan-y;
    transition: transform 0.28s cubic-bezier(0.24, 1.7, 0.44, 1);
    will-change: transform;
    -webkit-overflow-scrolling: touch;
    background: var(--yap-superficie);
    border: 1.5px solid var(--yap-borde);
    border-radius: 26px;
    box-shadow: 4px 5px 0 #ded7c2;
    padding: 30px 18px 18px;
    scrollbar-width: none;
  }
  .pagina.tirando {
    transition: none;
  }
  .tirador-zona {
    position: relative;
    z-index: 5;
    display: flex;
    align-items: center;
    justify-content: center;
    height: 28px;
    margin: -22px -18px 2px;
    /* El gesto es SUYO: sin esto el navegador se lo queda para el scroll y
       el arrastre para cerrar no llegaba nunca. */
    touch-action: none;
    cursor: grab;
  }
  .tirador {
    display: block;
    width: 46px;
    height: 5px;
    border-radius: 999px;
    background: var(--yap-tinta-suave);
    opacity: 0.5;
  }
  .pagina::-webkit-scrollbar { display: none; }
  :global([data-theme="dark"]) .pagina { box-shadow: 4px 5px 0 rgba(0, 0, 0, 0.5); }
  .celo {
    position: absolute;
    width: 54px;
    height: 16px;
    background: rgba(232, 180, 26, 0.35);
    border: 1px solid rgba(232, 180, 26, 0.5);
    top: -7px;
    z-index: 2;
  }
  .celo-a { left: 26px; transform: rotate(-9deg); }
  .celo-b { right: 60px; transform: rotate(7deg); }
  .cerrar {
    position: absolute;
    top: 10px;
    right: 10px;
    width: 40px;
    height: 40px;
    border-radius: 13px;
    border: 1.5px solid var(--yap-borde);
    background: var(--yap-papel);
    color: var(--yap-tinta);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    box-shadow: 2px 2.5px 0 #ded7c2;
    cursor: pointer;
    z-index: 3;
  }
  .escena {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    text-align: center;
  }

  /* ── El loro y sus atrezzos ── */
  .leyendo {
    position: absolute;
    left: -60px;
    right: -60px;
    bottom: -34px;
    margin: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    pointer-events: none;
  }
  .leyendo .frase {
    font-family: var(--yap-lectura);
    font-size: 13px;
    line-height: 1.3;
    color: var(--yap-tinta);
    background: var(--yap-subrayador);
    border-radius: 4px;
    padding: 1px 6px;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    max-width: 300px;
  }
  .loro-marco {
    position: relative;
    width: 200px;
    height: 160px;
    display: flex;
    align-items: flex-end;
    justify-content: center;
  }
  .llave {
    position: absolute;
    width: 148px;
    height: 148px;
    left: 50%;
    bottom: 0;
    margin-left: -74px;
    pointer-events: none;
    transform-origin: 60px 36px;
  }
  .en-percha .llave { transform: translateY(-14px); }
  .en-percha :global(.criatura) { transform: translateY(-14px); }
  .llave.tiembla { animation: llave-tiembla 0.5s ease; }
  .llave.gira { animation: llave-gira 0.9s linear infinite; }
  .llave.gira-rapido { animation: llave-gira 0.42s linear infinite; }
  @keyframes llave-tiembla {
    0%, 100% { transform: rotate(0); }
    20% { transform: rotate(-22deg); }
    50% { transform: rotate(16deg); }
    75% { transform: rotate(-8deg); }
  }
  /* La llave gira ALREDEDOR de su cabeza (esquina de la espalda). */
  @keyframes llave-gira {
    from { transform: rotate(0); }
    to { transform: rotate(360deg); }
  }
  /* ── La percha ── */
  .percha {
    position: absolute;
    left: 50%;
    bottom: 0;
    width: 190px;
    height: 60px;
    margin-left: -95px;
    pointer-events: none;
  }
  .barra {
    position: absolute;
    left: 6px;
    right: 6px;
    top: 4px;
    height: 9px;
    border-radius: 5px;
    background: #b8651f;
    border: 1.5px solid #2b2418;
  }
  .barra-sombra {
    position: absolute;
    left: 9px;
    right: 3px;
    top: 8px;
    height: 9px;
    border-radius: 5px;
    background: #ded7c2;
    z-index: -1;
  }
  .colgada {
    position: absolute;
    top: 12px;
    left: calc(22px + var(--i) * 48px);
    width: 38px;
    height: 46px;
    transform: rotate(var(--giro));
    transform-origin: 50% 0;
    animation: mece 3.4s ease-in-out infinite;
    animation-delay: calc(var(--i) * 0.45s);
  }
  .hilo {
    position: absolute;
    left: 50%;
    top: -2px;
    width: 1.5px;
    height: 10px;
    background: #2b2418;
  }
  .pega, .pega-costura {
    position: absolute;
    left: 0;
    top: 8px;
    width: 38px;
    height: 38px;
  }
  .que-cae {
    position: absolute;
    top: 6px;
    right: -14px;
    width: 38px;
    height: 46px;
    transform: rotate(28deg);
    animation: resbala 2.6s ease-in-out infinite;
    opacity: 0.95;
  }
  @keyframes mece {
    0%, 100% { transform: rotate(calc(var(--giro) - 3deg)); }
    50% { transform: rotate(calc(var(--giro) + 3deg)); }
  }
  @keyframes resbala {
    0%, 100% { transform: rotate(24deg) translate(0, 0); }
    50% { transform: rotate(38deg) translate(6px, 10px); }
  }
  .tampon {
    position: absolute;
    left: 2px;
    top: 18px;
    font-family: var(--yap-mono);
    font-size: 11px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--yap-peligro);
    border: 2px solid var(--yap-peligro);
    border-radius: 6px;
    padding: 3px 7px;
    transform: rotate(-14deg);
    opacity: 0.85;
    mix-blend-mode: multiply;
  }
  :global([data-theme="dark"]) .tampon { mix-blend-mode: normal; }
  .libros { position: absolute; width: 74px; right: 2px; bottom: 4px; transform: rotate(3deg); }
  .ordenador { position: absolute; width: 78px; right: 0; bottom: 6px; transform: rotate(-2deg); }

  .titulo {
    margin: 4px 0 0;
    font-size: clamp(22px, 6.4vw, 27px);
    color: var(--yap-tinta);
    transform: rotate(-0.8deg);
    text-wrap: balance;
  }
  .lectura {
    margin: 0;
    font-family: var(--yap-lectura);
    font-size: 15.5px;
    line-height: 1.5;
    color: var(--yap-tinta);
    max-width: 34ch;
    text-wrap: pretty;
  }

  /* ── Las ventajas: líneas de formulario con iconos a mano ── */
  .ventajas {
    list-style: none;
    margin: 6px 0 0;
    padding: 0;
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 4px;
    text-align: left;
  }
  .ventajas li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 4px;
    border-bottom: 1.5px dotted var(--yap-borde);
    font-weight: 700;
    font-size: 14.5px;
    color: var(--yap-tinta);
  }
  .ventajas li:nth-child(odd) { transform: rotate(-0.25deg); }
  .ventajas li:nth-child(even) { transform: rotate(0.3deg); }
  .ventajas svg {
    flex: 0 0 auto;
    width: 24px;
    height: 24px;
    fill: none;
    stroke: var(--vivo, var(--voz-tinta, var(--yap-voz)));
    stroke-width: 2.2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  /* ── Las tres cuerdas ── */
  .elige { margin: 10px 0 0; }
  .cuerdas {
    display: flex;
    align-items: stretch;
    justify-content: center;
    gap: 2px;
    width: calc(100% + 16px);
    margin: 10px -8px 0;
    height: 176px;
  }
  .cuerda {
    position: relative;
    flex: var(--peso) 1 0;
    min-width: 0;
    appearance: none;
    border: 0;
    background: transparent;
    padding: 0;
    cursor: pointer;
    color: #fffdf7;
    transform: rotate(var(--giro)) scale(0.94);
    transition: transform 0.42s cubic-bezier(0.24, 1.7, 0.44, 1), filter 0.2s;
    font: inherit;
  }
  .cuerda.activa { transform: rotate(var(--giro)) scale(1.08); z-index: 1; }
  .cuerda:not(.activa) { filter: saturate(0.9) brightness(0.97); }
  .cuerda:disabled { cursor: default; }
  .capa {
    position: absolute;
    inset: 0;
    clip-path: var(--clip);
  }
  .capa.sombra { background: #ded7c2; transform: translate(3px, 4px); }
  :global([data-theme="dark"]) .capa.sombra { background: rgba(0, 0, 0, 0.45); }
  .capa.borde { background: var(--yap-superficie); }
  .capa.cuerpo { background: var(--tinta); clip-path: var(--clip-in); transition: clip-path 0.4s ease; }
  .costura {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
  }
  .etiqueta {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
    padding: 22px 8px 30px;
    gap: 3px;
    transform: translateY(var(--dy));
  }
  .precio {
    font-size: clamp(16px, 5.4vw, 22px);
    line-height: 1;
    white-space: nowrap;
    text-shadow: 0 1px 0 var(--honda);
  }
  .cuerda.activa .precio { font-size: clamp(19px, 6.4vw, 26px); }
  .precio.nombre-solo { font-size: clamp(13px, 4.2vw, 17px); white-space: normal; line-height: 1.05; }
  .periodo {
    font-family: var(--yap-mono);
    font-size: 9.5px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    opacity: 0.95;
    white-space: nowrap;
  }
  .pastilla {
    position: absolute;
    left: 50%;
    bottom: 4px;
    transform: translateX(-50%) rotate(-2deg);
    background: var(--honda);
    color: #fffdf7;
    font-weight: 800;
    font-size: 11px;
    padding: 4px 10px;
    border-radius: 999px;
    white-space: nowrap;
    border: 1.5px solid var(--yap-superficie);
  }
  .cuerda.activa .pastilla { transform: translateX(-50%) rotate(0); }
  .tampon-cuerda {
    position: absolute;
    top: 2px;
    background: #e8b41a;
    color: #2b2418;
    font-weight: 800;
    font-size: 10px;
    padding: 3px 8px;
    border-radius: 10px;
    border: 1.5px solid #2b2418;
    box-shadow: 2px 2px 0 #2b2418;
    white-space: nowrap;
    z-index: 2;
  }
  .tampon-cuerda.izquierda { left: -6px; transform: rotate(-8deg); background: #fffdf7; }
  .tampon-cuerda.derecha { right: -6px; transform: rotate(8deg); }

  .nota {
    margin: 4px 0 0;
    font-size: 12.5px;
    color: var(--yap-tinta-suave);
    max-width: 36ch;
  }
  .nota.peligro { color: var(--yap-peligro); font-weight: 700; }
  .tecla-grande {
    margin-top: 8px;
    width: 100%;
    min-height: 56px;
    font-size: 18px;
    font-weight: 800;
    border-radius: 18px;
    background: var(--vivo, var(--voz-tinta, var(--yap-voz)));
    color: #fffdf7;
    box-shadow: 3px 4px 0 #2b2418;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
    transform: rotate(-0.4deg);
  }
  .tecla-grande.cocinando { filter: none; opacity: 0.92; }
  .giro-llave { display: inline-flex; animation: llave-gira 0.9s linear infinite; }
  .enlace {
    border: 0;
    background: transparent;
    color: var(--yap-ultramar);
    font: inherit;
    font-weight: 700;
    font-size: 13.5px;
    text-decoration: underline;
    text-underline-offset: 4px;
    text-decoration-thickness: 2px;
    padding: 8px 10px;
    cursor: pointer;
    min-height: 40px;
  }
  .enlace:disabled { opacity: 0.6; }
  .restaurar { margin-top: 2px; }
  .legal {
    margin: 6px 0 0;
    font-family: var(--yap-lectura);
    font-size: 11px;
    line-height: 1.4;
    color: var(--yap-tinta-suave);
    max-width: 44ch;
  }
  .enlaces {
    display: flex;
    align-items: center;
    gap: 2px;
    color: var(--yap-tinta-suave);
  }
  .enlaces .enlace { font-size: 12.5px; padding: 6px 8px; }
  .luego { color: var(--yap-tinta-suave); margin-top: 2px; }

  /* ── La celebración ── */
  .celebra .loro-marco { height: 168px; }
  .escena:has(.leyendo) .titulo { margin-top: 30px; }
  .bandada {
    display: flex;
    gap: 18px;
    margin-top: -6px;
  }
  .pajaro {
    display: inline-block;
    animation: brinca-pajaro 0.62s ease-in-out infinite alternate;
    animation-delay: calc(var(--i) * 0.14s);
  }
  @keyframes brinca-pajaro {
    from { transform: translateY(0) rotate(-3deg); }
    to { transform: translateY(-7px) rotate(3deg); }
  }

  @media (prefers-reduced-motion: reduce) {
    .llave, .colgada, .que-cae, .pajaro, .giro-llave { animation: none !important; }
    .cuerda { transition: none; }
  }
</style>
