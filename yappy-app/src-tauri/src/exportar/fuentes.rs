//! LAS LETRAS DEL PDF. Viajan dentro del binario las tres voces de la casa
//! (Literata para leer, Baloo 2 para gritar, Space Mono para susurrar), en
//! instancias estáticas recortadas: ~600 KB. Cubren el latín entero
//! (tildes, eñes, ç, ł, ő, ș…), el cirílico, el griego y el vietnamita.
//!
//! Lo que no cubren (japonés, chino, coreano, árabe, hebreo, devanagari,
//! tailandés, emoji…) se pide PRESTADO al sistema en el momento de exportar:
//! las letras de iOS, Android, macOS, Windows o Linux ya están en el
//! aparato, así que no engordan la app. Solo se leen si el documento las
//! necesita y solo se incrusta en el PDF el subconjunto de glifos usado.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rustybuzz::ttf_parser;

pub const LITERATA: &[u8] = include_bytes!("fuentes/Literata-Regular.ttf");
pub const LITERATA_NEGRA: &[u8] = include_bytes!("fuentes/Literata-Bold.ttf");
pub const BALOO: &[u8] = include_bytes!("fuentes/Baloo2-ExtraBold.ttf");
pub const MONO: &[u8] = include_bytes!("fuentes/SpaceMono-Regular.ttf");

#[derive(Clone)]
pub enum Datos {
    Propios(&'static [u8]),
    Prestados(Arc<Vec<u8>>),
}

#[derive(Clone)]
pub struct Fuente {
    pub datos: Datos,
    pub indice: u32,
    /// Para los registros: de dónde salió.
    pub nombre: String,
}

impl Fuente {
    pub fn bytes(&self) -> &[u8] {
        match &self.datos {
            Datos::Propios(b) => b,
            Datos::Prestados(v) => v.as_slice(),
        }
    }
    pub fn krilla(&self) -> Option<krilla::text::Font> {
        let datos: krilla::Data = match &self.datos {
            Datos::Propios(b) => (*b).into(),
            Datos::Prestados(v) => v.clone().into(),
        };
        krilla::text::Font::new(datos, self.indice)
    }
}

/// Índices fijos de las tres voces dentro del juego.
pub const I_LITERATA: usize = 0;
pub const I_LITERATA_NEGRA: usize = 1;
pub const I_BALOO: usize = 2;
pub const I_MONO: usize = 3;

pub fn propias() -> Vec<Fuente> {
    [
        (LITERATA, "Literata"),
        (LITERATA_NEGRA, "Literata Bold"),
        (BALOO, "Baloo 2 ExtraBold"),
        (MONO, "Space Mono"),
    ]
    .into_iter()
    .map(|(b, n)| Fuente {
        datos: Datos::Propios(b),
        indice: 0,
        nombre: n.into(),
    })
    .collect()
}

/// Caracteres que no necesitan glifo (los controles, los saltos y los
/// selectores invisibles): nunca disparan la búsqueda de respaldo.
pub fn sin_glifo(c: char) -> bool {
    c.is_control()
        || matches!(c as u32, 0x200B..=0x200F | 0x2028..=0x202E | 0x2060..=0x2064 | 0xFE00..=0xFE0F | 0xFEFF)
}

pub fn cubre(face: &ttf_parser::Face<'_>, c: char) -> bool {
    face.glyph_index(c).is_some()
}

/// Las carpetas de letras del sistema, por plataforma.
fn carpetas_del_sistema() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = Vec::new();
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        v.push("/System/Library/Fonts".into());
        #[cfg(target_os = "macos")]
        {
            v.push("/Library/Fonts".into());
            if let Some(h) = dirs::home_dir() {
                v.push(h.join("Library/Fonts"));
            }
        }
    }
    #[cfg(target_os = "android")]
    {
        v.push("/system/fonts".into());
        v.push("/product/fonts".into());
    }
    #[cfg(target_os = "windows")]
    {
        let raiz = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
        v.push(PathBuf::from(raiz).join("Fonts"));
        if let Some(l) = dirs::data_local_dir() {
            v.push(l.join("Microsoft\\Windows\\Fonts"));
        }
    }
    #[cfg(all(
        unix,
        not(any(target_os = "macos", target_os = "ios", target_os = "android"))
    ))]
    {
        v.push("/usr/share/fonts".into());
        v.push("/usr/local/share/fonts".into());
        if let Some(h) = dirs::home_dir() {
            v.push(h.join(".local/share/fonts"));
            v.push(h.join(".fonts"));
        }
    }
    v
}

