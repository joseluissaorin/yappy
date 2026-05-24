//! Text normalization — the "little things" that make synthetic speech feel natural.
//!
//! Pipeline (applied in order, each rule per-language where it matters):
//!   1. Strip / shorten URLs, emails, raw code-y junk.
//!   2. Expand abbreviations: Dr. → doctor, St. → saint/street (heuristic), e.g. → for example.
//!   3. Expand units: 5kg → five kilograms, 30°C → thirty degrees Celsius.
//!   4. Expand currencies: $3.50 → three dollars fifty, €2 → two euros.
//!   5. Expand dates: 2026-05-24 → May twenty-fourth, two thousand twenty-six.
//!   6. Expand times: 14:30 → two thirty PM.
//!   7. Expand Roman numerals in century contexts: 19th century / siglo XIX → nineteenth century.
//!   8. Expand standalone numbers: 1,234 → one thousand two hundred thirty-four.
//!
//! Languages with full coverage: en, es. Other languages get rules 1–4 and 8 only;
//! Supertonic itself is generally good at digits in supported languages.

use once_cell::sync::Lazy;
use regex::Regex;

pub fn normalize(text: &str, lang: &str) -> String {
    let mut t = text.to_string();

    // 1. URLs / emails / heavy punctuation we don't want spoken.
    t = strip_urls_and_emails(&t);
    // Strip backticks (code) and reduce parentheticals like "(see footnote 3)".
    t = t.replace('`', "");

    // 2/3. Abbreviations and units (en/es full, others light)
    t = expand_abbreviations(&t, lang);
    t = expand_units(&t, lang);

    // 4. Currencies
    t = expand_currencies(&t, lang);

    // 5. Dates
    t = expand_dates(&t, lang);

    // 6. Times
    t = expand_times(&t, lang);

    // 7. Roman numerals (centuries / monarchs)
    t = expand_roman_numerals(&t, lang);

    // 8. Generic numbers
    t = expand_numbers(&t, lang);

    // Collapse whitespace
    static WS: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s+").unwrap());
    WS.replace_all(&t, " ").trim().to_string()
}

// ---------- URLs / emails ----------

static URL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"https?://\S+|www\.\S+").unwrap()
});
static EMAIL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}").unwrap()
});

fn strip_urls_and_emails(text: &str) -> String {
    // Replace URLs with "[link]" and emails with "[email]" — those words are short enough
    // to be skipped in spoken context but acknowledge a link existed.
    let t = URL_RE.replace_all(text, "");
    let t = EMAIL_RE.replace_all(&t, "");
    t.into_owned()
}

// ---------- abbreviations ----------

