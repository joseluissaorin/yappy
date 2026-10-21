//! DOCX, ODT y RTF → párrafos de lectura, en Rust puro (igual en el Mac, en
//! iOS y en Android: el progreso se guarda por índice de párrafo).
//!
//! Antes el Mac usaba pandoc (`-t plain`) y el móvil anytomd/strip: el mismo
//! documento daba 818 párrafos en uno y 403 en otro, las tablas salían
//! dibujadas con «+---+|» y se leían, el ODT del móvil era UN párrafo y el
//! RTF perdía las tildes de `\u233`. Aquí se lee la estructura del propio
//! formato: estilos de título, listas, citas, notas al pie (al final, tras
//! una pausa) y tablas (las de datos fila a fila; las de maqueta, celda a
//! celda, con las estrofas juntas).

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use quick_xml::events::Event;
use quick_xml::Reader;

use super::doc_loader::{rich, RichParagraph};

// ───────────────────────────── modelo común ─────────────────────────────

/// Un párrafo del documento fuente. `texto` puede llevar '\n' (saltos de
/// línea manuales dentro del párrafo).
#[derive(Debug, Clone)]
pub(crate) struct Parr {
    pub clase: &'static str,
    pub texto: String,
}

#[derive(Debug, Clone)]
pub(crate) enum Nodo {
    P(Parr),
    /// filas → celdas → nodos
    Tabla(Vec<Vec<Vec<Nodo>>>),
}

/// Nodos → párrafos de lectura (+ notas al final).
pub(crate) fn ensamblar(nodos: &[Nodo], notas: &[String]) -> Vec<RichParagraph> {
    let mut out = Vec::new();
    volcar(nodos, &mut out, false);
    let notas: Vec<String> = notas
        .iter()
        .map(|n| compactar(n))
        .filter(|n| !n.is_empty())
        .collect();
    if !notas.is_empty() {
        out.push(rich(String::new(), "hr"));
        for n in notas {
            out.push(rich(n, "paragraph"));
        }
    }
    // Ni separadores al principio, ni dos seguidos, ni al final.
    let mut limpia: Vec<RichParagraph> = Vec::with_capacity(out.len());
    for p in out {
        if p.kind == "hr" && limpia.last().is_none_or(|u| u.kind == "hr") {
            continue;
        }
        limpia.push(p);
    }
    while limpia.last().is_some_and(|u| u.kind == "hr") {
        limpia.pop();
    }
    limpia
}

