# EL JUGUETE: los primitivos de la interfaz de Yappy

Doctrina de la cuarta vida de la cinta. No es una pantalla: son PRIMITIVOS
nuevos para toda la app. Nacen en la pantalla de inicio y se extienden a
todo. La raíz es el tablero de comidas favoritas de Kalorica
(`food-preferences.tsx`, leído entero), pero cada mecanismo se lleva un
paso más lejos. La ley de la casa sigue intacta: colores vivísimos y
PLANOS, texto blanco encima, sin sombras, sin gradientes, sin
transparencias. Y la prueba de fuego manda: el juguete es textura, jamás
obstáculo.

## 1. El credo

1. No se anima el elemento: se cambia la VERDAD (los pesos) y el tablero
   entero renegocia con física (FLIP con muelle).
2. Tres contactos hápticos por gesto: posar, soltar, confirmar. En los
   arrastres, hápticas por etapas que narran el gesto.
3. Muelles siempre. Tres rigideces con nombre:
   - SECO (contacto): 0.16s, `cubic-bezier(0.3, 1.4, 0.6, 1)`.
   - SERENO (tablero): 0.6s, `cubic-bezier(0.18, 1.5, 0.32, 1)`.
   - REBOTÓN (llegada): 0.42s, `cubic-bezier(0.24, 1.7, 0.44, 1)`.
4. La velocidad del dedo se hereda: lo soltado sigue tu impulso.
5. Todo reacciona DURANTE el gesto, nunca después.
6. Escalonado por distancia (la onda, 24 ms por vecina) o por índice
   (la cascada, 40 ms por pieza).
7. La tipografía se calcula desde la geometría de la pieza, no al revés.
8. Exageración sin miedo: la pieza elegida pesa SEIS veces una normal.

## 2. Los primitivos (`$lib`)

- `juguete.ts`: los muelles con nombre, la paleta viva, `colorDe(id)`,
  `tonoHondo(color)` (el mismo color, hundido, para tinta sobre tinta),
  `radiosDe(id)` (esquinas asimétricas deterministas: cada pieza con su
  canto propio, entre 10 y 26 px), `tiltDe(id)` (inclinación de collage,
  ±1.3°, determinista).
- `presionable.ts` (ampliado): háptica al POSAR y también al SOLTAR
  (tick), hundimiento `.pulsado` con SECO; sirve para cualquier cosa
  tocable de toda la app.
- `mosaico.ts` (v2): empaquetado Kalorica de verdad: SIN hueco
  horizontal (borde con borde, losa continua), 3 px entre filas, alto de
  fila `64 + pesoMedio × 26` con techo en 320 px, mínimo de ancho por
  longitud del título, redistribución en ambos sentidos.
- `cartel.ts`: el motor tipográfico de los titulares (§5).

## 3. La percha (la integración profunda)

La pantalla deja de ser tres estratos (cabecera, boca, tablero) y pasa a
ser un cuerpo. NO HAY CABECERA: la primera fila del mosaico son las
piezas del SISTEMA, en crema (para distinguirse de las piezas vivas):

- LA MARCA: una pieza con el «yappy» en letras de recorte (cada letra de
  un color de la paleta, con su giro propio, como pegadas de revistas
  distintas) y el LORO posado ENCIMA de su canto superior, asomando por
  encima de la fila. Su mirada sigue el dedo; celebra las llegadas; en la
  ola (pulsar la marca) las letras hacen la gelatina escalonada; doble
  pulsación: voltereta. Cuando algo falla, se avergüenza.
- LA TRASTIENDA: su engranaje, pieza del mosaico; gira al pulsarlo y
  navega. Nada de píldora flotante en una barra.
- LA BOCA: el ＋ es UNA BALDOSA MÁS (crema, borde discontinuo; su ＋
  recorre la paleta lentamente para invitar). Al pulsarla su peso salta a
  26 y el propio FLIP la hace crecer hasta tragarse el tablero; dentro,
  pegar, enlace y archivo con entrada escalonada. Añadir no abre un menú:
  una pieza SE CONVIERTE en el menú.
