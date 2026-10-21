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
    reclamado: Vec<bool>,                          // por byte
    spans: Vec<(usize, usize, ClaseSpan, String)>, // rangos en bytes
}

impl<'a> Lienzo<'a> {
    fn new(texto: &'a str) -> Self {
        Lienzo {
            texto,
            reclamado: vec![false; texto.len()],
            spans: Vec::new(),
        }
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
        // Un signo fuerte entre la palabra y la posición rompe el contexto:
        // en «…the century; D…», «century» ya no acompaña a la «D».
        if palabra.ends_with([';', ':', '!', '?', '…']) {
            return None;
        }
        let palabra = palabra
            .trim_matches(|c: char| !(c.is_alphanumeric() || c == '.' || c == 'º' || c == 'ª'));
        if palabra.is_empty() {
            None
        } else {
            Some(palabra)
        }
    }

    /// La palabra siguiente al byte `fin`.
    fn palabra_siguiente(&self, fin: usize) -> Option<&'a str> {
        let despues = self.texto[fin..].trim_start();
        let final_ = despues
            .find(|c: char| c.is_whitespace())
            .unwrap_or(despues.len());
        let palabra = despues[..final_].trim_matches(|c: char| !(c.is_alphanumeric() || c == '.'));
        if palabra.is_empty() {
            None
        } else {
            Some(palabra)
        }
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
    let (miles, decimal) = if decimal_con_punto(lang) {
        (',', '.')
    } else {
        ('.', ',')
    };
    let limpio: String = crudo
        .chars()
        .filter(|c| *c != miles && *c != ' ' && *c != '\u{a0}')
        .collect();
    let (ent, dec) = match limpio.split_once(decimal) {
        Some((e, d)) => (e, Some(d.to_string())),
        None => (limpio.as_str(), None),
    };
    let entero: i128 = ent.parse().ok()?;
    Some(Numero {
        negativo: false,
        entero,
        decimales: dec,
    })
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
            'I' => 1,
            'V' => 5,
            'X' => 10,
            'L' => 50,
            'C' => 100,
            'D' => 500,
            'M' => 1000,
            _ => 0,
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

static RE_URL: Lazy<Regex> = Lazy::new(|| Regex::new(r"https?://\S+|www\.\S+").unwrap());
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
            let Some(dia) = rbnf::ordinal("en", d as i128, Genero::Masculino) else {
                continue;
            };
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
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
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
            hablado.push_str(if x.eq_ignore_ascii_case(&'a') {
                " AM"
            } else {
                " PM"
            });
        }
        l.reclamar(ini, fin, ClaseSpan::Hora, hablado);
    }
}

static RE_MONEDA_PRE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"([€$£¥])\s?(\d+(?:[.,\s\u{a0}]\d{3})*(?:[.,]\d{1,2})?)").unwrap());
static RE_MONEDA_POS: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(\d+(?:[.,\s\u{a0}]\d{3})*(?:[.,]\d{1,2})?)\s?([€$£¥])").unwrap());

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
        let Some(moneda) = t.monedas.iter().find(|m| m.simbolo == simbolo) else {
            continue;
        };
        let Some(numero) = parsear_numero(&crudo, t.lang) else {
            continue;
        };
        let centimos = numero
            .decimales
            .as_deref()
            .filter(|d| d.len() == 2)
            .and_then(|d| d.parse::<i128>().ok())
            .filter(|c| *c > 0);
        let entero = Numero {
            decimales: None,
            ..numero.clone()
        };
        let Some(parte_entera) = cardinal_txt(t.lang, &entero) else {
            continue;
        };
        let palabra = if entero.entero == 1 {
            moneda.singular
        } else {
            moneda.plural
        };
        let hablado = match centimos {
            Some(c) if !moneda.sub_plural.is_empty() => {
                let Some(cent) = cardinal_txt(t.lang, &Numero::entero_de(c)) else {
                    continue;
                };
                let sub = if c == 1 {
                    moneda.sub_singular
                } else {
                    moneda.sub_plural
                };
                if moneda.nexo.is_empty() {
                    format!("{parte_entera} {palabra} {cent} {sub}")
                } else {
                    format!("{parte_entera} {palabra} {} {cent} {sub}", moneda.nexo)
                }
            }
            Some(c) => {
                let Some(cent) = cardinal_txt(t.lang, &Numero::entero_de(c)) else {
                    continue;
                };
                format!("{parte_entera} {palabra} {cent}")
            }
            None => format!("{parte_entera} {palabra}"),
        };
        l.reclamar(ini, fin, ClaseSpan::Moneda, hablado);
    }
}

