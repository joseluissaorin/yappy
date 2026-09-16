<script lang="ts">
  // El guionizador, en vivo. Carga perezosa del WASM: 1,6 MB que solo se
  // descargan si alguien pulsa el botón, y una sola vez. Es el mismo Rust
  // que va dentro de la app, compilado para el navegador; por eso la frase
  // «corre el mismo código» de la página es verdad y no publicidad.
  let {
    lang = "es",
    ejemplo = "",
    marcador = "",
    boton = "Guionizar",
    espera = "",
    fallo = "",
    idiomas = [] as { codigo: string; nombre: string }[],
  } = $props();

  let texto = $state(ejemplo);
  let lengua = $state(lang);
  let salida = $state("");
  let trabajando = $state(false);
  let error = $state("");
  let guionizar: ((t: string, l: string) => string) | null = null;

  const escapar = (s: string) =>
    s.replace(/[<>&"]/g, (c) => ({ "<": "&lt;", ">": "&gt;", "&": "&amp;", '"': "&quot;" })[c]!);

  async function correr() {
    if (trabajando) return;
    trabajando = true;
    error = "";
    try {
      if (guionizar == null) {
        // La ruta va en una variable a propósito: si es literal, Rollup
        // intenta resolverla en compilación y no puede, porque el WASM no
        // es un módulo del proyecto sino un fichero servido tal cual.
        const ruta = "/guion/yappy_wasm.js";
        const mod = await import(/* @vite-ignore */ ruta);
        await mod.default();
        guionizar = mod.guionizar;
      }
      const guion = JSON.parse(guionizar!(texto, lengua));
      salida = (guion.piezas ?? [])
        .map((pieza: any) => {
          const pausa = pieza.pausa_antes_s > 0.05
            ? `<span class="pausa">[${String(pieza.pausa_antes_s.toFixed(1)).replace(".", ",")} s] </span>`
            : "";
          const cuerpo = (pieza.spans ?? [])
            .map((s: any) =>
              s.clase === "literal" ? escapar(s.hablado) : `<span class="cambio">${escapar(s.hablado)}</span>`,
            )
            .join("");
          return pausa + cuerpo;
        })
        .join("<br />");
    } catch (e) {
      error = fallo;
    } finally {
      trabajando = false;
    }
  }
</script>

<label class="vh" for="guion-entrada">{marcador}</label>
<textarea
  id="guion-entrada"
  class="guion-campo"
  bind:value={texto}
  placeholder={marcador}
  rows="3"
  spellcheck="false"
></textarea>

<div class="guion-barra">
  <button class="tecla" type="button" onclick={correr} disabled={trabajando}>{boton}</button>
  <label class="vh" for="guion-idioma">{lengua}</label>
  <select id="guion-idioma" class="guion-campo" style="width:auto;min-height:0;padding:10px 12px" bind:value={lengua}>
    {#each idiomas as i (i.codigo)}
      <option value={i.codigo}>{i.nombre}</option>
    {/each}
  </select>
</div>

<div class="guion-salida" aria-live="polite">
  {#if error}
    <p style="margin:0;color:var(--tinta-suave)">{error}</p>
  {:else if salida}
    {@html salida}
  {:else}
    <p style="margin:0;color:var(--tinta-suave)">{espera}</p>
  {/if}
</div>
