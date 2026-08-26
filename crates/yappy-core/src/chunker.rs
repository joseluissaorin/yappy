//! Paragraph-based chunking.
//!
//! Supertonic 3 handles up to ~300 chars per call comfortably (120 for ko/ja).
//! For long-form reading we split by blank lines first, then by sentences when
//! a paragraph is too long. The returned chunks preserve order and are short
//! enough to synthesize quickly so the player can start audio after the first one.

use regex::Regex;

use once_cell::sync::Lazy;

static PARA_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\n\s*\n").unwrap());
static SENT_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?P<p>[.!?…])(?:\s+|$)").unwrap());

const ABBREV: &[&str] = &[
    "Dr.", "Mr.", "Mrs.", "Ms.", "Prof.", "Sr.", "Jr.", "St.", "Ave.", "Rd.", "Blvd.", "Dept.",
    "Inc.", "Ltd.", "Co.", "Corp.", "etc.", "vs.", "i.e.", "e.g.", "Ph.D.", "No.", "vol.", "Vol.",
];

#[derive(Debug, Clone)]
pub struct Chunk {
    pub text: String,
    /// Character offset of the chunk's first byte in the original (post-normalize) text.
    pub start: usize,
    pub end: usize,
}

pub fn chunk_for_language(text: &str, lang: &str) -> Vec<Chunk> {
    let max = max_chars_for_lang(lang);
    chunk_paragraphs(text, max)
}

pub fn max_chars_for_lang(lang: &str) -> usize {
    match lang {
        "ko" | "ja" => 120,
        _ => 300,
    }
}

pub fn chunk_paragraphs(text: &str, max_chars: usize) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut cursor = 0usize;
    for raw_para in PARA_RE.split(text) {
        let start_in_text = find_offset(text, cursor, raw_para);
        let para = raw_para.trim();
        cursor = start_in_text + raw_para.len();
        if para.is_empty() {
            continue;
        }
        let para_start = start_in_text
            + raw_para
                .find(para.chars().next().unwrap_or(' '))
                .unwrap_or(0);
        // SIEMPRE por frases: la palabra gigante del móvil vive de esta
        // granularidad (karaoke frase a frase, primer audio antes). Las
        // frases minúsculas (<25 caracteres) se pegan a la anterior para no
        // trocear el fraseo en migas.
        let sentences = split_sentences(para);
        let mut current = String::new();
        let mut current_start = para_start;
        let mut running_offset = para_start;
        for s in sentences {
            let s_len_chars = s.chars().count();
            if s_len_chars > max_chars {
                if !current.is_empty() {
                    chunks.push(Chunk {
                        text: current.trim().to_string(),
                        start: current_start,
                        end: current_start + current.len(),
                    });
                    current.clear();
                }
                // Split by comma, then by space.
                let sub = split_too_long(&s, max_chars);
                let mut sub_start = running_offset;
                for piece in sub {
                    let piece_len = piece.len();
                    chunks.push(Chunk {
                        text: piece.trim().to_string(),
                        start: sub_start,
                        end: sub_start + piece_len,
                    });
                    sub_start += piece_len;
                }
                running_offset += s.len();
                current_start = running_offset;
                continue;
            }
            let cabe_la_miga = s.trim().chars().count() < 25
                && (current.chars().count() + s_len_chars) <= max_chars;
            if !current.is_empty() && !cabe_la_miga {
                chunks.push(Chunk {
                    text: current.trim().to_string(),
                    start: current_start,
                    end: current_start + current.len(),
                });
                current.clear();
                current_start = running_offset;
            }
            if current.is_empty() {
                current_start = running_offset;
            }
            current.push_str(&s);
            running_offset += s.len();
        }
        if !current.is_empty() {
            chunks.push(Chunk {
                text: current.trim().to_string(),
                start: current_start,
                end: current_start + current.len(),
            });
        }
    }
    if chunks.is_empty() && !text.trim().is_empty() {
        chunks.push(Chunk {
            text: text.trim().to_string(),
            start: 0,
            end: text.len(),
        });
    }
    chunks
}

fn find_offset(haystack: &str, start: usize, needle: &str) -> usize {
    if needle.is_empty() {
        return start;
    }
    haystack[start..]
        .find(needle)
        .map(|i| start + i)
        .unwrap_or(start)
}

