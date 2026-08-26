//! Tablas de verbalización por idioma.
//!
//! Los seis idiomas «profundos» (es, en, fr, de, it, pt) tienen tabla
//! completa: meses, monedas, unidades, abreviaturas, nombres de letra para
//! deletrear siglas, y las reglas de lectura de números romanos (siglos y
//! nombres de reyes, que se leen distinto en cada lengua). El resto de
//! idiomas se queda con los números de RBNF y nada más: mejor no tocar que
//! tocar mal.

use super::rbnf::{self, Genero, Numero};

pub struct Moneda {
    pub simbolo: &'static str,
    pub singular: &'static str,
    pub plural: &'static str,
    pub sub_singular: &'static str,
    pub sub_plural: &'static str,
    /// nexo entre parte entera y céntimos («con», «e», «and», o nada)
    pub nexo: &'static str,
}

pub struct TablaIdioma {
    pub lang: &'static str,
    pub meses: [&'static str; 12],
    pub porcentaje: &'static str,
    pub monedas: &'static [Moneda],
    /// símbolo, singular, plural. Los símbolos de una letra (m, s, g, l)
    /// solo se expanden pegados al número: «10m» sí, «1990 s» no.
    pub unidades: &'static [(&'static str, &'static str, &'static str)],
    pub abreviaturas: &'static [(&'static str, &'static str)],
    /// deletreo de siglas: nombre de cada letra
    pub letras: &'static [(char, &'static str)],
    /// palabras que marcan contexto de siglo alrededor de un romano
    pub palabras_siglo: &'static [&'static str],
    /// palabras que marcan contexto de división de obra (capítulo, acto…)
    pub palabras_capitulo: &'static [&'static str],
    pub fecha: fn(u32, u32, i32) -> Option<String>,
    pub hora: fn(u32, u32) -> Option<String>,
    /// cómo se lee un romano tras nombre propio (reyes, papas)
    pub romano_nombre: fn(i128) -> Option<String>,
    /// cómo se lee un romano en contexto de siglo
    pub romano_siglo: fn(i128) -> Option<String>,
    /// cómo se lee un romano tras palabra de capítulo/acto/tomo
    pub romano_capitulo: fn(i128) -> Option<String>,
}

fn card(lang: &str, n: i128) -> Option<String> {
    rbnf::cardinal(lang, &Numero::entero_de(n), Genero::Masculino)
}
fn ord_m(lang: &str, n: i128) -> Option<String> {
    rbnf::ordinal(lang, n, Genero::Masculino)
}

// ── Español ──────────────────────────────────────────────────────────────

fn fecha_es(d: u32, m: u32, a: i32) -> Option<String> {
    let dia = if d == 1 {
        "uno".to_string()
    } else {
        card("es", d as i128)?
    };
    Some(format!(
        "{dia} de {} de {}",
        TABLA_ES.meses.get(m as usize - 1)?,
        card("es", a as i128)?
    ))
}
fn hora_es(h: u32, min: u32) -> Option<String> {
    let hh = card("es", h as i128)?;
    if min == 0 {
        Some(hh)
    } else {
        Some(format!("{hh} y {}", card("es", min as i128)?))
    }
}
fn romano_nombre_es(n: i128) -> Option<String> {
    // RAE: hasta el diez, ordinal (Enrique octavo); después, cardinal
    // (Alfonso trece, Luis quince).
    if n <= 10 {
        ord_m("es", n)
    } else {
        card("es", n)
    }
}
fn romano_siglo_es(n: i128) -> Option<String> {
    // RAE: siglos I-X en ordinal culto; del XI en adelante, cardinal.
    if n <= 10 {
        ord_m("es", n)
    } else {
        card("es", n)
    }
}
fn romano_capitulo_es(n: i128) -> Option<String> {
    card("es", n)
}

