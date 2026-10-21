//! Banco de medida de la importación de documentos (diagnóstico manual).
//!
//! ```text
//! CORPUS_IMPORT=/tmp/corpus-import CORPUS_REF=/tmp/corpus-ref \
//!   cargo test -p yappy-app corpus_importacion -- --ignored --nocapture
//! ```
//!
//! Por cada fichero imprime: párrafos, párrafo más largo, cobertura del texto
//! de referencia (palabras de ≥4 letras de `pdftotext`/`textutil` que siguen
//! apareciendo), y la basura que se leería en voz alta: folios, mobiliario
//! repetido, URLs, llamadas de nota pegadas, palabras partidas por guion,
//! símbolos de Markdown, ligaduras y cortes de párrafo en mitad de frase.
//! Con `VUELCA=/dir` escribe además cada documento párrafo a párrafo.

use super::*;
use std::collections::{HashMap, HashSet};

fn palabras(t: &str) -> HashSet<String> {
    t.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 4 && w.chars().all(|c| c.is_alphabetic()))
        .map(|w| w.to_lowercase())
        .collect()
}

pub(super) struct Medida {
    pub parrafos: usize,
    pub max: usize,
    pub chars: usize,
    pub cobertura: Option<f32>,
    pub folios: usize,
    pub repetidos: usize,
    pub urls: usize,
    pub llamadas: usize,
    pub guiones: usize,
    pub markdown: usize,
    pub ligaduras: usize,
    pub cortes: usize,
}

pub(super) fn medir(ps: &[RichParagraph], referencia: Option<&str>) -> Medida {
    use once_cell::sync::Lazy;
    use regex::Regex;
    static LLAMADA: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"(\p{L}[.,;:»”\x22)]?\d{1,3}(?:\s|$))|[¹²³⁴⁵⁶⁷⁸⁹⁰]").unwrap());
    static GUION: Lazy<Regex> = Lazy::new(|| Regex::new(r"\p{Ll}- \p{Ll}|\p{Ll}-$").unwrap());
    static MD: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"(^|\s)#{1,6}\s|\*\*|\]\(|\|\s|`|</?[a-zA-Z][^>]*>|^\s*[-*+]\s|\[\^").unwrap()
    });
    let textos: Vec<&str> = ps
        .iter()
        .map(|p| p.text.as_str())
        .filter(|t| !t.trim().is_empty())
        .collect();
    let mut veces: HashMap<&str, usize> = HashMap::new();
    for t in &textos {
        if t.chars().count() < 90 {
            *veces.entry(t).or_default() += 1;
        }
    }
    let todo = textos.join("\n");
    let cobertura = referencia.map(|r| {
        let rp = palabras(r);
        let op = palabras(&todo);
        if rp.is_empty() {
            1.0
        } else {
            rp.iter().filter(|w| op.contains(*w)).count() as f32 / rp.len() as f32
        }
    });
    let mut cortes = 0;
    let prosa: Vec<&RichParagraph> = ps.iter().filter(|p| !p.text.trim().is_empty()).collect();
    for w in prosa.windows(2) {
        let (a, b) = (w[0], w[1]);
        if a.kind != "paragraph" || b.kind != "paragraph" {
            continue;
        }
        let fin = a.text.trim_end().chars().last().unwrap_or('.');
        let abre_min = b
            .text
            .chars()
            .find(|c| c.is_alphabetic())
            .is_some_and(|c| c.is_lowercase());
        if !".!?:;»\"'”’)…]".contains(fin) && abre_min {
            cortes += 1;
        }
    }
    Medida {
        parrafos: textos.len(),
        max: textos.iter().map(|t| t.chars().count()).max().unwrap_or(0),
        chars: textos.iter().map(|t| t.chars().count()).sum(),
        cobertura,
        folios: textos.iter().filter(|t| es_folio(t)).count(),
        repetidos: veces.values().filter(|&&v| v >= 3).map(|v| v).sum(),
        urls: todo.matches("http").count() + todo.matches("www.").count(),
        llamadas: LLAMADA.find_iter(&todo).count(),
        guiones: textos.iter().map(|t| GUION.find_iter(t).count()).sum(),
        markdown: textos.iter().map(|t| MD.find_iter(t).count()).sum(),
        ligaduras: todo
            .chars()
            .filter(|c| ('\u{FB00}'..='\u{FB06}').contains(c))
            .count(),
        cortes,
    }
}