fn compactar(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Un párrafo con saltos de línea: si las líneas son prosa larga, cada una
/// es un párrafo; si son cortas (versos, direcciones), van juntas.
fn emitir_parr(p: &Parr, out: &mut Vec<RichParagraph>) {
    let lineas: Vec<String> = p
        .texto
        .split('\n')
        .map(compactar)
        .filter(|l| !l.is_empty())
        .collect();
    if lineas.is_empty() {
        return;
    }
    let media = lineas.iter().map(|l| l.chars().count()).sum::<usize>() / lineas.len();
    if lineas.len() > 1 && media >= 120 && !p.clase.starts_with("heading") {
        for l in lineas {
            out.push(rich(l, p.clase));
        }
    } else {
        let texto = lineas.join(" ");
        let clase = if p.clase.starts_with("heading") && texto.chars().count() > 300 {
            "paragraph"
        } else {
            p.clase
        };
        out.push(rich(texto, clase));
    }
}

fn texto_plano(nodos: &[Nodo]) -> String {
    let mut s = String::new();
    for n in nodos {
        match n {
            Nodo::P(p) => {
                if !s.is_empty() {
                    s.push(' ');
                }
                s.push_str(&compactar(&p.texto));
            }
            Nodo::Tabla(filas) => {
                for f in filas {
                    for c in f {
                        let t = texto_plano(c);
                        if !t.is_empty() {
                            if !s.is_empty() {
                                s.push(' ');
                            }
                            s.push_str(&t);
                        }
                    }
                }
            }
        }
    }
    s.trim().to_string()
}

fn volcar(nodos: &[Nodo], out: &mut Vec<RichParagraph>, en_celda: bool) {
    let mut i = 0;
    while i < nodos.len() {
        match &nodos[i] {
            Nodo::P(p) => {
                // Versos: párrafos cortos seguidos (un poema o una obra de
                // teatro escritos verso a verso, o la tabla de maqueta de
                // una edición) se leen por estrofas; un párrafo vacío separa
                // estrofas. Fuera de las tablas se exige además que algún
                // verso empiece en minúscula (la frase sigue de un verso a
                // otro): una lista de rótulos cortos no es un poema.
                if p.clase == "paragraph" && es_verso(p) {
                    let mut j = i;
                    let mut versos: Vec<String> = Vec::new();
                    while j < nodos.len() {
                        match &nodos[j] {
                            Nodo::P(q) if q.clase == "paragraph" && es_verso(q) => {
                                versos.push(compactar(&q.texto));
                                j += 1;
                            }
                            _ => break,
                        }
                    }
                    let encabalgado = versos.iter().skip(1).any(|v| {
                        v.chars()
                            .find(|c| c.is_alphabetic())
                            .is_some_and(|c| c.is_lowercase())
                    });
                    if versos.len() >= 2 && (en_celda || encabalgado) {
                        // Estrofas largas sin línea en blanco: se cortan en
                        // un final de frase cada diez versos o más.
                        let mut estrofa: Vec<String> = Vec::new();
                        for v in versos {
                            let cierra = v.ends_with(['.', '!', '?', '…', ';']);
                            estrofa.push(v);
                            if estrofa.len() >= 10 && cierra {
                                out.push(rich(std::mem::take(&mut estrofa).join(" "), "paragraph"));
                            }
                        }
                        if !estrofa.is_empty() {
                            out.push(rich(estrofa.join(" "), "paragraph"));
                        }
                        i = j;
                        continue;
                    }
                }
                emitir_parr(p, out);
            }
            Nodo::Tabla(filas) => volcar_tabla(filas, out),
        }
        i += 1;
    }
}

fn es_verso(p: &Parr) -> bool {
    let t = compactar(&p.texto);
    let n = t.chars().count();
    n > 0 && n <= 80 && !p.texto.contains('\n') && !es_rotulo(&t)
}

/// El nombre de un personaje en una obra de teatro («NIÑO 1», «SATANÁS»):
/// todo en mayúsculas. No es verso: abre una réplica.
fn es_rotulo(t: &str) -> bool {
    let letras: Vec<char> = t.chars().filter(|c| c.is_alphabetic()).collect();
    letras.len() >= 2 && letras.iter().all(|c| c.is_uppercase()) && t.chars().count() <= 40
}

fn volcar_tabla(filas: &[Vec<Vec<Nodo>>], out: &mut Vec<RichParagraph>) {
    // ¿Tabla de datos? Varias filas con dos o más celdas llenas y todas las
    // celdas cortas y de un solo párrafo.
    let mut filas_multiples = 0;
    let mut todo_corto = true;
    for f in filas {
        let llenas: Vec<String> = f
            .iter()
            .map(|c| texto_plano(c))
            .filter(|t| !t.is_empty())
            .collect();
        if llenas.len() >= 2 {
            filas_multiples += 1;
        }
        for c in f {
            let parrs = c
                .iter()
                .filter(|n| !matches!(n, Nodo::P(p) if p.texto.trim().is_empty()))
                .count();
            if parrs > 1 || texto_plano(c).chars().count() > 160 {
                todo_corto = false;
            }
        }
    }
    if filas_multiples >= 1 && todo_corto {
        for f in filas {
            let celdas: Vec<String> = f
                .iter()
                .map(|c| texto_plano(c))
                .filter(|t| !t.is_empty())
                .collect();
            if celdas.is_empty() {
                continue;
            }
            let mut fila = celdas.join(", ");
            if !fila.ends_with(['.', '!', '?', ':', ';']) {
                fila.push('.');
            }
            out.push(rich(fila, "paragraph"));
        }
    } else {
        // Tabla de maqueta (la edición de un poema o de una obra de teatro,
        // a veces un verso por fila): celda a celda en orden de lectura, las
        // celdas vacías fuera y las filas vacías como separación de estrofa;
        // así los versos se agrupan aunque cada uno viva en su fila.
        let mut plano: Vec<Nodo> = Vec::new();
        for f in filas {
            let llenas: Vec<&Vec<Nodo>> = f.iter().filter(|c| !texto_plano(c).is_empty()).collect();
            if llenas.is_empty() {
                plano.push(Nodo::P(Parr {
                    clase: "paragraph",
                    texto: String::new(),
                }));
                continue;
            }
            for c in llenas {
                plano.extend(c.iter().cloned());
            }
        }
        volcar(&plano, out, true);
    }
}

// ───────────────────────────── utilidades XML ─────────────────────────────

fn leer_zip(path: &Path, nombre: &str) -> Result<Option<String>> {
    let f = std::fs::File::open(path).with_context(|| format!("abrir {}", path.display()))?;
    let mut z = zip::ZipArchive::new(f).context("no es un ZIP válido")?;
    let mut entrada = match z.by_name(nombre) {
        Ok(e) => e,
        Err(_) => return Ok(None),
    };
    let mut bytes = Vec::new();
    entrada.read_to_end(&mut bytes)?;
    Ok(Some(String::from_utf8_lossy(&bytes).into_owned()))
}

fn atributo(e: &quick_xml::events::BytesStart, nombre: &[u8]) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == nombre)
        .map(|a| {
            a.unescape_value()
                .map(|v| v.into_owned())
                .unwrap_or_else(|_| String::from_utf8_lossy(&a.value).into_owned())
        })
}

fn texto_evento(t: &quick_xml::events::BytesText) -> String {
    t.unescape()
        .map(|c| c.into_owned())
        .unwrap_or_else(|_| String::from_utf8_lossy(t.as_ref()).into_owned())
}

