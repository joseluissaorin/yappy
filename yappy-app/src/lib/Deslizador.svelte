<script lang="ts">
  // El deslizador de la casa: fuera el input[type=range] del navegador.
  // Pulgar grande con relieve, pista rellena con la tinta de la voz,
  // BURBUJA de valor mientras arrastras, e IMANES: valores con gravedad
  // (1×, 1,25×, 1,5×…) que atrapan el pulgar con un tick háptico.
  import { haptic } from "$lib/haptic";

  let {
    value = $bindable(1),
    min = 0,
    max = 1,
    step = 0.05,
    imanes = [] as number[],
    formatear = (v: number) => v.toFixed(2),
    etiqueta = "",
    disabled = false,
    alCambiar = undefined as ((v: number) => void) | undefined,
    alSoltar = undefined as ((v: number) => void) | undefined,
  } = $props();

  let pista = $state<HTMLDivElement | null>(null);
  let arrastrando = $state(false);

  const pct = $derived(((value - min) / (max - min)) * 100);

  function cuantizar(v: number): number {
    const crudo = Math.max(min, Math.min(max, v));
    // Los imanes atrapan dentro de su radio de gravedad.
    const radio = (max - min) * 0.022;
    for (const im of imanes) {
      if (Math.abs(crudo - im) < radio) return im;
    }
    return Math.round(crudo / step) * step;
  }

  function valorDesde(e: PointerEvent): number {
    if (!pista) return value;
    const r = pista.getBoundingClientRect();
    const f = Math.max(0, Math.min(1, (e.clientX - r.left) / r.width));
    return cuantizar(min + f * (max - min));
  }

  function abajo(e: PointerEvent) {
    if (disabled) return;
    arrastrando = true;
    (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
    mover(e);
  }
  function mover(e: PointerEvent) {
    if (!arrastrando || disabled) return;
    const nuevo = valorDesde(e);
    if (Math.abs(nuevo - value) > 1e-9) {
      // Tick al cruzar cada paso; los imanes suenan igual pero se SIENTEN
      // porque el pulgar se queda pegado.
      haptic("tick");
      value = nuevo;
      alCambiar?.(nuevo);
    }
  }
  function arriba() {
    if (!arrastrando) return;
    arrastrando = false;
    alSoltar?.(value);
  }
</script>

<div
  class="deslizador-casa"
  class:arrastrando
  class:deshabilitado={disabled}
  role="slider"
  aria-label={etiqueta}
  aria-valuemin={min}
  aria-valuemax={max}
  aria-valuenow={value}
  aria-valuetext={formatear(value)}
  tabindex="0"
  onpointerdown={abajo}
  onpointermove={mover}
  onpointerup={arriba}
  onpointercancel={arriba}
  onkeydown={(e) => {
    if (disabled) return;
    if (e.key === "ArrowRight" || e.key === "ArrowUp") { value = cuantizar(value + step); alCambiar?.(value); alSoltar?.(value); }
    if (e.key === "ArrowLeft" || e.key === "ArrowDown") { value = cuantizar(value - step); alCambiar?.(value); alSoltar?.(value); }
  }}
>
  <div class="pista" bind:this={pista}>
    <div class="lleno" style="width: {pct}%"></div>
    {#each imanes as im (im)}
      <span class="iman" style="left: {((im - min) / (max - min)) * 100}%"></span>
    {/each}
    <div class="pulgar" style="left: {pct}%">
      {#if arrastrando}
        <span class="burbuja">{formatear(value)}</span>
      {/if}
    </div>
  </div>
</div>

<style>
  .deslizador-casa {
    width: 100%;
    padding: 14px 0 10px;
    touch-action: pan-y;
    cursor: pointer;
    -webkit-tap-highlight-color: transparent;
  }
  .deslizador-casa.deshabilitado {
    opacity: 0.45;
    pointer-events: none;
  }
  .pista {
    position: relative;
    height: 10px;
    border-radius: 6px;
    background: var(--yap-superficie-2, #ece4d2);
    box-shadow: inset 0 1.5px 3px rgba(64, 46, 12, 0.14);
  }
  .lleno {
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    border-radius: 6px;
    background: linear-gradient(90deg, var(--acento-voz, var(--yap-voz, #e0502a)), var(--acento-voz-claro, #f4682e));
    transition: width 0.06s linear;
  }
  .iman {
    position: absolute;
    top: 50%;
    width: 4px;
    height: 4px;
    border-radius: 50%;
    background: var(--yap-tinta-suave, #82755a);
    opacity: 0.55;
    transform: translate(-50%, -50%);
  }
  .pulgar {
    position: absolute;
    top: 50%;
    width: 30px;
    height: 30px;
    border-radius: 50%;
    background: var(--yap-superficie, #fffdf7);
    border: 2.5px solid var(--acento-voz, var(--yap-voz, #e0502a));
    box-shadow: 0 3px 8px rgba(64, 46, 12, 0.22);
    transform: translate(-50%, -50%);
    transition: transform 0.14s cubic-bezier(0.34, 1.56, 0.64, 1), left 0.06s linear;
  }
  .arrastrando .pulgar {
    transform: translate(-50%, -50%) scale(1.22);
  }
  .burbuja {
    position: absolute;
    bottom: calc(100% + 12px);
    left: 50%;
    transform: translateX(-50%);
    background: var(--yap-tinta, #2b2418);
    color: var(--yap-papel, #f7f2e7);
    font-weight: 800;
    font-size: 15px;
    padding: 6px 12px;
    border-radius: 12px;
    white-space: nowrap;
    box-shadow: 0 6px 14px rgba(64, 46, 12, 0.25);
    animation: burbuja-nace 0.16s cubic-bezier(0.34, 1.56, 0.64, 1) both;
  }
  .burbuja::after {
    content: "";
    position: absolute;
    top: 100%;
    left: 50%;
    transform: translateX(-50%);
    border: 5px solid transparent;
    border-top-color: var(--yap-tinta, #2b2418);
  }
  @keyframes burbuja-nace {
    from { transform: translateX(-50%) translateY(4px) scale(0.8); opacity: 0; }
    to { transform: translateX(-50%) translateY(0) scale(1); opacity: 1; }
  }
</style>
