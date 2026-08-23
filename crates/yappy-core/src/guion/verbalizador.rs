//! El verbalizador: convierte el texto de una pieza en spans clasificados,
//! cada uno con su forma hablada. No destruye nada: el texto original queda
//! intacto y cada span recuerda de qué rango salió, que es lo que permite
//! un karaoke exacto y un guion editable.
//!
//! Estrategia: pasadas ordenadas sobre rangos aún sin reclamar (fecha antes
//! que cardinal, moneda antes que decimal…), cada una con el contexto que
//! necesita. Los seis idiomas con tabla completa reciben todas las pasadas;
//! el resto, solo las universales (números, horas, años), porque expandir
//! con palabras de otro idioma es peor que no expandir.

use super::rbnf::{self, Genero, Numero};
use super::tablas::{tabla, TablaIdioma};
use super::{ClaseSpan, Span};
use once_cell::sync::Lazy;
use regex::Regex;

struct Lienzo<'a> {
    texto: &'a str,
    reclamado: Vec<bool>, // por byte
    spans: Vec<(usize, usize, ClaseSpan, String)>, // rangos en bytes
}

impl<'a> Lienzo<'a> {
    fn new(texto: &'a str) -> Self {
        Lienzo { texto, reclamado: vec![false; texto.len()], spans: Vec::new() }
    }

    fn libre(&self, ini: usize, fin: usize) -> bool {
        !self.reclamado[ini..fin].iter().any(|b| *b)
    }

    fn reclamar(&mut self, ini: usize, fin: usize, clase: ClaseSpan, hablado: String) {
        for b in &mut self.reclamado[ini..fin] {
            *b = true;
        }
        self.spans.push((ini, fin, clase, hablado));
    }

    /// La palabra anterior al byte `ini` (sin puntuación pegada).
    fn palabra_anterior(&self, ini: usize) -> Option<&'a str> {
        let antes = self.texto[..ini].trim_end();
        let inicio = antes
            .rfind(|c: char| c.is_whitespace())
            .map(|i| i + 1)
            .unwrap_or(0);
        let palabra = &antes[inicio..];
        let palabra = palabra.trim_matches(|c: char| {
            !(c.is_alphanumeric() || c == '.' || c == 'º' || c == 'ª')
        });
        if palabra.is_empty() { None } else { Some(palabra) }
    }

    /// La palabra siguiente al byte `fin`.
    fn palabra_siguiente(&self, fin: usize) -> Option<&'a str> {
        let despues = self.texto[fin..].trim_start();
        let final_ = despues
            .find(|c: char| c.is_whitespace())
            .unwrap_or(despues.len());
        let palabra = despues[..final_]
            .trim_matches(|c: char| !(c.is_alphanumeric() || c == '.'));
        if palabra.is_empty() { None } else { Some(palabra) }
    }

    /// Convierte los spans reclamados + los huecos literales en la lista
    /// final ordenada, con rangos EN CARACTERES.
    fn terminar(mut self) -> Vec<Span> {
        self.spans.sort_by_key(|s| s.0);
        let mut resultado = Vec::new();
        let mut cursor = 0usize;

        let byte_a_char: Vec<usize> = {
            let mut mapa = vec![0usize; self.texto.len() + 1];
            for (i, (b, _)) in self.texto.char_indices().enumerate() {
                mapa[b] = i;
            }
            mapa[self.texto.len()] = self.texto.chars().count();
            mapa
        };

        for (ini, fin, clase, hablado) in self.spans.drain(..) {
            if ini > cursor {
                let literal = &self.texto[cursor..ini];
                resultado.push(Span {
                    original: byte_a_char[cursor]..byte_a_char[ini],
                    hablado: literal.to_string(),
                    clase: ClaseSpan::Literal,
                });
            }
            resultado.push(Span {
                original: byte_a_char[ini]..byte_a_char[fin],
                hablado,
                clase,
            });
            cursor = fin;
        }
        if cursor < self.texto.len() {
            resultado.push(Span {
                original: byte_a_char[cursor]..byte_a_char[self.texto.len()],
                hablado: self.texto[cursor..].to_string(),
                clase: ClaseSpan::Literal,
            });
        }
        resultado
    }
}

// ── Utilidades numéricas ─────────────────────────────────────────────────

fn decimal_con_punto(lang: &str) -> bool {
    matches!(lang, "en" | "ja" | "ko" | "hi")
}

