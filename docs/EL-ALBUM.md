# EL ÁLBUM DE LA BANDADA: plan de obra completo

La segunda piel del juguete (sucede a docs/EL-JUGUETE.md, que puso la
física; este pone la materia). Dirección de José Luis: el logo de colores
se queda; el color abundante vale si TODO obedece reglas internas; cada
documento es una PEGATINA TROQUELADA con forma propia (como parches);
mejor legibilidad; toda la app; el cartel y el karaoke de /read no se
tocan salvo sus botones; los loros mucho más integrados. Guía de fondo:
la Estética Saorín (manifiesto + checklist de autenticidad) aplicada sin
copiar: materia, jerarquía, huella, humor.

## Las nueve reglas de la casa

1. Papel siempre: fondo crema con grano en toda la app; el blanco
   absoluto no existe.
2. Una tinta por pieza (de la carta de diez), más crema y negro humo.
   Vecinas nunca repiten tinta NI forma.
3. Jerarquía de imprenta: solo una pieza gigante a la vez (la que la voz
   trabaja o la elegida); la pausada queda notable pero humilde.
4. La sangre: el color de la pieza sonando tiñe aguja, karaoke y acentos.
5. Materia o nada: borde de troquel, sombra dura de contacto, costura,
   celo, tampón. Nada flota.
6. Tres voces tipográficas: grito (condensada negra con desregistro),
   susurro (mono, a veces girada en un borde), lectura (Georgia).
7. El dorado se gana: solo favoritos y logros.
8. Iconos a mano alzada (dos o tres trazos); los de librería, prohibidos.
9. Legibilidad por jerarquía: en reposo se RECONOCE (deformación acotada,
   cuerpo mínimo, recorte con intención); al elegir se LEE entero.

## Etapas

### E0. Cirugía previa (HECHA)
Pausa-zombi muerto (teletipo y peso gigante solo mientras trabaja la
voz); blancos absolutos a crema. Commit e83da82.

### E1. El motor de troqueles (`$lib/troquel.ts`)
- Catálogo de formas como POLÍGONOS NORMALIZADOS de 48 anclas (0..100),
  todos con el mismo número de puntos para que cualquier forma pueda
  fundirse en otra por transición de clip-path (el morph gratis).
- Generadores paramétricos (nada de assets): círculo, rombo, nube, flor,
  escudo, hexágono, sello dentado, luna, más las dos RESERVADAS con
  significado: corazón (favorito) y estrella (completada al 100%).
- Cada forma declara su VENTANA de texto (zona segura en porcentaje)
  donde se compone el cartel.
- `troquelDe(item)`: favorito manda corazón; completada manda estrella;
  el resto, forma base determinista con anti-colisión entre vecinas.
- `poligono(pts, escala)`: la misma silueta a escala interior (para el
  borde de troquel: capa crema fuera, capa de tinta dentro).

### E2. La pieza-parche (cinta)
- Render por capas, todas con el mismo polígono: sombra dura de contacto
  (crema hundido, desplazada 2-3 px), borde de troquel (crema), cuerpo
  (la tinta viva), y la COSTURA (SVG de puntadas discontinuas siguiendo
  la silueta, en tono hondo).
- El morph: transición de clip-path al marcar favorito (cualquier forma
  fluye a corazón) y al completar (a estrella, con borde dorado).
- La marea sube DENTRO del troquel (el recorte la recorta solo).
- El cartel se compone en la ventana de la forma con la válvula DURA:
  compresión mínima 0.8, estirón máximo 1.35, cuerpo mínimo legible;
  si no cabe, se enseñan las primeras palabras con «…» y el título
  entero aparece al elegir (la pieza crece y su ventana también).
- La pastilla vive en el borde inferior de la ventana; el tampón LEÍDO
  (estrella pequeña dorada) para las completadas cuando no son la forma
  estrella (transición); rotaciones de pegado a mano.

### E3. El chrome de la cinta
- MEMBRETE: el logo arcoíris se queda (cada letra su tinta FIJA de la
  carta), el loro jefe posado encima, y debajo la mono susurrando datos
  reales («n piezas · m minutos de voz»).
- La TRASTIENDA como etiqueta mono GIRADA cortando el borde derecho.
- La BOCA como buzón dibujado a mano (ranura + ＋ garabateado); abierta
  sigue siendo la pieza que se traga el tablero.
- El COLOFÓN al pie del álbum: diagonal discontinua, monograma, «hecho a
  mano en Tenerife»: la pantalla es una página con final, no un feed.
- GRANO de papel global (turbulencia SVG sutil sobre el crema).

### E4. Los iconos a mano (`$lib/Trazo.svelte`)
Un componente con el vocabulario dibujado (trazos levemente temblorosos,
2.2 de grosor): play, pausa, estrella, corazón, lápiz, papelera, tijera,
engranaje, mas, visto, enlace, carpeta, portapapeles. Sustitución en
cinta, acciones, menús y aguja. Cero iconos de librería.

### E5. /read: SOLO los botones (y la aguja)
El cartel y el karaoke NO SE TOCAN. La tecla gigante, el mando de tres
teclas, el botón de ajustes y el de volver pasan a teclas de PAPEL:
fondo crema, borde tinta fino, sombra dura de contacto, icono a mano;
la protagonista lleva la tinta de la sangre (--vivo). La aguja: chrome
de papel con su tinta viva (ya bebe --vivo).

### E6. La trastienda: cajas de taller
Cada sección es una CAJA con su etiqueta mono girada en el lomo; las
tarjetas de voces (bien como están) ganan un parche de color detrás de
cada pájaro; los selectores (calidad, tema) como sellos que se estampan;
el pie de amor con el monograma y el colofón.

### E7. La biblioteca
Los audiolibros como parches-disco (círculo con surco dibujado), la
duración en el susurro, la misma física presionable.

### E8. Ficha, deshacer y vacío
- El menú de pieza: FICHA DE CATÁLOGO (la silueta del troquel en
  miniatura arriba, opciones como líneas de formulario, renombrar sobre
  línea de puntos).
- El deshacer: papelito asomando con el loro sujetándolo.
- El vacío: página de álbum con esquinas de foto esperando y el loro
  paseando.

### E9. La llegada: el troquelado
La escena de compartir se remata: el papel cae, el loro lo TROQUELA a
picotazos (tres golpes, salta confeti de recortes crema) y el parche
recién nacido vuela en arco a su hueco del álbum.

### E10. La bandada con oficios
- El bibliotecario: al soltar una pieza reordenada, el loro jefe ASIENTE.
- El fisgón contextual: varias piezas añadidas sin escuchar ninguna, y
  uno se asoma a mirar la pila.
- El maestro de ceremonias: al llegar una pieza al 100%, vuelo hasta
  ella, ESTAMPADO de la estrella (golpe de escala + háptica de éxito) y
  el decir «terminado» (una sola frase; los pájaros jamás hablan sobre
  una lectura ni dos veces seguidas).

### E11. Verificación
Checklist de autenticidad puntuada (mínimo 7/10) sobre capturas reales
de CADA pantalla tocada; regla 9 comprobada pieza a pieza; svelte-check,
fmt, clippy, tests; claves i18n nuevas en los 31 diccionarios con el
verificador en verde.

### E12. El cierre
Commit, salto a 0.2.0.9 (tauri.ios.conf.json + project.yml ×3 +
xcodegen), build de dispositivo con los rituales, subida, VALID en
App Store Connect, informe.
