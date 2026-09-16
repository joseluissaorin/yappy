Tercera sesión y seguimos tropezando en errores de raccord y física básica que no son aceptables en este nivel de producción. El estilo de recortes nos permite abstracción, pero no justifica la pereza en la puesta en escena. El espectador perdona la falta de in-betweens, pero no perdona que le rompan la lógica espacial. 

Aquí tienes las notas. Aplícalas sin excusas.

**Análisis por código de tiempo:**

*   **00:17 - Animación:** "Desenredó el cable con los dientes". El móvil salta a la cara de la chica de ultramar en un solo frame. Es un recorte, sí, pero necesita al menos un *smear* (un frame de arrastre) o un frame intermedio curvo para que se sienta el peso del brazo subiendo. Ahora mismo parece un *glitch*.
*   **00:19 - Raccord / Puesta en escena:** **Error grave.** La voz dice: "Me dio el auricular derecho [...] y se quedó el izquierdo". La narradora (coral) está sentada a la derecha, por lo que el oído que vemos es su *izquierdo*. La chica de ultramar está a la izquierda, vemos su oído *derecho*. Visulamente, la narradora lleva el auricular en la oreja izquierda y la otra en la derecha, contradiciendo el texto. *Propuesta:* Dibuja el cable pasando por detrás de la nuca de ambas para indicar que los auriculares van a las orejas opuestas (las que no vemos), o redibuja el cable cruzando la cara de la narradora. 
*   **01:08 - Legibilidad:** Se pidió que la bolsa de pipas fuera visible. Lo que habéis puesto en el respaldo del asiento de la anciana parece un código de barras rojo o una rejilla de ventilación. *Propuesta:* Cambiad ese asset por una silueta irregular que se lea claramente como una bolsa de plástico arrugada, no un rectángulo perfecto.
*   **01:31 - Animación / Rigging:** Al girar la cabeza la chica de ultramar, el punto de pivote (anchor point) está en el centro geométrico de la cabeza, no en la base del cuello. Parece que se le ha roto las cervicales. *Propuesta:* Bajad el punto de pivote al cuello para que la rotación sea natural.
*   **02:52 - Puesta en escena:** **Inaceptable.** "Se bajó la señora de las pipas". El recorte de la señora simplemente se desliza hacia abajo en el eje Y, atravesando el hormigón de la calle como si fuera un fantasma hundiéndose en el suelo. *Propuesta:* Animad su salida en diagonal hacia abajo y a la derecha, siguiendo la perspectiva de la acera, para simular que camina calle abajo.
*   **03:27 - Animación:** "Le bajé el volumen". El brazo coral que entra a cuadro se estira de una forma grotesca, desvinculado por completo del cuerpo de la narradora. Las proporciones se rompen. *Propuesta:* Haced que la chica de ultramar acerque un poco el móvil hacia el centro, para que el brazo coral no tenga que cruzar el 80% de la pantalla.
*   **03:34 - Errores visibles (Z-index):** Cuando la narradora apoya la cabeza, la coleta negra se solapa de forma extraña con el hombro azul, creando una tangente visual confusa. *Propuesta:* Aseguraos de que la capa de la cabeza coral esté estrictamente por encima del torso azul, y dadle un par de píxeles de sombra proyectada (drop shadow) dura para separar los dos planos.
*   **03:44 - Emoción del final:** La transición al plano del cable abstracto es correcta, pero el plano anterior (ellas apoyadas) dura muy poco para que la intimidad aterrice. *Propuesta:* Añadid 1.5 segundos de pausa estática (solo con el hervor de línea) antes del corte a la cartela final.

---

### (a) Las 5 correcciones más importantes ordenadas por impacto

1.  **00:19 - Raccord del auricular:** Dibuja el cable pasando por detrás de sus cuellos para que la distribución visual de izquierdo/derecho coincida con lo que dice el texto.
2.  **02:52 - Salida de la anciana:** Cambia la trayectoria de salida del recorte de la señora de un eje Y vertical a una diagonal que siga la fuga de la acera.
3.  **03:27 - Brazo elástico:** Acerca el móvil al centro de la pantalla antes de que el brazo coral entre a bajar el volumen para evitar que la extremidad se deforme.
4.  **01:31 - Pivote de la cabeza:** Corrige el *anchor point* de la cabeza de la chica de ultramar a la base del cuello durante su rotación.
5.  **01:08 - Rediseño de la bolsa:** Sustituye el rectángulo rojo de rayas por un recorte con silueta de bolsa arrugada para que se lea como "bolsa de pipas".

### (b) Nota
**6.5 / 10** — Hay oficio en el uso del color y el tempo general, pero la pereza en las resoluciones mecánicas (pivotes, trayectorias) arruina la inmersión narrativa.

### (c) Tres cosas que NO hay que tocar

1.  **02:55 - El apagado del motor:** La decisión de detener el hervor (boiling) de la línea cuando el motor se apaga es brillante. Refuerza el silencio de forma espectacular.
2.  **02:26 - El óvalo del recuerdo:** El tratamiento del flashback en el patio como un recorte de papel ovalado superpuesto mantiene perfectamente la estética de teatro de papel.
3.  **03:44 - El karaoke final:** El subrayado dorado avanzando en sincronía con la respiración del cable de fondo cierra el concepto de la app de forma redonda y elegante.