static TABLA_ES: TablaIdioma = TablaIdioma {
    lang: "es",
    meses: [
        "enero",
        "febrero",
        "marzo",
        "abril",
        "mayo",
        "junio",
        "julio",
        "agosto",
        "septiembre",
        "octubre",
        "noviembre",
        "diciembre",
    ],
    porcentaje: "por ciento",
    monedas: &[
        Moneda {
            simbolo: "€",
            singular: "euro",
            plural: "euros",
            sub_singular: "céntimo",
            sub_plural: "céntimos",
            nexo: "con",
        },
        Moneda {
            simbolo: "$",
            singular: "dólar",
            plural: "dólares",
            sub_singular: "centavo",
            sub_plural: "centavos",
            nexo: "con",
        },
        Moneda {
            simbolo: "£",
            singular: "libra",
            plural: "libras",
            sub_singular: "penique",
            sub_plural: "peniques",
            nexo: "con",
        },
        Moneda {
            simbolo: "¥",
            singular: "yen",
            plural: "yenes",
            sub_singular: "",
            sub_plural: "",
            nexo: "",
        },
    ],
    unidades: &[
        ("°C", "grado Celsius", "grados Celsius"),
        ("°F", "grado Fahrenheit", "grados Fahrenheit"),
        ("km/h", "kilómetro por hora", "kilómetros por hora"),
        ("km²", "kilómetro cuadrado", "kilómetros cuadrados"),
        ("m²", "metro cuadrado", "metros cuadrados"),
        ("kHz", "kilohercio", "kilohercios"),
        ("MHz", "megahercio", "megahercios"),
        ("GHz", "gigahercio", "gigahercios"),
        ("Hz", "hercio", "hercios"),
        ("KB", "kilobyte", "kilobytes"),
        ("MB", "megabyte", "megabytes"),
        ("GB", "gigabyte", "gigabytes"),
        ("TB", "terabyte", "terabytes"),
        ("ms", "milisegundo", "milisegundos"),
        ("kg", "kilogramo", "kilogramos"),
        ("mg", "miligramo", "miligramos"),
        ("km", "kilómetro", "kilómetros"),
        ("cm", "centímetro", "centímetros"),
        ("mm", "milímetro", "milímetros"),
        ("ml", "mililitro", "mililitros"),
        ("m", "metro", "metros"),
        ("g", "gramo", "gramos"),
        ("l", "litro", "litros"),
        ("s", "segundo", "segundos"),
    ],
    abreviaturas: &[
        ("Dra.", "doctora"),
        ("Dr.", "doctor"),
        ("Srta.", "señorita"),
        ("Sra.", "señora"),
        ("Sr.", "señor"),
        ("Ud.", "usted"),
        ("Uds.", "ustedes"),
        ("Vd.", "usted"),
        ("etc.", "etcétera"),
        ("p. ej.", "por ejemplo"),
        ("p.ej.", "por ejemplo"),
        ("núm.", "número"),
        ("Núm.", "número"),
        ("nº", "número"),
        ("n.º", "número"),
        ("págs.", "páginas"),
        ("pág.", "página"),
        ("pp.", "páginas"),
        ("cap.", "capítulo"),
        ("Cap.", "capítulo"),
        ("art.", "artículo"),
        ("Art.", "artículo"),
        ("vol.", "volumen"),
        ("Vol.", "volumen"),
        ("ed.", "edición"),
        ("Ed.", "editorial"),
        ("a. C.", "antes de Cristo"),
        ("a.C.", "antes de Cristo"),
        ("d. C.", "después de Cristo"),
        ("d.C.", "después de Cristo"),
        ("a. de C.", "antes de Cristo"),
        ("S. A.", "sociedad anónima"),
        ("S.A.", "sociedad anónima"),
        ("Avda.", "avenida"),
        ("Av.", "avenida"),
        ("c/", "calle "),
        ("op. cit.", "obra citada"),
        ("ibíd.", "ibídem"),
        ("íd.", "ídem"),
        ("ss.", "siguientes"),
        ("trad.", "traducción"),
        ("coord.", "coordinador"),
        ("dir.", "director"),
        ("comp.", "compilador"),
        ("vs.", "contra"),
    ],
    letras: &[
        ('a', "a"),
        ('b', "be"),
        ('c', "ce"),
        ('d', "de"),
        ('e', "e"),
        ('f', "efe"),
        ('g', "ge"),
        ('h', "hache"),
        ('i', "i"),
        ('j', "jota"),
        ('k', "ka"),
        ('l', "ele"),
        ('m', "eme"),
        ('n', "ene"),
        ('ñ', "eñe"),
        ('o', "o"),
        ('p', "pe"),
        ('q', "cu"),
        ('r', "erre"),
        ('s', "ese"),
        ('t', "te"),
        ('u', "u"),
        ('v', "uve"),
        ('w', "uve doble"),
        ('x', "equis"),
        ('y', "i griega"),
        ('z', "zeta"),
    ],
    palabras_siglo: &["siglo", "siglos", "s."],
    palabras_capitulo: &[
        "capítulo",
        "cap.",
        "acto",
        "tomo",
        "libro",
        "parte",
        "volumen",
        "vol.",
        "escena",
        "canto",
        "título",
        "anexo",
        "apéndice",
        "sección",
    ],
    fecha: fecha_es,
    hora: hora_es,
    romano_nombre: romano_nombre_es,
    romano_siglo: romano_siglo_es,
    romano_capitulo: romano_capitulo_es,
};

