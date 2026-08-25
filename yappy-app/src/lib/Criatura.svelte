<script lang="ts">
  // El loro de Yappy, versión marioneta. Ya no es un dibujo: es un rig con
  // piezas (cabeza, párpados, ojos, boca) y un cerebro de estados. La regla
  // de la casa: el loro ES la app (un loro repite lo que oye), así que
  // habla con la amplitud REAL de la voz (prop «apertura», que llega del
  // evento playback_nivel), mira lo que tocas («mirada»), parpadea solo,
  // se acicala si lo abandonas, come el modelo y duerme de noche.
  //
  //   estado    posado | hablando | pausa | comiendo | dormido |
  //             avergonzado | celebrando
  //   apertura  0..1: cuánto abre la boca (la envolvente de la voz)
  //   mirada    {x,y} en -1..1: hacia dónde miran los ojos
  //   barriga   0..1: la descarga del modelo (comiendo)
  //   andando / cantando / mirando: compatibilidad con los usos previos
  import { onMount, onDestroy } from "svelte";

  let {
    size = 64,
    estado = "posado",
    apertura = null,
    mirada = { x: 0, y: 0 },
    barriga = 0,
    tinta = null,
    andando = false,
    cantando = false,
    mirando = 1,
  }: {
    size?: number;
    estado?: "posado" | "hablando" | "pausa" | "comiendo" | "dormido" | "avergonzado" | "celebrando";
    apertura?: number | null;
    mirada?: { x: number; y: number };
    barriga?: number;
    tinta?: string | null;
    andando?: boolean;
    cantando?: boolean;
    mirando?: 1 | -1;
  } = $props();

  // ── Compatibilidad: cantando sin apertura = aleteo interno de boca ────
  let aleteo = $state(0);
  let aleteoTimer: ReturnType<typeof setInterval> | undefined;
  $effect(() => {
    const habla = (estado === "hablando" || cantando) && apertura === null;
    clearInterval(aleteoTimer);
    if (habla) {
      aleteoTimer = setInterval(() => {
        aleteo = 0.18 + Math.random() * 0.72;
      }, 90 + Math.random() * 60);
    } else {
      aleteo = 0;
    }
    return () => clearInterval(aleteoTimer);
  });
  const boca = $derived(
    estado === "dormido" ? 0 : Math.max(0, Math.min(1, apertura ?? aleteo)),
  );

  // ── Parpadeo con vida propia ──────────────────────────────────────────
  let parpadeo = $state(false);
  let acicalando = $state(false);
  let timers: ReturnType<typeof setTimeout>[] = [];
  function programarParpadeo() {
    const t = setTimeout(() => {
      parpadeo = true;
      const t2 = setTimeout(() => {
        parpadeo = false;
        // Doble parpadeo de vez en cuando: los pájaros lo hacen.
        if (Math.random() < 0.18) {
          const t3 = setTimeout(() => {
            parpadeo = true;
            const t4 = setTimeout(() => (parpadeo = false), 90);
            timers.push(t4);
          }, 140);
          timers.push(t3);
        }
        programarParpadeo();
      }, 110);
      timers.push(t2);
    }, 2600 + Math.random() * 3800);
    timers.push(t);
  }
  function programarAcicalado() {
    const t = setTimeout(() => {
      if (estado === "posado" && !andando && !cantando) {
        acicalando = true;
        const t2 = setTimeout(() => {
          acicalando = false;
          programarAcicalado();
        }, 1100);
        timers.push(t2);
      } else {
        programarAcicalado();
      }
    }, 14000 + Math.random() * 14000);
    timers.push(t);
  }
  onMount(() => {
    programarParpadeo();
    programarAcicalado();
  });
  onDestroy(() => timers.forEach(clearTimeout));

  // ── Derivados de pose ─────────────────────────────────────────────────
  const cabezaGiro = $derived(
    estado === "pausa" ? -9 : estado === "avergonzado" ? 7 : acicalando ? 12 : boca * 1.6,
  );
  const ojosX = $derived(Math.max(-1, Math.min(1, mirada.x)) * 1.7);
  const ojosY = $derived(Math.max(-1, Math.min(1, mirada.y)) * 1.3);
  const parpadosCerrados = $derived(estado === "dormido" ? 0.92 : parpadeo ? 1 : 0);
  const cuerpoAncho = $derived(1 + barriga * 0.14);
  const tintaCuerpo = $derived(tinta ?? "#e0502a");
