#!/usr/bin/env python3
"""Convierte los cuatro ONNX de Supertonic 3 a pesos fp16 (la mitad de
tamaño) manteniendo entradas y salidas en f32, para que el motor Rust no
cambie. Receta probada el 9-09-2026 (onnx 1.21, onnxconverter_common,
onnxruntime 1.23):

  - se borra el `value_info` viejo para que los tipos se reinfieran;
  - los Cast que el conversor añade para las entradas se ponen delante
    (orden topológico);
  - si ORT se queja de un nodo («Type Error … of node (X)»), ese nodo se
    bloquea en f32 y se repite: en el estimador hacen falta dos.

Uso: python3 tooling/modelos/convertir-fp16.py <raíz-fp32> <raíz-fp16>
"""
import os, re, shutil, sys, time, warnings
warnings.filterwarnings("ignore")
import onnx
from onnxconverter_common import float16
import onnxruntime as ort

src, dst = sys.argv[1], sys.argv[2]
os.makedirs(f"{dst}/onnx", exist_ok=True)
shutil.copytree(f"{src}/voice_styles", f"{dst}/voice_styles", dirs_exist_ok=True)
for aux in ("tts.json", "unicode_indexer.json"):
    shutil.copy(f"{src}/onnx/{aux}", f"{dst}/onnx/{aux}")

def convertir(name, bloqueados):
    m = onnx.load(f"{src}/onnx/{name}.onnx")
    del m.graph.value_info[:]
    m16 = float16.convert_float_to_float16(
        m, keep_io_types=True, disable_shape_infer=False, node_block_list=bloqueados or None)
    nodes = list(m16.graph.node)
    entradas = {x.name for x in m16.graph.input}
    casts = [n for n in nodes if n.op_type == "Cast" and any(i in entradas for i in n.input)]
    rest = [n for n in nodes if n not in casts]
    del m16.graph.node[:]
    m16.graph.node.extend(casts + rest)
    out = f"{dst}/onnx/{name}.onnx"
    onnx.save(m16, out)
    return out

for name in ("duration_predictor", "text_encoder", "vector_estimator", "vocoder"):
    t0, bloqueados = time.time(), []
    for _ in range(12):
        out = convertir(name, bloqueados)
        try:
            ort.InferenceSession(out, providers=["CPUExecutionProvider"])
            break
        except Exception as e:
            m = re.search(r"of node \(([^)]+)\)", str(e))
            if not m:
                raise
            bloqueados.append(m.group(1))
    a = os.path.getsize(f"{src}/onnx/{name}.onnx") / 1e6
    b = os.path.getsize(out) / 1e6
    print(f"{name}: {a:.1f} MB -> {b:.1f} MB, bloqueados={bloqueados} ({time.time()-t0:.0f}s)")
print("listo")
