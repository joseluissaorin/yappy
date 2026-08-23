//! Intérprete de reglas RBNF (Rule-Based Number Format) de CLDR.
//!
//! Lee las gramáticas vendorizadas en `data/rbnf/*.txt` (extraídas de CLDR
//! con `tooling/rbnf/extraer.py`) y deletrea números en los 31 idiomas de
//! Supertonic: cardinales, ordinales, años y decimales, con género y
//! declinación donde el idioma los tiene.
//!
//! Cubre exactamente lo que aparece en esos datos: sustituciones ←← / →→
//! (con conjunto nombrado o no), →→→ (regla anterior; ja/ko), =%conjunto=,
//! =#,##0= y =0.0= (dígitos), tramos opcionales [...], reglas con radix
//! (`1010/100:`), reglas -x / x.x / x,x / 0.x, y la selección de plural
//! $(cardinal|ordinal, caso{...}...)$ de las lenguas eslavas y otras.

use std::collections::HashMap;
use std::sync::OnceLock;

// ── Datos vendorizados ───────────────────────────────────────────────────

macro_rules! datos {
    ($($lang:literal),+ $(,)?) => {
        fn texto_reglas(lang: &str) -> Option<&'static str> {
            match lang {
                $($lang => Some(include_str!(concat!("../../data/rbnf/", $lang, ".txt"))),)+
                _ => None,
            }
        }
    };
}

datos!(
    "en", "ko", "ja", "ar", "bg", "cs", "da", "de", "el", "es", "et", "fi",
    "fr", "hi", "hr", "hu", "id", "it", "lt", "lv", "nl", "pl", "pt", "ro",
    "ru", "sk", "sl", "sv", "tr", "uk", "vi", "root",
);

