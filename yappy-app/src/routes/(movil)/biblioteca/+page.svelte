<script lang="ts">
  // La biblioteca vive ahora DENTRO de la cinta: pasado, presente y futuro
  // son un solo eje. Esta ruta queda como redirección para enlaces viejos
  // (widgets, Spotlight, marcadores). El estante de audiolibros sigue en
  // /biblioteca/audiolibros porque es el reproductor de .m4b.
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  onMount(() => goto("/escuchar", { replaceState: true }));
</script>