fn listar(dir: &Path, prof: u8, out: &mut Vec<(PathBuf, u64)>) {
    let Ok(entradas) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entradas.flatten() {
        let p = e.path();
        let Ok(meta) = e.metadata() else { continue };
        if meta.is_dir() {
            if prof < 4 {
                listar(&p, prof + 1, out);
            }
            continue;
        }
        let ext = p
            .extension()
            .and_then(|x| x.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if matches!(ext.as_str(), "ttf" | "otf" | "ttc" | "otc") {
            out.push((p, meta.len()));
        }
    }
}

/// Las preferencias por escritura: fragmentos del nombre de fichero, de
/// mejor a peor. Serif primero (casa con Literata), luego lo que haya.
fn preferencias(faltan: &BTreeSet<char>, idioma: Option<&str>) -> Vec<&'static str> {
    let hay = |f: fn(u32) -> bool| faltan.iter().any(|c| f(*c as u32));
    let kana = hay(|u| (0x3040..=0x30FF).contains(&u) || (0x31F0..=0x31FF).contains(&u));
    let hangul = hay(|u| {
        (0xAC00..=0xD7AF).contains(&u)
            || (0x1100..=0x11FF).contains(&u)
            || (0x3130..=0x318F).contains(&u)
    });
    let han = hay(|u| {
        (0x4E00..=0x9FFF).contains(&u)
            || (0x3400..=0x4DBF).contains(&u)
            || (0x3000..=0x303F).contains(&u)
            || (0xFF00..=0xFFEF).contains(&u)
    });
    let arabe = hay(|u| {
        (0x0600..=0x06FF).contains(&u)
            || (0x0750..=0x077F).contains(&u)
            || (0xFB50..=0xFEFC).contains(&u)
    });
    let hebreo = hay(|u| (0x0590..=0x05FF).contains(&u));
    let devanagari = hay(|u| (0x0900..=0x097F).contains(&u));
    let thai = hay(|u| (0x0E00..=0x0E7F).contains(&u));
    let japones = kana || idioma.map(|l| l.starts_with("ja")).unwrap_or(false);
    let coreano = hangul || idioma.map(|l| l.starts_with("ko")).unwrap_or(false);

    // Primero las letras PROPIAS de cada escritura (todas), luego las
    // generalistas: así una «Arial» que lo cubre todo a medias no se
    // adelanta a la Kohinoor del devanagari ni a la Mincho del japonés.
    // (Ojo: macOS guarda los nombres en NFD; nada de kana con dakuten aquí.)
    let mut p: Vec<&'static str> = Vec::new();
    if japones && (han || kana) {
        p.extend([
            "明朝",
            "hiraginomincho",
            "notoserifcjkjp",
            "notoserifcjk",
            "yumincho",
            "notosanscjkjp",
            "notosanscjk",
            "角",
            "yugothic",
            "msmincho",
            "meiryo",
        ]);
    }
    if coreano {
        p.extend([
            "applemyungjo",
            "notoserifcjkkr",
            "notoserifcjk",
            "applesdgothicneo",
            "notosanscjkkr",
            "notosanscjk",
            "nanummyeongjo",
            "malgun",
        ]);
    }
    if han {
        p.extend([
            "songti",
            "stsong",
            "notoserifcjksc",
            "notoserifcjk",
            "pingfang",
            "hiraginosansgb",
            "notosanscjksc",
            "notosanscjk",
            "simsun",
            "msyh",
            "stheiti",
        ]);
    }
    if arabe {
        p.extend([
            "sfarabic",
            "notonaskharabic",
            "notosansarabic",
            "geezapro",
            "damascus",
            "segoeui",
        ]);
    }
    if hebreo {
        p.extend([
            "sfhebrew",
            "notoserifhebrew",
            "notosanshebrew",
            "arialhb",
            "segoeui",
        ]);
    }
    if devanagari {
        p.extend([
            "kohinoor",
            "notoserifdevanagari",
            "notosansdevanagari",
            "itfdevanagari",
            "devanagarimt",
            "nirmala",
            "mangal",
        ]);
    }
    if thai {
        p.extend([
            "notoserifthai",
            "notosansthai",
            "thonburi",
            "sukhumvit",
            "leelawad",
            "tahoma",
        ]);
    }
    // Lo general: símbolos, flechas, letras sueltas de otras escrituras.
    p.extend([
        "notoserif",
        "notosans",
        "sfns",
        "dejavuserif",
        "dejavusans",
        "applesymbols",
        "arialunicode",
        "timesnewroman",
        "times",
        "arial",
        "helvetica",
        "segoeuisymbol",
        "seguisym",
        "symbol",
        "notocoloremoji",
        "seguiemj",
        "emoji",
    ]);
    p
}