// ── Inglés ───────────────────────────────────────────────────────────────

fn fecha_en(d: u32, m: u32, a: i32) -> Option<String> {
    Some(format!(
        "{} {}, {}",
        TABLA_EN.meses.get(m as usize - 1)?,
        ord_m("en", d as i128)?,
        rbnf::anio("en", a as i128)?
    ))
}
fn hora_en(h: u32, min: u32) -> Option<String> {
    let hh = card("en", h as i128)?;
    if min == 0 {
        Some(format!("{hh} o'clock"))
    } else if min < 10 {
        Some(format!("{hh} oh {}", card("en", min as i128)?))
    } else {
        Some(format!("{hh} {}", card("en", min as i128)?))
    }
}
fn romano_nombre_en(n: i128) -> Option<String> {
    // «Henry VIII» → «Henry the Eighth»
    Some(format!("the {}", ord_m("en", n)?))
}
fn romano_siglo_en(n: i128) -> Option<String> {
    ord_m("en", n)
}
fn romano_capitulo_en(n: i128) -> Option<String> {
    card("en", n)
}

static TABLA_EN: TablaIdioma = TablaIdioma {
    lang: "en",
    meses: [
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
    ],
    porcentaje: "percent",
    monedas: &[
        Moneda {
            simbolo: "$",
            singular: "dollar",
            plural: "dollars",
            sub_singular: "cent",
            sub_plural: "cents",
            nexo: "",
        },
        Moneda {
            simbolo: "€",
            singular: "euro",
            plural: "euros",
            sub_singular: "cent",
            sub_plural: "cents",
            nexo: "",
        },
        Moneda {
            simbolo: "£",
            singular: "pound",
            plural: "pounds",
            sub_singular: "penny",
            sub_plural: "pence",
            nexo: "",
        },
        Moneda {
            simbolo: "¥",
            singular: "yen",
            plural: "yen",
            sub_singular: "",
            sub_plural: "",
            nexo: "",
        },
    ],
    unidades: &[
        ("°C", "degree Celsius", "degrees Celsius"),
        ("°F", "degree Fahrenheit", "degrees Fahrenheit"),
        ("km/h", "kilometer per hour", "kilometers per hour"),
        ("mph", "mile per hour", "miles per hour"),
        ("km²", "square kilometer", "square kilometers"),
        ("m²", "square meter", "square meters"),
        ("kHz", "kilohertz", "kilohertz"),
        ("MHz", "megahertz", "megahertz"),
        ("GHz", "gigahertz", "gigahertz"),
        ("Hz", "hertz", "hertz"),
        ("KB", "kilobyte", "kilobytes"),
        ("MB", "megabyte", "megabytes"),
        ("GB", "gigabyte", "gigabytes"),
        ("TB", "terabyte", "terabytes"),
        ("ms", "millisecond", "milliseconds"),
        ("kg", "kilogram", "kilograms"),
        ("mg", "milligram", "milligrams"),
        ("km", "kilometer", "kilometers"),
        ("cm", "centimeter", "centimeters"),
        ("mm", "millimeter", "millimeters"),
        ("ml", "milliliter", "milliliters"),
        ("m", "meter", "meters"),
        ("g", "gram", "grams"),
        ("l", "liter", "liters"),
        ("s", "second", "seconds"),
    ],
    abreviaturas: &[
        ("Dr.", "Doctor"),
        ("Mr.", "Mister"),
        ("Mrs.", "Missus"),
        ("Ms.", "Miss"),
        ("Prof.", "Professor"),
        ("Jr.", "Junior"),
        ("St.", "Saint"),
        ("Ave.", "Avenue"),
        ("Blvd.", "Boulevard"),
        ("Dept.", "Department"),
        ("Inc.", "Incorporated"),
        ("Ltd.", "Limited"),
        ("Corp.", "Corporation"),
        ("Ph.D.", "PhD"),
        ("etc.", "etcetera"),
        ("vs.", "versus"),
        ("i.e.", "that is"),
        ("e.g.", "for example"),
        ("No.", "number"),
        ("vol.", "volume"),
        ("Vol.", "volume"),
        ("pp.", "pages"),
        ("p.", "page"),
        ("ed.", "edition"),
        ("cf.", "compare"),
        ("ca.", "circa"),
    ],
    // En inglés basta con separar las letras: el modelo las lee bien.
    letras: &[
        ('a', "A"),
        ('b', "B"),
        ('c', "C"),
        ('d', "D"),
        ('e', "E"),
        ('f', "F"),
        ('g', "G"),
        ('h', "H"),
        ('i', "I"),
        ('j', "J"),
        ('k', "K"),
        ('l', "L"),
        ('m', "M"),
        ('n', "N"),
        ('o', "O"),
        ('p', "P"),
        ('q', "Q"),
        ('r', "R"),
        ('s', "S"),
        ('t', "T"),
        ('u', "U"),
        ('v', "V"),
        ('w', "W"),
        ('x', "X"),
        ('y', "Y"),
        ('z', "Z"),
    ],
    palabras_siglo: &["century", "centuries"],
    palabras_capitulo: &[
        "chapter", "act", "book", "part", "volume", "vol.", "scene", "canto", "appendix",
        "section", "annex", "war", "phase", "type", "level", "grade", "class",
    ],
    fecha: fecha_en,
    hora: hora_en,
    romano_nombre: romano_nombre_en,
    romano_siglo: romano_siglo_en,
    romano_capitulo: romano_capitulo_en,
};

