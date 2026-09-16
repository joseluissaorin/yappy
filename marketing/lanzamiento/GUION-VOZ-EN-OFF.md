# El loro no sabe leer · Voz en off

*Las 61 tomas de José Luis, en orden, tal como se leen. Di el número antes de cada toma y deja un segundo. Las pausas ⏸ se callan de verdad. Lo que va en cursiva entre paréntesis es tono, no se lee. Las líneas con 🦜 son lo que lee la app: no se graban, están para que sepas qué viene después. Fechas ya corregidas para publicar el 17 (tomas 16, 45, 53 y 60).*

2.684 palabras · unos 20 minutos a ritmo de conversación.

### 0 · La Alhambra habla (0:00–1:05) · película, Granada

**01** _(bajo, como quien señala algo que está delante)_ En Granada hay un edificio que habla en primera persona.

> ⏸ 1,5 s

**02** No es una metáfora. Las paredes de la Alhambra están escritas, y muchas de las inscripciones dicen «yo». Yo soy el jardín. Yo soy la corona. Yo soy la fuente. Un edificio que se presenta a sí mismo.

**03** Esto lo escribió un poeta, Ibn Zamrak, hacia 1380. Era el visir. Y aquí viene la pregunta rara.

> ⏸ 1 s

**04** Casi nadie que entraba en esta sala sabía leerlo. Ni el pueblo, ni buena parte de la corte, ni los embajadores. Entonces, ¿para quién está escrito un poema en una pared que casi nadie puede leer?

**05** Para ser dicho. Estos poemas se recitaban. La pared era el guion; la voz, la lectura. El edificio hablaba porque alguien lo leía en voz alta a quien no podía leerlo.

> 🦜 _la app_ (voz masculina, grave; karaoke): Yo soy el jardín que la belleza adorna. Mírame y sabrás lo que soy. Nunca vieron ojos nada…

**06** Le he copiado el poema y se lo he dado a mi app. Suena aquí, delante de la pared, seiscientos años después. Y lo raro es que eso, leer en voz alta lo escrito, fue lo normal durante casi toda la historia de la escritura. Lo raro somos nosotros.


### 0b · Milán, 384 (1:05–1:50) · teatro de papel

**07** Mil años antes de Ibn Zamrak, un hombre entra en el cuarto de otro y lo encuentra leyendo. No pasa nada raro. Salvo una cosa.

> ⏸ 2 s

**08** El que lee no mueve los labios.

**09** El que mira es Agustín. Tiene treinta años, todavía no es santo, y lo escribe en sus Confesiones porque no lo entiende.

> 🦜 _la app_ (voz grave, sin karaoke; tarjeta de papel viejo en la marquesina): «Cuando leía, sus ojos recorrían las páginas y su corazón penetraba el sentido; pero su vo…

**10** Leer era sonar. Lo que había que explicar era el silencio.

> ⏸ 3 s


### 0c · El corte (1:50–2:15) · película

**11** He hecho una app que devuelve la voz a lo escrito. Cualquier texto, en el teléfono, sin internet.

> ⏸ 2 s

**12** Y no sé si es buena idea.


### 1 · Flexiones (2:15–4:00) · película, comedia muda

**13** Ese era yo, en junio. Estaba en el ordenador leyendo un artículo y quería hacer unas flexiones. Las dos cosas, no una y luego la otra. No es una historia importante; casi nada se inventa por una razón importante.

**14** Tenía en el ordenador una app que me gustaba, Handy: le hablas y escribe. Pensé que quería exactamente lo contrario. Que escribieran y me hablara. Y que lo hiciera aquí, en el suelo, con las manos ocupadas.

**15** Lo que hice en junio fue una ventana gris en el Mac que leía un texto en voz alta. Nada más. Ni teléfono, ni compartir, ni loro.


### 2 · Un mes (4:00–6:15) · mesa y artefactos

**16** En agosto, XMihura ofreció en Twitter una minibeca: doscientos euros, un mes de Claude, para hacer una cosa que uno quisiera hacer y no pudiera. Me la dio a mí. La presentación de resultados era el quince de septiembre. Este vídeo es la presentación de resultados, con dos días de retraso.

**17** Esto era Yappy al empezar el mes.

**18** Esto es Yappy hoy. En medio hay un mes. Ciento diez commits. Sesenta mil líneas de código entre Rust, Svelte y TypeScript, y otras mil setecientas de Swift para lo que iOS solo deja hacer en Swift. Y una pila de papeles.

**19** No trabajé solo. Trabajé con una inteligencia artificial como se trabaja con alguien en un taller. Yo decía qué tenía que ser verdad; ella lo hacía verdad; y cuando no lo era, discutíamos hasta que se escribía una regla. Cada papel de estos es una discusión que terminó en regla. «El papel se ve siempre.» «Nada de pruebas gratuitas.» «Un solo código, tres plataformas.» Y la primera de todas, la que decide lo demás:

**20** Y el registro de todo, el git log, que suele ser lo menos literario que existe, acabó siendo esto.

> 🦜 _la app_ : El loro aprende Android: la APK compila, arranca y viaja por el Send. El Mac abre con vent…

**21** Siete commits llevan la palabra «loro» en el título. No lo hice a propósito.

> ⏸ 2 s


### 3 · Cómo se hace (6:15–12:15) · mesa, diagramas legítimos, pantallas reales


**3.1 · El esqueleto (6:15–7:00)**

**22** Primero, de qué está hecha. Yappy es una sola base de código para iPhone, Android, Mac, Windows y Linux. El motor está en Rust: la síntesis, el audio, la cola de documentos, la imprenta, el puente. La interfaz está en Svelte, como una página web, y Tauri la envuelve en una app nativa. Y hay catorce ficheros de Swift para lo que iOS solo deja tocar en Swift: la sesión de audio, la pantalla de bloqueo, las compras, la extensión de compartir, la ventana flotante del loro.

**23** Decidí eso por una razón que no es técnica: quiero que esta app dure. Que dentro de cinco años, cuando haya voces mejores y el servicio de moda de este año haya cerrado, Yappy siga leyendo igual. Por eso todo pasa dentro del teléfono. No hay servidor. No hay cuenta. No hay nada tuyo que salga de ahí. Estable, previsible, y aburrida si hace falta.


**3.2 · La voz (7:00–8:30)**

**24** La voz se llama Supertonic 3. La hizo Supertone, en Corea; no es de código abierto, pero sus pesos son públicos, y eso basta para meterla en un teléfono. Son cuatro redes que se ejecutan en fila para cada frase.

**25** Una predice cuánto dura cada sílaba. Otra convierte el texto en un espacio latente. La tercera, la que de verdad canta, quita ruido paso a paso, como quien revela una foto: ocho pasos en la calidad normal, cinco si tienes prisa, doce si quieres lo mejor. Y la cuarta convierte ese latente en sonido, a cuarenta y cuatro mil cien muestras por segundo.

**26** Todo eso corre con ONNX Runtime, en el chip del teléfono. Y aquí la primera guerra: el modelo pesaba trescientos noventa y ocho megas. Demasiado para bajarlo el primer día en cualquier parte.

**27** Lo pasamos a media precisión, dieciséis bits en vez de treinta y dos. Suena fácil. No lo es: si conviertes todo, el modelo no carga, porque dos nodos del estimador no soportan la media precisión. Hay que encontrarlos y dejarlos en treinta y dos, y quitar la información de tipos vieja que el convertidor arrastra. Doscientos megas. Un veinte por ciento más lento, y nadie distingue las dos voces: se lo di a otra inteligencia artificial a ciegas y transcribió lo mismo con las mismas notas.

**28** Y en iPhone ni eso: los dieciséis ficheros del modelo bajan con la propia instalación, como recursos esenciales de la App Store, antes de que abras la app la primera vez. Si la abres antes de que terminen, el loro sigue comiendo y te enseña la barriga.

**29** Un fallo de esa parte, que me encanta por lo tonto: durante semanas la voz no cargaba en el iPhone, y era porque el acelerador de Apple se registraba dos veces, una global y otra por sesión, y la segunda vez ONNX se ofendía y decía «este proveedor ya está registrado». Una línea.


**3.3 · El guion (8:30–10:00)**

**30** Pero una voz no sabe leer. Sabe pronunciar. Entre lo que está escrito y lo que se dice hay un abismo, y ese abismo se llama guionizador.

> 🦜 _la app_ (es): El doce de septiembre de mil novecientos treinta y seis, a las ocho y media, el doctor Rui…

**31** Eso alguien lo tuvo que escribir. Cómo se dice una fecha. Cómo se dice un porcentaje. Que «Dr.» es doctor pero «Dra.» es doctora. Que «s. XIX» es siglo diecinueve y «Luis XIV» es Luis catorce, no Luis decimocuarto, y que en italiano el siglo va detrás. Y no en un idioma. En treinta y uno.

**32** Los números los deletrea un intérprete de las reglas del consorcio Unicode, las mismas que usan los sistemas operativos para escribir «mil novecientos» en ruso con sus plurales o en japonés con sus contadores. Las llevamos dentro de la app, sin llamar a nadie. El resto, fechas, horas, monedas, unidades, abreviaturas, romanos, son tablas escritas a mano, idioma a idioma, mil ciento sesenta y una líneas, con una regla de oro: el texto original no se toca nunca. Cada pieza guarda el original intacto y, al lado, una partición en trozos con su forma hablada.