// ── Modelo ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
enum Tok {
    Literal(String),
    /// ←← o ←%conjunto← : el cociente
    Cociente(Option<String>),
    /// →→ o →%conjunto→ : el resto
    Resto(Option<String>),
    /// →→→ : el resto, con la regla anterior
    RestoReglaAnterior,
    /// =%conjunto= : el mismo número por otro conjunto
    MismoConjunto(String),
    /// =0=, =0.0=, =#,##0= : el número en dígitos
    Digitos { decimal: bool },
    /// [ ... ] : tramo opcional
    Opcional(Vec<Tok>),
    /// $(cardinal|ordinal, caso{texto}...)$
    Plural { ordinal: bool, casos: Vec<(String, String)> },
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Clave {
    Entero(i128),
    Negativo,
    /// x.x y x,x (fracción impropia)
    Fraccion,
    /// 0.x (fracción propia)
    FraccionPropia,
    Otro,
}

#[derive(Debug, Clone)]
struct Regla {
    clave: Clave,
    radix: Option<i128>,
    cuerpo: Vec<Tok>,
}

#[derive(Debug, Default)]
struct Conjunto {
    reglas_enteras: Vec<Regla>, // ordenadas por base ascendente
    negativa: Option<Regla>,
    /// regla x.x (la palabra suele ser «punto»)
    fraccion: Option<Regla>,
    /// regla x,x (la palabra suele ser «coma»)
    fraccion_coma: Option<Regla>,
    fraccion_propia: Option<Regla>,
}

/// Idiomas cuyo separador decimal canónico es el punto; el resto leen coma.
fn decimal_con_punto(lang: &str) -> bool {
    matches!(lang, "en" | "ja" | "ko" | "hi")
}

pub struct MotorRbnf {
    lang: String,
    conjuntos: HashMap<String, Conjunto>,
}

/// Un número ya descompuesto: parte entera y dígitos decimales tal cual
/// venían escritos (para leer «3,14» como «tres coma uno cuatro»).
#[derive(Debug, Clone)]
pub struct Numero {
    pub negativo: bool,
    pub entero: i128,
    pub decimales: Option<String>,
}

impl Numero {
    pub fn entero_de(n: i128) -> Self {
        Numero { negativo: n < 0, entero: n.abs(), decimales: None }
    }
}

// ── Parseo ───────────────────────────────────────────────────────────────

fn parsear_cuerpo(s: &str) -> Vec<Tok> {
    let mut toks: Vec<Tok> = Vec::new();
    let mut lit = String::new();
    let cs: Vec<char> = s.chars().collect();
    let mut i = 0usize;

    fn cerrar(lit: &mut String, toks: &mut Vec<Tok>) {
        if !lit.is_empty() {
            toks.push(Tok::Literal(std::mem::take(lit)));
        }
    }

    while i < cs.len() {
        let c = cs[i];
        match c {
            '←' => {
                cerrar(&mut lit, &mut toks);
                if i + 1 < cs.len() && cs[i + 1] == '←' {
                    toks.push(Tok::Cociente(None));
                    i += 2;
                } else {
                    // ←%conjunto←
                    let mut j = i + 1;
                    let mut nombre = String::new();
                    while j < cs.len() && cs[j] != '←' {
                        nombre.push(cs[j]);
                        j += 1;
                    }
                    toks.push(Tok::Cociente(Some(nombre.trim_start_matches('%').to_string())));
                    i = j + 1;
                }
            }
            '→' => {
                cerrar(&mut lit, &mut toks);
                if i + 2 < cs.len() && cs[i + 1] == '→' && cs[i + 2] == '→' {
                    toks.push(Tok::RestoReglaAnterior);
                    i += 3;
                } else if i + 1 < cs.len() && cs[i + 1] == '→' {
                    toks.push(Tok::Resto(None));
                    i += 2;
                } else {
                    let mut j = i + 1;
                    let mut nombre = String::new();
                    while j < cs.len() && cs[j] != '→' {
                        nombre.push(cs[j]);
                        j += 1;
                    }
                    toks.push(Tok::Resto(Some(nombre.trim_start_matches('%').to_string())));
                    i = j + 1;
                }
            }
            '=' => {
                cerrar(&mut lit, &mut toks);
                let mut j = i + 1;
                let mut dentro = String::new();
                while j < cs.len() && cs[j] != '=' {
                    dentro.push(cs[j]);
                    j += 1;
                }
                if dentro.starts_with('%') {
                    toks.push(Tok::MismoConjunto(dentro.trim_start_matches('%').to_string()));
                } else {
                    toks.push(Tok::Digitos { decimal: dentro.contains('.') });
                }
                i = j + 1;
            }
            '[' => {
                cerrar(&mut lit, &mut toks);
                let mut j = i + 1;
                let mut dentro = String::new();
                let mut prof = 1;
                while j < cs.len() && prof > 0 {
                    match cs[j] {
                        '[' => prof += 1,
                        ']' => prof -= 1,
                        _ => {}
                    }
                    if prof > 0 {
                        dentro.push(cs[j]);
                    }
                    j += 1;
                }
                toks.push(Tok::Opcional(parsear_cuerpo(&dentro)));
                i = j;
            }
            '$' if i + 1 < cs.len() && cs[i + 1] == '(' => {
                cerrar(&mut lit, &mut toks);
                let mut j = i + 2;
                let mut dentro = String::new();
                while j + 1 < cs.len() && !(cs[j] == ')' && cs[j + 1] == '$') {
                    dentro.push(cs[j]);
                    j += 1;
                }
                let (tipo, resto) = dentro.split_once(',').unwrap_or(("cardinal", ""));
                let mut casos = Vec::new();
                let rc: Vec<char> = resto.chars().collect();
                let mut k = 0usize;
                while k < rc.len() {
                    // nombre{texto}
                    let mut nombre = String::new();
                    while k < rc.len() && rc[k] != '{' {
                        nombre.push(rc[k]);
                        k += 1;
                    }
                    k += 1; // '{'
                    let mut texto = String::new();
                    while k < rc.len() && rc[k] != '}' {
                        texto.push(rc[k]);
                        k += 1;
                    }
                    k += 1; // '}'
                    if !nombre.trim().is_empty() {
                        casos.push((nombre.trim().to_string(), texto));
                    }
                }
                toks.push(Tok::Plural { ordinal: tipo.trim() == "ordinal", casos });
                i = j + 2;
            }
            '\'' => {
                // Apóstrofo de cita RBNF (ICU): protege espacios o signos al
                // principio del texto («' e», «' '») y se descarta siempre.
                // Ningún dato vendorizado usa «''» como apóstrofo literal
                // (el ucraniano escribe «пʼять» con U+02BC, que no es este
                // carácter), así que soltarlo sin más es seguro.
                i += 1;
            }
            _ => {
                lit.push(c);
                i += 1;
            }
        }
    }
    cerrar(&mut lit, &mut toks);
    toks
}

fn parsear(texto: &str, lang: &str) -> MotorRbnf {
    let mut conjuntos: HashMap<String, Conjunto> = HashMap::new();
    let mut actual: Option<String> = None;

    for linea in texto.lines() {
        let linea = linea.trim_end();
        if linea.is_empty() {
            continue;
        }
        if linea.starts_with('%') {
            let nombre = linea.trim_start_matches('%').trim_end_matches(':').to_string();
            conjuntos.entry(nombre.clone()).or_default();
            actual = Some(nombre);
            continue;
        }
        let Some(nombre) = &actual else { continue };
        let Some((clave_txt, cuerpo_txt)) = linea.split_once(':') else { continue };
        let cuerpo_txt = cuerpo_txt.trim_start().trim_end_matches(';');
        let cuerpo = parsear_cuerpo(cuerpo_txt);
        let clave_txt = clave_txt.trim();

        let (clave, radix) = match clave_txt {
            "-x" => (Clave::Negativo, None),
            "x.x" | "x,x" => (Clave::Fraccion, None),
            "0.x" | "0,x" => (Clave::FraccionPropia, None),
            "x.0" | "x,0" | "Inf" | "NaN" => (Clave::Otro, None),
            _ => {
                if let Some((base, r)) = clave_txt.split_once('/') {
                    match (base.parse::<i128>(), r.parse::<i128>()) {
                        (Ok(b), Ok(r)) => (Clave::Entero(b), Some(r)),
                        _ => (Clave::Otro, None),
                    }
                } else {
                    match clave_txt.parse::<i128>() {
                        Ok(b) => (Clave::Entero(b), None),
                        Err(_) => (Clave::Otro, None),
                    }
                }
            }
        };

        let regla = Regla { clave, radix, cuerpo };
        let conjunto = conjuntos.get_mut(nombre).expect("conjunto recién creado");
        match clave {
            Clave::Entero(_) => conjunto.reglas_enteras.push(regla),
            Clave::Negativo => conjunto.negativa = Some(regla),
            Clave::Fraccion => {
                // x.x y x,x traen palabras de separador distintas («punto»
                // y «coma»); guardamos las dos y se elige por idioma al
                // deletrear.
                if clave_txt.contains(',') {
                    conjunto.fraccion_coma = Some(regla);
                } else {
                    conjunto.fraccion = Some(regla);
                }
            }
            Clave::FraccionPropia => conjunto.fraccion_propia = Some(regla),
            Clave::Otro => {}
        }
    }

    for c in conjuntos.values_mut() {
        c.reglas_enteras.sort_by_key(|r| match r.clave {
            Clave::Entero(b) => b,
            _ => 0,
        });
    }

    MotorRbnf { lang: lang.to_string(), conjuntos }
}

// ── Plurales (solo las lenguas cuyos datos usan $(...)$) ────────────────

fn categoria_cardinal(lang: &str, n: i128) -> &'static str {
    let n = n.abs();
    let (m10, m100) = (n % 10, n % 100);
    match lang {
        "ru" | "uk" => {
            if m10 == 1 && m100 != 11 {
                "one"
            } else if (2..=4).contains(&m10) && !(12..=14).contains(&m100) {
                "few"
            } else {
                "many"
            }
        }
        "pl" => {
            if n == 1 {
                "one"
            } else if (2..=4).contains(&m10) && !(12..=14).contains(&m100) {
                "few"
            } else {
                "many"
            }
        }
        "cs" | "sk" => {
            if n == 1 {
                "one"
            } else if (2..=4).contains(&n) {
                "few"
            } else {
                "other"
            }
        }
        "bg" | "en" | "de" | "es" | "it" | "nl" | "sv" | "el" | "fi" | "et" | "hu" | "tr" => {
            if n == 1 { "one" } else { "other" }
        }
        "fr" | "pt" => {
            if n <= 1 { "one" } else { "other" }
        }
        _ => {
            if n == 1 { "one" } else { "other" }
        }
    }
}