fn normaliza(nombre: &str) -> String {
    nombre
        .chars()
        .filter(|c| !matches!(c, ' ' | '-' | '_' | '.'))
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Busca en el sistema letras para lo que las propias no cubren. Devuelve
/// las fuentes en orden de preferencia (cada una cubre algo que faltaba).
/// Tope de lectura para no recorrer medio disco en un aparato modesto.
pub fn respaldo(faltan_ini: &BTreeSet<char>, idioma: Option<&str>) -> Vec<Fuente> {
    let mut faltan: BTreeSet<char> = faltan_ini
        .iter()
        .copied()
        .filter(|c| !sin_glifo(*c))
        .collect();
    if faltan.is_empty() {
        return Vec::new();
    }
    let mut ficheros: Vec<(PathBuf, u64)> = Vec::new();
    for c in carpetas_del_sistema() {
        listar(&c, 0, &mut ficheros);
    }
    let prefs = preferencias(&faltan, idioma);
    // Puntuación: posición de la primera preferencia que aparece en el
    // nombre; lo que no coincide va al final, por tamaño (los pequeños
    // primero: se leen antes). Los gigantes (>60 MB, p. ej. Apple Color
    // Emoji) se descartan: no caben en la memoria de un móvil modesto.
    let mut puntuados: Vec<(usize, u64, PathBuf)> = ficheros
        .into_iter()
        .filter(|(_, tam)| *tam > 0 && *tam <= 60 * 1024 * 1024)
        .map(|(p, tam)| {
            let n = normaliza(
                &p.file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default(),
            );
            let punt = prefs
                .iter()
                .position(|f| n.contains(&normaliza(f)))
                .unwrap_or(10_000);
            // Dentro de la misma preferencia, la redonda antes que la negrita
            // o la cursiva («Times New Roman Bold Italic.ttf» no, gracias).
            let estilo = [
                "bold",
                "italic",
                "oblique",
                "black",
                "heavy",
                "light",
                "thin",
                "medium",
                "semibold",
                "condensed",
                "rounded",
                "mono",
            ]
            .iter()
            .filter(|e| n.contains(*e))
            .count();
            (punt * 16 + estilo.min(15), tam, p)
        })
        .collect();
    puntuados.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

    let mut elegidas: Vec<Fuente> = Vec::new();
    let mut leidos: u64 = 0;
    let mut desconocidos = 0usize;
    for (punt, tam, ruta) in puntuados {
        if faltan.is_empty() {
            break;
        }
        if punt >= 10_000 * 16 {
            // Fuera de las preferencias: solo un puñado y con tope de bytes.
            desconocidos += 1;
            if desconocidos > 80 || leidos > 160 * 1024 * 1024 {
                break;
            }
        }
        let Ok(bytes) = std::fs::read(&ruta) else {
            continue;
        };
        leidos += tam;
        let datos = Arc::new(bytes);
        let n = ttf_parser::fonts_in_collection(&datos).unwrap_or(1);
        // Dentro de una colección, la cara más cercana a «regular».
        let mut caras: Vec<(u32, i32)> = Vec::new();
        for i in 0..n {
            if let Ok(f) = ttf_parser::Face::parse(&datos, i) {
                let peso = f.weight().to_number() as i32;
                let cursiva = if f.is_italic() { 1000 } else { 0 };
                caras.push((i, (peso - 400).abs() + cursiva));
            }
        }
        caras.sort_by_key(|c| c.1);
        for (i, _) in caras {
            let Ok(f) = ttf_parser::Face::parse(&datos, i) else {
                continue;
            };
            let cubiertas: Vec<char> = faltan.iter().copied().filter(|c| cubre(&f, *c)).collect();
            if cubiertas.is_empty() {
                continue;
            }
            for c in &cubiertas {
                faltan.remove(c);
            }
            let nombre = format!(
                "{}#{i}",
                ruta.file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default()
            );
            elegidas.push(Fuente {
                datos: Datos::Prestados(datos.clone()),
                indice: i,
                nombre,
            });
            if faltan.is_empty() {
                break;
            }
        }
    }
    if !faltan.is_empty() {
        let muestra: String = faltan.iter().take(12).collect();
        tracing::warn!(
            "[exportar] sin letra en el sistema para {} caracteres (p. ej. «{muestra}»)",
            faltan.len()
        );
    }
    elegidas
}