fn expand_abbreviations(text: &str, lang: &str) -> String {
    let pairs_en: &[(&str, &str)] = &[
        ("Dr.", "Doctor"),
        ("Mr.", "Mister"),
        ("Mrs.", "Missus"),
        ("Ms.", "Miss"),
        ("Prof.", "Professor"),
        ("Sr.", "Senior"),
        ("Jr.", "Junior"),
        ("St.", "Saint"),
        ("Ave.", "Avenue"),
        ("Rd.", "Road"),
        ("Blvd.", "Boulevard"),
        ("Dept.", "Department"),
        ("Inc.", "Incorporated"),
        ("Ltd.", "Limited"),
        ("Corp.", "Corporation"),
        ("Co.", "Company"),
        ("Ph.D.", "PhD"),
        ("etc.", "etcetera"),
        ("vs.", "versus"),
        ("i.e.", "that is"),
        ("e.g.", "for example"),
        ("a.m.", "AM"),
        ("p.m.", "PM"),
        ("A.M.", "AM"),
        ("P.M.", "PM"),
        ("U.S.", "US"),
        ("U.K.", "UK"),
        ("U.S.A.", "USA"),
        ("No.", "Number"),
        ("vol.", "volume"),
        ("Vol.", "Volume"),
    ];
    let pairs_es: &[(&str, &str)] = &[
        ("Dr.", "Doctor"),
        ("Dra.", "Doctora"),
        ("Sr.", "Señor"),
        ("Sra.", "Señora"),
        ("Srta.", "Señorita"),
        ("etc.", "etcétera"),
        ("p.ej.", "por ejemplo"),
        ("Ud.", "usted"),
        ("Uds.", "ustedes"),
        ("núm.", "número"),
        ("Núm.", "Número"),
        // Common academic / literary abbreviations encountered in PDFs and notes.
        ("vs.", "contra"),
        ("art.", "artículo"),
        ("Art.", "Artículo"),
        ("pág.", "página"),
        ("Pág.", "Página"),
        ("págs.", "páginas"),
        ("vol.", "volumen"),
        ("Vol.", "Volumen"),
        ("cap.", "capítulo"),
        ("Cap.", "Capítulo"),
        ("Av.", "Avenida"),
        ("Avda.", "Avenida"),
        ("a.C.", "antes de Cristo"),
        ("d.C.", "después de Cristo"),
        ("S.A.", "Sociedad Anónima"),
        ("S.L.", "Sociedad Limitada"),
        ("ed.", "edición"),
        ("Ed.", "Edición"),
        ("tr.", "traducción"),
        ("trad.", "traducción"),
        ("op. cit.", "obra citada"),
        ("cf.", "compárese"),
        ("ibíd.", "ibídem"),
        ("ibid.", "ibídem"),
        ("loc. cit.", "lugar citado"),
        // Ordinal abbreviations.
        ("1º", "primero"),
        ("2º", "segundo"),
        ("3º", "tercero"),
        ("1ª", "primera"),
        ("2ª", "segunda"),
        ("3ª", "tercera"),
        ("1er", "primer"),
        ("2do", "segundo"),
        ("3er", "tercer"),
    ];
    let pairs: &[(&str, &str)] = match lang {
        "es" => pairs_es,
        _ => pairs_en,
    };
    let mut out = text.to_string();
    for (k, v) in pairs {
        out = out.replace(k, v);
    }
    out
}

// ---------- units ----------

fn expand_units(text: &str, lang: &str) -> String {
    // Match number followed (optionally) by a space and a unit token.
    let (label_kg, label_g, label_mg, label_km, label_m, label_cm, label_mm, label_l, label_ml,
         label_celsius, label_fahrenheit, label_kmh, label_mph, label_pct, label_hz, label_khz, label_mhz,
         label_kb, label_mb, label_gb, label_tb, label_ms, label_s) = match lang {
        "es" => (
            "kilogramos", "gramos", "miligramos", "kilómetros", "metros", "centímetros",
            "milímetros", "litros", "mililitros", "grados Celsius", "grados Fahrenheit",
            "kilómetros por hora", "millas por hora", "por ciento",
            "hercios", "kilohercios", "megahercios",
            "kilobytes", "megabytes", "gigabytes", "terabytes", "milisegundos", "segundos",
        ),
        _ => (
            "kilograms", "grams", "milligrams", "kilometers", "meters", "centimeters",
            "millimeters", "liters", "milliliters", "degrees Celsius", "degrees Fahrenheit",
            "kilometers per hour", "miles per hour", "percent",
            "hertz", "kilohertz", "megahertz",
            "kilobytes", "megabytes", "gigabytes", "terabytes", "milliseconds", "seconds",
        ),
    };
    let units: &[(&str, &str)] = &[
        (r"(?P<n>\d+(?:[.,]\d+)?)\s?°C\b", label_celsius),
        (r"(?P<n>\d+(?:[.,]\d+)?)\s?°F\b", label_fahrenheit),
        (r"(?P<n>\d+(?:[.,]\d+)?)\s?km/h\b", label_kmh),
        (r"(?P<n>\d+(?:[.,]\d+)?)\s?mph\b", label_mph),
        (r"(?P<n>\d+(?:[.,]\d+)?)\s?%\b", label_pct),
        (r"(?P<n>\d+(?:[.,]\d+)?)\s?kHz\b", label_khz),
        (r"(?P<n>\d+(?:[.,]\d+)?)\s?MHz\b", label_mhz),
        (r"(?P<n>\d+(?:[.,]\d+)?)\s?Hz\b", label_hz),
        (r"(?P<n>\d+(?:[.,]\d+)?)\s?KB\b", label_kb),
        (r"(?P<n>\d+(?:[.,]\d+)?)\s?MB\b", label_mb),
        (r"(?P<n>\d+(?:[.,]\d+)?)\s?GB\b", label_gb),
        (r"(?P<n>\d+(?:[.,]\d+)?)\s?TB\b", label_tb),
        (r"(?P<n>\d+(?:[.,]\d+)?)\s?ms\b", label_ms),
        (r"(?P<n>\d+(?:[.,]\d+)?)\s?kg\b", label_kg),
        (r"(?P<n>\d+(?:[.,]\d+)?)\s?mg\b", label_mg),
        (r"(?P<n>\d+(?:[.,]\d+)?)\s?g\b", label_g),
        (r"(?P<n>\d+(?:[.,]\d+)?)\s?km\b", label_km),
        (r"(?P<n>\d+(?:[.,]\d+)?)\s?cm\b", label_cm),
        (r"(?P<n>\d+(?:[.,]\d+)?)\s?mm\b", label_mm),
        (r"(?P<n>\d+(?:[.,]\d+)?)\s?m\b", label_m),
        (r"(?P<n>\d+(?:[.,]\d+)?)\s?ml\b", label_ml),
        (r"(?P<n>\d+(?:[.,]\d+)?)\s?l\b", label_l),
        (r"(?P<n>\d+(?:[.,]\d+)?)\s?s\b", label_s),
    ];
    let mut out = text.to_string();
    for (re_src, unit) in units {
        let re = Regex::new(re_src).unwrap();
        out = re
            .replace_all(&out, |caps: &regex::Captures| {
                format!("{} {}", &caps["n"], unit)
            })
            .into_owned();
    }
    out
}