/// Parsea «1.234,56» o «1,234.56» según la convención del idioma.
fn parsear_numero(crudo: &str, lang: &str) -> Option<Numero> {
    let (miles, decimal) = if decimal_con_punto(lang) { (',', '.') } else { ('.', ',') };
    let limpio: String = crudo.chars().filter(|c| *c != miles && *c != ' ' && *c != '\u{a0}').collect();
    let (ent, dec) = match limpio.split_once(decimal) {
        Some((e, d)) => (e, Some(d.to_string())),
        None => (limpio.as_str(), None),
    };
    let entero: i128 = ent.parse().ok()?;
    Some(Numero { negativo: false, entero, decimales: dec })
}

fn cardinal_txt(lang: &str, numero: &Numero) -> Option<String> {
    rbnf::cardinal(lang, numero, Genero::Masculino)
}

fn romano_a_entero(s: &str) -> Option<i128> {
    static VALIDO: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"^M{0,4}(CM|CD|D?C{0,3})(XC|XL|L?X{0,3})(IX|IV|V?I{0,3})$").unwrap()
    });
    if s.is_empty() || !VALIDO.is_match(s) {
        return None;
    }
    let valor = |c: char| -> i128 {
        match c {
            'I' => 1, 'V' => 5, 'X' => 10, 'L' => 50,
            'C' => 100, 'D' => 500, 'M' => 1000, _ => 0,
        }
    };
    let cs: Vec<char> = s.chars().collect();
    let mut total = 0i128;
    for i in 0..cs.len() {
        let v = valor(cs[i]);
        if i + 1 < cs.len() && v < valor(cs[i + 1]) {
            total -= v;
        } else {
            total += v;
        }
    }
    (total > 0).then_some(total)
}

fn es_mayusculas(palabra: &str) -> bool {
    let letras: Vec<char> = palabra.chars().filter(|c| c.is_alphabetic()).collect();
    letras.len() >= 2 && letras.iter().all(|c| c.is_uppercase())
}

fn es_nombre_propio(palabra: &str) -> bool {
    let mut cs = palabra.chars();
    match cs.next() {
        Some(c) if c.is_uppercase() => {
            let resto: Vec<char> = cs.filter(|c| c.is_alphabetic()).collect();
            resto.len() >= 2 && resto.iter().all(|c| c.is_lowercase())
        }
        _ => false,
    }
}

// ── Pasadas ──────────────────────────────────────────────────────────────

static RE_URL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"https?://\S+|www\.\S+").unwrap());
static RE_EMAIL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}").unwrap());

fn pasada_electronico(l: &mut Lienzo) {
    let matches: Vec<(usize, usize)> = RE_URL
        .find_iter(l.texto)
        .chain(RE_EMAIL.find_iter(l.texto))
        .map(|m| (m.start(), m.end()))
        .collect();
    for (a, b) in matches {
        if l.libre(a, b) {
            l.reclamar(a, b, ClaseSpan::Electronico, String::new());
        }
    }
}

static RE_FECHA_ISO: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(\d{4})-(\d{2})-(\d{2})\b").unwrap());
static RE_FECHA_NUM: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(\d{1,2})[/.](\d{1,2})[/.](\d{4})\b").unwrap());