fn categoria_ordinal(lang: &str, n: i128) -> &'static str {
    let n = n.abs();
    let (m10, m100) = (n % 10, n % 100);
    match lang {
        "en" => {
            if m10 == 1 && m100 != 11 {
                "one"
            } else if m10 == 2 && m100 != 12 {
                "two"
            } else if m10 == 3 && m100 != 13 {
                "few"
            } else {
                "other"
            }
        }
        "fr" => {
            if n == 1 { "one" } else { "other" }
        }
        "sv" => {
            if (m10 == 1 || m10 == 2) && !(m100 == 11 || m100 == 12) {
                "one"
            } else {
                "other"
            }
        }
        _ => "other",
    }
}

fn elegir_plural<'a>(
    lang: &str,
    n: i128,
    ordinal: bool,
    casos: &'a [(String, String)],
) -> &'a str {
    let cat = if ordinal { categoria_ordinal(lang, n) } else { categoria_cardinal(lang, n) };
    casos
        .iter()
        .find(|(c, _)| c == cat)
        .or_else(|| casos.iter().find(|(c, _)| c == "other"))
        .or_else(|| casos.last())
        .map(|(_, t)| t.as_str())
        .unwrap_or("")
}

// ── Evaluación ───────────────────────────────────────────────────────────

impl MotorRbnf {
    fn conjunto(&self, nombre: &str) -> Option<&Conjunto> {
        self.conjuntos.get(nombre)
    }

