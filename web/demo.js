// La demo sonora y el guionizador en vivo. Sin frameworks: la página debe
// pesar poco y decir la verdad (el audio es real, el WASM es el mismo Rust
// de la app). El mismo script sirve a la portada y a las páginas interiores:
// cada bloque comprueba si sus piezas existen antes de engancharse.

(function () {
  // ── El menú plegado de la cabecera: cerrarlo al tocar fuera ────────────
  const menu = document.querySelector('details.menu');
  if (menu) {
    document.addEventListener('click', (e) => {
      if (menu.open && !menu.contains(e.target)) menu.open = false;
    });
    menu.querySelectorAll('a').forEach((a) =>
      a.addEventListener('click', () => (menu.open = false)),
    );
  }

  // ── Muestras de audio (solo en las portadas) ───────────────────────────
  const frases = {
    es: 'En <mark>1492</mark>, tres carabelas zarparon de Palos. El <mark>siglo XV</mark> terminaba, y nadie a bordo sabía que llevaban el mapa del mundo por dibujar.',
    en: 'In <mark>1985</mark>, a letter arrived forty years late. <mark>Henry VIII</mark> never read it, but the <mark>21st century</mark> did.',
    fr: 'Le <mark>1er</mark> janvier, <mark>Louis XIV</mark> fit servir <mark>3,50 €</mark> de chocolat chaud. Le <mark>XIXe siècle</mark> en parlerait encore.',
    de: 'Am <mark>24. Mai</mark> wanderten wir <mark>7 km</mark> im <mark>19. Jahrhundert</mark>. Es war <mark>14:30 Uhr</mark>, und niemand hatte es eilig.',
    it: 'Il <mark>1º maggio</mark>, <mark>Luigi XIV</mark> assaggiò il gelato del <mark>XX secolo</mark>: <mark>3,50 €</mark> ben spesi.',
    pt: 'No <mark>século XIX</mark>, D. Pedro II leu <mark>1.234</mark> páginas num verão. Hoje seriam <mark>14</mark> minutos por dia.',
  };

  const audio = document.getElementById('demo-audio');
  const frase = document.getElementById('demo-frase');
  const play = document.getElementById('demo-play');
  const pills = document.querySelectorAll('.demo .idiomas button');

  if (audio && frase && play && pills.length) {
    // La página inglesa vive en /en/: el prefijo se deduce del src inicial
    // para que las rutas relativas sigan funcionando.
    const base = (audio.getAttribute('src') || '').replace(/audio\/demo-\w+\.m4a$/, '');

    const elegir = (lang) => {
      pills.forEach((b) => b.classList.toggle('activa', b.dataset.lang === lang));
      frase.innerHTML = frases[lang];
      audio.pause();
      audio.src = base + 'audio/demo-' + lang + '.m4a';
      play.textContent = '▶';
    };
    pills.forEach((b) => b.addEventListener('click', () => elegir(b.dataset.lang)));

    play.addEventListener('click', () => {
      if (audio.paused) {
        audio.play();
        play.textContent = '❚❚';
      } else {
        audio.pause();
        play.textContent = '▶';
      }
    });
    audio.addEventListener('ended', () => (play.textContent = '▶'));
  }

  // ── El guionizador (WASM) ──────────────────────────────────────────────
  const entrada = document.getElementById('guion-entrada');
  const salida = document.getElementById('guion-salida');
  const boton = document.getElementById('guion-boton');
  const selIdioma = document.getElementById('guion-idioma');

  if (entrada && salida && boton && selIdioma) {
    let guionizar = null;
    async function cargarWasm() {
      if (guionizar) return guionizar;
      // El sitio se sirve en la raíz del dominio: la ruta absoluta vale
      // desde la portada y desde cualquier carpeta.
      const mod = await import('/guion/yappy_wasm.js');
      await mod.default();
      guionizar = mod.guionizar;
      return guionizar;
    }

    function escapar(s) {
      return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
    }

    const etiquetaOcupado = boton.textContent;
    boton.addEventListener('click', async () => {
      const texto = entrada.value.trim();
      if (!texto) return;
      boton.disabled = true;
      boton.textContent = '…';
      try {
        const fn = await cargarWasm();
        const guion = JSON.parse(fn(texto, selIdioma.value));
        const html = guion.piezas
          .map((p) => {
            const spans = p.spans
              .map((s) =>
                s.clase === 'literal'
                  ? escapar(s.hablado)
                  : '<span class="cambio">' + escapar(s.hablado) + '</span>',
              )
              .join('');
            const pausa =
              p.pausa_antes_s > 0.05
                ? '<span class="pausa">[pausa ' + p.pausa_antes_s.toFixed(1).replace('.', ',') + ' s] </span>'
                : '';
            return '<p style="margin:0 0 10px">' + pausa + spans + '</p>';
          })
          .join('');
        salida.innerHTML = html || '<em>…</em>';
      } catch (e) {
        console.error(e);
        salida.innerHTML =
          '<span style="color: var(--tinta-suave)">' +
          (salida.dataset.fallo ||
            'No pude cargar el motor en este navegador. Un ejemplo de lo que hace: «El siglo XIX terminó en 1900» → «El <span class="cambio">siglo diecinueve</span> terminó en <span class="cambio">mil novecientos</span>».') +
          '</span>';
      } finally {
        boton.disabled = false;
        boton.textContent = etiquetaOcupado;
      }
    });
    // Autotest para verificación sin manos: /#autotest pulsa Guionizar solo.
    if (location.hash === '#autotest') {
      window.addEventListener('load', () => setTimeout(() => boton.click(), 400));
    }
  }
})();