fn pasada_fechas(l: &mut Lienzo, t: &TablaIdioma) {
    let mut encontrados: Vec<(usize, usize, u32, u32, i32)> = Vec::new();
    for c in RE_FECHA_ISO.captures_iter(l.texto) {
        let m = c.get(0).unwrap();
        let (a, mes, d) = (
            c[1].parse::<i32>().unwrap_or(0),
            c[2].parse::<u32>().unwrap_or(0),
            c[3].parse::<u32>().unwrap_or(0),
        );
        encontrados.push((m.start(), m.end(), d, mes, a));
    }
    for c in RE_FECHA_NUM.captures_iter(l.texto) {
        let m = c.get(0).unwrap();
        let (p1, p2, a) = (
            c[1].parse::<u32>().unwrap_or(0),
            c[2].parse::<u32>().unwrap_or(0),
            c[3].parse::<i32>().unwrap_or(0),
        );
        // en: mes/día; el resto: día/mes
        let (d, mes) = if t.lang == "en" { (p2, p1) } else { (p1, p2) };
        encontrados.push((m.start(), m.end(), d, mes, a));
    }
    for (ini, fin, d, mes, a) in encontrados {
        if !(1..=31).contains(&d) || !(1..=12).contains(&mes) || !(1000..=2999).contains(&a) {
            continue;
        }
        if l.libre(ini, fin) {
            if let Some(hablado) = (t.fecha)(d, mes, a) {
                l.reclamar(ini, fin, ClaseSpan::Fecha, hablado);
            }
        }
    }

    // «May 24, 2026» (en) y «24. Mai 2026» (de): día + mes con nombre.
    if t.lang == "en" {
        static RE: Lazy<Regex> = Lazy::new(|| {
            Regex::new(r"\b(January|February|March|April|May|June|July|August|September|October|November|December)\s+(\d{1,2})(?:,\s*(\d{4}))?\b").unwrap()
        });
        let capturas: Vec<(usize, usize, u32, u32, Option<i32>)> = RE
            .captures_iter(l.texto)
            .filter_map(|c| {
                let m = c.get(0)?;
                let mes = TABLA_MESES_EN.iter().position(|x| *x == &c[1])? as u32 + 1;
                let d = c[2].parse::<u32>().ok()?;
                let a = c.get(3).and_then(|x| x.as_str().parse::<i32>().ok());
                Some((m.start(), m.end(), d, mes, a))
            })
            .collect();
        for (ini, fin, d, mes, a) in capturas {
            if !(1..=31).contains(&d) || !l.libre(ini, fin) {
                continue;
            }
            let Some(dia) = rbnf::ordinal("en", d as i128, Genero::Masculino) else { continue };
            let nombre_mes = TABLA_MESES_EN[mes as usize - 1];
            let hablado = match a.and_then(|a| rbnf::anio("en", a as i128)) {
                Some(anio) => format!("{nombre_mes} {dia}, {anio}"),
                None => format!("{nombre_mes} {dia}"),
            };
            l.reclamar(ini, fin, ClaseSpan::Fecha, hablado);
        }
    }
}

static TABLA_MESES_EN: [&str; 12] = [
    "January", "February", "March", "April", "May", "June", "July", "August",
    "September", "October", "November", "December",
];

static RE_HORA: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(\d{1,2}):(\d{2})(?:\s?([AaPp])\.?[Mm]\.?)?\b").unwrap());

fn pasada_horas(l: &mut Lienzo, lang: &str, t: Option<&TablaIdioma>) {
    let capturas: Vec<(usize, usize, u32, u32, Option<char>)> = RE_HORA
        .captures_iter(l.texto)
        .filter_map(|c| {
            let m = c.get(0)?;
            let h = c[1].parse::<u32>().ok()?;
            let min = c[2].parse::<u32>().ok()?;
            let ampm = c.get(3).map(|x| x.as_str().chars().next().unwrap_or('a'));
            Some((m.start(), m.end(), h, min, ampm))
        })
        .collect();
    for (ini, mut fin, h, min, ampm) in capturas {
        if h > 23 || min > 59 || !l.libre(ini, fin) {
            continue;
        }
        // En alemán la hora escrita suele llevar «Uhr» detrás («14:30
        // Uhr»); la verbalización ya lo dice, así que el span se lo traga.
        if lang == "de" {
            let resto = &l.texto[fin..];
            let sin_espacios = resto.trim_start();
            if sin_espacios.starts_with("Uhr") {
                let consumido = resto.len() - sin_espacios.len() + 3;
                if l.libre(fin, fin + consumido) {
                    fin += consumido;
                }
            }
        }
        let hablado = match t {
            Some(t) => (t.hora)(h, min),
            None => {
                // Universal: hora y minutos en cardinal, separados.
                let hh = cardinal_txt(lang, &Numero::entero_de(h as i128));
                let mm = (min > 0)
                    .then(|| cardinal_txt(lang, &Numero::entero_de(min as i128)))
                    .flatten();
                match (hh, mm) {
                    (Some(hh), Some(mm)) => Some(format!("{hh} {mm}")),
                    (Some(hh), None) => Some(hh),
                    _ => None,
                }
            }
        };
        let Some(mut hablado) = hablado else { continue };
        if let Some(x) = ampm {
            hablado.push_str(if x.eq_ignore_ascii_case(&'a') { " AM" } else { " PM" });
        }
        l.reclamar(ini, fin, ClaseSpan::Hora, hablado);
    }
}

static RE_MONEDA_PRE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"([€$£¥])\s?(\d+(?:[.,\s\u{a0}]\d{3})*(?:[.,]\d{1,2})?)").unwrap()
});
static RE_MONEDA_POS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(\d+(?:[.,\s\u{a0}]\d{3})*(?:[.,]\d{1,2})?)\s?([€$£¥])").unwrap()
});

