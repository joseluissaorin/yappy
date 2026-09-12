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
        let sentences = if parece_verso(para) {
            split_versos(para)
        } else {
            split_sentences(para)
        };
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
                // Por signo débil, luego por espacio: SIEMPRE rebanadas
                // exactas de la frase, para que los offsets valgan y
                // `trocear` pueda localizar cada trozo.
                for (a, b) in split_too_long(&s, max_chars) {
                    let crudo = &s[a..b];
                    let t = crudo.trim();
                    if t.is_empty() {
                        continue;
                    }
                    let desplaz = crudo.len() - crudo.trim_start().len();
                    let ini = running_offset + a + desplaz;
                    chunks.push(Chunk {
                        text: t.to_string(),
                        start: ini,
                        end: ini + t.len(),
                    });
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

/// ¿Este párrafo es VERSO? Varios saltos de línea internos con líneas
/// cortas: poesía, letras de canciones, texto experimental. Partirlo por
/// frases no sirve (apenas hay puntos): el verso ES la unidad de dicción.
fn parece_verso(text: &str) -> bool {
    let lineas: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    if lineas.len() < 3 {
        return false;
    }
    let media = lineas.iter().map(|l| l.chars().count()).sum::<usize>() / lineas.len();
    media <= 60
}

/// Cada verso, una «frase»: substrings consecutivos que cubren el párrafo
/// entero (los offsets del karaoke dependen de esa continuidad). Los
/// versos minúsculos se pegan después, como las migas de siempre.
fn split_versos(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut last = 0usize;
    for (i, b) in text.bytes().enumerate() {
        if b == b'\n' {
            out.push(text[last..=i].to_string());
            last = i + 1;
        }
    }
    if last < text.len() {
        out.push(text[last..].to_string());
    }
    if out.is_empty() {
        out.push(text.to_string());
    }
    out
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

/// Corta una frase demasiado larga en trozos que son REBANADAS EXACTAS de
/// ella. Nunca reconstruye el texto: la versión vieja partía por comas,
/// recortaba y volvía a unir con «, », y entonces el trozo ya no aparecía
/// literal en el hablado; `guion::trocear`, que los localiza buscándolos,
/// los descartaba EN SILENCIO. Ese era el bug de «solo lee hasta la coma»
/// y el de los párrafos larguísimos que se quedaban en las primeras frases.
///
/// Prefiere cortar tras un signo débil (coma, punto y coma, dos puntos,
/// raya, paréntesis o comilla de cierre); si el signo cae demasiado
/// pronto, corta en un espacio; si no hay ninguno, corta a lo bruto.
/// Devuelve rangos de bytes CONTIGUOS que cubren la frase entera.
fn split_too_long(s: &str, max_chars: usize) -> Vec<(usize, usize)> {
    let max = max_chars.max(16);
    let mut debiles: Vec<usize> = Vec::new();
    let mut espacios: Vec<usize> = Vec::new();
    for (i, c) in s.char_indices() {
        let fin = i + c.len_utf8();
        if matches!(
            c,
            ',' | ';' | ':' | '\u{2014}' | '\u{2013}' | ')' | '\u{bb}'
        ) {
            debiles.push(fin);
        } else if c.is_whitespace() {
            espacios.push(fin);
        }
    }
    let mut out: Vec<(usize, usize)> = Vec::new();
    let mut ini = 0usize;
    while ini < s.len() {
        let resto = &s[ini..];
        if resto.chars().count() <= max {
            out.push((ini, s.len()));
            break;
        }
        // El techo: como mucho `max` caracteres desde `ini`.
        let limite = ini
            + resto
                .char_indices()
                .nth(max)
                .map(|(b, _)| b)
                .unwrap_or(resto.len());
        // Un corte débil que deje el trozo demasiado corto parte el fraseo
        // en migas: por debajo del 45 % del cupo, mejor un espacio.
        let suelo = ini
            + resto
                .char_indices()
                .nth(max * 45 / 100)
                .map(|(b, _)| b)
                .unwrap_or(0);
        let corte = debiles
            .iter()
            .rev()
            .find(|&&p| p > suelo && p <= limite)
            .copied()
            .or_else(|| {
                espacios
                    .iter()
                    .rev()
                    .find(|&&p| p > suelo && p <= limite)
                    .copied()
            })
            .or_else(|| {
                espacios
                    .iter()
                    .rev()
                    .find(|&&p| p > ini && p <= limite)
                    .copied()
            })
            .unwrap_or(limite);
        out.push((ini, corte));
        ini = corte;
    }
    if out.is_empty() {
        out.push((0, s.len()));
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn el_verso_es_la_unidad_de_diccion() {
        let poema = "En la cumbre del cerro\nse pelearon dos vientos\nuno del norte helado\ny otro que venía ardiendo\nse agarraron con furia\ncomo dos perros hambrientos";
        let chunks = super::chunk_paragraphs(poema, 220);
        // Sin el corte por versos esto era UNA masa de 160 caracteres.
        assert!(
            chunks.len() >= 3,
            "esperaba versos, hay {} trozos",
            chunks.len()
        );
        // Los offsets siguen siendo exactos sobre el original.
        for c in &chunks {
            assert_eq!(poema[c.start..c.end].trim(), c.text, "offset roto en {c:?}");
        }
        // La prosa normal sigue partiendo por frases, no por líneas.
        let prosa = "Una frase normal. Otra frase que sigue. Y una tercera que cierra el párrafo con calma.";
        let normales = super::chunk_paragraphs(prosa, 220);
        assert!(normales.len() <= 2, "la prosa no debe trocearse por líneas");
    }

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

#[cfg(test)]
mod tests_largos {
    use super::*;

    /// La ley del troceador: cada trozo es una REBANADA EXACTA del texto.
    /// Si se rompe, `guion::trocear` no encuentra el trozo y lo descarta en
    /// silencio: la voz se para a mitad del párrafo.
    fn rebanadas_exactas(texto: &str, chunks: &[Chunk]) {
        for c in chunks {
            assert_eq!(
                texto[c.start..c.end].trim(),
                c.text,
                "offset roto: {:?} no casa con {:?}",
                &texto[c.start..c.end],
                c.text
            );
        }
    }

    /// Nada se pierde: todas las palabras del original salen en algún trozo,
    /// en orden.
    fn nada_se_pierde(texto: &str, chunks: &[Chunk]) {
        let dichas: Vec<String> = chunks
            .iter()
            .flat_map(|c| c.text.split_whitespace().map(|w| w.to_string()))
            .collect();
        let originales: Vec<String> = texto.split_whitespace().map(|w| w.to_string()).collect();
        assert_eq!(
            dichas.len(),
            originales.len(),
            "se perdieron palabras: {} de {}",
            dichas.len(),
            originales.len()
        );
        assert_eq!(dichas, originales, "el orden o el contenido cambió");
    }

    #[test]
    fn la_frase_con_subordinadas_se_lee_entera() {
        // Una sola frase de más de 300 caracteres, llena de comas: el caso
        // que se quedaba «leyendo hasta la coma».
        let frase = "Cuando el hombre llegó al recodo del río, que a esa hora bajaba turbio y \
                     lento, con esa lentitud que engaña a quien no lo conoce, se dio cuenta, \
                     aunque tarde, de que la canoa, atada con un nudo que él mismo había hecho \
                     la noche anterior, ya no estaba donde la había dejado, y que la corriente, \
                     paciente como todas las cosas del monte, se la había llevado sin ruido.";
        assert!(
            frase.chars().count() > 300,
            "la prueba necesita una frase larga"
        );
        let chunks = chunk_paragraphs(frase, 300);
        assert!(chunks.len() > 1, "debería trocearse");
        rebanadas_exactas(frase, &chunks);
        nada_se_pierde(frase, &chunks);
    }

    #[test]
    fn el_parrafo_larguisimo_se_lee_entero() {
        // Doce frases seguidas: antes solo sonaban las primeras.
        let mut parrafo = String::new();
        for i in 1..=12 {
            parrafo.push_str(&format!(
                "Esta es la frase número {i} del párrafo, y viene con su coma, su inciso y su \
                 final. "
            ));
        }
        let parrafo = parrafo.trim();
        let chunks = chunk_paragraphs(parrafo, 300);
        rebanadas_exactas(parrafo, &chunks);
        nada_se_pierde(parrafo, &chunks);
    }

    #[test]
    fn la_frase_sin_comas_ni_espacios_raros_tambien() {
        // Sin una sola coma: hay que cortar por espacios, sin perder nada.
        let frase = "palabra ".repeat(80);
        let frase = frase.trim();
        let chunks = chunk_paragraphs(frase, 120);
        assert!(chunks.len() > 3);
        rebanadas_exactas(frase, &chunks);
        nada_se_pierde(frase, &chunks);
    }

    #[test]
    fn los_espacios_dobles_y_saltos_no_pierden_texto() {
        // La versión vieja normalizaba los espacios y el trozo dejaba de
        // aparecer literal en el hablado: se perdía.
        let frase = "Primera parte con espacios  dobles,   segunda parte con más texto todavía, \
                     tercera parte que alarga la frase por encima del límite del troceador, \
                     cuarta parte final que cierra la oración con un punto.";
        let chunks = chunk_paragraphs(frase, 100);
        rebanadas_exactas(frase, &chunks);
        nada_se_pierde(frase, &chunks);
    }
}