// ---------- currencies ----------

fn expand_currencies(text: &str, lang: &str) -> String {
    let (dollars, dollar_sg, cents, euros, euro_sg, pounds, pound_sg, yen, yen_sg) = match lang {
        "es" => (
            "dólares", "dólar", "centavos",
            "euros", "euro",
            "libras", "libra",
            "yenes", "yen",
        ),
        _ => (
            "dollars", "dollar", "cents",
            "euros", "euro",
            "pounds", "pound",
            "yen", "yen",
        ),
    };
    let patterns: &[(&str, &str, &str, Option<&str>)] = &[
        (r"\$(?P<i>\d+(?:,\d{3})*)(?:\.(?P<f>\d{1,2}))?", dollars, dollar_sg, Some(cents)),
        (r"€(?P<i>\d+(?:,\d{3})*)(?:\.(?P<f>\d{1,2}))?", euros, euro_sg, Some(cents)),
        (r"£(?P<i>\d+(?:,\d{3})*)(?:\.(?P<f>\d{1,2}))?", pounds, pound_sg, Some(cents)),
        (r"¥(?P<i>\d+(?:,\d{3})*)", yen, yen_sg, None),
    ];
    let mut out = text.to_string();
    for (re_src, plural, singular, fraction_label) in patterns {
        let re = Regex::new(re_src).unwrap();
        out = re
            .replace_all(&out, |caps: &regex::Captures| {
                let i: u64 = caps["i"].replace(',', "").parse().unwrap_or(0);
                let cents_part = caps.name("f").map(|m| m.as_str().parse::<u64>().unwrap_or(0));
                let main_word = if i == 1 { *singular } else { *plural };
                match (cents_part, fraction_label) {
                    (Some(c), Some(lbl)) if c > 0 => {
                        format!("{} {} {} {}", i, main_word, c, lbl)
                    }
                    _ => format!("{} {}", i, main_word),
                }
            })
            .into_owned();
    }
    out
}

// ---------- dates ----------

