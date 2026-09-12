# Las voces sin muro: las cuatro palancas de la descarga

El modelo de voces (Supertonic 3) pesaba 398 MB en fp32 y se bajaba de
Hugging Face en serie al tocar «Descargar». Desde el 9 de septiembre de
2026 hay cuatro palancas, y todas están puestas.

## 1. Llegan con la instalación (iOS, Background Assets)

- Extensión `YappyRecursos` (`gen/apple/YappyRecursos/Descargador.swift`,
  ExtensionKit, punto `com.apple.background-asset-downloader-extension`).
  La App Store la lanza al instalar y al actualizar; lee el manifiesto,
  encola los 16 ficheros como descargas ESENCIALES (cuentan en la barra de
  instalación) y coloca cada uno en el App Group:
  `group.com.joseluissaorin.yappy/models/supertonic-3/<id>`.
- Claves del Info.plist de la app (en `project.yml`): `BAManifestURL`
  (el manifiesto iOS del espejo), `BAEssentialMaxInstallSize`,
  `BAMaxInstallSize`, `BAInitialDownloadRestrictions` con
  `BADownloadAllowance`, `BAEssentialDownloadAllowance` y el dominio
  permitido `modelos.yappy.joseluissaorin.com`.
- En iOS la RAÍZ del modelo es el App Group (`model::model_root`); una
  instalación vieja con las voces en el contenedor privado se muda con un
  rename la primera vez.
- Si el usuario abre la app antes de que acaben las esenciales,
  `Recursos.swift` las sube a primer plano (`yappy_ba_reanudar`) y publica
  el progreso a Rust, que lo emite como `model_download`: la tarjeta del
  loro comiendo no distingue de dónde vienen las voces.
- Probar: `backgroundassets-debug --simulate --app-install --app-bundle-id
  com.joseluissaorin.yappy --device-id <iPhone>` (solo con un iPhone
  conectado; el simulador no vale). En TestFlight funciona como en la
  App Store.

## 2. La variante fp16 (la mitad)

- Receta: `tooling/modelos/convertir-fp16.py <raíz-fp32> <raíz-fp16>`
  (`onnxconverter_common` con `keep_io_types=True`, sin `value_info`
  viejo, y los dos nodos Cast del estimador bloqueados en f32; sin eso
  ORT no carga el modelo).
- Tamaños: 2,1 + 18,6 + 128,5 + 50,8 = 200 MB frente a 398.
- Medido en este Mac (CPU EP): 17,5 s de audio en 4,4 s (fp16) frente a
  3,6 s (fp32), un 20 % más lento y aún 4× tiempo real. Gemini no
  distingue los dos clips (transcripción idéntica, mismas notas).
- `model::variante()`: fp16 en iOS y Android, fp32 en escritorio
  (x86 sin aritmética de media precisión). `YAPPY_VARIANTE_MODELO` lo
  fuerza para pruebas.

## 3. El espejo propio y las descargas en paralelo

- Bucket R2 `yappy-modelos` (cuenta de Cloudflare de la casa) con dominio
  `modelos.yappy.joseluissaorin.com`. Rutas:
  `supertonic-3/{fp16,fp32}/onnx/*.onnx`, `supertonic-3/onnx/*.json`,
  `supertonic-3/voice_styles/*.json`, y los manifiestos
  `manifiesto-ios.json` (= fp16), `manifiesto-fp16.json`,
  `manifiesto-fp32.json` (`scripts/manifiesto-modelos.mjs` los regenera
  leyendo los tamaños del propio bucket).
- `model::download_model`: HEAD de tamaños en paralelo, cuatro descargas a
  la vez (semáforo), reanudación por rangos del `.part`, espejo primero y
  Hugging Face de reserva (solo fp32; sin espejo, un teléfono cae a fp32
  del espejo y luego a HF).

## 4. Esconder la espera

- En el móvil la descarga arranca SOLA a los 600 ms de la primera apertura
  si la red es barata (`yappy_red_barata`: wifi o cable, sin ahorro de
  datos); si es cara, la tarjeta espera al toque del usuario.
- Las presentaciones de las diez voces en los 31 idiomas van EMPAQUETADAS
  (`resources/muestras/*.m4a`, AAC 40 kbps, 310 ficheros, 12 MB): tocar
  un cromo suena sin motor. Se regeneran con el `yappy-cli` y `afconvert`
  (frases de `sample_for_voice` en `commands.rs`). `muestra_empaquetada` en `commands.rs`; la cocina de muestras ya
  no cocina lo que viene empaquetado.
- Los cuentos: cualquier `.yappy` en `resources/cuentos/` entra en la
  biblioteca en la primera apertura (`importar_cuentos_empaquetados`), una
  vez por título. Pendiente de contenido: los cuentos de José Luis.
