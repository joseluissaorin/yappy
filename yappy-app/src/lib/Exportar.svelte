<script lang="ts">
  // EXPORTAR, en pegatinas: tres troqueles de la carta (nube, hexágono,
  // sello), cada uno con su tinta, su costura y su sombra dura. Tocar uno
  // lo despega (el niño juega); el filólogo tiene su .md, .html o .pdf a
  // un toque, con la hoja de compartir del sistema o el diálogo de guardar.
  import { TROQUELES_BASE, aPuntosSvg } from "$lib/troquel";
  import { tonoHondo } from "$lib/juguete";
  import { presionable } from "$lib/presionable";
  import { haptic } from "$lib/haptic";
  import { t } from "$lib/i18n";
  import { exportar, contarError, type FuenteExportar } from "$lib/exportar";
  import type { FormatoExportar } from "$lib/ipc";

  let {
    fuente,
    alTerminar,
    compacto = false,
  }: {
    fuente: FuenteExportar;
    /// Tras compartir o guardar (para cerrar la hoja que nos contiene).
    alTerminar?: (r: "compartido" | "guardado") => void;
    compacto?: boolean;
  } = $props();

  const PIEZAS: { f: FormatoExportar; rotulo: string; pista: string; tinta: string; forma: string; giro: number }[] = [
    { f: "md", rotulo: ".md", pista: "exportar.md", tinta: "#5D5FEF", forma: "nube", giro: -4 },
    { f: "html", rotulo: ".html", pista: "exportar.html", tinta: "#00A896", forma: "hexagono", giro: 3 },
    { f: "pdf", rotulo: ".pdf", pista: "exportar.pdf", tinta: "#F94892", forma: "sello", giro: -2 },
  ];
  const forma = (n: string) => TROQUELES_BASE.find((x) => x.nombre === n) ?? TROQUELES_BASE[0];

  let trabajando = $state<FormatoExportar | null>(null);
  let sello = $state<{ f: FormatoExportar; ok: boolean } | null>(null);
  let selloTimer: ReturnType<typeof setTimeout> | undefined;

  async function pulsar(f: FormatoExportar) {
    if (trabajando) return;
    haptic("medium");
    trabajando = f;
    sello = null;
    try {
      const r = await exportar(f, fuente);
      if (r === "cancelado") return;
      haptic("success");
      sello = { f, ok: true };
      clearTimeout(selloTimer);
      selloTimer = setTimeout(() => (sello = null), 2600);
      alTerminar?.(r);
    } catch (e) {
      contarError(e);
      haptic("error");
      sello = { f, ok: false };
      clearTimeout(selloTimer);
      selloTimer = setTimeout(() => (sello = null), 4000);
    } finally {
      trabajando = null;
    }
  }
</script>

