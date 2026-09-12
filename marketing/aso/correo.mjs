// Genera el correo HTML del informe ASO: node correo.mjs > correo.html
import { readFileSync, existsSync } from "node:fs";
const P = JSON.parse(readFileSync("propuesta.json", "utf8"));
const A = JSON.parse(readFileSync("anterior.json", "utf8"));
const E = existsSync("estado.json") ? JSON.parse(readFileSync("estado.json", "utf8")) : {};
const esc = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
// Números de Astro: popularidad/dificultad. Los términos en negrita son los que ahora cubrimos.
const D = {
 "en-US": ["Estados Unidos", "text to speech 60/62 · tts 41/56 · text to speech reader 28/48 · document reader 37/47 · accessibility 23/13 · pdf to audio 20/40 · voice reader 16/52 · read to me 17/63 · dyslexia 13/23", "El título ya tenía «text to speech reader». El subtítulo pasa a «voice reader» con PDF y EPUB; las palabras clave añaden document, accessibility, dyslexia, read to me y audio."],
 "en-GB": ["Reino Unido (y ~100 países sin idioma propio)", "text to speech 54/44 · read aloud 29/39 · tts 23/23 · voice reader 16/48 · read to me 16/40", "Subtítulo «Read Aloud PDF, EPUB & Docs»: aquí «read aloud» sí tiene demanda. Palabras clave distintas de en-US para sumar en la tienda de EE. UU."],
 "en-AU": ["Australia", "text to speech 50/23 · voice reader 16/19 · read to me 16/44 · read out loud 9/21", "Subtítulo «Text to Audio: Read Out Loud». Palabras clave propias, que EE. UU. también indexa."],
 "en-CA": ["Canadá", "text to speech 51/42 · read aloud 36/41 · read out loud 25/37 · dyslexia 20/11 · voice reader 19/43", "Subtítulo «Read Out Loud: PDF, EPUB, Docs»; dyslexia en palabras clave (20/11 es de lo mejor de Canadá)."],
 "es-ES": ["España", "lector de textos 44/40 · texto a voz 43/39 · epub 50/41 · text to speech 32/37 · leer texto 15/43 · narrador 11/23 · leer en voz alta 5", "Título «lector de textos»; subtítulo «Texto a voz para PDF y EPUB»; leer, narrador y audiolibro en palabras clave. «text to speech» llega por en-GB, que España indexa."],
 "es-MX": ["México (e hispanohablantes de EE. UU.)", "lector de textos 45/17 · texto a voz 44/44 · text to speech 47/17 · leer pdf 17/52 · epub 49/40", "Igual que España: «lector de textos» con dificultad 17 es la mejor oportunidad del mundo hispano."],
 "de-DE": ["Alemania", "text to speech 52/42 · text vorlesen app 46/37 · vorlese app 23/21 · stimme 22/45 · text zu sprache 12/38 · epub 33/40", "Título «Text vorlesen App» (antes faltaba «App»); subtítulo con «Text to Speech»; vorlese, sprache, stimme y sprachausgabe en palabras clave."],
 "fr-FR": ["Francia", "lecteur de texte 44/41 · text to speech 35/38 · epub 45/43 · voix 24/50 · lecture à voix haute 5", "Título «lecteur de texte»; subtítulo con «Text to Speech»; synthèse vocale, voix y lecture en palabras clave."],
 "fr-CA": ["Canadá (francés)", "text to speech 51/42 (vía en-CA) · synthèse vocale 6/11 · lecteur de texte 5/40", "Misma ficha que Francia; en Canadá el volumen lo trae el inglés."],
 "it": ["Italia", "text to speech 47/38 · epub 44/43 · voce 21/54 · lettura 18/63 · lettore di testi 8/38 · legge ad alta voce 5", "Título «Text to Speech, lettore»; testi, PDF y EPUB en el subtítulo; voce, lettura y sintesi vocale en palabras clave."],
 "pt-PT": ["Portugal", "text to speech 25/11 · epub 31/35 · audiobooks 53/45 · lê em voz alta 5", "Título «leitor Text to Speech»; subtítulo con texto, PDF, EPUB y livros."],
 "pt-BR": ["Brasil", "ler texto 41/23 · leitor de livros 43/50 · ler livros 43/57 · epub 51/45 · artigos 14/15 · narrador 10/37", "Título «ler texto em voz alta»; subtítulo «Leitor de livros, PDF e EPUB»; narrador, artigo y dislexia en palabras clave."],
 "nl-NL": ["Países Bajos", "text to speech 35/15 · tts 30/17 · epub 32/43 · voorlezen 5", "Título «Text to Speech (TTS)»; «voorlezen» y EPUB en el subtítulo."],
 "pl": ["Polonia", "tts 34/13 · text to speech 30/17 · epub 38/39 · czyta na głos 5", "Título «Text to Speech i lektor»; subtítulo empieza por «TTS»."],
 "ro": ["Rumanía", "text to speech 33/11 · audiobook 29/43 · citește cu voce tare 5", "Título «Text to Speech, cititor»; audiobook en palabras clave."],
 "sv": ["Suecia", "text to speech 36/21 · epub 30/41 · läser högt 5", "Título «Text to Speech & röst»; «läser upp» y EPUB en el subtítulo."],
 "da": ["Dinamarca", "text to speech 30/11 · epub 24/17 · læser højt 5", "Título «Text to Speech, læs op»; EPUB en el subtítulo."],
 "fi": ["Finlandia", "text to speech 36/9 · epub 26/15 · lukee ääneen 5", "Título «Text to Speech, lukija»; epub en palabras clave."],
 "hr": ["Croacia", "text to speech 19/7 · čita naglas 5", "Título «Text to Speech, čitač»; EPUB en el subtítulo."],
 "sk": ["Eslovaquia", "text to speech 19/7 · epub 29/15", "Título «Text to Speech, čítačka»; EPUB en el subtítulo."],
 "cs": ["Chequia", "text to speech 32/9 · epub 36/23 · čte nahlas 5", "Título «Text to Speech, čtečka»; EPUB en el subtítulo."],
 "hu": ["Hungría", "text to speech 28/9 · epub 41/19 · hangoskönyv 58/19", "Título «Text to Speech, olvasó»; epub y hangoskönyv en palabras clave."],
 "el": ["Grecia", "text to speech 31/9 · pdf reader 48/40 · audiobook 24/23", "Título «Text to Speech & φωνή»; reader y audiobook en palabras clave para «pdf reader»."],
 "uk": ["Ucrania", "читалка 60/49 · читання 50/38 · text to speech 29/15 · epub 36/38 · читає вголос 5", "Título «читалка, Text to Speech»; читання y озвучка en palabras clave."],
 "ru": ["Rusia", "текст в речь 21/21 · text to speech 22/15 · голос 32/39 · статьи 24/21 · читает вслух 5", "Título «текст в речь, читалка»; subtítulo con «Text to Speech»; голос y статья en palabras clave."],
 "tr": ["Turquía", "pdf okuyucu 52/38 · kitap okuma 50/45 · epub 41/23 · text to speech 25/17 · tts 21/23", "Título «Text to Speech, okuyucu»; subtítulo con kitap, PDF y okuma, que forman «pdf okuyucu» y «kitap okuma»."],
 "ar-SA": ["Arabia Saudí", "text to speech 41/15 · صوت 35/53 · قارئ pdf 18/23 · يقرأ لك بصوت عالٍ 5", "Título «قارئ Text to Speech»; PDF en el subtítulo para «قارئ pdf»."],
 "hi": ["India", "text to speech 47/45 · pdf reader 56/47 · voice 42/41 · hindi text to speech 5", "En India se busca en inglés: título «Text to Speech Reader» con subtítulo en hindi."],
 "id": ["Indonesia", "tts 59/46 · text to speech 46/17 · pembaca pdf 44/21 · baca buku 43/38", "Título «Text to Speech, pembaca»; subtítulo «Baca buku, PDF…» para «pembaca pdf» y «baca buku»."],
 "vi": ["Vietnam", "text to speech 64/42 · đọc truyện 59/57 · sách nói 54/57 · đọc báo 42/48", "Título «Text to Speech, đọc báo»; subtítulo con đọc truyện y sách."],
 "ko": ["Corea del Sur", "tts 59/19 · 텍스트 음성 변환 31/41 · 오디오북 59/54 · 독서 65/47", "Título «TTS 텍스트 음성 변환»: las dos búsquedas líderes juntas."],
 "ja": ["Japón", "読み上げアプリ 54/45 · text to speech 51/15 · 読み上げ 41/47 · 音声読み上げ 41/45 · 音読 28/46 · 朗読 22/45", "Título «音声読み上げアプリ・TTS», que contiene 読み上げアプリ, 音声読み上げ y 読み上げ; 朗読 y 音読 en el subtítulo; text y speech en palabras clave."],
};
const orden = ["en-US","en-GB","en-AU","en-CA","es-ES","es-MX","de-DE","fr-FR","fr-CA","it","pt-PT","pt-BR","nl-NL","pl","ro","sv","da","fi","hr","sk","cs","hu","el","uk","ru","tr","ar-SA","hi","id","vi","ko","ja"];
const etiqueta = {"es-ES":"TEXTO A VOZ","es-MX":"TEXTO A VOZ","fr-FR":"LECTEUR DE TEXTE","fr-CA":"LECTEUR DE TEXTE","pt-PT":"LER TEXTO · TEXT TO SPEECH","pt-BR":"LER TEXTO · TEXT TO SPEECH","ru":"ТЕКСТ В РЕЧЬ","ko":"TTS · 텍스트 음성 변환","ja":"読み上げアプリ"};
const td = 'style="border-bottom:1px solid #e6dcc6;padding:10px 8px;vertical-align:top;font-size:13px;line-height:1.45;word-break:break-word"';
const filas = orden.map((l) => {
  const [tienda, datos, como] = D[l]; const [n, s, k] = P[l]; const [an, as] = A[l];
  return `<tr><td ${td}><b>${esc(tienda)}</b><br><span style="color:#82755a">${l}</span></td>
<td ${td}>${esc(datos)}</td>
<td ${td}><span style="color:#82755a">Antes:</span> ${esc(an)}<br><span style="color:#82755a">${esc(as)}</span><br><br><b>Ahora:</b> ${esc(n)}<br>${esc(s)}<br><span style="font-family:Menlo,monospace;font-size:11px;color:#82755a">${esc(k.split(",").join(", "))}</span></td>
<td ${td}>${esc(como)}<br><span style="color:#82755a">Captura 1: «${esc(etiqueta[l] ?? "TEXT TO SPEECH")}»</span></td></tr>`;
}).join("\n");
const p = (t) => `<p style="font-size:15px;line-height:1.6;margin:0 0 14px">${t}</p>`;
console.log(`<div style="background:#f7f2e7;padding:24px;font-family:Georgia,serif;color:#2b2418">
<div style="max-width:980px;margin:0 auto;background:#fffdf7;border:1px solid #e6dcc6;border-radius:16px;padding:28px">
<h1 style="font-size:24px;margin:0 0 6px">Yappy: las 32 tiendas, reoptimizadas con los datos de Astro</h1>
<p style="color:#82755a;margin:0 0 22px">Informe del 12 de septiembre de 2026</p>
${p("José Luis:")}
${p("He cambiado el título, el subtítulo y las palabras clave de las 32 localizaciones de Yappy en App Store Connect, y lo he comprobado leyendo de vuelta la API: las 32 coinciden con lo que te cuento aquí. También he quitado de todas la captura del paywall y he añadido a la primera captura el término de búsqueda ganador de cada idioma, porque Apple indexa el texto de las capturas.")}
<h2 style="font-size:18px;margin:26px 0 10px">Qué medí</h2>
${p("Con Astro puntué unas 650 búsquedas en 35 tiendas (Lituania no la admite). Cada búsqueda tiene dos números: <b>popularidad</b> (5 es el suelo de Apple, es decir, nadie la busca; 20 ya es demanda real; 40 o más es fuerte) y <b>dificultad</b> (menos de 20 es fácil; de 20 a 40, ganable; más de 50, dura). En las tablas los verás como popularidad/dificultad.")}
<h2 style="font-size:18px;margin:26px 0 10px">Qué estaba mal</h2>
<ul style="font-size:15px;line-height:1.6;margin:0 0 14px">
<li><b>Los títulos usaban frases que nadie busca.</b> «lee en voz alta», «lecture à voix haute», «legge ad alta voce», «čte nahlas» y casi todas las demás puntuaban 5.</li>
<li><b>La búsqueda con más volumen es la inglesa «text to speech», y casi ninguna localización la llevaba.</b> Fuera de EE. UU. tiene demanda real con dificultad bajísima: Japón 51/15, México 47/17, Arabia Saudí 41/15, Finlandia 36/9, Chequia 32/9.</li>
<li><b>Había caracteres tirados.</b> «youtube» (dificultad 70 a 97 y marca registrada, con riesgo de rechazo), «offline» y «private» (dificultad 50 a 83), y dislexia o accesibilidad en idiomas donde nadie las busca.</li>
<li><b>Las cuatro fichas en inglés eran copias exactas.</b> La tienda de EE. UU. indexa las cuatro más la de México, así que teníamos 160 caracteres útiles donde caben unos 640.</li>
</ul>
<h2 style="font-size:18px;margin:26px 0 10px">Cómo lo he incluido</h2>
<ul style="font-size:15px;line-height:1.6;margin:0 0 14px">
<li><b>Título:</b> «Yappy:» más el término con mejor demanda y dificultad de cada tienda. Es el campo que más pesa.</li>
<li><b>Subtítulo:</b> el segundo término y los formatos (PDF, EPUB), formando combinaciones que la gente busca, como «pdf okuyucu» en Turquía o «pembaca pdf» en Indonesia.</li>
<li><b>Palabras clave:</b> 100 caracteres sin repetir ninguna palabra del título ni del subtítulo (Apple ya las cuenta una vez) y sin términos muertos.</li>
<li><b>Cross-localización:</b> en-GB, en-AU y en-CA llevan ahora palabras clave distintas de en-US, y es-MX captura a los hispanohablantes de EE. UU.</li>
<li><b>Capturas:</b> la primera lleva una etiqueta con el término ganador (TEXT TO SPEECH, TEXTO A VOZ, LECTEUR DE TEXTE, 読み上げアプリ…) con la tipografía mecanografiada de la app. El resto del diseño no cambia.</li>
</ul>
<h2 style="font-size:18px;margin:26px 0 10px">Tienda por tienda</h2>
<table style="border-collapse:collapse;width:100%;table-layout:fixed;font-family:Georgia,serif"><colgroup><col style="width:15%"><col style="width:25%"><col style="width:34%"><col style="width:26%"></colgroup>
<tr style="background:#f3ecdb"><th ${td} align="left">Tienda</th><th ${td} align="left">Astro (popularidad/dificultad)</th><th ${td} align="left">Ficha</th><th ${td} align="left">Cómo lo cubrimos</th></tr>
${filas}
</table>
${p("<br>Las tiendas sin idioma propio en App Store Connect (Bulgaria 30/7, Estonia 24/7, Letonia 27/9, Eslovenia 19/7) muestran la ficha en-GB, cuyo título ya es «Yappy: Text to Speech Reader».")}
<h2 style="font-size:18px;margin:26px 0 10px">Estado</h2>
<ul style="font-size:15px;line-height:1.6;margin:0 0 14px">
<li>Título, subtítulo y palabras clave: 32 de 32 en App Store Connect, verificados.</li>
<li>Capturas de iPhone: ${esc(E.iphone ?? "pendiente")}.</li>
<li>Capturas de iPad: ${esc(E.ipad ?? "pendiente")}.</li>
<li>Lo que solo puedes hacer tú en la web de App Store Connect: las etiquetas de privacidad, adjuntar las tres compras de Yappy Parlanchín a la versión 0.3.0 y pulsar «Enviar a revisión».</li>
</ul>
${p("Todo queda en <span style=\"font-family:Menlo,monospace\">marketing/aso/</span> (análisis, propuesta, ficha anterior por si quieres volver atrás) y en Astro, con la app provisional «Yappy (ASO)» para seguir posiciones cuando esté publicada.")}
${p("Un abrazo.")}
</div></div>`);
