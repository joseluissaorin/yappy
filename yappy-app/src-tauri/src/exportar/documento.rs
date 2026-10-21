//! EL DOCUMENTO QUE SALE: lo que Yappy tiene de una pieza, tal cual, con la
//! clase de cada párrafo (títulos, citas, listas, separadores, código,
//! versos). Las tres imprentas (Markdown, HTML, PDF) beben de aquí y no
//! saben nada de Tauri: se ensayan sin la app.

/// La web de la casa: la marca de agua la cita en las tres salidas.
pub const WEB: &str = "yappy.joseluissaorin.com";
pub const URL_WEB: &str = "https://yappy.joseluissaorin.com";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clase {
    Parrafo,
    /// Nivel 1..=6 tal como lo trae el documento (el título del documento
    /// va aparte, por encima de todos).
    Titulo(u8),
    Cita,
    Lista,
    Separador,
    Codigo,
    Verso,
}

impl Clase {
    /// Las clases de `RichParagraph::kind` (doc_loader) y del guion.
    pub fn desde(kind: &str) -> Self {
        match kind {
            "heading1" => Clase::Titulo(1),
            "heading2" => Clase::Titulo(2),
            "heading3" => Clase::Titulo(3),
            "heading4" => Clase::Titulo(4),
            "heading5" => Clase::Titulo(5),
            "heading6" => Clase::Titulo(6),
            "quote" => Clase::Cita,
            "list" => Clase::Lista,
            "hr" => Clase::Separador,
            "code" => Clase::Codigo,
            "verse" => Clase::Verso,
            _ => Clase::Parrafo,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Bloque {
    pub clase: Clase,
    pub texto: String,
}

#[derive(Debug, Clone, Default)]
pub struct Documento {
    pub titulo: String,
    pub autor: Option<String>,
    /// La URL de origen (artículos, cuentos con fuente).
    pub url: Option<String>,
    /// Código de idioma (es, en, ja…) si se sabe: `lang` del HTML, el PDF
    /// y la preferencia de letra para el han (japonés frente a chino).
    pub idioma: Option<String>,
    pub bloques: Vec<Bloque>,
}

/// Los textos de la marca, ya traducidos por el frontend (31 idiomas) y
/// con la fecha ya escrita a la manera de cada lengua.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Marca {
    /// «Leído con Yappy».
    pub leido_con: String,
    /// «Exportado el 21 de octubre de 2026».
    pub exportado: String,
}

impl Documento {
    /// Monta el documento desde los arrays paralelos que ya viajan por la
    /// app (`paragraphs` + `paragraph_kinds`). Limpia lo que no se ve
    /// (párrafos vacíos, separadores en fila o en los bordes) y quita el
    /// primer título si repite el título del documento.
    pub fn nuevo(titulo: &str, parrafos: &[String], kinds: &[String]) -> Self {
        let mut bloques: Vec<Bloque> = Vec::with_capacity(parrafos.len());
        for (i, p) in parrafos.iter().enumerate() {
            let clase = Clase::desde(kinds.get(i).map(String::as_str).unwrap_or("paragraph"));
            let texto = limpiar(p, clase);
            if clase == Clase::Separador {
                if bloques
                    .last()
                    .map(|b| b.clase != Clase::Separador)
                    .unwrap_or(false)
                {
                    bloques.push(Bloque {
                        clase,
                        texto: String::new(),
                    });
                }
                continue;
            }
            if texto.trim().is_empty() {
                continue;
            }
            bloques.push(Bloque { clase, texto });
        }
        while bloques
            .last()
            .map(|b| b.clase == Clase::Separador)
            .unwrap_or(false)
        {
            bloques.pop();
        }
        let titulo = titulo.trim().to_string();
        if let Some(primero) = bloques.first() {
            if matches!(primero.clase, Clase::Titulo(_)) && mismo_texto(&primero.texto, &titulo) {
                bloques.remove(0);
            }
        }
        while bloques
            .first()
            .map(|b| b.clase == Clase::Separador)
            .unwrap_or(false)
        {
            bloques.remove(0);
        }
        Documento {
            titulo,
            bloques,
            ..Default::default()
        }
    }

    /// Todo el texto que se va a imprimir (para elegir letras de respaldo).
    pub fn todo_el_texto(&self, marca: &Marca) -> String {
        let mut s = String::with_capacity(4096);
        s.push_str(&self.titulo);
        for extra in [self.autor.as_deref(), self.url.as_deref()]
            .into_iter()
            .flatten()
        {
            s.push('\n');
            s.push_str(extra);
        }
        s.push('\n');
        s.push_str(&marca.leido_con);
        s.push('\n');
        s.push_str(&marca.exportado);
        for b in &self.bloques {
            s.push('\n');
            s.push_str(&b.texto);
        }
        s
    }
}

/// Normaliza el texto de un bloque: fuera los retornos de Windows y, salvo
/// en versos y código (donde el salto de línea es la forma), los saltos
/// sueltos se funden en espacios.
fn limpiar(texto: &str, clase: Clase) -> String {
    let t = texto.replace("\r\n", "\n").replace('\r', "\n");
    match clase {
        Clase::Verso | Clase::Codigo => t.trim_matches('\n').to_string(),
        _ => t.split_whitespace().collect::<Vec<_>>().join(" "),
    }
}

fn mismo_texto(a: &str, b: &str) -> bool {
    let norma = |s: &str| {
        s.chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(|c| c.to_lowercase())
            .collect::<String>()
    };
    let (a, b) = (norma(a), norma(b));
    !a.is_empty() && a == b
}

/// El nombre de fichero de la exportación: el título, sin los caracteres
/// que prohíben los sistemas de ficheros, con su extensión. Las tildes,
/// eñes y escrituras no latinas se quedan (los tres sistemas las aceptan).
pub fn nombre_fichero(titulo: &str, extension: &str) -> String {
    let limpio: String = titulo
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => ' ',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    let mut base = limpio.split_whitespace().collect::<Vec<_>>().join(" ");
    base = base.trim_matches('.').trim().to_string();
    // Sin extensiones de origen colgando («cuento.epub.pdf»).
    for ext in [
        ".epub", ".pdf", ".docx", ".md", ".txt", ".html", ".htm", ".yappy",
    ] {
        if base.to_lowercase().ends_with(ext) && base.len() > ext.len() {
            base.truncate(base.len() - ext.len());
        }
    }
    if base.chars().count() > 80 {
        base = base.chars().take(80).collect::<String>().trim().to_string();
    }
    if base.is_empty() {
        base = "Yappy".into();
    }
    format!("{base}.{extension}")
}

/// La fecha de hoy (UTC) en ISO, sin depender de chrono.
pub fn hoy_iso() -> (u16, u8, u8, String) {
    let segs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (y, m, d) = civil_desde_dias(segs.div_euclid(86_400));
    (y as u16, m as u8, d as u8, format!("{y:04}-{m:02}-{d:02}"))
}

/// Howard Hinnant, «days_from_civil» al revés.
fn civil_desde_dias(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn quita_el_titulo_repetido_y_los_separadores_sobrantes() {
        let d = Documento::nuevo(
            "El papel pintado",
            &s(&["", "El Papel Pintado", "Uno", "", "", "Dos", ""]),
            &s(&["hr", "heading1", "paragraph", "hr", "hr", "paragraph", "hr"]),
        );
        let clases: Vec<_> = d.bloques.iter().map(|b| b.clase).collect();
        assert_eq!(
            clases,
            vec![Clase::Parrafo, Clase::Separador, Clase::Parrafo]
        );
    }

    #[test]
    fn nombres_de_fichero_con_tildes_y_sin_barras() {
        assert_eq!(
            nombre_fichero("A la deriva: un cuento", "pdf"),
            "A la deriva un cuento.pdf"
        );
        assert_eq!(nombre_fichero("niño/ñandú.epub", "md"), "niño ñandú.md");
        assert_eq!(nombre_fichero("  ", "html"), "Yappy.html");
        assert_eq!(nombre_fichero("蜘蛛の糸", "pdf"), "蜘蛛の糸.pdf");
    }

    #[test]
    fn fecha_civil() {
        assert_eq!(civil_desde_dias(0), (1970, 1, 1));
        assert_eq!(civil_desde_dias(20_747), (2026, 10, 21));
    }
}