fn pasada_monedas(l: &mut Lienzo, t: &TablaIdioma) {
    let mut capturas: Vec<(usize, usize, String, String)> = Vec::new();
    for c in RE_MONEDA_PRE.captures_iter(l.texto) {
        let m = c.get(0).unwrap();
        capturas.push((m.start(), m.end(), c[1].to_string(), c[2].to_string()));
    }
    for c in RE_MONEDA_POS.captures_iter(l.texto) {
        let m = c.get(0).unwrap();
        capturas.push((m.start(), m.end(), c[2].to_string(), c[1].to_string()));
    }
    for (ini, fin, simbolo, crudo) in capturas {
        if !l.libre(ini, fin) {
            continue;
        }
        let Some(moneda) = t.monedas.iter().find(|m| m.simbolo == simbolo) else { continue };
        let Some(numero) = parsear_numero(&crudo, t.lang) else { continue };
        let centimos = numero
            .decimales
            .as_deref()
            .filter(|d| d.len() == 2)
            .and_then(|d| d.parse::<i128>().ok())
            .filter(|c| *c > 0);
        let entero = Numero { decimales: None, ..numero.clone() };
        let Some(parte_entera) = cardinal_txt(t.lang, &entero) else { continue };
        let palabra = if entero.entero == 1 { moneda.singular } else { moneda.plural };
        let hablado = match centimos {
            Some(c) if !moneda.sub_plural.is_empty() => {
                let Some(cent) = cardinal_txt(t.lang, &Numero::entero_de(c)) else { continue };
                let sub = if c == 1 { moneda.sub_singular } else { moneda.sub_plural };
                if moneda.nexo.is_empty() {
                    format!("{parte_entera} {palabra} {cent} {sub}")
                } else {
                    format!("{parte_entera} {palabra} {} {cent} {sub}", moneda.nexo)
                }
            }
            Some(c) => {
                let Some(cent) = cardinal_txt(t.lang, &Numero::entero_de(c)) else { continue };
                format!("{parte_entera} {palabra} {cent}")
            }
            None => format!("{parte_entera} {palabra}"),
        };
        l.reclamar(ini, fin, ClaseSpan::Moneda, hablado);
    }
}

static RE_PORCENTAJE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(\d+(?:[.,]\d+)?)\s?%").unwrap());

fn pasada_porcentajes(l: &mut Lienzo, t: &TablaIdioma) {
    let capturas: Vec<(usize, usize, String)> = RE_PORCENTAJE
        .captures_iter(l.texto)
        .map(|c| {
            let m = c.get(0).unwrap();
            (m.start(), m.end(), c[1].to_string())
        })
        .collect();
    for (ini, fin, crudo) in capturas {
        if !l.libre(ini, fin) {
            continue;
        }
        let Some(n) = parsear_numero(&crudo, t.lang) else { continue };
        let Some(num) = cardinal_txt(t.lang, &n) else { continue };
        l.reclamar(ini, fin, ClaseSpan::Porcentaje, format!("{num} {}", t.porcentaje));
    }
}

fn pasada_unidades(l: &mut Lienzo, t: &TablaIdioma) {
    for (simbolo, singular, plural) in t.unidades {
        let una_letra = simbolo.chars().count() == 1;
        let re = Regex::new(&format!(
            r"(\d+(?:[.,]\d+)?){}{}",
            if una_letra { "" } else { r"\s?" },
            regex::escape(simbolo)
        ))
        .unwrap();
        let capturas: Vec<(usize, usize, String)> = re
            .captures_iter(l.texto)
            .map(|c| {
                let m = c.get(0).unwrap();
                (m.start(), m.end(), c[1].to_string())
            })
            .collect();
        for (ini, fin, crudo) in capturas {
            // El símbolo no puede continuar en letra («10 ms» ya se capturó
            // antes que «10 m»; «5kmx» no es unidad).
            if l.texto[fin..].chars().next().is_some_and(|c| c.is_alphanumeric() || c == '²' || c == '³') {
                continue;
            }
            if !l.libre(ini, fin) {
                continue;
            }
            let Some(n) = parsear_numero(&crudo, t.lang) else { continue };
            let Some(num) = cardinal_txt(t.lang, &n) else { continue };
            let es_uno = n.entero == 1 && n.decimales.is_none();
            let palabra = if es_uno { singular } else { plural };
            l.reclamar(ini, fin, ClaseSpan::Unidad, format!("{num} {palabra}"));
        }
    }
}