// ── Francés ──────────────────────────────────────────────────────────────

fn fecha_fr(d: u32, m: u32, a: i32) -> Option<String> {
    let dia = if d == 1 {
        "premier".to_string()
    } else {
        card("fr", d as i128)?
    };
    Some(format!(
        "{dia} {} {}",
        TABLA_FR.meses.get(m as usize - 1)?,
        card("fr", a as i128)?
    ))
}
fn hora_fr(h: u32, min: u32) -> Option<String> {
    let hh = card("fr", h as i128)?;
    let heures = if h == 1 { "heure" } else { "heures" };
    if min == 0 {
        Some(format!("{hh} {heures}"))
    } else {
        Some(format!("{hh} {heures} {}", card("fr", min as i128)?))
    }
}
fn romano_nombre_fr(n: i128) -> Option<String> {
    // «François Ier» → «François premier»; el resto, cardinal
    // («Louis XIV» → «Louis quatorze»).
    if n == 1 {
        Some("premier".to_string())
    } else {
        card("fr", n)
    }
}
fn romano_siglo_fr(n: i128) -> Option<String> {
    rbnf::ordinal("fr", n, Genero::Masculino)
}
fn romano_capitulo_fr(n: i128) -> Option<String> {
    if n == 1 {
        Some("premier".to_string())
    } else {
        card("fr", n)
    }
}

static TABLA_FR: TablaIdioma = TablaIdioma {
    lang: "fr",
    meses: [
        "janvier",
        "février",
        "mars",
        "avril",
        "mai",
        "juin",
        "juillet",
        "août",
        "septembre",
        "octobre",
        "novembre",
        "décembre",
    ],
    porcentaje: "pour cent",
    monedas: &[
        Moneda {
            simbolo: "€",
            singular: "euro",
            plural: "euros",
            sub_singular: "",
            sub_plural: "",
            nexo: "",
        },
        Moneda {
            simbolo: "$",
            singular: "dollar",
            plural: "dollars",
            sub_singular: "",
            sub_plural: "",
            nexo: "",
        },
        Moneda {
            simbolo: "£",
            singular: "livre",
            plural: "livres",
            sub_singular: "",
            sub_plural: "",
            nexo: "",
        },
        Moneda {
            simbolo: "¥",
            singular: "yen",
            plural: "yens",
            sub_singular: "",
            sub_plural: "",
            nexo: "",
        },
    ],
    unidades: &[
        ("°C", "degré Celsius", "degrés Celsius"),
        ("°F", "degré Fahrenheit", "degrés Fahrenheit"),
        ("km/h", "kilomètre par heure", "kilomètres par heure"),
        ("km²", "kilomètre carré", "kilomètres carrés"),
        ("m²", "mètre carré", "mètres carrés"),
        ("kHz", "kilohertz", "kilohertz"),
        ("MHz", "mégahertz", "mégahertz"),
        ("Hz", "hertz", "hertz"),
        ("KB", "kilooctet", "kilooctets"),
        ("MB", "mégaoctet", "mégaoctets"),
        ("GB", "gigaoctet", "gigaoctets"),
        ("Mo", "mégaoctet", "mégaoctets"),
        ("Go", "gigaoctet", "gigaoctets"),
        ("ms", "milliseconde", "millisecondes"),
        ("kg", "kilogramme", "kilogrammes"),
        ("mg", "milligramme", "milligrammes"),
        ("km", "kilomètre", "kilomètres"),
        ("cm", "centimètre", "centimètres"),
        ("mm", "millimètre", "millimètres"),
        ("ml", "millilitre", "millilitres"),
        ("m", "mètre", "mètres"),
        ("g", "gramme", "grammes"),
        ("l", "litre", "litres"),
        ("s", "seconde", "secondes"),
    ],
    abreviaturas: &[
        ("Mme", "madame"),
        ("Mlle", "mademoiselle"),
        ("Dr", "docteur"),
        ("etc.", "et cetera"),
        ("p. ex.", "par exemple"),
        ("c.-à-d.", "c'est-à-dire"),
        ("n°", "numéro"),
        ("N°", "numéro"),
        ("vol.", "volume"),
        ("chap.", "chapitre"),
        ("éd.", "édition"),
        ("av. J.-C.", "avant Jésus-Christ"),
        ("ap. J.-C.", "après Jésus-Christ"),
    ],
    letras: &[
        ('a', "a"),
        ('b', "bé"),
        ('c', "cé"),
        ('d', "dé"),
        ('e', "e"),
        ('f', "effe"),
        ('g', "gé"),
        ('h', "ache"),
        ('i', "i"),
        ('j', "ji"),
        ('k', "ka"),
        ('l', "elle"),
        ('m', "emme"),
        ('n', "enne"),
        ('o', "o"),
        ('p', "pé"),
        ('q', "cu"),
        ('r', "erre"),
        ('s', "esse"),
        ('t', "té"),
        ('u', "u"),
        ('v', "vé"),
        ('w', "double vé"),
        ('x', "ixe"),
        ('y', "i grec"),
        ('z', "zède"),
    ],
    palabras_siglo: &["siècle", "siècles"],
    palabras_capitulo: &[
        "chapitre", "acte", "tome", "livre", "partie", "volume", "vol.", "scène", "chant",
        "annexe", "section",
    ],
    fecha: fecha_fr,
    hora: hora_fr,
    romano_nombre: romano_nombre_fr,
    romano_siglo: romano_siglo_fr,
    romano_capitulo: romano_capitulo_fr,
};