static RE_PORCENTAJE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(\d+(?:[.,]\d+)?)\s?%").unwrap());

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
        let Some(n) = parsear_numero(&crudo, t.lang) else {
            continue;
        };
        let Some(num) = cardinal_txt(t.lang, &n) else {
            continue;
        };
        l.reclamar(
            ini,
            fin,
            ClaseSpan::Porcentaje,
            format!("{num} {}", t.porcentaje),
        );
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
            if l.texto[fin..]
                .chars()
                .next()
                .is_some_and(|c| c.is_alphanumeric() || c == '²' || c == '³')
            {
                continue;
            }
            if !l.libre(ini, fin) {
                continue;
            }
            let Some(n) = parsear_numero(&crudo, t.lang) else {
                continue;
            };
            let Some(num) = cardinal_txt(t.lang, &n) else {
                continue;
            };
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
        static RE_DE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b(\d{1,3})\.\s+\p{Lu}").unwrap());
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
                .is_some_and(|p| {
                    matches!(
                        p.as_str(),
                        "am" | "im" | "vom" | "zum" | "den" | "dem" | "der"
                    )
                });
            if !declinado {
                continue;
            }
            let Some(base) = rbnf::ordinal("de", n, Genero::Masculino) else {
                continue;
            };
            l.reclamar(ini, fin, ClaseSpan::Ordinal, format!("{base}n"));
        }
        return;
    }

    let (re, genero_por_sufijo): (&Lazy<Regex>, bool) = match t.lang {
        "es" | "pt" | "it" => {
            static RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b(\d+)\.?\s?([ºª°])").unwrap());
            (&RE, true)
        }
        "en" => {
            static RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b(\d+)(st|nd|rd|th)\b").unwrap());
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
        let genero = if (genero_por_sufijo && sufijo == "ª")
            || (t.lang == "fr" && (sufijo == "re" || sufijo == "ère"))
        {
            Genero::Femenino
        } else {
            Genero::Masculino
        };
        // El primero francés es «premier/première», no el «unième» que
        // CLDR reserva para los compuestos («vingt-et-unième»).
        let hablado = if t.lang == "fr" && n == 1 {
            Some(if genero == Genero::Femenino {
                "première".to_string()
            } else {
                "premier".to_string()
            })
        } else {
            rbnf::ordinal(t.lang, n, genero)
        };
        let Some(hablado) = hablado else { continue };
        l.reclamar(ini, fin, ClaseSpan::Ordinal, hablado);
    }
}

// La frontera FINAL importa: sin ella, la «D» de «Dr.» casaba sola como
// romano (quinientos) y se comía la abreviatura.
static RE_ROMANO: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b([MDCLXVI]+)(?:(e|ème|er))?\b").unwrap());

