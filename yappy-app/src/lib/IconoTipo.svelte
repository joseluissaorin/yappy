<script lang="ts">
  // Iconos de tipo de item, dibujados a trazo (nada de emojis): artículo,
  // vídeo, audio, recorte, documento, enlace, portapapeles, carpeta.
  let {
    tipo = "documento",
    size = 20,
  }: {
    tipo?: "articulo" | "video" | "audio" | "recorte" | "documento" | "enlace" | "portapapeles";
    size?: number;
  } = $props();
</script>

<svg
  viewBox="0 0 24 24"
  width={size}
  height={size}
  fill="none"
  stroke="currentColor"
  stroke-width="1.9"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {#if tipo === "articulo"}
    <path d="M4 4h13a2 2 0 0 1 2 2v13a1.8 1.8 0 0 1-1.8 1.8H5.8A1.8 1.8 0 0 1 4 19V4z" />
    <path d="M8 8h7M8 12h7M8 16h4" />
  {:else if tipo === "video"}
    <rect x="3" y="5" width="18" height="14" rx="3" />
    <path d="m10 9 5 3-5 3z" fill="currentColor" stroke="none" />
  {:else if tipo === "audio"}
    <path d="M12 3a3 3 0 0 1 3 3v6a3 3 0 0 1-6 0V6a3 3 0 0 1 3-3z" />
    <path d="M5 11a7 7 0 0 0 14 0M12 18v3" />
  {:else if tipo === "recorte"}
    <circle cx="6" cy="6" r="2.6" />
    <circle cx="6" cy="18" r="2.6" />
    <path d="M8.2 7.8 20 19M8.2 16.2 20 5" />
  {:else if tipo === "enlace"}
    <path d="M10 13a5 5 0 0 0 7.5.5l2-2a5 5 0 0 0-7-7l-1.2 1.1" />
    <path d="M14 11a5 5 0 0 0-7.5-.5l-2 2a5 5 0 0 0 7 7l1.2-1.1" />
  {:else if tipo === "portapapeles"}
    <rect x="6" y="4" width="12" height="17" rx="2" />
    <path d="M9 4a2 2 0 0 1 6 0" />
    <path d="M9 10h6M9 14h6" />
  {:else}
    <path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z" />
    <path d="M14 3v5h5" />
  {/if}
</svg>