fn pasada_ordinales(l: &mut Lienzo, t: &TablaIdioma) {
    // Alemán: «N.» seguido de mayúscula es ordinal, pero solo lo leemos
    // cuando la preposición anterior fija la declinación («im 19.
    // Jahrhundert» → «im neunzehnten Jahrhundert», «am 24. Mai» → «am
    // vierundzwanzigsten Mai»). Sin ese contexto, mejor no declinar a
    // ciegas.
    if t.lang == "de" {
        static RE_DE: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"\b(\d{1,3})\.\s+\p{Lu}").unwrap());
        let capturas: Vec<(usize, usize, i128)> = RE_DE
            .captures_iter(l.texto)
            .filter_map(|c| {
                let g = c.get(1)?;
                // Se reclama solo «N.»; la mayúscula siguiente era contexto.
                Some((g.start(), g.end() + 1, c[1].parse::<i128>().ok()?))
            })
            .collect();
        for (ini, fin, n) in capturas {
            if !l.libre(ini, fin) {
                continue;
            }
            let declinado = l
                .palabra_anterior(ini)
                .map(|p| p.to_lowercase())
                .is_some_and(|p| matches!(p.as_str(), "am" | "im" | "vom" | "zum" | "den" | "dem" | "der"));
            if !declinado {
                continue;
            }
            let Some(base) = rbnf::ordinal("de", n, Genero::Masculino) else { continue };
            l.reclamar(ini, fin, ClaseSpan::Ordinal, format!("{base}n"));
        }
        return;
    }

    let (re, genero_por_sufijo): (&Lazy<Regex>, bool) = match t.lang {
        "es" | "pt" | "it" => {
            static RE: Lazy<Regex> =
                Lazy::new(|| Regex::new(r"\b(\d+)\.?\s?([ºª°])").unwrap());
            (&RE, true)
        }
        "en" => {
            static RE: Lazy<Regex> =
                Lazy::new(|| Regex::new(r"\b(\d+)(st|nd|rd|th)\b").unwrap());
            (&RE, false)
        }
        "fr" => {
            static RE: Lazy<Regex> =
                Lazy::new(|| Regex::new(r"\b(\d+)(er|re|ère|ème|e)\b").unwrap());
            (&RE, false)
        }
        _ => return,
    };
    let capturas: Vec<(usize, usize, i128, String)> = re
        .captures_iter(l.texto)
        .filter_map(|c| {
            let m = c.get(0)?;
            let n = c[1].parse::<i128>().ok()?;
            Some((m.start(), m.end(), n, c[2].to_string()))
        })
        .collect();
    for (ini, fin, n, sufijo) in capturas {
        if !l.libre(ini, fin) {
            continue;
        }
        let genero = if genero_por_sufijo && sufijo == "ª" {
            Genero::Femenino
        } else if t.lang == "fr" && (sufijo == "re" || sufijo == "ère") {
            Genero::Femenino
        } else {
            Genero::Masculino
        };
        // El primero francés es «premier/première», no el «unième» que
        // CLDR reserva para los compuestos («vingt-et-unième»).
        let hablado = if t.lang == "fr" && n == 1 {
            Some(if genero == Genero::Femenino { "première".to_string() } else { "premier".to_string() })
        } else {
            rbnf::ordinal(t.lang, n, genero)
        };
        let Some(hablado) = hablado else { continue };
        l.reclamar(ini, fin, ClaseSpan::Ordinal, hablado);
    }
}

static RE_ROMANO: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b([MDCLXVI]+)(?:(e|ème|er)\b)?").unwrap());