/// Nombres de reyes, reinas, papas y emperadores. En inglés una «I», «V» o
/// «X» SUELTAS solo se leen como ordinal tras uno de estos: la «I» es el
/// pronombre, y cualquier palabra capitalizada delante («Then I said»,
/// «Mom I», «Paris I») no basta para hacerla numeral.
static NOMBRES_REGIOS: &[&str] = &[
    "Abdulhamid",
    "Adrian",
    "Afonso",
    "Ahmed",
    "Albert",
    "Alexander",
    "Alexios",
    "Alfonso",
    "Amadeus",
    "Amenhotep",
    "Anastasius",
    "Andrew",
    "Anne",
    "Antiochus",
    "Artaxerxes",
    "Baldwin",
    "Bayezid",
    "Bela",
    "Benedict",
    "Boleslaw",
    "Boniface",
    "Carlos",
    "Casimir",
    "Catherine",
    "Celestine",
    "Charles",
    "Childeric",
    "Christian",
    "Christina",
    "Clement",
    "Clovis",
    "Cnut",
    "Conrad",
    "Constantine",
    "Cyrus",
    "Darius",
    "David",
    "Duarte",
    "Duncan",
    "Edmund",
    "Edward",
    "Elizabeth",
    "Emmanuel",
    "Eric",
    "Erik",
    "Eugene",
    "Faisal",
    "Felipe",
    "Ferdinand",
    "Fernando",
    "Francis",
    "Franz",
    "Frederick",
    "Friedrich",
    "Fuad",
    "George",
    "Gregory",
    "Gustav",
    "Gustavus",
    "Haakon",
    "Harald",
    "Henry",
    "Honorius",
    "Hussein",
    "Innocent",
    "Isabella",
    "Ivan",
    "Jacob",
    "James",
    "Joao",
    "João",
    "John",
    "Joseph",
    "Juan",
    "Julius",
    "Justinian",
    "Karl",
    "Kenneth",
    "Ladislaus",
    "Leo",
    "Leopold",
    "Louis",
    "Ludwig",
    "Magnus",
    "Malcolm",
    "Manuel",
    "Margaret",
    "Margrethe",
    "Maria",
    "Martin",
    "Mary",
    "Matilda",
    "Maximilian",
    "Mehmed",
    "Michael",
    "Mieszko",
    "Murad",
    "Napoleon",
    "Nicholas",
    "Olaf",
    "Osman",
    "Otto",
    "Paul",
    "Pedro",
    "Peter",
    "Philip",
    "Pius",
    "Ptolemy",
    "Rama",
    "Ramesses",
    "Ramses",
    "Richard",
    "Robert",
    "Roger",
    "Rudolf",
    "Sancho",
    "Sebastian",
    "Seleucus",
    "Selim",
    "Seti",
    "Sigismund",
    "Sixtus",
    "Stephen",
    "Suleiman",
    "Sylvester",
    "Theodosius",
    "Thutmose",
    "Umberto",
    "Urban",
    "Valdemar",
    "Vasily",
    "Victor",
    "Wilhelm",
    "William",
    "Xerxes",
];

/// Verbos que tras «I» delatan al pronombre aunque delante haya un nombre
/// de rey («John I guess», «Mary I think»). «was», «had», «died» no
/// entran: «Henry I was king» es el rey.
static VERBOS_DE_YO: &[&str] = &[
    "am", "guess", "think", "mean", "suppose", "believe", "hope", "wish", "know", "feel", "want",
    "love", "swear", "promise", "bet", "reckon", "need", "can", "can't", "cannot", "don't",
    "won't", "didn't", "do", "will", "would", "should", "could", "must", "might", "have", "just",
    "really", "also", "never", "always", "still",
];

/// Lo que precede al romano es una palabra pegada por espacio, sin coma,
/// punto ni otro signo en medio. «Paris. I think» o «Mom, I» no dan
/// contexto de nombre propio.
fn pegado_a_palabra(texto: &str, ini: usize) -> bool {
    texto[..ini]
        .trim_end()
        .chars()
        .next_back()
        .is_some_and(|c| c.is_alphabetic())
        && texto[..ini].ends_with(char::is_whitespace)
}

