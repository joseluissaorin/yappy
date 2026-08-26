//! El guionizador: de cualquier texto a un GUION de lectura en voz alta.
//!
//! El Guion es la representación intermedia central de Yappy: una lista de
//! piezas (párrafos, títulos, citas, versos…) donde cada pieza conserva su
//! texto original intacto y lo acompaña de una partición en spans, cada
//! span con su forma hablada. Así la verbalización anota en vez de
//! destruir: el editor enseña el original, el sintetizador lee lo hablado,
//! y el karaoke mapea el audio de vuelta al original sin adivinar nada.

pub mod rbnf;
pub mod tablas;
pub mod verbalizador;

use crate::lang_detect::detect_lang;
use serde::{Deserialize, Serialize};
use std::ops::Range;

/// Qué es cada pieza dentro del documento.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClasePieza {
    Titulo1,
    Titulo2,
    Titulo3,
    Titulo4,
    Titulo5,
    Titulo6,
    Parrafo,
    Cita,
    Lista,
    Separador,
    Verso,
}

impl ClasePieza {
    /// El ritmo de lectura: pausa previa (segundos) y multiplicador de
    /// velocidad. Un título respira antes y se lee un punto más lento; un
    /// separador es un silencio largo.
    pub fn ritmo(&self) -> (f32, f32) {
        match self {
            ClasePieza::Titulo1 => (1.2, 0.92),
            ClasePieza::Titulo2 => (0.9, 0.95),
            ClasePieza::Titulo3 => (0.6, 0.97),
            ClasePieza::Titulo4 => (0.45, 0.98),
            ClasePieza::Titulo5 => (0.35, 0.99),
            ClasePieza::Titulo6 => (0.25, 1.0),
            ClasePieza::Separador => (1.5, 1.0),
            ClasePieza::Cita => (0.45, 0.97),
            ClasePieza::Lista => (0.25, 1.0),
            ClasePieza::Verso => (0.35, 0.97),
            ClasePieza::Parrafo => (0.0, 1.0),
        }
    }

    pub fn desde_kind(kind: &str) -> Self {
        match kind {
            "heading1" => ClasePieza::Titulo1,
            "heading2" => ClasePieza::Titulo2,
            "heading3" => ClasePieza::Titulo3,
            "heading4" => ClasePieza::Titulo4,
            "heading5" => ClasePieza::Titulo5,
            "heading6" => ClasePieza::Titulo6,
            "quote" => ClasePieza::Cita,
            "list" => ClasePieza::Lista,
            "hr" => ClasePieza::Separador,
            "verse" => ClasePieza::Verso,
            _ => ClasePieza::Parrafo,
        }
    }
}

/// De qué clase es un tramo de texto clasificado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaseSpan {
    Literal,
    Cardinal,
    Decimal,
    Ordinal,
    Romano,
    Anio,
    Fecha,
    Hora,
    Moneda,
    Unidad,
    Porcentaje,
    Sigla,
    Abreviatura,
    Electronico,
}

/// Un tramo del texto original con su forma hablada.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    /// Rango en CARACTERES sobre el texto original de la pieza.
    pub original: Range<usize>,
    /// Lo que entra al sintetizador (vacío = se omite, p. ej. una URL).
    pub hablado: String,
    pub clase: ClaseSpan,
}

/// Una pieza del guion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pieza {
    pub clase: ClasePieza,
    /// El texto tal cual venía en el documento. Intacto.
    pub texto: String,
    /// Idioma detectado (código Supertonic).
    pub idioma: String,
    /// Partición completa del texto en spans (los literales incluidos).
    pub spans: Vec<Span>,
    pub pausa_antes_s: f32,
    pub mult_velocidad: f32,
    /// Voz que pide esta pieza (si la anula el editor); None = la global.
    #[serde(default)]
    pub voz: Option<String>,
}

impl Pieza {
    pub fn texto_hablado(&self) -> String {
        verbalizador::texto_hablado(&self.spans)
    }
}

/// El guion completo de un documento.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Guion {
    pub idioma_base: String,
    pub piezas: Vec<Pieza>,
}

/// Construye una pieza: detecta idioma, clasifica y verbaliza.
pub fn construir_pieza(texto: &str, clase: ClasePieza, idioma_base: &str) -> Pieza {
    let idioma = detect_lang(texto, idioma_base);
    let spans = verbalizador::verbalizar(texto, &idioma);
    let (pausa, vel) = clase.ritmo();
    Pieza {
        clase,
        texto: texto.to_string(),
        idioma,
        spans,
        pausa_antes_s: pausa,
        mult_velocidad: vel,
        voz: None,
    }
}

/// Un trozo sintetizable de una pieza: el texto hablado que entra al
/// modelo, con el rango del ORIGINAL al que corresponde (en caracteres).
#[derive(Debug, Clone)]
pub struct Trozo {
    pub texto: String,
    pub origen: Range<usize>,
}