// ── Alemán ───────────────────────────────────────────────────────────────

fn fecha_de(d: u32, m: u32, a: i32) -> Option<String> {
    // «24.05.2026» → «vierundzwanzigster Mai zweitausendsechsundzwanzig»
    let dia = ord_m("de", d as i128)?;
    let dia = format!("{dia}r"); // erste → erster (nominativo masculino)
    Some(format!(
        "{dia} {} {}",
        TABLA_DE.meses.get(m as usize - 1)?,
        rbnf::anio("de", a as i128)?
    ))
}
fn hora_de(h: u32, min: u32) -> Option<String> {
    let hh = card("de", h as i128)?;
    if min == 0 {
        Some(format!("{hh} Uhr"))
    } else {
        Some(format!("{hh} Uhr {}", card("de", min as i128)?))
    }
}
fn romano_nombre_de(n: i128) -> Option<String> {
    // «Heinrich VIII.» → «Heinrich der Achte»
    let o = ord_m("de", n)?;
    let mut cs = o.chars();
    let mayuscula = cs
        .next()
        .map(|c| c.to_uppercase().collect::<String>() + cs.as_str());
    Some(format!("der {}", mayuscula?))
}
fn romano_siglo_de(n: i128) -> Option<String> {
    ord_m("de", n)
}
fn romano_capitulo_de(n: i128) -> Option<String> {
    card("de", n)
}