<div class="exportar" class:compacto>
  {#each PIEZAS as p (p.f)}
    <button
      class="pegatina"
      class:despegada={trabajando === p.f}
      class:quieta={trabajando !== null && trabajando !== p.f}
      style="--giro: {p.giro}deg; --tinta: {p.tinta}"
      use:presionable={{ hap: "soft" }}
      onclick={() => pulsar(p.f)}
      disabled={trabajando !== null}
      aria-label="{$t('exportar.boton')} {p.rotulo} · {$t(p.pista)}"
    >
      <svg class="troquel" viewBox="0 0 100 100" aria-hidden="true">
        <polygon points={aPuntosSvg(forma(p.forma), 1)} fill="var(--yap-superficie, #fffdf7)" />
        <polygon points={aPuntosSvg(forma(p.forma), 0.9)} fill={p.tinta} />
        <polygon
          points={aPuntosSvg(forma(p.forma), 0.8)}
          fill="none"
          stroke={tonoHondo(p.tinta)}
          stroke-width="1.6"
          stroke-dasharray="4 3.2"
          stroke-linecap="round"
        />
      </svg>
      <span class="rotulo">{p.rotulo}</span>
      {#if sello?.f === p.f}
        <span class="sello" class:mal={!sello.ok} aria-live="polite">
          <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            {#if sello.ok}<path d="M5.2 12.6 Q8 15 9.8 17.6 Q13.4 10.6 19 6.2" />{:else}<path d="M12 5.5 Q11.6 10 12 13.2 M12 18.2 v0.1" />{/if}
          </svg>
        </span>
      {/if}
    </button>
  {/each}
</div>
{#if !compacto}
  <div class="pistas" aria-hidden="true">
    {#each PIEZAS as p (p.f)}
      <span>{$t(p.pista)}</span>
    {/each}
  </div>
{/if}
{#if sello && !sello.ok}
  <p class="aviso" role="alert">{$t("exportar.error")}</p>
{:else if sello?.ok && !compacto}
  <p class="aviso bien" aria-live="polite">{$t("exportar.listo")}</p>
{/if}

<style>
  .exportar {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 10px;
    padding: 4px 6px 0;
    justify-items: center;
  }
  .pegatina {
    position: relative;
    width: 86px;
    height: 74px;
    padding: 0;
    border: 0;
    background: none;
    cursor: pointer;
    rotate: var(--giro);
    transition:
      transform 0.16s cubic-bezier(0.3, 1.4, 0.6, 1),
      rotate 0.42s cubic-bezier(0.24, 1.7, 0.44, 1),
      opacity 0.2s;
    -webkit-tap-highlight-color: transparent;
  }
  .compacto .pegatina {
    width: 64px;
    height: 56px;
  }
  .troquel {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    overflow: visible;
    /* Sombra dura de contacto, sin desenfoque: nada flota. */
    filter: drop-shadow(2.5px 3px 0 #ded7c2);
  }
  .rotulo {
    position: relative;
    font-family: var(--yap-fuente, "Baloo 2", system-ui, sans-serif);
    font-weight: 800;
    font-size: 19px;
    letter-spacing: -0.01em;
    color: #fff6ef;
    text-shadow: 1px 1.5px 0 color-mix(in srgb, var(--tinta) 60%, #000);
  }
  .compacto .rotulo {
    font-size: 15px;
  }
  :global(.pegatina.pulsado) {
    transform: translateY(2px) scale(0.96);
  }
  /* Al tocarla, la pegatina se despega del papel y tiembla mientras se
     imprime: se levanta (sombra larga) y se mece. */
  .pegatina.despegada {
    transform: translateY(-5px) scale(1.06);
    animation: mecer 0.5s ease-in-out infinite alternate;
  }
  .pegatina.despegada .troquel {
    filter: drop-shadow(4px 6px 0 #ded7c2);
  }
  .pegatina.quieta {
    opacity: 0.5;
  }
  .pegatina:disabled {
    cursor: default;
  }
  @keyframes mecer {
    from { rotate: calc(var(--giro) - 5deg); }
    to { rotate: calc(var(--giro) + 5deg); }
  }
  /* El tampón del resultado: visto dorado (o aviso coral). */
  .sello {
    position: absolute;
    top: -8px;
    right: -6px;
    width: 26px;
    height: 26px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    border: 1.5px solid #2b2418;
    background: var(--yap-dorado, #e8b41a);
    color: #2b2418;
    font-weight: 800;
    font-size: 14px;
    box-shadow: 1.5px 2px 0 #2b2418;
    rotate: 12deg;
    animation: estampar 0.34s cubic-bezier(0.24, 1.7, 0.44, 1) both;
  }
  .sello.mal {
    background: var(--yap-voz, #e0502a);
    color: #fff6ef;
  }
  @keyframes estampar {
    from { transform: scale(2.2); opacity: 0; }
    to { transform: scale(1); opacity: 1; }
  }
  .pistas {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 10px;
    margin-top: 8px;
    text-align: center;
    font-family: var(--yap-mono, ui-monospace, monospace);
    font-size: 10px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    line-height: 1.35;
    color: var(--yap-tinta-suave, #82755a);
  }
  .aviso {
    margin: 8px 0 0;
    text-align: center;
    font-size: 13px;
    font-weight: 700;
    color: var(--yap-voz, #e0502a);
  }
  .aviso.bien {
    color: var(--yap-tinta-suave, #82755a);
  }
  @media (prefers-reduced-motion: reduce) {
    .pegatina.despegada { animation: none; }
    .sello { animation: none; }
  }
</style>