**33** Ese doble libro es lo que permite dos cosas. Que el editor te enseñe lo que escribiste y no lo que se dice. Y que el karaoke sepa exactamente qué palabra del original corresponde al sonido que suena, sin adivinar.

**34** Después el guion se trocea por frases, porque la voz cocina frase a frase. Y aquí el fallo más caro del mes: el troceador, cuando una frase era muy larga, la partía por comas, recortaba los trozos y los volvía a pegar con una coma nueva. Y como el sintetizador busca cada trozo literalmente en el texto y aquel trozo ya no existía, lo descartaba en silencio. Resultado: párrafos que sonaban a medias. Dos frases y se callaba. Me lo encontré yo el nueve de septiembre, escuchando un libro.

**35** La regla que salió: cada trozo es una rebanada exacta del texto, y el troceador nunca descarta. Está escrita en un fichero de memoria, para que no vuelva a pasar.


**3.4 · El oído (10:00–11:00)**

**36** La voz cocina siempre dos frases por delante de tu oído. Cuando ves «preparando» en el cartel es eso: el colchón. Dos frases con texto, o cuatro segundos de audio, o el final del documento, y entonces empieza a sonar. Y si algo va mal antes del primer sonido, reintenta una vez sin decírtelo.

**37** Pero el fallo que más tiempo se llevó no fue de la voz: fue del silencio. «Abro el documento y no suena.» Tenía tres causas distintas, y hubo que encontrar las tres.

**38** Una: la imprenta, que hace audiolibros en segundo plano, se quedaba con el motor y no lo soltaba. Ahora la lectura tiene prioridad: cuando alguien quiere escuchar, la imprenta suelta el motor en el siguiente trozo y vuelve a la cola.

**39** Dos: iOS. Un proceso de mantenimiento apagaba la sesión de audio por debajo de una lectura viva, y el sistema de audio se quedaba mudo para siempre. Ahora Swift sabe si hay una lectura viva y no toca la sesión.

**40** Y tres, la que me gusta: nadie vigilaba que el audio avanzara de verdad. Así que ahora hay un oído. Un vigilante en el hilo de audio que, si el estado dice «sonando», hay material en el búfer y el reloj de muestras no se mueve durante un segundo y dos décimas, da por muerta la salida y la reconstruye entera. Y si la reconstruye y sigue muerta, espera el doble y lo intenta otra vez, hasta seis veces. Se abre el documento y suena. Siempre. Aunque sea al segundo intento.

> 🦜 _la app_ (karaoke, grande): El cable tiraba un poco cada vez que respiraba.…

> ⏸ 2 s

**41** Las palabras se encienden cuando se dicen. No cuando la app estima que se dicen: el motor publica, para cada frase, el segundo exacto en que empieza y termina en el reloj de muestras, y la interfaz solo interpola. Volveremos a esa frase.


**3.5 · Entrar (11:00–11:40)**

**42** Y todo esto no sirve de nada si el texto no entra. La puerta es compartir. Desde Safari, desde Archivos, desde WhatsApp, eliges Yappy y ya está.

**43** Lo que hay detrás de esa puerta es lo menos glamuroso y lo más trabajado. Para una página web, la extensión de compartir no descarga la página: se lleva la página que tú estás viendo, con tu sesión, y si estás suscrito a un periódico, se lleva el artículo entero sin muro. Para un hilo de Hacker News, va a su API y lo convierte en una conversación: fulano dice, mengano responde. Para un tuit largo, a la API de sindicación de Twitter, con un token que se calcula a partir del identificador multiplicado por pi, en base treinta y seis, sin ceros. No me lo he inventado; es así. Reddit, Bluesky, Mastodon, un PDF con OCR, un EPUB, un DOCX. Y una nota de voz de WhatsApp, que la extensión transcribe con un segundo modelo, Parakeet, dentro del límite de ciento veinte megas de memoria que iOS da a una extensión.


**3.6 · El puente y lo que se rompió (11:40–12:15)**

**44** Un libro entero son horas de voz, y un teléfono no está para eso. Por eso hay imprenta: eliges los capítulos y la app te deja el audiolibro hecho, con sus marcas. Y si tienes un ordenador, le pasas el encargo por el puente: una conexión cifrada de punto a punto, sin servidor en medio, que se empareja escaneando un código con la cámara. El Mac imprime de noche; por la mañana el libro está en el teléfono. Si se corta a la mitad, sigue desde donde iba.