    /// Divisor de una regla: la mayor potencia del radix ≤ base.
    fn divisor(base: i128, radix: i128) -> i128 {
        if base == 0 {
            return 1;
        }
        let mut d = 1i128;
        while d.saturating_mul(radix) <= base {
            d = d.saturating_mul(radix);
        }
        d
    }

    fn indice_regla(conjunto: &Conjunto, n: i128) -> Option<usize> {
        // La última regla con base ≤ n.
        let mut elegido: Option<usize> = None;
        for (i, r) in conjunto.reglas_enteras.iter().enumerate() {
            if let Clave::Entero(b) = r.clave {
                if b <= n {
                    elegido = Some(i);
                } else {
                    break;
                }
            }
        }
        elegido
    }

    fn evaluar_entero(&self, nombre: &str, n: i128, prof: usize) -> Option<String> {
        if prof > 64 {
            return None; // recursión desbocada: datos corruptos
        }
        let conjunto = self.conjunto(nombre)?;

        if n < 0 {
            if let Some(neg) = &conjunto.negativa {
                return self.evaluar_cuerpo(nombre, &neg.cuerpo, n.abs(), 1, None, prof + 1);
            }
            return self
                .evaluar_entero(nombre, n.abs(), prof + 1)
                .map(|s| format!("-{s}"));
        }

        let idx = Self::indice_regla(conjunto, n)?;
        self.evaluar_regla(nombre, conjunto, idx, n, prof)
    }

