#!/usr/bin/env python3
"""Pasa un vídeo por Gemini con muestreo denso de fotogramas y le pide una crítica.

    critica-gemini.py <video.mp4> [--fps 5] [--modelo gemini-pro-latest] [--guion texto.md] [-o critica.md]

Gemini muestrea los vídeos a 1 fps por defecto; aquí se pide `fps` (hasta 10) con
`video_metadata`, para que vea el movimiento y no solo fotogramas sueltos. Con 4 min
a 5 fps son ~1200 fotogramas (~300k tokens): entra en gemini-pro-latest.
El audio del vídeo también entra (Gemini lo escucha), así que puede juzgar la sincronía.
"""
import argparse, os, pathlib, sys, time
from google import genai
from google.genai import types

key = os.environ.get("GEMINI_API_KEY") or next(
    l.split("=", 1)[1].strip() for l in (pathlib.Path.home() / ".claude/.secrets/gemini.env").read_text().splitlines() if l.startswith("GEMINI_API_KEY="))
client = genai.Client(api_key=key)

ap = argparse.ArgumentParser()
ap.add_argument("video"); ap.add_argument("--fps", type=float, default=5); ap.add_argument("--modelo", default="gemini-pro-latest")
ap.add_argument("--guion", help="texto del cuento/guion para que compare lo que ve con lo que debía pasar")
ap.add_argument("--prompt", help="fichero con el prompt; si no, el de crítica por defecto")
ap.add_argument("-o", "--out")
a = ap.parse_args()

print(f"subiendo {a.video}…", file=sys.stderr)
f = client.files.upload(file=a.video)
while f.state.name == "PROCESSING":
    time.sleep(4); f = client.files.get(name=f.name)
if f.state.name != "ACTIVE":
    sys.exit(f"el fichero quedó en {f.state.name}")
print(f"listo: {f.uri} · modelo {a.modelo} · {a.fps} fps", file=sys.stderr)

guion = pathlib.Path(a.guion).read_text() if a.guion else ""
prompt = pathlib.Path(a.prompt).read_text() if a.prompt else f"""Eres un director de animación y montador exigente, con oficio en cortometraje de animación de recortes (cut-out, teatro de papel) y en vídeo-ensayo. Vas a criticar este vídeo con dureza y precisión, como en una sala de visionado antes del máster. No elogies por cortesía: cada elogio cuesta; cada defecto, con su timestamp exacto (mm:ss) y una propuesta concreta de arreglo.

Contexto: es un cuento corto («Media voz») que va dentro de un vídeo de lanzamiento de una app de lectura en voz alta. La voz que lee es sintética (la de la app) y eso es deliberado. Estética buscada: papel crema, recortes con sombra dura, dos tintas (coral y ultramar) y una tercera, el verde, que solo aparece cuando habla la voz del libro (los versos de Lorca). La línea «hierve» a propósito. En ningún plano debe aparecer la app ni decirse su nombre.

{('TEXTO DEL CUENTO (lo que se oye):\n' + guion) if guion else ''}

Analiza, con timestamps, en este orden:
1. LEGIBILIDAD: ¿se entiende en cada momento qué pasa y quién es quién? ¿Los subtítulos son legibles, están bien cortados, van a tiempo con la voz? ¿Hay texto cortado, tapado o fuera de cuadro?
2. RITMO Y SINCRONÍA: ¿la imagen cambia cuando la voz lo pide? ¿Hay planos que se quedan quietos demasiado tiempo o cortes que llegan tarde/pronto? ¿Los versos de Lorca (verde) entran y salen bien?
3. PUESTA EN ESCENA: composición, escala de los personajes, dirección de miradas, dónde está el cable, dónde el móvil, la relación entre las dos chicas, la señora, la ventana. ¿Algo está mal colocado, superpuesto o flotando?
4. ANIMACIÓN: movimientos que resultan mecánicos, tics, cosas que vibran cuando no deben o no vibran cuando deben (el motor del autobús se apaga en un momento dado), transiciones bruscas, fotogramas raros.
5. EMOCIÓN: ¿el final (ella dormida, «El cable tiraba un poco cada vez que respiraba») llega? ¿Qué lo estorba?
6. ERRORES: cualquier cosa rota (glifos, textos solapados, elementos que aparecen de golpe, colores fuera de paleta, algo que parezca un bug).

Termina con: (a) las CINCO correcciones más importantes, ordenadas por impacto, cada una con timestamp y una frase de cómo hacerla; (b) una nota de 1 a 10 con una línea de justificación; (c) tres cosas que NO hay que tocar porque funcionan.
Responde en español, con tildes."""

resp = client.models.generate_content(
    model=a.modelo,
    contents=types.Content(parts=[
        types.Part(file_data=types.FileData(file_uri=f.uri, mime_type=f.mime_type), video_metadata=types.VideoMetadata(fps=a.fps)),
        types.Part(text=prompt),
    ]),
    config=types.GenerateContentConfig(temperature=0.6, media_resolution="MEDIA_RESOLUTION_MEDIUM"),
)
texto = resp.text
if resp.usage_metadata:
    print(f"tokens: entrada {resp.usage_metadata.prompt_token_count} · salida {resp.usage_metadata.candidates_token_count}", file=sys.stderr)
if a.out:
    pathlib.Path(a.out).write_text(texto); print(f"✓ {a.out}", file=sys.stderr)
else:
    print(texto)
