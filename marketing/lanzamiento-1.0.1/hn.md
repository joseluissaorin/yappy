# Show HN: ficha para escribirlo a mano

**Regla que manda:** HN prohíbe el texto generado o editado por IA
(newsguidelines: «Don't post generated text or AI-edited text»). dang lo
repitió en marzo de 2026: «Write your text by hand. Don't use an LLM to
generate any of it (not even a tiny bit, including to edit or spruce it up).»
Por eso aquí no hay texto para pegar: hay los hechos comprobados y el orden.
El inglés tiene que ser el tuyo; un inglés sencillo y con algún tropiezo
convence más que uno pulido.

## Antes de nada

- **¿Tu cuenta puede publicar Show HN?** Desde marzo de 2026 HN restringe los
  Show HN de cuentas con poco historial (https://news.ycombinator.com/showlim).
  Entra con tu cuenta en esa página: si te dice que estás restringido, toca
  comentar de verdad dos o tres semanas antes, o escribir a hn@ycombinator.com.
- Pon un correo en tu perfil de HN (así pueden invitarte a repetir).
- Tu usuario no puede llamarse como el proyecto.

## Cuándo

Según los 26 426 Show HN de marzo a octubre de 2026 (API de Algolia), las
mejores horas son **las 9–10 de la mañana del Pacífico**, que desde el 2 de
noviembre son **las 18:00 en la Península** (17:00 en Canarias). El fin de
semana rinde un poco más que entre semana. Propuesta: **jueves 5 de noviembre a
las 18:00**, o el sábado 7 a la misma hora. Y quedarse seis u ocho horas
contestando.

## Qué enlazar

El repositorio (https://github.com/joseluissaorin/yappy) o la página de
descargas en inglés (https://yappy.joseluissaorin.com/en/download/). Las dos
cosas se pueden probar sin registro, que es lo que exige Show HN.

## Título

Empieza por `Show HN:`, sin adjetivos, sin mayúsculas de adorno, sin
exclamaciones. Que diga qué es. Ideas de por dónde ir (escríbelo tú):
- qué hace: lee en voz alta artículos, PDF y EPUB;
- dónde: todo en el dispositivo, sin conexión;
- un dato técnico si cabe: 31 idiomas, Rust.

## El texto (en este orden, con tus palabras)

1. **Quién eres y por qué:** estudiante de Filología en España; lees mucho y
   querías seguir leyendo mientras cocinas o caminas; los lectores que probaste
   mandaban el texto a un servidor, cobraban por carácter o leían «Enrique VIII»
   como «Enrique V-I-I-I».
2. **Qué hace, en una frase:** compartes un artículo, un PDF, un EPUB, un Word o
   un vídeo de YouTube y lo lee en voz alta mientras se ilumina frase a frase.
3. **Lo diferente (lo que interesa a HN):**
   - el guionizador: antes de hablar convierte el texto en un guion de lectura;
   - los números de los 31 idiomas salen de un intérprete en Rust puro de las
     gramáticas RBNF de CLDR (con género y declinación eslava);
   - cada cambio se guarda como un tramo (original ↔ dicho), y por eso el
     subrayado sigue a la voz exactamente;
   - la web corre ese mismo código compilado a WebAssembly.
4. **Cómo está hecho:** Supertonic 3 (~99 M de parámetros, modelo abierto de
   Supertone) con ONNX Runtime; Parakeet para transcribir notas de voz (CoreML en
   iOS); OCR propio; Tauri 2 + Svelte; puente móvil↔ordenador por iroh (QUIC
   cifrado de extremo a extremo), sin cuentas; audiolibros .m4b con capítulos.
5. **Lo que hay que decir sin que te lo pregunten** (HN lo va a buscar):
   - el código es MIT, pero el modelo de voz es OpenRAIL-M, no tuyo;
   - escritorio y Android: gratis con todo; iOS: tres documentos a la vez gratis,
     y suscripción (3,99 €/mes, 29,99 €/año) o pago único (59,99 €) para quitar el
     límite, audiolibros y puente;
   - el móvil manda eventos anónimos (nombre de evento de lista cerrada,
     identificador aleatorio de instalación, idioma, versión) a un Worker tuyo de
     Cloudflare; nada de textos, títulos, enlaces ni identificador de publicidad;
     se apaga en la trastienda; el escritorio no manda nada.
6. **Qué opinión buscas:** idiomas donde la voz tropieza (polaco, hindi, japonés…).
7. Enlaces: App Store, Google Play, descargas de escritorio.

## Hechos comprobados el 24-10-2026 (para citar sin miedo)

Salida real del guionizador:
- `In 1492, Columbus sailed.` → «In fourteen ninety-two, Columbus sailed.»
- `Henry VIII … in the 16th century.` → «Henry the eighth … in the sixteenth century.»
- `Louis XIV régna en 1661.` → «Louis quatorze régna en mille six cent soixante-et-un.»
- `Enrique VIII y el siglo XX. EL CID llegó.` → «Enrique octavo y el siglo veinte. El CID llegó.»

Otros:
- El modelo se descarga una vez: unos 400 MB en el ordenador, unos 200 MB en el móvil (fp16).
- El .dmg del Mac está firmado y notarizado; Windows va sin firma (SmartScreen avisa).
- Requisitos: macOS 12 con Apple Silicon, Windows 10 x64, Android 7.0.

## Preguntas que van a llegar (los hechos; la respuesta, tuya)

- **¿Por qué no la voz del sistema?** La misma voz en todas las plataformas y en
  31 idiomas, calidad neuronal, sin conexión, y tiempos exactos para el subrayado.
- **¿Velocidad?** Hay un colchón de dos frases por delante. *Medir en tu Mac y
  en tu iPhone antes de dar ninguna cifra.*
- **¿Open source si iOS es de pago?** Todo el código está en el repositorio, iOS
  incluido. *Comprobar que compila sin las claves de RevenueCat antes de decirlo.*
- **¿Frente a ElevenReader o Speechify?** Ellos usan voces en la nube, mejores en
  inglés y con precio acorde; Yappy no saca nada del dispositivo, funciona en un
  avión y no cobra por carácter.
- **¿Por qué un loro?** Repite lo que lee sin entenderlo, como la síntesis de voz.

## Lo que no hay que hacer

- Pedir votos o comentarios a nadie (HN penaliza o banea el sitio y la cuenta).
- Pasar el enlace del envío para que voten desde ahí.
- Borrar y volver a publicar.
- Contestar a la defensiva: si encuentran un fallo, se les da la razón y se abre
  una issue en el momento.