    fn evaluar_regla(
        &self,
        nombre: &str,
        conjunto: &Conjunto,
        idx: usize,
        n: i128,
        prof: usize,
    ) -> Option<String> {
        let regla = &conjunto.reglas_enteras[idx];
        let base = match regla.clave {
            Clave::Entero(b) => b,
            _ => return None,
        };
        let radix = regla.radix.unwrap_or(10);
        let divisor = Self::divisor(base, radix);
        self.evaluar_cuerpo_con_regla(nombre, conjunto, idx, &regla.cuerpo, n, divisor, prof)
    }

    fn evaluar_cuerpo(
        &self,
        nombre: &str,
        cuerpo: &[Tok],
        n: i128,
        divisor: i128,
        idx_regla: Option<usize>,
        prof: usize,
    ) -> Option<String> {
        let conjunto = self.conjunto(nombre)?;
        match idx_regla {
            Some(i) => self.evaluar_cuerpo_con_regla(nombre, conjunto, i, cuerpo, n, divisor, prof),
            None => self.evaluar_toks(nombre, conjunto, None, cuerpo, n, divisor, prof),
        }
    }

    fn evaluar_cuerpo_con_regla(
        &self,
        nombre: &str,
        conjunto: &Conjunto,
        idx: usize,
        cuerpo: &[Tok],
        n: i128,
        divisor: i128,
        prof: usize,
    ) -> Option<String> {
        self.evaluar_toks(nombre, conjunto, Some(idx), cuerpo, n, divisor, prof)
    }

    fn evaluar_toks(
        &self,
        nombre: &str,
        conjunto: &Conjunto,
        idx: Option<usize>,
        cuerpo: &[Tok],
        n: i128,
        divisor: i128,
        prof: usize,
    ) -> Option<String> {
        let mut salida = String::new();
        // El valor de contexto para $(...)$: el del último ←← evaluado, o n.
        let mut contexto_plural = n;

        for tok in cuerpo {
            match tok {
                Tok::Literal(t) => salida.push_str(t),
                Tok::Cociente(conj) => {
                    let q = if divisor > 0 { n / divisor } else { n };
                    contexto_plural = q;
                    let destino = conj.as_deref().unwrap_or(nombre);
                    salida.push_str(&self.evaluar_entero(destino, q, prof + 1)?);
                }
                Tok::Resto(conj) => {
                    let r = if divisor > 0 { n % divisor } else { n };
                    let destino = conj.as_deref().unwrap_or(nombre);
                    salida.push_str(&self.evaluar_entero(destino, r, prof + 1)?);
                }
                Tok::RestoReglaAnterior => {
                    let r = if divisor > 0 { n % divisor } else { n };
                    let i = idx?;
                    if i == 0 {
                        return None;
                    }
                    salida.push_str(&self.evaluar_regla(nombre, conjunto, i - 1, r, prof + 1)?);
                }
                Tok::MismoConjunto(otro) => {
                    salida.push_str(&self.evaluar_entero(otro, n, prof + 1)?);
                }
                Tok::Digitos { .. } => {
                    salida.push_str(&n.to_string());
                }
                Tok::Opcional(dentro) => {
                    // Se omite si el número es múltiplo exacto del divisor.
                    if divisor > 1 && n % divisor == 0 {
                        continue;
                    }
                    salida.push_str(&self.evaluar_toks(
                        nombre, conjunto, idx, dentro, n, divisor, prof + 1,
                    )?);
                }
                Tok::Plural { ordinal, casos } => {
                    salida.push_str(elegir_plural(&self.lang, contexto_plural, *ordinal, casos));
                }
            }
        }
        Some(salida)
    }