static TABLA_DE: TablaIdioma = TablaIdioma {
    lang: "de",
    meses: [
        "Januar",
        "Februar",
        "März",
        "April",
        "Mai",
        "Juni",
        "Juli",
        "August",
        "September",
        "Oktober",
        "November",
        "Dezember",
    ],
    porcentaje: "Prozent",
    monedas: &[
        Moneda {
            simbolo: "€",
            singular: "Euro",
            plural: "Euro",
            sub_singular: "Cent",
            sub_plural: "Cent",
            nexo: "",
        },
        Moneda {
            simbolo: "$",
            singular: "Dollar",
            plural: "Dollar",
            sub_singular: "Cent",
            sub_plural: "Cent",
            nexo: "",
        },
        Moneda {
            simbolo: "£",
            singular: "Pfund",
            plural: "Pfund",
            sub_singular: "Penny",
            sub_plural: "Pence",
            nexo: "",
        },
        Moneda {
            simbolo: "¥",
            singular: "Yen",
            plural: "Yen",
            sub_singular: "",
            sub_plural: "",
            nexo: "",
        },
    ],
    unidades: &[
        ("°C", "Grad Celsius", "Grad Celsius"),
        ("°F", "Grad Fahrenheit", "Grad Fahrenheit"),
        ("km/h", "Kilometer pro Stunde", "Kilometer pro Stunde"),
        ("km²", "Quadratkilometer", "Quadratkilometer"),
        ("m²", "Quadratmeter", "Quadratmeter"),
        ("kHz", "Kilohertz", "Kilohertz"),
        ("MHz", "Megahertz", "Megahertz"),
        ("Hz", "Hertz", "Hertz"),
        ("KB", "Kilobyte", "Kilobyte"),
        ("MB", "Megabyte", "Megabyte"),
        ("GB", "Gigabyte", "Gigabyte"),
        ("TB", "Terabyte", "Terabyte"),
        ("ms", "Millisekunde", "Millisekunden"),
        ("kg", "Kilogramm", "Kilogramm"),
        ("mg", "Milligramm", "Milligramm"),
        ("km", "Kilometer", "Kilometer"),
        ("cm", "Zentimeter", "Zentimeter"),
        ("mm", "Millimeter", "Millimeter"),
        ("ml", "Milliliter", "Milliliter"),
        ("m", "Meter", "Meter"),
        ("g", "Gramm", "Gramm"),
        ("l", "Liter", "Liter"),
        ("s", "Sekunde", "Sekunden"),
    ],
    abreviaturas: &[
        ("z. B.", "zum Beispiel"),
        ("z.B.", "zum Beispiel"),
        ("d. h.", "das heißt"),
        ("d.h.", "das heißt"),
        ("usw.", "und so weiter"),
        ("bzw.", "beziehungsweise"),
        ("ca.", "circa"),
        ("Nr.", "Nummer"),
        ("Bd.", "Band"),
        ("Kap.", "Kapitel"),
        ("Jh.", "Jahrhundert"),
        ("v. Chr.", "vor Christus"),
        ("n. Chr.", "nach Christus"),
    ],
    letras: &[
        ('a', "a"),
        ('b', "be"),
        ('c', "ce"),
        ('d', "de"),
        ('e', "e"),
        ('f', "ef"),
        ('g', "ge"),
        ('h', "ha"),
        ('i', "i"),
        ('j', "jot"),
        ('k', "ka"),
        ('l', "el"),
        ('m', "em"),
        ('n', "en"),
        ('o', "o"),
        ('p', "pe"),
        ('q', "ku"),
        ('r', "er"),
        ('s', "es"),
        ('t', "te"),
        ('u', "u"),
        ('v', "vau"),
        ('w', "we"),
        ('x', "ix"),
        ('y', "ypsilon"),
        ('z', "zett"),
    ],
    palabras_siglo: &["Jahrhundert", "Jahrhunderts", "Jh."],
    palabras_capitulo: &[
        "Kapitel",
        "Akt",
        "Band",
        "Buch",
        "Teil",
        "Szene",
        "Anhang",
        "Abschnitt",
    ],
    fecha: fecha_de,
    hora: hora_de,
    romano_nombre: romano_nombre_de,
    romano_siglo: romano_siglo_de,
    romano_capitulo: romano_capitulo_de,
};

// ── Italiano ─────────────────────────────────────────────────────────────

fn fecha_it(d: u32, m: u32, a: i32) -> Option<String> {
    let dia = if d == 1 {
        "primo".to_string()
    } else {
        card("it", d as i128)?
    };
    Some(format!(
        "{dia} {} {}",
        TABLA_IT.meses.get(m as usize - 1)?,
        card("it", a as i128)?
    ))
}
fn hora_it(h: u32, min: u32) -> Option<String> {
    let hh = card("it", h as i128)?;
    if min == 0 {
        Some(hh)
    } else {
        Some(format!("{hh} e {}", card("it", min as i128)?))
    }
}
fn romano_nombre_it(n: i128) -> Option<String> {
    // «Luigi XIV» → «Luigi quattordicesimo»
    ord_m("it", n)
}
fn romano_siglo_it(n: i128) -> Option<String> {
    ord_m("it", n)
}
fn romano_capitulo_it(n: i128) -> Option<String> {
    card("it", n)
}

