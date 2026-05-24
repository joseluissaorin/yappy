<script lang="ts">
  let {
    size = 140,
    talking = false,
    happy = false,
  }: { size?: number; talking?: boolean; happy?: boolean } = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 200 200"
  fill="none"
  xmlns="http://www.w3.org/2000/svg"
  class="yappy-mascot"
  class:talking
  class:happy
>
  <!-- subtle pink blob behind -->
  <ellipse class="bg-blob" cx="100" cy="115" rx="78" ry="62" fill="#FFD3E0" opacity="0.7" />

  <!-- speech bubble / dog head shape -->
  <path
    d="M40 90 C40 50 70 30 100 30 C130 30 160 50 160 90 C160 115 145 135 120 144 L130 170 L92 144 L80 144 C58 144 40 122 40 100 Z"
    fill="#FFFFFF"
    stroke="#1A1A1A"
    stroke-width="3.5"
    stroke-linejoin="round"
  />

  <!-- left ear (floppy) -->
  <path
    class="ear ear-left"
    d="M52 50 C42 40 28 44 26 60 C24 76 38 88 52 84"
    fill="#FFB3C8"
    stroke="#1A1A1A"
    stroke-width="3.5"
    stroke-linejoin="round"
    stroke-linecap="round"
  />

  <!-- right ear -->
  <path
    class="ear ear-right"
    d="M148 50 C158 38 174 42 176 60 C178 76 162 88 148 84"
    fill="#FFB3C8"
    stroke="#1A1A1A"
    stroke-width="3.5"
    stroke-linejoin="round"
    stroke-linecap="round"
  />

  <!-- eyes -->
  <g class="eyes">
    <ellipse class="eye eye-l" cx="78" cy="86" rx="6" ry="8" fill="#1A1A1A" />
    <ellipse class="eye eye-r" cx="122" cy="86" rx="6" ry="8" fill="#1A1A1A" />
    <!-- eye sparkles -->
    <circle cx="76" cy="83" r="2" fill="#FFFFFF" />
    <circle cx="120" cy="83" r="2" fill="#FFFFFF" />
  </g>

  <!-- blush -->
  <ellipse cx="65" cy="106" rx="9" ry="5" fill="#FFB3C8" opacity="0.75" />
  <ellipse cx="135" cy="106" rx="9" ry="5" fill="#FFB3C8" opacity="0.75" />

  <!-- nose -->
  <ellipse cx="100" cy="104" rx="5" ry="4" fill="#1A1A1A" />

  <!-- mouth (animates open/closed when talking) -->
  <g class="mouth-group">
    <path
      class="mouth-static"
      d="M88 116 C92 124 108 124 112 116"
      stroke="#1A1A1A"
      stroke-width="3"
      stroke-linecap="round"
      fill="none"
    />
    <path
      class="mouth-open"
      d="M93 117 C95 128 105 128 107 117 L107 127 C105 130 95 130 93 127 Z"
      fill="#FF80AB"
      stroke="#1A1A1A"
      stroke-width="2"
      stroke-linejoin="round"
    />
    <ellipse class="tongue" cx="100" cy="128" rx="3.5" ry="2" fill="#FF5D97" />
  </g>

  <!-- yap waves (only visible when talking) -->
  <g class="waves" stroke="#1A1A1A" stroke-width="2.5" stroke-linecap="round" fill="none">
    <path class="wave w1" d="M170 90 Q180 90 184 95" />
    <path class="wave w2" d="M170 100 Q186 100 192 105" />
    <path class="wave w3" d="M170 110 Q180 112 184 117" />
  </g>
</svg>

<style>
  .yappy-mascot {
    display: block;
  }
  /* Idle: gentle hover bob */
  .yappy-mascot:hover {
    animation: bob 0.6s ease-in-out;
  }
  @keyframes bob {
    0%, 100% { transform: translateY(0) rotate(0deg); }
    25% { transform: translateY(-6px) rotate(-3deg); }
    75% { transform: translateY(-2px) rotate(3deg); }
  }
  /* Talking */
  .yappy-mascot.talking {
    animation: idle-bounce 2.4s ease-in-out infinite;
  }
  @keyframes idle-bounce {
    0%, 100% { transform: translateY(0); }
    50% { transform: translateY(-3px); }
  }
  .yappy-mascot .mouth-static { opacity: 1; }
  .yappy-mascot .mouth-open, .yappy-mascot .tongue { opacity: 0; }
  .yappy-mascot.talking .mouth-static { opacity: 0; animation: none; }
  .yappy-mascot.talking .mouth-open {
    opacity: 1;
    transform-origin: 100px 122px;
    animation: yap 0.36s ease-in-out infinite alternate;
  }
  .yappy-mascot.talking .tongue {
    opacity: 1;
    transform-origin: 100px 126px;
    animation: yap-tongue 0.36s ease-in-out infinite alternate;
  }
  @keyframes yap {
    from { transform: scaleY(0.4); }
    to { transform: scaleY(1.2); }
  }
  @keyframes yap-tongue {
    from { transform: scaleY(0.5); }
    to { transform: scaleY(1); }
  }
  .yappy-mascot .wave { opacity: 0; transition: opacity 0.2s ease; }
  .yappy-mascot.talking .w1 { animation: wave-pulse 0.6s ease-out infinite; animation-delay: 0s; }
  .yappy-mascot.talking .w2 { animation: wave-pulse 0.6s ease-out infinite; animation-delay: 0.2s; }
  .yappy-mascot.talking .w3 { animation: wave-pulse 0.6s ease-out infinite; animation-delay: 0.4s; }
  @keyframes wave-pulse {
    0% { opacity: 0; transform: translateX(-6px); }
    50% { opacity: 1; transform: translateX(0); }
    100% { opacity: 0; transform: translateX(8px); }
  }
  .yappy-mascot .ear { transform-origin: 100px 60px; transition: transform 0.2s ease; }
  .yappy-mascot.talking .ear-left { animation: ear-wiggle-l 1.2s ease-in-out infinite; }
  .yappy-mascot.talking .ear-right { animation: ear-wiggle-r 1.2s ease-in-out infinite 0.6s; }
  @keyframes ear-wiggle-l {
    0%, 100% { transform: rotate(0deg); }
    50% { transform: rotate(-6deg); }
  }
  @keyframes ear-wiggle-r {
    0%, 100% { transform: rotate(0deg); }
    50% { transform: rotate(6deg); }
  }
  /* Happy / blinking eyes when celebrating */
  .yappy-mascot.happy .eye { animation: happy-blink 1.4s ease-in-out 3; }
  @keyframes happy-blink {
    0%, 100% { transform: scaleY(1); }
    40%, 60% { transform: scaleY(0.1); }
  }
</style>