    /// Deletrea un número (con decimales si los trae) por un conjunto dado.
    pub fn deletrear(&self, numero: &Numero, conjunto: &str) -> Option<String> {
        let c = self.conjunto(conjunto)?;

        if let Some(dec) = &numero.decimales {
            if !dec.is_empty() {
                let regla = if decimal_con_punto(&self.lang) {
                    c.fraccion.as_ref().or(c.fraccion_coma.as_ref())?
                } else {
                    c.fraccion_coma.as_ref().or(c.fraccion.as_ref())?
                };
                let mut salida = String::new();
                for tok in &regla.cuerpo {
                    match tok {
                        Tok::Literal(t) => salida.push_str(t),
                        Tok::Cociente(conj) => {
                            let destino = conj.as_deref().unwrap_or(conjunto);
                            let e = if numero.negativo { -numero.entero } else { numero.entero };
                            salida.push_str(&self.evaluar_entero(destino, e, 0)?);
                        }
                        Tok::Resto(conj) => {
                            let destino = conj.as_deref().unwrap_or(conjunto);
                            // Dos decimales sin cero inicial se leen como
                            // un cardinal («1,75» → «uno coma setenta y
                            // cinco»), que es como habla la gente; lo demás
                            // va dígito a dígito («3,14159»).
                            if dec.len() == 2 && !dec.starts_with('0') {
                                let v: i128 = dec.parse().ok()?;
                                salida.push_str(&self.evaluar_entero(destino, v, 0)?);
                            } else {
                                let mut partes = Vec::new();
                                for d in dec.chars() {
                                    let v = d.to_digit(10)? as i128;
                                    partes.push(self.evaluar_entero(destino, v, 0)?);
                                }
                                salida.push_str(&partes.join(" "));
                            }
                        }
                        Tok::Digitos { .. } => {
                            salida.push_str(&format!("{}.{}", numero.entero, dec));
                        }
                        _ => {}
                    }
                }
                return Some(salida);
            }
        }

        let n = if numero.negativo { -numero.entero } else { numero.entero };
        self.evaluar_entero(conjunto, n, 0)
    }

    pub fn tiene_conjunto(&self, nombre: &str) -> bool {
        self.conjuntos.contains_key(nombre)
    }
}

// ── Caché por idioma y fachada ───────────────────────────────────────────

fn cache() -> &'static std::sync::Mutex<HashMap<String, &'static MotorRbnf>> {
    static CACHE: OnceLock<std::sync::Mutex<HashMap<String, &'static MotorRbnf>>> =
        OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

/// Motor para un idioma, cacheado para siempre (los datos son estáticos).
pub fn motor(lang: &str) -> Option<&'static MotorRbnf> {
    if let Some(m) = cache().lock().unwrap().get(lang) {
        return Some(m);
    }
    let texto = texto_reglas(lang)?;
    let motor: &'static MotorRbnf = Box::leak(Box::new(parsear(texto, lang)));
    cache().lock().unwrap().insert(lang.to_string(), motor);
    Some(motor)
}

fn primero_disponible(m: &MotorRbnf, candidatos: &[&str]) -> Option<String> {
    candidatos
        .iter()
        .find(|c| m.tiene_conjunto(c))
        .map(|c| c.to_string())
}

/// Género gramatical pedido al deletrear (donde el idioma distingue).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Genero {
    #[default]
    Masculino,
    Femenino,
    Neutro,
}

/// Limpia tipografía interna de CLDR que no debe llegar al sintetizador:
/// guiones blandos (U+00AD) y espacios duros.
fn limpiar(s: String) -> String {
    s.replace('\u{ad}', "").replace('\u{a0}', " ")
}

