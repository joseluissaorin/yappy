//! LA IMPRENTA DE MARKDOWN: el documento en texto plano con forma, para
//! notas y editores. La marca de agua es una línea final con enlace.

use super::documento::{Clase, Documento, Marca, URL_WEB, WEB};

pub fn markdown(doc: &Documento, marca: &Marca) -> String {
    let mut s =
        String::with_capacity(doc.bloques.iter().map(|b| b.texto.len() + 4).sum::<usize>() + 512);
    s.push_str("# ");
    s.push_str(&una_linea(&doc.titulo));
    s.push_str("\n\n");

    let mut cabecera: Vec<String> = Vec::new();
    if let Some(a) = doc.autor.as_deref().filter(|a| !a.trim().is_empty()) {
        cabecera.push(format!("*{}*", escapar_en_linea(a.trim())));
    }
    if let Some(u) = doc.url.as_deref().filter(|u| !u.trim().is_empty()) {
        cabecera.push(format!("<{}>", u.trim()));
    }
    cabecera.push(escapar_en_linea(&marca.exportado));
    // Dos espacios al final: salto de línea duro, el bloque queda junto.
    s.push_str(&cabecera.join("  \n"));
    s.push_str("\n\n---\n\n");

    let mut en_lista = false;
    for b in &doc.bloques {
        if en_lista && b.clase != Clase::Lista {
            s.push('\n');
        }
        en_lista = b.clase == Clase::Lista;
        match b.clase {
            Clase::Titulo(n) => {
                // El título del documento es el único «#»: los de dentro bajan uno.
                let nivel = (n + 1).min(6) as usize;
                s.push_str(&"#".repeat(nivel));
                s.push(' ');
                s.push_str(&una_linea(&b.texto));
                s.push_str("\n\n");
            }
            Clase::Parrafo => {
                s.push_str(&escapar_bloque(&b.texto));
                s.push_str("\n\n");
            }
            Clase::Cita => {
                for linea in b.texto.lines() {
                    s.push_str("> ");
                    s.push_str(&escapar_bloque(linea));
                    s.push('\n');
                }
                s.push('\n');
            }
            Clase::Lista => {
                s.push_str("- ");
                s.push_str(&escapar_bloque(&una_linea(&b.texto)));
                s.push('\n');
            }
            Clase::Separador => s.push_str("---\n\n"),
            Clase::Codigo => {
                let valla = if b.texto.contains("```") {
                    "~~~~"
                } else {
                    "```"
                };
                s.push_str(valla);
                s.push('\n');
                s.push_str(&b.texto);
                s.push('\n');
                s.push_str(valla);
                s.push_str("\n\n");
            }
            Clase::Verso => {
                let lineas: Vec<String> = b.texto.lines().map(escapar_bloque).collect();
                s.push_str(&lineas.join("  \n"));
                s.push_str("\n\n");
            }
        }
    }
    if en_lista {
        s.push('\n');
    }
    s.push_str("---\n\n");
    s.push_str(&format!(
        "*{} · [{WEB}]({URL_WEB})*\n",
        escapar_en_linea(&marca.leido_con)
    ));
    s
}

fn una_linea(t: &str) -> String {
    t.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Lo que al principio de un párrafo cambiaría su naturaleza en Markdown
/// (un «# » que no es título, un «1. » que no es lista…) lleva barra.
fn escapar_bloque(t: &str) -> String {
    let t = t.trim_start();
    let mut chars = t.chars();
    let primero = chars.next();
    let necesita = match primero {
        Some('#' | '>' | '-' | '+' | '*' | '=' | '|' | '`' | '~') => true,
        Some(c) if c.is_ascii_digit() => {
            let resto: String = t
                .chars()
                .skip_while(|c| c.is_ascii_digit())
                .take(2)
                .collect();
            resto.starts_with(". ") || resto.starts_with(") ")
        }
        _ => false,
    };
    if !necesita {
        return t.to_string();
    }
    if primero.is_some_and(|c| c.is_ascii_digit()) {
        // «1984. Fue un año…» → «1984\. Fue un año…»
        let cifras: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
        return format!("{cifras}\\{}", &t[cifras.len()..]);
    }
    format!("\\{t}")
}

/// En una línea de cabecera los asteriscos y guiones bajos sueltos se
/// escapan para no abrir énfasis por accidente.
fn escapar_en_linea(t: &str) -> String {
    let mut s = String::with_capacity(t.len());
    for c in t.chars() {
        if matches!(c, '*' | '_' | '[' | ']' | '`') {
            s.push('\\');
        }
        s.push(c);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exportar::documento::Documento;

    #[test]
    fn forma_y_marca() {
        let mut d = Documento::nuevo(
            "Título",
            &[
                "Capítulo uno".into(),
                "Un párrafo con ñ.".into(),
                "uno".into(),
                "dos".into(),
                "Cita".into(),
                "".into(),
                "1984. Fue un año".into(),
            ],
            &[
                "heading1".into(),
                "paragraph".into(),
                "list".into(),
                "list".into(),
                "quote".into(),
                "hr".into(),
                "paragraph".into(),
            ],
        );
        d.autor = Some("Quiroga".into());
        let md = markdown(
            &d,
            &Marca {
                leido_con: "Leído con Yappy".into(),
                exportado: "Exportado el 21 de octubre de 2026".into(),
            },
        );
        assert!(md.starts_with("# Título\n\n*Quiroga*  \nExportado el 21 de octubre de 2026\n\n---\n\n## Capítulo uno\n\n"));
        assert!(md.contains("- uno\n- dos\n\n> Cita\n"));
        assert!(md.contains("1984\\. Fue un año"));
        assert!(md.trim_end().ends_with(
            "*Leído con Yappy · [yappy.joseluissaorin.com](https://yappy.joseluissaorin.com)*"
        ));
    }
}