**45** Y las cosas que se rompieron y no salen en ningún teaser. En iOS, cada actualización cambia de sitio la carpeta de la app, y yo guardaba rutas absolutas: cada versión nueva de TestFlight convertía toda la cinta en piezas zombi. Se guardan nombres, nunca rutas. El número de compilación: una vez lo generé con la fecha y la hora, y Apple lo rechazó porque ningún número puede pasar de dos mil ciento cuarenta y siete millones. El rsync de Homebrew, que rompía la exportación de Apple sin decir por qué, hasta que apareció en un log escondido. Un plugin de notificaciones que hacía crashear la app cinco veces seguidas cada vez que un audiolibro terminaba con la app abierta. Y hace unos días, un script mío de traducciones que se comió tres frases en los treinta y un idiomas a la vez, y el comprobador de paridad no lo vio porque los treinta y uno habían perdido lo mismo.

**46** Nada de esto es difícil por separado. Es difícil porque son cuarenta cosas así, y cada una tiene que funcionar para que un niño de seis toque una pegatina y suene.


### 4 · El juguete (12:15–14:00) · película y teatro de papel

**47** La app no parece una app y es a propósito. Es una percha con pegatinas. Cada texto es una pegatina troquelada, con su forma y su color, que salen del nombre del fichero: el mismo texto siempre tiene la misma pegatina. Todas las formas tienen cuarenta y ocho puntos, ni uno más, para que una pueda fundirse en otra: marcar favorito la vuelve corazón; terminarla, estrella.

**48** Lo que has oído sube por dentro como una marea. Tocar es escuchar. Mantener pulsado abre el menú. Y hay un loro: vuela cuando llega algo nuevo, se asoma detrás de las piezas cuando te distraes, come mientras bajan las voces o se imprime un libro, y cada tanto reordena la percha porque le apetece.

**49** La regla de todo esto la llamé la prueba de fuego: que un niño de seis años quiera jugar con esto sin querer escuchar nada, y que un filólogo de sesenta la use todos los días sin que le estorbe.

**50** No existen. Son la regla. Pero se ven en dos gestos.

**51** Alguien se quita las gafas y sigue leyendo.

**52** Alguien juega mientras la máquina hace lo suyo.

**53** Todo lo demás sale de esa regla: el onboarding es un paseo en el que el loro te habla en voz alta y te enseña a compartir yéndose contigo a Safari en una ventanita; la interfaz habla treinta y una lenguas, cuatrocientas frases cada una; los mandos del lector son la pantalla entera. Cuando algo no pasaba la prueba, se quitaba. Se quitó bastante: una biblioteca entera, la semana pasada.

> ⏸ 3 s. *Fundido a papel vacío. El silencio más largo del vídeo.*


### 5 · Media voz (14:00–17:20) · el corto

> 🦜 _la app_ : El cable tiraba un poco cada vez que respiraba.…

> ⏸ 4 s. *La orquesta sostiene.*


### 6 · El loro no sabe leer (17:20–18:20) · teatro de papel

**54** Agustín se asustó de un hombre que leía sin voz. Ibn Zamrak escribió en una pared para que alguien la dijera. Yo he hecho una máquina que le devuelve la voz a lo escrito. Parece un círculo bonito. No lo es.

**55** Esto no es una tesis sobre la lectura. Es una herramienta. Y la herramienta es un loro.

> 🦜 _la app_ (voz grave): «Sus ojos recorrían las páginas y su corazón penetraba el sentido; pero su voz y su lengua…

**56** Un loro dice cualquier cosa. La dice bien, con acento de ninguna parte, en treinta y una lenguas, sin cansarse, con el segundo exacto de cada palabra. Y no sabe qué dice.

> ⏸ 4 s. *Ambrosio pasa una página. El loro ladea la cabeza.*


### 7 · No lo sé (18:20–19:20) · a cámara y taller de noche

**57** Os digo lo que no sé.

**58** No sé si esto ayuda. Hacer tan fácil que un texto suene puede ser justo lo que no hace falta. Puede que lo que haga falta sea lo contrario: sentarse, aguantar la página, entrenar la atención. Puede que una app así sea la última cosa que necesitamos.

> ⏸ 2 s

**59** La he hecho porque me encantaría que alguien leyera un poco más gracias a ella. Un artículo más. Un capítulo más, en el autobús, con un oído. Eso es todo lo que quiero. Y no sé si lo consigue.

**60** Lo que sí sé es lo que viene. Ya está en la App Store, desde hoy. Android, en cuanto Google apruebe la cuenta; el escritorio después; el código, abierto, cuando esté a la altura. Y esta noche, mientras veis esto, ese ordenador está imprimiendo la primera tanda de una biblioteca pública: los clásicos en español que ya no son de nadie, de Cervantes al último autor libre, en audiolibro, gratis, para cualquier lector y cualquier aparato.


### 8 · Cierre en verso (19:20–20:00) · película

**61** _(el poema; borrador si no tienes otro. Léelo verso a verso, con aire)_ Yo no sé leer con los ojos cerrados. Sé escuchar. Sé que hay una voz que no es de nadie diciendo lo que alguien escribió para alguien, y que en medio, sin querer, estoy yo.