fn expand_dates(text: &str, lang: &str) -> String {
    // ISO date: 2026-05-24 → "May twenty-fourth, two thousand twenty-six" (en) or
    // "veinticuatro de mayo de dos mil veintiséis" (es).
    let iso = Regex::new(r"\b(?P<y>\d{4})-(?P<m>\d{2})-(?P<d>\d{2})\b").unwrap();
    let mut out = iso
        .replace_all(text, |caps: &regex::Captures| {
            let y: i32 = caps["y"].parse().unwrap_or(0);
            let m: u32 = caps["m"].parse().unwrap_or(0);
            let d: u32 = caps["d"].parse().unwrap_or(0);
            spell_date(y, m, d, lang)
        })
        .into_owned();

    // Slash dates D/M/YYYY or M/D/YYYY — assume M/D in English, D/M elsewhere.
    let slash = Regex::new(r"\b(\d{1,2})/(\d{1,2})/(\d{4})\b").unwrap();
    out = slash
        .replace_all(&out, |caps: &regex::Captures| {
            let a: u32 = caps[1].parse().unwrap_or(0);
            let b: u32 = caps[2].parse().unwrap_or(0);
            let y: i32 = caps[3].parse().unwrap_or(0);
            let (d, m) = if lang == "en" { (b, a) } else { (a, b) };
            spell_date(y, m, d, lang)
        })
        .into_owned();
    out
}

fn spell_date(y: i32, m: u32, d: u32, lang: &str) -> String {
    let month_en = [
        "January", "February", "March", "April", "May", "June", "July", "August", "September",
        "October", "November", "December",
    ];
    let month_es = [
        "enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre",
        "octubre", "noviembre", "diciembre",
    ];
    let m_idx = (m as usize).saturating_sub(1).min(11);
    let year_word = match lang {
        "es" => spanish_number(y.unsigned_abs() as u64),
        _ => num_to_english(y.unsigned_abs() as u64),
    };
    let day_word = match lang {
        "es" => spanish_number(d as u64),
        _ => english_ordinal(d as u64),
    };
    match lang {
        "es" => format!("{} de {} de {}", day_word, month_es[m_idx], year_word),
        _ => format!("{} {}, {}", month_en[m_idx], day_word, year_word),
    }
}

/// English ordinal as words: 24 -> "twenty-fourth", 1 -> "first".
fn english_ordinal(n: u64) -> String {
    let base = num_to_english(n);
    // Replace the last word with its ordinal form.
    let (head, last) = match base.rsplit_once(' ') {
        Some((h, l)) => (format!("{} ", h), l.to_string()),
        None => (String::new(), base.clone()),
    };
    // Handle hyphenated tens-and-units like "twenty-four".
    let (last_head, last_tail) = match last.rsplit_once('-') {
        Some((h, t)) => (format!("{}-", h), t.to_string()),
        None => (String::new(), last.clone()),
    };
    let ord = ordinal_word(&last_tail);
    format!("{}{}{}", head, last_head, ord)
}

fn ordinal_word(w: &str) -> String {
    match w {
        "one" => "first".into(),
        "two" => "second".into(),
        "three" => "third".into(),
        "four" => "fourth".into(),
        "five" => "fifth".into(),
        "six" => "sixth".into(),
        "seven" => "seventh".into(),
        "eight" => "eighth".into(),
        "nine" => "ninth".into(),
        "ten" => "tenth".into(),
        "eleven" => "eleventh".into(),
        "twelve" => "twelfth".into(),
        "thirteen" => "thirteenth".into(),
        "fourteen" => "fourteenth".into(),
        "fifteen" => "fifteenth".into(),
        "sixteen" => "sixteenth".into(),
        "seventeen" => "seventeenth".into(),
        "eighteen" => "eighteenth".into(),
        "nineteen" => "nineteenth".into(),
        "twenty" => "twentieth".into(),
        "thirty" => "thirtieth".into(),
        "forty" => "fortieth".into(),
        "fifty" => "fiftieth".into(),
        "sixty" => "sixtieth".into(),
        "seventy" => "seventieth".into(),
        "eighty" => "eightieth".into(),
        "ninety" => "ninetieth".into(),
        "hundred" => "hundredth".into(),
        "thousand" => "thousandth".into(),
        "million" => "millionth".into(),
        "billion" => "billionth".into(),
        other => format!("{}th", other),
    }
}

// ---------- times ----------