fn pasada_romanos(l: &mut Lienzo, t: &TablaIdioma) {
    let capturas: Vec<(usize, usize, String)> = RE_ROMANO
        .captures_iter(l.texto)
        .map(|c| {
            let m = c.get(0).unwrap();
            (m.start(), m.end(), c[1].to_string())
        })
        .collect();
    for (ini, fin, crudo) in capturas {
        if !l.libre(ini, fin) {
            continue;
        }
        let Some(n) = romano_a_entero(&crudo) else { continue };
        let anterior = l.palabra_anterior(ini);
        let siguiente = l.palabra_siguiente(fin);

        let minus = |s: Option<&str>| s.map(|w| w.to_lowercase());
        let ant = minus(anterior);
        let sig = minus(siguiente);
        let en_lista = |palabra: &Option<String>, lista: &[&str]| {
            palabra
                .as_deref()
                .is_some_and(|p| lista.iter().any(|x| x.eq_ignore_ascii_case(p)))
        };

        let hablado = if en_lista(&ant, t.palabras_siglo) || en_lista(&sig, t.palabras_siglo) {
            (t.romano_siglo)(n)
        } else if en_lista(&ant, t.palabras_capitulo) {
            (t.romano_capitulo)(n)
        } else if crudo.chars().count() >= 2 || matches!(crudo.as_str(), "I" | "V" | "X") {
            // Nombre propio delante («Enrique VIII», «Louis XIV»): el
            // anterior es Capitalizado normal, no VERSALES (que serían un
            // título tipo «EL CID» donde CID parecería romano).
            match anterior {
                Some(p) if es_nombre_propio(p) && !es_mayusculas(p) => {
                    if crudo.chars().count() == 1 && !matches!(crudo.as_str(), "I" | "V" | "X") {
                        None
                    } else {
                        (t.romano_nombre)(n)
                    }
                }
                _ => None,
            }
        } else {
            None
        };

        if let Some(h) = hablado {
            l.reclamar(ini, fin, ClaseSpan::Romano, h);
        }
    }
}

static RE_SIGLA: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b([A-ZÑ]{2,6})\b").unwrap());

fn pasada_siglas(l: &mut Lienzo, t: &TablaIdioma) {
    let capturas: Vec<(usize, usize, String)> = RE_SIGLA
        .captures_iter(l.texto)
        .map(|c| {
            let m = c.get(0).unwrap();
            (m.start(), m.end(), c[1].to_string())
        })
        .collect();
    for (ini, fin, sigla) in capturas {
        if !l.libre(ini, fin) {
            continue;
        }
        // Solo se deletrean las impronunciables. Heurística fonotáctica:
        // una sigla se puede leer como palabra (NASA, UNESCO, OTAN) si
        // TODAS sus consonantes tienen una vocal al lado; si alguna queda
        // encajonada entre consonantes (FBI, BBC, DVD), se deletrea.
        let es_vocal = |c: char| "AEIOUÁÉÍÓÚ".contains(c);
        let cs: Vec<char> = sigla.chars().collect();
        let pronunciable = cs.iter().enumerate().all(|(i, c)| {
            es_vocal(*c)
                || (i > 0 && es_vocal(cs[i - 1]))
                || (i + 1 < cs.len() && es_vocal(cs[i + 1]))
        }) && cs.iter().any(|c| es_vocal(*c));
        if pronunciable {
            continue;
        }
        // Un romano válido sin contexto (ya rechazado antes) se queda como
        // está: mejor «XVI» leído por el modelo que «equis uve i».
        if romano_a_entero(&sigla).is_some() {
            continue;
        }
        let letras: Option<Vec<&str>> = sigla
            .chars()
            .map(|c| {
                t.letras
                    .iter()
                    .find(|(l, _)| *l == c.to_lowercase().next().unwrap_or(c))
                    .map(|(_, nombre)| *nombre)
            })
            .collect();
        if let Some(letras) = letras {
            l.reclamar(ini, fin, ClaseSpan::Sigla, letras.join(" "));
        }
    }
}

fn pasada_abreviaturas(l: &mut Lienzo, t: &TablaIdioma) {
    for (clave, valor) in t.abreviaturas {
        let solo_ante_numero = matches!(*clave, "p." | "pp." | "pág." | "págs." | "n.º" | "nº" | "n." | "No." | "Nr." | "n°" | "N°");
        let re = Regex::new(&format!(r"(?:^|[\s(«\[])({})", regex::escape(clave))).unwrap();
        let capturas: Vec<(usize, usize)> = re
            .captures_iter(l.texto)
            .filter_map(|c| c.get(1).map(|g| (g.start(), g.end())))
            .collect();
        for (ini, fin) in capturas {
            if !l.libre(ini, fin) {
                continue;
            }
            // La clave con punto no puede continuar en letra («art.» no
            // debe comerse el principio de «artículo» escrito entero).
            if l.texto[fin..].chars().next().is_some_and(|c| c.is_alphanumeric()) {
                continue;
            }
            if solo_ante_numero {
                let sig = l.texto[fin..].trim_start();
                if !sig.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                    continue;
                }
            }
            l.reclamar(ini, fin, ClaseSpan::Abreviatura, valor.to_string());
        }
    }
}

