# Cuenta las palabras de los libros descargados (solo el texto entre los
# marcadores START/END de Gutenberg) y escribe las N más frecuentes de cada
# idioma con su forma habitual («knew», «Haus»). Uso:
#   python3 lista.py 4000 && cp /tmp/frecuencias/out/*.txt \
#     ../../crates/yappy-core/src/guion/frecuentes/
import os,re,collections,glob,sys
N=int(sys.argv[1]) if len(sys.argv)>1 else 3000
for lang in ["en","es","fr","de","it","pt"]:
    cnt=collections.Counter(); formas=collections.defaultdict(collections.Counter)
    for f in glob.glob(f"/tmp/frecuencias/libros/{lang}-*.txt"):
        t=open(f,encoding="utf-8",errors="ignore").read().replace("\r\n","\n")
        a=t.find("*** START OF"); b=t.find("*** END OF")
        if a>=0: t=t[t.find("\n",a)+1:b if b>a else len(t)]
        for w in re.findall(r"[^\W\d_]+",t):
            if not (2<=len(w)<=12): continue
            k=w.lower()
            cnt[k]+=1
            if not (w.isupper() and len(w)>1):
                formas[k][w]+=1
    out=[]
    for k,c in cnt.most_common():
        if not formas[k]: continue
        # forma canónica: la más frecuente que no esté en versales; si es
        # Capitalizada solo por abrir frase y la minúscula existe, minúscula.
        f=formas[k]
        low=f.get(k,0); cap=f.get(k.capitalize(),0)
        canon = k if low*3 >= cap else max(f, key=f.get)
        out.append(canon)
        if len(out)>=N: break
    open(f"/tmp/frecuencias/out/{lang}.txt","w").write("\n".join(out)+"\n")
    print(lang, len(out), os.path.getsize(f"/tmp/frecuencias/out/{lang}.txt"), out[:15], [w for w in out if w.lower() in ("knew","nasa","fbi","bbc","html","us","it","who")])
