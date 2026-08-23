<script lang="ts">
  // El loro de Yappy: un pájaro-huevo que repite en voz alta lo que le
  // llega. Dos tintas de imprenta con desregistro: la voz (coral) delante,
  // el texto (ultramar) detrás. Tintas fijas de marca, como en papel: no
  // cambian con el tema.
  //
  //   andando  → brinca en el sitio (está vivo)
  //   cantando → suelta dos notas mientras suena audio
  //   mirando  → -1 mira a la izquierda (para pasear hacia allá)
  let {
    size = 64,
    andando = false,
    cantando = false,
    mirando = 1,
  }: { size?: number; andando?: boolean; cantando?: boolean; mirando?: 1 | -1 } = $props();
</script>

<span
  class="criatura"
  class:anda={andando}
  style="width:{size}px;height:{size}px;transform:scaleX({mirando});"
  aria-hidden="true"
>
  <svg viewBox="0 0 64 64" width={size} height={size}>
    <g transform="rotate(-3 32 32)">
      <g fill="#2f4bc4" transform="translate(1.8 2)">
        <path
          d="M32 7 C42.5 6.5 50 14 50.5 24 C51 33 49.5 41.5 43 46.4 C39 49.4 25 49.4 21 46.4 C14.5 41.5 13 33 13.5 24 C14 14 21.5 6.5 32 7 Z"
        />
        <path d="M29.2 7.4 C26.8 1.8 32 -0.6 37.8 1.6 C34.6 3.1 33.7 5 34 7 Z" />
        <ellipse cx="23.5" cy="49.6" rx="5.6" ry="4.2" />
        <ellipse cx="40.5" cy="50.8" rx="5.6" ry="4.2" />
      </g>
      <g fill="#e0502a">
        <path
          d="M32 7 C42.5 6.5 50 14 50.5 24 C51 33 49.5 41.5 43 46.4 C39 49.4 25 49.4 21 46.4 C14.5 41.5 13 33 13.5 24 C14 14 21.5 6.5 32 7 Z"
        />
        <path d="M29.2 7.4 C26.8 1.8 32 -0.6 37.8 1.6 C34.6 3.1 33.7 5 34 7 Z" />
        <ellipse cx="23.5" cy="49.6" rx="5.6" ry="4.2" />
        <ellipse cx="40.5" cy="50.8" rx="5.6" ry="4.2" />
      </g>
      <circle cx="19.8" cy="29.5" r="2.9" fill="#f7f2e7" opacity=".55" />
      <circle cx="44.2" cy="29.5" r="2.9" fill="#f7f2e7" opacity=".55" />
      <g fill="#2b2418">
        <ellipse cx="25.5" cy="23.5" rx="2" ry="3.7" />
        <ellipse cx="38.5" cy="23.5" rx="2" ry="3.7" />
      </g>
      <path
        d="M28 29 Q32 27.8 36 29 Q36.2 35.4 32.6 38.9 Q32 39.4 31.5 38.9 Q28 35.4 28 29 Z"
        fill="#f7f2e7"
      />
    </g>
    {#if cantando}
      <!-- Las notas: salen junto al pico y suben. Dos corcheas a destiempo. -->
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
    line-height: 0;
    user-select: none;
    -webkit-user-select: none;
  }
  .criatura svg {
    overflow: visible;
  }
  .criatura.anda svg {
    animation: yap-brinco 0.55s ease-in-out infinite alternate;
    transform-origin: 50% 100%;
  }
  .nota {
    opacity: 0;
    animation: nota-sube 1.6s ease-out infinite;
  }
  .nota-2 {
    animation-delay: 0.7s;
  }
  @keyframes nota-sube {
    0% {
      opacity: 0;
      transform: translate(0, 4px) rotate(0deg);
    }
    20% {
      opacity: 1;
    }
    80% {
      opacity: 0.9;
    }
    100% {
      opacity: 0;
      transform: translate(5px, -11px) rotate(10deg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .criatura.anda svg,
    .nota {
      animation: none;
    }
    .nota {
      opacity: 0.8;
    }
  }
</style>