/// El romano abre la pieza como encabezado: la pieza entera es el romano
/// («I», «IV.») o el romano va seguido de punto o paréntesis y del título
/// («I. Introduction», «IV) Results»).
fn romano_encabezado(texto: &str, ini: usize, fin: usize, crudo: &str) -> bool {
    if !texto[..ini].trim().is_empty() {
        return false;
    }
    let resto = &texto[fin..];
    let resto_limpio = resto.trim_end();
    if resto_limpio.is_empty() || resto_limpio == "." {
        return true;
    }
    if !crudo.chars().all(|c| matches!(c, 'I' | 'V' | 'X')) {
        return false;
    }
    let Some(tras) = resto.strip_prefix(['.', ')']) else {
        return false;
    };
    if !tras.starts_with(char::is_whitespace) {
        return false;
    }
    // «V. S. Naipaul», «X. Y. Z.»: si lo que sigue es otra inicial, el
    // romano era una inicial de nombre.
    let siguiente = tras.trim_start();
    let mut cs = siguiente.chars();
    let inicial = cs.next().is_some_and(|c| c.is_uppercase()) && cs.next() == Some('.');
    !inicial && !siguiente.is_empty()
}

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
        let Some(n) = romano_a_entero(&crudo) else {
            continue;
        };
        let una_letra = crudo.chars().count() == 1;
        // Una letra sola pegada a apóstrofo o guion es una palabra o una
        // marca, no un número: «I'm», «I'll», «I’d», «X-ray», «V-2».
        if una_letra && l.texto[fin..].starts_with(['\'', '’', '-', '‑']) {
            continue;
        }
        // En inglés la «I» suelta es el pronombre salvo prueba en contra.
        let pronombre = t.lang == "en" && crudo == "I";
        let anterior = l.palabra_anterior(ini);
        let siguiente = l.palabra_siguiente(fin);

        let minus = |s: Option<&str>| s.map(|w| w.to_lowercase());
        let ant = minus(anterior);
        let sig = minus(siguiente);
        let en_lista = |palabra: &Option<String>, lista: &[&str]| {
            palabra.as_deref().is_some_and(|p| {
                // «del XX secolo.» llega con el punto final de la frase
                // pegado; para comparar con la lista se pela la puntuación
                // de cierre (sin tocar claves con punto propio como «s.»).
                let pelado = p.trim_end_matches(['.', ',', ';', ':']);
                lista
                    .iter()
                    .any(|x| x.eq_ignore_ascii_case(p) || x.eq_ignore_ascii_case(pelado))
            })
        };
        // Las letras C, D, L, M solas casi nunca son romanos: «Appendix C»,
        // «Class D», «vitamin C», «Type L». Solo I, V y X sueltas cuentan.
        let letra_numeral = !una_letra || matches!(crudo.as_str(), "I" | "V" | "X");
        let pegado = pegado_a_palabra(l.texto, ini);

        // Ningún siglo real pasa de cien: sin este tope, un falso contexto
        // convertía romanos enormes en ordinales absurdos.
        let hablado = if letra_numeral
            && !pronombre
            && (en_lista(&ant, t.palabras_siglo) || en_lista(&sig, t.palabras_siglo))
            && (1..=100).contains(&n)
        {
            (t.romano_siglo)(n)
        } else if letra_numeral
            && pegado
            && en_lista(&ant, t.palabras_capitulo)
            // «the part I played», «the war I fought»: en inglés la «I» solo
            // es numeral tras la palabra de división CAPITALIZADA
            // («Chapter I», «World War I», «PART I»).
            && (!pronombre
                || anterior.is_some_and(|p| p.chars().next().is_some_and(char::is_uppercase)))
        {
            (t.romano_capitulo)(n)
        } else if romano_encabezado(l.texto, ini, fin, &crudo) && letra_numeral {
            // «I. Introduction», «IV.» solo en su línea: encabezado numerado.
            (t.romano_capitulo)(n)
        } else if pegado && crudo.chars().all(|c| matches!(c, 'I' | 'V' | 'X')) {
            // Nombre propio delante («Enrique VIII», «Louis XIV»): el
            // anterior es Capitalizado normal, no VERSALES (que serían un
            // título tipo «EL CID» donde CID parecería romano). Ningún
            // ordinal regio pasa de XXXIX: con C, D, L o M es otra cosa
            // («Washington DC», «John Smith MD»).
            match anterior {
                Some(p) if es_nombre_propio(p) && !es_mayusculas(p) => {
                    let regio = NOMBRES_REGIOS.contains(&p);
                    if p == "Malcolm" && crudo == "X" {
                        None
                    } else if (t.lang == "en" && una_letra) && !regio {
                        // En inglés, la «I» (y la «V» o la «X» sueltas) solo
                        // tras nombre de rey o papa: «Henry I», «Charles V».
                        None
                    } else if pronombre
                        && sig.as_deref().is_some_and(|s| {
                            VERBOS_DE_YO.contains(&s.trim_end_matches(['.', ',', ';', ':']))
                        })
                    {
                        // «John I guess…», «Mary I think…»: verbo que solo
                        // conjuga con el pronombre; «Henry I was» sí es rey.
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
            // Alemán: «Heinrich VIII.» escribe el ordinal con punto. Si la
            // frase sigue en minúscula, ese punto pertenece al romano y se
            // reclama con él; si no, era un final de oración y se respeta.
            let mut fin_real = fin;
            if t.lang == "de" && l.texto[fin..].starts_with('.') {
                let resto = l.texto[fin + 1..].trim_start();
                if resto.chars().next().is_some_and(|c| c.is_lowercase()) {
                    fin_real = fin + 1;
                }
            }
            l.reclamar(ini, fin_real, ClaseSpan::Romano, h);
        }
    }
}

static RE_SIGLA: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b(\p{Lu}{2,12})\b").unwrap());

/// Las 4.000 palabras más frecuentes de cada idioma con tabla, en su forma
/// habitual («knew», «Haus»). Sacadas de libros de dominio público del
/// Proyecto Gutenberg (tooling/frecuentes/): sin licencias de por medio.
/// Una palabra en VERSALES cuya forma en minúsculas está aquí es énfasis
/// («it KNEW», «DOES exhaust», «NO quiero»), no una sigla.
static FRECUENTES: Lazy<
    std::collections::HashMap<&'static str, std::collections::HashMap<String, &'static str>>,
> = Lazy::new(|| {
    let fuentes: [(&str, &str); 6] = [
        ("en", include_str!("frecuentes/en.txt")),
        ("es", include_str!("frecuentes/es.txt")),
        ("fr", include_str!("frecuentes/fr.txt")),
        ("de", include_str!("frecuentes/de.txt")),
        ("it", include_str!("frecuentes/it.txt")),
        ("pt", include_str!("frecuentes/pt.txt")),
    ];
    fuentes
        .into_iter()
        .map(|(lang, lista)| {
            let m = lista
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(|l| (l.to_lowercase(), l))
                .collect();
            (lang, m)
        })
        .collect()
});

/// Siglas que también son palabras corrientes: en VERSALES casi siempre son
/// la sigla («the US economy», «the WHO said», «USA» en un texto italiano,
/// donde «usa» es un verbo).
const SIGLAS_QUE_SON_PALABRAS: &[&str] = &[
    "US", "USA", "UK", "IT", "WHO", "EU", "UE", "UN", "ONU", "ID", "AM", "PM", "TV", "PC", "CD",
    "DJ", "PR", "OK", "AI", "IA", "OMS", "ONG", "NGO", "PIB", "GDP", "AIDS", "SIDA", "NATO",
    "OTAN", "CIA", "MI", "ETA", "RAE", "UNO",
];

/// La forma de la palabra común para leerla como énfasis: la habitual del
/// idioma («knew», «Haus»), con mayúscula si abre frase.
fn forma_de_enfasis(versal: &str, lang: &str, abre_frase: bool) -> Option<String> {
    if SIGLAS_QUE_SON_PALABRAS.contains(&versal) {
        return None;
    }
    let canon = FRECUENTES.get(lang)?.get(&versal.to_lowercase())?;
    if abre_frase {
        let mut cs = canon.chars();
        let primera = cs.next()?;
        Some(primera.to_uppercase().chain(cs).collect())
    } else {
        Some(canon.to_string())
    }
}

/// ¿La palabra que empieza en `ini` abre frase? (principio de pieza, o tras
/// punto, signo de apertura, comillas o raya).
fn abre_frase(texto: &str, ini: usize) -> bool {
    let antes = texto[..ini].trim_end_matches(|c: char| {
        c.is_whitespace()
            || matches!(
                c,
                '"' | '“' | '«' | '‘' | '\'' | '(' | '¿' | '¡' | '—' | '–' | '*' | '_'
            )
    });
    match antes.chars().next_back() {
        None => true,
        Some(c) => matches!(c, '.' | '!' | '?' | '…'),
    }
}

/// Un titular en VERSALES («WHAT I LEARNED IN SCHOOL»): casi todas sus
/// palabras van en mayúsculas. Ahí las «siglas» son palabras normales y
/// deletrearlas («W H A T») es un destrozo.
fn es_titular_en_versales(texto: &str) -> bool {
    let palabras: Vec<&str> = texto
        .split_whitespace()
        .filter(|p| p.chars().filter(|c| c.is_alphabetic()).count() >= 2)
        .collect();
    let versales = palabras.iter().filter(|p| es_mayusculas(p)).count();
    // Entero en versales («DERECHO CIVIL», «CHAPTER I»), o casi entero si es
    // largo.
    (versales >= 1 && versales == palabras.len())
        || (versales >= 3 && versales * 10 >= palabras.len() * 8)
}

/// Palabras en VERSALES: énfasis o sigla.
///
/// 1. Si su forma en minúsculas es una palabra común del idioma (lista de
///    frecuencias embebida), es ÉNFASIS: el modelo la recibe en minúsculas
///    (o capitalizada si abre frase), como cualquier otra palabra.
/// 2. Si no, y se puede pronunciar (todas sus consonantes tienen una vocal
///    al lado: NASA, UNESCO, OTAN), se deja tal cual: es sigla que se lee.
/// 3. Si no, se deletrea (FBI, HTML, BBC, JPEG). Las de cinco letras o más
///    con alguna vocal que no están en la lista se dejan al modelo: son
///    palabras raras de énfasis («SMOOCH») más que siglas.
fn pasada_siglas(l: &mut Lienzo, t: &TablaIdioma) {
    // Un titular entero en versales se deja como está (ni énfasis ni
    // siglas): el modelo lo lee como el título que es.
    if es_titular_en_versales(l.texto) {
        return;
    }
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
        if let Some(forma) = forma_de_enfasis(&sigla, t.lang, abre_frase(l.texto, ini)) {
            if forma != sigla {
                l.reclamar(ini, fin, ClaseSpan::Versal, forma);
            }
            continue;
        }
        let cs: Vec<char> = sigla.chars().collect();
        if cs.len() > 6 {
            continue;
        }
        // Heurística fonotáctica: una sigla se puede leer como palabra
        // (NASA, UNESCO, OTAN) si TODAS sus consonantes tienen una vocal al
        // lado; si alguna queda encajonada entre consonantes (FBI, BBC,
        // DVD), se deletrea.
        let es_vocal = |c: char| "AEIOUÁÉÍÓÚÀÈÌÒÙÂÊÎÔÛÄËÏÖÜ".contains(c);
        let pronunciable = cs.iter().enumerate().all(|(i, c)| {
            es_vocal(*c)
                || (i > 0 && es_vocal(cs[i - 1]))
                || (i + 1 < cs.len() && es_vocal(cs[i + 1]))
        }) && cs.iter().any(|c| es_vocal(*c));
        if pronunciable {
            continue;
        }
        let vocales = cs.iter().filter(|c| es_vocal(**c)).count();
        if cs.len() >= 5 && vocales >= 1 {
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
        let solo_ante_numero = matches!(
            *clave,
            "p." | "pp." | "pág." | "págs." | "n.º" | "nº" | "n." | "No." | "Nr." | "n°" | "N°"
        );
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
            if l.texto[fin..]
                .chars()
                .next()
                .is_some_and(|c| c.is_alphanumeric())
            {
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
        let Ok(n) = crudo.parse::<i128>() else {
            continue;
        };
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
        texto_hablado(&spans)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn espanol_basico() {
        assert_eq!(
            habla("En 1492 zarparon.", "es"),
            "En mil cuatrocientos noventa y dos zarparon."
        );
        assert_eq!(
            habla("el siglo XX fue corto", "es"),
            "el siglo veinte fue corto"
        );
        assert_eq!(
            habla("el siglo IX fue largo", "es"),
            "el siglo noveno fue largo"
        );
        assert_eq!(
            habla("Enrique VIII tuvo seis esposas", "es"),
            "Enrique octavo tuvo seis esposas"
        );
        assert_eq!(habla("Alfonso XIII reinó", "es"), "Alfonso trece reinó");
        assert_eq!(habla("EL CID CAMPEADOR", "es"), "EL CID CAMPEADOR");
        assert_eq!(habla("DERECHO CIVIL", "es"), "DERECHO CIVIL");
        assert_eq!(habla("subió un 50%", "es"), "subió un cincuenta por ciento");
        assert_eq!(
            habla("el capítulo 21º", "es"),
            "el capítulo vigésimo primero"
        );
        assert_eq!(habla("la 2ª edición", "es"), "la segunda edición");
        assert_eq!(
            habla("cuesta 3,50 €", "es"),
            "cuesta tres euros con cincuenta céntimos"
        );
        assert_eq!(habla("son las 14:30", "es"), "son las catorce y treinta");
        assert_eq!(
            habla("mide 1,75 m de alto", "es"),
            "mide uno coma setenta y cinco m de alto"
        );
        assert_eq!(habla("pesa 70kg", "es"), "pesa setenta kilogramos");
        assert_eq!(habla("el FBI investiga", "es"), "el efe be i investiga");
        assert_eq!(habla("la NASA lanzó", "es"), "la NASA lanzó");
        assert_eq!(
            habla("Dr. Ramírez, pág. 12", "es"),
            "doctor Ramírez, página doce"
        );
        assert_eq!(
            habla("1.234 personas", "es"),
            "mil doscientos treinta y cuatro personas"
        );
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
        assert_eq!(
            habla("in 1985 it began", "en"),
            "in nineteen eighty-five it began"
        );
        assert_eq!(
            habla("Henry VIII had six wives", "en"),
            "Henry the eighth had six wives"
        );
        assert_eq!(habla("World War II ended", "en"), "World War two ended");
        assert_eq!(habla("the 21st century", "en"), "the twenty-first century");
        assert_eq!(
            habla("it costs $3.50", "en"),
            "it costs three dollars fifty cents"
        );
        assert_eq!(
            habla("May 24, 2026", "en"),
            "May twenty-fourth, twenty twenty-six"
        );
        assert_eq!(habla("50% of people", "en"), "fifty percent of people");
    }

    #[test]
    fn frances_aleman() {
        assert_eq!(habla("Louis XIV régna", "fr"), "Louis quatorze régna");
        assert_eq!(habla("le XIXe siècle", "fr"), "le dix-neuvième siècle");
        assert_eq!(habla("le 1er prix", "fr"), "le premier prix");
        assert_eq!(
            habla("es kostet 3,50 €", "de"),
            "es kostet drei Euro fünfzig Cent"
        );
        assert_eq!(habla("um 14:30 Uhr", "de"), "um vierzehn Uhr dreißig");
        assert_eq!(
            habla("im 19. Jahrhundert", "de"),
            "im neunzehnten Jahrhundert"
        );
        assert_eq!(habla("am 24. Mai", "de"), "am vierundzwanzigsten Mai");
    }

    #[test]
    fn idiomas_sin_tabla_no_se_tocan_las_palabras() {
        // Neerlandés: sin tabla profunda, los números sí, el resto intacto.
        assert_eq!(
            habla("het jaar 1985", "nl"),
            "het jaar duizendnegenhonderdvijfentachtig"
        );
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

    #[test]
    fn decimales_ingleses_y_porcentaje() {
        assert_eq!(habla("2.5% of it", "en"), "two point five percent of it");
        assert_eq!(habla("it is 2.5 today", "en"), "it is two point five today");
    }

    #[test]
    fn dr_no_es_romano() {
        assert_eq!(
            habla("Dr. Lopez came 3rd.", "en"),
            "Doctor Lopez came third."
        );
    }

    #[test]
    fn siglo_pospuesto_italiano() {
        assert_eq!(
            habla("un gelato del XX secolo.", "it"),
            "un gelato del ventesimo secolo."
        );
    }

    #[test]
    fn ordinal_aleman_con_punto() {
        assert_eq!(
            habla("Heinrich VIII. genau", "de"),
            "Heinrich der Achte genau"
        );
    }

    #[test]
    fn contexto_roto_por_punto_y_coma() {
        // «century; D» no debe leer la D como romano de quinientos.
        assert_eq!(
            habla("the century; Dr. Lopez arrived", "en"),
            "the century; Doctor Lopez arrived"
        );
    }
    #[test]
    fn la_i_inglesa_es_pronombre() {
        // José Luis (21-10-2026): «I» se leía «the first» tras cualquier
        // palabra capitalizada.
        for frase in [
            "I think I can",
            "I am here",
            "Then I said no",
            "So I went home and I slept.",
            "Yesterday I'm sure I'll go, I'd say.",
            "Mom, I love you.",
            "We met in Paris. I think it rained.",
            "Mary and I went out.",
            "WHAT I LEARNED IN SCHOOL",
            "THEN I SAID NO TO THE BBC",
            "the war I fought was long",
            "the part I played",
            "a type I like",
            "the century I was born in",
            "and then John I guess left",
            "Mary I think so",
            "Generation X grew up",
            "Malcolm X spoke",
            "Washington DC is a city",
            "John Smith MD signed",
            "see Appendix C and Class D",
            "take vitamin C daily",
            "an X-ray and a V-shaped neck",
        ] {
            assert_eq!(habla(frase, "en"), frase, "«{frase}» no debe tocarse");
        }
    }

    #[test]
    fn la_i_inglesa_numeral_en_contexto() {
        assert_eq!(habla("Henry I was king", "en"), "Henry the first was king");
        assert_eq!(
            habla("Elizabeth I died in 1603", "en"),
            "Elizabeth the first died in sixteen oh-three"
        );
        assert_eq!(habla("Pope John Paul I", "en"), "Pope John Paul the first");
        assert_eq!(
            habla("Charles V ruled Spain", "en"),
            "Charles the fifth ruled Spain"
        );
        assert_eq!(habla("Pius X was pope", "en"), "Pius the tenth was pope");
        assert_eq!(
            habla("after World War I ended", "en"),
            "after World War one ended"
        );
        assert_eq!(habla("Chapter I", "en"), "Chapter one");
        assert_eq!(habla("CHAPTER I", "en"), "CHAPTER one");
        assert_eq!(
            habla("Part I: The Beginning", "en"),
            "Part one: The Beginning"
        );
        assert_eq!(habla("Book I", "en"), "Book one");
        assert_eq!(habla("Act I, Scene II", "en"), "Act one, Scene two");
        assert_eq!(habla("Volume I", "en"), "Volume one");
        assert_eq!(habla("Type I diabetes", "en"), "Type one diabetes");
        assert_eq!(habla("I. Introduction", "en"), "one. Introduction");
        assert_eq!(habla("IV. Results", "en"), "four. Results");
        assert_eq!(habla("I", "en"), "one");
        assert_eq!(habla("XII.", "en"), "twelve.");
        assert_eq!(
            habla("Louis XIV's reign", "en"),
            "Louis the fourteenth's reign"
        );
        assert_eq!(
            habla("Henry VIII had six wives", "en"),
            "Henry the eighth had six wives"
        );
    }

    #[test]
    fn romanos_de_una_letra_en_otros_idiomas() {
        assert_eq!(
            habla("Carlos V fue emperador", "es"),
            "Carlos quinto fue emperador"
        );
        assert_eq!(
            habla("Juan Pablo I murió pronto", "es"),
            "Juan Pablo primero murió pronto"
        );
        assert_eq!(
            habla("el capítulo I empieza", "es"),
            "el capítulo uno empieza"
        );
        // El artículo italiano tras punto no es romano.
        assert_eq!(
            habla("Siamo a Roma. I ragazzi giocano.", "it"),
            "Siamo a Roma. I ragazzi giocano."
        );
        assert_eq!(
            habla("el anexo C y la clase D", "es"),
            "el anexo C y la clase D"
        );
        assert_eq!(habla("Malcolm X habló", "es"), "Malcolm X habló");
        assert_eq!(habla("Pedro, I", "es"), "Pedro, I");
        assert_eq!(habla("C. S. Lewis escribió", "es"), "C. S. Lewis escribió");
        assert_eq!(habla("V. S. Naipaul wrote", "en"), "V. S. Naipaul wrote");
        assert_eq!(
            habla("el HTML y la MSNBC", "es"),
            "el hache te eme ele y la eme ese ene be ce"
        );
        // Una sigla suelta en texto normal se sigue deletreando.
        assert_eq!(habla("el FBI investiga", "es"), "el efe be i investiga");
    }

    #[test]
    fn versales_de_enfasis_van_en_minusculas() {
        // Palabra común en versales: énfasis, al modelo en minúsculas.
        assert_eq!(
            habla("as if it KNEW what a vicious influence it had", "en"),
            "as if it knew what a vicious influence it had"
        );
        assert_eq!(
            habla("and THE paper looks at me", "en"),
            "and the paper looks at me"
        );
        assert_eq!(habla("John DOES exhaust me", "en"), "John does exhaust me");
        // Abre frase: capitalizada.
        assert_eq!(
            habla("He laughed. PERHAPS he knew.", "en"),
            "He laughed. Perhaps he knew."
        );
        assert_eq!(
            habla("Te digo que NO quiero ir.", "es"),
            "Te digo que no quiero ir."
        );
        // Siglas de verdad: se deletrean las impronunciables...
        assert_eq!(
            habla("the FBI and the BBC", "en"),
            "the F B I and the B B C"
        );
        assert_eq!(habla("an HTML page", "en"), "an H T M L page");
        // ...y las pronunciables se dejan al modelo, que las dice como palabra.
        assert_eq!(habla("NASA launched it", "en"), "NASA launched it");
        // Siglas que coinciden con una palabra corriente: siguen siendo sigla.
        assert_eq!(habla("the US economy", "en"), "the US economy");
        assert_eq!(habla("negli USA si usa", "it"), "negli USA si usa");
        // Los sustantivos alemanes conservan su mayúscula.
        assert_eq!(habla("Das ist mein HAUS", "de"), "Das ist mein Haus");
        // Un titular entero en versales no se toca.
        assert_eq!(
            habla("WHAT I LEARNED IN SCHOOL", "en"),
            "WHAT I LEARNED IN SCHOOL"
        );
    }
}