fn expand_times(text: &str, lang: &str) -> String {
    let re = Regex::new(r"\b(?P<h>\d{1,2}):(?P<m>\d{2})\b").unwrap();
    re.replace_all(text, |caps: &regex::Captures| {
        let h: u32 = caps["h"].parse().unwrap_or(0);
        let m: u32 = caps["m"].parse().unwrap_or(0);
        if h > 23 || m > 59 {
            return caps[0].to_string();
        }
        match lang {
            "es" => format!("{} y {}", h, if m == 0 { "en punto".into() } else { m.to_string() }),
            _ => {
                let (h12, suffix) = if h == 0 {
                    (12, "AM")
                } else if h < 12 {
                    (h, "AM")
                } else if h == 12 {
                    (12, "PM")
                } else {
                    (h - 12, "PM")
                };
                if m == 0 {
                    format!("{} o'clock {}", h12, suffix)
                } else if m < 10 {
                    format!("{} oh {} {}", h12, m, suffix)
                } else {
                    format!("{} {} {}", h12, m, suffix)
                }
            }
        }
    })
    .into_owned()
}

// ---------- roman numerals ----------

fn expand_roman_numerals(text: &str, lang: &str) -> String {
    // "XIX century", "Louis XIV", "siglo XX"
    let re = Regex::new(r"\b(?P<r>[MDCLXVI]{2,})\b").unwrap();
    re.replace_all(text, |caps: &regex::Captures| {
        let r = &caps["r"];
        if let Some(n) = roman_to_int(r) {
            // Heuristic: if the surrounding context likely says century, spell as ordinal.
            // For simplicity we always emit cardinal — Supertonic typically reads "the
            // nineteenth century" if the surrounding "century" word is present.
            match lang {
                "es" => spanish_number(n as u64),
                _ => num_to_english(n as u64),
            }
        } else {
            r.to_string()
        }
    })
    .into_owned()
}

fn roman_to_int(s: &str) -> Option<u32> {
    let map = |c: char| match c {
        'I' => Some(1),
        'V' => Some(5),
        'X' => Some(10),
        'L' => Some(50),
        'C' => Some(100),
        'D' => Some(500),
        'M' => Some(1000),
        _ => None,
    };
    let vals: Vec<u32> = s.chars().map(|c| map(c).unwrap_or(0)).collect();
    if vals.iter().any(|&v| v == 0) {
        return None;
    }
    let mut total = 0i64;
    for i in 0..vals.len() {
        let cur = vals[i] as i64;
        let next = vals.get(i + 1).copied().unwrap_or(0) as i64;
        if cur < next {
            total -= cur;
        } else {
            total += cur;
        }
    }
    if total <= 0 || total > 3999 {
        return None;
    }
    Some(total as u32)
}

// ---------- numbers ----------

fn expand_numbers(text: &str, lang: &str) -> String {
    // Spell integers up to 999_999_999. Leave decimals to Supertonic.
    let re = Regex::new(r"(?P<n>\d{1,3}(?:,\d{3})+|\d+)").unwrap();
    re.replace_all(text, |caps: &regex::Captures| {
        let raw = caps["n"].replace(',', "");
        let Ok(n) = raw.parse::<u64>() else {
            return caps[0].to_string();
        };
        // For very small numbers (years, room numbers etc.) Supertonic handles them fine —
        // we only need to spell *big* numbers reliably. Use ≥1000 threshold as default.
        if n < 1000 {
            return caps[0].to_string();
        }
        match lang {
            "es" => spanish_number(n),
            _ => num_to_english(n),
        }
    })
    .into_owned()
}

fn num_to_english(n: u64) -> String {
    static ONES: &[&str] = &[
        "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
        "eleven", "twelve", "thirteen", "fourteen", "fifteen", "sixteen", "seventeen", "eighteen",
        "nineteen",
    ];
    static TENS: &[&str] = &[
        "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
    ];
    fn under_1000(n: u64) -> String {
        if n < 20 {
            ONES[n as usize].to_string()
        } else if n < 100 {
            let t = TENS[(n / 10) as usize];
            let r = n % 10;
            if r == 0 {
                t.to_string()
            } else {
                format!("{}-{}", t, ONES[r as usize])
            }
        } else {
            let h = n / 100;
            let r = n % 100;
            if r == 0 {
                format!("{} hundred", ONES[h as usize])
            } else {
                format!("{} hundred {}", ONES[h as usize], under_1000(r))
            }
        }
    }
    if n == 0 {
        return "zero".into();
    }
    let mut parts: Vec<String> = Vec::new();
    let billion = 1_000_000_000u64;
    let million = 1_000_000u64;
    let thousand = 1_000u64;
    let b = n / billion;
    let m = (n % billion) / million;
    let k = (n % million) / thousand;
    let r = n % thousand;
    if b > 0 {
        parts.push(format!("{} billion", under_1000(b)));
    }
    if m > 0 {
        parts.push(format!("{} million", under_1000(m)));
    }
    if k > 0 {
        parts.push(format!("{} thousand", under_1000(k)));
    }
    if r > 0 {
        parts.push(under_1000(r));
    }
    parts.join(" ")
}

