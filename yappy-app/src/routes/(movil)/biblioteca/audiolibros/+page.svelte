<script lang="ts">
  // No hay biblioteca en el móvil: hay percha. Los audiolibros son
  // pegatinas perforadas de la cinta y la imprenta es una hoja sobre ella.
  // Esta ruta pervive para los enlaces viejos (widgets, Spotlight) y abre
  // la percha con la hoja de la imprenta.
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  onMount(() => goto("/escuchar?imprenta=1", { replaceState: true }));
</script>