static RE_GRUPO_MILES: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b\d{1,3}(?:[.,\u{a0} ]\d{3})+(?:[.,]\d{1,2})?\b").unwrap());
static RE_DECIMAL: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b\d+[.,]\d+\b").unwrap());
static RE_ENTERO: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b\d+\b").unwrap());

fn pasada_numeros(l: &mut Lienzo, lang: &str) {
    // Números con separador de miles primero (para no partirlos), luego
    // decimales, luego enteros. Los años entran por el cardinal salvo en
    // inglés, donde 1985 se lee «nineteen eighty-five».
    let grupos: Vec<(usize, usize, String)> = RE_GRUPO_MILES
        .find_iter(l.texto)
        .map(|m| (m.start(), m.end(), m.as_str().to_string()))
        .collect();
    for (ini, fin, crudo) in grupos {
        if !l.libre(ini, fin) {
            continue;
        }
        if let Some(n) = parsear_numero(&crudo, lang) {
            if let Some(h) = cardinal_txt(lang, &n) {
                l.reclamar(ini, fin, ClaseSpan::Cardinal, h);
            }
        }
    }

    let decimales: Vec<(usize, usize, String)> = RE_DECIMAL
        .find_iter(l.texto)
        .map(|m| (m.start(), m.end(), m.as_str().to_string()))
        .collect();
    for (ini, fin, crudo) in decimales {
        if !l.libre(ini, fin) {
            continue;
        }
        if let Some(n) = parsear_numero(&crudo, lang) {
            if let Some(h) = cardinal_txt(lang, &n) {
                l.reclamar(ini, fin, ClaseSpan::Decimal, h);
            }
        }
    }

    let enteros: Vec<(usize, usize, String)> = RE_ENTERO
        .find_iter(l.texto)
        .map(|m| (m.start(), m.end(), m.as_str().to_string()))
        .collect();
    for (ini, fin, crudo) in enteros {
        if !l.libre(ini, fin) {
            continue;
        }
        let Ok(n) = crudo.parse::<i128>() else { continue };
        // En inglés, un año de cuatro cifras se lee como año.
        let hablado = if lang == "en" && (1100..=2199).contains(&n) && crudo.len() == 4 {
            rbnf::anio(lang, n)
        } else {
            cardinal_txt(lang, &Numero::entero_de(n))
        };
        if let Some(h) = hablado {
            let clase = if crudo.len() == 4 && (1100..=2199).contains(&n) {
                ClaseSpan::Anio
            } else {
                ClaseSpan::Cardinal
            };
            l.reclamar(ini, fin, clase, h);
        }
    }
}

// ── Entrada pública ──────────────────────────────────────────────────────

/// Clasifica y verbaliza el texto de una pieza. Devuelve la partición
/// completa en spans (los literales incluidos), con rangos en caracteres.
pub fn verbalizar(texto: &str, lang: &str) -> Vec<Span> {
    let mut lienzo = Lienzo::new(texto);
    pasada_electronico(&mut lienzo);

    if let Some(t) = tabla(lang) {
        pasada_fechas(&mut lienzo, t);
        pasada_horas(&mut lienzo, lang, Some(t));
        pasada_monedas(&mut lienzo, t);
        pasada_porcentajes(&mut lienzo, t);
        pasada_unidades(&mut lienzo, t);
        pasada_ordinales(&mut lienzo, t);
        pasada_romanos(&mut lienzo, t);
        pasada_abreviaturas(&mut lienzo, t);
        pasada_siglas(&mut lienzo, t);
    } else {
        pasada_horas(&mut lienzo, lang, None);
    }

    pasada_numeros(&mut lienzo, lang);
    lienzo.terminar()
}