/// Trocea una pieza en llamadas al modelo respetando el límite del idioma,
/// y mapea cada trozo de vuelta al texto original vía los spans.
pub fn trocear(pieza: &Pieza) -> Vec<Trozo> {
    let hablado = pieza.texto_hablado();
    if hablado.trim().is_empty() {
        return Vec::new();
    }
    let trozos_texto: Vec<String> = crate::chunker::chunk_for_language(&hablado, &pieza.idioma)
        .into_iter()
        .map(|c| c.text)
        .collect();

    // Segmentos hablado→original acumulando la longitud hablada de cada span.
    struct Seg {
        hab: Range<usize>,
        orig: Range<usize>,
        literal: bool,
    }
    let mut segs: Vec<Seg> = Vec::new();
    let mut cursor_hab = 0usize;
    for s in &pieza.spans {
        let len = s.hablado.chars().count();
        segs.push(Seg {
            hab: cursor_hab..cursor_hab + len,
            orig: s.original.clone(),
            literal: s.clase == ClaseSpan::Literal,
        });
        cursor_hab += len;
    }
    let total_hab = cursor_hab;
    let mapear = |pos: usize, es_fin: bool| -> usize {
        if pos >= total_hab {
            return segs.last().map(|s| s.orig.end).unwrap_or(0);
        }
        for s in &segs {
            let dentro = if es_fin {
                pos > s.hab.start && pos <= s.hab.end
            } else {
                pos >= s.hab.start && pos < s.hab.end
            };
            if dentro {
                if s.literal {
                    return s.orig.start + (pos - s.hab.start);
                }
                return if es_fin { s.orig.end } else { s.orig.start };
            }
        }
        segs.last().map(|s| s.orig.end).unwrap_or(0)
    };

    // Localiza cada trozo secuencialmente en el hablado (en caracteres).
    let hab_chars: Vec<char> = hablado.chars().collect();
    let mut resultado = Vec::new();
    let mut cursor = 0usize;
    for t in trozos_texto {
        let t_chars: Vec<char> = t.chars().collect();
        if t_chars.is_empty() {
            continue;
        }
        let mut ini = None;
        'busca: for i in cursor..hab_chars.len().saturating_sub(t_chars.len() - 1) {
            for (j, tc) in t_chars.iter().enumerate() {
                if hab_chars[i + j] != *tc {
                    continue 'busca;
                }
            }
            ini = Some(i);
            break;
        }
        let Some(ini) = ini else { continue };
        let fin = ini + t_chars.len();
        cursor = fin;
        resultado.push(Trozo {
            texto: t,
            origen: mapear(ini, false)..mapear(fin, true),
        });
    }
    resultado
}

/// Construye un guion desde texto plano/markdown ligero: parte por líneas
/// en blanco y reconoce títulos (#), citas (>), listas (-, *, número.) y
/// separadores (---).
pub fn construir_desde_texto(texto: &str, idioma_base: &str) -> Guion {
    // El idioma del documento se decide mirando el documento entero (sobre
    // texto largo la detección es fiable); el parámetro queda de respaldo.
    let idioma_base = crate::lang_detect::detect_document_lang(texto, idioma_base);
    let idioma_base = idioma_base.as_str();
    let mut piezas = Vec::new();
    for bloque in texto.split("\n\n") {
        let bloque = bloque.trim();
        if bloque.is_empty() {
            continue;
        }
        let (clase, contenido) = clasificar_bloque(bloque);
        if contenido.trim().is_empty() && clase != ClasePieza::Separador {
            continue;
        }
        piezas.push(construir_pieza(&contenido, clase, idioma_base));
    }
    Guion {
        idioma_base: idioma_base.to_string(),
        piezas,
    }
}

fn clasificar_bloque(bloque: &str) -> (ClasePieza, String) {
    let recortado = bloque.trim_start();
    if recortado == "---" || recortado == "***" || recortado == "___" {
        return (ClasePieza::Separador, String::new());
    }
    if let Some(resto) = recortado.strip_prefix('#') {
        let mut nivel = 1u8;
        let mut resto = resto;
        while let Some(r) = resto.strip_prefix('#') {
            nivel += 1;
            resto = r;
        }
        let clase = match nivel {
            1 => ClasePieza::Titulo1,
            2 => ClasePieza::Titulo2,
            3 => ClasePieza::Titulo3,
            4 => ClasePieza::Titulo4,
            5 => ClasePieza::Titulo5,
            _ => ClasePieza::Titulo6,
        };
        return (clase, resto.trim().to_string());
    }
    if recortado.starts_with("> ") {
        let limpio = recortado
            .lines()
            .map(|l| {
                l.trim_start()
                    .trim_start_matches("> ")
                    .trim_start_matches('>')
            })
            .collect::<Vec<_>>()
            .join("\n");
        return (ClasePieza::Cita, limpio);
    }
    if recortado.starts_with("- ") || recortado.starts_with("* ") || recortado.starts_with("+ ") {
        let limpio = recortado
            .lines()
            .map(|l| {
                l.trim_start()
                    .trim_start_matches("- ")
                    .trim_start_matches("* ")
                    .trim_start_matches("+ ")
            })
            .collect::<Vec<_>>()
            .join(". ");
        return (ClasePieza::Lista, limpio);
    }
    (ClasePieza::Parrafo, bloque.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guion_desde_markdown() {
        let g = construir_desde_texto(
            "# El siglo XX\n\nEn 1914 empezó la guerra.\n\n---\n\n> Una cita.",
            "es",
        );
        assert_eq!(g.piezas.len(), 4);
        assert_eq!(g.piezas[0].clase, ClasePieza::Titulo1);
        assert_eq!(g.piezas[0].texto_hablado(), "El siglo veinte");
        assert!(g.piezas[1]
            .texto_hablado()
            .contains("mil novecientos catorce"));
        assert_eq!(g.piezas[2].clase, ClasePieza::Separador);
        assert_eq!(g.piezas[3].clase, ClasePieza::Cita);
    }

    #[test]
    fn el_original_queda_intacto() {
        let g = construir_desde_texto("En 1492, Colón.", "es");
        assert_eq!(g.piezas[0].texto, "En 1492, Colón.");
        assert_eq!(
            g.piezas[0].texto_hablado(),
            "En mil cuatrocientos noventa y dos, Colón."
        );
    }
}
