<script lang="ts">
  // Envoltorio de compatibilidad: la marca dentro de la app es el loro
  // (Criatura.svelte). Mantiene las props históricas (size / talking /
  // happy) para que los sitios de llamada viejos sigan funcionando.
  import Criatura from "./Criatura.svelte";

  let {
    size = 140,
    talking = false,
    happy = false,
  }: { size?: number; talking?: boolean; happy?: boolean } = $props();
</script>

<Criatura {size} andando={talking || happy} cantando={talking} />