- Con la cinta vacía, el tablero es la fila del sistema con la boca
  crecida, y el loro pasea debajo con el «enséñame». El vacío es el
  tablero recién nacido, no otra pantalla.

## 4. Las piezas de fieltro

- Forma: NADA de rectángulos clónicos. Cuatro FAMILIAS de recorte
  deterministas por pieza (`formaDe`): fieltro (canto blando muy
  asimétrico, radios 14 a 42), sesgada (cuadrilátero irregular por
  clip-path, papel cortado a tijera con esquinas vivas), mordida (dos
  esquinas enormes enfrentadas, dos mínimas) y canto (casi blob, la
  piedra pulida). Inclinación propia de ±2.4 grados. El tablero es un
  collage, no una retícula.
- Colores sin choques: el índice de paleta se AJUSTA para que dos
  vecinas no repitan color (la que suena conserva siempre su color base:
  la sangre `--vivo` debe coincidir).
- La forma es ESTADO: al presionar, los radios se hinchan (el material
  cede); la pieza que SUENA pierde el recorte anguloso y respira: sus
  radios ondulan en bucle.
- Pesos renegociados (el corazón de Kalorica, exagerado): con `n` piezas
  grandes a la vez (la elegida y la que suena), cada grande pesa
  `max(3, 7 − n)`; favorita 2.0; con error 1.1; el resto
  `1 + min(0.9, minutos/22)`: los minutos SE VEN. Elegir una pieza la
  hace CRECER de verdad (×6) y TODO el tablero se recoloca con muelle.
- Modo noche: todas duermen (atenuadas con filtro), salvo la que suena,
  que conserva su color despierto. El loro, con los ojos cerrados.

## 5. Los carteles (la tipografía nueva)

La regla del encargo, literal: el titular llena el ANCHO y el ALTO de la
pieza, con una tipografía que SE ESTIRA Y SE DEFORMA, y SIN padding.

- Motor: cada línea es un `<svg>` con `preserveAspectRatio="none"` y su
  `viewBox` medido con canvas a cuerpo 100: al estirarse al ancho y al
  alto de su franja, los glifos se deforman en los dos ejes. Tipografía
  de madera de feria: la palabra corta sale gorda, la larga sale apretada.
- Partición: el título se parte en 1 a 4 líneas según la proporción de la
  baldosa, balanceando anchuras naturales medidas (no por letras).
- Padding CERO: la caja del cartel toca los cuatro cantos de la pieza.
  Los adornos viven ENCIMA del cartel, no a su lado.
- La línea de flotación: lo ya escuchado sube desde abajo como marea en
  `tonoHondo`, con el borde superior nítido. En la que suena, la marea es
  `elapsed/duration` en vivo.
- LA PASTILLA: la única etiqueta es un objeto sólido (fondo `tonoHondo`,
  texto blanco, cápsula pequeña abajo a la izquierda). En reposo solo los
  minutos con su icono de tipo; sonando, el estado con sus ondas; en la
  cocina y el error, su aviso. JAMÁS texto suelto peleándose con el
  cartel: eso era el lío.

## 6. La baldosa que habla

La pieza que suena deja el cartel y muestra EL TEXTO QUE SE ESTÁ
DICIENDO (`snapshot.current_text`): cada frase nueva empuja a la anterior
hacia arriba como un teletipo con muelle. El tablero y el lector dejan de
ser dos sitios. Encima del canto superior, asomado, el pájaro de la voz.
Tocarla lleva al cartel grande (/read) como siempre.

## 7. La bandada

- El vuelo: cuando llega una pieza nueva, un pájaro pequeño sale de la
  baldosa ＋ y vuela EN ARCO (principio de los arcos) hasta la pieza
  recién nacida, la picotea, y la pieza late. Háptica heavy al posarse.
- Las asomadas: en ratos muertos (sin nada sonando), muy de tarde en
  tarde, un pájaro se asoma tras una pieza al azar, mira, y se esconde.