/// Cardinal: «1492» → «mil cuatrocientos noventa y dos».
///
/// `Genero::Masculino` funciona como forma NO MARCADA: prefiere el conjunto
/// llano (es: «veintiuno», no el apocopado «veintiún» de antes de
/// sustantivo). El femenino sí pide su conjunto propio donde exista.
pub fn cardinal(lang: &str, numero: &Numero, genero: Genero) -> Option<String> {
    let m = motor(lang)?;
    let orden: &[&str] = match genero {
        Genero::Femenino => &[
            "spellout-cardinal-feminine",
            "spellout-cardinal",
            "spellout-numbering",
        ],
        Genero::Neutro => &[
            "spellout-cardinal-neuter",
            "spellout-cardinal",
            "spellout-numbering",
        ],
        // No marcado: el conjunto de «numeración» es la forma plena
        // estándar en CLDR (es: «veintiuno»; el cardinal-masculine es el
        // apocopado de antes de sustantivo, «veintiún»).
        Genero::Masculino => &[
            "spellout-numbering",
            "spellout-cardinal",
            "spellout-cardinal-masculine",
        ],
    };
    // Con decimales, el conjunto elegido tiene que saber leer la fracción:
    // en inglés «spellout-numbering» no trae regla x.x (vive en
    // «spellout-cardinal», con su «point»), y quedarse en el primero
    // convertía «2.5» en nada.
    let conjunto = if numero.decimales.is_some() {
        orden
            .iter()
            .find(|nombre| {
                m.conjuntos
                    .get(**nombre)
                    .is_some_and(|c| c.fraccion.is_some() || c.fraccion_coma.is_some())
            })
            .map(|s| s.to_string())
            .or_else(|| primero_disponible(m, orden))?
    } else {
        primero_disponible(m, orden)?
    };
    m.deletrear(numero, &conjunto).map(limpiar)
}

/// Ordinal: «8.º» → «octavo». Devuelve None si el idioma no lo define.
pub fn ordinal(lang: &str, n: i128, genero: Genero) -> Option<String> {
    let m = motor(lang)?;
    let orden: &[&str] = match genero {
        Genero::Femenino => &["spellout-ordinal-feminine", "spellout-ordinal"],
        Genero::Neutro => &["spellout-ordinal-neuter", "spellout-ordinal"],
        Genero::Masculino => &["spellout-ordinal", "spellout-ordinal-masculine"],
    };
    let conjunto = primero_disponible(m, orden)?;
    m.deletrear(&Numero::entero_de(n), &conjunto).map(limpiar)
}

