# Hilos para X (y Bluesky, Mastodon)

El mismo texto vale para las tres redes. En Mastodon y Bluesky, con las
etiquetas al final del primer mensaje (#TTS #OpenSource #Accesibilidad).
Cada hilo lleva un vídeo o una captura en el primer mensaje: sin imagen, un hilo
no existe. El último mensaje, siempre con los enlaces; el primero, nunca (X
entierra los mensajes con enlace).

---

## En español (el día de la 1.0.1 en las tiendas)

1/ Yappy ya está en Android, y la 1.0.1 en el iPhone. Es un loro: le compartes
cualquier texto (un artículo, un PDF, un libro, un vídeo de YouTube) y te lo lee
en voz alta mientras el texto se ilumina. Todo dentro del teléfono.
[vídeo: compartir desde Safari → la pegatina cae en la cinta → empieza a leer]

2/ Lo que más trabajo me ha costado no es la voz, es el guion. Leer en voz alta
no es deletrear: «Enrique VIII» es «Enrique octavo», pero «siglo XX» es «siglo
veinte», y «EL CID» en mayúsculas no es un número romano. Yappy convierte cada
texto en un guion de lectura antes de hablar, con las reglas de cada lengua.
[captura: el lector con el karaoke sobre una fecha y un romano]

3/ No hay nube, ni cuenta, ni tarifa por carácter. La voz la sintetiza un modelo
abierto de unos 99 millones de parámetros que vive en tu móvil. Diez voces, 31
idiomas, y cambia de lengua sola cuando el texto cita en otra.

4/ Los libros se convierten en audiolibros con capítulos mientras duermes. Y si
tienes ordenador, el puente le pasa el trabajo: apuntas con el móvil a un QR, el
ordenador renderiza el libro y te lo devuelve solo. Sin cuentas.

5/ En Android y en el ordenador (Mac, Windows y Linux) es gratis, con todo. En el
iPhone caben tres documentos a la vez gratis, sin límite de tiempo; Yappy
Parlanchín quita el límite. El código entero es abierto, licencia MIT.

6/ Lo hice porque leo mucho y quería seguir leyendo mientras cocino. Si lo pruebas
y algo suena raro en tu idioma, contéstame aquí: es lo que más me ayuda.

yappy.joseluissaorin.com

---

## In English (el día de Show HN, después de publicarlo)

1/ I built a parrot that reads to you. Share any article, PDF, EPUB or YouTube
video with Yappy and it reads it aloud while the text lights up, sentence by
sentence. It all runs on your phone. Nothing goes to a server.
[video]

2/ The hard part wasn't the voice, it was the script. Reading aloud isn't spelling
out: "Henry VIII" is "Henry the eighth", "1492" is "fourteen ninety-two", and in
French "Louis XIV" is "Louis quatorze". Yappy rewrites every text into a reading
script first, with each language's rules.

3/ The number rules for all 31 languages come from a tiny pure-Rust interpreter of
Unicode CLDR's spell-out grammars, gender and Slavic declension included. Every
rewrite is kept as a span, so the highlight follows the voice exactly.
[captura del código o del demo WASM de la web]

4/ No cloud, no account, no per-character fees. The voice is an open ~99M
parameter model running locally. Books render overnight into chaptered
audiobooks. Free on Android, Mac, Windows and Linux; three documents at a time
free on iPhone. MIT.

5/ If something sounds wrong in your language, reply here: that's the feedback
I need most.
yappy.joseluissaorin.com · github.com/joseluissaorin/yappy

*(No enlazar el hilo de HN desde aquí: los votos que llegan por enlace directo
cuentan como anillo de voto y hunden la publicación.)*