static TABLA_IT: TablaIdioma = TablaIdioma {
    lang: "it",
    meses: [
        "gennaio",
        "febbraio",
        "marzo",
        "aprile",
        "maggio",
        "giugno",
        "luglio",
        "agosto",
        "settembre",
        "ottobre",
        "novembre",
        "dicembre",
    ],
    porcentaje: "per cento",
    monedas: &[
        Moneda {
            simbolo: "€",
            singular: "euro",
            plural: "euro",
            sub_singular: "centesimo",
            sub_plural: "centesimi",
            nexo: "e",
        },
        Moneda {
            simbolo: "$",
            singular: "dollaro",
            plural: "dollari",
            sub_singular: "centesimo",
            sub_plural: "centesimi",
            nexo: "e",
        },
        Moneda {
            simbolo: "£",
            singular: "sterlina",
            plural: "sterline",
            sub_singular: "penny",
            sub_plural: "pence",
            nexo: "e",
        },
        Moneda {
            simbolo: "¥",
            singular: "yen",
            plural: "yen",
            sub_singular: "",
            sub_plural: "",
            nexo: "",
        },
    ],
    unidades: &[
        ("°C", "grado Celsius", "gradi Celsius"),
        ("°F", "grado Fahrenheit", "gradi Fahrenheit"),
        ("km/h", "chilometro orario", "chilometri orari"),
        ("km²", "chilometro quadrato", "chilometri quadrati"),
        ("m²", "metro quadrato", "metri quadrati"),
        ("kHz", "kilohertz", "kilohertz"),
        ("MHz", "megahertz", "megahertz"),
        ("Hz", "hertz", "hertz"),
        ("KB", "kilobyte", "kilobyte"),
        ("MB", "megabyte", "megabyte"),
        ("GB", "gigabyte", "gigabyte"),
        ("ms", "millisecondo", "millisecondi"),
        ("kg", "chilogrammo", "chilogrammi"),
        ("mg", "milligrammo", "milligrammi"),
        ("km", "chilometro", "chilometri"),
        ("cm", "centimetro", "centimetri"),
        ("mm", "millimetro", "millimetri"),
        ("ml", "millilitro", "millilitri"),
        ("m", "metro", "metri"),
        ("g", "grammo", "grammi"),
        ("l", "litro", "litri"),
        ("s", "secondo", "secondi"),
    ],
    abreviaturas: &[
        ("Sig.ra", "signora"),
        ("Sig.", "signor"),
        ("Dott.", "dottor"),
        ("ecc.", "eccetera"),
        ("p. es.", "per esempio"),
        ("n.", "numero"),
        ("vol.", "volume"),
        ("cap.", "capitolo"),
        ("ed.", "edizione"),
        ("a.C.", "avanti Cristo"),
        ("d.C.", "dopo Cristo"),
    ],
    letras: &[
        ('a', "a"),
        ('b', "bi"),
        ('c', "ci"),
        ('d', "di"),
        ('e', "e"),
        ('f', "effe"),
        ('g', "gi"),
        ('h', "acca"),
        ('i', "i"),
        ('j', "i lunga"),
        ('k', "cappa"),
        ('l', "elle"),
        ('m', "emme"),
        ('n', "enne"),
        ('o', "o"),
        ('p', "pi"),
        ('q', "cu"),
        ('r', "erre"),
        ('s', "esse"),
        ('t', "ti"),
        ('u', "u"),
        ('v', "vu"),
        ('w', "doppia vu"),
        ('x', "ics"),
        ('y', "ipsilon"),
        ('z', "zeta"),
    ],
    palabras_siglo: &["secolo", "secoli", "sec."],
    palabras_capitulo: &[
        "capitolo",
        "atto",
        "tomo",
        "libro",
        "parte",
        "volume",
        "vol.",
        "scena",
        "canto",
        "appendice",
        "sezione",
    ],
    fecha: fecha_it,
    hora: hora_it,
    romano_nombre: romano_nombre_it,
    romano_siglo: romano_siglo_it,
    romano_capitulo: romano_capitulo_it,
};

// ── Portugués ────────────────────────────────────────────────────────────

fn fecha_pt(d: u32, m: u32, a: i32) -> Option<String> {
    let dia = if d == 1 {
        "primeiro".to_string()
    } else {
        card("pt", d as i128)?
    };
    Some(format!(
        "{dia} de {} de {}",
        TABLA_PT.meses.get(m as usize - 1)?,
        card("pt", a as i128)?
    ))
}
fn hora_pt(h: u32, min: u32) -> Option<String> {
    let hh = card("pt", h as i128)?;
    if min == 0 {
        Some(hh)
    } else {
        Some(format!("{hh} e {}", card("pt", min as i128)?))
    }
}
fn romano_nombre_pt(n: i128) -> Option<String> {
    if n <= 10 {
        ord_m("pt", n)
    } else {
        card("pt", n)
    }
}
fn romano_siglo_pt(n: i128) -> Option<String> {
    if n <= 10 {
        ord_m("pt", n)
    } else {
        card("pt", n)
    }
}
fn romano_capitulo_pt(n: i128) -> Option<String> {
    card("pt", n)
}

