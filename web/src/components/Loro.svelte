<script lang="ts">
  // El loro de Yappy en la web. Puerto de yappy-app/src/lib/Criatura.svelte:
  // la misma anatomía, el mismo pico ganchudo en dos piezas (el gancho
  // superior no se deforma jamás: hablar es dejar caer la mandíbula
  // inferior y echar la cabeza un punto atrás), el mismo anillo ocular, la
  // misma ala festoneada y las mismas tres plumas de cola.
  //
  // Sin directiva de cliente sale como SVG estático, sin una línea de
  // JavaScript. Con `client:visible` cobra vida: parpadea, respira, se
  // acicala si lo abandonas y sigue el ratón con la pupila.
  import { onMount, onDestroy } from "svelte";

  let {
    size = 64,
    estado = "posado",
    apertura = null,
    tinta = null,
    andando = false,
    cantando = false,
    mirando = 1,
    sigueAlRaton = false,
    etiqueta = "",
  }: {
    size?: number;
    estado?: "posado" | "hablando" | "pausa" | "dormido" | "celebrando";
    apertura?: number | null;
    tinta?: string | null;
    andando?: boolean;
    cantando?: boolean;
    mirando?: 1 | -1;
    sigueAlRaton?: boolean;
    etiqueta?: string;
  } = $props();

  // Cantar sin envolvente real: aleteo interno del pico.
  let aleteo = $state(0);
  let aleteoTimer: ReturnType<typeof setInterval> | undefined;
  $effect(() => {
    const habla = (estado === "hablando" || cantando) && apertura === null;
    clearInterval(aleteoTimer);
    if (habla) {
      aleteoTimer = setInterval(() => { aleteo = 0.18 + Math.random() * 0.72; }, 90 + Math.random() * 60);
    } else {
      aleteo = 0;
    }
    return () => clearInterval(aleteoTimer);
  });
  const boca = $derived(estado === "dormido" ? 0 : Math.max(0, Math.min(1, apertura ?? aleteo)));

  // Parpadeo y acicalado con vida propia.
  let parpadeo = $state(false);
  let acicalando = $state(false);
  let ojo = $state({ x: 0, y: 0 });
  let timers: ReturnType<typeof setTimeout>[] = [];
  let raiz: HTMLElement | undefined = $state();

  function programarParpadeo() {
    const t = setTimeout(() => {
      parpadeo = true;
      const t2 = setTimeout(() => {
        parpadeo = false;
        // Doble parpadeo de vez en cuando: los pájaros lo hacen.
        if (Math.random() < 0.18) {
          const t3 = setTimeout(() => {
            parpadeo = true;
            timers.push(setTimeout(() => (parpadeo = false), 90));
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
        timers.push(setTimeout(() => { acicalando = false; programarAcicalado(); }, 1150));
      } else {
        programarAcicalado();
      }
    }, 14000 + Math.random() * 14000);
    timers.push(t);
  }

  function alMover(e: MouseEvent) {
    if (raiz == null) return;
    const c = raiz.getBoundingClientRect();
    const dx = (e.clientX - (c.left + c.width / 2)) / Math.max(260, c.width * 4);
    const dy = (e.clientY - (c.top + c.height / 2)) / Math.max(220, c.height * 4);
    ojo = { x: Math.max(-1, Math.min(1, dx)), y: Math.max(-1, Math.min(1, dy)) };
  }

  onMount(() => {
    const quieto = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (quieto) return;
    programarParpadeo();
    programarAcicalado();
    if (sigueAlRaton && window.matchMedia("(pointer: fine)").matches) {
      window.addEventListener("mousemove", alMover, { passive: true });
    }
  });
  onDestroy(() => {
    timers.forEach(clearTimeout);
    clearInterval(aleteoTimer);
    if (typeof window !== "undefined") window.removeEventListener("mousemove", alMover);
  });

  // El dibujo base mira a la IZQUIERDA. Hablar echa la cabeza atrás; la
  // pausa y el sueño la bajan; acicalarse la hunde hacia el ala.
  const cabezaGiro = $derived(
    estado === "dormido" ? 12 : estado === "pausa" ? 7 : acicalando ? 30 : -boca * 5,
  );
  const ojoX = $derived(ojo.x * 1.8 * mirando);
  const ojoY = $derived(ojo.y * 1.5);
  const parpadoCerrado = $derived(estado === "dormido" ? 0.94 : parpadeo ? 1 : 0);
  const cuerpo = $derived(tinta ?? "#e0502a");
</script>

<span
  bind:this={raiz}
  class="loro"
  class:anda={andando || estado === "celebrando"}
  class:habla={boca > 0.04}
  class:duerme={estado === "dormido"}
  class:celebra={estado === "celebrando"}
  style="width:{size}px;height:{size}px;transform:scaleX({mirando});"
  role={etiqueta ? "img" : undefined}
  aria-label={etiqueta || undefined}
  aria-hidden={etiqueta ? undefined : "true"}
>
  <svg viewBox="0 0 64 64" width={size} height={size}>
    <g class="cuerpo-vivo">
      <!-- La sombra ultramar: el desregistro de imprenta de la casa. -->
      <g fill="#2f4bc4" opacity="0.9" transform="translate(1.8 2)">
        <path d="M41 40 C48 44 54 50 57 56 C51 53 45 49 41 45 Z" />
        <path d="M42.5 37.5 C51 40 58 44 61.5 49 C55 48 48 45 43.5 41.5 Z" />
        <path d="M30 5 C 22.5 5 17.5 9 16 14 C 11 14.5 7.5 17.5 7.5 21.5 C 7.5 25.5 10 28.5 13.5 30 C 12.5 27 13.5 24.5 16 23.5 C 17 24.5 18.5 25.3 20 25.6 C 18.3 28.4 17.2 31.6 17.2 35 C 17.2 43.5 22.5 50.5 30.5 51 C 38.5 51.5 44.5 45 45.3 36 C 45.8 31 45.3 26 44.3 22 C 42.8 12 37.5 5 30 5 Z" />
      </g>

      <!-- La cola: tres plumas, de la más oscura a la más clara. -->
      <g class="cola" style="transform-origin: 41px 40px;">
        <path d="M41 40 C48 44 54 50 57 56 C51 53 45 49 41 45 Z" fill="color-mix(in srgb, {cuerpo} 62%, #2b2418)" />
        <path d="M42.5 37.5 C51 40 58 44 61.5 49 C55 48 48 45 43.5 41.5 Z" fill="color-mix(in srgb, {cuerpo} 80%, #2b2418)" />
        <path d="M43 34.5 C52 35 60 38 63.5 42 C57 42 49.5 40.5 44 38 Z" fill={cuerpo} />
      </g>

      <!-- El cuerpo: huevo rechoncho, con la barriga crema delante. -->
      <path d="M30 8 C 21 8.5 16.8 16 16.8 26 C 16.8 27 16.9 28 17 29 C 17.2 40.5 22.5 50.5 30.5 51 C 38.5 51.5 44.5 45 45.3 36 C 45.8 31 45.3 26 44.3 22 C 42.8 12 37.5 8 30 8 Z" fill={cuerpo} />
      <path d="M23.5 31 C 20.5 34.5 20 40 22 44.5 C 23.8 48.5 27 50.8 30.5 51 C 34 51 36.5 49 38 46 C 34 46.5 29.5 44.5 27 40.5 C 25 37.5 24 34 23.5 31 Z" fill="#f7f2e7" opacity="0.92" />

      <!-- El ala festoneada, un punto más oscura que el cuerpo. -->
      <g class="ala" style="transform-origin: 36px 25px;">
        <path d="M35 22 C 41 23 44.5 28 44.5 34 C 44.5 39 42 43.5 38.5 45.5 C 39 43 37.5 41.5 35.5 41.5 C 36.5 39.5 36 37.5 34 37 C 35 35 34.5 33 32.5 32.5 C 33.5 29 33.5 25 35 22 Z" fill="color-mix(in srgb, {cuerpo} 74%, #2b2418)" />
      </g>

      <!-- Las patas: dos dedos, tinta de imprenta. -->
      <g stroke="#2b2418" stroke-width="2.6" fill="none" stroke-linecap="round">
        <path d="M26.5 50.5 L26.5 54.5 L23.5 57.5 M26.5 54.5 L29.5 57.5" />
        <path d="M34.5 50.5 L34.5 54.5 L31.5 57.5 M34.5 54.5 L37.5 57.5" />
      </g>

      <!-- LA CABEZA: gira entera desde el cuello; el gancho no se deforma. -->
      <g class="cabeza" style="transform: rotate({cabezaGiro}deg); transform-origin: 29px 26px;">
        <path d="M30 5 C 22.5 5 17.5 9 16 14 C 15.2 16.8 15.2 20.5 16.2 23.5 C 17.5 27.5 20.5 30 24.5 30.5 C 30 31 36 30 40 27 C 43 24.5 44 19 42.5 14.5 C 40.5 8.5 36 5 30 5 Z" fill={cuerpo} />
        <g class="copete">
          <path d="M28 5.5 C 27 2 29.5 -0.5 33 0.5 C 31 2 30.5 4 31 5.5 Z" fill={cuerpo} />
          <path d="M32.5 5.2 C 32.8 2.4 35.5 1 38 2.2 C 36.2 3.2 35.4 4.6 35.4 6.2 Z" fill="color-mix(in srgb, {cuerpo} 74%, #2b2418)" />
        </g>
        <!-- El interior de la boca: crema, solo se ve al abrir. -->
        <path d="M11 21 C 13 20 17 20.5 19.5 22.5 L 18.5 27.5 C 15 27 12 25 11 21 Z" fill="#f7f2e7" />
        <!-- Mandíbula inferior: la que CAE al hablar. -->
        <g class="mandibula" style="transform: rotate({boca * 25}deg); transform-origin: 19px 24.5px;">
          <path d="M11.5 24.5 C 12.5 27.5 15.5 29.5 19 29.8 L 18.2 25.6 C 15.8 25.4 13.5 25 11.5 24.5 Z" fill="#2b2418" />
        </g>
        <!-- Mandíbula superior: EL GANCHO. Sube un pelín, nunca se deforma. -->
        <g style="transform: rotate({-boca * 7}deg); transform-origin: 17px 16px;">
          <path d="M16 13.5 C 11 14 7.5 17.5 7.5 21.5 C 7.5 25.5 10 28.5 13.5 30 C 12.5 27 13.5 24.5 16 23.5 C 17.5 24.8 19.5 25.6 21.5 25.8 C 22.5 22 21.5 17 19 14.5 C 18 13.8 17 13.5 16 13.5 Z" fill="#2b2418" />
        </g>
        <!-- El ojo: anillo crema, pupila que te sigue, párpado del cuerpo. -->
        <circle cx="27" cy="18" r="5.1" fill="#f7f2e7" />
        <circle cx={27 + ojoX} cy={18 + ojoY} r="2.5" fill="#2b2418" />
        <g style="transform: scaleY({parpadoCerrado}); transform-origin: 27px 13.4px; transition: transform 0.08s ease;">
          <rect x="21.6" y="13.2" width="10.8" height="9.6" rx="4.6" fill={cuerpo} />
        </g>
        <circle cx="21" cy="26.5" r="2.4" fill="#f7f2e7" opacity="0.4" />
      </g>

      {#if estado === "dormido"}
        <g class="zeta" fill="#2f4bc4">
          <ellipse cx="49" cy="14" rx="2" ry="1.6" transform="rotate(-20 49 14)" />
          <rect x="50.4" y="6.6" width="1.4" height="7.6" rx="0.7" />
        </g>
      {/if}
    </g>
  </svg>
</span>

<style>
  .loro { display: inline-block; position: relative; }
  .loro svg { display: block; overflow: visible; }
  .cabeza, .mandibula { transition: transform 0.12s ease; }
  /* Respiración de posado: sutil, viva. */
  .cuerpo-vivo { animation: respira 3.2s ease-in-out infinite; transform-origin: 30px 46px; }
  .duerme .cuerpo-vivo { animation: respira 5.2s ease-in-out infinite; }
  .habla .cuerpo-vivo { animation: none; }
  @keyframes respira {
    0%, 100% { transform: scale(1); }
    50% { transform: scale(1.016); }
  }
  .anda svg { animation: brinco 0.55s ease-in-out infinite alternate; transform-origin: 50% 100%; }
  .celebra .ala { animation: aletea 0.32s ease-in-out infinite alternate; }
  .celebra .cola { animation: colea 0.4s ease-in-out infinite alternate; }
  @keyframes aletea { from { transform: rotate(0deg); } to { transform: rotate(-24deg) translateY(-1.5px); } }
  @keyframes colea { from { transform: rotate(-4deg); } to { transform: rotate(5deg); } }
  @keyframes brinco {
    from { transform: scaleY(0.96) scaleX(1.02); }
    to { transform: scaleY(1.05) scaleX(0.98) translateY(-4px); }
  }
  .zeta { animation: flota 2.6s ease-in-out infinite; }
  @keyframes flota {
    0%, 100% { transform: translateY(0); opacity: 0.5; }
    50% { transform: translateY(-4px); opacity: 1; }
  }
  @media (prefers-reduced-motion: reduce) {
    .cuerpo-vivo, .anda svg, .celebra .ala, .celebra .cola, .zeta { animation: none; }
  }
</style>