</script>

<span
  class="criatura"
  class:anda={andando || estado === "celebrando"}
  class:habla={boca > 0.04}
  class:duerme={estado === "dormido"}
  class:verguenza={estado === "avergonzado"}
  style="width:{size}px;height:{size}px;transform:scaleX({mirando});"
  aria-hidden="true"
>
  <svg viewBox="0 0 64 64" width={size} height={size}>
    <g class="cuerpo-vivo" transform="rotate(-3 32 32)">
      <g style="transform: scaleX({cuerpoAncho}); transform-origin: 32px 40px;">
        <g fill="#2f4bc4" transform="translate(1.8 2)">
          <path d="M32 7 C42.5 6.5 50 14 50.5 24 C51 33 49.5 41.5 43 46.4 C39 49.4 25 49.4 21 46.4 C14.5 41.5 13 33 13.5 24 C14 14 21.5 6.5 32 7 Z" />
          <path d="M29.2 7.4 C26.8 1.8 32 -0.6 37.8 1.6 C34.6 3.1 33.7 5 34 7 Z" />
          <ellipse cx="23.5" cy="49.6" rx="5.6" ry="4.2" />
          <ellipse cx="40.5" cy="50.8" rx="5.6" ry="4.2" />
        </g>
        <g fill={tintaCuerpo}>
          <path d="M32 7 C42.5 6.5 50 14 50.5 24 C51 33 49.5 41.5 43 46.4 C39 49.4 25 49.4 21 46.4 C14.5 41.5 13 33 13.5 24 C14 14 21.5 6.5 32 7 Z" />
          <path d="M29.2 7.4 C26.8 1.8 32 -0.6 37.8 1.6 C34.6 3.1 33.7 5 34 7 Z" />
          <ellipse cx="23.5" cy="49.6" rx="5.6" ry="4.2" />
          <ellipse cx="40.5" cy="50.8" rx="5.6" ry="4.2" />
        </g>
      </g>

      <!-- La cabeza: mejillas, ojos con mirada, párpados y la boca. -->
      <g class="cabeza" style="transform: rotate({cabezaGiro}deg); transform-origin: 32px 34px;">
        <circle cx="19.8" cy="29.5" r={estado === "avergonzado" ? 3.8 : 2.9} fill="#f7f2e7" opacity={estado === "avergonzado" ? 0.85 : 0.55} />
        <circle cx="44.2" cy="29.5" r={estado === "avergonzado" ? 3.8 : 2.9} fill="#f7f2e7" opacity={estado === "avergonzado" ? 0.85 : 0.55} />
        <g fill="#2b2418" style="transform: translate({ojosX}px, {ojosY}px);">
          <ellipse cx="25.5" cy="23.5" rx="2" ry="3.7" />
          <ellipse cx="38.5" cy="23.5" rx="2" ry="3.7" />
        </g>
        <!-- Párpados: del color del cuerpo, caen desde arriba. -->
        <g style="transform: scaleY({parpadosCerrados}); transform-origin: 32px 20px; transition: transform 0.08s ease;">
          <rect x="22.6" y="19.4" width="5.8" height="8.4" rx="2.6" fill={tintaCuerpo} />
          <rect x="35.6" y="19.4" width="5.8" height="8.4" rx="2.6" fill={tintaCuerpo} />
        </g>
        <!-- La boca: se abre con la amplitud REAL de la voz. -->
        <g style="transform: scaleY({0.28 + boca * 0.92}); transform-origin: 32px 29.5px;">
          <path
            d="M28 29 Q32 27.8 36 29 Q36.2 35.4 32.6 38.9 Q32 39.4 31.5 38.9 Q28 35.4 28 29 Z"
            fill="#f7f2e7"
          />
          {#if boca > 0.3}
            <path d="M29.4 34.4 Q32 36.6 34.6 34.4 L34.2 36.2 Q32 37.9 29.8 36.2 Z" fill={tintaCuerpo} opacity="0.55" />
          {/if}
        </g>
      </g>

      {#if estado === "dormido"}
        <g class="zeta" fill="#2f4bc4">
          <ellipse cx="50" cy="16" rx="2" ry="1.6" transform="rotate(-20 50 16)" />
          <rect x="51.4" y="8.6" width="1.4" height="7.6" rx="0.7" />
        </g>
      {/if}
    </g>

    {#if cantando && apertura === null}
      <!-- Las notas clásicas cuando canta sin envolvente (compatibilidad). -->
      <g class="nota nota-1" fill="#2f4bc4">
        <ellipse cx="52" cy="18" rx="2.2" ry="1.7" transform="rotate(-20 52 18)" />
        <rect x="53.6" y="9.5" width="1.5" height="8.8" rx="0.75" />
        <path d="M53.6 9.5 q3.4 0.6 3.8 3.4 q-1.9-1.4-3.8-1.4 Z" />
      </g>
      <g class="nota nota-2" fill="#e0502a">
        <ellipse cx="58" cy="26" rx="1.9" ry="1.5" transform="rotate(-20 58 26)" />
        <rect x="59.3" y="18.8" width="1.3" height="7.5" rx="0.65" />
      </g>
    {/if}
  </svg>
</span>

<style>
  .criatura {
    display: inline-block;
    position: relative;
  }
  .criatura svg {
    display: block;
    overflow: visible;
  }
  .cabeza,
  .cabeza g {
    transition: transform 0.14s ease;
  }
  /* Respiración de posado: sutil, viva. */
  .cuerpo-vivo {
    animation: respira 3.2s ease-in-out infinite;
    transform-origin: 32px 46px;
  }
  .duerme .cuerpo-vivo {
    animation: respira 5.2s ease-in-out infinite;
  }
  .habla .cuerpo-vivo {
    animation: none;
  }
  @keyframes respira {
    0%, 100% { transform: rotate(-3deg) scale(1); }
    50% { transform: rotate(-3deg) scale(1.016); }
  }
  .anda svg {
    animation: brinco 0.55s ease-in-out infinite alternate;
    transform-origin: 50% 100%;
  }
  .verguenza svg {
    animation: tiembla 0.28s ease-in-out 3;
  }
  @keyframes brinco {
    from { transform: scaleY(0.96) scaleX(1.02); }
    to { transform: scaleY(1.05) scaleX(0.98) translateY(-4px); }
  }
  @keyframes tiembla {
    0%, 100% { transform: translateX(0); }
    35% { transform: translateX(-1.6px) rotate(-1deg); }
    70% { transform: translateX(1.6px) rotate(1deg); }
  }
  .zeta {
    animation: flota 2.6s ease-in-out infinite;
  }
  @keyframes flota {
    0%, 100% { transform: translateY(0); opacity: 0.5; }
    50% { transform: translateY(-4px); opacity: 1; }
  }
  .nota {
    opacity: 0;
    animation: sube-nota 1.6s ease-out infinite;
  }
  .nota-2 {
    animation-delay: 0.55s;
  }
  @keyframes sube-nota {
    0% { transform: translateY(4px); opacity: 0; }
    25% { opacity: 1; }
    100% { transform: translateY(-10px); opacity: 0; }
  }
  @media (prefers-reduced-motion: reduce) {
    .cuerpo-vivo,
    .anda svg,
    .verguenza svg,
    .zeta,
    .nota {
      animation: none;
    }
  }
</style>