/// Constructor de árbol compartido por DOCX y ODT: pila de tablas/filas/
/// celdas y el párrafo en curso.
/// (filas, fila en curso, celda en curso)
type TablaAbierta = (Vec<Vec<Vec<Nodo>>>, Vec<Vec<Nodo>>, Vec<Nodo>);

#[derive(Default)]
struct Arbol {
    raiz: Vec<Nodo>,
    /// Pila de tablas abiertas: (filas, fila en curso, celda en curso)
    tablas: Vec<TablaAbierta>,
}

impl Arbol {
    fn destino(&mut self) -> &mut Vec<Nodo> {
        match self.tablas.last_mut() {
            Some((_, _, celda)) => celda,
            None => &mut self.raiz,
        }
    }
    fn parrafo(&mut self, clase: &'static str, texto: String) {
        self.destino().push(Nodo::P(Parr { clase, texto }));
    }
    fn abrir_tabla(&mut self) {
        self.tablas.push((Vec::new(), Vec::new(), Vec::new()));
    }
    fn cerrar_celda(&mut self) {
        if let Some((_, fila, celda)) = self.tablas.last_mut() {
            fila.push(std::mem::take(celda));
        }
    }
    fn cerrar_fila(&mut self) {
        if let Some((filas, fila, _)) = self.tablas.last_mut() {
            filas.push(std::mem::take(fila));
        }
    }
    fn cerrar_tabla(&mut self) {
        if let Some((filas, _, _)) = self.tablas.pop() {
            self.destino().push(Nodo::Tabla(filas));
        }
    }
}

// ───────────────────────────── DOCX ─────────────────────────────

struct EstiloDocx {
    nombre: String,
    nivel: Option<u8>,
    lista: bool,
    base: Option<String>,
}

fn estilos_docx(xml: &str) -> HashMap<String, EstiloDocx> {
    let mut m = HashMap::new();
    let mut r = Reader::from_str(xml);
    let mut actual: Option<(String, EstiloDocx)> = None;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => match e.name().as_ref() {
                b"w:style" => {
                    let id = atributo(&e, b"w:styleId").unwrap_or_default();
                    actual = Some((
                        id,
                        EstiloDocx {
                            nombre: String::new(),
                            nivel: None,
                            lista: false,
                            base: None,
                        },
                    ));
                }
                b"w:name" => {
                    if let Some((_, s)) = actual.as_mut() {
                        s.nombre = atributo(&e, b"w:val").unwrap_or_default().to_lowercase();
                    }
                }
                b"w:basedOn" => {
                    if let Some((_, s)) = actual.as_mut() {
                        s.base = atributo(&e, b"w:val");
                    }
                }
                b"w:outlineLvl" => {
                    if let Some((_, s)) = actual.as_mut() {
                        s.nivel = atributo(&e, b"w:val").and_then(|v| v.parse::<u8>().ok());
                    }
                }
                b"w:numPr" => {
                    if let Some((_, s)) = actual.as_mut() {
                        s.lista = true;
                    }
                }
                _ => {}
            },
            Ok(Event::End(e)) if e.name().as_ref() == b"w:style" => {
                if let Some((id, s)) = actual.take() {
                    m.insert(id, s);
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    m
}

/// Clase de lectura de un estilo de párrafo (siguiendo la herencia).
fn clase_de_estilo(id: &str, estilos: &HashMap<String, EstiloDocx>) -> &'static str {
    let mut actual = Some(id.to_string());
    for _ in 0..8 {
        let Some(i) = actual else { break };
        let Some(e) = estilos.get(&i) else { break };
        let n = e.nombre.as_str();
        // Las entradas del índice («toc 1», «tdc 1») no se leen: son la
        // lista de títulos con sus números de página.
        if n.starts_with("toc ") || n.starts_with("tdc ") || n == "toc heading" {
            return "toc";
        }
        if n == "title" || n == "título" || n == "titre" {
            return "heading1";
        }
        if n == "subtitle" || n == "subtítulo" {
            return "heading2";
        }
        if let Some(nv) = e.nivel {
            if nv < 9 {
                return nivel_a_clase(nv + 1);
            }
        }
        for prefijo in [
            "heading ",
            "título ",
            "titre ",
            "überschrift ",
            "titolo ",
            "kop ",
        ] {
            if let Some(resto) = n.strip_prefix(prefijo) {
                if let Ok(nv) = resto.trim().parse::<u8>() {
                    return nivel_a_clase(nv);
                }
            }
        }
        if n.contains("quote")
            || n.contains("cita")
            || n.contains("citation")
            || n.contains("zitat")
        {
            return "quote";
        }
        if e.lista || n.starts_with("list") || n.starts_with("lista") {
            return "list";
        }
        actual = e.base.clone();
    }
    "paragraph"
}

fn nivel_a_clase(n: u8) -> &'static str {
    match n {
        0 | 1 => "heading1",
        2 => "heading2",
        3 => "heading3",
        4 => "heading4",
        5 => "heading5",
        _ => "heading6",
    }
}