fn split_sentences(text: &str) -> Vec<String> {
    let matches: Vec<_> = SENT_RE.find_iter(text).collect();
    if matches.is_empty() {
        return vec![text.to_string()];
    }
    let mut sentences = Vec::new();
    let mut last_end = 0;
    for m in matches {
        // Incluir el PRIMER signo de puntuación de la racha, midiendo su
        // ancho real en bytes: «…» ocupa TRES, y el «+ 1» de antes caía en
        // mitad del carácter y PANICABA con cualquier texto que tuviera
        // puntos suspensivos antes de un espacio (el caso Borges: la
        // síntesis moría y «preparando la voz» se quedaba colgado).
        let tras_primer_signo = text[m.start()..]
            .chars()
            .next()
            .map(|c| m.start() + c.len_utf8())
            .unwrap_or(m.end());
        let before = &text[last_end..tras_primer_signo];
        let before_trim = before.trim_start();
        let mut is_abbrev = false;
        for a in ABBREV {
            if before_trim.trim_end().ends_with(a) {
                is_abbrev = true;
                break;
            }
        }
        if !is_abbrev {
            sentences.push(text[last_end..m.end()].to_string());
            last_end = m.end();
        }
    }
    if last_end < text.len() {
        sentences.push(text[last_end..].to_string());
    }
    sentences
}

fn split_too_long(s: &str, max_chars: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let parts: Vec<&str> = s.split(',').collect();
    let mut current = String::new();
    for part in parts {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        if p.chars().count() > max_chars {
            if !current.is_empty() {
                out.push(current.trim().to_string());
                current.clear();
            }
            // last resort: split on spaces
            let mut wchunk = String::new();
            for w in p.split_whitespace() {
                if wchunk.chars().count() + w.chars().count() + 1 > max_chars && !wchunk.is_empty()
                {
                    out.push(wchunk.trim().to_string());
                    wchunk.clear();
                }
                if !wchunk.is_empty() {
                    wchunk.push(' ');
                }
                wchunk.push_str(w);
            }
            if !wchunk.is_empty() {
                out.push(wchunk.trim().to_string());
            }
            continue;
        }
        if current.chars().count() + p.chars().count() + 2 > max_chars && !current.is_empty() {
            out.push(current.trim().to_string());
            current.clear();
        }
        if !current.is_empty() {
            current.push_str(", ");
        }
        current.push_str(p);
    }
    if !current.is_empty() {
        out.push(current.trim().to_string());
    }
    if out.is_empty() {
        out.push(s.trim().to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn puntos_suspensivos_no_rompen_el_corte() {
        // El caso Borges: «…» (tres bytes) delante de espacio hacía caer el
        // corte en mitad del carácter y el motor entero pánicaba.
        let texto = "Vaciló un momento… Después dijo unas palabras. “…y colgó el tubo.” Salió.";
        let frases = split_sentences(texto);
        assert!(frases.len() >= 3, "{frases:?}");
        assert!(frases[0].contains('…'));
        let pegado: String = frases.concat();
        assert_eq!(pegado.trim(), texto.trim());
    }

    #[test]
    fn signos_multibyte_al_inicio_de_racha() {
        for t in ["Uno… dos. Tres.", "¿Sí…? Claro. Fin.", "Ah!… Bueno. Ya."] {
            let frases = split_sentences(t);
            assert!(!frases.is_empty(), "{t}");
        }
    }

    #[test]
    fn parte_por_frases_siempre() {
        let texto = "La primera frase dice una cosa con calma. La segunda dice otra distinta y algo más larga. ¿Sí?";
        let trozos = chunk_paragraphs(texto, 300);
        // Dos frases largas + una miga («¿Sí?» < 25) pegada a la segunda.
        assert_eq!(trozos.len(), 2);
        assert!(trozos[0].text.starts_with("La primera"));
        assert!(trozos[1].text.starts_with("La segunda"));
        assert!(trozos[1].text.ends_with("¿Sí?"));
        // Los offsets siguen cubriendo el texto en orden.
        assert!(trozos[0].start < trozos[1].start);
    }

    #[test]
    fn parrafo_de_una_frase_queda_entero() {
        let trozos = chunk_paragraphs("Una sola frase tranquila sin más compañía.", 300);
        assert_eq!(trozos.len(), 1);
    }
}