/// El texto hablado completo de una pieza: los spans concatenados.
pub fn texto_hablado(spans: &[Span]) -> String {
    spans.iter().map(|s| s.hablado.as_str()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn habla(texto: &str, lang: &str) -> String {
        let spans = verbalizar(texto, lang);
        // Normaliza espacios para comparar cómodamente.
        texto_hablado(&spans).split_whitespace().collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn espanol_basico() {
        assert_eq!(habla("En 1492 zarparon.", "es"), "En mil cuatrocientos noventa y dos zarparon.");
        assert_eq!(habla("el siglo XX fue corto", "es"), "el siglo veinte fue corto");
        assert_eq!(habla("el siglo IX fue largo", "es"), "el siglo noveno fue largo");
        assert_eq!(habla("Enrique VIII tuvo seis esposas", "es"), "Enrique octavo tuvo seis esposas");
        assert_eq!(habla("Alfonso XIII reinó", "es"), "Alfonso trece reinó");
        assert_eq!(habla("EL CID CAMPEADOR", "es"), "EL CID CAMPEADOR");
        assert_eq!(habla("DERECHO CIVIL", "es"), "DERECHO CIVIL");
        assert_eq!(habla("subió un 50%", "es"), "subió un cincuenta por ciento");
        assert_eq!(habla("el capítulo 21º", "es"), "el capítulo vigésimo primero");
        assert_eq!(habla("la 2ª edición", "es"), "la segunda edición");
        assert_eq!(habla("cuesta 3,50 €", "es"), "cuesta tres euros con cincuenta céntimos");
        assert_eq!(habla("son las 14:30", "es"), "son las catorce y treinta");
        assert_eq!(habla("mide 1,75 m de alto", "es"), "mide uno coma setenta y cinco m de alto");
        assert_eq!(habla("pesa 70kg", "es"), "pesa setenta kilogramos");
        assert_eq!(habla("el FBI investiga", "es"), "el efe be i investiga");
        assert_eq!(habla("la NASA lanzó", "es"), "la NASA lanzó");
        assert_eq!(habla("Dr. Ramírez, pág. 12", "es"), "doctor Ramírez, página doce");
        assert_eq!(habla("1.234 personas", "es"), "mil doscientos treinta y cuatro personas");
    }

    #[test]
    fn espanol_fechas() {
        assert_eq!(
            habla("el 24/05/2026 salió", "es"),
            "el veinticuatro de mayo de dos mil veintiséis salió"
        );
        assert_eq!(
            habla("2026-05-24", "es"),
            "veinticuatro de mayo de dos mil veintiséis"
        );
    }

    #[test]
    fn ingles_basico() {
        assert_eq!(habla("in 1985 it began", "en"), "in nineteen eighty-five it began");
        assert_eq!(habla("Henry VIII had six wives", "en"), "Henry the eighth had six wives");
        assert_eq!(habla("World War II ended", "en"), "World War two ended");
        assert_eq!(habla("the 21st century", "en"), "the twenty-first century");
        assert_eq!(habla("it costs $3.50", "en"), "it costs three dollars fifty cents");
        assert_eq!(habla("May 24, 2026", "en"), "May twenty-fourth, twenty twenty-six");
        assert_eq!(habla("50% of people", "en"), "fifty percent of people");
    }

    #[test]
    fn frances_aleman() {
        assert_eq!(habla("Louis XIV régna", "fr"), "Louis quatorze régna");
        assert_eq!(habla("le XIXe siècle", "fr"), "le dix-neuvième siècle");
        assert_eq!(habla("le 1er prix", "fr"), "le premier prix");
        assert_eq!(habla("es kostet 3,50 €", "de"), "es kostet drei Euro fünfzig Cent");
        assert_eq!(habla("um 14:30 Uhr", "de"), "um vierzehn Uhr dreißig");
        assert_eq!(habla("im 19. Jahrhundert", "de"), "im neunzehnten Jahrhundert");
        assert_eq!(habla("am 24. Mai", "de"), "am vierundzwanzigsten Mai");
    }

    #[test]
    fn idiomas_sin_tabla_no_se_tocan_las_palabras() {
        // Neerlandés: sin tabla profunda, los números sí, el resto intacto.
        assert_eq!(habla("het jaar 1985", "nl"), "het jaar duizendnegenhonderdvijfentachtig");
        assert_eq!(habla("Dr. Jansen", "nl"), "Dr. Jansen");
    }

    #[test]
    fn spans_particionan_completo() {
        let texto = "En 1492, Colón zarpó con 3 naves.";
        let spans = verbalizar(texto, "es");
        let total: usize = spans.iter().map(|s| s.original.len()).sum();
        assert_eq!(total, texto.chars().count());
        // Y son contiguos y ordenados.
        let mut cursor = 0;
        for s in &spans {
            assert_eq!(s.original.start, cursor);
            cursor = s.original.end;
        }
    }

    #[test]
    fn urls_se_omiten() {
        assert_eq!(habla("mira https://example.com/x ya", "es"), "mira ya");
    }
}