/// Año: en inglés «1985» → «nineteen eighty-five»; donde no hay regla de
/// año, cae al cardinal.
pub fn anio(lang: &str, n: i128) -> Option<String> {
    let m = motor(lang)?;
    if m.tiene_conjunto("spellout-numbering-year") {
        return m
            .deletrear(&Numero::entero_de(n), "spellout-numbering-year")
            .map(limpiar);
    }
    cardinal(lang, &Numero::entero_de(n), Genero::Masculino)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(lang: &str, n: i128) -> String {
        cardinal(lang, &Numero::entero_de(n), Genero::Masculino).unwrap_or_default()
    }

    #[test]
    fn espanol() {
        assert_eq!(c("es", 0), "cero");
        assert_eq!(c("es", 16), "dieciséis");
        assert_eq!(c("es", 21), "veintiuno");
        assert_eq!(c("es", 77), "setenta y siete");
        assert_eq!(c("es", 100), "cien");
        assert_eq!(c("es", 101), "ciento uno");
        assert_eq!(c("es", 1492), "mil cuatrocientos noventa y dos");
        assert_eq!(c("es", 2026), "dos mil veintiséis");
        assert_eq!(c("es", 1_000_000), "un millón");
        assert_eq!(c("es", 5_000_000), "cinco millones");
        assert_eq!(
            cardinal("es", &Numero::entero_de(21), Genero::Femenino).unwrap(),
            "veintiuna"
        );
        assert_eq!(ordinal("es", 8, Genero::Masculino).unwrap(), "octavo");
        assert_eq!(ordinal("es", 20, Genero::Masculino).unwrap(), "vigésimo");
    }

    #[test]
    fn espanol_decimales() {
        let n = Numero { negativo: false, entero: 3, decimales: Some("14".into()) };
        assert_eq!(cardinal("es", &n, Genero::Masculino).unwrap(), "tres coma catorce");
        let pi = Numero { negativo: false, entero: 3, decimales: Some("1416".into()) };
        assert_eq!(
            cardinal("es", &pi, Genero::Masculino).unwrap(),
            "tres coma uno cuatro uno seis"
        );
    }

    #[test]
    fn ingles() {
        assert_eq!(c("en", 42), "forty-two");
        assert_eq!(c("en", 1985), "one thousand nine hundred eighty-five");
        assert_eq!(anio("en", 1985).unwrap(), "nineteen eighty-five");
        assert_eq!(anio("en", 2026).unwrap(), "twenty twenty-six");
        assert_eq!(ordinal("en", 21, Genero::Masculino).unwrap(), "twenty-first");
        assert_eq!(ordinal("en", 3, Genero::Masculino).unwrap(), "third");
    }

    #[test]
    fn frances() {
        assert_eq!(c("fr", 21), "vingt-et-un");
        assert_eq!(c("fr", 71), "soixante-et-onze");
        assert_eq!(c("fr", 80), "quatre-vingts");
        assert_eq!(c("fr", 99), "quatre-vingt-dix-neuf");
    }

    #[test]
    fn aleman() {
        assert_eq!(c("de", 21), "einundzwanzig");
        assert_eq!(c("de", 1985), "eintausendneunhundertfünfundachtzig");
    }

    #[test]
    fn italiano_portugues() {
        assert_eq!(c("it", 21), "ventuno");
        assert_eq!(c("it", 28), "ventotto");
        assert_eq!(c("it", 1800), "milleottocento");
        assert_eq!(c("pt", 21), "vinte e um");
        assert_eq!(c("pt", 100), "cem");
        assert_eq!(c("pt", 101), "cento e um");
    }

    #[test]
    fn ruso_plurales() {
        // La concordancia «тысяча/тысячи/тысяч» sale del $(cardinal,...)$.
        assert_eq!(c("ru", 1000), "одна тысяча");
        assert_eq!(c("ru", 2000), "две тысячи");
        assert_eq!(c("ru", 5000), "пять тысяч");
        assert_eq!(c("ru", 21), "двадцать один");
    }

    #[test]
    fn japones_coreano() {
        assert_eq!(c("ja", 42), "四十二");
        assert_eq!(c("ja", 2026), "二千二十六");
        assert_eq!(c("ko", 42), "사십이");
    }

    #[test]
    fn todos_los_idiomas_deletrean_algo() {
        for lang in [
            "en", "ko", "ja", "ar", "bg", "cs", "da", "de", "el", "es", "et", "fi", "fr", "hi",
            "hr", "hu", "id", "it", "lt", "lv", "nl", "pl", "pt", "ro", "ru", "sk", "sl", "sv",
            "tr", "uk", "vi",
        ] {
            for n in [0i128, 1, 7, 15, 21, 99, 100, 101, 1000, 1234, 1_000_000] {
                let s = cardinal(lang, &Numero::entero_de(n), Genero::Masculino);
                assert!(
                    s.as_deref().is_some_and(|s| !s.is_empty()),
                    "{lang} no deletrea {n}: {s:?}"
                );
                // Un cardinal deletreado no debería quedarse en dígitos
                // (salvo lenguas cuyo propio CLDR caiga a dígitos).
                if !["ja", "ko"].contains(&lang) && n <= 1000 {
                    assert!(
                        !s.as_deref().unwrap().contains(char::is_numeric),
                        "{lang} deja dígitos en {n}: {s:?}"
                    );
                }
                // El apóstrofo de cita de ICU jamás debe llegar a la voz.
                assert!(
                    !s.as_deref().unwrap().contains('\''),
                    "{lang} filtra un apóstrofo en {n}: {s:?}"
                );
            }
        }
    }

    #[test]
    fn portugues_sin_apostrofo() {
        let n = Numero::entero_de(1234);
        assert_eq!(
            cardinal("pt", &n, Genero::Masculino).unwrap(),
            "mil duzentos e trinta e quatro"
        );
    }

    #[test]
    fn decimal_ingles_con_point() {
        let n = Numero { negativo: false, entero: 2, decimales: Some("5".into()) };
        assert_eq!(cardinal("en", &n, Genero::Masculino).unwrap(), "two point five");
    }
}