/// Las notas al pie (o finales) de un DOCX: id → texto.
fn notas_docx(xml: &str, etiqueta: &[u8]) -> HashMap<String, String> {
    let mut m = HashMap::new();
    let mut r = Reader::from_str(xml);
    let mut id: Option<String> = None;
    let mut buf = String::new();
    let mut en_t = false;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                if e.name().as_ref() == etiqueta {
                    id = atributo(&e, b"w:id");
                    buf.clear();
                } else if e.name().as_ref() == b"w:t" {
                    en_t = true;
                } else if e.name().as_ref() == b"w:p" && !buf.is_empty() {
                    buf.push(' ');
                }
            }
            Ok(Event::Empty(e)) if e.name().as_ref() == b"w:tab" => buf.push(' '),
            Ok(Event::Text(t)) if en_t => buf.push_str(&texto_evento(&t)),
            Ok(Event::End(e)) => {
                if e.name().as_ref() == b"w:t" {
                    en_t = false;
                } else if e.name().as_ref() == etiqueta {
                    if let Some(i) = id.take() {
                        m.insert(i, compactar(&buf));
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    m
}

pub fn docx(path: &Path) -> Result<Vec<RichParagraph>> {
    let doc = leer_zip(path, "word/document.xml")?
        .ok_or_else(|| anyhow!("DOCX sin word/document.xml"))?;
    let estilos = leer_zip(path, "word/styles.xml")?
        .map(|x| estilos_docx(&x))
        .unwrap_or_default();
    let al_pie = leer_zip(path, "word/footnotes.xml")?
        .map(|x| notas_docx(&x, b"w:footnote"))
        .unwrap_or_default();
    let finales = leer_zip(path, "word/endnotes.xml")?
        .map(|x| notas_docx(&x, b"w:endnote"))
        .unwrap_or_default();

    let mut arbol = Arbol::default();
    let mut notas: Vec<String> = Vec::new();
    let mut r = Reader::from_str(&doc);
    // Estado del párrafo en curso (pila: los cuadros de texto anidan párrafos).
    struct EstadoP {
        texto: String,
        clase: &'static str,
        lista: bool,
    }
    let mut pila: Vec<EstadoP> = Vec::new();
    let mut en_t = false;
    let mut saltar = 0usize; // profundidad dentro de algo que no se lee
    let mut oculto_run = false;
    let mut en_rpr = false;
    loop {
        let ev = match r.read_event() {
            Ok(e) => e,
            Err(e) => return Err(anyhow!("DOCX mal formado: {e}")),
        };
        match ev {
            Event::Start(e) => {
                let nombre = e.name();
                let n = nombre.as_ref();
                if saltar > 0 {
                    saltar += 1;
                    continue;
                }
                match n {
                    // Lo que no se lee: tachado de control de cambios, el
                    // respaldo duplicado de los cuadros de texto, códigos de
                    // campo, comentarios.
                    b"w:del"
                    | b"w:moveFrom"
                    | b"mc:Fallback"
                    | b"w:instrText"
                    | b"w:delText"
                    | b"w:commentReference"
                    | b"w:pPrChange"
                    | b"w:rPrChange" => saltar = 1,
                    b"w:p" => pila.push(EstadoP {
                        texto: String::new(),
                        clase: "paragraph",
                        lista: false,
                    }),
                    b"w:t" => en_t = true,
                    b"w:tbl" => arbol.abrir_tabla(),
                    b"w:rPr" => en_rpr = true,
                    b"w:r" => oculto_run = false,
                    _ => {}
                }
            }
            Event::Empty(e) => {
                if saltar > 0 {
                    continue;
                }
                let nombre = e.name();
                match nombre.as_ref() {
                    b"w:pStyle" => {
                        if let (Some(p), Some(id)) = (pila.last_mut(), atributo(&e, b"w:val")) {
                            let c = clase_de_estilo(&id, &estilos);
                            p.clase = c;
                        }
                    }
                    b"w:outlineLvl" => {
                        if let (Some(p), Some(v)) = (pila.last_mut(), atributo(&e, b"w:val")) {
                            if let Ok(nv) = v.parse::<u8>() {
                                if nv < 9 {
                                    p.clase = nivel_a_clase(nv + 1);
                                }
                            }
                        }
                    }
                    b"w:numPr" => {
                        if let Some(p) = pila.last_mut() {
                            p.lista = true;
                        }
                    }
                    b"w:vanish" if en_rpr => {
                        let val = atributo(&e, b"w:val").unwrap_or_default();
                        oculto_run = !(val == "0" || val == "false");
                    }
                    b"w:tab" | b"w:ptab" => {
                        if let Some(p) = pila.last_mut() {
                            p.texto.push(' ');
                        }
                    }
                    b"w:br" | b"w:cr" => {
                        let tipo = atributo(&e, b"w:type").unwrap_or_default();
                        if tipo != "page" && tipo != "column" {
                            if let Some(p) = pila.last_mut() {
                                p.texto.push('\n');
                            }
                        }
                    }
                    b"w:noBreakHyphen" => {
                        if let Some(p) = pila.last_mut() {
                            p.texto.push('-');
                        }
                    }
                    b"w:footnoteReference" | b"w:endnoteReference" => {
                        let id = atributo(&e, b"w:id").unwrap_or_default();
                        let fuente = if nombre.as_ref() == b"w:footnoteReference" {
                            &al_pie
                        } else {
                            &finales
                        };
                        if let Some(t) = fuente.get(&id) {
                            notas.push(t.clone());
                        }
                    }
                    b"w:p" => arbol.parrafo("paragraph", String::new()),
                    _ => {}
                }
            }
            Event::Text(t) if saltar == 0 && en_t && !oculto_run => {
                if let Some(p) = pila.last_mut() {
                    p.texto.push_str(&texto_evento(&t));
                }
            }
            Event::End(e) => {
                if saltar > 0 {
                    saltar -= 1;
                    continue;
                }
                match e.name().as_ref() {
                    b"w:t" => en_t = false,
                    b"w:rPr" => en_rpr = false,
                    b"w:p" => {
                        if let Some(p) = pila.pop() {
                            let clase = if p.lista && p.clase == "paragraph" {
                                "list"
                            } else {
                                p.clase
                            };
                            if clase != "toc" {
                                arbol.parrafo(clase, p.texto);
                            }
                        }
                    }
                    b"w:tc" => arbol.cerrar_celda(),
                    b"w:tr" => arbol.cerrar_fila(),
                    b"w:tbl" => arbol.cerrar_tabla(),
                    _ => {}
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    while !arbol.tablas.is_empty() {
        arbol.cerrar_celda();
        arbol.cerrar_fila();
        arbol.cerrar_tabla();
    }
    Ok(ensamblar(&arbol.raiz, &notas))
}

// ───────────────────────────── ODT ─────────────────────────────

pub fn odt(path: &Path) -> Result<Vec<RichParagraph>> {
    let xml = leer_zip(path, "content.xml")?.ok_or_else(|| anyhow!(".odt sin content.xml"))?;
    // Estilos con nombre (styles.xml) y automáticos (content.xml): nombre →
    // padre, para saber si un P7 es en realidad una «Quotations».
    let mut padres: HashMap<String, String> = HashMap::new();
    for fuente in [
        leer_zip(path, "styles.xml")?.unwrap_or_default(),
        xml.clone(),
    ] {
        let mut r = Reader::from_str(&fuente);
        loop {
            match r.read_event() {
                Ok(Event::Start(e)) | Ok(Event::Empty(e))
                    if e.name().as_ref() == b"style:style" =>
                {
                    if let (Some(n), Some(p)) = (
                        atributo(&e, b"style:name"),
                        atributo(&e, b"style:parent-style-name"),
                    ) {
                        padres.insert(n, p);
                    }
                }
                Ok(Event::Eof) | Err(_) => break,
                _ => {}
            }
        }
    }
    let clase_odt = |estilo: &str| -> &'static str {
        let mut n = estilo.to_string();
        for _ in 0..8 {
            let l = n.to_lowercase().replace("_20_", " ");
            if l.contains("quotation") || l.contains("cita") || l.contains("zitat") {
                return "quote";
            }
            if l == "title" || l == "título" {
                return "heading1";
            }
            if l == "subtitle" || l == "subtítulo" {
                return "heading2";
            }
            match padres.get(&n) {
                Some(p) => n = p.clone(),
                None => break,
            }
        }
        "paragraph"
    };

    let mut arbol = Arbol::default();
    let mut notas: Vec<String> = Vec::new();
    let mut r = Reader::from_str(&xml);
    struct EstadoP {
        texto: String,
        clase: &'static str,
    }
    let mut pila: Vec<EstadoP> = Vec::new();
    let mut saltar = 0usize;
    let mut en_cuerpo_nota = 0usize;
    let mut nota_actual = String::new();
    let mut en_lista = 0usize;
    let mut en_cuerpo = false;
    loop {
        let ev = match r.read_event() {
            Ok(e) => e,
            Err(e) => return Err(anyhow!("ODT mal formado: {e}")),
        };
        match ev {
            Event::Start(e) => {
                if saltar > 0 {
                    saltar += 1;
                    continue;
                }
                let nombre = e.name();
                match nombre.as_ref() {
                    b"office:body" => en_cuerpo = true,
                    b"office:annotation"
                    | b"text:tracked-changes"
                    | b"text:note-citation"
                    | b"office:automatic-styles"
                    | b"text:table-of-content-source"
                    | b"text:index-title-template"
                    | b"svg:desc"
                    | b"svg:title" => saltar = 1,
                    b"text:note-body" => {
                        en_cuerpo_nota += 1;
                        nota_actual.clear();
                    }
                    b"text:p" | b"text:h" if en_cuerpo_nota == 0 => {
                        let estilo = atributo(&e, b"text:style-name").unwrap_or_default();
                        let clase = if nombre.as_ref() == b"text:h" {
                            let nv = atributo(&e, b"text:outline-level")
                                .and_then(|v| v.parse::<u8>().ok())
                                .unwrap_or(1);
                            nivel_a_clase(nv)
                        } else {
                            match clase_odt(&estilo) {
                                "paragraph" if en_lista > 0 => "list",
                                c => c,
                            }
                        };
                        pila.push(EstadoP {
                            texto: String::new(),
                            clase,
                        });
                    }
                    b"text:p" | b"text:h" if !nota_actual.is_empty() => {
                        nota_actual.push(' ');
                    }
                    b"text:list-item" => en_lista += 1,
                    b"table:table" => arbol.abrir_tabla(),
                    _ => {}
                }
            }
            Event::Empty(e) => {
                if saltar > 0 {
                    continue;
                }
                let destino: Option<&mut String> = if en_cuerpo_nota > 0 {
                    Some(&mut nota_actual)
                } else {
                    pila.last_mut().map(|p| &mut p.texto)
                };
                let Some(d) = destino else { continue };
                match e.name().as_ref() {
                    b"text:s" => {
                        let n = atributo(&e, b"text:c")
                            .and_then(|v| v.parse::<usize>().ok())
                            .unwrap_or(1);
                        d.push_str(&" ".repeat(n.min(4)));
                    }
                    b"text:tab" => d.push(' '),
                    b"text:line-break" => d.push('\n'),
                    _ => {}
                }
            }
            Event::Text(t) => {
                if saltar > 0 || !en_cuerpo {
                    continue;
                }
                let s = texto_evento(&t);
                if en_cuerpo_nota > 0 {
                    nota_actual.push_str(&s);
                } else if let Some(p) = pila.last_mut() {
                    p.texto.push_str(&s);
                }
            }
            Event::End(e) => {
                if saltar > 0 {
                    saltar -= 1;
                    continue;
                }
                match e.name().as_ref() {
                    b"text:note-body" => {
                        en_cuerpo_nota = en_cuerpo_nota.saturating_sub(1);
                        notas.push(std::mem::take(&mut nota_actual));
                    }
                    b"text:p" | b"text:h" if en_cuerpo_nota == 0 => {
                        if let Some(p) = pila.pop() {
                            arbol.parrafo(p.clase, p.texto);
                        }
                    }
                    b"text:list-item" => en_lista = en_lista.saturating_sub(1),
                    b"table:table-cell" | b"table:covered-table-cell" => arbol.cerrar_celda(),
                    b"table:table-row" => arbol.cerrar_fila(),
                    b"table:table" => arbol.cerrar_tabla(),
                    _ => {}
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    while !arbol.tablas.is_empty() {
        arbol.cerrar_celda();
        arbol.cerrar_fila();
        arbol.cerrar_tabla();
    }
    Ok(ensamblar(&arbol.raiz, &notas))
}

// ───────────────────────────── RTF ─────────────────────────────

/// Destinos RTF cuyo contenido no es texto del documento.
const DESTINOS_MUDOS: &[&str] = &[
    "fonttbl",
    "colortbl",
    "stylesheet",
    "info",
    "pict",
    "object",
    "header",
    "headerl",
    "headerr",
    "headerf",
    "footer",
    "footerl",
    "footerr",
    "footerf",
    "fldinst",
    "themedata",
    "colorschememapping",
    "datastore",
    "latentstyles",
    "listtable",
    "listoverridetable",
    "rsidtbl",
    "generator",
    "xmlnstbl",
    "mmathPr",
    "pgdsctbl",
    "revtbl",
    "filetbl",
    "bkmkstart",
    "bkmkend",
    "annotation",
    "atnid",
    "atnauthor",
    "atndate",
    "nonshppict",
    "shp",
    "shpinst",
    "sp",
    "xe",
    "tc",
    "expandedcolortbl",
    "listtext",
    "pntext",
    "pntxta",
    "pntxtb",
    "template",
    "userprops",
    "docvar",
    "wgrffmtfilter",
    "passwordhash",
    "blipuid",
];

#[allow(unused_assignments)] // el contador de \uc se decrementa dentro de las macros
pub fn rtf(bytes: &[u8]) -> Result<Vec<RichParagraph>> {
    if !bytes.starts_with(b"{\\rtf") {
        return Err(anyhow!("no es RTF"));
    }
    #[derive(Clone)]
    struct Grupo {
        mudo: bool,
        uc: usize,
        oculto: bool,
        nota: bool,
    }
    let mut pila: Vec<Grupo> = vec![Grupo {
        mudo: false,
        uc: 1,
        oculto: false,
        nota: false,
    }];
    let mut codificacion: &'static encoding_rs::Encoding = encoding_rs::WINDOWS_1252;
    let mut pendientes: Vec<u8> = Vec::new();
    let mut parr = String::new();
    let mut clase: &'static str = "paragraph";
    let mut en_tabla = false;
    let mut nota = String::new();
    let mut notas: Vec<String> = Vec::new();
    let mut nodos: Vec<Nodo> = Vec::new();
    // Celdas de la fila en curso (párrafos de cada una) y filas de la tabla.
    let mut celda: Vec<Nodo> = Vec::new();
    let mut fila: Vec<Vec<Nodo>> = Vec::new();
    let mut filas: Vec<Vec<Vec<Nodo>>> = Vec::new();
    let mut saltar_chars = 0usize;
    let mut i = 0usize;
    let n = bytes.len();

    // Vuelca bytes \'hh pendientes con la página de códigos del documento.
    macro_rules! volcar_pendientes {
        () => {
            if !pendientes.is_empty() {
                let (txt, _, _) = codificacion.decode(&pendientes);
                let g = pila.last().unwrap();
                if !g.mudo && !g.oculto {
                    if g.nota {
                        nota.push_str(&txt);
                    } else {
                        parr.push_str(&txt);
                    }
                }
                pendientes.clear();
            }
        };
    }
    macro_rules! empujar {
        ($c:expr) => {{
            let g = pila.last().unwrap();
            if saltar_chars > 0 {
                saltar_chars -= 1;
            } else if !g.mudo && !g.oculto {
                if g.nota {
                    nota.push($c);
                } else {
                    parr.push($c);
                }
            }
        }};
    }
    macro_rules! fin_parrafo {
        () => {{
            let t = std::mem::take(&mut parr);
            let p = Nodo::P(Parr { clase, texto: t });
            if en_tabla {
                celda.push(p);
            } else {
                // Primer párrafo fuera de la tabla: la tabla se cierra.
                if !filas.is_empty() {
                    nodos.push(Nodo::Tabla(std::mem::take(&mut filas)));
                }
                nodos.push(p);
            }
        }};
    }

    while i < n {
        let b = bytes[i];
        match b {
            b'{' => {
                volcar_pendientes!();
                let tope = pila.last().cloned().unwrap();
                pila.push(tope);
                i += 1;
            }
            b'}' => {
                volcar_pendientes!();
                let era_nota = pila.last().is_some_and(|g| g.nota);
                if pila.len() > 1 {
                    pila.pop();
                }
                if era_nota && !pila.last().is_some_and(|g| g.nota) {
                    notas.push(std::mem::take(&mut nota));
                }
                i += 1;
            }
            b'\\' => {
                i += 1;
                if i >= n {
                    break;
                }
                let c = bytes[i];
                if c.is_ascii_alphabetic() {
                    let ini = i;
                    while i < n && bytes[i].is_ascii_alphabetic() {
                        i += 1;
                    }
                    let palabra = std::str::from_utf8(&bytes[ini..i]).unwrap_or("");
                    let mut num: Option<i64> = None;
                    let ini_n = i;
                    if i < n && (bytes[i] == b'-' || bytes[i].is_ascii_digit()) {
                        i += 1;
                        while i < n && bytes[i].is_ascii_digit() {
                            i += 1;
                        }
                        num = std::str::from_utf8(&bytes[ini_n..i])
                            .ok()
                            .and_then(|s| s.parse().ok());
                    }
                    if i < n && bytes[i] == b' ' {
                        i += 1;
                    }
                    volcar_pendientes!();
                    // ¿Abre un destino que no se lee?
                    if DESTINOS_MUDOS.contains(&palabra) {
                        if let Some(g) = pila.last_mut() {
                            g.mudo = true;
                        }
                        continue;
                    }
                    match palabra {
                        "ansicpg" => {
                            if let Some(cp) = num {
                                codificacion =
                                    codificacion_de(cp).unwrap_or(encoding_rs::WINDOWS_1252);
                            }
                        }
                        "uc" => {
                            if let Some(g) = pila.last_mut() {
                                g.uc = num.unwrap_or(1).max(0) as usize;
                            }
                        }
                        "u" => {
                            if let Some(v) = num {
                                let v = if v < 0 { v + 65536 } else { v } as u32;
                                // Pares sustitutos UTF-16: se ignora la mitad
                                // suelta en vez de romper.
                                if let Some(ch) = char::from_u32(v) {
                                    empujar!(ch);
                                }
                                saltar_chars = pila.last().map(|g| g.uc).unwrap_or(1);
                            }
                        }
                        "par" | "sect" | "page" => {
                            if pila.last().is_some_and(|g| g.nota) {
                                nota.push(' ');
                            } else {
                                fin_parrafo!();
                            }
                        }
                        "line" => empujar!('\n'),
                        "tab" => empujar!(' '),
                        "emdash" => empujar!('—'),
                        "endash" => empujar!('–'),
                        "lquote" => empujar!('‘'),
                        "rquote" => empujar!('’'),
                        "ldblquote" => empujar!('“'),
                        "rdblquote" => empujar!('”'),
                        "bullet" => empujar!('•'),
                        "pard" => {
                            clase = "paragraph";
                            en_tabla = false;
                        }
                        "intbl" => en_tabla = true,
                        "outlinelevel" => {
                            clase = nivel_a_clase(num.unwrap_or(0).clamp(0, 8) as u8 + 1);
                        }
                        "cell" | "nestcell" => {
                            fin_parrafo!();
                            fila.push(std::mem::take(&mut celda));
                        }
                        "row" | "nestrow" => {
                            if !parr.trim().is_empty() {
                                fin_parrafo!();
                            }
                            if !celda.is_empty() {
                                fila.push(std::mem::take(&mut celda));
                            }
                            filas.push(std::mem::take(&mut fila));
                        }
                        "v" => {
                            if let Some(g) = pila.last_mut() {
                                g.oculto = num.unwrap_or(1) != 0;
                            }
                        }
                        "footnote" => {
                            if let Some(g) = pila.last_mut() {
                                g.nota = true;
                            }
                        }
                        "chftn" => {}
                        "fldrslt" => {}
                        _ => {}
                    }
                } else {
                    i += 1;
                    match c {
                        b'\'' if i + 2 <= n => {
                            if let Ok(v) = u8::from_str_radix(
                                std::str::from_utf8(&bytes[i..i + 2]).unwrap_or(""),
                                16,
                            ) {
                                if saltar_chars > 0 {
                                    saltar_chars -= 1;
                                } else {
                                    pendientes.push(v);
                                }
                            }
                            i += 2;
                        }
                        b'*' => {
                            // Destino ignorable desconocido: fuera entero.
                            if let Some(g) = pila.last_mut() {
                                g.mudo = true;
                            }
                        }
                        b'~' => {
                            volcar_pendientes!();
                            empujar!(' ');
                        }
                        b'_' => {
                            volcar_pendientes!();
                            empujar!('-');
                        }
                        b'-' => {} // guion opcional
                        b'\\' | b'{' | b'}' => {
                            volcar_pendientes!();
                            empujar!(c as char);
                        }
                        b'\n' | b'\r' => {
                            volcar_pendientes!();
                            fin_parrafo!();
                        }
                        _ => {}
                    }
                }
            }
            b'\r' | b'\n' => i += 1,
            _ => {
                volcar_pendientes!();
                // Texto literal: puede ser UTF-8 en RTF «modernos» mal hechos.
                let ancho = utf8_ancho(b);
                let fin = (i + ancho).min(n);
                let trozo = String::from_utf8_lossy(&bytes[i..fin]).into_owned();
                for ch in trozo.chars() {
                    empujar!(ch);
                }
                i = fin;
            }
        }
    }
    volcar_pendientes!();
    if !parr.trim().is_empty() {
        fin_parrafo!();
    }
    if !celda.is_empty() {
        fila.push(celda);
    }
    if !fila.is_empty() {
        filas.push(fila);
    }
    if !filas.is_empty() {
        nodos.push(Nodo::Tabla(filas));
    }
    Ok(ensamblar(&nodos, &notas))
}

fn utf8_ancho(b: u8) -> usize {
    match b {
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF7 => 4,
        _ => 1,
    }
}

fn codificacion_de(cp: i64) -> Option<&'static encoding_rs::Encoding> {
    let etiqueta = match cp {
        65001 => "utf-8",
        932 => "shift_jis",
        936 => "gbk",
        949 => "euc-kr",
        950 => "big5",
        874 => "windows-874",
        1250..=1258 => return encoding_rs::Encoding::for_label(format!("windows-{cp}").as_bytes()),
        10000 => "macintosh",
        437 | 850 => "windows-1252",
        _ => return None,
    };
    encoding_rs::Encoding::for_label(etiqueta.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rtf_de_textedit_con_tildes_y_parrafos() {
        let rtf_txt = br"{\rtf1\ansi\ansicpg1252\cocoartf2822
{\fonttbl\f0\fswiss\fcharset0 Helvetica;}
{\colortbl;\red255\green255\blue255;}
{\*\expandedcolortbl;;}
\pard\tx566\pardirnatural\partightenfactor0
\f0\fs24 \cf0 Canci\'f3n de oto\'f1o\par
\par
El ni\uc0\u241 o dijo \'93hola\'94.\par
}";
        let ps = rtf(rtf_txt).unwrap();
        let t: Vec<&str> = ps.iter().map(|p| p.text.as_str()).collect();
        assert_eq!(t, vec!["Canción de otoño", "El niño dijo “hola”."]);
    }

    #[test]
    fn rtf_nota_al_final() {
        let r = br"{\rtf1\ansi Texto con nota{\super\chftn}{\footnote\pard\plain{\super\chftn} La nota.}. Sigue.\par}";
        let ps = rtf(r).unwrap();
        assert_eq!(ps[0].text, "Texto con nota. Sigue.");
        assert_eq!(ps.last().unwrap().text, "La nota.");
    }
}
