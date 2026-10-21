# Descarga 14 libros de dominio público por idioma (en, es, fr, de, it, pt)
# del Proyecto Gutenberg (vía gutendex) a /tmp/frecuencias/libros. Son la
# materia de las listas de palabras frecuentes del verbalizador
# (crates/yappy-core/src/guion/frecuentes/). Quitar a mano los diccionarios
# (p. ej. pt-31552, «Novo dicionário»): sesgan la lista.
import json, urllib.request, os, sys, re, time
langs = ["en","es","fr","de","it","pt"]
out = "/tmp/frecuencias/libros"
for lang in langs:
    url = f"https://gutendex.com/books?languages={lang}&sort=popular"
    got = 0
    while url and got < 14:
        try:
            data = json.load(urllib.request.urlopen(urllib.request.Request(url, headers={"User-Agent":"Mozilla/5.0"}), timeout=60))
        except Exception as e:
            print("gutendex fail", lang, e); break
        for b in data["results"]:
            if b["languages"] != [lang]: continue
            fmts = b["formats"]
            t = next((v for k,v in fmts.items() if k.startswith("text/plain")), None)
            if not t: continue
            dest = f"{out}/{lang}-{b['id']}.txt"
            if not os.path.exists(dest):
                try:
                    raw = urllib.request.urlopen(urllib.request.Request(t, headers={"User-Agent":"Mozilla/5.0"}), timeout=60).read()
                    open(dest,"wb").write(raw)
                except Exception as e:
                    print("fail", t, e); continue
            got += 1
            print(lang, b["id"], b["title"][:50]); sys.stdout.flush()
            if got >= 14: break
        url = data.get("next")