#[test]
#[ignore]
fn corpus_importacion() {
    let dir = std::env::var("CORPUS_IMPORT").expect("CORPUS_IMPORT sin definir");
    let refdir = std::env::var("CORPUS_REF").ok();
    let vuelca = std::env::var("VUELCA").ok();
    let filtro = std::env::var("SOLO").ok();
    let mut ficheros: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file())
        .collect();
    ficheros.sort();
    println!(
        "{:<44} {:>6} {:>6} {:>7} {:>5} {:>4} {:>4} {:>4} {:>5} {:>4} {:>4} {:>4} {:>5} {:>7}",
        "fichero",
        "párr",
        "máx",
        "chars",
        "cob%",
        "fol",
        "rep",
        "url",
        "nota",
        "gui",
        "md",
        "lig",
        "corte",
        "ms"
    );
    for f in ficheros {
        let nombre = f.file_name().unwrap().to_string_lossy().to_string();
        if filtro.as_deref().is_some_and(|s| !nombre.contains(s)) {
            continue;
        }
        let t0 = std::time::Instant::now();
        let r = load_rich_from_file(&f);
        let ms = t0.elapsed().as_millis();
        let ps = match r {
            Ok(v) => v,
            Err(e) => {
                println!(
                    "{:<44} ERROR {e}",
                    nombre.chars().take(44).collect::<String>()
                );
                continue;
            }
        };
        let referencia = refdir
            .as_ref()
            .and_then(|d| std::fs::read_to_string(Path::new(d).join(format!("{nombre}.txt"))).ok());
        let m = medir(&ps, referencia.as_deref());
        println!(
            "{:<44} {:>6} {:>6} {:>7} {:>5} {:>4} {:>4} {:>4} {:>5} {:>4} {:>4} {:>4} {:>5} {:>7}",
            nombre.chars().take(44).collect::<String>(),
            m.parrafos,
            m.max,
            m.chars,
            m.cobertura
                .map(|c| format!("{:.1}", c * 100.0))
                .unwrap_or("-".into()),
            m.folios,
            m.repetidos,
            m.urls,
            m.llamadas,
            m.guiones,
            m.markdown,
            m.ligaduras,
            m.cortes,
            ms
        );
        if let Some(v) = &vuelca {
            let _ = std::fs::create_dir_all(v);
            let cuerpo: String = ps
                .iter()
                .enumerate()
                .map(|(i, p)| format!("[{i}] {} │ {}\n", p.kind, p.text))
                .collect();
            let _ = std::fs::write(Path::new(v).join(format!("{nombre}.txt")), cuerpo);
        }
    }
}

/// «The Yellow Wallpaper» tiene versales de énfasis («it KNEW», «DOES»,
/// «PERHAPS»): ninguna se deletrea y las comunes llegan en minúsculas.
#[test]
fn el_papel_amarillo_no_deletrea_las_versales() {
    use yappy_core::guion::{construir_desde_texto, ClaseSpan};
    let ruta =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/the-yellow-wallpaper.epub");
    let ps = load_rich_from_file(&ruta).expect("epub");
    let mut deletreadas: Vec<String> = Vec::new();
    let mut enfasis: Vec<String> = Vec::new();
    for p in &ps {
        let g = construir_desde_texto(&p.text, "en");
        for pieza in &g.piezas {
            let chars: Vec<char> = pieza.texto.chars().collect();
            for s in &pieza.spans {
                let original: String = chars[s.original.clone()].iter().collect();
                match s.clase {
                    ClaseSpan::Sigla => deletreadas.push(original),
                    ClaseSpan::Versal => enfasis.push(format!("{original}→{}", s.hablado)),
                    _ => {}
                }
            }
        }
    }
    assert!(
        deletreadas.is_empty(),
        "versales deletreadas: {deletreadas:?}"
    );
    for esperado in ["KNEW→knew", "DOES→does", "MUST→must"] {
        assert!(
            enfasis.iter().any(|e| e == esperado),
            "falta {esperado} en {enfasis:?}"
        );
    }
    // Y las siglas de verdad, en el mismo idioma, sí.
    let g = construir_desde_texto("The FBI, the BBC and NASA read HTML.", "en");
    let hablado: String = g
        .piezas
        .iter()
        .flat_map(|p| p.spans.iter().map(|s| s.hablado.clone()))
        .collect();
    assert!(
        hablado.contains("F B I") && hablado.contains("B B C") && hablado.contains("H T M L"),
        "{hablado}"
    );
    assert!(hablado.contains("NASA"), "{hablado}");
}
