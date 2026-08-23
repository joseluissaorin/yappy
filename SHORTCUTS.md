# Yappy y los atajos del sistema

## iPhone: App Intents (lo nuevo, sin recetas)

Yappy expone App Intents de verdad: aparecen SOLOS en la app **Atajos**, en
**Siri** y se pueden asignar al **botón de acción**. No hay nada que montar.

- **Lee mi portapapeles** («Oye Siri, lee mi portapapeles con Yappy»)
- **Reanuda la lectura** («Reanuda la lectura en Yappy»)
- **Escuchar un enlace** (acepta una URL como parámetro: perfecta para
  automatizaciones de Atajos que terminan en «…y que Yappy me lo lea»)

## iPhone: deep links `yappy://`

Para automatizaciones a mano, el esquema sigue disponible y AHORA se
procesa de verdad:

| URL | Qué hace |
|---|---|
| `yappy://action/read-clipboard` | lee el portapapeles ya |
| `yappy://action/resume` | reanuda (o alterna) la reproducción |
| `yappy://action/open` | abre el selector de documento |
| `yappy://library?path=…` | reproduce ese audiolibro |
| `yappy://pair?d=…` | empareja con un ordenador (el puente) |

## Escritorio: atajos globales

Configurables en Preferencias; por defecto:

| Acción | macOS | Windows/Linux |
|---|---|---|
| Leer lo que estoy mirando | ⌥⌘R | Ctrl+Alt+R |
| Pausar / reanudar | ⌥⌘Espacio | Ctrl+Alt+Espacio |
| Leer el portapapeles | ⌥⌘V | Ctrl+Alt+V |
