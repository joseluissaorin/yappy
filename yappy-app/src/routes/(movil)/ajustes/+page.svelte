<script lang="ts">
  // La ruta /ajustes pervive para los deep links (emparejar, atajos);
  // el contenido REAL vive en el componente Trastienda, que también
  // monta el cajón deslizante de la cinta.
  import Trastienda from "$lib/Trastienda.svelte";
</script>

<Trastienda />