- Decir el título: al elegir una pieza, si no suena nada, su voz DICE el
  título en voz baja (síntesis local corta por el camino de las muestras:
  `efecto_play`, sin tocar jamás la sesión de lectura; si el motor está
  ocupado, silencio y a otra cosa). La interfaz se lee a sí misma.
- El sello: una pieza escuchada al 100% recibe una huella de pata blanca
  estampada (con su golpe de escala al aparecer).
- La marca: pulsar el logotipo hace la OLA letra a letra (gelatina
  escalonada 40 ms) y el loro celebra; doble pulsación: voltereta entera.
  El engranaje de la trastienda gira al ir y también al volver.

## 8. La sangre de color

El color viaja con la pieza: cuando algo suena, `--vivo` (variable CSS en
la raíz) toma su color y tiñe la aguja universal, el resaltado del
karaoke del cartel y los acentos de la cinta. Abres una pieza naranja y
Yappy queda naranja por dentro hasta que calla. Sin nada sonando, la casa
vuelve a su tinta.

## 9. Los gestos

- Tocar: elegir (crece ×6, acciones dentro, onda a las vecinas, y la voz
  dice el título si hay silencio).
- Agarrar: 160 ms de dedo posado y la pieza SE RECOGE con un pop (escala
  con muelle + háptica rígida): ya es tuya. Arrastrarla reordena EN
  VIVO: el tablero se reempaqueta bajo el dedo, tic háptico por cada
  desplazamiento, la levantada se estira con la velocidad y se ladea con
  el gesto; al soltar, aterriza con SERENO y las vecinas ondulan.
- Lanzar (soltar la levantada con velocidad horizontal alta): la pieza
  sale despedida girando con tu impulso: es borrar, con «deshacer» de
  cinco segundos (el borrado real espera al deshacer caducado).
- Deslizar corto lateral: descarte clásico, se conserva.
- Todo botón de la app: posar hunde con tick, soltar devuelve con
  sobreimpulso y tick.

## 10. Los doce principios, aterrizados

| Principio | Dónde vive |
|---|---|
| Squash y stretch | la levantada se deforma con la velocidad; todo se hunde al presionar |
| Anticipación | el hundimiento previo; el hueco que late antes de soltar |
| Staging | cascada de entrada; la elegida elevada; la onda dirige la mirada |
| Straight ahead y pose a pose | el dedo manda en directo; el layout se resuelve entre poses (FLIP) |
| Follow-through | muelles con sobreimpulso; la velocidad heredada al soltar |
| Slow in y slow out | muelles siempre, lineal jamás |
| Arcos | el vuelo del pájaro; el ladeo de la levantada |
| Acción secundaria | la onda, el loro que mira, las hápticas, el teletipo |
| Timing | tres rigideces con nombre; escalonados de 24 y 40 ms |
| Exageración | pesos ×6; titulares deformados llenando la pieza |
| Dibujo sólido | escala uniforme (media geométrica) en el FLIP; carteles medidos, no estimados |
| Appeal | la losa viva, el blanco rotundo, cada pieza con su canto propio |

## 11. La válvula

- Nada bloquea: la síntesis arranca en el toque, no al acabar la
  animación; las hápticas no esperan; los vuelos son decorado.
- `prefers-reduced-motion`: los adornos se apagan; el significado queda.
- Los pájaros callan cuando algo suena: jamás pisan una lectura.
- Legibilidad: la partición del cartel evita compresiones ilegibles
  subiendo o bajando el número de líneas antes que aplastar de más.

## 12. El orden de obra

1. `juguete.ts` + `presionable` doble + `mosaico` v2 + `cartel.ts`.
2. La percha entera (pórtico habitado, boca-baldosa, tablero de fieltro,
   carteles, marea, numeral, sello).
3. La bandada (mirada dirigida, vuelo, asomadas, decir el título:
   comando `decir_cmd` clonando el camino iOS de `sample_voice`).
4. La sangre (`--vivo` en aguja y karaoke).
5. Gestos: lanzar-para-borrar con deshacer; ola y voltereta de la marca.
6. Verificación en simulador pieza a pieza (capturas), `svelte-check`,
   `fmt`, `clippy`, tests; luego build de dispositivo y TestFlight.
