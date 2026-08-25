# Plan vivo: de app correcta a criatura viva (24-08-2026)

El encargo: matar los dos fallos graves (zoom de página web, háptica muda),
llevar lo nativo a primera clase, y darle cuerda al diseño entero: loro con
conducta, tipografía que respira con la voz, coreografía con oficio de
animador, la voz elegida tiñendo la app, y completar la aplicación.

## Fase 0 · El suelo
- [x] 0.1 Zoom muerto: viewport (max-scale=1, user-scalable=no, viewport-fit=cover)
      + touch-action: manipulation + user-select none + touch-callout none
      + text-size-adjust none + overscroll-behavior none.
- [x] 0.2 Háptica NATIVA: Haptics.swift (@_cdecl, generadores precalentados:
      impact light/soft/medium/rigid/heavy + notification + selection),
      extern C en Rust + haptic_cmd, haptic.ts reescrito (iOS nativo,
      Android vibrate). Acción `presionable` global: tick + .pulsado al
      POSAR el dedo. Mapa: selection (posar, frase a frase, detentes),
      rigid (piano), medium (pausa/reanudar), heavy (aterrizaje, ñam),
      success/warning (descarte, error).
- [x] 0.3 Teclado (enterkeyhint, sin autocorrect en URL, esquiva con
      visualViewport), gesto de borde = volver, reduced-motion en todo.

## Fase 1 · El alma: el loro
- [x] 1.1 Criatura 2.0: marioneta (cabeza, pupilas, párpados, pico con
      apertura 0..1) + estados (posado/hablando/pausa/comiendo/dormido/
      avergonzado/celebrando) + parpadeo y acicalado con temporizador.
- [x] 1.2 `playback_nivel` (~24 Hz, RMS de la ventana sonando) desde el
      hilo de audio: el sistema nervioso de pico, tipografía y aguja.
- [x] 1.3 Colocaciones: carrete (mirada sigue el dedo), cameo en el
      cartel, asomado a la boca, comiéndose el modelo, avergonzado en
      errores, dormido de noche.
- [x] 1.4 Overscroll-loro: cuelga del carrete y silba al refrescar.

## Fase 2 · El cuerpo: cartel
- [x] 2.1 Peso variable de Literata respirando con el nivel.
- [x] 2.2 Subrayador-rotulador (borde irregular, grosor por amplitud).
- [x] 2.3 Frases con {#key}: la vieja se derrama arriba, la nueva aterriza
      con sobreimpulso.
- [x] 2.4 Swipe de párrafo pegado al dedo con goma.
- [x] 2.5 Anticipación del mando (inclinación previa).
- [x] 2.6 Pausa exhala / reanudar inhala; piano escalonado.
- [x] 2.7 Preguntas inclinadas; títulos con asentimiento.
- [x] 2.8 Dial-cuentakilómetros con detentes hápticos.

## Fase 3 · La cinta viva
- [x] 3.1 Cascada al montar. 3.2 Aterrizaje con squash + brinco del loro.
- [x] 3.3 Descarte que se arranca girando. 3.4 FLIP en recolocaciones.
- [x] 3.5 Aguja con latido real y 3.6 pegajosa. 3.7 Parallax del riel.
- [x] 3.8 Boca con muelle (＋→×). 3.9 Bobina girando. 3.10 .pulsado global.

## Fase 4 · El casting
- [x] La voz elegida = tu loro en toda la app; su tinta se filtra en los
      acentos con color-mix (70 % marca / 30 % voz). Presentación con pico.

## Fase 5 · Completar
- [~] Audiolibros al idioma nuevo (PENDIENTE la restilización del
      reproductor m4b); mini-ondas en el segmento sonando HECHO;
      teatro de errores; accesibilidad (VoiceOver sobre gestos, aria es);
      manual al día; Android PENDIENTE de recompilar; 0.2.0.5 a TestFlight HECHO; capturas HECHAS.

## Fase S (si cabe) · Foley nativo
- [ ] AudioServices + .caf procedurales (papel, ñam, tock, silbido).
      (No entró en esta pasada: la háptica es la primera voz del tacto.)
