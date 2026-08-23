#!/usr/bin/env python3
"""Vendoriza las reglas RBNF de CLDR para los idiomas de Supertonic.

Descarga common/rbnf/<locale>.xml del repo de Unicode (release fijada para
reproducibilidad) y las convierte al formato de texto plano de ICU, que es
el que interpreta crates/yappy-core/src/guion/rbnf.rs:

    %spellout-cardinal:
    0: cero;
    1: uno;
    ...

Correr cuando se quiera subir de versión de CLDR; el resultado se commitea.
"""

import os
import sys
import urllib.request
import xml.etree.ElementTree as ET

RELEASE = "release-46"
IDIOMAS = [
    "en", "ko", "ja", "ar", "bg", "cs", "da", "de", "el", "es", "et", "fi",
    "fr", "hi", "hr", "hu", "id", "it", "lt", "lv", "nl", "pl", "pt", "ro",
    "ru", "sk", "sl", "sv", "tr", "uk", "vi",
    # raíz: reglas de respaldo (dígitos) para lo que falte
    "root",
]
DESTINO = os.path.join(os.path.dirname(__file__), "..", "..",
                       "crates", "yappy-core", "data", "rbnf")


def bajar(locale: str) -> str:
    url = (f"https://raw.githubusercontent.com/unicode-org/cldr/{RELEASE}"
           f"/common/rbnf/{locale}.xml")
    with urllib.request.urlopen(url, timeout=30) as r:
        return r.read().decode("utf-8")


def convertir(xml_texto: str) -> str:
    arbol = ET.fromstring(xml_texto)
    lineas = []
    for grupo in arbol.iter("rulesetGrouping"):
        tipo = grupo.get("type")
        if tipo not in ("SpelloutRules", "OrdinalRules"):
            continue
        for ruleset in grupo.iter("ruleset"):
            nombre = ruleset.get("type")
            acceso = ruleset.get("access", "public")
            prefijo = "%%" if acceso == "private" else "%"
            lineas.append(f"{prefijo}{nombre}:")
            for regla in ruleset.iter("rbnfrule"):
                valor = regla.get("value")
                radix = regla.get("radix")
                clave = f"{valor}/{radix}" if radix else valor
                cuerpo = (regla.text or "").strip()
                if not cuerpo.endswith(";"):
                    cuerpo += ";"
                lineas.append(f"{clave}: {cuerpo}")
    return "\n".join(lineas) + "\n"


def main() -> None:
    os.makedirs(DESTINO, exist_ok=True)
    for loc in IDIOMAS:
        try:
            texto = convertir(bajar(loc))
        except Exception as e:  # noqa: BLE001
            print(f"  ✗ {loc}: {e}", file=sys.stderr)
            continue
        ruta = os.path.join(DESTINO, f"{loc}.txt")
        with open(ruta, "w", encoding="utf-8") as f:
            f.write(texto)
        print(f"  ✓ {loc} ({len(texto)} bytes)")


if __name__ == "__main__":
    main()