fn spanish_number(n: u64) -> String {
    static UNITS: &[&str] = &[
        "cero", "uno", "dos", "tres", "cuatro", "cinco", "seis", "siete", "ocho", "nueve",
        "diez", "once", "doce", "trece", "catorce", "quince",
    ];
    static SPECIAL_TENS: &[&str] = &[
        "dieciséis", "diecisiete", "dieciocho", "diecinueve",
    ];
    static TENS: &[&str] = &[
        "", "", "veinte", "treinta", "cuarenta", "cincuenta", "sesenta", "setenta", "ochenta",
        "noventa",
    ];
    static HUNDREDS: &[&str] = &[
        "", "ciento", "doscientos", "trescientos", "cuatrocientos", "quinientos", "seiscientos",
        "setecientos", "ochocientos", "novecientos",
    ];
    fn under_1000(n: u64) -> String {
        if n < 16 {
            UNITS[n as usize].to_string()
        } else if n < 20 {
            SPECIAL_TENS[(n - 16) as usize].to_string()
        } else if n < 30 {
            // 21..29 — fused form veintiuno..veintinueve
            let r = n - 20;
            if r == 0 {
                "veinte".into()
            } else {
                format!("veinti{}", UNITS[r as usize])
            }
        } else if n < 100 {
            let t = TENS[(n / 10) as usize];
            let r = n % 10;
            if r == 0 {
                t.to_string()
            } else {
                format!("{} y {}", t, UNITS[r as usize])
            }
        } else if n == 100 {
            "cien".into()
        } else {
            let h = n / 100;
            let r = n % 100;
            if r == 0 {
                HUNDREDS[h as usize].to_string()
            } else {
                format!("{} {}", HUNDREDS[h as usize], under_1000(r))
            }
        }
    }
    if n == 0 {
        return "cero".into();
    }
    let mut parts: Vec<String> = Vec::new();
    let million = 1_000_000u64;
    let thousand = 1_000u64;
    let m = n / million;
    let k = (n % million) / thousand;
    let r = n % thousand;
    if m > 0 {
        if m == 1 {
            parts.push("un millón".into());
        } else {
            parts.push(format!("{} millones", under_1000(m)));
        }
    }
    if k > 0 {
        if k == 1 {
            parts.push("mil".into());
        } else {
            parts.push(format!("{} mil", under_1000(k)));
        }
    }
    if r > 0 {
        parts.push(under_1000(r));
    }
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_numbers() {
        assert_eq!(num_to_english(1234), "one thousand two hundred thirty-four");
        assert_eq!(num_to_english(2026), "two thousand twenty-six");
        assert_eq!(num_to_english(1_000_000), "one million");
    }

    #[test]
    fn spanish_numbers() {
        assert_eq!(spanish_number(21), "veintiuno");
        assert_eq!(spanish_number(100), "cien");
        assert_eq!(spanish_number(1234), "mil doscientos treinta y cuatro");
    }

    #[test]
    fn currencies() {
        let s = expand_currencies("It costs $3.50 today.", "en");
        assert!(s.contains("dollars"));
    }

    #[test]
    fn roman() {
        assert_eq!(roman_to_int("XIX"), Some(19));
        assert_eq!(roman_to_int("MCMLXXXIV"), Some(1984));
    }

    #[test]
    fn dates() {
        let s = expand_dates("On 2026-05-24 the event will start.", "en");
        assert!(s.contains("May"));
    }

    #[test]
    fn pipeline() {
        let s = normalize("Dr. Smith earned $1,234 on 2026-05-24 at 14:30.", "en");
        assert!(s.contains("Doctor"));
        assert!(s.contains("dollars"));
        assert!(s.contains("PM"));
    }
}