static TABLA_PT: TablaIdioma = TablaIdioma {
    lang: "pt",
    meses: [
        "janeiro",
        "fevereiro",
        "março",
        "abril",
        "maio",
        "junho",
        "julho",
        "agosto",
        "setembro",
        "outubro",
        "novembro",
        "dezembro",
    ],
    porcentaje: "por cento",
    monedas: &[
        Moneda {
            simbolo: "€",
            singular: "euro",
            plural: "euros",
            sub_singular: "cêntimo",
            sub_plural: "cêntimos",
            nexo: "e",
        },
        Moneda {
            simbolo: "$",
            singular: "dólar",
            plural: "dólares",
            sub_singular: "centavo",
            sub_plural: "centavos",
            nexo: "e",
        },
        Moneda {
            simbolo: "£",
            singular: "libra",
            plural: "libras",
            sub_singular: "péni",
            sub_plural: "pence",
            nexo: "e",
        },
        Moneda {
            simbolo: "¥",
            singular: "iene",
            plural: "ienes",
            sub_singular: "",
            sub_plural: "",
            nexo: "",
        },
    ],
    unidades: &[
        ("°C", "grau Celsius", "graus Celsius"),
        ("°F", "grau Fahrenheit", "graus Fahrenheit"),
        ("km/h", "quilómetro por hora", "quilómetros por hora"),
        ("km²", "quilómetro quadrado", "quilómetros quadrados"),
        ("m²", "metro quadrado", "metros quadrados"),
        ("kHz", "quilohertz", "quilohertz"),
        ("MHz", "megahertz", "megahertz"),
        ("Hz", "hertz", "hertz"),
        ("KB", "quilobyte", "quilobytes"),
        ("MB", "megabyte", "megabytes"),
        ("GB", "gigabyte", "gigabytes"),
        ("ms", "milissegundo", "milissegundos"),
        ("kg", "quilograma", "quilogramas"),
        ("mg", "miligrama", "miligramas"),
        ("km", "quilómetro", "quilómetros"),
        ("cm", "centímetro", "centímetros"),
        ("mm", "milímetro", "milímetros"),
        ("ml", "mililitro", "mililitros"),
        ("m", "metro", "metros"),
        ("g", "grama", "gramas"),
        ("l", "litro", "litros"),
        ("s", "segundo", "segundos"),
    ],
    abreviaturas: &[
        ("Sr.", "senhor"),
        ("Sra.", "senhora"),
        ("Dr.", "doutor"),
        ("Dra.", "doutora"),
        ("etc.", "et cetera"),
        ("p. ex.", "por exemplo"),
        ("n.º", "número"),
        ("pág.", "página"),
        ("cap.", "capítulo"),
        ("vol.", "volume"),
        ("ed.", "edição"),
        ("a.C.", "antes de Cristo"),
        ("d.C.", "depois de Cristo"),
    ],
    letras: &[
        ('a', "á"),
        ('b', "bê"),
        ('c', "cê"),
        ('d', "dê"),
        ('e', "é"),
        ('f', "efe"),
        ('g', "gê"),
        ('h', "agá"),
        ('i', "i"),
        ('j', "jota"),
        ('k', "capa"),
        ('l', "ele"),
        ('m', "eme"),
        ('n', "ene"),
        ('o', "ó"),
        ('p', "pê"),
        ('q', "quê"),
        ('r', "erre"),
        ('s', "esse"),
        ('t', "tê"),
        ('u', "u"),
        ('v', "vê"),
        ('w', "dâblio"),
        ('x', "xis"),
        ('y', "ípsilon"),
        ('z', "zê"),
    ],
    palabras_siglo: &["século", "séculos", "séc."],
    palabras_capitulo: &[
        "capítulo",
        "ato",
        "tomo",
        "livro",
        "parte",
        "volume",
        "vol.",
        "cena",
        "canto",
        "apêndice",
        "secção",
        "seção",
    ],
    fecha: fecha_pt,
    hora: hora_pt,
    romano_nombre: romano_nombre_pt,
    romano_siglo: romano_siglo_pt,
    romano_capitulo: romano_capitulo_pt,
};

/// La tabla del idioma, si es de los profundos.
pub fn tabla(lang: &str) -> Option<&'static TablaIdioma> {
    match lang {
        "es" => Some(&TABLA_ES),
        "en" => Some(&TABLA_EN),
        "fr" => Some(&TABLA_FR),
        "de" => Some(&TABLA_DE),
        "it" => Some(&TABLA_IT),
        "pt" => Some(&TABLA_PT),
        _ => None,
    }
}
